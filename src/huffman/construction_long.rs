//! Cold out-of-line Huffman construction for rare tables with `max_prefix_len > 32`.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    build_prefix_lookup, canonical_first_code, canonical_range_fits, check_table_memory,
    try_reserve, HuffmanError, HuffmanLine, HuffmanLines, HuffmanTable, HuffmanTableError, Limits,
    LongPrefixTables, TableSlice, WideCode, MAX_PREFIX_LEN, MIN_FAST_PREFIX_LEN, SHORT_PREFIX_MAX,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        line_count == owned_lines@.len(),
        huffman_spec::is_raw(lines) ==>
            owned_lines@ == huffman_spec::raw_lines(lines)
            && line_count == huffman_spec::raw_lines(lines).len(),
    ensures
        match out {
            Ok(table) => huffman_spec::is_raw(lines) ==>
                huffman_spec::assignment_contents(huffman_spec::raw_lines(lines), &table)
                && huffman_spec::assignment_codes(huffman_spec::raw_lines(lines), &table)
                && huffman_spec::assignment_starts(huffman_spec::raw_lines(lines), &table)
                && huffman_spec::assignment_order(huffman_spec::raw_lines(lines), &table),
            Err(_) => true,
        },
        match out {
            Ok(table) => huffman_lookup_spec::table_lookup_valid(&table),
            Err(_) => true,
        },
))]
#[allow(clippy::redundant_closure, clippy::too_many_lines, unused_variables)]
#[cold]
#[inline(never)]
pub(super) fn from_line_source_long(
    lines: HuffmanLines<'_>,
    owned_lines: Vec<HuffmanLine>,
    line_count: usize,
    limits: &Limits,
) -> Result<HuffmanTable, HuffmanError> {
    let mut length_count = [0_u32; MAX_PREFIX_LEN + 1];
    let mut count_index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        if huffman_spec::is_raw(lines) {
            huffman_spec::histogram_empty(huffman_spec::raw_lines(lines), length_count@);
        }
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count_index <= line_count,
            line_count == owned_lines@.len(),
            huffman_spec::is_raw(lines) ==> owned_lines@ == huffman_spec::raw_lines(lines),
            huffman_spec::is_raw(lines) ==> line_count == huffman_spec::raw_lines(lines).len(),
            huffman_spec::is_raw(lines) ==>
                huffman_spec::histogram(huffman_spec::raw_lines(lines), length_count@, count_index as nat),
        decreases line_count - count_index,
    ))]
    while count_index < line_count {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost old_counts = length_count@; }
        let line = owned_lines[count_index];
        if line.prefix_len != 0 {
            let count = length_count
                .get_mut(usize::from(line.prefix_len))
                .ok_or(HuffmanTableError::Oversubscribed)
                .map_err(|error| HuffmanError::InvalidTable(error))?;
            *count = count
                .checked_add(1)
                .ok_or(HuffmanTableError::Oversubscribed)
                .map_err(|error| HuffmanError::InvalidTable(error))?;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if huffman_spec::is_raw(lines) {
                huffman_spec::histogram_step(
                    huffman_spec::raw_lines(lines), old_counts, length_count@, count_index as nat,
                );
            }
        }
        count_index += 1;
    }

    let mut max_length = MAX_PREFIX_LEN;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            max_length <= MAX_PREFIX_LEN,
            forall|length: int| max_length < length < 256 ==> length_count@[length] == 0,
        decreases max_length,
    ))]
    while max_length > 0 && length_count[max_length] == 0 {
        max_length -= 1;
    }
    if max_length == 0 || length_count[max_length] == 0 {
        return Err(HuffmanError::InvalidTable(HuffmanTableError::NoCodes));
    }
    let mut min_length = 1_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            1 <= min_length,
            min_length <= max_length,
            max_length <= MAX_PREFIX_LEN,
            forall|length: int| 1 <= length < min_length ==>
                #[trigger] length_count@[length] == 0,
        decreases max_length - min_length,
    ))]
    while min_length < max_length && length_count[min_length] == 0 {
        min_length += 1;
    }
    let max_prefix_len = u8::try_from(max_length)
        .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;

    let mut first_code = [WideCode::zero(); MAX_PREFIX_LEN + 1];
    let mut first_line = [0_u32; MAX_PREFIX_LEN + 1];
    let mut ordered_lines = Vec::new();
    try_reserve::<u32>(&mut ordered_lines, line_count)?;

    let mut length = 1_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(huffman_spec::is_raw(lines) ==>
            huffman_spec::assignment_prefix(
                huffman_spec::raw_lines(lines), first_code@, first_line@, ordered_lines@, 1,
            ));
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            1 <= length,
            length <= max_prefix_len as usize + 1,
            line_count == owned_lines@.len(),
            huffman_spec::is_raw(lines) ==> owned_lines@ == huffman_spec::raw_lines(lines),
            huffman_spec::is_raw(lines) ==> line_count == huffman_spec::raw_lines(lines).len(),
            huffman_spec::is_raw(lines) ==>
                huffman_spec::histogram(huffman_spec::raw_lines(lines), length_count@, line_count as nat),
            huffman_spec::is_raw(lines) ==>
                huffman_spec::assignment_prefix(
                    huffman_spec::raw_lines(lines), first_code@, first_line@, ordered_lines@, length as nat,
                ),
            forall|i: int| 0 <= i < length && i <= 32 ==>
                first_code@[i].limbs@[1] == 0
                && first_code@[i].limbs@[2] == 0
                && first_code@[i].limbs@[3] == 0
                && first_code@[i].limbs@[0] <= u32::MAX as u64,
        decreases max_prefix_len as usize + 1 - length,
    ))]
    while length <= usize::from(max_prefix_len) {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost old_codes = first_code@;
            let ghost old_starts = first_line@;
            let ghost old_order = ordered_lines@;
        }
        let previous = canonical_first_code(first_code[length - 1], length_count[length - 1])
            .ok_or(HuffmanTableError::Oversubscribed)
            .map_err(|error| HuffmanError::InvalidTable(error))?;
        if length <= SHORT_PREFIX_MAX
            && (previous.limbs[1] != 0
                || previous.limbs[2] != 0
                || previous.limbs[3] != 0
                || previous.limbs[0] > u64::from(u32::MAX))
        {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::Oversubscribed,
            ));
        }
        first_code[length] = previous;

        let count = length_count[length];
        if count != 0 {
            if !canonical_range_fits(previous, count, length) {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::Oversubscribed,
                ));
            }
            first_line[length] = u32::try_from(ordered_lines.len())
                .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;
            let mut index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    index <= line_count,
                    line_count == owned_lines@.len(),
                    1 <= length <= max_prefix_len,
                    huffman_spec::is_raw(lines) ==> owned_lines@ == huffman_spec::raw_lines(lines),
                    huffman_spec::is_raw(lines) ==> line_count == huffman_spec::raw_lines(lines).len(),
                    huffman_spec::is_raw(lines) ==> ordered_lines@ ==
                        old_order + huffman_spec::bucket(
                            huffman_spec::raw_lines(lines), length as nat, index as nat,
                        ),
                decreases line_count - index,
            ))]
            while index < line_count {
                #[cfg(verus_keep_ghost)]
                proof_decl! { let ghost before_order = ordered_lines@; }
                let line = owned_lines[index];
                if usize::from(line.prefix_len) == length {
                    let index = u32::try_from(index).map_err(|_| {
                        HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed)
                    })?;
                    ordered_lines.push(index);
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    if huffman_spec::is_raw(lines) {
                        huffman_spec::bucket_step(
                            huffman_spec::raw_lines(lines), length as nat, index as nat,
                            old_order, before_order, ordered_lines@,
                        );
                    }
                }
                index += 1;
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if huffman_spec::is_raw(lines) {
                if count == 0 {
                    huffman_spec::empty_bucket(
                        huffman_spec::raw_lines(lines), length as nat, line_count as nat, old_order,
                    );
                }
                huffman_spec::assignment_step(
                    huffman_spec::raw_lines(lines), length_count@,
                    old_codes, old_starts, old_order,
                    first_code@, first_line@, ordered_lines@, length as nat,
                );
            }
        }
        length += 1;
    }

    let prefer_prefix_lookup = min_length >= MIN_FAST_PREFIX_LEN;
    let prefix_lookup = if prefer_prefix_lookup {
        check_table_memory(line_count, true, limits)?;
        build_prefix_lookup(
            &length_count,
            &first_code,
            &first_line,
            &ordered_lines,
            usize::from(max_prefix_len),
        )?
    } else {
        Vec::new()
    };

    let mut short_length_count = [0_u32; SHORT_PREFIX_MAX + 1];
    let mut short_first_code = [0_u32; SHORT_PREFIX_MAX + 1];
    let mut short_first_line = [0_u32; SHORT_PREFIX_MAX + 1];
    let mut i = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            i <= SHORT_PREFIX_MAX + 1,
            forall|k: int| 0 <= k < i ==>
                short_length_count@[k] == length_count@[k]
                && short_first_line@[k] == first_line@[k]
                && (k <= max_prefix_len ==>
                    WideCode { limbs: [short_first_code@[k] as u64, 0, 0, 0] } == first_code@[k]),
            forall|k: int| 0 <= k <= max_prefix_len && k <= 32 ==>
                first_code@[k].limbs@[1] == 0
                && first_code@[k].limbs@[2] == 0
                && first_code@[k].limbs@[3] == 0
                && first_code@[k].limbs@[0] <= u32::MAX as u64,
        decreases SHORT_PREFIX_MAX + 1 - i,
    ))]
    while i <= SHORT_PREFIX_MAX {
        short_length_count[i] = length_count[i];
        #[allow(clippy::cast_possible_truncation)]
        {
            short_first_code[i] = first_code[i].limbs[0] as u32;
        }
        short_first_line[i] = first_line[i];
        #[cfg(verus_keep_ghost)]
        proof! {
            if i <= max_prefix_len as usize {
                assert(ext_equal(
                    WideCode { limbs: [short_first_code@[i as int] as u64, 0, 0, 0] }.limbs,
                    first_code@[i as int].limbs,
                ));
            }
        }
        i += 1;
    }

    let long_tables = Some(Box::new(LongPrefixTables {
        length_count,
        first_code,
        first_line,
    }));
    let output = HuffmanTable {
        lines: TableSlice::Owned(owned_lines),
        ordered_lines: TableSlice::Owned(ordered_lines),
        short_length_count,
        short_first_code,
        short_first_line,
        long_tables,
        prefix_lookup: TableSlice::Owned(prefix_lookup),
        prefer_prefix_lookup,
        max_prefix_len,
    };

    #[cfg(verus_keep_ghost)]
    proof! {
        assert(ext_equal(huffman_spec::table_length_counts(&output), length_count@));
        assert(ext_equal(huffman_spec::table_first_lines(&output), first_line@));
        assert forall|k: int| 0 <= k <= max_prefix_len as int implies
            huffman_spec::table_first_codes(&output)[k] == first_code@[k]
        by {
        };
        huffman_lookup_spec::table_lookup_definition();
        if prefer_prefix_lookup {
            assert(min_length >= 8);
            assert(max_prefix_len >= 8);
            assert(huffman_lookup_spec::fast_shape(huffman_spec::table_length_counts(&output)));
            assert(huffman_spec::slice_view(&output.prefix_lookup).len() == 256);
            assert(huffman_spec::table_first_codes(&output)[8] == first_code@[8]);
        }
        huffman_spec::canonical_assignment_definition();
        if huffman_spec::is_raw(lines) {
            assert(huffman_spec::slice_view(&output.lines) == huffman_spec::raw_lines(lines));
            assert(huffman_spec::histogram(
                huffman_spec::raw_lines(lines),
                huffman_spec::table_length_counts(&output),
                line_count as nat,
            ));
            assert(huffman_spec::table_first_codes(&output)[0].limbs@ == seq![0u64, 0u64, 0u64, 0u64]);
            assert(huffman_spec::assignment_prefix(
                huffman_spec::raw_lines(lines),
                huffman_spec::table_first_codes(&output),
                huffman_spec::table_first_lines(&output),
                huffman_spec::slice_view(&output.ordered_lines),
                max_prefix_len as nat + 1,
            ));
            huffman_spec::assignment_complete(huffman_spec::raw_lines(lines), &output);
        }
    }
    Ok(output)
}
