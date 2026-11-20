use super::{
    allocation_size, bitmap_byte_len, check_combination_operator, compose_immediate,
    decode_arithmetic_refinement, decode_generic_region_segment_into,
    decode_generic_region_segment_reused, decode_halftone_region, decode_pattern_dictionary,
    decode_symbol_dictionary, decode_text_region, generic_error_into_decode, get_huffman_table,
    get_patterns, get_region_buffer, get_runtime_page, get_symbol_resource,
    halftone_error_into_decode, huffman_table_index, insert_buffer, insert_patterns,
    insert_symbol_resource, new_gray_mmr_state, new_halftone_arithmetic_state,
    new_pattern_arithmetic_state, new_pattern_mmr_state, note_page_write,
    parse_generic_region_header, parse_halftone_header, parse_pattern_dictionary_header,
    parse_refinement_region_header, parse_region_info, parse_symbol_dictionary_header,
    pattern_index, require_auxiliary_page, require_matching_geometry, runtime_page_index,
    snapshot_region, symbol_index, symbol_text_error_into_decode, Bitmap, ByteReader,
    CollectedSymbols, CombinationOperator, DecodeError, DecodeWork, GenericTemplate, HuffmanTable,
    HuffmanTables, Limits, MalformedError, MmrScratch, ParsedSegment, PatternStore,
    RefinementParameters, RegionBuffers, RetainedBudget, RuntimePage, RuntimePages, SegmentType,
    SymbolDictionaryInput, SymbolResource, SymbolStore, TextRegionInput, WorkBudget,
};

#[cfg(verus_keep_ghost)]
use super::decode_spec;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(number) =>
            referred@.len() > 0
                && number == referred@[0],
        Err(_) => referred@.len() == 0,
    },
))]
pub(super) fn first_referred_segment(referred: &[u32], owner: u32) -> Result<u32, DecodeError> {
    match referred.first() {
        Some(number) => Ok(*number),
        None => Err(MalformedError::InvalidSegmentReferences(owner).into()),
    }
}

/// T.88 7.4.2.2: tables do not replace the last dictionary, but a dictionary
/// without retained contexts does. Symbol imports are collected independently.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures decode_spec::dictionary_context_source(symbols@, referred@, context_used, out),
))]
pub(super) fn dictionary_context_source(
    symbols: &SymbolStore,
    referred: &[u32],
    context_used: bool,
) -> Option<usize> {
    if !context_used {
        return None;
    }
    let mut reference_index = referred.len();
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            context_used,
            reference_index <= referred@.len(),
            forall|later: int, entry: int|
                reference_index <= later < referred@.len()
                    && 0 <= entry < symbols@.len() ==>
                    symbols@[entry].0 != referred@[later],
        decreases reference_index,
    ))]
    while reference_index > 0 {
        reference_index -= 1;
        if let Some(index) = symbol_index(symbols, referred[reference_index]) {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(exists|reference: int| 0 <= reference < referred@.len()
                    && decode_spec::first_symbol_index(
                        symbols@, referred@[reference], index as int,
                    )
                    && forall|later: int, entry: int|
                        reference < later < referred@.len()
                            && 0 <= entry < symbols@.len() ==>
                            symbols@[entry].0 != referred@[later]) by {
                    assert(decode_spec::first_symbol_index(
                        symbols@, referred@[reference_index as int], index as int,
                    ));
                }
            }
            return Some(index);
        }
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::symbol_store_valid(old(symbols)),
    ensures match out {
        Ok(()) => decode_spec::symbol_store_valid(final(symbols)),
        Err(_) => true,
    },
))]
pub(super) fn decode_symbol_dictionary_segment(
    segment: &ParsedSegment<'_>,
    symbols: &mut SymbolStore,
    tables: &HuffmanTables,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    if symbol_index(symbols, segment.header.number).is_some() {
        return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
    }
    let imported = collect_referred_symbols(segment, symbols, retained.bytes, limits)?;
    let custom_tables = collect_referred_tables(segment, tables)?;
    let (header, _) = parse_symbol_dictionary_header(segment.payload, limits)
        .map_err(symbol_text_error_into_decode)?;
    let used_contexts = match dictionary_context_source(
        symbols,
        &segment.header.referred_segments,
        header.config.bitmap_context_used,
    ) {
        Some(index) => symbols[index].1.contexts.as_ref(),
        None => None,
    };
    let output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: segment.payload,
        imported_symbols: imported.codebook(),
        custom_tables: &custom_tables,
        used_contexts,
        limits,
    })
    .map_err(symbol_text_error_into_decode)?;
    if output.consumed_bytes != segment.payload.len() {
        let trailing = segment
            .payload
            .len()
            .checked_sub(output.consumed_bytes)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        return Err(MalformedError::TrailingData(trailing).into());
    }
    let mut bitmap_bytes = 0_u64;
    let mut bitmap_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant bitmap_index <= output.exported_symbols@.len(),
        decreases output.exported_symbols@.len() - bitmap_index,
    ))]
    while bitmap_index < output.exported_symbols.len() {
        bitmap_bytes = bitmap_bytes
            .checked_add(bitmap_byte_len(&output.exported_symbols[bitmap_index])?)
            .ok_or(DecodeError::Malformed(MalformedError::IntegerOverflow))?;
        bitmap_index += 1;
    }
    retained.retain(bitmap_bytes, limits)?;
    let mut symbol_work = 0_u64;
    let mut work_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant work_index <= output.exported_symbols@.len(),
        decreases output.exported_symbols@.len() - work_index,
    ))]
    while work_index < output.exported_symbols.len() {
        let bitmap = &output.exported_symbols[work_index];
        let pixels = u64::from(bitmap.width())
            .checked_mul(u64::from(bitmap.height()))
            .ok_or(MalformedError::IntegerOverflow)?;
        symbol_work = symbol_work
            .checked_add(pixels)
            .ok_or(MalformedError::IntegerOverflow)?;
        work_index += 1;
    }
    work.charge(symbol_work, limits)?;
    let resource = SymbolResource {
        exported: output.exported_symbols,
        contexts: output.retained_contexts,
    };
    if !insert_symbol_resource(symbols, segment.header.number, resource) {
        return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        decode_spec::region_buffers_valid(old(buffers)),
        decode_spec::symbol_store_valid(symbols),
    ensures match out {
        Ok(()) =>
            decode_spec::runtime_pages_valid(final(pages))
                && decode_spec::region_buffers_valid(final(buffers)),
        Err(_) => true,
    },
))]
pub(super) fn decode_text_segment(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    symbols: &SymbolStore,
    tables: &HuffmanTables,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let available_symbols = collect_referred_symbols(segment, symbols, retained.bytes, limits)?;
    let custom_tables = collect_referred_tables(segment, tables)?;
    let output = decode_text_region(&TextRegionInput {
        payload: segment.payload,
        symbols: available_symbols.codebook(),
        custom_tables: &custom_tables,
        limits,
    })
    .map_err(symbol_text_error_into_decode)?;
    if output.consumed_bytes != segment.payload.len() {
        let trailing = segment
            .payload
            .len()
            .checked_sub(output.consumed_bytes)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        return Err(MalformedError::TrailingData(trailing).into());
    }
    let region_work = u64::from(output.region.width)
        .checked_mul(u64::from(output.region.height))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(region_work, limits)?;
    if segment.header.segment_type == SegmentType::IntermediateTextRegion {
        require_auxiliary_page(segment, pages)?;
        insert_buffer(
            buffers,
            segment.header.number,
            output.region,
            output.bitmap,
            retained,
            limits,
        )?;
    } else {
        compose_immediate(
            segment,
            pages,
            &output.region,
            &output.bitmap,
            false,
            work,
            limits,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::runtime_pages_valid(pages),
    ensures match out {
        Some(index) => index < pages@.len(),
        None => true,
    },
))]
pub(super) fn direct_generic_page_index(
    segment: &ParsedSegment<'_>,
    pages: &RuntimePages,
    work: &DecodeWork,
    limits: &Limits,
) -> Option<usize> {
    if !matches!(
        segment.header.segment_type,
        SegmentType::ImmediateGenericRegion | SegmentType::ImmediateLosslessGenericRegion
    ) {
        return None;
    }
    let (header, _) = match parse_generic_region_header(segment.payload, limits) {
        Ok(header) => header,
        Err(_error) => return None,
    };
    if header.mmr
        || header.region.x != 0
        || header.region.y != 0
        || !matches!(
            header.region.operator,
            CombinationOperator::Or | CombinationOperator::Replace
        )
    {
        return None;
    }
    let index = runtime_page_index(pages, segment.header.page_association)?;
    let page = &pages[index].1;
    if !page.pristine
        || page.default_pixel
        || page.last_stripe_end.is_some()
        || page.open_stripe_max_row.is_some()
        || header.region.width != page.bitmap.width()
        || header.region.height != page.bitmap.height()
        || (!page.combination_override && page.default_operator != header.region.operator)
    {
        return None;
    }
    let pixels = u64::from(header.region.width).checked_mul(u64::from(header.region.height))?;
    let region_work = pixels.checked_mul(18)?;
    if !work.can_charge_pair(region_work, pixels, limits) {
        return None;
    }
    Some(index)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(&old(page).bitmap),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(&final(page).bitmap),
))]
pub(super) fn decode_generic_segment_direct_on_page(
    segment: &ParsedSegment<'_>,
    page: &mut RuntimePage,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<bool, DecodeError> {
    let Some(decoded) = decode_generic_region_segment_into(
        segment.payload,
        segment.header.data_length.is_none(),
        limits,
        &mut page.bitmap,
    )
    .map_err(generic_error_into_decode)?
    else {
        return Ok(false);
    };
    if decoded.consumed_bytes != segment.payload.len() {
        let trailing = segment
            .payload
            .len()
            .checked_sub(decoded.consumed_bytes)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        return Err(MalformedError::TrailingData(trailing).into());
    }
    let region_work = u64::from(decoded.header.region.width)
        .checked_mul(u64::from(decoded.decoded_rows))
        .and_then(|pixels| pixels.checked_mul(18))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(region_work, limits)?;
    check_combination_operator(
        page.combination_override,
        decoded.header.region.operator,
        page.default_operator,
        false,
    )?;
    note_page_write(page, &decoded.header.region)?;
    let composition_work = u64::from(decoded.header.region.width)
        .checked_mul(u64::from(decoded.header.region.height))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(composition_work, limits)?;
    page.pristine = false;
    Ok(true)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        decode_spec::region_buffers_valid(old(buffers)),
    ensures match out {
        Ok(()) =>
            decode_spec::runtime_pages_valid(final(pages))
                && decode_spec::region_buffers_valid(final(buffers)),
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_arguments)]
pub(super) fn decode_generic_segment(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
) -> Result<(), DecodeError> {
    let arithmetic = match segment.payload.get(17) {
        Some(flags) => *flags & 0x01 == 0,
        None => false,
    };
    if arithmetic {
        if let Some(index) = direct_generic_page_index(segment, pages, work, limits) {
            if decode_generic_segment_direct_on_page(segment, &mut pages[index].1, work, limits)? {
                return Ok(());
            }
        }
    }
    decode_generic_segment_buffered(segment, pages, buffers, retained, work, limits, mmr_scratch)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        decode_spec::region_buffers_valid(old(buffers)),
    ensures match out {
        Ok(()) =>
            decode_spec::runtime_pages_valid(final(pages))
                && decode_spec::region_buffers_valid(final(buffers)),
        Err(_) => true,
    },
))]
pub(super) fn decode_generic_segment_buffered(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
) -> Result<(), DecodeError> {
    let decoded = decode_generic_region_segment_reused(
        segment.payload,
        segment.header.data_length.is_none(),
        limits,
        mmr_scratch,
    )
    .map_err(generic_error_into_decode)?;
    if decoded.consumed_bytes != segment.payload.len() {
        let trailing = segment
            .payload
            .len()
            .checked_sub(decoded.consumed_bytes)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        return Err(MalformedError::TrailingData(trailing).into());
    }
    let region_work = u64::from(decoded.header.region.width)
        .checked_mul(u64::from(decoded.decoded_rows))
        .and_then(|pixels| pixels.checked_mul(18))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(region_work, limits)?;
    if segment.header.segment_type == SegmentType::IntermediateGenericRegion {
        require_auxiliary_page(segment, pages)?;
        insert_buffer(
            buffers,
            segment.header.number,
            decoded.header.region,
            decoded.bitmap,
            retained,
            limits,
        )?;
    } else {
        compose_immediate(
            segment,
            pages,
            &decoded.header.region,
            &decoded.bitmap,
            false,
            work,
            limits,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        decode_spec::region_buffers_valid(old(buffers)),
    ensures match out {
        Ok(()) =>
            decode_spec::runtime_pages_valid(final(pages))
                && decode_spec::region_buffers_valid(final(buffers)),
        Err(_) => true,
    },
))]
pub(super) fn decode_refinement_segment(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let (header, header_length) = parse_refinement_region_header(segment.payload, limits)
        .map_err(generic_error_into_decode)?;
    let coded = segment
        .payload
        .get(header_length..)
        .ok_or(MalformedError::IntegerOverflow)?;
    let owned_snapshot;
    let reference = if let Some(number) = segment.header.referred_segments.first() {
        let buffer =
            get_region_buffer(buffers, *number).ok_or(MalformedError::MissingReferredSegment {
                segment: segment.header.number,
                referred: *number,
            })?;
        require_matching_geometry(buffer.region == header.region)?;
        &buffer.bitmap
    } else {
        if header.region.operator != crate::bitmap::CombinationOperator::Replace {
            return Err(MalformedError::InvalidPageState(
                "unreferenced refinement must use REPLACE",
            )
            .into());
        }
        let page = get_runtime_page(pages, segment.header.page_association).ok_or(
            MalformedError::MissingPageInformation(segment.header.page_association),
        )?;
        owned_snapshot = snapshot_region(&page.bitmap, &header.region, limits)?;
        &owned_snapshot
    };
    let bitmap = decode_arithmetic_refinement(
        coded,
        &header,
        RefinementParameters {
            reference_dx: 0,
            reference_dy: 0,
        },
        reference,
        limits,
    )
    .map_err(generic_error_into_decode)?;
    let refinement_work = u64::from(header.region.width)
        .checked_mul(u64::from(header.region.height))
        .and_then(|pixels| pixels.checked_mul(23))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(refinement_work, limits)?;

    if segment.header.segment_type == SegmentType::IntermediateGenericRefinementRegion {
        require_auxiliary_page(segment, pages)?;
        insert_buffer(
            buffers,
            segment.header.number,
            header.region,
            bitmap,
            retained,
            limits,
        )?;
    } else {
        let page = get_runtime_page(pages, segment.header.page_association).ok_or(
            MalformedError::MissingPageInformation(segment.header.page_association),
        )?;
        if !page.refinements_allowed {
            return Err(MalformedError::InvalidPageState(
                "refinement on page that forbids refinements",
            )
            .into());
        }
        compose_immediate(segment, pages, &header.region, &bitmap, true, work, limits)?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::pattern_store_valid(old(patterns)),
    ensures match out {
        Ok(()) => decode_spec::pattern_store_valid(final(patterns)),
        Err(_) => true,
    },
))]
pub(super) fn decode_pattern_segment(
    segment: &ParsedSegment<'_>,
    patterns: &mut PatternStore,
    retained: &mut RetainedBudget,
    cumulative_work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    if pattern_index(patterns, segment.header.number).is_some() {
        return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
    }
    let mut reader = ByteReader::new(segment.payload);
    let header =
        parse_pattern_dictionary_header(&mut reader).map_err(halftone_error_into_decode)?;
    let coded = reader.remaining_bytes();
    let mut work = WorkBudget::new(limits);
    let mut decoder = if header.mmr {
        new_pattern_mmr_state(coded, limits)
    } else {
        let template = generic_template(header.template)?;
        new_pattern_arithmetic_state(coded, template, header.template, limits)?
    };
    let pattern_bitmaps = decode_pattern_dictionary(header, &mut decoder, limits, &mut work)
        .map_err(halftone_error_into_decode)?;
    decoder.finish()?;

    let requested_bytes = u64::try_from(std::mem::size_of::<(u32, Vec<Bitmap>)>())
        .map_err(|_| MalformedError::IntegerOverflow)?;
    patterns
        .try_reserve(1)
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    let mut retained_bytes = 0_u64;
    let mut decoded_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant decoded_index <= pattern_bitmaps@.len(),
        decreases pattern_bitmaps@.len() - decoded_index,
    ))]
    while decoded_index < pattern_bitmaps.len() {
        retained_bytes = retained_bytes
            .checked_add(bitmap_byte_len(&pattern_bitmaps[decoded_index])?)
            .ok_or(DecodeError::Malformed(MalformedError::IntegerOverflow))?;
        decoded_index += 1;
    }
    cumulative_work.charge(work.used(), limits)?;
    retained.retain(retained_bytes, limits)?;
    if !insert_patterns(patterns, segment.header.number, pattern_bitmaps) {
        return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        decode_spec::pattern_store_valid(patterns),
        decode_spec::region_buffers_valid(old(buffers)),
    ensures match out {
        Ok(()) =>
            decode_spec::runtime_pages_valid(final(pages))
                && decode_spec::region_buffers_valid(final(buffers)),
        Err(_) => true,
    },
))]
pub(super) fn decode_halftone_segment(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    patterns: &PatternStore,
    retained: &mut RetainedBudget,
    cumulative_work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let pattern_number =
        first_referred_segment(&segment.header.referred_segments, segment.header.number)?;
    let dictionary =
        get_patterns(patterns, pattern_number).ok_or(MalformedError::MissingReferredSegment {
            segment: segment.header.number,
            referred: pattern_number,
        })?;
    let mut reader = ByteReader::new(segment.payload);
    let region = parse_region_info(&mut reader, limits)?;
    let header = parse_halftone_header(&mut reader).map_err(halftone_error_into_decode)?;
    let coded = reader.remaining_bytes();
    let mut work = WorkBudget::new(limits);

    let mut decoder = if header.mmr {
        new_gray_mmr_state(coded, limits)
    } else {
        let template = generic_template(header.template)?;
        new_halftone_arithmetic_state(coded, template, header.template, limits)?
    };
    let bitmap = decode_halftone_region(
        &header,
        region.width,
        region.height,
        dictionary,
        &mut decoder,
        limits,
        &mut work,
    )
    .map_err(halftone_error_into_decode)?;
    decoder.finish()?;

    cumulative_work.charge(work.used(), limits)?;
    if segment.header.segment_type == SegmentType::IntermediateHalftoneRegion {
        require_auxiliary_page(segment, pages)?;
        insert_buffer(
            buffers,
            segment.header.number,
            region,
            bitmap,
            retained,
            limits,
        )?;
    } else {
        compose_immediate(
            segment,
            pages,
            &region,
            &bitmap,
            false,
            cumulative_work,
            limits,
        )?;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(value) => decode_spec::generic_template_code(value) == template as nat,
        Err(_) => template > 3,
    },
))]
pub(super) fn generic_template(template: u8) -> Result<GenericTemplate, DecodeError> {
    let template = match template {
        0 => GenericTemplate::Template0,
        1 => GenericTemplate::Template1,
        2 => GenericTemplate::Template2,
        3 => GenericTemplate::Template3,
        _ => return Err(MalformedError::InvalidPageState("invalid generic template").into()),
    };
    Ok(template)
}

mod references;

#[allow(unused_imports)]
use references::check_borrowed_symbol_memory;
#[allow(unused_imports)]
pub(super) use references::{
    borrowed_symbol_memory, collect_referred_symbols, collect_referred_tables,
};
