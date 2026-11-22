//! Standalone policy types and bounded recovery diagnostic accounting.

use super::{allocation_size, budget, DecodeError, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Validation policy for standalone files. Existing entry points remain strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum DecodePolicy {
    /// Require the ordinary standalone syntax and page lifecycle.
    Strict,
    /// Apply only the bounded, observable exceptions documented below.
    Compatibility,
}

/// An explicit standalone compatibility exception, not partial-output decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum RecoveryKind {
    /// A zero-associated end-of-page had exactly one open owner.
    ZeroAssociatedEndOfPage,
    /// A nonzero EOP named an already-closed earlier page and had one open owner.
    ClosedPageEndOfPage,
    /// A complete page ended at the next page-information or exact stream end.
    MissingEndOfPage,
    /// The declared count was replaced by the number of fully validated pages.
    ObservedPageCount,
    /// A single CR LF suffix followed an independently validated document end.
    TrailingCrLf,
    /// Only reserved region bit 4 was cleared (bit 3 is colour, not reserved).
    ReservedRegionBit4,
    /// An unretained, unreferenced, fixed-length type-1 segment was omitted.
    UnreferencedReservedType1,
}

/// A recovery location in the original input, or page closure at its boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Recovery {
    /// Affected page; zero denotes file-level metadata.
    pub page: u32,
    /// Byte offset in the original input; `None` for inferred page closure.
    pub byte_offset: Option<usize>,
    /// The specific exception.
    pub kind: RecoveryKind,
}

/// Completely decoded output with every compatibility exception reported.
///
/// Errors return neither pages nor diagnostics. An empty list means no
/// exception was used. This is not a partial-output or entropy-repair policy.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct DecodeResult<T> {
    /// The completely validated and decoded result.
    pub value: T,
    /// Exceptions in preparation order, followed by page/count validation.
    pub recoveries: Vec<Recovery>,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        final(limits).max_work_units == old(limits).max_work_units,
        final(limits).max_retained_state_bytes <= old(limits).max_retained_state_bytes,
        final(limits).max_decoder_managed_memory_bytes <= old(limits).max_decoder_managed_memory_bytes,
        out.is_ok() ==> final(recoveries)@ == old(recoveries)@.push(recovery),
))]
pub(crate) fn record(
    recoveries: &mut Vec<Recovery>,
    recovery: Recovery,
    limits: &mut Limits,
) -> Result<(), DecodeError> {
    let before = allocation_size::<Recovery>(recoveries.capacity())?;
    let bytes = allocation_size::<Recovery>(1)?;
    budget(
        LimitKind::RetainedState,
        bytes,
        limits.max_retained_state_bytes,
    )?;
    budget(
        LimitKind::Memory,
        bytes,
        limits.max_decoder_managed_memory_bytes,
    )?;
    recoveries
        .try_reserve(1)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: bytes,
        })?;
    let bytes = allocation_size::<Recovery>(recoveries.capacity())?
        .checked_sub(before)
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(
        LimitKind::RetainedState,
        bytes,
        limits.max_retained_state_bytes,
    )?;
    budget(
        LimitKind::Memory,
        bytes,
        limits.max_decoder_managed_memory_bytes,
    )?;
    recoveries.push(recovery);
    limits.max_retained_state_bytes -= bytes;
    limits.max_decoder_managed_memory_bytes -= bytes;
    Ok(())
}
