#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn single_context_restore_reset_and_default_cover_every_stored_byte_pair() {
    for state_index in 0..=u8::MAX {
        for mps in 0..=u8::MAX {
            match ArithmeticContext::from_state(state_index, mps) {
                Ok(mut context) => {
                    assert!(state_index < 47 && mps <= 1);
                    assert_eq!((context.state_index(), context.mps()), (state_index, mps));
                    context.reset();
                    assert_eq!(context, ArithmeticContext::default());
                    context.reset();
                    assert_eq!((context.state_index(), context.mps()), (0, 0));
                }
                Err(error) => {
                    assert!(state_index >= 47 || mps > 1);
                    assert_eq!(
                        error,
                        ArithmeticError::InvalidContextState { state_index, mps },
                    );
                }
            }
        }
    }
}

#[test]
fn arithmetic_error_adapters_and_formatters_preserve_every_variant_and_field() {
    use std::fmt::Write as _;

    struct RejectWrites;
    impl fmt::Write for RejectWrites {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    for via_from in [false, true] {
        for (error, expected, message) in [
            (
                ArithmeticError::from(MalformedError::IntegerOverflow),
                MalformedError::IntegerOverflow,
                "input-derived integer overflow".to_owned(),
            ),
            (
                ArithmeticError::InvalidContextState {
                    state_index: u8::MAX,
                    mps: u8::MAX,
                },
                MalformedError::InvalidArithmeticContextState {
                    state_index: u8::MAX,
                    mps: u8::MAX,
                },
                "invalid arithmetic context state I=255, MPS=255".to_owned(),
            ),
            (
                ArithmeticError::InsufficientContexts {
                    required: usize::MAX,
                    actual: 0,
                },
                MalformedError::InsufficientArithmeticContexts {
                    required: usize::MAX,
                    actual: 0,
                },
                format!(
                    "arithmetic decoding requires {} contexts, but 0 were supplied",
                    usize::MAX
                ),
            ),
            (
                ArithmeticError::InvalidIaidCodeLength(u8::MAX),
                MalformedError::InvalidIaidCodeLength(u8::MAX),
                "unsupported IAID code length 255".to_owned(),
            ),
        ] {
            assert_eq!(error.to_string(), message);
            assert!(std::error::Error::source(&error).is_none());
            assert!(write!(&mut RejectWrites, "{error}").is_err());
            let converted = if via_from {
                DecodeError::from(error)
            } else {
                error.into_decode()
            };
            match converted {
                DecodeError::Malformed(actual) => assert_eq!(actual, expected),
                other => panic!("unexpected arithmetic error conversion: {other:?}"),
            }
        }
    }
}

#[test]
fn contexts_reset_and_restore_safely() {
    let mut contexts = [
        ArithmeticContext::from_state(14, 1).expect("valid"),
        ArithmeticContext::from_state(46, 0).expect("valid"),
    ];
    reset_contexts(&mut contexts);
    assert_eq!(contexts, [ArithmeticContext::new(); 2]);
    assert!(matches!(
        ArithmeticContext::from_state(47, 0),
        Err(ArithmeticError::InvalidContextState { .. })
    ));
    assert!(matches!(
        ArithmeticContext::from_state(0, 2),
        Err(ArithmeticError::InvalidContextState { .. })
    ));
}

#[test]
fn batch_reset_zeros_all_stored_byte_pairs_and_is_idempotent() {
    reset_contexts(&mut []);
    let mut contexts = Vec::new();
    for packed in 0..=u8::MAX {
        contexts.push(ArithmeticContext::from_packed(packed));
    }
    let length = contexts.len();
    reset_contexts(&mut contexts);
    assert_eq!(contexts.len(), length);
    assert!(contexts
        .iter()
        .all(|context| context.state_index() == 0 && context.mps() == 0));
    reset_contexts(&mut contexts);
    assert!(contexts
        .iter()
        .all(|context| context.state_index() == 0 && context.mps() == 0));
}

#[test]
fn packed_lookup_tables_match_qe_states_for_every_valid_context() {
    assert_eq!(core::mem::size_of::<ArithmeticContext>(), 1);
    for packed in 0..94_u8 {
        let context = ArithmeticContext::from_packed(packed);
        let state = QE_STATES[usize::from(context.state_index())];
        assert_eq!(QE_PACKED_VALS[usize::from(packed)], state.qe);
        let next = ArithmeticContext::from_packed(NEXT_MPS_PACKED[usize::from(packed)]);
        assert!(next.packed() < 94);
        assert_eq!(next.state_index(), state.next_mps);
        assert_eq!(next.mps(), context.mps());
    }
}

#[test]
fn batch_reset_only_changes_the_selected_slice() {
    let original = [(1, 0), (2, 1), (3, 0), (4, 1), (5, 0), (46, 1)]
        .map(|(index, mps)| ArithmeticContext::from_state(index, mps).expect("valid"));
    for start in 0..=original.len() {
        for end in start..=original.len() {
            let mut contexts = original;
            reset_contexts(&mut contexts[start..end]);
            for (index, actual) in contexts.iter().enumerate() {
                let expected = if (start..end).contains(&index) {
                    ArithmeticContext::new()
                } else {
                    original[index]
                };
                assert_eq!(*actual, expected, "range {start}..{end}, slot {index}");
            }
        }
    }
}
