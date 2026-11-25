// Compact standard descriptors reuse the canonical assignment and exact suffix models.
// These are local representation/result bridges, not independent normative approval.

use super::*;
use vstd::prelude::*;

verus! {

pub(crate) closed spec fn selected(table: StandardHuffmanTable) -> &'static StandardDescriptor {
    match table {
        StandardHuffmanTable::A => &data::A,
        StandardHuffmanTable::B => &data::B,
        StandardHuffmanTable::C => &data::C,
        StandardHuffmanTable::D => &data::D,
        StandardHuffmanTable::E => &data::E,
        StandardHuffmanTable::F => &data::F,
        StandardHuffmanTable::G => &data::G,
        StandardHuffmanTable::H => &data::H,
        StandardHuffmanTable::I => &data::I,
        StandardHuffmanTable::J => &data::J,
        StandardHuffmanTable::K => &data::K,
        StandardHuffmanTable::L => &data::L,
        StandardHuffmanTable::M => &data::M,
        StandardHuffmanTable::N => &data::N,
        StandardHuffmanTable::O => &data::O,
    }
}

pub(super) open spec fn first(desc: &StandardDescriptor, length: int) -> WideCode {
    WideCode { limbs: [desc.first_code@[length] as u64, 0, 0, 0] }
}

pub(super) open spec fn lines(desc: &StandardDescriptor) -> Seq<crate::huffman::HuffmanLine> {
    huffman_spec::standard_source_lines(desc.lines)
}

pub(super) open spec fn order(desc: &StandardDescriptor) -> Seq<u32> {
    let a = desc.ordered_lines@;
    seq![
        a[0] as u32, a[1] as u32, a[2] as u32, a[3] as u32, a[4] as u32,
        a[5] as u32, a[6] as u32, a[7] as u32, a[8] as u32, a[9] as u32,
        a[10] as u32, a[11] as u32, a[12] as u32, a[13] as u32, a[14] as u32,
        a[15] as u32, a[16] as u32, a[17] as u32, a[18] as u32, a[19] as u32,
        a[20] as u32, a[21] as u32,
    ].take(lines(desc).len() as int)
}

pub(super) open spec fn canonical_lengths(desc: &StandardDescriptor, next: nat) -> bool
    decreases next,
{
    if next == 0 { true } else {
        let length = (next - 1) as int;
        &&& canonical_lengths(desc, (next - 1) as nat)
        &&& desc.length_count@[length] == huffman_spec::bucket(
            lines(desc), length as nat, lines(desc).len(),
        ).len()
        &&& (length <= desc.max_prefix_len ==> huffman_spec::first_prefix(
            lines(desc), length as nat,
        ) == Some(first(desc, length)))
        &&& (desc.length_count@[length] > 0 ==> (
            desc.first_line@[length] == huffman_spec::ordered_before(
                lines(desc), length as nat,
            ).len()
            && desc.first_line@[length] as nat + desc.length_count@[length] as nat
                <= lines(desc).len()
        ))
        &&& (length > desc.max_prefix_len ==> desc.length_count@[length] == 0)
    }
}

pub(super) open spec fn bounded_prefixes(desc: &StandardDescriptor, scanned: nat) -> bool
    decreases scanned,
{
    scanned == 0 || (
        bounded_prefixes(desc, (scanned - 1) as nat)
        && 1 <= lines(desc)[scanned - 1].prefix_len <= desc.max_prefix_len
    )
}

pub(crate) closed spec fn canonical(desc: &StandardDescriptor) -> bool {
    &&& 1 <= desc.max_prefix_len <= 9
    &&& lines(desc).len() <= 22
    &&& canonical_lengths(desc, 10)
    &&& bounded_prefixes(desc, lines(desc).len())
    &&& desc.length_count@[desc.max_prefix_len as int] > 0
    &&& order(desc) == huffman_spec::ordered_before(lines(desc), desc.max_prefix_len as nat + 1)
    &&& order(desc).len() == lines(desc).len()
    &&& (desc.length_count@[1] > 0 || desc.length_count@[2] > 0
        || desc.length_count@[3] > 0 || desc.length_count@[4] > 0
        || desc.length_count@[5] > 0 || desc.length_count@[6] > 0
        || desc.length_count@[7] > 0)
}

pub(super) proof fn canonical_at(desc: &StandardDescriptor, length: nat)
    requires canonical(desc), length < 10,
    ensures
        desc.length_count@[length as int] == huffman_spec::bucket(
            lines(desc), length, lines(desc).len(),
        ).len(),
        length <= desc.max_prefix_len ==>
            huffman_spec::first_prefix(lines(desc), length) == Some(first(desc, length as int)),
        desc.length_count@[length as int] > 0 ==> (
            desc.first_line@[length as int]
                == huffman_spec::ordered_before(lines(desc), length).len()
            && desc.first_line@[length as int] as nat + desc.length_count@[length as int] as nat
                <= lines(desc).len()
        ),
        length > desc.max_prefix_len ==> desc.length_count@[length as int] == 0,
{
    reveal(canonical);
    canonical_prefix_at(desc, 10, length);
}

proof fn canonical_prefix_at(desc: &StandardDescriptor, next: nat, length: nat)
    requires canonical_lengths(desc, next), length < next,
    ensures canonical_lengths(desc, length + 1),
    decreases next,
{
    if next != length + 1 { canonical_prefix_at(desc, (next - 1) as nat, length); }
}

pub(crate) closed spec fn decode_state(
    desc: &StandardDescriptor, bytes: Seq<u8>, position: usize,
) -> (Result<HuffmanValue, HuffmanError>, usize) {
    run(desc, bytes, position, 1, WideCode { limbs: [0u64, 0, 0, 0] })
}

pub(super) open spec fn run(
    desc: &StandardDescriptor, bytes: Seq<u8>, position: usize, length: usize, code: WideCode,
) -> (Result<HuffmanValue, HuffmanError>, usize)
    decreases if length < 10 { (10 - length) as nat } else { 0nat },
{
    if length > desc.max_prefix_len || length >= 10 {
        (Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)), position)
    } else {
        match crate::spec::readers::bit_read_error(bytes, position, 1) {
            Some(error) => (Err(HuffmanError::Decode(DecodeError::Malformed(error))), position),
            None => {
                let next = (position + 1) as usize;
                let bit = crate::spec::readers::bits_value(bytes, position as nat, 1) as u32;
                match huffman_spec::append_bit(code, bit) {
                    None => (Err(HuffmanError::Decode(DecodeError::Malformed(
                        MalformedError::IntegerOverflow,
                    ))), next),
                    Some(prefix) => match huffman_spec::small_distance_from(
                        prefix, first(desc, length as int), desc.length_count@[length as int] as u32,
                    ) {
                        None => run(desc, bytes, next, (length + 1) as usize, prefix),
                        Some(distance) => {
                            let ordered = desc.first_line@[length as int] as nat + distance as nat;
                            if ordered > usize::MAX {
                                (Err(HuffmanError::Decode(DecodeError::Malformed(
                                    MalformedError::IntegerOverflow,
                                ))), next)
                            } else if ordered >= 22 {
                                (Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)), next)
                            } else {
                                let index = desc.ordered_lines@[ordered as int] as nat;
                                if index >= lines(desc).len() {
                                    (Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)), next)
                                } else { huffman_spec::line_state(lines(desc)[index as int], bytes, next) }
                            }
                        },
                    },
                }
            },
        }
    }
}

pub(crate) closed spec fn zero_padding(desc: &StandardDescriptor, reader: &BitReader) -> bool {
    let remaining = reader.bytes@.len() * 8 - reader.bit_position;
    &&& huffman_spec::reader_valid(reader)
    &&& remaining < 8
    &&& forall|length: int| 1 <= length <= remaining ==> desc.length_count@[length] == 0
    &&& crate::spec::readers::bits_value(
        reader.bytes@, reader.bit_position as nat, remaining as nat,
    ) == 0
}

pub(crate) open spec fn codec_state(
    table: CodecHuffmanTable, bytes: Seq<u8>, position: usize,
) -> (Result<HuffmanValue, HuffmanError>, usize) {
    match table {
        CodecHuffmanTable::Standard(desc) => decode_state(desc, bytes, position),
        CodecHuffmanTable::General(table) => huffman_spec::decode_state(table, bytes, position),
    }
}

pub(crate) closed spec fn line_count(table: StandardHuffmanTable) -> nat {
    lines(selected(table)).len()
}

pub(super) proof fn selected_definition(table: StandardHuffmanTable)
    ensures *selected(table) == *match table {
        StandardHuffmanTable::A => &data::A,
        StandardHuffmanTable::B => &data::B,
        StandardHuffmanTable::C => &data::C,
        StandardHuffmanTable::D => &data::D,
        StandardHuffmanTable::E => &data::E,
        StandardHuffmanTable::F => &data::F,
        StandardHuffmanTable::G => &data::G,
        StandardHuffmanTable::H => &data::H,
        StandardHuffmanTable::I => &data::I,
        StandardHuffmanTable::J => &data::J,
        StandardHuffmanTable::K => &data::K,
        StandardHuffmanTable::L => &data::L,
        StandardHuffmanTable::M => &data::M,
        StandardHuffmanTable::N => &data::N,
        StandardHuffmanTable::O => &data::O,
    },
{
    reveal(selected);
}

pub(super) proof fn line_count_definition(table: StandardHuffmanTable)
    ensures line_count(table) == lines(selected(table)).len(),
{
    reveal(line_count);
}

pub(super) proof fn decode_state_definition(
    desc: &StandardDescriptor, bytes: Seq<u8>, position: usize,
)
    ensures decode_state(desc, bytes, position)
        == run(desc, bytes, position, 1, WideCode { limbs: [0u64, 0, 0, 0] }),
{
    reveal(decode_state);
}

pub(super) proof fn zero_padding_definition(desc: &StandardDescriptor, reader: &BitReader)
    ensures zero_padding(desc, reader) == {
        let remaining = reader.bytes@.len() * 8 - reader.bit_position;
        &&& huffman_spec::reader_valid(reader)
        &&& remaining < 8
        &&& forall|length: int| 1 <= length <= remaining ==> desc.length_count@[length] == 0
        &&& crate::spec::readers::bits_value(
            reader.bytes@, reader.bit_position as nat, remaining as nat,
        ) == 0
    },
{
    reveal(zero_padding);
}

}

include!("../proofs/shared/huffman_standard.rs");
