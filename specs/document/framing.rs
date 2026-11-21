// Source-only author navigation, not normative approval.
// Guide: ../README.md; audit ledger: ../../validation/traceability/requirements.json

use vstd::prelude::*;

verus! {

// Project unknown-length framing: first complete body marker and bounded row count.
// Model navigation: spec::unknown_generic_marker [representation-invariant]
// Purpose: Literal marker and row-count byte window inside the coding body.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn unknown_generic_marker(bytes: Seq<u8>, offset: int) -> bool {
    &&& bytes.len() >= 18
    &&& 0 <= offset && offset + 6 <= bytes.len()
    &&& bytes[offset] == if bytes[17] & 1 != 0 { 0u8 } else { 0xffu8 }
    &&& bytes[offset + 1] == if bytes[17] & 1 != 0 { 0u8 } else { 0xacu8 }
}

// Model navigation: spec::unknown_generic_window [representation-invariant]
// Purpose: First complete body marker after the full header and bounded rows.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
#[verifier::opaque]
pub(crate) open spec fn unknown_generic_window(bytes: Seq<u8>, end: int) -> bool {
    &&& bytes.len() >= 18
    &&& crate::generic_region::generic_region_spec::generic_header_length(bytes[17]) <= end - 6
    &&& unknown_generic_marker(bytes, end - 6)
    &&& crate::spec::segments::be_u32_at(bytes, end - 4)
        <= crate::spec::segments::be_u32_at(bytes, 4)
    &&& forall|before: int|
        crate::generic_region::generic_region_spec::generic_header_length(bytes[17]) <= before < end - 6 ==>
            !unknown_generic_marker(bytes, before)
}

pub(crate) proof fn unknown_generic_window_prefix(bytes: Seq<u8>, end: int)
    requires unknown_generic_window(bytes, end),
    ensures unknown_generic_window(bytes.subrange(0, end), end),
{
    reveal(unknown_generic_window);
    let prefix = bytes.subrange(0, end);
    assert forall|before: int|
        crate::generic_region::generic_region_spec::generic_header_length(prefix[17]) <= before < end - 6
        implies !unknown_generic_marker(prefix, before) by {
        assert(!unknown_generic_marker(bytes, before));
    }
}

// Model navigation: spec::payload_window [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: document-payload-window
// REQUIREMENT CONTEXT: T88-SEQUENTIAL-SEGMENT-LAYOUT | ITU-T T.88 (08/2018) | D.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn payload_window(
    header: &crate::segment_header::SegmentHeader,
    bytes: Seq<u8>,
    start: int,
    end: int,
    payload: Seq<u8>,
) -> bool {
    &&& 0 <= start <= end <= bytes.len()
    &&& payload == bytes.subrange(start, end)
    &&& match header.data_length {
        Some(length) => end - start == length,
        None => true,
    }
}

}
