use super::{
    allocation_size, bitmap_byte_len, decode_generic_segment, decode_halftone_segment,
    decode_pattern_segment, decode_refinement_segment, decode_symbol_dictionary_segment,
    decode_text_segment, find_page_info, huffman_error_into_decode, insert_huffman_table,
    insert_runtime_page, parse_embedded_document, release_referred_buffers,
    release_referred_patterns, release_referred_symbols, release_referred_tables,
    runtime_page_index, seal_stripe, structural_work, write_page_to_sink, Bitmap, DecodeError,
    DecodeWork, DocumentInfo, HuffmanTable, InitialResources, Limits, MalformedError, MmrScratch,
    Page, PageCollector, PageSink, ParsedData, ParsedSegment, RegionBuffer, RetainedBudget,
    RuntimePage, SegmentType, SymbolResource, UnsupportedFeature,
};

#[cfg(verus_keep_ghost)]
use super::{decode_properties, decode_spec};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.0 <= segments@.len(),
        out.1 <= segments@.len(),
        out.2 <= segments@.len(),
        out.3 <= segments@.len(),
))]
fn count_resource_segments(segments: &[ParsedSegment<'_>]) -> (usize, usize, usize, usize) {
    let mut patterns = 0_usize;
    let mut symbols = 0_usize;
    let mut tables = 0_usize;
    let mut buffers = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= segments@.len(),
            patterns <= index,
            symbols <= index,
            tables <= index,
            buffers <= index,
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        match segments[index].header.segment_type {
            SegmentType::PatternDictionary => patterns += 1,
            SegmentType::SymbolDictionary => symbols += 1,
            SegmentType::Tables => tables += 1,
            SegmentType::IntermediateTextRegion
            | SegmentType::IntermediateHalftoneRegion
            | SegmentType::IntermediateGenericRegion
            | SegmentType::IntermediateGenericRefinementRegion => buffers += 1,
            _ => {}
        }
        index += 1;
    }
    (patterns, symbols, tables, buffers)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(resources) =>
            decode_spec::pattern_store_valid(&resources.patterns)
                && decode_spec::symbol_store_valid(&resources.symbols),
        Err(_) => true,
    },
))]
pub(super) fn decode_global_resources(
    input: &[u8],
    limits: &Limits,
) -> Result<InitialResources, DecodeError> {
    let (_info, segments) = parse_embedded_document(input, limits, None)?;
    let (pattern_count, symbol_count, table_count, _buffer_count) =
        count_resource_segments(&segments);
    let mut resources = InitialResources {
        patterns: Vec::new(),
        symbols: Vec::new(),
        tables: Vec::new(),
    };
    if pattern_count != 0 {
        let requested_bytes = allocation_size::<(u32, Vec<Bitmap>)>(pattern_count)?;
        resources
            .patterns
            .try_reserve(pattern_count)
            .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    }
    if symbol_count != 0 {
        let symbol_bytes = allocation_size::<(u32, SymbolResource)>(symbol_count)?;
        resources
            .symbols
            .try_reserve(symbol_count)
            .map_err(|_| DecodeError::Allocation {
                requested_bytes: symbol_bytes,
            })?;
    }
    if table_count != 0 {
        let table_bytes = allocation_size::<(u32, HuffmanTable)>(table_count)?;
        resources
            .tables
            .try_reserve(table_count)
            .map_err(|_| DecodeError::Allocation {
                requested_bytes: table_bytes,
            })?;
    }
    let mut retained = RetainedBudget::new();
    let mut work = DecodeWork::new(structural_work(input.len(), &segments)?, limits)?;
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            segment_index <= segments@.len(),
            decode_spec::pattern_store_valid(&resources.patterns),
            decode_spec::symbol_store_valid(&resources.symbols),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        match segment.header.segment_type {
            SegmentType::Tables => {
                let table = HuffmanTable::from_encoded(segment.payload, limits)
                    .map_err(huffman_error_into_decode)?;
                let _ = insert_huffman_table(&mut resources.tables, segment.header.number, table);
            }
            SegmentType::PatternDictionary => {
                decode_pattern_segment(
                    segment,
                    &mut resources.patterns,
                    &mut retained,
                    &mut work,
                    limits,
                )?;
            }
            SegmentType::SymbolDictionary => {
                decode_symbol_dictionary_segment(
                    segment,
                    &mut resources.symbols,
                    &resources.tables,
                    &mut retained,
                    &mut work,
                    limits,
                )?;
            }
            _ => {}
        }
        release_referred_patterns(segment, &mut resources.patterns, &mut retained)?;
        release_referred_symbols(segment, &mut resources.symbols, &mut retained)?;
        release_referred_tables(segment, &mut resources.tables);
        segment_index += 1;
    }
    Ok(resources)
}

macro_rules! decode_segments_body {
    (
        $segments:expr,
        $info:expr,
        $initial_work:expr,
        $initial_retained_bytes:expr,
        $limits:expr,
        $mmr_scratch:expr,
        $initial_resources:expr,
        $sink:expr,
        $embedded:expr,
        $recoveries:expr,
        $text_recoveries:expr,
        $write_page:path
    ) => {{
        let segments = $segments;
        let info = $info;
        let initial_work = $initial_work;
        let initial_retained_bytes = $initial_retained_bytes;
        let limits = $limits;
        let mmr_scratch = $mmr_scratch;
        let initial_resources = $initial_resources;
        let sink = $sink;
        let implicit_ends = crate::document::standalone::has_missing_end($recoveries);
        let (pattern_count, symbol_count, table_count, buffer_count) =
            count_resource_segments(segments);
        let mut pages: Vec<(u32, RuntimePage)> = Vec::new();
        if !info.pages().is_empty() {
            let page_allocation = allocation_size::<(u32, RuntimePage)>(info.pages().len())?;
            pages
                .try_reserve(info.pages().len())
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: page_allocation,
                })?;
        }
        let mut buffers = Vec::new();
        if buffer_count != 0 {
            let buffer_allocation = allocation_size::<(u32, RegionBuffer)>(buffer_count)?;
            buffers
                .try_reserve(buffer_count)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: buffer_allocation,
                })?;
        }
        let initial_resources = match initial_resources {
            Some(resources) => resources,
            None => InitialResources {
                patterns: Vec::new(),
                symbols: Vec::new(),
                tables: Vec::new(),
            },
        };
        let InitialResources {
            mut patterns,
            mut symbols,
            mut tables,
        } = initial_resources;
        if pattern_count != 0 {
            let pattern_allocation = allocation_size::<(u32, Vec<Bitmap>)>(pattern_count)?;
            patterns
                .try_reserve(pattern_count)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: pattern_allocation,
                })?;
        }
        if table_count != 0 {
            let table_allocation = allocation_size::<(u32, HuffmanTable)>(table_count)?;
            tables
                .try_reserve(table_count)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: table_allocation,
                })?;
        }
        if symbol_count != 0 {
            let symbol_allocation = allocation_size::<(u32, SymbolResource)>(symbol_count)?;
            symbols
                .try_reserve(symbol_count)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: symbol_allocation,
                })?;
        }
        let mut retained = RetainedBudget::new();
        retained.retain(initial_retained_bytes, limits)?;
        let mut cumulative_work = DecodeWork::new(initial_work, limits)?;
        let mut pattern_position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
                                    invariant
                                        pattern_position <= patterns@.len(),
                                        decode_spec::pattern_store_valid(&patterns),
                                    decreases patterns@.len() - pattern_position,
                                ))]
        while pattern_position < patterns.len() {
            let bitmaps = &patterns[pattern_position].1;
            let mut bytes = 0_u64;
            let mut bitmap_position = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                                        invariant
                                            bitmap_position <= bitmaps@.len(),
                                            crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
                                        decreases bitmaps@.len() - bitmap_position,
                                    ))]
            while bitmap_position < bitmaps.len() {
                bytes = bytes
                    .checked_add(bitmap_byte_len(&bitmaps[bitmap_position])?)
                    .ok_or(DecodeError::Malformed(MalformedError::IntegerOverflow))?;
                bitmap_position += 1;
            }
            retained.retain(bytes, limits)?;
            pattern_position += 1;
        }
        let mut symbol_position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
                                    invariant
                                        symbol_position <= symbols@.len(),
                                        decode_spec::symbol_store_valid(&symbols),
                                    decreases symbols@.len() - symbol_position,
                                ))]
        while symbol_position < symbols.len() {
            let resource = &symbols[symbol_position].1;
            let mut bytes = 0_u64;
            let mut bitmap_position = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                                        invariant
                                            bitmap_position <= resource.exported@.len(),
                                            crate::halftone::halftone_spec::bitmaps_valid(
                                                resource.exported@
                                            ),
                                        decreases resource.exported@.len() - bitmap_position,
                                    ))]
            while bitmap_position < resource.exported.len() {
                bytes = bytes
                    .checked_add(bitmap_byte_len(&resource.exported[bitmap_position])?)
                    .ok_or(DecodeError::Malformed(MalformedError::IntegerOverflow))?;
                bitmap_position += 1;
            }
            retained.retain(bytes, limits)?;
            symbol_position += 1;
        }

        let mut segment_position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
                                    invariant
                                        segment_position <= segments@.len(),
                                        decode_spec::runtime_pages_valid(&pages),
                                        decode_spec::region_buffers_valid(&buffers),
                                        decode_spec::pattern_store_valid(&patterns),
                                        decode_spec::symbol_store_valid(&symbols),
                                    decreases segments@.len() - segment_position,
                                ))]
        while segment_position < segments.len() {
            let segment = &segments[segment_position];
            #[cfg(verus_keep_ghost)]
            proof! {
                decode_properties::dispatch_is_deterministic(segment.header.segment_type);
            }
            match segment.header.segment_type {
                SegmentType::PageInformation => {
                    if implicit_ends && pages.len() > 1 {
                        return Err(MalformedError::InvalidPageState(
                            "ambiguous implicit page boundary",
                        ).into());
                    }
                    if !pages.is_empty()
                        && crate::document::standalone::has_implicit_end(
                            $recoveries, pages[0].0,
                        )
                    {
                        if pages.len() != 1 {
                            return Err(MalformedError::InvalidPageState(
                                "ambiguous implicit page boundary",
                            ).into());
                        }
                        let (number, page) = pages.remove(0);
                        retained.release(bitmap_byte_len(&page.bitmap)?)?;
                        $write_page(sink, Page { number, bitmap: page.bitmap })?;
                    }
                    let ParsedData::Page(page_data) = segment.data else {
                        return Err(MalformedError::InvalidPageState(
                            "page information payload was not parsed",
                        )
                        .into());
                    };
                    let public = find_page_info(info, segment.header.page_association)?;
                    let bitmap = Bitmap::filled(
                        public.width(),
                        public.height(),
                        page_data.default_pixel,
                        limits,
                    )?;
                    retained.retain(bitmap_byte_len(&bitmap)?, limits)?;
                    insert_runtime_page(
                        &mut pages,
                        public.number(),
                        RuntimePage {
                            default_pixel: page_data.default_pixel,
                            pristine: true,
                            default_operator: page_data.default_operator,
                            combination_override: page_data.combination_operator_override,
                            refinements_allowed: page_data.might_contain_refinements,
                            auxiliary_allowed: page_data.requires_auxiliary_buffers,
                            last_stripe_end: None,
                            open_stripe_max_row: None,
                            bitmap,
                        },
                    );
                }
                SegmentType::IntermediateGenericRegion
                | SegmentType::ImmediateGenericRegion
                | SegmentType::ImmediateLosslessGenericRegion => {
                    decode_generic_segment(
                        segment,
                        &mut pages,
                        &mut buffers,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                        mmr_scratch,
                    )?;
                }
                SegmentType::IntermediateGenericRefinementRegion
                | SegmentType::ImmediateGenericRefinementRegion
                | SegmentType::ImmediateLosslessGenericRefinementRegion => {
                    decode_refinement_segment(
                        segment,
                        &mut pages,
                        &mut buffers,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                    )?;
                }
                SegmentType::PatternDictionary => {
                    decode_pattern_segment(
                        segment,
                        &mut patterns,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                    )?;
                }
                SegmentType::SymbolDictionary => {
                    decode_symbol_dictionary_segment(
                        segment,
                        &mut symbols,
                        &tables,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                    )?;
                }
                SegmentType::IntermediateTextRegion
                | SegmentType::ImmediateTextRegion
                | SegmentType::ImmediateLosslessTextRegion => {
                    decode_text_segment(
                        segment,
                        &mut pages,
                        &mut buffers,
                        &symbols,
                        &tables,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                        $text_recoveries,
                    )?;
                }
                SegmentType::IntermediateHalftoneRegion
                | SegmentType::ImmediateHalftoneRegion
                | SegmentType::ImmediateLosslessHalftoneRegion => {
                    decode_halftone_segment(
                        segment,
                        &mut pages,
                        &mut buffers,
                        &patterns,
                        &mut retained,
                        &mut cumulative_work,
                        limits,
                    )?;
                }
                SegmentType::EndOfPage => {
                    let number = segment.header.page_association;
                    let index = runtime_page_index(&pages, number).ok_or(
                        MalformedError::InvalidPageState("end-of-page without active page"),
                    )?;
                    #[cfg(verus_keep_ghost)]
                    proof_decl! {
                        let ghost original_pages = pages@;
                    }
                    let page = pages.remove(index).1;
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        reveal(decode_spec::runtime_pages_valid);
                        assert forall|entry: int| 0 <= entry < pages@.len()
                            implies crate::spec::bitmap::storage_valid(
                                #[trigger] &pages@[entry].1.bitmap
                            ) by {
                            if entry < index {
                                assert(pages@[entry] == original_pages[entry]);
                            } else {
                                assert(pages@[entry] == original_pages[entry + 1]);
                            }
                        }
                    }
                    retained.release(bitmap_byte_len(&page.bitmap)?)?;
                    $write_page(
                        sink,
                        Page {
                            number,
                            bitmap: page.bitmap,
                        },
                    )?;
                }
                SegmentType::EndOfStripe => {
                    let ParsedData::EndOfStripe(row) = segment.data else {
                        return Err(MalformedError::InvalidPageState(
                            "end-of-stripe payload was not parsed",
                        )
                        .into());
                    };
                    seal_stripe(&mut pages, segment.header.page_association, row)?;
                }
                SegmentType::EndOfFile | SegmentType::Profiles | SegmentType::Extension => {}
                SegmentType::Tables => {
                    let table = HuffmanTable::from_encoded(segment.payload, limits)
                        .map_err(huffman_error_into_decode)?;
                    if !insert_huffman_table(&mut tables, segment.header.number, table) {
                        return Err(
                            MalformedError::DuplicateSegmentNumber(segment.header.number).into(),
                        );
                    }
                }
                SegmentType::ColourPalette => {
                    return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
                }
            }
            release_referred_buffers(segment, &mut buffers, &mut retained)?;
            release_referred_patterns(segment, &mut patterns, &mut retained)?;
            release_referred_symbols(segment, &mut symbols, &mut retained)?;
            release_referred_tables(segment, &mut tables);
            segment_position += 1;
        }
        if !pages.is_empty() {
            if !$embedded || pages.len() != 1 {
                return Err(MalformedError::InvalidPageState("active page after final segment").into());
            }
            // Parsing has validated the complete embedded stream and final page.
            let (number, page) = pages.remove(0);
            retained.release(bitmap_byte_len(&page.bitmap)?)?;
            $write_page(sink, Page { number, bitmap: page.bitmap })?;
        }
        Ok(())
    }};
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires match initial_resources {
        Some(resources) => decode_spec::initial_resources_valid(&resources),
        None => true,
    },
))]
pub(super) fn decode_segments_collected(
    segments: &[ParsedSegment<'_>],
    info: &DocumentInfo,
    initial_work: u64,
    initial_retained_bytes: u64,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
    initial_resources: Option<InitialResources>,
    sink: &mut PageCollector,
    embedded: bool,
) -> Result<(), DecodeError> {
    decode_segments_body!(
        segments,
        info,
        initial_work,
        initial_retained_bytes,
        limits,
        mmr_scratch,
        initial_resources,
        sink,
        embedded,
        &[],
        None::<&mut Vec<crate::Recovery>>,
        PageCollector::push_page
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn decode_segments<S: PageSink>(
    segments: &[ParsedSegment<'_>],
    info: &DocumentInfo,
    initial_work: u64,
    initial_retained_bytes: u64,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
    initial_resources: Option<InitialResources>,
    sink: &mut S,
) -> Result<(), DecodeError> {
    decode_segments_body!(
        segments,
        info,
        initial_work,
        initial_retained_bytes,
        limits,
        mmr_scratch,
        initial_resources,
        sink,
        false,
        &[],
        None::<&mut Vec<crate::Recovery>>,
        write_page_to_sink::<S>
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn decode_segments_compatible_collected(
    segments: &[ParsedSegment<'_>],
    info: &DocumentInfo,
    initial_work: u64,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
    sink: &mut PageCollector,
    recoveries: &mut Vec<crate::Recovery>,
) -> Result<(), DecodeError> {
    decode_segments_body!(
        segments,
        info,
        initial_work,
        0,
        limits,
        mmr_scratch,
        None::<InitialResources>,
        sink,
        true,
        &*recoveries,
        Some(&mut *recoveries),
        PageCollector::push_page
    )
}
