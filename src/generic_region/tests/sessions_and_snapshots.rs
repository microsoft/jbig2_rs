#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

fn seed_probability_bank(contexts: &mut [ArithmeticContext]) {
    for (index, context) in contexts.iter_mut().enumerate() {
        *context = ArithmeticContext::from_state(
            u8::try_from((index * 11 + 7) % 47).unwrap(),
            u8::try_from(index % 2).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn validated_generic_calls_match_checked_calls_across_reused_probability_banks() {
    let limits = Limits::default();
    for template in [
        GenericTemplate::Template0,
        GenericTemplate::Template1,
        GenericTemplate::Template2,
        GenericTemplate::Template3,
    ] {
        let header = generic_header(9, 3, template, false, false);
        let mut checked = GenericContexts::new(template, false, &limits).unwrap();
        seed_probability_bank(&mut checked.contexts);
        let mut validated = GenericContexts::new(template, false, &limits).unwrap();
        validated.contexts.copy_from_slice(&checked.contexts);
        let coded = [0xff, 0xac];
        let mut checked_decoder = ArithmeticDecoder::new(&coded).unwrap();
        let mut validated_decoder = ArithmeticDecoder::new(&coded).unwrap();
        let mut checked_work = 11;
        let mut validated_work = 11;
        for _ in 0..4 {
            let call = GenericArithmeticCall {
                width: 9,
                height: 3,
                adaptive: header.adaptive,
                typical_prediction: false,
                skip: None,
            };
            let expected = decode_generic_on(
                &mut checked_decoder,
                &mut checked,
                call,
                3,
                &limits,
                &mut checked_work,
            )
            .unwrap();
            let actual = decode_generic_on_validated(
                &mut validated_decoder,
                &mut validated,
                call,
                3,
                &limits,
                &mut validated_work,
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(validated.contexts, checked.contexts);
            assert_eq!(validated_work, checked_work);
            assert_decoder_state_eq(&validated_decoder, &checked_decoder);
        }
    }
}

#[test]
fn checked_generic_boundary_still_rejects_an_unused_invalid_probability_state() {
    let limits = Limits::default();
    let template = GenericTemplate::Template0;
    let header = generic_header(1, 1, template, false, false);
    let mut contexts = GenericContexts::new(template, false, &limits).unwrap();
    let last = contexts.contexts.len() - 1;
    contexts.contexts[last] = ArithmeticContext {
        state_index: 47,
        mps: 0,
    };
    let before = contexts.contexts.clone();
    let coded = [0xff, 0xac];
    let mut decoder = ArithmeticDecoder::new(&coded).unwrap();
    let mut work = 11;
    let result = decode_generic_on(
        &mut decoder,
        &mut contexts,
        GenericArithmeticCall {
            width: 1,
            height: 1,
            adaptive: header.adaptive,
            typical_prediction: false,
            skip: None,
        },
        1,
        &limits,
        &mut work,
    );
    assert_invalid_probability(&result.map(|_| ()));
    assert_eq!(contexts.contexts, before);
    assert_eq!(work, 11);
    assert_decoder_state_eq(&decoder, &ArithmeticDecoder::new(&coded).unwrap());
}

fn assert_invalid_probability(result: &Result<(), GenericRegionError>) {
    assert!(matches!(
        result,
        Err(GenericRegionError::Decode(DecodeError::Malformed(
            MalformedError::InvalidArithmeticContextState {
                state_index: 47,
                mps: 0
            }
        )))
    ));
}

#[test]
fn fresh_context_banks_have_exact_metadata_and_zero_probability_states() {
    let limits = Limits::default();
    assert_eq!(core::mem::size_of::<ArithmeticContext>(), 2);
    for (template, count) in [
        (GenericTemplate::Template0, 65_536),
        (GenericTemplate::Template1, 8_192),
        (GenericTemplate::Template2, 1_024),
        (GenericTemplate::Template3, 1_024),
    ] {
        for extended in [false, true] {
            let bank = GenericContexts::new(template, extended, &limits).unwrap();
            assert_eq!(bank.template, template);
            assert_eq!(bank.extended_template, extended);
            assert_eq!(bank.contexts.len(), count);
            assert_eq!(bank.storage_bytes, u64::try_from(count).unwrap() * 2);
            assert!(bank
                .contexts
                .iter()
                .all(|c| c.state_index() == 0 && c.mps() == 0));
        }
    }
    for (template, count) in [
        (RefinementTemplate::Template0, 8_192),
        (RefinementTemplate::Template1, 1_024),
    ] {
        let bank = RefinementContexts::new(template, &limits).unwrap();
        assert_eq!(bank.template, template);
        assert_eq!(bank.contexts.len(), count);
        assert_eq!(bank.storage_bytes, u64::try_from(count).unwrap() * 2);
        assert!(bank
            .contexts
            .iter()
            .all(|c| c.state_index() == 0 && c.mps() == 0));
    }
}

#[test]
fn generic_context_rejections_preserve_full_bank_and_metadata() {
    let limits = Limits::default();
    for extended in [false, true] {
        let mut bank = GenericContexts::new(GenericTemplate::Template3, extended, &limits).unwrap();
        seed_probability_bank(&mut bank.contexts);
        let mut snapshot = bank.try_snapshot(&limits).unwrap();
        assert_eq!(snapshot.template, bank.template);
        assert_eq!(snapshot.extended_template, extended);
        assert_eq!(snapshot.storage_bytes, bank.storage_bytes);
        assert_eq!(snapshot.contexts, bank.contexts);
        snapshot.contexts[7] = ArithmeticContext {
            state_index: 47,
            mps: 0,
        };
        snapshot.contexts[9] = ArithmeticContext {
            state_index: 0,
            mps: 2,
        };
        bank.storage_bytes = 73;
        let before = bank.contexts.clone();
        assert_invalid_probability(&bank.restore(&snapshot, &limits));
        assert_eq!(bank.contexts, before);
        assert_eq!(bank.template, GenericTemplate::Template3);
        assert_eq!(bank.extended_template, extended);
        assert_eq!(bank.storage_bytes, 73);
        bank.contexts[7] = ArithmeticContext {
            state_index: 47,
            mps: 0,
        };
        assert_invalid_probability(&bank.try_snapshot(&limits).map(|_| ()));
        assert_eq!(bank.contexts[7].state_index, 47);
        bank.reset().unwrap();
        assert!(bank
            .contexts
            .iter()
            .all(|c| c.state_index() == 0 && c.mps() == 0));
        assert_eq!(bank.storage_bytes, 73);
        bank.contexts.pop();
        let before = bank.contexts.clone();
        assert!(matches!(
            bank.reset(),
            Err(GenericRegionError::ContextIndex {
                index: 1024,
                count: 1023
            })
        ));
        assert_eq!(bank.contexts, before);
        assert_eq!(bank.template, GenericTemplate::Template3);
        assert_eq!(bank.extended_template, extended);
        assert_eq!(bank.storage_bytes, 73);
    }
}

#[test]
fn refinement_context_rejections_preserve_full_bank_and_metadata() {
    let limits = Limits::default();
    let mut bank = RefinementContexts::new(RefinementTemplate::Template1, &limits).unwrap();
    seed_probability_bank(&mut bank.contexts);
    let mut snapshot = bank.try_snapshot(&limits).unwrap();
    assert_eq!(snapshot.template, bank.template);
    assert_eq!(snapshot.storage_bytes, bank.storage_bytes);
    assert_eq!(snapshot.contexts, bank.contexts);
    snapshot.contexts[7] = ArithmeticContext {
        state_index: 47,
        mps: 0,
    };
    snapshot.contexts[9] = ArithmeticContext {
        state_index: 0,
        mps: 2,
    };
    bank.storage_bytes = 73;
    let before = bank.contexts.clone();
    assert_invalid_probability(&bank.restore(&snapshot, &limits));
    assert_eq!(bank.contexts, before);
    assert_eq!(bank.template, RefinementTemplate::Template1);
    assert_eq!(bank.storage_bytes, 73);
    bank.contexts[7] = ArithmeticContext {
        state_index: 47,
        mps: 0,
    };
    assert_invalid_probability(&bank.try_snapshot(&limits).map(|_| ()));
    assert_eq!(bank.contexts[7].state_index, 47);
    bank.reset().unwrap();
    assert!(bank
        .contexts
        .iter()
        .all(|c| c.state_index() == 0 && c.mps() == 0));
    assert_eq!(bank.storage_bytes, 73);
    bank.contexts.pop();
    let before = bank.contexts.clone();
    assert!(matches!(
        bank.reset(),
        Err(GenericRegionError::ContextIndex {
            index: 1024,
            count: 1023
        })
    ));
    assert_eq!(bank.contexts, before);
    assert_eq!(bank.template, RefinementTemplate::Template1);
    assert_eq!(bank.storage_bytes, 73);
}

#[test]
fn generic_arithmetic_session_preserves_contexts_and_stream() {
    const ANNEX_H_ENCODED: [u8; 30] = [
        0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb,
        0x86, 0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
    ];
    let limits = Limits::default();
    let header = generic_header(4, 1, GenericTemplate::Template3, false, false);
    let call = GenericArithmeticCall {
        width: 4,
        height: 1,
        adaptive: header.adaptive,
        typical_prediction: false,
        skip: None,
    };
    let mut session =
        GenericArithmeticSession::new(&ANNEX_H_ENCODED, GenericTemplate::Template3, false, &limits)
            .expect("session");

    let first = session.decode_bitmap(call).expect("first bitmap");
    let first_state = session.contexts.contexts[0];
    let first_position = session.consumed_byte_offset();
    let second = session.decode_bitmap(call).expect("second bitmap");
    let second_state = session.contexts.contexts[0];
    let second_position = session.consumed_byte_offset();

    assert!(first.bytes().iter().all(|byte| *byte == 0));
    assert!(second.bytes().iter().all(|byte| *byte == 0));
    assert_ne!(
        first_state, second_state,
        "the second call must reuse, not reset, context zero"
    );
    assert!(
        second_position >= first_position,
        "the continuous stream cannot move backwards"
    );
    assert_eq!(session.validate_final_marker().expect("marker"), 28);
}

#[test]
fn generic_arithmetic_session_stays_bounded_at_observed_marker() {
    let limits = Limits::default();
    let header = generic_header(1, 1, GenericTemplate::Template3, false, false);
    let call = GenericArithmeticCall {
        width: 1,
        height: 1,
        adaptive: header.adaptive,
        typical_prediction: false,
        skip: None,
    };
    let mut session =
        GenericArithmeticSession::new(&[0xff, 0xac], GenericTemplate::Template3, false, &limits)
            .expect("marker-fill session");

    let state_before = session.contexts.contexts[0];
    session.decode_bitmap(call).expect("first bitmap");
    let state_after_first = session.contexts.contexts[0];
    session.decode_bitmap(call).expect("second bitmap");
    let state_after_second = session.contexts.contexts[0];

    assert_ne!(state_before, state_after_first);
    assert_ne!(state_after_first, state_after_second);
    assert_eq!(session.observed_marker_offset(), Some(0));
    assert_eq!(session.consumed_byte_offset(), 0);
    assert_eq!(session.validate_final_marker().expect("bounded marker"), 0);
}

#[test]
fn generic_arithmetic_session_charges_work_cumulatively() {
    let limits = Limits {
        max_work_units: 20,
        ..Limits::default()
    };
    let header = generic_header(1, 1, GenericTemplate::Template3, false, false);
    let call = GenericArithmeticCall {
        width: 1,
        height: 1,
        adaptive: header.adaptive,
        typical_prediction: false,
        skip: None,
    };
    let mut session =
        GenericArithmeticSession::new(&[0xff, 0xac], GenericTemplate::Template3, false, &limits)
            .expect("session");
    session.decode_bitmap(call).expect("first call costs 17");
    assert!(matches!(
        session.decode_bitmap(call),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Work,
                limit: 20,
                actual: 34
            }
        )))
    ));
}

#[test]
fn generic_and_refinement_contexts_snapshot_reset_and_restore() {
    let limits = Limits::default();
    let header = generic_header(1, 1, GenericTemplate::Template3, false, false);
    let mut generic =
        GenericContexts::new(GenericTemplate::Template3, false, &limits).expect("generic contexts");
    let mut decoder = ArithmeticDecoder::new(&[0xff, 0xac]).expect("bounded decoder");
    let mut work = 0;
    decode_generic_on(
        &mut decoder,
        &mut generic,
        GenericArithmeticCall {
            width: 1,
            height: 1,
            adaptive: header.adaptive,
            typical_prediction: false,
            skip: None,
        },
        1,
        &limits,
        &mut work,
    )
    .expect("mutate generic contexts");
    let retained_generic = generic.try_snapshot(&limits).expect("retain generic");
    let generic_state = generic.contexts[0];
    generic.reset().expect("reset generic");
    assert_eq!(generic.contexts[0], ArithmeticContext::new());
    generic
        .restore(&retained_generic, &limits)
        .expect("restore generic");
    assert_eq!(generic.contexts[0], generic_state);

    let mut incompatible = GenericContexts::new(GenericTemplate::Template2, false, &limits)
        .expect("same-sized incompatible contexts");
    assert!(incompatible.restore(&retained_generic, &limits).is_err());

    let mut refinement = RefinementContexts::new(RefinementTemplate::Template1, &limits)
        .expect("refinement contexts");
    let reference = Bitmap::blank(1, 1, &limits).expect("reference");
    decode_refinement_on(
        &mut decoder,
        &mut refinement,
        RefinementArithmeticCall {
            width: 1,
            height: 1,
            adaptive: [TemplateOffset::ZERO; 2],
            typical_prediction: false,
        },
        &reference,
        RefinementParameters {
            reference_dx: 0,
            reference_dy: 0,
        },
        &limits,
        &mut work,
    )
    .expect("mutate refinement contexts");
    let retained_refinement = refinement.try_snapshot(&limits).expect("retain refinement");
    let refinement_state = refinement.contexts[0];
    refinement.reset().expect("reset refinement");
    assert_eq!(refinement.contexts[0], ArithmeticContext::new());
    refinement
        .restore(&retained_refinement, &limits)
        .expect("restore refinement");
    assert_eq!(refinement.contexts[0], refinement_state);

    let mut retained_limited = limits;
    retained_limited.max_retained_state_bytes = 0;
    assert!(matches!(
        generic.try_snapshot(&retained_limited),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::RetainedState,
                ..
            }
        )))
    ));

    let mut allocation_limited = limits;
    allocation_limited.max_decoder_managed_memory_bytes = 0;
    assert!(matches!(
        GenericContexts::new(GenericTemplate::Template3, false, &allocation_limited),
        Err(GenericRegionError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Memory,
                ..
            }
        )))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn raw_refinement_snapshots_preserve_all_states_and_failure_frames() {
    let limits = Limits::default();
    for (template, count, code) in [
        (RefinementTemplate::Template0, 8192, 0),
        (RefinementTemplate::Template1, 1024, 1),
    ] {
        let mut source = RefinementContexts::new(template, &limits).unwrap();
        for (index, context) in source.contexts.iter_mut().enumerate() {
            *context = ArithmeticContext::from_state(
                u8::try_from(index % 47).unwrap(),
                u8::try_from((index / 47) % 2).unwrap(),
            )
            .unwrap();
        }
        let bytes = u64::try_from(count).unwrap() * 2;
        let exact = Limits {
            max_retained_state_bytes: bytes,
            max_decoder_managed_memory_bytes: bytes * 2,
            ..limits
        };
        let snapshot = source.try_snapshot(&exact).unwrap();
        assert_eq!(snapshot.contexts, source.contexts);
        assert_ne!(snapshot.contexts.as_ptr(), source.contexts.as_ptr());
        source.reset().unwrap();
        assert!(source
            .contexts
            .iter()
            .all(|c| *c == ArithmeticContext::new()));
        source.restore(&snapshot, &exact).unwrap();
        assert_eq!(source.contexts, snapshot.contexts);
        assert_eq!(source.storage_bytes, bytes);
        for target_template in [RefinementTemplate::Template0, RefinementTemplate::Template1] {
            let mut target = RefinementContexts::new(target_template, &limits).unwrap();
            target
                .contexts
                .fill(ArithmeticContext::from_state(46, 1).unwrap());
            let before = target.contexts.clone();
            let result = target.restore(&snapshot, &exact);
            if target_template == template {
                result.unwrap();
                assert_eq!(target.contexts, snapshot.contexts);
            } else {
                assert!(matches!(result, Err(GenericRegionError::InvalidFlags(c)) if c == code));
                assert_eq!(target.contexts, before);
            }
        }
        for (limited, kind, limit, actual) in [
            (
                Limits {
                    max_retained_state_bytes: bytes - 1,
                    ..exact
                },
                LimitKind::RetainedState,
                bytes - 1,
                bytes,
            ),
            (
                Limits {
                    max_decoder_managed_memory_bytes: bytes * 2 - 1,
                    ..exact
                },
                LimitKind::Memory,
                bytes * 2 - 1,
                bytes * 2,
            ),
        ] {
            let before = source.contexts.clone();
            assert_raw_guard_result(
                source.try_snapshot(&limited).map(|_| ()),
                Err(Some(LimitExceeded {
                    kind,
                    limit,
                    actual,
                })),
            );
            assert_raw_guard_result(
                source.restore(&snapshot, &limited),
                Err(Some(LimitExceeded {
                    kind,
                    limit,
                    actual,
                })),
            );
            assert_eq!(source.contexts, before);
        }
        source.contexts.pop();
        let before = source.contexts.clone();
        assert!(matches!(source.restore(&snapshot, &exact),
            Err(GenericRegionError::ContextIndex { index, count: actual })
                if index == count && actual == count - 1));
        assert_eq!(source.contexts, before);
        let mut invalid = RefinementContexts::new(template, &limits)
            .unwrap()
            .try_snapshot(&limits)
            .unwrap();
        invalid.contexts.pop();
        assert!(matches!(source.restore(&invalid, &exact),
            Err(GenericRegionError::ContextIndex { index, count: actual })
                if index == count && actual == count - 1));
        assert_eq!(source.contexts, before);
    }
    let mut generic = GenericContexts::new(GenericTemplate::Template3, false, &limits).unwrap();
    let snapshot = generic.try_snapshot(&limits).unwrap();
    generic.contexts.pop();
    let before = generic.contexts.clone();
    assert!(matches!(
        generic.restore(&snapshot, &limits),
        Err(GenericRegionError::ContextIndex {
            index: 1024,
            count: 1023
        })
    ));
    assert_eq!(generic.contexts, before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn generic_snapshots_copy_all_states_and_preserve_rejected_targets() {
    let limits = Limits::default();
    let templates = [
        (GenericTemplate::Template0, false, 65_536, 0),
        (GenericTemplate::Template0, true, 65_536, 0x10),
        (GenericTemplate::Template1, false, 8_192, 2),
        (GenericTemplate::Template2, false, 1_024, 4),
        (GenericTemplate::Template3, false, 1_024, 6),
    ];
    for (template, extended, count, code) in templates {
        let mut source = GenericContexts::new(template, extended, &limits).unwrap();
        assert_eq!(source.contexts.len(), count);
        for (index, context) in source.contexts.iter_mut().enumerate() {
            *context = ArithmeticContext::from_state(
                u8::try_from(index % 47).unwrap(),
                u8::try_from(index % 2).unwrap(),
            )
            .unwrap();
        }
        let bytes = u64::try_from(count).unwrap() * 2;
        let exact = Limits {
            max_retained_state_bytes: bytes,
            max_decoder_managed_memory_bytes: bytes * 2,
            ..limits
        };
        let snapshot = source.try_snapshot(&exact).unwrap();
        assert_ne!(source.contexts.as_ptr(), snapshot.contexts.as_ptr());
        assert_eq!(snapshot.storage_bytes, bytes);
        source.reset().unwrap();
        assert!(source
            .contexts
            .iter()
            .all(|c| c.state_index() == 0 && c.mps() == 0));
        for (target_template, target_extended, _, _) in templates {
            let mut target =
                GenericContexts::new(target_template, target_extended, &limits).unwrap();
            target
                .contexts
                .fill(ArithmeticContext::from_state(46, 1).unwrap());
            let result = target.restore(&snapshot, &exact);
            if (target_template, target_extended) == (template, extended) {
                result.unwrap();
                for (index, context) in target.contexts.iter().enumerate() {
                    assert_eq!(usize::from(context.state_index()), index % 47);
                    assert_eq!(usize::from(context.mps()), index % 2);
                }
            } else {
                assert!(matches!(result, Err(GenericRegionError::InvalidFlags(c)) if c == code));
                assert!(target
                    .contexts
                    .iter()
                    .all(|c| c.state_index() == 46 && c.mps() == 1));
            }
        }
        for (limited, kind, limit, actual) in [
            (
                Limits {
                    max_retained_state_bytes: bytes - 1,
                    ..exact
                },
                LimitKind::RetainedState,
                bytes - 1,
                bytes,
            ),
            (
                Limits {
                    max_decoder_managed_memory_bytes: bytes * 2 - 1,
                    ..exact
                },
                LimitKind::Memory,
                bytes * 2 - 1,
                bytes * 2,
            ),
        ] {
            assert_raw_guard_result(
                source.try_snapshot(&limited).map(|_| ()),
                Err(Some(LimitExceeded {
                    kind,
                    limit,
                    actual,
                })),
            );
            let mut target = GenericContexts::new(template, extended, &limits).unwrap();
            target
                .contexts
                .fill(ArithmeticContext::from_state(46, 1).unwrap());
            assert_raw_guard_result(
                target.restore(&snapshot, &limited),
                Err(Some(LimitExceeded {
                    kind,
                    limit,
                    actual,
                })),
            );
            assert!(target
                .contexts
                .iter()
                .all(|c| c.state_index() == 46 && c.mps() == 1));
            assert!(source
                .contexts
                .iter()
                .all(|c| c.state_index() == 0 && c.mps() == 0));
        }
        for (index, context) in snapshot.contexts.iter().enumerate() {
            assert_eq!(usize::from(context.state_index()), index % 47);
            assert_eq!(usize::from(context.mps()), index % 2);
        }
    }
}

#[test]
fn arithmetic_procedures_interleave_on_one_decoder() {
    const ANNEX_H_ENCODED: [u8; 30] = [
        0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb,
        0x86, 0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
    ];
    let limits = Limits::default();
    let mut decoder = ArithmeticDecoder::new(&ANNEX_H_ENCODED).expect("decoder");
    let mut integer_contexts = [ArithmeticContext::new(); 512];
    let first_integer = decoder
        .decode_integer(&mut integer_contexts)
        .expect("first integer");

    let header = generic_header(1, 1, GenericTemplate::Template3, false, false);
    let mut generic =
        GenericContexts::new(GenericTemplate::Template3, false, &limits).expect("generic contexts");
    let mut cumulative_work = 0;
    let generic_bitmap = decode_generic_on(
        &mut decoder,
        &mut generic,
        GenericArithmeticCall {
            width: 1,
            height: 1,
            adaptive: header.adaptive,
            typical_prediction: false,
            skip: None,
        },
        1,
        &limits,
        &mut cumulative_work,
    )
    .expect("generic bitmap");

    let second_integer = decoder
        .decode_integer(&mut integer_contexts)
        .expect("second integer");

    let reference = Bitmap::blank(1, 1, &limits).expect("reference");
    let mut refinement = RefinementContexts::new(RefinementTemplate::Template1, &limits)
        .expect("refinement contexts");
    let refined_bitmap = decode_refinement_on(
        &mut decoder,
        &mut refinement,
        RefinementArithmeticCall {
            width: 1,
            height: 1,
            adaptive: [TemplateOffset::ZERO; 2],
            typical_prediction: false,
        },
        &reference,
        RefinementParameters {
            reference_dx: 0,
            reference_dy: 0,
        },
        &limits,
        &mut cumulative_work,
    )
    .expect("refinement bitmap");

    assert_ne!(first_integer, second_integer);
    assert!(generic_bitmap.pixel(0, 0).is_some());
    assert!(refined_bitmap.pixel(0, 0).is_some());
    assert_eq!(cumulative_work, 32);
    validate_observed_marker(&decoder, ANNEX_H_ENCODED.len())
        .expect("any observed marker remains the bounded final marker");
}
