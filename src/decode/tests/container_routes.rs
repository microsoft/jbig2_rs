#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn pdfium_completion_is_not_enabled_for_the_standalone_dispatcher() {
    let limits = Limits::default();
    let bytes = document_with_generic(&[0x80]);
    let (info, mut segments) = parse_standalone_document(&bytes, &limits).unwrap();
    segments.retain(|segment| {
        !matches!(
            segment.header.segment_type,
            SegmentType::EndOfPage | SegmentType::EndOfFile
        )
    });
    let work = structural_work(bytes.len(), &segments).unwrap();
    let mut calls = 0;
    let result = decode_segments(
        &segments,
        &info,
        work,
        0,
        &limits,
        &mut MmrScratch::default(),
        None,
        &mut |_| {
            calls += 1;
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(DecodeError::Malformed(MalformedError::InvalidPageState(
            "active page after final segment"
        )))
    ));
    assert_eq!(calls, 0);
    let mut collector = PageCollector::new(&limits);
    let result = decode_segments_collected(
        &segments,
        &info,
        work,
        0,
        &limits,
        &mut MmrScratch::default(),
        None,
        &mut collector,
        false,
    );
    assert!(matches!(
        result,
        Err(DecodeError::Malformed(MalformedError::InvalidPageState(
            "active page after final segment"
        )))
    ));
    assert!(collector.pages.is_empty());
}

#[test]
fn public_results_preserve_exact_standalone_and_embedded_metadata() {
    let decoder = Decoder::default();
    let sequential = all_white_mmr_document(9, 3);
    let mut unknown_count = SIGNATURE.to_vec();
    unknown_count.push(0x03);
    unknown_count.extend_from_slice(&sequential[13..]);
    let generic = generic_data(&[0x80]);
    let mut random = SIGNATURE.to_vec();
    random.push(0);
    random.extend_from_slice(&1_u32.to_be_bytes());
    random.extend_from_slice(&segment_header(0, 48, 1, 19));
    random.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).unwrap(),
    ));
    random.extend_from_slice(&segment_header(2, 49, 1, 0));
    random.extend_from_slice(&segment_header(3, 51, 0, 0));
    random.extend_from_slice(&page_data());
    random.extend_from_slice(&generic);
    let check = |document: &DecodedDocument, organization, count, width, height| {
        let info = document.info();
        assert_eq!(info.organization(), organization);
        assert_eq!(info.declared_page_count(), count);
        assert_eq!(info.segment_count(), 4);
        assert_eq!(info.pages().len(), 1);
        assert_eq!(info.pages()[0].number(), 1);
        assert_eq!(info.pages()[0].width(), width);
        assert_eq!(info.pages()[0].height(), height);
        assert_eq!(document.pages().len(), 1);
        assert_eq!(document.pages()[0].number(), 1);
        assert_eq!(document.pages()[0].bitmap().width(), width);
        assert_eq!(document.pages()[0].bitmap().height(), height);
    };
    for (input, organization, count, width, height) in [
        (&sequential, crate::Organization::Sequential, Some(1), 9, 3),
        (&unknown_count, crate::Organization::Sequential, None, 9, 3),
        (&random, crate::Organization::RandomAccess, Some(1), 8, 1),
    ] {
        for result in [
            decoder.decode(input),
            decoder.context().decode(input),
            decoder.context().decode_internal(input),
        ] {
            check(&result.unwrap(), organization, count, width, height);
        }
    }
    let globals = decoder.parse_globals(&[]).unwrap();
    let embedded = &sequential[13..];
    for result in [
        decoder.decode_embedded(embedded),
        decoder.decode_embedded_with_globals(embedded, &globals),
        decoder.context().decode_embedded(embedded),
        decoder
            .context()
            .decode_embedded_with_globals(embedded, &globals),
        decoder.decode_embedded_internal(embedded, Some(&globals), &mut MmrScratch::default()),
    ] {
        check(
            &result.unwrap(),
            crate::Organization::Sequential,
            None,
            9,
            3,
        );
    }
}

#[test]
fn page_sink_receives_completed_page() {
    let mut pages = 0;
    let mut sink = |page: Page| {
        pages += 1;
        assert_eq!(page.bitmap().width(), 8);
        Ok(())
    };
    Decoder::default()
        .decode_with_sink(&document_with_generic(&[0x80]), &mut sink)
        .expect("page streams");
    assert_eq!(pages, 1);
}

#[test]
fn decodes_random_access_generic_page() {
    let generic = generic_data(&[0x80]);
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x00);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&segment_header(2, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(3, 51, 0, 0));
    bytes.extend_from_slice(&page_data());
    bytes.extend_from_slice(&generic);

    let decoded = Decoder::default()
        .decode(&bytes)
        .expect("random access decodes");
    assert_eq!(
        decoded.info().organization(),
        crate::Organization::RandomAccess
    );
    assert_eq!(decoded.pages()[0].bitmap().bytes(), &[0]);
}

#[test]
fn sequential_and_random_access_outputs_match() {
    let sequential = Decoder::default()
        .decode(&document_with_generic(&[0x80]))
        .expect("sequential decodes");

    let generic = generic_data(&[0x80]);
    let mut random = SIGNATURE.to_vec();
    random.push(0x00);
    random.extend_from_slice(&1_u32.to_be_bytes());
    random.extend_from_slice(&segment_header(0, 48, 1, 19));
    random.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("payload fits"),
    ));
    random.extend_from_slice(&segment_header(2, 49, 1, 0));
    random.extend_from_slice(&segment_header(3, 51, 0, 0));
    random.extend_from_slice(&page_data());
    random.extend_from_slice(&generic);
    let random = Decoder::default().decode(&random).expect("random decodes");

    assert_eq!(sequential.pages()[0].bitmap(), random.pages()[0].bitmap());
}

#[test]
fn decodes_unknown_length_mmr_region_and_row_count() {
    let mut page = page_data();
    page[4..8].copy_from_slice(&2_u32.to_be_bytes());
    let mut generic = Vec::new();
    generic.extend_from_slice(&8_u32.to_be_bytes());
    generic.extend_from_slice(&2_u32.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.push(0);
    generic.push(1);
    generic.extend_from_slice(&[0x80, 0x08, 0x00, 0x80]);
    generic.extend_from_slice(&[0x00, 0x00]);
    generic.extend_from_slice(&1_u32.to_be_bytes());

    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&segment_header(1, 38, 1, u32::MAX));
    bytes.extend_from_slice(&generic);
    bytes.extend_from_slice(&segment_header(2, 49, 1, 0));

    let decoded = Decoder::default()
        .decode(&bytes)
        .expect("unknown-length region decodes");
    assert_eq!(decoded.pages()[0].bitmap().height(), 2);
    assert_eq!(decoded.pages()[0].bitmap().bytes(), &[0, 0]);
}

#[test]
fn decodes_unknown_height_striped_page() {
    let mut page = page_data();
    page[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    page[17..19].copy_from_slice(&0x8001_u16.to_be_bytes());
    let generic = generic_data(&[0x80]);

    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&generic);
    bytes.extend_from_slice(&segment_header(2, 50, 1, 4));
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(3, 49, 1, 0));

    let decoded = Decoder::default()
        .decode(&bytes)
        .expect("striped page decodes");
    assert_eq!(decoded.pages()[0].bitmap().height(), 1);
}

#[test]
fn bounded_reader_decode_matches_slice_decode() {
    let bytes = document_with_generic(&[0x80]);
    let expected = Decoder::default().decode(&bytes).expect("slice decodes");
    let mut reader = std::io::Cursor::new(bytes);
    let actual = Decoder::default()
        .decode_reader(&mut reader)
        .expect("reader decodes");
    assert_eq!(actual, expected);
}

#[test]
fn embedded_decode_matches_standalone_page() {
    let bytes = document_with_generic(&[0x80]);
    let standalone = Decoder::default()
        .decode(&bytes)
        .expect("standalone decodes");
    let embedded = Decoder::default()
        .decode_embedded(&bytes[13..])
        .expect("embedded stream decodes");
    assert_eq!(embedded.pages(), standalone.pages());
}

#[test]
fn decodes_mmr_pattern_dictionary_and_halftone_page() {
    let mut pattern = vec![0x01, 0x01, 0x01, 0, 0, 0, 0];
    pattern.extend_from_slice(&[0x26, 0xA8]);

    let mut halftone = Vec::new();
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&0_u32.to_be_bytes());
    halftone.extend_from_slice(&0_u32.to_be_bytes());
    halftone.push(0);
    halftone.push(0x01);
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&0_i32.to_be_bytes());
    halftone.extend_from_slice(&0_i32.to_be_bytes());
    halftone.extend_from_slice(&256_u16.to_be_bytes());
    halftone.extend_from_slice(&0_u16.to_be_bytes());

    let mut page = page_data();
    page[0..4].copy_from_slice(&1_u32.to_be_bytes());

    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    let mut pattern_header = segment_header(
        0,
        16,
        0,
        u32::try_from(pattern.len()).expect("pattern payload fits"),
    );
    pattern_header[5] = 1;
    bytes.extend_from_slice(&pattern_header);
    bytes.extend_from_slice(&pattern);
    bytes.extend_from_slice(&segment_header(1, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&referenced_segment_header(
        2,
        22,
        1,
        0,
        u32::try_from(halftone.len()).expect("halftone payload fits"),
    ));
    bytes.extend_from_slice(&halftone);
    bytes.extend_from_slice(&segment_header(3, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(4, 51, 0, 0));

    let document = Decoder::default().decode(&bytes).expect("halftone decodes");
    assert_eq!(document.pages()[0].bitmap().bytes(), &[0x80]);
}

#[test]
fn embedded_halftone_uses_reusable_global_pattern_dictionary() {
    let mut pattern = vec![0x01, 0x01, 0x01, 0, 0, 0, 0];
    pattern.extend_from_slice(&[0x26, 0xA8]);
    let mut global_bytes = segment_header(
        0,
        16,
        0,
        u32::try_from(pattern.len()).expect("pattern payload fits"),
    );
    global_bytes.extend_from_slice(&pattern);
    let globals = Decoder::default()
        .parse_globals(&global_bytes)
        .expect("globals validate");

    let mut halftone = Vec::new();
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&0_u32.to_be_bytes());
    halftone.extend_from_slice(&0_u32.to_be_bytes());
    halftone.push(0);
    halftone.push(0x01);
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&1_u32.to_be_bytes());
    halftone.extend_from_slice(&0_i32.to_be_bytes());
    halftone.extend_from_slice(&0_i32.to_be_bytes());
    halftone.extend_from_slice(&256_u16.to_be_bytes());
    halftone.extend_from_slice(&0_u16.to_be_bytes());

    let mut page = page_data();
    page[0..4].copy_from_slice(&1_u32.to_be_bytes());
    let mut embedded = segment_header(1, 48, 1, 19);
    embedded.extend_from_slice(&page);
    embedded.extend_from_slice(&referenced_segment_header(
        2,
        22,
        1,
        0,
        u32::try_from(halftone.len()).expect("halftone payload fits"),
    ));
    embedded.extend_from_slice(&halftone);
    embedded.extend_from_slice(&segment_header(3, 49, 1, 0));

    let document = Decoder::default()
        .decode_embedded_with_globals(&embedded, &globals)
        .expect("global pattern resolves");
    assert_eq!(document.pages()[0].bitmap().bytes(), &[0x80]);
}

#[test]
fn decodes_symbol_dictionary_and_arithmetic_text_region() {
    const ANNEX_H_ENCODED: [u8; 30] = [
        0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb,
        0x86, 0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
    ];
    let mut dictionary = vec![0x00, 0x00];
    dictionary.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    dictionary.extend_from_slice(&0_u32.to_be_bytes());
    dictionary.extend_from_slice(&0_u32.to_be_bytes());
    dictionary.extend_from_slice(&[0xff, 0xac]);

    let mut text = vec![0_u8; 17];
    text[3] = 1;
    text[7] = 1;
    text.extend_from_slice(&0_u16.to_be_bytes());
    text.extend_from_slice(&0_u32.to_be_bytes());
    text.extend_from_slice(&ANNEX_H_ENCODED);

    let mut page = page_data();
    page[0..4].copy_from_slice(&1_u32.to_be_bytes());
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    let mut dictionary_header = segment_header(
        0,
        0,
        0,
        u32::try_from(dictionary.len()).expect("dictionary payload fits"),
    );
    dictionary_header[5] = 1;
    bytes.extend_from_slice(&dictionary_header);
    bytes.extend_from_slice(&dictionary);
    bytes.extend_from_slice(&segment_header(1, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&referenced_segment_header(
        2,
        6,
        1,
        0,
        u32::try_from(text.len()).expect("text payload fits"),
    ));
    bytes.extend_from_slice(&text);
    bytes.extend_from_slice(&segment_header(3, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(4, 51, 0, 0));

    let document = Decoder::default()
        .decode(&bytes)
        .expect("text document decodes");
    assert_eq!(document.pages()[0].bitmap().bytes(), &[0]);
}

#[test]
fn embedded_text_uses_reusable_global_symbol_dictionary() {
    const ANNEX_H_ENCODED: [u8; 30] = [
        0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb,
        0x86, 0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
    ];
    let mut dictionary = vec![0x00, 0x00];
    dictionary.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    dictionary.extend_from_slice(&0_u32.to_be_bytes());
    dictionary.extend_from_slice(&0_u32.to_be_bytes());
    dictionary.extend_from_slice(&[0xff, 0xac]);
    let mut globals_bytes = segment_header(
        0,
        0,
        0,
        u32::try_from(dictionary.len()).expect("dictionary payload fits"),
    );
    globals_bytes.extend_from_slice(&dictionary);
    let globals = Decoder::default()
        .parse_globals(&globals_bytes)
        .expect("global dictionary validates");

    let mut text = vec![0_u8; 17];
    text[3] = 1;
    text[7] = 1;
    text.extend_from_slice(&0_u16.to_be_bytes());
    text.extend_from_slice(&0_u32.to_be_bytes());
    text.extend_from_slice(&ANNEX_H_ENCODED);
    let mut page = page_data();
    page[0..4].copy_from_slice(&1_u32.to_be_bytes());
    let mut embedded = segment_header(1, 48, 1, 19);
    embedded.extend_from_slice(&page);
    embedded.extend_from_slice(&referenced_segment_header(
        2,
        6,
        1,
        0,
        u32::try_from(text.len()).expect("text payload fits"),
    ));
    embedded.extend_from_slice(&text);
    embedded.extend_from_slice(&segment_header(3, 49, 1, 0));

    let document = Decoder::default()
        .decode_embedded_with_globals(&embedded, &globals)
        .expect("global symbols resolve");
    assert_eq!(document.pages()[0].bitmap().bytes(), &[0]);
}

#[test]
fn decodes_huffman_symbol_dictionary_and_text_instance() {
    let mut dictionary = vec![0x00, 0x01];
    dictionary.extend_from_slice(&1_u32.to_be_bytes());
    dictionary.extend_from_slice(&1_u32.to_be_bytes());
    dictionary.extend_from_slice(&[0x5f, 0x80, 0x80, 0x00, 0x40]);

    let mut text = vec![0_u8; 17];
    text[3] = 1;
    text[7] = 1;
    text.extend_from_slice(&1_u16.to_be_bytes());
    text.extend_from_slice(&0_u16.to_be_bytes());
    text.extend_from_slice(&1_u32.to_be_bytes());
    text.push(0x01);
    text.extend_from_slice(&[0_u8; 19]);
    *text.last_mut().expect("Huffman entropy byte") = 0x04; // Final Table H OOB, then zero padding.

    let mut page = page_data();
    page[0..4].copy_from_slice(&1_u32.to_be_bytes());
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    let mut dictionary_header = segment_header(
        0,
        0,
        0,
        u32::try_from(dictionary.len()).expect("dictionary payload fits"),
    );
    dictionary_header[5] = 1;
    bytes.extend_from_slice(&dictionary_header);
    bytes.extend_from_slice(&dictionary);
    bytes.extend_from_slice(&segment_header(1, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&referenced_segment_header(
        2,
        6,
        1,
        0,
        u32::try_from(text.len()).expect("text payload fits"),
    ));
    let entropy_tail = bytes.len() + text.len() - 1;
    bytes.extend_from_slice(&text);
    bytes.extend_from_slice(&segment_header(3, 49, 1, 0));

    let document = Decoder::default()
        .decode(&bytes)
        .expect("Huffman text document decodes");
    assert_eq!(document.pages()[0].bitmap().bytes(), &[0x80]);
    bytes[entropy_tail] = 0;
    assert!(matches!(
        Decoder::default().decode(&bytes),
        Err(DecodeError::Malformed(MalformedError::InvalidSymbolText(
            "too many text instances"
        )))
    ));
}

#[test]
fn rejects_region_that_modifies_a_sealed_stripe() {
    let mut page = page_data();
    page[4..8].copy_from_slice(&2_u32.to_be_bytes());
    page[17..19].copy_from_slice(&0x8002_u16.to_be_bytes());
    let first = generic_data(&[0x80]);
    let second = generic_data(&[0x80]);

    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(first.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&first);
    bytes.extend_from_slice(&segment_header(2, 50, 1, 4));
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(
        3,
        38,
        1,
        u32::try_from(second.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&second);
    bytes.extend_from_slice(&segment_header(4, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(5, 51, 0, 0));

    assert!(matches!(
        Decoder::default().decode(&bytes),
        Err(DecodeError::Malformed(MalformedError::InvalidPageState(
            "region modifies a sealed stripe"
        )))
    ));
}

#[test]
fn decoder_enforces_cumulative_retained_state_limit() {
    let limits = Limits {
        max_retained_state_bytes: 0,
        ..Limits::default()
    };
    assert!(matches!(
        Decoder::new(limits).decode(&document_with_generic(&[0x80])),
        Err(DecodeError::LimitExceeded(crate::LimitExceeded {
            kind: crate::LimitKind::RetainedState,
            limit: 0,
            actual: 1
        }))
    ));
}

#[test]
fn owned_document_enforces_cumulative_page_memory() {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page_data());
    bytes.extend_from_slice(&segment_header(1, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(2, 48, 2, 19));
    bytes.extend_from_slice(&page_data());
    bytes.extend_from_slice(&segment_header(3, 49, 2, 0));

    let reserved = Bitmap::blank(8, 1, &Limits::default())
        .unwrap()
        .allocated_bytes();
    let limits = Limits {
        max_decoder_managed_memory_bytes: reserved,
        ..Limits::default()
    };
    let mut emitted = 0;
    Decoder::new(limits)
        .decode_with_sink(&bytes, &mut |_page: Page| {
            emitted += 1;
            Ok(())
        })
        .expect("streaming sink releases each page");
    assert_eq!(emitted, 2);
    assert!(matches!(
        Decoder::new(limits).decode(&bytes),
        Err(DecodeError::LimitExceeded(crate::LimitExceeded {
            kind: crate::LimitKind::Memory,
            limit,
            actual
        })) if limit == reserved && actual == reserved * 2
    ));
}
