use crate::bitmap::CombinationOperator;
use crate::generic_region::{
    decode_generic_on_validated, GenericArithmeticCall, GenericArithmeticSession,
    GenericRegionError, GenericTemplate, TemplateOffset,
};
use crate::mmr::{decode_mmr, decode_mmr_prefix, EofbPolicy, MmrError};
use crate::{
    Bitmap, DecodeError, GenericRegionErrorKind, HalftoneErrorKind, LimitExceeded, LimitKind,
    Limits, MalformedError, MmrErrorKind,
};
#[cfg(verus_keep_ghost)]
use vstd::layout;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/halftone.rs"]
pub(crate) mod halftone_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/halftone.rs"]
mod halftone_properties;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/halftone.rs"]
mod halftone_shared;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/halftone.rs"]
mod halftone_proofs;

mod headers;
mod planes;
mod render;
mod resources;
mod scratch;

pub(crate) use headers::{parse_halftone_header, parse_pattern_dictionary_header};
#[allow(unused_imports)]
pub(crate) use planes::{
    decode_gray_planes, decode_pattern_dictionary, gray_bit_depth, slice_collective_bitmap,
};
#[allow(unused_imports)]
pub(crate) use render::{
    decode_halftone_region, generate_skip_mask, grid_position, validate_pattern_indices,
};
pub(crate) use scratch::{
    new_halftone_state_from_scratch, new_pattern_arithmetic_state_from_scratch,
};

/// Errors specific to pattern-dictionary and halftone decoding.
///
/// The structural variants need corresponding public [`MalformedError`]
/// variants when this module is wired into the segment dispatcher.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum HalftoneError {
    Decode(DecodeError),
    InvalidPatternDictionaryFlags(u8),
    InvalidPatternDimensions {
        width: u8,
        height: u8,
    },
    InvalidHalftoneFlags(u8),
    InvalidHalftoneConfiguration,
    InvalidBitmapDimensions {
        expected_width: u32,
        expected_height: u32,
        actual_width: u32,
        actual_height: u32,
    },
    InvalidGrayBitDepth(u8),
    EmptyPatternDictionary,
    InconsistentPatternDimensions,
    InvalidPatternIndex {
        index: u32,
        pattern_count: u64,
    },
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<DecodeError> for HalftoneError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<MalformedError> for HalftoneError {
    fn from(error: MalformedError) -> Self {
        Self::Decode(error.into())
    }
}

/// The coding mode and generic-region template parameters for one bitmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum GenericRegionCoding {
    Arithmetic {
        template: u8,
        adaptive_pixels: [(i32, i32); 4],
    },
    Mmr {
        /// Gray planes require EOFB; a known-length collective bitmap does not.
        require_eofb: bool,
    },
}

/// A generic-region decode requested by the pattern or halftone procedures.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct GenericRegionRequest<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) coding: GenericRegionCoding,
    pub(crate) typical_prediction: bool,
    pub(crate) extended_template: bool,
    pub(crate) skip: Option<&'a Bitmap>,
}

#[cfg(test)]
pub(crate) trait GenericRegionTestDecoder {
    fn decode_generic_region(
        &mut self,
        request: GenericRegionRequest<'_>,
    ) -> Result<Bitmap, DecodeError>;
}

#[cfg(test)]
impl<F> GenericRegionTestDecoder for F
where
    F: for<'a> FnMut(GenericRegionRequest<'a>) -> Result<Bitmap, DecodeError>,
{
    fn decode_generic_region(
        &mut self,
        request: GenericRegionRequest<'_>,
    ) -> Result<Bitmap, DecodeError> {
        self(request)
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn generic_error_into_decode(error: GenericRegionError) -> DecodeError {
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
fn mmr_error_into_decode(error: MmrError) -> DecodeError {
    let kind = match error {
        MmrError::Reader(error) => return DecodeError::Malformed(error),
        MmrError::Bitmap(error) => return error,
        MmrError::Allocation { requested_bytes } => {
            return DecodeError::Allocation { requested_bytes };
        }
        MmrError::IntegerOverflow => {
            return DecodeError::Malformed(MalformedError::IntegerOverflow);
        }
        MmrError::InvalidMode { bit_offset } => MmrErrorKind::InvalidMode { bit_offset },
        MmrError::ExtensionMode { bit_offset } => MmrErrorKind::ExtensionMode { bit_offset },
        MmrError::InvalidRunCode { bit_offset, black } => {
            MmrErrorKind::InvalidRunCode { bit_offset, black }
        }
        MmrError::RunExceedsRow => MmrErrorKind::RunExceedsRow,
        MmrError::InvalidTransition => MmrErrorKind::InvalidTransition,
        MmrError::NonProgress => MmrErrorKind::NonProgress,
        MmrError::TooManyModes => MmrErrorKind::TooManyModes,
        MmrError::MissingEofb => MmrErrorKind::MissingEofb,
        MmrError::InvalidEofb => MmrErrorKind::InvalidEofb,
        MmrError::TrailingData => MmrErrorKind::TrailingData,
        MmrError::NonZeroEofbPadding => MmrErrorKind::NonZeroPadding,
    };
    DecodeError::Malformed(MalformedError::InvalidMmr(kind))
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum ArithmeticPurpose {
    Pattern,
    Halftone,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum GenericRegionState<'a> {
    ZeroPlanes,
    Arithmetic {
        session: GenericArithmeticSession<'a>,
        template: u8,
        purpose: ArithmeticPurpose,
    },
    PatternMmr {
        coded: &'a [u8],
        limits: Limits,
        used: bool,
    },
    GrayMmr {
        coded: &'a [u8],
        limits: Limits,
        offset: usize,
    },
    #[cfg(test)]
    Scripted(&'a mut dyn GenericRegionTestDecoder),
}

/// A one-pattern grid has no coded planes in either entropy mode.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) => coded@.len() == 0 && state == GenericRegionState::ZeroPlanes,
        Err(error) => coded@.len() > 0
            && error == DecodeError::Malformed(MalformedError::TrailingData(coded@.len() as u64)),
    },
))]
pub(crate) fn new_zero_plane_state(coded: &[u8]) -> Result<GenericRegionState<'_>, DecodeError> {
    if !coded.is_empty() {
        let trailing = u64::try_from(coded.len()).map_err(|_| MalformedError::IntegerOverflow)?;
        return Err(MalformedError::TrailingData(trailing).into());
    }
    Ok(GenericRegionState::ZeroPlanes)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) =>
            halftone_spec::decoder_state_valid(&state)
                && ((state == GenericRegionState::ZeroPlanes) == (pattern_count == 1))
                && (pattern_count == 1 ==> coded@.len() == 0),
        Err(_) => true,
    },
))]
pub(crate) fn new_halftone_state<'a>(
    coded: &'a [u8],
    mmr: bool,
    template: GenericTemplate,
    template_code: u8,
    pattern_count: usize,
    limits: &Limits,
) -> Result<GenericRegionState<'a>, DecodeError> {
    let mut scratch = Vec::new();
    new_halftone_state_from_scratch(
        coded,
        mmr,
        template,
        template_code,
        pattern_count,
        limits,
        &mut scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) => halftone_spec::decoder_state_valid(&state)
            && matches!(state, GenericRegionState::Arithmetic { .. }),
        Err(_) => true,
    },
))]
fn new_arithmetic_state<'a>(
    coded: &'a [u8],
    template: GenericTemplate,
    template_code: u8,
    purpose: ArithmeticPurpose,
    limits: &Limits,
) -> Result<GenericRegionState<'a>, DecodeError> {
    let session = match GenericArithmeticSession::new(coded, template, false, limits) {
        Ok(session) => session,
        Err(error) => return Err(generic_error_into_decode(error)),
    };
    Ok(GenericRegionState::Arithmetic {
        session,
        template: template_code,
        purpose,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) => halftone_spec::decoder_state_valid(&state)
            && matches!(state, GenericRegionState::Arithmetic { .. }),
        Err(_) => true,
    },
))]
pub(crate) fn new_pattern_arithmetic_state<'a>(
    coded: &'a [u8],
    template: GenericTemplate,
    template_code: u8,
    limits: &Limits,
) -> Result<GenericRegionState<'a>, DecodeError> {
    new_arithmetic_state(
        coded,
        template,
        template_code,
        ArithmeticPurpose::Pattern,
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) => halftone_spec::decoder_state_valid(&state)
            && matches!(state, GenericRegionState::Arithmetic { .. }),
        Err(_) => true,
    },
))]
pub(crate) fn new_halftone_arithmetic_state<'a>(
    coded: &'a [u8],
    template: GenericTemplate,
    template_code: u8,
    limits: &Limits,
) -> Result<GenericRegionState<'a>, DecodeError> {
    new_arithmetic_state(
        coded,
        template,
        template_code,
        ArithmeticPurpose::Halftone,
        limits,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures halftone_spec::decoder_state_valid(&out),
))]
pub(crate) fn new_pattern_mmr_state<'a>(
    coded: &'a [u8],
    limits: &Limits,
) -> GenericRegionState<'a> {
    GenericRegionState::PatternMmr {
        coded,
        limits: *limits,
        used: false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        halftone_spec::decoder_state_valid(&out),
        matches!(out, GenericRegionState::GrayMmr { .. }),
))]
pub(crate) fn new_gray_mmr_state<'a>(coded: &'a [u8], limits: &Limits) -> GenericRegionState<'a> {
    GenericRegionState::GrayMmr {
        coded,
        limits: *limits,
        offset: 0,
    }
}

#[allow(clippy::elidable_lifetime_names)]
impl<'a> GenericRegionState<'a> {
    #[cfg(test)]
    pub(crate) fn scripted(decoder: &'a mut dyn GenericRegionTestDecoder) -> Self {
        Self::Scripted(decoder)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            halftone_spec::decoder_state_valid(old(self)),
            match request.skip {
                Some(skip) => crate::spec::bitmap::storage_valid(skip),
                None => true,
            },
        ensures match out {
            Ok(bitmap) =>
                halftone_spec::decoder_state_valid(final(self))
                    && crate::spec::bitmap::storage_valid(&bitmap),
            Err(_) => true,
        },
    ))]
    #[allow(clippy::too_many_lines)]
    fn decode_generic_region(
        &mut self,
        request: GenericRegionRequest<'_>,
    ) -> Result<Bitmap, DecodeError> {
        match self {
            Self::ZeroPlanes => Err(MalformedError::InvalidPageState(
                "zero-plane halftone requested a coded bitmap",
            )
            .into()),
            Self::Arithmetic {
                session,
                template,
                purpose,
            } => {
                let GenericRegionCoding::Arithmetic {
                    template: requested_template,
                    adaptive_pixels,
                } = request.coding
                else {
                    let reason = match purpose {
                        ArithmeticPurpose::Pattern => "pattern dictionary coding mode changed",
                        ArithmeticPurpose::Halftone => "halftone coding mode changed",
                    };
                    return Err(MalformedError::InvalidPageState(reason).into());
                };
                if requested_template != *template {
                    let reason = match purpose {
                        ArithmeticPurpose::Pattern => "pattern dictionary template changed",
                        ArithmeticPurpose::Halftone => "halftone template changed",
                    };
                    return Err(MalformedError::InvalidPageState(reason).into());
                }
                let mut adaptive = [TemplateOffset::new(0, 0); 12];
                adaptive[0] = TemplateOffset::new(adaptive_pixels[0].0, adaptive_pixels[0].1);
                adaptive[1] = TemplateOffset::new(adaptive_pixels[1].0, adaptive_pixels[1].1);
                adaptive[2] = TemplateOffset::new(adaptive_pixels[2].0, adaptive_pixels[2].1);
                adaptive[3] = TemplateOffset::new(adaptive_pixels[3].0, adaptive_pixels[3].1);
                match decode_generic_on_validated(
                    &mut session.decoder,
                    &mut session.contexts,
                    GenericArithmeticCall {
                        width: request.width,
                        height: request.height,
                        adaptive,
                        typical_prediction: request.typical_prediction,
                        skip: request.skip,
                    },
                    request.height,
                    &session.limits,
                    &mut session.work_used,
                ) {
                    Ok(bitmap) => Ok(bitmap),
                    Err(error) => Err(generic_error_into_decode(error)),
                }
            }
            Self::PatternMmr {
                coded,
                limits,
                used,
            } => {
                if *used {
                    return Err(MalformedError::InvalidPageState(
                        "pattern dictionary requested multiple MMR bitmaps",
                    )
                    .into());
                }
                if request.coding
                    != (GenericRegionCoding::Mmr {
                        require_eofb: false,
                    })
                {
                    return Err(MalformedError::InvalidPageState(
                        "pattern dictionary coding mode changed",
                    )
                    .into());
                }
                *used = true;
                match decode_mmr(
                    coded,
                    request.width,
                    request.height,
                    limits,
                    EofbPolicy::Optional,
                ) {
                    Ok(bitmap) => Ok(bitmap),
                    Err(error) => Err(mmr_error_into_decode(error)),
                }
            }
            Self::GrayMmr {
                coded,
                limits,
                offset,
            } => {
                if request.coding != (GenericRegionCoding::Mmr { require_eofb: true }) {
                    return Err(
                        MalformedError::InvalidPageState("halftone coding mode changed").into(),
                    );
                }
                let remaining = coded
                    .get(*offset..)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let (bitmap, consumed) =
                    match decode_mmr_prefix(remaining, request.width, request.height, limits) {
                        Ok(decoded) => decoded,
                        Err(error) => return Err(mmr_error_into_decode(error)),
                    };
                *offset = offset
                    .checked_add(consumed)
                    .ok_or(MalformedError::IntegerOverflow)?;
                Ok(bitmap)
            }
            #[cfg(test)]
            Self::Scripted(decoder) => decoder.decode_generic_region(request),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures *self == GenericRegionState::ZeroPlanes ==> out == Ok(()),
    ))]
    pub(crate) fn finish(&self) -> Result<(), DecodeError> {
        match self {
            Self::ZeroPlanes => {}
            Self::Arithmetic { session, .. } => {
                if let Err(error) = session.validate_final_marker() {
                    return Err(generic_error_into_decode(error));
                }
            }
            Self::PatternMmr { .. } => {}
            Self::GrayMmr { coded, offset, .. } => {
                if *offset != coded.len() {
                    let trailing = coded
                        .len()
                        .checked_sub(*offset)
                        .and_then(|bytes| u64::try_from(bytes).ok())
                        .ok_or(MalformedError::IntegerOverflow)?;
                    return Err(MalformedError::TrailingData(trailing).into());
                }
            }
            #[cfg(test)]
            Self::Scripted(_) => {}
        }
        Ok(())
    }
}

/// A cumulative work meter shared by all stages of one decode operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct WorkBudget {
    limit: u64,
    used: u64,
}

impl WorkBudget {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            halftone_spec::work_limit(&out) == limits.max_work_units,
            halftone_spec::work_used(&out) == 0,
            halftone_spec::work_valid(&out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn new(limits: &Limits) -> Self {
        let out = Self {
            limit: limits.max_work_units,
            used: 0,
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            halftone_spec::work_fields(&out);
        }
        out
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == halftone_spec::work_used(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn used(&self) -> u64 {
        #[cfg(verus_keep_ghost)]
        proof! {
            halftone_spec::work_fields(self);
        }
        self.used
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(()) =>
                halftone_spec::work_limit(final(self))
                    == halftone_spec::work_limit(old(self))
                && halftone_spec::work_used(final(self))
                    == halftone_spec::work_used(old(self)) + units
                && halftone_spec::work_used(final(self))
                    <= halftone_spec::work_limit(final(self)),
            Err(_) =>
                halftone_spec::work_limit(final(self))
                    == halftone_spec::work_limit(old(self))
                && halftone_spec::work_used(final(self))
                    == halftone_spec::work_used(old(self)),
        },
    ))]
    fn charge(&mut self, units: u64) -> Result<(), HalftoneError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            halftone_spec::work_fields(self);
        }
        let used = self
            .used
            .checked_add(units)
            .ok_or(MalformedError::IntegerOverflow)?;
        if used > self.limit {
            #[cfg(verus_keep_ghost)]
            proof! {
                halftone_spec::work_fields(self);
            }
            return Err(LimitExceeded {
                kind: LimitKind::Work,
                limit: self.limit,
                actual: used,
            }
            .into());
        }
        self.used = used;
        #[cfg(verus_keep_ghost)]
        proof! {
            halftone_spec::work_fields(self);
        }
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<LimitExceeded> for HalftoneError {
    fn from(error: LimitExceeded) -> Self {
        Self::Decode(error.into())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == halftone_error_result(error),
))]
pub(crate) fn halftone_error_into_decode(error: HalftoneError) -> DecodeError {
    let kind = match error {
        HalftoneError::Decode(error) => return error,
        HalftoneError::InvalidPatternDictionaryFlags(flags) => {
            HalftoneErrorKind::InvalidPatternDictionaryFlags(flags)
        }
        HalftoneError::InvalidPatternDimensions { width, height } => {
            HalftoneErrorKind::InvalidPatternDimensions { width, height }
        }
        HalftoneError::InvalidHalftoneFlags(flags) => {
            HalftoneErrorKind::InvalidHalftoneFlags(flags)
        }
        HalftoneError::InvalidHalftoneConfiguration => HalftoneErrorKind::InvalidConfiguration,
        HalftoneError::InvalidBitmapDimensions {
            expected_width,
            expected_height,
            actual_width,
            actual_height,
        } => HalftoneErrorKind::InvalidBitmapDimensions {
            expected_width,
            expected_height,
            actual_width,
            actual_height,
        },
        HalftoneError::InvalidGrayBitDepth(depth) => HalftoneErrorKind::InvalidGrayBitDepth(depth),
        HalftoneError::EmptyPatternDictionary => HalftoneErrorKind::EmptyPatternDictionary,
        HalftoneError::InconsistentPatternDimensions => {
            HalftoneErrorKind::InconsistentPatternDimensions
        }
        HalftoneError::InvalidPatternIndex {
            index,
            pattern_count,
        } => HalftoneErrorKind::InvalidPatternIndex {
            index,
            pattern_count,
        },
    };
    DecodeError::Malformed(MalformedError::InvalidHalftone(kind))
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<HalftoneError> for DecodeError {
    fn from(error: HalftoneError) -> Self {
        halftone_error_into_decode(error)
    }
}

#[cfg(verus_keep_ghost)]
verus! {

impl vstd::std_specs::convert::FromSpecImpl<DecodeError> for HalftoneError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: DecodeError) -> HalftoneError {
        HalftoneError::Decode(error)
    }
}

impl vstd::std_specs::convert::FromSpecImpl<MalformedError> for HalftoneError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: MalformedError) -> HalftoneError {
        HalftoneError::Decode(DecodeError::Malformed(error))
    }
}

impl vstd::std_specs::convert::FromSpecImpl<LimitExceeded> for HalftoneError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: LimitExceeded) -> HalftoneError {
        HalftoneError::Decode(DecodeError::LimitExceeded(error))
    }
}

impl vstd::std_specs::convert::FromSpecImpl<HalftoneError> for DecodeError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: HalftoneError) -> DecodeError {
        halftone_error_result(error)
    }
}

pub(crate) open spec fn halftone_error_result(error: HalftoneError) -> DecodeError {
    match error {
            HalftoneError::Decode(error) => error,
            HalftoneError::InvalidPatternDictionaryFlags(flags) => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidPatternDictionaryFlags(flags))),
            HalftoneError::InvalidPatternDimensions { width, height } => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidPatternDimensions { width, height })),
            HalftoneError::InvalidHalftoneFlags(flags) => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidHalftoneFlags(flags))),
            HalftoneError::InvalidHalftoneConfiguration => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidConfiguration)),
            HalftoneError::InvalidBitmapDimensions { expected_width, expected_height, actual_width, actual_height } =>
                DecodeError::Malformed(MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidBitmapDimensions {
                    expected_width, expected_height, actual_width, actual_height,
                })),
            HalftoneError::InvalidGrayBitDepth(depth) => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidGrayBitDepth(depth))),
            HalftoneError::EmptyPatternDictionary => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::EmptyPatternDictionary)),
            HalftoneError::InconsistentPatternDimensions => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InconsistentPatternDimensions)),
            HalftoneError::InvalidPatternIndex { index, pattern_count } => DecodeError::Malformed(
                MalformedError::InvalidHalftone(HalftoneErrorKind::InvalidPatternIndex { index, pattern_count })),
    }
}

}

/// Parsed seven-byte pattern-dictionary data header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct PatternDictionaryHeader {
    pub(crate) mmr: bool,
    pub(crate) template: u8,
    pub(crate) pattern_width: u8,
    pub(crate) pattern_height: u8,
    pub(crate) gray_max: u32,
}

impl PatternDictionaryHeader {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out as nat == halftone_spec::pattern_count(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn pattern_count(self) -> u64 {
        u64::from(self.gray_max) + 1
    }
}

/// Parsed 21-byte halftone-specific header following region information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct HalftoneHeader {
    pub(crate) mmr: bool,
    pub(crate) template: u8,
    pub(crate) enable_skip: bool,
    pub(crate) operator: CombinationOperator,
    pub(crate) default_pixel: bool,
    pub(crate) grid_width: u32,
    pub(crate) grid_height: u32,
    pub(crate) grid_x: i32,
    pub(crate) grid_y: i32,
    pub(crate) grid_vector_x: u16,
    pub(crate) grid_vector_y: u16,
}

#[cfg(test)]
mod tests;
