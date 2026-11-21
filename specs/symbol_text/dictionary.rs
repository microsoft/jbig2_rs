// Source-only author navigation, not normative approval.
// Guide: ../README.md; audit ledger: ../../validation/traceability/requirements.json

use super::*;

verus! {

// T.88 (08/2018) 7.4.2.2 step 3: model identity excludes counts,
// Huffman table choices and the used/retained lifecycle flags.
pub(in crate::symbol_text) open spec fn dictionary_context_models_match(
    current: &crate::symbol_text::SymbolDictionaryHeader,
    retained: &crate::symbol_text::SymbolDictionaryHeader,
) -> bool {
    current.config.huffman == retained.config.huffman
        && current.config.refinement_aggregation == retained.config.refinement_aggregation
        && current.config.template == retained.config.template
        && current.config.refinement_template == retained.config.refinement_template
        && current.adaptive_count == retained.adaptive_count
        && (forall|index: int| 0 <= index < 4 ==>
            current.adaptive@[index] == retained.adaptive@[index])
        && (forall|index: int| 0 <= index < 2 ==>
            current.refinement_adaptive@[index] == retained.refinement_adaptive@[index])
}

pub(in crate::symbol_text) open spec fn dictionary_context_validation(
    header: &crate::symbol_text::SymbolDictionaryHeader,
    used_contexts: Option<&crate::symbol_text::DictionaryContexts>,
) -> Result<(), crate::symbol_text::SymbolTextError> {
    if header.config.bitmap_context_used {
        match used_contexts {
            None => Err(crate::symbol_text::SymbolTextError::InvalidState("missing retained contexts")),
            Some(used) =>
                if !header.config.huffman && !header.config.refinement_aggregation
                    && used.generic.is_none() {
                    Err(crate::symbol_text::SymbolTextError::InvalidState("missing retained generic contexts"))
                } else if header.config.refinement_aggregation && used.refinement.is_none() {
                    Err(crate::symbol_text::SymbolTextError::InvalidState("missing retained refinement contexts"))
                } else if !dictionary_context_models_match(header, &used.model) {
                    Err(crate::symbol_text::SymbolTextError::InvalidState(
                        "incompatible retained dictionary context model",
                    ))
                } else {
                    Ok(())
                },
        }
    } else if used_contexts.is_some() {
        Err(crate::symbol_text::SymbolTextError::InvalidState(
            "unexpected retained contexts without the context-used flag",
        ))
    } else {
        Ok(())
    }
}

pub(in crate::symbol_text) open spec fn bounded_arithmetic_terminator_result(
    data: Seq<u8>,
) -> Result<(), crate::symbol_text::SymbolTextError> {
    if data.len() < 2 || data[data.len() - 2] != 0xff || data[data.len() - 1] != 0xac {
        Err(crate::symbol_text::SymbolTextError::InvalidState(
            "embedded arithmetic refinement data is missing its FF AC terminator",
        ))
    } else if exists|index: int| 0 <= index < data.len() - 2
        && #[trigger] data[index] == 0xff && data[index + 1] > 0x8f {
        Err(crate::symbol_text::SymbolTextError::InvalidState(
            "premature arithmetic marker in embedded refinement data",
        ))
    } else {
        Ok(())
    }
}

pub(in crate::symbol_text) open spec fn normalized_arithmetic_bytes(
    normalized: &crate::symbol_text::NormalizedArithmetic,
) -> Seq<u8> {
    match normalized {
        crate::symbol_text::NormalizedArithmetic::Borrowed(bytes) => bytes@,
        crate::symbol_text::NormalizedArithmetic::Owned(bytes) => bytes@,
    }
}

// Existing bounded-section policy accepts FF AC or normalizes a terminal FF.
pub(in crate::symbol_text) open spec fn bounded_arithmetic_source_tail(data: Seq<u8>) -> bool {
    (data.len() >= 2 && data[data.len() - 2] == 0xff && data[data.len() - 1] == 0xac)
        || (data.len() > 0 && data[data.len() - 1] == 0xff)
}

pub(crate) open spec fn symbol_count_result(
    imported: u32,
    new_symbols: u32,
    exported: u32,
    limits: &crate::Limits,
) -> Result<u32, crate::symbol_text::SymbolTextError> {
    let total = imported as nat + new_symbols as nat;
    if total > u32::MAX {
        Err(crate::symbol_text::SymbolTextError::Decode(crate::DecodeError::Malformed(
            crate::MalformedError::IntegerOverflow,
        )))
    } else if total > limits.max_symbols {
        Err(crate::symbol_text::SymbolTextError::Decode(crate::DecodeError::LimitExceeded(
            crate::LimitExceeded {
                kind: crate::LimitKind::Symbols,
                limit: limits.max_symbols as u64,
                actual: total as u64,
            },
        )))
    } else if exported > total {
        Err(crate::symbol_text::SymbolTextError::InvalidCount("exported symbols exceed candidates"))
    } else {
        Ok(total as u32)
    }
}

// Model navigation: spec::dictionary_config_safe [candidate-standard-facing-model]
// Purpose: Selected dictionary/text flags, strip/refinement choices and OOB candidates.
// Audit: dictionary-config-safe
// REQUIREMENT CONTEXT: T88-AGGREGATION-MODE-FLAG | ITU-T T.88 (08/2018) | 7.4.2.1.1 bit 1; 6.5.8.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn dictionary_config_safe(
    flags: u16,
    config: crate::symbol_text::SymbolDictionaryConfig,
) -> bool {
    flags & 0xe000 == 0
        && config.huffman == (flags & 1 != 0)
        && config.refinement_aggregation == (flags & 2 != 0)
}

// Model navigation: spec::dictionary_state_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Audit: dictionary-state-valid
// REQUIREMENT CONTEXT: T88-DICTIONARY-HEIGHT-CLASS-STATE | ITU-T T.88 (08/2018) | 6.5.5 step 4; 6.5.6-6.5.7
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn dictionary_state_valid(
    state: &crate::symbol_text::DictionaryClassState,
) -> bool {
    state.decoded_symbols <= state.declared_new_symbols
        && state.width <= state.total_width
}

}
