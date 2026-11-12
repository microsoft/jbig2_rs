#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn raw_error_trait_bridges_preserve_all_symbol_variants_and_nested_fields() {
    for error in [
        SymbolTextError::InvalidFlags("raw diagnostic"),
        SymbolTextError::InvalidTableAssociation("raw diagnostic"),
        SymbolTextError::InvalidCount("raw diagnostic"),
        SymbolTextError::InvalidEntropyValue("raw diagnostic"),
        SymbolTextError::UnexpectedOutOfBand("raw diagnostic"),
        SymbolTextError::InvalidState("raw diagnostic"),
    ] {
        assert!(matches!(
            DecodeError::from(error),
            DecodeError::Malformed(MalformedError::InvalidSymbolText("raw diagnostic"))
        ));
    }
    for (error, expected) in [
        (
            SymbolTextError::InvalidDimensions,
            MalformedError::InvalidSymbolDimensions,
        ),
        (
            SymbolTextError::SymbolIdOutOfRange {
                id: u32::MAX,
                available: 1,
            },
            MalformedError::SymbolIdOutOfRange {
                id: u32::MAX,
                available: 1,
            },
        ),
        (
            SymbolTextError::InvalidSymbolLengthRun,
            MalformedError::InvalidSymbolText("invalid symbol-code length run"),
        ),
    ] {
        assert!(
            matches!(DecodeError::from(error), DecodeError::Malformed(actual)
            if actual == expected)
        );
    }
    let arithmetic: SymbolTextError = ArithmeticError::InvalidContextState {
        state_index: u8::MAX,
        mps: u8::MAX,
    }
    .into();
    assert!(matches!(
        DecodeError::from(arithmetic),
        DecodeError::Malformed(MalformedError::InvalidArithmeticContextState {
            state_index: u8::MAX,
            mps: u8::MAX,
        })
    ));
    let generic: SymbolTextError = GenericRegionError::UnexpectedArithmeticMarker {
        offset: usize::MAX,
        code: u8::MAX,
    }
    .into();
    assert!(matches!(
        DecodeError::from(generic),
        DecodeError::Malformed(MalformedError::InvalidGenericRegion(
            crate::GenericRegionErrorKind::UnexpectedArithmeticMarker {
                offset: usize::MAX,
                code: u8::MAX,
            }
        ))
    ));
    let mmr: SymbolTextError =
        MmrError::Bitmap(DecodeError::Io(crate::error::IoErrorKind::PermissionDenied)).into();
    assert!(matches!(
        DecodeError::from(mmr),
        DecodeError::Io(crate::error::IoErrorKind::PermissionDenied)
    ));
    let generic: GenericRegionError = MmrError::Bitmap(DecodeError::Allocation {
        requested_bytes: u64::MAX,
    })
    .into();
    assert!(matches!(
        generic,
        GenericRegionError::Decode(DecodeError::Allocation {
            requested_bytes: u64::MAX
        })
    ));
    let huffman: HuffmanError = DecodeError::Io(crate::error::IoErrorKind::Interrupted).into();
    assert!(matches!(
        DecodeError::from(huffman),
        DecodeError::Io(crate::error::IoErrorKind::Interrupted)
    ));
}

#[test]
fn raw_dictionary_flag_words_match_independent_field_policy() {
    for flags in 0..=u16::MAX {
        let huffman = flags & 1 != 0;
        let aggregation = flags & 2 != 0;
        let height = usize::from((flags >> 2) & 3);
        let width = usize::from((flags >> 4) & 3);
        let expected_error = if flags & 0xe000 != 0 {
            Some("reserved symbol-dictionary flags")
        } else if !huffman && flags & 0x00fc != 0 {
            Some("Huffman selections in arithmetic dictionary")
        } else if huffman && flags & 0x0c00 != 0 {
            Some("direct bitmap template in Huffman dictionary")
        } else if !aggregation && flags & 0x1080 != 0 {
            Some("aggregation fields without refinement aggregation")
        } else if huffman && !aggregation && flags & 0x0300 != 0 {
            Some("bitmap contexts in direct Huffman dictionary")
        } else if height == 2 {
            Some("invalid height-delta table selection")
        } else if width == 2 {
            Some("invalid width-delta table selection")
        } else {
            None
        };
        let result = parse_symbol_dictionary_flags(flags);
        if let Some(expected) = expected_error {
            assert!(
                matches!(result, Err(SymbolTextError::InvalidFlags(reason)) if reason == expected),
                "{flags:#06x}: {result:?}"
            );
            continue;
        }
        let config = result.unwrap();
        assert_eq!(
            config,
            SymbolDictionaryConfig {
                huffman,
                refinement_aggregation: aggregation,
                height_delta: [
                    TableChoice::Standard(StandardHuffmanTable::D),
                    TableChoice::Standard(StandardHuffmanTable::E),
                    TableChoice::Custom,
                    TableChoice::Custom,
                ][height],
                width_delta: [
                    TableChoice::Standard(StandardHuffmanTable::B),
                    TableChoice::Standard(StandardHuffmanTable::C),
                    TableChoice::Custom,
                    TableChoice::Custom,
                ][width],
                bitmap_size: if flags & 0x0040 == 0 {
                    TableChoice::Standard(StandardHuffmanTable::A)
                } else {
                    TableChoice::Custom
                },
                aggregation_instances: if flags & 0x0080 == 0 {
                    TableChoice::Standard(StandardHuffmanTable::A)
                } else {
                    TableChoice::Custom
                },
                bitmap_context_used: flags & 0x0100 != 0,
                bitmap_context_retained: flags & 0x0200 != 0,
                template: u8::try_from((flags >> 10) & 3).unwrap(),
                refinement_template: u8::from(flags & 0x1000 != 0),
            },
            "{flags:#06x}"
        );
    }
}

#[test]
fn raw_text_flag_words_preserve_geometry_and_error_precedence() {
    for flags in 0..=u16::MAX {
        for huffman_flags in [None, Some(0), Some(0x8000)] {
            let huffman = flags & 1 != 0;
            let refinement = flags & 2 != 0;
            let expected_error = if !refinement && flags & 0x8000 != 0 {
                Some("refinement template without refinement")
            } else {
                match (huffman, huffman_flags) {
                    (true, None) => Some("missing text Huffman flags"),
                    (false, Some(_)) => Some("text Huffman flags in arithmetic region"),
                    (true, Some(0x8000)) => Some("reserved text Huffman flag"),
                    _ => None,
                }
            };
            let result = parse_text_region_flags(flags, huffman_flags, false);
            if let Some(expected) = expected_error {
                assert!(
                    matches!(result, Err(SymbolTextError::InvalidFlags(reason)) if reason == expected),
                    "{flags:#06x}/{huffman_flags:?}: {result:?}"
                );
            } else {
                let config = result.unwrap();
                let signed_offset = i16::try_from((flags >> 10) & 31).unwrap();
                assert_eq!(config.huffman, huffman);
                assert_eq!(config.refinement, refinement);
                assert_eq!(config.strip_size, 1 << ((flags >> 2) & 3));
                assert_eq!(
                    config.reference_corner,
                    [
                        ReferenceCorner::BottomLeft,
                        ReferenceCorner::TopLeft,
                        ReferenceCorner::BottomRight,
                        ReferenceCorner::TopRight,
                    ][usize::from((flags >> 4) & 3)]
                );
                assert_eq!(config.transposed, flags & 0x0040 != 0);
                assert_eq!(
                    config.operator,
                    [
                        CombinationOperator::Or,
                        CombinationOperator::And,
                        CombinationOperator::Xor,
                        CombinationOperator::Xnor,
                    ][usize::from((flags >> 7) & 3)]
                );
                assert_eq!(config.default_pixel, flags & 0x0200 != 0);
                assert_eq!(
                    i16::from(config.s_offset),
                    if signed_offset >= 16 {
                        signed_offset - 32
                    } else {
                        signed_offset
                    }
                );
                assert_eq!(config.refinement_template, u8::from(flags & 0x8000 != 0));
                assert_eq!(config.huffman_choices.is_some(), huffman);
            }
            assert!(matches!(
                parse_text_region_flags(flags, huffman_flags, true),
                Err(SymbolTextError::Decode(DecodeError::Unsupported(
                    UnsupportedFeature::Colour
                )))
            ));
        }
    }
}

#[test]
fn raw_text_table_selectors_match_complete_word_domain() {
    use StandardHuffmanTable::{A, F, G, H, I, J, K, L, M, N, O};
    use TableChoice::{Custom, Standard};
    for flags in 0..=u16::MAX {
        for refinement in [false, true] {
            let selectors = [0, 6, 8, 10, 12].map(|shift| (flags >> shift) & 3);
            let valid = flags & 0x8000 == 0
                && selectors.iter().all(|selector| *selector != 2)
                && (refinement || flags & 0x7fc0 == 0);
            let result = parse_text_huffman_flags(flags, refinement);
            assert_eq!(result.is_ok(), valid, "{flags:#06x}/{refinement}");
            if let Ok(choices) = result {
                let refinement_choices = [Standard(N), Standard(O), Custom, Custom];
                assert_eq!(
                    choices,
                    TextHuffmanChoices {
                        first_s: [Standard(F), Standard(G), Custom, Custom][usize::from(flags & 3)],
                        subsequent_s: [Standard(H), Standard(I), Standard(J), Custom]
                            [usize::from((flags >> 2) & 3)],
                        strip_delta: [Standard(K), Standard(L), Standard(M), Custom]
                            [usize::from((flags >> 4) & 3)],
                        refinement_width: refinement_choices[usize::from((flags >> 6) & 3)],
                        refinement_height: refinement_choices[usize::from((flags >> 8) & 3)],
                        refinement_x: refinement_choices[usize::from((flags >> 10) & 3)],
                        refinement_y: refinement_choices[usize::from((flags >> 12) & 3)],
                        refinement_size: if flags & 0x4000 == 0 {
                            Standard(A)
                        } else {
                            Custom
                        },
                    }
                );
            }
        }
    }
}

#[test]
fn raw_table_bindings_preserve_selector_order_identity_and_oob_policy() {
    let limits = Limits::default();
    let tables = [
        Box::new(HuffmanTable::standard(StandardHuffmanTable::A, &limits).unwrap()),
        Box::new(HuffmanTable::standard(StandardHuffmanTable::B, &limits).unwrap()),
    ];
    let parameters = [
        TableParameter::TextFirstS,
        TableParameter::TextSubsequentS,
        TableParameter::TextStripDelta,
        TableParameter::TextRefinementWidth,
        TableParameter::TextRefinementHeight,
        TableParameter::TextRefinementX,
        TableParameter::TextRefinementY,
        TableParameter::TextRefinementSize,
    ];
    for mask in 0..256_u16 {
        let specifications: Vec<_> = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                (
                    *parameter,
                    if mask & (1 << index) == 0 {
                        TableChoice::Standard(StandardHuffmanTable::O)
                    } else {
                        TableChoice::Custom
                    },
                    index == 1,
                )
            })
            .collect();
        let supplied: Vec<_> = specifications
            .iter()
            .enumerate()
            .filter(|(_, (_, choice, _))| *choice == TableChoice::Custom)
            .map(|(index, (_, _, has_oob))| CustomHuffmanTable {
                table: &tables[index % 2],
                has_oob: *has_oob,
            })
            .collect();
        let bindings = bind_tables(&specifications, &supplied).unwrap();
        assert_eq!(bindings.len, parameters.len());
        for (index, parameter) in parameters.iter().enumerate() {
            assert_eq!(bindings.entries[index].unwrap().parameter, *parameter);
            let found = find_table_binding(&bindings, *parameter).unwrap();
            match found {
                BoundHuffmanTable::Standard(table) => {
                    assert_eq!(mask & (1 << index), 0);
                    assert_eq!(table, StandardHuffmanTable::O);
                }
                BoundHuffmanTable::Custom(table) => {
                    assert_ne!(mask & (1 << index), 0);
                    assert!(std::ptr::eq(table, tables[index % 2].as_ref()));
                }
            }
        }
        assert!(find_table_binding(&bindings, TableParameter::DictionaryHeight).is_none());
        if !supplied.is_empty() {
            assert!(matches!(
                bind_tables(&specifications, &supplied[..supplied.len() - 1]),
                Err(SymbolTextError::InvalidTableAssociation(
                    "custom table count does not match selectors"
                ))
            ));
            for index in 0..supplied.len() {
                let mut invalid = supplied.clone();
                invalid[index].has_oob = !invalid[index].has_oob;
                assert!(matches!(
                    bind_tables(&specifications, &invalid),
                    Err(SymbolTextError::InvalidTableAssociation(
                        "custom table OOB capability is invalid"
                    ))
                ));
            }
        }
        let mut extra = supplied.clone();
        extra.push(CustomHuffmanTable {
            table: &tables[0],
            has_oob: false,
        });
        assert!(matches!(
            bind_tables(&specifications, &extra),
            Err(SymbolTextError::InvalidTableAssociation(
                "custom table count does not match selectors"
            ))
        ));
    }
    let oversized = [(TableParameter::TextFirstS, TableChoice::Custom, false); 9];
    assert!(matches!(
        bind_tables(&oversized, &[]),
        Err(SymbolTextError::InvalidTableAssociation(
            "too many table selectors"
        ))
    ));
    for left in parameters {
        for right in parameters {
            assert_eq!(same_table_parameter(left, right), left == right);
        }
    }
}

#[test]
fn raw_standard_table_cache_covers_every_slot_and_failed_materialization() {
    let limits = Limits::default();
    let restrictive = Limits {
        max_symbols: 0,
        ..limits
    };
    let names = [
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
    let mut cache = StandardHuffmanTables::empty();
    for (index, name) in names.into_iter().enumerate() {
        assert!(cache.table(name).is_none());
        assert!(matches!(
            cache.materialize(name, &restrictive),
            Err(HuffmanError::Decode(DecodeError::LimitExceeded(
                LimitExceeded {
                    kind: LimitKind::Symbols,
                    limit: 0,
                    ..
                }
            )))
        ));
        assert!(cache.table(name).is_none());
        cache.materialize(name, &limits).unwrap();
        let first = std::ptr::from_ref(cache.table(name).unwrap());
        cache.materialize(name, &restrictive).unwrap();
        assert_eq!(std::ptr::from_ref(cache.table(name).unwrap()), first);
        for (other_index, other) in names.into_iter().enumerate() {
            assert_eq!(cache.table(other).is_some(), other_index <= index);
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn raw_table_associations_reject_missing_fields_and_preserve_borrowed_tables(
) -> Result<(), SymbolTextError> {
    let limits = Limits::default();
    let plain = HuffmanTable::standard(StandardHuffmanTable::A, &limits).unwrap();
    let oob = HuffmanTable::standard(StandardHuffmanTable::B, &limits).unwrap();
    let borrowed = [&plain, &oob, &plain, &plain, &plain, &plain, &plain, &plain];
    let wrapped = wrap_custom_tables(&borrowed).unwrap();
    for (index, supplied) in wrapped.iter().enumerate() {
        assert!(std::ptr::eq(supplied.table, borrowed[index]));
        assert_eq!(supplied.has_oob, index == 1);
    }
    assert!(wrap_custom_tables(&[]).unwrap().is_empty());
    let arithmetic_dictionary = parse_symbol_dictionary_flags(0).unwrap();
    let arithmetic_text = parse_text_region_flags(0, None, false).unwrap();
    assert_eq!(
        associate_dictionary_tables(&arithmetic_dictionary, &[])
            .unwrap()
            .len,
        0
    );
    assert_eq!(associate_text_tables(&arithmetic_text, &[]).unwrap().len, 0);
    assert!(matches!(
        associate_dictionary_tables(&arithmetic_dictionary, &wrapped),
        Err(SymbolTextError::InvalidTableAssociation(
            "custom tables on arithmetic dictionary"
        ))
    ));
    assert!(matches!(
        associate_text_tables(&arithmetic_text, &wrapped),
        Err(SymbolTextError::InvalidTableAssociation(
            "custom tables on arithmetic text region"
        ))
    ));
    let empty = TableBindings {
        entries: [None; MAX_TABLE_BINDINGS],
        len: 0,
    };
    assert!(matches!(
        select_dictionary_tables(&arithmetic_dictionary, &empty),
        Err(SymbolTextError::InvalidTableAssociation(
            "dictionary choices require Huffman mode"
        ))
    ));
    let dictionary = parse_symbol_dictionary_flags(0x00ff).unwrap();
    let dictionary_bindings = associate_dictionary_tables(&dictionary, &wrapped[..4]).unwrap();
    let text = parse_text_region_flags(3, Some(0x7fff), false).unwrap();
    let text_bindings = associate_text_tables(&text, &wrapped).unwrap();
    for (bindings, dictionary_mode, errors) in [
        (
            dictionary_bindings,
            true,
            &[
                "missing height table",
                "missing width table",
                "missing bitmap-size table",
                "missing aggregation-instances table",
            ][..],
        ),
        (
            text_bindings,
            false,
            &[
                "missing FS table",
                "missing DS table",
                "missing DT table",
                "missing RDW table",
                "missing RDH table",
                "missing RDX table",
                "missing RDY table",
                "missing RSIZE table",
            ][..],
        ),
    ] {
        for (index, expected) in errors.iter().enumerate() {
            let mut incomplete = bindings;
            incomplete
                .entries
                .copy_within(index + 1..bindings.len, index);
            incomplete.len -= 1;
            incomplete.entries[incomplete.len] = None;
            let result = if dictionary_mode {
                select_dictionary_tables(&dictionary, &incomplete)
            } else {
                select_text_tables(&incomplete, true)
            };
            assert!(matches!(
                result,
                Err(SymbolTextError::InvalidTableAssociation(reason)) if reason == *expected
            ));
        }
    }
    let dictionary_choices = select_dictionary_tables(&dictionary, &dictionary_bindings).unwrap();
    let dictionary_tables =
        materialize_huffman_tables!(HuffmanChoiceSet::Dictionary(dictionary_choices), &limits)
            .unwrap();
    assert!(std::ptr::eq(
        dictionary_tables
            .table_for(IntegerField::HeightDelta)
            .unwrap(),
        std::ptr::from_ref(&plain)
    ));
    assert!(std::ptr::eq(
        dictionary_tables
            .table_for(IntegerField::WidthDelta)
            .unwrap(),
        std::ptr::from_ref(&oob)
    ));
    for field in [
        IntegerField::AggregationInstances,
        IntegerField::ExportRun,
        IntegerField::RefinementX,
        IntegerField::RefinementY,
        IntegerField::RefinementSize,
        IntegerField::DictionaryBitmapSize,
        IntegerField::StripDelta,
        IntegerField::FirstS,
        IntegerField::SubsequentS,
        IntegerField::RefinementWidth,
        IntegerField::RefinementHeight,
    ] {
        assert!(dictionary_tables.table_for(field).is_some());
    }
    for field in [
        IntegerField::WithinStripT,
        IntegerField::RefinementIndicator,
    ] {
        assert!(dictionary_tables.table_for(field).is_none());
    }
    let choices = select_text_tables(&text_bindings, true).unwrap();
    let text_tables =
        materialize_huffman_tables!(HuffmanChoiceSet::TextRegion(choices), &limits).unwrap();
    for (index, field) in [
        IntegerField::FirstS,
        IntegerField::SubsequentS,
        IntegerField::StripDelta,
        IntegerField::RefinementWidth,
        IntegerField::RefinementHeight,
        IntegerField::RefinementX,
        IntegerField::RefinementY,
        IntegerField::RefinementSize,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(std::ptr::eq(
            text_tables.table_for(field).unwrap(),
            borrowed[index]
        ));
    }
    for field in [
        IntegerField::AggregationInstances,
        IntegerField::HeightDelta,
        IntegerField::WidthDelta,
        IntegerField::ExportRun,
        IntegerField::DictionaryBitmapSize,
        IntegerField::WithinStripT,
        IntegerField::RefinementIndicator,
    ] {
        assert!(text_tables.table_for(field).is_none());
    }
    Ok(())
}

#[test]
fn validates_symbol_dictionary_flags() {
    let config = parse_symbol_dictionary_flags(1).expect("valid Huffman flags");
    assert!(config.huffman);
    assert_eq!(
        config.height_delta,
        TableChoice::Standard(StandardHuffmanTable::D)
    );
    assert!(matches!(
        parse_symbol_dictionary_flags(1 | (2 << 2)),
        Err(SymbolTextError::InvalidFlags(_))
    ));
    assert!(matches!(
        parse_symbol_dictionary_flags(0xe000),
        Err(SymbolTextError::InvalidFlags(_))
    ));
}

#[test]
fn validates_text_flags_and_signed_offset() {
    let flags = 1 | (2 << 2) | (3 << 4) | (0x1d << 10);
    let config = parse_text_region_flags(flags, Some(0), false).expect("valid text flags");
    assert_eq!(config.strip_size, 4);
    assert_eq!(config.reference_corner, ReferenceCorner::TopRight);
    assert_eq!(config.s_offset, -3);
    assert!(matches!(
        parse_text_region_flags(flags, Some(0x8000), false),
        Err(SymbolTextError::InvalidFlags(_))
    ));
}

#[test]
fn associates_custom_tables_in_normative_order_and_checks_oob() {
    let limits = Limits::default();
    let table = HuffmanTable::standard(StandardHuffmanTable::A, &limits).expect("standard table");
    let config = parse_symbol_dictionary_flags(1 | (3 << 2) | (3 << 4)).expect("valid flags");
    let supplied = [
        CustomHuffmanTable {
            table: &table,
            has_oob: false,
        },
        CustomHuffmanTable {
            table: &table,
            has_oob: true,
        },
    ];
    let bindings = associate_dictionary_tables(&config, &supplied).expect("valid associations");
    assert_eq!(
        bindings.entries[0].expect("height").parameter,
        TableParameter::DictionaryHeight
    );
    assert_eq!(
        bindings.entries[1].expect("width").parameter,
        TableParameter::DictionaryWidth
    );

    let invalid = [
        CustomHuffmanTable {
            table: &table,
            has_oob: true,
        },
        supplied[1],
    ];
    assert!(matches!(
        associate_dictionary_tables(&config, &invalid),
        Err(SymbolTextError::InvalidTableAssociation(_))
    ));
}

#[test]
fn expands_and_validates_symbol_length_runs() {
    let limits = Limits::default();
    let mut resources = ResourceTracker::default();
    let lengths = expand_symbol_code_lengths(
        8,
        &[
            SymbolLengthRun::Literal(3),
            SymbolLengthRun::RepeatPrevious(3),
            SymbolLengthRun::Zeros(4),
        ],
        &limits,
        &mut resources,
    )
    .expect("valid runs");
    assert_eq!(lengths, vec![3, 3, 3, 3, 0, 0, 0, 0]);

    let mut resources = ResourceTracker::default();
    assert!(matches!(
        expand_symbol_code_lengths(
            4,
            &[SymbolLengthRun::RepeatPrevious(3)],
            &limits,
            &mut resources
        ),
        Err(SymbolTextError::InvalidSymbolLengthRun)
    ));
    let mut resources = ResourceTracker::default();
    assert!(matches!(
        expand_symbol_code_lengths(4, &[SymbolLengthRun::Zeros(5)], &limits, &mut resources),
        Err(SymbolTextError::InvalidSymbolLengthRun)
    ));
}

#[test]
fn builds_symbol_id_huffman_table_from_lengths() {
    let limits = Limits::default();
    let table = HuffmanTable::new_symbol_ids(&[1, 3, 3], &limits);
    assert!(table.is_ok());
    let invalid = HuffmanTable::new_symbol_ids(&[1, 1, 1], &limits);
    assert!(invalid.is_err());
}

#[test]
fn rejects_unused_fixed_length_symbol_ids() {
    let error = checked_symbol_id(7, 5).expect_err("ID 7 is outside five symbols");
    assert!(matches!(
        error,
        SymbolTextError::SymbolIdOutOfRange {
            id: 7,
            available: 5
        }
    ));
}

#[test]
fn repeated_standard_table_materialization_is_reused() {
    let limits = Limits::default();
    let mut tables = StandardHuffmanTables::empty();
    tables
        .materialize(StandardHuffmanTable::A, &limits)
        .expect("first table A");
    let first = tables
        .table(StandardHuffmanTable::A)
        .expect("materialized table") as *const HuffmanTable;
    tables
        .materialize(StandardHuffmanTable::A, &limits)
        .expect("second table A");
    let second =
        tables.table(StandardHuffmanTable::A).expect("reused table") as *const HuffmanTable;
    assert_eq!(first, second);
}
