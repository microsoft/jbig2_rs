//! Exact output, strict scope, deferred errors and resource-policy regressions.

mod common;

use std::io::{self, Read};

use image::{ColorType, DynamicImage, ImageDecoder, ImageError, Limits};
use jbig2_decoder::{DecodeError, Decoder, LimitKind, MalformedError};
use jbig2_image::{DecoderOptions, Jbig2Decoder};

#[test]
fn direct_and_boxed_decode_preserve_polarity_bit_order_and_row_padding() {
    for sequential in [true, false] {
        for striped in [false, true] {
            let bytes = common::patterned(sequential, striped);
            let decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
            assert_eq!(decoder.dimensions(), (9, 3));
            assert_eq!(decoder.color_type(), ColorType::L8);
            assert_eq!(decoder.total_bytes(), 27);
            let image = DynamicImage::from_decoder(decoder).unwrap();
            assert_eq!(image.as_luma8().unwrap().as_raw(), &common::expected());
            let decoder: Box<dyn ImageDecoder> =
                Box::new(Jbig2Decoder::new(bytes.as_slice()).unwrap());
            assert_eq!(DynamicImage::from_decoder(decoder).unwrap(), image);
        }
    }
}

#[test]
fn rejects_multipage_empty_embedded_and_truncated_files_without_dropping_pages() {
    let bytes = common::standalone(
        true,
        Some(2),
        vec![
            common::page(0, 1, 9, 3, false),
            common::segment(1, 49, 1, vec![]),
            common::page(2, 2, 9, 3, false),
            common::segment(3, 49, 2, vec![]),
            common::segment(4, 51, 0, vec![]),
        ],
    );
    assert_eq!(Decoder::default().decode(&bytes).unwrap().pages().len(), 2);
    assert!(matches!(
        Jbig2Decoder::new(bytes.as_slice()),
        Err(ImageError::Unsupported(_))
    ));
    let empty = common::standalone(true, Some(0), vec![common::segment(0, 51, 0, vec![])]);
    assert!(Jbig2Decoder::new(empty.as_slice()).is_err());
    let bytes = common::patterned(true, false);
    assert!(matches!(
        Jbig2Decoder::new(&bytes[13..]),
        Err(ImageError::Decoding(_))
    ));
    assert!(matches!(
        Jbig2Decoder::new(&bytes[..bytes.len() - 1]),
        Err(ImageError::Decoding(_))
    ));
    let missing_end = common::standalone(
        true,
        Some(1),
        vec![
            common::page(0, 1, 9, 3, false),
            common::segment(1, 51, 0, vec![]),
        ],
    );
    let Err(ImageError::Decoding(error)) = Jbig2Decoder::new(missing_end.as_slice()) else {
        panic!("standalone page-end marker must remain required");
    };
    assert!(matches!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<DecodeError>(),
        Some(DecodeError::Malformed(MalformedError::MissingEndOfPage(1)))
    ));
}

#[test]
fn construction_defers_entropy_errors_and_pixel_output_is_not_returned_as_success() {
    let mut broken = common::white_region(1, 9, 0, 0);
    *broken.1.last_mut().unwrap() = 0;
    let bytes = common::standalone(
        true,
        Some(1),
        vec![
            common::page(0, 1, 9, 3, false),
            broken,
            common::segment(2, 49, 1, vec![]),
            common::segment(3, 51, 0, vec![]),
        ],
    );
    let decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
    assert_eq!(decoder.dimensions(), (9, 3));
    assert!(matches!(
        DynamicImage::from_decoder(decoder),
        Err(ImageError::Decoding(_))
    ));
}

#[test]
fn image_dimensions_and_remaining_memory_are_enforced_without_disabling_codec_caps() {
    let bytes = common::patterned(true, false);
    for (width, height) in [(Some(8), None), (None, Some(2))] {
        let mut decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
        let mut limits = Limits::no_limits();
        limits.max_image_width = width;
        limits.max_image_height = height;
        assert!(matches!(
            decoder.set_limits(limits),
            Err(ImageError::Limits(_))
        ));
    }
    let mut decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(bytes.len() as u64 - 1);
    assert!(matches!(
        decoder.set_limits(limits),
        Err(ImageError::Limits(_))
    ));
    let mut decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
    let mut limits = Limits::no_limits();
    limits.max_alloc = Some(bytes.len() as u64);
    decoder.set_limits(limits).unwrap();
    assert!(matches!(
        DynamicImage::from_decoder(decoder),
        Err(ImageError::Limits(_))
    ));

    let mut options = DecoderOptions::default();
    options.limits.max_width = 8;
    assert!(matches!(
        Jbig2Decoder::with_options(bytes.as_slice(), options),
        Err(ImageError::Limits(_))
    ));
    let mut options = DecoderOptions::default();
    options.limits.max_work_units = 1;
    let Err(ImageError::Decoding(error)) = Jbig2Decoder::with_options(bytes.as_slice(), options)
    else {
        panic!("work budget must remain enforced");
    };
    assert!(
        matches!(std::error::Error::source(&error).unwrap().downcast_ref::<DecodeError>(),
        Some(DecodeError::LimitExceeded(limit)) if limit.kind == LimitKind::Work)
    );
    let mut options = DecoderOptions::default();
    options.limits.max_decoder_managed_memory_bytes = bytes.len() as u64;
    let mut decoder = Jbig2Decoder::with_options(bytes.as_slice(), options).unwrap();
    decoder.set_limits(Limits::no_limits()).unwrap();
    assert!(matches!(
        DynamicImage::from_decoder(decoder),
        Err(ImageError::Limits(_))
    ));

    let mut decoder = Jbig2Decoder::new(bytes.as_slice()).unwrap();
    let mut limits = Limits::no_limits();
    limits.max_image_width = Some(9);
    limits.max_image_height = Some(3);
    limits.max_alloc = Some(1024 * 1024);
    decoder.set_limits(limits).unwrap();
    assert_eq!(
        DynamicImage::from_decoder(decoder)
            .unwrap()
            .into_luma8()
            .as_raw(),
        &common::expected()
    );
}

struct InterruptedOnce<R> {
    reader: R,
    interrupted: bool,
}

impl<R: Read> Read for InterruptedOnce<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.reader.read(buf)
    }
}

#[test]
fn input_is_capped_before_reading_with_one_probe_and_preserves_io_errors() {
    let bytes = common::patterned(true, false);
    let mut options = DecoderOptions::default();
    options.limits.max_stream_bytes = bytes.len() as u64;
    let reader = InterruptedOnce {
        reader: bytes.as_slice(),
        interrupted: false,
    };
    assert_eq!(
        DynamicImage::from_decoder(Jbig2Decoder::with_options(reader, options).unwrap())
            .unwrap()
            .into_luma8()
            .as_raw(),
        &common::expected()
    );
    let mut cursor = io::Cursor::new([bytes.as_slice(), &[1, 2, 3]].concat());
    assert!(matches!(
        Jbig2Decoder::with_options(&mut cursor, options),
        Err(ImageError::Limits(_))
    ));
    assert_eq!(cursor.position(), bytes.len() as u64 + 1);
    options.limits.max_stream_bytes = 0;
    let mut cursor = io::Cursor::new(&bytes);
    assert!(matches!(
        Jbig2Decoder::with_options(&mut cursor, options),
        Err(ImageError::Limits(_))
    ));
    assert_eq!(cursor.position(), 1);

    struct BrokenReader;
    impl Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::PermissionDenied.into())
        }
    }
    assert!(matches!(Jbig2Decoder::new(BrokenReader),
        Err(ImageError::IoError(error)) if error.kind() == io::ErrorKind::PermissionDenied));
}
