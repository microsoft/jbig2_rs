#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn pattern_adaptive_rows_preserve_full_raster_and_entropy_state() {
    let limits = Limits::default();
    let mut random = 0x6a22_7c93_5b06_1f4d;
    for lag in [9, 16, 23, 48, 128, 129, 255] {
        let width = 2 * lag + 3;
        for prediction in [false, true] {
            for skipping in [false, true] {
                for truncated in [false, true] {
                    let mut coded = arithmetic_bytes(&mut random);
                    if truncated {
                        coded.truncate(2);
                        coded.copy_from_slice(&[0x10, 0x04]);
                    }
                    let mut decoder = ArithmeticDecoder::new(&coded).unwrap();
                    let mut reference_decoder = ArithmeticDecoder::new(&coded).unwrap();
                    let mut contexts = vec![
                        ArithmeticContext::new();
                        generic_context_count(GenericTemplate::Template0)
                    ];
                    let mut reference_contexts = contexts.clone();
                    let mut bitmap = Bitmap::blank(width, 6, &limits).unwrap();
                    let mut reference = Bitmap::blank(width, 6, &limits).unwrap();
                    let skip = skipping.then(|| random_bitmap(width, 6, &mut random));
                    let mut adaptive =
                        generic_header(width, 6, GenericTemplate::Template0, false, prediction)
                            .adaptive;
                    adaptive[0] = TemplateOffset::new(-i32::try_from(lag).unwrap(), 0);
                    let actual = decode_generic_with(
                        &mut bitmap,
                        6,
                        GenericTemplate::Template0,
                        false,
                        &adaptive,
                        prediction,
                        skip.as_ref(),
                        &mut decoder,
                        &mut contexts,
                    );
                    let expected = decode_generic_with_oracle(
                        &mut reference,
                        6,
                        GenericTemplate::Template0,
                        false,
                        &adaptive,
                        prediction,
                        skip.as_ref(),
                        &mut reference_decoder,
                        &mut reference_contexts,
                    );
                    assert_eq!(
                        actual.as_ref().err().map(|error| format!("{error:?}")),
                        expected.as_ref().err().map(|error| format!("{error:?}")),
                    );
                    assert_eq!(bitmap, reference);
                    assert_eq!(contexts, reference_contexts);
                    assert_decoder_state_eq(&decoder, &reference_decoder);
                }
            }
        }
    }
}

#[test]
fn specialized_generic_kernels_match_scalar_oracle_randomized() {
    let limits = Limits::default();
    let templates = [
        GenericTemplate::Template0,
        GenericTemplate::Template1,
        GenericTemplate::Template2,
        GenericTemplate::Template3,
    ];
    let prior_offsets = [
        TemplateOffset::new(-1, 0),
        TemplateOffset::new(-4, 0),
        TemplateOffset::new(-7, -1),
        TemplateOffset::new(3, -1),
        TemplateOffset::new(7, -2),
    ];
    let mut state = 0x8d26_4f31_ba90_75ce;
    for case in 0..96_u32 {
        let template = templates[case as usize % templates.len()];
        let variant = case / u32::try_from(templates.len()).expect("four templates");
        let extended = template == GenericTemplate::Template0 && variant & 1 != 0;
        let width = 1 + next_random(&mut state) % 41;
        let height = 1 + next_random(&mut state) % 23;
        let rows = next_random(&mut state) % (height + 1);
        let typical = variant & 2 != 0;
        let mut adaptive = generic_header(width, height, template, extended, typical).adaptive;
        let adaptive_count = match (template, extended) {
            (GenericTemplate::Template0, true) => 12,
            (GenericTemplate::Template0, false) => 4,
            _ => 1,
        };
        if variant & 8 != 0 {
            for offset in adaptive.iter_mut().take(adaptive_count) {
                *offset = prior_offsets[next_random(&mut state) as usize % prior_offsets.len()];
            }
        }
        let skip = (variant & 4 != 0).then(|| random_bitmap(width, height, &mut state));
        let coded = arithmetic_bytes(&mut state);
        let mut optimized_decoder = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut oracle_decoder = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut optimized_contexts =
            vec![ArithmeticContext::new(); generic_context_count(template)];
        let mut oracle_contexts = optimized_contexts.clone();
        let mut optimized = Bitmap::blank(width, height, &limits).expect("bitmap");
        let mut oracle = Bitmap::blank(width, height, &limits).expect("bitmap");

        decode_generic_with(
            &mut optimized,
            rows,
            template,
            extended,
            &adaptive,
            typical,
            skip.as_ref(),
            &mut optimized_decoder,
            &mut optimized_contexts,
        )
        .expect("optimized decode");
        decode_generic_with_oracle(
            &mut oracle,
            rows,
            template,
            extended,
            &adaptive,
            typical,
            skip.as_ref(),
            &mut oracle_decoder,
            &mut oracle_contexts,
        )
        .expect("oracle decode");

        assert_eq!(optimized, oracle, "bitmap mismatch in case {case}");
        assert_eq!(
            optimized_contexts, oracle_contexts,
            "context mismatch in case {case}"
        );
        assert_decoder_state_eq(&optimized_decoder, &oracle_decoder);
    }
}

#[test]
fn word_template0_matches_byte_oracle_exhaustive_row_shapes() {
    let mut state = 0x942d_63a1_b806_f75c;
    for width in 0..=97_u32 {
        for height in 0..=6_u32 {
            for rows in 0..=height {
                for mode in 0..4_u32 {
                    let extended = mode & 1 != 0;
                    let typical = mode & 2 != 0;
                    let coded = arithmetic_bytes(&mut state);
                    assert_word_template0_parity(width, height, rows, extended, typical, &coded);
                }
            }
        }
    }
}

#[test]
fn word_template0_matches_byte_oracle_randomized() {
    let mut state = 0x16cb_78d2_4af9_035e;
    for case in 0..256_u32 {
        let width = next_random(&mut state) % 258;
        let height = next_random(&mut state) % 19;
        let rows = if height == 0 {
            0
        } else {
            next_random(&mut state) % (height + 1)
        };
        let extended = case & 1 != 0;
        let typical = case & 2 != 0;
        let coded = arithmetic_bytes(&mut state);
        assert_word_template0_parity(width, height, rows, extended, typical, &coded);
    }
}

#[test]
fn word_template0_matches_byte_oracle_on_entropy_error() {
    for coded in [
        vec![0x00, 0xff, 0xac],
        vec![0x7f, 0x00, 0xff, 0xac],
        vec![0x01, 0x02, 0x03, 0xff, 0xac],
    ] {
        assert_word_template0_parity(97, 11, 11, false, true, &coded);
    }
}

#[test]
fn direct_bitmap_matches_owned_region_exhaustive_widths() {
    let limits = Limits::default();
    let mut state = 0xa314_6d29_8bf0_c572;
    for width in 1..=65_u32 {
        for height in 1..=4_u32 {
            for typical in [false, true] {
                let coded = arithmetic_bytes(&mut state);
                let payload = arithmetic_segment_payload(width, height, typical, &coded);
                let mut scratch = MmrScratch::default();
                let owned =
                    decode_generic_region_segment_reused(&payload, false, &limits, &mut scratch)
                        .expect("owned decode");
                let mut direct = Bitmap::blank(width, height, &limits).expect("direct target");
                let metadata =
                    decode_generic_region_segment_into(&payload, false, &limits, &mut direct)
                        .expect("direct decode")
                        .expect("arithmetic route");
                assert_eq!(direct, owned.bitmap);
                assert_eq!(metadata.decoded_rows, owned.decoded_rows);
                assert_eq!(metadata.consumed_bytes, owned.consumed_bytes);
            }
        }
    }
}

#[test]
fn direct_bitmap_matches_owned_unknown_rows_randomized() {
    let limits = Limits::default();
    let mut state = 0x4bc7_9da2_105e_f638;
    for case in 0..128_u32 {
        let width = 1 + next_random(&mut state) % 129;
        let height = 1 + next_random(&mut state) % 17;
        let rows = next_random(&mut state) % (height + 1);
        let typical = case & 1 != 0;
        let mut coded = arithmetic_bytes(&mut state);
        coded.extend_from_slice(&rows.to_be_bytes());
        let payload = arithmetic_segment_payload(width, height, typical, &coded);
        let mut scratch = MmrScratch::default();
        let owned = decode_generic_region_segment_reused(&payload, true, &limits, &mut scratch)
            .expect("owned unknown-height decode");
        let mut direct = Bitmap::blank(width, height, &limits).expect("direct target");
        let metadata = decode_generic_region_segment_into(&payload, true, &limits, &mut direct)
            .expect("direct unknown-height decode")
            .expect("arithmetic route");
        assert_eq!(direct, owned.bitmap);
        assert_eq!(metadata.decoded_rows, rows);
        assert_eq!(metadata.consumed_bytes, owned.consumed_bytes);
    }
}

#[test]
fn direct_bitmap_preserves_error_and_memory_limit_classification() {
    let width = 97;
    let height = 11;
    let limits = Limits::default();
    for coded in [
        vec![0x00, 0xff, 0xac],
        vec![0x7f, 0x00, 0xff, 0xac],
        vec![0x01, 0x02, 0x03, 0xff, 0xac],
    ] {
        let payload = arithmetic_segment_payload(width, height, false, &coded);
        let mut scratch = MmrScratch::default();
        let owned = decode_generic_region_segment_reused(&payload, false, &limits, &mut scratch);
        let mut direct = Bitmap::blank(width, height, &limits).expect("direct target");
        let direct = decode_generic_region_segment_into(&payload, false, &limits, &mut direct);
        assert_eq!(
            owned.as_ref().err().map(ToString::to_string),
            direct.as_ref().err().map(ToString::to_string)
        );
    }

    let mut state = 0x18e5_a720_64bc_93df;
    let coded = arithmetic_bytes(&mut state);
    let payload = arithmetic_segment_payload(width, height, false, &coded);
    let bitmap_bytes = bitmap_storage_bytes(width, height).expect("bitmap bytes");
    let context_bytes = context_storage_bytes(generic_context_count(GenericTemplate::Template0))
        .expect("context bytes");
    let limited = Limits {
        max_decoder_managed_memory_bytes: context_bytes + bitmap_bytes - 1,
        ..limits
    };
    let mut scratch = MmrScratch::default();
    let owned = decode_generic_region_segment_reused(&payload, false, &limited, &mut scratch);
    let mut direct = Bitmap::blank(width, height, &limits).expect("preallocated target");
    let direct = decode_generic_region_segment_into(&payload, false, &limited, &mut direct);
    assert_eq!(
        owned.as_ref().err().map(ToString::to_string),
        direct.as_ref().err().map(ToString::to_string)
    );
}

#[test]
fn row_ring_preserves_continuous_decoder_and_retained_context_state() {
    let limits = Limits::default();
    let coded = arithmetic_bytes(&mut 0xb47e_93d0_2c51_a86f);
    let mut ring_decoder = ArithmeticDecoder::new(&coded).expect("ring decoder");
    let mut word_decoder = ArithmeticDecoder::new(&coded).expect("word decoder");
    let mut byte_decoder = ArithmeticDecoder::new(&coded).expect("byte decoder");
    let mut ring_contexts =
        vec![ArithmeticContext::new(); generic_context_count(GenericTemplate::Template0)];
    let mut word_contexts = ring_contexts.clone();
    let mut byte_contexts = ring_contexts.clone();

    for (width, height, rows, typical) in [(31, 5, 5, true), (65, 7, 4, false), (9, 3, 3, true)] {
        let header = generic_header(width, height, GenericTemplate::Template0, false, typical);
        let mut ring_bitmap = Bitmap::blank(width, height, &limits).expect("ring bitmap");
        let mut word_bitmap = Bitmap::blank(width, height, &limits).expect("word bitmap");
        let mut byte_bitmap = Bitmap::blank(width, height, &limits).expect("byte bitmap");
        let mut ring = GenericRowRing::try_new(width, rows, u64::MAX).expect("row ring");
        let mut raster = GenericWordRaster::try_new(width, height).expect("word raster");

        decode_nominal_template0_row_ring(
            &mut ring,
            &mut ring_bitmap,
            rows,
            typical,
            &mut ring_decoder,
            &mut ring_contexts,
        )
        .expect("row-ring decode");
        decode_nominal_template0_words(
            &mut raster,
            rows,
            typical,
            &mut word_decoder,
            &mut word_contexts,
        )
        .expect("word decode");
        raster
            .materialize(&mut word_bitmap)
            .expect("word materialization");
        decode_generic_with_oracle(
            &mut byte_bitmap,
            rows,
            GenericTemplate::Template0,
            false,
            &header.adaptive,
            typical,
            None,
            &mut byte_decoder,
            &mut byte_contexts,
        )
        .expect("byte decode");

        assert_eq!(ring_bitmap, byte_bitmap);
        assert_eq!(word_bitmap, byte_bitmap);
        assert_eq!(ring_contexts, byte_contexts);
        assert_eq!(word_contexts, byte_contexts);
        assert_decoder_state_eq(&ring_decoder, &byte_decoder);
        assert_decoder_state_eq(&word_decoder, &byte_decoder);
    }
}

#[test]
fn nominal_template0_skip_uses_exact_byte_fallback() {
    let limits = Limits::default();
    let width = 67;
    let height = 8;
    let rows = 5;
    let header = generic_header(width, height, GenericTemplate::Template0, false, true);
    let mut state = 0xe534_18af_92c7_60bd;
    let coded = arithmetic_bytes(&mut state);
    let skip = random_bitmap(width, height, &mut state);
    let mut routed_decoder = ArithmeticDecoder::new(&coded).expect("routed decoder");
    let mut oracle_decoder = ArithmeticDecoder::new(&coded).expect("oracle decoder");
    let mut routed_contexts =
        GenericContexts::new(GenericTemplate::Template0, false, &limits).expect("contexts");
    let mut oracle_contexts = routed_contexts.contexts.clone();
    let mut work = 0;
    let routed = decode_generic_on(
        &mut routed_decoder,
        &mut routed_contexts,
        GenericArithmeticCall {
            width,
            height,
            adaptive: header.adaptive,
            typical_prediction: true,
            skip: Some(&skip),
        },
        rows,
        &limits,
        &mut work,
    )
    .expect("fallback decode");
    let mut oracle = Bitmap::blank(width, height, &limits).expect("oracle bitmap");
    decode_generic_with_oracle(
        &mut oracle,
        rows,
        GenericTemplate::Template0,
        false,
        &header.adaptive,
        true,
        Some(&skip),
        &mut oracle_decoder,
        &mut oracle_contexts,
    )
    .expect("oracle decode");

    assert_eq!(routed, oracle);
    assert_eq!(routed_contexts.contexts, oracle_contexts);
    assert_decoder_state_eq(&routed_decoder, &oracle_decoder);
    assert_eq!(
        work,
        generic_work(width, rows, true).expect("expected work")
    );
}

#[test]
fn row_ring_peak_is_charged_released_and_low_memory_falls_back() {
    let width = 33;
    let height = 100;
    let context_bytes = context_storage_bytes(generic_context_count(GenericTemplate::Template0))
        .expect("context bytes");
    let bitmap_bytes = bitmap_storage_bytes(width, height).expect("bitmap bytes");
    let row_bytes = row_ring_storage_bytes(width, height).expect("row bytes");
    let raster_bytes = word_raster_storage_bytes(width, height).expect("word bytes");
    assert_eq!(bitmap_bytes, 500);
    assert_eq!(row_bytes, 24);
    assert_eq!(raster_bytes, 800);
    assert!(row_bytes < raster_bytes);

    let fallback_limits = Limits {
        max_decoder_managed_memory_bytes: context_bytes + bitmap_bytes,
        ..Limits::default()
    };
    assert!(!row_ring_fits_managed_memory(
        width,
        height,
        context_bytes,
        bitmap_bytes,
        &fallback_limits,
    )
    .expect("peak check"));
    let mut ring_limits = fallback_limits;
    ring_limits.max_decoder_managed_memory_bytes += row_bytes;
    assert!(
        row_ring_fits_managed_memory(width, height, context_bytes, bitmap_bytes, &ring_limits,)
            .expect("peak check")
    );
    let ring_charge =
        GenericRowRing::try_new(width, height, row_bytes).expect("exact charged capacity");
    assert_eq!(ring_charge.charged_bytes, row_bytes);
    drop(ring_charge);

    let header = generic_header(width, height, GenericTemplate::Template0, false, true);
    let coded = arithmetic_bytes(&mut 0x52c7_f931_0bae_846d);
    let mut fallback_decoder = ArithmeticDecoder::new(&coded).expect("fallback decoder");
    let mut ring_decoder = ArithmeticDecoder::new(&coded).expect("ring decoder");
    let mut fallback_contexts =
        GenericContexts::new(GenericTemplate::Template0, false, &fallback_limits)
            .expect("fallback contexts");
    let mut ring_contexts = GenericContexts::new(GenericTemplate::Template0, false, &ring_limits)
        .expect("ring contexts");
    let call = GenericArithmeticCall {
        width,
        height,
        adaptive: header.adaptive,
        typical_prediction: true,
        skip: None,
    };
    let mut fallback_work = 0;
    let mut ring_work = 0;
    let fallback = decode_generic_on(
        &mut fallback_decoder,
        &mut fallback_contexts,
        call,
        height,
        &fallback_limits,
        &mut fallback_work,
    )
    .expect("byte fallback");
    let ring = decode_generic_on(
        &mut ring_decoder,
        &mut ring_contexts,
        call,
        height,
        &ring_limits,
        &mut ring_work,
    )
    .expect("row-ring route");
    assert_eq!(ring, fallback);
    assert_eq!(ring_contexts.contexts, fallback_contexts.contexts);
    assert_decoder_state_eq(&ring_decoder, &fallback_decoder);
    assert_eq!(ring_work, fallback_work);
}

#[test]
fn specialized_refinement_kernels_match_scalar_oracle_randomized() {
    let limits = Limits::default();
    let mut state = 0x71e4_c09b_325d_a86f;
    for case in 0..96_u32 {
        let template = if case & 1 == 0 {
            RefinementTemplate::Template0
        } else {
            RefinementTemplate::Template1
        };
        let width = 1 + next_random(&mut state) % 37;
        let height = 1 + next_random(&mut state) % 21;
        let reference_width = 1 + next_random(&mut state) % 43;
        let reference_height = 1 + next_random(&mut state) % 25;
        let reference = random_bitmap(reference_width, reference_height, &mut state);
        let adaptive = if case & 8 == 0 {
            [TemplateOffset::new(-1, -1); 2]
        } else {
            let target = [
                TemplateOffset::new(-1, 0),
                TemplateOffset::new(-5, -1),
                TemplateOffset::new(4, -2),
            ][next_random(&mut state) as usize % 3];
            let reference = TemplateOffset::new(
                i32::try_from(next_random(&mut state) % 11).expect("bounded") - 5,
                i32::try_from(next_random(&mut state) % 7).expect("bounded") - 3,
            );
            [target, reference]
        };
        let parameters = RefinementParameters {
            reference_dx: i32::try_from(next_random(&mut state) % 9).expect("bounded") - 4,
            reference_dy: i32::try_from(next_random(&mut state) % 7).expect("bounded") - 3,
        };
        let typical = case & 2 != 0;
        let coded = arithmetic_bytes(&mut state);
        let mut optimized_decoder = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut oracle_decoder = ArithmeticDecoder::new(&coded).expect("decoder");
        let mut optimized_contexts =
            vec![ArithmeticContext::new(); refinement_context_count(template)];
        let mut oracle_contexts = optimized_contexts.clone();
        let mut optimized = Bitmap::blank(width, height, &limits).expect("bitmap");
        let mut oracle = Bitmap::blank(width, height, &limits).expect("bitmap");

        decode_refinement_with(
            &mut optimized,
            &reference,
            template,
            adaptive,
            typical,
            parameters,
            &mut optimized_decoder,
            &mut optimized_contexts,
        )
        .expect("optimized decode");
        decode_refinement_with_oracle(
            &mut oracle,
            &reference,
            template,
            adaptive,
            typical,
            parameters,
            &mut oracle_decoder,
            &mut oracle_contexts,
        )
        .expect("oracle decode");

        assert_eq!(optimized, oracle, "bitmap mismatch in case {case}");
        assert_eq!(
            optimized_contexts, oracle_contexts,
            "context mismatch in case {case}"
        );
        assert_decoder_state_eq(&optimized_decoder, &oracle_decoder);
    }
}
