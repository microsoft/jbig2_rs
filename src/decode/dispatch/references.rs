//! References implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    allocation_size, get_huffman_table, get_symbol_resource, huffman_table_index, Bitmap,
    CollectedSymbols, DecodeError, HuffmanTable, HuffmanTables, Limits, MalformedError,
    ParsedSegment, SymbolStore,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::symbol_store_valid(symbols),
    ensures match out {
        Ok(bitmaps) => decode_spec::collected_symbols_valid(&bitmaps)
            && decode_spec::collected_symbol_count(&bitmaps)
                == decode_spec::referred_symbol_count(
                    symbols@, segment.header.referred_segments@,
                    segment.header.referred_segments@.len(),
                ),
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_lines)]
pub(in crate::decode) fn collect_referred_symbols<'a>(
    segment: &ParsedSegment<'_>,
    symbols: &'a SymbolStore,
    active_retained_bytes: u64,
    limits: &Limits,
) -> Result<CollectedSymbols<'a>, DecodeError> {
    let mut count = 0_usize;
    let mut nonempty_groups = 0_usize;
    let mut contiguous: Option<&[Bitmap]> = None;
    let mut reference_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            reference_index <= segment.header.referred_segments@.len(),
            decode_spec::symbol_store_valid(symbols),
            count as nat == decode_spec::referred_symbol_count(
                symbols@, segment.header.referred_segments@, reference_index as nat,
            ),
            nonempty_groups == 0 ==> count == 0 && contiguous.is_none(),
            nonempty_groups <= 1 ==> match contiguous {
                Some(bitmaps) => count == bitmaps@.len(),
                None => count == 0,
            },
            match contiguous {
                Some(bitmaps) =>
                    crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
                None => true,
            },
        decreases segment.header.referred_segments@.len() - reference_index,
    ))]
    while reference_index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[reference_index];
        if let Some(resource) = get_symbol_resource(symbols, number) {
            if !resource.exported.is_empty() {
                nonempty_groups = nonempty_groups
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?;
                contiguous = Some(&resource.exported);
            }
            let mut bitmap_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    bitmap_index <= resource.exported@.len(),
                    count as nat == decode_spec::referred_symbol_count(
                        symbols@, segment.header.referred_segments@, reference_index as nat,
                    ) + bitmap_index as nat,
                    crate::halftone::halftone_spec::bitmaps_valid(
                        resource.exported@
                    ),
                decreases resource.exported@.len() - bitmap_index,
            ))]
            while bitmap_index < resource.exported.len() {
                count = count
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?;
                bitmap_index += 1;
            }
        }
        reference_index += 1;
    }
    if nonempty_groups <= 1 {
        let direct = match contiguous {
            Some(bitmaps) => bitmaps,
            None => &[],
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(decode_spec::collected_symbols_valid);
        }
        return Ok(CollectedSymbols::Contiguous(direct));
    }
    let requested_bytes = check_borrowed_symbol_memory(count, active_retained_bytes, limits)?;
    let mut imported: Vec<&Bitmap> = Vec::new();
    if imported.try_reserve(count).is_err() {
        return Err(DecodeError::Allocation { requested_bytes });
    }
    check_borrowed_symbol_memory(imported.capacity(), active_retained_bytes, limits)?;
    reference_index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            reference_index <= segment.header.referred_segments@.len(),
            decode_spec::symbol_store_valid(symbols),
            decode_spec::bitmap_refs_valid(imported@),
            imported@.len() == decode_spec::referred_symbol_count(
                symbols@, segment.header.referred_segments@, reference_index as nat,
            ),
        decreases segment.header.referred_segments@.len() - reference_index,
    ))]
    while reference_index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[reference_index];
        if let Some(resource) = get_symbol_resource(symbols, number) {
            let mut bitmap_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    bitmap_index <= resource.exported@.len(),
                    imported@.len() == decode_spec::referred_symbol_count(
                        symbols@, segment.header.referred_segments@, reference_index as nat,
                    ) + bitmap_index as nat,
                    crate::halftone::halftone_spec::bitmaps_valid(
                        resource.exported@
                    ),
                    decode_spec::bitmap_refs_valid(imported@),
                decreases resource.exported@.len() - bitmap_index,
            ))]
            while bitmap_index < resource.exported.len() {
                imported.push(&resource.exported[bitmap_index]);
                bitmap_index += 1;
            }
        }
        reference_index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(decode_spec::collected_symbols_valid);
    }
    Ok(CollectedSymbols::Borrowed(imported))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((bytes, peak)) =>
            bytes as nat == decode_spec::allocation_bytes::<&Bitmap>(count)
            && peak as nat == active_retained_bytes as nat + bytes as nat,
        Err(_) => true,
    },
))]
pub(in crate::decode) fn borrowed_symbol_memory(
    count: usize,
    active_retained_bytes: u64,
) -> Result<(u64, u64), DecodeError> {
    let requested_bytes = allocation_size::<&Bitmap>(count)?;
    let peak = active_retained_bytes
        .checked_add(requested_bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok((requested_bytes, peak))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) =>
            bytes as nat == decode_spec::allocation_bytes::<&Bitmap>(count)
            && active_retained_bytes as nat + bytes as nat
                <= limits.max_decoder_managed_memory_bytes,
        Err(_) => true,
    },
))]
pub(super) fn check_borrowed_symbol_memory(
    count: usize,
    active_retained_bytes: u64,
    limits: &Limits,
) -> Result<u64, DecodeError> {
    let (requested_bytes, peak) = borrowed_symbol_memory(count, active_retained_bytes)?;
    if peak > limits.max_decoder_managed_memory_bytes {
        return Err(crate::LimitExceeded {
            kind: crate::LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual: peak,
        }
        .into());
    }
    Ok(requested_bytes)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) => result@.len() <= segment.header.referred_segments@.len(),
        Err(_) => true,
    },
))]
pub(in crate::decode) fn collect_referred_tables<'a>(
    segment: &ParsedSegment<'_>,
    tables: &'a HuffmanTables,
) -> Result<Vec<&'a HuffmanTable>, DecodeError> {
    let mut count = 0_usize;
    let mut reference_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant reference_index <= segment.header.referred_segments@.len(),
        decreases segment.header.referred_segments@.len() - reference_index,
    ))]
    while reference_index < segment.header.referred_segments.len() {
        if huffman_table_index(tables, segment.header.referred_segments[reference_index]).is_some()
        {
            count = count
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        reference_index += 1;
    }
    let requested_bytes = allocation_size::<&HuffmanTable>(count)?;
    let mut result = Vec::new();
    if result.try_reserve(count).is_err() {
        return Err(DecodeError::Allocation { requested_bytes });
    }
    reference_index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            reference_index <= segment.header.referred_segments@.len(),
            result@.len() <= reference_index,
        decreases segment.header.referred_segments@.len() - reference_index,
    ))]
    while reference_index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[reference_index];
        if let Some(table) = get_huffman_table(tables, number) {
            result.push(table);
        }
        reference_index += 1;
    }
    Ok(result)
}
