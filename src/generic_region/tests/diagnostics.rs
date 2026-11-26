#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
#[allow(clippy::too_many_lines)]
fn packed_rows_match_scalar_msb_windows_padding_and_numeric_edges() {
    let mut byte_cases = 0;
    let mut window_cases = 0;
    let mut extreme_cases = 0;
    for width in [0, 1, 7, 8, 9, 15, 16, 17] {
        let mut bitmap = Bitmap::blank(width, 3, &Limits::default()).unwrap();
        for (index, byte) in bitmap.data.iter_mut().enumerate() {
            *byte = u8::try_from(index % 256)
                .unwrap()
                .wrapping_mul(73)
                .wrapping_add(u8::try_from(width).unwrap())
                .wrapping_add(0xa5);
        }
        let before = bitmap.clone();
        for row in [-1, 0, 1, 2, 3] {
            let row_start = if (0..3).contains(&row) {
                Some(usize::try_from(row).unwrap() * bitmap.stride)
            } else {
                None
            };
            assert_eq!(packed_row_start(&bitmap, row), row_start);
            for center in -9..=i64::from(width) + 8 {
                let byte = if center < 0 || center >= i64::from(width) || row_start.is_none() {
                    0
                } else {
                    u8::try_from((0..8).fold(0_usize, |value, offset| {
                        value * 2
                            + usize::from(control_pixel(
                                &bitmap,
                                i128::from(center) + i128::from(offset),
                                i128::from(row),
                            ))
                    }))
                    .unwrap()
                };
                assert_eq!(packed_row_byte(&bitmap, row_start, center), byte);
                byte_cases += 1;
                for left in [-8, -3, 0, 3, 8] {
                    for count in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, u8::MAX] {
                        assert_eq!(
                            packed_row_bits(&bitmap, center, row, left, count),
                            control_window(&bitmap, center, row, left, count),
                        );
                        window_cases += 1;
                    }
                }
            }
            for center in [i64::MIN, i64::MIN + 1, i64::MAX - 1, i64::MAX] {
                for left in [i32::MIN, -1, 0, 1, i32::MAX] {
                    for count in [0, 1, 7, 8, 9, u8::MAX] {
                        assert_eq!(
                            packed_row_bits(&bitmap, center, row, left, count),
                            control_window(&bitmap, center, row, left, count),
                        );
                        extreme_cases += 1;
                    }
                }
            }
        }
        for invalid_start in [None, Some(bitmap.data.len()), Some(usize::MAX)] {
            for x in [0, 8] {
                assert_eq!(packed_row_byte(&bitmap, invalid_start, x), 0);
            }
        }
        assert_eq!(bitmap, before);
    }
    assert_eq!(byte_cases, 1085);
    assert_eq!(window_cases, 59_675);
    assert_eq!(extreme_cases, 4800);
}

#[test]
fn probability_state_validation_matches_all_raw_fields_and_first_error() {
    assert!(validate_probability_states(&[]).is_ok());
    let mut cases = 0;
    let mut accepted = 0;
    for packed in 0..=u8::MAX {
        let candidate = ArithmeticContext::from_packed(packed);
        let contexts = [
            ArithmeticContext::new(),
            candidate,
            ArithmeticContext::from_packed(u8::MAX),
        ];
        let valid = packed < 94;
        for with_tail in [false, true] {
            let slice = if with_tail {
                &contexts[..]
            } else {
                &contexts[..2]
            };
            match validate_probability_states(slice) {
                Ok(()) => {
                    assert!(valid && !with_tail);
                    accepted += 1;
                }
                Err(error) => {
                    let expected = if valid {
                        assert!(with_tail);
                        (u8::MAX >> 1, u8::MAX & 1)
                    } else {
                        (packed >> 1, packed & 1)
                    };
                    assert_eq!(
                        control_malformed(error),
                        MalformedError::InvalidArithmeticContextState {
                            state_index: expected.0,
                            mps: expected.1,
                        }
                    );
                }
            }
            assert_eq!(contexts[1].state_index(), packed >> 1);
            assert_eq!(contexts[1].mps(), packed & 1);
            cases += 1;
        }
    }
    assert_eq!(cases, 512);
    assert_eq!(accepted, 94);
}

#[test]
fn generic_error_trait_projection_and_eof_factory_preserve_fields() {
    for (error, expected) in [
        (
            GenericRegionError::ReservedFlags(u8::MAX),
            GenericRegionErrorKind::ReservedFlags(u8::MAX),
        ),
        (
            GenericRegionError::InvalidFlags(u8::MAX),
            GenericRegionErrorKind::InvalidFlags(u8::MAX),
        ),
        (
            GenericRegionError::InvalidAdaptivePixel {
                number: u8::MAX,
                x: i32::MIN,
                y: i32::MAX,
            },
            GenericRegionErrorKind::InvalidAdaptivePixel {
                number: u8::MAX,
                x: i32::MIN,
                y: i32::MAX,
            },
        ),
        (
            GenericRegionError::InvalidSkipDimensions,
            GenericRegionErrorKind::InvalidSkipDimensions,
        ),
        (
            GenericRegionError::InvalidArithmeticTerminator,
            GenericRegionErrorKind::InvalidArithmeticTerminator,
        ),
        (
            GenericRegionError::UnexpectedArithmeticMarker {
                offset: usize::MAX,
                code: u8::MAX,
            },
            GenericRegionErrorKind::UnexpectedArithmeticMarker {
                offset: usize::MAX,
                code: u8::MAX,
            },
        ),
        (
            GenericRegionError::RowCountMismatch {
                expected: u32::MAX,
                actual: 0,
            },
            GenericRegionErrorKind::RowCountMismatch {
                expected: u32::MAX,
                actual: 0,
            },
        ),
        (
            GenericRegionError::ContextIndex {
                index: usize::MAX,
                count: 0,
            },
            GenericRegionErrorKind::ContextIndex {
                index: usize::MAX,
                count: 0,
            },
        ),
    ] {
        assert_eq!(
            control_malformed(error),
            MalformedError::InvalidGenericRegion(expected)
        );
    }
    let mut cases = 0;
    for offset in [0, 1, 65_535, usize::MAX] {
        for needed in [0, 1, 65_535, usize::MAX] {
            for remaining in [0, 1, 65_535, usize::MAX] {
                let fits = [offset, needed, remaining]
                    .into_iter()
                    .all(|value| value as u128 <= u128::from(u64::MAX));
                let actual = control_malformed(unexpected_eof(offset, needed, remaining));
                if fits {
                    assert_eq!(
                        actual,
                        MalformedError::UnexpectedEof {
                            offset: u64::try_from(offset).unwrap(),
                            needed: u64::try_from(needed).unwrap(),
                            remaining: u64::try_from(remaining).unwrap(),
                        }
                    );
                } else {
                    assert_eq!(actual, MalformedError::IntegerOverflow);
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 64);
}

#[test]
fn probability_failure_projection_preserves_context_and_decoder_errors() {
    let bytes = [0, 0];
    let decoder = ArithmeticDecoder::new(&bytes).unwrap();
    let before_position = decoder.byte_position();
    let before_marker = decoder.marker();
    let mut cases = 0;
    for signal in 0..=u8::MAX {
        for index in [0, 1, usize::MAX] {
            for context_count in [0, 37, usize::MAX] {
                let actual = control_malformed(probability_signal_error(
                    &decoder,
                    index,
                    context_count,
                    signal,
                ));
                let expected = if signal == RASTER_SIGNAL_CONTEXT {
                    MalformedError::InvalidGenericRegion(GenericRegionErrorKind::ContextIndex {
                        index,
                        count: context_count,
                    })
                } else {
                    match decoder.signal_error(signal).into_decode() {
                        DecodeError::Malformed(error) => error,
                        other => panic!("unexpected arithmetic diagnostic: {other:?}"),
                    }
                };
                assert_eq!(actual, expected);
                assert_eq!(decoder.byte_position(), before_position);
                assert_eq!(decoder.marker(), before_marker);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 2304);
}

#[test]
fn diagnostic_variants_preserve_fields_sources_and_formatter_failures() {
    use std::fmt::Write as _;

    struct RejectWrites;
    impl fmt::Write for RejectWrites {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    for (error, message, delegated) in [
        (
            GenericRegionError::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow)),
            "malformed JBIG2 input: input-derived integer overflow".to_owned(),
            true,
        ),
        (
            GenericRegionError::ReservedFlags(u8::MAX),
            "reserved generic-region flags are set: 0xff".to_owned(),
            false,
        ),
        (
            GenericRegionError::InvalidFlags(u8::MAX),
            "invalid generic-region flag combination: 0xff".to_owned(),
            false,
        ),
        (
            GenericRegionError::InvalidAdaptivePixel {
                number: u8::MAX,
                x: i32::MIN,
                y: i32::MAX,
            },
            "adaptive pixel 255 at (-2147483648, 2147483647) is not an earlier target pixel"
                .to_owned(),
            false,
        ),
        (
            GenericRegionError::InvalidSkipDimensions,
            "generic-region skip bitmap has the wrong dimensions".to_owned(),
            false,
        ),
        (
            GenericRegionError::InvalidArithmeticTerminator,
            "arithmetic region does not end in FF AC".to_owned(),
            false,
        ),
        (
            GenericRegionError::UnexpectedArithmeticMarker {
                offset: usize::MAX,
                code: 0xfe,
            },
            format!("unexpected arithmetic marker FF FE at byte {}", usize::MAX),
            false,
        ),
        (
            GenericRegionError::RowCountMismatch {
                expected: u32::MAX,
                actual: 0,
            },
            "terminal row count 0 differs from decoded row count 4294967295".to_owned(),
            false,
        ),
        (
            GenericRegionError::ContextIndex {
                index: usize::MAX,
                count: 0,
            },
            format!(
                "context index {} is outside a statistics set of length 0",
                usize::MAX
            ),
            false,
        ),
    ] {
        assert_eq!(error.to_string(), message);
        let source = std::error::Error::source(&error);
        assert_eq!(source.is_some(), delegated);
        if let Some(source) = source {
            assert!(matches!(
                source.downcast_ref::<DecodeError>(),
                Some(DecodeError::Malformed(MalformedError::IntegerOverflow)),
            ));
        }
        assert!(write!(&mut RejectWrites, "{error}").is_err());
    }
    let private_message = "private diagnostic message must not escape";
    let io = std::io::Error::new(std::io::ErrorKind::TimedOut, private_message);
    let error = GenericRegionError::from(DecodeError::from(io));
    assert_eq!(error.to_string(), "input/output error: TimedOut");
    assert!(!error.to_string().contains(private_message));
    assert!(matches!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<DecodeError>(),
        Some(DecodeError::Io(_)),
    ));
}
