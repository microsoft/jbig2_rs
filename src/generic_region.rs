//! Generic and generic-refinement region decoding.
//!
//! This module implements T.88 clauses 6.2, 6.3, 7.4.6, and 7.4.7 without
//! duplicating the crate's arithmetic or MMR entropy decoders.  Context slots
//! are kept in normative geometric order; moving an adaptive pixel changes
//! only the sampled coordinate, never the probability-context bit assigned to
//! that slot.

use core::fmt;

use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::mmr::MmrError;
use crate::segment_data::RegionInfo;
use crate::{Bitmap, DecodeError, GenericRegionErrorKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::convert::FromSpecImpl;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/generic_region.rs"]
pub(crate) mod generic_region_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/generic_region.rs"]
mod generic_region_properties;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/generic_region.rs"]
mod generic_region_shared;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/generic_region.rs"]
mod generic_region_proofs;

mod contexts;
mod generic;
mod generic_context;
mod headers;
mod refinement;
mod segments;
mod support;

#[allow(unused_imports)]
pub(crate) use generic::{
    decode_arithmetic_generic, decode_generic_on, decode_generic_on_validated,
    decode_probability_signal, probability_signal_error, RASTER_SIGNAL_CONTEXT,
};
pub(crate) use headers::{parse_generic_region_header, parse_refinement_region_header};
pub(crate) use refinement::{decode_arithmetic_refinement, decode_refinement_on};
#[allow(unused_imports)]
pub(crate) use segments::{
    decode_generic_region_segment, decode_generic_region_segment_into,
    decode_generic_region_segment_reused,
};
pub(crate) use support::{enforce_managed_memory, integer_overflow, word_byte};

#[cfg_attr(verus_keep_ghost, verus_verify)]
const ARITHMETIC_TERMINATOR: [u8; 2] = [0xff, 0xac];
#[cfg_attr(verus_keep_ghost, verus_verify)]
const MMR_UNKNOWN_LENGTH_SEPARATOR: [u8; 2] = [0x00, 0x00];
#[cfg_attr(verus_keep_ghost, verus_verify)]
const MAX_GENERIC_CONTEXTS: usize = 1 << 16;
#[cfg_attr(verus_keep_ghost, verus_verify)]
const MAX_REFINEMENT_CONTEXTS: usize = 1 << 13;

/// A module-local error whose dedicated crate-wide malformed classifications
/// can be added when this isolated module is wired into the document decoder.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum GenericRegionError {
    /// An error already represented by the crate-wide error vocabulary.
    Decode(DecodeError),
    /// Reserved bits were set in a generic or refinement flag byte.
    ReservedFlags(u8),
    /// A flag combination violates the segment syntax.
    InvalidFlags(u8),
    /// A target-plane adaptive pixel does not refer to an earlier pixel.
    InvalidAdaptivePixel {
        /// One-based adaptive-pixel number.
        number: u8,
        /// Horizontal offset.
        x: i32,
        /// Vertical offset.
        y: i32,
    },
    /// A caller supplied a skip bitmap of the wrong size.
    InvalidSkipDimensions,
    /// A required arithmetic marker was absent or not the final coded bytes.
    InvalidArithmeticTerminator,
    /// A marker other than `FF AC` occurred in an arithmetic block.
    UnexpectedArithmeticMarker { offset: usize, code: u8 },
    /// The unknown-length terminal count disagreed with decoded rows.
    RowCountMismatch { expected: u32, actual: u32 },
    /// A generated context index was outside its allocated statistics set.
    ContextIndex { index: usize, count: usize },
}

impl fmt::Display for GenericRegionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::ReservedFlags(flags) => {
                write!(
                    formatter,
                    "reserved generic-region flags are set: {flags:#04x}"
                )
            }
            Self::InvalidFlags(flags) => {
                write!(
                    formatter,
                    "invalid generic-region flag combination: {flags:#04x}"
                )
            }
            Self::InvalidAdaptivePixel { number, x, y } => write!(
                formatter,
                "adaptive pixel {number} at ({x}, {y}) is not an earlier target pixel"
            ),
            Self::InvalidSkipDimensions => {
                formatter.write_str("generic-region skip bitmap has the wrong dimensions")
            }
            Self::InvalidArithmeticTerminator => {
                formatter.write_str("arithmetic region does not end in FF AC")
            }
            Self::UnexpectedArithmeticMarker { offset, code } => write!(
                formatter,
                "unexpected arithmetic marker FF {code:02X} at byte {offset}"
            ),
            Self::RowCountMismatch { expected, actual } => write!(
                formatter,
                "terminal row count {actual} differs from decoded row count {expected}"
            ),
            Self::ContextIndex { index, count } => write!(
                formatter,
                "context index {index} is outside a statistics set of length {count}"
            ),
        }
    }
}

impl std::error::Error for GenericRegionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<DecodeError> for GenericRegionError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<MalformedError> for GenericRegionError {
    fn from(error: MalformedError) -> Self {
        Self::Decode(error.into())
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<MmrError> for GenericRegionError {
    fn from(error: MmrError) -> Self {
        Self::Decode(error.into())
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<GenericRegionError> for DecodeError {
    fn from(error: GenericRegionError) -> Self {
        let kind = match error {
            GenericRegionError::Decode(error) => return error,
            GenericRegionError::ReservedFlags(flags) => {
                GenericRegionErrorKind::ReservedFlags(flags)
            }
            GenericRegionError::InvalidFlags(flags) => GenericRegionErrorKind::InvalidFlags(flags),
            GenericRegionError::InvalidAdaptivePixel { number, x, y } => {
                GenericRegionErrorKind::InvalidAdaptivePixel { number, x, y }
            }
            GenericRegionError::InvalidSkipDimensions => {
                GenericRegionErrorKind::InvalidSkipDimensions
            }
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
        Self::Malformed(MalformedError::InvalidGenericRegion(kind))
    }
}

#[cfg(verus_keep_ghost)]
verus! {

impl FromSpecImpl<DecodeError> for GenericRegionError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: DecodeError) -> GenericRegionError {
        GenericRegionError::Decode(error)
    }
}

impl FromSpecImpl<MalformedError> for GenericRegionError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: MalformedError) -> GenericRegionError {
        GenericRegionError::Decode(DecodeError::Malformed(error))
    }
}

impl FromSpecImpl<MmrError> for GenericRegionError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: MmrError) -> GenericRegionError {
        GenericRegionError::Decode(crate::mmr::mmr_error_result(error))
    }
}

// Project error policy, discharged by the executing From body through the pinned trait contract.
impl FromSpecImpl<GenericRegionError> for DecodeError {
    open spec fn obeys_from_spec() -> bool {
        true
    }

    closed spec fn from_spec(error: GenericRegionError) -> DecodeError {
        match error {
            GenericRegionError::Decode(error) => error,
            GenericRegionError::ReservedFlags(flags) => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::ReservedFlags(flags))),
            GenericRegionError::InvalidFlags(flags) => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::InvalidFlags(flags))),
            GenericRegionError::InvalidAdaptivePixel { number, x, y } => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::InvalidAdaptivePixel { number, x, y })),
            GenericRegionError::InvalidSkipDimensions => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::InvalidSkipDimensions)),
            GenericRegionError::InvalidArithmeticTerminator => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::InvalidArithmeticTerminator)),
            GenericRegionError::UnexpectedArithmeticMarker { offset, code } => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::UnexpectedArithmeticMarker { offset, code })),
            GenericRegionError::RowCountMismatch { expected, actual } => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::RowCountMismatch { expected, actual })),
            GenericRegionError::ContextIndex { index, count } => DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(GenericRegionErrorKind::ContextIndex { index, count })),
        }
    }
}

}

/// A signed template offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct TemplateOffset {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl TemplateOffset {
    const ZERO: Self = Self { x: 0, y: 0 };

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.x == x, out.y == y,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// One of the four generic-region arithmetic templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum GenericTemplate {
    Template0,
    Template1,
    Template2,
    Template3,
}

/// Parsed generic-region data header.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericRegionHeader {
    pub(crate) region: RegionInfo,
    pub(crate) mmr: bool,
    pub(crate) template: GenericTemplate,
    pub(crate) typical_prediction: bool,
    pub(crate) extended_template: bool,
    pub(crate) adaptive: [TemplateOffset; 12],
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Clone for GenericRegionHeader {
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        ensures(|out: Self| equal(&out, self));
        Self {
            region: self.region,
            mmr: self.mmr,
            template: self.template,
            typical_prediction: self.typical_prediction,
            extended_template: self.extended_template,
            adaptive: self.adaptive,
        }
    }
}

/// The decoded bitmap and number of data bytes consumed.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct DecodedGenericRegion {
    pub(crate) header: GenericRegionHeader,
    pub(crate) bitmap: Bitmap,
    pub(crate) consumed_bytes: usize,
    pub(crate) decoded_rows: u32,
}

/// Metadata for a generic region decoded directly into caller-owned storage.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct DecodedGenericRegionDirect {
    pub(crate) header: GenericRegionHeader,
    pub(crate) consumed_bytes: usize,
    pub(crate) decoded_rows: u32,
}

/// Reusable `GB` probability statistics for one generic template.
///
/// This type intentionally has no infallible [`Clone`] implementation:
/// retaining dictionary statistics must use [`Self::try_snapshot`] so memory
/// limits and allocation failure remain observable.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericContexts {
    pub(crate) template: GenericTemplate,
    pub(crate) extended_template: bool,
    pub(crate) contexts: Vec<ArithmeticContext>,
    pub(crate) storage_bytes: u64,
}

/// A fallibly allocated retained copy of generic arithmetic statistics.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericContextSnapshot {
    template: GenericTemplate,
    extended_template: bool,
    contexts: Vec<ArithmeticContext>,
    storage_bytes: u64,
}

/// Parameters for one bitmap decoded from a reusable generic arithmetic
/// session.
///
/// A session fixes the template and its probability-statistics set, while
/// dimensions, adaptive locations, typical prediction, and skip data belong
/// to each individual generic-region procedure invocation.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericArithmeticCall<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) adaptive: [TemplateOffset; 12],
    pub(crate) typical_prediction: bool,
    pub(crate) skip: Option<&'a Bitmap>,
}

/// A continuous arithmetic stream and reusable generic-region statistics.
///
/// This is used when multiple bitmaps, such as halftone gray planes, are coded
/// consecutively with one arithmetic decoder and one `GB` statistics set.
/// Calling [`Self::decode_bitmap`] does not reset either the entropy decoder or
/// the contexts.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericArithmeticSession<'a> {
    pub(crate) decoder: ArithmeticDecoder<'a>,
    pub(crate) contexts: GenericContexts,
    pub(crate) coded_length: usize,
    pub(crate) work_used: u64,
    pub(crate) limits: Limits,
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct UnknownLengthData<'a> {
    coded: &'a [u8],
    rows: u32,
    consumed: usize,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct GenericSlot {
    nominal: TemplateOffset,
    adaptive: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum GenericContextDescriptor {
    NominalTemplate0,
    Template0([TemplateOffset; 4]),
    Template0Extended([TemplateOffset; 16]),
    NominalTemplate1,
    Template1(TemplateOffset),
    NominalTemplate2,
    Template2(TemplateOffset),
    NominalTemplate3,
    Template3(TemplateOffset),
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum GenericRowContext {
    Template0 {
        top: usize,
        previous: usize,
        top_row: Option<usize>,
        previous_row: Option<usize>,
        top_queue: u8,
        previous_queue: u8,
        remaining: u8,
    },
    Template1 {
        top: usize,
        previous: usize,
        top_row: Option<usize>,
        previous_row: Option<usize>,
        top_queue: u8,
        previous_queue: u8,
        remaining: u8,
    },
    Template2 {
        top: usize,
        previous: usize,
        top_row: Option<usize>,
        previous_row: Option<usize>,
        top_queue: u8,
        previous_queue: u8,
        remaining: u8,
    },
    Template3 {
        previous: usize,
        previous_row: Option<usize>,
        previous_queue: u8,
        remaining: u8,
    },
    Adaptive(GenericContextDescriptor),
}

/// One of the two generic-refinement templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum RefinementTemplate {
    Template0,
    Template1,
}

/// Reusable `GR` probability statistics for one refinement template.
///
/// Retention is explicitly fallible through [`Self::try_snapshot`]; this type
/// intentionally does not implement [`Clone`].
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RefinementContexts {
    pub(crate) template: RefinementTemplate,
    pub(crate) contexts: Vec<ArithmeticContext>,
    pub(crate) storage_bytes: u64,
}

/// A fallibly allocated retained copy of refinement arithmetic statistics.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RefinementContextSnapshot {
    template: RefinementTemplate,
    contexts: Vec<ArithmeticContext>,
    storage_bytes: u64,
}

/// Per-bitmap parameters for decoding on retained refinement statistics.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RefinementArithmeticCall {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) adaptive: [TemplateOffset; 2],
    pub(crate) typical_prediction: bool,
}

/// Parsed generic-refinement region header.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RefinementRegionHeader {
    pub(crate) region: RegionInfo,
    pub(crate) template: RefinementTemplate,
    pub(crate) typical_prediction: bool,
    pub(crate) adaptive: [TemplateOffset; 2],
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Clone for RefinementRegionHeader {
    fn clone(&self) -> Self {
        #[cfg(verus_keep_ghost)]
        ensures(|out: Self| equal(&out, self));
        Self {
            region: self.region,
            template: self.template,
            typical_prediction: self.typical_prediction,
            adaptive: self.adaptive,
        }
    }
}

/// Runtime parameters not carried by a standalone refinement data header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RefinementParameters {
    pub(crate) reference_dx: i32,
    pub(crate) reference_dy: i32,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum RefinementPlane {
    Target,
    Reference,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct RefinementSlot {
    plane: RefinementPlane,
    nominal: TemplateOffset,
    adaptive: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum RefinementContextDescriptor {
    NominalTemplate0,
    Template0 {
        target: TemplateOffset,
        reference: TemplateOffset,
    },
    Template1,
}

#[path = "generic_row_ring.rs"]
mod generic_row_ring;

#[cfg(test)]
mod tests;
