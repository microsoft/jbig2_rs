//! Adaptive binary arithmetic decoding for JBIG2.
//!
//! This module implements the normative decoder in T.88 Annex E, the
//! arithmetic integer procedures in Annex A, and strict bounded input
//! handling. The Annex G complemented-code-register convention is not used:
//! it is output-equivalent but non-normative, while the Annex E convention is
//! easier to audit directly against the standard.

use core::fmt;

use crate::{DecodeError, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[path = "../specs/arithmetic.rs"]
pub(crate) mod arithmetic_spec;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[path = "../proofs/production_impl/arithmetic.rs"]
mod arithmetic_proofs;

#[cfg(verus_keep_ghost)]
#[path = "../proofs/shared/arithmetic.rs"]
mod arithmetic_shared;

#[cfg(verus_keep_ghost)]
#[path = "../proofs/spec_properties/arithmetic.rs"]
mod arithmetic_properties;

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MIN_INTERVAL: u32 = 0x8000;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const INTEGER_CONTEXT_COUNT: usize = 512;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const MAX_IAID_CODE_LENGTH: u8 = 31;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const RASTER_SIGNAL_EOF: u8 = 2;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const RASTER_SIGNAL_OVERFLOW: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct QeState {
    qe: u16,
    next_mps: u8,
    next_lps: u8,
    switch_mps: bool,
}

/// An arithmetic probability context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct ArithmeticContext {
    pub(crate) state_index: u8,
    pub(crate) mps: u8,
}

impl ArithmeticContext {
    /// Returns a context in the default `I = 0, MPS = 0` state.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out.state_index == 0,
            out.mps == 0,
            arithmetic_spec::context_valid(out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn new() -> Self {
        Self {
            state_index: 0,
            mps: 0,
        }
    }

    /// Constructs a validated context, for restoring saved statistics.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(context) =>
                state_index < 47
                && mps <= 1
                && context.state_index == state_index
                && context.mps == mps,
            Err(error) => (state_index >= 47 || mps > 1)
                && error == ArithmeticError::InvalidContextState { state_index, mps },
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn from_state(state_index: u8, mps: u8) -> Result<Self, ArithmeticError> {
        if state_index >= 47 || mps > 1 {
            return Err(ArithmeticError::InvalidContextState { state_index, mps });
        }
        Ok(Self { state_index, mps })
    }

    /// Restores the default probability state.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures
            final(self).state_index == 0,
            final(self).mps == 0,
            arithmetic_spec::context_valid(*final(self)),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    /// Returns the Table E.1 state index.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.state_index,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn state_index(self) -> u8 {
        self.state_index
    }

    /// Returns the current more-probable symbol.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.mps,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn mps(self) -> u8 {
        self.mps
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl Default for ArithmeticContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Resets every context in a statistics set.
#[cfg_attr(verus_keep_ghost, verus_spec(
    ensures
        final(contexts)@.len() == old(contexts)@.len(),
        forall|index: int| 0 <= index < final(contexts)@.len() ==>
            arithmetic_spec::context_valid(final(contexts)@[index]),
        forall|index: int| 0 <= index < final(contexts)@.len() ==>
            final(contexts)@[index].state_index == 0
            && final(contexts)@[index].mps == 0,
))]
pub(crate) fn reset_contexts(contexts: &mut [ArithmeticContext]) {
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            0 <= index <= contexts@.len(),
            contexts@.len() == old(contexts)@.len(),
            forall|completed: int| 0 <= completed < index ==>
                arithmetic_spec::context_valid(contexts@[completed]),
            forall|completed: int| 0 <= completed < index ==>
                contexts@[completed].state_index == 0
                && contexts@[completed].mps == 0,
        decreases contexts@.len() - index,
    ))]
    while index < contexts.len() {
        contexts[index] = ArithmeticContext::new();
        index += 1;
    }
}

/// A marker observed without consuming its two bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct ArithmeticMarker {
    /// Offset of the `0xff` marker prefix.
    pub(crate) offset: usize,
    /// Marker byte following the prefix.
    pub(crate) code: u8,
}

/// Arithmetic decoding failure.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum ArithmeticError {
    /// Existing crate-wide malformed-input classification.
    Malformed(MalformedError),
    /// A restored arithmetic context was outside Table E.1.
    InvalidContextState { state_index: u8, mps: u8 },
    /// Too few probability contexts were supplied.
    InsufficientContexts { required: usize, actual: usize },
    /// IAID cannot safely represent or index this code length.
    InvalidIaidCodeLength(u8),
}

impl fmt::Display for ArithmeticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => error.fmt(formatter),
            Self::InvalidContextState { state_index, mps } => write!(
                formatter,
                "invalid arithmetic context state I={state_index}, MPS={mps}"
            ),
            Self::InsufficientContexts { required, actual } => write!(
                formatter,
                "arithmetic decoding requires {required} contexts, but {actual} were supplied"
            ),
            Self::InvalidIaidCodeLength(length) => {
                write!(formatter, "unsupported IAID code length {length}")
            }
        }
    }
}

impl std::error::Error for ArithmeticError {}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<MalformedError> for ArithmeticError {
    fn from(error: MalformedError) -> Self {
        Self::Malformed(error)
    }
}

impl ArithmeticError {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == arithmetic_spec::decode_error(self),
    ))]
    pub(crate) fn into_decode(self) -> DecodeError {
        match self {
            Self::Malformed(error) => DecodeError::Malformed(error),
            Self::InvalidContextState { state_index, mps } => {
                DecodeError::Malformed(MalformedError::InvalidArithmeticContextState {
                    state_index,
                    mps,
                })
            }
            Self::InsufficientContexts { required, actual } => {
                DecodeError::Malformed(MalformedError::InsufficientArithmeticContexts {
                    required,
                    actual,
                })
            }
            Self::InvalidIaidCodeLength(length) => {
                DecodeError::Malformed(MalformedError::InvalidIaidCodeLength(length))
            }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<ArithmeticError> for DecodeError {
    fn from(error: ArithmeticError) -> Self {
        error.into_decode()
    }
}

#[cfg(verus_keep_ghost)]
verus! {

impl vstd::std_specs::convert::FromSpecImpl<MalformedError> for ArithmeticError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: MalformedError) -> ArithmeticError {
        ArithmeticError::Malformed(error)
    }
}

impl vstd::std_specs::convert::FromSpecImpl<ArithmeticError> for DecodeError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: ArithmeticError) -> DecodeError {
        arithmetic_spec::decode_error(error)
    }
}

}

/// Result of an Annex A arithmetic integer decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum ArithmeticInteger {
    /// A signed integer value.
    Value(i64),
    /// The special negative-zero out-of-band code.
    OutOfBand,
}

/// A bounded Annex E arithmetic decoder.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct ArithmeticDecoder<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) byte_position: usize,
    pub(crate) current_byte: u8,
    pub(crate) code: u32,
    pub(crate) interval: u32,
    pub(crate) bit_count: u8,
    pub(crate) marker: Option<ArithmeticMarker>,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == arithmetic_spec::eof_error(offset, needed, remaining),
))]
fn unexpected_eof(offset: usize, needed: usize, remaining: usize) -> ArithmeticError {
    let Ok(offset) = u64::try_from(offset) else {
        return ArithmeticError::Malformed(MalformedError::IntegerOverflow);
    };
    let Ok(needed) = u64::try_from(needed) else {
        return ArithmeticError::Malformed(MalformedError::IntegerOverflow);
    };
    let Ok(remaining) = u64::try_from(remaining) else {
        return ArithmeticError::Malformed(MalformedError::IntegerOverflow);
    };
    ArithmeticError::Malformed(MalformedError::UnexpectedEof {
        offset,
        needed,
        remaining,
    })
}

mod certainty;
mod certified_integers;
mod decoding;
mod integers;
mod probability;
mod refill;

#[cfg(verus_keep_ghost)]
pub(crate) use certainty::certainty_spec;
pub(crate) use certainty::PrefixCertainty;

use probability::QE_STATES;

#[cfg(test)]
mod tests;
