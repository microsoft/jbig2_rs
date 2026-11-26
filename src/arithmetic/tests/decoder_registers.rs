#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn initialization_accepts_every_byte_pair_and_preserves_suffixes() {
    for first in 0_u8..=u8::MAX {
        for next in 0_u8..=u8::MAX {
            let bytes = [first, next];
            let marker = first == 0xff && next >= 0x90;
            let stuffed = first == 0xff && !marker;
            let increment = if marker {
                65_280
            } else {
                u64::from(next) * if stuffed { 512 } else { 256 }
            };
            let expected = ArithmeticDecoder {
                bytes: &bytes,
                byte_position: usize::from(!marker),
                current_byte: if marker { first } else { next },
                code: u32::try_from((u64::from(first) * 65_536 + increment) * 128)
                    .expect("two-byte initialization fits u32"),
                interval: MIN_INTERVAL,
                bit_count: u8::from(!stuffed),
                marker: if marker {
                    Some(ArithmeticMarker {
                        offset: 0,
                        code: next,
                    })
                } else {
                    None
                },
            };
            let actual = ArithmeticDecoder::new(&bytes).expect("every two-byte prefix");
            assert_decoder_state_eq(&actual, &expected);
            assert_eq!(actual.bytes, bytes);
            let suffix = [first, next, 0xff, 0xac];
            let extended = ArithmeticDecoder::new(&suffix).expect("unchanged prefix");
            assert_decoder_state_eq(&extended, &expected);
            assert_eq!(extended.bytes, suffix);
        }
    }
}

#[test]
fn initialization_short_input_errors_preserve_exact_offsets() {
    assert!(matches!(
        ArithmeticDecoder::new(&[]),
        Err(ArithmeticError::Malformed(MalformedError::UnexpectedEof {
            offset: 0,
            needed: 1,
            remaining: 0
        }))
    ));
    for first in 0_u8..=u8::MAX {
        assert!(matches!(
            ArithmeticDecoder::new(&[first]),
            Err(ArithmeticError::Malformed(MalformedError::UnexpectedEof {
                offset: 1,
                needed: 1,
                remaining: 0
            }))
        ));
    }
}

#[test]
fn refill_byte_pairs_match_numeric_code_consumption_and_counter_boundaries() {
    for current in 0_u8..=u8::MAX {
        for next in 0_u8..=u8::MAX {
            let bytes = [current, next];
            let increment = if current == 0xff && next >= 0x90 {
                65_280
            } else {
                u32::from(next) * if current == 0xff { 512 } else { 256 }
            };
            let maximum = u32::MAX - increment;
            for bit_count in [0, 7, 8] {
                for code in [0, maximum, u32::MAX] {
                    assert_refill_matches_numeric_oracle(&bytes, 0, code, bit_count, None);
                }
                if increment != 0 {
                    assert_refill_matches_numeric_oracle(&bytes, 0, maximum + 1, bit_count, None);
                }
            }
        }
    }
}

#[test]
fn refill_eof_and_existing_markers_preserve_exact_error_order_and_frames() {
    for current in 0_u8..=u8::MAX {
        for code in [0, u32::MAX] {
            for bit_count in [0, 3, 8] {
                assert_refill_matches_numeric_oracle(&[current], 0, code, bit_count, None);
                assert_refill_matches_numeric_oracle(&[0x12, current], 1, code, bit_count, None);
            }
        }
    }
    for next in 0x90_u8..=u8::MAX {
        let bytes = [0x12, 0xff, next];
        for code in [0, u32::MAX - 65_280, u32::MAX - 65_279, u32::MAX] {
            for bit_count in [0, 3, 8] {
                assert_refill_matches_numeric_oracle(
                    &bytes,
                    1,
                    code,
                    bit_count,
                    Some(ArithmeticMarker {
                        offset: 1,
                        code: next,
                    }),
                );
            }
        }
    }
}

#[test]
fn successful_decisions_and_contexts_cover_every_probability_row_and_route() {
    let bytes = [0; 32];
    for (index, state) in QE_STATES.iter().copied().enumerate() {
        let qe = u32::from(state.qe);
        for old_mps in 0..=1 {
            for interval in [
                MIN_INTERVAL,
                0xffff,
                (2 * qe).max(MIN_INTERVAL),
                (MIN_INTERVAL + qe).min(0xffff),
            ] {
                for upper in [0, qe - 1, qe, qe + 1] {
                    let code = upper << 16;
                    let reduced = interval - qe;
                    let lower = upper < qe;
                    let fast = !lower && reduced >= MIN_INTERVAL;
                    let mps_route = (lower && reduced < qe) || (!lower && reduced >= qe);
                    let decision = if fast || mps_route {
                        old_mps
                    } else {
                        1 - old_mps
                    };
                    let expected = ArithmeticContext::from_state(
                        if fast {
                            u8::try_from(index).unwrap()
                        } else if mps_route {
                            state.next_mps
                        } else {
                            state.next_lps
                        },
                        if !fast && !mps_route && state.switch_mps {
                            1 - old_mps
                        } else {
                            old_mps
                        },
                    )
                    .unwrap();
                    let mut decoder = decision_fixture(&bytes, interval, code, 8);
                    let mut context =
                        ArithmeticContext::from_state(u8::try_from(index).unwrap(), old_mps)
                            .unwrap();
                    assert_eq!(decoder.decode_signal(&mut context), decision);
                    assert_eq!(context, expected);
                }
            }
        }
    }
}

#[test]
fn exchange_context_update_precedes_truncation_and_keeps_supplied_mps_distinct() {
    let bytes = [0];
    for state in QE_STATES {
        let qe = u32::from(state.qe);
        for lower in [false, true] {
            for prior_mps in 0..=1 {
                for supplied_mps in 0..=1 {
                    let mut decoder = decision_fixture(&bytes, 1, 0, 0);
                    let mut context = ArithmeticContext::from_state(0, prior_mps).unwrap();
                    let mps_route = (1 < qe) == lower;
                    let expected_index = if mps_route {
                        state.next_mps
                    } else {
                        state.next_lps
                    };
                    let expected_mps = if !mps_route && state.switch_mps {
                        1 - supplied_mps
                    } else {
                        prior_mps
                    };
                    let result = if lower {
                        decoder.decode_lower_signal(&mut context, state, qe, supplied_mps)
                    } else {
                        decoder.decode_upper_signal(&mut context, state, qe, supplied_mps)
                    };
                    assert_eq!(result, RASTER_SIGNAL_EOF);
                    assert_eq!(context.state_index(), expected_index);
                    assert_eq!(context.mps(), expected_mps);
                }
            }
        }
    }
}

#[test]
fn entry_errors_preserve_context_updates_input_and_exact_partial_registers() {
    let bytes = [0x11];
    for (index, state) in QE_STATES.iter().copied().enumerate() {
        let qe = u32::from(state.qe);
        for old_mps in 0..=1 {
            for interval in [MIN_INTERVAL, 0xffff] {
                for upper in [0, qe - 1, qe, qe + 1] {
                    for low in [0, 0xffff] {
                        let code = (upper << 16) | low;
                        let reduced = interval - qe;
                        let lower = upper < qe;
                        let fast = !lower && reduced >= MIN_INTERVAL;
                        let mps_route = (lower && reduced < qe) || (!lower && reduced >= qe);
                        let expected_context = ArithmeticContext::from_state(
                            if fast {
                                u8::try_from(index).unwrap()
                            } else if mps_route {
                                state.next_mps
                            } else {
                                state.next_lps
                            },
                            if !fast && !mps_route && state.switch_mps {
                                1 - old_mps
                            } else {
                                old_mps
                            },
                        )
                        .unwrap();
                        for checked in [false, true] {
                            let mut decoder = decision_fixture(&bytes, interval, code, 0);
                            let mut context = ArithmeticContext::from_state(
                                u8::try_from(index).unwrap(),
                                old_mps,
                            )
                            .unwrap();
                            if checked {
                                assert_eq!(
                                    decoder.decode(&mut context),
                                    if fast {
                                        Ok(old_mps)
                                    } else {
                                        Err(unexpected_eof(1, 1, 0))
                                    },
                                );
                            } else {
                                assert_eq!(
                                    decoder.decode_signal(&mut context),
                                    if fast { old_mps } else { RASTER_SIGNAL_EOF },
                                );
                            }
                            assert_eq!(context, expected_context);
                            assert_eq!(decoder.interval, if lower { qe } else { reduced });
                            let expected_code = if lower {
                                u64::from(code)
                            } else {
                                u64::from(code) - u64::from(qe) * 65536
                            };
                            assert_eq!(u64::from(decoder.code), expected_code);
                            assert_eq!(decoder.bit_count, 0);
                            assert_eq!(decoder.byte_position, 0);
                            assert_eq!(decoder.current_byte, 0x11);
                            assert_eq!(decoder.marker, None);
                            assert_eq!(decoder.bytes, bytes);
                            assert_eq!(decoder.bytes.as_ptr(), bytes.as_ptr());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn first_annex_h_decisions_follow_mps_state_transitions() {
    let mut decoder = ArithmeticDecoder::new(&ANNEX_H_ENCODED).expect("valid initialization");
    let mut context = ArithmeticContext::new();
    assert_eq!(decoder.code, 0x4263_8000);

    assert_eq!(decoder.decode(&mut context).expect("decision"), 0);
    assert_eq!((context.state_index(), context.mps()), (1, 0));
    assert_eq!(decoder.decode(&mut context).expect("decision"), 0);
    assert_eq!((context.state_index(), context.mps()), (2, 0));
    assert_eq!(decoder.decode(&mut context).expect("decision"), 0);
    assert_eq!((context.state_index(), context.mps()), (2, 0));
}

#[test]
fn marker_is_strictly_greater_than_8f_and_is_not_consumed() {
    let stuffed = [0xff, 0x8f, 0x00];
    let decoder = ArithmeticDecoder::new(&stuffed).expect("0x8f is stuffed data");
    assert_eq!(decoder.marker(), None);
    assert_eq!(decoder.byte_position(), 1);
    assert_eq!(decoder.bit_count, 0);

    let marker = [0xff, 0x90];
    let decoder = ArithmeticDecoder::new(&marker).expect("0x90 is a marker");
    assert_eq!(
        decoder.marker(),
        Some(ArithmeticMarker {
            offset: 0,
            code: 0x90,
        })
    );
    assert_eq!(decoder.byte_position(), 0);
    assert_eq!(decoder.bit_count, 1);
}

#[test]
fn initialization_and_byte_in_reject_truncation() {
    for bytes in [&[][..], &[0x00][..], &[0xff][..]] {
        let error = ArithmeticDecoder::new(bytes).expect_err("must reject truncation");
        assert!(matches!(
            error,
            ArithmeticError::Malformed(MalformedError::UnexpectedEof { .. })
        ));
    }
}

#[test]
fn marker_fill_requires_a_real_marker() {
    let mut decoder = ArithmeticDecoder::new(&[0xff, 0xac]).expect("complete marker");
    decoder.bit_count = 0;
    let old_code = decoder.code;
    decoder.byte_in().expect("marker supplies fill bits");
    assert_eq!(decoder.code, old_code + 0xff00);
    assert_eq!(decoder.byte_position(), 0);

    let mut truncated = ArithmeticDecoder {
        bytes: &[0x00, 0x01],
        byte_position: 1,
        current_byte: 0x01,
        code: 0,
        interval: 0x4000,
        bit_count: 0,
        marker: None,
    };
    let error = truncated
        .renormalize()
        .expect_err("raw EOF is not marker fill");
    assert!(matches!(
        error,
        ArithmeticError::Malformed(MalformedError::UnexpectedEof { .. })
    ));
}

#[test]
fn renormalization_shifts_until_interval_is_normalized() {
    let mut decoder = ArithmeticDecoder {
        bytes: &[0x00, 0x00],
        byte_position: 1,
        current_byte: 0,
        code: 3,
        interval: 0x2000,
        bit_count: 3,
        marker: None,
    };
    decoder.renormalize().expect("bounded shifts");
    assert_eq!(decoder.interval, 0x8000);
    assert_eq!(decoder.code, 12);
    assert_eq!(decoder.bit_count, 1);
}

#[test]
fn complete_decision_registers_match_wide_numeric_oracle_on_success_and_partial_errors() {
    let samples: [&[u8]; 6] = [
        &[0x13],
        &[0, 0x11],
        &[0xff, 0x8f, 0],
        &[0xff, 0x90, 0],
        &[0x5a, 0xff, 0x90],
        &[0; 32],
    ];
    let mut outcomes = [0usize; 3];
    for bytes in samples {
        for position in [0, bytes.len() - 1] {
            let marker = bytes
                .get(position + 1)
                .copied()
                .filter(|&next| bytes[position] == 0xff && next >= 144)
                .map(|code| ArithmeticMarker {
                    offset: position,
                    code,
                });
            for marker in [None, marker] {
                for (index, row) in QE_STATES.iter().copied().enumerate() {
                    let qe = u32::from(row.qe);
                    for mps in 0..=1 {
                        let context =
                            ArithmeticContext::from_state(u8::try_from(index).unwrap(), mps)
                                .unwrap();
                        for interval in [
                            MIN_INTERVAL,
                            0xffff,
                            (2 * qe).max(MIN_INTERVAL),
                            (MIN_INTERVAL + qe).min(0xffff),
                        ] {
                            for code in [
                                0,
                                (qe - 1) << 16,
                                qe << 16,
                                (qe + 1) << 16,
                                u32::MAX / 2,
                                u32::MAX / 2 + 1,
                                u32::MAX,
                            ] {
                                for count in [0, 1, 7, 8] {
                                    let mut initial =
                                        refill_fixture(bytes, position, code, count, marker);
                                    initial.interval = interval;
                                    let signal =
                                        assert_decision_matches_numeric_oracle(initial, context);
                                    let outcome = if signal <= 1 {
                                        0
                                    } else if signal == RASTER_SIGNAL_EOF {
                                        1
                                    } else {
                                        assert_eq!(signal, RASTER_SIGNAL_OVERFLOW);
                                        2
                                    };
                                    outcomes[outcome] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(outcomes.into_iter().all(|count| count > 0));
}

#[test]
fn normalization_no_refill_matches_closed_form_and_exact_shift_boundaries() {
    let bytes = [0xff, 0xac];
    let marker = Some(ArithmeticMarker {
        offset: 0,
        code: 0xac,
    });
    for interval in 128_u32..=0xffff {
        let shifts = interval.leading_zeros().saturating_sub(16);
        let available = u8::try_from(shifts).unwrap();
        let maximum = u32::MAX >> shifts;
        for bit_count in [available, 8] {
            for code in [0, 1, maximum, maximum.saturating_add(1), u32::MAX] {
                let mut initial = refill_fixture(&bytes, 0, code, bit_count, marker);
                initial.interval = interval;
                if code <= maximum {
                    let mut actual = refill_fixture(&bytes, 0, code, bit_count, marker);
                    actual.interval = interval;
                    actual.renormalize().unwrap();
                    assert_eq!(actual.interval, interval << shifts);
                    assert_eq!(actual.code, code << shifts);
                    assert_eq!(actual.bit_count, bit_count - available);
                    assert_eq!(actual.byte_position, 0);
                    assert_eq!(actual.current_byte, 0xff);
                    assert_eq!(actual.marker, marker);
                }
                assert_normalization_matches_numeric_oracle(initial);
            }
        }
    }
}

#[test]
fn normalization_byte_pairs_and_partial_errors_match_wide_numeric_oracle() {
    for first in 0_u8..=u8::MAX {
        for next in 0_u8..=u8::MAX {
            let bytes = [first, next, 0x55];
            let marker = (first == 255 && next >= 144).then_some(ArithmeticMarker {
                offset: 0,
                code: next,
            });
            for interval in [1, 255, 0x4000, 0x7fff, MIN_INTERVAL] {
                for bit_count in [0, 1, 8] {
                    for code in [0, u32::MAX / 2, u32::MAX] {
                        let mut initial = refill_fixture(&bytes, 0, code, bit_count, marker);
                        initial.interval = interval;
                        assert_normalization_matches_numeric_oracle(initial);
                    }
                }
            }
        }
    }
    for bytes in [&[0_u8][..], &[0xff][..], &[0, 1][..], &[0xff, 0x8f][..]] {
        for position in 0..bytes.len() {
            for interval in [1, 0x4000, MIN_INTERVAL] {
                for bit_count in [0, 1, 8] {
                    for code in [0, u32::MAX / 2, u32::MAX] {
                        let mut initial = refill_fixture(bytes, position, code, bit_count, None);
                        initial.interval = interval;
                        assert_normalization_matches_numeric_oracle(initial);
                    }
                }
            }
        }
    }
}
