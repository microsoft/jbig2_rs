//! Explicit, bounded recovery of supported embedded metadata inconsistencies.

use super::{
    allocation_size, parse_embedded_document, DecodeError, Decoder, DecoderContext, DocumentInfo,
    GlobalSegments, LimitExceeded, LimitKind, Limits, MalformedError, ParsedData, SegmentType,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Caller-selected policy for embedded JBIG2 streams. Existing APIs stay strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum EmbeddedPolicy {
    /// Apply the same validation as the existing embedded entry points.
    Strict,
    /// Allow only the documented page-flag and complete-marker recoveries.
    BestEffort,
}

/// A metadata inconsistency explicitly recovered by the embedded policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum EmbeddedRecoveryKind {
    /// A page used an intermediate region despite its auxiliary-buffer flag.
    AuxiliaryBuffersFlag,
    /// A page used a refinement region despite its refinement-capability flag.
    RefinementsFlag,
    /// A complete known-length arithmetic generic region ended in FF D0.
    ///
    /// BYTEIN treats this existing marker identically to FF AC; only its
    /// nonstandard terminator code is normalized. Missing bytes are never added.
    ArithmeticTerminatorCode,
}

/// One allowlisted metadata or complete-marker override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct EmbeddedRecovery {
    /// Segment whose metadata or complete terminator was overridden.
    pub segment: u32,
    /// Affected page number.
    pub page: u32,
    /// Offset of the overridden byte in the supplied stream.
    pub byte_offset: usize,
    /// The specific, allowlisted recovery.
    pub kind: EmbeddedRecoveryKind,
}

/// Successful embedded output and explicit recovery diagnostics.
///
/// An empty `recoveries` list means no source metadata was overridden. Errors
/// never return a partial bitmap. The supplied source and globals are immutable.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct EmbeddedResult<T> {
    /// Inspected metadata or completely decoded document.
    pub value: T,
    /// Every applied recovery, in source page-information order.
    pub recoveries: Vec<EmbeddedRecovery>,
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct PreparedEmbedded {
    bytes: Option<Vec<u8>>,
    recoveries: Vec<EmbeddedRecovery>,
    pub(crate) limits: Limits,
}

#[cfg(verus_keep_ghost)]
verus! {
    pub(crate) closed spec fn recovery_view(prepared: &PreparedEmbedded) -> Seq<EmbeddedRecovery> {
        prepared.recoveries@
    }
}

impl PreparedEmbedded {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) fn limits(&self) -> Limits {
        self.limits
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) fn input<'a>(&'a self, original: &'a [u8]) -> &'a [u8] {
        match self.bytes.as_ref() {
            Some(bytes) => bytes,
            None => original,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match result {
            Ok(value) => match out {
                Ok(output) => output.value == value && output.recoveries@ == recovery_view(&self),
                Err(_) => false,
            },
            Err(error) => out == Err(error),
        },
    ))]
    pub(crate) fn finish<T>(
        self,
        result: Result<T, DecodeError>,
    ) -> Result<EmbeddedResult<T>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(recovery_view); }
        Ok(EmbeddedResult {
            value: result?,
            recoveries: self.recoveries,
        })
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn budget(kind: LimitKind, actual: u64, limit: u64) -> Result<(), DecodeError> {
    if actual > limit {
        return Err(LimitExceeded {
            kind,
            limit,
            actual,
        }
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn record(
    recoveries: &mut Vec<EmbeddedRecovery>,
    recovery: EmbeddedRecovery,
    input_bytes: u64,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let count = recoveries
        .len()
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    let bytes = allocation_size::<EmbeddedRecovery>(count)?;
    let total = input_bytes
        .checked_add(bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(
        LimitKind::RetainedState,
        total,
        limits.max_retained_state_bytes,
    )?;
    budget(
        LimitKind::Memory,
        total,
        limits.max_decoder_managed_memory_bytes,
    )?;
    recoveries
        .try_reserve(1)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: bytes,
        })?;
    recoveries.push(recovery);
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn required_flags(segments: &[super::ParsedSegment<'_>], page: u32) -> u8 {
    let mut required = 0_u8;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segments@.len(),
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        let region = &segments[index].header;
        if region.page_association == page {
            required |= match region.segment_type {
                SegmentType::IntermediateTextRegion
                | SegmentType::IntermediateHalftoneRegion
                | SegmentType::IntermediateGenericRegion => 0x20,
                SegmentType::IntermediateGenericRefinementRegion => 0x22,
                SegmentType::ImmediateGenericRefinementRegion
                | SegmentType::ImmediateLosslessGenericRefinementRegion => 0x02,
                _ => 0,
            };
        }
        index += 1;
    }
    required
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn normalized_bytes(input: &[u8], recoveries: &[EmbeddedRecovery]) -> Result<Vec<u8>, DecodeError> {
    let input_bytes = u64::try_from(input.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve(input.len())
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: input_bytes,
        })?;
    let mut byte = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant byte <= input@.len(), bytes@.len() == byte,
        decreases input@.len() - byte,
    ))]
    while byte < input.len() {
        bytes.push(input[byte]);
        byte += 1;
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= recoveries@.len(),
        decreases recoveries@.len() - index,
    ))]
    while index < recoveries.len() {
        let recovery = recoveries[index];
        let flag = bytes
            .get_mut(recovery.byte_offset)
            .ok_or(MalformedError::InvalidPageState(
                "recovery offset is outside the stream",
            ))?;
        match recovery.kind {
            EmbeddedRecoveryKind::AuxiliaryBuffersFlag => *flag |= 0x20,
            EmbeddedRecoveryKind::RefinementsFlag => *flag |= 0x02,
            EmbeddedRecoveryKind::ArithmeticTerminatorCode => *flag = 0xac,
        }
        index += 1;
    }
    Ok(bytes)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn record_page_flags(
    recoveries: &mut Vec<EmbeddedRecovery>,
    header: &super::SegmentHeader,
    flag_offset: usize,
    flags: u8,
    required: u8,
    input_bytes: u64,
    limits: &Limits,
) -> Result<(), DecodeError> {
    if required & 0x20 != 0 && flags & 0x20 == 0 {
        record(
            recoveries,
            EmbeddedRecovery {
                segment: header.number,
                page: header.page_association,
                byte_offset: flag_offset,
                kind: EmbeddedRecoveryKind::AuxiliaryBuffersFlag,
            },
            input_bytes,
            limits,
        )?;
    }
    if required & 0x02 != 0 && flags & 0x02 == 0 {
        record(
            recoveries,
            EmbeddedRecovery {
                segment: header.number,
                page: header.page_association,
                byte_offset: flag_offset,
                kind: EmbeddedRecoveryKind::RefinementsFlag,
            },
            input_bytes,
            limits,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn record_arithmetic_marker(
    recoveries: &mut Vec<EmbeddedRecovery>,
    segment: &super::ParsedSegment<'_>,
    offset: usize,
    input_bytes: u64,
    limits: &Limits,
) -> Result<(), DecodeError> {
    if matches!(
        segment.header.segment_type,
        SegmentType::IntermediateGenericRegion
            | SegmentType::ImmediateGenericRegion
            | SegmentType::ImmediateLosslessGenericRegion
    ) && segment.header.data_length.is_some()
        && matches!(segment.payload.get(17), Some(flags) if *flags & 1 == 0)
        && segment.payload.ends_with(&[0xff, 0xd0])
    {
        let (_, header_length) =
            crate::generic_region::parse_generic_region_header(segment.payload, limits)
                .map_err(DecodeError::from)?;
        let coded_length = segment
            .payload
            .len()
            .checked_sub(header_length)
            .ok_or(MalformedError::IntegerOverflow)?;
        if coded_length < 2 {
            return Err(MalformedError::InvalidGenericRegion(
                crate::GenericRegionErrorKind::InvalidArithmeticTerminator,
            )
            .into());
        }
        let byte_offset = offset
            .checked_add(segment.payload.len())
            .and_then(|end| end.checked_sub(1))
            .ok_or(MalformedError::IntegerOverflow)?;
        record(
            recoveries,
            EmbeddedRecovery {
                segment: segment.header.number,
                page: segment.header.page_association,
                byte_offset,
                kind: EmbeddedRecoveryKind::ArithmeticTerminatorCode,
            },
            input_bytes,
            limits,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) fn prepare(
    input: &[u8],
    globals: Option<&GlobalSegments>,
    policy: EmbeddedPolicy,
    limits: &Limits,
) -> Result<PreparedEmbedded, DecodeError> {
    if policy == EmbeddedPolicy::Strict {
        return Ok(PreparedEmbedded {
            bytes: None,
            recoveries: Vec::new(),
            limits: *limits,
        });
    }
    // Validate the original framing, references and page lifecycle before any change.
    let (info, segments) = parse_embedded_document(input, limits, globals)?;
    let input_bytes = u64::try_from(input.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let count = u64::try_from(segments.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let page_count =
        u64::try_from(info.pages().len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let references = super::references::count_total_references(&segments)?;
    let mut extra_work = count
        .checked_mul(page_count)
        .and_then(|work| work.checked_add(input_bytes))
        .and_then(|work| work.checked_add(count))
        .and_then(|work| work.checked_add(references))
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(LimitKind::Work, extra_work, limits.max_work_units)?;
    let mut recoveries = Vec::new();
    let mut offset = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segments@.len(),
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        let segment = &segments[index];
        offset = offset
            .checked_add(segment.header.byte_length)
            .ok_or(MalformedError::IntegerOverflow)?;
        if let ParsedData::Page(_) = segment.data {
            let flag_offset = offset
                .checked_add(16)
                .ok_or(MalformedError::IntegerOverflow)?;
            let flags = *input
                .get(flag_offset)
                .ok_or(MalformedError::InvalidPageState(
                    "page-information flag offset is outside the stream",
                ))?;
            let required = required_flags(&segments, segment.header.page_association);
            record_page_flags(
                &mut recoveries,
                &segment.header,
                flag_offset,
                flags,
                required,
                input_bytes,
                limits,
            )?;
        } else {
            record_arithmetic_marker(&mut recoveries, segment, offset, input_bytes, limits)?;
        }
        offset = offset
            .checked_add(segment.payload.len())
            .ok_or(MalformedError::IntegerOverflow)?;
        index += 1;
    }
    if offset != input.len() {
        return Err(MalformedError::InvalidPageState(
            "embedded segment windows do not cover the stream",
        )
        .into());
    }
    let retained = if recoveries.is_empty() {
        0
    } else {
        input_bytes
            .checked_add(allocation_size::<EmbeddedRecovery>(recoveries.len())?)
            .ok_or(MalformedError::IntegerOverflow)?
    };
    let bytes = if recoveries.is_empty() {
        None
    } else {
        extra_work = extra_work
            .checked_add(input_bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        budget(LimitKind::Work, extra_work, limits.max_work_units)?;
        Some(normalized_bytes(input, &recoveries)?)
    };
    let mut remaining = *limits;
    remaining.max_work_units = remaining
        .max_work_units
        .checked_sub(extra_work)
        .ok_or(MalformedError::IntegerOverflow)?;
    remaining.max_retained_state_bytes = remaining
        .max_retained_state_bytes
        .checked_sub(retained)
        .ok_or(MalformedError::IntegerOverflow)?;
    remaining.max_decoder_managed_memory_bytes = remaining
        .max_decoder_managed_memory_bytes
        .checked_sub(retained)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(PreparedEmbedded {
        bytes,
        recoveries,
        limits: remaining,
    })
}

impl Decoder {
    /// Inspects an embedded stream with an explicit, observable recovery policy.
    ///
    /// Best effort overrides inconsistent page auxiliary/refinement flags
    /// (T.88 7.4.8.5) and normalizes an existing FF D0 marker at the exact end of
    /// a known-length arithmetic generic region to FF AC (E.2.9, E.3.3).
    /// It never adds compressed data or repairs framing or dimensions.
    /// Preparation debits work and temporary storage from the existing budgets;
    /// any later resource error reports the remaining effective budget.
    ///
    /// # Errors
    ///
    /// Returns typed validation, resource or allocation errors. There is no retry.
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(output) => super::document_spec::embedded_metadata(&output.value),
            Err(_) => true,
        },
    ))]
    pub fn inspect_embedded_with_policy(
        &self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        policy: EmbeddedPolicy,
    ) -> Result<EmbeddedResult<DocumentInfo>, DecodeError> {
        let prepared = prepare(input, globals, policy, self.limits())?;
        let result =
            Decoder::new(prepared.limits).inspect_embedded_internal(prepared.input(input), globals);
        prepared.finish(result)
    }
}

impl DecoderContext<'_> {
    /// Inspects with the explicit policy of [`Decoder::inspect_embedded_with_policy`].
    ///
    /// # Errors
    ///
    /// Returns the same typed errors and always performs normal context cleanup.
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures super::document_spec::decoder_context_finished(old(self), final(self)),
    ))]
    pub fn inspect_embedded_with_policy(
        &mut self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        policy: EmbeddedPolicy,
    ) -> Result<EmbeddedResult<DocumentInfo>, DecodeError> {
        let result = self
            .decoder
            .inspect_embedded_with_policy(input, globals, policy);
        self.finish_call();
        result
    }
}
