use super::*;
use crate::huffman::{bit_reader, HuffmanLine, LineKind};

const TABLES: [StandardHuffmanTable; 15] = [
    StandardHuffmanTable::A,
    StandardHuffmanTable::B,
    StandardHuffmanTable::C,
    StandardHuffmanTable::D,
    StandardHuffmanTable::E,
    StandardHuffmanTable::F,
    StandardHuffmanTable::G,
    StandardHuffmanTable::H,
    StandardHuffmanTable::I,
    StandardHuffmanTable::J,
    StandardHuffmanTable::K,
    StandardHuffmanTable::L,
    StandardHuffmanTable::M,
    StandardHuffmanTable::N,
    StandardHuffmanTable::O,
];

fn compare(desc: &StandardDescriptor, owned: &HuffmanTable, bytes: &[u8], start: usize) {
    let mut actual = bit_reader(bytes);
    actual.bit_position = start;
    let mut expected = actual;
    assert_eq!(
        format!("{:?}", desc.decode(&mut actual)),
        format!("{:?}", owned.decode(&mut expected)),
    );
    assert_eq!(actual.bit_position, expected.bit_position);
    assert_eq!(actual.bytes.as_ptr(), bytes.as_ptr());
    let mut actual_final = bit_reader(bytes);
    actual_final.bit_position = start;
    let mut expected_final = actual_final;
    assert_eq!(
        format!("{:?}", desc.decode_final_strip_value(&mut actual_final)),
        format!("{:?}", owned.decode_final_strip_value(&mut expected_final)),
    );
    assert_eq!(actual_final.bit_position, expected_final.bit_position);
    assert_eq!(
        desc.zero_padding_before_prefix(&actual).unwrap(),
        owned.zero_padding_before_prefix(&expected).unwrap(),
    );
    assert_eq!(
        format!("{:?}", desc.decode(&mut actual)),
        format!("{:?}", owned.decode(&mut expected)),
    );
    assert_eq!(actual.bit_position, expected.bit_position);
}

fn bits_for_line(owned: &HuffmanTable, index: usize, suffix: u32) -> Vec<bool> {
    let (prefix, width) = owned.code_for_line(index).unwrap();
    let line = owned.lines[index];
    let mut bits = (0..width)
        .rev()
        .map(|bit| prefix.limbs[0] >> bit & 1 != 0)
        .collect::<Vec<_>>();
    if line.kind != LineKind::OutOfBand {
        bits.extend((0..line.range_len).rev().map(|bit| suffix >> bit & 1 != 0));
    }
    bits
}

fn packed(bits: &[bool], start: usize, available: usize) -> Vec<u8> {
    let mut bytes = vec![0; (start + available).div_ceil(8)];
    for (index, bit) in bits.iter().take(available).enumerate() {
        if *bit {
            bytes[(start + index) / 8] |= 1 << (7 - (start + index) % 8);
        }
    }
    bytes
}

#[test]
fn all_standard_descriptors_match_canonical_metadata_and_every_prefix() {
    for tag in TABLES {
        let owned = HuffmanTable::standard(tag, &Limits::default()).unwrap();
        let desc = StandardDescriptor::new(tag, &Limits::default()).unwrap();
        for (index, line) in owned.lines.iter().enumerate() {
            assert_eq!(desc.lines.line(index), Some(*line));
        }
        assert_eq!(desc.max_prefix_len, owned.max_prefix_len);
        for length in 0..10 {
            assert_eq!(
                u32::from(desc.length_count[length]),
                owned.length_count[length]
            );
            if length <= usize::from(desc.max_prefix_len) {
                assert_eq!(
                    WideCode {
                        limbs: [u64::from(desc.first_code[length]), 0, 0, 0]
                    },
                    owned.first_code[length],
                );
            }
            if desc.length_count[length] > 0 {
                assert_eq!(u32::from(desc.first_line[length]), owned.first_line[length]);
            }
        }
        assert_eq!(
            desc.ordered_lines[..desc.lines.len()]
                .iter()
                .map(|v| u32::from(*v))
                .collect::<Vec<_>>(),
            owned.ordered_lines,
        );
        for prefix in 0..512_u16 {
            let bytes = (prefix << 7).to_be_bytes();
            for start in 0..=bytes.len() * 8 {
                compare(desc, &owned, &bytes, start);
            }
        }
    }
}

#[test]
fn all_lines_preserve_exact_truncation_suffix_boundaries_and_continuation() {
    for tag in TABLES {
        let owned = HuffmanTable::standard(tag, &Limits::default()).unwrap();
        let desc = StandardDescriptor::new(tag, &Limits::default()).unwrap();
        for (index, line) in owned.lines.iter().enumerate() {
            let maximum = u32::MAX >> (32 - u32::from(line.range_len)).min(31);
            for suffix in [0, 1, maximum, i32::MAX as u32, 0x8000_0000, u32::MAX] {
                let bits = bits_for_line(&owned, index, suffix);
                for available in 0..bits.len() {
                    let start = (8 - available % 8) % 8;
                    let bytes = packed(&bits, start, available);
                    compare(desc, &owned, &bytes, start);
                }
                for start in 0..8 {
                    let bytes = packed(&bits, start, bits.len());
                    compare(desc, &owned, &bytes, start);
                    let mut reader = bit_reader(&bytes);
                    reader.bit_position = start;
                    let result = desc.decode(&mut reader);
                    assert_eq!(reader.bit_position, start + bits.len());
                    check_value(*line, suffix, result);
                }
            }
        }
    }
}

fn check_value(line: HuffmanLine, suffix: u32, actual: Result<HuffmanValue, HuffmanError>) {
    if line.kind == LineKind::OutOfBand {
        assert_eq!(actual.unwrap(), HuffmanValue::OutOfBand);
        return;
    }
    let mask = if line.range_len == 32 {
        u32::MAX
    } else {
        (1 << line.range_len) - 1
    };
    let offset = i64::from(suffix & mask);
    let value = if line.kind == LineKind::Lower {
        line.range_low - offset
    } else {
        line.range_low + offset
    };
    match i32::try_from(value) {
        Ok(expected) => assert_eq!(actual.unwrap(), HuffmanValue::Integer(expected)),
        Err(_) => assert!(matches!(
            actual,
            Err(HuffmanError::Decode(DecodeError::Malformed(
                MalformedError::IntegerOverflow
            ))),
        )),
    }
}

#[test]
fn descriptor_limits_charge_logical_lines_but_no_removed_owned_memory() {
    for tag in TABLES {
        let count = HuffmanTable::standard(tag, &Limits::default())
            .unwrap()
            .lines
            .len();
        for max_symbols in [
            0,
            u32::try_from(count - 1).unwrap(),
            u32::try_from(count).unwrap(),
        ] {
            let limits = Limits {
                max_symbols,
                max_decoder_managed_memory_bytes: 0,
                ..Limits::default()
            };
            let result = StandardDescriptor::new(tag, &limits);
            if max_symbols as usize >= count {
                assert!(result.is_ok());
                assert!(matches!(
                    HuffmanTable::standard(tag, &limits),
                    Err(HuffmanError::Decode(DecodeError::LimitExceeded(
                        LimitExceeded {
                            kind: LimitKind::Memory,
                            ..
                        }
                    ))),
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(HuffmanError::Decode(DecodeError::LimitExceeded(LimitExceeded {
                        kind: LimitKind::Symbols, limit, actual,
                    }))) if limit == u64::from(max_symbols) && actual == count as u64,
                ));
            }
        }
    }
}
