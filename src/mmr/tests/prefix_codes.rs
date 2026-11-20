#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

fn scalar_run_code(reader: &mut BitReader<'_>, black: bool) -> Result<RunCode, MmrError> {
    let start = reader.bit_position();
    let terminating = if black {
        &BLACK_TERMINATING
    } else {
        &WHITE_TERMINATING
    };
    let makeup = if black { &BLACK_MAKEUP } else { &WHITE_MAKEUP };
    let mut bits = 0_u16;
    for length in 1..=MAX_RUN_CODE_BITS {
        bits = (bits << 1) | u16::try_from(reader.read_bits(1)?).unwrap();
        if let Some(code) = terminating
            .iter()
            .chain(makeup)
            .chain(&COMMON_MAKEUP)
            .find(|code| code.length == length && code.bits == bits)
        {
            return Ok(*code);
        }
    }
    Err(MmrError::InvalidRunCode {
        bit_offset: start,
        black,
    })
}

#[test]
fn indexed_exact_keys_match_canonical_tables_and_reject_out_of_range_keys() {
    for black in [false, true] {
        let index = if black {
            &super::super::tables::BLACK_RUN_INDEX
        } else {
            &super::super::tables::WHITE_RUN_INDEX
        };
        let terminating = if black {
            &BLACK_TERMINATING
        } else {
            &WHITE_TERMINATING
        };
        let makeup = if black { &BLACK_MAKEUP } else { &WHITE_MAKEUP };
        for length in 0..=MAX_RUN_CODE_BITS {
            for bits in 0..(1_u16 << length) {
                let expected = terminating
                    .iter()
                    .chain(makeup)
                    .chain(&COMMON_MAKEUP)
                    .find(|code| code.length == length && code.bits == bits);
                assert_eq!(
                    find_run_code(black, bits, length, index)
                        .map(|code| { (code.bits, code.length, code.value, code.terminal) }),
                    expected.map(|code| { (code.bits, code.length, code.value, code.terminal) }),
                    "colour {black}, key {bits}/{length}",
                );
            }
            for bits in [1_u16 << length, u16::MAX] {
                assert!(find_run_code(black, bits, length, index).is_none());
            }
        }
        for length in [MAX_RUN_CODE_BITS + 1, 31, u8::MAX] {
            assert!(find_run_code(black, 0, length, index).is_none());
            assert!(find_run_code(black, u16::MAX, length, index).is_none());
        }
    }
}

#[test]
fn indexed_ordinal_validation_rejects_bad_keys_and_out_of_table_entries() {
    for black in [false, true] {
        let terminating = if black {
            &BLACK_TERMINATING
        } else {
            &WHITE_TERMINATING
        };
        let makeup = if black { &BLACK_MAKEUP } else { &WHITE_MAKEUP };
        let codes: Vec<_> = terminating
            .iter()
            .chain(makeup)
            .chain(&COMMON_MAKEUP)
            .collect();
        for ordinal in 0..=u8::MAX {
            let mut index = [0_u8; super::super::tables::RUN_INDEX_SIZE];
            let expected = ordinal
                .checked_sub(1)
                .and_then(|index| codes.get(usize::from(index)));
            if let Some(expected) = expected {
                let key = usize::from(expected.length) * 256 + usize::from(expected.bits);
                index[key] = ordinal;
                let actual = find_run_code(black, expected.bits, expected.length, &index).unwrap();
                assert_eq!(
                    (actual.bits, actual.length, actual.value, actual.terminal),
                    (
                        expected.bits,
                        expected.length,
                        expected.value,
                        expected.terminal
                    ),
                );
                index.fill(0);
                let wrong_bits = expected.bits ^ 1;
                index[usize::from(expected.length) * 256 + usize::from(wrong_bits)] = ordinal;
                assert!(find_run_code(black, wrong_bits, expected.length, &index,).is_none());
                index.fill(0);
                let wrong_length = expected.length + 1;
                let wrong_key = usize::from(wrong_length) * 256 + usize::from(expected.bits);
                if let Some(entry) = index.get_mut(wrong_key) {
                    *entry = ordinal;
                }
                assert!(find_run_code(black, expected.bits, wrong_length, &index,).is_none());
            } else {
                for (bits, length) in [(0, 0), (1, 1)] {
                    index[usize::from(length) * 256 + usize::from(bits)] = ordinal;
                    assert!(find_run_code(black, bits, length, &index).is_none());
                }
                assert!(find_run_code(black, u16::MAX, u8::MAX, &index).is_none());
            }
        }
    }
}

#[test]
fn indexed_lookup_preserves_scalar_short_tail_cursor_and_typed_eof() {
    for black in [false, true] {
        for width in 0..MAX_RUN_CODE_BITS {
            for tail in 0..(1_u16 << width) {
                let bytes = tail.to_be_bytes();
                let start = 16 - usize::from(width);
                let mut actual = BitReader {
                    bytes: &bytes,
                    bit_position: start,
                };
                let mut scalar = actual;
                match (
                    decode_run_code(&mut actual, black),
                    scalar_run_code(&mut scalar, black),
                ) {
                    (Ok(actual), Ok(expected)) => assert_eq!(
                        (actual.bits, actual.length, actual.value, actual.terminal),
                        (
                            expected.bits,
                            expected.length,
                            expected.value,
                            expected.terminal
                        ),
                    ),
                    (Err(actual), Err(expected)) => {
                        assert_eq!(horizontal_error(actual), horizontal_error(expected));
                    }
                    (actual, expected) => panic!("tail result differs: {actual:?}/{expected:?}"),
                }
                assert_eq!(actual.bit_position(), scalar.bit_position());
                assert_eq!(actual.bytes.as_ptr(), bytes.as_ptr());
            }
        }
        let mut empty = bit_reader(&[]);
        let mut scalar = empty;
        assert_eq!(
            horizontal_error(decode_run_code(&mut empty, black).unwrap_err()),
            horizontal_error(scalar_run_code(&mut scalar, black).unwrap_err()),
        );
        assert_eq!(empty.bit_position(), 0);
    }
}

#[test]
fn every_mode_window_and_alignment_matches_scalar_prefix_lookup() {
    for window in 0..(1_u16 << MAX_MODE_CODE_BITS) {
        let expected = MODE_CODES
            .iter()
            .filter(|code| window >> (MAX_MODE_CODE_BITS - code.length) == code.bits)
            .min_by_key(|code| code.length);
        for start in 0..8 {
            let encoded = prefix_window_bytes(window, MAX_MODE_CODE_BITS, start);
            let mut reader = BitReader {
                bytes: &encoded,
                bit_position: start,
            };
            if let Some(code) = expected {
                assert_eq!(decode_mode(&mut reader).unwrap(), code.mode);
                assert_eq!(reader.bit_position(), start + usize::from(code.length));
                let packed = u64::from_be_bytes(encoded);
                let bits = (packed >> (64 - reader.bit_position())) & ((1_u64 << code.length) - 1);
                assert_eq!(bits, u64::from(code.bits));
            } else {
                assert_kind(
                    decode_mode(&mut reader).unwrap_err(),
                    MmrErrorKind::InvalidMode { bit_offset: start },
                );
                assert_eq!(
                    reader.bit_position(),
                    start + usize::from(MAX_MODE_CODE_BITS)
                );
            }
            assert_eq!(reader.bytes.as_ptr(), encoded.as_ptr());
        }
    }
}

#[test]
fn every_run_window_colour_and_alignment_matches_scalar_prefix_lookup() {
    for black in [false, true] {
        let terminating = if black {
            &BLACK_TERMINATING
        } else {
            &WHITE_TERMINATING
        };
        let makeup = if black { &BLACK_MAKEUP } else { &WHITE_MAKEUP };
        for window in 0..(1_u16 << MAX_RUN_CODE_BITS) {
            let expected = terminating
                .iter()
                .chain(makeup)
                .chain(&COMMON_MAKEUP)
                .filter(|code| window >> (MAX_RUN_CODE_BITS - code.length) == code.bits)
                .min_by_key(|code| code.length);
            for start in 0..8 {
                let encoded = prefix_window_bytes(window, MAX_RUN_CODE_BITS, start);
                let mut reader = BitReader {
                    bytes: &encoded,
                    bit_position: start,
                };
                if let Some(code) = expected {
                    let actual = decode_run_code(&mut reader, black).unwrap();
                    assert_eq!(
                        (actual.bits, actual.length, actual.value, actual.terminal),
                        (code.bits, code.length, code.value, code.terminal)
                    );
                    assert_eq!(reader.bit_position(), start + usize::from(code.length));
                    let packed = u64::from_be_bytes(encoded);
                    let bits =
                        (packed >> (64 - reader.bit_position())) & ((1_u64 << actual.length) - 1);
                    assert_eq!(bits, u64::from(actual.bits));
                } else {
                    assert_kind(
                        decode_run_code(&mut reader, black).unwrap_err(),
                        MmrErrorKind::InvalidRunCode {
                            bit_offset: start,
                            black,
                        },
                    );
                    assert_eq!(
                        reader.bit_position(),
                        start + usize::from(MAX_RUN_CODE_BITS)
                    );
                }
                assert_eq!(reader.bytes.as_ptr(), encoded.as_ptr());
            }
        }
    }
}

#[test]
fn code_constructors_preserve_raw_fields_and_terminal_sense() {
    for bits in [0, 1, u16::MAX] {
        for length in [0, 1, MAX_RUN_CODE_BITS, u8::MAX] {
            for mode in [
                Mode::Pass,
                Mode::Horizontal,
                Mode::Extension,
                Mode::Vertical(i8::MIN),
                Mode::Vertical(0),
                Mode::Vertical(i8::MAX),
            ] {
                let code = ModeCode::new(bits, length, mode);
                assert_eq!((code.bits, code.length, code.mode), (bits, length, mode));
            }
            for value in [0, 63, 64, u16::MAX] {
                let terminal = RunCode::terminal(value, bits, length);
                let makeup = RunCode::makeup(value, bits, length);
                assert_eq!(
                    (
                        terminal.bits,
                        terminal.length,
                        terminal.value,
                        terminal.terminal
                    ),
                    (bits, length, value, true)
                );
                assert_eq!(
                    (makeup.bits, makeup.length, makeup.value, makeup.terminal),
                    (bits, length, value, false)
                );
            }
        }
    }
}

#[test]
fn sequential_prefix_checks_equal_prior_closure_at_every_u16_value() {
    for prefix in 0..=u16::MAX {
        for bit in [0, 1, 2, u16::MAX] {
            let prior = prefix
                .checked_mul(2)
                .and_then(|value| value.checked_add(bit));
            let expanded = append_code_bit(prefix, bit).ok();
            assert_eq!(expanded, prior);
        }
    }
}

#[test]
fn all_normative_run_codes_are_unique_per_colour() {
    fn assert_unique(codes: &[RunCode]) {
        for (index, left) in codes.iter().enumerate() {
            for right in &codes[index + 1..] {
                assert!(
                    left.length != right.length || left.bits != right.bits,
                    "duplicate run code"
                );
            }
        }
    }

    let mut white = Vec::new();
    white.extend_from_slice(&WHITE_TERMINATING);
    white.extend_from_slice(&WHITE_MAKEUP);
    white.extend_from_slice(&COMMON_MAKEUP);
    assert_unique(&white);

    let mut black = Vec::new();
    black.extend_from_slice(&BLACK_TERMINATING);
    black.extend_from_slice(&BLACK_MAKEUP);
    black.extend_from_slice(&COMMON_MAKEUP);
    assert_unique(&black);
}

#[test]
fn every_mode_table_entry_decodes() {
    for expected in MODE_CODES {
        let shift = u32::from(16_u8 - expected.length);
        let encoded = expected
            .bits
            .checked_shl(shift)
            .expect("mode code fits two test bytes")
            .to_be_bytes();
        let mut reader = bit_reader(&encoded);
        assert_eq!(
            decode_mode(&mut reader).expect("table mode must decode"),
            expected.mode
        );
        assert_eq!(reader.bit_position(), usize::from(expected.length));
    }
}

#[test]
fn every_run_table_entry_decodes_for_its_colour() {
    fn assert_codes_decode(codes: &[RunCode], black: bool) {
        for expected in codes {
            let shift = u32::from(16_u8 - expected.length);
            let encoded = expected
                .bits
                .checked_shl(shift)
                .expect("run code fits two test bytes")
                .to_be_bytes();
            let mut reader = bit_reader(&encoded);
            let actual = decode_run_code(&mut reader, black).expect("table run must decode");
            assert_eq!(actual.bits, expected.bits);
            assert_eq!(actual.length, expected.length);
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.terminal, expected.terminal);
            assert_eq!(reader.bit_position(), usize::from(expected.length));
        }
    }

    assert_codes_decode(&WHITE_TERMINATING, false);
    assert_codes_decode(&WHITE_MAKEUP, false);
    assert_codes_decode(&COMMON_MAKEUP, false);
    assert_codes_decode(&BLACK_TERMINATING, true);
    assert_codes_decode(&BLACK_MAKEUP, true);
    assert_codes_decode(&COMMON_MAKEUP, true);
}
