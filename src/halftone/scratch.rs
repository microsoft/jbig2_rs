//! Scratch-backed halftone and pattern-dictionary decoder constructors.

#[cfg(verus_keep_ghost)]
use super::halftone_spec;
use super::{
    generic_error_into_decode, new_gray_mmr_state, new_zero_plane_state, ArithmeticPurpose,
    GenericRegionState,
};
use crate::arithmetic::ArithmeticContext;
use crate::generic_region::{GenericArithmeticSession, GenericTemplate};
use crate::{DecodeError, Limits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(state) => halftone_spec::decoder_state_valid(&state)
            && matches!(state, GenericRegionState::Arithmetic { .. }),
        Err(_) => true,
    },
))]
fn scratch_arithmetic_state<'a>(
    coded: &'a [u8],
    template: GenericTemplate,
    template_code: u8,
    purpose: ArithmeticPurpose,
    limits: &Limits,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<GenericRegionState<'a>, DecodeError> {
    let session = match GenericArithmeticSession::new_from_scratch(
        coded,
        template,
        false,
        limits,
        generic_scratch,
    ) {
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
pub(crate) fn new_pattern_arithmetic_state_from_scratch<'a>(
    coded: &'a [u8],
    template: GenericTemplate,
    template_code: u8,
    limits: &Limits,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<GenericRegionState<'a>, DecodeError> {
    scratch_arithmetic_state(
        coded,
        template,
        template_code,
        ArithmeticPurpose::Pattern,
        limits,
        generic_scratch,
    )
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
pub(crate) fn new_halftone_state_from_scratch<'a>(
    coded: &'a [u8],
    mmr: bool,
    template: GenericTemplate,
    template_code: u8,
    pattern_count: usize,
    limits: &Limits,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<GenericRegionState<'a>, DecodeError> {
    if pattern_count == 1 {
        new_zero_plane_state(coded)
    } else if mmr {
        Ok(new_gray_mmr_state(coded, limits))
    } else {
        scratch_arithmetic_state(
            coded,
            template,
            template_code,
            ArithmeticPurpose::Halftone,
            limits,
            generic_scratch,
        )
    }
}

impl GenericRegionState<'_> {
    /// Returns any owned generic context buffer to `generic_scratch` for reuse.
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) fn into_scratch(self, generic_scratch: &mut Vec<ArithmeticContext>) {
        if let Self::Arithmetic { session, .. } = self {
            session.into_scratch(generic_scratch);
        }
    }
}
