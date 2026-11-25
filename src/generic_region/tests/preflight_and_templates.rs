#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn pattern_rows_match_literal_contexts_for_wide_lags_and_other_adaptive_shapes() {
    let mut random = 0x7d26_4f13_025c_9a6b;
    for width in [9, 33, 257, 513] {
        let bitmap = random_bitmap(width, 5, &mut random);
        for lag in [1, 7, 8, 9, 16, 23, 48, 128, 129, 255, 256, 1024, i32::MAX] {
            for moved in [false, true] {
                let mut adaptive =
                    generic_header(width, 5, GenericTemplate::Template0, false, false).adaptive;
                adaptive[0] = TemplateOffset::new(-lag, 0);
                if moved {
                    adaptive[2].x = 1;
                }
                let descriptor =
                    GenericContextDescriptor::new(GenericTemplate::Template0, false, &adaptive);
                for y in 0..7 {
                    let mut row = GenericRowContext::new(descriptor, &bitmap, y);
                    let mut pattern = super::super::pattern_context::pattern_lag(&[
                        adaptive[0],
                        adaptive[1],
                        adaptive[2],
                        adaptive[3],
                    ])
                    .map(|lag| {
                        super::super::pattern_context::PatternRowContext::new(&bitmap, y, lag)
                    });
                    for x in 0..width + 5 {
                        let history = control_window(&bitmap, i64::from(x), i64::from(y), -4, 4);
                        let expected = generic_context(
                            &bitmap,
                            x,
                            y,
                            GenericTemplate::Template0,
                            false,
                            &adaptive,
                        );
                        assert_eq!(descriptor.context(&bitmap, x, y, history), expected);
                        if x % 7 == 0 {
                            row.advance(&bitmap, x);
                            if let Some(pattern) = pattern.as_mut() {
                                pattern.advance(&bitmap, x);
                            }
                        } else {
                            if let Some(pattern) = pattern.as_mut() {
                                assert_eq!(
                                    pattern.context_and_advance(&bitmap, x, history),
                                    expected
                                );
                            }
                            assert_eq!(
                                row.context_and_advance(&bitmap, x, y, history),
                                expected,
                                "pattern lag{lag},x{x},y{y},moved{moved}",
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn raw_context_memory_guards_match_wide_arithmetic_and_priority() {
    let values = [0, 1, 2, u64::MAX / 2, u64::MAX / 2 + 1, u64::MAX];
    for bytes in values {
        for retained_limit in values {
            for memory_limit in values {
                let limits = Limits {
                    max_retained_state_bytes: retained_limit,
                    max_decoder_managed_memory_bytes: memory_limit,
                    ..Limits::default()
                };
                let combined = u128::from(bytes) * 2;
                let expected = if bytes > retained_limit {
                    Err(Some(LimitExceeded {
                        kind: LimitKind::RetainedState,
                        limit: retained_limit,
                        actual: bytes,
                    }))
                } else if combined > u128::from(u64::MAX) {
                    Err(None)
                } else if combined > u128::from(memory_limit) {
                    Err(Some(LimitExceeded {
                        kind: LimitKind::Memory,
                        limit: memory_limit,
                        actual: u64::try_from(combined).unwrap(),
                    }))
                } else {
                    Ok(())
                };
                assert_raw_guard_result(validate_snapshot_limits(bytes, &limits), expected);
                for bitmap_bytes in values {
                    let combined = u128::from(bytes) + u128::from(bitmap_bytes);
                    let expected = if combined > u128::from(u64::MAX) {
                        Err(None)
                    } else if combined > u128::from(memory_limit) {
                        Err(Some(LimitExceeded {
                            kind: LimitKind::Memory,
                            limit: memory_limit,
                            actual: u64::try_from(combined).unwrap(),
                        }))
                    } else {
                        Ok(())
                    };
                    assert_raw_guard_result(
                        enforce_managed_memory(bytes, bitmap_bytes, &limits),
                        expected,
                    );
                }
            }
        }
    }
    for count in [0, 1, 65_536, usize::MAX / 2, usize::MAX] {
        let wide = count as u128 * 2;
        assert_raw_guard_result(
            context_storage_bytes(count).map(|bytes| assert_eq!(u128::from(bytes), wide)),
            if wide > u128::from(u64::MAX) {
                Err(None)
            } else {
                Ok(())
            },
        );
    }
}

#[test]
fn raw_raster_work_guards_cover_exact_addition_overflow_boundaries() {
    for (width, rows) in [
        (0_u32, u32::MAX),
        (u32::MAX, 0),
        (1, 1),
        (65_535, 65_535),
        (u32::MAX, u32::MAX),
        (4_294_967_280, 252_645_136),
        (4_294_967_286, 306_783_379),
        (4_294_967_285, 186_737_709),
    ] {
        for typical in [false, true] {
            let pixels = u128::from(width) * u128::from(rows);
            let generic = pixels * 17 + if typical { u128::from(rows) } else { 0 };
            let refinement = pixels * if typical { 23 } else { 14 } + u128::from(rows);
            for (is_generic, expected) in [(true, generic), (false, refinement)] {
                let actual = if is_generic {
                    generic_work(width, rows, typical)
                } else {
                    refinement_work(width, rows, typical)
                };
                assert_raw_guard_result(
                    actual.map(|work| assert_eq!(u128::from(work), expected)),
                    if expected > u128::from(u64::MAX) {
                        Err(None)
                    } else {
                        Ok(())
                    },
                );
                let work = u64::try_from(expected).unwrap_or(u64::MAX);
                for limit in [work.saturating_sub(1), work, work.saturating_add(1)] {
                    let limits = Limits {
                        max_work_units: limit,
                        ..Limits::default()
                    };
                    let actual = if is_generic {
                        enforce_generic_work(width, rows, typical, &limits)
                    } else {
                        enforce_refinement_work(width, rows, typical, &limits)
                    };
                    let result = if expected > u128::from(u64::MAX) {
                        Err(None)
                    } else if expected > u128::from(limit) {
                        Err(Some(LimitExceeded {
                            kind: LimitKind::Work,
                            limit,
                            actual: work,
                        }))
                    } else {
                        Ok(())
                    };
                    assert_raw_guard_result(actual, result);
                }
            }
        }
    }
}

#[test]
fn raw_template_slots_preserve_offsets_planes_and_context_sizes() {
    for x in [i32::MIN, -1, 0, 1, i32::MAX] {
        for y in [i32::MIN, -1, 0, 1, i32::MAX] {
            let generic = generic_slot(x, y);
            assert_eq!(generic.nominal, TemplateOffset { x, y });
            assert_eq!(generic.adaptive, None);
            let target = target_slot(x, y);
            assert!(matches!(target.plane, RefinementPlane::Target));
            assert_eq!(target.nominal, TemplateOffset { x, y });
            assert_eq!(target.adaptive, None);
            let reference = reference_slot(x, y);
            assert!(matches!(reference.plane, RefinementPlane::Reference));
            assert_eq!(reference.nominal, TemplateOffset { x, y });
            assert_eq!(reference.adaptive, None);
            for number in [0, 1, 2, u8::MAX] {
                let generic = generic_at(x, y, number);
                assert_eq!(generic.nominal, TemplateOffset { x, y });
                assert_eq!(generic.adaptive, Some(number));
                let target = target_at(x, y, number);
                assert!(matches!(target.plane, RefinementPlane::Target));
                assert_eq!(target.nominal, TemplateOffset { x, y });
                assert_eq!(target.adaptive, Some(number));
                let reference = reference_at(x, y, number);
                assert!(matches!(reference.plane, RefinementPlane::Reference));
                assert_eq!(reference.nominal, TemplateOffset { x, y });
                assert_eq!(reference.adaptive, Some(number));
            }
        }
    }
    for (template, bits, sltp) in [
        (GenericTemplate::Template0, 16, 0x9b25),
        (GenericTemplate::Template1, 13, 0x0795),
        (GenericTemplate::Template2, 10, 0x00e5),
        (GenericTemplate::Template3, 10, 0x0195),
    ] {
        assert_eq!(template.context_bits(), bits);
        assert_eq!(template.sltp_context(), sltp);
    }
    for (template, bits, sltp) in [
        (RefinementTemplate::Template0, 13, 0x10),
        (RefinementTemplate::Template1, 10, 0x08),
    ] {
        assert_eq!(template.context_bits(), bits);
        assert_eq!(template.sltp_context(), sltp);
    }
}

#[test]
fn raw_context_descriptors_match_literal_sampling_and_history_masks() {
    let pixels = [
        [true, false, true, true, false, false, true, false, true],
        [false, true, false, false, true, true, false, true, false],
        [true, true, false, true, false, true, true, false, false],
        [false, false, true, false, true, false, false, true, true],
    ];
    let mut bitmap = Bitmap::blank(9, 4, &Limits::default()).unwrap();
    for (y, row) in pixels.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            bitmap
                .set_pixel(u32::try_from(x).unwrap(), u32::try_from(y).unwrap(), *value)
                .unwrap();
        }
    }
    let offset = TemplateOffset::new(-5, -3);
    for (descriptor, top, previous, adaptive, history_bits) in [
        (
            GenericContextDescriptor::NominalTemplate0,
            (-2, 5),
            (-3, 7),
            false,
            4,
        ),
        (
            GenericContextDescriptor::NominalTemplate1,
            (-1, 4),
            (-2, 6),
            false,
            3,
        ),
        (
            GenericContextDescriptor::NominalTemplate2,
            (-1, 3),
            (-2, 5),
            false,
            2,
        ),
        (
            GenericContextDescriptor::NominalTemplate3,
            (0, 0),
            (-3, 6),
            false,
            4,
        ),
        (
            GenericContextDescriptor::Template1(offset),
            (-1, 4),
            (-2, 5),
            true,
            3,
        ),
        (
            GenericContextDescriptor::Template2(offset),
            (-1, 3),
            (-2, 4),
            true,
            2,
        ),
        (
            GenericContextDescriptor::Template3(offset),
            (0, 0),
            (-3, 5),
            true,
            4,
        ),
    ] {
        for y in 0..6 {
            for x in 0..12 {
                for history in [0, 1, 0x55, usize::MAX] {
                    let mut expected = 0;
                    for (row_delta, (left, count)) in [(-2, top), (-1, previous)] {
                        for column_delta in left..left + count {
                            let px = i64::from(x) + column_delta;
                            let py = i64::from(y) + row_delta;
                            let value = (0..9).contains(&px)
                                && (0..4).contains(&py)
                                && pixels[usize::try_from(py).unwrap()]
                                    [usize::try_from(px).unwrap()];
                            expected = (expected << 1) | usize::from(value);
                        }
                    }
                    if adaptive {
                        let px = i64::from(x) - 5;
                        let py = i64::from(y) - 3;
                        let value = (0..9).contains(&px)
                            && (0..4).contains(&py)
                            && pixels[usize::try_from(py).unwrap()][usize::try_from(px).unwrap()];
                        expected = (expected << 1) | usize::from(value);
                    }
                    expected = (expected << history_bits) | (history & ((1 << history_bits) - 1));
                    assert_eq!(descriptor.context(&bitmap, x, y, history), expected);
                }
            }
        }
    }
}
