//! Constructor-to-descriptor bridge; suffixes retain the existing line-state model.

use super::*;
use crate::huffman::{HuffmanLine, LineKind};
use vstd::prelude::*;

verus! {

proof fn fixed_lines()
    ensures
        TABLE_A@[0] == (HuffmanLine {
            prefix_len: 1, range_len: 4, range_low: 0, kind: LineKind::Normal,
        }),
        TABLE_A@[1] == (HuffmanLine {
            prefix_len: 2, range_len: 8, range_low: 16, kind: LineKind::Normal,
        }),
        TABLE_A@[2] == (HuffmanLine {
            prefix_len: 3, range_len: 16, range_low: 272, kind: LineKind::Normal,
        }),
        TABLE_A@[3] == (HuffmanLine {
            prefix_len: 3, range_len: 32, range_low: 65_808, kind: LineKind::Upper,
        }),
{
    assert(TABLE_A@[0].prefix_len == 1 && TABLE_A@[0].range_len == 4
        && TABLE_A@[0].range_low == 0 && TABLE_A@[0].kind == LineKind::Normal) by (compute);
    assert(TABLE_A@[1].prefix_len == 2 && TABLE_A@[1].range_len == 8
        && TABLE_A@[1].range_low == 16 && TABLE_A@[1].kind == LineKind::Normal) by (compute);
    assert(TABLE_A@[2].prefix_len == 3 && TABLE_A@[2].range_len == 16
        && TABLE_A@[2].range_low == 272 && TABLE_A@[2].kind == LineKind::Normal) by (compute);
    assert(TABLE_A@[3].prefix_len == 3 && TABLE_A@[3].range_len == 32
        && TABLE_A@[3].range_low == 65_808 && TABLE_A@[3].kind == LineKind::Upper) by (compute);
}

proof fn shape_from_constructor(table: &HuffmanTable)
    requires
        huffman_spec::canonical_assignment(TABLE_A@, table),
        huffman_lookup_spec::table_lookup_valid(table),
    ensures standard_a_spec::owned_shape(table),
{
    fixed_lines();
    huffman_spec::canonical_assignment_definition();
    huffman_lookup_spec::table_lookup_definition();
    assert(huffman_spec::bucket(TABLE_A@, 0, 4) == Seq::<u32>::empty()) by (compute);
    assert(huffman_spec::bucket(TABLE_A@, 1, 4) == seq![0u32]) by (compute);
    assert(huffman_spec::bucket(TABLE_A@, 2, 4) == seq![1u32]) by (compute);
    assert(huffman_spec::bucket(TABLE_A@, 3, 4) == seq![2u32, 3]) by (compute);
    assert forall|length: int| 4 <= length < 256 implies
        #[trigger] huffman_spec::bucket(TABLE_A@, length as nat, 4).len() == 0 by {
        reveal_with_fuel(huffman_spec::bucket, 5);
    }
    assert(table.length_count@[1] == 1);
    assert(table.length_count@[2] == 1);
    assert(table.length_count@[3] == 2);
    if table.max_prefix_len < 3 {
        assert(table.length_count@[3] == 0);
    }
    if table.max_prefix_len > 3 {
        assert(table.length_count@[table.max_prefix_len as int] == 0);
    }
    assert(table.max_prefix_len == 3);
    assert(!table.prefer_prefix_lookup);
    assert(huffman_spec::ordered_before(TABLE_A@, 1) == Seq::<u32>::empty()) by (compute);
    assert(huffman_spec::ordered_before(TABLE_A@, 2) == seq![0u32]) by (compute);
    assert(huffman_spec::ordered_before(TABLE_A@, 3) == seq![0u32, 1]) by (compute);
    assert(huffman_spec::ordered_before(TABLE_A@, 4) == seq![0u32, 1, 2, 3]) by (compute);
    assert(huffman_spec::first_prefix(TABLE_A@, 1)
        == Some(WideCode { limbs: [0u64, 0, 0, 0] })) by (compute);
    assert(huffman_spec::first_prefix(TABLE_A@, 2)
        == Some(WideCode { limbs: [2u64, 0, 0, 0] })) by (compute);
    assert(huffman_spec::first_prefix(TABLE_A@, 3)
        == Some(WideCode { limbs: [6u64, 0, 0, 0] })) by (compute);
    assert(huffman_spec::first_prefix(TABLE_A@, 1) == Some(table.first_code@[1]));
    assert(huffman_spec::first_prefix(TABLE_A@, 2) == Some(table.first_code@[2]));
    assert(huffman_spec::first_prefix(TABLE_A@, 3) == Some(table.first_code@[3]));
}

proof fn append_small(code: u64, bit: u32)
    requires code <= 3, bit <= 1,
    ensures huffman_spec::append_bit(WideCode { limbs: [code, 0, 0, 0] }, bit)
        == Some(WideCode { limbs: [(code * 2 + bit as u64) as u64, 0, 0, 0] }),
{
    reveal(huffman_spec::append_bit);
    reveal(huffman_spec::checked_shift_left_one);
    reveal(huffman_spec::checked_add_small);
    reveal(huffman_spec::add_limb);
    assert(0u64 >> 63 == 0 && (0u64 << 1) | (0u64 >> 63) == 0) by (bit_vector);
    assert(code >> 63 == 0 && code << 1 == code * 2) by (bit_vector)
        requires code <= 3;
    assert((0u64 << 1) | (code >> 63) == 0) by (bit_vector)
        requires code <= 3;
}

proof fn available_bit(bytes: Seq<u8>, position: usize)
    requires
        bytes.len() <= usize::MAX / 8,
        position <= bytes.len() * 8,
        crate::spec::readers::bit_read_error(bytes, position, 1).is_none(),
    ensures
        position < bytes.len() * 8,
        position < usize::MAX,
        crate::spec::readers::bits_value(bytes, position as nat, 1) <= 1,
{
    huffman_shared::bits_value_is_bounded(bytes, position as nat, 1);
    reveal_with_fuel(huffman_spec::pow2, 2);
}

/// Applies to successful results of both `standard(A, ..)` and `new(TABLE_A, ..)`.
pub(crate) proof fn constructor_agreement(
    table: &HuffmanTable, bytes: Seq<u8>, position: usize,
)
    requires
        huffman_spec::canonical_assignment(huffman_spec::standard_a_lines(), table),
        huffman_lookup_spec::table_lookup_valid(table),
        bytes.len() <= usize::MAX / 8,
        position <= bytes.len() * 8,
    ensures huffman_spec::decode_state(table, bytes, position)
        == standard_a_spec::decode_state(bytes, position),
{
    huffman_spec::standard_a_lines_definition();
    shape_from_constructor(table);
    huffman_spec::decode_state_definition(table, bytes, position);
    huffman_spec::fallback_run_definition(
        table, bytes, position, 1, WideCode { limbs: [0u64, 0, 0, 0] },
    );
    standard_a_spec::decode_state_definition(bytes, position);
    reveal(huffman_spec::small_distance_from);
    reveal(huffman_spec::subtract_limb);
    if crate::spec::readers::bit_read_error(bytes, position, 1).is_none() {
        available_bit(bytes, position);
        let first = crate::spec::readers::bits_value(bytes, position as nat, 1) as u32;
        append_small(0, first);
        let second_position = (position + 1) as usize;
        if first != 0 {
            assert(first == 1);
            huffman_spec::fallback_run_definition(
                table, bytes, second_position, 2, WideCode { limbs: [1u64, 0, 0, 0] },
            );
            if crate::spec::readers::bit_read_error(bytes, second_position, 1).is_none() {
                available_bit(bytes, second_position);
                let second = crate::spec::readers::bits_value(
                    bytes, second_position as nat, 1,
                ) as u32;
                append_small(1, second);
                let third_position = (second_position + 1) as usize;
                if second != 0 {
                    assert(second == 1);
                    huffman_spec::fallback_run_definition(
                        table, bytes, third_position, 3, WideCode { limbs: [3u64, 0, 0, 0] },
                    );
                    if crate::spec::readers::bit_read_error(bytes, third_position, 1).is_none() {
                        available_bit(bytes, third_position);
                        let third = crate::spec::readers::bits_value(
                            bytes, third_position as nat, 1,
                        ) as u32;
                        append_small(3, third);
                        if third != 0 {
                            assert(third == 1);
                        }
                    }
                }
            }
        }
    }
}

pub(crate) proof fn final_strip_padding_agreement(
    table: &HuffmanTable, reader: &BitReader,
)
    requires
        huffman_spec::canonical_assignment(huffman_spec::standard_a_lines(), table),
        huffman_lookup_spec::table_lookup_valid(table),
        huffman_spec::reader_valid(reader),
    ensures huffman_spec::zero_padding_before_prefix(table, reader)
        == (reader.bit_position == reader.bytes@.len() * 8),
{
    huffman_spec::standard_a_lines_definition();
    shape_from_constructor(table);
    huffman_spec::prefix_count_definition(table);
    reveal(huffman_spec::zero_padding_before_prefix);
    let remaining = reader.bytes@.len() * 8 - reader.bit_position;
    if 0 < remaining < 8 {
        assert(huffman_spec::prefix_count(table, 1) == 1);
    }
}

}
