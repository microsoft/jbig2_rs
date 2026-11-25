#![cfg(test)]

use super::*;

#[derive(Clone, Copy)]
enum Family {
    TwoBit,
    EightBit,
    SignedAndOob,
    Range31,
}

const FAMILIES: [Family; 4] = [
    Family::TwoBit,
    Family::EightBit,
    Family::SignedAndOob,
    Family::Range31,
];
const SAMPLES: [&[u8]; 6] = [
    &[],
    &[0],
    &[0xff],
    &[0x7f, 0xff, 0xff, 0xff],
    &[0x03, 0x55, 0xff, 0xff, 0xff, 0xff],
    &[0xff; 8],
];

fn table(family: Family) -> HuffmanTable {
    let lines = match family {
        Family::TwoBit | Family::EightBit => (0..4)
            .map(|id| {
                HuffmanLine::normal(
                    id,
                    if matches!(family, Family::TwoBit) {
                        2
                    } else {
                        8
                    },
                    0,
                )
            })
            .collect::<Vec<_>>(),
        Family::SignedAndOob => vec![HuffmanLine::normal(-1, 1, 1), HuffmanLine::out_of_band(1)],
        Family::Range31 => vec![HuffmanLine::normal(0, 1, 31)],
    };
    HuffmanTable::new(&lines, &Limits::default()).unwrap()
}

fn fixture(bytes: &[u8], position: usize, family: Family) -> HuffmanCodec<'_> {
    let mut codec = super::huffman_fields::codec(bytes, position, 2);
    codec.symbol_id_table = Some(table(family));
    codec.tables.aggregation_instances =
        Some(MaterializedHuffmanTable::Standard(StandardHuffmanTable::A));
    codec.refinement =
        Some(RefinementContexts::new(RefinementTemplate::Template1, &Limits::default()).unwrap());
    codec
}

fn metadata(codec: &HuffmanCodec<'_>) -> String {
    format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
        [
            codec.tables.aggregation_instances,
            codec.tables.height_delta,
            codec.tables.width_delta,
            codec.tables.export_run,
            codec.tables.strip_delta,
            codec.tables.first_s,
            codec.tables.subsequent_s,
            codec.tables.refinement_width,
            codec.tables.refinement_height,
            codec.tables.refinement_x,
            codec.tables.refinement_y,
            codec.tables.refinement_size,
            codec.tables.dictionary_bitmap_size,
        ],
        [
            &codec.tables.standard_tables.a,
            &codec.tables.standard_tables.b,
            &codec.tables.standard_tables.c,
            &codec.tables.standard_tables.d,
            &codec.tables.standard_tables.e,
            &codec.tables.standard_tables.f,
            &codec.tables.standard_tables.g,
            &codec.tables.standard_tables.h,
            &codec.tables.standard_tables.i,
            &codec.tables.standard_tables.j,
            &codec.tables.standard_tables.k,
            &codec.tables.standard_tables.l,
            &codec.tables.standard_tables.m,
            &codec.tables.standard_tables.n,
            &codec.tables.standard_tables.o,
        ],
        codec.log_strip_size,
        codec.symbol_id_table,
        codec.refinement,
        codec.refinement_work,
    )
}

fn oracle(bytes: &[u8], position: usize, family: Family) -> (Result<u32, SymbolTextError>, usize) {
    let prefix_length = match family {
        Family::TwoBit => 2,
        Family::EightBit => 8,
        Family::SignedAndOob | Family::Range31 => 1,
    };
    let remaining = bytes.len() * 8 - position;
    if remaining < usize::from(prefix_length) {
        return (
            Err(SymbolTextError::Decode(DecodeError::Malformed(
                MalformedError::UnexpectedEof {
                    offset: u64::try_from(bytes.len() * 8).unwrap(),
                    needed: 1,
                    remaining: 0,
                },
            ))),
            bytes.len() * 8,
        );
    }
    let (prefix, after_prefix) =
        super::huffman_fields::scalar(bytes, position, prefix_length).unwrap();
    match family {
        Family::TwoBit | Family::EightBit if prefix < 4 => (Ok(prefix), after_prefix),
        Family::SignedAndOob if prefix == 1 => (
            Err(SymbolTextError::UnexpectedOutOfBand("symbol ID")),
            after_prefix,
        ),
        Family::SignedAndOob | Family::Range31 if prefix == 0 => {
            let width = if matches!(family, Family::Range31) {
                31
            } else {
                1
            };
            match super::huffman_fields::scalar(bytes, after_prefix, width) {
                Ok((value, next)) => (
                    if matches!(family, Family::SignedAndOob) {
                        if value == 0 {
                            Err(SymbolTextError::InvalidEntropyValue("symbol ID"))
                        } else {
                            Ok(0)
                        }
                    } else {
                        Ok(value)
                    },
                    next,
                ),
                Err(error) => (
                    Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                    after_prefix,
                ),
            }
        }
        _ => (
            Err(SymbolTextError::Decode(DecodeError::Malformed(
                MalformedError::InvalidHuffman(crate::HuffmanErrorKind::InvalidCode),
            ))),
            after_prefix,
        ),
    }
}

fn assert_result(actual: Result<u32, SymbolTextError>, expected: Result<u32, SymbolTextError>) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (
            Err(SymbolTextError::Decode(DecodeError::Malformed(actual))),
            Err(SymbolTextError::Decode(DecodeError::Malformed(expected))),
        ) => assert_eq!(actual, expected),
        (
            Err(SymbolTextError::InvalidEntropyValue(actual)),
            Err(SymbolTextError::InvalidEntropyValue(expected)),
        )
        | (
            Err(SymbolTextError::UnexpectedOutOfBand(actual)),
            Err(SymbolTextError::UnexpectedOutOfBand(expected)),
        )
        | (
            Err(SymbolTextError::InvalidCount(actual)),
            Err(SymbolTextError::InvalidCount(expected)),
        ) => assert_eq!(actual, expected),
        (
            Err(SymbolTextError::SymbolIdOutOfRange { id, available }),
            Err(SymbolTextError::SymbolIdOutOfRange {
                id: expected_id,
                available: expected_available,
            }),
        ) => assert_eq!((id, available), (expected_id, expected_available)),
        (actual, expected) => panic!("decoded ID {actual:?}, expected {expected:?}"),
    }
}

fn assert_frame(codec: &mut HuffmanCodec<'_>, bytes: &[u8], end: usize, before: &str) {
    assert_eq!(codec.reader.bytes.as_ptr(), bytes.as_ptr());
    assert_eq!(codec.reader.bytes, bytes);
    assert_eq!(codec.reader.bit_position, end);
    assert_eq!(metadata(codec), before);
    let width = u8::try_from((bytes.len() * 8 - end).min(8)).unwrap();
    let (value, next) = super::huffman_fields::scalar(bytes, end, width).unwrap();
    assert_result(codec.read_raw_bits(width), Ok(value));
    assert_eq!(codec.reader.bit_position, next);
    assert_eq!(metadata(codec), before);
}

#[test]
fn table_ids_preserve_scalar_prefix_range_errors_cursor_and_metadata() {
    for family in FAMILIES {
        for bytes in SAMPLES {
            for position in 0..=bytes.len() * 8 {
                for code_length in [0, 1, 8, 32, 255] {
                    let request = SymbolIdRequest {
                        huffman: code_length != 0,
                        code_length,
                        available_symbols: u32::MAX,
                    };
                    let mut actual = fixture(bytes, position, family);
                    let before = metadata(&actual);
                    let (expected, end) = oracle(bytes, position, family);
                    assert_result(actual.decode_symbol_id(request), expected);
                    assert_frame(&mut actual, bytes, end, &before);
                }
            }
        }
    }
}

#[test]
fn table_ids_enum_uses_table_not_literal_width_and_preserves_frames() {
    for family in FAMILIES {
        for bytes in SAMPLES {
            for position in 0..=bytes.len() * 8 {
                for code_length in [0, 1, 32, 255] {
                    let concrete = fixture(bytes, position, family);
                    let before = metadata(&concrete);
                    let mut actual = SymbolCodec::Huffman(concrete);
                    let (expected, end) = oracle(bytes, position, family);
                    assert_result(
                        actual.decode_symbol_id(SymbolIdRequest {
                            huffman: code_length != 0,
                            code_length,
                            available_symbols: u32::MAX,
                        }),
                        expected,
                    );
                    let SymbolCodec::Huffman(ref mut after) = actual else {
                        panic!("changed codec variant");
                    };
                    assert_frame(after, bytes, end, &before);
                }
            }
        }
    }
}

#[test]
fn checked_table_ids_preserve_values_rejections_errors_and_consumption() {
    for family in FAMILIES {
        for bytes in SAMPLES {
            for position in 0..=bytes.len() * 8 {
                for available_symbols in [0, 1, 3, u32::MAX] {
                    let concrete = fixture(bytes, position, family);
                    let before = metadata(&concrete);
                    let mut actual = SymbolCodec::Huffman(concrete);
                    let (decoded, consumed) = oracle(bytes, position, family);
                    let (expected, end) = if available_symbols == 0 {
                        (
                            Err(SymbolTextError::InvalidCount(
                                "symbol ID with no available symbols",
                            )),
                            position,
                        )
                    } else {
                        (
                            match decoded {
                                Ok(id) if id >= available_symbols => {
                                    Err(SymbolTextError::SymbolIdOutOfRange {
                                        id,
                                        available: available_symbols,
                                    })
                                }
                                result => result,
                            },
                            consumed,
                        )
                    };
                    assert_result(
                        crate::symbol_text::codec::decode_checked_symbol_id(
                            &mut actual,
                            SymbolIdRequest {
                                huffman: true,
                                code_length: 255,
                                available_symbols,
                            },
                        ),
                        expected,
                    );
                    let SymbolCodec::Huffman(ref mut after) = actual else {
                        panic!("changed codec variant");
                    };
                    assert_frame(after, bytes, end, &before);
                }
            }
        }
    }
}

#[test]
fn table_id_range_endpoints_zero_and_i32_max_are_exact() {
    for (bytes, value) in [([0; 4], 0), ([0x7f, 0xff, 0xff, 0xff], i32::MAX as u32)] {
        let mut actual = fixture(&bytes, 0, Family::Range31);
        let before = metadata(&actual);
        assert_result(
            actual.decode_symbol_id(SymbolIdRequest {
                code_length: 255,
                available_symbols: u32::MAX,
                huffman: false,
            }),
            Ok(value),
        );
        assert_frame(&mut actual, &bytes, 32, &before);
    }
}

#[test]
fn standard_cache_lookup_preserves_each_slot_presence_and_contents() {
    let mut cache = StandardHuffmanTables::empty();
    for tag in [
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
    ] {
        assert!(cache.table(tag).is_none());
        cache.materialize(tag, &Limits::default()).unwrap();
    }
    for (tag, slot) in [
        (StandardHuffmanTable::A, &cache.a),
        (StandardHuffmanTable::B, &cache.b),
        (StandardHuffmanTable::C, &cache.c),
        (StandardHuffmanTable::D, &cache.d),
        (StandardHuffmanTable::E, &cache.e),
        (StandardHuffmanTable::F, &cache.f),
        (StandardHuffmanTable::G, &cache.g),
        (StandardHuffmanTable::H, &cache.h),
        (StandardHuffmanTable::I, &cache.i),
        (StandardHuffmanTable::J, &cache.j),
        (StandardHuffmanTable::K, &cache.k),
        (StandardHuffmanTable::L, &cache.l),
        (StandardHuffmanTable::M, &cache.m),
        (StandardHuffmanTable::N, &cache.n),
        (StandardHuffmanTable::O, &cache.o),
    ] {
        assert!(core::ptr::eq(
            cache.table(tag).unwrap(),
            slot.as_deref().unwrap()
        ));
    }
}

fn bind_integer_table<'a>(
    tables: &mut HuffmanFieldTables<'a>,
    field: IntegerField,
    binding: Option<MaterializedHuffmanTable<'a>>,
) {
    let slot = match field {
        IntegerField::AggregationInstances => &mut tables.aggregation_instances,
        IntegerField::HeightDelta => &mut tables.height_delta,
        IntegerField::WidthDelta => &mut tables.width_delta,
        IntegerField::ExportRun => &mut tables.export_run,
        IntegerField::StripDelta => &mut tables.strip_delta,
        IntegerField::FirstS => &mut tables.first_s,
        IntegerField::SubsequentS => &mut tables.subsequent_s,
        IntegerField::RefinementWidth => &mut tables.refinement_width,
        IntegerField::RefinementHeight => &mut tables.refinement_height,
        IntegerField::RefinementX => &mut tables.refinement_x,
        IntegerField::RefinementY => &mut tables.refinement_y,
        IntegerField::RefinementSize => &mut tables.refinement_size,
        IntegerField::DictionaryBitmapSize => &mut tables.dictionary_bitmap_size,
        _ => panic!("literal field has no table slot"),
    };
    *slot = binding;
}

#[derive(Clone, Copy)]
enum IntegerBinding {
    Missing,
    Custom,
    MissingStandard,
    Standard,
}

fn standard_family(family: Family) -> StandardHuffmanTable {
    match family {
        Family::TwoBit => StandardHuffmanTable::N,
        Family::EightBit => StandardHuffmanTable::I,
        Family::SignedAndOob => StandardHuffmanTable::B,
        Family::Range31 => StandardHuffmanTable::A,
    }
}

fn standard_integer_oracle(
    bytes: &[u8],
    position: usize,
    tag: StandardHuffmanTable,
) -> (Result<EntropyInteger, SymbolTextError>, usize) {
    let table = HuffmanTable::standard(tag, &Limits::default()).unwrap();
    let mut reader = BitReader {
        bytes,
        bit_position: position,
    };
    let result = table
        .decode(&mut reader)
        .map(|value| match value {
            HuffmanValue::Integer(value) => EntropyInteger::Value(i64::from(value)),
            HuffmanValue::OutOfBand => EntropyInteger::OutOfBand,
        })
        .map_err(SymbolTextError::from);
    (result, reader.bit_position)
}

#[test]
fn table_integer_fields_preserve_selection_results_errors_cursor_and_frames() {
    use IntegerBinding as Binding;
    for field in [
        IntegerField::AggregationInstances,
        IntegerField::HeightDelta,
        IntegerField::WidthDelta,
        IntegerField::ExportRun,
        IntegerField::StripDelta,
        IntegerField::FirstS,
        IntegerField::SubsequentS,
        IntegerField::RefinementWidth,
        IntegerField::RefinementHeight,
        IntegerField::RefinementX,
        IntegerField::RefinementY,
        IntegerField::RefinementSize,
        IntegerField::DictionaryBitmapSize,
    ] {
        for family in FAMILIES {
            let custom = table(family);
            for binding in [
                Binding::Missing,
                Binding::Custom,
                Binding::MissingStandard,
                Binding::Standard,
            ] {
                for bytes in SAMPLES {
                    for position in [0, bytes.len() * 4, bytes.len() * 8] {
                        for wrapped in [false, true] {
                            let mut actual = fixture(bytes, position, Family::EightBit);
                            bind_integer_table(
                                &mut actual.tables,
                                field,
                                match binding {
                                    Binding::Missing => None,
                                    Binding::Custom => {
                                        Some(MaterializedHuffmanTable::Custom(&custom))
                                    }
                                    Binding::MissingStandard | Binding::Standard => Some(
                                        MaterializedHuffmanTable::Standard(StandardHuffmanTable::A),
                                    ),
                                },
                            );
                            let standard_tag = standard_family(family);
                            if matches!(binding, Binding::Standard) {
                                actual.tables.standard_tables.a = Some(
                                    crate::huffman::standard::StandardDescriptor::new(
                                        standard_tag,
                                        &Limits::default(),
                                    )
                                    .unwrap(),
                                );
                            }
                            let (expected, end) =
                                if matches!(binding, Binding::Missing | Binding::MissingStandard) {
                                    (
                                        Err(SymbolTextError::InvalidTableAssociation(
                                            "no Huffman table bound for field",
                                        )),
                                        position,
                                    )
                                } else if matches!(binding, Binding::Standard) {
                                    // Deliberately store another standard descriptor in slot A:
                                    // selection must use supplied contents, not reconstruct its tag.
                                    standard_integer_oracle(bytes, position, standard_tag)
                                } else {
                                    let (decoded, end) = oracle(bytes, position, family);
                                    // The scalar ID oracle rejects the fixture's -1 and OOB;
                                    // integer fields must preserve those entropy values instead.
                                    (
                                        match decoded {
                                            Ok(value) => {
                                                Ok(EntropyInteger::Value(i64::from(value)))
                                            }
                                            Err(SymbolTextError::InvalidEntropyValue(
                                                "symbol ID",
                                            )) => Ok(EntropyInteger::Value(-1)),
                                            Err(SymbolTextError::UnexpectedOutOfBand(
                                                "symbol ID",
                                            )) => Ok(EntropyInteger::OutOfBand),
                                            Err(error) => Err(error),
                                        },
                                        end,
                                    )
                                };
                            let before = metadata(&actual);
                            let result = if wrapped {
                                let mut codec = SymbolCodec::Huffman(actual);
                                let result = codec.decode_integer(field);
                                let SymbolCodec::Huffman(codec) = codec else {
                                    panic!("changed codec variant");
                                };
                                actual = codec;
                                result
                            } else {
                                actual.decode_integer(field)
                            };
                            assert_eq!(format!("{result:?}"), format!("{expected:?}"));
                            assert_frame(&mut actual, bytes, end, &before);
                        }
                    }
                }
            }
        }
    }
}
