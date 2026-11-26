use super::{arithmetic_contexts_valid, integer_contexts_valid};
use crate::symbol_text::ArithmeticCodec;
use vstd::prelude::*;

verus! {

// Model navigation: spec::arithmetic_codec_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn arithmetic_codec_valid(
    arithmetic: &ArithmeticCodec,
) -> bool {
    crate::arithmetic::arithmetic_spec::decoder_valid(&arithmetic.decoder)
        && integer_contexts_valid(&arithmetic.integers)
        && arithmetic_contexts_valid(arithmetic.iaid@)
        && prefix_valid(arithmetic)
        && match &arithmetic.refinement {
            Some(contexts) =>
                crate::generic_region::generic_region_spec::refinement_contexts_valid(contexts),
            None => true,
        }
}

// Model navigation: spec::prefix_valid [representation-invariant]
// Purpose: Bind an optional numeric-certainty observer to the real arithmetic codec.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn prefix_valid(a: &ArithmeticCodec) -> bool {
    match a.prefix {
        Some(p) => crate::arithmetic::certainty_spec::observer_valid(&p, &a.decoder),
        None => true,
    }
}

// Model navigation: spec::arithmetic_end_certificate [project-policy]
// Purpose: Require a valid codec, a saved complete prefix and the exact terminal marker.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn arithmetic_end_certificate(a: &ArithmeticCodec, count: u32) -> bool {
    arithmetic_codec_valid(a)
    && match a.prefix {
        Some(p) => crate::arithmetic::certainty_spec::saved_prefix(&p, count)
            && match a.decoder.marker {
                Some(m) => m.offset == p.entropy_end && m.code == 0xac,
                None => false,
            },
        None => false,
    }
}

// Model navigation: spec::prefix_saved_unchanged [representation-invariant]
// Purpose: Preserve the pre-final-DS certificate across genuine later decoding.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn prefix_saved_unchanged(before: &ArithmeticCodec,
    after: &ArithmeticCodec) -> bool {
    match before.prefix {
        Some(p) => after.prefix.is_some() && after.prefix.unwrap().saved == p.saved,
        None => after.prefix.is_none(),
    }
}

}
