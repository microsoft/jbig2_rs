use std::io::Read;

use crate::file_header::{inspect_file_header, Organization};
use crate::huffman::{HuffmanError, HuffmanTable};
use crate::mmr::MmrScratch;
use crate::reader::ByteReader;
use crate::segment_data::{
    parse_end_of_stripe, parse_extension, parse_page_information, parse_profiles, require_empty,
    PageInformation,
};
use crate::segment_header::{parse_segment_header, SegmentHeader, SegmentType};
use crate::{DecodeError, LimitExceeded, LimitKind, Limits, MalformedError, UnsupportedFeature};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/document.rs"]
mod document_spec;
#[cfg(verus_keep_ghost)]
pub(crate) use document_spec::{
    decoder_context_fields, decoder_context_finished, decoder_context_state, embedded_metadata,
    standalone_metadata,
};

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/document.rs"]
mod document_properties;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/document.rs"]
mod document_shared;

pub(crate) mod embedded;
mod input;
mod pages;
mod parsing;
mod references;

use input::{
    allocation_size, enforce_input_work, enforce_page_limit, enforce_segment_limit,
    enforce_stream_limit, enforce_work_limit,
};
use pages::{build_document_info, validate_profiles};
use parsing::parse_sequential;
use references::validate_references;

pub(crate) use input::read_bounded_input;
pub(crate) use parsing::{parse_embedded_document, parse_standalone_document};

type SegmentTypes = Vec<(u32, SegmentType)>;
type SegmentDepths = Vec<(u32, u32)>;
type SegmentIndexes = Vec<(u32, usize)>;
type PageStates = Vec<(u32, PageState)>;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < entries@.len()
            && entries@[index as int].0 == number
            && forall|prior: int| 0 <= prior < index ==> entries@[prior].0 != number,
        None =>
            forall|index: int| 0 <= index < entries@.len() ==>
                entries@[index].0 != number,
    },
))]
fn keyed_index<T>(entries: &[(u32, T)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= entries@.len(),
            forall|completed: int| 0 <= completed < index ==>
                entries@[completed].0 != number,
        decreases entries@.len() - index,
    ))]
    while index < entries.len() {
        if entries[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(value) => exists|index: int|
            0 <= index < entries@.len()
            && entries@[index].0 == number
            && *value == entries@[index].1
            && forall|prior: int| 0 <= prior < index ==> entries@[prior].0 != number,
        None => forall|index: int| 0 <= index < entries@.len() ==> entries@[index].0 != number,
    },
))]
fn keyed_get<T>(entries: &[(u32, T)], number: u32) -> Option<&T> {
    let index = keyed_index(entries, number)?;
    Some(&entries[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    ensures final(entries)@.len() == old(entries)@.len(),
))]
fn keyed_get_mut<T>(entries: &mut [(u32, T)], number: u32) -> Option<&mut T> {
    let index = keyed_index(entries, number)?;
    Some(&mut entries[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures if out {
        final(entries)@ == old(entries)@.push((number, value))
    } else {
        final(entries)@ == old(entries)@
    },
))]
fn keyed_insert<T>(entries: &mut Vec<(u32, T)>, number: u32, value: T) -> bool {
    if keyed_index(entries, number).is_some() {
        return false;
    }
    entries.push((number, value));
    true
}

/// High-level decoder configuration.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Decoder {
    limits: Limits,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Clone for Decoder {
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        ensures(|out: Self| {
            equal(
                document_spec::decoder_limits(&out),
                document_spec::decoder_limits(self),
            )
        });
        let out = Self {
            limits: self.limits,
        };
        #[cfg(verus_keep_ghost)]
        {
            production_decoder_fields!(self);
            production_decoder_fields!(&out);
        }
        out
    }
}

/// Reusable decoding state for repeated documents with one [`Decoder`].
///
/// The context retains only allocation capacity, never document semantics.
/// Every operation clears scratch lengths after success or error. Call
/// [`Self::reset`] to clear scratch while retaining capacity, or
/// [`Self::release_scratch`] to return all retained allocations.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct DecoderContext<'a> {
    pub(crate) decoder: &'a Decoder,
    pub(crate) mmr: MmrScratch,
}

impl Decoder {
    /// Creates reusable scratch state tied to this decoder's limits.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            document_spec::decoder_context_state(&out).0 == document_spec::decoder_limits(self),
            document_spec::decoder_context_state(&out).1.len() == 0,
            document_spec::decoder_context_state(&out).2.len() == 0,
            document_spec::decoder_context_state(&out).3 == 0,
    ))]
    pub fn context(&self) -> DecoderContext<'_> {
        let out = DecoderContext {
            decoder: self,
            mmr: MmrScratch::default(),
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::mmr::scratch_view_definition();
            document_spec::decoder_context_fields();
        }
        out
    }

    /// Creates a decoder using the supplied resource limits.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures document_spec::decoder_limits(&out) == limits,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn new(limits: Limits) -> Self {
        #[cfg(verus_keep_ghost)]
        production_document_policy_bundle!();
        let out = Self { limits };
        #[cfg(verus_keep_ghost)]
        production_decoder_fields!(&out);
        out
    }
}

impl DecoderContext<'_> {
    /// Returns the limits governing this context.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures *out == document_spec::decoder_context_state(self).0,
    ))]
    pub const fn limits(&self) -> &Limits {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        self.decoder.limits()
    }

    /// Exact bytes currently retained by reusable allocation capacities.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == document_spec::decoder_context_state(self).3,
    ))]
    pub fn retained_scratch_bytes(&self) -> u64 {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        self.mmr.retained_bytes()
    }

    /// Clears all per-document scratch contents while retaining capacity.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            document_spec::decoder_context_state(final(self)).0 == document_spec::decoder_context_state(old(self)).0,
            document_spec::decoder_context_state(final(self)).1.len() == 0,
            document_spec::decoder_context_state(final(self)).2.len() == 0,
            document_spec::decoder_context_state(final(self)).3 == document_spec::decoder_context_state(old(self)).3,
    ))]
    pub fn reset(&mut self) {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        self.mmr.clear();
    }

    /// Clears per-document state and releases every retained allocation.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            document_spec::decoder_context_state(final(self)).0 == document_spec::decoder_context_state(old(self)).0,
            document_spec::decoder_context_state(final(self)).1.len() == 0,
            document_spec::decoder_context_state(final(self)).2.len() == 0,
            document_spec::decoder_context_state(final(self)).3 == 0,
    ))]
    pub fn release_scratch(&mut self) {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        self.mmr.release();
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            document_spec::decoder_context_state(final(self)).0 == document_spec::decoder_context_state(old(self)).0,
            document_spec::decoder_context_state(final(self)).1.len() == 0,
            document_spec::decoder_context_state(final(self)).2.len() == 0,
            document_spec::decoder_context_state(final(self)).3 ==
                if document_spec::decoder_context_state(old(self)).3
                    > document_spec::decoder_context_state(old(self)).0.max_retained_state_bytes {
                    0u64
                } else { document_spec::decoder_context_state(old(self)).3 },
    ))]
    pub(crate) fn finish_call(&mut self) {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        self.reset();
        if self.retained_scratch_bytes() > self.limits().max_retained_state_bytes {
            self.release_scratch();
        }
    }

    /// Parses and validates one standalone document.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::inspect`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            document_spec::decoder_context_finished(old(self), final(self)),
            match out {
                Ok(info) => document_spec::standalone_metadata(input@, &info),
                Err(_) => true,
            },
    ))]
    pub fn inspect(&mut self, input: &[u8]) -> Result<DocumentInfo, DecodeError> {
        let result = self.decoder.inspect(input);
        self.finish_call();
        result
    }

    /// Parses one embedded page stream.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::inspect_embedded`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            document_spec::decoder_context_finished(old(self), final(self)),
            match out {
                Ok(info) => document_spec::embedded_metadata(&info),
                Err(_) => true,
            },
    ))]
    pub fn inspect_embedded(&mut self, input: &[u8]) -> Result<DocumentInfo, DecodeError> {
        let result = self.decoder.inspect_embedded(input);
        self.finish_call();
        result
    }

    /// Parses one embedded page stream with validated globals.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::inspect_embedded_with_globals`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            document_spec::decoder_context_finished(old(self), final(self)),
            match out {
                Ok(info) => document_spec::embedded_metadata(&info),
                Err(_) => true,
            },
    ))]
    pub fn inspect_embedded_with_globals(
        &mut self,
        input: &[u8],
        globals: &GlobalSegments,
    ) -> Result<DocumentInfo, DecodeError> {
        let result = self.decoder.inspect_embedded_with_globals(input, globals);
        self.finish_call();
        result
    }

    /// Parses and validates immutable embedded global segments.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::parse_globals`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures document_spec::decoder_context_finished(old(self), final(self)),
        document_spec::global_parse_result(
            input@, document_spec::decoder_context_state(old(self)).0, out,
        ),
        match out {
            Ok(globals) => document_spec::global_segment_count(&globals)
                == document_spec::global_depth_count(&globals),
            Err(_) => true,
        },
    ))]
    pub fn parse_globals(&mut self, input: &[u8]) -> Result<GlobalSegments, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::decoder_context_fields(); }
        let result = self.decoder.parse_globals(input);
        self.finish_call();
        result
    }
}

impl Decoder {
    /// Returns the active resource limits.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures *out == document_spec::decoder_limits(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn limits(&self) -> &Limits {
        #[cfg(verus_keep_ghost)]
        production_decoder_fields!(self);
        &self.limits
    }

    /// Parses and validates a standalone sequential or random-access stream.
    ///
    /// This method validates the document envelope and page lifecycle without
    /// decoding compressed region payloads.
    /// On success, organization and the optional declared count match the
    /// standalone header; a known count equals the returned page count.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, unsupported, over-limit, or
    /// allocation-failing input.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(info) => document_spec::standalone_metadata(input@, &info),
            Err(_) => true,
        },
    ))]
    pub fn inspect(&self, input: &[u8]) -> Result<DocumentInfo, DecodeError> {
        let (info, _segments) = parse_standalone_document(input, &self.limits)?;
        Ok(info)
    }

    /// Reads a bounded standalone stream and validates its document envelope.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, malformed, unsupported, resource-limit, or
    /// allocation error.
    pub fn inspect_reader<R: Read>(&self, reader: &mut R) -> Result<DocumentInfo, DecodeError> {
        let bytes = read_bounded_input(reader, &self.limits)?;
        self.inspect(&bytes)
    }

    /// Parses a contiguous embedded segment stream such as PDF page data.
    ///
    /// For PDF integration, including [PDFium](https://pdfium.googlesource.com/pdfium/),
    /// the end of the complete input may terminate the final page without an
    /// end-of-page segment. An initially unknown height still requires a final
    /// completed stripe. Segment headers and payloads must not be truncated.
    /// Standalone validation remains strict.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, unsupported, over-limit, or
    /// allocation-failing input.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(info) => document_spec::embedded_metadata(&info),
            Err(_) => true,
        },
    ))]
    pub fn inspect_embedded(&self, input: &[u8]) -> Result<DocumentInfo, DecodeError> {
        self.inspect_embedded_internal(input, None)
    }

    /// Parses an embedded page stream with reusable validated global segments.
    ///
    /// Final-page completion follows [`Decoder::inspect_embedded`].
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, unsupported, over-limit,
    /// allocation-failing, or unresolved input.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(info) => document_spec::embedded_metadata(&info),
            Err(_) => true,
        },
    ))]
    pub fn inspect_embedded_with_globals(
        &self,
        input: &[u8],
        globals: &GlobalSegments,
    ) -> Result<DocumentInfo, DecodeError> {
        self.inspect_embedded_internal(input, Some(globals))
    }

    /// Parses and validates an immutable embedded global-segment stream.
    ///
    /// # Errors
    ///
    /// Returns a typed error if a global segment is malformed, associated with
    /// a page, unresolved, unsupported, over-limit, or cannot be allocated.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures document_spec::global_parse_result(
            input@, document_spec::decoder_limits(self), out,
        ),
        match out {
            Ok(globals) => document_spec::global_segment_count(&globals)
                == document_spec::global_depth_count(&globals),
            Err(_) => true,
        },
    ))]
    pub fn parse_globals(&self, input: &[u8]) -> Result<GlobalSegments, DecodeError> {
        #[cfg(verus_keep_ghost)]
        production_decoder_fields!(self);
        enforce_stream_limit(input.len(), &self.limits)?;
        enforce_input_work(input.len(), &self.limits)?;
        let segments = parse_sequential(input, &self.limits, false)?;
        let mut segment_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                segment_index <= segments@.len(),
                input@.len() <= self.limits.max_stream_bytes,
                document_spec::decoder_limits(self) == self.limits,
            decreases segments@.len() - segment_index,
        ))]
        while segment_index < segments.len() {
            let segment = &segments[segment_index];
            if segment.header.page_association != 0 {
                return Err(MalformedError::InvalidPageAssociation {
                    segment: segment.header.number,
                    page: segment.header.page_association,
                }
                .into());
            }
            segment_index += 1;
        }
        validate_profiles(&segments)?;
        enforce_work_limit(input.len(), &segments, &self.limits)?;
        let (types, depths) = validate_references(&segments, None, &self.limits)?;
        let Ok(input_bytes) = u64::try_from(input.len()) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        if input_bytes > self.limits.max_retained_state_bytes {
            return Err(LimitExceeded {
                kind: LimitKind::RetainedState,
                limit: self.limits.max_retained_state_bytes,
                actual: input_bytes,
            }
            .into());
        }
        let mut bytes = Vec::new();
        if bytes.try_reserve(input.len()).is_err() {
            return Err(DecodeError::Allocation {
                requested_bytes: input_bytes,
            });
        }
        let mut byte_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                byte_index <= input@.len(),
                bytes@ == input@.subrange(0, byte_index as int),
                types@.len() == depths@.len(),
                input@.len() <= self.limits.max_stream_bytes,
                input@.len() <= self.limits.max_retained_state_bytes,
                document_spec::decoder_limits(self) == self.limits,
            decreases input@.len() - byte_index,
        ))]
        while byte_index < input.len() {
            bytes.push(input[byte_index]);
            byte_index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! { vstd::assert_seqs_equal!(input@.subrange(0, input@.len() as int), input@); }
        let out = GlobalSegments {
            types,
            depths,
            bytes,
        };
        #[cfg(verus_keep_ghost)]
        production_global_fields!(&out);
        Ok(out)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(info) => document_spec::embedded_metadata(&info),
            Err(_) => true,
        },
    ))]
    fn inspect_embedded_internal(
        &self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
    ) -> Result<DocumentInfo, DecodeError> {
        let (info, _segments) = parse_embedded_document(input, &self.limits, globals)?;
        Ok(info)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures document_spec::default_limits(
            document_spec::decoder_limits(&out),
        ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn project_default() -> Self {
        Self::new(Limits::project_default())
    }
}

#[cfg_attr(verus_keep_ghost, verifier::verify)]
impl Default for Decoder {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures document_spec::default_limits(
            document_spec::decoder_limits(&out),
        ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn default() -> Self {
        Self::project_default()
    }
}

/// Immutable validated metadata for reusable embedded global segments.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct GlobalSegments {
    types: SegmentTypes,
    depths: SegmentDepths,
    bytes: Vec<u8>,
}

impl GlobalSegments {
    /// Number of global segments.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out as nat == document_spec::global_segment_count(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn segment_count(&self) -> usize {
        #[cfg(verus_keep_ghost)]
        production_global_fields!(self);
        self.types.len()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out@ == document_spec::global_bytes(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn bytes(&self) -> &[u8] {
        #[cfg(verus_keep_ghost)]
        production_global_fields!(self);
        &self.bytes
    }
}

/// Validated document-level metadata.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct DocumentInfo {
    organization: Organization,
    declared_page_count: Option<u32>,
    segment_count: u32,
    pages: Vec<PageInfo>,
}

impl DocumentInfo {
    /// Standalone organization, or sequential envelope semantics for embedded
    /// streams.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == document_spec::document_organization(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn organization(&self) -> Organization {
        #[cfg(verus_keep_ghost)]
        production_document_fields!(self);
        self.organization
    }

    /// Page count declared by a standalone header.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == document_spec::document_declared_page_count(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn declared_page_count(&self) -> Option<u32> {
        #[cfg(verus_keep_ghost)]
        production_document_fields!(self);
        self.declared_page_count
    }

    /// Number of parsed segments.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == document_spec::document_segment_count(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn segment_count(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_document_fields!(self);
        self.segment_count
    }

    /// Validated pages in ascending page-number order.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out@ == document_spec::document_pages(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn pages(&self) -> &[PageInfo] {
        #[cfg(verus_keep_ghost)]
        production_document_fields!(self);
        &self.pages
    }
}

/// Public page metadata established by page and stripe boundary segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct PageInfo {
    pub(crate) number: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) x_resolution: u32,
    pub(crate) y_resolution: u32,
    pub(crate) striped: bool,
}

impl PageInfo {
    /// One-based page number.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_number(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn number(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.number
    }

    /// Page width in pixels.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_width(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn width(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.width
    }

    /// Final page height in pixels.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_height(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn height(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.height
    }

    /// Horizontal resolution in pixels per metre, or zero when unknown.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_x_resolution(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn x_resolution(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.x_resolution
    }

    /// Vertical resolution in pixels per metre, or zero when unknown.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_y_resolution(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn y_resolution(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.y_resolution
    }

    /// Whether the page uses stripe boundaries.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == crate::views::page_is_striped(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn is_striped(&self) -> bool {
        #[cfg(verus_keep_ghost)]
        production_page_info_fields!(self);
        self.striped
    }
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct ParsedSegment<'a> {
    pub(crate) header: SegmentHeader,
    #[allow(dead_code)]
    pub(crate) payload: &'a [u8],
    pub(crate) data: ParsedData,
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum ParsedData {
    Page(PageInformation),
    EndOfStripe(u32),
    Profiles(Vec<u32>),
    Other,
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct PageState {
    info: PageInformation,
    last_stripe: Option<u32>,
    ended: bool,
}

#[cfg(test)]
mod tests;
