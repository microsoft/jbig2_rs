//! Decoding implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    BitReader, DecodeError, HuffmanError, HuffmanLine, HuffmanTable, HuffmanTableError,
    HuffmanValue, LineKind, MalformedError, WideCode, PREFIX_LOOKUP_BITS, SHORT_PREFIX_MAX,
};

impl HuffmanTable {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(reader),
        ensures out.is_ok(), match out {
            Ok(allowed) => allowed == huffman_spec::zero_padding_before_prefix(self, reader),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn zero_padding_before_prefix(
        &self,
        reader: &BitReader<'_>,
    ) -> Result<bool, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(huffman_spec::zero_padding_before_prefix);
            huffman_spec::prefix_count_definition(self);
        }
        let total = reader
            .bytes
            .len()
            .checked_mul(8)
            .ok_or(MalformedError::IntegerOverflow)?;
        let remaining = total
            .checked_sub(reader.bit_position)
            .ok_or(MalformedError::IntegerOverflow)?;
        if remaining >= 8 {
            return Ok(false);
        }
        let mut length = 1_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant 1 <= length <= remaining + 1, remaining < 8,
                huffman_spec::reader_valid(reader),
                remaining as int == reader.bytes@.len() * 8 - reader.bit_position,
                forall|done: int| 1 <= done < length ==>
                    huffman_spec::table_length_counts(self)[done] == 0,
            decreases remaining + 1 - length,
        ))]
        while length <= remaining {
            if self.length_count_at(length) != 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    huffman_spec::prefix_count_definition(self);
                    reveal(huffman_spec::zero_padding_before_prefix);
                    assert(huffman_spec::prefix_count(self, length as int) != 0);
                    assert(!huffman_spec::zero_padding_before_prefix(self, reader));
                }
                return Ok(false);
            }
            length += 1;
        }
        let mut padding = *reader;
        let count = u8::try_from(remaining).map_err(|_| MalformedError::IntegerOverflow)?;
        Ok(padding.read_bits(count)? == 0)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures
            huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == Ok(None) ==> (
                huffman_spec::zero_padding_before_prefix(self, old(reader))
                && final(reader).bit_position == old(reader).bytes@.len() * 8
            ),
            match out {
                Ok(Some(value)) => huffman_spec::decode_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Ok(value),
                Err(error) => huffman_spec::decode_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Err(error),
                _ => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn decode_final_strip_value(
        &self,
        reader: &mut BitReader<'_>,
    ) -> Result<Option<HuffmanValue>, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(huffman_spec::zero_padding_before_prefix); }
        let padding = self.zero_padding_before_prefix(reader)?;
        match self.decode(reader) {
            Ok(value) => Ok(Some(value)),
            Err(HuffmanError::Decode(DecodeError::Malformed(MalformedError::UnexpectedEof {
                offset,
                needed,
                remaining,
            }))) if padding
                && needed == 1
                && remaining == 0
                && offset == reader.bytes.len() as u64 * 8
                && reader.bit_position == reader.bytes.len() * 8 =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == huffman_spec::table_has_oob(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn has_oob(&self) -> bool {
        let lines_slice = self.lines.as_slice();
        let mut index = 0;
        let mut found = false;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                index <= lines_slice@.len(),
                lines_slice@ == huffman_spec::slice_view(&self.lines),
                found == exists|line_index: int|
                    0 <= line_index < index
                    && lines_slice@[line_index].kind == LineKind::OutOfBand,
            decreases lines_slice@.len() - index,
        ))]
        while index < lines_slice.len() {
            match lines_slice[index].kind {
                LineKind::OutOfBand => found = true,
                LineKind::Normal | LineKind::Lower | LineKind::Upper => {}
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        production_huffman_has_oob!(self);
        found
    }

    /// Decodes one integer or OOB value according to T.88 B.4.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures
            huffman_spec::reader_valid(final(reader)),
            huffman_spec::cached_decode_outcome(self, old(reader), final(reader), out),
            final(reader).bytes@ == old(reader).bytes@,
            out == huffman_spec::decode_state(
                self, old(reader).bytes@, old(reader).bit_position,
            ).0,
            final(reader).bit_position == huffman_spec::decode_state(
                self, old(reader).bytes@, old(reader).bit_position,
            ).1,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn decode(&self, reader: &mut BitReader<'_>) -> Result<HuffmanValue, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_spec::cached_decode_outcome_definition();
            huffman_spec::decode_state_definition(
                self, reader.bytes@, reader.bit_position,
            );
        }
        if self.prefer_prefix_lookup {
            if let Some(prefix) = peek_prefix(reader) {
                let entry = match self.prefix_lookup.get(usize::from(prefix)) {
                    Some(entry) => *entry,
                    None => 0,
                };
                if entry != 0 {
                    let line_index = u32::from(entry)
                        .checked_sub(1)
                        .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                    let line = self
                        .lines
                        .get(line_index as usize)
                        .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                    let length = line.prefix_len;
                    if length > PREFIX_LOOKUP_BITS {
                        return Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode));
                    }
                    reader.bit_position = reader
                        .bit_position
                        .checked_add(usize::from(length))
                        .ok_or(MalformedError::IntegerOverflow)?;
                    return decode_line(*line, reader);
                }
            }
        }
        self.decode_fallback(reader)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures
            huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == huffman_spec::fallback_run(
                self, old(reader).bytes@, old(reader).bit_position,
                1, WideCode { limbs: [0u64, 0, 0, 0] },
            ).0,
            final(reader).bit_position == huffman_spec::fallback_run(
                self, old(reader).bytes@, old(reader).bit_position,
                1, WideCode { limbs: [0u64, 0, 0, 0] },
            ).1,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::manual_range_contains
    )]
    pub(super) fn decode_fallback(
        &self,
        reader: &mut BitReader<'_>,
    ) -> Result<HuffmanValue, HuffmanError> {
        let max_len = usize::from(self.max_prefix_len);
        let start_pos = reader.bit_position;
        let byte_idx = start_pos / 8;
        if self.long_tables.is_none()
            && max_len >= 1
            && max_len <= SHORT_PREFIX_MAX
            && reader.bytes.len() >= byte_idx + 8
        {
            let b = reader.bytes;
            let word = (u64::from(b[byte_idx]) << 56)
                | (u64::from(b[byte_idx + 1]) << 48)
                | (u64::from(b[byte_idx + 2]) << 40)
                | (u64::from(b[byte_idx + 3]) << 32)
                | (u64::from(b[byte_idx + 4]) << 24)
                | (u64::from(b[byte_idx + 5]) << 16)
                | (u64::from(b[byte_idx + 6]) << 8)
                | u64::from(b[byte_idx + 7]);
            let bit_off = (start_pos % 8) as u32;
            let mut code32 = 0_u32;
            let mut length = 1_usize;
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(huffman_spec::reader_valid);
                assert((1u64 << 0u64) == 1u64) by (bit_vector);
                huffman_spec::fallback_run_definition(
                    self, reader.bytes@, start_pos, 1, WideCode { limbs: [0u64, 0, 0, 0] },
                );
            }
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    huffman_spec::reader_valid(reader),
                    huffman_spec::reader_valid(old(reader)),
                    reader.bytes@ == old(reader).bytes@,
                    reader.bit_position == start_pos,
                    start_pos == old(reader).bit_position,
                    byte_idx == start_pos / 8,
                    bit_off == (start_pos % 8) as u32,
                    byte_idx + 8 <= reader.bytes@.len(),
                    word == ((((reader.bytes@[byte_idx as int] as u64) << 56u64)
                        | ((reader.bytes@[(byte_idx + 1) as int] as u64) << 48u64)
                        | ((reader.bytes@[(byte_idx + 2) as int] as u64) << 40u64)
                        | ((reader.bytes@[(byte_idx + 3) as int] as u64) << 32u64)
                        | ((reader.bytes@[(byte_idx + 4) as int] as u64) << 24u64)
                        | ((reader.bytes@[(byte_idx + 5) as int] as u64) << 16u64)
                        | ((reader.bytes@[(byte_idx + 6) as int] as u64) << 8u64)
                        | (reader.bytes@[(byte_idx + 7) as int] as u64))),
                    1 <= length <= max_len + 1,
                    max_len == self.max_prefix_len as usize,
                    1 <= max_len <= 32,
                    self.long_tables.is_none(),
                    length <= 32 ==> (code32 as u64) < (1u64 << ((length - 1) as u64)),
                    huffman_spec::fallback_run(
                        self, old(reader).bytes@, old(reader).bit_position,
                        1, WideCode { limbs: [0u64, 0, 0, 0] },
                    ) == huffman_spec::fallback_run(
                        self, reader.bytes@, (start_pos + length - 1) as usize,
                        length, WideCode { limbs: [code32 as u64, 0, 0, 0] },
                    ),
                decreases max_len + 1 - length,
            ))]
            while length <= max_len {
                #[allow(unused_variables)]
                let cur_pos = start_pos + (length - 1);
                let k = bit_off + (length - 1) as u32;
                let bit = ((word >> (63 - k)) & 1) as u32;
                #[cfg(verus_keep_ghost)]
                let prev_code = code32;
                code32 = (code32 << 1) | bit;
                #[cfg(verus_keep_ghost)]
                proof! {
                    reveal(huffman_spec::reader_valid);
                    huffman_spec::fallback_run_definition(
                        self, reader.bytes@, cur_pos, length,
                        WideCode { limbs: [prev_code as u64, 0, 0, 0] },
                    );
                    crate::reader_properties::u64_word_bit_at(
                        reader.bytes@, byte_idx as nat, k, word,
                    );
                    reveal_with_fuel(crate::spec::readers::bits_value, 2);
                    assert(bit as nat == crate::spec::readers::bits_value(
                        reader.bytes@, cur_pos as nat, 1,
                    ));
                    assert(prev_code <= 0x7fff_ffffu32 && bit <= 1u32) by (bit_vector)
                        requires
                            1usize <= length <= 32usize,
                            (prev_code as u64) < (1u64 << ((length - 1) as u64)),
                            bit == (((word >> sub(63u64, k as u64)) & 1u64) as u32);
                    huffman_spec::short_append_bit_matches(prev_code, bit);
                    if length < 32 {
                        assert((code32 as u64) < (1u64 << (length as u64))) by (bit_vector)
                            requires
                                1usize <= length < 32usize,
                                (prev_code as u64) < (1u64 << ((length - 1) as u64)),
                                bit <= 1u32,
                                code32 == ((prev_code << 1u32) | bit);
                    }
                    huffman_spec::short_distance_matches(
                        code32,
                        self.short_first_code@[length as int],
                        self.short_length_count@[length as int],
                    );
                }
                let count = self.short_length_count[length];
                let first = self.short_first_code[length];
                if count != 0 && code32 >= first && code32 - first < count {
                    let distance = code32 - first;
                    reader.bit_position = start_pos + length;
                    let fast_ordered_index = self.short_first_line[length]
                        .checked_add(distance)
                        .ok_or(MalformedError::IntegerOverflow)?
                        as usize;
                    let line_index = *self
                        .ordered_lines
                        .get(fast_ordered_index)
                        .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                    let line = self
                        .lines
                        .get(line_index as usize)
                        .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                    return decode_line(*line, reader);
                }
                length += 1;
            }
            reader.bit_position = start_pos + max_len;
            #[cfg(verus_keep_ghost)]
            proof! {
                huffman_spec::fallback_run_definition(
                    self, reader.bytes@, reader.bit_position, length,
                    WideCode { limbs: [code32 as u64, 0, 0, 0] },
                );
            }
            return Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode));
        }

        let mut code = WideCode { limbs: [0; 4] };
        let mut length = 1_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(code.limbs@ == seq![0u64, 0, 0, 0]);
            reveal(huffman_spec::reader_valid);
            huffman_spec::fallback_run_definition(
                self, reader.bytes@, reader.bit_position, length, code,
            );
            huffman_spec::fallback_run_definition(
                self, reader.bytes@, reader.bit_position, length,
                WideCode { limbs: [0u64, 0, 0, 0] },
            );
            reveal(huffman_spec::append_bit);
            reveal(huffman_spec::checked_shift_left_one);
            reveal(huffman_spec::checked_add_small);
            reveal(huffman_spec::add_limb);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                huffman_spec::reader_valid(reader),
                huffman_spec::reader_valid(old(reader)),
                1 <= length,
                length <= self.max_prefix_len as usize + 1,
                reader.bytes@ == old(reader).bytes@,
                huffman_spec::fallback_run(
                    self, old(reader).bytes@, old(reader).bit_position,
                    1, WideCode { limbs: [0u64, 0, 0, 0] },
                ) == huffman_spec::fallback_run(
                    self, reader.bytes@, reader.bit_position, length, code,
                ),
            decreases self.max_prefix_len as usize + 1 - length,
        ))]
        while length <= self.max_prefix_len as usize {
            #[cfg(verus_keep_ghost)]
            let start = reader.bit_position;
            #[cfg(verus_keep_ghost)]
            proof! {
                huffman_spec::fallback_run_definition(
                    self, reader.bytes@, reader.bit_position, length, code,
                );
            }
            let bit = match reader.read_bits(1) {
                Ok(bit) => bit,
                Err(error) => {
                    return Err(HuffmanError::Decode(DecodeError::Malformed(error)));
                }
            };
            #[cfg(verus_keep_ghost)]
            production_huffman_one_bit!(reader, start, bit);
            code = code
                .append_bit(bit)
                .ok_or(MalformedError::IntegerOverflow)?;
            let count = self.length_count_at(length);
            if let Some(distance) = code.small_distance_from(self.first_code_at(length), count) {
                let ordered_index =
                    self.first_line_at(length)
                        .checked_add(distance)
                        .ok_or(MalformedError::IntegerOverflow)? as usize;
                let line_index = *self
                    .ordered_lines
                    .get(ordered_index)
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                let line = self
                    .lines
                    .get(line_index as usize)
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                return decode_line(*line, reader);
            }
            length = next_prefix_length(length, usize::from(self.max_prefix_len));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_spec::fallback_run_definition(
                self, reader.bytes@, reader.bit_position, length, code,
            );
        }
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))
    }

    #[cfg(test)]
    pub(super) fn decode_canonical(
        &self,
        reader: &mut BitReader<'_>,
    ) -> Result<HuffmanValue, HuffmanError> {
        let mut code = WideCode { limbs: [0; 4] };
        for length in 1..=usize::from(self.max_prefix_len) {
            let bit = reader.read_bits(1)?;
            code = code
                .append_bit(bit)
                .ok_or(MalformedError::IntegerOverflow)?;
            let count = self.length_count_at(length);
            if let Some(distance) = code.small_distance_from(self.first_code_at(length), count) {
                let ordered_index =
                    self.first_line_at(length)
                        .checked_add(distance)
                        .ok_or(MalformedError::IntegerOverflow)? as usize;
                let line_index = *self
                    .ordered_lines
                    .get(ordered_index)
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                let line = self
                    .lines
                    .get(line_index as usize)
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                return decode_line(*line, reader);
            }
        }
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))
    }

    #[cfg(test)]
    pub(super) fn code_for_line(&self, line_index: usize) -> Option<(WideCode, u8)> {
        let line = self.lines.get(line_index)?;
        let length = usize::from(line.prefix_len);
        if length == 0 {
            return None;
        }

        let first = usize::try_from(self.first_line_at(length)).ok()?;
        let count = usize::try_from(self.length_count_at(length)).ok()?;
        let within_length = self
            .ordered_lines
            .as_slice()
            .get(first..first.checked_add(count)?)?
            .iter()
            .position(|&candidate| usize::try_from(candidate).ok() == Some(line_index))?;
        let code = self
            .first_code_at(length)
            .checked_add_small(u32::try_from(within_length).ok()?)?;
        Some((code, line.prefix_len))
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires huffman_spec::reader_valid(reader),
    ensures
        match out {
            Some(_) =>
                reader.bit_position + PREFIX_LOOKUP_BITS as usize
                    <= reader.bytes@.len() * 8,
            None => true,
        },
        out == if reader.bit_position + PREFIX_LOOKUP_BITS as nat
            > reader.bytes@.len() * 8 {
            None
        } else {
            let index = reader.bit_position / 8;
            let offset = reader.bit_position % 8;
            if offset == 0 {
                Some(reader.bytes@[index as int])
            } else {
                let first = reader.bytes@[index as int] as u16;
                let second = reader.bytes@[(index + 1) as int] as u16;
                Some(((((first << 8) | second) >> ((8 - offset) as usize)) & 0xff) as u8)
            }
        },
))]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn peek_prefix(reader: &BitReader<'_>) -> Option<u8> {
    let total_bits = reader.bytes.len().checked_mul(8)?;
    let remaining = total_bits.checked_sub(reader.bit_position)?;
    if remaining < usize::from(PREFIX_LOOKUP_BITS) {
        return None;
    }
    let byte_index = reader.bit_position / 8;
    let bit_offset = reader.bit_position % 8;
    let first = u16::from(*reader.bytes.get(byte_index)?);
    if bit_offset == 0 {
        return Some(first as u8);
    }
    let second_index = byte_index.checked_add(1)?;
    let second = u16::from(*reader.bytes.get(second_index)?);
    let combined = (first << 8) | second;
    let shift = 8_usize.checked_sub(bit_offset)?;
    Some(((combined >> shift) & 0xff) as u8)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires length <= _maximum, _maximum < usize::MAX,
    ensures
        out == length + 1,
        _maximum + 1 - out < _maximum + 1 - length,
))]
pub(super) fn next_prefix_length(length: usize, _maximum: usize) -> usize {
    length + 1
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == huffman_spec::canonical_first_code(previous, count),
))]
pub(super) fn canonical_first_code(previous: WideCode, count: u32) -> Option<WideCode> {
    match previous.checked_add_small(count) {
        Some(sum) => sum.checked_shift_left_one(),
        None => None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires count > 0,
    ensures out == huffman_spec::canonical_range_fits(first, count, length),
))]
pub(super) fn canonical_range_fits(first: WideCode, count: u32, length: usize) -> bool {
    match first.checked_add_small(count - 1) {
        Some(last) => last.fits_in_bits(length),
        None => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires huffman_spec::reader_valid(old(reader)),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        huffman_spec::reader_valid(final(reader)),
        out == huffman_spec::line_state(
            line, old(reader).bytes@, old(reader).bit_position,
        ).0,
        final(reader).bit_position == huffman_spec::line_state(
            line, old(reader).bytes@, old(reader).bit_position,
        ).1,
        match out {
            Ok(HuffmanValue::OutOfBand) =>
                line.kind == LineKind::OutOfBand
                && final(reader).bit_position == old(reader).bit_position,
            Ok(HuffmanValue::Integer(value)) =>
                line.kind != LineKind::OutOfBand
                && final(reader).bit_position
                    == old(reader).bit_position + line.range_len as usize
                && huffman_spec::decoded_line_value(
                    line,
                    crate::spec::readers::bits_value(
                        old(reader).bytes@,
                        old(reader).bit_position as nat,
                        line.range_len as nat,
                    ),
                    value,
                ),
            Err(_) =>
                final(reader).bit_position == old(reader).bit_position
                || (
                    line.kind != LineKind::OutOfBand
                    && line.range_len <= 32
                    && final(reader).bit_position
                        == old(reader).bit_position + line.range_len as usize
                ),
        },
        match line.kind {
            LineKind::OutOfBand =>
                out == Ok(HuffmanValue::OutOfBand)
                    && final(reader).bit_position == old(reader).bit_position,
            _ => match crate::spec::readers::bit_read_error(
                old(reader).bytes@, old(reader).bit_position, line.range_len,
            ) {
                Some(error) =>
                    out == Err(HuffmanError::Decode(DecodeError::Malformed(error)))
                        && final(reader).bit_position == old(reader).bit_position,
                None => {
                    let offset = crate::spec::readers::bits_value(
                        old(reader).bytes@, old(reader).bit_position as nat,
                        line.range_len as nat,
                    ) as int;
                    let candidate = if line.kind == LineKind::Lower {
                        line.range_low as int - offset
                    } else {
                        line.range_low as int + offset
                    };
                    &&& final(reader).bit_position
                        == old(reader).bit_position + line.range_len as nat
                    &&& out == if i32::MIN as int <= candidate <= i32::MAX as int {
                        Ok(HuffmanValue::Integer(candidate as i32))
                    } else {
                        Err(HuffmanError::Decode(DecodeError::Malformed(
                            MalformedError::IntegerOverflow,
                        )))
                    }
                },
            },
        },
))]
#[allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::manual_let_else
)]
pub(super) fn decode_line(
    line: HuffmanLine,
    reader: &mut BitReader<'_>,
) -> Result<HuffmanValue, HuffmanError> {
    match line.kind {
        LineKind::OutOfBand => return Ok(HuffmanValue::OutOfBand),
        LineKind::Normal | LineKind::Lower | LineKind::Upper => {}
    }
    let offset = reader.read_bits(line.range_len)? as i64;
    let candidate = match line.kind {
        LineKind::Lower => line.range_low.checked_sub(offset),
        LineKind::Normal | LineKind::Upper => line.range_low.checked_add(offset),
        LineKind::OutOfBand => None,
    };
    let value = match candidate {
        Some(value) => value,
        None => {
            return Err(HuffmanError::Decode(DecodeError::Malformed(
                MalformedError::IntegerOverflow,
            )));
        }
    };
    if value < i32::MIN as i64 || value > i32::MAX as i64 {
        return Err(HuffmanError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow,
        )));
    }
    Ok(HuffmanValue::Integer(value as i32))
}
