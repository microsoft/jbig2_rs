#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn work_charges_match_wide_results_and_attempted_charge_failure_state() {
    let values = [0, 1, 2, u64::MAX / 2, u64::MAX - 1, u64::MAX];
    let mut cases = 0;
    for initial in values {
        for units in values {
            for limit in values {
                let mut tracker = ResourceTracker {
                    work_units: initial,
                    decoded_bitmap_bytes: 37,
                };
                let limits = Limits {
                    max_work_units: limit,
                    ..Limits::default()
                };
                let total = u128::from(initial) + u128::from(units);
                let expected = if total > u128::from(u64::MAX) {
                    Err(None)
                } else if total > u128::from(limit) {
                    Err(Some(LimitExceeded {
                        kind: LimitKind::Work,
                        limit,
                        actual: u64::try_from(total).unwrap(),
                    }))
                } else {
                    Ok(())
                };
                assert_resource_guard_result(tracker.charge_work(units, &limits), expected);
                let charged = if total <= u128::from(u64::MAX) {
                    u64::try_from(total).unwrap()
                } else {
                    initial
                };
                assert_eq!(tracker.work_units, charged);
                assert_eq!(tracker.decoded_bitmap_bytes, 37);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 216);
}

#[test]
#[allow(clippy::too_many_lines)]
fn bitmap_charges_match_wide_priority_and_attempted_charge_failure_state() {
    let mut cases = 0;
    for width in [0_u32, 1, 8, 9, u32::MAX] {
        for height in [0_u32, 1, 2, u32::MAX] {
            let pixels = u128::from(width) * u128::from(height);
            let bytes = u128::from(width).div_ceil(8) * u128::from(height);
            for initial in [0, 1, u64::MAX / 2, u64::MAX] {
                let total = u128::from(initial) + bytes;
                let boundary = u64::try_from(total.min(u128::from(u64::MAX))).unwrap();
                for max_width in [width.saturating_sub(1), width, u32::MAX] {
                    for max_height in [height.saturating_sub(1), height, u32::MAX] {
                        let pixels = u64::try_from(pixels).unwrap();
                        for max_pixels in [pixels.saturating_sub(1), pixels, u64::MAX] {
                            for limit in [0, 1, boundary, u64::MAX] {
                                let limits = Limits {
                                    max_width,
                                    max_height,
                                    max_pixels_per_page: max_pixels,
                                    max_cumulative_decoded_bitmap_bytes: limit,
                                    ..Limits::default()
                                };
                                let dimension_error = if width > max_width {
                                    Some(LimitExceeded {
                                        kind: LimitKind::Width,
                                        limit: u64::from(max_width),
                                        actual: u64::from(width),
                                    })
                                } else if height > max_height {
                                    Some(LimitExceeded {
                                        kind: LimitKind::Height,
                                        limit: u64::from(max_height),
                                        actual: u64::from(height),
                                    })
                                } else if pixels > max_pixels {
                                    Some(LimitExceeded {
                                        kind: LimitKind::PagePixels,
                                        limit: max_pixels,
                                        actual: pixels,
                                    })
                                } else {
                                    None
                                };
                                let expected = if let Some(error) = dimension_error {
                                    Err(Some(error))
                                } else if total > u128::from(u64::MAX) {
                                    Err(None)
                                } else if total > u128::from(limit) {
                                    Err(Some(LimitExceeded {
                                        kind: LimitKind::DecodedBitmapBytes,
                                        limit,
                                        actual: u64::try_from(total).unwrap(),
                                    }))
                                } else {
                                    Ok(())
                                };
                                let mut tracker = ResourceTracker {
                                    work_units: 37,
                                    decoded_bitmap_bytes: initial,
                                };
                                assert_resource_guard_result(
                                    tracker.charge_bitmap(width, height, &limits),
                                    expected,
                                );
                                let charged =
                                    if dimension_error.is_none() && total <= u128::from(u64::MAX) {
                                        u64::try_from(total).unwrap()
                                    } else {
                                        initial
                                    };
                                assert_eq!(tracker.work_units, 37);
                                assert_eq!(tracker.decoded_bitmap_bytes, charged);
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 8_640);
}

#[test]
fn raw_symbol_counts_and_deltas_match_wide_integer_domains() {
    let values = [0, 1, 2, u32::MAX - 1, u32::MAX];
    for imported in values {
        for new_symbols in values {
            for exported in values {
                for maximum in values {
                    let limits = Limits {
                        max_symbols: maximum,
                        ..Limits::default()
                    };
                    let total = u64::from(imported) + u64::from(new_symbols);
                    let result = check_symbol_counts(imported, new_symbols, exported, &limits);
                    if total > u64::from(u32::MAX) {
                        assert!(matches!(
                            result,
                            Err(SymbolTextError::Decode(DecodeError::Malformed(
                                MalformedError::IntegerOverflow
                            )))
                        ));
                    } else if total > u64::from(maximum) {
                        assert!(matches!(result, Err(SymbolTextError::Decode(
                            DecodeError::LimitExceeded(LimitExceeded {
                                kind: LimitKind::Symbols, limit, actual,
                            })
                        )) if limit == u64::from(maximum) && actual == total));
                    } else if u64::from(exported) > total {
                        assert!(matches!(
                            result,
                            Err(SymbolTextError::InvalidCount(
                                "exported symbols exceed candidates"
                            ))
                        ));
                    } else {
                        assert_eq!(u64::from(result.unwrap()), total);
                    }
                }
            }
        }
    }
    for current in values {
        for delta in [
            i64::MIN,
            -i64::from(u32::MAX) - 1,
            -1,
            0,
            1,
            i64::from(u32::MAX),
            i64::MAX,
        ] {
            for maximum in values {
                let sum = i128::from(current) + i128::from(delta);
                let result = checked_unsigned_delta(current, delta, maximum);
                if sum > i128::from(i64::MAX) {
                    assert!(matches!(
                        result,
                        Err(SymbolTextError::Decode(DecodeError::Malformed(
                            MalformedError::IntegerOverflow
                        )))
                    ));
                } else if sum < 0 || sum > i128::from(maximum) {
                    assert!(matches!(result, Err(SymbolTextError::InvalidDimensions)));
                } else {
                    assert_eq!(i128::from(result.unwrap()), sum);
                }
            }
        }
    }
    for power in 0..=32 {
        let boundary = 1_u64 << power;
        for count in [boundary.saturating_sub(1), boundary, boundary + 1] {
            if let Ok(count) = u32::try_from(count) {
                let length = symbol_id_code_length(count).unwrap();
                assert!((1_u64 << length) >= u64::from(count));
                assert!(length == 0 || (1_u64 << (length - 1)) < u64::from(count));
            }
        }
    }
}

#[test]
fn checks_cumulative_dictionary_dimensions_and_counts() {
    let limits = Limits {
        max_width: 8,
        max_height: 8,
        ..Limits::default()
    };
    let mut resources = ResourceTracker::default();
    let mut state = DictionaryClassState::new(2, &limits).expect("state");
    assert_eq!(
        state
            .begin_height_class(2, &limits, &mut resources)
            .expect("height"),
        2
    );
    assert_eq!(
        state
            .add_symbol(2, &limits, &mut resources)
            .expect("first symbol"),
        (2, 2)
    );
    assert_eq!(
        state
            .add_symbol(1, &limits, &mut resources)
            .expect("second symbol"),
        (3, 2)
    );
    assert_eq!(state.finish_height_class().expect("finish class"), 5);
    state.finish().expect("exact symbol count");

    let mut state = DictionaryClassState::new(1, &limits).expect("state");
    assert!(matches!(
        state.begin_height_class(-1, &limits, &mut resources),
        Err(SymbolTextError::InvalidDimensions)
    ));
}

#[test]
fn individual_symbols_do_not_charge_a_nonexistent_collective_bitmap_width() {
    let limits = Limits {
        max_width: 8,
        max_height: 1,
        max_pixels_per_page: 8,
        ..Limits::default()
    };
    let mut resources = ResourceTracker::default();
    let mut individual = DictionaryClassState::new(2, &limits).unwrap();
    individual
        .begin_height_class(1, &limits, &mut resources)
        .unwrap();
    assert_eq!(
        individual
            .add_individual_symbol(8, &limits, &mut resources)
            .unwrap(),
        (8, 1)
    );
    assert_eq!(
        individual
            .add_individual_symbol(0, &limits, &mut resources)
            .unwrap(),
        (8, 1)
    );
    individual.finish_height_class().unwrap();
    individual.finish().unwrap();
    let mut collective = DictionaryClassState::new(2, &limits).unwrap();
    collective
        .begin_height_class(1, &limits, &mut resources)
        .unwrap();
    collective.add_symbol(8, &limits, &mut resources).unwrap();
    assert!(matches!(
        collective.add_symbol(0, &limits, &mut resources),
        Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Width,
                limit: 8,
                actual: 16,
            }
        )))
    ));
    let mut too_wide = DictionaryClassState::new(1, &limits).unwrap();
    too_wide
        .begin_height_class(1, &limits, &mut resources)
        .unwrap();
    assert!(matches!(
        too_wide.add_individual_symbol(9, &limits, &mut resources),
        Err(SymbolTextError::InvalidDimensions)
    ));
}

fn independent_symbol_width(count: u32) -> u8 {
    let mut capacity = 1_u64;
    let mut width = 0_u8;
    while capacity < u64::from(count) {
        capacity *= 2;
        width += 1;
    }
    width
}

fn assert_minimal_symbol_width(count: u32) {
    let width = symbol_id_code_length(count).expect("all u32 counts have a symbol width");
    assert_eq!(width, independent_symbol_width(count), "count={count}");
    assert!(width <= 32);
    assert_eq!(width == 0, count <= 1);
    assert!(u64::from(count) <= 1_u64 << width);
    for smaller in 0..width {
        assert!(
            1_u64 << smaller < u64::from(count),
            "count={count}, smaller={smaller}"
        );
    }
}

#[test]
fn symbol_width_thresholds_zero_max_and_every_class_are_exact() {
    assert_eq!(symbol_id_code_length(0).unwrap(), 0);
    assert_eq!(symbol_id_code_length(1).unwrap(), 0);
    assert_eq!(symbol_id_code_length(u32::MAX).unwrap(), 32);
    let mut reached = [false; 33];
    for exponent in 0..=32 {
        let boundary = 1_i64 << exponent;
        for delta in -8..=8 {
            if let Ok(count) = u32::try_from(boundary + delta) {
                assert_minimal_symbol_width(count);
                reached[usize::from(symbol_id_code_length(count).unwrap())] = true;
            }
        }
    }
    assert!(reached.into_iter().all(|value| value));
}

#[test]
fn symbol_width_dense_and_stratified_counts_match_independent_capacity_oracle() {
    for count in 0..=65_536 {
        assert_minimal_symbol_width(count);
    }
    let mut state = 0x9e37_79b9_u32;
    for _ in 0..65_536 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        assert_minimal_symbol_width(state);
    }
}

#[test]
fn decodes_export_runs_including_zero_length_toggles() {
    let limits = Limits::default();
    let mut resources = ResourceTracker::default();
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([
            EntropyInteger::Value(2),
            EntropyInteger::Value(1),
            EntropyInteger::Value(2),
        ]),
        symbol_id: 0,
    });
    let exported =
        decode_export_runs(&mut codec, 5, 1, &limits, &mut resources).expect("valid export runs");
    assert_eq!(exported, vec![2]);

    let mut resources = ResourceTracker::default();
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([
            EntropyInteger::Value(0),
            EntropyInteger::Value(0),
            EntropyInteger::Value(5),
        ]),
        symbol_id: 0,
    });
    let exported = decode_export_runs(&mut codec, 5, 0, &limits, &mut resources)
        .expect("zero runs make input progress");
    assert!(exported.is_empty());
}

#[test]
fn bounds_nonprogressing_export_runs_by_work_limit() {
    let limits = Limits {
        max_work_units: 2,
        ..Limits::default()
    };
    let mut resources = ResourceTracker::default();
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([
            EntropyInteger::Value(0),
            EntropyInteger::Value(0),
            EntropyInteger::Value(0),
        ]),
        symbol_id: 0,
    });
    let result = decode_export_runs(&mut codec, 1, 0, &limits, &mut resources);
    assert!(matches!(
        result,
        Err(SymbolTextError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Work,
                ..
            }
        )))
    ));
}

#[test]
fn legal_empty_classes_and_interleaved_zero_runs_have_exact_work_charges() {
    let limits = Limits::default();
    let mut resources = ResourceTracker::default();
    let mut state = DictionaryClassState::new(1, &limits).unwrap();
    for _ in 0..257 {
        state
            .begin_height_class(0, &limits, &mut resources)
            .unwrap();
        assert_eq!(state.finish_height_class().unwrap(), 0);
        assert_eq!(state.decoded_symbols, 0);
    }
    state
        .begin_height_class(1, &limits, &mut resources)
        .unwrap();
    assert_eq!(
        state
            .add_individual_symbol(1, &limits, &mut resources)
            .unwrap(),
        (1, 1)
    );
    state.finish_height_class().unwrap();
    state.finish().unwrap();
    assert_eq!(resources.work_units, 259);
    let mut integers = VecDeque::new();
    // Each pair leaves the export flag unchanged but consumes two integers.
    for run in [1, 2, 1] {
        integers.extend(std::iter::repeat_n(EntropyInteger::Value(0), 258));
        integers.push_back(EntropyInteger::Value(run));
    }
    integers.push_back(EntropyInteger::Value(91)); // Following field.
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers,
        symbol_id: 0,
    });
    assert_eq!(
        decode_export_runs(&mut codec, 4, 2, &limits, &mut resources).unwrap(),
        vec![1, 2]
    );
    assert_eq!(resources.work_units, 259 + 3 * 259);
    assert!(matches!(
        codec.decode_integer(IntegerField::ExportRun),
        Ok(EntropyInteger::Value(91))
    ));
}

#[test]
fn exported_symbols_match_exact_clone_oracle() {
    type Descriptor<'a> = (u32, u32, usize, &'a [u8]);
    let limits = Limits::default();
    let descriptions: [Descriptor<'_>; 6] = [
        (8, 1, 1, &[0x11]),
        (16, 2, 2, &[0x21, 0x22, 0x23, 0x24]),
        (1, 3, 1, &[0x80, 0, 0x80]),
        (9, 1, 2, &[0x81, 0x80]),
        (0, 2, 0, &[]),
        (8, 2, 1, &[0x5A, 0xA5]),
    ];
    let symbols: Vec<Bitmap> = descriptions
        .iter()
        .map(|&(width, height, stride, data)| Bitmap {
            width,
            height,
            stride,
            data: data.to_vec(),
        })
        .collect();
    let (imported, new_symbols) = symbols.split_at(3);
    let borrowed: Vec<&Bitmap> = imported.iter().collect();
    let selections: [&[u32]; 8] = [
        &[],
        &[0],
        &[2],
        &[3],
        &[5],
        &[0, 1, 2, 3, 4, 5],
        &[5, 2, 0, 5, 3, 4, 1, 0],
        &[1, 1, 1, 1],
    ];
    for codebook in [
        SymbolCodebook::Contiguous(imported),
        SymbolCodebook::Borrowed(&borrowed),
    ] {
        for selected in selections {
            let output = assemble_exported_symbols(codebook, new_symbols, selected, &limits)
                .expect("assembled raw symbols");
            assert_eq!(output.len(), selected.len());
            assert!(output.capacity() >= selected.len());
            for (offset, &id) in selected.iter().enumerate() {
                let id = usize::try_from(id).unwrap();
                let (width, height, stride, data) = descriptions[id];
                let bitmap = &output[offset];
                assert_eq!(
                    (bitmap.width, bitmap.height, bitmap.stride),
                    (width, height, stride)
                );
                assert_eq!(bitmap.data.as_slice(), data);
                if !data.is_empty() {
                    assert_ne!(bitmap.data.as_ptr(), symbols[id].data.as_ptr());
                    for earlier in &output[..offset] {
                        if !earlier.data.is_empty() {
                            assert_ne!(bitmap.data.as_ptr(), earlier.data.as_ptr());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sparse_new_exports_preserve_scalar_order() {
    let limits = Limits::default();
    let mut symbols = Vec::new();
    for value in 0_u8..8 {
        symbols.push(Bitmap {
            width: 8,
            height: 1,
            stride: 1,
            data: vec![value],
        });
    }
    let output = assemble_exported_symbols(
        SymbolCodebook::Contiguous(&[]),
        &symbols,
        &[0, 3, 7],
        &limits,
    )
    .expect("sparse exports");
    assert_eq!(
        output
            .iter()
            .map(|bitmap| bitmap.data[0])
            .collect::<Vec<_>>(),
        vec![0, 3, 7]
    );
}

#[test]
fn borrowed_symbol_buffer_reuses_pointer_capacity() {
    let limits = Limits::default();
    let symbols = [
        Bitmap::blank(8, 1, &limits).expect("symbol"),
        Bitmap::blank(8, 1, &limits).expect("symbol"),
    ];
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve(symbols.len())
        .expect("pointer reserve");
    for symbol in &symbols {
        borrowed.push(symbol);
    }
    let pointer = borrowed.as_ptr();
    let capacity = borrowed.capacity();
    borrowed.clear();
    for symbol in &symbols {
        borrowed.push(symbol);
    }
    assert_eq!(borrowed.as_ptr(), pointer);
    assert_eq!(borrowed.capacity(), capacity);
}
