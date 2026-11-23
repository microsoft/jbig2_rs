//! Private, allocation-free codec view of the four immutable standard-A lines.

use super::{
    decode_line, BitReader, HuffmanError, HuffmanTable, HuffmanValue, LimitExceeded, LimitKind,
    Limits, MalformedError, TABLE_A,
};
#[cfg(verus_keep_ghost)]
use super::{huffman_lookup_spec, huffman_shared, huffman_spec, DecodeError, WideCode};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../../specs/huffman_standard_a.rs"]
pub(crate) mod standard_a_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../../proofs/shared/huffman_standard_a.rs"]
mod agreement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct StandardADescriptor {
    _sealed: (),
}

impl StandardADescriptor {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.is_ok() == (limits.max_symbols >= 4),
            match out {
                Err(error) => error == HuffmanError::Decode(DecodeError::LimitExceeded(
                    LimitExceeded {
                        kind: LimitKind::Symbols,
                        limit: limits.max_symbols as u64,
                        actual: 4,
                    },
                )),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(limits: &Limits) -> Result<Self, HuffmanError> {
        if limits.max_symbols < 4 {
            return Err(LimitExceeded {
                kind: LimitKind::Symbols,
                limit: u64::from(limits.max_symbols),
                actual: 4,
            }
            .into());
        }
        // No owned allocation is made. The normative four-line constant is shared.
        Ok(Self { _sealed: () })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == standard_a_spec::decode_state(
                old(reader).bytes@, old(reader).bit_position,
            ).0,
            final(reader).bit_position == standard_a_spec::decode_state(
                old(reader).bytes@, old(reader).bit_position,
            ).1,
    ))]
    pub(crate) fn decode(self, reader: &mut BitReader<'_>) -> Result<HuffmanValue, HuffmanError> {
        let Self { _sealed: () } = self;
        #[cfg(verus_keep_ghost)]
        proof! {
            standard_a_spec::decode_state_definition(reader.bytes@, reader.bit_position);
        }
        if reader.read_bits(1)? == 0 {
            return decode_line(TABLE_A[0], reader);
        }
        if reader.read_bits(1)? == 0 {
            return decode_line(TABLE_A[1], reader);
        }
        if reader.read_bits(1)? == 0 {
            decode_line(TABLE_A[2], reader)
        } else {
            decode_line(TABLE_A[3], reader)
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == Ok(None) ==> (
                final(reader).bit_position == old(reader).bit_position
                && final(reader).bit_position == old(reader).bytes@.len() * 8
            ),
            final(reader).bit_position == standard_a_spec::decode_state(
                old(reader).bytes@, old(reader).bit_position,
            ).1,
            match out {
                Ok(Some(value)) => standard_a_spec::decode_state(
                    old(reader).bytes@, old(reader).bit_position,
                ).0 == Ok(value),
                Err(error) => standard_a_spec::decode_state(
                    old(reader).bytes@, old(reader).bit_position,
                ).0 == Err(error),
                _ => true,
            },
    ))]
    pub(crate) fn decode_final_strip_value(
        self,
        reader: &mut BitReader<'_>,
    ) -> Result<Option<HuffmanValue>, HuffmanError> {
        let total = reader
            .bytes
            .len()
            .checked_mul(8)
            .ok_or(MalformedError::IntegerOverflow)?;
        let remaining = total
            .checked_sub(reader.bit_position)
            .ok_or(MalformedError::IntegerOverflow)?;
        if remaining == 0 {
            #[cfg(verus_keep_ghost)]
            proof! {
                standard_a_spec::decode_state_definition(reader.bytes@, reader.bit_position);
            }
            return Ok(None);
        }
        let value = self.decode(reader)?;
        Ok(Some(value))
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum CodecHuffmanTable<'a> {
    StandardA(StandardADescriptor),
    General(&'a HuffmanTable),
}

impl CodecHuffmanTable<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires huffman_spec::reader_valid(old(reader)),
        ensures huffman_spec::reader_valid(final(reader)),
            final(reader).bytes@ == old(reader).bytes@,
            out == standard_a_spec::codec_state(
                self, old(reader).bytes@, old(reader).bit_position,
            ).0,
            final(reader).bit_position == standard_a_spec::codec_state(
                self, old(reader).bytes@, old(reader).bit_position,
            ).1,
    ))]
    pub(crate) fn decode(self, reader: &mut BitReader<'_>) -> Result<HuffmanValue, HuffmanError> {
        match self {
            Self::StandardA(table) => table.decode(reader),
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
            final(reader).bit_position == standard_a_spec::codec_state(
                self, old(reader).bytes@, old(reader).bit_position,
            ).1,
            match out {
                Ok(Some(value)) => standard_a_spec::codec_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Ok(value),
                Err(error) => standard_a_spec::codec_state(
                    self, old(reader).bytes@, old(reader).bit_position,
                ).0 == Err(error),
                _ => true,
            },
    ))]
    pub(crate) fn decode_final_strip_value(
        self,
        reader: &mut BitReader<'_>,
    ) -> Result<Option<HuffmanValue>, HuffmanError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal(huffman_spec::zero_padding_before_prefix); }
        match self {
            Self::StandardA(table) => table.decode_final_strip_value(reader),
            Self::General(table) => table.decode_final_strip_value(reader),
        }
    }
}

#[cfg(test)]
impl<'a> CodecHuffmanTable<'a> {
    pub(crate) fn general(self) -> Option<&'a HuffmanTable> {
        match self {
            Self::StandardA(_) => None,
            Self::General(table) => Some(table),
        }
    }
}

#[cfg(test)]
mod tests;
