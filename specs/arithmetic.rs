// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

#[path = "arithmetic/packed.rs"]
mod packed;
pub(crate) use packed::{context_index, context_mps, packed_context};

verus! {

// Model navigation: spec::context_valid [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Audit: context-valid
// REQUIREMENT CONTEXT: T88-ARITHMETIC-RESET-ONE | ITU-T T.88 (08/2018) | E.3.7
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn context_valid(
    context: super::ArithmeticContext,
) -> bool {
    context.packed < 94
}

// Model navigation: spec::exchange_is_mps [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-exchange-is-mps
// REQUIREMENT CONTEXT: T88-ARITHMETIC-EXCHANGE-MPS-COMPARE | ITU-T T.88 (08/2018) | E.3.2, Figure E.16, A less than Qe test (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn exchange_is_mps(lower: bool, interval: u32, qe: u32) -> bool {
    (interval < qe) == lower
}

// Model navigation: spec::exchange_decision [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-exchange-decision
// REQUIREMENT CONTEXT: T88-ARITHMETIC-EXCHANGE-MPS-COMPARE | ITU-T T.88 (08/2018) | E.3.2, Figure E.16, A less than Qe test (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn exchange_decision(
    lower: bool, interval: u32, qe: u32, old_mps: u8,
) -> u8
    recommends old_mps <= 1,
{
    if exchange_is_mps(lower, interval, qe) {
        old_mps
    } else {
        (1 - old_mps) as u8
    }
}

// Model navigation: spec::exchange_index [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-exchange-index
// REQUIREMENT CONTEXT: T88-ARITHMETIC-EXCHANGE-MPS-COMPARE | ITU-T T.88 (08/2018) | E.3.2, Figure E.16, A less than Qe test (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn exchange_index(
    lower: bool, interval: u32, qe: u32, next_mps: u8, next_lps: u8,
) -> u8 {
    if exchange_is_mps(lower, interval, qe) { next_mps } else { next_lps }
}

// Model navigation: spec::exchange_mps [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-exchange-mps
// REQUIREMENT CONTEXT: T88-ARITHMETIC-EXCHANGE-MPS-COMPARE | ITU-T T.88 (08/2018) | E.3.2, Figure E.16, A less than Qe test (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn exchange_mps(
    lower: bool, interval: u32, qe: u32, switch_mps: bool,
    prior_context_mps: u8, old_mps: u8,
) -> u8
    recommends old_mps <= 1,
{
    if !exchange_is_mps(lower, interval, qe) && switch_mps {
        exchange_decision(lower, interval, qe, old_mps)
    } else {
        prior_context_mps
    }
}

// Model navigation: spec::entry_decision [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn entry_decision(
    interval: u32, code: u32, index: u8, mps: u8,
) -> u8
    recommends index < 47, mps <= 1, interval >= super::MIN_INTERVAL,
{
    let state = super::QE_STATES@[index as int];
    let qe = state.qe as u32;
    let reduced = (interval as int - qe as int) as u32;
    if code >> 16 >= qe && reduced >= super::MIN_INTERVAL {
        mps
    } else {
        exchange_decision(code >> 16 < qe, reduced, qe, mps)
    }
}

// Model navigation: spec::entry_index [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn entry_index(
    interval: u32, code: u32, index: u8,
) -> u8
    recommends index < 47, interval >= super::MIN_INTERVAL,
{
    let state = super::QE_STATES@[index as int];
    let qe = state.qe as u32;
    let reduced = (interval as int - qe as int) as u32;
    if code >> 16 >= qe && reduced >= super::MIN_INTERVAL {
        index
    } else {
        exchange_index(code >> 16 < qe, reduced, qe, state.next_mps, state.next_lps)
    }
}

// Model navigation: spec::entry_mps [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn entry_mps(
    interval: u32, code: u32, index: u8, mps: u8,
) -> u8
    recommends index < 47, mps <= 1, interval >= super::MIN_INTERVAL,
{
    let state = super::QE_STATES@[index as int];
    let qe = state.qe as u32;
    let reduced = (interval as int - qe as int) as u32;
    if code >> 16 >= qe && reduced >= super::MIN_INTERVAL {
        mps
    } else {
        exchange_mps(code >> 16 < qe, reduced, qe, state.switch_mps, mps, mps)
    }
}

// Representation preconditions used by production contracts, not normative rules.
// Model navigation: spec::decoder_position_valid [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_position_valid(
    decoder: &super::ArithmeticDecoder,
) -> bool {
    decoder.byte_position < decoder.bytes@.len()
        && decoder.current_byte == decoder.bytes@[decoder.byte_position as int]
}

// Model navigation: spec::decoder_state_valid [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Audit: decoder-state-valid
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_state_valid(
    decoder: &super::ArithmeticDecoder,
) -> bool {
    decoder_position_valid(decoder)
        && 0 < decoder.interval
        && decoder.interval <= 0xffff
        && decoder.bit_count <= 8
        && match decoder.marker {
            Some(marker) =>
                marker.offset == decoder.byte_position
                    && marker.offset + 1 < decoder.bytes@.len()
                    && decoder.bytes@[marker.offset as int] == 0xff
                    && decoder.bytes@[(marker.offset + 1) as int] == marker.code
                    && marker.code > 0x8f,
            None => true,
        }
}

// Model navigation: spec::decoder_valid [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Audit: decoder-valid
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DECISION | ITU-T T.88 (08/2018) | E.3.2; Figures E.15-E.17 (pp. 119-121)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_valid(
    decoder: &super::ArithmeticDecoder,
) -> bool {
    decoder_state_valid(decoder)
        && decoder.interval >= super::MIN_INTERVAL
}

// Audit: arithmetic-refill-step
// T.88 (08/2018), E.3.4 / Figure E.19: https://www.itu.int/rec/T-REC-T.88-201808-I/en
// Inherited ledger locator; independent BYTEIN and caller interpretation remain pending.
// Model navigation: spec::refill_step [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// REQUIREMENT CONTEXT: T88-ARITHMETIC-BYTEIN-BRANCH | ITU-T T.88 (08/2018) | E.3.4, Figure E.19, B equals FF test (p. 122)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refill_step(
    decoder: &super::ArithmeticDecoder,
) -> (u8, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_state_valid(decoder),
{
    let next_position = decoder.byte_position as nat + 1;
    if next_position >= decoder.bytes@.len() {
        (
            super::RASTER_SIGNAL_EOF, decoder.byte_position, decoder.current_byte,
            decoder.code, decoder.bit_count, decoder.marker,
        )
    } else {
        let next = decoder.bytes@[next_position as int];
        let marker = decoder.current_byte == 0xff && next > 0x8f;
        let increment: u32 = if marker {
            0xff00
        } else if decoder.current_byte == 0xff {
            (next as u32) << 9
        } else {
            (next as u32) << 8
        };
        let position = if marker { decoder.byte_position } else { next_position as usize };
        let current = if marker { decoder.current_byte } else { next };
        let code = decoder.code as nat + increment as nat;
        if code > u32::MAX {
            (
                super::RASTER_SIGNAL_OVERFLOW, position, current,
                decoder.code, decoder.bit_count, decoder.marker,
            )
        } else {
            (
                0, position, current, code as u32,
                if decoder.current_byte == 0xff && !marker { 7u8 } else { 8u8 },
                if marker {
                    Some(super::ArithmeticMarker { offset: decoder.byte_position, code: next })
                } else {
                    decoder.marker
                },
            )
        }
    }
}

// Audit: arithmetic-initialized-state
// T.88 (08/2018), E.3.5 / Figure E.20: https://www.itu.int/rec/T-REC-T.88-201808-I/en
// Project initialization result, not independent approval of the standard translation.
// Model navigation: spec::initialized_state [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INITDEC-BYTEIN | ITU-T T.88 (08/2018) | E.3.5, Figure E.20, BYTEIN call (p. 122)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn initialized_state(
    bytes: &[u8],
) -> Option<(usize, u8, u32, u8, Option<super::ArithmeticMarker>)> {
    if bytes@.len() < 2 {
        None
    } else {
        let seed = super::ArithmeticDecoder {
            bytes,
            byte_position: 0,
            current_byte: bytes@[0],
            code: (bytes@[0] as u32) << 16,
            interval: super::MIN_INTERVAL,
            bit_count: 0,
            marker: None,
        };
        let refill = refill_step(&seed);
        Some((
            refill.1, refill.2, (refill.3 as nat * 128) as u32,
            (refill.4 as int - 7) as u8, refill.5,
        ))
    }
}

// Model navigation: spec::renormalization_steps [proof-support]
// Purpose: Private decoder projections and the interval-doubling termination measure.
// Audit: arithmetic-renormalization-steps
// REQUIREMENT CONTEXT: T88-ARITHMETIC-REGISTER-LOW-INPUT | ITU-T T.88 (08/2018) | E.3.1, input alignment and cross-half shift (p. 119)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn renormalization_steps(interval: nat) -> nat
    decreases if 0 < interval < super::MIN_INTERVAL {
        super::MIN_INTERVAL - interval
    } else {
        0
    },
{
    if interval == 0 || interval >= super::MIN_INTERVAL {
        0
    } else {
        1 + renormalization_steps(interval * 2)
    }
}

// Model navigation: spec::normalized_state [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-normalized-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-REGISTER-LOW-INPUT | ITU-T T.88 (08/2018) | E.3.1, input alignment and cross-half shift (p. 119)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn normalized_state(
    decoder: &super::ArithmeticDecoder,
) -> (u8, u32, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_state_valid(decoder),
    decreases if 0 < decoder.interval < super::MIN_INTERVAL {
        super::MIN_INTERVAL as int - decoder.interval as int
    } else {
        0
    },
{
    if decoder.interval == 0 || decoder.interval >= super::MIN_INTERVAL {
        (
            0, decoder.interval, decoder.byte_position, decoder.current_byte,
            decoder.code, decoder.bit_count, decoder.marker,
        )
    } else {
        let refill = if decoder.bit_count == 0 {
            refill_step(decoder)
        } else {
            (
                0u8, decoder.byte_position, decoder.current_byte,
                decoder.code, decoder.bit_count, decoder.marker,
            )
        };
        if refill.0 != 0 {
            (
                refill.0, decoder.interval, refill.1, refill.2,
                refill.3, refill.4, refill.5,
            )
        } else {
            let interval = (decoder.interval as nat * 2) as u32;
            if refill.3 as nat * 2 > u32::MAX {
                (
                    super::RASTER_SIGNAL_OVERFLOW, interval, refill.1,
                    refill.2, refill.3, refill.4, refill.5,
                )
            } else {
                normalized_state(&super::ArithmeticDecoder {
                    bytes: decoder.bytes,
                    byte_position: refill.1,
                    current_byte: refill.2,
                    code: (refill.3 as nat * 2) as u32,
                    interval,
                    bit_count: (refill.4 as int - 1) as u8,
                    marker: refill.5,
                })
            }
        }
    }
}

// Model navigation: spec::exchange_normalization [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-exchange-normalization
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DECODE-INTERVAL-SUBTRACT | ITU-T T.88 (08/2018) | E.3.2, Figure E.15, initial interval subtraction (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn exchange_normalization(
    decoder: &super::ArithmeticDecoder, lower: bool, qe: u32,
) -> (u8, u32, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_state_valid(decoder), 0 < qe <= 0x5601,
{
    normalized_state(&super::ArithmeticDecoder {
        bytes: decoder.bytes,
        byte_position: decoder.byte_position,
        current_byte: decoder.current_byte,
        code: decoder.code,
        interval: if lower { qe } else { decoder.interval },
        bit_count: decoder.bit_count,
        marker: decoder.marker,
    })
}

// Model navigation: spec::decision_registers [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-decision-registers
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DECODE-INTERVAL-SUBTRACT | ITU-T T.88 (08/2018) | E.3.2, Figure E.15, initial interval subtraction (p. 120)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decision_registers(
    decoder: &super::ArithmeticDecoder, qe: u32, decision: u8,
) -> (u8, u32, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_valid(decoder), 0 < qe <= 0x5601, decision <= 1,
{
    let interval = (decoder.interval as int - qe as int) as u32;
    let lower = decoder.code >> 16 < qe;
    let code = if lower {
        decoder.code
    } else {
        (decoder.code as int - (qe << 16) as int) as u32
    };
    if !lower && interval >= super::MIN_INTERVAL {
        (
            decision, interval, decoder.byte_position, decoder.current_byte,
            code, decoder.bit_count, decoder.marker,
        )
    } else {
        let normalized = exchange_normalization(&super::ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: decoder.byte_position,
            current_byte: decoder.current_byte,
            code,
            interval,
            bit_count: decoder.bit_count,
            marker: decoder.marker,
        }, lower, qe);
        (
            if normalized.0 == 0 {
                decision
            } else {
                normalized.0
            },
            normalized.1, normalized.2, normalized.3, normalized.4,
            normalized.5, normalized.6,
        )
    }
}

// Model navigation: spec::decision_state [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-decision-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-CONTEXT-SELECT | ITU-T T.88 (08/2018) | A.2, step 2 (p. 89)
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn decision_state(
    decoder: &super::ArithmeticDecoder, context: super::ArithmeticContext,
) -> (u8, u32, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_valid(decoder), context_valid(context),
{
    decision_registers(
        decoder,
        super::QE_STATES@[context_index(context) as int].qe as u32,
        entry_decision(decoder.interval, decoder.code, context_index(context), context_mps(context)),
    )
}

// Model navigation: spec::eof_error [project-policy]
// Purpose: Project arithmetic EOF/overflow diagnostics and DecodeError translation.
// Audit: arithmetic-eof-error
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn eof_error(
    offset: usize, needed: usize, remaining: usize,
) -> super::ArithmeticError {
    if offset as nat > u64::MAX || needed as nat > u64::MAX || remaining as nat > u64::MAX {
        super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)
    } else {
        super::ArithmeticError::Malformed(super::MalformedError::UnexpectedEof {
            offset: offset as u64, needed: needed as u64, remaining: remaining as u64,
        })
    }
}

// Model navigation: spec::error_for_signal [project-policy]
// Purpose: Project arithmetic EOF/overflow diagnostics and DecodeError translation.
// Audit: arithmetic-signal-error
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn error_for_signal(
    decoder: &super::ArithmeticDecoder, signal: u8,
) -> super::ArithmeticError {
    let next = decoder.byte_position as nat + 1;
    if signal == super::RASTER_SIGNAL_EOF && next <= usize::MAX {
        eof_error(next as usize, 1, 0)
    } else {
        super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)
    }
}

// Model navigation: spec::decode_error [project-policy]
// Purpose: Project arithmetic EOF/overflow diagnostics and DecodeError translation.
// Audit: arithmetic-decode-error
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decode_error(error: super::ArithmeticError) -> crate::DecodeError {
    match error {
        super::ArithmeticError::Malformed(error) => crate::DecodeError::Malformed(error),
        super::ArithmeticError::InvalidContextState { state_index, mps } =>
            crate::DecodeError::Malformed(super::MalformedError::InvalidArithmeticContextState {
                state_index, mps,
            }),
        super::ArithmeticError::InsufficientContexts { required, actual } =>
            crate::DecodeError::Malformed(super::MalformedError::InsufficientArithmeticContexts {
                required, actual,
            }),
        super::ArithmeticError::InvalidIaidCodeLength(length) =>
            crate::DecodeError::Malformed(super::MalformedError::InvalidIaidCodeLength(length)),
    }
}

// Model navigation: spec::integer_history_step [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-history-step
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-CONTEXT-SELECT | ITU-T T.88 (08/2018) | A.2, step 2 (p. 89)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_history_step(previous: u16, decision: u8) -> u16
    recommends previous <= 511, decision <= 1,
{
    let shifted = (previous as nat * 2 + decision as nat) as u16;
    if previous < 256 { shifted } else { (shifted & 511) | 256 }
}

// Model navigation: spec::context_after_decision [project-result-model]
// Purpose: Exact project exchange, refill, initialization and normalized register results.
// Audit: arithmetic-context-after-decision
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-CONTEXT-SELECT | ITU-T T.88 (08/2018) | A.2, step 2 (p. 89)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn context_after_decision(
    decoder: &super::ArithmeticDecoder, context: super::ArithmeticContext,
) -> super::ArithmeticContext
    recommends decoder_valid(decoder), context_valid(context),
{
    packed_context(
        entry_index(decoder.interval, decoder.code, context_index(context)),
        entry_mps(decoder.interval, decoder.code, context_index(context), context_mps(context)),
    )
}

// Model navigation: spec::integer_bit_state [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-bit-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-CONTEXT-SELECT | ITU-T T.88 (08/2018) | A.2, step 2 (p. 89)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_bit_state(
    decoder: &super::ArithmeticDecoder,
    contexts: Seq<super::ArithmeticContext>,
    previous: u16,
) -> (u8, u32, usize, u8, u32, u8, Option<super::ArithmeticMarker>)
    recommends decoder_valid(decoder), previous <= 511,
        contexts.len() == 512,
        forall|index: int| 0 <= index < contexts.len() ==> context_valid(contexts[index]),
{
    decision_state(decoder, contexts[(previous & 511) as int])
}

// Model navigation: spec::iaid_run [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-iaid-run
// REQUIREMENT CONTEXT: T88-ARITHMETIC-IAID-LENGTH | ITU-T T.88 (08/2018) | A.3, introductory fixed-length rule (p. 91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn iaid_run<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
    previous: usize,
    remaining: u8,
    required: usize,
) -> (Result<usize, super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>)
    decreases remaining,
{
    if remaining == 0 {
        (Ok(previous), decoder, contexts)
    } else if previous as int >= contexts.len() {
        (
            Err(super::ArithmeticError::InsufficientContexts {
                required, actual: contexts.len() as usize,
            }),
            decoder, contexts,
        )
    } else if !decoder_valid(&decoder) || !context_valid(contexts[previous as int]) {
        // Totality only; the production contract excludes this state.
        (
            Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)),
            decoder, contexts,
        )
    } else {
        let context = contexts[previous as int];
        let step = decision_state(&decoder, context);
        let next_contexts = contexts.update(
            previous as int, context_after_decision(&decoder, context),
        );
        let next_decoder = super::ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: step.2,
            current_byte: step.3,
            code: step.4,
            interval: step.1,
            bit_count: step.5,
            marker: step.6,
        };
        if step.0 > 1 {
            (Err(error_for_signal(&next_decoder, step.0)), next_decoder, next_contexts)
        } else {
            let next = previous as nat * 2 + step.0 as nat;
            if next > usize::MAX {
                (
                    Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)),
                    next_decoder, next_contexts,
                )
            } else {
                iaid_run(next_decoder, next_contexts, next as usize, (remaining - 1) as u8, required)
            }
        }
    }
}

// Model navigation: spec::integer_step_state [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-step-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_step_state<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
    previous: u16,
) -> (Result<u8, super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>, u16) {
    if previous > 511 || previous as int >= contexts.len()
        || !decoder_valid(&decoder) || !context_valid(contexts[previous as int]) {
        // Totality only; production callers supply a valid decoder, bank and history.
        (
            Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)),
            decoder, contexts, previous,
        )
    } else {
        let context = contexts[previous as int];
        let step = decision_state(&decoder, context);
        let next_contexts = contexts.update(
            previous as int, context_after_decision(&decoder, context),
        );
        let next_decoder = super::ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: step.2,
            current_byte: step.3,
            code: step.4,
            interval: step.1,
            bit_count: step.5,
            marker: step.6,
        };
        if step.0 > 1 {
            (Err(error_for_signal(&next_decoder, step.0)), next_decoder, next_contexts, previous)
        } else {
            (
                Ok(step.0), next_decoder, next_contexts,
                integer_history_step(previous, step.0),
            )
        }
    }
}

// Model navigation: spec::integer_bits_run [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-bits-run
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_bits_run<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
    previous: u16,
    remaining: u8,
    value: u32,
) -> (Result<u32, super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>, u16)
    decreases remaining,
{
    if remaining == 0 {
        (Ok(value), decoder, contexts, previous)
    } else {
        let step = integer_step_state(decoder, contexts, previous);
        match step.0 {
            Err(error) => (Err(error), step.1, step.2, step.3),
            Ok(bit) => {
                let next = value as nat * 2 + bit as nat;
                if next > u32::MAX {
                    (
                        Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)),
                        step.1, step.2, step.3,
                    )
                } else {
                    integer_bits_run(step.1, step.2, step.3, (remaining - 1) as u8, next as u32)
                }
            },
        }
    }
}

// Model navigation: spec::integer_selection_state [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-selection-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_selection_state<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
    previous: u16,
    level: u8,
) -> (Result<(u8, u64), super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>, u16)
    decreases 5 - level,
{
    if level >= 5 {
        (Ok((32, 4436)), decoder, contexts, previous)
    } else {
        let step = integer_step_state(decoder, contexts, previous);
        match step.0 {
            Err(error) => (Err(error), step.1, step.2, step.3),
            Ok(bit) => if bit == 0 {
                let leaf: (u8, u64) = match level {
                    0 => (2, 0),
                    1 => (4, 4),
                    2 => (6, 20),
                    3 => (8, 84),
                    _ => (12, 340),
                };
                (Ok(leaf), step.1, step.2, step.3)
            } else {
                integer_selection_state(step.1, step.2, step.3, (level + 1) as u8)
            },
        }
    }
}

// Model navigation: spec::integer_state [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-integer-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn integer_state<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
) -> (Result<super::ArithmeticInteger, super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>) {
    let sign = integer_step_state(decoder, contexts, 1);
    match sign.0 {
        Err(error) => (Err(error), sign.1, sign.2),
        Ok(sign_bit) => {
            let leaf = integer_selection_state(sign.1, sign.2, sign.3, 0);
            match leaf.0 {
                Err(error) => (Err(error), leaf.1, leaf.2),
                Ok((count, offset)) => {
                    let payload = integer_bits_run(leaf.1, leaf.2, leaf.3, count, 0);
                    match payload.0 {
                        Err(error) => (Err(error), payload.1, payload.2),
                        Ok(value) => {
                            let magnitude = offset as nat + value as nat;
                            if magnitude > i64::MAX {
                                (
                                    Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow)),
                                    payload.1, payload.2,
                                )
                            } else {
                                let result = if sign_bit == 1 && magnitude == 0 {
                                    super::ArithmeticInteger::OutOfBand
                                } else if sign_bit == 0 {
                                    super::ArithmeticInteger::Value(magnitude as i64)
                                } else {
                                    super::ArithmeticInteger::Value((-(magnitude as int)) as i64)
                                };
                                (Ok(result), payload.1, payload.2)
                            }
                        },
                    }
                },
            }
        },
    }
}

// Model navigation: spec::iaid_state [project-result-model]
// Purpose: Decision histories, integer payload folds, IAID state and partial-error results.
// Audit: arithmetic-iaid-state
// REQUIREMENT CONTEXT: T88-ARITHMETIC-IAID-LENGTH | ITU-T T.88 (08/2018) | A.3, introductory fixed-length rule (p. 91)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn iaid_state<'a>(
    decoder: super::ArithmeticDecoder<'a>,
    contexts: Seq<super::ArithmeticContext>,
    code_length: u8,
) -> (Result<u32, super::ArithmeticError>, super::ArithmeticDecoder<'a>, Seq<super::ArithmeticContext>)
    recommends
        decoder_valid(&decoder),
        contexts.len() <= usize::MAX,
        forall|index: int| 0 <= index < contexts.len() ==> context_valid(contexts[index]),
{
    if code_length > super::MAX_IAID_CODE_LENGTH {
        (Err(super::ArithmeticError::InvalidIaidCodeLength(code_length)), decoder, contexts)
    } else {
        let required = 1usize << code_length;
        if contexts.len() < required {
            (
                Err(super::ArithmeticError::InsufficientContexts {
                    required, actual: contexts.len() as usize,
                }),
                decoder, contexts,
            )
        } else {
            let run = iaid_run(decoder, contexts, 1, code_length, required);
            let result = match run.0 {
                Err(error) => Err(error),
                Ok(prefix) =>
                    if prefix < required || prefix as nat - required as nat > u32::MAX {
                        Err(super::ArithmeticError::Malformed(super::MalformedError::IntegerOverflow))
                    } else {
                        Ok((prefix as nat - required as nat) as u32)
                    },
            };
            (result, run.1, run.2)
        }
    }
}

// Model navigation: spec::decoder_marker [proof-support]
// Purpose: Private decoder projections and the interval-doubling termination measure.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_marker(decoder: &super::ArithmeticDecoder) -> Option<super::ArithmeticMarker> {
    decoder.marker
}

// Model navigation: spec::decoder_byte_position [proof-support]
// Purpose: Private decoder projections and the interval-doubling termination measure.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_byte_position(decoder: &super::ArithmeticDecoder) -> usize {
    decoder.byte_position
}

}

// Keep closed-model reveal ownership here while separating implementation proofs.
include!("../proofs/model_support/arithmetic.rs");
