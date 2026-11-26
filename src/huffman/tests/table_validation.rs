#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn wide_zero_constructor_matches_default_and_array_initialization() {
    assert_eq!(WideCode::zero().limbs, [0; 4]);
    assert_eq!(WideCode::zero(), WideCode::default());
    assert_eq!(
        [WideCode::zero(); MAX_PREFIX_LEN + 1],
        [WideCode::default(); MAX_PREFIX_LEN + 1]
    );
}

#[test]
fn raw_canonical_assignment_matches_numeric_histogram_and_stable_order() {
    for line_count in 1..=5_u32 {
        for mut pattern in 0..6_usize.pow(line_count) {
            let mut lines = Vec::new();
            for index in 0..line_count {
                let length = u8::try_from(pattern % 6).expect("small length");
                pattern /= 6;
                lines.push(HuffmanLine::normal(
                    i32::try_from(index).expect("small index"),
                    length,
                    0,
                ));
            }
            let mut counts = [0_u32; MAX_PREFIX_LEN + 1];
            let mut expected_order: Vec<_> = (0..lines.len())
                .filter(|&index| lines[index].prefix_len != 0)
                .collect();
            expected_order.sort_by_key(|&index| (lines[index].prefix_len, index));
            for &index in &expected_order {
                counts[usize::from(lines[index].prefix_len)] += 1;
            }
            let mut first = [0_u64; 6];
            let mut fits = !expected_order.is_empty();
            for length in 1..6 {
                first[length] = 2 * (first[length - 1] + u64::from(counts[length - 1]));
                fits &= first[length] + u64::from(counts[length]) <= 1 << length;
            }
            let result = HuffmanTable::new(&lines, &Limits::default());
            if !fits {
                assert!(matches!(
                    result,
                    Err(HuffmanError::InvalidTable(
                        HuffmanTableError::NoCodes | HuffmanTableError::Oversubscribed
                    ))
                ));
                continue;
            }
            let table = result.expect("numeric prefix allocation fits");
            assert_eq!(table.lines.as_slice(), lines.as_slice());
            for (len, &expected_count) in counts.iter().enumerate() {
                assert_eq!(table.length_count_at(len), expected_count);
            }
            assert_eq!(
                table.ordered_lines.as_slice(),
                expected_order
                    .iter()
                    .map(|&i| u32::try_from(i).unwrap())
                    .collect::<Vec<_>>()
                    .as_slice()
            );
            let maximum = usize::from(table.max_prefix_len);
            assert_eq!(
                maximum,
                usize::from(lines[*expected_order.last().unwrap()].prefix_len)
            );
            for length in 1..=maximum {
                assert_eq!(table.first_code_at(length).limbs, [first[length], 0, 0, 0]);
                let start = expected_order
                    .iter()
                    .take_while(|&&index| usize::from(lines[index].prefix_len) < length)
                    .count();
                if counts[length] != 0 {
                    assert_eq!(usize::try_from(table.first_line_at(length)).unwrap(), start);
                }
                for (rank, &index) in expected_order
                    .iter()
                    .skip(start)
                    .take(usize::try_from(counts[length]).unwrap())
                    .enumerate()
                {
                    assert_eq!(
                        table.code_for_line(index),
                        Some((
                            WideCode {
                                limbs: [first[length] + u64::try_from(rank).unwrap(), 0, 0, 0]
                            },
                            lines[index].prefix_len
                        ))
                    );
                }
            }
            for (index, line) in lines.iter().enumerate() {
                if line.prefix_len == 0 {
                    assert!(table.code_for_line(index).is_none());
                }
            }
        }
    }
}

#[test]
fn canonical_assignment_follows_table_order() {
    let lines = [
        HuffmanLine::normal(0, 2, 0),
        HuffmanLine::normal(1, 1, 0),
        HuffmanLine::normal(2, 3, 0),
        HuffmanLine::normal(3, 3, 0),
    ];
    let table = HuffmanTable::new(&lines, &Limits::default()).expect("valid table");
    assert_eq!(decode_bits(&table, "0"), HuffmanValue::Integer(1));
    assert_eq!(decode_bits(&table, "10"), HuffmanValue::Integer(0));
    assert_eq!(decode_bits(&table, "110"), HuffmanValue::Integer(2));
    assert_eq!(decode_bits(&table, "111"), HuffmanValue::Integer(3));
}

#[test]
fn symbol_length_source_matches_materialized_line_oracle() {
    let lengths = [2_u8, 2, 3, 3, 3, 3];
    let lines = lengths
        .iter()
        .copied()
        .enumerate()
        .map(|(index, length)| {
            HuffmanLine::normal(i32::try_from(index).expect("small index"), length, 0)
        })
        .collect::<Vec<_>>();
    let specialized =
        HuffmanTable::new_symbol_ids(&lengths, &Limits::default()).expect("specialized table");
    let materialized = HuffmanTable::new(&lines, &Limits::default()).expect("materialized table");
    assert_eq!(specialized.lines.len(), materialized.lines.len());
    for prefix in 0_u16..=u16::MAX {
        let bytes = prefix.to_be_bytes();
        let mut specialized_reader = bit_reader(&bytes);
        let mut materialized_reader = bit_reader(&bytes);
        assert_eq!(
            format!("{:?}", specialized.decode(&mut specialized_reader)),
            format!("{:?}", materialized.decode(&mut materialized_reader))
        );
        assert_eq!(
            specialized_reader.bit_position,
            materialized_reader.bit_position
        );
    }
}

#[test]
fn parses_annex_b_example_table() {
    let encoded = [
        0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x10, 0x49, 0x23, 0x81, 0x80,
    ];
    let table =
        HuffmanTable::from_encoded(&encoded, &Limits::default()).expect("Annex B example table");
    assert_eq!(
        decode_bits(&table, "1100000000000000000"),
        HuffmanValue::Integer(272)
    );
    assert_eq!(
        decode_bits(&table, "11100000000000000000000000000000000"),
        HuffmanValue::Integer(65_808)
    );
}

#[test]
fn parses_oob_custom_table() {
    // HTOOB=1, HTPS=2, HTRS=1, low=0, high=1.
    // Lines are (prefix=1, range=0), lower=0, upper=2, OOB=2.
    let encoded = [0x03, 0, 0, 0, 0, 0, 0, 0, 1, 0b0100_0101, 0];
    let table = HuffmanTable::from_encoded(&encoded, &Limits::default()).expect("custom table");
    assert_eq!(decode_bits(&table, "0"), HuffmanValue::Integer(0));
    assert_eq!(decode_bits(&table, "11"), HuffmanValue::OutOfBand);
}

#[test]
fn rejects_reserved_flags_and_nonzero_padding() {
    let reserved = [0x80, 0, 0, 0, 0, 0, 0, 0, 1];
    assert!(matches!(
        HuffmanTable::from_encoded(&reserved, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::ReservedFlags(0x80)
        ))
    ));

    let padding = [0, 0, 0, 0, 0, 0, 0, 0, 1, 0b1011_1111];
    assert!(matches!(
        HuffmanTable::from_encoded(&padding, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::NonZeroPadding
        ))
    ));
}

#[test]
fn rejects_invalid_bounds_and_range_length() {
    let bounds = [0, 0, 0, 0, 1, 0, 0, 0, 1];
    assert!(matches!(
        HuffmanTable::from_encoded(&bounds, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::InvalidBounds { low: 1, high: 1 }
        ))
    ));

    // HTRS=6 and the first line has range length 33.
    let long_range = [0x50, 0, 0, 0, 0, 0, 0, 0, 1, 0b1100_0010];
    assert!(matches!(
        HuffmanTable::from_encoded(&long_range, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::InvalidRangeLength(33)
        ))
    ));
}

#[test]
fn encoded_final_range_crosses_high_without_clamping_or_moving_the_upper_base() {
    let mut payload = vec![0x12, 0, 0, 0, 0, 0, 0, 0, 2];
    payload.extend(bits("01100010"));
    let table = HuffmanTable::from_encoded(&payload, &Limits::default()).unwrap();
    for (wire, value) in [("000", 0), ("001", 1), ("010", 2), ("011", 3)] {
        assert_eq!(decode_bits(&table, wire), HuffmanValue::Integer(value));
    }
    for offset in 0..=2_u32 {
        let wire = format!("10{offset:032b}");
        assert_eq!(
            decode_bits(&table, &wire),
            HuffmanValue::Integer(2 + i32::try_from(offset).unwrap()),
        );
    }
    let overflow = format!("10{:032b}", i32::MAX as u32);
    let encoded = bits(&overflow);
    assert!(matches!(
        table.decode(&mut bit_reader(&encoded)),
        Err(HuffmanError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow
        )))
    ));
    payload.pop();
    assert!(HuffmanTable::from_encoded(&payload, &Limits::default()).is_err());
}

#[test]
fn raw_normal_ranges_match_wide_endpoint_and_contiguity_rules() {
    for low in [
        i64::MIN,
        i64::from(i32::MIN) - 1,
        i64::from(i32::MIN),
        -1,
        0,
        i64::from(i32::MAX) - 1,
        i64::from(i32::MAX),
        i64::from(i32::MAX) + 1,
        i64::MAX,
    ] {
        for range_len in [0, 1, 2, 31, 32, 33, u8::MAX] {
            let lines = [HuffmanLine {
                prefix_len: 1,
                range_len,
                range_low: low,
                kind: LineKind::Normal,
            }];
            let result = validate_lines(HuffmanLines::Borrowed(&lines));
            if range_len > 32 {
                assert!(matches!(result, Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidRangeLength(actual)
                    )) if actual == range_len));
                continue;
            }
            let end = i128::from(low) + (1_i128 << range_len) - 1;
            if end > i128::from(i64::MAX) {
                assert!(matches!(
                    result,
                    Err(HuffmanError::Decode(DecodeError::Malformed(
                        MalformedError::IntegerOverflow
                    )))
                ));
            } else if i32::try_from(low).is_err() || i32::try_from(end).is_err() {
                assert!(matches!(
                    result,
                    Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidRangePartition
                    ))
                ));
            } else {
                result.unwrap();
                for displacement in [-1, 0, 1] {
                    let mut adjacent = lines.to_vec();
                    adjacent.push(HuffmanLine {
                        prefix_len: 1,
                        range_len: 0,
                        range_low: i64::try_from(end + 1 + displacement).unwrap(),
                        kind: LineKind::Normal,
                    });
                    let result = validate_lines(HuffmanLines::Borrowed(&adjacent));
                    let valid = displacement == 0 && end < i128::from(i32::MAX);
                    assert_eq!(result.is_ok(), valid, "{adjacent:?}");
                    if !valid {
                        assert!(matches!(
                            result,
                            Err(HuffmanError::InvalidTable(
                                HuffmanTableError::InvalidRangePartition
                            ))
                        ));
                    }
                }
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn raw_special_line_metadata_rejects_order_duplicates_and_invalid_fields() {
    let normal = HuffmanLine::normal(0, 2, 0);
    let lower = HuffmanLine::lower(-1, 2);
    let upper = HuffmanLine::upper(1, 2);
    let oob = HuffmanLine::out_of_band(2);
    let cases = [
        (vec![], Some(HuffmanTableError::NoCodes)),
        (
            vec![lower, upper, oob],
            Some(HuffmanTableError::InvalidRangePartition),
        ),
        (vec![normal, lower, upper, oob], None),
        (vec![normal, lower], None),
        (vec![normal, upper], None),
        (vec![normal, oob], None),
        (
            vec![normal, lower, normal],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![normal, upper, lower],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![normal, oob, lower],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![normal, oob, upper],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![normal, lower, lower],
            Some(HuffmanTableError::DuplicateSpecialLine),
        ),
        (
            vec![normal, upper, upper],
            Some(HuffmanTableError::DuplicateSpecialLine),
        ),
        (
            vec![normal, oob, oob],
            Some(HuffmanTableError::DuplicateSpecialLine),
        ),
        (
            vec![
                normal,
                HuffmanLine {
                    range_len: 0,
                    ..lower
                },
            ],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![
                normal,
                HuffmanLine {
                    range_len: 0,
                    ..upper
                },
            ],
            Some(HuffmanTableError::InvalidLineOrder),
        ),
        (
            vec![
                normal,
                HuffmanLine {
                    range_len: 1,
                    ..oob
                },
            ],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![normal, HuffmanLine::lower(-2, 2)],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![normal, HuffmanLine::upper(2, 2)],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![
                normal,
                HuffmanLine {
                    range_low: i64::MIN,
                    ..upper
                },
            ],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![
                HuffmanLine::normal(i32::MAX, 2, 0),
                HuffmanLine {
                    range_low: i64::from(i32::MAX) + 1,
                    ..upper
                },
            ],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![
                HuffmanLine::normal(i32::MIN, 2, 0),
                HuffmanLine::lower(i64::from(i32::MIN) - 1, 2),
            ],
            Some(HuffmanTableError::InvalidSpecialLine),
        ),
        (
            vec![
                HuffmanLine::normal(i32::MIN, 2, 0),
                HuffmanLine::lower(i64::from(i32::MIN) - 1, 0),
            ],
            None,
        ),
    ];
    for (lines, expected) in cases {
        match (validate_lines(HuffmanLines::Borrowed(&lines)), expected) {
            (Ok(()), None) => {}
            (Err(HuffmanError::InvalidTable(actual)), Some(expected)) => {
                assert_eq!(actual, expected, "{lines:?}");
            }
            (actual, expected) => panic!("{lines:?}: got {actual:?}, expected {expected:?}"),
        }
    }
}

#[test]
fn raw_table_memory_limits_match_wide_sizes_and_error_priority() {
    for line_count in [0, 1, 15, 256, u32::MAX as usize, usize::MAX] {
        for prefix_lookup in [false, true] {
            let bytes = size_of::<HuffmanTable>() as u128
                + line_count as u128 * (size_of::<HuffmanLine>() + size_of::<u32>()) as u128
                + if prefix_lookup { 256 * 2 } else { 0 };
            for symbol_limit in [0, 15, u32::MAX] {
                for memory_limit in [0, u64::try_from(bytes).unwrap_or(u64::MAX), u64::MAX] {
                    let limits = Limits {
                        max_symbols: symbol_limit,
                        max_decoder_managed_memory_bytes: memory_limit,
                        ..Limits::default()
                    };
                    let result = check_table_memory(line_count, prefix_lookup, &limits);
                    if line_count as u128 > u128::from(symbol_limit) {
                        assert!(matches!(result, Err(HuffmanError::Decode(
                                DecodeError::LimitExceeded(LimitExceeded {
                                    kind: LimitKind::Symbols, limit, actual,
                                })
                            )) if limit == u64::from(symbol_limit) && u128::from(actual) == line_count as u128));
                    } else if bytes > usize::MAX as u128 || bytes > u128::from(u64::MAX) {
                        assert!(matches!(
                            result,
                            Err(HuffmanError::Decode(DecodeError::Malformed(
                                MalformedError::IntegerOverflow
                            )))
                        ));
                    } else if bytes > u128::from(memory_limit) {
                        assert!(matches!(result, Err(HuffmanError::Decode(
                                DecodeError::LimitExceeded(LimitExceeded {
                                    kind: LimitKind::Memory, limit, actual,
                                })
                            )) if limit == memory_limit && u128::from(actual) == bytes));
                    } else {
                        result.unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn rejects_oversubscribed_and_structurally_invalid_tables() {
    let oversubscribed = [
        HuffmanLine::normal(0, 1, 0),
        HuffmanLine::normal(1, 1, 0),
        HuffmanLine::normal(2, 1, 0),
    ];
    assert!(matches!(
        HuffmanTable::new(&oversubscribed, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::Oversubscribed
        ))
    ));

    let gap = [HuffmanLine::normal(0, 1, 0), HuffmanLine::normal(2, 1, 0)];
    assert!(matches!(
        HuffmanTable::new(&gap, &Limits::default()),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::InvalidRangePartition
        ))
    ));

    let no_codes = [HuffmanLine::normal(0, 0, 0)];
    assert!(matches!(
        HuffmanTable::new(&no_codes, &Limits::default()),
        Err(HuffmanError::InvalidTable(HuffmanTableError::NoCodes))
    ));
}

#[test]
fn invalid_prefix_and_eof_are_oob_safe() {
    let lines = [HuffmanLine::normal(0, 1, 0), HuffmanLine::normal(1, 3, 0)];
    let table = HuffmanTable::new(&lines, &Limits::default()).expect("valid sparse table");

    let mut invalid_reader = bit_reader(&[0b1100_0000]);
    assert!(matches!(
        table.decode(&mut invalid_reader),
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))
    ));

    let mut eof_reader = bit_reader(&[0b0000_0001]);
    eof_reader.bit_position = 7;
    assert!(matches!(
        table.decode(&mut eof_reader),
        Err(HuffmanError::Decode(DecodeError::Malformed(
            MalformedError::UnexpectedEof {
                offset: 8,
                needed: 1,
                remaining: 0
            }
        )))
    ));
    assert_eq!(eof_reader.bit_position, 8);
}

#[test]
fn open_ranges_reject_values_outside_i32() {
    let table =
        HuffmanTable::standard(StandardHuffmanTable::A, &Limits::default()).expect("table A");
    let encoded = format!("111{:032b}", u32::MAX);
    let bytes = bits(&encoded);
    let mut reader = bit_reader(&bytes);
    assert!(matches!(
        table.decode(&mut reader),
        Err(HuffmanError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow
        )))
    ));
}

#[test]
fn honors_memory_and_stream_limits() {
    let limits = Limits {
        max_decoder_managed_memory_bytes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        HuffmanTable::standard(StandardHuffmanTable::A, &limits),
        Err(HuffmanError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Memory,
                ..
            }
        )))
    ));

    let limits = Limits {
        max_stream_bytes: 8,
        ..Limits::default()
    };
    let encoded = [0_u8; 9];
    assert!(matches!(
        HuffmanTable::from_encoded(&encoded, &limits),
        Err(HuffmanError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::StreamBytes,
                limit: 8,
                actual: 9
            }
        )))
    ));
}
