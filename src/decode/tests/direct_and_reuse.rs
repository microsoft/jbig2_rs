#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn decodes_generic_page_and_collects_document() {
    let decoded = Decoder::default()
        .decode(&document_with_generic(&[0x26, 0xA2, 0x80]))
        .expect("MMR page decodes");
    assert_eq!(decoded.pages().len(), 1);
    assert_eq!(decoded.pages()[0].number(), 1);
    assert_eq!(decoded.pages()[0].bitmap().bytes(), &[0xFF]);
}

#[test]
fn direct_page_matches_intermediate_compose_oracle() {
    let limits = Limits::default();
    let mut state = 0x5ca1_49e7_2d83_b604;
    for width in 1..=65_u32 {
        let height = 1 + width % 4;
        let coded = arithmetic_test_bytes(&mut state);
        let document = arithmetic_generic_document(width, height, 0, &coded);
        assert_direct_page_matches_oracle(&document, false, true, CombinationOperator::Or, &limits);
    }
}

#[test]
fn pristine_xor_pages_match_buffered_pixels_and_charge_the_same_work() {
    let limits = Limits::default();
    let mut random = 0x7d31_4a86_52b0_9fce;
    for width in [1, 7, 8, 9, 31, 32, 33, 65, 129] {
        let height = 1 + width % 4;
        let coded = arithmetic_test_bytes(&mut random);
        let document = arithmetic_generic_document(width, height, 2, &coded);
        let (_, segments) = parse_standalone_document(&document, &limits).unwrap();
        let mut pages = vec![(
            1,
            runtime_test_page(
                width,
                height,
                false,
                true,
                CombinationOperator::Xor,
                &limits,
            ),
        )];
        let work = DecodeWork::new(0, &limits).unwrap();
        assert_eq!(
            direct_generic_page_index(&segments[1], &pages, &work, &limits),
            Some(0)
        );
        assert_direct_page_matches_oracle(
            &document,
            false,
            true,
            CombinationOperator::Xor,
            &limits,
        );
        pages[0].1.bitmap.set_pixel(0, 0, true).unwrap();
        assert_eq!(
            direct_generic_page_index(&segments[1], &pages, &work, &limits),
            None
        );
        assert_direct_page_matches_oracle(&document, true, true, CombinationOperator::Xor, &limits);
        assert_direct_page_matches_oracle(
            &document,
            false,
            false,
            CombinationOperator::Xor,
            &limits,
        );
    }
}

#[test]
fn incompatible_and_limited_direct_pages_use_exact_oracle_fallback() {
    let mut state = 0xd630_18ab_795c_42ef;
    let coded = arithmetic_test_bytes(&mut state);
    let document = arithmetic_generic_document(97, 11, 0, &coded);
    for (default_pixel, pristine, operator) in [
        (true, true, CombinationOperator::Or),
        (false, false, CombinationOperator::Or),
        (false, true, CombinationOperator::Xor),
    ] {
        assert_direct_page_matches_oracle(
            &document,
            default_pixel,
            pristine,
            operator,
            &Limits::default(),
        );
    }
    let limits = Limits {
        max_work_units: 1,
        ..Limits::default()
    };
    assert_direct_page_matches_oracle(&document, false, true, CombinationOperator::Or, &limits);
}

#[test]
fn reusable_context_matches_stateless_and_reuses_capacity() {
    let input = document_with_generic(&[0x80]);
    let decoder = Decoder::default();
    let expected = decoder.decode(&input).expect("stateless decode");
    let mut context = decoder.context();
    let first = context.decode(&input).expect("context decode");
    assert_eq!(first, expected);
    let retained = context.retained_scratch_bytes();
    let reserve_events = context.mmr.reserve_events;
    assert!(retained > 0);
    assert_eq!(reserve_events, 2);
    let second = context.decode(&input).expect("reused context decode");
    assert_eq!(second, expected);
    assert_eq!(context.retained_scratch_bytes(), retained);
    assert_eq!(context.mmr.reserve_events, reserve_events);
    context.release_scratch();
    assert_eq!(context.retained_scratch_bytes(), 0);
    assert_eq!(context.mmr.reserve_events, 0);
}

#[test]
fn reusable_context_recovers_after_error_without_state_leakage() {
    let decoder = Decoder::default();
    let valid = document_with_generic(&[0x80]);
    let expected = decoder.decode(&valid).expect("stateless decode");
    let mut context = decoder.context();
    assert!(context.decode(&[0]).is_err());
    let recovered = context.decode(&valid).expect("decode after error");
    assert_eq!(recovered, expected);
    context.reset();
    assert_eq!(
        context.decode(&valid).expect("decode after reset"),
        expected
    );
}

#[test]
fn reusable_context_obeys_retained_state_limit() {
    let limits = Limits {
        max_retained_state_bytes: 1,
        ..Limits::default()
    };
    let decoder = Decoder::new(limits);
    let mut context = decoder.context();
    context
        .decode(&document_with_generic(&[0x80]))
        .expect("scratch retention is optional");
    assert!(context.retained_scratch_bytes() <= 1);
}

#[test]
fn reusable_context_handles_mixed_sizes_deterministically() {
    let decoder = Decoder::default();
    let small = all_white_mmr_document(64, 64);
    let large = all_white_mmr_document(512, 512);
    let mut context = decoder.context();
    let small_expected = decoder.decode(&small).expect("small stateless");
    let large_expected = decoder.decode(&large).expect("large stateless");
    assert_eq!(
        context.decode(&small).expect("small context"),
        small_expected
    );
    let small_capacity = context.retained_scratch_bytes();
    let small_events = context.mmr.reserve_events;
    assert_eq!(
        context.decode(&large).expect("large context"),
        large_expected
    );
    let large_capacity = context.retained_scratch_bytes();
    assert!(large_capacity >= small_capacity);
    assert!(context.mmr.reserve_events > small_events);
    let large_events = context.mmr.reserve_events;
    assert_eq!(
        context.decode(&small).expect("small reused"),
        small_expected
    );
    assert_eq!(context.retained_scratch_bytes(), large_capacity);
    assert_eq!(context.mmr.reserve_events, large_events);
}

#[test]
#[ignore = "microbenchmark; run explicitly with --release --ignored --nocapture"]
fn benchmark_reusable_context_against_d669954_stateless() {
    let decoder = Decoder::default();
    let same = all_white_mmr_document(64, 64);
    let mixed = [
        all_white_mmr_document(64, 64),
        same.clone(),
        all_white_mmr_document(2048, 2048),
        same.clone(),
    ];
    let mut stateless_times = Vec::new();
    let mut reused_times = Vec::new();
    for sample in 0..9 {
        if sample % 2 == 0 {
            let start = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(decoder.decode(&same).expect("stateless"));
            }
            stateless_times.push(start.elapsed());
            let mut context = decoder.context();
            let start = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(context.decode(&same).expect("reused"));
            }
            reused_times.push(start.elapsed());
        } else {
            let mut context = decoder.context();
            let start = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(context.decode(&same).expect("reused"));
            }
            reused_times.push(start.elapsed());
            let start = std::time::Instant::now();
            for _ in 0..50 {
                std::hint::black_box(decoder.decode(&same).expect("stateless"));
            }
            stateless_times.push(start.elapsed());
        }
    }
    stateless_times.sort_unstable();
    reused_times.sort_unstable();
    let stateless = stateless_times[stateless_times.len() / 2];
    let reused = reused_times[reused_times.len() / 2];

    let mut stateless_mixed = Vec::new();
    let mut reused_mixed = Vec::new();
    for sample in 0..7 {
        if sample % 2 == 0 {
            let start = std::time::Instant::now();
            for _ in 0..20 {
                for input in &mixed {
                    std::hint::black_box(decoder.decode(input).expect("stateless mixed"));
                }
            }
            stateless_mixed.push(start.elapsed());
            let mut context = decoder.context();
            let start = std::time::Instant::now();
            for _ in 0..20 {
                for input in &mixed {
                    std::hint::black_box(context.decode(input).expect("reused mixed"));
                }
            }
            reused_mixed.push(start.elapsed());
        } else {
            let mut context = decoder.context();
            let start = std::time::Instant::now();
            for _ in 0..20 {
                for input in &mixed {
                    std::hint::black_box(context.decode(input).expect("reused mixed"));
                }
            }
            reused_mixed.push(start.elapsed());
            let start = std::time::Instant::now();
            for _ in 0..20 {
                for input in &mixed {
                    std::hint::black_box(decoder.decode(input).expect("stateless mixed"));
                }
            }
            stateless_mixed.push(start.elapsed());
        }
    }
    stateless_mixed.sort_unstable();
    reused_mixed.sort_unstable();
    let stateless_mixed = stateless_mixed[stateless_mixed.len() / 2];
    let reused_mixed = reused_mixed[reused_mixed.len() / 2];
    println!(
        "context same_stateless_ns={} same_reused_ns={} same_speedup={:.3} \
         mixed_stateless_ns={} mixed_reused_ns={} mixed_speedup={:.3}",
        stateless.as_nanos(),
        reused.as_nanos(),
        stateless.as_secs_f64() / reused.as_secs_f64(),
        stateless_mixed.as_nanos(),
        reused_mixed.as_nanos(),
        stateless_mixed.as_secs_f64() / reused_mixed.as_secs_f64(),
    );
}
