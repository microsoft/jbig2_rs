//! Exact prefix-error and shared suffix-state model for the private A descriptor.

use vstd::prelude::*;

verus! {

pub(super) open spec fn decode_body(
    bytes: Seq<u8>, position: usize,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
{
    match crate::spec::readers::bit_read_error(bytes, position, 1) {
        Some(error) => (Err(super::HuffmanError::Decode(
            crate::DecodeError::Malformed(error))), position),
        None => {
            let next = (position + 1) as usize;
            if crate::spec::readers::bits_value(bytes, position as nat, 1) == 0 {
                super::huffman_spec::line_state(super::TABLE_A@[0], bytes, next)
            } else {
                match crate::spec::readers::bit_read_error(bytes, next, 1) {
                    Some(error) => (Err(super::HuffmanError::Decode(
                        crate::DecodeError::Malformed(error))), next),
                    None => {
                        let third_position = (next + 1) as usize;
                        if crate::spec::readers::bits_value(bytes, next as nat, 1) == 0 {
                            super::huffman_spec::line_state(super::TABLE_A@[1], bytes, third_position)
                        } else {
                            match crate::spec::readers::bit_read_error(bytes, third_position, 1) {
                                Some(error) => (Err(super::HuffmanError::Decode(
                                    crate::DecodeError::Malformed(error))), third_position),
                                None => {
                                    let line = if crate::spec::readers::bits_value(
                                        bytes, third_position as nat, 1,
                                    ) == 0 { 2int } else { 3int };
                                    super::huffman_spec::line_state(
                                        super::TABLE_A@[line], bytes, (third_position + 1) as usize,
                                    )
                                },
                            }
                        }
                    },
                }
            }
        },
    }
}

pub(crate) closed spec fn decode_state(
    bytes: Seq<u8>, position: usize,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
{
    decode_body(bytes, position)
}

pub(super) proof fn decode_state_definition(bytes: Seq<u8>, position: usize)
    requires bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
    ensures decode_state(bytes, position) == decode_body(bytes, position),
{
    reveal(decode_state);
}

pub(crate) open spec fn codec_state(
    table: super::CodecHuffmanTable, bytes: Seq<u8>, position: usize,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
{
    match table {
        super::CodecHuffmanTable::StandardA(_) => decode_state(bytes, position),
        super::CodecHuffmanTable::General(table) =>
            super::huffman_spec::decode_state(table, bytes, position),
    }
}

pub(super) open spec fn owned_shape(table: &super::HuffmanTable) -> bool {
    &&& table.lines@ == super::TABLE_A@
    &&& table.max_prefix_len == 3
    &&& !table.prefer_prefix_lookup
    &&& table.length_count@[1] == 1
    &&& table.length_count@[2] == 1
    &&& table.length_count@[3] == 2
    &&& table.first_code@[1] == (super::WideCode { limbs: [0u64, 0, 0, 0] })
    &&& table.first_code@[2] == (super::WideCode { limbs: [2u64, 0, 0, 0] })
    &&& table.first_code@[3] == (super::WideCode { limbs: [6u64, 0, 0, 0] })
    &&& table.first_line@[1] == 0
    &&& table.first_line@[2] == 1
    &&& table.first_line@[3] == 2
    &&& table.ordered_lines@ == seq![0u32, 1, 2, 3]
}

}
