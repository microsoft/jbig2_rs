// Concrete staged reference classification, not independent normative approval.
use super::*;

verus! {

// Model navigation: spec::catalog_first_index [proof-support]
// Purpose: Exact first-match lookup on arbitrary duplicate-key catalogs.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn catalog_first_index<T>(entries: Seq<(u32, T)>, number: u32) -> Option<int> {
    if exists|index: int| 0 <= index < entries.len() && entries[index].0 == number {
        Some(choose|index: int| 0 <= index < entries.len()
            && entries[index].0 == number
            && forall|prior: int| 0 <= prior < index ==> entries[prior].0 != number)
    } else {
        None
    }
}

pub(crate) proof fn catalog_first_index_value<T>(
    entries: Seq<(u32, T)>, number: u32, index: int,
)
    requires
        0 <= index < entries.len(), entries[index].0 == number,
        forall|prior: int| 0 <= prior < index ==> entries[prior].0 != number,
    ensures catalog_first_index(entries, number) == Some(index),
{
    let selected = choose|selected: int| 0 <= selected < entries.len()
        && entries[selected].0 == number
        && forall|prior: int| 0 <= prior < selected ==> entries[prior].0 != number;
    assert(selected == index);
}

// Model navigation: spec::resolved_reference_type [project-result-model]
// Purpose: First local index overrides global types; out-of-range and unresolved are absent.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn resolved_reference_type(
    references: Seq<u32>, reference_index: nat,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>, globals: Option<&crate::document::GlobalSegments>,
) -> Option<crate::segment_header::SegmentType> {
    if reference_index >= references.len() {
        None
    } else {
        let number = references[reference_index as int];
        match catalog_first_index(indexes, number) {
            Some(entry) => Some(segments[indexes[entry].1 as int].header.segment_type),
            None => match globals {
                Some(state) => match catalog_first_index(global_types(state), number) {
                    Some(entry) => Some(global_types(state)[entry].1),
                    None => None,
                },
                None => None,
            },
        }
    }
}

// Model navigation: spec::resolved_intermediate_reference [project-result-model]
// Purpose: First resolved reference belongs to the existing four intermediate classes.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn resolved_intermediate_reference(
    references: Seq<u32>, segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>, globals: Option<&crate::document::GlobalSegments>,
) -> bool {
    match resolved_reference_type(references, 0, segments, indexes, globals) {
        Some(kind) => production_is_intermediate_region(kind),
        None => false,
    }
}

// Model navigation: spec::resolved_table_count [proof-support]
// Purpose: Count resolved table references in a concrete prefix, including duplicates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn resolved_table_count(
    references: Seq<u32>, segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>, globals: Option<&crate::document::GlobalSegments>, prefix: nat,
) -> nat
    recommends prefix <= references.len(),
    decreases prefix,
{
    if prefix == 0 { 0 } else {
        resolved_table_count(references, segments, indexes, globals, (prefix - 1) as nat)
            + if resolved_reference_type(references, (prefix - 1) as nat, segments, indexes, globals)
                == Some(crate::segment_header::SegmentType::Tables) { 1nat } else { 0nat }
    }
}

pub(crate) proof fn resolved_table_count_monotone(
    references: Seq<u32>, segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>, globals: Option<&crate::document::GlobalSegments>, low: nat, high: nat,
)
    requires low <= high <= references.len(),
    ensures resolved_table_count(references, segments, indexes, globals, low)
        <= resolved_table_count(references, segments, indexes, globals, high),
    decreases high - low,
{
    if low < high {
        resolved_table_count_monotone(references, segments, indexes, globals, low, (high - 1) as nat);
    }
}

// Model navigation: spec::resolved_symbol_resources [project-result-model]
// Purpose: Resolved kinds and table limit; missing references are owned by the preceding stage.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn resolved_symbol_resources(
    references: Seq<u32>, segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>, globals: Option<&crate::document::GlobalSegments>, maximum_tables: nat,
) -> bool {
    &&& resolved_table_count(references, segments, indexes, globals, references.len()) <= maximum_tables
    &&& forall|index: int| 0 <= index < references.len() ==> {
        match #[trigger] resolved_reference_type(references, index as nat, segments, indexes, globals) {
            Some(kind) => kind == crate::segment_header::SegmentType::SymbolDictionary
                || kind == crate::segment_header::SegmentType::Tables,
            None => true,
        }
    }
}

// Model navigation: spec::reference_types_accepted [project-result-model]
// Purpose: Exact concrete staged type/class acceptance, not full reference validity.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn reference_types_accepted(
    header: &crate::segment_header::SegmentHeader,
    segments: Seq<crate::document::ParsedSegment<'_>>, indexes: Seq<(u32, usize)>,
    globals: Option<&crate::document::GlobalSegments>,
) -> bool {
    use crate::segment_header::SegmentType::*;
    match header.segment_type {
        SymbolDictionary => resolved_symbol_resources(header.referred_segments@, segments, indexes, globals, 4),
        IntermediateTextRegion | ImmediateTextRegion | ImmediateLosslessTextRegion =>
            resolved_symbol_resources(header.referred_segments@, segments, indexes, globals, 8),
        IntermediateHalftoneRegion | ImmediateHalftoneRegion | ImmediateLosslessHalftoneRegion =>
            header.referred_segments@.len() == 1
                && resolved_reference_type(header.referred_segments@, 0, segments, indexes, globals)
                    == Some(PatternDictionary),
        IntermediateGenericRefinementRegion =>
            header.referred_segments@.len() == 1
                && resolved_intermediate_reference(header.referred_segments@, segments, indexes, globals),
        ImmediateGenericRefinementRegion | ImmediateLosslessGenericRefinementRegion =>
            header.referred_segments@.len() == 0
                || (header.referred_segments@.len() == 1
                    && resolved_intermediate_reference(header.referred_segments@, segments, indexes, globals)),
        Extension => true,
        _ => header.referred_segments@.len() == 0,
    }
}

}
