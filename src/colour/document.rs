use super::{palette, ColourPage, DecodedColourDocument, RgbBitmap};
use crate::generic_region::decode_generic_with_colour;
use crate::mmr::MmrScratch;
use crate::reader::ByteReader;
use crate::segment_data::{parse_page_information_with_colour, PageInformation};
use crate::segment_header::{parse_segment_header, SegmentType};
use crate::{
    DecodeError, DecodePolicy, DecodeResult, LimitExceeded, LimitKind, Limits, MalformedError,
    Organization, Recovery, RecoveryKind, UnsupportedFeature,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out.is_ok() ==> actual <= maximum))]
pub(super) fn limit(kind: LimitKind, actual: u64, maximum: u64) -> Result<(), DecodeError> {
    if actual > maximum {
        return Err(LimitExceeded {
            kind,
            limit: maximum,
            actual,
        }
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn contains_page(pages: &[ColourPage], number: u32) -> bool {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= pages@.len(),
        decreases pages@.len() - index,
    ))]
    while index < pages.len() {
        if pages[index].number == number {
            return true;
        }
        index += 1;
    }
    false
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn scratch_used(
    pages: &Vec<ColourPage>,
    palettes: &Vec<palette::Palette>,
    recoveries: &Vec<Recovery>,
    header: Option<&crate::segment_header::SegmentHeader>,
) -> Result<u64, DecodeError> {
    let mut used = (pages.capacity() as u64)
        .checked_mul(core::mem::size_of::<ColourPage>() as u64)
        .and_then(|n| {
            (palettes.capacity() as u64)
                .checked_mul(core::mem::size_of::<palette::Palette>() as u64)
                .and_then(|p| n.checked_add(p))
        })
        .and_then(|n| {
            (recoveries.capacity() as u64)
                .checked_mul(core::mem::size_of::<Recovery>() as u64)
                .and_then(|p| n.checked_add(p))
        })
        .ok_or(MalformedError::IntegerOverflow)?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= palettes@.len(),
        decreases palettes@.len() - index,
    ))]
    while index < palettes.len() {
        used = (palettes[index].colours.capacity() as u64)
            .checked_mul(3)
            .and_then(|n| used.checked_add(n))
            .ok_or(MalformedError::IntegerOverflow)?;
        index += 1;
    }
    if let Some(header) = header {
        used = (header.referred_segments.capacity() as u64)
            .checked_mul(4)
            .and_then(|n| used.checked_add(n))
            .and_then(|n| n.checked_add(header.retention_flags.values.capacity() as u64))
            .ok_or(MalformedError::IntegerOverflow)?;
    }
    Ok(used)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn pool_limits(limits: &Limits, reservation: u64, used: u64) -> Result<Limits, DecodeError> {
    limit(LimitKind::Memory, used, reservation)?;
    let mut result = *limits;
    result.max_decoder_managed_memory_bytes = reservation - used;
    result.max_retained_state_bytes = reservation - used;
    Ok(result)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.is_ok() ==> *final(declared),
        !*old(declared) && !compatible ==> out.is_err(),
        out.is_ok() ==> final(recoveries)@ == if *old(declared) {
            old(recoveries)@
        } else {old(recoveries)@.push(recovery)},
        final(limits).max_work_units == old(limits).max_work_units,
        final(limits).max_decoder_managed_memory_bytes <= old(limits).max_decoder_managed_memory_bytes,
        final(limits).max_retained_state_bytes <= old(limits).max_retained_state_bytes,
))]
fn ensure_declaration(
    declared: &mut bool,
    compatible: bool,
    recoveries: &mut Vec<Recovery>,
    recovery: Recovery,
    limits: &mut Limits,
) -> Result<(), DecodeError> {
    if *declared {
        return Ok(());
    }
    if !compatible {
        return Err(MalformedError::InvalidPageState("missing colour declaration").into());
    }
    crate::document::standalone::record(recoveries, recovery, limits)?;
    *declared = true;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
struct Frame {
    number: u32,
    declared_colour: bool,
    information_offset: usize,
    info: PageInformation,
    bitmap: RgbBitmap,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(output) => super::colour_spec::document_valid(&output.value),
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_lines)]
pub(super) fn decode(
    input: &[u8],
    policy: DecodePolicy,
    limits: &Limits,
) -> Result<DecodeResult<DecodedColourDocument>, DecodeError> {
    limit(
        LimitKind::StreamBytes,
        input.len() as u64,
        limits.max_stream_bytes,
    )?;
    let file = crate::inspect_file_header(input)?;
    if !matches!(file.organization(), Organization::Sequential) {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let count = input.len() as u64 / 11;
    let scanning = count
        .checked_mul(count)
        .and_then(|n| n.checked_add(input.len() as u64))
        .and_then(|n| n.checked_mul(8))
        .ok_or(MalformedError::IntegerOverflow)?;
    limit(LimitKind::Work, scanning, limits.max_work_units)?;
    let scratch_bytes = (input.len() as u64)
        .checked_mul(64)
        .ok_or(MalformedError::IntegerOverflow)?;
    limit(
        LimitKind::Memory,
        scratch_bytes,
        limits.max_decoder_managed_memory_bytes,
    )?;
    limit(
        LimitKind::RetainedState,
        scratch_bytes,
        limits.max_retained_state_bytes,
    )?;
    let mut remaining = *limits;
    remaining.max_work_units -= scanning;
    remaining.max_decoder_managed_memory_bytes -= scratch_bytes;
    remaining.max_retained_state_bytes -= scratch_bytes;
    let mut reader = ByteReader::new(input);
    reader.read_bytes(file.byte_length())?;
    let mut pages: Vec<ColourPage> = Vec::new();
    let mut palettes = Vec::new();
    let mut frame: Option<Frame> = None;
    let mut recoveries = Vec::new();
    let mut previous: Option<u32> = None;
    let mut segments = 0_u32;
    let mut references = 0_u64;
    let mut decoded_bytes = 0_u64;
    let mut declared_file_colour = file.uses_colour_regions();
    let compatible = matches!(policy, DecodePolicy::Compatibility);
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant reader.position <= reader.bytes@.len(),
            forall|index:int| 0 <= index < pages@.len() ==> pages@[index].number > 0
                && super::colour_spec::storage_valid(&pages@[index].bitmap),
            match &frame {Some(frame) => frame.number > 0 && super::colour_spec::storage_valid(&frame.bitmap), None => true},
        decreases reader.bytes@.len() - reader.position,
    ))]
    while reader.remaining() != 0 {
        segments = segments
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        limit(
            LimitKind::Segments,
            u64::from(segments),
            u64::from(limits.max_segments),
        )?;
        let header_limits = pool_limits(
            &remaining,
            scratch_bytes,
            scratch_used(&pages, &palettes, &recoveries, None)?,
        )?;
        let header = parse_segment_header(&mut reader, &header_limits)?;
        if let Some(number) = previous {
            if number >= header.number {
                return Err(MalformedError::NonIncreasingSegmentNumber {
                    previous: number,
                    current: header.number,
                }
                .into());
            }
        }
        previous = Some(header.number);
        if header.deferred_non_retain {
            return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
        }
        references = references
            .checked_add(header.referred_segments.len() as u64)
            .ok_or(MalformedError::IntegerOverflow)?;
        limit(
            LimitKind::TotalReferences,
            references,
            limits.max_total_references,
        )?;
        let length = usize::try_from(
            header
                .data_length
                .ok_or(DecodeError::Unsupported(UnsupportedFeature::Colour))?,
        )
        .map_err(|_| MalformedError::IntegerOverflow)?;
        let payload_offset = reader.position();
        let payload = reader.read_bytes(length)?;
        limit(
            LimitKind::ReferenceDepth,
            if header.referred_segments.is_empty() {
                1
            } else {
                2
            },
            u64::from(limits.max_reference_graph_depth),
        )?;
        let generic = matches!(
            header.segment_type,
            SegmentType::ImmediateGenericRegion | SegmentType::ImmediateLosslessGenericRegion
        );
        if !generic && !header.referred_segments.is_empty() {
            return Err(MalformedError::InvalidSegmentReferences(header.number).into());
        }
        match header.segment_type {
            SegmentType::PageInformation => {
                if frame.is_some() {
                    return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
                }
                if header.page_association == 0 || contains_page(&pages, header.page_association) {
                    return Err(MalformedError::InvalidPageState(
                        "invalid or duplicate colour page",
                    )
                    .into());
                }
                limit(
                    LimitKind::Pages,
                    (pages.len() as u64)
                        .checked_add(1)
                        .ok_or(MalformedError::IntegerOverflow)?,
                    u64::from(limits.max_pages),
                )?;
                let (info, declared_colour) =
                    parse_page_information_with_colour(payload, &remaining, true)?;
                let height = info
                    .height
                    .ok_or(DecodeError::Unsupported(UnsupportedFeature::Colour))?;
                if info.striped {
                    return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
                }
                let output_bytes = u64::from(info.width)
                    .checked_mul(u64::from(height))
                    .and_then(|n| n.checked_mul(3))
                    .ok_or(MalformedError::IntegerOverflow)?;
                let next_decoded = decoded_bytes
                    .checked_add(output_bytes)
                    .ok_or(MalformedError::IntegerOverflow)?;
                limit(
                    LimitKind::DecodedBitmapBytes,
                    next_decoded,
                    limits.max_cumulative_decoded_bitmap_bytes,
                )?;
                let bitmap = RgbBitmap::filled(info.width, height, info.default_pixel, &remaining)?;
                let allocated = bitmap.bytes.capacity() as u64;
                remaining.max_work_units = remaining
                    .max_work_units
                    .checked_sub(bitmap.bytes.len() as u64)
                    .ok_or(MalformedError::IntegerOverflow)?;
                remaining.max_decoder_managed_memory_bytes = remaining
                    .max_decoder_managed_memory_bytes
                    .checked_sub(allocated)
                    .ok_or(MalformedError::IntegerOverflow)?;
                remaining.max_retained_state_bytes = remaining
                    .max_retained_state_bytes
                    .checked_sub(allocated)
                    .ok_or(MalformedError::IntegerOverflow)?;
                decoded_bytes = next_decoded;
                frame = Some(Frame {
                    number: header.page_association,
                    declared_colour,
                    information_offset: payload_offset + 16,
                    info,
                    bitmap,
                });
            }
            SegmentType::ImmediateGenericRegion | SegmentType::ImmediateLosslessGenericRegion => {
                let page = frame
                    .as_mut()
                    .ok_or(MalformedError::MissingPageInformation(
                        header.page_association,
                    ))?;
                if header.page_association != page.number {
                    return Err(
                        MalformedError::MissingPageInformation(header.page_association).into(),
                    );
                }
                let region = {
                    let mut region_reader = ByteReader::new(payload);
                    crate::segment_data::parse_region_info_with_colour(
                        &mut region_reader,
                        &remaining,
                        true,
                    )?
                };
                let (region, colour) = region;
                if colour && !declared_file_colour {
                    let mut diagnostic_limits = pool_limits(
                        &remaining,
                        scratch_bytes,
                        scratch_used(&pages, &palettes, &recoveries, Some(&header))?,
                    )?;
                    ensure_declaration(
                        &mut declared_file_colour,
                        compatible,
                        &mut recoveries,
                        Recovery {
                            page: 0,
                            byte_offset: Some(8),
                            kind: RecoveryKind::MissingFileColourFlag,
                        },
                        &mut diagnostic_limits,
                    )?;
                }
                if colour && !page.declared_colour {
                    let mut diagnostic_limits = pool_limits(
                        &remaining,
                        scratch_bytes,
                        scratch_used(&pages, &palettes, &recoveries, Some(&header))?,
                    )?;
                    ensure_declaration(
                        &mut page.declared_colour,
                        compatible,
                        &mut recoveries,
                        Recovery {
                            page: page.number,
                            byte_offset: Some(page.information_offset),
                            kind: RecoveryKind::MissingPageColourFlag,
                        },
                        &mut diagnostic_limits,
                    )?;
                }
                if !colour && !header.referred_segments.is_empty() {
                    return Err(MalformedError::InvalidSegmentReferences(header.number).into());
                }
                palette::validate_references(&header, &palettes)?;
                crate::decode::check_combination_operator(
                    page.info.combination_operator_override,
                    region.operator,
                    page.info.default_operator,
                    false,
                )?;
                let work = u64::from(region.width)
                    .checked_mul(u64::from(region.height))
                    .and_then(|n| n.checked_mul(64))
                    .and_then(|n| {
                        (payload.len() as u64)
                            .checked_mul(16)
                            .and_then(|bytes| n.checked_add(bytes))
                    })
                    .ok_or(MalformedError::IntegerOverflow)?;
                limit(LimitKind::Work, work, remaining.max_work_units)?;
                remaining.max_work_units -= work;
                let mut region_limits = remaining;
                region_limits.max_work_units = work;
                let mut mmr = MmrScratch::default();
                let (decoded, foreground) =
                    decode_generic_with_colour(payload, &region_limits, &mut mmr)
                        .map_err(crate::decode::generic_error_into_decode)?;
                if !decoded.header.mmr
                    && decoded.header.extended_template
                    && !file.uses_extended_templates()
                {
                    return Err(MalformedError::InvalidPageState(
                        "missing extended-template file declaration",
                    )
                    .into());
                }
                let foreground = match foreground {
                    Some(id) => Some(palette::resolve(&header, &palettes, id)?),
                    None => None,
                };
                page.bitmap.compose(
                    &decoded.bitmap,
                    region.x,
                    region.y,
                    foreground,
                    region.operator,
                )?;
                palette::release(&header, &mut palettes)?;
            }
            SegmentType::ColourPalette => {
                if header.page_association != 0
                    && !matches!(&frame, Some(p) if p.number == header.page_association)
                {
                    return Err(
                        MalformedError::MissingPageInformation(header.page_association).into(),
                    );
                }

                let palette_limits = pool_limits(
                    &remaining,
                    scratch_bytes,
                    scratch_used(&pages, &palettes, &recoveries, Some(&header))?,
                )?;
                let palette = palette::parse(&header, payload, &palette_limits)?;
                palettes
                    .try_reserve(1)
                    .map_err(|_| DecodeError::Allocation {
                        requested_bytes: scratch_bytes,
                    })?;
                palettes.push(palette);
            }
            SegmentType::EndOfPage => {
                crate::segment_data::require_empty(49, payload)?;
                let page = frame.take().ok_or(MalformedError::MissingPageInformation(
                    header.page_association,
                ))?;
                if page.number != header.page_association {
                    return Err(MalformedError::InvalidPageState(
                        "end-of-page names another colour page",
                    )
                    .into());
                }
                pages.try_reserve(1).map_err(|_| DecodeError::Allocation {
                    requested_bytes: scratch_bytes,
                })?;
                pages.push(ColourPage {
                    number: page.number,
                    bitmap: page.bitmap,
                });
            }
            SegmentType::EndOfFile => {
                crate::segment_data::require_empty(51, payload)?;
                if header.page_association != 0 || reader.remaining() != 0 {
                    return Err(
                        MalformedError::InvalidPageState("invalid colour EOF boundary").into(),
                    );
                }
            }
            SegmentType::Extension => {
                if header.page_association != 0 {
                    return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
                }
                crate::segment_data::parse_extension(payload)?;
            }
            _ => return Err(DecodeError::Unsupported(UnsupportedFeature::Colour)),
        }
        limit(
            LimitKind::Memory,
            scratch_used(&pages, &palettes, &recoveries, Some(&header))?,
            scratch_bytes,
        )?;
    }
    if frame.is_some() {
        return Err(MalformedError::InvalidPageState("unfinished colour page").into());
    }
    if let Some(count) = file.page_count() {
        if u64::from(count) != pages.len() as u64 {
            return Err(MalformedError::PageCountMismatch {
                declared: count,
                actual: u32::try_from(pages.len()).map_err(|_| MalformedError::IntegerOverflow)?,
            }
            .into());
        }
    }
    let document = DecodedColourDocument { pages };
    #[cfg(verus_keep_ghost)]
    proof! {
        super::colour_spec::document_fields(&document);
        assert forall|index: int| 0 <= index < document.pages@.len()
            implies super::colour_spec::page_number(&document.pages@[index]) > 0
                && super::colour_spec::storage_valid(
                    super::colour_spec::page_bitmap(&document.pages@[index]),
                ) by {
            super::colour_spec::page_fields(&document.pages@[index]);
        }
    }
    Ok(DecodeResult {
        value: document,
        recoveries,
    })
}
