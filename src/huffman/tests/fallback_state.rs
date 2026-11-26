#![cfg(test)]

use super::canonical_decoding::raw_line_oracle;
use super::*;

fn assert_result(
    actual: Result<HuffmanValue, HuffmanError>,
    expected: &Result<HuffmanValue, HuffmanError>,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, *expected),
        (
            Err(HuffmanError::Decode(DecodeError::Malformed(actual))),
            Err(HuffmanError::Decode(DecodeError::Malformed(expected))),
        ) => assert_eq!(&actual, expected),
        (
            Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
            Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
        ) => {}
        (actual, expected) => panic!("fallback {actual:?}, expected {expected:?}"),
    }
}

fn scalar_distance(code: &[bool; 256], lower: WideCode, bound: u32) -> Option<u32> {
    if bound == 0 {
        return None;
    }
    let mut borrow = 0_i8;
    let mut distance = 0_u32;
    let mut wide = false;
    for index in (0..256).rev() {
        let bit = 255 - index;
        let lower_bit = (lower.limbs[bit / 64] >> (bit % 64)) & 1 != 0;
        let difference = i8::from(code[index]) - i8::from(lower_bit) - borrow;
        let value = if difference < 0 {
            borrow = 1;
            difference + 2
        } else {
            borrow = 0;
            difference
        };
        if index < 224 {
            wide |= value != 0;
        } else {
            distance |= u32::try_from(value).unwrap() << bit;
        }
    }
    (borrow == 0 && !wide && distance < bound).then_some(distance)
}

fn scalar_fallback(
    table: &HuffmanTable,
    bytes: &[u8],
    start: usize,
) -> (Result<HuffmanValue, HuffmanError>, usize) {
    let mut prefix = [false; 256];
    let mut position = start;
    for length in 1..=usize::from(table.max_prefix_len) {
        if position == bytes.len() * 8 {
            return (
                Err(HuffmanError::Decode(DecodeError::Malformed(
                    MalformedError::UnexpectedEof {
                        offset: u64::try_from(position).unwrap(),
                        needed: 1,
                        remaining: 0,
                    },
                ))),
                position,
            );
        }
        prefix.copy_within(1.., 0);
        prefix[255] = bytes[position / 8] & (0x80 >> (position % 8)) != 0;
        position += 1;
        if let Some(distance) = scalar_distance(
            &prefix,
            table.first_code_at(length),
            table.length_count_at(length),
        ) {
            let ordered = u64::from(table.first_line_at(length)) + u64::from(distance);
            if ordered > u64::from(u32::MAX) {
                return (
                    Err(HuffmanError::Decode(DecodeError::Malformed(
                        MalformedError::IntegerOverflow,
                    ))),
                    position,
                );
            }
            let Some(&index) = table.ordered_lines.get(usize::try_from(ordered).unwrap()) else {
                return (
                    Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
                    position,
                );
            };
            let Some(line) = table.lines.get(usize::try_from(index).unwrap()) else {
                return (
                    Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
                    position,
                );
            };
            let (out, position, _) = raw_line_oracle(*line, bytes, position);
            return (
                out.map_err(|error| HuffmanError::Decode(DecodeError::Malformed(error))),
                position,
            );
        }
    }
    (
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
        position,
    )
}

fn scalar_assignment(lines: &[HuffmanLine]) -> HuffmanTable {
    let maximum = lines.iter().map(|line| line.prefix_len).max().unwrap();
    assert!(maximum < 128);
    let mut table = HuffmanTable::test_empty(lines.to_vec(), Vec::new(), maximum);
    for line in lines {
        if line.prefix_len != 0 {
            let l = usize::from(line.prefix_len);
            let cur = table.length_count_at(l);
            table.set_test_count(l, cur + 1);
        }
    }
    let mut first = 0_u128;
    let mut ordered = Vec::new();
    for length in 1..=usize::from(maximum) {
        first = (first + u128::from(table.length_count_at(length - 1))) * 2;
        table.set_test_first_code(
            length,
            WideCode {
                limbs: [
                    u64::try_from(first & u128::from(u64::MAX)).unwrap(),
                    u64::try_from(first >> 64).unwrap(),
                    0,
                    0,
                ],
            },
        );
        table.set_test_first_line(length, u32::try_from(ordered.len()).unwrap());
        ordered.extend(
            lines
                .iter()
                .enumerate()
                .filter(|(_, line)| usize::from(line.prefix_len) == length)
                .map(|(index, _)| u32::try_from(index).unwrap()),
        );
    }
    table.ordered_lines = TableSlice::Owned(ordered);
    table
}

#[test]
fn standard_prefix_selection_matches_independent_numeric_assignment_and_frames() {
    let limits = Limits::default();
    let mut checked = 0;
    for standard in ALL_STANDARD_TABLES {
        let mut actual = HuffmanTable::standard(standard, &limits).unwrap();
        let reference = scalar_assignment(actual.lines.as_slice());
        force_prefix_lookup(&mut actual);
        for word in 0_u16..1024 {
            let [high, low] = (word << 6).to_be_bytes();
            let buffer = [high, low, 0xa5, 0x5a, 0x3c, 0xc3];
            for byte_count in [1, 2, 6] {
                let bytes = &buffer[..byte_count];
                for start in [0, 1, 7] {
                    let (expected, position) = scalar_fallback(&reference, bytes, start);
                    let mut fallback = bit_reader(bytes);
                    fallback.bit_position = start;
                    assert_result(actual.decode_fallback(&mut fallback), &expected);
                    assert_eq!(fallback.bytes, bytes);
                    assert_eq!(fallback.bit_position, position);
                    for prefer in [false, true] {
                        actual.prefer_prefix_lookup = prefer;
                        let mut reader = bit_reader(bytes);
                        reader.bit_position = start;
                        assert_result(actual.decode(&mut reader), &expected);
                        assert_eq!(reader.bytes, bytes);
                        assert_eq!(reader.bit_position, position);
                    }
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 138_240);
}

#[test]
fn longest_literal_prefixes_keep_complete_success_truncation_and_miss_state() {
    let limits = Limits::default();
    let mut checked = 0;
    for length in [1, 7, 8, 31, 32, 63, 64, 127, 128, 254, 255] {
        let table = HuffmanTable::new(&[HuffmanLine::normal(23, length, 0)], &limits).unwrap();
        let mut bytes = vec![0_u8; usize::from(length).div_ceil(8)];
        let mut reader = bit_reader(&bytes);
        assert_result(table.decode(&mut reader), &Ok(HuffmanValue::Integer(23)));
        assert_eq!(reader.bit_position, usize::from(length));
        checked += 1;
        for cut in 0..bytes.len() {
            let (expected, position) = scalar_fallback(&table, &bytes[..cut], 0);
            let mut reader = bit_reader(&bytes[..cut]);
            assert_result(table.decode(&mut reader), &expected);
            assert_eq!(reader.bit_position, position);
            checked += 1;
        }
        let last = usize::from(length - 1);
        bytes[last / 8] |= 0x80 >> (last % 8);
        let mut reader = bit_reader(&bytes);
        assert_result(
            table.decode(&mut reader),
            &Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode)),
        );
        assert_eq!(reader.bit_position, usize::from(length));
        checked += 1;
    }
    assert_eq!(checked, 145);
}

#[test]
fn fallback_metadata_errors_and_wide_bucket_selection_match_scalar_state() {
    let mut table = HuffmanTable::test_empty(
        vec![HuffmanLine::normal(17, 1, 0), HuffmanLine::out_of_band(1)],
        vec![0, 1],
        1,
    );
    let mut outcomes = [0_usize; 4];
    let mut checked = 0;
    for maximum in [0, 1, 8, 31, 64, 127, 255] {
        table.max_prefix_len = maximum;
        for bound in [0, 1, 2, u32::MAX] {
            table.set_test_count(usize::from(maximum), bound);
            for first in [0, u32::MAX] {
                table.set_test_first_line(usize::from(maximum), first);
                for mapping in [vec![], vec![0, 1], vec![u32::MAX, 0]] {
                    table.ordered_lines = TableSlice::Owned(mapping);
                    for seed in [0_u8, 0xff, 0xa5] {
                        let bytes = [seed; 33];
                        for count in [0, 1, 8, 16, 32, 33] {
                            let input = &bytes[..count];
                            for start in [0, 1, 7] {
                                if start > input.len() * 8 {
                                    continue;
                                }
                                let (expected, position) = scalar_fallback(&table, input, start);
                                match &expected {
                                    Ok(_) => outcomes[0] += 1,
                                    Err(HuffmanError::InvalidTable(_)) => outcomes[1] += 1,
                                    Err(HuffmanError::Decode(DecodeError::Malformed(
                                        MalformedError::UnexpectedEof { .. },
                                    ))) => outcomes[2] += 1,
                                    Err(HuffmanError::Decode(DecodeError::Malformed(
                                        MalformedError::IntegerOverflow,
                                    ))) => outcomes[3] += 1,
                                    _ => panic!("unexpected scalar error"),
                                }
                                let mut reader = bit_reader(input);
                                reader.bit_position = start;
                                assert_result(table.decode(&mut reader), &expected);
                                assert_eq!(reader.bytes, input);
                                assert_eq!(reader.bit_position, position);
                                checked += 1;
                            }
                        }
                    }
                }
            }
            table.set_test_count(usize::from(maximum), 0);
        }
        table.set_test_first_line(usize::from(maximum), 0);
    }
    assert!(outcomes.into_iter().all(|count| count > 0));
    assert_eq!(checked, 8_064);
}

#[test]
fn nonzero_long_prefixes_select_the_given_wide_code_and_ordered_line() {
    let mut table = HuffmanTable::test_empty(
        vec![HuffmanLine::normal(91, 1, 0), HuffmanLine::normal(23, 1, 0)],
        vec![1],
        1,
    );
    let mut checked = 0;
    for length in [31_u8, 32, 63, 64, 65, 127, 128, 129, 254, 255] {
        table.max_prefix_len = length;
        table.set_test_count(usize::from(length), 1);
        let high = usize::from(length - 1);
        let mut fc = WideCode::zero();
        fc.limbs[high / 64] = 1 << (high % 64);
        table.set_test_first_code(usize::from(length), fc);
        for start in [0, 1, 7] {
            let mut bytes = [0_u8; 33];
            bytes[start / 8] |= 0x80 >> (start % 8);
            let (expected, position) = scalar_fallback(&table, &bytes, start);
            assert!(matches!(expected, Ok(HuffmanValue::Integer(23))));
            let mut reader = bit_reader(&bytes);
            reader.bit_position = start;
            assert_result(table.decode(&mut reader), &expected);
            assert_eq!(reader.bit_position, start + usize::from(length));
            assert_eq!(position, reader.bit_position);
            checked += 1;
        }
        table.set_test_count(usize::from(length), 0);
    }
    assert_eq!(checked, 30);
}
