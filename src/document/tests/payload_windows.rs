#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn raw_known_segment_dispatch_preserves_page_stripe_profile_variants_and_values() {
    let limits = Limits::default();
    let mut header = reference_validation_segment(0, &[], 1).header;
    let mut page = page_data(23);
    page[8..12].copy_from_slice(&0x0123_4567_u32.to_be_bytes());
    page[12..16].copy_from_slice(&0x89ab_cdef_u32.to_be_bytes());
    header.segment_type = SegmentType::PageInformation;
    match parse_known_segment_data(&header, &page, &limits).unwrap() {
        ParsedData::Page(info) => {
            assert_eq!(info.width, 8);
            assert_eq!(info.height, Some(23));
            assert_eq!(info.x_resolution, 0x0123_4567);
            assert_eq!(info.y_resolution, 0x89ab_cdef);
        }
        other => panic!("expected literal page metadata, got {other:?}"),
    }
    header.segment_type = SegmentType::EndOfStripe;
    assert!(matches!(
        parse_known_segment_data(&header, &0x0123_4567_u32.to_be_bytes(), &limits).unwrap(),
        ParsedData::EndOfStripe(0x0123_4567)
    ));
    header.segment_type = SegmentType::Profiles;
    let mut profiles = 3_u32.to_be_bytes().to_vec();
    for value in [0, u32::MAX, 0x0123_4567_u32] {
        profiles.extend_from_slice(&value.to_be_bytes());
    }
    match parse_known_segment_data(&header, &profiles, &limits).unwrap() {
        ParsedData::Profiles(values) => assert_eq!(values, [0, u32::MAX, 0x0123_4567]),
        other => panic!("expected literal profile metadata, got {other:?}"),
    }
    for kind in [SegmentType::EndOfPage, SegmentType::EndOfFile] {
        header.segment_type = kind;
        assert!(matches!(
            parse_known_segment_data(&header, &[], &limits).unwrap(),
            ParsedData::Other
        ));
        assert!(parse_known_segment_data(&header, &[1], &limits).is_err());
    }
    header.segment_type = SegmentType::Extension;
    assert!(matches!(
        parse_known_segment_data(&header, &0x100_u32.to_be_bytes(), &limits).unwrap(),
        ParsedData::Other
    ));
}

#[test]
fn raw_page_metadata_survives_sequential_random_and_embedded_container_composition() {
    let limits = Limits::default();
    let sequential = valid_sequential_document();
    let body = &sequential[13..];
    let mut random = SIGNATURE.to_vec();
    random.push(0);
    random.extend_from_slice(&1_u32.to_be_bytes());
    random.extend(header(0, 48, 1, 19));
    random.extend(header(1, 49, 1, 0));
    random.extend(header(2, 51, 0, 0));
    random.extend_from_slice(&page_data(8));
    let mut collections = vec![
        parse_sequential(body, &limits, true).unwrap(),
        parse_random_access(&random[13..], &limits).unwrap(),
    ];
    collections.push(parse_standalone_document(&sequential, &limits).unwrap().1);
    collections.push(parse_standalone_document(&random, &limits).unwrap().1);
    collections.push(parse_embedded_document(body, &limits, None).unwrap().1);
    for segments in collections {
        assert_eq!(segments.len(), 3);
        match &segments[0].data {
            ParsedData::Page(info) => {
                assert_eq!(info.width, 8);
                assert_eq!(info.height, Some(8));
                assert_eq!(info.x_resolution, 0);
                assert_eq!(info.y_resolution, 0);
            }
            other => panic!("expected original page metadata, got {other:?}"),
        }
        assert!(matches!(segments[1].data, ParsedData::Other));
        assert!(matches!(segments[2].data, ParsedData::Other));
    }
}

#[test]
fn raw_payload_windows_keep_exact_known_and_existing_unknown_extents() {
    for length in 0..=32_u32 {
        let mut header = reference_validation_segment(100, &[], 1).header;
        header.data_length = Some(length);
        let mut input = vec![0x11, 0x22, 0x33];
        input.extend((0..length).map(|index| u8::try_from(index * 7 % 256).unwrap()));
        input.extend_from_slice(&[0xaa, 0x55]);
        let mut reader = ByteReader::new(&input);
        reader.read_bytes(3).unwrap();
        let payload = read_payload(&mut reader, &header, &Limits::default()).unwrap();
        assert_eq!(payload, &input[3..3 + usize::try_from(length).unwrap()]);
        assert_eq!(reader.position(), 3 + usize::try_from(length).unwrap());
        assert_eq!(reader.remaining_bytes(), &[0xaa, 0x55]);
    }
    let mut existing = vec![0_u8; 18];
    existing[4..8].copy_from_slice(&3_u32.to_be_bytes());
    existing.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    existing.extend_from_slice(&[0x11, 0x22, 0xff, 0xac]);
    existing.extend_from_slice(&3_u32.to_be_bytes());
    existing.extend_from_slice(&[0xaa, 0xbb]);
    let mut input = vec![0x11, 0x22, 0x33];
    input.extend_from_slice(&existing);
    let mut header = reference_validation_segment(100, &[], 1).header;
    header.segment_type = SegmentType::ImmediateGenericRegion;
    header.data_length = None;
    let mut reader = ByteReader::new(&input);
    reader.read_bytes(3).unwrap();
    assert_eq!(
        read_payload(&mut reader, &header, &Limits::default()).unwrap(),
        &existing[..34]
    );
    assert_eq!(reader.position(), 37);
    assert_eq!(reader.remaining_bytes(), &[0xaa, 0xbb]);
}

#[test]
fn unknown_generic_framing_starts_after_all_adaptive_coordinates() {
    let limits = Limits::default();
    for (flags, adaptive_bytes) in [(0, 8), (2, 2), (4, 2), (6, 2), (16, 24), (1, 0)] {
        let mut bytes = vec![0_u8; 18];
        bytes[4..8].copy_from_slice(&3_u32.to_be_bytes());
        bytes[17] = flags;
        // Every pair is a valid preceding adaptive pixel and looks like a marker.
        bytes.extend(std::iter::repeat_n([0xff, 0xac], adaptive_bytes / 2).flatten());
        let body_start = bytes.len();
        let marker = if flags & 1 == 0 { [0xff, 0xac] } else { [0, 0] };
        bytes.extend_from_slice(&[0x13, 0x24, marker[0], marker[1]]);
        bytes.extend_from_slice(&3_u32.to_be_bytes());
        let end = bytes.len();
        bytes.extend_from_slice(&[0xaa, 0x55]);
        assert_eq!(find_unknown_generic_length(&bytes, &limits).unwrap(), end);
        for truncated in 0..end {
            assert!(
                find_unknown_generic_length(&bytes[..truncated], &limits).is_err(),
                "flags {flags}, truncation {truncated}, body {body_start}"
            );
        }
        bytes[end - 4..end].copy_from_slice(&4_u32.to_be_bytes());
        assert!(matches!(
            find_unknown_generic_length(&bytes, &limits),
            Err(DecodeError::Malformed(
                MalformedError::GenericRegionRowCount {
                    declared: 3,
                    actual: 4
                }
            ))
        ));
        bytes.truncate(body_start);
        assert!(find_unknown_generic_length(&bytes, &limits).is_err());
    }
}

#[test]
fn unknown_generic_framing_validates_the_header_before_scanning() {
    let mut bytes = vec![0_u8; 18];
    bytes[0..4].copy_from_slice(&8_u32.to_be_bytes());
    bytes[4..8].copy_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[0xff, 0xac, 0, 0, 0xff, 0xac, 0xff, 0xac]);
    bytes.extend_from_slice(&[0xff, 0xac, 0, 0, 0, 3]);
    assert!(matches!(
        find_unknown_generic_length(&bytes, &Limits::default()),
        Err(DecodeError::Malformed(
            MalformedError::InvalidGenericRegion(
                crate::GenericRegionErrorKind::InvalidAdaptivePixel {
                    number: 2,
                    x: 0,
                    y: 0
                }
            )
        ))
    ));
    let limits = Limits {
        max_width: 7,
        ..Limits::default()
    };
    assert!(matches!(
        find_unknown_generic_length(&bytes, &limits),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Width,
            ..
        }))
    ));
}

#[test]
fn raw_sequential_and_embedded_layouts_match_literal_windows_and_order_policy() {
    let limits = Limits::default();
    for numbers in [&[2_u32, 7, 42][..], &[42_u32, 7, 2][..]] {
        for eof in [false, true] {
            let (input, expected) = raw_extension_windows(numbers, eof);
            for ordered in [false, true] {
                let result = parse_sequential(&input, &limits, ordered);
                if ordered && numbers[0] > numbers[1] {
                    assert!(matches!(
                        result,
                        Err(DecodeError::Malformed(
                            MalformedError::NonIncreasingSegmentNumber {
                                previous: 42,
                                current: 7
                            }
                        ))
                    ));
                    continue;
                }
                let segments = result.unwrap();
                assert_eq!(segments.len(), expected.len());
                for (index, (segment, (number, payload, start, end))) in
                    segments.iter().zip(&expected).enumerate()
                {
                    assert_eq!(segment.header.number, *number);
                    assert_eq!(
                        segment.header.segment_type,
                        if index < numbers.len() {
                            SegmentType::Extension
                        } else {
                            SegmentType::EndOfFile
                        }
                    );
                    assert_eq!(segment.header.byte_length, 11);
                    assert_eq!(
                        segment.header.data_length,
                        Some(u32::try_from(payload.len()).unwrap())
                    );
                    assert_eq!(segment.payload, payload);
                    assert_eq!(segment.payload, &input[*start..*end]);
                }
            }
            let (info, embedded) = parse_embedded_document(&input, &limits, None).unwrap();
            assert_eq!(info.segment_count(), u32::try_from(expected.len()).unwrap());
            assert!(info.pages().is_empty());
            for (segment, (number, payload, _, _)) in embedded.iter().zip(&expected) {
                assert_eq!(segment.header.number, *number);
                assert_eq!(segment.payload, payload);
            }
            if eof {
                let mut trailing = input.clone();
                trailing.push(0xaa);
                assert!(matches!(
                    parse_sequential(&trailing, &limits, false),
                    Err(DecodeError::Malformed(MalformedError::TrailingData(1)))
                ));
            }
        }
    }
}

#[test]
fn standalone_sequential_body_origin_is_exact_and_unordered_headers_are_rejected() {
    let limits = Limits::default();
    let (body, expected) = raw_extension_windows(&[2, 7, 42], true);
    for known_count in [false, true] {
        let mut input = SIGNATURE.to_vec();
        input.push(if known_count { 1 } else { 3 });
        if known_count {
            input.extend_from_slice(&0_u32.to_be_bytes());
        }
        let body_start = input.len();
        input.extend_from_slice(&body);
        let (info, segments) = parse_standalone_document(&input, &limits).unwrap();
        assert_eq!(info.segment_count(), 4);
        assert!(info.pages().is_empty());
        for (segment, (number, payload, start, end)) in segments.iter().zip(&expected) {
            assert_eq!(segment.header.number, *number);
            assert_eq!(segment.payload, payload);
            assert_eq!(
                segment.payload,
                &input[body_start + *start..body_start + *end]
            );
        }
    }
    let (unordered, _) = raw_extension_windows(&[42, 7, 2], true);
    let mut input = SIGNATURE.to_vec();
    input.push(3);
    input.extend_from_slice(&unordered);
    assert!(matches!(
        parse_standalone_document(&input, &limits),
        Err(DecodeError::Malformed(
            MalformedError::NonIncreasingSegmentNumber {
                previous: 42,
                current: 7,
            }
        ))
    ));
}

#[test]
fn raw_random_access_layout_matches_separate_literal_headers_and_payloads() {
    let limits = Limits::default();
    for numbers in [
        &[][..],
        &[0][..],
        &[2, 7, 42][..],
        &[0, 255, 65_535, 16_777_215][..],
    ] {
        let (input, expected) = raw_random_extension_windows(numbers);
        let segments = parse_random_access(&input, &limits).unwrap();
        assert_eq!(segments.len(), expected.len());
        assert_eq!(expected[0].2, expected.len() * 11);
        for (index, (segment, (number, payload, start, end))) in
            segments.iter().zip(&expected).enumerate()
        {
            assert_eq!(segment.header.number, *number);
            assert_eq!(
                segment.header.segment_type,
                if index < numbers.len() {
                    SegmentType::Extension
                } else {
                    SegmentType::EndOfFile
                }
            );
            assert_eq!(segment.header.byte_length, 11);
            assert_eq!(
                segment.header.data_length,
                Some(u32::try_from(payload.len()).unwrap())
            );
            assert_eq!(segment.header.page_association, 0);
            assert!(segment.header.referred_segments.is_empty());
            assert_eq!(segment.payload, payload);
            assert_eq!(segment.payload, &input[*start..*end]);
        }
        assert_eq!(expected.last().unwrap().3, input.len());
    }
}

#[test]
fn standalone_random_access_origins_order_eof_and_trailing_bytes_match_raw_metadata() {
    let limits = Limits::default();
    let (body, expected) = raw_random_extension_windows(&[2, 7, 42]);
    for known_count in [false, true] {
        let mut input = SIGNATURE.to_vec();
        input.push(if known_count { 0 } else { 2 });
        if known_count {
            input.extend_from_slice(&0_u32.to_be_bytes());
        }
        let body_start = input.len();
        assert_eq!(body_start, if known_count { 13 } else { 9 });
        input.extend_from_slice(&body);
        let (info, segments) = parse_standalone_document(&input, &limits).unwrap();
        assert_eq!(info.organization(), Organization::RandomAccess);
        assert_eq!(info.segment_count(), 4);
        assert!(info.pages().is_empty());
        assert_eq!(segments.len(), expected.len());
        for (segment, (number, payload, start, end)) in segments.iter().zip(&expected) {
            assert_eq!(segment.header.number, *number);
            assert_eq!(segment.payload, payload);
            assert_eq!(
                segment.payload,
                &input[body_start + *start..body_start + *end]
            );
        }
    }
    for numbers in [&[42, 7, 2][..], &[2, 2][..]] {
        let (unordered, _) = raw_random_extension_windows(numbers);
        assert!(matches!(
            parse_random_access(&unordered, &limits),
            Err(DecodeError::Malformed(
                MalformedError::NonIncreasingSegmentNumber { previous, current }
            )) if previous == numbers[0] && current == numbers[1]
        ));
    }
    for missing in [Vec::new(), header(2, 62, 0, 4)] {
        assert!(matches!(
            parse_random_access(&missing, &limits),
            Err(DecodeError::Malformed(MalformedError::MissingEndOfFile))
        ));
    }
    let mut trailing = body;
    trailing.push(0xaa);
    assert!(matches!(
        parse_random_access(&trailing, &limits),
        Err(DecodeError::Malformed(MalformedError::TrailingData(1)))
    ));
    let mut early_eof = header(0, 51, 0, 0);
    early_eof.extend(header(1, 51, 0, 0));
    assert!(matches!(
        parse_random_access(&early_eof, &limits),
        Err(DecodeError::Malformed(MalformedError::TrailingData(11)))
    ));
}

#[test]
fn raw_structural_work_matches_wide_counts_and_exact_rejection_priority() {
    for count in 0..=2 {
        for references in [&[][..], &[0][..], &[0, 1][..]] {
            let segments: Vec<_> = (0..count)
                .map(|index| reference_validation_segment(index + 2, references, 1))
                .collect();
            let headers: Vec<_> = segments
                .iter()
                .map(|segment| segment.header.clone())
                .collect();
            for input in [
                0,
                1,
                8,
                usize::MAX / 2,
                usize::MAX - 3,
                usize::MAX - 1,
                usize::MAX,
            ] {
                let expected = input as u128
                    + u128::from(count)
                    + u128::from(count) * references.len() as u128;
                for limit in [0, 1, 8, u64::MAX] {
                    let limits = Limits {
                        max_work_units: limit,
                        ..Limits::default()
                    };
                    let result = enforce_work_limit(input, &segments, &limits);
                    if expected > u128::from(u64::MAX) {
                        assert!(matches!(
                            result,
                            Err(DecodeError::Malformed(MalformedError::IntegerOverflow))
                        ));
                    } else if expected > u128::from(limit) {
                        assert!(matches!(result, Err(DecodeError::LimitExceeded(
                            LimitExceeded { kind: LimitKind::Work, limit: actual_limit, actual }
                        )) if actual_limit == limit && u128::from(actual) == expected));
                    } else {
                        result.unwrap();
                    }
                    assert_eq!(
                        segments
                            .iter()
                            .map(|s| s.header.clone())
                            .collect::<Vec<_>>(),
                        headers
                    );
                }
            }
        }
    }
    for count in [0, 1, usize::MAX / 8, usize::MAX / 8 + 1, usize::MAX] {
        let bytes = count as u128 * 8;
        match allocation_size::<u64>(count) {
            Ok(actual) => assert_eq!(u128::from(actual), bytes),
            Err(MalformedError::IntegerOverflow) => {
                assert!(bytes > usize::MAX as u128 || bytes > u128::from(u64::MAX));
            }
            other => panic!("unexpected allocation arithmetic: {other:?}"),
        }
        assert_eq!(allocation_size::<()>(count).unwrap(), 0);
    }
}

#[test]
fn raw_profile_and_retention_metadata_rejects_inconsistent_tags_and_frames() {
    let mut global = reference_validation_segment(1, &[], 1);
    global.header.segment_type = SegmentType::Profiles;
    global.data = ParsedData::Other;
    assert!(matches!(
        validate_profiles(&[global]),
        Err(DecodeError::Malformed(
            MalformedError::InvalidProfilesSegment(1)
        ))
    ));
    let mut page = reference_validation_segment(2, &[], 1);
    page.header.segment_type = SegmentType::Profiles;
    page.header.page_association = 1;
    let mut profiles = Vec::new();
    assert!(matches!(
        validate_page_profiles(&[page], 0, &[17], &mut profiles),
        Err(DecodeError::Malformed(
            MalformedError::InvalidProfilesSegment(2)
        ))
    ));
    assert_eq!(profiles, vec![(1, 2)]);
    for number in [0, 1, u32::MAX] {
        for retained in [false, true] {
            for last in [number.saturating_sub(1), number, number.saturating_add(1)] {
                let mut segment = reference_validation_segment(number, &[], u8::from(retained));
                let before = segment.header.clone();
                let result =
                    validate_retention_flags(std::slice::from_ref(&segment), &[(number, last)]);
                if retained && last > number {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(DecodeError::Malformed(
                        MalformedError::InvalidRetentionFlags { segment: owner, referred }
                    )) if owner == number && referred == number));
                }
                assert_eq!(segment.header, before);
                segment.header.retention_flags.values.clear();
                assert!(matches!(validate_retention_flags(&[segment], &[]),
                    Err(DecodeError::Malformed(MalformedError::InvalidRetentionFlags {
                        segment: owner, referred,
                    })) if owner == number && referred == number));
            }
        }
    }
}
