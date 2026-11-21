#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn generic_header_clone_copies_every_flag_region_and_adaptive_slot() {
    let adaptive = std::array::from_fn(|index| {
        let values = [i32::MIN, 0, i32::MAX];
        TemplateOffset::new(values[index % 3], values[(index + 1) % 3])
    });
    for template in [
        GenericTemplate::Template0,
        GenericTemplate::Template1,
        GenericTemplate::Template2,
        GenericTemplate::Template3,
    ] {
        for extended in [false, true] {
            for mmr in [false, true] {
                for typical in [false, true] {
                    let header = GenericRegionHeader {
                        region: crate::segment_data::RegionInfo {
                            width: 17,
                            height: u32::MAX,
                            x: 5,
                            y: 7,
                            operator: CombinationOperator::Xnor,
                        },
                        mmr,
                        template,
                        typical_prediction: typical,
                        extended_template: extended,
                        adaptive,
                    };
                    let mut copied = header.clone();
                    assert_eq!(
                        (
                            copied.region.width,
                            copied.region.height,
                            copied.region.x,
                            copied.region.y,
                            copied.region.operator
                        ),
                        (17, u32::MAX, 5, 7, CombinationOperator::Xnor),
                    );
                    assert_eq!((copied.mmr, copied.template), (mmr, template));
                    assert_eq!(
                        (copied.typical_prediction, copied.extended_template),
                        (typical, extended)
                    );
                    assert_eq!(copied.adaptive, adaptive);
                    copied.adaptive[0].x = 31;
                    assert_eq!(header.adaptive[0].x, i32::MIN);
                }
            }
        }
    }
}

#[test]
fn refinement_header_clone_copies_every_field_and_both_adaptive_slots() {
    for template in [RefinementTemplate::Template0, RefinementTemplate::Template1] {
        for typical in [false, true] {
            let header = RefinementRegionHeader {
                region: crate::segment_data::RegionInfo {
                    width: 29,
                    height: 31,
                    x: u32::MAX,
                    y: 7,
                    operator: CombinationOperator::Replace,
                },
                template,
                typical_prediction: typical,
                adaptive: [
                    TemplateOffset::new(i32::MIN, i32::MAX),
                    TemplateOffset::new(37, -41),
                ],
            };
            let mut copied = header.clone();
            assert_eq!(
                (
                    copied.region.width,
                    copied.region.height,
                    copied.region.x,
                    copied.region.y,
                    copied.region.operator
                ),
                (29, 31, u32::MAX, 7, CombinationOperator::Replace),
            );
            assert_eq!(
                (copied.template, copied.typical_prediction),
                (template, typical)
            );
            assert_eq!(
                copied.adaptive,
                [
                    TemplateOffset::new(i32::MIN, i32::MAX),
                    TemplateOffset::new(37, -41),
                ]
            );
            copied.adaptive[1].y = 43;
            assert_eq!(header.adaptive[1].y, -41);
        }
    }
}

#[test]
fn parses_all_generic_header_shapes() {
    let limits = Limits::default();

    let mut standard = region_prefix(8, 2);
    standard.push(0);
    standard.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    standard.push(0xaa);
    let (header, used) = parse_generic_region_header(&standard, &limits).expect("standard header");
    assert_eq!(used, 26);
    assert_eq!(header.template, GenericTemplate::Template0);
    assert_eq!(header.adaptive[0], TemplateOffset::new(3, -1));
    assert_eq!(standard[used], 0xaa);

    let mut extended = region_prefix(8, 2);
    extended.push(0x10);
    for offset in generic_header(8, 2, GenericTemplate::Template0, true, false).adaptive {
        extended.push(offset.x.to_ne_bytes()[0]);
        extended.push(offset.y.to_ne_bytes()[0]);
    }
    extended.push(0xbb);
    let (header, used) = parse_generic_region_header(&extended, &limits).expect("extended header");
    assert!(header.extended_template);
    assert_eq!(used, 42, "17 + flags + twelve signed pairs");
    assert_eq!(extended[used], 0xbb);

    for (flag, template) in [
        (0x02, GenericTemplate::Template1),
        (0x04, GenericTemplate::Template2),
        (0x06, GenericTemplate::Template3),
    ] {
        let mut bytes = region_prefix(1, 1);
        bytes.extend_from_slice(&[flag, 2, 0xff]);
        let (header, used) = parse_generic_region_header(&bytes, &limits).expect("small template");
        assert_eq!(header.template, template);
        assert_eq!(used, 20);
    }
}

#[test]
fn rejects_reserved_flags_mmr_template_and_future_at() {
    let limits = Limits::default();
    for flags in [0x20, 0x03] {
        let mut bytes = region_prefix(1, 1);
        bytes.push(flags);
        assert!(parse_generic_region_header(&bytes, &limits).is_err());
    }

    let mut future = region_prefix(1, 1);
    future.extend_from_slice(&[0x02, 0, 0]);
    assert!(matches!(
        parse_generic_region_header(&future, &limits),
        Err(GenericRegionError::InvalidAdaptivePixel { .. })
    ));
}

#[test]
fn generic_template_geometry_and_sltp_indices_are_exact() {
    assert_eq!(GENERIC_TEMPLATE_0.len(), 16);
    assert_eq!(GENERIC_TEMPLATE_0_EXTENDED.len(), 16);
    assert_eq!(GENERIC_TEMPLATE_1.len(), 13);
    assert_eq!(GENERIC_TEMPLATE_2.len(), 10);
    assert_eq!(GENERIC_TEMPLATE_3.len(), 10);
    assert_eq!(GenericTemplate::Template0.sltp_context(), 0x9b25);
    assert_eq!(GenericTemplate::Template1.sltp_context(), 0x0795);
    assert_eq!(GenericTemplate::Template2.sltp_context(), 0x00e5);
    assert_eq!(GenericTemplate::Template3.sltp_context(), 0x0195);
    assert_eq!(
        GENERIC_TEMPLATE_0_EXTENDED
            .iter()
            .filter(|slot| slot.adaptive.is_some())
            .count(),
        12
    );
}

#[test]
fn moved_adaptive_pixel_keeps_its_slot_identity() {
    let limits = Limits::default();
    let mut bitmap = Bitmap::blank(10, 3, &limits).expect("bitmap");
    bitmap.set_pixel(0, 2, true).expect("set");
    let mut header = generic_header(10, 3, GenericTemplate::Template0, false, false);
    header.adaptive[0] = TemplateOffset::new(-5, 0);
    let context = generic_context(&bitmap, 5, 2, header.template, false, &header.adaptive);
    assert_eq!(context, 1 << 4, "A1 remains the twelfth row-major slot");

    bitmap.set_pixel(1, 2, true).expect("set overlap");
    header.adaptive[0] = TemplateOffset::new(-4, 0);
    let overlap = generic_context(&bitmap, 5, 2, header.template, false, &header.adaptive);
    assert_eq!(
        overlap, 0x18,
        "overlap keeps both the A1 and fixed template slots"
    );
}

#[test]
fn out_of_bounds_generic_samples_are_zero() {
    let limits = Limits::default();
    let bitmap = Bitmap::blank(1, 1, &limits).expect("bitmap");
    let header = generic_header(1, 1, GenericTemplate::Template3, false, false);
    assert_eq!(
        generic_context(&bitmap, 0, 0, header.template, false, &header.adaptive),
        0
    );
}

#[test]
fn tpgd_toggles_and_copies_without_pixel_decisions() {
    let limits = Limits::default();
    let header = generic_header(3, 3, GenericTemplate::Template2, false, true);
    let mut bitmap = Bitmap::blank(3, 3, &limits).expect("bitmap");
    // Row 0: explicit 101. Row 1: SLTP=1, copy. Row 2: SLTP=0, copy.
    let contexts = decode_bits(&mut bitmap, 3, &header, None, &[0, 1, 0, 1, 1, 0]).expect("decode");
    assert_eq!(contexts.len(), 6);
    assert_eq!(contexts[0], 0x00e5);
    for y in 0..3 {
        assert_eq!(bitmap.pixel(0, y), Some(true));
        assert_eq!(bitmap.pixel(1, y), Some(false));
        assert_eq!(bitmap.pixel(2, y), Some(true));
    }
}

#[test]
fn first_typical_generic_row_is_zero() {
    let limits = Limits::default();
    let header = generic_header(2, 1, GenericTemplate::Template3, false, true);
    let mut bitmap = Bitmap::blank(2, 1, &limits).expect("bitmap");
    let contexts = decode_bits(&mut bitmap, 1, &header, None, &[1]).expect("decode");
    assert_eq!(contexts, vec![0x0195]);
    assert_eq!(bitmap.pixel(0, 0), Some(false));
    assert_eq!(bitmap.pixel(1, 0), Some(false));
}

#[test]
fn height_padding_preserves_decoded_bytes_and_zeroes_new_rows() {
    let limits = Limits::default();
    for width in [0, 1, 7, 8, 9, 63, 64, 65] {
        for height in [0, 1, 3] {
            let mut decoded = Bitmap::blank(width, height, &limits).unwrap();
            for (index, byte) in decoded.data.iter_mut().enumerate() {
                *byte = u8::try_from(index % 251).unwrap().wrapping_add(17);
            }
            for nominal in [height, height + 1, height + 3] {
                let result = pad_bitmap_height(decoded.clone(), nominal, &limits).unwrap();
                assert_eq!(
                    (result.width(), result.height(), result.stride()),
                    (width, nominal, decoded.stride())
                );
                assert_eq!(&result.bytes()[..decoded.data.len()], decoded.bytes());
                assert!(result.bytes()[decoded.data.len()..]
                    .iter()
                    .all(|byte| *byte == 0));
            }
        }
    }
    let decoded = Bitmap::blank(8, 2, &limits).unwrap();
    assert!(matches!(
        pad_bitmap_height(decoded.clone(), 1, &limits),
        Err(GenericRegionError::RowCountMismatch {
            expected: 1,
            actual: 2
        })
    ));
    let restricted = Limits {
        max_height: 2,
        ..limits
    };
    assert!(matches!(
        pad_bitmap_height(decoded, 3, &restricted),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            crate::LimitExceeded {
                kind: crate::LimitKind::Height,
                limit: 2,
                actual: 3,
            }
        )))
    ));
}

#[test]
fn typical_row_copy_preserves_source_and_other_rows() {
    for width in [
        0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 511, 512, 513, 1017, 1023, 1024, 1025, 4096,
    ] {
        let mut original = Bitmap::blank(width, 4, &Limits::default()).unwrap();
        for (index, byte) in original.data.iter_mut().enumerate() {
            *byte = u8::try_from(index % 251)
                .unwrap()
                .wrapping_mul(37)
                .wrapping_add(11);
        }
        for source in 0..4 {
            for destination in 0..4 {
                let stride = original.stride();
                let source_start = usize::try_from(source).unwrap() * stride;
                let destination_start = usize::try_from(destination).unwrap() * stride;
                let mut expected = original.bytes().to_vec();
                expected[destination_start..destination_start + stride]
                    .copy_from_slice(&original.bytes()[source_start..source_start + stride]);
                let mut actual = original.clone();
                copy_bitmap_row(&mut actual, source, destination).unwrap();
                assert_eq!(
                    actual.bytes(),
                    expected,
                    "{width}: {source} -> {destination}"
                );
                assert_eq!(
                    (actual.width(), actual.height(), actual.stride()),
                    (width, 4, stride)
                );
                for x in 0..width {
                    assert_eq!(actual.pixel(x, destination), original.pixel(x, source));
                }
            }
        }
    }
}

#[test]
fn skip_outputs_zero_and_consumes_no_decision() {
    let limits = Limits::default();
    let header = generic_header(3, 1, GenericTemplate::Template3, false, false);
    let mut bitmap = Bitmap::blank(3, 1, &limits).expect("bitmap");
    let mut skip = Bitmap::blank(3, 1, &limits).expect("skip");
    skip.set_pixel(1, 0, true).expect("set skip");
    let contexts = decode_bits(&mut bitmap, 1, &header, Some(&skip), &[1, 1]).expect("decode");
    assert_eq!(contexts.len(), 2);
    assert_eq!(bitmap.pixel(0, 0), Some(true));
    assert_eq!(bitmap.pixel(1, 0), Some(false));
    assert_eq!(bitmap.pixel(2, 0), Some(true));
}

#[test]
fn parses_refinement_headers_and_nominal_adaptive_pixels() {
    let limits = Limits::default();
    let mut template0 = region_prefix(4, 4);
    template0.extend_from_slice(&[0x02, 0xff, 0xff, 0xff, 0xff]);
    let (header, used) = parse_refinement_region_header(&template0, &limits).expect("template 0");
    assert_eq!(used, 22);
    assert_eq!(header.template, RefinementTemplate::Template0);
    assert!(header.typical_prediction);
    assert_eq!(header.adaptive, [TemplateOffset::new(-1, -1); 2]);

    let mut template1 = region_prefix(4, 4);
    template1.push(0x01);
    let (header, used) = parse_refinement_region_header(&template1, &limits).expect("template 1");
    assert_eq!(used, 18);
    assert_eq!(header.template, RefinementTemplate::Template1);
}

#[test]
fn refinement_header_rejects_reserved_flags_before_reading_adaptive_pixels() {
    let limits = Limits::default();
    for flags in 4..=u8::MAX {
        let mut payload = region_prefix(4, 4);
        payload.push(flags);
        assert!(matches!(
            parse_refinement_region_header(&payload, &limits),
            Err(GenericRegionError::ReservedFlags(reserved)) if reserved == flags & 0xfc
        ));
    }
}

#[test]
fn refinement_templates_and_sltp_indices_are_exact() {
    assert_eq!(REFINEMENT_TEMPLATE_0.len(), 13);
    assert_eq!(REFINEMENT_TEMPLATE_1.len(), 10);
    assert_eq!(RefinementTemplate::Template0.sltp_context(), 0x10);
    assert_eq!(RefinementTemplate::Template1.sltp_context(), 0x08);
}

#[test]
fn reference_offset_uses_target_minus_displacement() {
    let limits = Limits::default();
    let mut reference = Bitmap::blank(4, 4, &limits).expect("reference");
    reference.set_pixel(0, 1, true).expect("set");
    assert!(sample_reference(
        &reference,
        2,
        1,
        TemplateOffset::ZERO,
        RefinementParameters {
            reference_dx: 2,
            reference_dy: 0,
        }
    ));
    assert!(!sample_reference(
        &reference,
        2,
        1,
        TemplateOffset::ZERO,
        RefinementParameters {
            reference_dx: -2,
            reference_dy: 0,
        }
    ));
}

#[test]
fn tpgr_predicts_only_uniform_reference_neighbourhoods() {
    let limits = Limits::default();
    let zeros = Bitmap::blank(3, 3, &limits).expect("zeros");
    assert_eq!(
        refinement_prediction(
            &zeros,
            1,
            1,
            RefinementParameters {
                reference_dx: 0,
                reference_dy: 0
            }
        ),
        Some(false)
    );

    let ones = Bitmap::filled(3, 3, true, &limits).expect("ones");
    assert_eq!(
        refinement_prediction(
            &ones,
            1,
            1,
            RefinementParameters {
                reference_dx: 0,
                reference_dy: 0
            }
        ),
        Some(true)
    );
    assert_eq!(
        refinement_prediction(
            &ones,
            0,
            0,
            RefinementParameters {
                reference_dx: 0,
                reference_dy: 0
            }
        ),
        None,
        "out-of-bounds zeroes prevent edge all-one prediction"
    );
}

#[test]
fn tpgr_skips_only_predictable_pixels() {
    let limits = Limits::default();
    let reference = Bitmap::blank(3, 1, &limits).expect("reference");
    let mut target = Bitmap::blank(3, 1, &limits).expect("target");
    let mut decisions = VecDeque::from(vec![1]);
    let mut contexts = Vec::new();
    decode_refinement_with_callback(
        &mut target,
        &reference,
        RefinementTemplate::Template1,
        [TemplateOffset::ZERO; 2],
        true,
        RefinementParameters {
            reference_dx: 0,
            reference_dy: 0,
        },
        |context| {
            contexts.push(context);
            decisions
                .pop_front()
                .ok_or(GenericRegionError::RowCountMismatch {
                    expected: 1,
                    actual: 0,
                })
        },
    )
    .expect("uniform zero reference predicts every pixel");
    assert_eq!(contexts, vec![0x08]);
    assert!(target.bytes().iter().all(|byte| *byte == 0));
}
