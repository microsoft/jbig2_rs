#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::io::{self, Read};

use image::error::{
    DecodingError, ImageFormatHint, LimitError, LimitErrorKind, UnsupportedError,
    UnsupportedErrorKind,
};
use image::{ColorType, ImageDecoder, ImageError, ImageResult, LimitSupport, Limits};
use jbig2_decoder::{DecodeError, Decoder, LimitExceeded, LimitKind};

mod hooks;

pub use hooks::{register_decoding_hooks, HookRegistrationError};
pub use jbig2_decoder::Limits as DecoderLimits;

/// Codec limits applied to input buffering, inspection and pixel decoding.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecoderOptions {
    /// Bounds retained even when `image` limits are disabled.
    pub limits: DecoderLimits,
}

/// Deferred strict decoder for exactly one standalone page.
///
/// Construction buffers bounded compressed bytes and inspects the full envelope,
/// without decoding entropy data. Embedded streams, page selection, animation,
/// PDF parsing and best-effort recovery are deliberately not exposed.
pub struct Jbig2Decoder {
    input: Vec<u8>,
    options: DecoderOptions,
    dimensions: (u32, u32),
    image_limits: Limits,
}

impl Jbig2Decoder {
    /// Inspect input with the core decoder's default resource limits.
    ///
    /// # Errors
    ///
    /// Returns input, envelope, resource or unsupported-page-count errors.
    pub fn new(reader: impl Read) -> ImageResult<Self> {
        Self::with_options(reader, DecoderOptions::default())
    }

    /// Apply codec limits before buffering or inspection.
    ///
    /// `ImageReader` installs its limits only after this constructor returns.
    ///
    /// # Errors
    ///
    /// Returns I/O, allocation, resource, envelope or unsupported-page-count errors.
    pub fn with_options(reader: impl Read, options: DecoderOptions) -> ImageResult<Self> {
        let input = read_input(reader, &options.limits)?;
        let mut decoder = Self {
            input,
            options,
            dimensions: (0, 0),
            image_limits: Limits::no_limits(),
        };
        let codec = Decoder::new(decoder.codec_limits(&decoder.image_limits)?);
        let info = codec.inspect(&decoder.input).map_err(codec_error)?;
        if info.pages().len() != 1 {
            return Err(unsupported("exactly one standalone JBIG2 page is required"));
        }
        let page = &info.pages()[0];
        decoder.dimensions = (page.width(), page.height());
        usize::try_from(decoder.total_bytes()).map_err(|_| memory_limit())?;
        Ok(decoder)
    }

    fn codec_limits(&self, image_limits: &Limits) -> ImageResult<DecoderLimits> {
        image_limits.check_dimensions(self.dimensions.0, self.dimensions.1)?;
        let mut remaining = image_limits.clone();
        // ImageReader already reserves the caller-owned L8 output.
        remaining.reserve_usize(self.input.capacity())?;
        let mut limits = self.options.limits;
        limits.max_decoder_managed_memory_bytes = limits
            .max_decoder_managed_memory_bytes
            .checked_sub(self.input.capacity() as u64)
            .ok_or_else(memory_limit)?;
        if let Some(bytes) = remaining.max_alloc {
            limits.max_decoder_managed_memory_bytes =
                limits.max_decoder_managed_memory_bytes.min(bytes);
        }
        Ok(limits)
    }
}

impl ImageDecoder for Jbig2Decoder {
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn color_type(&self) -> ColorType {
        ColorType::L8
    }

    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&LimitSupport::default())?;
        self.codec_limits(&limits)?;
        self.image_limits = limits;
        Ok(())
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(buf.len() as u64, self.total_bytes());
        let codec = Decoder::new(self.codec_limits(&self.image_limits)?);
        let document = codec.decode(&self.input).map_err(codec_error)?;
        if document.pages().len() != 1 {
            return Err(invalid_output());
        }
        let bitmap = document.pages()[0].bitmap();
        if (bitmap.width(), bitmap.height()) != self.dimensions {
            return Err(invalid_output());
        }
        let width = usize::try_from(bitmap.width()).map_err(|_| memory_limit())?;
        for (row, packed) in buf
            .chunks_exact_mut(width)
            .zip(bitmap.bytes().chunks_exact(bitmap.stride()))
        {
            for (x, sample) in row.iter_mut().enumerate() {
                *sample = if packed[x / 8] & (0x80 >> (x % 8)) == 0 {
                    255
                } else {
                    0
                };
            }
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

fn read_input(mut reader: impl Read, limits: &DecoderLimits) -> ImageResult<Vec<u8>> {
    let maximum = limits
        .max_stream_bytes
        .min(limits.max_decoder_managed_memory_bytes);
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let remaining = maximum - bytes.len() as u64;
        let length = if remaining < chunk.len() as u64 {
            remaining as usize
        } else {
            chunk.len()
        };
        let count = match reader.read(&mut chunk[..length.max(1)]) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ImageError::IoError(error)),
        };
        if remaining == 0 {
            let kind = if maximum == limits.max_stream_bytes {
                LimitKind::StreamBytes
            } else {
                LimitKind::Memory
            };
            return Err(codec_error(DecodeError::LimitExceeded(LimitExceeded {
                kind,
                limit: maximum,
                actual: maximum.saturating_add(1),
            })));
        }
        bytes.try_reserve_exact(count).map_err(|_| memory_limit())?;
        if bytes.capacity() as u64 > limits.max_decoder_managed_memory_bytes {
            return Err(memory_limit());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}

fn format_hint() -> ImageFormatHint {
    ImageFormatHint::Name("JBIG2".into())
}

fn unsupported(message: impl Into<String>) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        format_hint(),
        UnsupportedErrorKind::GenericFeature(message.into()),
    ))
}

fn memory_limit() -> ImageError {
    ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
}

fn invalid_output() -> ImageError {
    ImageError::Decoding(DecodingError::new(
        format_hint(),
        "unexpected JBIG2 page layout",
    ))
}

fn codec_error(error: DecodeError) -> ImageError {
    match error {
        DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Width | LimitKind::Height | LimitKind::PagePixels,
            ..
        }) => ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError)),
        DecodeError::LimitExceeded(LimitExceeded {
            kind:
                LimitKind::Memory
                | LimitKind::StreamBytes
                | LimitKind::RetainedState
                | LimitKind::DecodedBitmapBytes,
            ..
        })
        | DecodeError::Allocation { .. } => memory_limit(),
        DecodeError::Unsupported(_) => unsupported(error.to_string()),
        _ => ImageError::Decoding(DecodingError::new(format_hint(), error)),
    }
}
