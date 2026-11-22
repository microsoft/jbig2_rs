//! Deliberate standalone compatibility, never a fallback from a failed decode.

use super::{
    allocation_size, inspect_file_header, ByteReader, DecodeError, LimitExceeded, LimitKind,
    Limits, MalformedError, Organization, SegmentType,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[path = "../../specs/standalone_policy.rs"]
mod policy_spec;

mod diagnostics;
mod ownership;

pub(crate) use diagnostics::record;
pub use diagnostics::{DecodePolicy, DecodeResult, Recovery, RecoveryKind};
use ownership::{close_page, open_index, recovery_owner};

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct Prepared {
    pub(crate) bytes: Vec<u8>,
    pub(crate) recoveries: Vec<Recovery>,
    pub(crate) limits: Limits,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> actual <= limit,
))]
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

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires start <= end <= input@.len(),
    ensures match out {
        Ok(()) => final(bytes)@ == old(bytes)@ + input@.subrange(start as int, end as int),
        Err(_) => final(bytes)@ == old(bytes)@,
    },
))]
fn append(bytes: &mut Vec<u8>, input: &[u8], start: usize, end: usize) -> Result<(), DecodeError> {
    let count = end - start;
    bytes
        .try_reserve(count)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: count as u64,
        })?;
    let mut index = start;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant start <= index <= end <= input@.len(),
            bytes@ == old(bytes)@ + input@.subrange(start as int, index as int),
        decreases end - index,
    ))]
    while index < end {
        bytes.push(input[index]);
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn is_region(kind: SegmentType) -> bool {
    matches!(
        kind,
        SegmentType::IntermediateTextRegion
            | SegmentType::ImmediateTextRegion
            | SegmentType::ImmediateLosslessTextRegion
            | SegmentType::IntermediateHalftoneRegion
            | SegmentType::ImmediateHalftoneRegion
            | SegmentType::ImmediateLosslessHalftoneRegion
            | SegmentType::IntermediateGenericRegion
            | SegmentType::ImmediateGenericRegion
            | SegmentType::ImmediateLosslessGenericRegion
            | SegmentType::IntermediateGenericRefinementRegion
            | SegmentType::ImmediateGenericRefinementRegion
            | SegmentType::ImmediateLosslessGenericRefinementRegion
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == policy_spec::complete_page_end(declared, observed, open@, closed@),
))]
fn complete_page_end(declared: Option<u32>, observed: usize, open: &[u32], closed: &[u32]) -> bool {
    let Some(count) = declared else {
        return false;
    };
    if count == 0 || observed != count as usize || !open.is_empty() || closed.len() != observed {
        return false;
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= closed@.len(),
            forall|done: int| 0 <= done < index ==> closed@[done] > 0,
            forall|left: int, right: int| 0 <= left < right < index ==> closed@[left] != closed@[right],
        decreases closed@.len() - index,
    ))]
    while index < closed.len() {
        if closed[index] == 0 {
            return false;
        }
        let mut previous = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant previous <= index < closed@.len(),
                forall|done: int| 0 <= done < previous ==> closed@[done] != closed@[index as int],
            decreases index - previous,
        ))]
        while previous < index {
            if closed[previous] == closed[index] {
                return false;
            }
            previous += 1;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
#[test]
fn final_page_boundary_rejects_invalid_closed_history() {
    assert!(complete_page_end(Some(2), 2, &[], &[1, 3]));
    assert!(!complete_page_end(Some(2), 2, &[], &[1, 1]));
    assert!(!complete_page_end(Some(2), 2, &[], &[0, 3]));
    assert!(!complete_page_end(Some(2), 2, &[], &[1]));
    assert!(!complete_page_end(Some(2), 2, &[4], &[1, 3]));
}

/// Preparation is itself bounded before allocation or traversal. Its result
/// still requires complete ordinary framing/reference/entropy validation.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(prepared) =>
            prepared.bytes@.len() <= input@.len()
            && prepared.limits.max_work_units <= limits.max_work_units
            && prepared.limits.max_retained_state_bytes <= limits.max_retained_state_bytes
            && prepared.limits.max_decoder_managed_memory_bytes
                <= limits.max_decoder_managed_memory_bytes,
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_lines)]
pub(crate) fn prepare(input: &[u8], limits: &Limits) -> Result<Prepared, DecodeError> {
    super::enforce_stream_limit(input.len(), limits)?;
    super::enforce_input_work(input.len(), limits)?;
    let file = inspect_file_header(input)?;
    if file.organization() != Organization::Sequential {
        // The separate header/payload layout is intentionally not rewritten.
        let cost = file.byte_length() as u64;
        budget(LimitKind::Work, cost, limits.max_work_units)?;
        let mut remaining = *limits;
        remaining.max_work_units -= cost;
        return Ok(Prepared {
            bytes: Vec::new(),
            recoveries: Vec::new(),
            limits: remaining,
        });
    }
    let length = input.len() as u64;
    let count = length / 11;
    let work = count
        .checked_mul(count)
        .and_then(|n| n.checked_add(length))
        .and_then(|n| n.checked_mul(4))
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(LimitKind::Work, work, limits.max_work_units)?;
    // Include the bounded scanner's header/reference and ownership scratch,
    // not just the output copy. Only the copy remains live during decoding.
    let scratch = length
        .checked_mul(64)
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(
        LimitKind::Memory,
        scratch,
        limits.max_decoder_managed_memory_bytes,
    )?;
    budget(
        LimitKind::RetainedState,
        length,
        limits.max_retained_state_bytes,
    )?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve(input.len())
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: length,
        })?;
    let copy_bytes = bytes.capacity() as u64;
    if copy_bytes < length || copy_bytes > scratch {
        return Err(MalformedError::IntegerOverflow.into());
    }
    budget(
        LimitKind::RetainedState,
        copy_bytes,
        limits.max_retained_state_bytes,
    )?;
    let mut remaining = *limits;
    remaining.max_work_units -= work;
    remaining.max_retained_state_bytes -= copy_bytes;
    remaining.max_decoder_managed_memory_bytes -= scratch;
    let mut recoveries = Vec::new();
    let mut open = Vec::<u32>::new();
    let mut closed = Vec::<u32>::new();
    let header_len = file.byte_length();
    if header_len > input.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    append(&mut bytes, input, 0, header_len)?;
    budget(LimitKind::Memory, bytes.capacity() as u64, copy_bytes)?;
    let mut reader = ByteReader::new(&input[header_len..]);
    let mut previous = None;
    let mut segments = 0_usize;
    let mut pages_seen = 0_usize;
    let mut references = 0_u64;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant reader.position <= reader.bytes@.len(),
            open@.len() + closed@.len() == pages_seen,
            reader.bytes@.len() + header_len == input@.len(),
            bytes@.len() <= header_len + reader.position,
            remaining.max_work_units <= limits.max_work_units,
            remaining.max_retained_state_bytes <= limits.max_retained_state_bytes,
            remaining.max_decoder_managed_memory_bytes <= limits.max_decoder_managed_memory_bytes,
            remaining.max_decoder_managed_memory_bytes as nat + scratch as nat
                <= limits.max_decoder_managed_memory_bytes as nat,
        decreases reader.bytes@.len() - reader.position,
    ))]
    while reader.remaining() != 0 {
        super::enforce_segment_limit(segments, limits)?;
        segments += 1;
        let start = header_len
            .checked_add(reader.position())
            .ok_or(MalformedError::IntegerOverflow)?;
        let available = reader.remaining_bytes();
        if available.len() >= 11 && available[4] == 1 && available[5] == 0 {
            let number = super::parsing::read_be_u32_at(available, 0);
            check_number(previous, number)?;
            previous = Some(number);
            let page = u32::from(available[6]);
            if page != 0 && open_index(&open, page).is_none() {
                return Err(MalformedError::ReservedSegmentType(1).into());
            }
            let length = super::parsing::read_be_u32_at(available, 7);
            if length == u32::MAX {
                return Err(MalformedError::ReservedSegmentType(1).into());
            }
            reader.read_bytes(11)?;
            reader.read_bytes(
                usize::try_from(length).map_err(|_| MalformedError::IntegerOverflow)?,
            )?;
            record(
                &mut recoveries,
                Recovery {
                    page,
                    byte_offset: Some(start),
                    kind: RecoveryKind::UnreferencedReservedType1,
                },
                &mut remaining,
            )?;
            continue;
        }
        let header = super::parse_segment_header(&mut reader, limits)?;
        references = references
            .checked_add(header.referred_segments.len() as u64)
            .ok_or(MalformedError::IntegerOverflow)?;
        budget(
            LimitKind::TotalReferences,
            references,
            limits.max_total_references,
        )?;
        scanner_storage(&bytes, &open, &closed, &header, scratch)?;
        check_number(previous, header.number)?;
        previous = Some(header.number);
        let payload_start = header_len
            .checked_add(reader.position())
            .ok_or(MalformedError::IntegerOverflow)?;
        let payload = super::parsing::read_payload(&mut reader, &header, limits)?;
        let end = header_len
            .checked_add(reader.position())
            .ok_or(MalformedError::IntegerOverflow)?;
        if end > input.len() || start > end {
            return Err(MalformedError::IntegerOverflow.into());
        }
        let output_start = bytes.len();
        append(&mut bytes, input, start, end)?;
        budget(LimitKind::Memory, bytes.capacity() as u64, copy_bytes)?;
        let mut page = header.page_association;
        if header.segment_type == SegmentType::PageInformation {
            super::enforce_page_limit(pages_seen, limits)?;
            pages_seen += 1;
            let requested_bytes = allocation_size::<u32>(
                open.len()
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?,
            )?;
            open.try_reserve(1)
                .map_err(|_| DecodeError::Allocation { requested_bytes })?;
            open.push(page);
            scanner_storage(&bytes, &open, &closed, &header, scratch)?;
        } else if header.segment_type == SegmentType::EndOfPage {
            let mut owner_index = open_index(&open, page);
            if page == 0 || owner_index.is_none() {
                if !payload.is_empty() || !header.referred_segments.is_empty() {
                    return Err(MalformedError::InvalidPageAssociation {
                        segment: header.number,
                        page,
                    }
                    .into());
                }
                let (owner, kind) = recovery_owner(header.number, page, &open, &closed)?;
                page = owner;
                owner_index = Some(0);
                let flag_offset = start
                    .checked_add(4)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let flags = *input
                    .get(flag_offset)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let width = if flags & 0x40 != 0 { 4 } else { 1 };
                let offset = payload_start
                    .checked_sub(4 + width)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let relative = offset
                    .checked_sub(start)
                    .ok_or(MalformedError::IntegerOverflow)?;
                patch_page(&mut bytes, output_start, relative, width, page)?;
                record(
                    &mut recoveries,
                    Recovery {
                        page,
                        byte_offset: Some(offset),
                        kind,
                    },
                    &mut remaining,
                )?;
            }
            if let Some(index) = owner_index {
                close_page(&mut open, &mut closed, index)?;
                scanner_storage(&bytes, &open, &closed, &header, scratch)?;
            }
        }
        if is_region(header.segment_type) && payload.len() >= 17 && payload[16] & 0xf0 == 0x10 {
            let offset = payload_start
                .checked_add(16)
                .ok_or(MalformedError::IntegerOverflow)?;
            let normalized = output_start
                .checked_add(offset - start)
                .ok_or(MalformedError::IntegerOverflow)?;
            let flag = bytes
                .get_mut(normalized)
                .ok_or(MalformedError::IntegerOverflow)?;
            if *flag & 0xf0 != 0x10 {
                return Err(MalformedError::IntegerOverflow.into());
            }
            *flag = mask_region_flag(*flag);
            record(
                &mut recoveries,
                Recovery {
                    page,
                    byte_offset: Some(offset),
                    kind: RecoveryKind::ReservedRegionBit4,
                },
                &mut remaining,
            )?;
        }
        let final_page = header.segment_type == SegmentType::EndOfPage
            && page > 0
            && payload.is_empty()
            && header.referred_segments.is_empty()
            && reader.remaining_bytes() == [13, 10]
            && complete_page_end(file.page_count(), pages_seen, &open, &closed);
        if header.segment_type == SegmentType::EndOfFile
            && (page != 0 || !payload.is_empty() || !header.referred_segments.is_empty())
        {
            return Err(MalformedError::InvalidPageAssociation {
                segment: header.number,
                page,
            }
            .into());
        }
        if header.segment_type == SegmentType::EndOfFile || final_page {
            if reader.remaining_bytes() == [13, 10] {
                record(
                    &mut recoveries,
                    Recovery {
                        page: 0,
                        byte_offset: Some(end),
                        kind: RecoveryKind::TrailingCrLf,
                    },
                    &mut remaining,
                )?;
                reader.read_bytes(2)?;
            } else if header.segment_type == SegmentType::EndOfFile && reader.remaining() != 0 {
                return Err(MalformedError::TrailingData(reader.remaining() as u64).into());
            }
        }
    }
    remaining.max_decoder_managed_memory_bytes = remaining
        .max_decoder_managed_memory_bytes
        .checked_add(scratch - copy_bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(Prepared {
        bytes,
        recoveries,
        limits: remaining,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out { Ok(observed_bytes) => observed_bytes <= limit, Err(_) => true },
))]
fn scanner_storage(
    bytes: &Vec<u8>,
    open: &Vec<u32>,
    closed: &Vec<u32>,
    header: &super::SegmentHeader,
    limit: u64,
) -> Result<u64, DecodeError> {
    let used = allocation_size::<u32>(header.referred_segments.capacity())?
        .checked_add(allocation_size::<u8>(
            header.retention_flags.values.capacity(),
        )?)
        .and_then(|n| n.checked_add(bytes.capacity() as u64))
        .ok_or(MalformedError::IntegerOverflow)?;
    let closed_bytes = allocation_size::<u32>(closed.capacity())?;
    let used = used
        .checked_add(allocation_size::<u32>(open.capacity())?)
        .and_then(|n| n.checked_add(closed_bytes))
        .ok_or(MalformedError::IntegerOverflow)?;
    budget(LimitKind::Memory, used, limit)?;
    Ok(used)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn check_number(previous: Option<u32>, current: u32) -> Result<(), DecodeError> {
    if let Some(previous) = previous {
        if current <= previous {
            return Err(MalformedError::NonIncreasingSegmentNumber { previous, current }.into());
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => policy_spec::page_patch(
            old(bytes)@, final(bytes)@, start + offset, width, page),
        Err(_) => true,
    },
))]
fn patch_page(
    bytes: &mut [u8],
    start: usize,
    offset: usize,
    width: usize,
    page: u32,
) -> Result<(), DecodeError> {
    if (width != 1 && width != 4) || (width == 1 && page > 255) {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let begin = start
        .checked_add(offset)
        .ok_or(MalformedError::IntegerOverflow)?;
    let end = begin
        .checked_add(width)
        .ok_or(MalformedError::IntegerOverflow)?;
    if end > bytes.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= width,
            width == 1 || width == 4,
            width == 1 ==> page <= 255,
            begin == start + offset,
            end == begin + width,
            end <= bytes@.len(),
            bytes@.len() == old(bytes)@.len(),
            forall|position: int| 0 <= position < bytes@.len() ==>
                bytes@[position] == if begin <= position < begin + index {
                    policy_spec::page_byte(page, width, position - begin)
                } else { old(bytes)@[position] },
        decreases width - index,
    ))]
    while index < width {
        let value = if width == 1 {
            u8::try_from(page).map_err(|_| MalformedError::IntegerOverflow)?
        } else if index == 0 {
            u8::try_from(page >> 24).map_err(|_| MalformedError::IntegerOverflow)?
        } else if index == 1 {
            u8::try_from((page >> 16) & 255).map_err(|_| MalformedError::IntegerOverflow)?
        } else if index == 2 {
            u8::try_from((page >> 8) & 255).map_err(|_| MalformedError::IntegerOverflow)?
        } else {
            u8::try_from(page & 255).map_err(|_| MalformedError::IntegerOverflow)?
        };
        bytes[begin + index] = value;
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) =>
            super::document_spec::document_segment_count(&result.0) as nat == result.1@.len()
            && match super::document_spec::document_declared_page_count(&result.0) {
                Some(count) => super::document_spec::document_pages(&result.0).len() == count as nat,
                None => true,
            },
        Err(_) => true,
    },
))]
pub(crate) fn parse<'a>(
    input: &'a [u8],
    recoveries: &mut Vec<Recovery>,
    limits: &mut Limits,
) -> Result<(super::DocumentInfo, Vec<super::ParsedSegment<'a>>), DecodeError> {
    let file = inspect_file_header(input)?;
    if file.uses_colour_regions() {
        return Err(DecodeError::Unsupported(crate::UnsupportedFeature::Colour));
    }
    let start = file.byte_length();
    if start > input.len() || file.organization() != Organization::Sequential {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let segments = super::parsing::parse_sequential(&input[start..], limits, true)?;
    super::enforce_work_limit(input.len(), &segments, limits)?;
    super::validate_references(&segments, None, limits)?;
    super::validate_profiles(&segments)?;
    let (mut states, order) = super::pages::collect_page_states(&segments, limits)?;
    let actual = u32::try_from(states.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= states@.len(),
            states@.len() == order@.len(),
            states@.len() == actual,
        decreases states@.len() - index,
    ))]
    while index < states.len() {
        let (page, state) = &mut states[index];
        if !state.ended {
            validate_missing_boundary(&segments, *page)?;
            if state.info.striped || state.info.height.is_none() {
                super::pages::validate_implicit_stripes(
                    &segments,
                    *page,
                    state.info.width,
                    limits,
                )?;
            }
            state.finish()?;
            record(
                recoveries,
                Recovery {
                    page: *page,
                    byte_offset: None,
                    kind: RecoveryKind::MissingEndOfPage,
                },
                limits,
            )?;
        }
        index += 1;
    }
    let pages = super::pages::finalize_pages(states, &order, &segments, limits, false)?;
    if let Some(declared) = file.page_count() {
        if actual != declared {
            record(
                recoveries,
                Recovery {
                    page: 0,
                    byte_offset: Some(9),
                    kind: RecoveryKind::ObservedPageCount,
                },
                limits,
            )?;
        }
    }
    let declared_page_count = if file.page_count().is_some() {
        Some(actual)
    } else {
        None
    };
    let info = super::DocumentInfo {
        organization: file.organization(),
        declared_page_count,
        segment_count: u32::try_from(segments.len())
            .map_err(|_| MalformedError::IntegerOverflow)?,
        pages,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        super::document_spec::document_fields(&info);
    }
    Ok((info, segments))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==>
        policy_spec::boundary_state(segments@, page, segments@.len()).2
        && policy_spec::no_page_revisit(segments@, page, segments@.len()),
))]
fn validate_missing_boundary(
    segments: &[super::ParsedSegment<'_>],
    page: u32,
) -> Result<(), DecodeError> {
    let mut started = false;
    let mut closed = false;
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segments@.len(),
            started == policy_spec::boundary_state(segments@, page, index as nat).0,
            closed == policy_spec::boundary_state(segments@, page, index as nat).1,
            policy_spec::boundary_state(segments@, page, index as nat).2,
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(policy_spec::boundary_state, 1); }
        let header = &segments[index].header;
        if started && matches!(header.segment_type, SegmentType::PageInformation) {
            closed = true;
        }
        if header.page_association == page {
            if closed {
                return Err(MalformedError::MissingEndOfPage(page).into());
            }
            started = true;
        }
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! { policy_spec::boundary_witnesses(segments@, page, segments@.len()); }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires flag & 0xf0u8 == 0x10u8,
    ensures policy_spec::reserved_mask_result(flag, out),
))]
fn mask_region_flag(flag: u8) -> u8 {
    let normalized = flag & !0x10;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(!0x10u8 == 0xefu8) by (bit_vector);
        assert((flag & 0xefu8) & 0x0fu8 == flag & 0x0fu8) by (bit_vector);
        assert(flag & 0xf0u8 == 0x10u8 ==> (flag & 0xefu8) & 0xf0u8 == 0u8)
            by (bit_vector);
    }
    normalized
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == exists|index: int| 0 <= index < recoveries@.len()
        && recoveries@[index].page == page
        && recoveries@[index].kind == RecoveryKind::MissingEndOfPage,
))]
pub(crate) fn has_implicit_end(recoveries: &[Recovery], page: u32) -> bool {
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= recoveries@.len(),
            forall|prior: int| 0 <= prior < index ==> !(
                recoveries@[prior].page == page
                && recoveries@[prior].kind == RecoveryKind::MissingEndOfPage),
        decreases recoveries@.len() - index,
    ))]
    while index < recoveries.len() {
        if recoveries[index].page == page
            && matches!(recoveries[index].kind, RecoveryKind::MissingEndOfPage)
        {
            return true;
        }
        index += 1;
    }
    false
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == exists|index: int| 0 <= index < recoveries@.len()
        && recoveries@[index].kind == RecoveryKind::MissingEndOfPage,
))]
pub(crate) fn has_missing_end(recoveries: &[Recovery]) -> bool {
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= recoveries@.len(),
            forall|prior: int| 0 <= prior < index ==>
                recoveries@[prior].kind != RecoveryKind::MissingEndOfPage,
        decreases recoveries@.len() - index,
    ))]
    while index < recoveries.len() {
        if matches!(recoveries[index].kind, RecoveryKind::MissingEndOfPage) {
            return true;
        }
        index += 1;
    }
    false
}
