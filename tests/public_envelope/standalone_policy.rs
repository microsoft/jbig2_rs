//! Standalone exceptions never imply a partial-page or entropy-repair policy.

use super::*;
use jbig2_decoder::{DecodePolicy, RecoveryKind};

fn patterned_page(number: u32, association: u32) -> Vec<Segment> {
    let mut information = page(number, association, 9, 3);
    information.payload[16] = 0x44;
    let mut payload = 9_u32.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&[4, 1, 0x80]);
    vec![
        information,
        Segment::new(number + 1, 38, association, payload),
    ]
}

fn compatibility(bytes: &[u8]) -> jbig2_decoder::DecodeResult<jbig2_decoder::DecodedDocument> {
    let decoder = Decoder::default();
    assert!(decoder.decode(bytes).is_err());
    assert!(decoder
        .decode_with_policy(bytes, DecodePolicy::Strict)
        .is_err());
    let source = bytes.to_vec();
    let output = decoder
        .decode_with_policy(bytes, DecodePolicy::Compatibility)
        .unwrap();
    let mut context = decoder.context();
    for repeated in [
        context
            .decode_with_policy(bytes, DecodePolicy::Compatibility)
            .unwrap(),
        decoder
            .decode_reader_with_policy(&mut Cursor::new(bytes), DecodePolicy::Compatibility)
            .unwrap(),
        context
            .decode_reader_with_policy(&mut Cursor::new(bytes), DecodePolicy::Compatibility)
            .unwrap(),
    ] {
        assert_eq!(output.value, repeated.value);
        assert_eq!(output.recoveries, repeated.recoveries);
    }
    assert_eq!(bytes, source);
    output
}

fn assert_pattern(page: &Page, number: u32) {
    assert_pixels(page, number, 9, 3, &[0xff, 0x80, 0, 0, 0xff, 0x80]);
}

#[test]
fn standalone_policy_strict_success_preserves_every_page_and_returns_no_recoveries() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 1));
    segments.extend(patterned_page(3, 2));
    segments.push(end(5, 2));
    segments.push(eof(6));
    let bytes = standalone(true, Some(2), &segments);
    let source = bytes.clone();
    let decoder = Decoder::default();
    let expected = decoder.decode(&bytes).unwrap();
    assert_eq!(expected.pages().len(), 2);
    assert_pattern(&expected.pages()[0], 1);
    assert_pattern(&expected.pages()[1], 2);
    let mut context = decoder.context();
    for output in [
        decoder
            .decode_with_policy(&bytes, DecodePolicy::Strict)
            .unwrap(),
        context
            .decode_with_policy(&bytes, DecodePolicy::Strict)
            .unwrap(),
        decoder
            .decode_reader_with_policy(&mut Cursor::new(&bytes), DecodePolicy::Strict)
            .unwrap(),
        context
            .decode_reader_with_policy(&mut Cursor::new(&bytes), DecodePolicy::Strict)
            .unwrap(),
    ] {
        assert_eq!(output.value, expected);
        assert!(output.recoveries.is_empty());
    }
    assert_eq!(bytes, source);
}

#[test]
fn standalone_policy_closes_complete_pages_at_exact_boundaries_and_counts_all_pixels() {
    let mut segments = patterned_page(0, 1);
    segments.extend(patterned_page(2, 2));
    segments.push(end(4, 2));
    segments.extend(patterned_page(5, 3));
    let bytes = standalone(true, Some(2), &segments);
    let output = compatibility(&bytes);
    assert_eq!(output.value.info().declared_page_count(), Some(3));
    assert_eq!(output.value.pages().len(), 3);
    for (index, page) in output.value.pages().iter().enumerate() {
        assert_pattern(page, u32::try_from(index + 1).unwrap());
    }
    let kinds: Vec<_> = output.recoveries.iter().map(|r| (r.page, r.kind)).collect();
    assert_eq!(
        kinds,
        [
            (1, RecoveryKind::MissingEndOfPage),
            (3, RecoveryKind::MissingEndOfPage),
            (0, RecoveryKind::ObservedPageCount),
        ]
    );
    let mut limits = Limits::default();
    limits.max_pages = 2;
    assert!(matches!(
        Decoder::new(limits).decode_with_policy(&bytes, DecodePolicy::Compatibility),
        Err(DecodeError::LimitExceeded(detail)) if detail.kind == LimitKind::Pages
    ));
}

#[test]
fn standalone_policy_zero_eop_requires_exactly_one_open_owner_and_preserves_nonzero_errors() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 0));
    segments.extend(patterned_page(3, 2));
    segments.push(end(5, 2));
    let bytes = standalone(true, Some(2), &segments);
    let output = compatibility(&bytes);
    assert_pattern(&output.value.pages()[0], 1);
    assert_pattern(&output.value.pages()[1], 2);
    assert_eq!(output.recoveries.len(), 1);
    assert_eq!(
        output.recoveries[0].kind,
        RecoveryKind::ZeroAssociatedEndOfPage
    );
    assert!(output.recoveries[0].byte_offset.is_some());
    for invalid in [
        vec![end(0, 0)],
        vec![page(0, 1, 9, 3), page(1, 2, 9, 3), end(2, 0)],
        vec![page(0, 1, 9, 3), end(1, 1), end(2, 0)],
        vec![page(0, 1, 9, 3), end(1, 1), page(2, 2, 9, 3), end(3, 3)],
    ] {
        assert!(Decoder::default()
            .decode_with_policy(
                &standalone(true, None, &invalid),
                DecodePolicy::Compatibility,
            )
            .is_err());
    }
}

#[test]
fn standalone_policy_closed_page_eop_uses_unique_owner_and_records_the_chain() {
    for first_association in [0, 1] {
        let mut segments = patterned_page(0, 1);
        segments.push(end(2, first_association));
        segments.extend(patterned_page(3, 2));
        segments.push(end(5, 1));
        segments.extend(patterned_page(6, 3));
        segments.push(end(8, 2));
        let bytes = standalone(true, Some(3), &segments);
        let output = compatibility(&bytes);
        assert_eq!(output.value.pages().len(), 3);
        for (index, page) in output.value.pages().iter().enumerate() {
            assert_pattern(page, u32::try_from(index + 1).unwrap());
        }
        let repairs: Vec<_> = output
            .recoveries
            .iter()
            .filter(|recovery| recovery.kind == RecoveryKind::ClosedPageEndOfPage)
            .collect();
        assert_eq!(repairs.len(), 2);
        for (recovery, (owner, marker_index)) in repairs.iter().zip([(2, 5), (3, 8)]) {
            assert_eq!(recovery.page, owner);
            assert_eq!(
                recovery.byte_offset,
                Some(13 + embedded(&segments[..marker_index]).len() + 6)
            );
        }
    }
}

#[test]
fn standalone_policy_closed_page_eop_rejects_live_unseen_future_and_ambiguous_names() {
    let earlier_not_closed = vec![
        page(0, 1, 9, 3),
        page(1, 2, 9, 3),
        end(2, 1),
        extension(3, 1, 0, &[]),
        end(4, 2),
    ];
    let unseen_earlier = vec![page(0, 2, 9, 3), end(1, 1)];
    let unseen_future = vec![page(0, 1, 9, 3), end(1, 2)];
    let closed_future = vec![page(0, 2, 9, 3), end(1, 2), page(2, 1, 9, 3), end(3, 2)];
    let no_owner = vec![page(0, 1, 9, 3), end(1, 1), end(2, 1)];
    let ambiguous = vec![
        page(0, 1, 9, 3),
        end(1, 1),
        page(2, 2, 9, 3),
        page(3, 3, 9, 3),
        end(4, 1),
    ];
    let referenced = vec![
        page(0, 1, 9, 3),
        end(1, 1),
        page(2, 2, 9, 3),
        end(3, 1).references(&[0], 0),
    ];
    let mut nonempty = referenced.clone();
    nonempty[3] = end(3, 1);
    nonempty[3].payload.push(0);
    let mut later_necessary = vec![page(0, 1, 9, 3), end(1, 1), page(2, 2, 9, 3), end(3, 1)];
    later_necessary.push(extension(4, 0, 0xa000_ffff, &[]));
    for segments in [
        earlier_not_closed,
        unseen_earlier,
        unseen_future,
        closed_future,
        no_owner,
        ambiguous,
        referenced,
        nonempty,
        later_necessary,
    ] {
        let bytes = standalone(true, None, &segments);
        assert!(Decoder::default()
            .decode_with_policy(&bytes, DecodePolicy::Compatibility)
            .is_err());
    }
    let complete = [page(0, 1, 9, 3), page(1, 2, 9, 3), end(2, 1), end(3, 2)];
    let bytes = standalone(true, Some(2), &complete);
    let decoder = Decoder::default();
    let output = decoder
        .decode_with_policy(&bytes, DecodePolicy::Compatibility)
        .unwrap();
    assert_eq!(output.value, decoder.decode(&bytes).unwrap());
    assert!(output.recoveries.is_empty());
}

#[test]
fn standalone_policy_suffix_requires_validated_eof_and_exact_crlf() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 1));
    let no_eof = standalone(true, Some(1), &segments);
    segments.push(eof(3));
    let valid = standalone(true, Some(1), &segments);
    let mut bytes = valid.clone();
    bytes.extend_from_slice(b"\r\n");
    let output = compatibility(&bytes);
    assert_pattern(&output.value.pages()[0], 1);
    assert_eq!(output.recoveries[0].kind, RecoveryKind::TrailingCrLf);
    assert_eq!(output.recoveries[0].byte_offset, Some(valid.len()));
    for mut bytes in [no_eof.clone(), valid.clone()] {
        for suffix in [b"\n".as_slice(), b"\r", b"xx", b"\r\n\r\n", b"\0\0\0"] {
            let length = bytes.len();
            bytes.extend_from_slice(suffix);
            assert!(Decoder::default()
                .decode_with_policy(&bytes, DecodePolicy::Compatibility)
                .is_err());
            bytes.truncate(length);
        }
    }
    let mut without_eof = no_eof;
    without_eof.extend_from_slice(b"\r\n");
    assert!(Decoder::default()
        .decode_with_policy(&without_eof, DecodePolicy::Compatibility)
        .is_err());
}

#[test]
fn standalone_policy_only_masks_bit_four_never_colour_or_other_reserved_bits() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 1));
    segments[1].payload[16] |= 0x10;
    let output = compatibility(&standalone(true, Some(1), &segments));
    assert_pattern(&output.value.pages()[0], 1);
    assert_eq!(output.recoveries[0].kind, RecoveryKind::ReservedRegionBit4);
    for bits in [0x08, 0x18, 0x20, 0x30, 0x80, 0x90] {
        segments[1].payload[16] = 4 | bits;
        assert!(
            Decoder::default()
                .decode_with_policy(
                    &standalone(true, Some(1), &segments),
                    DecodePolicy::Compatibility,
                )
                .is_err(),
            "{bits:#x}"
        );
    }
}

#[test]
fn standalone_policy_type_one_skip_never_discards_references_or_live_retention() {
    let mut segments = patterned_page(0, 1);
    segments[1].number = 2;
    let mut reserved = Segment::new(1, 1, 1, b"opaque!!".to_vec());
    reserved.retention = 0;
    segments.insert(1, reserved);
    segments.push(end(3, 1));
    let output = compatibility(&standalone(true, Some(1), &segments));
    assert_pattern(&output.value.pages()[0], 1);
    assert_eq!(
        output.recoveries[0].kind,
        RecoveryKind::UnreferencedReservedType1
    );
    let mut limits = Limits::default();
    limits.max_segments = 3;
    assert!(matches!(
        Decoder::new(limits).decode_with_policy(
            &standalone(true, Some(1), &segments), DecodePolicy::Compatibility),
        Err(DecodeError::LimitExceeded(detail)) if detail.kind == LimitKind::Segments
    ));
    let mut retained = segments.clone();
    retained[1].retention = 1;
    let mut referenced = segments.clone();
    referenced.insert(3, extension(3, 1, 0, &[]).references(&[1], 0));
    referenced[4].number = 4;
    let mut referring = segments.clone();
    referring[1] = referring[1].clone().references(&[0], 0);
    let mut unknown = segments.clone();
    unknown[1].unknown_length = true;
    let mut truncated = standalone(true, Some(1), &segments);
    truncated.truncate(55);
    for bytes in [
        standalone(true, Some(1), &retained),
        standalone(true, Some(1), &referenced),
        standalone(true, Some(1), &referring),
        standalone(true, Some(1), &unknown),
        truncated,
    ] {
        assert!(Decoder::default()
            .decode_with_policy(&bytes, DecodePolicy::Compatibility)
            .is_err());
    }
}

#[test]
fn standalone_policy_rejects_revisited_pages_unfinished_stripes_and_truncated_entropy() {
    let revisited = vec![
        page(0, 1, 9, 3),
        page(1, 2, 9, 3),
        extension(2, 1, 0, &[]),
        end(3, 2),
    ];
    let mut partial = patterned_page(0, 1);
    let overlapping = vec![
        page(0, 1, 9, 3),
        page(1, 2, 9, 3),
        page(2, 3, 9, 3),
        end(3, 1),
        end(4, 3),
    ];
    partial[1].payload.pop();
    let unknown = vec![striped_page(0, u32::MAX, 3)];
    let unfinished = vec![
        striped_page(0, u32::MAX, 3),
        stripe(1, 1),
        patterned_page(2, 1).remove(1),
    ];
    let mut unknown_intermediate = patterned_page(0, 1);
    unknown_intermediate[1].kind = 36;
    unknown_intermediate[1].unknown_length = true;
    unknown_intermediate[1]
        .payload
        .extend_from_slice(&[0, 0, 0, 0, 0, 1]);
    for segments in [
        revisited,
        overlapping,
        partial,
        unknown,
        unfinished,
        unknown_intermediate,
    ] {
        assert!(Decoder::default()
            .decode_with_policy(
                &standalone(true, None, &segments),
                DecodePolicy::Compatibility,
            )
            .is_err());
    }
    let mut complete = vec![striped_page(0, u32::MAX, 3), stripe(1, 2)];
    complete[0].payload[16] = 4;
    let output = compatibility(&standalone(true, Some(1), &complete));
    assert_pixels(
        &output.value.pages()[0],
        1,
        9,
        3,
        &[0xff, 0x80, 0xff, 0x80, 0xff, 0x80],
    );
}

#[test]
fn standalone_policy_preparation_and_diagnostics_cannot_bypass_budgets_or_cleanup() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 0));
    let bytes = standalone(true, Some(1), &segments);
    for kind in [
        LimitKind::StreamBytes,
        LimitKind::Work,
        LimitKind::Memory,
        LimitKind::RetainedState,
    ] {
        let mut limits = Limits::default();
        match kind {
            LimitKind::StreamBytes => limits.max_stream_bytes = 1,
            LimitKind::Work => limits.max_work_units = 1,
            LimitKind::Memory => limits.max_decoder_managed_memory_bytes = 1,
            LimitKind::RetainedState => {
                limits.max_retained_state_bytes = u64::try_from(bytes.len()).unwrap();
            }
            _ => unreachable!(),
        }
        let decoder = Decoder::new(limits);
        let mut context = decoder.context();
        for error in [
            decoder
                .decode_with_policy(&bytes, DecodePolicy::Compatibility)
                .unwrap_err(),
            context
                .decode_with_policy(&bytes, DecodePolicy::Compatibility)
                .unwrap_err(),
        ] {
            assert!(
                matches!(error, DecodeError::LimitExceeded(detail) if detail.kind == kind),
                "{kind:?}: {error:?}"
            );
        }
        assert_eq!(context.retained_scratch_bytes(), 0);
    }
    let valid = standalone(true, Some(1), &[page(0, 1, 9, 3), end(1, 1)]);
    let decoder = Decoder::default();
    let strict = decoder.decode(&valid).unwrap();
    let compatible = decoder
        .decode_with_policy(&valid, DecodePolicy::Compatibility)
        .unwrap();
    assert_eq!(compatible.value, strict);
    assert!(compatible.recoveries.is_empty());
}

#[test]
fn standalone_policy_preserves_necessary_extension_errors() {
    let segments = [
        page(0, 1, 9, 3),
        extension(1, 1, 0xa000_ffff, &[]),
        end(2, 1),
    ];
    let bytes = standalone(true, Some(1), &segments);
    for policy in [DecodePolicy::Strict, DecodePolicy::Compatibility] {
        assert!(matches!(
            Decoder::default().decode_with_policy(&bytes, policy),
            Err(DecodeError::Unsupported(_)),
        ));
    }
}

#[test]
fn standalone_policy_readers_preserve_typed_limits_late_io_errors_and_cleanup() {
    struct LateError;
    impl std::io::Read for LateError {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "late failure",
            ))
        }
    }
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 0));
    let bytes = standalone(true, Some(1), &segments);
    for kind in [
        LimitKind::StreamBytes,
        LimitKind::Work,
        LimitKind::Memory,
        LimitKind::RetainedState,
    ] {
        let mut limits = Limits::default();
        match kind {
            LimitKind::StreamBytes => limits.max_stream_bytes = 1,
            LimitKind::Work => limits.max_work_units = 1,
            LimitKind::Memory => limits.max_decoder_managed_memory_bytes = 1,
            LimitKind::RetainedState => limits.max_retained_state_bytes = 1,
            _ => unreachable!(),
        }
        let decoder = Decoder::new(limits);
        let mut context = decoder.context();
        for error in [
            decoder
                .decode_reader_with_policy(&mut Cursor::new(&bytes), DecodePolicy::Compatibility)
                .unwrap_err(),
            context
                .decode_reader_with_policy(&mut Cursor::new(&bytes), DecodePolicy::Compatibility)
                .unwrap_err(),
        ] {
            assert!(
                matches!(error, DecodeError::LimitExceeded(detail) if detail.kind == kind),
                "{kind:?}: {error:?}"
            );
        }
        assert_eq!(context.retained_scratch_bytes(), 0);
    }
    let decoder = Decoder::default();
    let mut context = decoder.context();
    segments[2] = end(2, 1);
    let canonical = standalone(true, Some(1), &segments);
    context.decode(&canonical).unwrap();
    assert!(context.retained_scratch_bytes() > 0);
    assert!(context
        .decode_with_policy(b"", DecodePolicy::Compatibility)
        .is_err());
    assert_eq!(context.retained_scratch_bytes(), 0);
    context.decode(&canonical).unwrap();
    assert!(context.retained_scratch_bytes() > 0);
    let mut reader = std::io::Read::chain(Cursor::new(&bytes), LateError);
    assert!(matches!(
        context.decode_reader_with_policy(&mut reader, DecodePolicy::Compatibility),
        Err(DecodeError::Io(jbig2_decoder::IoErrorKind::TimedOut)),
    ));
    assert_eq!(context.retained_scratch_bytes(), 0);
    assert!(context
        .decode_with_policy(&bytes, DecodePolicy::Compatibility)
        .is_ok());
}

#[test]
fn standalone_policy_random_access_remains_strict() {
    let mut segments = patterned_page(0, 1);
    segments.push(end(2, 1));
    segments.push(eof(3));
    let bytes = standalone(false, Some(1), &segments);
    let decoder = Decoder::default();
    let output = decoder
        .decode_with_policy(&bytes, DecodePolicy::Compatibility)
        .unwrap();
    assert_eq!(output.value, decoder.decode(&bytes).unwrap());
    assert!(output.recoveries.is_empty());
    let mismatch = standalone(false, Some(2), &segments);
    assert!(decoder
        .decode_with_policy(&mismatch, DecodePolicy::Compatibility)
        .is_err());
}

#[test]
#[ignore = "requires hash-checked pinned fixtures and PNG-derived packed golden bitmaps"]
fn standalone_policy_pinned_family_all_pages_and_both_text_oob_failures() {
    let assets = std::path::PathBuf::from(std::env::var("JBIG2_COMPAT28_ASSETS").unwrap());
    let golden = std::path::PathBuf::from(std::env::var("JBIG2_COMPAT28_GOLDEN").unwrap());
    let decoder = Decoder::default();
    for (name, expected) in [
        (
            "end_of_page_of_previous_page",
            &["truchet", "honeycomb", "maze"][..],
        ),
        (
            "page_without_end_of_page",
            &["truchet", "honeycomb", "maze"][..],
        ),
        ("page_count_mismatch", &["truchet", "honeycomb", "maze"][..]),
        ("reserved_region_flags", &["truchet"][..]),
        ("reserved_segment_type", &["truchet"][..]),
    ] {
        let bytes = std::fs::read(
            assets
                .join("fixtures")
                .join(format!("lenient_{name}.jbig2")),
        )
        .unwrap();
        let output = compatibility(&bytes);
        assert_eq!(output.value.pages().len(), expected.len(), "{name}");
        if name == "end_of_page_of_previous_page" {
            assert_eq!(
                output
                    .recoveries
                    .iter()
                    .map(|recovery| (recovery.page, recovery.kind))
                    .collect::<Vec<_>>(),
                [
                    (1, RecoveryKind::ZeroAssociatedEndOfPage),
                    (2, RecoveryKind::ClosedPageEndOfPage),
                    (3, RecoveryKind::ClosedPageEndOfPage),
                ]
            );
        }
        for (page, image) in output.value.pages().iter().zip(expected) {
            let packed = std::fs::read(golden.join(format!("{image}.bitmap"))).unwrap();
            let width = u32::from_be_bytes(packed[..4].try_into().unwrap());
            let height = u32::from_be_bytes(packed[4..8].try_into().unwrap());
            assert_pixels(page, page.number(), width, height, &packed[8..]);
        }
    }
    for name in [
        "trailing_bytes",
        "missing_final_oob",
        "missing_final_oob_huffman",
        "unknown_length_intermediate",
    ] {
        let bytes = std::fs::read(
            assets
                .join("fixtures")
                .join(format!("lenient_{name}.jbig2")),
        )
        .unwrap();
        for policy in [DecodePolicy::Strict, DecodePolicy::Compatibility] {
            assert!(
                decoder.decode_with_policy(&bytes, policy).is_err(),
                "{name}: {policy:?}"
            );
        }
    }
}
