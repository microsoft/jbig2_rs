//! Explicit RGB presentation of supported generic colour regions.

use crate::{DecodeError, DecodePolicy, DecodeResult, Decoder};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

mod bitmap;
#[cfg(verus_keep_ghost)]
#[path = "../specs/colour.rs"]
mod colour_spec;
mod document;
mod palette;

pub use bitmap::RgbBitmap;

/// One completely decoded, opaque RGB page.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct ColourPage {
    pub(crate) number: u32,
    pub(crate) bitmap: RgbBitmap,
}

impl ColourPage {
    /// Original positive page association.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == colour_spec::page_number(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn number(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::page_fields(self); }
        self.number
    }

    /// Row-major RGB8 pixels.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == colour_spec::page_bitmap(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn bitmap(&self) -> &RgbBitmap {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::page_fields(self); }
        &self.bitmap
    }
}

/// Completely decoded pages in source page-information order.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct DecodedColourDocument {
    pub(crate) pages: Vec<ColourPage>,
}

impl DecodedColourDocument {
    /// Complete RGB pages; no partially rendered page is returned.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == colour_spec::pages(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn pages(&self) -> &[ColourPage] {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::document_fields(self); }
        &self.pages
    }
}

impl Decoder {
    /// Decodes the supported sequential generic-region RGB8 profile strictly.
    ///
    /// Existing bi-level entry points retain their colour `Unsupported` result.
    /// This byte-only profile supports consecutive known-height unstriped pages, immediate
    /// generic regions, RGB8 palettes, monochrome OR/REPLACE and transparent
    /// coloured REPLACE, without deferred retention. Other profiles return typed `Unsupported`.
    ///
    /// # Errors
    ///
    /// Returns typed framing, palette, entropy, resource or profile errors.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(output) => colour_spec::document_valid(&output),
            Err(_) => true,
        },
    ))]
    pub fn decode_rgb(&self, input: &[u8]) -> Result<DecodedColourDocument, DecodeError> {
        Ok(self
            .decode_rgb_with_policy(input, DecodePolicy::Strict)?
            .value)
    }

    /// Decodes RGB8 with explicit colour declaration-bit recovery.
    ///
    /// Compatibility may report missing file/page colour declarations only.
    /// It does not inherit the bi-level decoder's other recovery exceptions.
    /// No palette ID is clamped and no colour flag is discarded.
    ///
    /// # Errors
    ///
    /// Every region and page must complete; errors return no pages or recoveries.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(output) => colour_spec::document_valid(&output.value),
            Err(_) => true,
        },
    ))]
    pub fn decode_rgb_with_policy(
        &self,
        input: &[u8],
        policy: DecodePolicy,
    ) -> Result<DecodeResult<DecodedColourDocument>, DecodeError> {
        document::decode(input, policy, self.limits())
    }
}
