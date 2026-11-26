#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn raw_header_only_direct_dispatch_checks_every_page_gate_without_side_effects() {
    let generous = Limits::default();
    for scenario in 0..=21 {
        let mut header = symbol_reference_segment(&[]).header;
        header.segment_type = SegmentType::ImmediateGenericRegion;
        let mut page = runtime_test_page(9, 1, false, true, CombinationOperator::Or, &generous);
        let (mut width, mut height, mut x, mut y) = (9_u32, 1_u32, 0_u32, 0_u32);
        let (mut operator, mut flags) = (0_u8, 6_u8);
        let mut limits = generous;
        match scenario {
            1 => header.segment_type = SegmentType::IntermediateGenericRegion,
            2 => header.segment_type = SegmentType::ImmediateLosslessGenericRegion,
            3 => flags = 1,
            4 => x = 1,
            5 => y = 1,
            6 => operator = 1,
            7 => page.pristine = false,
            8 => page = runtime_test_page(9, 1, true, true, CombinationOperator::Or, &generous),
            9 => page.last_stripe_end = Some(0),
            10 => page.open_stripe_max_row = Some(0),
            11 => width = 8,
            12 => height = 2,
            13 => {
                page.combination_override = true;
                page.default_operator = CombinationOperator::And;
            }
            14 => page.default_operator = CombinationOperator::And,
            15 => header.page_association = 2,
            16 => {
                operator = 4;
                page.combination_override = true;
            }
            17 => operator = 4,
            18 => limits.max_work_units = 170,
            19 => flags = 0xe0,
            21 => limits.max_work_units = 171,
            _ => {}
        }
        let mut payload = Vec::new();
        for value in [width, height, x, y] {
            payload.extend_from_slice(&value.to_be_bytes());
        }
        payload.extend_from_slice(&[operator, flags]);
        if flags == 6 {
            payload.extend_from_slice(&[0xff, 0]);
        }
        if scenario == 20 {
            payload.clear();
        }
        let segment = ParsedSegment {
            header,
            payload: &payload,
            data: ParsedData::Other,
        };
        let before = page.bitmap.bytes().to_vec();
        let pages = vec![
            (
                7,
                runtime_test_page(1, 1, false, true, CombinationOperator::Or, &generous),
            ),
            (1, page),
        ];
        let work = DecodeWork { used: 0 };
        let expected = if matches!(scenario, 0 | 2 | 13 | 16 | 21) {
            Some(1)
        } else {
            None
        };
        assert_eq!(
            direct_generic_page_index(&segment, &pages, &work, &limits),
            expected,
            "header-only scenario {scenario}"
        );
        assert_eq!(pages[1].1.bitmap.bytes(), before);
        assert_eq!(work.used, 0);
    }
}

#[test]
fn raw_dispatch_selectors_match_complete_codes_and_operator_policy() {
    for code in 0..=u8::MAX {
        let result = generic_template(code);
        if code < 4 {
            assert_eq!(
                result.unwrap(),
                [
                    GenericTemplate::Template0,
                    GenericTemplate::Template1,
                    GenericTemplate::Template2,
                    GenericTemplate::Template3,
                ][usize::from(code)]
            );
        } else {
            assert!(matches!(
                result,
                Err(DecodeError::Malformed(MalformedError::InvalidPageState(
                    "invalid generic template"
                )))
            ));
        }
    }
    let operators = [
        CombinationOperator::Or,
        CombinationOperator::And,
        CombinationOperator::Xor,
        CombinationOperator::Xnor,
        CombinationOperator::Replace,
    ];
    for region in operators {
        for page in operators {
            assert_eq!(same_combination_operator(region, page), region == page);
            for combination_override in [false, true] {
                for refinement in [false, true] {
                    let allowed = region == page || combination_override || refinement;
                    let result =
                        check_combination_operator(combination_override, region, page, refinement);
                    assert_eq!(result.is_ok(), allowed);
                    if !allowed {
                        assert!(matches!(
                            result,
                            Err(DecodeError::Malformed(MalformedError::InvalidPageState(
                                "region overrides the page combination operator"
                            )))
                        ));
                    }
                }
            }
        }
    }
    assert!(require_matching_geometry(true).is_ok());
    assert!(matches!(
        require_matching_geometry(false),
        Err(DecodeError::Malformed(MalformedError::InvalidPageState(
            "refinement geometry differs from its intermediate reference"
        )))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn raw_resource_maps_preserve_key_order_identity_and_duplicate_state() {
    let limits = Limits::default();
    let mut pages = Vec::new();
    let mut tables = Vec::new();
    let mut patterns = Vec::new();
    let mut symbols = Vec::new();
    let mut buffers = Vec::new();
    for (index, number) in [7, 0, u32::MAX].into_iter().enumerate() {
        let width = u32::try_from(index + 1).unwrap();
        insert_runtime_page(
            &mut pages,
            number,
            runtime_test_page(width, 1, true, false, CombinationOperator::Replace, &limits),
        );
        assert!(insert_huffman_table(
            &mut tables,
            number,
            HuffmanTable::standard(crate::huffman::StandardHuffmanTable::A, &limits).unwrap()
        ));
        assert!(insert_patterns(
            &mut patterns,
            number,
            vec![Bitmap::blank(width, 1, &limits).unwrap()]
        ));
        assert!(insert_symbol_resource(
            &mut symbols,
            number,
            SymbolResource {
                exported: vec![Bitmap::blank(width, 1, &limits).unwrap()],
                contexts: None
            }
        ));
        let region = crate::segment_data::RegionInfo {
            width,
            height: 1,
            x: 0,
            y: 0,
            operator: CombinationOperator::Replace,
        };
        buffers.push((
            number,
            RegionBuffer {
                region,
                bitmap: Bitmap::blank(width, 1, &limits).unwrap(),
            },
        ));
    }
    for (index, number) in [7, 0, u32::MAX].into_iter().enumerate() {
        assert_eq!(runtime_page_index(&pages, number), Some(index));
        assert_eq!(
            get_runtime_page(&pages, number).unwrap().bitmap.width(),
            u32::try_from(index + 1).unwrap()
        );
        assert_eq!(
            get_region_buffer(&buffers, number).unwrap().region.width,
            u32::try_from(index + 1).unwrap()
        );
        let first = std::ptr::from_ref(get_huffman_table(&tables, number).unwrap());
        assert!(!insert_huffman_table(
            &mut tables,
            number,
            HuffmanTable::standard(crate::huffman::StandardHuffmanTable::B, &limits).unwrap()
        ));
        assert_eq!(
            std::ptr::from_ref(get_huffman_table(&tables, number).unwrap()),
            first
        );
        assert!(!get_huffman_table(&tables, number).unwrap().has_oob());
        assert!(!insert_patterns(&mut patterns, number, Vec::new()));
        assert_eq!(
            get_patterns(&patterns, number).unwrap()[0].width(),
            u32::try_from(index + 1).unwrap()
        );
        assert!(!insert_symbol_resource(
            &mut symbols,
            number,
            SymbolResource {
                exported: Vec::new(),
                contexts: None
            }
        ));
        assert_eq!(
            get_symbol_resource(&symbols, number).unwrap().exported[0].width(),
            u32::try_from(index + 1).unwrap()
        );
        insert_runtime_page(
            &mut pages,
            number,
            runtime_test_page(9, 1, false, true, CombinationOperator::Or, &limits),
        );
        assert_eq!(get_runtime_page(&pages, number).unwrap().bitmap.width(), 9);
    }
    assert_eq!(
        pages.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
        vec![7, 0, u32::MAX]
    );
    assert_eq!(tables.len(), 3);
    assert_eq!(patterns.len(), 3);
    assert_eq!(symbols.len(), 3);
    for missing in [1, 8, u32::MAX - 1] {
        assert!(get_runtime_page(&pages, missing).is_none());
        assert!(get_region_buffer(&buffers, missing).is_none());
        assert!(get_huffman_table(&tables, missing).is_none());
        assert!(get_patterns(&patterns, missing).is_none());
        assert!(get_symbol_resource(&symbols, missing).is_none());
    }
}

#[test]
fn raw_referred_tables_preserve_order_repeats_missing_keys_and_retention() {
    let limits = Limits::default();
    let mut tables = Vec::new();
    for number in [1, 2, 7] {
        assert!(insert_huffman_table(
            &mut tables,
            number,
            HuffmanTable::standard(crate::huffman::StandardHuffmanTable::A, &limits).unwrap()
        ));
    }
    let choices = [0, 1, 2, 9];
    for encoded in 0..256 {
        let references = [
            choices[encoded % 4],
            choices[(encoded / 4) % 4],
            choices[(encoded / 16) % 4],
            choices[(encoded / 64) % 4],
        ];
        let segment = symbol_reference_segment(&references);
        let collected = collect_referred_tables(&segment, &tables).unwrap();
        let expected: Vec<_> = references
            .iter()
            .filter_map(|number| tables.iter().find(|(key, _)| key == number))
            .map(|(_, table)| table)
            .collect();
        assert_eq!(collected.len(), expected.len());
        for (actual, expected) in collected.iter().zip(expected) {
            assert!(std::ptr::eq(*actual, expected));
        }
        assert_eq!(
            first_referred_segment(&references, 100).unwrap(),
            references[0]
        );
        assert_eq!(
            tables.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            vec![1, 2, 7]
        );
    }
    assert!(
        collect_referred_tables(&symbol_reference_segment(&[]), &tables)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        first_referred_segment(&[], u32::MAX),
        Err(DecodeError::Malformed(
            MalformedError::InvalidSegmentReferences(u32::MAX)
        ))
    ));
    let references = [2, 1, 2, 9];
    for mask in 0..16 {
        let mut segment = symbol_reference_segment(&references);
        segment.header.retention_flags.values = std::iter::once(1)
            .chain((0..4).map(|index| (mask >> index) & 1))
            .collect();
        let mut tables = Vec::new();
        for number in [1, 2, 7] {
            insert_huffman_table(
                &mut tables,
                number,
                HuffmanTable::standard(crate::huffman::StandardHuffmanTable::A, &limits).unwrap(),
            );
        }
        let expected: Vec<_> = [1, 2, 7]
            .into_iter()
            .filter(|number| {
                references
                    .iter()
                    .enumerate()
                    .all(|(index, referred)| referred != number || mask & (1 << index) != 0)
            })
            .collect();
        release_referred_tables(&segment, &mut tables);
        assert_eq!(
            tables.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            expected
        );
        release_referred_tables(&segment, &mut tables);
        assert_eq!(
            tables.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn raw_page_collector_guards_preserve_order_pixels_and_rejected_totals() {
    let reserved = Bitmap::blank(9, 1, &Limits::default())
        .unwrap()
        .allocated_bytes();
    for memory_limit in [
        0,
        1,
        2,
        3,
        4,
        reserved - 1,
        reserved,
        reserved * 2 - 1,
        reserved * 2,
        u64::MAX,
    ] {
        for decoded_limit in [0, 1, 2, 3, 4, u64::MAX] {
            let limits = Limits {
                max_decoder_managed_memory_bytes: memory_limit,
                max_cumulative_decoded_bitmap_bytes: decoded_limit,
                ..Limits::default()
            };
            let mut collector = PageCollector::new(&limits);
            let mut accepted = 0;
            for number in [u32::MAX, 1] {
                let mut bitmap = Bitmap::blank(9, 1, &Limits::default()).unwrap();
                bitmap.set_pixel(8, 0, true).unwrap();
                let expected = collector.bitmap_bytes + 2;
                let expected_allocated =
                    collector.allocated_bitmap_bytes + bitmap.allocated_bytes();
                let result = collector.write_page(Page { number, bitmap });
                if expected > decoded_limit
                    || expected > memory_limit
                    || expected_allocated > memory_limit
                {
                    let (kind, limit, expected_actual) = if expected > decoded_limit {
                        (
                            crate::LimitKind::DecodedBitmapBytes,
                            decoded_limit,
                            expected,
                        )
                    } else if expected > memory_limit {
                        (crate::LimitKind::Memory, memory_limit, expected)
                    } else {
                        (crate::LimitKind::Memory, memory_limit, expected_allocated)
                    };
                    assert!(matches!(result, Err(DecodeError::LimitExceeded(
                        crate::LimitExceeded { kind: actual_kind, limit: actual_limit, actual }
                    )) if actual_kind == kind && actual_limit == limit && actual == expected_actual));
                } else {
                    result.unwrap();
                    accepted += 1;
                }
                assert_eq!(collector.pages.len(), accepted);
                assert_eq!(collector.bitmap_bytes, u64::try_from(accepted).unwrap() * 2);
                assert_eq!(
                    collector.allocated_bitmap_bytes,
                    u64::try_from(accepted).unwrap() * reserved
                );
                for page in &collector.pages {
                    assert_eq!(page.bitmap.bytes(), &[0, 0x80]);
                }
            }
            if accepted == 2 {
                assert_eq!(collector.pages[0].number, u32::MAX);
                assert_eq!(collector.pages[1].number, 1);
            }
        }
    }
    let mut collector = PageCollector::new(&Limits {
        max_decoder_managed_memory_bytes: u64::MAX,
        max_cumulative_decoded_bitmap_bytes: u64::MAX,
        ..Limits::default()
    });
    collector.bitmap_bytes = u64::MAX;
    assert!(matches!(
        collector.write_page(Page {
            number: 1,
            bitmap: Bitmap::blank(1, 1, &Limits::default()).unwrap(),
        }),
        Err(DecodeError::Malformed(MalformedError::IntegerOverflow))
    ));
    assert_eq!(collector.bitmap_bytes, u64::MAX);
    assert!(collector.pages.is_empty());
}

#[test]
fn owned_page_collector_rejects_allocated_counter_overflow_without_mutation() {
    let mut collector = PageCollector::new(&Limits::default());
    collector.allocated_bitmap_bytes = u64::MAX;
    let result = collector.write_page(Page {
        number: 1,
        bitmap: Bitmap::blank(1, 1, &Limits::default()).unwrap(),
    });
    assert!(matches!(
        result,
        Err(DecodeError::Malformed(MalformedError::IntegerOverflow))
    ));
    assert_eq!(collector.allocated_bitmap_bytes, u64::MAX);
    assert_eq!(collector.bitmap_bytes, 0);
    assert!(collector.pages.is_empty());
}

#[test]
fn global_stream_with_tables_segment_pre_reserves_and_stores_table() {
    let mut table_payload = vec![0x12, 0, 0, 0, 0, 0, 0, 0, 2];
    table_payload.push(0b0110_0010);
    let mut stream = Vec::new();
    stream.extend_from_slice(&0_u32.to_be_bytes());
    stream.push(53); // SegmentType::Tables
    stream.push(0); // 0 referred segments
    stream.push(0); // page 0 (global)
    stream.extend_from_slice(&u32::try_from(table_payload.len()).unwrap().to_be_bytes());
    stream.extend_from_slice(&table_payload);

    let limits = Limits::default();
    let globals = super::orchestration::decode_global_resources(&stream, &limits).unwrap();
    assert_eq!(globals.tables.len(), 1);
    assert_eq!(globals.tables[0].0, 0);
}
