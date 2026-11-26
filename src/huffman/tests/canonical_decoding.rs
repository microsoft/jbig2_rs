#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

fn assert_cache_result(
    actual: Result<HuffmanValue, HuffmanError>,
    expected: Result<HuffmanValue, HuffmanError>,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (
            Err(HuffmanError::Decode(DecodeError::Malformed(actual))),
            Err(HuffmanError::Decode(DecodeError::Malformed(expected))),
        ) => assert_eq!(actual, expected),
        (
            Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
            Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
        ) => {}
        (actual, expected) => panic!("cache result {actual:?}, expected {expected:?}"),
    }
}

#[test]
fn fixed_width_cached_table_selects_literal_values_with_exact_consumption() {
    let lines = (0..=u8::MAX)
        .map(|value| HuffmanLine::normal(i32::from(value), 8, 0))
        .collect::<Vec<_>>();
    let table = HuffmanTable::new(&lines, &Limits::default()).unwrap();
    assert!(table.prefer_prefix_lookup);
    let mut checked = 0;
    for word in 0..=u16::MAX {
        let [high, low] = word.to_be_bytes();
        let bytes = [high, low, 0x5a, 0xa5];
        for start in 0..8 {
            let expected = (start..start + 8).fold(0_i32, |value, bit| {
                (value << 1) | i32::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
            });
            let mut reader = bit_reader(&bytes);
            reader.bit_position = start;
            assert_cache_result(
                table.decode(&mut reader),
                Ok(HuffmanValue::Integer(expected)),
            );
            assert_eq!(reader.bytes, &bytes);
            assert_eq!(reader.bit_position, start + 8);
            let next = (start + 8..start + 16).fold(0_u32, |value, bit| {
                (value << 1) | u32::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
            });
            assert_eq!(reader.read_bits(8).unwrap(), next);
            let mut fallback = bit_reader(&bytes);
            fallback.bit_position = start;
            assert_cache_result(
                table.decode_fallback(&mut fallback),
                Ok(HuffmanValue::Integer(expected)),
            );
            assert_eq!(fallback.bit_position, start + 8);
            checked += 1;
        }
    }
    assert_eq!(checked, 524_288);
}

#[test]
fn raw_cached_line_selection_composes_exact_values_errors_and_cursor_frames() {
    let cases = [
        (8, 0, 7, LineKind::Normal),
        (8, 7, -3, LineKind::Normal),
        (8, 8, 0, LineKind::Lower),
        (8, 32, i64::from(i32::MIN), LineKind::Upper),
        (8, 8, i64::from(i32::MAX), LineKind::Normal),
        (8, 8, i64::MAX, LineKind::Upper),
        (8, 8, i64::MIN, LineKind::Lower),
        (8, 33, 0, LineKind::Normal),
        (8, 255, 0, LineKind::OutOfBand),
        (0, 0, 11, LineKind::Normal),
        (3, 8, 17, LineKind::Lower),
        (9, 0, 13, LineKind::Normal),
    ];
    let mut outcomes = [0_usize; 9];
    let mut checked = 0;
    let mut table = HuffmanTable::test_empty(vec![HuffmanLine::normal(91, 8, 0); 2], Vec::new(), 0);
    table.prefix_lookup = TableSlice::Owned(vec![0; PREFIX_LOOKUP_SIZE]);
    table.prefer_prefix_lookup = true;
    for prefix in 0..=u8::MAX {
        for start in 0..8 {
            let mut buffer = [0xa5_u8; 8];
            for bit in 0..8 {
                let position = start + bit;
                let mask = 0x80 >> (position % 8);
                if prefix & (0x80 >> bit) != 0 {
                    buffer[position / 8] |= mask;
                } else {
                    buffer[position / 8] &= !mask;
                }
            }
            for byte_count in [2, 4, 8] {
                let bytes = &buffer[..byte_count];
                for (prefix_len, range_len, range_low, kind) in cases {
                    if let TableSlice::Owned(ref mut ls) = table.lines {
                        ls[1] = HuffmanLine {
                            prefix_len,
                            range_len,
                            range_low,
                            kind,
                        };
                    }
                    for entry in [0_u16, 1, 2, 3, u16::MAX] {
                        if let TableSlice::Owned(ref mut pl) = table.prefix_lookup {
                            pl[usize::from(prefix)] = entry;
                        }
                        let (expected, position, outcome) = if entry == 0 {
                            (
                                Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
                                start,
                                8,
                            )
                        } else {
                            match table.lines.get(usize::from(entry - 1)) {
                                None => (
                                    Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
                                    start,
                                    6,
                                ),
                                Some(line) if line.prefix_len > 8 => (
                                    Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
                                    start,
                                    7,
                                ),
                                Some(line) => {
                                    let (out, position, outcome) = raw_line_oracle(
                                        *line,
                                        bytes,
                                        start + usize::from(line.prefix_len),
                                    );
                                    (
                                        out.map_err(|error| {
                                            HuffmanError::Decode(DecodeError::Malformed(error))
                                        }),
                                        position,
                                        outcome,
                                    )
                                }
                            }
                        };
                        let mut reader = bit_reader(bytes);
                        reader.bit_position = start;
                        assert_cache_result(table.decode(&mut reader), expected);
                        assert_eq!(reader.bytes, bytes);
                        assert_eq!(reader.bit_position, position);
                        outcomes[outcome] += 1;
                        checked += 1;
                    }
                }
            }
            if let TableSlice::Owned(ref mut pl) = table.prefix_lookup {
                pl[usize::from(prefix)] = 0;
            }
        }
    }
    assert_eq!(checked, 368_640);
    assert!(outcomes.into_iter().all(|count| count > 0));
}

#[test]
fn prefix_peek_matches_every_byte_window_with_exact_availability_and_no_consumption() {
    let mut checked = 0;
    let mut outcomes = [0_usize; 3];
    for word in 0..=u16::MAX {
        let [high, low] = word.to_be_bytes();
        let buffer = [high, low, 0x5a];
        for byte_count in 0..=buffer.len() {
            let bytes = &buffer[..byte_count];
            for start in 0..=bytes.len() * 8 {
                let expected = if bytes.len() * 8 - start >= 8 {
                    let value = (start..start + 8).fold(0_u8, |value, bit| {
                        (value << 1) | ((bytes[bit / 8] >> (7 - bit % 8)) & 1)
                    });
                    outcomes[usize::from(start % 8 != 0)] += 1;
                    Some(value)
                } else {
                    outcomes[2] += 1;
                    None
                };
                let mut reader = bit_reader(bytes);
                reader.bit_position = start;
                let actual = crate::huffman::decoding::peek_prefix(&reader);
                assert_eq!(actual, expected, "{bytes:02x?} at {start}");
                assert_eq!(reader.bytes, bytes);
                assert_eq!(reader.bit_position, start);
                assert_eq!(crate::huffman::decoding::peek_prefix(&reader), expected);
                if let Some(value) = expected {
                    assert_eq!(reader.read_bits(8).unwrap(), u32::from(value));
                    assert_eq!(reader.bit_position, start + 8);
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3_407_872);
    assert!(outcomes.into_iter().all(|count| count > 0));
}

pub(super) fn raw_line_oracle(
    line: HuffmanLine,
    bytes: &[u8],
    start: usize,
) -> (Result<HuffmanValue, MalformedError>, usize, usize) {
    if line.kind == LineKind::OutOfBand {
        return (Ok(HuffmanValue::OutOfBand), start, 0);
    }
    if line.range_len > 32 {
        return (
            Err(MalformedError::InvalidBitCount(line.range_len)),
            start,
            2,
        );
    }
    let remaining = bytes.len() * 8 - start;
    if usize::from(line.range_len) > remaining {
        return (
            Err(MalformedError::UnexpectedEof {
                offset: u64::try_from(start).unwrap(),
                needed: u64::from(line.range_len),
                remaining: u64::try_from(remaining).unwrap(),
            }),
            start,
            3,
        );
    }
    let end = start + usize::from(line.range_len);
    let offset = (start..end).fold(0_i128, |value, bit| {
        (value << 1) | i128::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
    });
    let candidate = if line.kind == LineKind::Lower {
        i128::from(line.range_low) - offset
    } else {
        i128::from(line.range_low) + offset
    };
    match i32::try_from(candidate) {
        Ok(value) => (Ok(HuffmanValue::Integer(value)), end, 1),
        Err(_) => (
            Err(MalformedError::IntegerOverflow),
            end,
            if candidate < i128::from(i64::MIN) || candidate > i128::from(i64::MAX) {
                4
            } else {
                5
            },
        ),
    }
}

#[test]
fn raw_line_outcomes_match_wide_oracle_with_exact_errors_frames_and_acceptance() {
    let samples: [&[u8]; 5] = [
        &[],
        &[0],
        &[0xff],
        &[0xa5, 0x32, 0xdc, 0x4f, 0x80],
        &[0xff; 8],
    ];
    let bases = [
        i64::MIN,
        i64::MIN + 1,
        i64::from(i32::MIN) - 1,
        i64::from(i32::MIN),
        i64::from(i32::MIN) + 1,
        -1,
        0,
        1,
        i64::from(i32::MAX) - 1,
        i64::from(i32::MAX),
        i64::from(i32::MAX) + 1,
        i64::MAX - 1,
        i64::MAX,
    ];
    let mut outcomes = [0_usize; 6];
    for bytes in samples {
        for start in 0..=bytes.len() * 8 {
            for range_len in 0..=u8::MAX {
                for kind in [
                    LineKind::Normal,
                    LineKind::Lower,
                    LineKind::Upper,
                    LineKind::OutOfBand,
                ] {
                    for range_low in bases {
                        let line = HuffmanLine {
                            prefix_len: range_len.wrapping_mul(3),
                            range_len,
                            range_low,
                            kind,
                        };
                        let (expected, position, outcome) = raw_line_oracle(line, bytes, start);
                        let mut reader = bit_reader(bytes);
                        reader.bit_position = start;
                        let actual = crate::huffman::decoding::decode_line(line, &mut reader);
                        match (actual, expected) {
                            (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
                            (
                                Err(HuffmanError::Decode(DecodeError::Malformed(actual))),
                                Err(expected),
                            ) => assert_eq!(actual, expected),
                            (actual, expected) => {
                                panic!(
                                    "line {line:?} at {start}: {actual:?}, expected {expected:?}"
                                )
                            }
                        }
                        assert_eq!(reader.bytes, bytes);
                        assert_eq!(reader.bit_position, position);
                        let next_count = u8::try_from((bytes.len() * 8 - position).min(8)).unwrap();
                        let expected_next = (position..position + usize::from(next_count)).fold(
                            0_u32,
                            |value, bit| {
                                (value << 1) | u32::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
                            },
                        );
                        assert_eq!(reader.read_bits(next_count).unwrap(), expected_next);
                        outcomes[outcome] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(outcomes.iter().sum::<usize>(), 1_664_000);
    assert!(outcomes.into_iter().all(|count| count > 0));
}

#[test]
fn annex_b_table_a_codes_and_endpoints() {
    let table =
        HuffmanTable::standard(StandardHuffmanTable::A, &Limits::default()).expect("table A");
    assert_eq!(decode_bits(&table, "00000"), HuffmanValue::Integer(0));
    assert_eq!(decode_bits(&table, "01111"), HuffmanValue::Integer(15));
    assert_eq!(decode_bits(&table, "1000000000"), HuffmanValue::Integer(16));
    assert_eq!(
        decode_bits(&table, "1011111111"),
        HuffmanValue::Integer(271)
    );
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
fn annex_b_table_b_oob_and_fixed_values() {
    let table =
        HuffmanTable::standard(StandardHuffmanTable::B, &Limits::default()).expect("table B");
    assert_eq!(decode_bits(&table, "0"), HuffmanValue::Integer(0));
    assert_eq!(decode_bits(&table, "10"), HuffmanValue::Integer(1));
    assert_eq!(decode_bits(&table, "110"), HuffmanValue::Integer(2));
    assert_eq!(decode_bits(&table, "1110000"), HuffmanValue::Integer(3));
    assert_eq!(decode_bits(&table, "111111"), HuffmanValue::OutOfBand);
}

#[test]
fn every_line_of_every_standard_table_decodes() {
    for standard in ALL_STANDARD_TABLES {
        let mut table =
            HuffmanTable::standard(standard, &Limits::default()).expect("standard table");
        force_prefix_lookup(&mut table);
        for (index, line) in table.lines.as_slice().iter().copied().enumerate() {
            if line.prefix_len == 0 {
                continue;
            }
            if line.kind == LineKind::OutOfBand {
                let encoded = encoded_line(&table, index, 0);
                assert_eq!(decode_bits(&table, &encoded), HuffmanValue::OutOfBand);
                continue;
            }

            let encoded = encoded_line(&table, index, 0);
            assert_eq!(
                decode_bits(&table, &encoded),
                HuffmanValue::Integer(
                    i32::try_from(line.range_low).expect("zero-offset value in range")
                )
            );

            let maximum_offset = match line.kind {
                LineKind::Normal => {
                    if line.range_len == 32 {
                        u32::MAX
                    } else {
                        (1_u32 << line.range_len) - 1
                    }
                }

                LineKind::Lower => {
                    u32::try_from(line.range_low - i64::from(i32::MIN)).expect("lower endpoint")
                }
                LineKind::Upper => {
                    u32::try_from(i64::from(i32::MAX) - line.range_low).expect("upper endpoint")
                }
                LineKind::OutOfBand => 0,
            };
            let expected = if line.kind == LineKind::Lower {
                line.range_low - i64::from(maximum_offset)
            } else {
                line.range_low + i64::from(maximum_offset)
            };
            let encoded = encoded_line(&table, index, maximum_offset);
            assert_eq!(
                decode_bits(&table, &encoded),
                HuffmanValue::Integer(i32::try_from(expected).expect("endpoint in range"))
            );
        }
    }
}

#[test]
fn prefix_lookup_matches_canonical_for_all_standard_prefixes() {
    for standard in ALL_STANDARD_TABLES {
        let table = HuffmanTable::standard(standard, &Limits::default()).expect("standard table");
        for prefix in 0_u32..=u32::from(u16::MAX) {
            let prefix = u16::try_from(prefix).expect("bounded prefix");
            let bytes = [
                u8::try_from(prefix >> 8).expect("high byte"),
                u8::try_from(prefix & 0xff).expect("low byte"),
                0x5a,
                0xa5,
                0x3c,
                0xc3,
            ];
            assert_decoders_equal(&table, &bytes, 0);
        }
        for byte_count in 0..=1 {
            let bytes = [0xa5];
            for bit_position in 0..=byte_count * 8 {
                assert_decoders_equal(&table, &bytes[..byte_count], bit_position);
            }
        }
    }
}

#[test]
fn randomized_custom_prefix_lookup_matches_canonical() {
    let mut random = 0x9e37_79b9_u32;
    for _ in 0..128 {
        random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let leaf_count = usize::try_from(random % 63 + 2).expect("small leaf count");
        let mut lengths = vec![1_u8, 1_u8];
        while lengths.len() < leaf_count {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let index = usize::try_from(random).expect("u32 fits usize") % lengths.len();
            let child_length = lengths[index].checked_add(1).expect("bounded depth");
            lengths[index] = child_length;
            lengths.push(child_length);
        }

        let mut lines = Vec::with_capacity(lengths.len());
        let mut range_low = -10_000_i32;
        for length in lengths {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let range_len = u8::try_from(random & 3).expect("two-bit range length");
            lines.push(HuffmanLine::normal(range_low, length, range_len));
            range_low = range_low
                .checked_add(1_i32 << range_len)
                .expect("small generated ranges");
        }
        let mut table = HuffmanTable::new(&lines, &Limits::default()).expect("custom table");
        force_prefix_lookup(&mut table);

        for _ in 0..512 {
            let mut bytes = [0_u8; 8];
            for byte in &mut bytes {
                random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *byte = u8::try_from(random >> 24).expect("high byte");
            }
            let bit_position = usize::try_from(random & 7).expect("three-bit position");
            assert_decoders_equal(&table, &bytes, bit_position);
        }
    }
}

#[test]
fn standard_prefixes_match_every_annex_b_table() {
    let expected: [&[&str]; 15] = [
        &["0", "10", "110", "111"],
        &["0", "10", "110", "1110", "11110", "111110", "111111"],
        &[
            "11111110", "0", "10", "110", "1110", "11110", "11111111", "1111110", "111110",
        ],
        &["0", "10", "110", "1110", "11110", "11111"],
        &[
            "1111110", "0", "10", "110", "1110", "11110", "1111111", "111110",
        ],
        &[
            "11100", "1000", "1001", "1010", "11101", "11110", "1011", "00", "010", "011", "1100",
            "1101", "111110", "111111",
        ],
        &[
            "1000", "000", "1001", "11010", "11011", "1010", "1011", "11100", "11101", "1100",
            "001", "010", "011", "11110", "11111",
        ],
        &[
            "11111100",
            "111111100",
            "11111101",
            "111111101",
            "1111100",
            "1010",
            "00",
            "11010",
            "111010",
            "100",
            "111011",
            "1011",
            "1100",
            "11011",
            "11100",
            "111100",
            "1111101",
            "111101",
            "111111110",
            "111111111",
            "01",
        ],
        &[
            "11111100",
            "111111100",
            "11111101",
            "111111101",
            "1111100",
            "1010",
            "010",
            "011",
            "11010",
            "111010",
            "100",
            "111011",
            "1011",
            "1100",
            "11011",
            "11100",
            "111100",
            "1111101",
            "111101",
            "111111110",
            "111111111",
            "00",
        ],
        &[
            "1111010", "11111100", "1111011", "11000", "00", "11001", "110110", "1111100",
            "11111101", "01", "11010", "110111", "111000", "111001", "111010", "111011", "111100",
            "1111101", "11111110", "11111111", "10",
        ],
        &[
            "0", "10", "1100", "1101", "11100", "11101", "111100", "1111010", "1111011", "1111100",
            "1111101", "1111110", "1111111",
        ],
        &[
            "0", "10", "110", "11100", "11101", "111100", "1111010", "1111011", "1111100",
            "1111101", "1111110", "11111110", "11111111",
        ],
        &[
            "0", "100", "1100", "11100", "1101", "101", "111010", "111011", "111100", "111101",
            "111110", "1111110", "1111111",
        ],
        &["100", "101", "0", "110", "111"],
        &[
            "1111100", "111100", "11100", "1100", "100", "0", "101", "1101", "11101", "111101",
            "1111101", "1111110", "1111111",
        ],
    ];

    for (table_number, standard) in ALL_STANDARD_TABLES.into_iter().enumerate() {
        let table = HuffmanTable::standard(standard, &Limits::default()).expect("standard table");
        let expected_codes = expected[table_number];
        assert_eq!(table.lines.len(), expected_codes.len());
        for (index, expected_code) in expected_codes.iter().enumerate() {
            assert_eq!(code_string(&table, index), *expected_code);
        }
    }
}
