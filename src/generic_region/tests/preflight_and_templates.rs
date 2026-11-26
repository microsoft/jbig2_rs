#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

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

#[test]
fn scalar_generic_fallback_handles_extended_and_non_nominal_templates() {
    let limits = Limits::default();
    let coded = arithmetic_bytes(&mut 0x9988_7766_5544_3322);
    for (template, extended, adaptive) in [
        (
            GenericTemplate::Template0,
            true,
            [
                TemplateOffset::new(-2, 0),
                TemplateOffset::new(0, -2),
                TemplateOffset::new(-2, -1),
                TemplateOffset::new(-1, -2),
                TemplateOffset::new(1, -2),
                TemplateOffset::new(2, -1),
                TemplateOffset::new(-3, 0),
                TemplateOffset::new(-4, 0),
                TemplateOffset::new(2, -2),
                TemplateOffset::new(3, -1),
                TemplateOffset::new(-2, -2),
                TemplateOffset::new(-3, -1),
            ],
        ),
        (
            GenericTemplate::Template1,
            false,
            [
                TemplateOffset::new(2, -1),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
                TemplateOffset::new(0, 0),
            ],
        ),
    ] {
        let mut dec_owned = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut dec_into = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut ctx_owned =
            GenericContexts::new(template, extended, &limits).expect("context bank");
        let mut ctx_into = GenericContexts::new(template, extended, &limits).expect("context bank");
        let call = GenericArithmeticCall {
            width: 11,
            height: 4,
            adaptive,
            typical_prediction: true,
            skip: None,
        };
        let mut work_owned = 0_u64;
        let owned = decode_generic_on(
            &mut dec_owned,
            &mut ctx_owned,
            call,
            4,
            &limits,
            &mut work_owned,
        )
        .expect("scalar owned fallback");
        let mut target = Bitmap::blank(11, 4, &limits).expect("target bitmap");
        let mut work_into = 0_u64;
        decode_generic_on_into(
            &mut dec_into,
            &mut ctx_into,
            call,
            4,
            &limits,
            &mut work_into,
            &mut target,
        )
        .expect("scalar into fallback");
        assert_eq!(owned, target);
        assert_eq!(ctx_owned.contexts, ctx_into.contexts);
        assert_decoder_state_eq(&dec_owned, &dec_into);
        assert!(!crate::generic_region::support::sample_signed(
            &target,
            None,
            Some(0)
        ));
    }
}

#[test]
fn generic_contexts_scratch_reuses_capacity_zeroes_entries_and_enforces_limits() {
    let limits = Limits::default();
    let mut scratch = Vec::new();
    let mut first =
        GenericContexts::from_scratch(GenericTemplate::Template0, false, &limits, &mut scratch)
            .expect("first template0");
    assert_eq!(first.contexts.len(), 65_536);
    let initial_cap = first.contexts.capacity();
    let initial_ptr = first.contexts.as_ptr();
    first.contexts[0] = ArithmeticContext::from_state(7, 1).unwrap();
    first.contexts[65_535] = ArithmeticContext::from_state(12, 1).unwrap();
    first.into_scratch(&mut scratch);
    assert_eq!(scratch.capacity(), initial_cap);

    // Reuse same 64 KiB allocation for Template 1 (8 KiB) then Template 0 (64 KiB).
    let second =
        GenericContexts::from_scratch(GenericTemplate::Template1, false, &limits, &mut scratch)
            .expect("reused template1");
    assert_eq!(second.contexts.len(), 8_192);
    assert_eq!(second.contexts.as_ptr(), initial_ptr);
    assert_eq!(second.contexts[0], ArithmeticContext::new());
    second.into_scratch(&mut scratch);

    let third =
        GenericContexts::from_scratch(GenericTemplate::Template0, false, &limits, &mut scratch)
            .expect("reused template0");
    assert_eq!(third.contexts.len(), 65_536);
    assert_eq!(third.contexts.as_ptr(), initial_ptr);
    assert_eq!(third.contexts[0], ArithmeticContext::new());
    assert_eq!(third.contexts[65_535], ArithmeticContext::new());
    third.into_scratch(&mut scratch);

    // Managed-memory limit is still enforced even when scratch already has capacity.
    let tight = Limits {
        max_decoder_managed_memory_bytes: 1024,
        ..limits
    };
    assert!(
        GenericContexts::from_scratch(GenericTemplate::Template0, false, &tight, &mut scratch)
            .is_err()
    );
    assert_eq!(scratch.capacity(), initial_cap);

    // Scratch-backed session returns buffer on ArithmeticDecoder::new error too.
    assert!(GenericArithmeticSession::new_from_scratch(
        &[0xff, 0xac],
        GenericTemplate::Template0,
        false,
        &limits,
        &mut scratch,
    )
    .is_ok());

    // Direct and owned arithmetic helpers reject MMR headers before decoding.
    let mut mmr_header = generic_header(4, 4, GenericTemplate::Template0, false, false);
    mmr_header.mmr = true;
    assert!(decode_arithmetic_generic(&[0xff, 0xac], &mmr_header, 4, None, &limits).is_err());
    let mut target = Bitmap::blank(4, 4, &limits).expect("target");
    assert!(
        crate::generic_region::segments::decode_arithmetic_generic_into_with_scratch(
            &[0xff, 0xac],
            &mmr_header,
            4,
            &limits,
            &mut target,
            &mut scratch,
        )
        .is_err()
    );
}
