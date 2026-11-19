#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

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
