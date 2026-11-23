//! Construction implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    bit_reader, build_prefix_lookup, canonical_first_code, canonical_range_fits,
    check_table_memory, encoded_table_fields, padding_bit_count, push_parsed_line, read_u8_bits,
    standard_lines, try_reserve, validate_lines, ByteReader, HuffmanError, HuffmanLine,
    HuffmanLines, HuffmanTable, HuffmanTableError, LimitExceeded, LimitKind, Limits,
    MalformedError, StandardHuffmanTable, WideCode, MAX_PREFIX_LEN, MIN_FAST_PREFIX_LEN,
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
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                source_index <= line_count,
                owned_lines@.len() == source_index,
                huffman_spec::is_raw(lines) ==>
                    owned_lines@ == huffman_spec::raw_lines(lines).take(source_index as int),
            decreases line_count - source_index,
        ))]
        while source_index < line_count {
            let line = lines.line(source_index).ok_or(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidLineOrder,
            ))?;
            owned_lines.push(line);
            source_index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if huffman_spec::is_raw(lines) {
                huffman_spec::full_prefix(huffman_spec::raw_lines(lines));
            }
        }

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
                huffman_spec::is_raw(lines) ==> line_count == huffman_spec::raw_lines(lines).len(),
                huffman_spec::is_raw(lines) ==>
                    huffman_spec::histogram(huffman_spec::raw_lines(lines), length_count@, count_index as nat),
            decreases line_count - count_index,
        ))]
        while count_index < line_count {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost old_counts = length_count@; }
            let line = lines.line(count_index).ok_or(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidLineOrder,
            ))?;
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
                huffman_spec::is_raw(lines) ==> line_count == huffman_spec::raw_lines(lines).len(),
                huffman_spec::is_raw(lines) ==>
                    huffman_spec::histogram(huffman_spec::raw_lines(lines), length_count@, line_count as nat),
                huffman_spec::is_raw(lines) ==>
                    huffman_spec::assignment_prefix(
                        huffman_spec::raw_lines(lines), first_code@, first_line@, ordered_lines@, length as nat,
                    ),
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
                        1 <= length <= max_prefix_len,
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
                    let line = lines.line(index).ok_or(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidLineOrder,
                    ))?;
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
        // Short canonical codes already beat lookahead setup. Restrict the
        // fast path to tables whose shortest code consumes the full prefix;
        // longer codes still use the exact canonical fallback.
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

        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_lookup_spec::table_lookup_definition();
            if prefer_prefix_lookup {
                assert(min_length >= 8);
                assert(max_prefix_len >= 8);
                assert(huffman_lookup_spec::fast_shape(length_count@));
                assert(prefix_lookup@.len() == 256);
                assert forall|prefix: int| 0 <= prefix < 256 implies
                    prefix_lookup@[prefix] == huffman_lookup_spec::fast_slot(
                        first_code@[8], length_count@[8], first_line@[8], ordered_lines@,
                        prefix as u8, length_count@[8],
                    ) by {
                }
            } else {
                assert(prefix_lookup@.len() == 0);
            }
            huffman_spec::canonical_assignment_definition();
            if huffman_spec::is_raw(lines) {
                assert(owned_lines@ == huffman_spec::raw_lines(lines));
                assert(huffman_spec::histogram(huffman_spec::raw_lines(lines), length_count@, line_count as nat));
                assert(length_count@[max_prefix_len as int] > 0);
                assert(forall|i: int| max_prefix_len < i < 256 ==> length_count@[i] == 0);
                assert(huffman_spec::assignment_prefix(
                    huffman_spec::raw_lines(lines), first_code@, first_line@, ordered_lines@,
                    max_prefix_len as nat + 1,
                ));
                let output = Self {
                    lines: owned_lines,
                    ordered_lines,
                    length_count,
                    first_code,
                    first_line,
                    prefix_lookup,
                    prefer_prefix_lookup,
                    max_prefix_len,
                };
                huffman_spec::assignment_complete(huffman_spec::raw_lines(lines), &output);
            }
        }
        Ok(Self {
            lines: owned_lines,
            ordered_lines,
            length_count,
            first_code,
            first_line,
            prefix_lookup,
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

    /// Constructs one of the standard tables in T.88 B.5.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(built) => huffman_lookup_spec::table_lookup_valid(&built)
                && (table == StandardHuffmanTable::A ==>
                    huffman_spec::canonical_assignment(huffman_spec::standard_a_lines(), &built)),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn standard(
        table: StandardHuffmanTable,
        limits: &Limits,
    ) -> Result<Self, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { huffman_spec::standard_a_lines_definition(); }
        Self::from_line_source(HuffmanLines::Standard(standard_lines(table)), limits)
    }
}
