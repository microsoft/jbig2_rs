// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use super::slice_view;
use vstd::prelude::*;

verus! {

// Model navigation: spec::encoded_header_reserved_rejected [project-policy]
// Purpose: Reserved-bit header rejection for encoded B.2 Huffman tables.
// Audit: huffman-encoded-reserved-rejected
// REQUIREMENT CONTEXT: T88-HUFFMAN-RESERVED-FLAG | ITU-T T.88 (08/2018) | B.2.1 bit 7
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn encoded_header_reserved_rejected(
    bytes: Seq<u8>,
    limits: crate::Limits,
) -> bool {
    bytes.len() <= limits.max_stream_bytes
        && bytes.len() >= 1
        && (bytes[0] & 0x80u8) != 0
}

// Model navigation: spec::encoded_header_bounds_rejected [project-result-model]
// Purpose: Signed lower/upper header bound ordering rejection for encoded B.2 Huffman tables.
// Audit: huffman-encoded-bounds-rejected
// REQUIREMENT CONTEXT: T88-HUFFMAN-SIGNED-BOUNDS | ITU-T T.88 (08/2018) | B.2 steps 2-4; B.2.2; B.2.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn encoded_header_bounds_rejected(
    bytes: Seq<u8>,
    limits: crate::Limits,
) -> bool {
    bytes.len() <= limits.max_stream_bytes
        && bytes.len() >= 9
        && (bytes[0] & 0x80u8) == 0
        && (crate::spec::segments::be_u32_at(bytes, 1) as i32)
            >= (crate::spec::segments::be_u32_at(bytes, 5) as i32)
}

// Model navigation: spec::encoded_normal_lines_valid [project-result-model]
// Purpose: Contiguous normal-line partition and upper-bound coverage of encoded B.2 Huffman tables.
// Audit: huffman-encoded-normal-lines
// REQUIREMENT CONTEXT: T88-HUFFMAN-NORMAL-LINES | ITU-T T.88 (08/2018) | B.2 steps 4-5
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn encoded_normal_lines_valid(
    bytes: Seq<u8>,
    table: &crate::huffman::HuffmanTable,
) -> bool {
    let lines = slice_view(&table.lines);
    let has_oob = (bytes[0] & 1u8) != 0;
    let special_count: int = if has_oob { 3 } else { 2 };
    let low = crate::spec::segments::be_u32_at(bytes, 1) as i32;
    let high = crate::spec::segments::be_u32_at(bytes, 5) as i32;
    let normal_count = lines.len() - special_count;
    &&& bytes.len() >= 9
    &&& (bytes[0] & 0x80u8) == 0
    &&& low < high
    &&& normal_count >= 1
    &&& lines[0].range_low == low as i64
    &&& forall|i: int| 0 <= i < normal_count ==> {
        &&& lines[i].kind == crate::huffman::LineKind::Normal
        &&& lines[i].range_len <= 32
        &&& (i + 1 < normal_count ==>
            lines[i + 1].range_low as int
                == lines[i].range_low as int + (1i64 << (lines[i].range_len as i64)) as int)
    }
    &&& lines[normal_count - 1].range_low as int
        + (1i64 << (lines[normal_count - 1].range_len as i64)) as int >= high as int
}

// Model navigation: spec::encoded_special_lines_valid [project-result-model]
// Purpose: Lower, upper and optional OOB trailing special-line layout of encoded B.2 Huffman tables.
// Audit: huffman-encoded-special-lines
// REQUIREMENT CONTEXT: T88-HUFFMAN-SPECIAL-LINES | ITU-T T.88 (08/2018) | B.2 steps 6-10
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn encoded_special_lines_valid(
    bytes: Seq<u8>,
    table: &crate::huffman::HuffmanTable,
) -> bool {
    let lines = slice_view(&table.lines);
    let has_oob = (bytes[0] & 1u8) != 0;
    let special_count: int = if has_oob { 3 } else { 2 };
    let low = crate::spec::segments::be_u32_at(bytes, 1) as i32;
    let high = crate::spec::segments::be_u32_at(bytes, 5) as i32;
    let normal_count = lines.len() - special_count;
    &&& bytes.len() >= 9
    &&& normal_count >= 1
    &&& lines[normal_count].kind == crate::huffman::LineKind::Lower
    &&& lines[normal_count].range_low as int == low as int - 1
    &&& lines[normal_count].range_len == 32
    &&& lines[normal_count + 1].kind == crate::huffman::LineKind::Upper
    &&& lines[normal_count + 1].range_low as int == high as int
    &&& lines[normal_count + 1].range_len == 32
    &&& (has_oob ==> {
        &&& lines[normal_count + 2].kind == crate::huffman::LineKind::OutOfBand
        &&& lines[normal_count + 2].range_low == 0
        &&& lines[normal_count + 2].range_len == 0
    })
}

}
