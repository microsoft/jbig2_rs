//! Complete RGB references, declaration recovery and bounded profile failures.
#![allow(clippy::field_reassign_with_default)]
use jbig2_decoder::{
    DecodeError, DecodePolicy, Decoder, LimitKind, Limits, RecoveryKind, UnsupportedFeature,
};

fn segment(
    number: u32,
    kind: u8,
    page: u8,
    references: &[u8],
    retention: u8,
    payload: &[u8],
) -> Vec<u8> {
    let mut bytes = number.to_be_bytes().to_vec();
    bytes.extend_from_slice(&[
        kind,
        (u8::try_from(references.len()).unwrap() << 5) | retention,
    ]);
    bytes.extend_from_slice(references);
    bytes.push(page);
    bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn page(colour: bool) -> Vec<u8> {
    let mut bytes = 4_u32.to_be_bytes().to_vec();
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&[0x40 | if colour { 0x80 } else { 0 }, 0, 0]);
    segment(0, 48, 1, &[], 0, &bytes)
}

fn region(number: u32, x: u32, black: bool, foreground: Option<u32>, references: &[u8]) -> Vec<u8> {
    let mut bytes = 2_u32.to_be_bytes().to_vec();
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&x.to_be_bytes());
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&[if foreground.is_some() { 12 } else { 0 }, 1]);
    bytes.extend_from_slice(if black { &[0x26, 0xb8] } else { &[0x80] });
    if let Some(id) = foreground {
        bytes.extend_from_slice(&id.to_be_bytes());
    }
    let retention = match references.len() {
        0 => 0,
        1 => 2,
        2 => 6,
        _ => panic!("test refs"),
    };
    segment(number, 39, 1, references, retention, &bytes)
}

fn document(colour: bool, parts: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"\x97JB2\r\n\x1a\n".to_vec();
    bytes.push(1 | if colour { 8 } else { 0 });
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    for part in parts {
        bytes.extend_from_slice(part);
    }
    bytes
}

fn end(number: u32) -> Vec<u8> {
    segment(number, 49, 1, &[], 0, &[])
}

#[test]
fn rgb_keeps_real_foreground_channels_and_repairs_only_missing_declarations() {
    let complete = document(
        true,
        &[page(true), region(1, 1, true, Some(4), &[]), end(2)],
    );
    let decoder = Decoder::default();
    let output = decoder.decode_rgb(&complete).unwrap();
    let bitmap = output.pages()[0].bitmap();
    assert_eq!(
        (bitmap.width(), bitmap.height(), bitmap.stride()),
        (4, 1, 12)
    );
    assert_eq!(
        bitmap.bytes(),
        &[255, 255, 255, 255, 0, 0, 255, 0, 0, 255, 255, 255]
    );
    assert_eq!(bitmap.pixel(1, 0), Some([255, 0, 0]));
    assert_eq!(bitmap.pixel(4, 0), None);
    assert_eq!(bitmap.pixel(0, 1), None);
    assert!(decoder
        .decode_rgb_with_policy(&complete, DecodePolicy::Compatibility)
        .unwrap()
        .recoveries
        .is_empty());
    for policy in [DecodePolicy::Strict, DecodePolicy::Compatibility] {
        assert!(matches!(
            decoder.decode_with_policy(&complete, policy),
            Err(DecodeError::Unsupported(UnsupportedFeature::Colour))
        ));
    }
    let original = document(
        false,
        &[page(false), region(1, 1, true, Some(4), &[]), end(2)],
    );
    let saved = original.clone();
    assert!(decoder.decode_rgb(&original).is_err());
    let recovered = decoder
        .decode_rgb_with_policy(&original, DecodePolicy::Compatibility)
        .unwrap();
    assert_eq!(original, saved);
    assert_eq!(recovered.value.pages()[0].bitmap().bytes(), bitmap.bytes());
    assert_eq!(
        recovered
            .recoveries
            .iter()
            .map(|r| (r.page, r.byte_offset, r.kind))
            .collect::<Vec<_>>(),
        [
            (0, Some(8), RecoveryKind::MissingFileColourFlag),
            (1, Some(40), RecoveryKind::MissingPageColourFlag)
        ]
    );
}

#[test]
fn rgb_transparency_preserves_existing_ink_and_monochrome_or_paints_black() {
    let bytes = document(
        true,
        &[
            page(true),
            region(1, 0, true, Some(6), &[]),
            region(2, 0, false, Some(4), &[]),
            region(3, 1, true, Some(4), &[]),
            region(4, 2, true, None, &[]),
            end(5),
        ],
    );
    let output = Decoder::default().decode_rgb(&bytes).unwrap();
    assert_eq!(
        output.pages()[0].bitmap().bytes(),
        &[0, 0, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    let outside = document(
        true,
        &[page(true), region(1, u32::MAX, true, Some(4), &[]), end(2)],
    );
    assert_eq!(
        Decoder::default().decode_rgb(&outside).unwrap().pages()[0]
            .bitmap()
            .bytes(),
        &[255; 12]
    );
    let bad_id = document(
        true,
        &[page(true), region(1, u32::MAX, true, Some(99), &[]), end(2)],
    );
    assert!(Decoder::default().decode_rgb(&bad_id).is_err());
}

#[test]
fn rgb_palette_references_and_truncation_remain_semantic_errors() {
    let mut payload = vec![0, 3, 1];
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&[13, 27, 41]);
    let palette = segment(1, 54, 0, &[], 1, &payload);
    let bytes = document(
        true,
        &[
            page(true),
            palette.clone(),
            region(2, 1, true, Some(32), &[1]),
            end(3),
        ],
    );
    let output = Decoder::default().decode_rgb(&bytes).unwrap();
    assert_eq!(output.pages()[0].bitmap().pixel(1, 0), Some([13, 27, 41]));
    let dead = segment(1, 54, 0, &[], 0, &payload);
    for bytes in [
        document(
            true,
            &[page(true), dead, region(2, 1, true, Some(4), &[1]), end(3)],
        ),
        document(
            true,
            &[
                page(true),
                palette.clone(),
                region(2, 1, true, Some(33), &[1]),
                end(3),
            ],
        ),
        document(
            true,
            &[page(true), region(1, 1, true, Some(32), &[2]), end(2)],
        ),
        document(
            true,
            &[
                page(true),
                segment(1, 54, 0, &[], 1, &payload[..payload.len() - 1]),
                end(2),
            ],
        ),
    ] {
        assert!(Decoder::default().decode_rgb(&bytes).is_err());
    }
    let mut truncated = bytes.clone();
    truncated.truncate(truncated.len() - 1);
    assert!(Decoder::default().decode_rgb(&truncated).is_err());
}

#[test]
fn rgb_limits_charge_three_channels_and_the_actual_reference_depth() {
    let bytes = document(
        true,
        &[page(true), region(1, 1, true, Some(4), &[]), end(2)],
    );
    let scan = 8 * ((bytes.len() as u64 / 11).pow(2) + bytes.len() as u64);
    let work = scan + 12 + 64 * 2 + 16 * 24;
    for kind in [
        LimitKind::DecodedBitmapBytes,
        LimitKind::Work,
        LimitKind::Memory,
        LimitKind::ReferenceDepth,
    ] {
        let mut limits = Limits::default();
        match kind {
            LimitKind::DecodedBitmapBytes => limits.max_cumulative_decoded_bitmap_bytes = 11,
            LimitKind::Work => limits.max_work_units = work - 1,
            LimitKind::Memory => limits.max_decoder_managed_memory_bytes = 0,
            LimitKind::ReferenceDepth => limits.max_reference_graph_depth = 0,
            _ => unreachable!(),
        }
        assert!(matches!(Decoder::new(limits).decode_rgb(&bytes),
            Err(DecodeError::LimitExceeded(detail)) if detail.kind == kind));
    }
    let mut limits = Limits::default();
    limits.max_cumulative_decoded_bitmap_bytes = 12;
    limits.max_work_units = work;
    assert!(Decoder::new(limits).decode_rgb(&bytes).is_ok());
}

#[test]
#[ignore = "requires hash-checked original colour fixture and independent full RGB oracle"]
fn rgb_original_colour_fixture_matches_every_reference_channel() {
    let input = std::fs::read(std::env::var("JBIG2_COLOUR_FIXTURE").unwrap()).unwrap();
    let expected = std::fs::read(std::env::var("JBIG2_COLOUR_RGB").unwrap()).unwrap();
    let decoder = Decoder::default();
    assert!(decoder.decode_rgb(&input).is_err());
    let output = decoder
        .decode_rgb_with_policy(&input, DecodePolicy::Compatibility)
        .unwrap();
    assert_eq!(output.value.pages().len(), 1);
    let page = &output.value.pages()[0];
    assert_eq!(
        (
            page.number(),
            page.bitmap().width(),
            page.bitmap().height(),
            page.bitmap().stride()
        ),
        (1, 163, 109, 489)
    );
    assert_eq!(page.bitmap().bytes(), expected);
    assert!(page
        .bitmap()
        .bytes()
        .chunks_exact(3)
        .any(|pixel| pixel == [255, 0, 0]));
    let mut conforming = input.clone();
    conforming[8] |= 8;
    conforming[40] |= 0x80;
    let strict = decoder
        .decode_rgb_with_policy(&conforming, DecodePolicy::Strict)
        .unwrap();
    assert!(strict.recoveries.is_empty());
    assert_eq!(strict.value.pages()[0].bitmap().bytes(), expected);
}
