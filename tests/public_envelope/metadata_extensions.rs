#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn profile_supersets_and_empty_profile_sets_are_accepted_without_changing_pixels() {
    let decoder = Decoder::default();
    for (global, local) in [
        (vec![], vec![]),
        (vec![], vec![0xdead_beef]),
        (
            vec![1, 0x1020_3040, u32::MAX],
            vec![9, u32::MAX, 0x1020_3040, 1, 9],
        ),
    ] {
        let segments = [
            profiles(0, 0, &global),
            page(1, 1, 9, 2),
            profiles(2, 1, &local),
            end(3, 1),
            eof(4),
        ];
        for sequential in [true, false] {
            let bytes = standalone(sequential, Some(1), &segments);
            let document = decoder.decode(&bytes).unwrap();
            assert_info(
                document.info(),
                if sequential {
                    Organization::Sequential
                } else {
                    Organization::RandomAccess
                },
                Some(1),
                5,
                &[(1, 9, 2, 300, 600, false)],
            );
            assert_pixels(&document.pages()[0], 1, 9, 2, &[0; 4]);
        }
        let globals = decoder.parse_globals(&embedded(&segments[..1])).unwrap();
        assert_eq!(globals.segment_count(), 1);
        let local = embedded(&[
            segments[1].clone(),
            segments[3].clone(),
            segments[4].clone(),
        ]);
        let document = decoder
            .decode_embedded_with_globals(&local, &globals)
            .unwrap();
        assert_info(
            document.info(),
            Organization::Sequential,
            None,
            3,
            &[(1, 9, 2, 300, 600, false)],
        );
        assert_pixels(&document.pages()[0], 1, 9, 2, &[0; 4]);
    }
}

#[test]
fn misplaced_duplicate_and_non_superset_profiles_have_exact_segment_numbers() {
    let cases = [
        (
            vec![extension(0, 0, 0, &[]), profiles(1, 0, &[]), eof(2)],
            1,
        ),
        (vec![profiles(0, 1, &[]), eof(1)], 0),
        (vec![profiles(0, 0, &[]), profiles(1, 0, &[]), eof(2)], 1),
        (
            vec![
                profiles(0, 0, &[]),
                page(1, 1, 8, 1),
                profiles(2, 1, &[]),
                profiles(3, 1, &[]),
                end(4, 1),
                eof(5),
            ],
            3,
        ),
        (
            vec![
                profiles(0, 0, &[1, 2]),
                page(1, 1, 8, 1),
                profiles(2, 1, &[3, 1]),
                end(3, 1),
                eof(4),
            ],
            2,
        ),
        (
            vec![
                profiles(0, 0, &[1]),
                page(1, 1, 8, 1),
                profiles(2, 1, &[]),
                end(3, 1),
                eof(4),
            ],
            2,
        ),
    ];
    for (segments, number) in cases {
        reject(&segments, &MalformedError::InvalidProfilesSegment(number));
    }
}

#[test]
fn profile_lengths_report_exact_u64_expected_lengths_before_allocation() {
    for length in 0..4 {
        reject(
            &[Segment::new(0, 52, 0, vec![0; length]), eof(1)],
            &MalformedError::InvalidSegmentDataLength {
                segment_type: 52,
                expected: 4,
                actual: length as u64,
            },
        );
    }
    for (payload, expected) in [
        (1_u32.to_be_bytes().to_vec(), 8),
        (u32::MAX.to_be_bytes().to_vec(), 17_179_869_184),
        (vec![0; 8], 4),
    ] {
        let actual = payload.len() as u64;
        reject(
            &[Segment::new(0, 52, 0, payload), eof(1)],
            &MalformedError::InvalidSegmentDataLength {
                segment_type: 52,
                expected,
                actual,
            },
        );
    }
}

#[test]
fn optional_extensions_and_both_comment_encodings_preserve_literal_output() {
    let mut extensions = Vec::new();
    for (index, kind) in [0, 1, 0x2000_0001, 0x6000_0000, 0x7fff_ffff]
        .into_iter()
        .enumerate()
    {
        extensions.push(extension(
            u32::try_from(index).unwrap(),
            0,
            kind,
            &[0xff, 0, 0x80],
        ));
    }
    extensions.push(extension(
        5,
        0,
        0x2000_0000,
        b"author\0project\0title\0generated\0\0",
    ));
    extensions.push(extension(6, 0, 0x2000_0000, &[0]));
    extensions.push(extension(7, 0, 0x2000_0002, &[0, 0]));
    extensions.push(extension(
        8,
        0,
        0x2000_0002,
        &[0x01, 0, 0, 0, 0, 0x41, 0, 0, 0, 0],
    ));
    extensions.extend([page(9, 1, 9, 1), end(10, 1), eof(11)]);
    for sequential in [true, false] {
        let document = Decoder::default()
            .decode(&standalone(sequential, Some(1), &extensions))
            .unwrap();
        assert_info(
            document.info(),
            if sequential {
                Organization::Sequential
            } else {
                Organization::RandomAccess
            },
            Some(1),
            12,
            &[(1, 9, 1, 300, 600, false)],
        );
        assert_pixels(&document.pages()[0], 1, 9, 1, &[0, 0]);
    }
}

#[test]
fn malformed_comment_framing_is_not_an_ignored_optional_extension() {
    for data in [
        &b""[..],
        &b"x"[..],
        &b"key\0\0"[..],
        &b"k\0v\0"[..],
        &b"\0trailing"[..],
        &b"k\0v\0\0trailing"[..],
    ] {
        reject(
            &[extension(0, 0, 0x2000_0000, data), eof(1)],
            &MalformedError::InvalidCommentExtension,
        );
    }
    for data in [
        &[][..],
        &[0][..],
        &[0, 1][..],
        &[0, 1, 0, 0][..],
        &[0, 1, 0, 0, 0, 0][..],
        &[0, 0, 0, 1][..],
    ] {
        reject(
            &[extension(0, 0, 0x2000_0002, data), eof(1)],
            &MalformedError::InvalidCommentExtension,
        );
    }
}

#[test]
fn extension_length_and_necessary_flag_errors_preserve_all_bits() {
    for length in 0..4 {
        reject(
            &[Segment::new(0, 62, 0, vec![0; length]), eof(1)],
            &MalformedError::InvalidSegmentDataLength {
                segment_type: 62,
                expected: 4,
                actual: length as u64,
            },
        );
    }
    for kind in [0x8000_0000, 0xc123_4567] {
        reject(
            &[extension(0, 0, kind, &[]), eof(1)],
            &MalformedError::InvalidExtensionType(kind),
        );
    }
    for kind in [0xa000_0000, 0xa000_0001, 0xffff_ffff] {
        let segments = [extension(0, 0, kind, &[]), eof(1)];
        for sequential in [true, false] {
            let decoder = Decoder::default();
            let bytes = standalone(sequential, None, &segments);
            for error in [
                decoder.inspect(&bytes).unwrap_err(),
                decoder.decode(&bytes).unwrap_err(),
            ] {
                assert!(matches!(
                    error, DecodeError::Unsupported(UnsupportedFeature::Extension(actual)) if actual == kind
                ));
            }
        }
    }
}
