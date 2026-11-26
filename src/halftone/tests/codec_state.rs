#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn owned_arithmetic_planes_match_checked_banks_and_complete_state_across_reuse() {
    let mut coded = [0; 130];
    coded[128..].copy_from_slice(&[0xff, 0xac]);
    let skip = Bitmap::blank(9, 3, &Limits::default()).unwrap();
    for (code, template) in [
        (0, GenericTemplate::Template0),
        (1, GenericTemplate::Template1),
        (2, GenericTemplate::Template2),
        (3, GenericTemplate::Template3),
    ] {
        for max_work_units in [32, Limits::default().max_work_units] {
            let limits = Limits {
                max_work_units,
                ..Limits::default()
            };
            let mut state = new_halftone_arithmetic_state(&coded, template, code, &limits).unwrap();
            let mut checked =
                GenericArithmeticSession::new(&coded, template, false, &limits).unwrap();
            for (width, prediction, skip) in [
                (1, false, None),
                (9, false, Some(&skip)),
                (17, true, None),
                (7, false, None),
            ] {
                let pixels = gray_adaptive_pixels(code);
                let mut adaptive = [TemplateOffset::new(0, 0); 12];
                for (offset, (x, y)) in adaptive.iter_mut().zip(pixels) {
                    *offset = TemplateOffset::new(x, y);
                }
                let expected = crate::generic_region::decode_generic_on(
                    &mut checked.decoder,
                    &mut checked.contexts,
                    GenericArithmeticCall {
                        width,
                        height: 3,
                        adaptive,
                        typical_prediction: prediction,
                        skip,
                    },
                    3,
                    &limits,
                    &mut checked.work_used,
                )
                .map_err(generic_error_into_decode);
                let actual = state.decode_generic_region(GenericRegionRequest {
                    width,
                    height: 3,
                    coding: GenericRegionCoding::Arithmetic {
                        template: code,
                        adaptive_pixels: pixels,
                    },
                    typical_prediction: prediction,
                    extended_template: false,
                    skip,
                });
                match (actual, expected) {
                    (Ok(actual), Ok(expected)) => {
                        assert_eq!((actual.width(), actual.height()), (width, 3));
                        assert_eq!(actual.bytes(), expected.bytes());
                    }
                    (Err(actual), Err(expected)) => assert_decode_error_exact(actual, expected),
                    (actual, expected) => {
                        panic!("owned/checked result differs: {actual:?}/{expected:?}")
                    }
                }
                let GenericRegionState::Arithmetic { session, .. } = &state else {
                    panic!("arithmetic owner changed kind");
                };
                assert_eq!(session.contexts.contexts, checked.contexts.contexts);
                assert_eq!(session.work_used, checked.work_used);
                assert_eq!(
                    format!("{:?}", session.decoder),
                    format!("{:?}", checked.decoder)
                );
            }
        }
    }
}

#[test]
fn zero_plane_state_rejects_coded_requests_and_trailing_payload() {
    let mut state = new_zero_plane_state(&[]).unwrap();
    state.finish().unwrap();
    let error = state
        .decode_generic_region(GenericRegionRequest {
            width: 1,
            height: 1,
            coding: GenericRegionCoding::Mmr { require_eofb: true },
            typical_prediction: false,
            extended_template: false,
            skip: None,
        })
        .unwrap_err();
    assert!(matches!(
        error,
        DecodeError::Malformed(MalformedError::InvalidPageState(
            "zero-plane halftone requested a coded bitmap"
        ))
    ));
    assert!(matches!(state, GenericRegionState::ZeroPlanes));
    for coded in [&[0][..], &[0xff, 0xac]] {
        let Err(error) = new_zero_plane_state(coded) else {
            panic!("zero-plane trailing bytes accepted")
        };
        assert_decode_error_exact(
            error,
            DecodeError::Malformed(MalformedError::TrailingData(
                u64::try_from(coded.len()).unwrap(),
            )),
        );
    }
}

#[test]
fn arithmetic_state_constructors_enforce_exact_input_and_context_budgets() {
    let coded = [0, 0, 0xff, 0xac];
    for (code, template, bits) in [
        (0, GenericTemplate::Template0, 16),
        (1, GenericTemplate::Template1, 13),
        (2, GenericTemplate::Template2, 10),
        (3, GenericTemplate::Template3, 10),
    ] {
        let bytes = 2_u64 << bits;
        let limits = Limits {
            max_stream_bytes: 4,
            max_decoder_managed_memory_bytes: bytes,
            ..Limits::default()
        };
        for pattern in [false, true] {
            let create = |data, limits: &Limits| {
                if pattern {
                    new_pattern_arithmetic_state(data, template, code, limits)
                } else {
                    new_halftone_arithmetic_state(data, template, code, limits)
                }
            };
            assert!(create(&coded, &limits).is_ok());
            for data in [&[][..], &[0][..], &[0, 0][..], &[0xff, 0x90][..]] {
                let Err(error) = create(data, &limits) else {
                    panic!("missing terminator accepted")
                };
                assert_decode_error_exact(
                    error,
                    DecodeError::Malformed(MalformedError::InvalidGenericRegion(
                        GenericRegionErrorKind::InvalidArithmeticTerminator,
                    )),
                );
            }
            let Err(error) = create(
                &coded,
                &Limits {
                    max_decoder_managed_memory_bytes: bytes - 1,
                    ..limits
                },
            ) else {
                panic!("context budget exceeded")
            };
            assert_decode_error_exact(
                error,
                DecodeError::LimitExceeded(LimitExceeded {
                    kind: LimitKind::Memory,
                    limit: bytes - 1,
                    actual: bytes,
                }),
            );
            let Err(error) = create(
                &coded,
                &Limits {
                    max_stream_bytes: 3,
                    max_decoder_managed_memory_bytes: 0,
                    ..limits
                },
            ) else {
                panic!("input budget exceeded")
            };
            assert_decode_error_exact(
                error,
                DecodeError::LimitExceeded(LimitExceeded {
                    kind: LimitKind::StreamBytes,
                    limit: 3,
                    actual: 4,
                }),
            );
        }
    }
}

#[test]
fn arithmetic_state_rejects_mode_and_template_changes_without_consumption() {
    let coded = [0, 0, 0xff, 0xac];
    for (code, template) in [
        (0, GenericTemplate::Template0),
        (1, GenericTemplate::Template1),
        (2, GenericTemplate::Template2),
        (3, GenericTemplate::Template3),
    ] {
        for pattern in [false, true] {
            let mut state = if pattern {
                new_pattern_arithmetic_state(&coded, template, code, &Limits::default()).unwrap()
            } else {
                new_halftone_arithmetic_state(&coded, template, code, &Limits::default()).unwrap()
            };
            for (coding, reason) in [
                (
                    GenericRegionCoding::Mmr {
                        require_eofb: false,
                    },
                    if pattern {
                        "pattern dictionary coding mode changed"
                    } else {
                        "halftone coding mode changed"
                    },
                ),
                (
                    GenericRegionCoding::Arithmetic {
                        template: (code + 1) % 4,
                        adaptive_pixels: [(0, 0); 4],
                    },
                    if pattern {
                        "pattern dictionary template changed"
                    } else {
                        "halftone template changed"
                    },
                ),
            ] {
                let Err(error) = state.decode_generic_region(empty_request(coding)) else {
                    panic!("changed coding accepted")
                };
                assert_decode_error_exact(
                    error,
                    DecodeError::Malformed(MalformedError::InvalidPageState(reason)),
                );
                let GenericRegionState::Arithmetic { session, .. } = &state else {
                    panic!("state variant changed")
                };
                assert_eq!(session.decoder.byte_position(), 1);
                assert_eq!(session.decoder.marker(), None);
                assert_eq!(session.work_used, 0);
            }
            let image = state
                .decode_generic_region(empty_request(GenericRegionCoding::Arithmetic {
                    template: code,
                    adaptive_pixels: gray_adaptive_pixels(code),
                }))
                .expect("zero-area helper bitmap");
            assert_eq!((image.width(), image.height(), image.data.len()), (0, 0, 0));
            let Err(error) = state.decode_generic_region(GenericRegionRequest {
                width: 1,
                height: 1,
                coding: GenericRegionCoding::Arithmetic {
                    template: code,
                    adaptive_pixels: [(0, 0); 4],
                },
                ..empty_request(GenericRegionCoding::Arithmetic {
                    template: code,
                    adaptive_pixels: [(0, 0); 4],
                })
            }) else {
                panic!("noncausal adaptive pixel accepted")
            };
            assert_decode_error_exact(
                error,
                DecodeError::Malformed(MalformedError::InvalidGenericRegion(
                    GenericRegionErrorKind::InvalidAdaptivePixel {
                        number: 1,
                        x: 0,
                        y: 0,
                    },
                )),
            );
            state.finish().expect("no wrong marker was observed");
        }
    }
}

#[test]
fn arithmetic_finalization_rejects_mismatched_extent_without_changing_the_session() {
    let coded = [0xff, 0xac];
    let limits = Limits::default();
    let mut scratch = Vec::new();
    let pat_state = new_pattern_arithmetic_state_from_scratch(
        &coded,
        GenericTemplate::Template3,
        3,
        &limits,
        &mut scratch,
    )
    .unwrap();
    pat_state.into_scratch(&mut scratch);
    let _ = new_halftone_state(&[], false, GenericTemplate::Template3, 3, 1, &limits).unwrap();
    let _ = new_halftone_state(&coded, true, GenericTemplate::Template3, 3, 2, &limits).unwrap();
    let _ = new_halftone_state(&coded, false, GenericTemplate::Template3, 3, 2, &limits).unwrap();
    for pattern in [false, true] {
        let mut state = if pattern {
            new_pattern_arithmetic_state(&coded, GenericTemplate::Template3, 3, &limits).unwrap()
        } else {
            new_halftone_arithmetic_state(&coded, GenericTemplate::Template3, 3, &limits).unwrap()
        };
        for declared_length in [0, 1, 3, usize::MAX, coded.len()] {
            let GenericRegionState::Arithmetic { session, .. } = &mut state else {
                panic!("unexpected coding mode")
            };
            // The decoder actually observed the marker; only its enclosing extent differs.
            let marker = session.decoder.marker().expect("initial FF AC marker");
            assert_eq!((marker.offset, marker.code), (0, 0xac));
            session.coded_length = declared_length;
            let before = format!("{session:?}");
            let result = state.finish();
            if declared_length == coded.len() {
                result.unwrap();
            } else {
                let expected = if declared_length < 2 {
                    GenericRegionErrorKind::InvalidArithmeticTerminator
                } else {
                    GenericRegionErrorKind::UnexpectedArithmeticMarker {
                        offset: 0,
                        code: 0xac,
                    }
                };
                assert_decode_error_exact(
                    result.unwrap_err(),
                    DecodeError::Malformed(MalformedError::InvalidGenericRegion(expected)),
                );
            }
            let GenericRegionState::Arithmetic { session, .. } = &state else {
                panic!("coding mode changed")
            };
            assert_eq!(format!("{session:?}"), before);
        }
    }
}

#[test]
fn pattern_mmr_is_single_use_and_mode_errors_do_not_consume_it() {
    let limits = Limits::default();
    let correct = empty_request(GenericRegionCoding::Mmr {
        require_eofb: false,
    });
    let mut state = new_pattern_mmr_state(&[], &limits);
    let Err(error) = state.decode_generic_region(empty_request(GenericRegionCoding::Mmr {
        require_eofb: true,
    })) else {
        panic!("changed mode accepted")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::InvalidPageState(
            "pattern dictionary coding mode changed",
        )),
    );
    let image = state
        .decode_generic_region(correct)
        .expect("empty helper bitmap");
    assert_eq!((image.width(), image.height(), image.data.len()), (0, 0, 0));
    let Err(error) = state.decode_generic_region(correct) else {
        panic!("reused MMR bitmap accepted")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::InvalidPageState(
            "pattern dictionary requested multiple MMR bitmaps",
        )),
    );
    state.finish().unwrap();
    let mut state = new_pattern_mmr_state(&[], &limits);
    let Err(error) = state.decode_generic_region(GenericRegionRequest {
        width: 1,
        height: 1,
        ..correct
    }) else {
        panic!("missing mode accepted")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::UnexpectedEof {
            offset: 0,
            needed: 1,
            remaining: 0,
        }),
    );
    let Err(error) = state.decode_generic_region(correct) else {
        panic!("failed state reused")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::InvalidPageState(
            "pattern dictionary requested multiple MMR bitmaps",
        )),
    );
}

#[test]
fn gray_mmr_consumes_exact_prefixes_and_requires_all_declared_bytes() {
    let coded = [0, 0x10, 1, 0, 0x10, 1];
    let mut state = new_gray_mmr_state(&coded, &Limits::default());
    assert_decode_error_exact(
        state.finish().unwrap_err(),
        DecodeError::Malformed(MalformedError::TrailingData(6)),
    );
    let Err(error) = state.decode_generic_region(empty_request(GenericRegionCoding::Mmr {
        require_eofb: false,
    })) else {
        panic!("optional EOFB accepted for gray plane")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::InvalidPageState(
            "halftone coding mode changed",
        )),
    );
    let request = empty_request(GenericRegionCoding::Mmr { require_eofb: true });
    for expected_offset in [3, 6] {
        let bitmap = state
            .decode_generic_region(request)
            .expect("exact EOFB prefix");
        assert_eq!((bitmap.width(), bitmap.height()), (0, 0));
        let GenericRegionState::GrayMmr { offset, .. } = &state else {
            panic!("wrong state")
        };
        assert_eq!(*offset, expected_offset);
        if expected_offset == 3 {
            assert_decode_error_exact(
                state.finish().unwrap_err(),
                DecodeError::Malformed(MalformedError::TrailingData(3)),
            );
        }
    }
    state.finish().unwrap();
    let Err(error) = state.decode_generic_region(request) else {
        panic!("missing EOFB accepted")
    };
    assert_decode_error_exact(
        error,
        DecodeError::Malformed(MalformedError::InvalidMmr(MmrErrorKind::MissingEofb)),
    );
    let GenericRegionState::GrayMmr { offset, .. } = state else {
        panic!("wrong state")
    };
    assert_eq!(offset, 6);
}

#[test]
#[allow(clippy::too_many_lines)]
fn header_flags_fields_and_every_truncation_keep_exact_cursor_and_errors() {
    for flags in 0..=u8::MAX {
        let pattern = [flags, 1, 2, 0xff, 0xff, 0xff, 0xff];
        let mut reader = ByteReader::new(&pattern);
        if flags >= 8 || flags & 1 != 0 && flags & 6 != 0 {
            assert!(matches!(parse_pattern_dictionary_header(&mut reader),
                Err(HalftoneError::InvalidPatternDictionaryFlags(actual)) if actual == flags));
            assert_eq!(reader.position(), 1);
        } else {
            let result = parse_pattern_dictionary_header(&mut reader).unwrap();
            assert_eq!(
                (
                    result.mmr,
                    result.template,
                    result.pattern_width,
                    result.pattern_height
                ),
                (flags & 1 != 0, flags >> 1, 1, 2)
            );
            assert_eq!(result.pattern_count(), u64::from(u32::MAX) + 1);
            assert_eq!(reader.position(), 7);
        }
        let mut bytes = vec![flags];
        for value in [u32::MAX, 0, 0x8000_0000, 0x7fff_ffff] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.extend_from_slice(&[0xff, 0xff, 0, 0]);
        let mut reader = ByteReader::new(&bytes);
        let result = parse_halftone_header(&mut reader);
        let operator = (flags >> 4) & 7;
        if flags & 1 != 0 && flags & 14 != 0 {
            assert!(
                matches!(result, Err(HalftoneError::InvalidHalftoneFlags(actual)) if actual == flags)
            );
            assert_eq!(reader.position(), 1);
        } else if operator > 4 {
            assert!(
                matches!(result, Err(HalftoneError::Decode(DecodeError::Malformed(
                MalformedError::InvalidCombinationOperator(actual)))) if actual == operator)
            );
            assert_eq!(reader.position(), 1);
        } else {
            let result = result.unwrap();
            assert_eq!(
                (
                    result.grid_width,
                    result.grid_height,
                    result.grid_x,
                    result.grid_y
                ),
                (u32::MAX, 0, i32::MIN, i32::MAX)
            );
            assert_eq!((result.grid_vector_x, result.grid_vector_y), (u16::MAX, 0));
            assert_eq!(
                result.operator,
                [
                    CombinationOperator::Or,
                    CombinationOperator::And,
                    CombinationOperator::Xor,
                    CombinationOperator::Xnor,
                    CombinationOperator::Replace
                ][usize::from(operator)]
            );
            assert_eq!(
                (
                    result.mmr,
                    result.template,
                    result.enable_skip,
                    result.default_pixel
                ),
                (
                    flags & 1 != 0,
                    (flags >> 1) & 3,
                    flags & 8 != 0,
                    flags & 0x80 != 0
                )
            );
            assert_eq!(reader.position(), 21);
        }
    }
    for (bytes, lengths) in [
        (vec![0, 1, 2, 0, 0, 0, 0], vec![1, 1, 1, 4]),
        (vec![0; 21], vec![1, 4, 4, 4, 4, 2, 2]),
    ] {
        for cut in 0..bytes.len() {
            let mut offset = 0;
            let needed = *lengths
                .iter()
                .find(|&&length| {
                    if offset + length > cut {
                        true
                    } else {
                        offset += length;
                        false
                    }
                })
                .unwrap();
            let mut reader = ByteReader::new(&bytes[..cut]);
            let error = if bytes.len() == 7 {
                parse_pattern_dictionary_header(&mut reader).unwrap_err()
            } else {
                parse_halftone_header(&mut reader).unwrap_err()
            };
            assert!(
                matches!(error, HalftoneError::Decode(DecodeError::Malformed(
                MalformedError::UnexpectedEof { offset: actual, needed: actual_needed, remaining }
            )) if actual == offset as u64 && actual_needed == needed as u64 && remaining == (cut - offset) as u64)
            );
            assert_eq!(reader.position(), offset);
        }
    }
}
