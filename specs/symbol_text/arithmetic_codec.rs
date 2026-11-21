// Source-only author navigation, not normative approval.
// Guide: ../README.md; audit ledger: ../../validation/traceability/requirements.json

use super::*;

verus! {

// Model navigation: spec::integer_contexts_initial [project-result-model]
// Purpose: Exact zero probability state in every freshly constructed integer bank.
// Audit: symbol-integer-contexts-initial
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn integer_contexts_initial(
    contexts: &crate::symbol_text::IntegerContexts,
) -> bool {
    forall|slot: int, index: int|
        0 <= slot < crate::symbol_text::INTEGER_FIELD_COUNT && 0 <= index < 512 ==>
            #[trigger] contexts.slots[slot][index]
                == crate::arithmetic::ArithmeticContext { state_index: 0, mps: 0 }
}

// Model navigation: spec::arithmetic_integer_transition [project-result-model]
// Purpose: Preserve the concrete integer result and full bank/cursor frame through enum dispatch.
// Audit: symbol-arithmetic-integer-transition
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn arithmetic_integer_transition<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after: &crate::symbol_text::SymbolCodec<'a>,
    field: crate::symbol_text::IntegerField,
    result: Result<crate::symbol_text::EntropyInteger, crate::symbol_text::SymbolTextError>,
) -> bool
    recommends codec_valid(before),
{
    match *before {
        crate::symbol_text::SymbolCodec::Arithmetic(prior) => match *after {
            crate::symbol_text::SymbolCodec::Arithmetic(next) => {
                let slot = integer_field_slot(field) as int;
                let state = crate::arithmetic::arithmetic_spec::integer_state(
                    prior.decoder, prior.integers.slots[slot]@,
                );
                &&& result == arithmetic_integer_result(state.0)
                &&& next.decoder == state.1
                &&& next.decoder.bytes == prior.decoder.bytes
                &&& next.decoder.bytes@ == prior.decoder.bytes@
                &&& next.integers.slots[slot]@ == state.2
                &&& forall|other: int| 0 <= other < crate::symbol_text::INTEGER_FIELD_COUNT
                    && other != slot ==> next.integers.slots[other]@ == prior.integers.slots[other]@
                &&& next.iaid@ == prior.iaid@
                &&& next.refinement == prior.refinement
                &&& next.refinement_work == prior.refinement_work
            },
            _ => false,
        },
        _ => true,
    }
}

// Model navigation: spec::arithmetic_symbol_id_transition [project-result-model]
// Purpose: Preserve exact IAID result/state, numeric range and untouched banks through enum dispatch.
// Audit: symbol-arithmetic-symbol-id-transition
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn arithmetic_symbol_id_transition<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after: &crate::symbol_text::SymbolCodec<'a>,
    request: crate::symbol_text::SymbolIdRequest,
    result: Result<u32, crate::symbol_text::SymbolTextError>,
) -> bool
    recommends codec_valid(before),
{
    match *before {
        crate::symbol_text::SymbolCodec::Arithmetic(prior) => match *after {
            crate::symbol_text::SymbolCodec::Arithmetic(next) => {
                let state = crate::arithmetic::arithmetic_spec::iaid_state(
                    prior.decoder, prior.iaid@, request.code_length,
                );
                &&& result == arithmetic_symbol_id_result(state.0)
                &&& next.decoder == state.1
                &&& next.iaid@ == state.2
                &&& next.integers.slots@ == prior.integers.slots@
                &&& next.refinement == prior.refinement
                &&& next.refinement_work == prior.refinement_work
                &&& match result {
                    Ok(value) => request.code_length <= crate::arithmetic::MAX_IAID_CODE_LENGTH
                        && (value as u64) < (1u64 << request.code_length),
                    Err(_) => true,
                }
            },
            _ => false,
        },
        _ => true,
    }
}

}
