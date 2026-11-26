//! Construction implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    bit_reader, build_prefix_lookup_short, check_table_memory, construction_long,
    encoded_table_fields, padding_bit_count, push_parsed_line, read_u8_bits, standard_lines,
    standard_static, try_reserve, validate_lines, ByteReader, HuffmanError, HuffmanLine,
    HuffmanLines, HuffmanTable, HuffmanTableError, LimitExceeded, LimitKind, Limits,
    MalformedError, StandardHuffmanTable, TableSlice, MIN_FAST_PREFIX_LEN, SHORT_PREFIX_MAX,
};

impl HuffmanTable {
    /// Validates table lines and assigns their canonical prefix codes.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(table) => huffman_spec::canonical_assignment(lines@, &table)
                && huffman_lookup_spec::table_lookup_valid(&table),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(lines: &[HuffmanLine], limits: &Limits) -> Result<Self, HuffmanError> {
        Self::from_line_source(HuffmanLines::Borrowed(lines), limits)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(table) => huffman_lookup_spec::table_lookup_valid(&table),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new_symbol_ids(lengths: &[u8], limits: &Limits) -> Result<Self, HuffmanError> {
        if let Some(maximum) = lengths.len().checked_sub(1) {
            i32::try_from(maximum).map_err(|_| MalformedError::IntegerOverflow)?;
        }
        Self::from_line_source(HuffmanLines::SymbolLengths(lengths), limits)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
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
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::redundant_closure, clippy::too_many_lines)]
    pub(super) fn from_line_source(
        lines: HuffmanLines<'_>,
        limits: &Limits,
    ) -> Result<Self, HuffmanError> {
        let line_count = lines.len();
        validate_lines(lines)?;
        check_table_memory(line_count, false, limits)?;

        let mut owned_lines: Vec<HuffmanLine> = Vec::new();
        try_reserve::<HuffmanLine>(&mut owned_lines, line_count)?;
        let mut source_index = 0_usize;
        let mut max_seen_len = 0_u8;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                source_index <= line_count,
                owned_lines@.len() == source_index,
                forall|i: int| 0 <= i < source_index ==>
                    owned_lines@[i].prefix_len <= max_seen_len,
                max_seen_len > 0 ==> exists|i: int|
                    0 <= i < source_index && owned_lines@[i].prefix_len == max_seen_len,
                huffman_spec::is_raw(lines) ==>
                    owned_lines@ == huffman_spec::raw_lines(lines).take(source_index as int),
            decreases line_count - source_index,
        ))]
        while source_index < line_count {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost old_lines = owned_lines@; }
            let line = lines.line(source_index).ok_or(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidLineOrder,
            ))?;
            let updated_max = line.prefix_len > max_seen_len;
            if updated_max {
                max_seen_len = line.prefix_len;
            }
            owned_lines.push(line);
            #[cfg(verus_keep_ghost)]
            proof! {
                if updated_max {
                    assert(owned_lines@[source_index as int].prefix_len == max_seen_len);
                } else if max_seen_len > 0 {
                    let prev_w = choose|i: int|
                        0 <= i < source_index && old_lines[i].prefix_len == max_seen_len;
                    assert(owned_lines@[prev_w] == old_lines[prev_w]);
                    assert(0 <= prev_w < source_index + 1);
                }
            }
            source_index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if huffman_spec::is_raw(lines) {
                huffman_spec::full_prefix(huffman_spec::raw_lines(lines));
            }
        }
        if max_seen_len == 0 {
            return Err(HuffmanError::InvalidTable(HuffmanTableError::NoCodes));
        }
        if usize::from(max_seen_len) > SHORT_PREFIX_MAX {
            return construction_long::from_line_source_long(
                lines,
                owned_lines,
                line_count,
                limits,
            );
        }

        let max_prefix_len = max_seen_len;
        let max_length = usize::from(max_prefix_len);
        let mut short_length_count = [0_u32; SHORT_PREFIX_MAX + 1];
        let mut count_index = 0_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            if huffman_spec::is_raw(lines) {
                let padded = Seq::new(256, |i: int| if i <= 32 { short_length_count@[i] } else { 0u32 });
                huffman_spec::histogram_empty(huffman_spec::raw_lines(lines), padded);
            }
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                count_index <= line_count,
                line_count == owned_lines@.len(),
                1 <= max_prefix_len <= 32,
                max_length == max_prefix_len as usize,
                forall|i: int| 0 <= i < line_count ==> owned_lines@[i].prefix_len <= max_prefix_len,
                exists|i: int| 0 <= i < line_count && owned_lines@[i].prefix_len == max_prefix_len,
                forall|l: int| 0 <= l <= 32 ==>
                    short_length_count@[l] == huffman_spec::bucket(owned_lines@, l as nat, count_index as nat).len(),
                huffman_spec::is_raw(lines) ==> owned_lines@ == huffman_spec::raw_lines(lines),
            decreases line_count - count_index,
        ))]
        while count_index < line_count {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before_counts = Seq::new(256, |i: int| if i <= 32 { short_length_count@[i] } else { 0u32 });
            }
            let plen = usize::from(owned_lines[count_index].prefix_len);
            if plen != 0 {
                let next_count = short_length_count[plen]
                    .checked_add(1)
                    .ok_or(HuffmanTableError::Oversubscribed)
                    .map_err(|error| HuffmanError::InvalidTable(error))?;
                short_length_count[plen] = next_count;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                let after_counts = Seq::new(256, |i: int| if i <= 32 { short_length_count@[i] } else { 0u32 });
                assert(huffman_spec::histogram(owned_lines@, before_counts, count_index as nat)) by {
                    assert forall|l: int| 0 <= l < 256 implies
                        before_counts[l] == huffman_spec::bucket(owned_lines@, l as nat, count_index as nat).len()
                    by {
                        if l > 32 {
                            huffman_spec::bucket_absent_above_max(owned_lines@, l as nat, count_index as nat, max_prefix_len);
                        }
                    };
                };
                huffman_spec::histogram_step(owned_lines@, before_counts, after_counts, count_index as nat);
            }
            count_index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            let witness = choose|i: int| 0 <= i < line_count && owned_lines@[i].prefix_len == max_prefix_len;
            huffman_spec::bucket_nonempty_witness(owned_lines@, max_prefix_len as nat, line_count as nat, witness);
            assert(short_length_count@[max_prefix_len as int] > 0);
        }

        let mut min_length = 1_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                1 <= min_length <= max_length,
                max_length <= 32,
                max_length == max_prefix_len as usize,
                forall|length: int| 1 <= length < min_length ==>
                    #[trigger] short_length_count@[length] == 0,
            decreases max_length - min_length,
        ))]
        while min_length < max_length && short_length_count[min_length] == 0 {
            min_length += 1;
        }

        let mut short_first_code = [0_u32; SHORT_PREFIX_MAX + 1];
        let mut short_first_line = [0_u32; SHORT_PREFIX_MAX + 1];
        let mut total_coded = 0_u32;
        let mut length = 1_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                1 <= length <= max_length + 1,
                max_length == max_prefix_len as usize,
                1 <= max_prefix_len <= 32,
                line_count == owned_lines@.len(),
                short_first_code@[0] == 0,
                total_coded as nat == huffman_spec::ordered_before(owned_lines@, length as nat).len(),
                forall|l: int| 0 <= l <= 32 ==>
                    short_length_count@[l] == huffman_spec::bucket(owned_lines@, l as nat, line_count as nat).len(),
                forall|l: int| 1 <= l < length ==>
                    #[trigger] huffman_spec::first_prefix(owned_lines@, l as nat)
                        == Some(WideCode { limbs: [short_first_code@[l] as u64, 0, 0, 0] })
                    && (short_length_count@[l] > 0 ==>
                        huffman_spec::canonical_range_fits(
                            WideCode { limbs: [short_first_code@[l] as u64, 0, 0, 0] },
                            short_length_count@[l],
                            l as usize,
                        )),
                forall|l: int| 1 <= l < length ==>
                    short_first_line@[l] as nat
                        == huffman_spec::ordered_before(owned_lines@, l as nat).len(),
            decreases max_length + 1 - length,
        ))]
        while length <= max_length {
            let prev_code = u64::from(short_first_code[length - 1]);
            let prev_count = u64::from(short_length_count[length - 1]);
            let next_u64 = (prev_code + prev_count) << 1;
            let limit = 1_u64 << length;
            if next_u64 > u64::from(u32::MAX) {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::Oversubscribed,
                ));
            }
            #[allow(clippy::cast_possible_truncation)]
            let code_u32 = next_u64 as u32;
            short_first_code[length] = code_u32;
            short_first_line[length] = total_coded;
            let count = short_length_count[length];
            if count != 0 {
                if next_u64 + u64::from(count - 1) >= limit {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::Oversubscribed,
                    ));
                }
                total_coded = total_coded
                    .checked_add(count)
                    .ok_or(HuffmanError::InvalidTable(
                        HuffmanTableError::Oversubscribed,
                    ))?;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(huffman_spec::first_prefix, 2);
                huffman_spec::short_canonical_step(
                    short_first_code@[(length - 1) as int],
                    short_length_count@[(length - 1) as int],
                    next_u64,
                );
                assert(short_length_count@[(length - 1) as int]
                    == huffman_spec::bucket(owned_lines@, (length - 1) as nat, line_count as nat).len());
                if length == 1 {
                    let z = WideCode { limbs: [0u64, 0u64, 0u64, 0u64] };
                    assert(ext_equal(z.limbs, [short_first_code@[0] as u64, 0u64, 0u64, 0u64]));
                    assert(huffman_spec::first_prefix(owned_lines@, 0) == Some(z));
                } else {
                    assert(huffman_spec::first_prefix(owned_lines@, (length - 1) as nat)
                        == Some(WideCode { limbs: [short_first_code@[(length - 1) as int] as u64, 0, 0, 0] }));
                }
                assert(huffman_spec::first_prefix(owned_lines@, length as nat)
                    == Some(WideCode { limbs: [code_u32 as u64, 0, 0, 0] }));
                if count != 0 {
                    huffman_spec::short_range_fits(code_u32, count, length);
                }
            }
            length += 1;
        }

        let total_usize = usize::try_from(total_coded)
            .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;
        let mut ordered_lines = Vec::new();
        try_reserve::<u32>(&mut ordered_lines, total_usize)?;
        ordered_lines.resize(total_usize, 0);

        let mut next_slot = short_first_line;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall|l: int| 1 <= l <= max_length implies
                (short_first_line@[l] as nat + short_length_count@[l] as nat) <= ordered_lines@.len()
                && ordered_lines@.subrange(short_first_line@[l] as int, next_slot@[l] as int)
                    == huffman_spec::bucket(owned_lines@, l as nat, 0)
            by {
                assert(short_first_line@[l] as nat
                    == huffman_spec::ordered_before(owned_lines@, l as nat).len());
                huffman_spec::ordered_before_disjoint(
                    owned_lines@, l as nat, (max_length + 1) as nat,
                );
                vstd::assert_seqs_equal!(
                    ordered_lines@.subrange(short_first_line@[l] as int, next_slot@[l] as int),
                    huffman_spec::bucket(owned_lines@, l as nat, 0)
                );
            };
        }
        let mut place_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                place_index <= line_count,
                line_count == owned_lines@.len(),
                1 <= max_prefix_len <= 32,
                max_length == max_prefix_len as usize,
                ordered_lines@.len() == total_coded as nat,
                total_coded as nat == huffman_spec::ordered_before(owned_lines@, max_prefix_len as nat + 1).len(),
                forall|i: int| 0 <= i < line_count ==> owned_lines@[i].prefix_len <= max_prefix_len,
                forall|l: int| 0 <= l <= 32 ==>
                    short_length_count@[l] == huffman_spec::bucket(owned_lines@, l as nat, line_count as nat).len(),
                forall|l: int| 1 <= l <= max_length ==>
                    short_first_line@[l] as nat == huffman_spec::ordered_before(owned_lines@, l as nat).len()
                    && (short_first_line@[l] as nat + short_length_count@[l] as nat) <= ordered_lines@.len()
                    && next_slot@[l] as nat == short_first_line@[l] as nat
                        + huffman_spec::bucket(owned_lines@, l as nat, place_index as nat).len()
                    && ordered_lines@.subrange(short_first_line@[l] as int, next_slot@[l] as int)
                        == huffman_spec::bucket(owned_lines@, l as nat, place_index as nat),
            decreases line_count - place_index,
        ))]
        while place_index < line_count {
            let plen = usize::from(owned_lines[place_index].prefix_len);
            if plen != 0 {
                let slot = usize::try_from(next_slot[plen])
                    .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;
                let idx_u32 = u32::try_from(place_index)
                    .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;
                #[cfg(verus_keep_ghost)]
                proof! {
                    huffman_spec::bucket_prefix_bound(
                        owned_lines@, plen as nat, place_index as nat, line_count as nat,
                    );
                    assert(slot < ordered_lines@.len());
                }
                #[cfg(verus_keep_ghost)]
                proof_decl! {
                    let ghost before_order = ordered_lines@;
                    let ghost before_next = next_slot@;
                }
                ordered_lines[slot] = idx_u32;
                next_slot[plen] += 1;
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(ordered_lines@ == before_order.update(slot as int, idx_u32));
                    assert forall|l: int| 1 <= l <= max_length implies
                        next_slot@[l] as nat == short_first_line@[l] as nat
                            + huffman_spec::bucket(owned_lines@, l as nat, (place_index + 1) as nat).len()
                        && ordered_lines@.subrange(short_first_line@[l] as int, next_slot@[l] as int)
                            == huffman_spec::bucket(owned_lines@, l as nat, (place_index + 1) as nat)
                    by {
                        reveal(huffman_spec::bucket);
                        if l == plen as int {
                            let s = short_first_line@[l] as int;
                            let e = before_next[l] as int;
                            assert(slot as int == e);
                            assert(next_slot@[l] as int == e + 1);
                            assert(s <= e < ordered_lines@.len());
                            vstd::assert_seqs_equal!(
                                ordered_lines@.subrange(s, e + 1),
                                before_order.subrange(s, e).push(idx_u32)
                            );
                        } else {
                            huffman_spec::ordered_before_disjoint(owned_lines@, l as nat, plen as nat);
                            huffman_spec::bucket_monotone(
                                owned_lines@, l as nat, place_index as nat, line_count as nat,
                            );
                            let s = short_first_line@[l] as int;
                            let e = before_next[l] as int;
                            assert(next_slot@[l] as int == e);
                            assert(e <= s + short_length_count@[l] as int);
                            assert((slot as int) < s || (slot as int) >= e);
                            vstd::assert_seqs_equal!(
                                ordered_lines@.subrange(s, e),
                                before_order.subrange(s, e)
                            );
                        }
                    };
                }
            } else {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert forall|l: int| 1 <= l <= max_length implies
                        #[trigger] huffman_spec::bucket(owned_lines@, l as nat, (place_index + 1) as nat)
                            == huffman_spec::bucket(owned_lines@, l as nat, place_index as nat)
                    by {
                        reveal(huffman_spec::bucket);
                    };
                }
            }
            place_index += 1;
        }

        let prefer_prefix_lookup = min_length >= MIN_FAST_PREFIX_LEN;
        let prefix_lookup = if prefer_prefix_lookup {
            check_table_memory(line_count, true, limits)?;
            build_prefix_lookup_short(
                &short_length_count,
                &short_first_code,
                &short_first_line,
                &ordered_lines,
            )?
        } else {
            Vec::new()
        };

        #[cfg(verus_keep_ghost)]
        proof! {
            let output = Self {
                lines: TableSlice::Owned(owned_lines),
                ordered_lines: TableSlice::Owned(ordered_lines),
                short_length_count,
                short_first_code,
                short_first_line,
                long_tables: None,
                prefix_lookup: TableSlice::Owned(prefix_lookup),
                prefer_prefix_lookup,
                max_prefix_len,
            };
            let counts_full = huffman_spec::table_length_counts(&output);
            let codes_full = huffman_spec::table_first_codes(&output);
            let starts_full = huffman_spec::table_first_lines(&output);
            let order_view = huffman_spec::slice_view(&output.ordered_lines);
            let lines_view = huffman_spec::slice_view(&output.lines);
            huffman_spec::linear_buckets_match_ordered_before(
                lines_view,
                short_first_line@,
                short_length_count@,
                order_view,
                max_prefix_len as nat + 1,
            );
            vstd::assert_seqs_equal!(
                order_view,
                order_view.subrange(0, huffman_spec::ordered_before(lines_view, max_prefix_len as nat + 1).len() as int)
            );
            huffman_lookup_spec::table_lookup_definition();
            if prefer_prefix_lookup {
                assert(min_length >= 8);
                assert(max_prefix_len >= 8);
                assert(huffman_lookup_spec::fast_shape(counts_full));
                assert(huffman_spec::slice_view(&output.prefix_lookup).len() == 256);
            }
            huffman_spec::canonical_assignment_definition();
            if huffman_spec::is_raw(lines) {
                assert(lines_view == huffman_spec::raw_lines(lines));
                assert(huffman_spec::histogram(lines_view, counts_full, line_count as nat)) by {
                    assert forall|l: int| 0 <= l < 256 implies
                        counts_full[l] == huffman_spec::bucket(lines_view, l as nat, line_count as nat).len()
                    by {
                        if l > 32 {
                            huffman_spec::bucket_absent_above_max(lines_view, l as nat, line_count as nat, max_prefix_len);
                        }
                    };
                };
                assert(counts_full[max_prefix_len as int] > 0);
                assert(forall|i: int| max_prefix_len < i < 256 ==> counts_full[i] == 0) by {
                    assert forall|i: int| max_prefix_len < i < 256 implies counts_full[i] == 0 by {
                        if i <= 32 {
                            huffman_spec::bucket_absent_above_max(lines_view, i as nat, line_count as nat, max_prefix_len);
                        }
                    };
                };
                assert(codes_full[0].limbs@ == seq![0u64, 0u64, 0u64, 0u64]);
                assert(huffman_spec::assignment_prefix(
                    lines_view, codes_full, starts_full, order_view, max_prefix_len as nat + 1,
                ));
                huffman_spec::assignment_complete(lines_view, &output);
            }
        }
        Ok(Self {
            lines: TableSlice::Owned(owned_lines),
            ordered_lines: TableSlice::Owned(ordered_lines),
            short_length_count,
            short_first_code,
            short_first_line,
            long_tables: None,
            prefix_lookup: TableSlice::Owned(prefix_lookup),
            prefer_prefix_lookup,
            max_prefix_len,
        })
    }

    /// Decodes the coded table structure described by T.88 B.2.
    ///
    /// `bytes` is exactly one code table segment payload, including at most
    /// seven zero padding bits after the final table line.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(table) => huffman_lookup_spec::table_lookup_valid(&table),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::cast_possible_wrap, clippy::too_many_lines)]
    pub(crate) fn from_encoded(bytes: &[u8], limits: &Limits) -> Result<Self, HuffmanError> {
        let byte_count = u64::try_from(bytes.len()).map_err(|_| MalformedError::IntegerOverflow)?;
        if byte_count > limits.max_stream_bytes {
            return Err(LimitExceeded {
                kind: LimitKind::StreamBytes,
                limit: limits.max_stream_bytes,
                actual: byte_count,
            }
            .into());
        }

        let mut header = ByteReader::new(bytes);
        let flags = header.read_u8()?;
        if flags & 0x80 != 0 {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::ReservedFlags(flags),
            ));
        }
        let low = header.read_u32()? as i32;
        let high = header.read_u32()? as i32;
        if low >= high {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidBounds { low, high },
            ));
        }

        let (has_oob, prefix_bits, range_bits) = encoded_table_fields(flags);
        if prefix_bits > 8 || range_bits > 8 {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidRangePartition,
            ));
        }
        let encoded_bytes = header.remaining_bytes();
        if encoded_bytes.len() > usize::MAX / 8 {
            return Err(MalformedError::IntegerOverflow.into());
        }
        let mut encoded_lines = bit_reader(encoded_bytes);
        let mut lines = Vec::new();
        let mut current = i64::from(low);
        let high_i64 = i64::from(high);

        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                low as int <= current,
                current <= high_i64,
                high_i64 == high as int,
                high_i64 <= i32::MAX,
                i32::MIN <= current,
                current <= i32::MAX,
                prefix_bits <= 8,
                range_bits <= 8,
                huffman_spec::reader_valid(&encoded_lines),
            decreases high_i64 - current,
        ))]
        while current < high_i64 {
            let prefix_len = read_u8_bits(&mut encoded_lines, prefix_bits)?;
            let range_len = read_u8_bits(&mut encoded_lines, range_bits)?;
            if range_len > 32 {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::InvalidRangeLength(range_len),
                ));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(range_len <= 32);
            }
            let range_low = i32::try_from(current).map_err(|_| {
                HuffmanError::InvalidTable(HuffmanTableError::InvalidRangePartition)
            })?;
            push_parsed_line(
                &mut lines,
                HuffmanLine::normal(range_low, prefix_len, range_len),
                limits,
            )?;
            let width = 1_i64 << range_len;
            let next = current
                .checked_add(width)
                .ok_or(MalformedError::IntegerOverflow)?;
            if next <= current {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::InvalidRangePartition,
                ));
            }
            if next >= high_i64 {
                break;
            }
            current = next;
        }

        let low_prefix = read_u8_bits(&mut encoded_lines, prefix_bits)?;
        push_parsed_line(
            &mut lines,
            HuffmanLine::lower(i64::from(low) - 1, low_prefix),
            limits,
        )?;
        let high_prefix = read_u8_bits(&mut encoded_lines, prefix_bits)?;
        push_parsed_line(&mut lines, HuffmanLine::upper(high, high_prefix), limits)?;
        if has_oob {
            let oob_prefix = read_u8_bits(&mut encoded_lines, prefix_bits)?;
            push_parsed_line(&mut lines, HuffmanLine::out_of_band(oob_prefix), limits)?;
        }

        let remaining_bits = padding_bit_count(header.remaining(), encoded_lines.bit_position)
            .ok_or(MalformedError::IntegerOverflow)?;
        if remaining_bits > 7 {
            return Err(HuffmanError::InvalidTable(HuffmanTableError::TrailingBits(
                remaining_bits as u64,
            )));
        }
        if remaining_bits != 0 {
            let padding =
                encoded_lines.read_bits(u8::try_from(remaining_bits).map_err(|_| {
                    HuffmanError::InvalidTable(HuffmanTableError::TrailingBits(
                        remaining_bits as u64,
                    ))
                })?)?;
            if padding != 0 {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::NonZeroPadding,
                ));
            }
        }

        Self::new(&lines, limits)
    }

    /// Returns a static precomputed Annex B standard table after enforcing
    /// `limits` memory checks identically to owned construction.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(table) => huffman_lookup_spec::table_lookup_valid(table),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::len_zero)]
    pub(crate) fn standard_static(
        table: StandardHuffmanTable,
        limits: &Limits,
    ) -> Result<&'static Self, HuffmanError> {
        let source = standard_lines(table);
        check_table_memory(source.len(), false, limits)?;
        let selected = standard_static::static_standard_table(table);
        if selected.prefer_prefix_lookup || selected.prefix_lookup.len() != 0 {
            return Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_lookup_spec::table_lookup_definition();
        }
        Ok(selected)
    }

    /// Constructs one of the standard tables in T.88 B.5.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(table) => huffman_lookup_spec::table_lookup_valid(&table),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn standard(
        table: StandardHuffmanTable,
        limits: &Limits,
    ) -> Result<Self, HuffmanError> {
        let static_table = Self::standard_static(table, limits)?;
        let lines = match &static_table.lines {
            TableSlice::Static(s) => TableSlice::Static(s),
            TableSlice::Owned(_) => TableSlice::Static(&[]),
        };
        let ordered_lines = match &static_table.ordered_lines {
            TableSlice::Static(s) => TableSlice::Static(s),
            TableSlice::Owned(_) => TableSlice::Static(&[]),
        };
        let output = Self {
            lines,
            ordered_lines,
            short_length_count: static_table.short_length_count,
            short_first_code: static_table.short_first_code,
            short_first_line: static_table.short_first_line,
            long_tables: None,
            prefix_lookup: TableSlice::Owned(Vec::new()),
            prefer_prefix_lookup: false,
            max_prefix_len: static_table.max_prefix_len,
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_lookup_spec::table_lookup_definition();
        }
        Ok(output)
    }
}
