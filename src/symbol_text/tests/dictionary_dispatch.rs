#![cfg(test)]

use super::super::dictionary::decode_one_new_symbol;
use super::*;
use std::fmt::Write;

fn header(huffman: bool, refinement: bool) -> SymbolDictionaryHeader {
    let flags = u16::from(huffman) | (u16::from(refinement) << 1);
    SymbolDictionaryHeader {
        config: parse_symbol_dictionary_flags(flags).unwrap(),
        adaptive: [TemplateOffset::new(0, 0); 4],
        adaptive_count: 0,
        refinement_adaptive: [TemplateOffset::new(0, 0); 2],
        exported_symbols: 1,
        new_symbols: 1,
    }
}

#[test]
fn actual_dictionary_id_request_counts_imported_and_new_with_enclosing_width() {
    let limits = Limits::default();
    let bytes = [0xff; 2];
    for width in 1_u8..=4 {
        let count = (1_u32 << width) - 1;
        let symbols: Vec<Bitmap> = (0..count)
            .map(|_| Bitmap::blank(1, 1, &limits).unwrap())
            .collect();
        for imported_count in 0..=symbols.len() {
            let (imported, new_symbols) = symbols.split_at(imported_count);
            let borrowed: Vec<&Bitmap> = imported.iter().collect();
            for book in [
                SymbolCodebook::Contiguous(imported),
                SymbolCodebook::Borrowed(&borrowed),
            ] {
                for position in 0..=8 {
                    for huffman in [false, true] {
                        let concrete = super::huffman_fields::codec(&bytes, position, 0);
                        let mut codec = SymbolCodec::Huffman(concrete);
                        let mut resources = ResourceTracker::default();
                        let result =
                            super::super::dictionary::decode_dictionary_single_instance_refinement(
                                &mut codec,
                                book,
                                new_symbols,
                                width,
                                1,
                                1,
                                0,
                                [(0, 0); 2],
                                huffman,
                                &limits,
                                &mut resources,
                            );
                        assert!(matches!(
                            result,
                            Err(SymbolTextError::SymbolIdOutOfRange { id, available })
                                if id == count && available == count
                        ));
                        assert_eq!(resources, ResourceTracker::default());
                        let SymbolCodec::Huffman(after) = codec else {
                            panic!("changed codec variant");
                        };
                        assert_eq!(after.reader.bit_position, position + usize::from(width));
                        assert_eq!(after.reader.bytes, bytes);
                        assert!(after.symbol_id_table.is_none());
                        assert_eq!(after.refinement_work, 71);
                    }
                }
            }
        }
    }
}

#[test]
fn single_instance_refinement_reaches_the_existing_raw_callback() {
    let limits = Limits::default();
    let reference = Bitmap::blank(1, 1, &limits).unwrap();
    let references = [&reference];
    for huffman in [false, true] {
        let mut integers = VecDeque::from([EntropyInteger::Value(0), EntropyInteger::Value(0)]);
        if huffman {
            integers.push_back(EntropyInteger::Value(2));
        }
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers,
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let mut generic_work = 0;
        let bitmap = decode_one_new_symbol(
            &header(huffman, true),
            &mut codec,
            SymbolCodebook::Borrowed(&references),
            &[],
            None,
            &mut generic_work,
            Some(1),
            1,
            1,
            0,
            [(0, 0); 2],
            &limits,
            &mut resources,
        )
        .expect("one-instance refinement must not reject its required count");
        assert_eq!(
            (bitmap.width(), bitmap.height(), bitmap.bytes()),
            (1, 1, &[0][..])
        );
        assert_eq!(resources.work_units, 0);
        assert_eq!(resources.decoded_bitmap_bytes, 1);
        assert_eq!(generic_work, 0);
        let SymbolCodec::Scripted(codec) = codec else {
            unreachable!()
        };
        assert!(codec.integers.is_empty());
    }
}

#[test]
fn direct_coded_symbol_rejects_an_aggregation_count_before_decoding() {
    let limits = Limits::default();
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([EntropyInteger::Value(9)]),
        symbol_id: 0,
    });
    let mut resources = ResourceTracker::default();
    let mut generic_work = 0;
    let result = decode_one_new_symbol(
        &header(false, false),
        &mut codec,
        SymbolCodebook::Contiguous(&[]),
        &[],
        None,
        &mut generic_work,
        Some(1),
        1,
        1,
        0,
        [(0, 0); 2],
        &limits,
        &mut resources,
    );
    assert!(matches!(
        result,
        Err(SymbolTextError::InvalidState(
            "aggregation count on direct-coded symbol",
        ))
    ));
    assert_eq!(resources, ResourceTracker::default());
    assert_eq!(generic_work, 0);
    let SymbolCodec::Scripted(codec) = codec else {
        unreachable!()
    };
    assert_eq!(codec.integers.len(), 1);
}

#[test]
fn refinement_still_rejects_missing_and_zero_instance_counts() {
    let limits = Limits::default();
    for count in [None, Some(0)] {
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers: VecDeque::new(),
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let mut generic_work = 0;
        let result = decode_one_new_symbol(
            &header(false, true),
            &mut codec,
            SymbolCodebook::Contiguous(&[]),
            &[],
            None,
            &mut generic_work,
            count,
            1,
            1,
            0,
            [(0, 0); 2],
            &limits,
            &mut resources,
        );
        assert!(matches!(
            (count, result),
            (
                None,
                Err(SymbolTextError::InvalidState(
                    "missing aggregation-instance count"
                ))
            ) | (
                Some(0),
                Err(SymbolTextError::InvalidCount(
                    "zero-instance aggregate symbol"
                ))
            )
        ));
        assert_eq!(resources, ResourceTracker::default());
    }
}

fn dictionary_bits(bits: &str) -> Vec<u8> {
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (index, bit) in bits.bytes().enumerate() {
        assert!(bit == b'0' || bit == b'1');
        bytes[index / 8] |= (bit - b'0') << (7 - index % 8);
    }
    bytes
}

fn dictionary_export_run(value: usize) -> String {
    assert!(value <= 32);
    if value < 16 {
        format!("0{value:04b}")
    } else {
        format!("10{:08b}", value - 16)
    }
}

fn fixed_id_dictionary_payload(imported_count: usize, ids: [usize; 2]) -> Vec<u8> {
    assert!(imported_count > 0 && imported_count <= 32);
    // Include the declared new symbol, not just the available imported symbols.
    let width = usize::BITS - imported_count.leading_zeros();
    let width = usize::try_from(width).unwrap();
    let mut payload = 0x1003_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&1_u32.to_be_bytes());

    // B.4 DH=1; B.2 DW=16; B.1 REFAGGNINST=2.
    let mut bits = String::from("0");
    bits.push_str("11110000101");
    bits.push_str("00010");
    // Table 17: B.11 initial DT=1 and DT=1, B.6 FS=0.
    // No transmitted symbol-ID table or byte alignment belongs here.
    bits.push_str("00");
    bits.push_str("000000000");
    write!(bits, "{:0width$b}0", ids[0]).unwrap();
    // B.8 DS=1 places the second eight-pixel symbol at x=8; both RI=0.
    bits.push_str("001");
    write!(bits, "{:0width$b}0", ids[1]).unwrap();
    bits.push_str("01"); // B.8 strip OOB, also required after the last instance.
    bits.push_str("111111"); // B.2 height-class OOB.
    bits.push_str(&dictionary_export_run(imported_count));
    bits.push_str(&dictionary_export_run(1));
    payload.extend_from_slice(&dictionary_bits(&bits));
    payload
}

#[test]
fn huffman_dictionary_aggregation_preserves_standard_fixed_symbol_ids() {
    // Regression for DEV-952C7B7561, retaining its 119 encoded challenges.
    let limits = Limits::default();
    let mut seed = 0x88_65_82_03_u32;
    for count in [1_usize, 2, 3, 4, 7, 8, 15, 16, 31, 32] {
        let pixels: Vec<u8> = (0..count)
            .map(|id| u8::try_from(id).unwrap().wrapping_mul(37) ^ 0xa5)
            .collect();
        let imported: Vec<Bitmap> = pixels
            .iter()
            .map(|pixel| bitmap_from_packed_bytes(8, 1, &[*pixel], &limits).unwrap())
            .collect();
        for first in 0..count {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let second = if first == 0 {
                count - 1
            } else {
                usize::try_from(seed).unwrap() % count
            };
            let ids = [first, second];
            let payload = fixed_id_dictionary_payload(count, ids);
            let output = decode_symbol_dictionary(&SymbolDictionaryInput {
                payload: &payload,
                imported_symbols: SymbolCodebook::Contiguous(&imported),
                custom_tables: &[],
                used_contexts: None,
                limits: &limits,
            })
            .unwrap_or_else(|error| panic!("count={count} ids={ids:?}: {error:?}"));
            assert_eq!(output.exported_symbols.len(), 1);
            let bitmap = &output.exported_symbols[0];
            assert_eq!(
                (bitmap.width(), bitmap.height(), bitmap.stride()),
                (16, 1, 2)
            );
            assert_eq!(
                bitmap.bytes(),
                &[pixels[first], pixels[second]],
                "count={count} ids={ids:?}"
            );
            assert_eq!(output.consumed_bytes, payload.len());
            assert!(output.retained_contexts.is_none());
            for (bitmap, expected) in imported.iter().zip(&pixels) {
                assert_eq!(bitmap.bytes(), &[*expected], "imported bitmap mutated");
            }
        }
    }
}

#[test]
fn huffman_dictionary_aggregate_rejects_unavailable_fixed_ids() {
    let limits = Limits::default();
    let imported = [bitmap_from_packed_bytes(8, 1, &[0xa5], &limits).unwrap()];
    for ids in [[1, 0], [0, 1]] {
        let payload = fixed_id_dictionary_payload(1, ids);
        assert!(matches!(
            decode_symbol_dictionary(&SymbolDictionaryInput {
                payload: &payload,
                imported_symbols: SymbolCodebook::Contiguous(&imported),
                custom_tables: &[],
                used_contexts: None,
                limits: &limits,
            }),
            Err(SymbolTextError::SymbolIdOutOfRange {
                id: 1,
                available: 1,
            })
        ));
    }
}

#[test]
fn huffman_dictionary_aggregates_can_reference_previous_new_symbols() {
    let limits = Limits::default();
    let imported = [bitmap_from_packed_bytes(8, 1, &[0xa5], &limits).unwrap()];
    let mut payload = 0x1003_u16.to_be_bytes().to_vec();
    payload.extend_from_slice(&2_u32.to_be_bytes());
    payload.extend_from_slice(&2_u32.to_be_bytes());
    // DH=1.
    let mut bits = String::from("0");
    // Both IDs have two bits from the outset: one import + two declared new.
    for id in ["00", "01"] {
        bits.push_str("11110000101"); // DW=16, yielding widths 16 then 32.
        bits.push_str("00010"); // Two instances.
        bits.push_str("00"); // Initial DT=1 and DT=1.
        bits.push_str("000000000"); // FS=0.
        bits.push_str(id);
        bits.push('0'); // RI=0.
        bits.push_str("001"); // DS=1.
        bits.push_str(id);
        bits.push('0');
        bits.push_str("01"); // Strip OOB.
    }
    bits.push_str("111111"); // Height-class OOB.
    bits.push_str(&dictionary_export_run(1));
    bits.push_str(&dictionary_export_run(2));
    payload.extend_from_slice(&dictionary_bits(&bits));
    let output = decode_symbol_dictionary(&SymbolDictionaryInput {
        payload: &payload,
        imported_symbols: SymbolCodebook::Contiguous(&imported),
        custom_tables: &[],
        used_contexts: None,
        limits: &limits,
    })
    .unwrap();
    assert_eq!(output.exported_symbols.len(), 2);
    for (bitmap, width) in output.exported_symbols.iter().zip([16, 32]) {
        assert_eq!((bitmap.width(), bitmap.height()), (width, 1));
        assert_eq!(
            bitmap.bytes(),
            vec![0xa5; usize::try_from(width / 8).unwrap()]
        );
    }
    assert_eq!(output.consumed_bytes, payload.len());
    assert!(output.retained_contexts.is_none());
    assert_eq!(imported[0].bytes(), &[0xa5]);
}
