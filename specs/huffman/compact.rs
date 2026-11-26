// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: ../README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::slice_view [proof-support]
// Purpose: Project owned or static Huffman table storage slices uniformly.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn slice_view<T: 'static>(storage: &crate::huffman::TableSlice<T>) -> Seq<T> {
    match storage {
        crate::huffman::TableSlice::Owned(v) => v@,
        crate::huffman::TableSlice::Static(s) => s@,
    }
}

// Model navigation: spec::table_length_counts [proof-support]
// Purpose: Project compact inline/spilled Huffman prefix-length counts as a 256-entry sequence.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn table_length_counts(table: &crate::huffman::HuffmanTable) -> Seq<u32> {
    Seq::new(256, |i: int| {
        if i <= 32 {
            table.short_length_count@[i]
        } else {
            match &table.long_tables {
                Some(long) => long.length_count@[i],
                None => 0u32,
            }
        }
    })
}

// Model navigation: spec::table_first_codes [proof-support]
// Purpose: Project compact inline/spilled Huffman first canonical codes as a 256-entry sequence.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn table_first_codes(
    table: &crate::huffman::HuffmanTable,
) -> Seq<crate::huffman::WideCode> {
    Seq::new(256, |i: int| {
        if i <= 32 {
            crate::huffman::WideCode {
                limbs: [table.short_first_code@[i] as u64, 0u64, 0u64, 0u64],
            }
        } else {
            match &table.long_tables {
                Some(long) => long.first_code@[i],
                None => crate::huffman::WideCode {
                    limbs: [0u64, 0u64, 0u64, 0u64],
                },
            }
        }
    })
}

// Model navigation: spec::table_first_lines [proof-support]
// Purpose: Project compact inline/spilled Huffman bucket start offsets as a 256-entry sequence.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn table_first_lines(table: &crate::huffman::HuffmanTable) -> Seq<u32> {
    Seq::new(256, |i: int| {
        if i <= 32 {
            table.short_first_line@[i]
        } else {
            match &table.long_tables {
                Some(long) => long.first_line@[i],
                None => 0u32,
            }
        }
    })
}

}
