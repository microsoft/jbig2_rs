//! Height-class decoding, collective bitmaps and exported dictionary assembly.

use super::bitmap::{
    bitmap_from_packed_bytes, clone_bitmap, crop_bitmap_columns, generic_template_from_u8,
    refinement_template_from_u8,
};
use super::codec::{
    byte_align, decode_checked_symbol_id, new_arithmetic_codec, new_huffman_codec,
    normalized_bounded_arithmetic, validate_bounded_arithmetic_marker,
};
#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::tables::{
    associate_dictionary_tables, materialize_huffman_table, select_dictionary_tables,
    wrap_custom_tables,
};
use super::text::decode_text_region_body;
use super::{
    check_dimensions, check_symbol_counts, checked_unsigned_delta, decode_mmr, enforce_limit,
    parse_symbol_dictionary_header, required_integer, symbol_by_id, symbol_id_code_length,
    ArithmeticDictionaryResult, BitReader, Bitmap, CombinationOperator, DecodeError,
    DictionaryClassState, DictionaryContexts, EntropyInteger, EofbPolicy, GenericArithmeticCall,
    GenericContexts, HuffmanChoiceSet, HuffmanFieldTables, HuffmanTable, IntegerField,
    LimitExceeded, LimitKind, Limits, MalformedError, ReferenceCorner, RefinementContextSnapshot,
    RefinementGeometry, RefinementRequest, ResourceTracker, StandardHuffmanTables, SymbolCodebook,
    SymbolCodec, SymbolDictionaryHeader, SymbolDictionaryInput, SymbolDictionaryOutput,
    SymbolIdRequest, SymbolTextError, TemplateOffset, TextRegionConfig,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Decodes one complete symbol-dictionary segment payload (T.88 6.5, 7.4.3).
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::codebook_valid(input.imported_symbols),
    ensures match out {
        Ok(output) =>
            symbol_text_spec::bitmaps_valid(output.exported_symbols@)
                && output.consumed_bytes <= input.payload@.len(),
        Err(_) => true,
    },
))]
pub(crate) fn decode_symbol_dictionary(
    input: &SymbolDictionaryInput<'_>,
) -> Result<SymbolDictionaryOutput, SymbolTextError> {
    let limits = input.limits;
    let (header, header_len) = parse_symbol_dictionary_header(input.payload, limits)?;
    let config = header.config;

    let imported_count =
        u32::try_from(input.imported_symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let total_symbols = check_symbol_counts(
        imported_count,
        header.new_symbols,
        header.exported_symbols,
        limits,
    )?;
    let code_length = symbol_id_code_length(total_symbols)?;
    #[cfg(verus_keep_ghost)]
    production_segment_symbol_width!(total_symbols, code_length, input, header);

    let uses_generic_contexts = !config.huffman && !config.refinement_aggregation;
    if config.bitmap_context_used {
        let used = input
            .used_contexts
            .ok_or(SymbolTextError::InvalidState("missing retained contexts"))?;
        if uses_generic_contexts && used.generic.is_none() {
            return Err(SymbolTextError::InvalidState(
                "missing retained generic contexts",
            ));
        }
        if config.refinement_aggregation && used.refinement.is_none() {
            return Err(SymbolTextError::InvalidState(
                "missing retained refinement contexts",
            ));
        }
    } else if input.used_contexts.is_some() {
        return Err(SymbolTextError::InvalidState(
            "unexpected retained contexts without the context-used flag",
        ));
    }

    let body = input
        .payload
        .get(header_len..)
        .ok_or(MalformedError::IntegerOverflow)?;

    let refinement_adaptive: [(i8, i8); 2] = [
        (
            i8::try_from(header.refinement_adaptive[0].x)
                .map_err(|_| MalformedError::IntegerOverflow)?,
            i8::try_from(header.refinement_adaptive[0].y)
                .map_err(|_| MalformedError::IntegerOverflow)?,
        ),
        (
            i8::try_from(header.refinement_adaptive[1].x)
                .map_err(|_| MalformedError::IntegerOverflow)?,
            i8::try_from(header.refinement_adaptive[1].y)
                .map_err(|_| MalformedError::IntegerOverflow)?,
        ),
    ];

    let (exported_symbols, retained_generic, retained_refinement, consumed_within_body) =
        if config.huffman {
            let (exported, retained_refinement, consumed) = decode_symbol_dictionary_huffman(
                &header,
                input.imported_symbols,
                body,
                total_symbols,
                refinement_adaptive,
                input.custom_tables,
                limits,
            )?;
            (exported, None, retained_refinement, consumed)
        } else {
            if !input.custom_tables.is_empty() {
                return Err(SymbolTextError::InvalidTableAssociation(
                    "custom tables on an arithmetic symbol dictionary",
                ));
            }
            let (exported, retained_generic, retained_refinement, consumed) =
                decode_symbol_dictionary_arithmetic(
                    &header,
                    input.imported_symbols,
                    body,
                    code_length,
                    total_symbols,
                    refinement_adaptive,
                    input.used_contexts,
                    limits,
                )?;
            (exported, retained_generic, retained_refinement, consumed)
        };

    let retained_contexts = if config.bitmap_context_retained {
        Some(DictionaryContexts {
            generic: retained_generic,
            refinement: retained_refinement,
        })
    } else {
        None
    };

    let consumed_bytes = header_len
        .checked_add(consumed_within_body)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(SymbolDictionaryOutput {
        exported_symbols,
        retained_contexts,
        consumed_bytes,
    })
}

mod arithmetic;
mod exports;
mod huffman;

#[allow(unused_imports)]
pub(super) use arithmetic::{
    decode_dictionary_single_instance_refinement, decode_one_new_symbol,
    decode_symbol_dictionary_arithmetic,
};
pub(super) use exports::{assemble_exported_symbols, decode_export_runs};
#[allow(unused_imports)]
pub(super) use huffman::{decode_collective_bitmap, decode_symbol_dictionary_huffman};
