#![cfg(test)]

use super::super::text::{decode_text_region_body, decode_text_region_body_with_end};
use super::*;

fn config(huffman: bool, log_strips: u16) -> TextRegionConfig {
    parse_text_region_flags(
        u16::from(huffman) | (log_strips << 2),
        huffman.then_some(0),
        false,
    )
    .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn bounded_arithmetic_end_requires_certainty_count_scope_and_flushes_placements() {
    let limits = Limits::default();
    let symbols = [Bitmap::filled(1, 1, true, &limits).unwrap()];
    for (bytes, observer, refinement, policy, count, success, recovered) in [
        (
            [45, 151, 255, 172],
            1,
            false,
            TextRegionEnd::BoundedArithmetic,
            1,
            true,
            true,
        ),
        (
            [45, 151, 255, 172],
            0,
            false,
            TextRegionEnd::BoundedArithmetic,
            1,
            false,
            false,
        ),
        (
            [45, 151, 255, 172],
            1,
            false,
            TextRegionEnd::Strict,
            1,
            false,
            false,
        ),
        (
            [45, 151, 255, 172],
            1,
            true,
            TextRegionEnd::BoundedArithmetic,
            1,
            false,
            false,
        ),
        (
            [45, 151, 255, 172],
            1,
            false,
            TextRegionEnd::BoundedArithmetic,
            2,
            false,
            false,
        ),
        (
            [59, 8, 255, 172],
            1,
            false,
            TextRegionEnd::BoundedArithmetic,
            1,
            true,
            false,
        ),
        (
            [59, 8, 255, 172],
            2,
            false,
            TextRegionEnd::BoundedArithmetic,
            1,
            true,
            false,
        ),
    ] {
        let mut arithmetic =
            super::super::codec::new_arithmetic_codec(&bytes, 0, None, &limits).unwrap();
        if observer != 0 {
            let mut prefix = PrefixCertainty::new(&arithmetic.decoder);
            if observer == 2 {
                prefix.active = false;
            }
            arithmetic.prefix = Some(prefix);
        }
        let mut codec = SymbolCodec::Arithmetic(arithmetic);
        let mut end = policy;
        let mut config = config(false, 0);
        config.refinement = refinement;
        let mut resources = ResourceTracker::default();
        let result = decode_text_region_body_with_end(
            &mut codec,
            SymbolCodebook::Contiguous(&symbols),
            0,
            1,
            1,
            count,
            &config,
            [(0, 0); 2],
            &limits,
            &mut resources,
            &mut end,
        );
        assert_eq!(
            result.is_ok(),
            success,
            "{bytes:?}, observer {observer}, refinement {refinement}, count {count}"
        );
        if let Ok(bitmap) = result {
            assert_eq!(bitmap.bytes(), [0x80]);
            assert_eq!(resources.work_units, 2);
            assert_eq!(resources.decoded_bitmap_bytes, 1);
        }
        assert_eq!(
            matches!(end, TextRegionEnd::RecoveredArithmetic { instances: 1 }),
            recovered
        );
    }
}

#[test]
fn arithmetic_prefix_recovery_obeys_ordinary_work_and_bitmap_memory_limits() {
    let symbols = [Bitmap::filled(1, 1, true, &Limits::default()).unwrap()];
    for (max_work, max_memory, expected) in [
        (2, u64::MAX, None),
        (1, u64::MAX, Some(LimitKind::Work)),
        (2, 0, Some(LimitKind::Memory)),
    ] {
        let bytes = [45, 151, 255, 172];
        let mut arithmetic =
            super::super::codec::new_arithmetic_codec(&bytes, 0, None, &Limits::default()).unwrap();
        arithmetic.prefix = Some(PrefixCertainty::new(&arithmetic.decoder));
        let mut codec = SymbolCodec::Arithmetic(arithmetic);
        let limits = Limits {
            max_work_units: max_work,
            max_decoder_managed_memory_bytes: max_memory,
            ..Limits::default()
        };
        let mut resources = ResourceTracker::default();
        let mut end = TextRegionEnd::BoundedArithmetic;
        let result = decode_text_region_body_with_end(
            &mut codec,
            SymbolCodebook::Contiguous(&symbols),
            0,
            1,
            1,
            1,
            &config(false, 0),
            [(0, 0); 2],
            &limits,
            &mut resources,
            &mut end,
        );
        match expected {
            None => assert_eq!(result.unwrap().bytes(), [0x80]),
            Some(kind) => assert!(
                matches!(result, Err(SymbolTextError::Decode(DecodeError::LimitExceeded(detail))) if detail.kind == kind)
            ),
        }
    }
}

#[test]
fn final_arithmetic_ds_errors_propagate_despite_native_recovery_gates() {
    let limits = Limits::default();
    let bytes = [0xff, 0x80, 0xff, 0xac];
    let mut arithmetic =
        super::super::codec::new_arithmetic_codec(&bytes, 0, None, &limits).unwrap();
    let mut ordinary = super::super::codec::new_arithmetic_codec(&bytes, 0, None, &limits).unwrap();
    arithmetic.prefix = Some(PrefixCertainty::new(&arithmetic.decoder));
    let expected = ordinary.decode_integer(IntegerField::SubsequentS);
    assert!(matches!(
        &expected,
        Err(SymbolTextError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow
        )))
    ));
    assert!(!assert_arithmetic_forwarding(
        arithmetic.decode_final_strip(1),
        expected.map(|_| false),
    ));
    let marker = arithmetic.decoder.marker().unwrap();
    assert_eq!((marker.offset, marker.code), (bytes.len() - 2, 0xac));
    assert_eq!(arithmetic.integers.slots, ordinary.integers.slots);
    assert_eq!(arithmetic.decoder.interval, ordinary.decoder.interval);
    assert_eq!(arithmetic.decoder.code, ordinary.decoder.code);
}

#[test]
fn actual_text_id_request_preserves_book_extent_segment_width_and_continuation() {
    let limits = Limits::default();
    let symbols = [
        Bitmap::filled(1, 1, false, &limits).unwrap(),
        Bitmap::filled(1, 1, true, &limits).unwrap(),
    ];
    let borrowed = [&symbols[0], &symbols[1]];
    let zero = HuffmanTable::new(&[HuffmanLine::normal(0, 1, 0)], &limits).unwrap();
    let end = HuffmanTable::new(
        &[HuffmanLine::normal(0, 1, 0), HuffmanLine::out_of_band(1)],
        &limits,
    )
    .unwrap();
    for book in [
        SymbolCodebook::Contiguous(&symbols),
        SymbolCodebook::Borrowed(&borrowed),
    ] {
        for width in [0_u8, 1, 2, 3, 4, 8, 16, 32] {
            let id = u32::from(width != 0);
            let mut bits = vec![false; 3];
            bits.extend((0..width).rev().map(|shift| (id >> shift) & 1 != 0));
            bits.push(true);
            bits.extend((0..8).rev().map(|shift| (0xa5_u8 >> shift) & 1 != 0));
            let mut bytes = vec![0_u8; bits.len().div_ceil(8)];
            for (position, bit) in bits.into_iter().enumerate() {
                if bit {
                    bytes[position / 8] |= 0x80 >> (position % 8);
                }
            }
            let mut concrete = super::huffman_fields::codec(&bytes, 0, 0);
            concrete.tables.strip_delta = Some(MaterializedHuffmanTable::Custom(&zero));
            concrete.tables.first_s = Some(MaterializedHuffmanTable::Custom(&zero));
            concrete.tables.subsequent_s = Some(MaterializedHuffmanTable::Custom(&end));
            let mut codec = SymbolCodec::Huffman(concrete);
            let mut resources = ResourceTracker::default();
            let bitmap = decode_text_region_body(
                &mut codec,
                book,
                width,
                1,
                1,
                1,
                &config(true, 0),
                [(0, 0); 2],
                &limits,
                &mut resources,
            )
            .unwrap();
            assert_eq!(bitmap.bytes(), if id == 0 { &[0][..] } else { &[0x80][..] });
            let SymbolCodec::Huffman(ref mut after) = codec else {
                panic!("changed codec variant");
            };
            assert_eq!(after.reader.bit_position, 4 + usize::from(width));
            assert_eq!(after.reader.bytes, bytes);
            assert!(after.symbol_id_table.is_none());
            assert_eq!(after.refinement_work, 71);
            assert_eq!(after.read_raw_bits(8).unwrap(), 0xa5);
        }
    }
}

#[test]
fn final_strip_terminator_precedes_following_dictionary_fields() {
    let limits = Limits::default();
    let symbol = Bitmap::filled(1, 1, true, &limits).unwrap();
    let symbols = [&symbol];
    for huffman in [false, true] {
        for instances in [1, 2] {
            let mut integers = VecDeque::from([
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
            ]);
            if instances == 2 {
                integers.push_back(EntropyInteger::Value(1));
            }
            integers.push_back(EntropyInteger::OutOfBand);
            integers.push_back(EntropyInteger::Value(37));
            let mut codec = SymbolCodec::Scripted(ScriptedCodec {
                integers,
                symbol_id: 0,
            });
            let mut resources = ResourceTracker::default();
            let bitmap = decode_text_region_body(
                &mut codec,
                SymbolCodebook::Borrowed(&symbols),
                0,
                instances,
                1,
                instances,
                &config(huffman, 0),
                [(0, 0); 2],
                &limits,
                &mut resources,
            )
            .unwrap();
            assert_eq!(
                bitmap.bytes(),
                if instances == 1 {
                    &[0x80][..]
                } else {
                    &[0xc0][..]
                }
            );
            assert_eq!(resources.work_units, u64::from(instances) * 2);
            assert_eq!(resources.decoded_bitmap_bytes, 1);
            assert_eq!(
                required_integer(&mut codec, IntegerField::WidthDelta).unwrap(),
                37
            );
            let SymbolCodec::Scripted(codec) = codec else {
                unreachable!()
            };
            assert!(codec.integers.is_empty());
        }
    }
}

#[test]
fn declared_instance_count_does_not_hide_a_missing_or_overfull_strip_end() {
    let limits = Limits::default();
    let symbol = Bitmap::filled(1, 1, true, &limits).unwrap();
    let symbols = [&symbol];
    for huffman in [false, true] {
        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers: VecDeque::from([
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
            ]),
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let result = decode_text_region_body(
            &mut codec,
            SymbolCodebook::Borrowed(&symbols),
            0,
            1,
            1,
            1,
            &config(huffman, 0),
            [(0, 0); 2],
            &limits,
            &mut resources,
        );
        assert!(matches!(
            result,
            Err(SymbolTextError::InvalidState("missing scripted integer"))
        ));
        assert_eq!(resources.work_units, 2);

        let mut codec = SymbolCodec::Scripted(ScriptedCodec {
            integers: VecDeque::from([
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
                EntropyInteger::Value(0),
                EntropyInteger::Value(4),
                EntropyInteger::OutOfBand,
                EntropyInteger::Value(37),
            ]),
            symbol_id: 0,
        });
        let mut resources = ResourceTracker::default();
        let result = decode_text_region_body(
            &mut codec,
            SymbolCodebook::Borrowed(&symbols),
            0,
            1,
            1,
            1,
            &config(huffman, 2),
            [(0, 0); 2],
            &limits,
            &mut resources,
        );
        assert!(matches!(
            result,
            Err(SymbolTextError::InvalidCount("too many text instances"))
        ));
        let SymbolCodec::Scripted(codec) = codec else {
            unreachable!()
        };
        assert_eq!(
            codec.integers,
            VecDeque::from([EntropyInteger::OutOfBand, EntropyInteger::Value(37),])
        );
        assert_eq!(resources.work_units, 2);
        assert_eq!(resources.decoded_bitmap_bytes, 1);
    }
}

#[test]
fn every_strip_consumes_its_own_terminator_without_eating_the_next_field() {
    let limits = Limits::default();
    let symbol = Bitmap::filled(1, 1, true, &limits).unwrap();
    let symbols = [&symbol];
    let mut codec = SymbolCodec::Scripted(ScriptedCodec {
        integers: VecDeque::from([
            EntropyInteger::Value(0),
            EntropyInteger::Value(0),
            EntropyInteger::Value(0),
            EntropyInteger::OutOfBand,
            EntropyInteger::Value(1),
            EntropyInteger::Value(1),
            EntropyInteger::OutOfBand,
            EntropyInteger::Value(37),
        ]),
        symbol_id: 0,
    });
    let mut resources = ResourceTracker::default();
    let bitmap = decode_text_region_body(
        &mut codec,
        SymbolCodebook::Borrowed(&symbols),
        0,
        2,
        2,
        2,
        &config(false, 0),
        [(0, 0); 2],
        &limits,
        &mut resources,
    )
    .unwrap();
    assert_eq!(bitmap.bytes(), &[0x80, 0x40]);
    assert_eq!(resources.work_units, 4);
    assert_eq!(resources.decoded_bitmap_bytes, 2);
    assert_eq!(
        required_integer(&mut codec, IntegerField::WidthDelta).unwrap(),
        37
    );
}

#[test]
fn final_huffman_padding_is_shorter_than_every_prefix_and_never_hides_a_partial_value() {
    let limits = Limits::default();
    for remaining in 0..8 {
        let table = HuffmanTable::new(
            &[
                HuffmanLine::normal(0, remaining + 1, 0),
                HuffmanLine::out_of_band(remaining + 1),
            ],
            &limits,
        )
        .unwrap();
        let mut reader = bit_reader(&[0]);
        reader.bit_position = 8 - usize::from(remaining);
        assert_eq!(table.decode_final_strip_value(&mut reader).unwrap(), None);
        assert_eq!(reader.bit_position, 8);
    }
    let table = HuffmanTable::new(
        &[HuffmanLine::normal(0, 1, 1), HuffmanLine::out_of_band(1)],
        &limits,
    )
    .unwrap();
    let mut truncated_value = bit_reader(&[0]);
    truncated_value.bit_position = 7;
    assert!(table
        .decode_final_strip_value(&mut truncated_value)
        .is_err());
    let mut valid_oob = bit_reader(&[1]);
    valid_oob.bit_position = 7;
    assert_eq!(
        table.decode_final_strip_value(&mut valid_oob).unwrap(),
        Some(HuffmanValue::OutOfBand)
    );
    let table = HuffmanTable::new(
        &[HuffmanLine::normal(0, 5, 0), HuffmanLine::out_of_band(5)],
        &limits,
    )
    .unwrap();
    for (bytes, position) in [(&[1][..], 4), (&[0, 0][..], 4)] {
        let mut reader = bit_reader(bytes);
        reader.bit_position = position;
        assert!(!matches!(
            table.decode_final_strip_value(&mut reader),
            Ok(None)
        ));
    }
}

#[test]
fn bounded_huffman_final_end_requires_complete_count_and_flushes_placements() {
    let limits = Limits::default();
    let symbols = [
        Bitmap::filled(1, 1, false, &limits).unwrap(),
        Bitmap::filled(1, 1, true, &limits).unwrap(),
    ];
    let zero = HuffmanTable::new(&[HuffmanLine::normal(0, 1, 0)], &limits).unwrap();
    let end_table = HuffmanTable::new(
        &[HuffmanLine::normal(0, 5, 0), HuffmanLine::out_of_band(5)],
        &limits,
    )
    .unwrap();
    for (bytes, count, policy, expected) in [
        (&[0x10][..], 1, TextRegionEnd::BoundedHuffman, true),
        (&[0x10][..], 1, TextRegionEnd::Strict, false),
        (&[0x10][..], 2, TextRegionEnd::BoundedHuffman, false),
        (&[0x11][..], 1, TextRegionEnd::BoundedHuffman, false),
        (&[0x10, 0][..], 1, TextRegionEnd::BoundedHuffman, false),
        (&[0x10, 0x80][..], 1, TextRegionEnd::BoundedHuffman, true),
    ] {
        let mut concrete = super::huffman_fields::codec(bytes, 0, 0);
        concrete.tables.strip_delta = Some(MaterializedHuffmanTable::Custom(&zero));
        concrete.tables.first_s = Some(MaterializedHuffmanTable::Custom(&zero));
        concrete.tables.subsequent_s = Some(MaterializedHuffmanTable::Custom(&end_table));
        let mut codec = SymbolCodec::Huffman(concrete);
        let mut resources = ResourceTracker::default();
        let mut end = policy;
        let result = decode_text_region_body_with_end(
            &mut codec,
            SymbolCodebook::Contiguous(&symbols),
            1,
            1,
            1,
            count,
            &config(true, 0),
            [(0, 0); 2],
            &limits,
            &mut resources,
            &mut end,
        );
        assert_eq!(result.is_ok(), expected, "{bytes:?}, count {count}");
        if let Ok(bitmap) = result {
            assert_eq!(bitmap.bytes(), &[0x80]);
            assert_eq!(resources.work_units, 2);
            if bytes.len() == 1 {
                assert!(matches!(
                    end,
                    TextRegionEnd::RecoveredHuffman {
                        instances: 1,
                        padding_start: 4
                    }
                ));
            } else {
                assert!(end == TextRegionEnd::BoundedHuffman);
            }
        } else {
            assert!(end == policy);
        }
    }
}
