#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[derive(Clone, Copy, Debug)]
enum FramingOracle {
    Accepted,
    InvalidTail,
    Unexpected { offset: usize, code: u8 },
}

fn scalar_terminator_oracle(data: &[u8]) -> FramingOracle {
    if data.len() < 2 || data[data.len() - 2..] != [0xff, 0xac] {
        return FramingOracle::InvalidTail;
    }
    for (offset, pair) in data.windows(2).take(data.len() - 2).enumerate() {
        if pair[0] == 255 && (144..=255).contains(&pair[1]) {
            return FramingOracle::Unexpected {
                offset,
                code: pair[1],
            };
        }
    }
    FramingOracle::Accepted
}

fn assert_framing_oracle<T: core::fmt::Debug>(
    actual: &Result<T, GenericRegionError>,
    expected: FramingOracle,
) {
    match (expected, actual) {
        (FramingOracle::Accepted, Ok(_))
        | (FramingOracle::InvalidTail, Err(GenericRegionError::InvalidArithmeticTerminator)) => {}
        (
            FramingOracle::Unexpected { offset, code },
            Err(GenericRegionError::UnexpectedArithmeticMarker {
                offset: actual_offset,
                code: actual_code,
            }),
        ) => {
            assert_eq!(offset, *actual_offset);
            assert_eq!(code, *actual_code);
        }
        _ => panic!("framing result {actual:?}, expected {expected:?}"),
    }
}

#[test]
fn strict_terminator_matches_every_byte_pair_and_first_error_priority() {
    let mut checked = 0;
    for first in 0..=u8::MAX {
        for second in 0..=u8::MAX {
            for data in [&[first, second][..], &[first, second, 0x33, 0xff, 0xac]] {
                assert_framing_oracle(
                    &validate_arithmetic_terminator(data),
                    scalar_terminator_oracle(data),
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 131_072);
    for code in 0x90..=u8::MAX {
        assert_framing_oracle(
            &validate_arithmetic_terminator(&[0x21, 0xff, code, 0xff, 0x91, 0xff, 0xac]),
            FramingOracle::Unexpected { offset: 1, code },
        );
        assert_framing_oracle(
            &validate_arithmetic_terminator(&[0xff, code, 0xff, 0xab]),
            FramingOracle::InvalidTail,
        );
    }
}

fn scalar_observed_oracle(
    marker: Option<crate::arithmetic::ArithmeticMarker>,
    length: usize,
) -> FramingOracle {
    match marker {
        None => FramingOracle::Accepted,
        Some(_) if length < 2 => FramingOracle::InvalidTail,
        Some(marker) if marker.code == 172 && marker.offset == length - 2 => {
            FramingOracle::Accepted
        }
        Some(marker) => FramingOracle::Unexpected {
            offset: marker.offset,
            code: marker.code,
        },
    }
}

#[test]
fn session_queries_and_marker_results_match_stored_metadata_without_mutation() {
    let limits = Limits::default();
    let mut session =
        GenericArithmeticSession::new(&[0xff, 0xac], GenericTemplate::Template3, false, &limits)
            .unwrap();
    let bank = session.contexts.contexts.clone();
    for length in [0, 1, 2, 3, 7, usize::MAX] {
        for position in [0, 1, 7, usize::MAX] {
            for code in [0, 0x8f, 0x90, 0xac, 0xad, 0xff] {
                for marker_offset in [
                    None,
                    Some(0),
                    Some(1),
                    Some(length.saturating_sub(2)),
                    Some(usize::MAX),
                ] {
                    let marker = marker_offset
                        .map(|offset| crate::arithmetic::ArithmeticMarker { offset, code });
                    session.coded_length = length;
                    session.decoder.byte_position = position;
                    session.decoder.marker = marker;
                    let before = format!("{:?}", session.decoder);
                    assert_eq!(session.consumed_byte_offset(), position);
                    assert_eq!(session.observed_marker_offset(), marker_offset);
                    let observed = scalar_observed_oracle(marker, length);
                    assert_framing_oracle(
                        &validate_observed_marker(&session.decoder, length),
                        observed,
                    );
                    let final_expected = if marker.is_none() && length < 2 {
                        FramingOracle::InvalidTail
                    } else {
                        observed
                    };
                    let actual = session.validate_final_marker();
                    assert_framing_oracle(&actual, final_expected);
                    if let Ok(offset) = actual {
                        assert_eq!(offset, length - 2);
                    }
                    assert_eq!(format!("{:?}", session.decoder), before);
                    assert_eq!(session.coded_length, length);
                    assert_eq!(session.work_used, 0);
                    assert_eq!(session.contexts.contexts, bank);
                }
            }
        }
    }
}

#[test]
fn fresh_sessions_preserve_supplied_stream_configuration_and_limits() {
    let limits = Limits {
        max_stream_bytes: 97,
        max_decoder_managed_memory_bytes: 300_000,
        max_work_units: 12_345,
        ..Limits::default()
    };
    for template in [
        GenericTemplate::Template0,
        GenericTemplate::Template1,
        GenericTemplate::Template2,
        GenericTemplate::Template3,
    ] {
        for extended in [false, true] {
            for coded in [&[0xff, 0xac][..], &[0x12, 0xff, 0xac]] {
                let session =
                    GenericArithmeticSession::new(coded, template, extended, &limits).unwrap();
                assert_eq!(session.decoder.bytes, coded);
                assert_eq!(session.coded_length, coded.len());
                assert_eq!(session.work_used, 0);
                assert_eq!(session.contexts.template, template);
                assert_eq!(session.contexts.extended_template, extended);
                assert_eq!(
                    (
                        session.limits.max_stream_bytes,
                        session.limits.max_decoder_managed_memory_bytes,
                        session.limits.max_work_units
                    ),
                    (97, 300_000, 12_345),
                );
            }
        }
    }
}

#[test]
fn arithmetic_marker_validation_is_bounded_and_strict() {
    assert!(validate_arithmetic_terminator(&[0x12, 0xff, 0xac]).is_ok());
    assert!(matches!(
        validate_arithmetic_terminator(&[0xff, 0x90, 0xff, 0xac]),
        Err(GenericRegionError::UnexpectedArithmeticMarker { .. })
    ));
    assert!(matches!(
        validate_arithmetic_terminator(&[0x12, 0xff]),
        Err(GenericRegionError::InvalidArithmeticTerminator)
    ));
}

#[test]
fn unknown_length_arithmetic_count_is_validated() {
    let limits = Limits::default();
    let data = [0x12, 0xff, 0xac, 0, 0, 0, 3, 0xde, 0xad];
    let terminal = split_unknown_length_data(&data, false, 4, &limits).expect("terminal");
    assert_eq!(terminal.coded, &[0x12, 0xff, 0xac]);
    assert_eq!(terminal.rows, 3);
    assert_eq!(terminal.consumed, 7);

    let too_many = [0xff, 0xac, 0, 0, 0, 5];
    assert!(matches!(
        split_unknown_length_data(&too_many, false, 4, &limits),
        Err(GenericRegionError::Decode(DecodeError::Malformed(
            MalformedError::GenericRegionRowCount {
                declared: 4,
                actual: 5
            }
        )))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn generic_preflight_errors_preserve_stream_statistics_work_and_direct_bytes() {
    let limits = Limits::default();
    let header = generic_header(9, 2, GenericTemplate::Template3, false, false);
    let coded = [0xff, 0xac];
    for route in 0..3 {
        for case in 0..4 {
            let rows = if case == 0 { 3 } else { 1 };
            let initial = if case == 1 { u64::MAX } else { 11 };
            let limited = match case {
                2 => Limits {
                    max_work_units: 163,
                    ..limits
                },
                3 => Limits {
                    max_decoder_managed_memory_bytes: 2051,
                    ..limits
                },
                _ => limits,
            };
            let mut decoder = ArithmeticDecoder::new(&coded).unwrap();
            let mut contexts =
                GenericContexts::new(GenericTemplate::Template3, false, &limits).unwrap();
            let mut work = initial;
            let mut target = Bitmap::filled(9, 2, true, &limits).unwrap();
            let call = GenericArithmeticCall {
                width: 9,
                height: 2,
                adaptive: header.adaptive,
                typical_prediction: false,
                skip: None,
            };
            let result = match route {
                0 => decode_generic_on_into(
                    &mut decoder,
                    &mut contexts,
                    call,
                    rows,
                    &limited,
                    &mut work,
                    &mut target,
                ),
                1 => {
                    decode_generic_on(&mut decoder, &mut contexts, call, rows, &limited, &mut work)
                        .map(|_| ())
                }
                _ => decode_generic_on_validated(
                    &mut decoder,
                    &mut contexts,
                    call,
                    rows,
                    &limited,
                    &mut work,
                )
                .map(|_| ()),
            };
            if case == 0 {
                assert!(matches!(
                    result,
                    Err(GenericRegionError::Decode(DecodeError::Malformed(
                        MalformedError::GenericRegionRowCount {
                            declared: 2,
                            actual: 3
                        }
                    )))
                ));
            } else {
                let expected = match case {
                    1 => None,
                    2 => Some(LimitExceeded {
                        kind: LimitKind::Work,
                        limit: 163,
                        actual: 164,
                    }),
                    3 => Some(LimitExceeded {
                        kind: LimitKind::Memory,
                        limit: 2051,
                        actual: 2052,
                    }),
                    _ => unreachable!(),
                };
                assert_raw_guard_result(result, Err(expected));
            }
            assert_eq!(work, initial);
            assert_eq!(target.bytes(), &[0xff, 0x80, 0xff, 0x80]);
            assert!(contexts
                .contexts
                .iter()
                .all(|c| c.state_index() == 0 && c.mps() == 0));
            assert_decoder_state_eq(&decoder, &ArithmeticDecoder::new(&coded).unwrap());
        }
    }
    for (width, height) in [(8, 2), (9, 3)] {
        let skip = Bitmap::blank(width, height, &limits).unwrap();
        assert!(matches!(
            decode_arithmetic_generic(&coded, &header, 0, Some(&skip), &limits),
            Err(GenericRegionError::InvalidSkipDimensions)
        ));
        let mut decoder = ArithmeticDecoder::new(&coded).unwrap();
        let mut contexts =
            GenericContexts::new(GenericTemplate::Template3, false, &limits).unwrap();
        let mut work = 11;
        assert!(matches!(
            decode_generic_on(
                &mut decoder,
                &mut contexts,
                GenericArithmeticCall {
                    width: 9,
                    height: 2,
                    adaptive: header.adaptive,
                    typical_prediction: false,
                    skip: Some(&skip),
                },
                0,
                &limits,
                &mut work,
            ),
            Err(GenericRegionError::InvalidSkipDimensions)
        ));
        assert_eq!(work, 11);
        assert_decoder_state_eq(&decoder, &ArithmeticDecoder::new(&coded).unwrap());
    }
}

#[test]
fn known_length_normalization_preserves_zero_row_bitmap_geometry() {
    let limits = Limits::default();
    for (template, extended) in [
        (GenericTemplate::Template0, false),
        (GenericTemplate::Template0, true),
        (GenericTemplate::Template1, false),
        (GenericTemplate::Template2, false),
        (GenericTemplate::Template3, false),
    ] {
        for typical in [false, true] {
            let header = generic_header(9, 2, template, extended, typical);
            for coded in [&[0xff][..], &[0x12, 0xff], &[0xff, 0xac]] {
                let owned = decode_known_length_arithmetic_generic(coded, &header, 0, &limits)
                    .expect("zero rows require no entropy decisions");
                let mut direct = Bitmap::blank(9, 2, &limits).unwrap();
                decode_known_length_arithmetic_generic_into(
                    coded,
                    &header,
                    0,
                    &limits,
                    &mut direct,
                )
                .expect("zero-row direct decode");
                for bitmap in [&owned, &direct] {
                    assert_eq!(bitmap.width(), 9);
                    assert_eq!(bitmap.height(), 2);
                    assert_eq!(bitmap.stride(), 2);
                    assert_eq!(bitmap.bytes(), &[0, 0, 0, 0]);
                }
            }
        }
    }
}

#[test]
fn known_length_normalization_reports_exact_errors_before_bitmap_mutation() {
    let limits = Limits::default();
    let header = generic_header(9, 2, GenericTemplate::Template3, false, false);
    for coded in [&[][..], &[0], &[0xff, 0], &[0, 0xac]] {
        let mut direct = Bitmap::filled(9, 2, true, &limits).unwrap();
        let before = direct.bytes().to_vec();
        for result in [
            decode_known_length_arithmetic_generic(coded, &header, 0, &limits).map(|_| ()),
            decode_known_length_arithmetic_generic_into(coded, &header, 0, &limits, &mut direct),
        ] {
            assert!(matches!(
                result,
                Err(GenericRegionError::InvalidArithmeticTerminator)
            ));
        }
        assert_eq!(direct.bytes(), before);
    }

    for (coded, limit, actual) in [(&[0xff][..], 1, 2), (&[0x12, 0xff][..], 2, 3)] {
        let limited = Limits {
            max_decoder_managed_memory_bytes: limit,
            ..limits
        };
        let mut direct = Bitmap::filled(9, 2, true, &limits).unwrap();
        let before = direct.bytes().to_vec();
        for result in [
            decode_known_length_arithmetic_generic(coded, &header, 0, &limited).map(|_| ()),
            decode_known_length_arithmetic_generic_into(coded, &header, 0, &limited, &mut direct),
        ] {
            assert!(matches!(
                result,
                Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
                    LimitExceeded {
                        kind: LimitKind::Memory,
                        limit: observed_limit,
                        actual: observed_actual,
                    }
                ))) if observed_limit == limit && observed_actual == actual
            ));
        }
        assert_eq!(direct.bytes(), before);
    }

    let context_bytes = 1024 * u64::try_from(std::mem::size_of::<ArithmeticContext>()).unwrap();
    let limited = Limits {
        max_decoder_managed_memory_bytes: 2,
        ..limits
    };
    for coded in [&[0xff][..], &[0xff, 0xac]] {
        let mut direct = Bitmap::filled(9, 2, true, &limits).unwrap();
        for result in [
            decode_known_length_arithmetic_generic(coded, &header, 0, &limited).map(|_| ()),
            decode_known_length_arithmetic_generic_into(coded, &header, 0, &limited, &mut direct),
        ] {
            assert!(matches!(
                result,
                Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
                    LimitExceeded {
                        kind: LimitKind::Memory,
                        limit: 2,
                        actual,
                    }
                ))) if actual == context_bytes
            ));
        }
        assert_eq!(direct.bytes(), &[0xff, 0x80, 0xff, 0x80]);
    }
}

#[test]
fn unknown_length_framing_preserves_count_extent_and_exact_eof_fields() {
    let limits = Limits::default();
    for mmr in [false, true] {
        let marker = if mmr { [0, 0] } else { [0xff, 0xac] };
        for rows in [0_u32, 1, 0x100, 0x1_0000, 0x100_0000, u32::MAX] {
            let mut data = vec![0x12, 0x34, marker[0], marker[1]];
            data.extend_from_slice(&rows.to_be_bytes());
            data.extend_from_slice(&[0xde, 0xad]);
            let terminal = split_unknown_length_data(&data, mmr, rows, &limits).unwrap();
            assert_eq!(terminal.rows, rows);
            assert_eq!(terminal.consumed, 8);
            assert_eq!(terminal.coded, &data[..if mmr { 2 } else { 4 }]);
            if rows != 0 {
                assert!(matches!(
                    split_unknown_length_data(&data, mmr, rows - 1, &limits),
                    Err(GenericRegionError::Decode(DecodeError::Malformed(
                        MalformedError::GenericRegionRowCount { declared, actual }
                    ))) if declared == rows - 1 && actual == rows
                ));
            }
        }
        for count_bytes in 0..4 {
            let mut truncated = marker.to_vec();
            truncated.extend_from_slice(&[0; 3][..count_bytes]);
            assert!(matches!(
                split_unknown_length_data(&truncated, mmr, 0, &limits),
                Err(GenericRegionError::Decode(DecodeError::Malformed(
                    MalformedError::UnexpectedEof {
                        offset: 2,
                        needed: 4,
                        remaining,
                    }
                ))) if remaining == u64::try_from(count_bytes).unwrap()
            ));
        }
        for data in [&[][..], &[0x12], &[0x12, marker[0]], &[marker[0], 0x34]] {
            assert!(matches!(
                split_unknown_length_data(data, mmr, 0, &limits),
                Err(GenericRegionError::Decode(DecodeError::Malformed(
                    MalformedError::MissingGenericRegionTerminator
                )))
            ));
        }
    }
}

#[test]
fn direct_generic_dispatch_declines_unsupported_routes_without_mutation() {
    let limits = Limits::default();
    for (mmr, width, height) in [(true, 9, 2), (false, 8, 2), (false, 9, 3)] {
        let mut payload = region_prefix(9, 2);
        if mmr {
            payload.push(1);
        } else {
            payload.extend_from_slice(&[6, 2, 0xff]);
        }
        for unknown_length in [false, true] {
            let mut target = Bitmap::filled(width, height, true, &limits).unwrap();
            let before = target.bytes().to_vec();
            assert!(decode_generic_region_segment_into(
                &payload,
                unknown_length,
                &limits,
                &mut target,
            )
            .unwrap()
            .is_none());
            assert_eq!(target.width(), width);
            assert_eq!(target.height(), height);
            assert_eq!(target.bytes(), before);
        }
    }
}

#[test]
fn zero_row_dispatch_reports_original_not_normalized_input_extent() {
    let limits = Limits::default();
    for unknown_length in [false, true] {
        let height = if unknown_length { 2 } else { 0 };
        let mut payload = region_prefix(9, height);
        payload.extend_from_slice(&[6, 2, 0xff]);
        if unknown_length {
            payload.extend_from_slice(&[0xff, 0xac, 0, 0, 0, 0, 0xde, 0xad]);
        } else {
            payload.push(0xff);
        }
        let expected_consumed = if unknown_length { 26 } else { 21 };
        let owned = decode_generic_region_segment(&payload, unknown_length, &limits).unwrap();
        let mut direct = Bitmap::blank(9, height, &limits).unwrap();
        let metadata =
            decode_generic_region_segment_into(&payload, unknown_length, &limits, &mut direct)
                .unwrap()
                .unwrap();
        assert_eq!(owned.consumed_bytes, expected_consumed);
        assert_eq!(metadata.consumed_bytes, expected_consumed);
        assert_eq!(owned.decoded_rows, 0);
        assert_eq!(metadata.decoded_rows, 0);
        for bitmap in [&owned.bitmap, &direct] {
            assert_eq!(bitmap.width(), 9);
            assert_eq!(bitmap.height(), height);
            assert!(bitmap.bytes().iter().all(|byte| *byte == 0));
        }
    }
}

#[test]
fn known_length_mmr_dispatches_to_shared_decoder() {
    let limits = Limits::default();
    let mut payload = region_prefix(8, 1);
    payload.push(0x01);
    payload.push(0x80);
    let decoded = decode_generic_region_segment(&payload, false, &limits).expect("MMR");
    assert_eq!(decoded.decoded_rows, 1);
    assert_eq!(decoded.consumed_bytes, payload.len());
    assert!(decoded.bitmap.bytes().iter().all(|byte| *byte == 0));
}

#[test]
fn annex_h_generic_arithmetic_segment_decodes() {
    let limits = Limits::default();
    let mut payload = region_prefix(54, 44);
    payload[8..12].copy_from_slice(&4_u32.to_be_bytes());
    payload[12..16].copy_from_slice(&11_u32.to_be_bytes());
    payload[16] = 0;
    payload.push(0x08);
    payload.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    payload.extend_from_slice(&[0x04, 0xee, 0xed, 0x87, 0xfb, 0xcb, 0x2b, 0xff, 0xac]);

    let decoded = decode_generic_region_segment(&payload, false, &limits)
        .expect("Annex H segment must decode");
    assert_eq!(decoded.bitmap.width(), 54);
    assert_eq!(decoded.bitmap.height(), 44);
    assert_eq!(decoded.consumed_bytes, payload.len());
    assert!(
        decoded.bitmap.bytes().iter().any(|byte| *byte != 0),
        "the Annex H region is not blank"
    );
}

#[test]
fn bounded_refinement_uses_shared_arithmetic_decoder() {
    let limits = Limits::default();
    let reference = Bitmap::blank(1, 1, &limits).expect("reference");
    let header = RefinementRegionHeader {
        region: region_info(1, 1),
        template: RefinementTemplate::Template1,
        typical_prediction: false,
        adaptive: [TemplateOffset::ZERO; 2],
    };
    let decoded = decode_arithmetic_refinement(
        &[0xff, 0xac],
        &header,
        RefinementParameters {
            reference_dx: 0,
            reference_dy: 0,
        },
        &reference,
        &limits,
    )
    .expect("marker fill supplies a bounded final decision");
    assert_eq!(decoded.pixel(0, 0), Some(true));
}

#[test]
fn unknown_length_mmr_requires_eofb_and_pads_nominal_rows() {
    let limits = Limits::default();
    let mut payload = region_prefix(8, 2);
    payload.push(0x01);
    payload.extend_from_slice(&[0x80, 0x08, 0x00, 0x80]);
    payload.extend_from_slice(&[0x00, 0x00, 0, 0, 0, 1]);
    payload.extend_from_slice(&[0xde, 0xad]);
    let decoded = decode_generic_region_segment(&payload, true, &limits).expect("MMR");
    assert_eq!(decoded.decoded_rows, 1);
    assert_eq!(decoded.bitmap.height(), 2);
    assert_eq!(decoded.consumed_bytes, payload.len() - 2);
    assert!(decoded.bitmap.bytes().iter().all(|byte| *byte == 0));
}

#[test]
fn work_and_memory_limits_are_enforced_before_decode() {
    let limits = Limits {
        max_work_units: 1,
        ..Limits::default()
    };
    assert!(matches!(
        enforce_generic_work(2, 1, false, &limits),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Work,
                ..
            }
        )))
    ));

    let memory_limits = Limits {
        max_decoder_managed_memory_bytes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        allocate_contexts(1024, 1, &memory_limits),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Memory,
                ..
            }
        )))
    ));
}
