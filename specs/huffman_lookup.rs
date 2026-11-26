use vstd::prelude::*;

verus! {

// Audit: huffman-fast-shape
// Model navigation: spec::fast_shape [representation-invariant]
// Purpose: Production lookahead tables have no prefixes shorter than eight bits.
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn fast_shape(counts: Seq<u32>) -> bool {
    counts.len() == 256
        && forall|length: int| 1 <= length < 8 ==> #[trigger] counts[length] == 0
}

// Audit: huffman-fast-slot
// Model navigation: spec::fast_slot [project-result-model]
// Purpose: Exact representable eight-bit code-to-original-line cache entry.
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn fast_slot(
    base: super::WideCode, count: u32, start: u32, order: Seq<u32>,
    prefix: u8, processed: u32,
) -> u16 {
    if base.limbs@[1] != 0 || base.limbs@[2] != 0 || base.limbs@[3] != 0
        || base.limbs@[0] > prefix as u64 {
        0
    } else {
        let distance = prefix as int - base.limbs@[0] as int;
        let index = start as int + distance;
        if distance >= count || distance >= processed
            || index > u32::MAX || index >= order.len() {
            0
        } else {
            let encoded = order[index] as int + 1;
            if encoded <= u16::MAX { encoded as u16 } else { 0u16 }
        }
    }
}

// Audit: huffman-table-lookup-valid
// Model navigation: spec::table_lookup_valid [project-result-model]
// Purpose: Constructor-owned cache agrees with its production eight-bit metadata.
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn table_lookup_valid(table: &super::HuffmanTable) -> bool {
    if table.prefer_prefix_lookup {
        table.max_prefix_len >= 8
            && fast_shape(super::huffman_spec::table_length_counts(table))
            && super::huffman_spec::slice_view(&table.prefix_lookup).len() == 256
            && forall|prefix: int| 0 <= prefix < 256 ==>
                #[trigger] super::huffman_spec::slice_view(&table.prefix_lookup)[prefix] == fast_slot(
                    super::huffman_spec::table_first_codes(table)[8], super::huffman_spec::table_length_counts(table)[8], super::huffman_spec::table_first_lines(table)[8],
                    super::huffman_spec::slice_view(&table.ordered_lines), prefix as u8, super::huffman_spec::table_length_counts(table)[8],
                )
    } else { super::huffman_spec::slice_view(&table.prefix_lookup).len() == 0 }
}

pub(super) proof fn small_code_value(
    base: super::WideCode, distance: u32, code: super::WideCode,
)
    requires
        super::huffman_spec::checked_add_small(base, distance) == Some(code),
        code.limbs@[1] == 0, code.limbs@[2] == 0, code.limbs@[3] == 0,
        code.limbs@[0] <= 255,
    ensures
        base.limbs@[1] == 0, base.limbs@[2] == 0, base.limbs@[3] == 0,
        base.limbs@[0] as int + distance as int == code.limbs@[0] as int,
{
    reveal(super::huffman_spec::checked_add_small);
    reveal(super::huffman_spec::add_limb);
}

pub(super) proof fn advance_slot(
    base: super::WideCode, count: u32, start: u32, order: Seq<u32>,
    processed: u32, code: u8, encoded: u16,
)
    requires
        processed < count,
        base.limbs@[1] == 0, base.limbs@[2] == 0, base.limbs@[3] == 0,
        base.limbs@[0] as int + processed as int == code as int,
        start as int + processed as int <= u32::MAX,
        (start as int + processed as int) < order.len(),
        encoded as int == if order[start as int + processed as int] as int + 1 <= u16::MAX {
            order[start as int + processed as int] as int + 1
        } else { 0int },
    ensures forall|prefix: u8| #[trigger] fast_slot(
        base, count, start, order, prefix, (processed + 1) as u32,
    ) == if prefix == code && encoded != 0 {
        encoded
    } else { fast_slot(base, count, start, order, prefix, processed) },
{
    assert forall|prefix: u8| #[trigger] fast_slot(
        base, count, start, order, prefix, (processed + 1) as u32,
    ) == if prefix == code && encoded != 0 {
        encoded
    } else { fast_slot(base, count, start, order, prefix, processed) } by {
        if (prefix as int) < base.limbs@[0] as int {
        } else {
            let distance = prefix as int - base.limbs@[0] as int;
            if distance == processed {
                assert(prefix == code);
            } else {
                assert((distance < processed + 1) == (distance < processed));
            }
        }
    }
}

pub(super) proof fn table_lookup_definition()
    ensures forall|table: &super::HuffmanTable|
        #[trigger] table_lookup_valid(table) == (
            if table.prefer_prefix_lookup {
                table.max_prefix_len >= 8
                    && fast_shape(super::huffman_spec::table_length_counts(table))
                    && super::huffman_spec::slice_view(&table.prefix_lookup).len() == 256
                    && forall|prefix: int| 0 <= prefix < 256 ==>
                        #[trigger] super::huffman_spec::slice_view(&table.prefix_lookup)[prefix] == fast_slot(
                            super::huffman_spec::table_first_codes(table)[8], super::huffman_spec::table_length_counts(table)[8], super::huffman_spec::table_first_lines(table)[8],
                            super::huffman_spec::slice_view(&table.ordered_lines), prefix as u8, super::huffman_spec::table_length_counts(table)[8],
                        )
            } else { super::huffman_spec::slice_view(&table.prefix_lookup).len() == 0 }),
{
    reveal(table_lookup_valid);
}

}
