//! Immutable codec descriptors for all fifteen Annex B tables.

use super::{
    decode_line, BitReader, DecodeError, HuffmanError, HuffmanTable, HuffmanTableError,
    HuffmanValue, LimitExceeded, LimitKind, Limits, MalformedError, StandardHuffmanTable, WideCode,
};
#[cfg(verus_keep_ghost)]
use super::{huffman_lookup_spec, huffman_spec};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../../specs/huffman_standard.rs"]
pub(crate) mod standard_spec;

mod data;

/// Compact, read-only canonical metadata. Custom/owned tables retain their
/// original representation and decoder; only codec standard-table slots use this.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct StandardDescriptor {
    lines: super::StandardLines,
    first_code: [u16; 10],
    length_count: [u8; 10],
    first_line: [u8; 10],
    ordered_lines: [u8; 22],
    max_prefix_len: u8,
}

impl StandardDescriptor {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(descriptor) => *descriptor == *standard_spec::selected(table)
                && standard_spec::canonical(descriptor),
            Err(error) => error == HuffmanError::Decode(DecodeError::LimitExceeded(
                LimitExceeded {
                    kind: LimitKind::Symbols,
                    limit: limits.max_symbols as u64,
                    actual: standard_spec::line_count(table) as u64,
                },
            )),
        },
        out.is_ok() == (standard_spec::line_count(table) <= limits.max_symbols),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(
        table: StandardHuffmanTable,
        limits: &Limits,
    ) -> Result<&'static Self, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { standard_spec::line_count_definition(table); }
        let descriptor = data::select(table);
        let count = descriptor.lines.len();
        if u64::from(limits.max_symbols) < count as u64 {
            return Err(LimitExceeded {
                kind: LimitKind::Symbols,
                limit: u64::from(limits.max_symbols),
                actual: count as u64,
            }
            .into());
        }
        Ok(descriptor)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == standard_spec::decode_state(self, old(reader).bytes@, old(reader).bit_position).0,
            final(reader).bit_position
                == standard_spec::decode_state(self, old(reader).bytes@, old(reader).bit_position).1,
    ))]
    pub(crate) fn decode(&self, reader: &mut BitReader<'_>) -> Result<HuffmanValue, HuffmanError> {
        let mut code = WideCode { limbs: [0; 4] };
        let mut length = 1_usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            standard_spec::decode_state_definition(self, reader.bytes@, reader.bit_position);
            reveal_with_fuel(standard_spec::run, 2);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                huffman_spec::reader_valid(reader),
                reader.bytes@ == old(reader).bytes@,
                1 <= length <= 10,
                standard_spec::decode_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ) == standard_spec::run(self, reader.bytes@, reader.bit_position, length, code),
            decreases 10 - length,
        ))]
        while length <= usize::from(self.max_prefix_len) && length < 10 {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(standard_spec::run, 2);
            }
            #[cfg(verus_keep_ghost)]
            let start = reader.bit_position;
            let bit = reader.read_bits(1)?;
            #[cfg(verus_keep_ghost)]
            production_huffman_one_bit!(reader, start, bit);
            code = code
                .append_bit(bit)
                .ok_or(MalformedError::IntegerOverflow)?;
            let count = u32::from(self.length_count[length]);
            let first = WideCode {
                limbs: [u64::from(self.first_code[length]), 0, 0, 0],
            };
            if let Some(distance) = code.small_distance_from(first, count) {
                let ordered = usize::from(self.first_line[length])
                    .checked_add(distance as usize)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let index = *self
                    .ordered_lines
                    .get(ordered)
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                let line = self
                    .lines
                    .line(usize::from(index))
                    .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
                return decode_line(line, reader);
            }
            length += 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(standard_spec::run, 2);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(standard_spec::run, 2);
        }
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(reader),
        ensures out.is_ok(),
            match out {
                Ok(padding) => padding == standard_spec::zero_padding(self, reader),
                _ => true,
            },
    ))]
    pub(crate) fn zero_padding_before_prefix(
        &self,
        reader: &BitReader<'_>,
    ) -> Result<bool, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { standard_spec::zero_padding_definition(self, reader); }
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
                forall|done: int| 1 <= done < length ==> self.length_count@[done] == 0,
            decreases remaining + 1 - length,
        ))]
        while length <= remaining {
            if self.length_count[length] != 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    standard_spec::zero_padding_definition(self, reader);
                    assert(!standard_spec::zero_padding(self, reader));
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
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            final(reader).bit_position
                == standard_spec::decode_state(self, old(reader).bytes@, old(reader).bit_position).1,
            out == Ok(None) ==> (
                standard_spec::zero_padding(self, old(reader))
                && final(reader).bit_position == old(reader).bytes@.len() * 8
            ),
            match out {
                Ok(Some(value)) => standard_spec::decode_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Ok(value),
                Err(error) => standard_spec::decode_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Err(error),
                _ => true,
            },
    ))]
    pub(crate) fn decode_final_strip_value(
        &self,
        reader: &mut BitReader<'_>,
    ) -> Result<Option<HuffmanValue>, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { standard_spec::zero_padding_definition(self, reader); }
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
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum CodecHuffmanTable<'a> {
    Standard(&'static StandardDescriptor),
    General(&'a HuffmanTable),
}

impl CodecHuffmanTable<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == standard_spec::codec_state(self, old(reader).bytes@, old(reader).bit_position).0,
            final(reader).bit_position
                == standard_spec::codec_state(self, old(reader).bytes@, old(reader).bit_position).1,
    ))]
    #[inline(always)]
    #[allow(clippy::inline_always)]
    pub(crate) fn decode(self, reader: &mut BitReader<'_>) -> Result<HuffmanValue, HuffmanError> {
        match self {
            Self::Standard(table) => table.decode(reader),
            Self::General(table) => table.decode(reader),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == Ok(None) ==> (
                old(reader).bit_position <= final(reader).bit_position
                && final(reader).bit_position - old(reader).bit_position < 8
                && final(reader).bit_position == old(reader).bytes@.len() * 8
            ),
            final(reader).bit_position
                == standard_spec::codec_state(self, old(reader).bytes@, old(reader).bit_position).1,
            match out {
                Ok(Some(value)) => standard_spec::codec_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Ok(value),
                Err(error) => standard_spec::codec_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Err(error),
                _ => true,
            },
    ))]
    #[inline(always)]
    #[allow(clippy::inline_always)]
    pub(crate) fn decode_final_strip_value(
        self,
        reader: &mut BitReader<'_>,
    ) -> Result<Option<HuffmanValue>, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(huffman_spec::zero_padding_before_prefix);
            match self {
                Self::Standard(table) => standard_spec::zero_padding_definition(table, reader),
                _ => {},
            }
        }
        match self {
            Self::Standard(table) => table.decode_final_strip_value(reader),
            Self::General(table) => table.decode_final_strip_value(reader),
        }
    }
}

#[cfg(test)]
impl<'a> CodecHuffmanTable<'a> {
    pub(crate) fn general(self) -> Option<&'a HuffmanTable> {
        match self {
            Self::General(table) => Some(table),
            Self::Standard(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
