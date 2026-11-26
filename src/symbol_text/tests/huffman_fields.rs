#![cfg(test)]

use super::super::codec::decode_symbol_id_code_lengths;
use super::super::codec::new_huffman_codec;
use super::*;

const SAMPLES: [&[u8]; 5] = [
    &[],
    &[0],
    &[0xff],
    &[0xa5, 0x32, 0xdc, 0x4f, 0x80],
    &[0xff; 8],
];

pub(super) fn codec(bytes: &[u8], position: usize, log_strip_size: u8) -> HuffmanCodec<'_> {
    HuffmanCodec {
        reader: BitReader {
            bytes,
            bit_position: position,
        },
        tables: HuffmanFieldTables {
            standard_tables: StandardHuffmanTables::empty(),
            aggregation_instances: None,
            height_delta: None,
            width_delta: None,
            export_run: None,
            strip_delta: None,
            first_s: None,
            subsequent_s: None,
            refinement_width: None,
            refinement_height: None,
            refinement_x: None,
            refinement_y: None,
            refinement_size: None,
            dictionary_bitmap_size: None,
        },
        log_strip_size,
        symbol_id_table: None,
        refinement: None,
        refinement_work: 71,
    }
}

pub(super) fn scalar(
    bytes: &[u8],
    start: usize,
    count: u8,
) -> Result<(u32, usize), MalformedError> {
    if count > 32 {
        return Err(MalformedError::InvalidBitCount(count));
    }
    let remaining = bytes.len() * 8 - start;
    if usize::from(count) > remaining {
        return Err(MalformedError::UnexpectedEof {
            offset: u64::try_from(start).unwrap(),
            needed: u64::from(count),
            remaining: u64::try_from(remaining).unwrap(),
        });
    }
    let end = start + usize::from(count);
    let value = (start..end).fold(0_u32, |value, bit| {
        (value << 1) | u32::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
    });
    Ok((value, end))
}

fn result_matches(
    actual: Result<u32, SymbolTextError>,
    expected: &Result<(u32, usize), MalformedError>,
) {
    match (actual, expected) {
        (Ok(actual), Ok((value, _))) => assert_eq!(actual, *value),
        (Err(SymbolTextError::Decode(DecodeError::Malformed(actual))), Err(expected)) => {
            assert_eq!(&actual, expected);
        }
        (actual, expected) => panic!("literal result {actual:?}, expected {expected:?}"),
    }
}

fn integer_value(result: Result<EntropyInteger, SymbolTextError>) -> Result<u32, SymbolTextError> {
    result.map(|value| match value {
        EntropyInteger::Value(value) => u32::try_from(value).expect("literal unsigned value"),
        EntropyInteger::OutOfBand => panic!("literal field returned OOB"),
    })
}

pub(super) fn frame_and_continuation(
    actual: &mut HuffmanCodec<'_>,
    bytes: &[u8],
    start: usize,
    log_strip_size: u8,
    expected: &Result<(u32, usize), MalformedError>,
) {
    let position = match expected {
        Ok((_, position)) => *position,
        Err(_) => start,
    };
    assert_eq!(actual.reader.bytes, bytes);
    assert_eq!(actual.reader.bit_position, position);
    assert_eq!(actual.log_strip_size, log_strip_size);
    assert!(actual.symbol_id_table.is_none());
    assert!(actual.refinement.is_none());
    assert_eq!(actual.refinement_work, 71);
    let next_count = u8::try_from((bytes.len() * 8 - position).min(8)).unwrap();
    result_matches(
        actual.read_raw_bits(next_count),
        &scalar(bytes, position, next_count),
    );
}

#[test]
fn raw_huffman_bits_preserve_exact_value_cursor_error_frame_and_next_field() {
    let mut checked = 0;
    for bytes in SAMPLES {
        for start in 0..=bytes.len() * 8 {
            for count in 0..=u8::MAX {
                let expected = scalar(bytes, start, count);
                let mut actual = codec(bytes, start, count);
                result_matches(actual.read_raw_bits(count), &expected);
                frame_and_continuation(&mut actual, bytes, start, count, &expected);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 32_000);
}

#[test]
fn fixed_huffman_integer_fields_match_literal_bits_through_both_adapters() {
    let mut checked = 0;
    for bytes in SAMPLES {
        for start in 0..=bytes.len() * 8 {
            for log_strips in 0..=u8::MAX {
                for field in [
                    IntegerField::WithinStripT,
                    IntegerField::RefinementIndicator,
                ] {
                    let count = if matches!(field, IntegerField::WithinStripT) {
                        log_strips
                    } else {
                        1
                    };
                    let expected = scalar(bytes, start, count);
                    let mut direct = codec(bytes, start, log_strips);
                    result_matches(integer_value(direct.decode_integer(field)), &expected);
                    frame_and_continuation(&mut direct, bytes, start, log_strips, &expected);
                    let mut wrapped = SymbolCodec::Huffman(codec(bytes, start, log_strips));
                    result_matches(integer_value(wrapped.decode_integer(field)), &expected);
                    let SymbolCodec::Huffman(wrapped) = &mut wrapped else {
                        panic!("Huffman codec variant changed");
                    };
                    frame_and_continuation(wrapped, bytes, start, log_strips, &expected);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 64_000);
}

#[test]
fn literal_huffman_symbol_ids_match_all_widths_through_both_adapters() {
    let mut checked = 0;
    for bytes in SAMPLES {
        for start in 0..=bytes.len() * 8 {
            for count in 0..=u8::MAX {
                let expected = scalar(bytes, start, count);
                let request = SymbolIdRequest {
                    available_symbols: u32::MAX,
                    code_length: count,
                    huffman: true,
                };
                let mut direct = codec(bytes, start, 3);
                result_matches(direct.decode_symbol_id(request), &expected);
                frame_and_continuation(&mut direct, bytes, start, 3, &expected);
                let mut wrapped = SymbolCodec::Huffman(codec(bytes, start, 3));
                result_matches(wrapped.decode_symbol_id(request), &expected);
                let SymbolCodec::Huffman(wrapped) = &mut wrapped else {
                    panic!("Huffman codec variant changed");
                };
                frame_and_continuation(wrapped, bytes, start, 3, &expected);
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 32_000);
}

#[test]
fn symbol_length_literals_do_not_include_the_run_code_prefix_length() {
    let limits = Limits::default();
    for prefix_len in [1, 2, 7, 8, 15] {
        for literal in 0..=31 {
            let table =
                HuffmanTable::new(&[HuffmanLine::normal(literal, prefix_len, 0)], &limits).unwrap();
            let mut reader = BitReader {
                bytes: &[0; 8],
                bit_position: 0,
            };
            let lengths = decode_symbol_id_code_lengths(&mut reader, &table, 4, &limits).unwrap();
            assert_eq!(lengths, vec![u8::try_from(literal).unwrap(); 4]);
            assert_eq!(reader.bit_position, usize::from(prefix_len) * 4);
        }
    }
}

#[test]
fn zero_symbol_length_literal_excludes_the_symbol_from_the_codebook() {
    let limits = Limits::default();
    let run_codes = HuffmanTable::new(
        &[HuffmanLine::normal(0, 1, 0), HuffmanLine::normal(1, 1, 0)],
        &limits,
    )
    .unwrap();
    let mut reader = BitReader {
        bytes: &[0xc0],
        bit_position: 0,
    };
    let lengths = decode_symbol_id_code_lengths(&mut reader, &run_codes, 3, &limits).unwrap();
    assert_eq!(lengths, [1, 1, 0]);
    assert_eq!(reader.bit_position, 3);

    let symbols = HuffmanTable::new_symbol_ids(&lengths, &limits).unwrap();
    let mut reader = BitReader {
        bytes: &[0x40],
        bit_position: 0,
    };
    assert_eq!(
        symbols.decode(&mut reader).unwrap(),
        HuffmanValue::Integer(0)
    );
    assert_eq!(
        symbols.decode(&mut reader).unwrap(),
        HuffmanValue::Integer(1)
    );
    assert_eq!(reader.bit_position, 2);
}

#[test]
fn fresh_huffman_codec_preserves_input_strip_tables_and_requested_refinement() {
    let limits = Limits::default();
    let first = HuffmanTable::new(&[HuffmanLine::normal(17, 1, 0)], &limits).unwrap();
    let second = HuffmanTable::new(&[HuffmanLine::normal(29, 1, 0)], &limits).unwrap();
    for bytes in SAMPLES {
        for log in 0..=u8::MAX {
            for template in [
                None,
                Some(RefinementTemplate::Template0),
                Some(RefinementTemplate::Template1),
            ] {
                let mut tables = codec(&[], 0, 0).tables;
                tables.first_s = Some(MaterializedHuffmanTable::Custom(&first));
                tables.height_delta = Some(MaterializedHuffmanTable::Custom(&second));
                let mut fresh = new_huffman_codec(bytes, tables, log, template, &limits).unwrap();
                assert_eq!(fresh.reader.bytes, bytes);
                assert_eq!(fresh.reader.bit_position, 0);
                assert_eq!(fresh.log_strip_size, log);
                assert!(fresh.symbol_id_table.is_none());
                assert_eq!(fresh.refinement_work, 0);
                for (field, expected) in [
                    (IntegerField::FirstS, &first),
                    (IntegerField::HeightDelta, &second),
                ] {
                    assert!(core::ptr::eq(
                        fresh.tables.table_for(field).unwrap(),
                        expected
                    ));
                }
                match (template, fresh.refinement.as_ref()) {
                    (None, None) => {}
                    (Some(expected), Some(actual)) => {
                        assert_eq!(actual.template, expected);
                        let count = match expected {
                            RefinementTemplate::Template0 => 8192,
                            RefinementTemplate::Template1 => 1024,
                        };
                        assert_eq!(actual.contexts.len(), count);
                        assert!(actual
                            .contexts
                            .iter()
                            .all(|context| { context.state_index() == 0 && context.mps() == 0 }));
                    }
                    _ => panic!("constructor changed requested refinement presence"),
                }
                let count = u8::try_from((bytes.len() * 8).min(8)).unwrap();
                result_matches(fresh.read_raw_bits(count), &scalar(bytes, 0, count));
            }
        }
    }
}

#[test]
fn actual_huffman_segment_setup_preserves_requested_refinement_allocation() {
    let requested = 8192 * 2_u64;
    let limits = Limits {
        max_decoder_managed_memory_bytes: requested - 1,
        ..Limits::default()
    };
    let mut text = vec![0_u8; 17];
    text[3] = 1;
    text[7] = 1;
    text.extend_from_slice(&3_u16.to_be_bytes());
    text.extend_from_slice(&0_u16.to_be_bytes());
    text.extend_from_slice(&[0xff; 4]);
    text.extend_from_slice(&0_u32.to_be_bytes());
    text.push(0);
    let mut dictionary = 3_u16.to_be_bytes().to_vec();
    dictionary.extend_from_slice(&[0xff; 4]);
    dictionary.extend_from_slice(&[0; 8]);
    dictionary.push(0);
    let text_result = decode_text_region(&TextRegionInput {
        allow_missing_final_huffman_oob: false,
        allow_missing_final_arithmetic_oob: false,
        payload: &text,
        symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &[],
        limits: &limits,
    });
    let dictionary_result = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &dictionary,
        imported_symbols: SymbolCodebook::Contiguous(&[]),
        custom_tables: &[],
        used_contexts: None,
        limits: &limits,
    });
    for result in [
        text_result.map(|output| output.consumed_bytes),
        dictionary_result.map(|output| output.consumed_bytes),
    ] {
        assert!(
            matches!(
                result,
                Err(SymbolTextError::Decode(DecodeError::LimitExceeded(LimitExceeded {
                    kind: LimitKind::Memory,
                    limit,
                    actual,
                }))) if limit == requested - 1 && actual == requested
            ),
            "{result:?}"
        );
    }
}
