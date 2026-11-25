#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn parses_pattern_header_and_rejects_invalid_flags_and_sizes() {
    let mut reader = ByteReader::new(&[0, 3, 2, 0, 0, 0, 1]);
    let header = parse_pattern_dictionary_header(&mut reader).expect("valid header should parse");
    assert_eq!(header.pattern_width, 3);
    assert_eq!(header.pattern_height, 2);
    assert_eq!(header.pattern_count(), 2);
    assert!(matches!(
        parse_pattern_dictionary_header(&mut ByteReader::new(&[3, 1, 1, 0, 0, 0, 0])),
        Err(HalftoneError::InvalidPatternDictionaryFlags(3))
    ));
    assert!(matches!(
        parse_pattern_dictionary_header(&mut ByteReader::new(&[0, 0, 1, 0, 0, 0, 0])),
        Err(HalftoneError::InvalidPatternDimensions {
            width: 0,
            height: 1
        })
    ));
}

#[test]
fn parses_halftone_header_and_signed_offsets() {
    let bytes = [
        0xca, 0, 0, 0, 2, 0, 0, 0, 3, 0xff, 0xff, 0xff, 0xff, 0, 0, 1, 0, 3, 0, 1, 0,
    ];
    let header = parse_halftone_header(&mut ByteReader::new(&bytes))
        .expect("valid halftone header should parse");
    assert!(!header.mmr);
    assert_eq!(header.template, 1);
    assert!(header.enable_skip);
    assert_eq!(header.operator, CombinationOperator::Replace);
    assert!(header.default_pixel);
    assert_eq!(header.grid_x, -1);
    assert!(matches!(
        parse_halftone_header(&mut ByteReader::new(&[
            0x39, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ])),
        Err(HalftoneError::InvalidHalftoneFlags(0x39))
    ));
    assert!(matches!(
        parse_halftone_header(&mut ByteReader::new(&[
            0x50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ])),
        Err(HalftoneError::Decode(DecodeError::Malformed(
            MalformedError::InvalidCombinationOperator(5)
        )))
    ));
}

#[test]
fn slices_collective_bitmap_without_row_aliasing() {
    let collective = bitmap(&["101011", "010110"]);
    let header = PatternDictionaryHeader {
        mmr: false,
        template: 0,
        pattern_width: 3,
        pattern_height: 2,
        gray_max: 1,
    };
    let mut work = WorkBudget::new(&generous_limits());
    let patterns = slice_collective_bitmap(&collective, header, &generous_limits(), &mut work)
        .expect("collective bitmap should slice");
    assert_eq!(patterns, [bitmap(&["101", "010"]), bitmap(&["011", "110"])]);
}

#[test]
fn pattern_decode_supplies_normative_generic_parameters() {
    let header = PatternDictionaryHeader {
        mmr: false,
        template: 0,
        pattern_width: 3,
        pattern_height: 2,
        gray_max: 1,
    };
    let mut calls = 0;
    let mut adapter = |request: GenericRegionRequest<'_>| {
        calls += 1;
        assert_eq!((request.width, request.height), (6, 2));
        assert!(!request.typical_prediction);
        assert!(!request.extended_template);
        assert!(request.skip.is_none());
        assert_eq!(
            request.coding,
            GenericRegionCoding::Arithmetic {
                template: 0,
                adaptive_pixels: [(-3, 0), (-3, -1), (2, -2), (-2, -2)]
            }
        );
        Ok(bitmap(&["101011", "010110"]))
    };
    let mut decoder = GenericRegionState::scripted(&mut adapter);
    let limits = generous_limits();
    let mut work = WorkBudget::new(&limits);
    let patterns = decode_pattern_dictionary(header, &mut decoder, &limits, &mut work)
        .expect("pattern dictionary should decode");
    assert_eq!(calls, 1);
    assert_eq!(patterns.len(), 2);
}

#[test]
fn reconstructs_gray_planes_most_significant_first() {
    let mut planes = VecDeque::from([bitmap(&["1110"]), bitmap(&["0111"]), bitmap(&["0010"])]);
    let mut adapter = |request: GenericRegionRequest<'_>| {
        assert!(matches!(
            request.coding,
            GenericRegionCoding::Arithmetic { template: 0, .. }
        ));
        assert!(!request.typical_prediction);
        assert!(!request.extended_template);
        Ok(planes.pop_front().expect("three planes are supplied"))
    };
    let mut decoder = GenericRegionState::scripted(&mut adapter);
    let mut work = WorkBudget::new(&generous_limits());
    let values = decode_gray_planes(
        4,
        1,
        3,
        false,
        0,
        None,
        &mut decoder,
        &generous_limits(),
        &mut work,
    )
    .expect("planes should reconstruct");
    assert_eq!(values.test_values(), [7, 4, 5, 3]);
    assert!(planes.is_empty());
}

#[test]
fn narrow_and_wide_gray_indices_match_literal_values_at_depth_and_tail_boundaries() {
    let limits = Limits::default();
    for depth in [0_u8, 1, 8, 9, 10, 31, 32] {
        let maximum = u32::try_from((1_u64 << depth) - 1).unwrap();
        let literals = [
            0,
            maximum,
            u32::from(depth != 0),
            maximum / 2 + u32::from(depth != 0),
        ];
        let expected = (0..13)
            .map(|i| literals[i % literals.len()])
            .collect::<Vec<_>>();
        let mut planes = (0..depth)
            .rev()
            .map(|plane| {
                let mut bitmap = Bitmap::blank(13, 1, &limits).unwrap();
                for (x, value) in expected.iter().enumerate() {
                    let gray = value ^ (value >> 1);
                    bitmap
                        .set_pixel(u32::try_from(x).unwrap(), 0, gray & (1_u32 << plane) != 0)
                        .unwrap();
                }
                bitmap
            })
            .collect::<VecDeque<_>>();
        let mut adapter = |_request: GenericRegionRequest<'_>| Ok(planes.pop_front().unwrap());
        let mut decoder = GenericRegionState::scripted(&mut adapter);
        let mut work = WorkBudget::new(&limits);
        let actual = decode_gray_planes(
            13,
            1,
            depth,
            false,
            0,
            None,
            &mut decoder,
            &limits,
            &mut work,
        )
        .unwrap();
        assert_eq!(actual.test_values(), expected);
        assert_eq!(matches!(actual, GrayIndexes::Narrow(_)), depth <= 8);
        assert!(planes.is_empty());
        assert_eq!(work.used(), u64::from(depth) * 13 * 2);
        actual.validate(1_u64 << depth).unwrap();
    }
}

#[test]
fn mmr_gray_plane_requires_eofb() {
    let mut calls = 0;
    let mut adapter = |request: GenericRegionRequest<'_>| {
        calls += 1;
        assert_eq!(
            request.coding,
            GenericRegionCoding::Mmr { require_eofb: true }
        );
        Ok(bitmap(&["0"]))
    };
    let mut decoder = GenericRegionState::scripted(&mut adapter);
    let limits = generous_limits();
    let mut work = WorkBudget::new(&limits);
    let values = decode_gray_planes(1, 1, 1, true, 0, None, &mut decoder, &limits, &mut work)
        .expect("MMR gray plane should decode");
    assert_eq!(calls, 1);
    assert_eq!(values.test_values(), [0]);
}

#[test]
fn one_pattern_uses_no_gray_plane() {
    let mut calls = 0;
    let mut adapter = |_request: GenericRegionRequest<'_>| {
        calls += 1;
        Bitmap::blank(1, 1, &generous_limits())
    };
    let mut decoder = GenericRegionState::scripted(&mut adapter);
    let mut work = WorkBudget::new(&generous_limits());
    let values = decode_gray_planes(
        2,
        2,
        gray_bit_depth(1).expect("one pattern has a depth"),
        false,
        0,
        None,
        &mut decoder,
        &generous_limits(),
        &mut work,
    )
    .expect("zero-plane image should decode");
    assert_eq!(values.test_values(), [0, 0, 0, 0]);
    assert_eq!(calls, 0);
}

#[test]
fn gray_depth_is_exact_at_every_power_of_two() {
    assert!(gray_bit_depth(0).is_err());
    assert_eq!(gray_bit_depth(1).expect("one pattern is valid"), 0);
    for depth in 1_u8..=32 {
        let power = 1_u64 << depth;
        assert_eq!(
            gray_bit_depth(power).expect("power-of-two count is valid"),
            depth
        );
        if depth < 32 {
            assert_eq!(
                gray_bit_depth(power + 1).expect("next count is valid"),
                depth + 1
            );
        }
    }
    assert!(gray_bit_depth((1_u64 << 32) + 1).is_err());
}

#[test]
fn grid_math_uses_signed_arithmetic_shift() {
    let header = HalftoneHeader {
        mmr: false,
        template: 0,
        enable_skip: false,
        operator: CombinationOperator::Or,
        default_pixel: false,
        grid_width: 1,
        grid_height: 1,
        grid_x: -1,
        grid_y: 512,
        grid_vector_x: 768,
        grid_vector_y: 256,
    };
    assert_eq!(
        grid_position(&header, 0, 0).expect("position fits"),
        (-1, 2)
    );
    assert_eq!(grid_position(&header, 2, 3).expect("position fits"), (8, 9));
}

#[test]
fn grid_math_extremes_fit_signed_intermediates() {
    let header = HalftoneHeader {
        mmr: false,
        template: 0,
        enable_skip: false,
        operator: CombinationOperator::Or,
        default_pixel: false,
        grid_width: 1,
        grid_height: 1,
        grid_x: i32::MIN,
        grid_y: i32::MAX,
        grid_vector_x: u16::MAX,
        grid_vector_y: u16::MAX,
    };
    let grid_x = u32::MAX;
    let grid_y = u32::MAX;
    let x_fixed = i128::from(header.grid_x)
        + i128::from(grid_y) * i128::from(header.grid_vector_y)
        + i128::from(grid_x) * i128::from(header.grid_vector_x);
    let y_fixed = i128::from(header.grid_y) + i128::from(grid_y) * i128::from(header.grid_vector_x)
        - i128::from(grid_x) * i128::from(header.grid_vector_y);
    assert_eq!(
        grid_position(&header, grid_x, grid_y).expect("all grid products fit"),
        (
            i64::try_from(x_fixed >> 8).expect("x position fits"),
            i64::try_from(y_fixed >> 8).expect("y position fits")
        )
    );
}

#[test]
fn rejects_unused_gray_code_and_work_overrun() {
    assert!(matches!(
        validate_pattern_indices(&[0, 1, 3], 3),
        Err(HalftoneError::InvalidPatternIndex {
            index: 3,
            pattern_count: 3
        })
    ));
    let limits = Limits {
        max_work_units: 3,
        ..generous_limits()
    };
    let mut work = WorkBudget::new(&limits);
    assert!(work.charge(3).is_ok());
    assert!(matches!(
        work.charge(1),
        Err(HalftoneError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Work,
                limit: 3,
                actual: 4
            }
        )))
    ));
}
