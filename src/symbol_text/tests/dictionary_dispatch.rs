#![cfg(test)]

mod huffman_aggregation;

use super::super::codec::validate_bounded_arithmetic_terminator;
use super::super::dictionary::decode_one_new_symbol;
use super::*;

fn empty_context_dictionary(flags: u16) -> Vec<u8> {
    let mut payload = flags.to_be_bytes().to_vec();
    if flags & 1 == 0 {
        if flags & 0x0c00 == 0 {
            payload.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
        } else {
            payload.extend_from_slice(&[0xff, 0xff]);
        }
    }
    if flags & 2 != 0 && flags & 0x1000 == 0 {
        payload.extend_from_slice(&[0xff; 4]);
    }
    payload.extend_from_slice(&[0; 8]);
    if flags & 1 == 0 {
        payload.extend_from_slice(&[0xff, 0xac]);
    }
    payload
}

#[test]
fn framed_dictionary_context_models_check_all_flags_and_adaptive_locations() {
    let limits = Limits::default();
    // Both sides are independently framed dictionaries; only their models differ.
    for (retained_flags, used_flags) in [
        (0x0202, 0x0303), // SDHUFF, same refinement model.
        (0x0202, 0x0300), // SDREFAGG.
        (0x0a00, 0x0f00), // SDTEMPLATE 2 versus 3 (same bank size).
        (0x0202, 0x1302), // SDRTEMPLATE.
        (0x0200, 0x0300), // Direct AT locations.
        (0x0202, 0x0302), // Refinement AT locations.
    ] {
        let retained_payload = empty_context_dictionary(retained_flags);
        let bank = decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &retained_payload,
            imported_symbols: SymbolCodebook::Contiguous(&[]),
            custom_tables: &[],
            used_contexts: None,
            limits: &limits,
        })
        .unwrap()
        .retained_contexts
        .unwrap();
        let matching_payload = empty_context_dictionary(retained_flags | 0x0100);
        let matching = decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &matching_payload,
            imported_symbols: SymbolCodebook::Contiguous(&[]),
            custom_tables: &[],
            used_contexts: Some(&bank),
            limits: &limits,
        })
        .unwrap();
        assert_eq!(matching.consumed_bytes, matching_payload.len());

        let mut payload = empty_context_dictionary(used_flags);
        if retained_flags == used_flags & !0x0100 {
            // Change each coordinate independently, not only the first AT.
            let range = if retained_flags & 2 == 0 {
                2..10
            } else {
                10..14
            };
            for offset in range {
                payload[offset] ^= 1;
                assert!(matches!(
                    decode_symbol_dictionary(&SymbolDictionaryInput {
                        payload: &payload,
                        imported_symbols: SymbolCodebook::Contiguous(&[]),
                        custom_tables: &[],
                        used_contexts: Some(&bank),
                        limits: &limits,
                    }),
                    Err(SymbolTextError::InvalidState(
                        "incompatible retained dictionary context model"
                    ))
                ));
                payload[offset] ^= 1;
            }
        } else {
            // Missing bank kinds remain a distinct, earlier error.
            let expected = if used_flags & 2 == 0 && bank.generic.is_none() {
                "missing retained generic contexts"
            } else {
                "incompatible retained dictionary context model"
            };
            assert!(matches!(
                decode_symbol_dictionary(&SymbolDictionaryInput {
                    payload: &payload,
                    imported_symbols: SymbolCodebook::Contiguous(&[]),
                    custom_tables: &[],
                    used_contexts: Some(&bank),
                    limits: &limits,
                }),
                Err(SymbolTextError::InvalidState(message)) if message == expected
            ));
        }
    }
}

fn one_symbol_arithmetic_dictionary(body: &[u8]) -> Vec<u8> {
    let mut payload = empty_context_dictionary(0);
    payload.truncate(payload.len() - 2);
    payload[10..14].copy_from_slice(&1_u32.to_be_bytes());
    payload[14..18].copy_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(body);
    payload
}

#[test]
fn repeated_empty_arithmetic_dictionary_work_remains_bounded_without_new_syntax_rules() {
    // Pinned #215 and #216 payloads, including their valid Annex E framing.
    // 6.5.10 permits zero runs; 6.5.5 step 4 permits an immediate class OOB.
    for body in [
        &[
            0x2a, 0x8a, 0xde, 0x99, 0x96, 0x13, 0xe9, 0x49, 0x32, 0xee, 0xe8, 0x35, 0x51, 0xc8,
            0x0a, 0xe1, 0x53, 0x99, 0xfd, 0x65, 0x06, 0x24, 0x5d, 0x84, 0x9c, 0x71, 0x41, 0x53,
            0x43, 0x7f, 0xff, 0xac,
        ][..],
        &[0xa1, 0x41, 0xdf, 0xff, 0x7f, 0xff, 0xac][..],
    ] {
        let payload = one_symbol_arithmetic_dictionary(body);
        let limits = Limits {
            max_work_units: 100_000,
            ..Limits::default()
        };
        assert!(matches!(
            decode_symbol_dictionary(&SymbolDictionaryInput {
                payload: &payload,
                imported_symbols: SymbolCodebook::Contiguous(&[]),
                custom_tables: &[],
                used_contexts: None,
                limits: &limits,
            }),
            Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
                LimitExceeded {
                    kind: LimitKind::Work,
                    limit: 100_000,
                    actual: 100_001,
                }
            )))
        ));
    }
}

#[test]
fn framed_dictionary_accepts_many_empty_classes_and_interleaved_zero_export_runs() {
    let limits = Limits::default();
    let imported = [bitmap_from_packed_bytes(8, 1, &[0xa5], &limits).unwrap()];
    let mut payload = 0x1003_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&1_u32.to_be_bytes());
    let mut bits = "0111111".repeat(257); // DH=1, immediate DW OOB.
    bits.push('0'); // Final DH=1.
    bits.push_str("11110000101"); // DW=16.
    bits.push_str("00010"); // Two instances.
    bits.push_str("00"); // Initial DT and strip DT.
    bits.push_str("000000000"); // FS=0.
    bits.push_str("00"); // One-bit ID=0, RI=0.
    bits.push_str("001"); // DS=1.
    bits.push_str("00"); // ID=0, RI=0.
    bits.push_str("01"); // Strip OOB.
    bits.push_str("111111"); // Class OOB.
    bits.push_str(&dictionary_export_run(1)); // Skip the imported symbol.
    for _ in 0..257 {
        bits.push_str(&dictionary_export_run(0));
        bits.push_str(&dictionary_export_run(0));
    }
    bits.push_str(&dictionary_export_run(1)); // Export the aggregate.
    payload.extend_from_slice(&dictionary_bits(&bits));
    let output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &payload,
        imported_symbols: SymbolCodebook::Contiguous(&imported),
        custom_tables: &[],
        used_contexts: None,
        limits: &limits,
    })
    .unwrap();
    assert_eq!(output.consumed_bytes, payload.len());
    assert_eq!(output.exported_symbols.len(), 1);
    let bitmap = &output.exported_symbols[0];
    assert_eq!((bitmap.width(), bitmap.height()), (16, 258));
    assert_eq!(&bitmap.bytes()[..2], &[0xa5, 0xa5]);
    assert!(bitmap.bytes()[2..].iter().all(|byte| *byte == 0));
}

#[test]
fn dictionary_declared_symbol_overrun_is_not_a_configured_limit() {
    // Both #218 fixtures carry this identical dictionary. The entropy requests
    // another symbol after the declared one; it does not exhaust max_symbols.
    let payload =
        one_symbol_arithmetic_dictionary(&[0x09, 0xda, 0x2c, 0xc9, 0xff, 0x7f, 0xff, 0xac]);
    for max_symbols in [0, 1, Limits::default().max_symbols] {
        let limits = Limits {
            max_symbols,
            ..Limits::default()
        };
        let result = decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &payload,
            imported_symbols: SymbolCodebook::Contiguous(&[]),
            custom_tables: &[],
            used_contexts: None,
            limits: &limits,
        });
        if max_symbols == 0 {
            assert!(matches!(
                result,
                Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
                    LimitExceeded {
                        kind: LimitKind::Symbols,
                        limit: 0,
                        actual: 1,
                    }
                )))
            ));
        } else {
            assert!(matches!(
                result,
                Err(SymbolTextError::InvalidCount("too many new symbols"))
            ));
        }
        let valid = empty_context_dictionary(0);
        assert!(decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &valid,
            imported_symbols: SymbolCodebook::Contiguous(&[]),
            custom_tables: &[],
            used_contexts: None,
            limits: &limits,
        })
        .is_ok());
    }
}

#[test]
fn dictionary_arithmetic_termination_is_independent_of_context_model_validation() {
    let limits = Limits::default();
    let valid = empty_context_dictionary(0x0e00);
    let bank = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &valid,
        imported_symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &[],
        used_contexts: None,
        limits: &limits,
    })
    .unwrap()
    .retained_contexts
    .unwrap();
    for body in [&[][..], &[0x08; 4][..], &[0xff, 0xac, 0][..]] {
        let mut payload = empty_context_dictionary(0x0f00);
        payload.truncate(payload.len() - 2);
        payload.extend_from_slice(body);
        assert!(matches!(
            decode_symbol_dictionary(&SymbolDictionaryInput {
                payload: &payload,
                imported_symbols: SymbolCodebook::Contiguous(&[]),
                custom_tables: &[],
                used_contexts: Some(&bank),
                limits: &limits,
            }),
            Err(SymbolTextError::InvalidState(
                "bounded arithmetic data has no terminal marker"
            ))
        ));
    }
    // The strict bounded marker validator does not borrow a missing suffix
    // from an adjacent section. Normalization's existing lone-FF policy is
    // separate and is not expanded by this change.
    for invalid in [&[][..], &[0xff][..], &[0xff, 0xac, 0][..]] {
        assert!(validate_bounded_arithmetic_terminator(invalid).is_err());
    }
    assert!(validate_bounded_arithmetic_terminator(&[0xff, 0xac]).is_ok());
    assert!(validate_bounded_arithmetic_terminator(&[0xff, 0x8f, 0xff, 0xac]).is_ok());
    assert!(matches!(
        validate_bounded_arithmetic_terminator(&[0xff, 0x90, 0xff, 0xac]),
        Err(SymbolTextError::InvalidState(
            "premature arithmetic marker in embedded refinement data"
        ))
    ));
}

#[test]
fn arithmetic_dictionary_reserves_generic_snapshot_only_when_retention_is_requested() {
    for (flags, adaptive, context_bytes) in [
        (
            0_u16,
            &[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe][..],
            131_072_u64,
        ),
        (0x0c00, &[0xff, 0xff][..], 2_048),
    ] {
        for retain in [false, true] {
            let mut payload = (flags | if retain { 0x0200 } else { 0 })
                .to_be_bytes()
                .to_vec();
            payload.extend_from_slice(adaptive);
            payload.extend_from_slice(&0_u32.to_be_bytes());
            payload.extend_from_slice(&0_u32.to_be_bytes());
            payload.extend_from_slice(&[0xff, 0xac]);
            for budget in [context_bytes * 3 / 2, context_bytes * 2] {
                let limits = Limits {
                    max_decoder_managed_memory_bytes: budget,
                    ..Limits::default()
                };
                let output = decode_symbol_dictionary(&SymbolDictionaryInput {
                    payload: &payload,
                    imported_symbols: SymbolCodebook::Contiguous(&[]),
                    custom_tables: &[],
                    used_contexts: None,
                    limits: &limits,
                });
                if retain && budget < context_bytes * 2 {
                    assert!(
                        matches!(output, Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
                        LimitExceeded { kind: LimitKind::Memory, limit, actual }
                    ))) if limit == budget && actual == context_bytes * 2)
                    );
                    continue;
                }
                let output = output.unwrap();
                assert_eq!(output.consumed_bytes, payload.len());
                assert!(output.exported_symbols.is_empty());
                assert_eq!(output.retained_contexts.is_some(), retain);
                if let Some(retained) = output.retained_contexts {
                    assert!(retained.generic.is_some());
                    assert!(retained.refinement.is_none());
                }
            }
        }
    }
}

#[test]
fn aggregate_dictionary_limits_each_bitmap_not_an_unallocated_collective() {
    let limits = Limits {
        max_width: 16,
        ..Limits::default()
    };
    let imported = [bitmap_from_packed_bytes(8, 1, &[0xa5], &limits).unwrap()];
    let mut payload = 0x1003_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&2_u32.to_be_bytes());
    payload.extend_from_slice(&2_u32.to_be_bytes());
    let mut bits = String::from("0"); // DH=1.
    for width_delta in ["11110000101", "0"] {
        // DW=16, then DW=0.
        bits.push_str(width_delta);
        bits.push_str("00010"); // Two instances.
        bits.push_str("00"); // Initial DT and strip DT.
        bits.push_str("000000000"); // FS=0.
        bits.push_str("000"); // Two-bit ID=0, RI=0.
        bits.push_str("001"); // DS=1.
        bits.push_str("000"); // ID=0, RI=0.
        bits.push_str("01"); // Strip OOB.
    }
    bits.push_str("111111");
    bits.push_str(&dictionary_export_run(1));
    bits.push_str(&dictionary_export_run(2));
    payload.extend_from_slice(&dictionary_bits(&bits));
    let output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &payload,
        imported_symbols: SymbolCodebook::Contiguous(&imported),
        custom_tables: &[],
        used_contexts: None,
        limits: &limits,
    })
    .unwrap();
    assert_eq!(output.exported_symbols.len(), 2);
    for bitmap in &output.exported_symbols {
        assert_eq!((bitmap.width(), bitmap.height()), (16, 1));
        assert_eq!(bitmap.bytes(), &[0xa5, 0xa5]);
    }
    assert_eq!(output.consumed_bytes, payload.len());
}

#[test]
fn huffman_dictionary_restores_retained_refinement_statistics() {
    let limits = Limits::default();
    let imported = [bitmap_from_packed_bytes(8, 1, &[0xa5], &limits).unwrap()];
    for (template, flags) in [
        (RefinementTemplate::Template0, 0x0303_u16),
        (RefinementTemplate::Template1, 0x1303_u16),
    ] {
        let mut bank = RefinementContexts::new(template, &limits).unwrap();
        bank.contexts[8] = ArithmeticContext::from_state(10, 1).unwrap();
        let mut model = header(true, true);
        model.config = parse_symbol_dictionary_flags(flags).unwrap();
        let used = DictionaryContexts {
            model,
            generic: None,
            refinement: Some(bank.try_snapshot(&limits).unwrap()),
        };
        let mut payload = flags.to_be_bytes().to_vec();
        if template == RefinementTemplate::Template0 {
            payload.extend_from_slice(&[0; 4]);
        }
        payload.extend_from_slice(&1_u32.to_be_bytes());
        payload.extend_from_slice(&0_u32.to_be_bytes());
        payload.extend_from_slice(&[0, 0x40]); // Export the one imported symbol.
        let output = decode_symbol_dictionary(&SymbolDictionaryInput {
            payload: &payload,
            imported_symbols: SymbolCodebook::Contiguous(&imported),
            custom_tables: &[],
            used_contexts: Some(&used),
            limits: &limits,
        })
        .unwrap();
        assert_eq!(output.exported_symbols[0].bytes(), &[0xa5]);
        assert_eq!(output.consumed_bytes, payload.len());
        let retained = output.retained_contexts.unwrap();
        let mut restored = RefinementContexts::new(template, &limits).unwrap();
        restored
            .restore(retained.refinement.as_ref().unwrap(), &limits)
            .unwrap();
        assert_eq!(restored.contexts.len(), bank.contexts.len());
        assert_eq!(
            restored
                .contexts
                .iter()
                .zip(&bank.contexts)
                .position(|(actual, expected)| actual != expected),
            None,
            "retained arithmetic statistics changed",
        );
        let other_template = if template == RefinementTemplate::Template0 {
            RefinementTemplate::Template1
        } else {
            RefinementTemplate::Template0
        };
        let incompatible = DictionaryContexts {
            model,
            generic: None,
            refinement: Some(
                RefinementContexts::new(other_template, &limits)
                    .unwrap()
                    .try_snapshot(&limits)
                    .unwrap(),
            ),
        };
        assert!(matches!(
            decode_symbol_dictionary(&SymbolDictionaryInput {
                payload: &payload,
                imported_symbols: SymbolCodebook::Contiguous(&imported),
                custom_tables: &[],
                used_contexts: Some(&incompatible),
                limits: &limits,
            }),
            Err(SymbolTextError::Decode(DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(crate::GenericRegionErrorKind::InvalidFlags(
                    _
                ))
            )))
        ));
    }
}

fn header(huffman: bool, refinement: bool) -> SymbolDictionaryHeader {
    let flags = u16::from(huffman) | (u16::from(refinement) << 1);
    SymbolDictionaryHeader {
        config: parse_symbol_dictionary_flags(flags).unwrap(),
        adaptive: [TemplateOffset::new(0, 0); 4],
        adaptive_count: 0,
        refinement_adaptive: [TemplateOffset::new(0, 0); 2],
        exported_symbols: 1,
        new_symbols: 1,
    }
}

#[test]
fn actual_dictionary_id_request_counts_imported_and_new_with_enclosing_width() {
    let limits = Limits::default();
    let bytes = [0xff; 2];
    for width in 1_u8..=4 {
        let count = (1_u32 << width) - 1;
        let symbols: Vec<Bitmap> = (0..count)
            .map(|_| Bitmap::blank(1, 1, &limits).unwrap())
            .collect();
        for imported_count in 0..=symbols.len() {
            let (imported, new_symbols) = symbols.split_at(imported_count);
            let borrowed: Vec<&Bitmap> = imported.iter().collect();
            for book in [
                SymbolCodebook::Contiguous(imported),
                SymbolCodebook::Borrowed(&borrowed),
            ] {
                for position in 0..=8 {
                    for huffman in [false, true] {
                        let concrete = super::huffman_fields::codec(&bytes, position, 0);
                        let mut codec = SymbolCodec::Huffman(concrete);
                        let mut resources = ResourceTracker::default();
                        let result =
                            super::super::dictionary::decode_dictionary_single_instance_refinement(
                                &mut codec,
                                book,
                                new_symbols,
                                width,
                                1,
                                1,
                                0,
                                [(0, 0); 2],
                                huffman,
                                &limits,
                                &mut resources,
                            );
                        assert!(matches!(
                            result,
                            Err(SymbolTextError::SymbolIdOutOfRange { id, available })
                                if id == count && available == count
                        ));
                        assert_eq!(resources, ResourceTracker::default());
                        let SymbolCodec::Huffman(after) = codec else {
                            panic!("changed codec variant");
                        };
                        assert_eq!(after.reader.bit_position, position + usize::from(width));
                        assert_eq!(after.reader.bytes, bytes);
                        assert!(after.symbol_id_table.is_none());
                        assert_eq!(after.refinement_work, 71);
                    }
                }
            }
        }
    }
}

#[test]
fn single_instance_refinement_reaches_the_existing_raw_callback() {
    let limits = Limits::default();
    let reference = Bitmap::blank(1, 1, &limits).unwrap();
    let references = [&reference];
    for huffman in [false, true] {
        let mut integers = VecDeque::from([EntropyInteger::Value(0), EntropyInteger::Value(0)]);
        if huffman {
            integers.push_back(EntropyInteger::Value(2));
        }
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers,
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let mut generic_work = 0;
        let bitmap = decode_one_new_symbol(
            &header(huffman, true),
            &mut codec,
            SymbolCodebook::Borrowed(&references),
            &[],
            &mut None,
            &mut generic_work,
            Some(1),
            1,
            1,
            0,
            [(0, 0); 2],
            &limits,
            &mut resources,
        )
        .expect("one-instance refinement must not reject its required count");
        assert_eq!(
            (bitmap.width(), bitmap.height(), bitmap.bytes()),
            (1, 1, &[0][..])
        );
        assert_eq!(resources.work_units, 0);
        assert_eq!(resources.decoded_bitmap_bytes, 1);
        assert_eq!(generic_work, 0);
        let SymbolCodec::Scripted(codec) = codec else {
            unreachable!()
        };
        assert!(codec.integers.is_empty());
    }
}

#[test]
fn direct_coded_symbol_rejects_an_aggregation_count_before_decoding() {
    let limits = Limits::default();
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([EntropyInteger::Value(9)]),
        symbol_id: 0,
    });
    let mut resources = ResourceTracker::default();
    let mut generic_work = 0;
    let result = decode_one_new_symbol(
        &header(false, false),
        &mut codec,
        SymbolCodebook::Contiguous(&[]),
        &[],
        &mut None,
        &mut generic_work,
        Some(1),
        1,
        1,
        0,
        [(0, 0); 2],
        &limits,
        &mut resources,
    );
    assert!(matches!(
        result,
        Err(SymbolTextError::InvalidState(
            "aggregation count on direct-coded symbol",
        ))
    ));
    assert_eq!(resources, ResourceTracker::default());
    assert_eq!(generic_work, 0);
    let SymbolCodec::Scripted(codec) = codec else {
        unreachable!()
    };
    assert_eq!(codec.integers.len(), 1);
}

#[test]
fn refinement_still_rejects_missing_and_zero_instance_counts() {
    let limits = Limits::default();
    for count in [None, Some(0)] {
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers: VecDeque::new(),
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let mut generic_work = 0;
        let result = decode_one_new_symbol(
            &header(false, true),
            &mut codec,
            SymbolCodebook::Contiguous(&[]),
            &[],
            &mut None,
            &mut generic_work,
            count,
            1,
            1,
            0,
            [(0, 0); 2],
            &limits,
            &mut resources,
        );
        assert!(matches!(
            (count, result),
            (
                None,
                Err(SymbolTextError::InvalidState(
                    "missing aggregation-instance count"
                ))
            ) | (
                Some(0),
                Err(SymbolTextError::InvalidCount(
                    "zero-instance aggregate symbol"
                ))
            )
        ));
        assert_eq!(resources, ResourceTracker::default());
    }
}

fn dictionary_bits(bits: &str) -> Vec<u8> {
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (index, bit) in bits.bytes().enumerate() {
        assert!(bit == b'0' || bit == b'1');
        bytes[index / 8] |= (bit - b'0') << (7 - index % 8);
    }
    bytes
}

fn dictionary_export_run(value: usize) -> String {
    assert!(value <= 32);
    if value < 16 {
        format!("0{value:04b}")
    } else {
        format!("10{:08b}", value - 16)
    }
}
