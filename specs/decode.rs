// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::page_number [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn page_number(page: &crate::decode::Page) -> u32 {
    page.number
}

// Model navigation: spec::page_bitmap [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn page_bitmap(page: &crate::decode::Page) -> &crate::Bitmap {
    &page.bitmap
}

// Model navigation: spec::document_info [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_info(
    document: &crate::decode::DecodedDocument,
) -> &crate::DocumentInfo {
    &document.info
}

// Model navigation: spec::document_pages [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_pages(
    document: &crate::decode::DecodedDocument,
) -> Seq<crate::decode::Page> {
    document.pages@
}

// Model navigation: spec::collector_pages [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collector_pages(
    collector: &crate::decode::PageCollector,
) -> Seq<crate::decode::Page> {
    collector.pages@
}

// Model navigation: spec::collector_bitmap_bytes [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collector_bitmap_bytes(
    collector: &crate::decode::PageCollector,
) -> u64 {
    collector.bitmap_bytes
}

// Model navigation: spec::collector_memory_limit [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collector_memory_limit(
    collector: &crate::decode::PageCollector,
) -> u64 {
    collector.memory_limit
}

// Model navigation: spec::collector_decoded_limit [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collector_decoded_limit(
    collector: &crate::decode::PageCollector,
) -> u64 {
    collector.decoded_limit
}

// Model navigation: spec::collector_result [project-result-model]
// Purpose: Successful owned-page append and bitmap-byte accounting, or unchanged page state on error.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn collector_result(
    before: &crate::decode::PageCollector,
    after: &crate::decode::PageCollector,
    page: crate::decode::Page,
    result: Result<(), crate::DecodeError>,
) -> bool {
    match result {
        Ok(()) =>
            collector_bitmap_bytes(after) as nat
                == collector_bitmap_bytes(before) as nat
                    + bitmap_byte_count(page_bitmap(&page))
            && collector_bitmap_bytes(after) <= collector_decoded_limit(after)
            && collector_bitmap_bytes(after) <= collector_memory_limit(after)
            && after.allocated_bitmap_bytes <= collector_memory_limit(after)
            && collector_pages(after) == collector_pages(before).push(page),
        Err(_) =>
            collector_bitmap_bytes(after) == collector_bitmap_bytes(before)
            && after.allocated_bitmap_bytes == before.allocated_bitmap_bytes
            && collector_pages(after) == collector_pages(before),
    }
}

// A checked bridge exposes the closed policy without publishing private field projections.
pub(super) proof fn collector_result_definition()
    ensures
        forall|before: &crate::decode::PageCollector,
               after: &crate::decode::PageCollector,
               page: crate::decode::Page,
               result: Result<(), crate::DecodeError>|
            #[trigger] collector_result(before, after, page, result)
                == match result {
                    Ok(()) =>
                        collector_bitmap_bytes(after) as nat
                            == collector_bitmap_bytes(before) as nat
                                + bitmap_byte_count(page_bitmap(&page))
                        && collector_bitmap_bytes(after) <= collector_decoded_limit(after)
                        && collector_bitmap_bytes(after) <= collector_memory_limit(after)
                        && after.allocated_bitmap_bytes <= collector_memory_limit(after)
                        && collector_pages(after) == collector_pages(before).push(page),
                    Err(_) =>
                        collector_bitmap_bytes(after) == collector_bitmap_bytes(before)
                        && after.allocated_bitmap_bytes == before.allocated_bitmap_bytes
                        && collector_pages(after) == collector_pages(before),
                },
{
}

// Model navigation: spec::retained_bytes [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn retained_bytes(budget: &crate::decode::RetainedBudget) -> u64 {
    budget.bytes
}

// Model navigation: spec::work_used [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn work_used(work: &crate::decode::DecodeWork) -> u64 {
    work.used
}

// Model navigation: spec::runtime_last_stripe_end [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn runtime_last_stripe_end(
    page: &crate::decode::RuntimePage,
) -> Option<u32> {
    page.last_stripe_end
}

// Model navigation: spec::runtime_open_stripe_max_row [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn runtime_open_stripe_max_row(
    page: &crate::decode::RuntimePage,
) -> Option<u32> {
    page.open_stripe_max_row
}

// Model navigation: spec::runtime_pages_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Audit: runtime-pages-valid
// REQUIREMENT CONTEXT: T88-IMMEDIATE-DIRECT-PAGE | ITU-T T.88 (08/2018) | 8.2 step 5a
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn runtime_pages_valid(
    pages: &crate::decode::RuntimePages,
) -> bool {
    forall|index: int| 0 <= index < pages@.len() ==>
        crate::spec::bitmap::storage_valid(&pages@[index].1.bitmap)
}

// Model navigation: spec::symbol_store_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn symbol_store_valid(
    symbols: &crate::decode::SymbolStore,
) -> bool {
    forall|index: int| 0 <= index < symbols@.len()
        ==> crate::halftone::halftone_spec::bitmaps_valid(
            #[trigger] symbols@[index].1.exported@
        )
}

// Model navigation: spec::bitmap_refs_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bitmap_refs_valid(
    bitmaps: Seq<&crate::bitmap::Bitmap>,
) -> bool {
    forall|index: int| 0 <= index < bitmaps.len()
        ==> crate::spec::bitmap::storage_valid(#[trigger] bitmaps[index])
}

// Model navigation: spec::collected_symbols_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collected_symbols_valid(
    symbols: &crate::decode::CollectedSymbols,
) -> bool {
    match symbols {
        crate::decode::CollectedSymbols::Contiguous(bitmaps) =>
            crate::halftone::halftone_spec::bitmaps_valid(bitmaps@),
        crate::decode::CollectedSymbols::Borrowed(bitmaps) =>
            bitmap_refs_valid(bitmaps@),
    }
}

// Model navigation: spec::collected_symbol_count [project-result-model]
// Purpose: First matching export group, referred counts and collection length results.
// Audit: collected-symbol-count
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn collected_symbol_count(
    symbols: &crate::decode::CollectedSymbols,
) -> nat {
    match symbols {
        crate::decode::CollectedSymbols::Contiguous(bitmaps) => bitmaps@.len(),
        crate::decode::CollectedSymbols::Borrowed(bitmaps) => bitmaps@.len(),
    }
}

// Model navigation: spec::first_symbol_index [project-result-model]
// Purpose: First matching export group, referred counts and collection length results.
// Audit: first-symbol-store-index
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn first_symbol_index(
    symbols: Seq<(u32, crate::decode::SymbolResource)>,
    number: u32,
    index: int,
) -> bool {
    0 <= index < symbols.len()
        && symbols[index].0 == number
        && forall|previous: int| 0 <= previous < index ==>
            #[trigger] symbols[previous].0 != number
}

// T.88 (08/2018) 7.4.2.2 steps 3-4: select the last dictionary,
// not the last dictionary that happens to have retained a bank.
pub(super) open spec fn dictionary_context_source(
    symbols: Seq<(u32, crate::decode::SymbolResource)>,
    referred: Seq<u32>,
    context_used: bool,
    source: Option<usize>,
) -> bool {
    if !context_used {
        source.is_none()
    } else {
        match source {
            Some(index) =>
                0 <= index < symbols.len()
                && exists|reference: int| 0 <= reference < referred.len()
                    && first_symbol_index(symbols, referred[reference], index as int)
                    && forall|later: int, entry: int|
                        reference < later < referred.len()
                            && 0 <= entry < symbols.len() ==>
                            symbols[entry].0 != referred[later],
            None => forall|reference: int, entry: int|
                0 <= reference < referred.len() && 0 <= entry < symbols.len() ==>
                    symbols[entry].0 != referred[reference],
        }
    }
}

// Model navigation: spec::symbol_group_count [project-result-model]
// Purpose: First matching export group, referred counts and collection length results.
// Audit: symbol-resource-group-count
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn symbol_group_count(
    symbols: Seq<(u32, crate::decode::SymbolResource)>,
    number: u32,
) -> nat {
    if exists|index: int| first_symbol_index(symbols, number, index) {
        let index = choose|index: int| first_symbol_index(symbols, number, index);
        symbols[index].1.exported@.len()
    } else {
        0
    }
}

// Model navigation: spec::referred_symbol_count [project-result-model]
// Purpose: First matching export group, referred counts and collection length results.
// Audit: referred-symbol-count
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn referred_symbol_count(
    symbols: Seq<(u32, crate::decode::SymbolResource)>,
    references: Seq<u32>,
    count: nat,
) -> nat
    recommends count <= references.len(),
    decreases count,
{
    if count == 0 {
        0
    } else {
        referred_symbol_count(symbols, references, (count - 1) as nat)
            + symbol_group_count(symbols, references[count - 1])
    }
}

// Model navigation: spec::pattern_store_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn pattern_store_valid(
    patterns: &crate::decode::PatternStore,
) -> bool {
    forall|index: int| 0 <= index < patterns@.len()
        ==> crate::halftone::halftone_spec::bitmaps_valid(
            #[trigger] patterns@[index].1@
        )
}

// Model navigation: spec::region_buffers_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Audit: region-buffers-valid
// REQUIREMENT CONTEXT: T88-INTERMEDIATE-AUXILIARY-STORAGE | ITU-T T.88 (08/2018) | 8.2 step 5b
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn region_buffers_valid(
    buffers: &crate::decode::RegionBuffers,
) -> bool {
    forall|index: int| 0 <= index < buffers@.len()
        ==> crate::spec::bitmap::storage_valid(
            #[trigger] &buffers@[index].1.bitmap
        )
}

// Model navigation: spec::initial_resources_valid [representation-invariant]
// Purpose: Bitmap storage invariants across runtime stores, symbols and resources.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn initial_resources_valid(
    resources: &crate::decode::InitialResources,
) -> bool {
    pattern_store_valid(&resources.patterns)
        && symbol_store_valid(&resources.symbols)
}

// Model navigation: spec::checked_sum_fits [project-policy]
// Purpose: Checked size/addition limits and retained-byte accounting policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn checked_sum_fits(left: u64, right: u64) -> bool {
    left as nat + right as nat <= u64::MAX as nat
}

// Model navigation: spec::charge_succeeds [project-policy]
// Purpose: Checked size/addition limits and retained-byte accounting policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn charge_succeeds(used: u64, units: u64, limit: u64) -> bool {
    checked_sum_fits(used, units) && used as nat + units as nat <= limit as nat
}

// Model navigation: spec::release_succeeds [project-policy]
// Purpose: Checked size/addition limits and retained-byte accounting policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn release_succeeds(retained: u64, bytes: u64) -> bool {
    bytes <= retained
}

// Model navigation: spec::generic_template_code [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_template_code(
    template: crate::generic_region::GenericTemplate,
) -> nat {
    match template {
        crate::generic_region::GenericTemplate::Template0 => 0,
        crate::generic_region::GenericTemplate::Template1 => 1,
        crate::generic_region::GenericTemplate::Template2 => 2,
        crate::generic_region::GenericTemplate::Template3 => 3,
    }
}

// Model navigation: spec::allocation_bytes [project-policy]
// Purpose: Checked size/addition limits and retained-byte accounting policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn allocation_bytes<T>(count: usize) -> nat {
    count as nat * core::mem::size_of::<T>() as nat
}

// Model navigation: spec::bitmap_byte_count [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn bitmap_byte_count(bitmap: &crate::Bitmap) -> nat {
    crate::views::bitmap_data(bitmap).len()
}

// Model navigation: spec::region_intersects_page [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Audit: region-intersects-page
// REQUIREMENT CONTEXT: T88-IMMEDIATE-SEALED-STRIPE | ITU-T T.88 (08/2018) | 7.4.10; 8.2
// Fidelity: unresolved; no independent normative approval.
pub open spec fn region_intersects_page(
    region_width: u32,
    region_height: u32,
    region_x: u32,
    region_y: u32,
    page_width: u32,
    page_height: u32,
) -> bool {
    region_width != 0
        && region_height != 0
        && region_x < page_width
        && region_y < page_height
}

// Model navigation: spec::dispatch_class [proof-support]
// Purpose: Private decode/resource projections, dispatch codes and intersection vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn dispatch_class(segment_type: crate::segment_header::SegmentType) -> nat {
    match segment_type {
        crate::segment_header::SegmentType::SymbolDictionary => 0,
        crate::segment_header::SegmentType::IntermediateTextRegion
        | crate::segment_header::SegmentType::ImmediateTextRegion
        | crate::segment_header::SegmentType::ImmediateLosslessTextRegion => 1,
        crate::segment_header::SegmentType::PatternDictionary => 2,
        crate::segment_header::SegmentType::IntermediateHalftoneRegion
        | crate::segment_header::SegmentType::ImmediateHalftoneRegion
        | crate::segment_header::SegmentType::ImmediateLosslessHalftoneRegion => 3,
        crate::segment_header::SegmentType::IntermediateGenericRegion
        | crate::segment_header::SegmentType::ImmediateGenericRegion
        | crate::segment_header::SegmentType::ImmediateLosslessGenericRegion => 4,
        crate::segment_header::SegmentType::IntermediateGenericRefinementRegion
        | crate::segment_header::SegmentType::ImmediateGenericRefinementRegion
        | crate::segment_header::SegmentType::ImmediateLosslessGenericRefinementRegion => 5,
        crate::segment_header::SegmentType::PageInformation => 6,
        crate::segment_header::SegmentType::EndOfPage => 7,
        crate::segment_header::SegmentType::EndOfStripe => 8,
        crate::segment_header::SegmentType::EndOfFile => 9,
        crate::segment_header::SegmentType::Profiles => 10,
        crate::segment_header::SegmentType::Tables => 11,
        crate::segment_header::SegmentType::ColourPalette => 12,
        crate::segment_header::SegmentType::Extension => 13,
    }
}

}

// Preserve the logical owner and private-field access of the model bridges.
include!("../proofs/model_support/decode.rs");
