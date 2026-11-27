use super::{
    Bitmap, DecodeError, DecodeWork, HuffmanTable, HuffmanTables, Limits, MalformedError,
    ParsedSegment, PatternStore, RegionBuffer, RegionBuffers, RetainedBudget, RuntimePage,
    RuntimePages, SymbolResource, SymbolStore,
};

#[cfg(verus_keep_ghost)]
use super::{decode_shared, decode_spec};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < pages@.len()
            && pages@[index as int].0 == number,
        None =>
            forall|index: int| 0 <= index < pages@.len() ==>
                pages@[index].0 != number,
    },
))]
pub(super) fn runtime_page_index(pages: &[(u32, RuntimePage)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        invariant
            index <= pages@.len(),
            forall|completed: int| 0 <= completed < index ==>
                pages@[completed].0 != number,
        decreases pages@.len() - index,
    ))]
    while index < pages.len() {
        if pages[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::runtime_pages_valid(pages),
    ensures match out {
        Some(page) => crate::spec::bitmap::storage_valid(&page.bitmap),
        None => true,
    },
))]
pub(super) fn get_runtime_page(pages: &RuntimePages, number: u32) -> Option<&RuntimePage> {
    let index = runtime_page_index(pages, number)?;
    Some(&pages[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        crate::spec::bitmap::storage_valid(&page.bitmap),
    ensures decode_spec::runtime_pages_valid(final(pages)),
))]
pub(super) fn insert_runtime_page(pages: &mut RuntimePages, number: u32, page: RuntimePage) {
    if let Some(index) = runtime_page_index(pages, number) {
        pages[index].1 = page;
    } else {
        pages.push((number, page));
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < buffers@.len()
            && buffers@[index as int].0 == number,
        None =>
            forall|index: int| 0 <= index < buffers@.len() ==>
                buffers@[index].0 != number,
    },
))]
pub(super) fn region_buffer_index(buffers: &[(u32, RegionBuffer)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        invariant
            index <= buffers@.len(),
            forall|completed: int| 0 <= completed < index ==>
                buffers@[completed].0 != number,
        decreases buffers@.len() - index,
    ))]
    while index < buffers.len() {
        if buffers[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::region_buffers_valid(buffers),
    ensures match out {
        Some(buffer) => crate::spec::bitmap::storage_valid(&buffer.bitmap),
        None => true,
    },
))]
pub(super) fn get_region_buffer(buffers: &RegionBuffers, number: u32) -> Option<&RegionBuffer> {
    let index = region_buffer_index(buffers, number)?;
    Some(&buffers[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < symbols@.len()
            && symbols@[index as int].0 == number
            && decode_spec::first_symbol_index(symbols@, number, index as int),
        None =>
            forall|index: int| 0 <= index < symbols@.len() ==>
                symbols@[index].0 != number,
    },
))]
pub(super) fn symbol_index(symbols: &[(u32, SymbolResource)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= symbols@.len(),
            forall|completed: int| 0 <= completed < index ==>
                symbols@[completed].0 != number,
        decreases symbols@.len() - index,
    ))]
    while index < symbols.len() {
        if symbols[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::symbol_store_valid(symbols),
    ensures match out {
        Some(resource) =>
            crate::halftone::halftone_spec::bitmaps_valid(resource.exported@)
            && resource.exported@.len() == decode_spec::symbol_group_count(symbols@, number),
        None => decode_spec::symbol_group_count(symbols@, number) == 0,
    },
))]
pub(super) fn get_symbol_resource(symbols: &SymbolStore, number: u32) -> Option<&SymbolResource> {
    let index = symbol_index(symbols, number)?;
    #[cfg(verus_keep_ghost)]
    production_symbol_resource_lookup!(symbols, number, index);
    Some(&symbols[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::symbol_store_valid(old(symbols)),
        crate::halftone::halftone_spec::bitmaps_valid(resource.exported@),
    ensures decode_spec::symbol_store_valid(final(symbols)),
))]
pub(super) fn insert_symbol_resource(
    symbols: &mut SymbolStore,
    number: u32,
    resource: SymbolResource,
) -> bool {
    if symbol_index(symbols, number).is_some() {
        return false;
    }
    symbols.push((number, resource));
    #[cfg(verus_keep_ghost)]
    production_symbol_resource_insert!(symbols, resource);
    true
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < tables@.len()
            && tables@[index as int].0 == number,
        None =>
            forall|index: int| 0 <= index < tables@.len() ==>
                tables@[index].0 != number,
    },
))]
pub(super) fn huffman_table_index(tables: &[(u32, HuffmanTable)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= tables@.len(),
            forall|completed: int| 0 <= completed < index ==>
                tables@[completed].0 != number,
        decreases tables@.len() - index,
    ))]
    while index < tables.len() {
        if tables[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn get_huffman_table(tables: &HuffmanTables, number: u32) -> Option<&HuffmanTable> {
    let index = huffman_table_index(tables, number)?;
    Some(&tables[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn insert_huffman_table(
    tables: &mut HuffmanTables,
    number: u32,
    table: HuffmanTable,
) -> bool {
    if huffman_table_index(tables, number).is_some() {
        return false;
    }
    tables.push((number, table));
    true
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) =>
            index < patterns@.len()
            && patterns@[index as int].0 == number,
        None =>
            forall|index: int| 0 <= index < patterns@.len() ==>
                patterns@[index].0 != number,
    },
))]
pub(super) fn pattern_index(patterns: &[(u32, Vec<Bitmap>)], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= patterns@.len(),
            forall|completed: int| 0 <= completed < index ==>
                patterns@[completed].0 != number,
        decreases patterns@.len() - index,
    ))]
    while index < patterns.len() {
        if patterns[index].0 == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::pattern_store_valid(patterns),
    ensures match out {
        Some(bitmaps) =>
            crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
        None => true,
    },
))]
pub(super) fn get_patterns(patterns: &PatternStore, number: u32) -> Option<&[Bitmap]> {
    let index = pattern_index(patterns, number)?;
    Some(&patterns[index].1)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::pattern_store_valid(old(patterns)),
        crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
    ensures decode_spec::pattern_store_valid(final(patterns)),
))]
pub(super) fn insert_patterns(
    patterns: &mut PatternStore,
    number: u32,
    bitmaps: Vec<Bitmap>,
) -> bool {
    if pattern_index(patterns, number).is_some() {
        return false;
    }
    patterns.push((number, bitmaps));
    true
}

impl DecodeWork {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(work) =>
                decode_spec::work_used(&work) == initial
                    && decode_spec::work_used(&work) <= limits.max_work_units,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn new(initial: u64, limits: &Limits) -> Result<Self, DecodeError> {
        let mut work = Self { used: 0 };
        #[cfg(verus_keep_ghost)]
        production_work_fields!(&work);
        work.charge(initial, limits)?;
        Ok(work)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(()) =>
                decode_spec::work_used(final(self))
                    == decode_spec::work_used(old(self)) + units
                && decode_spec::work_used(final(self)) <= limits.max_work_units,
            Err(_) =>
                decode_spec::work_used(final(self))
                    == decode_spec::work_used(old(self)),
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn charge(&mut self, units: u64, limits: &Limits) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        production_work_fields!(self);
        let next = self
            .used
            .checked_add(units)
            .ok_or(MalformedError::IntegerOverflow)?;
        if next > limits.max_work_units {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::Work,
                limit: limits.max_work_units,
                actual: next,
            }
            .into());
        }
        self.used = next;
        #[cfg(verus_keep_ghost)]
        production_work_fields!(self);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn can_charge_pair(&self, first: u64, second: u64, limits: &Limits) -> bool {
        match self
            .used
            .checked_add(first)
            .and_then(|used| used.checked_add(second))
        {
            Some(used) => used <= limits.max_work_units,
            None => false,
        }
    }
}

impl RetainedBudget {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures decode_spec::retained_bytes(&out) == 0,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) const fn new() -> Self {
        let out = Self { bytes: 0 };
        #[cfg(verus_keep_ghost)]
        production_retained_fields!(&out);
        out
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(()) =>
                decode_spec::retained_bytes(final(self))
                    == decode_spec::retained_bytes(old(self)) + bytes
                && decode_spec::retained_bytes(final(self))
                    <= limits.max_retained_state_bytes,
            Err(_) =>
                decode_spec::retained_bytes(final(self))
                    == decode_spec::retained_bytes(old(self)),
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn retain(&mut self, bytes: u64, limits: &Limits) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        production_retained_fields!(self);
        let next = self
            .bytes
            .checked_add(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        if next > limits.max_retained_state_bytes {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::RetainedState,
                limit: limits.max_retained_state_bytes,
                actual: next,
            }
            .into());
        }
        self.bytes = next;
        #[cfg(verus_keep_ghost)]
        production_retained_fields!(self);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(()) =>
                decode_spec::retained_bytes(final(self)) + bytes
                    == decode_spec::retained_bytes(old(self)),
            Err(_) =>
                decode_spec::retained_bytes(final(self))
                    == decode_spec::retained_bytes(old(self)),
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn release(&mut self, bytes: u64) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        production_retained_fields!(self);
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        #[cfg(verus_keep_ghost)]
        production_retained_fields!(self);
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::region_buffers_valid(old(buffers)),
        crate::spec::bitmap::storage_valid(&bitmap),
    ensures
        decode_spec::buffer_insertion_result(
            old(buffers)@, final(buffers)@, number, region, bitmap, out,
        ),
        match out {
            Ok(()) => decode_spec::region_buffers_valid(final(buffers)),
            Err(_) => true,
        },
))]
pub(super) fn insert_buffer(
    buffers: &mut RegionBuffers,
    number: u32,
    region: crate::segment_data::RegionInfo,
    bitmap: Bitmap,
    retained: &mut RetainedBudget,
    limits: &Limits,
) -> Result<(), DecodeError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(decode_spec::buffer_insertion_result);
    }
    if region_buffer_index(buffers, number).is_some() {
        return Err(DecodeError::Malformed(
            MalformedError::DuplicateSegmentNumber(number),
        ));
    }
    let requested_bytes = allocation_size::<(u32, RegionBuffer)>(1)?;
    if buffers.try_reserve(1).is_err() {
        return Err(DecodeError::Allocation { requested_bytes });
    }
    retain_bitmap(retained, &bitmap, limits)?;
    buffers.push((number, RegionBuffer { region, bitmap }));
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::region_buffers_valid(old(buffers)),
    ensures decode_spec::region_buffers_valid(final(buffers)),
))]
pub(super) fn release_referred_buffers(
    segment: &ParsedSegment<'_>,
    buffers: &mut RegionBuffers,
    retained: &mut RetainedBudget,
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= segment.header.referred_segments@.len(),
            decode_spec::region_buffers_valid(buffers),
        decreases segment.header.referred_segments@.len() - index,
    ))]
    while index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[index];
        if segment
            .header
            .retention_flags
            .retain_referred_segment(index)
            == Some(false)
        {
            if let Some(buffer_index) = region_buffer_index(buffers, number) {
                let buffer = buffers.remove(buffer_index).1;
                release_bitmap(retained, &buffer.bitmap)?;
            }
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            decode_spec::retained_bytes(final(retained)) as nat
                == decode_spec::retained_bytes(old(retained)) as nat
                    + decode_spec::bitmap_byte_count(bitmap)
            && decode_spec::retained_bytes(final(retained))
                <= limits.max_retained_state_bytes,
        Err(_) =>
            decode_spec::retained_bytes(final(retained))
                == decode_spec::retained_bytes(old(retained)),
    },
))]
pub(super) fn retain_bitmap(
    retained: &mut RetainedBudget,
    bitmap: &Bitmap,
    limits: &Limits,
) -> Result<(), DecodeError> {
    retained.retain(bitmap_byte_len(bitmap)?, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            decode_spec::retained_bytes(final(retained)) as nat
                    + decode_spec::bitmap_byte_count(bitmap)
                == decode_spec::retained_bytes(old(retained)) as nat,
        Err(_) =>
            decode_spec::retained_bytes(final(retained))
                == decode_spec::retained_bytes(old(retained)),
    },
))]
pub(super) fn release_bitmap(
    retained: &mut RetainedBudget,
    bitmap: &Bitmap,
) -> Result<(), DecodeError> {
    retained.release(bitmap_byte_len(bitmap)?)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::pattern_store_valid(old(patterns)),
    ensures decode_spec::pattern_store_valid(final(patterns)),
))]
pub(super) fn release_referred_patterns(
    segment: &ParsedSegment<'_>,
    patterns: &mut PatternStore,
    retained: &mut RetainedBudget,
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= segment.header.referred_segments@.len(),
            decode_spec::pattern_store_valid(patterns),
        decreases segment.header.referred_segments@.len() - index,
    ))]
    while index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[index];
        if segment
            .header
            .retention_flags
            .retain_referred_segment(index)
            == Some(false)
        {
            if let Some(pattern_index) = pattern_index(patterns, number) {
                #[cfg(verus_keep_ghost)]
                proof_decl! {
                    let ghost original_patterns = patterns@;
                }
                let bitmaps = patterns.remove(pattern_index).1;
                #[cfg(verus_keep_ghost)]
                production_pattern_store_removed!(patterns, pattern_index, original_patterns);
                let mut bitmap_index = 0_usize;
                let mut released = 0_u64;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        bitmap_index <= bitmaps@.len(),
                        decode_spec::pattern_store_valid(patterns),
                        crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
                    decreases bitmaps@.len() - bitmap_index,
                ))]
                while bitmap_index < bitmaps.len() {
                    released = released
                        .checked_add(bitmap_byte_len(&bitmaps[bitmap_index])?)
                        .ok_or(MalformedError::IntegerOverflow)?;
                    bitmap_index += 1;
                }
                retained.release(released)?;
            }
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::symbol_store_valid(old(symbols)),
    ensures decode_spec::symbol_store_valid(final(symbols)),
))]
pub(super) fn release_referred_symbols(
    segment: &ParsedSegment<'_>,
    symbols: &mut SymbolStore,
    retained: &mut RetainedBudget,
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= segment.header.referred_segments@.len(),
            decode_spec::symbol_store_valid(symbols),
        decreases segment.header.referred_segments@.len() - index,
    ))]
    while index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[index];
        if segment
            .header
            .retention_flags
            .retain_referred_segment(index)
            == Some(false)
        {
            if let Some(symbol_index) = symbol_index(symbols, number) {
                #[cfg(verus_keep_ghost)]
                proof_decl! {
                    let ghost original_symbols = symbols@;
                }
                let resource = symbols.remove(symbol_index).1;
                #[cfg(verus_keep_ghost)]
                production_symbol_store_removed!(symbols, symbol_index, original_symbols);
                let mut bitmap_index = 0_usize;
                let mut released = 0_u64;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        bitmap_index <= resource.exported@.len(),
                        decode_spec::symbol_store_valid(symbols),
                    decreases resource.exported@.len() - bitmap_index,
                ))]
                while bitmap_index < resource.exported.len() {
                    released = released
                        .checked_add(bitmap_byte_len(&resource.exported[bitmap_index])?)
                        .ok_or(MalformedError::IntegerOverflow)?;
                    bitmap_index += 1;
                }
                retained.release(released)?;
            }
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn release_referred_tables(segment: &ParsedSegment<'_>, tables: &mut HuffmanTables) {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segment.header.referred_segments@.len(),
        decreases segment.header.referred_segments@.len() - index,
    ))]
    while index < segment.header.referred_segments.len() {
        let number = segment.header.referred_segments[index];
        if segment
            .header
            .retention_flags
            .retain_referred_segment(index)
            == Some(false)
        {
            if let Some(table_index) = huffman_table_index(tables, number) {
                tables.remove(table_index);
            }
        }
        index += 1;
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) => bytes as nat == decode_spec::allocation_bytes::<T>(count),
        Err(_) => true,
    },
))]
pub(super) fn allocation_size<T>(count: usize) -> Result<u64, DecodeError> {
    let Some(bytes) = count.checked_mul(std::mem::size_of::<T>()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let Ok(bytes) = u64::try_from(bytes) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    Ok(bytes)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn structural_work(
    input_length: usize,
    segments: &[ParsedSegment<'_>],
) -> Result<u64, DecodeError> {
    let Ok(input) = u64::try_from(input_length) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let Ok(segment_count) = u64::try_from(segments.len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let mut references = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segments@.len(),
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        let Ok(count) = u64::try_from(segments[index].header.referred_segments.len()) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        let Some(next) = references.checked_add(count) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        references = next;
        index += 1;
    }
    structural_work_totals(input, segment_count, references)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(total) =>
            total as nat
                == input as nat + segment_count as nat + references as nat
            && total >= input
            && total >= segment_count
            && total >= references,
        Err(_) =>
            input as nat + segment_count as nat + references as nat
                > u64::MAX as nat,
    },
))]
pub(super) fn structural_work_totals(
    input: u64,
    segment_count: u64,
    references: u64,
) -> Result<u64, DecodeError> {
    checked_work_add(checked_work_add(input, segment_count)?, references)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(total) =>
            total as nat == left as nat + right as nat
                && total >= left
                && total >= right,
        Err(_) => left as nat + right as nat > u64::MAX as nat,
    },
))]
pub(super) fn checked_work_add(left: u64, right: u64) -> Result<u64, DecodeError> {
    match left.checked_add(right) {
        Some(total) => Ok(total),
        None => Err(MalformedError::IntegerOverflow.into()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) => bytes as nat == decode_spec::bitmap_byte_count(bitmap),
        Err(_) => decode_spec::bitmap_byte_count(bitmap) > u64::MAX as nat,
    },
))]
pub(super) fn bitmap_byte_len(bitmap: &Bitmap) -> Result<u64, DecodeError> {
    u64::try_from(bitmap.bytes().len()).map_err(|_error| MalformedError::IntegerOverflow.into())
}
