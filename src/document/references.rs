use super::{
    allocation_size, keyed_get, keyed_get_mut, keyed_index, keyed_insert, DecodeError,
    GlobalSegments, LimitExceeded, LimitKind, Limits, MalformedError, ParsedSegment, SegmentDepths,
    SegmentHeader, SegmentIndexes, SegmentType, SegmentTypes,
};

#[cfg(verus_keep_ghost)]
use super::{document_shared, document_spec};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(catalogs) =>
            catalogs.0@.len() == segments@.len()
            && catalogs.1@.len() == segments@.len()
            && document_spec::reference_total(segments@, segments@.len())
                <= limits.max_total_references
            && (forall|index: int| 0 <= index < segments@.len() ==>
                catalogs.0@[index].0 == segments@[index].header.number
                && catalogs.0@[index].1 == segments@[index].header.segment_type)
            && (forall|entry: int| 0 <= entry < catalogs.1@.len() ==>
                1 <= (#[trigger] catalogs.1@[entry]).1 <= limits.max_reference_graph_depth),
        Err(_) => true,
    },
))]
pub(super) fn validate_references(
    segments: &[ParsedSegment<'_>],
    globals: Option<&GlobalSegments>,
    limits: &Limits,
) -> Result<(SegmentTypes, SegmentDepths), DecodeError> {
    let total_references = count_total_references(segments)?;
    if total_references > limits.max_total_references {
        return Err(LimitExceeded {
            kind: LimitKind::TotalReferences,
            limit: limits.max_total_references,
            actual: total_references,
        }
        .into());
    }

    let mut indexes: SegmentIndexes = Vec::new();
    let index_allocation = allocation_size::<(u32, usize)>(segments.len())?;
    if indexes.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: index_allocation,
        });
    }
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            segment_index <= segments@.len(),
            document_spec::reference_total(segments@, segments@.len())
                <= limits.max_total_references,
            forall|entry: int| 0 <= entry < indexes@.len() ==>
                indexes@[entry].1 < segments@.len(),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        let global_duplicate = match globals {
            Some(state) => keyed_index(&state.types, segment.header.number).is_some(),
            None => false,
        };
        if global_duplicate {
            return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
        }
        if !keyed_insert(&mut indexes, segment.header.number, segment_index) {
            return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
        }
        segment_index += 1;
    }

    validate_reference_links(segments, &indexes, globals)?;
    validate_retention(segments, &indexes)?;
    let depths = validate_reference_depth(segments, &indexes, globals, limits)?;
    let mut types: SegmentTypes = Vec::new();
    let type_allocation = allocation_size::<(u32, SegmentType)>(segments.len())?;
    if types.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: type_allocation,
        });
    }
    let mut type_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            type_index <= segments@.len(),
            types@.len() == type_index,
            depths@.len() == segments@.len(),
            document_spec::reference_total(segments@, segments@.len())
                <= limits.max_total_references,
            forall|index: int| 0 <= index < types@.len() ==>
                types@[index].0 == segments@[index].header.number
                && types@[index].1 == segments@[index].header.segment_type,
            forall|index: int| 0 <= index < depths@.len() ==>
                1 <= (#[trigger] depths@[index]).1 <= limits.max_reference_graph_depth,
        decreases segments@.len() - type_index,
    ))]
    while type_index < segments.len() {
        let segment = &segments[type_index];
        if !keyed_insert(
            &mut types,
            segment.header.number,
            segment.header.segment_type,
        ) {
            return Err(MalformedError::DuplicateSegmentNumber(segment.header.number).into());
        }
        type_index += 1;
    }
    Ok((types, depths))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(total) => total as nat == document_spec::reference_total(segments@, segments@.len()),
        Err(error) =>
            document_spec::reference_total(segments@, segments@.len()) > u64::MAX
            && error == DecodeError::Malformed(MalformedError::IntegerOverflow),
    },
))]
pub(super) fn count_total_references(segments: &[ParsedSegment<'_>]) -> Result<u64, DecodeError> {
    let mut total = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= segments@.len(),
            total as nat == document_spec::reference_total(segments@, index as nat),
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            document_spec::reference_total_monotone(segments@, index as nat + 1, segments@.len());
            reveal_with_fuel(document_spec::reference_total, 1);
        }
        let Ok(count) = u64::try_from(segments[index].header.referred_segments.len()) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        let Some(next) = total.checked_add(count) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        total = next;
        index += 1;
    }
    Ok(total)
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires forall|entry: int| 0 <= entry < indexes@.len() ==>
        indexes@[entry].1 < segments@.len(),
))]
pub(super) fn validate_reference_links(
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
) -> Result<(), DecodeError> {
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            segment_index <= segments@.len(),
            forall|entry: int| 0 <= entry < indexes@.len() ==>
                indexes@[entry].1 < segments@.len(),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        let mut reference_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                reference_index <= segment.header.referred_segments@.len(),
                forall|entry: int| 0 <= entry < indexes@.len() ==>
                    indexes@[entry].1 < segments@.len(),
            decreases segment.header.referred_segments@.len() - reference_index,
        ))]
        while reference_index < segment.header.referred_segments.len() {
            let referred = segment.header.referred_segments[reference_index];
            let Some(entry) = keyed_index(indexes, referred) else {
                let global_exists = match globals {
                    Some(state) => keyed_index(&state.types, referred).is_some(),
                    None => false,
                };
                if !global_exists {
                    return Err(MalformedError::MissingReferredSegment {
                        segment: segment.header.number,
                        referred,
                    }
                    .into());
                }
                reference_index += 1;
                continue;
            };
            let target_index = indexes[entry].1;
            let target = &segments[target_index].header;
            if target.page_association != 0
                && target.page_association != segment.header.page_association
            {
                return Err(MalformedError::InvalidPageAssociation {
                    segment: segment.header.number,
                    page: segment.header.page_association,
                }
                .into());
            }
            reference_index += 1;
        }
        validate_reference_types(&segment.header, segments, indexes, globals)?;
        segment_index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn validate_retention(
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
) -> Result<(), DecodeError> {
    let mut last_references: Vec<(u32, u32)> = Vec::new();
    let retention_allocation = allocation_size::<(u32, u32)>(segments.len())?;
    if last_references.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: retention_allocation,
        });
    }
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant segment_index <= segments@.len(),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        let mut reference_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant reference_index <= segment.header.referred_segments@.len(),
            decreases segment.header.referred_segments@.len() - reference_index,
        ))]
        while reference_index < segment.header.referred_segments.len() {
            let referred = segment.header.referred_segments[reference_index];
            if keyed_index(indexes, referred).is_none() {
                reference_index += 1;
                continue;
            }
            if let Some(last) = keyed_get_mut(&mut last_references, referred) {
                *last = (*last).max(segment.header.number);
            } else {
                let inserted = keyed_insert(&mut last_references, referred, segment.header.number);
                if !inserted {
                    return Err(MalformedError::DuplicateSegmentNumber(referred).into());
                }
            }
            reference_index += 1;
        }
        segment_index += 1;
    }

    validate_retention_flags(segments, &last_references)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn validate_retention_flags(
    segments: &[ParsedSegment<'_>],
    last_references: &[(u32, u32)],
) -> Result<(), DecodeError> {
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant segment_index <= segments@.len(),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        if segment.header.retention_flags.values.is_empty() {
            return Err(MalformedError::InvalidRetentionFlags {
                segment: segment.header.number,
                referred: segment.header.number,
            }
            .into());
        }
        if let Some(last) = keyed_get(last_references, segment.header.number) {
            if !segment.header.retention_flags.retain_this_segment() {
                return Err(MalformedError::InvalidRetentionFlags {
                    segment: segment.header.number,
                    referred: segment.header.number,
                }
                .into());
            }
            if *last <= segment.header.number {
                return Err(MalformedError::InvalidRetentionFlags {
                    segment: segment.header.number,
                    referred: segment.header.number,
                }
                .into());
            }
        }
        let mut reference_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant reference_index <= segment.header.referred_segments@.len(),
            decreases segment.header.referred_segments@.len() - reference_index,
        ))]
        while reference_index < segment.header.referred_segments.len() {
            let referred = segment.header.referred_segments[reference_index];
            let Some(last) = keyed_get(last_references, referred) else {
                reference_index += 1;
                continue;
            };
            if segment
                .header
                .retention_flags
                .retain_referred_segment(reference_index)
                == Some(false)
                && *last != segment.header.number
            {
                return Err(MalformedError::InvalidRetentionFlags {
                    segment: segment.header.number,
                    referred,
                }
                .into());
            }
            reference_index += 1;
        }
        segment_index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires forall|entry: int| 0 <= entry < indexes@.len() ==>
        indexes@[entry].1 < segments@.len(),
    ensures match out {
        Ok(depths) =>
            depths@.len() == segments@.len()
            && forall|entry: int| 0 <= entry < depths@.len() ==>
                1 <= (#[trigger] depths@[entry]).1 <= limits.max_reference_graph_depth,
        Err(_) => true,
    },
))]
pub(super) fn validate_reference_depth(
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
    limits: &Limits,
) -> Result<SegmentDepths, DecodeError> {
    let ordered_allocation = allocation_size::<(u32, usize)>(segments.len())?;
    let mut ordered: Vec<(u32, usize)> = Vec::new();
    if ordered.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: ordered_allocation,
        });
    }
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            segment_index <= segments@.len(),
            ordered@.len() == segment_index,
            forall|entry: int| 0 <= entry < ordered@.len() ==>
                ordered@[entry].1 < segments@.len(),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let number = segments[segment_index].header.number;
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                position <= ordered@.len(),
                ordered@.len() == segment_index,
                forall|entry: int| 0 <= entry < ordered@.len() ==>
                    ordered@[entry].1 < segments@.len(),
            decreases ordered@.len() - position,
        ))]
        while position < ordered.len() && ordered[position].0 < number {
            position += 1;
        }
        ordered.insert(position, (number, segment_index));
        segment_index += 1;
    }

    let mut depths = Vec::new();
    let depth_allocation = allocation_size::<(u32, u32)>(segments.len())?;
    if depths.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: depth_allocation,
        });
    }
    validate_reference_depth_ordered(&ordered, segments, indexes, globals, limits, &mut depths)?;
    Ok(depths)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        forall|entry: int| 0 <= entry < ordered@.len() ==>
            ordered@[entry].1 < segments@.len(),
        forall|entry: int| 0 <= entry < indexes@.len() ==>
            indexes@[entry].1 < segments@.len(),
    ensures
        old(depths)@.len() <= final(depths)@.len()
            <= old(depths)@.len() + ordered@.len(),
        forall|entry: int| 0 <= entry < old(depths)@.len() ==>
            final(depths)@[entry] == old(depths)@[entry],
        forall|entry: int| old(depths)@.len() <= entry < final(depths)@.len() ==>
            1 <= (#[trigger] final(depths)@[entry]).1 <= limits.max_reference_graph_depth,
        out.is_ok() ==> final(depths)@.len() == old(depths)@.len() + ordered@.len(),
))]
#[allow(clippy::manual_map)]
pub(super) fn validate_reference_depth_ordered(
    ordered: &[(u32, usize)],
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
    limits: &Limits,
    depths: &mut SegmentDepths,
) -> Result<(), DecodeError> {
    let mut ordered_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            ordered_index <= ordered@.len(),
            depths@.len() == old(depths)@.len() + ordered_index,
            forall|entry: int| 0 <= entry < old(depths)@.len() ==>
                depths@[entry] == old(depths)@[entry],
            forall|entry: int| old(depths)@.len() <= entry < depths@.len() ==>
                1 <= (#[trigger] depths@[entry]).1 <= limits.max_reference_graph_depth,
            forall|entry: int| 0 <= entry < ordered@.len() ==>
                ordered@[entry].1 < segments@.len(),
            forall|entry: int| 0 <= entry < indexes@.len() ==>
                indexes@[entry].1 < segments@.len(),
        decreases ordered@.len() - ordered_index,
    ))]
    while ordered_index < ordered.len() {
        let index = ordered[ordered_index].1;
        let segment = &segments[index].header;
        let mut depth = 1_u32;
        let mut reference_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                reference_index <= segment.referred_segments@.len(),
                1 <= depth,
                ordered_index <= ordered@.len(),
                depths@.len() == old(depths)@.len() + ordered_index,
                forall|entry: int| 0 <= entry < old(depths)@.len() ==>
                    depths@[entry] == old(depths)@[entry],
                forall|entry: int| old(depths)@.len() <= entry < depths@.len() ==>
                    1 <= (#[trigger] depths@[entry]).1 <= limits.max_reference_graph_depth,
                forall|entry: int| 0 <= entry < indexes@.len() ==>
                    indexes@[entry].1 < segments@.len(),
            decreases segment.referred_segments@.len() - reference_index,
        ))]
        while reference_index < segment.referred_segments.len() {
            let referred = segment.referred_segments[reference_index];
            let referred_depth = if keyed_index(indexes, referred).is_some() {
                match keyed_get(depths, referred) {
                    Some(value) => Some(*value),
                    None => None,
                }
            } else {
                match globals {
                    Some(state) => match keyed_get(&state.depths, referred) {
                        Some(value) => Some(*value),
                        None => None,
                    },
                    None => None,
                }
            };
            let Some(referred_depth) = referred_depth else {
                return Err(MalformedError::MissingReferredSegment {
                    segment: segment.number,
                    referred,
                }
                .into());
            };
            let Some(candidate) = referred_depth.checked_add(1) else {
                return Err(MalformedError::IntegerOverflow.into());
            };
            if candidate > depth {
                depth = candidate;
            }
            reference_index += 1;
        }
        if depth > limits.max_reference_graph_depth {
            return Err(LimitExceeded {
                kind: LimitKind::ReferenceDepth,
                limit: u64::from(limits.max_reference_graph_depth),
                actual: u64::from(depth),
            }
            .into());
        }
        if !keyed_insert(depths, segment.number, depth) {
            return Err(MalformedError::DuplicateSegmentNumber(segment.number).into());
        }
        ordered_index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires forall|index: int| 0 <= index < indexes@.len() ==>
        indexes@[index].1 < segments@.len(),
    ensures out == if document_spec::reference_types_accepted(header, segments@, indexes@, globals) {
        Ok(())
    } else {
        Err(DecodeError::Malformed(MalformedError::InvalidSegmentReferences(header.number)))
    },
))]
pub(super) fn validate_reference_types(
    header: &SegmentHeader,
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
) -> Result<(), DecodeError> {
    let reference_count = header.referred_segments.len();
    let valid = match header.segment_type {
        SegmentType::SymbolDictionary => {
            references_are_symbol_resources(header, segments, indexes, globals, 4)
        }
        SegmentType::IntermediateTextRegion
        | SegmentType::ImmediateTextRegion
        | SegmentType::ImmediateLosslessTextRegion => {
            references_are_symbol_resources(header, segments, indexes, globals, 8)
        }
        SegmentType::PatternDictionary
        | SegmentType::IntermediateGenericRegion
        | SegmentType::ImmediateGenericRegion
        | SegmentType::ImmediateLosslessGenericRegion
        | SegmentType::PageInformation
        | SegmentType::EndOfPage
        | SegmentType::EndOfStripe
        | SegmentType::EndOfFile
        | SegmentType::Profiles
        | SegmentType::Tables
        | SegmentType::ColourPalette => reference_count == 0,
        SegmentType::IntermediateHalftoneRegion
        | SegmentType::ImmediateHalftoneRegion
        | SegmentType::ImmediateLosslessHalftoneRegion => {
            reference_count == 1
                && matches!(
                    known_reference_type(header, 0, segments, indexes, globals),
                    Some(SegmentType::PatternDictionary)
                )
        }
        SegmentType::IntermediateGenericRefinementRegion => {
            reference_count == 1
                && known_reference_is_intermediate(header, segments, indexes, globals)
        }
        SegmentType::ImmediateGenericRefinementRegion
        | SegmentType::ImmediateLosslessGenericRefinementRegion => {
            reference_count == 0
                || (reference_count == 1
                    && known_reference_is_intermediate(header, segments, indexes, globals))
        }
        SegmentType::Extension => true,
    };
    if !valid {
        return Err(MalformedError::InvalidSegmentReferences(header.number).into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires forall|index: int| 0 <= index < indexes@.len() ==>
        indexes@[index].1 < segments@.len(),
    ensures out == document_spec::resolved_reference_type(
        header.referred_segments@, reference_index as nat, segments@, indexes@, globals,
    ),
))]
pub(super) fn known_reference_type(
    header: &SegmentHeader,
    reference_index: usize,
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
) -> Option<SegmentType> {
    if reference_index >= header.referred_segments.len() {
        return None;
    }
    let number = header.referred_segments[reference_index];
    if let Some(entry) = keyed_index(indexes, number) {
        #[cfg(verus_keep_ghost)]
        proof! { document_spec::catalog_first_index_value(indexes@, number, entry as int); }
        let segment_index = indexes[entry].1;
        return Some(segments[segment_index].header.segment_type);
    }
    let state = globals?;
    #[cfg(verus_keep_ghost)]
    proof! { document_spec::global_fields(state); }
    let kind = keyed_get(&state.types, number)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        let entry = choose|entry: int| 0 <= entry < state.types@.len()
            && state.types@[entry].0 == number && *kind == state.types@[entry].1
            && forall|prior: int| 0 <= prior < entry ==> state.types@[prior].0 != number;
        document_spec::catalog_first_index_value(state.types@, number, entry);
    }
    Some(*kind)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires forall|index: int| 0 <= index < indexes@.len() ==>
        indexes@[index].1 < segments@.len(),
    ensures out == document_spec::resolved_intermediate_reference(
        header.referred_segments@, segments@, indexes@, globals,
    ),
))]
pub(super) fn known_reference_is_intermediate(
    header: &SegmentHeader,
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
) -> bool {
    match known_reference_type(header, 0, segments, indexes, globals) {
        Some(kind) => is_intermediate_region(kind),
        None => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires forall|entry: int| 0 <= entry < indexes@.len() ==>
        indexes@[entry].1 < segments@.len(),
    ensures out == document_spec::resolved_symbol_resources(
        header.referred_segments@, segments@, indexes@, globals, maximum_tables as nat,
    ),
))]
pub(super) fn references_are_symbol_resources(
    header: &SegmentHeader,
    segments: &[ParsedSegment<'_>],
    indexes: &SegmentIndexes,
    globals: Option<&GlobalSegments>,
    maximum_tables: usize,
) -> bool {
    let mut table_count = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= header.referred_segments@.len(),
            table_count <= index,
            table_count <= maximum_tables,
            table_count as nat == document_spec::resolved_table_count(
                header.referred_segments@, segments@, indexes@, globals, index as nat,
            ),
            forall|prior: int| 0 <= prior < index ==> match
                #[trigger] document_spec::resolved_reference_type(
                    header.referred_segments@, prior as nat, segments@, indexes@, globals,
                ) {
                    Some(kind) => kind == SegmentType::SymbolDictionary || kind == SegmentType::Tables,
                    None => true,
                },
            forall|entry: int| 0 <= entry < indexes@.len() ==>
                indexes@[entry].1 < segments@.len(),
        decreases header.referred_segments@.len() - index,
    ))]
    while index < header.referred_segments.len() {
        #[cfg(verus_keep_ghost)]
        proof! {
            document_spec::resolved_table_count_monotone(
                header.referred_segments@, segments@, indexes@, globals,
                index as nat + 1, header.referred_segments@.len(),
            );
            reveal_with_fuel(document_spec::resolved_table_count, 1);
        }
        if let Some(kind) = known_reference_type(header, index, segments, indexes, globals) {
            match kind {
                SegmentType::SymbolDictionary => {}
                SegmentType::Tables => {
                    table_count += 1;
                    if table_count > maximum_tables {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        index += 1;
    }
    true
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == document_spec::production_is_intermediate_region(segment_type),
))]
pub(super) const fn is_intermediate_region(segment_type: SegmentType) -> bool {
    #[cfg(verus_keep_ghost)]
    proof! {
        document_shared::intermediate_region_runtime_fact(segment_type);
    }
    matches!(
        segment_type,
        SegmentType::IntermediateTextRegion
            | SegmentType::IntermediateHalftoneRegion
            | SegmentType::IntermediateGenericRegion
            | SegmentType::IntermediateGenericRefinementRegion
    )
}
