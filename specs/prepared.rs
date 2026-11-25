// Project opt-in ownership/copy policy, not normative JBIG2 or full-stream semantics.
// Source navigation: README.md. No independent normative approval is asserted.

use super::{PreparedGlobals, PreparedGlobalsBypass, PreparedGlobalsInfo, PreparedGlobalsState};
use crate::decode::{decode_spec, InitialResources};
use super::copying::{CopyBudget, CopyFailure};
use vstd::prelude::*;

verus! {

// Model navigation: spec::allowance [project-policy]
// Purpose: Explicit additional storage cap; not the per-image/aggregate memory budget.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn allowance(limits: crate::Limits, requested: u64) -> u64 {
    let memory = if requested > limits.max_decoder_managed_memory_bytes {
        limits.max_decoder_managed_memory_bytes
    } else { requested };
    if memory > limits.max_retained_state_bytes {
        limits.max_retained_state_bytes
    } else { memory }
}

// Model navigation: spec::admission [project-policy]
// Purpose: Exact additional-storage clamping, conservative resource admission, inclusive checked budget outcomes and allocation-only optional bypass classification in the new opt-in owner.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn admission(
    resources: &InitialResources,
) -> Option<PreparedGlobalsBypass> {
    if resources.tables@.len() != 0 {
        Some(PreparedGlobalsBypass::CustomTables)
    } else if exists|index: int| 0 <= index < resources.symbols@.len()
        && resources.symbols@[index].1.contexts.is_some() {
        Some(PreparedGlobalsBypass::RetainedContexts)
    } else if resources.patterns@.len() == 0 && resources.symbols@.len() == 0 {
        Some(PreparedGlobalsBypass::NoResources)
    } else { None }
}

// Model navigation: spec::valid_fields [representation-invariant]
// Purpose: Constructor-established sealed owner state, decoder-limit bounds, Ready/resource agreement and valid bitmap stores without retained contexts or custom tables.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn valid_fields(owner: &PreparedGlobals) -> bool {
    &&& owner.additional_storage_bytes
        <= crate::document::decoder_limits(owner.decoder).max_decoder_managed_memory_bytes
    &&& owner.additional_storage_bytes
        <= crate::document::decoder_limits(owner.decoder).max_retained_state_bytes
    &&& owner.resident_bytes <= owner.additional_storage_bytes
    &&& match owner.resources {
        Some(resources) =>
            owner.state == PreparedGlobalsState::Ready
            && decode_spec::initial_resources_valid(&resources)
            && admission(&resources).is_none(),
        None => owner.resident_bytes == 0 && owner.state != PreparedGlobalsState::Ready,
    }
}

// Model navigation: spec::valid [representation-invariant]
// Purpose: Constructor-established sealed owner state, bitmap stores, admission and bounds.
// Scope: Existing capacity observations are checked at runtime, not new allocator axioms.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn valid(owner: &PreparedGlobals) -> bool {
    valid_fields(owner)
}

// Model navigation: spec::info_fields [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn info_fields(owner: &PreparedGlobals) -> PreparedGlobalsInfo {
    PreparedGlobalsInfo {
        state: owner.state,
        additional_storage_bytes: owner.additional_storage_bytes,
        resident_bytes: owner.resident_bytes,
    }
}

// Model navigation: spec::info [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn info(owner: &PreparedGlobals) -> PreparedGlobalsInfo {
    info_fields(owner)
}

// Model navigation: spec::same_owner_fields [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_owner_fields(
    before: &PreparedGlobals, after: &PreparedGlobals,
) -> bool {
    after.decoder == before.decoder
        && after.globals == before.globals
        && after.additional_storage_bytes == before.additional_storage_bytes
}

// Exact borrowed configuration/globals, not an equality/hash-based cache key.
// Model navigation: spec::same_owner [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn same_owner(before: &PreparedGlobals, after: &PreparedGlobals) -> bool {
    same_owner_fields(before, after)
}

// Model navigation: spec::fresh_fields [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn fresh_fields(
    decoder: &crate::Decoder,
    globals: &crate::GlobalSegments,
    requested: u64,
    owner: &PreparedGlobals,
) -> bool {
    &&& owner.decoder == decoder
    &&& owner.globals == globals
    &&& owner.additional_storage_bytes
        == allowance(crate::document::decoder_limits(decoder), requested)
    &&& owner.state == PreparedGlobalsState::Pending
    &&& owner.resources.is_none()
    &&& owner.resident_bytes == 0
    &&& valid(owner)
}

// Model navigation: spec::fresh [project-result-model]
// Purpose: Actual lazy construction, immutable borrowed decoder/global/allowance frames, allocation-free public state snapshots and reset/return lifecycle results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn fresh(
    decoder: &crate::Decoder,
    globals: &crate::GlobalSegments,
    requested: u64,
    owner: &PreparedGlobals,
) -> bool {
    fresh_fields(decoder, globals, requested, owner)
}

pub(super) proof fn definitions()
    ensures
        forall|owner: &PreparedGlobals| #[trigger] valid(owner) == valid_fields(owner),
        forall|owner: &PreparedGlobals| #[trigger] info(owner) == info_fields(owner),
        forall|before: &PreparedGlobals, after: &PreparedGlobals|
            #[trigger] same_owner(before, after) == same_owner_fields(before, after),
        forall|decoder: &crate::Decoder, globals: &crate::GlobalSegments,
               requested: u64, owner: &PreparedGlobals|
            #[trigger] fresh(decoder, globals, requested, owner)
                == fresh_fields(decoder, globals, requested, owner),
{
    reveal(valid);
    reveal(info);
    reveal(same_owner);
    reveal(fresh);
}

// Model navigation: spec::budget_frame [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn budget_frame(before: &CopyBudget, after: &CopyBudget) -> bool {
    after.limit == before.limit && before.used <= after.used <= after.limit
}

// Model navigation: spec::budget_check [project-policy]
// Purpose: Exact additional-storage clamping, conservative resource admission, inclusive checked budget outcomes and allocation-only optional bypass classification in the new opt-in owner.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn budget_check(
    used: u64, bytes: u64, limit: u64, result: &Result<(), CopyFailure>,
) -> bool {
    if used as nat + bytes as nat > u64::MAX {
        *result == Err(CopyFailure::Decode(crate::DecodeError::Malformed(
            crate::MalformedError::IntegerOverflow)))
    } else if used as nat + bytes as nat > limit {
        *result == Err(CopyFailure::Optional(PreparedGlobalsBypass::AdditionalStorage))
    } else { *result == Ok(()) }
}

// Model navigation: spec::copy_failure [project-policy]
// Purpose: Exact additional-storage clamping, conservative resource admission, inclusive checked budget outcomes and allocation-only optional bypass classification in the new opt-in owner.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn copy_failure(error: &crate::DecodeError, failure: &CopyFailure) -> bool {
    match error {
        crate::DecodeError::Allocation { .. } =>
            *failure == CopyFailure::Optional(PreparedGlobalsBypass::CopyAllocation),
        _ => *failure == CopyFailure::Decode(*error),
    }
}

// Model navigation: spec::same_bitmap [project-result-model]
// Purpose: Exact width/height/stride and every packed byte of the fallible copy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_bitmap(source: &crate::Bitmap, copied: &crate::Bitmap) -> bool {
    &&& crate::spec::bitmap::storage_valid(source)
    &&& crate::spec::bitmap::storage_valid(copied)
    &&& source.width == copied.width
    &&& source.height == copied.height
    &&& source.stride == copied.stride
    &&& source.data@ == copied.data@
}

// Model navigation: spec::bitmap_prefix [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bitmap_prefix(
    source: Seq<crate::Bitmap>, copied: Seq<crate::Bitmap>,
) -> bool {
    copied.len() <= source.len() && forall|index: int| 0 <= index < copied.len() ==>
        #[trigger] same_bitmap(&source[index], &copied[index])
}

// Model navigation: spec::same_bitmaps [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_bitmaps(
    source: Seq<crate::Bitmap>, copied: Seq<crate::Bitmap>,
) -> bool {
    source.len() == copied.len() && bitmap_prefix(source, copied)
}

pub(super) proof fn bitmap_prefix_valid(source: Seq<crate::Bitmap>, copied: Seq<crate::Bitmap>)
    requires bitmap_prefix(source, copied),
    ensures crate::halftone::halftone_spec::bitmaps_valid(copied),
{
    assert forall|done: int| 0 <= done < copied.len()
        implies crate::spec::bitmap::storage_valid(#[trigger] &copied[done]) by {
        assert(same_bitmap(&source[done], &copied[done]));
    }
}

pub(super) proof fn bitmap_push(
    source: Seq<crate::Bitmap>, copied: Seq<crate::Bitmap>, index: int,
)
    requires
        0 <= index < source.len(),
        copied.len() == index + 1,
        bitmap_prefix(source, copied.take(index)),
        same_bitmap(&source[index], &copied[index]),
    ensures
        bitmap_prefix(source, copied),
        crate::halftone::halftone_spec::bitmaps_valid(copied),
{
    assert forall|done: int| 0 <= done < copied.len()
        implies #[trigger] same_bitmap(&source[done], &copied[done]) by {
        if done < index {
            assert(copied.take(index)[done] == copied[done]);
        }
    }
    assert forall|done: int| 0 <= done < copied.len()
        implies crate::spec::bitmap::storage_valid(#[trigger] &copied[done]) by {
        assert(same_bitmap(&source[done], &copied[done]));
    }
}

// Model navigation: spec::same_symbol [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_symbol(
    source: (u32, crate::decode::SymbolResource),
    copied: (u32, crate::decode::SymbolResource),
) -> bool {
    source.0 == copied.0 && source.1.contexts.is_none() && copied.1.contexts.is_none()
        && same_bitmaps(source.1.exported@, copied.1.exported@)
}

// Model navigation: spec::symbol_prefix [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn symbol_prefix(
    source: Seq<(u32, crate::decode::SymbolResource)>,
    copied: Seq<(u32, crate::decode::SymbolResource)>,
) -> bool {
    copied.len() <= source.len() && forall|index: int| 0 <= index < copied.len() ==>
        #[trigger] same_symbol(source[index], copied[index])
}

pub(super) proof fn symbol_push(
    source: Seq<(u32, crate::decode::SymbolResource)>,
    copied: Seq<(u32, crate::decode::SymbolResource)>,
    index: int,
)
    requires
        0 <= index < source.len(), copied.len() == index + 1,
        symbol_prefix(source, copied.take(index)),
        same_symbol(source[index], copied[index]),
    ensures symbol_prefix(source, copied),
        forall|done: int| 0 <= done < copied.len() ==>
            crate::halftone::halftone_spec::bitmaps_valid(
                #[trigger] copied[done].1.exported@),
{
    assert forall|done: int| 0 <= done < copied.len()
        implies #[trigger] same_symbol(source[done], copied[done]) by {
        if done < index {
            assert(copied.take(index)[done] == copied[done]);
        }
    }
    assert forall|done: int| 0 <= done < copied.len()
        implies crate::halftone::halftone_spec::bitmaps_valid(
            #[trigger] copied[done].1.exported@) by {
        assert(same_symbol(source[done], copied[done]));
        bitmap_prefix_valid(source[done].1.exported@, copied[done].1.exported@);
    }
}

// Model navigation: spec::same_pattern [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_pattern(
    source: (u32, Vec<crate::Bitmap>), copied: (u32, Vec<crate::Bitmap>),
) -> bool {
    source.0 == copied.0 && same_bitmaps(source.1@, copied.1@)
}

// Model navigation: spec::pattern_prefix [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn pattern_prefix(
    source: Seq<(u32, Vec<crate::Bitmap>)>, copied: Seq<(u32, Vec<crate::Bitmap>)>,
) -> bool {
    copied.len() <= source.len() && forall|index: int| 0 <= index < copied.len() ==>
        #[trigger] same_pattern(source[index], copied[index])
}

pub(super) proof fn pattern_push(
    source: Seq<(u32, Vec<crate::Bitmap>)>, copied: Seq<(u32, Vec<crate::Bitmap>)>, index: int,
)
    requires
        0 <= index < source.len(), copied.len() == index + 1,
        pattern_prefix(source, copied.take(index)),
        same_pattern(source[index], copied[index]),
    ensures pattern_prefix(source, copied),
        forall|done: int| 0 <= done < copied.len() ==>
            crate::halftone::halftone_spec::bitmaps_valid(#[trigger] copied[done].1@),
{
    assert forall|done: int| 0 <= done < copied.len()
        implies #[trigger] same_pattern(source[done], copied[done]) by {
        if done < index {
            assert(copied.take(index)[done] == copied[done]);
        }
    }
    assert forall|done: int| 0 <= done < copied.len()
        implies crate::halftone::halftone_spec::bitmaps_valid(#[trigger] copied[done].1@) by {
        assert(same_pattern(source[done], copied[done]));
        bitmap_prefix_valid(source[done].1@, copied[done].1@);
    }
}

// Model navigation: spec::same_resources [project-result-model]
// Purpose: Exact fallible-copy dimensions, stride, every packed byte, resource IDs/order/context absence and valid source/copy frames; monotone bounded observed-allocation metadata.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn same_resources(source: &InitialResources, copied: &InitialResources) -> bool {
    &&& source.tables@.len() == 0 && copied.tables@.len() == 0
    &&& source.symbols@.len() == copied.symbols@.len()
    &&& source.patterns@.len() == copied.patterns@.len()
    &&& symbol_prefix(source.symbols@, copied.symbols@)
    &&& pattern_prefix(source.patterns@, copied.patterns@)
}

pub(super) proof fn finish_resources(source: &InitialResources, copied: &InitialResources)
    requires admission(source).is_none(), same_resources(source, copied),
    ensures decode_spec::initial_resources_valid(copied), admission(copied).is_none(),
{
    assert forall|index: int| 0 <= index < copied.symbols@.len()
        implies copied.symbols@[index].1.contexts.is_none() by {
        assert(same_symbol(source.symbols@[index], copied.symbols@[index]));
    }
    assert forall|index: int| 0 <= index < copied.symbols@.len()
        implies crate::halftone::halftone_spec::bitmaps_valid(
            #[trigger] copied.symbols@[index].1.exported@) by {
        assert(same_symbol(source.symbols@[index], copied.symbols@[index]));
        bitmap_prefix_valid(source.symbols@[index].1.exported@, copied.symbols@[index].1.exported@);
    }
    assert forall|index: int| 0 <= index < copied.patterns@.len()
        implies crate::halftone::halftone_spec::bitmaps_valid(
            #[trigger] copied.patterns@[index].1@) by {
        assert(same_pattern(source.patterns@[index], copied.patterns@[index]));
        bitmap_prefix_valid(source.patterns@[index].1@, copied.patterns@[index].1@);
    }
}

}
