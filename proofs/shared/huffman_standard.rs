// Finite metadata checks and constructor/cursor agreement for all standard tables.

verus! {

pub(super) proof fn all_canonical()
    ensures
        standard_spec::canonical(&data::A), standard_spec::canonical(&data::B),
        standard_spec::canonical(&data::C), standard_spec::canonical(&data::D),
        standard_spec::canonical(&data::E), standard_spec::canonical(&data::F),
        standard_spec::canonical(&data::G), standard_spec::canonical(&data::H),
        standard_spec::canonical(&data::I), standard_spec::canonical(&data::J),
        standard_spec::canonical(&data::K), standard_spec::canonical(&data::L),
        standard_spec::canonical(&data::M), standard_spec::canonical(&data::N),
        standard_spec::canonical(&data::O),
{
    reveal(canonical);
    assert(standard_spec::canonical(&data::A)) by (compute);
    assert(standard_spec::canonical(&data::B)) by (compute);
    assert(standard_spec::canonical(&data::C)) by (compute);
    assert(standard_spec::canonical(&data::D)) by (compute);
    assert(standard_spec::canonical(&data::E)) by (compute);
    assert(standard_spec::canonical(&data::F)) by (compute);
    assert(standard_spec::canonical(&data::G)) by (compute);
    assert(standard_spec::canonical(&data::H)) by (compute);
    assert(standard_spec::canonical(&data::I)) by (compute);
    assert(standard_spec::canonical(&data::J)) by (compute);
    assert(standard_spec::canonical(&data::K)) by (compute);
    assert(standard_spec::canonical(&data::L)) by (compute);
    assert(standard_spec::canonical(&data::M)) by (compute);
    assert(standard_spec::canonical(&data::N)) by (compute);
    assert(standard_spec::canonical(&data::O)) by (compute);
}

proof fn bounded_at(desc: &StandardDescriptor, scanned: nat, index: nat)
    requires bounded_prefixes(desc, scanned), index < scanned,
    ensures 1 <= lines(desc)[index as int].prefix_len <= desc.max_prefix_len,
    decreases scanned,
{
    if index + 1 != scanned { bounded_at(desc, (scanned - 1) as nat, index); }
}

proof fn bucket_above(desc: &StandardDescriptor, length: nat, scanned: nat)
    requires canonical(desc), length > desc.max_prefix_len, scanned <= lines(desc).len(),
    ensures huffman_spec::bucket(lines(desc), length, scanned) == Seq::<u32>::empty(),
    decreases scanned,
{
    reveal(canonical);
    if scanned > 0 {
        bounded_at(desc, lines(desc).len(), (scanned - 1) as nat);
        bucket_above(desc, length, (scanned - 1) as nat);
    }
}

pub(super) open spec fn same_shape(desc: &StandardDescriptor, table: &HuffmanTable) -> bool {
    &&& canonical(desc)
    &&& table.lines@ == lines(desc)
    &&& table.max_prefix_len == desc.max_prefix_len
    &&& !table.prefer_prefix_lookup
    &&& table.ordered_lines@ == order(desc)
    &&& forall|length: int| 0 <= length < 10 ==>
        table.length_count@[length] == desc.length_count@[length]
    &&& forall|length: int| 1 <= length <= desc.max_prefix_len ==> (
        table.first_code@[length] == first(desc, length)
        && (desc.length_count@[length] > 0 ==> table.first_line@[length] == desc.first_line@[length])
    )
}

proof fn shape_from_constructor(desc: &StandardDescriptor, table: &HuffmanTable)
    requires
        canonical(desc),
        huffman_spec::canonical_assignment(lines(desc), table),
        huffman_lookup_spec::table_lookup_valid(table),
    ensures same_shape(desc, table),
{
    reveal(canonical);
    huffman_spec::canonical_assignment_definition();
    huffman_lookup_spec::table_lookup_definition();
    assert(huffman_spec::assignment_contents(lines(desc), table));
    assert(huffman_spec::assignment_order(lines(desc), table));
    if table.max_prefix_len > desc.max_prefix_len {
        bucket_above(desc, table.max_prefix_len as nat, lines(desc).len());
        assert(table.length_count@[table.max_prefix_len as int] == 0);
    }
    canonical_at(desc, desc.max_prefix_len as nat);
    assert(table.length_count@[desc.max_prefix_len as int] == desc.length_count@[desc.max_prefix_len as int]);
    if table.max_prefix_len < desc.max_prefix_len {
        assert(table.length_count@[desc.max_prefix_len as int] == 0);
    }
    assert(table.max_prefix_len == desc.max_prefix_len);
    assert(table.lines@ == lines(desc));
    assert(table.ordered_lines@ == order(desc));
    assert forall|length: int| 0 <= length < 10 implies
        table.length_count@[length] == desc.length_count@[length] by {
        canonical_at(desc, length as nat);
    }
    assert forall|length: int| 1 <= length <= desc.max_prefix_len implies (
        table.first_code@[length] == first(desc, length)
        && (desc.length_count@[length] > 0 ==> table.first_line@[length] == desc.first_line@[length])
    ) by {
        canonical_at(desc, length as nat);
        assert(huffman_spec::first_prefix(lines(desc), length as nat)
            == Some(table.first_code@[length]));
    }
    if table.prefer_prefix_lookup {
        assert(table.length_count@[1] == 0);
        assert(table.length_count@[2] == 0);
        assert(table.length_count@[3] == 0);
        assert(table.length_count@[4] == 0);
        assert(table.length_count@[5] == 0);
        assert(table.length_count@[6] == 0);
        assert(table.length_count@[7] == 0);
    }
    assert(!table.prefer_prefix_lookup);
}

proof fn order_at(desc: &StandardDescriptor, index: int)
    requires canonical(desc), 0 <= index < lines(desc).len(),
    ensures order(desc)[index] == desc.ordered_lines@[index] as u32,
{
    reveal(canonical);
}

proof fn run_agreement(
    desc: &StandardDescriptor, table: &HuffmanTable, bytes: Seq<u8>, position: usize,
    length: usize, code: WideCode,
)
    requires
        same_shape(desc, table), bytes.len() <= usize::MAX / 8,
        position <= bytes.len() * 8, 1 <= length <= desc.max_prefix_len as usize + 1,
    ensures run(desc, bytes, position, length, code)
        == huffman_spec::fallback_run(table, bytes, position, length, code),
    decreases desc.max_prefix_len as usize + 1 - length,
{
    reveal(canonical);
    reveal(run);
    huffman_spec::fallback_run_definition(table, bytes, position, length, code);
    if length <= desc.max_prefix_len {
        if crate::spec::readers::bit_read_error(bytes, position, 1).is_none() {
            let next = (position + 1) as usize;
            let bit = crate::spec::readers::bits_value(bytes, position as nat, 1) as u32;
            if let Some(prefix) = huffman_spec::append_bit(code, bit) {
                let count = desc.length_count@[length as int] as u32;
                let distance = huffman_spec::small_distance_from(prefix, first(desc, length as int), count);
                match distance {
                    None => run_agreement(desc, table, bytes, next, (length + 1) as usize, prefix),
                    Some(distance) => {
                        reveal(huffman_spec::small_distance_from);
                        assert(distance < count);
                        canonical_at(desc, length as nat);
                        let ordered = desc.first_line@[length as int] as int + distance as int;
                        assert(0 <= ordered < lines(desc).len());
                        order_at(desc, ordered);
                        assert(table.ordered_lines@[ordered] == desc.ordered_lines@[ordered] as u32);
                    },
                }
            }
        }
    }
}

/// Exact result and consumed cursor, for any successful owned canonical constructor.
/// The immutable standard constructor supplies the same literal line sequence.
pub(super) proof fn constructor_agreement(
    desc: &StandardDescriptor, table: &HuffmanTable, bytes: Seq<u8>, position: usize,
)
    requires
        canonical(desc),
        huffman_spec::canonical_assignment(lines(desc), table),
        huffman_lookup_spec::table_lookup_valid(table),
        bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
    ensures decode_state(desc, bytes, position) == huffman_spec::decode_state(table, bytes, position),
{
    shape_from_constructor(desc, table);
    reveal(decode_state);
    huffman_spec::decode_state_definition(table, bytes, position);
    run_agreement(desc, table, bytes, position, 1, WideCode { limbs: [0u64, 0, 0, 0] });
}

pub(super) proof fn selected_lines(table: StandardHuffmanTable)
    ensures lines(selected(table)) == huffman_spec::standard_table_lines(table),
{
    reveal(selected);
    huffman_spec::standard_table_lines_definition(table);
}

pub(crate) proof fn standard_constructor_agreement(
    tag: StandardHuffmanTable, table: &HuffmanTable, bytes: Seq<u8>, position: usize,
)
    requires
        huffman_spec::canonical_assignment(huffman_spec::standard_table_lines(tag), table),
        huffman_lookup_spec::table_lookup_valid(table),
        bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
    ensures decode_state(selected(tag), bytes, position)
        == huffman_spec::decode_state(table, bytes, position),
{
    all_canonical();
    selected_definition(tag);
    selected_lines(tag);
    constructor_agreement(selected(tag), table, bytes, position);
}

pub(crate) proof fn standard_padding_agreement(
    tag: StandardHuffmanTable, table: &HuffmanTable, reader: &BitReader,
)
    requires
        huffman_spec::canonical_assignment(huffman_spec::standard_table_lines(tag), table),
        huffman_lookup_spec::table_lookup_valid(table),
        huffman_spec::reader_valid(reader),
    ensures zero_padding(selected(tag), reader)
        == huffman_spec::zero_padding_before_prefix(table, reader),
{
    all_canonical();
    selected_definition(tag);
    selected_lines(tag);
    shape_from_constructor(selected(tag), table);
    reveal(zero_padding);
    reveal(huffman_spec::zero_padding_before_prefix);
    huffman_spec::prefix_count_definition(table);
    let remaining = reader.bytes@.len() * 8 - reader.bit_position;
    if remaining < 8 {
        assert forall|length: int| 1 <= length <= remaining implies
            huffman_spec::prefix_count(table, length)
                == selected(tag).length_count@[length] by {
            assert(table.length_count@[length] == selected(tag).length_count@[length]);
        }
        if zero_padding(selected(tag), reader) {
            assert forall|length: int| 1 <= length <= remaining implies
                huffman_spec::prefix_count(table, length) == 0 by {
                assert(selected(tag).length_count@[length] == 0);
            }
        }
        if huffman_spec::zero_padding_before_prefix(table, reader) {
            assert forall|length: int| 1 <= length <= remaining implies
                selected(tag).length_count@[length] == 0 by {
                assert(huffman_spec::prefix_count(table, length) == 0);
            }
        }
    }
}

}
