#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn arithmetic_integer_caller_matches_direct_procedure_and_preserves_other_banks() {
    let fields = [
        IntegerField::AggregationInstances,
        IntegerField::HeightDelta,
        IntegerField::WidthDelta,
        IntegerField::ExportRun,
        IntegerField::StripDelta,
        IntegerField::FirstS,
        IntegerField::SubsequentS,
        IntegerField::WithinStripT,
        IntegerField::RefinementWidth,
        IntegerField::RefinementHeight,
        IntegerField::RefinementX,
        IntegerField::RefinementY,
        IntegerField::RefinementSize,
        IntegerField::RefinementIndicator,
        IntegerField::DictionaryBitmapSize,
    ];
    let samples: [&[u8]; 6] = [
        &[0, 0],
        &[0xff, 0x90],
        &[0; 64],
        &[0x13; 64],
        &[0xaa; 64],
        &[0xff, 0x8f, 0, 0],
    ];
    let mut outcomes = [0; 2];
    for (slot, field) in fields.into_iter().enumerate() {
        assert_eq!(field.slot(), slot);
        for bytes in samples {
            for salt in [0, 1, 23, 46] {
                let mut actual = arithmetic_codec_fixture(bytes, 7, salt);
                let before = actual.integers.slots;
                let before_iaid = actual.iaid.clone();
                let mut expected_bank = before[slot];
                let mut expected_decoder = ArithmeticDecoder::new(bytes).unwrap();
                let expected = expected_decoder
                    .decode_integer(&mut expected_bank)
                    .map(|value| match value {
                        ArithmeticInteger::Value(value) => EntropyInteger::Value(value),
                        ArithmeticInteger::OutOfBand => EntropyInteger::OutOfBand,
                    })
                    .map_err(SymbolTextError::from);
                let success = assert_arithmetic_forwarding(actual.decode_integer(field), expected);
                outcomes[usize::from(success)] += 1;
                assert_eq!(actual.integers.slots[slot], expected_bank);
                for (index, bank) in before.iter().enumerate() {
                    if index != slot {
                        assert_eq!(actual.integers.slots[index], *bank);
                    }
                }
                assert_eq!(actual.iaid, before_iaid);
                assert!(actual.refinement.is_none());
                assert_eq!(actual.refinement_work, 73);
                assert_arithmetic_cursor_and_continuation(
                    &mut actual.decoder,
                    &mut expected_decoder,
                    success,
                );
            }
        }
    }
    assert_eq!(outcomes.iter().sum::<usize>(), 360);
    assert!(outcomes.into_iter().all(|count| count > 0));
}

#[test]
fn arithmetic_iaid_caller_forwards_every_length_and_preserves_integer_banks() {
    let samples: [&[u8]; 6] = [
        &[0, 0],
        &[0xff, 0x90],
        &[0; 64],
        &[0x13; 64],
        &[0xaa; 64],
        &[0xff, 0x8f, 0, 0],
    ];
    let mut outcomes = [0; 2];
    for length in 0..=u8::MAX {
        for count in [0, 1, 2, 3, 4, 16, 32, 35] {
            for bytes in samples {
                let mut actual = arithmetic_codec_fixture(bytes, count, usize::from(length) % 47);
                let before = actual.integers.slots;
                let mut expected_bank = actual.iaid.clone();
                let mut expected_decoder = ArithmeticDecoder::new(bytes).unwrap();
                let expected = expected_decoder
                    .decode_iaid(length, &mut expected_bank)
                    .map_err(SymbolTextError::from);
                let request = SymbolIdRequest {
                    available_symbols: 0,
                    code_length: length,
                    huffman: false,
                };
                let success =
                    assert_arithmetic_forwarding(actual.decode_symbol_id(request), expected);
                outcomes[usize::from(success)] += 1;
                assert_eq!(actual.iaid, expected_bank);
                assert_eq!(actual.integers.slots, before);
                assert!(actual.refinement.is_none());
                assert_eq!(actual.refinement_work, 73);
                assert_arithmetic_cursor_and_continuation(
                    &mut actual.decoder,
                    &mut expected_decoder,
                    success,
                );
            }
        }
    }
    assert_eq!(outcomes.iter().sum::<usize>(), 12_288);
    assert!(outcomes.into_iter().all(|count| count > 0));
}

#[test]
fn symbol_code_length_is_minimal_at_every_u32_power_boundary() {
    let boundaries = (0..u32::BITS).flat_map(|shift| {
        let power = 1_u32 << shift;
        [power - 1, power, power + 1]
    });
    for count in (0..=65_536).chain(boundaries).chain([u32::MAX]) {
        let length = symbol_id_code_length(count).expect("bounded code length");
        let expected = u8::try_from(u32::BITS - count.saturating_sub(1).leading_zeros())
            .expect("at most 32 bits");
        assert_eq!(length, expected, "symbol count {count}");
        assert!((1_u64 << length) >= u64::from(count.max(1)));
        if length != 0 {
            assert!((1_u64 << (length - 1)) < u64::from(count));
        }
    }
}

#[test]
fn packed_symbol_storage_matches_pixels_and_rejects_each_dirty_row() {
    let limits = Limits::default();
    for width in 0_u32..=33 {
        for height in [0_u32, 1, 3] {
            let stride = usize::try_from(width.div_ceil(8)).unwrap();
            let length = stride * usize::try_from(height).unwrap();
            for seed in [0_usize, 0x55, 0xaa, 0xff] {
                let mut bytes: Vec<_> = (0..length)
                    .map(|index| u8::try_from((index * 73 + seed) % 256).unwrap())
                    .collect();
                if width % 8 != 0 {
                    for row in bytes.chunks_exact_mut(stride) {
                        row[stride - 1] &= 0xff << (8 - width % 8);
                    }
                }
                let expected = bytes.clone();
                bytes.extend_from_slice(&[0xff, 0x93]);
                let input = bytes.clone();
                let bitmap = bitmap_from_packed_bytes(width, height, &bytes, &limits)
                    .expect("packed storage");
                assert_eq!(
                    (bitmap.width, bitmap.height, bitmap.stride),
                    (width, height, stride)
                );
                assert_eq!(bitmap.data, expected);
                assert_eq!(bytes, input);
                for y in 0..height {
                    for x in 0..width {
                        let index =
                            usize::try_from(y).unwrap() * stride + usize::try_from(x / 8).unwrap();
                        assert_eq!(
                            bitmap.pixel(x, y),
                            Some(expected[index] & (0x80 >> (x % 8)) != 0),
                        );
                    }
                }
                for cut in 0..length {
                    assert!(matches!(
                        bitmap_from_packed_bytes(width, height, &bytes[..cut], &limits),
                        Err(SymbolTextError::Decode(DecodeError::Malformed(
                            MalformedError::UnexpectedEof {
                                offset: 0, needed, remaining,
                            }
                        ))) if needed == length as u64 && remaining == cut as u64
                    ));
                }
                if width % 8 != 0 {
                    for y in 0..usize::try_from(height).unwrap() {
                        let mut dirty = expected.clone();
                        dirty[(y + 1) * stride - 1] |= 1;
                        assert!(matches!(
                            bitmap_from_packed_bytes(width, height, &dirty, &limits),
                            Err(SymbolTextError::InvalidState(
                                "raw collective bitmap has nonzero padding"
                            ))
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn packed_symbol_limits_are_inclusive_and_precede_payload_checks() {
    let inclusive = Limits {
        max_width: 9,
        max_height: 3,
        max_pixels_per_page: 27,
        max_decoder_managed_memory_bytes: 6,
        ..Limits::default()
    };
    let bytes = [0xff, 0x80, 0x33, 0, 0, 0];
    assert_eq!(
        bitmap_from_packed_bytes(9, 3, &bytes, &inclusive)
            .expect("inclusive limits")
            .data,
        bytes,
    );
    let failures = [
        (LimitKind::Width, 8, 9),
        (LimitKind::Height, 2, 3),
        (LimitKind::PagePixels, 26, 27),
        (LimitKind::Memory, 5, 6),
    ];
    for rejected in 1_u8..16 {
        let limits = Limits {
            max_width: 9 - u32::from(rejected & 1 != 0),
            max_height: 3 - u32::from(rejected & 2 != 0),
            max_pixels_per_page: 27 - u64::from(rejected & 4 != 0),
            max_decoder_managed_memory_bytes: 6 - u64::from(rejected & 8 != 0),
            ..inclusive
        };
        let expected = failures
            .iter()
            .enumerate()
            .find(|(index, _)| rejected & (1 << index) != 0)
            .unwrap()
            .1;
        assert!(matches!(
            bitmap_from_packed_bytes(9, 3, &[], &limits),
            Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
                LimitExceeded { kind, limit, actual }
            ))) if (kind, limit, actual) == *expected
        ));
    }
    let limits = Limits {
        max_width: u32::MAX,
        max_height: u32::MAX,
        max_pixels_per_page: u64::MAX,
        max_decoder_managed_memory_bytes: 0,
        ..Limits::default()
    };
    let needed = u64::from(u32::MAX).div_ceil(8) * u64::from(u32::MAX);
    assert!(matches!(
        bitmap_from_packed_bytes(u32::MAX, u32::MAX, &[], &limits),
        Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
            LimitExceeded { kind: LimitKind::Memory, limit: 0, actual }
        ))) if actual == needed
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn export_run_errors_preserve_classification_work_and_unread_input() {
    let cases = [
        (
            vec![EntropyInteger::Value(1)],
            1,
            2,
            10,
            0,
            1,
            SymbolTextError::InvalidCount("export count exceeds candidate count"),
        ),
        (
            vec![EntropyInteger::Value(-1)],
            3,
            0,
            10,
            1,
            0,
            SymbolTextError::InvalidEntropyValue("negative export run"),
        ),
        (
            vec![EntropyInteger::Value(i64::from(u32::MAX) + 1)],
            3,
            0,
            10,
            1,
            0,
            SymbolTextError::InvalidEntropyValue("negative export run"),
        ),
        (
            vec![EntropyInteger::OutOfBand],
            3,
            0,
            10,
            1,
            0,
            SymbolTextError::UnexpectedOutOfBand("export run"),
        ),
        (
            vec![EntropyInteger::Value(4)],
            3,
            0,
            10,
            1,
            0,
            SymbolTextError::InvalidCount("export run exceeds candidate count"),
        ),
        (
            vec![EntropyInteger::Value(0), EntropyInteger::Value(2)],
            3,
            1,
            10,
            2,
            0,
            SymbolTextError::InvalidCount("too many exported symbols"),
        ),
        (
            vec![EntropyInteger::Value(3)],
            3,
            1,
            10,
            1,
            0,
            SymbolTextError::InvalidCount("exported-symbol popcount mismatch"),
        ),
        (
            vec![
                EntropyInteger::Value(1),
                EntropyInteger::Value(i64::from(u32::MAX)),
            ],
            u32::MAX,
            0,
            10,
            2,
            0,
            SymbolTextError::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow)),
        ),
        (
            vec![],
            1,
            0,
            10,
            1,
            0,
            SymbolTextError::InvalidState("missing scripted integer"),
        ),
        (
            vec![EntropyInteger::Value(1)],
            1,
            0,
            0,
            1,
            1,
            SymbolTextError::Decode(DecodeError::LimitExceeded(LimitExceeded {
                kind: LimitKind::Work,
                limit: 0,
                actual: 1,
            })),
        ),
    ];
    for (integers, candidates, expected_exports, max_work_units, work, unread, expected) in cases {
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers: integers.into(),
            symbol_id: 0,
        });
        let mut resources = ResourceTracker {
            work_units: 0,
            decoded_bitmap_bytes: 17,
        };
        let limits = Limits {
            max_work_units,
            ..Limits::default()
        };
        let error = decode_export_runs(
            &mut codec,
            candidates,
            expected_exports,
            &limits,
            &mut resources,
        )
        .expect_err("invalid export run");
        match (error, expected) {
            (SymbolTextError::InvalidCount(actual), SymbolTextError::InvalidCount(expected))
            | (
                SymbolTextError::InvalidEntropyValue(actual),
                SymbolTextError::InvalidEntropyValue(expected),
            )
            | (
                SymbolTextError::UnexpectedOutOfBand(actual),
                SymbolTextError::UnexpectedOutOfBand(expected),
            )
            | (SymbolTextError::InvalidState(actual), SymbolTextError::InvalidState(expected)) => {
                assert_eq!(actual, expected);
            }
            (
                SymbolTextError::Decode(DecodeError::Malformed(actual)),
                SymbolTextError::Decode(DecodeError::Malformed(expected)),
            ) => assert_eq!(actual, expected),
            (
                SymbolTextError::Decode(DecodeError::LimitExceeded(actual)),
                SymbolTextError::Decode(DecodeError::LimitExceeded(expected)),
            ) => assert_eq!(actual, expected),
            (actual, expected) => {
                panic!("wrong export error: {actual:?}, expected {expected:?}")
            }
        }
        assert_eq!(
            resources,
            ResourceTracker {
                work_units: work,
                decoded_bitmap_bytes: 17
            }
        );
        let SymbolCodec::Scripted(scripted) = codec else {
            panic!("scripted codec")
        };
        assert_eq!(scripted.integers.len(), unread);
    }
}

#[test]
fn export_runs_match_every_small_selection_with_exact_work_and_input_frames() {
    let mut checked = 0;
    for count in 0_u32..=10 {
        for selected in 0_u32..(1 << count) {
            let expected: Vec<_> = (0..count)
                .filter(|index| selected & (1 << index) != 0)
                .collect();
            let mut integers = VecDeque::new();
            let mut flag = false;
            let mut run = 0;
            for index in 0..count {
                let next = selected & (1 << index) != 0;
                if next != flag {
                    integers.push_back(EntropyInteger::Value(run));
                    flag = next;
                    run = 0;
                }
                run += 1;
            }
            if count != 0 {
                integers.push_back(EntropyInteger::Value(run));
                for _ in 0..2 * (selected.count_ones() % 3) {
                    integers.push_front(EntropyInteger::Value(0));
                }
            }
            let work = u64::try_from(integers.len()).unwrap() + 5;
            integers.push_back(EntropyInteger::Value(-73));
            let mut codec = SymbolCodec::Scripted(ScriptedCodec {
                integers,
                symbol_id: 0,
            });
            let mut resources = ResourceTracker {
                work_units: 5,
                decoded_bitmap_bytes: 17,
            };
            let limits = Limits {
                max_work_units: work,
                ..Limits::default()
            };
            let result = decode_export_runs(
                &mut codec,
                count,
                selected.count_ones(),
                &limits,
                &mut resources,
            )
            .expect("scalar selection");
            assert_eq!(result, expected);
            assert!(result.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(
                resources,
                ResourceTracker {
                    work_units: work,
                    decoded_bitmap_bytes: 17
                }
            );
            let SymbolCodec::Scripted(scripted) = codec else {
                panic!("scripted codec")
            };
            assert_eq!(
                scripted.integers,
                VecDeque::from([EntropyInteger::Value(-73)])
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 2_047);
}

#[test]
fn lossless_byte_and_boolean_bridges_match_standard_conversions() {
    for value in 0..=u8::MAX {
        assert_eq!(widen_u8_i64(value), i64::from(value));
        assert_eq!(widen_u8_i32(value), i32::from(value));
    }
    for value in [false, true] {
        assert_eq!(widen_bool_i64(value), i64::from(value));
    }
}

#[test]
fn full_u32_widening_matches_standard_conversion() {
    for value in [
        0,
        1,
        255,
        256,
        65_535,
        65_536,
        0x7fff_ffff,
        1_u32 << 31,
        u32::MAX - 1,
        u32::MAX,
    ] {
        assert_eq!(widen_u32_i64(value), i64::from(value));
    }
    let mut value = 0x1020_3040_u32;
    for _ in 0..4096 {
        value = value.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        assert_eq!(widen_u32_i64(value), i64::from(value));
    }
}

#[test]
fn concrete_empty_dictionary_and_arithmetic_text_segments_decode() {
    const ANNEX_H_ENCODED: [u8; 30] = [
        0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb,
        0x86, 0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
    ];
    let limits = Limits::default();
    let custom: [&HuffmanTable; 0] = [];

    let mut arithmetic_dictionary = vec![0x00, 0x00];
    arithmetic_dictionary.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    arithmetic_dictionary.extend_from_slice(&0_u32.to_be_bytes());
    arithmetic_dictionary.extend_from_slice(&0_u32.to_be_bytes());
    arithmetic_dictionary.extend_from_slice(&[0xff, 0xac]);
    let arithmetic_output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &arithmetic_dictionary,
        imported_symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &custom,
        used_contexts: None,
        limits: &limits,
    })
    .expect("empty arithmetic dictionary decodes");
    assert!(arithmetic_output.exported_symbols.is_empty());
    assert_eq!(
        arithmetic_output.consumed_bytes,
        arithmetic_dictionary.len()
    );

    let mut huffman_dictionary = vec![0x00, 0x01];
    huffman_dictionary.extend_from_slice(&0_u32.to_be_bytes());
    huffman_dictionary.extend_from_slice(&0_u32.to_be_bytes());
    let huffman_output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &huffman_dictionary,
        imported_symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &custom,
        used_contexts: None,
        limits: &limits,
    })
    .expect("empty Huffman dictionary decodes");
    assert!(huffman_output.exported_symbols.is_empty());
    assert_eq!(huffman_output.consumed_bytes, huffman_dictionary.len());

    let mut arithmetic_text = vec![0_u8; 17];
    arithmetic_text[3] = 1;
    arithmetic_text[7] = 1;
    arithmetic_text.extend_from_slice(&0_u16.to_be_bytes());
    arithmetic_text.extend_from_slice(&0_u32.to_be_bytes());
    arithmetic_text.extend_from_slice(&ANNEX_H_ENCODED);
    let arithmetic_text_output = decode_text_region(&TextRegionInput {
        allow_missing_final_huffman_oob: false,
        allow_missing_final_arithmetic_oob: false,
        payload: &arithmetic_text,
        symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &custom,
        limits: &limits,
    })
    .expect("empty arithmetic text region decodes");
    assert_eq!(arithmetic_text_output.bitmap.bytes(), &[0]);
}

fn independent_iaid_bytes(count: u32) -> u64 {
    let mut capacity = 1_u64;
    while capacity < u64::from(count) {
        capacity *= 2;
    }
    capacity * u64::try_from(core::mem::size_of::<ArithmeticContext>()).unwrap()
}

#[test]
fn actual_dictionary_setup_preserves_retained_context_error_priority() {
    let limits = Limits::default();
    let custom: [&HuffmanTable; 0] = [];
    let empty = DictionaryContexts {
        model: SymbolDictionaryHeader {
            config: parse_symbol_dictionary_flags(0).unwrap(),
            adaptive: [TemplateOffset::new(0, 0); 4],
            adaptive_count: 0,
            refinement_adaptive: [TemplateOffset::new(0, 0); 2],
            exported_symbols: 0,
            new_symbols: 0,
        },
        generic: None,
        refinement: None,
    };
    for (flags, used, expected) in [
        (0x0100_u16, None, "missing retained contexts"),
        (0x0100, Some(&empty), "missing retained generic contexts"),
        (0x0102, Some(&empty), "missing retained refinement contexts"),
        (
            0,
            Some(&empty),
            "unexpected retained contexts without the context-used flag",
        ),
    ] {
        let mut payload = flags.to_be_bytes().to_vec();
        payload.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
        if flags & 2 != 0 {
            payload.extend_from_slice(&[0; 4]);
        }
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&[0xff, 0xac]);
        let result = decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &payload,
            imported_symbols: SymbolCodebook::Contiguous(&[]),
            custom_tables: &custom,
            used_contexts: used,
            limits: &limits,
        });
        match result {
            Err(SymbolTextError::InvalidState(actual)) => {
                assert_eq!(actual, expected, "flags={flags:#x}");
            }
            Err(actual) => panic!("flags={flags:#x}: expected {expected}, got {actual:?}"),
            Ok(_) => panic!("flags={flags:#x}: expected {expected}, got success"),
        }
    }
}

fn assert_segment_iaid_limit<T>(result: &Result<T, SymbolTextError>, requested: u64) {
    assert!(matches!(
        result,
        Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
            LimitExceeded { kind: LimitKind::Memory, limit, actual },
        ))) if *limit == requested - 1 && *actual == requested
    ));
}

#[test]
fn actual_text_segment_width_uses_full_contiguous_or_borrowed_extent() {
    let default = Limits::default();
    let custom: [&HuffmanTable; 0] = [];
    let mut payload = vec![0_u8; 17];
    payload[3] = 1;
    payload[7] = 1;
    payload.extend_from_slice(&0_u16.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&[0xff, 0xac]);
    for count in [0_u32, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
        let symbols: Vec<Bitmap> = (0..count)
            .map(|_| Bitmap::blank(1, 1, &default).unwrap())
            .collect();
        let borrowed: Vec<&Bitmap> = symbols.iter().collect();
        let requested = independent_iaid_bytes(count);
        let limits = Limits {
            max_decoder_managed_memory_bytes: requested - 1,
            ..default
        };
        for book in [
            SymbolCodebook::Contiguous(&symbols),
            SymbolCodebook::Borrowed(&borrowed),
        ] {
            assert_segment_iaid_limit(
                &decode_text_region(&TextRegionInput {
                    allow_missing_final_huffman_oob: false,
                    allow_missing_final_arithmetic_oob: false,
                    payload: &payload,
                    symbols: book,
                    custom_tables: &custom,
                    limits: &limits,
                }),
                requested,
            );
        }
    }
}

#[test]
fn actual_dictionary_segment_width_uses_imported_plus_parsed_new_count() {
    let default = Limits::default();
    let custom: [&HuffmanTable; 0] = [];
    for imported in 0..=8 {
        let symbols: Vec<Bitmap> = (0..imported)
            .map(|_| Bitmap::blank(1, 1, &default).unwrap())
            .collect();
        let borrowed: Vec<&Bitmap> = symbols.iter().collect();
        for new_symbols in 0_u32..=8 {
            let mut payload = vec![0, 0, 3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe];
            payload.extend_from_slice(&0_u32.to_be_bytes());
            payload.extend_from_slice(&new_symbols.to_be_bytes());
            payload.extend_from_slice(&[0xff, 0xac]);
            let requested = independent_iaid_bytes(imported + new_symbols);
            let limits = Limits {
                max_decoder_managed_memory_bytes: requested - 1,
                ..default
            };
            for book in [
                SymbolCodebook::Contiguous(&symbols),
                SymbolCodebook::Borrowed(&borrowed),
            ] {
                assert_segment_iaid_limit(
                    &decode_symbol_dictionary(&SymbolDictionaryInput {
                        payload: &payload,
                        imported_symbols: book,
                        custom_tables: &custom,
                        used_contexts: None,
                        limits: &limits,
                    }),
                    requested,
                );
            }
        }
    }
}
