#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn page_lifecycle_rejections_are_exact_and_precede_all_sink_effects() {
    let cases = [
        (
            vec![end(0, 1), eof(1)],
            MalformedError::MissingPageInformation(1),
        ),
        (
            vec![page(0, 1, 8, 1), page(1, 1, 8, 1), end(2, 1), eof(3)],
            MalformedError::DuplicatePageBoundary {
                page: 1,
                segment_type: 48,
            },
        ),
        (
            vec![page(0, 1, 8, 1), end(1, 1), end(2, 1), eof(3)],
            MalformedError::SegmentAfterEndOfPage {
                segment: 2,
                page: 1,
            },
        ),
        (
            vec![page(0, 1, 8, 1), end(1, 1), extension(2, 1, 0, &[]), eof(3)],
            MalformedError::SegmentAfterEndOfPage {
                segment: 2,
                page: 1,
            },
        ),
        (
            vec![page(0, 2, 8, 1), end(1, 2), eof(2)],
            MalformedError::MissingPageInformation(1),
        ),
        (
            vec![page(0, 0, 8, 1), eof(1)],
            MalformedError::InvalidPageAssociation {
                segment: 0,
                page: 0,
            },
        ),
        (
            vec![Segment::new(0, 51, 257, vec![])],
            MalformedError::InvalidPageAssociation {
                segment: 0,
                page: 257,
            },
        ),
    ];
    for (segments, expected) in cases {
        reject(&segments, &expected);
    }
}

fn white_row(number: u32, y: u32) -> Segment {
    let mut payload = 9_u32.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&y.to_be_bytes());
    payload.extend_from_slice(&[4, 1, 0x80]); // Replace with an MMR-coded white row.
    Segment::new(number, 38, 1, payload)
}

fn implicit_page(unknown_height: bool) -> Vec<Segment> {
    let mut first = if unknown_height {
        striped_page(1, u32::MAX, 2)
    } else {
        page(1, 1, 9, 3)
    };
    first.payload[16] = 0x44; // Black default, per-region combination override.
    let mut segments = vec![first, white_row(2, 1)];
    if unknown_height {
        segments.push(stripe(3, 2));
    }
    segments
}

#[test]
fn clean_embedded_stream_end_completes_the_final_page_with_and_without_globals() {
    let decoder = Decoder::default();
    let globals = decoder
        .parse_globals(&embedded(&[extension(0, 0, 0, &[])]))
        .unwrap();
    for unknown_height in [false, true] {
        for explicit_end in [false, true] {
            for file_end in [false, true] {
                let mut segments = implicit_page(unknown_height);
                if explicit_end {
                    segments.push(end(4, 1));
                }
                if file_end {
                    segments.push(eof(5));
                }
                let bytes = embedded(&segments);
                let info = decoder.inspect_embedded(&bytes).unwrap();
                let document = decoder.decode_embedded(&bytes).unwrap();
                assert_eq!(document.info(), &info);
                assert_eq!(info.pages().len(), 1);
                assert_eq!(info.pages()[0].is_striped(), unknown_height);
                assert_eq!(info.segment_count(), u32::try_from(segments.len()).unwrap());
                assert_pixels(
                    &document.pages()[0],
                    1,
                    9,
                    3,
                    &[0xff, 0x80, 0, 0, 0xff, 0x80],
                );
                let mut context = decoder.context();
                assert_eq!(context.inspect_embedded(&bytes).unwrap(), info);
                assert_eq!(context.decode_embedded(&bytes).unwrap(), document);
                assert_eq!(
                    decoder
                        .inspect_embedded_with_globals(&bytes, &globals)
                        .unwrap(),
                    info
                );
                assert_eq!(
                    decoder
                        .decode_embedded_with_globals(&bytes, &globals)
                        .unwrap(),
                    document
                );
                assert_eq!(
                    context
                        .inspect_embedded_with_globals(&bytes, &globals)
                        .unwrap(),
                    info
                );
                assert_eq!(
                    context
                        .decode_embedded_with_globals(&bytes, &globals)
                        .unwrap(),
                    document
                );
            }
        }
        let mut segments = implicit_page(unknown_height);
        segments.push(extension(4, 1, 0, &[]).retaining_references(&[0]));
        let bytes = embedded(&segments);
        assert!(matches!(
            decoder.inspect_embedded(&bytes),
            Err(DecodeError::Malformed(
                MalformedError::MissingReferredSegment { .. }
            ))
        ));
        let info = decoder
            .inspect_embedded_with_globals(&bytes, &globals)
            .unwrap();
        let document = decoder
            .decode_embedded_with_globals(&bytes, &globals)
            .unwrap();
        assert_eq!(document.info(), &info);
        assert_pixels(
            &document.pages()[0],
            1,
            9,
            3,
            &[0xff, 0x80, 0, 0, 0xff, 0x80],
        );
    }
}

#[test]
fn missing_page_end_is_not_recovered_in_standalone_or_before_another_page() {
    let decoder = Decoder::default();
    for unknown_height in [false, true] {
        let mut segments = implicit_page(unknown_height);
        segments.push(eof(4));
        for sequential in [false, true] {
            let bytes = standalone(sequential, None, &segments);
            assert_malformed(
                decoder.inspect(&bytes).unwrap_err(),
                &MalformedError::MissingEndOfPage(1),
            );
            assert_malformed(
                decoder.decode(&bytes).unwrap_err(),
                &MalformedError::MissingEndOfPage(1),
            );
            let mut calls = 0;
            assert_malformed(
                decoder
                    .decode_with_sink(&bytes, &mut |_| {
                        calls += 1;
                        Ok(())
                    })
                    .unwrap_err(),
                &MalformedError::MissingEndOfPage(1),
            );
            assert_eq!(calls, 0);
        }
    }
    let bytes = embedded(&[page(1, 1, 8, 1), page(2, 2, 8, 1), end(3, 2)]);
    assert_malformed(
        decoder.inspect_embedded(&bytes).unwrap_err(),
        &MalformedError::MissingEndOfPage(1),
    );
    assert_malformed(
        decoder.decode_embedded(&bytes).unwrap_err(),
        &MalformedError::MissingEndOfPage(1),
    );
}

#[test]
fn embedded_completion_preserves_explicit_earlier_pages_and_stream_order() {
    let decoder = Decoder::default();
    let mut segments = vec![page(10, 2, 8, 1), end(11, 2)];
    segments.extend(implicit_page(true));
    let bytes = embedded(&segments);
    let document = decoder.decode_embedded(&bytes).unwrap();
    assert_eq!(
        document
            .info()
            .pages()
            .iter()
            .map(jbig2_decoder::PageInfo::number)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(document.pages().len(), 2);
    assert_pixels(&document.pages()[0], 2, 8, 1, &[0]);
    assert_pixels(
        &document.pages()[1],
        1,
        9,
        3,
        &[0xff, 0x80, 0, 0, 0xff, 0x80],
    );
}

#[test]
fn embedded_completion_does_not_hide_truncated_segments_or_unfinished_stripes() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&[]).unwrap();
    let complete = embedded(&implicit_page(false));
    let page_end = embedded(&implicit_page(false)[..1]).len();
    for end in page_end + 1..complete.len() {
        let bytes = &complete[..end];
        for error in [
            decoder.inspect_embedded(bytes).unwrap_err(),
            decoder.decode_embedded(bytes).unwrap_err(),
            decoder
                .inspect_embedded_with_globals(bytes, &globals)
                .unwrap_err(),
            decoder
                .decode_embedded_with_globals(bytes, &globals)
                .unwrap_err(),
        ] {
            assert!(matches!(
                error,
                DecodeError::Malformed(MalformedError::UnexpectedEof { .. })
            ));
        }
    }
    let mut empty_entropy = white_row(2, 1);
    empty_entropy.payload.pop();
    let bytes = embedded(&[implicit_page(false).remove(0), empty_entropy]);
    assert!(matches!(
        decoder.decode_embedded(&bytes),
        Err(DecodeError::Malformed(MalformedError::UnexpectedEof { .. }))
    ));
    assert!(matches!(
        decoder.decode_embedded_with_globals(&bytes, &globals),
        Err(DecodeError::Malformed(MalformedError::UnexpectedEof { .. }))
    ));
    let mut incomplete_stripe = embedded(&implicit_page(true));
    incomplete_stripe.pop();
    assert!(matches!(
        decoder.inspect_embedded(&incomplete_stripe),
        Err(DecodeError::Malformed(MalformedError::UnexpectedEof { .. }))
    ));
    for segments in [
        vec![striped_page(1, u32::MAX, 2)],
        vec![striped_page(1, u32::MAX, 2), white_row(2, 0)],
        vec![striped_page(1, u32::MAX, 2), stripe(2, 0), white_row(3, 1)],
        vec![striped_page(1, u32::MAX, 2), white_row(2, 1), stripe(3, 0)],
    ] {
        let bytes = embedded(&segments);
        assert!(decoder.inspect_embedded(&bytes).is_err());
        assert!(decoder.decode_embedded(&bytes).is_err());
        assert!(decoder
            .inspect_embedded_with_globals(&bytes, &globals)
            .is_err());
        assert!(decoder
            .decode_embedded_with_globals(&bytes, &globals)
            .is_err());
    }
    let mut limits = Limits::default();
    limits.max_height = 2;
    for unknown_height in [false, true] {
        assert_limit(
            Decoder::new(limits)
                .decode_embedded(&embedded(&implicit_page(unknown_height)))
                .unwrap_err(),
            LimitKind::Height,
            2,
            3,
        );
    }
}

#[test]
fn empty_page_information_is_not_recovered_from_following_segment_bytes() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&[]).unwrap();
    for association in [0, 1] {
        let bytes = embedded(&[Segment::new(1, 48, association, vec![]), white_row(2, 0)]);
        let expected = MalformedError::InvalidSegmentDataLength {
            segment_type: 48,
            expected: 19,
            actual: 0,
        };
        assert_malformed(decoder.inspect_embedded(&bytes).unwrap_err(), &expected);
        assert_malformed(decoder.decode_embedded(&bytes).unwrap_err(), &expected);
        assert_malformed(
            decoder
                .inspect_embedded_with_globals(&bytes, &globals)
                .unwrap_err(),
            &expected,
        );
        assert_malformed(
            decoder
                .decode_embedded_with_globals(&bytes, &globals)
                .unwrap_err(),
            &expected,
        );
    }
}

#[test]
fn stripe_metadata_and_zero_padding_follow_independently_declared_rows() {
    for height in [3, u32::MAX] {
        let segments = [
            striped_page(0, height, 2),
            stripe(1, 0),
            stripe(2, 2),
            end(3, 1),
            eof(4),
        ];
        for sequential in [true, false] {
            let document = Decoder::default()
                .decode(&standalone(sequential, Some(1), &segments))
                .unwrap();
            assert_info(
                document.info(),
                if sequential {
                    Organization::Sequential
                } else {
                    Organization::RandomAccess
                },
                Some(1),
                5,
                &[(1, 9, 3, 300, 600, true)],
            );
            assert_pixels(&document.pages()[0], 1, 9, 3, &[0; 6]);
        }
    }
}

#[test]
fn stripe_end_boundaries_and_unknown_height_failures_are_typed() {
    for (first, rows, invalid) in [
        (page(0, 1, 9, 3), vec![0], 0),
        (striped_page(0, 8, 2), vec![3], 3),
        (striped_page(0, 8, 2), vec![1, 1], 1),
        (striped_page(0, 8, 2), vec![1, 0], 0),
        (striped_page(0, 8, 2), vec![1, 4], 4),
        (striped_page(0, 3, 3), vec![3], 3),
        (striped_page(0, u32::MAX, 2), vec![], 0),
    ] {
        let mut segments = vec![first];
        for (index, row) in rows.into_iter().enumerate() {
            segments.push(stripe(u32::try_from(index + 1).unwrap(), row));
        }
        let next = u32::try_from(segments.len()).unwrap();
        segments.extend([end(next, 1), eof(next + 1)]);
        reject(&segments, &MalformedError::InvalidStripeEndRow(invalid));
    }
    reject(
        &[page(0, 1, 9, u32::MAX), end(1, 1), eof(2)],
        &MalformedError::UnknownHeightWithoutStriping,
    );
    let mut limits = Limits::default();
    limits.max_width = 8;
    assert_limit(
        Decoder::new(limits)
            .inspect_embedded(&embedded(&[
                striped_page(0, u32::MAX, 2),
                stripe(1, 0),
                end(2, 1),
            ]))
            .unwrap_err(),
        LimitKind::Width,
        8,
        9,
    );
}

struct StopAfterSecond {
    pages: Vec<(u32, Vec<u8>)>,
}

impl PageSink for StopAfterSecond {
    fn write_page(&mut self, page: Page) -> Result<(), DecodeError> {
        let number = page.number();
        let bitmap = page.into_bitmap();
        assert_eq!(
            (bitmap.width(), bitmap.height(), bitmap.stride()),
            (9, 1, 2)
        );
        assert_eq!(bitmap.bytes(), &[0, 0]);
        self.pages.push((number, bitmap.bytes().to_vec()));
        if self.pages.len() == 2 {
            Err(DecodeError::Allocation {
                requested_bytes: 123,
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn second_page_sink_failure_preserves_prior_effects_and_prevents_third_delivery() {
    let segments = [
        page(0, 1, 9, 1),
        end(1, 1),
        page(2, 2, 9, 1),
        end(3, 2),
        page(4, 3, 9, 1),
        end(5, 3),
        eof(6),
    ];
    let decoder = Decoder::default();
    for sequential in [true, false] {
        let bytes = standalone(sequential, Some(3), &segments);
        for reader in [false, true] {
            let mut sink = StopAfterSecond { pages: Vec::new() };
            let error = if reader {
                decoder.decode_reader_with_sink(&mut Cursor::new(&bytes), &mut sink)
            } else {
                decoder.decode_with_sink(&bytes, &mut sink)
            }
            .unwrap_err();
            assert!(matches!(
                error,
                DecodeError::Allocation {
                    requested_bytes: 123
                }
            ));
            assert_eq!(sink.pages, [(1, vec![0, 0]), (2, vec![0, 0])]);
            let recovered = decoder.decode(&bytes).unwrap();
            assert_eq!(recovered.pages().len(), 3);
            for (index, page) in recovered.pages().iter().enumerate() {
                assert_pixels(page, u32::try_from(index + 1).unwrap(), 9, 1, &[0, 0]);
            }
        }
    }
}
