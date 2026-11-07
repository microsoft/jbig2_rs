#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;
use crate::document::references::{
    known_reference_is_intermediate, references_are_symbol_resources,
};

const KINDS: [SegmentType; 22] = [
    SegmentType::SymbolDictionary,
    SegmentType::IntermediateTextRegion,
    SegmentType::ImmediateTextRegion,
    SegmentType::ImmediateLosslessTextRegion,
    SegmentType::PatternDictionary,
    SegmentType::IntermediateHalftoneRegion,
    SegmentType::ImmediateHalftoneRegion,
    SegmentType::ImmediateLosslessHalftoneRegion,
    SegmentType::IntermediateGenericRegion,
    SegmentType::ImmediateGenericRegion,
    SegmentType::ImmediateLosslessGenericRegion,
    SegmentType::IntermediateGenericRefinementRegion,
    SegmentType::ImmediateGenericRefinementRegion,
    SegmentType::ImmediateLosslessGenericRefinementRegion,
    SegmentType::PageInformation,
    SegmentType::EndOfPage,
    SegmentType::EndOfStripe,
    SegmentType::EndOfFile,
    SegmentType::Profiles,
    SegmentType::Tables,
    SegmentType::ColourPalette,
    SegmentType::Extension,
];

fn owner(kind: SegmentType, references: &[u32]) -> SegmentHeader {
    let mut header = reference_validation_segment(29, &[], 1).header;
    header.segment_type = kind;
    header.referred_segments = references.to_vec();
    header
}

fn local(number: u32, kind: SegmentType) -> ParsedSegment<'static> {
    let mut segment = reference_validation_segment(number, &[], 1);
    segment.header.segment_type = kind;
    segment
}

#[test]
fn reference_types_lookup_keeps_bounds_first_local_precedence_and_global_first_match() {
    let header = owner(SegmentType::Extension, &[7, 19, 31]);
    let segments = [
        local(7, SegmentType::Tables),
        local(7, SegmentType::PatternDictionary),
    ];
    let indexes = vec![(7, 1), (7, 0)];
    let globals = GlobalSegments {
        types: vec![
            (7, SegmentType::SymbolDictionary),
            (19, SegmentType::ImmediateGenericRegion),
            (19, SegmentType::IntermediateGenericRegion),
        ],
        depths: Vec::new(),
        bytes: Vec::new(),
    };
    for (index, expected) in [
        (0, Some(SegmentType::PatternDictionary)),
        (1, Some(SegmentType::ImmediateGenericRegion)),
        (2, None),
        (3, None),
        (usize::MAX, None),
    ] {
        assert_eq!(
            known_reference_type(&header, index, &segments, &indexes, Some(&globals)),
            expected
        );
    }
    assert_eq!(
        known_reference_type(&header, 0, &segments, &indexes, None),
        Some(SegmentType::PatternDictionary)
    );
    assert_eq!(
        known_reference_type(&header, 1, &segments, &indexes, None),
        None
    );
    assert_eq!(
        known_reference_type(&header, 0, &segments, &Vec::new(), Some(&globals)),
        Some(SegmentType::SymbolDictionary)
    );
}

#[test]
fn reference_types_intermediate_classifies_every_resolved_kind_and_missing() {
    for (index, kind) in KINDS.into_iter().enumerate() {
        let segments = [local(7, kind)];
        let indexes = vec![(7, 0)];
        let header = owner(SegmentType::Extension, &[7]);
        let expected = matches!(index, 1 | 5 | 8 | 11);
        assert_eq!(
            known_reference_is_intermediate(&header, &segments, &indexes, None),
            expected
        );
        let globals = GlobalSegments {
            types: vec![(7, kind)],
            depths: Vec::new(),
            bytes: Vec::new(),
        };
        assert_eq!(
            known_reference_is_intermediate(&header, &[], &Vec::new(), Some(&globals)),
            expected
        );
    }
    let header = owner(SegmentType::Extension, &[7]);
    assert!(!known_reference_is_intermediate(
        &header,
        &[],
        &Vec::new(),
        None
    ));
    assert!(!known_reference_is_intermediate(
        &owner(SegmentType::Extension, &[]),
        &[],
        &Vec::new(),
        None
    ));
}

#[test]
fn reference_types_symbol_resources_keep_table_boundary_and_stage_missing_behavior() {
    let segments = [
        local(7, SegmentType::SymbolDictionary),
        local(19, SegmentType::Tables),
    ];
    let indexes = vec![(7, 0), (19, 1)];
    for tables in 0..=10_usize {
        let mut refs = vec![7, 31];
        refs.extend(std::iter::repeat_n(19, tables));
        refs.push(7);
        let header = owner(SegmentType::SymbolDictionary, &refs);
        for maximum in [0_usize, 1, 4, 8, tables, usize::MAX] {
            assert_eq!(
                references_are_symbol_resources(&header, &segments, &indexes, None, maximum),
                tables <= maximum
            );
        }
    }
    for kind in KINDS {
        let segments = [local(7, kind)];
        let header = owner(SegmentType::SymbolDictionary, &[7]);
        assert_eq!(
            references_are_symbol_resources(&header, &segments, &vec![(7, 0)], None, 1),
            matches!(kind, SegmentType::SymbolDictionary | SegmentType::Tables)
        );
    }
    let missing = owner(SegmentType::SymbolDictionary, &[31]);
    assert!(references_are_symbol_resources(
        &missing,
        &[],
        &Vec::new(),
        None,
        0
    ));
    assert!(validate_reference_types(&missing, &[], &Vec::new(), None).is_ok());
    let mut segment = local(29, SegmentType::SymbolDictionary);
    segment.header.referred_segments = vec![31];
    assert!(matches!(
        validate_reference_links(&[segment], &vec![(29, 0)], None),
        Err(DecodeError::Malformed(
            MalformedError::MissingReferredSegment {
                segment: 29,
                referred: 31
            }
        ))
    ));
}

#[test]
fn reference_types_dispatch_matches_every_owner_kind_cardinality_and_resolved_class() {
    let indexes = vec![(7, 0)];
    for (owner_index, kind) in KINDS.into_iter().enumerate() {
        for (reference_index, reference_kind) in KINDS.into_iter().enumerate() {
            let segments = [local(7, reference_kind)];
            for count in [0_usize, 1, 2, 4, 5, 8, 9] {
                let refs = vec![7; count];
                let header = owner(kind, &refs);
                let expected = match owner_index {
                    0..=3 => {
                        let max = if owner_index == 0 { 4 } else { 8 };
                        count == 0
                            || reference_index == 0
                            || (reference_index == 19 && count <= max)
                    }
                    5..=7 => count == 1 && reference_index == 4,
                    11 => count == 1 && matches!(reference_index, 1 | 5 | 8 | 11),
                    12..=13 => {
                        count == 0 || (count == 1 && matches!(reference_index, 1 | 5 | 8 | 11))
                    }
                    21 => true,
                    _ => count == 0,
                };
                match validate_reference_types(&header, &segments, &indexes, None) {
                    Ok(()) => assert!(expected, "{owner_index}/{reference_index}/{count}"),
                    Err(DecodeError::Malformed(MalformedError::InvalidSegmentReferences(
                        number,
                    ))) => {
                        assert!(!expected, "{owner_index}/{reference_index}/{count}");
                        assert_eq!(number, 29);
                    }
                    other => panic!("unexpected type-stage result: {other:?}"),
                }
            }
        }
    }
}
