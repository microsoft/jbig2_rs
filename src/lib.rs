#![forbid(unsafe_code)]
#![cfg_attr(verus_keep_ghost, feature(proc_macro_hygiene))]
//! A dependency-free Rust decoder for JBIG2 black-and-white pages.
//!
//! **Development status:** this is an early release and is not yet suitable
//! for untrusted production documents. Implemented features are not a claim of
//! complete JBIG2 or Annex F conformance.
//!
//! # Decode a standalone file
//!
//! [`Decoder`] provides the ordinary Rust API; neither Verus nor the repository's
//! proof files are needed to build or use it. Reader entry points apply the
//! decoder's configured input limit while reading.
//!
//! ```no_run
//! use std::fs::File;
//! use jbig2_decoder::{Decoder, Limits};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let decoder = Decoder::new(Limits::default());
//! let mut input = File::open("document.jb2")?;
//! let document = decoder.decode_reader(&mut input)?;
//!
//! for page in document.pages() {
//!     let bitmap = page.bitmap();
//!     println!("page {}: {} x {}", page.number(), bitmap.width(), bitmap.height());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Choose the right entry point
//!
//! - [`Decoder`] inspects or decodes standalone documents from bytes or a reader.
//! - [`DecoderContext`] reuses scratch storage across calls; it can release that
//!   storage explicitly when reuse is no longer needed.
//! - [`GlobalSegments`] supports embedded JBIG2 streams with reusable globals,
//!   as used by PDF containers. This crate does not parse PDF documents.
//! - [`PageSink`] receives completed pages without requiring the caller to
//!   collect every decoded page in one [`DecodedDocument`]. Internal reference
//!   retention still follows the stream and configured limits.
//!
//! # Pixels, limits and errors
//!
//! [`Bitmap`] stores packed, row-major, one-bit pixels, with the leftmost pixel
//! in the most significant bit. Its stride is measured in bytes, not pixels.
//! Use [`Bitmap::pixel`] for bounds-checked access or [`Bitmap::bytes`] together
//! with the dimensions and stride for bulk processing.
//!
//! [`Limits`] is the place to set application-specific input, image, memory and
//! work policies. These policies are not an operating-system process-memory
//! limit and do not replace the crate's remaining release/safety review.
//! [`DecodeError`] distinguishes malformed or unsupported input, configured
//! limits, allocation failures and I/O errors; callers should handle those
//! explicitly rather than treating a rejected document as an empty image.
//!
//! # Implementation and verification
//!
//! The same Rust implementation is built by ordinary Cargo and by the pinned
//! verification toolchain. Detailed models and proof helpers live separately
//! in the repository; small `verus_keep_ghost` annotations attach them to the
//! implementation and are inactive in ordinary builds.

#[cfg(verus_keep_ghost)]
extern crate vstd;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/mod.rs"]
mod spec;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/readers.rs"]
mod reader_proofs;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/file_header.rs"]
mod file_header_proofs;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/bitmap.rs"]
mod bitmap_proofs;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/document.rs"]
mod document_proofs;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/decode.rs"]
mod decode_proofs;

#[allow(dead_code)]
mod arithmetic;
mod bitmap;
mod colour;
mod decode;
mod document;
mod error;
mod file_header;
#[allow(dead_code)]
mod generic_region;
#[allow(dead_code)]
mod halftone;
#[allow(dead_code)]
mod huffman;
mod limits;
#[allow(dead_code)]
mod mmr;
mod reader;
#[allow(dead_code)]
mod segment_data;
#[allow(dead_code)]
mod segment_header;
#[allow(dead_code)]
mod symbol_text;

pub use bitmap::Bitmap;
pub use colour::{ColourPage, DecodedColourDocument, RgbBitmap};
pub use decode::{DecodedDocument, Page, PageSink};
pub use document::embedded::{
    EmbeddedPolicy, EmbeddedRecovery, EmbeddedRecoveryKind, EmbeddedResult,
};
pub use document::standalone::{DecodePolicy, DecodeResult, Recovery, RecoveryKind};
pub use document::{Decoder, DecoderContext, DocumentInfo, GlobalSegments, PageInfo};
pub use error::{
    DecodeError, GenericRegionErrorKind, HalftoneErrorKind, HuffmanErrorKind, IoErrorKind,
    LimitExceeded, LimitKind, MalformedError, MmrErrorKind, UnsupportedFeature,
};
pub use file_header::{inspect_file_header, FileHeader, Organization};
pub use limits::Limits;

#[cfg(any(fuzzing, jbig2_validation))]
#[path = "../validation/runtime_access.rs"]
pub mod runtime_access;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/views.rs"]
mod views;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/invariants.rs"]
mod invariants;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/readers.rs"]
mod reader_properties;

#[cfg(verus_keep_ghost)]
extern crate jbig2_capacity_interface;
