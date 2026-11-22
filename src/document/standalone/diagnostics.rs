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
    /// Complete Huffman text instances ended with only unambiguous zero padding.
    MissingFinalHuffmanStripEnd,
}

/// A recovery location in the original input, or page closure at its boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Recovery {
    /// Affected page; zero denotes file-level metadata.
    pub page: u32,
    /// Original byte offset; `None` for an inferred page or entropy boundary.
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
    /// Preparation/page/count exceptions, followed by entropy-boundary recoveries.
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

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        final(recoveries)@ == old(recoveries)@,
        final(limits).max_work_units == old(limits).max_work_units,
        final(limits).max_retained_state_bytes <= old(limits).max_retained_state_bytes,
        final(limits).max_decoder_managed_memory_bytes <= old(limits).max_decoder_managed_memory_bytes,
))]
pub(crate) fn reserve(
    recoveries: &mut Vec<Recovery>,
    additional: usize,
    limits: &mut Limits,
) -> Result<(), DecodeError> {
    if additional == 0 {
        return Ok(());
    }
    let before = allocation_size::<Recovery>(recoveries.capacity())?;
    let requested = allocation_size::<Recovery>(additional)?;
    budget(
        LimitKind::RetainedState,
        requested,
        limits.max_retained_state_bytes,
    )?;
    budget(
        LimitKind::Memory,
        requested,
        limits.max_decoder_managed_memory_bytes,
    )?;
    let needed = recoveries
        .len()
        .checked_add(additional)
        .ok_or(MalformedError::IntegerOverflow)?;
    recoveries
        .try_reserve(additional)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: requested,
        })?;
    if recoveries.capacity() < needed {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let growth = allocation_size::<Recovery>(recoveries.capacity())?
        .checked_sub(before)
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(
        LimitKind::RetainedState,
        growth,
        limits.max_retained_state_bytes,
    )?;
    budget(
        LimitKind::Memory,
        growth,
        limits.max_decoder_managed_memory_bytes,
    )?;
    limits.max_retained_state_bytes -= growth;
    limits.max_decoder_managed_memory_bytes -= growth;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> final(recoveries)@ == old(recoveries)@.push(recovery),
))]
pub(crate) fn record_reserved(
    recoveries: &mut Vec<Recovery>,
    recovery: Recovery,
) -> Result<(), DecodeError> {
    if recoveries.len() >= recoveries.capacity() {
        return Err(
            MalformedError::InvalidPageState("unreserved entropy recovery diagnostic").into(),
        );
    }
    recoveries.push(recovery);
    Ok(())
}

#[cfg(test)]
#[test]
fn entropy_diagnostics_reserve_and_debit_before_recording_without_growth() {
    let mut recoveries = Vec::new();
    let mut limits = Limits {
        max_decoder_managed_memory_bytes: 1,
        ..Limits::default()
    };
    assert!(matches!(reserve(&mut recoveries, 1, &mut limits),
        Err(DecodeError::LimitExceeded(detail)) if detail.kind == LimitKind::Memory));
    assert!(recoveries.is_empty());

    let mut limits = Limits::default();
    let before = limits;
    reserve(&mut recoveries, 2, &mut limits).unwrap();
    let capacity = recoveries.capacity();
    let allocation = (capacity * core::mem::size_of::<Recovery>()) as u64;
    assert_eq!(
        limits.max_retained_state_bytes,
        before.max_retained_state_bytes - allocation
    );
    assert_eq!(
        limits.max_decoder_managed_memory_bytes,
        before.max_decoder_managed_memory_bytes - allocation
    );
    let recovery = Recovery {
        page: 1,
        byte_offset: None,
        kind: RecoveryKind::MissingFinalHuffmanStripEnd,
    };
    record_reserved(&mut recoveries, recovery).unwrap();
    assert_eq!(recoveries.capacity(), capacity);
    assert_eq!(recoveries, [recovery]);
    assert!(record_reserved(&mut Vec::new(), recovery).is_err());
}
