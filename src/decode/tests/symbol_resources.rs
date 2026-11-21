#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn dictionary_context_selection_uses_reference_order_not_retention_or_store_order() {
    let symbols: SymbolStore = [30, 10, 20]
        .into_iter()
        .map(|number| {
            (
                number,
                SymbolResource {
                    exported: Vec::new(),
                    contexts: None,
                },
            )
        })
        .collect();
    for length in 0..=4 {
        for mut encoded in 0..4_usize.pow(length) {
            let mut references = Vec::new();
            for _ in 0..length {
                references.push([10, 20, 30, 99][encoded % 4]);
                encoded /= 4;
            }
            let expected = references.iter().rev().find_map(|number| {
                symbols
                    .iter()
                    .position(|(candidate, _)| candidate == number)
            });
            for used in [false, true] {
                assert_eq!(
                    super::super::dispatch::dictionary_context_source(&symbols, &references, used),
                    if used { expected } else { None },
                );
            }
        }
    }
}

fn retained_refinement_dictionary_resource() -> SymbolResource {
    use crate::arithmetic::ArithmeticContext;
    use crate::generic_region::{RefinementContexts, RefinementTemplate};
    let limits = Limits::default();
    let mut payload = 0x1203_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&[0, 0x40]); // Export the imported bitmap.
    let (model, _) = parse_symbol_dictionary_header(&payload, &limits).unwrap();
    let mut contexts = RefinementContexts::new(RefinementTemplate::Template1, &limits).unwrap();
    contexts.contexts[8] = ArithmeticContext::from_state(10, 1).unwrap();
    let mut bitmap = Bitmap::blank(8, 1, &limits).unwrap();
    bitmap.data[0] = 0xa5;
    SymbolResource {
        exported: vec![bitmap],
        contexts: Some(DictionaryContexts {
            model,
            generic: None,
            refinement: Some(contexts.try_snapshot(&limits).unwrap()),
        }),
    }
}

#[test]
fn dictionary_dispatch_imports_pixels_but_only_restores_requested_last_contexts() {
    use crate::arithmetic::ArithmeticContext;
    use crate::generic_region::{RefinementContexts, RefinementTemplate};
    for used in [false, true] {
        for later_dictionary in [false, true] {
            let mut symbols = vec![(10, retained_refinement_dictionary_resource())];
            // The final referred segment is a table, not a dictionary. It must
            // not erase the prescribed dictionary source.
            let references = if later_dictionary {
                symbols.push((
                    20,
                    SymbolResource {
                        exported: Vec::new(),
                        contexts: None,
                    },
                ));
                vec![10, 20, 99]
            } else {
                vec![10, 99]
            };
            let mut segment = symbol_reference_segment(&references);
            let mut payload = (0x1203_u16 | if used { 0x0100 } else { 0 })
                .to_be_bytes()
                .to_vec();
            payload.extend_from_slice(&1_u32.to_be_bytes());
            payload.extend_from_slice(&0_u32.to_be_bytes());
            payload.extend_from_slice(&[0, 0x40]);
            segment.payload = &payload;
            let result = super::super::dispatch::decode_symbol_dictionary_segment(
                &segment,
                &mut symbols,
                &Vec::new(),
                &mut RetainedBudget::new(),
                &mut DecodeWork::new(0, &Limits::default()).unwrap(),
                &Limits::default(),
            );
            if used && later_dictionary {
                assert!(matches!(
                    result,
                    Err(DecodeError::Malformed(MalformedError::InvalidSymbolText(
                        "missing retained contexts"
                    )))
                ));
                assert_eq!(symbols.len(), 2);
                continue;
            }
            result.unwrap();
            let output = get_symbol_resource(&symbols, 100).unwrap();
            assert_eq!(output.exported.len(), 1);
            assert_eq!(output.exported[0].bytes(), &[0xa5]);
            let mut restored =
                RefinementContexts::new(RefinementTemplate::Template1, &Limits::default()).unwrap();
            restored
                .restore(
                    output
                        .contexts
                        .as_ref()
                        .unwrap()
                        .refinement
                        .as_ref()
                        .unwrap(),
                    &Limits::default(),
                )
                .unwrap();
            for (index, state) in restored.contexts.iter().enumerate() {
                let expected = if used && index == 8 {
                    ArithmeticContext::from_state(10, 1).unwrap()
                } else {
                    ArithmeticContext::new()
                };
                assert_eq!(*state, expected, "used={used}, index={index}");
            }
        }
    }
}

#[test]
fn dictionary_dispatch_can_import_arithmetic_symbols_into_fresh_huffman_refinement() {
    let mut payload = 0x0200_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    payload.extend_from_slice(&[0; 8]);
    payload.extend_from_slice(&[0xff, 0xac]);
    let source = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &payload,
        imported_symbols: crate::symbol_text::SymbolCodebook::Contiguous(&[]),
        custom_tables: &[],
        used_contexts: None,
        limits: &Limits::default(),
    })
    .unwrap();
    let mut resource = retained_refinement_dictionary_resource();
    resource.contexts = source.retained_contexts;
    let mut symbols = vec![(10, resource)];
    let mut segment = symbol_reference_segment(&[10]);
    let mut payload = 0x1203_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&[0, 0x40]);
    segment.payload = &payload;
    super::super::dispatch::decode_symbol_dictionary_segment(
        &segment,
        &mut symbols,
        &Vec::new(),
        &mut RetainedBudget::new(),
        &mut DecodeWork::new(0, &Limits::default()).unwrap(),
        &Limits::default(),
    )
    .unwrap();
    let output = get_symbol_resource(&symbols, 100).unwrap();
    assert_eq!(output.exported[0].bytes(), &[0xa5]);
    let contexts = output.contexts.as_ref().unwrap();
    assert!(contexts.generic.is_none());
    assert!(contexts.refinement.is_some());
}

#[test]
fn standalone_dictionary_model_controls_are_framed_independently_of_rejection() {
    let first = [
        0x0e, 0x00, 0xff, 0xff, // Retained generic template 3, AT=(-1,-1).
        0, 0, 0, 0, 0, 0, 0, 0, // No exported or new symbols.
        0xff, 0xac,
    ];
    for (flags, adaptive_x, expected) in [
        (0x0f00_u16, 0xff, None), // Matching reuse.
        (
            0x0b00,
            0xff,
            Some("incompatible retained dictionary context model"),
        ),
        (
            0x0f00,
            0xfe,
            Some("incompatible retained dictionary context model"),
        ),
        (0x0a00, 0xfe, None), // Different model is legal with fresh contexts.
    ] {
        let mut second = first;
        second[..2].copy_from_slice(&flags.to_be_bytes());
        second[2] = adaptive_x;
        let mut document = SIGNATURE.to_vec();
        document.push(1);
        document.extend_from_slice(&1_u32.to_be_bytes());
        document.extend_from_slice(&segment_header(0, 48, 1, 19));
        document.extend_from_slice(&page_data());
        let mut dictionary_header = segment_header(1, 0, 0, 14);
        dictionary_header[5] = 1; // Retain segment 1 for the following reference.
        document.extend_from_slice(&dictionary_header);
        document.extend_from_slice(&first);
        document.extend_from_slice(&referenced_segment_header(2, 0, 0, 1, 14));
        document.extend_from_slice(&second);
        document.extend_from_slice(&segment_header(3, 49, 1, 0));
        document.extend_from_slice(&segment_header(4, 51, 0, 0));
        match (crate::Decoder::default().decode(&document), expected) {
            (Ok(decoded), None) => {
                assert_eq!(decoded.pages().len(), 1);
                assert_eq!(decoded.pages()[0].bitmap().bytes(), &[0]);
            }
            (
                Err(DecodeError::Malformed(MalformedError::InvalidSymbolText(message))),
                Some(expected),
            ) => assert_eq!(message, expected),
            (actual, _) => panic!("unexpected model-control outcome: {actual:?}"),
        }
    }
}

#[test]
fn borrowed_symbol_memory_counts_only_pointer_capacity() {
    let retained = 4_096_u64;
    let count = 37_usize;
    let (allocation, peak) =
        borrowed_symbol_memory(count, retained).expect("representable pointer storage");
    assert_eq!(
        allocation,
        u64::try_from(count * core::mem::size_of::<&Bitmap>()).expect("allocation")
    );
    assert_eq!(peak, retained + allocation);
}

#[test]
fn borrowed_symbol_collection_enforces_observed_capacity() {
    let mut symbols = Vec::new();
    for number in [10, 20] {
        symbols.push((
            number,
            SymbolResource {
                exported: vec![Bitmap::blank(9, 1, &Limits::default()).unwrap()],
                contexts: None,
            },
        ));
    }
    let segment = symbol_reference_segment(&[10, 20]);
    let mut probe: Vec<&Bitmap> = Vec::new();
    probe.try_reserve(2).unwrap();
    let requested_bytes = u64::try_from(2 * core::mem::size_of::<&Bitmap>()).unwrap();
    let observed_bytes = u64::try_from(probe.capacity() * core::mem::size_of::<&Bitmap>()).unwrap();
    let retained = 4096;
    let requested_peak = retained + requested_bytes;
    let observed_peak = retained + observed_bytes;
    for limit in [requested_peak - 1, observed_peak - 1, observed_peak] {
        let limits = Limits {
            max_decoder_managed_memory_bytes: limit,
            ..Limits::default()
        };
        match collect_referred_symbols(&segment, &symbols, retained, &limits) {
            Ok(collected) => {
                let CollectedSymbols::Borrowed(pointers) = collected else {
                    panic!("two nonempty resources need borrowed pointers");
                };
                let actual =
                    u64::try_from(pointers.capacity() * core::mem::size_of::<&Bitmap>()).unwrap();
                assert!(retained + actual <= limit);
                assert_eq!(pointers.len(), 2);
            }
            Err(DecodeError::LimitExceeded(error)) => {
                assert!(limit < observed_peak);
                assert_eq!(error.kind, crate::LimitKind::Memory);
                assert_eq!(error.limit, limit);
                assert_eq!(
                    error.actual,
                    if limit < requested_peak {
                        requested_peak
                    } else {
                        observed_peak
                    },
                );
            }
            Err(error) => panic!("unexpected borrowed capacity failure: {error:?}"),
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn referred_symbol_collection_preserves_order_identity_and_exact_borrow_budget() {
    let mut cases = 0;
    let mut borrowed_cases = 0;
    for first_count in 0..=2 {
        for second_count in 0..=2 {
            for third_count in 0..=2 {
                let mut store = Vec::new();
                for (number, count) in [(10, first_count), (20, second_count), (30, third_count)] {
                    let mut exported = Vec::new();
                    for index in 0..count {
                        let mut bitmap = Bitmap::blank(9, 1, &Limits::default()).unwrap();
                        bitmap.data[0] = u8::try_from(number + index).unwrap();
                        bitmap.data[1] = u8::try_from(number).unwrap();
                        exported.push(bitmap);
                    }
                    store.push((
                        number,
                        SymbolResource {
                            exported,
                            contexts: None,
                        },
                    ));
                }
                for length in 0..=4 {
                    for mut encoded in 0..4_usize.pow(length) {
                        let mut references = Vec::new();
                        for _ in 0..length {
                            references.push([10, 20, 30, 99][encoded % 4]);
                            encoded /= 4;
                        }
                        let segment = symbol_reference_segment(&references);
                        let mut expected = Vec::new();
                        let mut nonempty_groups = 0;
                        for number in &references {
                            if let Some((_, resource)) =
                                store.iter().find(|(candidate, _)| candidate == number)
                            {
                                nonempty_groups += usize::from(!resource.exported.is_empty());
                                expected.extend(resource.exported.iter());
                            }
                        }
                        let retained = 4096;
                        let requested_extra = if nonempty_groups > 1 {
                            u64::try_from(expected.len() * core::mem::size_of::<&Bitmap>()).unwrap()
                        } else {
                            0
                        };
                        let probe = collect_referred_symbols(
                            &segment,
                            &store,
                            retained,
                            &Limits::default(),
                        )
                        .unwrap();
                        let observed_extra = match &probe {
                            CollectedSymbols::Contiguous(_) => 0,
                            CollectedSymbols::Borrowed(symbols) => {
                                u64::try_from(symbols.capacity() * core::mem::size_of::<&Bitmap>())
                                    .unwrap()
                            }
                        };
                        assert!(observed_extra >= requested_extra);
                        drop(probe);
                        let peak = retained + observed_extra;
                        let limits = Limits {
                            max_decoder_managed_memory_bytes: peak,
                            ..Limits::default()
                        };
                        let collected =
                            collect_referred_symbols(&segment, &store, retained, &limits).unwrap();
                        if let CollectedSymbols::Borrowed(symbols) = &collected {
                            let actual_extra =
                                u64::try_from(symbols.capacity() * core::mem::size_of::<&Bitmap>())
                                    .unwrap();
                            assert!(
                                retained + actual_extra <= limits.max_decoder_managed_memory_bytes
                            );
                        }
                        match collected.codebook() {
                            SymbolCodebook::Contiguous(symbols) => {
                                assert!(nonempty_groups <= 1);
                                assert_eq!(symbols.len(), expected.len());
                                for (actual, expected) in symbols.iter().zip(&expected) {
                                    assert!(core::ptr::eq(actual, *expected));
                                }
                            }
                            SymbolCodebook::Borrowed(symbols) => {
                                assert!(nonempty_groups > 1);
                                assert_eq!(symbols.len(), expected.len());
                                for (actual, expected) in symbols.iter().zip(&expected) {
                                    assert!(core::ptr::eq(*actual, *expected));
                                }
                            }
                        }
                        if nonempty_groups > 1 {
                            let tight = Limits {
                                max_decoder_managed_memory_bytes: retained + requested_extra - 1,
                                ..Limits::default()
                            };
                            match collect_referred_symbols(&segment, &store, retained, &tight) {
                                Err(DecodeError::LimitExceeded(error)) => {
                                    assert_eq!(error.kind, crate::LimitKind::Memory);
                                    assert_eq!(error.limit, retained + requested_extra - 1);
                                    assert_eq!(error.actual, retained + requested_extra);
                                }
                                Err(other) => {
                                    panic!("unexpected borrowed-pointer error: {other:?}")
                                }
                                Ok(_) => {
                                    panic!("one byte below requested pointer minimum succeeded")
                                }
                            }
                            borrowed_cases += 1;
                        }
                        assert_eq!(segment.header.referred_segments, references);
                        assert!(store
                            .iter()
                            .all(|(_, resource)| resource.contexts.is_none()));
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 9207);
    assert_eq!(borrowed_cases, 5460);
}

#[test]
fn borrowed_symbol_memory_numeric_boundaries_match_wide_preflight() {
    let pointer_size = core::mem::size_of::<&Bitmap>();
    let mut cases = 0;
    for count in [0, 1, 37, usize::MAX / pointer_size, usize::MAX] {
        for retained in [0, 4096, u64::MAX - 1, u64::MAX] {
            let requested = (count as u128) * (pointer_size as u128);
            let peak = requested + u128::from(retained);
            let fits = requested <= usize::MAX as u128
                && requested <= u128::from(u64::MAX)
                && peak <= u128::from(u64::MAX);
            match borrowed_symbol_memory(count, retained) {
                Ok((actual_bytes, actual_peak)) => {
                    assert!(fits);
                    assert_eq!(u128::from(actual_bytes), requested);
                    assert_eq!(u128::from(actual_peak), peak);
                }
                Err(DecodeError::Malformed(MalformedError::IntegerOverflow)) => {
                    assert!(!fits);
                }
                other => panic!("unexpected numeric preflight result: {other:?}"),
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 20);
}

#[test]
fn first_matching_symbol_group_controls_duplicate_keys_and_collection_count() {
    let bitmap = Bitmap::blank(9, 1, &Limits::default()).unwrap();
    let symbols = vec![
        (
            10,
            SymbolResource {
                exported: vec![bitmap.clone()],
                contexts: None,
            },
        ),
        (
            10,
            SymbolResource {
                exported: vec![bitmap.clone(), bitmap],
                contexts: None,
            },
        ),
        (
            20,
            SymbolResource {
                exported: Vec::new(),
                contexts: None,
            },
        ),
    ];
    assert_eq!(symbol_index(&symbols, 10), Some(0));
    assert_eq!(get_symbol_resource(&symbols, 10).unwrap().exported.len(), 1);
    assert_eq!(symbol_index(&symbols, 20), Some(2));
    assert_eq!(get_symbol_resource(&symbols, 20).unwrap().exported.len(), 0);
    assert_eq!(symbol_index(&symbols, 99), None);
    assert!(get_symbol_resource(&symbols, 99).is_none());
    let segment = symbol_reference_segment(&[10, 10, 20, 99]);
    let collected = collect_referred_symbols(&segment, &symbols, 0, &Limits::default()).unwrap();
    let SymbolCodebook::Borrowed(bitmaps) = collected.codebook() else {
        panic!("two nonempty reference occurrences require borrowed storage");
    };
    assert_eq!(bitmaps.len(), 2);
    let expected = &symbols[0].1.exported[0];
    assert!(bitmaps
        .iter()
        .all(|bitmap| core::ptr::eq(*bitmap, expected)));
}
