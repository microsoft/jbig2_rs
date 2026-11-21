#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn zero_plane_rendering_preserves_geometry_memory_work_and_configuration_guards() {
    let header = HalftoneHeader {
        mmr: false,
        template: 0,
        enable_skip: false,
        operator: CombinationOperator::Or,
        default_pixel: false,
        grid_width: 3,
        grid_height: 1,
        grid_x: 256,
        grid_y: 0,
        grid_vector_x: 512,
        grid_vector_y: 0,
    };
    let patterns = [bitmap(&["1"])];
    let limits = Limits {
        max_width: 8,
        max_decoder_managed_memory_bytes: 14,
        max_work_units: 6,
        ..Limits::default()
    };
    let mut state = new_zero_plane_state(&[]).unwrap();
    let mut work = WorkBudget::new(&limits);
    let output =
        decode_halftone_region(&header, 8, 1, &patterns, &mut state, &limits, &mut work).unwrap();
    assert_eq!(output.bytes(), &[0x54]);
    assert_eq!(work.used(), 6);
    assert!(matches!(state, GenericRegionState::ZeroPlanes));
    for (limited, kind, actual) in [
        (
            Limits {
                max_width: 7,
                ..limits
            },
            LimitKind::Width,
            8,
        ),
        (
            Limits {
                max_decoder_managed_memory_bytes: 13,
                ..limits
            },
            LimitKind::Memory,
            14,
        ),
        (
            Limits {
                max_work_units: 5,
                ..limits
            },
            LimitKind::Work,
            6,
        ),
    ] {
        let mut work = WorkBudget::new(&limited);
        assert!(matches!(
            decode_halftone_region(&header, 8, 1, &patterns, &mut state, &limited, &mut work),
            Err(HalftoneError::Decode(DecodeError::LimitExceeded(LimitExceeded {
                kind: observed, actual: observed_actual, ..
            }))) if observed == kind && observed_actual == actual
        ));
        assert!(matches!(state, GenericRegionState::ZeroPlanes));
        assert_eq!(work.used(), 0);
    }
    for invalid in [
        HalftoneHeader {
            template: 4,
            ..header
        },
        HalftoneHeader {
            mmr: true,
            enable_skip: true,
            ..header
        },
    ] {
        let mut work = WorkBudget::new(&limits);
        assert!(matches!(
            decode_halftone_region(&invalid, 8, 1, &patterns, &mut state, &limits, &mut work),
            Err(HalftoneError::InvalidHalftoneConfiguration)
        ));
        assert_eq!(work.used(), 0);
    }
}

#[test]
fn raw_gray_shape_guards_and_skip_rectangles_use_exact_half_open_geometry() {
    for (width, height) in [(8, 2), (9, 1)] {
        let mut current = bitmap(&["101010101", "010101010"]);
        let previous = Bitmap::blank(width, height, &Limits::default()).unwrap();
        let before = current.bytes().to_vec();
        assert!(matches!(
            xor_gray_plane_bytes(&mut current, &previous),
            Err(HalftoneError::Decode(DecodeError::Malformed(
                MalformedError::IntegerOverflow
            )))
        ));
        assert_eq!(current.bytes(), before);
    }
    for x in [-10_i64, -2, -1, 0, 8, 9, 10] {
        for y in [-10_i64, -2, -1, 0, 2, 3, 4] {
            for width in [1_u32, 2, 9] {
                for height in [1_u32, 2, 3] {
                    let right = x + i64::from(width);
                    let bottom = y + i64::from(height);
                    let overlaps = x < 9 && right > 0 && y < 3 && bottom > 0;
                    assert_eq!(
                        half_open_outside(x, y, right, bottom, width, height, 9, 3),
                        !overlaps
                    );
                }
            }
        }
    }
}

#[test]
fn raw_halftone_memory_peaks_match_wide_stage_arithmetic() {
    let patterns = [bitmap(&["101010101", "010101010"])];
    for (grid_width, grid_height) in [
        (0, 0),
        (1, 1),
        (9, 3),
        (u32::MAX, 1_000_000_000),
        (u32::MAX, u32::MAX),
    ] {
        for (width, height) in [(0, 0), (9, 3), (u32::MAX, u32::MAX)] {
            for enable_skip in [false, true] {
                for depth in [0, 1, 2, u8::MAX] {
                    let header = HalftoneHeader {
                        mmr: false,
                        template: 0,
                        enable_skip,
                        operator: CombinationOperator::Or,
                        default_pixel: false,
                        grid_width,
                        grid_height,
                        grid_x: 0,
                        grid_y: 0,
                        grid_vector_x: 256,
                        grid_vector_y: 0,
                    };
                    let cells = u128::from(grid_width) * u128::from(grid_height);
                    let plane = u128::from(grid_width).div_ceil(8) * u128::from(grid_height);
                    let skip = if enable_skip { plane } else { 0 };
                    let active = u128::from(depth.min(2));
                    let output = u128::from(width).div_ceil(8) * u128::from(height);
                    let fixed = 4 + 4 * cells + skip;
                    let peak = (fixed + active * plane).max(fixed + output);
                    let representable = u64::try_from(peak).unwrap_or(u64::MAX);
                    for limit in [0, representable.saturating_sub(1), representable, u64::MAX] {
                        let limits = Limits {
                            max_decoder_managed_memory_bytes: limit,
                            ..Limits::default()
                        };
                        let result = enforce_halftone_memory(
                            width, height, &header, &patterns, depth, &limits,
                        );
                        if peak > u128::from(u64::MAX) {
                            assert!(matches!(
                                result,
                                Err(HalftoneError::Decode(DecodeError::Malformed(
                                    MalformedError::IntegerOverflow
                                )))
                            ));
                        } else if peak > u128::from(limit) {
                            assert!(matches!(result, Err(HalftoneError::Decode(
                                DecodeError::LimitExceeded(LimitExceeded {
                                    kind: LimitKind::Memory, limit: actual_limit, actual,
                                })
                            )) if actual_limit == limit && u128::from(actual) == peak));
                        } else {
                            result.unwrap();
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn raw_pattern_and_bitmap_preflights_preserve_exact_fields_and_priority() {
    for width in [0, 1, u8::MAX] {
        for height in [0, 1, u8::MAX] {
            for template in 0..=u8::MAX {
                for mmr in [false, true] {
                    let header = PatternDictionaryHeader {
                        mmr,
                        template,
                        pattern_width: width,
                        pattern_height: height,
                        gray_max: u32::MAX,
                    };
                    let result = validate_pattern_header(header);
                    if width == 0 || height == 0 {
                        assert!(matches!(result,
                            Err(HalftoneError::InvalidPatternDimensions { width: w, height: h })
                                if w == width && h == height));
                    } else if template > 3 || (mmr && template != 0) {
                        let encoded =
                            u8::try_from((u16::from(template) * 2) & 255).unwrap() | u8::from(mmr);
                        assert!(matches!(result,
                            Err(HalftoneError::InvalidPatternDictionaryFlags(actual))
                                if actual == encoded));
                    } else {
                        result.unwrap();
                    }
                }
            }
        }
    }
    let limits = Limits::default();
    for (width, height) in [(0, 0), (0, 1), (1, 0), (1, 1), (9, 2)] {
        let bitmap = Bitmap::blank(width, height, &limits).unwrap();
        for expected_width in [0, 1, 9, u32::MAX] {
            for expected_height in [0, 1, 2, u32::MAX] {
                let result = require_bitmap_dimensions(&bitmap, expected_width, expected_height);
                if (width, height) == (expected_width, expected_height) {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(result, Err(HalftoneError::InvalidBitmapDimensions {
                        expected_width: ew, expected_height: eh,
                        actual_width: aw, actual_height: ah,
                    }) if (ew, eh, aw, ah) == (expected_width, expected_height, width, height))
                    );
                }
            }
        }
    }
}

#[test]
fn raw_collective_slicing_matches_literal_pixels_and_exact_budget_boundaries() {
    let generous = Limits::default();
    for count in [1_u32, 2, 3, 8] {
        for width in [1_u8, 7, 8, 9] {
            let total_width = count * u32::from(width);
            let mut collective = Bitmap::blank(total_width, 2, &generous).unwrap();
            for y in 0..2 {
                for x in 0..total_width {
                    collective.set_pixel(x, y, (x + y * 3) % 5 < 2).unwrap();
                }
            }
            let before = collective.bytes().to_vec();
            let header = PatternDictionaryHeader {
                mmr: false,
                template: 0,
                pattern_width: width,
                pattern_height: 2,
                gray_max: count - 1,
            };
            let memory = u64::from(total_width).div_ceil(8) * 2
                + u64::from(count)
                    * (u64::from(width).div_ceil(8) * 2
                        + u64::try_from(core::mem::size_of::<Bitmap>()).unwrap());
            let pixels = u64::from(total_width) * 2;
            for memory_limit in [memory - 1, memory] {
                for work_limit in [pixels - 1, pixels] {
                    let limits = Limits {
                        max_symbols: count,
                        max_decoder_managed_memory_bytes: memory_limit,
                        max_work_units: work_limit,
                        ..generous
                    };
                    let mut work = WorkBudget::new(&limits);
                    let result = slice_collective_bitmap(&collective, header, &limits, &mut work);
                    if memory_limit < memory {
                        assert!(matches!(result, Err(HalftoneError::Decode(
                            DecodeError::LimitExceeded(LimitExceeded {
                                kind: LimitKind::Memory, limit, actual,
                            })
                        )) if limit == memory_limit && actual == memory));
                        assert_eq!(work.used(), 0);
                    } else if work_limit < pixels {
                        assert!(matches!(result, Err(HalftoneError::Decode(
                            DecodeError::LimitExceeded(LimitExceeded {
                                kind: LimitKind::Work, limit, actual,
                            })
                        )) if limit == work_limit && actual == pixels));
                        assert_eq!(work.used(), 0);
                    } else {
                        let patterns = result.unwrap();
                        assert_eq!(patterns.len(), usize::try_from(count).unwrap());
                        for (index, pattern) in patterns.iter().enumerate() {
                            let mut expected = vec![0_u8; usize::from(width).div_ceil(8) * 2];
                            for y in 0..2_usize {
                                for x in 0..usize::from(width) {
                                    if (index * usize::from(width) + x + y * 3) % 5 < 2 {
                                        expected[y * usize::from(width).div_ceil(8) + x / 8] |=
                                            0x80 >> (x % 8);
                                    }
                                }
                            }
                            assert_eq!(pattern.bytes(), expected);
                        }
                        assert_eq!(work.used(), pixels);
                    }
                    assert_eq!(collective.bytes(), before);
                }
            }
        }
    }
}

#[test]
fn raw_render_extreme_coordinates_preserve_storage_on_checked_overflow() {
    let pattern = bitmap(&["101", "010"]);
    for x in [i64::MIN, -3, -1, 0, 8, i64::MAX - 2, i64::MAX] {
        for y in [i64::MIN, -2, -1, 0, 2, i64::MAX - 1, i64::MAX] {
            for operator in [
                CombinationOperator::Or,
                CombinationOperator::And,
                CombinationOperator::Xor,
                CombinationOperator::Xnor,
                CombinationOperator::Replace,
            ] {
                let mut destination = bitmap(&["101010101", "010101010", "111000111"]);
                let before = destination.bytes().to_vec();
                let result = render_clipped_pattern(&mut destination, &pattern, x, y, operator);
                let overflows = i128::from(x) + 3 > i128::from(i64::MAX)
                    || i128::from(y) + 2 > i128::from(i64::MAX);
                if overflows {
                    assert!(matches!(
                        result,
                        Err(HalftoneError::Decode(DecodeError::Malformed(
                            MalformedError::IntegerOverflow
                        )))
                    ));
                    assert_eq!(destination.bytes(), before);
                } else {
                    result.unwrap();
                    let mut expected = bitmap(&["101010101", "010101010", "111000111"]);
                    scalar_render_clipped_pattern_oracle(&mut expected, &pattern, x, y, operator)
                        .unwrap();
                    assert_eq!(destination.bytes(), expected.bytes());
                }
            }
        }
    }
}

#[test]
fn bytewise_gray_xor_matches_scalar_pixels_randomized() {
    let mut state = 0x6a18_ef03_91c5_d247;
    for width in [1, 2, 7, 8, 9, 15, 31, 32, 33, 63, 127] {
        for height in [1, 2, 5, 17] {
            let current = random_bitmap(width, height, &mut state);
            let previous = random_bitmap(width, height, &mut state);
            let mut expected = current.clone();
            let mut actual = current;
            scalar_xor_gray_plane_oracle(&mut expected, &previous).expect("scalar gray conversion");
            xor_gray_plane_bytes(&mut actual, &previous).expect("bytewise gray conversion");
            assert_eq!(actual, expected, "dimensions {width}x{height}");
        }
    }
}

#[test]
fn fixed_gray_extraction_matches_scalar_oracle() {
    let mut state = 0x39d2_a175_64fe_08bc;
    for plane_index in [0, 1, 7, 15, 23, 31] {
        for byte in 0..=u8::MAX {
            let mut expected = vec![0_u32; 24];
            for value in &mut expected {
                *value = next_random(&mut state);
            }
            let mut actual = expected.clone();
            scalar_accumulate_gray_byte_oracle(&mut expected, 8, byte, plane_index);
            accumulate_gray_byte(&mut actual, 8, byte, plane_index).expect("fixed extraction");
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn skip_mask_obeys_half_open_boundary_rules() {
    let header = HalftoneHeader {
        mmr: false,
        template: 0,
        enable_skip: true,
        operator: CombinationOperator::Or,
        default_pixel: false,
        grid_width: 4,
        grid_height: 1,
        grid_x: -3 * 256,
        grid_y: 0,
        grid_vector_x: 256,
        grid_vector_y: 0,
    };
    let mut work = WorkBudget::new(&generous_limits());
    let skip = generate_skip_mask(&header, 10, 8, 3, 2, &generous_limits(), &mut work)
        .expect("skip mask should fit");
    assert_eq!(
        (0..4).map(|x| skip.pixel(x, 0)).collect::<Vec<_>>(),
        [Some(true), Some(false), Some(false), Some(false)]
    );
}

#[test]
fn clipped_replace_rendering_and_order_are_deterministic() {
    let limits = generous_limits();
    let patterns = [bitmap(&["101", "011"]), bitmap(&["010", "110"])];
    let header = HalftoneHeader {
        mmr: false,
        template: 0,
        enable_skip: false,
        operator: CombinationOperator::Replace,
        default_pixel: false,
        grid_width: 2,
        grid_height: 1,
        grid_x: -256,
        grid_y: 0,
        grid_vector_x: 512,
        grid_vector_y: 0,
    };
    let mut planes = VecDeque::from([bitmap(&["01"])]);
    let mut adapter =
        |_request: GenericRegionRequest<'_>| Ok(planes.pop_front().expect("one plane is supplied"));
    let mut decoder = GenericRegionState::scripted(&mut adapter);
    let mut work = WorkBudget::new(&limits);
    let output = decode_halftone_region(&header, 4, 2, &patterns, &mut decoder, &limits, &mut work)
        .expect("halftone should render");
    assert_eq!(output, bitmap(&["0010", "1110"]));
}

#[test]
fn clipped_pattern_rendering_exhaustively_matches_scalar_oracle() {
    let limits = generous_limits();
    let operators = [
        CombinationOperator::Or,
        CombinationOperator::And,
        CombinationOperator::Xor,
        CombinationOperator::Xnor,
        CombinationOperator::Replace,
    ];
    for pattern_width in 1..=10 {
        for destination_width in 1..=10 {
            let pattern = Bitmap::filled(pattern_width, 3, true, &limits).expect("pattern bitmap");
            let original =
                Bitmap::filled(destination_width, 4, false, &limits).expect("destination");
            for x in -12..=12 {
                for y in -4..=4 {
                    for operator in operators {
                        let mut expected = original.clone();
                        let mut actual = original.clone();
                        scalar_render_clipped_pattern_oracle(
                            &mut expected,
                            &pattern,
                            x,
                            y,
                            operator,
                        )
                        .expect("scalar render");
                        render_clipped_pattern(&mut actual, &pattern, x, y, operator)
                            .expect("chunk render");
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }
}
