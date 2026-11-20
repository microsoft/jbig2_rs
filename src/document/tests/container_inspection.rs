#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn public_global_results_own_complete_metadata_input_for_decoder_and_context() {
    // These are accepted metadata containers, not entropy-valid dictionary witnesses.
    let payload: Vec<u8> = (0..=u8::MAX).collect();
    let mut input = header(7, 0, 0, u32::try_from(payload.len()).unwrap());
    input.extend_from_slice(&payload);
    input.extend_from_slice(&header(19, 0, 0, 0));
    let expected = input.clone();
    let decoder = Decoder::default();
    let direct = decoder.parse_globals(&input).unwrap();
    let mut context = decoder.context();
    let reused = context.parse_globals(&input).unwrap();
    for byte in &mut input {
        *byte ^= 0xff;
    }
    for globals in [direct, reused] {
        assert_eq!(globals.segment_count(), 2);
        assert_eq!(globals.bytes(), expected);
    }
    assert_eq!(context.retained_scratch_bytes(), 0);
    assert!(std::ptr::eq(context.limits(), decoder.limits()));
}

#[test]
fn public_global_results_preserve_first_stream_error_even_for_malformed_input() {
    for length in [1_usize, 2, 7, 64] {
        let input = vec![0xff; length];
        let actual = u64::try_from(length).unwrap();
        let decoder = Decoder::new(Limits {
            max_stream_bytes: actual - 1,
            max_work_units: 0,
            max_retained_state_bytes: 0,
            ..Limits::default()
        });
        for result in [
            decoder.parse_globals(&input),
            decoder.context().parse_globals(&input),
        ] {
            assert!(matches!(
                result,
                Err(DecodeError::LimitExceeded(LimitExceeded {
                    kind: LimitKind::StreamBytes,
                    limit,
                    actual: observed,
                })) if limit == actual - 1 && observed == actual
            ));
        }
    }
}

#[test]
fn public_global_results_match_exact_retained_admission_boundaries() {
    let mut input = header(7, 0, 0, 4);
    input.extend_from_slice(&[0, 0x7f, 0x80, 0xff]);
    let actual = u64::try_from(input.len()).unwrap();
    for limit in [0, actual - 1, actual, actual + 1, u64::MAX] {
        let decoder = Decoder::new(Limits {
            max_stream_bytes: actual,
            max_retained_state_bytes: limit,
            ..Limits::default()
        });
        for result in [
            decoder.parse_globals(&input),
            decoder.context().parse_globals(&input),
        ] {
            if limit < actual {
                assert!(matches!(
                    result,
                    Err(DecodeError::LimitExceeded(LimitExceeded {
                        kind: LimitKind::RetainedState,
                        limit: observed_limit,
                        actual: observed_actual,
                    })) if observed_limit == limit && observed_actual == actual
                ));
            } else {
                assert_eq!(result.unwrap().bytes(), input);
            }
        }
    }
}

fn seeded_public_context(decoder: &Decoder, retained: u64) -> DecoderContext<'_> {
    let mut context = decoder.context();
    context.mmr.transitions.extend_from_slice(&[0, 7, u32::MAX]);
    context.mmr.row_transitions.extend_from_slice(&[13, 17]);
    context.mmr.retained_bytes = retained;
    context.mmr.reserve_events = 23;
    context
}

#[test]
fn public_context_starts_empty_with_exact_limits_and_retained_metadata() {
    let decoder = Decoder::new(Limits {
        max_width: 29,
        max_work_units: 31,
        max_retained_state_bytes: 37,
        ..Limits::default()
    });
    let context = decoder.context();
    assert!(std::ptr::eq(context.limits(), decoder.limits()));
    assert_eq!(
        format!("{:?}", context.limits()),
        format!("{:?}", decoder.limits())
    );
    assert!(context.mmr.transitions.is_empty());
    assert!(context.mmr.row_transitions.is_empty());
    assert_eq!(context.retained_scratch_bytes(), 0);
    assert_eq!(context.mmr.reserve_events, 0);
    assert_eq!(context.mmr.transitions.capacity(), 0);
    assert_eq!(context.mmr.row_transitions.capacity(), 0);
}

#[test]
fn public_context_reset_keeps_configuration_capacity_and_arbitrary_stored_bytes() {
    let decoder = Decoder::new(Limits::default());
    for retained in [0, 1, 41, u64::MAX] {
        let mut context = seeded_public_context(&decoder, retained);
        let capacity = (
            context.mmr.transitions.capacity(),
            context.mmr.row_transitions.capacity(),
        );
        assert_eq!(context.retained_scratch_bytes(), retained);
        context.reset();
        assert!(context.mmr.transitions.is_empty());
        assert!(context.mmr.row_transitions.is_empty());
        assert_eq!(context.retained_scratch_bytes(), retained);
        assert_eq!(
            (
                context.mmr.transitions.capacity(),
                context.mmr.row_transitions.capacity()
            ),
            capacity,
        );
        assert_eq!(context.mmr.reserve_events, 23);
        assert!(std::ptr::eq(context.limits(), decoder.limits()));
    }
}

#[test]
fn public_context_release_resets_all_storage_metadata_and_configuration_frame() {
    let decoder = Decoder::new(Limits::default());
    let mut context = seeded_public_context(&decoder, u64::MAX);
    context.release_scratch();
    assert!(context.mmr.transitions.is_empty());
    assert!(context.mmr.row_transitions.is_empty());
    assert_eq!(context.retained_scratch_bytes(), 0);
    assert_eq!(context.mmr.transitions.capacity(), 0);
    assert_eq!(context.mmr.row_transitions.capacity(), 0);
    assert_eq!(context.mmr.reserve_events, 0);
    assert!(std::ptr::eq(context.limits(), decoder.limits()));
}

#[test]
fn public_context_finish_matches_strict_wide_threshold_and_empty_state() {
    for limit in [0, 1, 47, u64::MAX] {
        let decoder = Decoder::new(Limits {
            max_retained_state_bytes: limit,
            ..Limits::default()
        });
        for retained in [
            0,
            1,
            limit.saturating_sub(1),
            limit,
            limit.saturating_add(1),
            u64::MAX,
        ] {
            let mut context = seeded_public_context(&decoder, retained);
            let capacity = (
                context.mmr.transitions.capacity(),
                context.mmr.row_transitions.capacity(),
            );
            context.finish_call();
            let released = u128::from(retained) > u128::from(limit);
            assert!(context.mmr.transitions.is_empty());
            assert!(context.mmr.row_transitions.is_empty());
            assert_eq!(
                context.retained_scratch_bytes(),
                if released { 0 } else { retained }
            );
            assert_eq!(
                (
                    context.mmr.transitions.capacity(),
                    context.mmr.row_transitions.capacity()
                ),
                if released { (0, 0) } else { capacity },
            );
            assert_eq!(context.mmr.reserve_events, if released { 0 } else { 23 });
            assert!(std::ptr::eq(context.limits(), decoder.limits()));
        }
    }
}

#[test]
fn public_context_all_error_returns_clear_scratch_and_keep_limits() {
    let decoder = Decoder::new(Limits {
        max_retained_state_bytes: 53,
        ..Limits::default()
    });
    let globals = decoder.parse_globals(&[]).unwrap();
    let mut checked = 0;
    for operation in 0..7 {
        let mut context = seeded_public_context(&decoder, 59);
        let rejected = match operation {
            0 => context.inspect(&[0]).is_err(),
            1 => context.inspect_embedded(&[0]).is_err(),
            2 => context
                .inspect_embedded_with_globals(&[0], &globals)
                .is_err(),
            3 => context.parse_globals(&[0]).is_err(),
            4 => context.decode(&[0]).is_err(),
            5 => context.decode_embedded(&[0]).is_err(),
            _ => context
                .decode_embedded_with_globals(&[0], &globals)
                .is_err(),
        };
        assert!(rejected);
        assert!(context.mmr.transitions.is_empty());
        assert!(context.mmr.row_transitions.is_empty());
        assert_eq!(context.retained_scratch_bytes(), 0);
        assert!(std::ptr::eq(context.limits(), decoder.limits()));
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn decoder_clone_preserves_all_supplied_limits_and_independent_configuration() {
    let limits = Limits {
        max_width: 17,
        max_height: 19,
        max_work_units: 59,
        max_stream_bytes: 61,
        max_pages: 67,
        max_retained_state_bytes: 71,
        ..Limits::default()
    };
    let decoder = Decoder::new(limits);
    let mut copied = decoder.clone();
    assert_eq!(format!("{:?}", copied.limits()), format!("{limits:?}"));
    copied.limits.max_width = 23;
    assert_eq!(decoder.limits().max_width, 17);
    assert_eq!(copied.limits().max_width, 23);
}

#[test]
fn page_stripe_boundaries_match_wide_policy_and_preserve_rejected_state() {
    for height in [None, Some(1), Some(4)] {
        for striped in [false, true] {
            for maximum in [0, 1, 3, u16::MAX] {
                let info = direct_page_info(height, striped, maximum);
                for previous in [None, Some(0), Some(2), Some(u32::MAX - 1), Some(u32::MAX)] {
                    for row in [0, 1, 2, 3, u32::from(u16::MAX), u32::MAX] {
                        let mut state = PageState::new(info);
                        state.last_stripe = previous;
                        let progress = match previous {
                            Some(before) => {
                                row > before
                                    && u64::from(row) - u64::from(before) <= u64::from(maximum)
                            }
                            None => row <= u32::from(maximum),
                        };
                        let valid = striped && progress && height.is_none_or(|height| row < height);
                        match state.add_stripe(row) {
                            Ok(()) => {
                                assert!(valid);
                                assert_eq!(state.last_stripe, Some(row));
                            }
                            Err(DecodeError::Malformed(MalformedError::InvalidStripeEndRow(
                                actual,
                            ))) => {
                                assert!(!valid);
                                assert_eq!(actual, row);
                                assert_eq!(state.last_stripe, previous);
                            }
                            other => panic!("unexpected stripe result: {other:?}"),
                        }
                        assert_eq!(state.info, info);
                        assert!(!state.ended);
                    }
                }
            }
        }
    }
}

#[test]
fn page_finish_and_public_metadata_keep_exact_error_precedence_and_fields() {
    for height in [None, Some(1), Some(4)] {
        let info = direct_page_info(height, true, 3);
        for last in [None, Some(0), Some(3), Some(u32::MAX)] {
            for ended in [false, true] {
                let mut state = PageState::new(info);
                state.last_stripe = last;
                state.ended = ended;
                match state.finish() {
                    Err(DecodeError::Malformed(MalformedError::DuplicatePageBoundary {
                        page,
                        segment_type,
                    })) => {
                        assert!(ended);
                        assert_eq!((page, segment_type), (0, 49));
                    }
                    Err(DecodeError::Malformed(MalformedError::InvalidStripeEndRow(row))) => {
                        assert!(!ended && height.is_none() && last.is_none());
                        assert_eq!(row, 0);
                    }
                    Ok(()) => {
                        assert!(!ended && (height.is_some() || last.is_some()));
                        assert!(state.ended);
                    }
                    other => panic!("unexpected page finish result: {other:?}"),
                }
                assert_eq!(state.info, info);
                assert_eq!(state.last_stripe, last);
                if ended || (height.is_none() && last.is_none()) {
                    assert_eq!(state.ended, ended);
                }
            }
            let mut state = PageState::new(info);
            state.last_stripe = last;
            match state.into_public(u32::MAX) {
                Ok(page) => {
                    let expected = height
                        .map(u64::from)
                        .or_else(|| last.map(|row| u64::from(row) + 1))
                        .unwrap();
                    assert!(u32::try_from(expected).is_ok());
                    assert_eq!(
                        (page.number, page.width, u64::from(page.height)),
                        (u32::MAX, 13, expected)
                    );
                    assert_eq!(
                        (page.x_resolution, page.y_resolution, page.striped),
                        (123, 456, true)
                    );
                }
                Err(DecodeError::Malformed(MalformedError::IntegerOverflow)) => {
                    assert!(height.is_none() && last == Some(u32::MAX));
                }
                Err(DecodeError::Malformed(MalformedError::InvalidStripeEndRow(row))) => {
                    assert!(height.is_none() && last.is_none());
                    assert_eq!(row, 0);
                }
                other => panic!("unexpected page metadata result: {other:?}"),
            }
        }
    }
}

#[test]
fn inspects_sequential_document() {
    let bytes = valid_sequential_document();

    let info = Decoder::default()
        .inspect(&bytes)
        .expect("document is valid");
    assert_eq!(info.segment_count(), 3);
    assert_eq!(info.pages().len(), 1);
    assert_eq!(info.pages()[0].width(), 8);
    assert_eq!(info.pages()[0].height(), 8);
}

#[test]
fn inspects_random_access_document() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x00);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&header(0, 48, 1, 19));
    bytes.extend_from_slice(&header(1, 49, 1, 0));
    bytes.extend_from_slice(&header(2, 51, 0, 0));
    bytes.extend_from_slice(&page_data(8));

    let info = Decoder::default()
        .inspect(&bytes)
        .expect("document is valid");
    assert_eq!(info.organization(), Organization::RandomAccess);
    assert_eq!(info.segment_count(), 3);
}

#[test]
fn finalizes_unknown_height_from_stripe() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x03);
    bytes.extend_from_slice(&header(0, 48, 1, 19));
    let mut page = page_data(u32::MAX);
    page[17..19].copy_from_slice(&0x800A_u16.to_be_bytes());
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&header(1, 50, 1, 4));
    bytes.extend_from_slice(&7_u32.to_be_bytes());
    bytes.extend_from_slice(&header(2, 49, 1, 0));

    let info = Decoder::default()
        .inspect(&bytes)
        .expect("document is valid");
    assert_eq!(info.pages()[0].height(), 8);
    assert!(info.pages()[0].is_striped());
}

#[test]
fn rejects_page_count_mismatch() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&header(0, 48, 1, 19));
    bytes.extend_from_slice(&page_data(8));
    bytes.extend_from_slice(&header(1, 49, 1, 0));
    assert!(matches!(
        Decoder::default().inspect(&bytes),
        Err(DecodeError::Malformed(MalformedError::PageCountMismatch {
            declared: 2,
            actual: 1
        }))
    ));
}

#[test]
fn bounded_reader_enforces_stream_limit() {
    let limits = Limits {
        max_stream_bytes: 4,
        ..Limits::default()
    };
    let mut input = std::io::Cursor::new([0_u8; 5]);
    assert!(matches!(
        Decoder::new(limits).inspect_reader(&mut input),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::StreamBytes,
            limit: 4,
            actual: 5
        }))
    ));
}

#[test]
fn inspection_enforces_work_limit() {
    let bytes = valid_sequential_document();
    let limits = Limits {
        max_work_units: 10,
        ..Limits::default()
    };
    assert!(matches!(
        Decoder::new(limits).inspect(&bytes),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Work,
            limit: 10,
            ..
        }))
    ));
}

#[test]
fn locates_unknown_length_generic_terminator() {
    let mut data = vec![0_u8; 18];
    data[4..8].copy_from_slice(&3_u32.to_be_bytes());
    data[17] = 0;
    data.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    data.extend_from_slice(&[0x11, 0x22, 0xFF, 0xAC]);
    data.extend_from_slice(&3_u32.to_be_bytes());
    data.extend_from_slice(&[0xAA, 0xBB]);
    assert_eq!(
        find_unknown_generic_length(&data, &Limits::default()).expect("terminator is present"),
        34
    );
}

#[test]
fn rejects_every_byte_truncation_of_minimal_document() {
    let bytes = valid_sequential_document();
    let valid_without_optional_eof = bytes.len() - header(2, 51, 0, 0).len();
    for length in 0..bytes.len() {
        if Decoder::default().inspect(&bytes[..length]).is_ok() {
            assert_eq!(
                length, valid_without_optional_eof,
                "truncation at byte {length} unexpectedly succeeded"
            );
        }
    }
}

#[test]
fn document_inspection_is_deterministic() {
    let bytes = valid_sequential_document();
    let first = Decoder::default()
        .inspect(&bytes)
        .expect("document is valid");
    let second = Decoder::default()
        .inspect(&bytes)
        .expect("document is valid");
    assert_eq!(first, second);
}

#[test]
fn enforces_total_reference_and_depth_limits() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&header(0, 0, 0, 0));
    let retention_index = SIGNATURE.len() + 1 + 4 + 5;
    bytes[retention_index] = 1;
    bytes.extend_from_slice(&header_with_reference(1, 0, 0));
    bytes.extend_from_slice(&header_with_reference(2, 0, 1));
    bytes.extend_from_slice(&header(3, 51, 0, 0));

    let depth_limits = Limits {
        max_reference_graph_depth: 2,
        ..Limits::default()
    };
    assert!(matches!(
        Decoder::new(depth_limits).inspect(&bytes),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::ReferenceDepth,
            limit: 2,
            actual: 3
        }))
    ));

    let reference_limits = Limits {
        max_total_references: 1,
        ..Limits::default()
    };
    assert!(matches!(
        Decoder::new(reference_limits).inspect(&bytes),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::TotalReferences,
            limit: 1,
            actual: 2
        }))
    ));
}

#[test]
fn rejects_reference_to_nonretained_segment() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&header(0, 0, 0, 0));
    bytes.extend_from_slice(&header_with_reference(1, 0, 0));
    assert!(matches!(
        Decoder::default().inspect(&bytes),
        Err(DecodeError::Malformed(
            MalformedError::InvalidRetentionFlags {
                segment: 0,
                referred: 0
            }
        ))
    ));
}

#[test]
fn embedded_page_resolves_reusable_globals_explicitly() {
    let global_bytes = header(0, 0, 0, 0);
    let globals = Decoder::default()
        .parse_globals(&global_bytes)
        .expect("global dictionary is valid");
    assert_eq!(globals.segment_count(), 1);

    let mut page_bytes = header(1, 48, 1, 19);
    page_bytes.extend_from_slice(&page_data(8));
    let mut text_header = header_with_reference(2, 6, 0);
    text_header[7] = 1;
    page_bytes.extend_from_slice(&text_header);
    page_bytes.extend_from_slice(&header(3, 49, 1, 0));

    assert!(matches!(
        Decoder::default().inspect_embedded(&page_bytes),
        Err(DecodeError::Malformed(
            MalformedError::MissingReferredSegment {
                segment: 2,
                referred: 0
            }
        ))
    ));
    let info = Decoder::default()
        .inspect_embedded_with_globals(&page_bytes, &globals)
        .expect("page resolves its global dictionary");
    assert_eq!(info.pages().len(), 1);
}
