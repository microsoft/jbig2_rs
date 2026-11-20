//! Strict defaults and explicitly reported, bounded embedded recoveries.

use super::*;
use jbig2_decoder::{EmbeddedPolicy, EmbeddedRecoveryKind};

#[test]
fn best_effort_does_not_replace_missing_resources_or_ignore_necessary_extensions() {
    let decoder = Decoder::default();
    let missing = embedded(&[
        page(0, 1, 8, 1),
        extension(3, 1, 0, &[]).retaining_references(&[1]),
    ]);
    let necessary = embedded(&[page(0, 1, 8, 1), extension(1, 0, 0xa000_0001, &[])]);
    for bytes in [&missing, &necessary] {
        let strict = decoder.decode_embedded(bytes).unwrap_err();
        let check = |error| match (&strict, error) {
            (DecodeError::Malformed(expected), DecodeError::Malformed(actual)) => {
                assert_eq!(&actual, expected);
            }
            (DecodeError::Unsupported(expected), DecodeError::Unsupported(actual)) => {
                assert_eq!(&actual, expected);
            }
            (expected, actual) => panic!("changed fatal error: {expected:?}/{actual:?}"),
        };
        for policy in [EmbeddedPolicy::Strict, EmbeddedPolicy::BestEffort] {
            check(
                decoder
                    .decode_embedded_with_policy(bytes, None, policy)
                    .unwrap_err(),
            );
            let mut context = decoder.context();
            check(
                context
                    .decode_embedded_with_policy(bytes, None, policy)
                    .unwrap_err(),
            );
        }
    }
}

fn arithmetic_generic(marker: u8) -> Vec<Segment> {
    let mut first = page(1, 1, 1, 1);
    first.payload[16] = 0x44;
    let mut data = 1_u32.to_be_bytes().to_vec();
    data.extend_from_slice(&1_u32.to_be_bytes());
    data.extend_from_slice(&[0; 8]);
    data.extend_from_slice(&[4, 6, 2, 0xff, 0, 0, 0xff, marker]);
    vec![first, Segment::new(2, 38, 1, data)]
}

#[test]
fn embedded_marker_recovery_requires_a_complete_known_length_ffd0_tail() {
    let decoder = Decoder::default();
    let globals = decoder
        .parse_globals(&embedded(&[extension(0, 0, 0, &[])]))
        .unwrap();
    for with_globals in [false, true] {
        let mut segments = arithmetic_generic(0xd0);
        let marker = embedded(&segments).len() - 1;
        if with_globals {
            segments.push(extension(3, 1, 0, &[]).retaining_references(&[0]));
        }
        let bytes = embedded(&segments);
        let source = bytes.clone();
        let globals = with_globals.then_some(&globals);
        assert!(matches!(
            decoder.decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::Strict),
            Err(DecodeError::Malformed(
                MalformedError::InvalidGenericRegion(
                    jbig2_decoder::GenericRegionErrorKind::InvalidArithmeticTerminator
                )
            ))
        ));
        let result = decoder
            .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
            .unwrap();
        assert_pixels(&result.value.pages()[0], 1, 1, 1, &[0]);
        assert_eq!(result.recoveries.len(), 1);
        assert_eq!(
            result.recoveries[0].kind,
            EmbeddedRecoveryKind::ArithmeticTerminatorCode
        );
        assert_eq!(result.recoveries[0].byte_offset, marker);
        let mut context = decoder.context();
        let repeated = context
            .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
            .unwrap();
        assert_eq!(repeated.value, result.value);
        assert_eq!(repeated.recoveries, result.recoveries);
        for inspected in [
            decoder
                .inspect_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap(),
            context
                .inspect_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap(),
        ] {
            assert_eq!(&inspected.value, result.value.info());
            assert_eq!(inspected.recoveries, result.recoveries);
        }
        assert_eq!(bytes, source);
    }
    let canonical = embedded(&arithmetic_generic(0xac));
    let result = decoder
        .decode_embedded_with_policy(&canonical, None, EmbeddedPolicy::BestEffort)
        .unwrap();
    assert!(result.recoveries.is_empty());
    assert_pixels(&result.value.pages()[0], 1, 1, 1, &[0]);
    let mut missing = arithmetic_generic(0xd0);
    let without_marker = missing[1].payload.len() - 2;
    missing[1].payload.truncate(without_marker);
    let mut interior = arithmetic_generic(0xd0);
    interior[1].payload.extend_from_slice(&[0xff, 0xac]);
    for segments in [arithmetic_generic(0xd1), missing, interior] {
        let bytes = embedded(&segments);
        for policy in [EmbeddedPolicy::Strict, EmbeddedPolicy::BestEffort] {
            let result = decoder.decode_embedded_with_policy(&bytes, None, policy);
            assert!(
                matches!(
                    result,
                    Err(DecodeError::Malformed(
                        MalformedError::InvalidGenericRegion(_)
                    ))
                ),
                "{policy:?}: {result:?}"
            );
        }
    }
    let mut unknown = arithmetic_generic(0xd0);
    unknown[1].unknown_length = true;
    unknown[1].payload.extend_from_slice(&1_u32.to_be_bytes());
    assert!(matches!(
        decoder.decode_embedded_with_policy(&embedded(&unknown), None, EmbeddedPolicy::BestEffort),
        Err(DecodeError::Malformed(
            MalformedError::MissingGenericRegionTerminator
        ))
    ));
}

fn inconsistent_page(refinement: bool) -> Vec<Segment> {
    let mut first = page(1, 1, 1, 1);
    first.payload[16] = 4;
    let mut data = 1_u32.to_be_bytes().to_vec();
    data.extend_from_slice(&1_u32.to_be_bytes());
    data.extend_from_slice(&[0; 8]);
    if refinement {
        data.extend_from_slice(&[4, 1, 0, 0, 0xff, 0xac]);
    } else {
        data.extend_from_slice(&[0, 1, 0x80]);
    }
    vec![
        first,
        Segment::new(2, if refinement { 42 } else { 36 }, 1, data),
    ]
}

#[test]
fn embedded_recoveries_are_opt_in_reported_and_consistent_across_apis() {
    let decoder = Decoder::default();
    let globals = decoder
        .parse_globals(&embedded(&[extension(0, 0, 0, &[])]))
        .unwrap();
    for refinement in [false, true] {
        for with_globals in [false, true] {
            let mut segments = inconsistent_page(refinement);
            if with_globals {
                segments.push(extension(3, 1, 0, &[]).retaining_references(&[0]));
            }
            let bytes = embedded(&segments);
            let source = bytes.clone();
            let globals = with_globals.then_some(&globals);
            let expected = MalformedError::InvalidPageState(if refinement {
                "refinement on page that forbids refinements"
            } else {
                "intermediate region on page that forbids auxiliary buffers"
            });
            let mut context = decoder.context();
            for error in [
                decoder
                    .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::Strict)
                    .unwrap_err(),
                context
                    .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::Strict)
                    .unwrap_err(),
            ] {
                assert_malformed(error, &expected);
            }
            let normal = match globals {
                Some(globals) => decoder.decode_embedded_with_globals(&bytes, globals),
                None => decoder.decode_embedded(&bytes),
            };
            assert_malformed(normal.unwrap_err(), &expected);
            let result = decoder
                .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap();
            assert_eq!(result.recoveries.len(), 1);
            let recovery = result.recoveries[0];
            assert_eq!(
                (recovery.page, recovery.segment, recovery.byte_offset),
                (1, 1, 27)
            );
            assert_eq!(
                recovery.kind,
                if refinement {
                    EmbeddedRecoveryKind::RefinementsFlag
                } else {
                    EmbeddedRecoveryKind::AuxiliaryBuffersFlag
                }
            );
            assert_pixels(
                &result.value.pages()[0],
                1,
                1,
                1,
                &[if refinement { 0 } else { 0x80 }],
            );
            let contextual = context
                .decode_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap();
            assert_eq!(contextual.value, result.value);
            assert_eq!(contextual.recoveries, result.recoveries);
            let inspected = decoder
                .inspect_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap();
            assert_eq!(&inspected.value, result.value.info());
            assert_eq!(inspected.recoveries, result.recoveries);
            let inspected = context
                .inspect_embedded_with_policy(&bytes, globals, EmbeddedPolicy::BestEffort)
                .unwrap();
            assert_eq!(&inspected.value, result.value.info());
            assert_eq!(inspected.recoveries, result.recoveries);
            assert_eq!(bytes, source);
            // A standalone file never inherits this embedded-only policy.
            if !with_globals {
                segments.extend([end(3, 1), eof(4)]);
                assert_malformed(
                    decoder
                        .decode(&standalone(true, None, &segments))
                        .unwrap_err(),
                    &expected,
                );
            }
        }
    }
}

#[test]
fn embedded_best_effort_does_not_repair_data_or_relax_resource_limits() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&[]).unwrap();
    for refinement in [false, true] {
        let segments = inconsistent_page(refinement);
        let mut bytes = embedded(&segments);
        bytes.pop();
        for policy in [EmbeddedPolicy::Strict, EmbeddedPolicy::BestEffort] {
            for state in [None, Some(&globals)] {
                assert!(matches!(
                    decoder.decode_embedded_with_policy(&bytes, state, policy),
                    Err(DecodeError::Malformed(MalformedError::UnexpectedEof { .. }))
                ));
            }
        }
        let mut malformed = segments.clone();
        malformed[1].payload.truncate(18);
        let bytes = embedded(&malformed);
        assert!(decoder
            .decode_embedded_with_policy(&bytes, None, EmbeddedPolicy::BestEffort)
            .is_err());
        let bytes = embedded(&segments);
        let mut limits = Limits::default();
        limits.max_width = 0;
        for policy in [EmbeddedPolicy::Strict, EmbeddedPolicy::BestEffort] {
            assert_limit(
                Decoder::new(limits)
                    .decode_embedded_with_policy(&bytes, None, policy)
                    .unwrap_err(),
                LimitKind::Width,
                0,
                1,
            );
        }
        let mut limits = Limits::default();
        limits.max_retained_state_bytes = u64::try_from(bytes.len()).unwrap() - 1;
        assert!(matches!(
            Decoder::new(limits).decode_embedded_with_policy(
                &bytes,
                None,
                EmbeddedPolicy::BestEffort
            ),
            Err(DecodeError::LimitExceeded(LimitExceeded {
                kind: LimitKind::RetainedState,
                ..
            }))
        ));
        let mut limits = Limits::default();
        limits.max_decoder_managed_memory_bytes = u64::try_from(bytes.len()).unwrap() - 1;
        assert!(matches!(
            Decoder::new(limits).decode_embedded_with_policy(
                &bytes,
                None,
                EmbeddedPolicy::BestEffort
            ),
            Err(DecodeError::LimitExceeded(LimitExceeded {
                kind: LimitKind::Memory,
                ..
            }))
        ));
        let mut limits = Limits::default();
        limits.max_work_units = u64::try_from(bytes.len() + segments.len()).unwrap();
        assert!(matches!(
            Decoder::new(limits).decode_embedded_with_policy(
                &bytes,
                None,
                EmbeddedPolicy::BestEffort
            ),
            Err(DecodeError::LimitExceeded(LimitExceeded {
                kind: LimitKind::Work,
                ..
            }))
        ));
    }
    for payload in [
        embedded(&[Segment::new(1, 48, 1, vec![])]),
        embedded(&[Segment::new(1, 0, 0, vec![0, 0x81])]),
    ] {
        let strict = decoder.decode_embedded(&payload).unwrap_err();
        let recovered = decoder
            .decode_embedded_with_policy(&payload, None, EmbeddedPolicy::BestEffort)
            .unwrap_err();
        let DecodeError::Malformed(expected) = strict else {
            panic!("expected a malformed-input rejection: {strict:?}");
        };
        assert_malformed(recovered, &expected);
    }
    let clean = embedded(&[page(1, 1, 8, 1)]);
    for policy in [EmbeddedPolicy::Strict, EmbeddedPolicy::BestEffort] {
        let result = decoder
            .decode_embedded_with_policy(&clean, None, policy)
            .unwrap();
        assert!(result.recoveries.is_empty());
        assert_pixels(&result.value.pages()[0], 1, 8, 1, &[0]);
    }
}
