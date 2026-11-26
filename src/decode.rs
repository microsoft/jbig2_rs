use std::io::Read;

use crate::arithmetic::ArithmeticContext;
use crate::bitmap::CombinationOperator;
use crate::document::{
    parse_embedded_document, parse_standalone_document, read_bounded_input, Decoder,
    DecoderContext, DocumentInfo, GlobalSegments, PageInfo, ParsedData, ParsedSegment,
};
use crate::generic_region::{
    decode_arithmetic_refinement, decode_generic_region_segment_into_with_scratch,
    decode_generic_region_segment_reused_with_scratch, parse_generic_region_header,
    parse_refinement_region_header, GenericRegionError, GenericTemplate, RefinementParameters,
};
use crate::halftone::{
    decode_halftone_region, decode_pattern_dictionary, halftone_error_into_decode,
    new_halftone_state_from_scratch, new_pattern_arithmetic_state_from_scratch,
    new_pattern_mmr_state, parse_halftone_header, parse_pattern_dictionary_header, WorkBudget,
};
use crate::huffman::{HuffmanError, HuffmanTable};
use crate::mmr::MmrScratch;
use crate::reader::ByteReader;
use crate::segment_data::parse_region_info;
use crate::segment_header::SegmentType;
use crate::symbol_text::{
    decode_symbol_dictionary, decode_text_region, parse_symbol_dictionary_header,
    DictionaryContexts, SymbolCodebook, SymbolDictionaryInput, SymbolTextError, TextRegionInput,
};
use crate::{
    Bitmap, DecodeError, GenericRegionErrorKind, Limits, MalformedError, UnsupportedFeature,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/decode.rs"]
mod decode_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/decode.rs"]
mod decode_properties;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/decode.rs"]
mod decode_shared;

mod api;
mod dispatch;
mod orchestration;
mod rendering;
mod resources;

use dispatch::{
    decode_generic_segment, decode_halftone_segment, decode_pattern_segment,
    decode_refinement_segment, decode_symbol_dictionary_segment, decode_text_segment,
};
use orchestration::{decode_global_resources, decode_segments, decode_segments_collected};
pub(crate) use rendering::check_combination_operator;
use rendering::{
    compose_immediate, find_page_info, note_page_write, require_auxiliary_page,
    require_matching_geometry, seal_stripe, snapshot_region,
};
use resources::{
    allocation_size, bitmap_byte_len, get_huffman_table, get_patterns, get_region_buffer,
    get_runtime_page, get_symbol_resource, huffman_table_index, insert_buffer,
    insert_huffman_table, insert_patterns, insert_runtime_page, insert_symbol_resource,
    pattern_index, release_referred_buffers, release_referred_patterns, release_referred_symbols,
    release_referred_tables, runtime_page_index, structural_work, symbol_index,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) fn generic_error_into_decode(error: GenericRegionError) -> DecodeError {
    let kind = match error {
        GenericRegionError::Decode(error) => return error,
        GenericRegionError::ReservedFlags(flags) => GenericRegionErrorKind::ReservedFlags(flags),
        GenericRegionError::InvalidFlags(flags) => GenericRegionErrorKind::InvalidFlags(flags),
        GenericRegionError::InvalidAdaptivePixel { number, x, y } => {
            GenericRegionErrorKind::InvalidAdaptivePixel { number, x, y }
        }
        GenericRegionError::InvalidSkipDimensions => GenericRegionErrorKind::InvalidSkipDimensions,
        GenericRegionError::InvalidArithmeticTerminator => {
            GenericRegionErrorKind::InvalidArithmeticTerminator
        }
        GenericRegionError::UnexpectedArithmeticMarker { offset, code } => {
            GenericRegionErrorKind::UnexpectedArithmeticMarker { offset, code }
        }
        GenericRegionError::RowCountMismatch { expected, actual } => {
            GenericRegionErrorKind::RowCountMismatch { expected, actual }
        }
        GenericRegionError::ContextIndex { index, count } => {
            GenericRegionErrorKind::ContextIndex { index, count }
        }
    };
    DecodeError::Malformed(MalformedError::InvalidGenericRegion(kind))
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn symbol_text_error_into_decode(error: SymbolTextError) -> DecodeError {
    match error {
        SymbolTextError::Decode(error) => error,
        SymbolTextError::InvalidFlags(reason)
        | SymbolTextError::InvalidTableAssociation(reason)
        | SymbolTextError::InvalidCount(reason)
        | SymbolTextError::InvalidEntropyValue(reason)
        | SymbolTextError::UnexpectedOutOfBand(reason)
        | SymbolTextError::InvalidState(reason) => {
            DecodeError::Malformed(MalformedError::InvalidSymbolText(reason))
        }

        SymbolTextError::InvalidDimensions => {
            DecodeError::Malformed(MalformedError::InvalidSymbolDimensions)
        }
        SymbolTextError::SymbolIdOutOfRange { id, available } => {
            DecodeError::Malformed(MalformedError::SymbolIdOutOfRange { id, available })
        }
        SymbolTextError::InvalidSymbolLengthRun => DecodeError::Malformed(
            MalformedError::InvalidSymbolText("invalid symbol-code length run"),
        ),
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn huffman_error_into_decode(error: HuffmanError) -> DecodeError {
    match error {
        HuffmanError::Decode(error) => error,
        HuffmanError::InvalidTable(kind) => {
            DecodeError::Malformed(MalformedError::InvalidHuffman(kind))
        }
    }
}

/// One fully decoded page.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Page {
    pub(crate) number: u32,
    pub(crate) bitmap: Bitmap,
}

impl Page {
    /// One-based page number.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::decoded_page_number(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn number(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_decoded_page_fields!(self);
        self.number
    }

    /// Decoded page bitmap.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == decode_spec::page_bitmap(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn bitmap(&self) -> &Bitmap {
        #[cfg(verus_keep_ghost)]
        production_decode_page_fields!(self);
        &self.bitmap
    }

    /// Consumes the page and returns its bitmap.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            crate::views::bitmap_width(&out)
                == crate::views::bitmap_width(decode_spec::page_bitmap(&self)),
            crate::views::bitmap_height(&out)
                == crate::views::bitmap_height(decode_spec::page_bitmap(&self)),
            crate::views::bitmap_stride(&out)
                == crate::views::bitmap_stride(decode_spec::page_bitmap(&self)),
            crate::views::bitmap_data(&out)
                == crate::views::bitmap_data(decode_spec::page_bitmap(&self)),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn into_bitmap(self) -> Bitmap {
        #[cfg(verus_keep_ghost)]
        {
            production_decode_page_fields!(&self);
            proof! {
                crate::views::bitmap_fields(&self.bitmap);
            }
        }
        self.bitmap
    }
}

/// Fully collected decoded document.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct DecodedDocument {
    info: DocumentInfo,
    pages: Vec<Page>,
}

impl DecodedDocument {
    /// Validated document metadata.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == decode_spec::document_info(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn info(&self) -> &DocumentInfo {
        #[cfg(verus_keep_ghost)]
        production_decode_document_fields!(self);
        &self.info
    }

    /// Decoded pages in stream order.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out@ == decode_spec::document_pages(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn pages(&self) -> &[Page] {
        #[cfg(verus_keep_ghost)]
        production_decode_document_fields!(self);
        &self.pages
    }
}

/// Streaming destination for completed pages.
pub trait PageSink {
    /// Receives ownership of one completed page.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the sink cannot accept the page.
    fn write_page(&mut self, page: Page) -> Result<(), DecodeError>;
}

impl<F> PageSink for F
where
    F: FnMut(Page) -> Result<(), DecodeError>,
{
    fn write_page(&mut self, page: Page) -> Result<(), DecodeError> {
        self(page)
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct PageCollector {
    pages: Vec<Page>,
    bitmap_bytes: u64,
    allocated_bitmap_bytes: u64,
    memory_limit: u64,
    decoded_limit: u64,
}

impl PageCollector {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            decode_spec::collector_pages(&out).len() == 0,
            decode_spec::collector_bitmap_bytes(&out) == 0,
            decode_spec::collector_memory_limit(&out)
                == limits.max_decoder_managed_memory_bytes,
            decode_spec::collector_decoded_limit(&out)
                == limits.max_cumulative_decoded_bitmap_bytes,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    const fn new(limits: &Limits) -> Self {
        let out = Self {
            pages: Vec::new(),
            bitmap_bytes: 0,
            allocated_bitmap_bytes: 0,
            memory_limit: limits.max_decoder_managed_memory_bytes,
            decoded_limit: limits.max_cumulative_decoded_bitmap_bytes,
        };
        #[cfg(verus_keep_ghost)]
        production_collector_fields!(&out);
        out
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures decode_spec::collector_result(old(self), final(self), page, out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn collect_page(&mut self, page: Page) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        {
            production_collector_fields!(self);
            production_decode_page_fields!(&page);
        }
        let page_bytes = bitmap_byte_len(&page.bitmap)?;
        let next_bytes = self
            .bitmap_bytes
            .checked_add(page_bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        if next_bytes > self.decoded_limit {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::DecodedBitmapBytes,
                limit: self.decoded_limit,
                actual: next_bytes,
            }
            .into());
        }
        if next_bytes > self.memory_limit {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::Memory,
                limit: self.memory_limit,
                actual: next_bytes,
            }
            .into());
        }
        let next_allocated_bytes = self
            .allocated_bitmap_bytes
            .checked_add(page.bitmap.allocated_bytes())
            .ok_or(MalformedError::IntegerOverflow)?;
        if next_allocated_bytes > self.memory_limit {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::Memory,
                limit: self.memory_limit,
                actual: next_allocated_bytes,
            }
            .into());
        }
        let new_length = self
            .pages
            .len()
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        let requested_bytes = allocation_size::<Page>(new_length)?;
        self.pages
            .try_reserve(1)
            .map_err(|_error| DecodeError::Allocation { requested_bytes })?;
        self.bitmap_bytes = next_bytes;
        self.allocated_bitmap_bytes = next_allocated_bytes;
        self.pages.push(page);
        #[cfg(verus_keep_ghost)]
        production_collector_fields!(self);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures decode_spec::collector_result(old(self), final(self), page, out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn push_page(&mut self, page: Page) -> Result<(), DecodeError> {
        self.collect_page(page)
    }
}

#[cfg_attr(verus_keep_ghost, verifier::verify)]
impl PageSink for PageCollector {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures decode_spec::collector_result(old(self), final(self), page, out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn write_page(&mut self, page: Page) -> Result<(), DecodeError> {
        self.push_page(page)
    }
}

fn write_page_to_sink<S: PageSink>(sink: &mut S, page: Page) -> Result<(), DecodeError> {
    sink.write_page(page)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[allow(clippy::struct_excessive_bools)]
struct RuntimePage {
    default_pixel: bool,
    pristine: bool,
    default_operator: crate::bitmap::CombinationOperator,
    combination_override: bool,
    refinements_allowed: bool,
    auxiliary_allowed: bool,
    last_stripe_end: Option<u32>,
    open_stripe_max_row: Option<u32>,
    bitmap: Bitmap,
}

type RuntimePages = Vec<(u32, RuntimePage)>;

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct RegionBuffer {
    region: crate::segment_data::RegionInfo,
    bitmap: Bitmap,
}

type RegionBuffers = Vec<(u32, RegionBuffer)>;

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct SymbolResource {
    exported: Vec<Bitmap>,
    contexts: Option<DictionaryContexts>,
}

type SymbolStore = Vec<(u32, SymbolResource)>;

#[cfg_attr(verus_keep_ghost, verus_verify)]
enum CollectedSymbols<'a> {
    Contiguous(&'a [Bitmap]),
    Borrowed(Vec<&'a Bitmap>),
}

impl<'a> CollectedSymbols<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires decode_spec::collected_symbols_valid(self),
        ensures
            crate::symbol_text::symbol_text_spec::codebook_valid(out),
            crate::symbol_text::symbol_text_spec::codebook_len(out)
                == decode_spec::collected_symbol_count(self),
    ))]
    fn codebook(&'a self) -> SymbolCodebook<'a> {
        match self {
            Self::Contiguous(symbols) => SymbolCodebook::Contiguous(symbols),
            Self::Borrowed(symbols) => SymbolCodebook::Borrowed(symbols),
        }
    }
}

type HuffmanTables = Vec<(u32, HuffmanTable)>;

type PatternStore = Vec<(u32, Vec<Bitmap>)>;

#[derive(Default)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct InitialResources {
    patterns: PatternStore,
    symbols: SymbolStore,
    tables: HuffmanTables,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct RetainedBudget {
    bytes: u64,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct DecodeWork {
    used: u64,
}

#[cfg(test)]
mod tests;
