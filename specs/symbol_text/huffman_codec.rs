// Source-only author navigation, not normative approval.
// Guide: ../README.md; audit ledger: ../../validation/traceability/requirements.json

use super::*;
use crate::symbol_text::{
    EntropyInteger, HuffmanCodec, HuffmanFieldTables, IntegerField, MaterializedHuffmanTable,
    StandardHuffmanTable, StandardHuffmanTables, SymbolTextError,
};
use crate::huffman::HuffmanTable;

verus! {

// Model navigation: spec::optional_table_contents [proof-support]
// Purpose: Compare a borrowed lookup result with complete table contents, not pointer identity.
// Audit: symbol-optional-table-contents
// REQUIREMENT CONTEXT: T88-HUFFMAN-STANDARD-TABLES | ITU-T T.88 (08/2018) | B.5; Tables B.1-B.15
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn optional_table_contents(
    table: Option<&HuffmanTable>,
) -> Option<HuffmanTable> {
    match table {
        Some(table) => Some(*table),
        None => None,
    }
}

// Model navigation: spec::selected_standard_table [project-result-model]
// Purpose: Exact cache-slot contents, including absent slots, for each supplied standard tag.
// Audit: symbol-selected-standard-table
// REQUIREMENT CONTEXT: T88-HUFFMAN-STANDARD-TABLES | ITU-T T.88 (08/2018) | B.5; Tables B.1-B.15
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn selected_standard_table(
    tables: &StandardHuffmanTables,
    table: StandardHuffmanTable,
) -> Option<HuffmanTable> {
    let selected = match table {
        StandardHuffmanTable::A => tables.a,
        StandardHuffmanTable::B => tables.b,
        StandardHuffmanTable::C => tables.c,
        StandardHuffmanTable::D => tables.d,
        StandardHuffmanTable::E => tables.e,
        StandardHuffmanTable::F => tables.f,
        StandardHuffmanTable::G => tables.g,
        StandardHuffmanTable::H => tables.h,
        StandardHuffmanTable::I => tables.i,
        StandardHuffmanTable::J => tables.j,
        StandardHuffmanTable::K => tables.k,
        StandardHuffmanTable::L => tables.l,
        StandardHuffmanTable::M => tables.m,
        StandardHuffmanTable::N => tables.n,
        StandardHuffmanTable::O => tables.o,
    };
    match selected {
        Some(table) => Some(*table),
        None => None,
    }
}

// Model navigation: spec::selected_field_table [project-result-model]
// Purpose: Resolve the supplied field's own custom or standard binding; literal fields have none.
// Audit: symbol-selected-field-table
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn selected_field_table(
    tables: &HuffmanFieldTables,
    field: IntegerField,
) -> Option<HuffmanTable> {
    let selected = match field {
        IntegerField::AggregationInstances => tables.aggregation_instances,
        IntegerField::HeightDelta => tables.height_delta,
        IntegerField::WidthDelta => tables.width_delta,
        IntegerField::ExportRun => tables.export_run,
        IntegerField::StripDelta => tables.strip_delta,
        IntegerField::FirstS => tables.first_s,
        IntegerField::SubsequentS => tables.subsequent_s,
        IntegerField::RefinementWidth => tables.refinement_width,
        IntegerField::RefinementHeight => tables.refinement_height,
        IntegerField::RefinementX => tables.refinement_x,
        IntegerField::RefinementY => tables.refinement_y,
        IntegerField::RefinementSize => tables.refinement_size,
        IntegerField::DictionaryBitmapSize => tables.dictionary_bitmap_size,
        IntegerField::WithinStripT | IntegerField::RefinementIndicator => None,
    };
    match selected {
        Some(MaterializedHuffmanTable::Standard(table)) =>
            selected_standard_table(&tables.standard_tables, table),
        Some(MaterializedHuffmanTable::Custom(table)) => Some(*table),
        None => None,
    }
}

// Model navigation: spec::huffman_table_integer_state [project-result-model]
// Purpose: Exact selected-table integer/OOB/error result and cursor, including missing bindings.
// Audit: symbol-huffman-table-integer-state
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_table_integer_state(
    codec: &HuffmanCodec,
    field: IntegerField,
) -> (Result<EntropyInteger, SymbolTextError>, usize)
    recommends crate::huffman::huffman_spec::reader_valid(&codec.reader),
{
    match selected_field_table(&codec.tables, field) {
        Some(table) => {
            let decoded = crate::huffman::huffman_spec::decode_state(
                &table, codec.reader.bytes@, codec.reader.bit_position,
            );
            let result = match decoded.0 {
                Ok(crate::huffman::HuffmanValue::Integer(value)) =>
                    Ok(EntropyInteger::Value(value as i64)),
                Ok(crate::huffman::HuffmanValue::OutOfBand) => Ok(EntropyInteger::OutOfBand),
                Err(error) => Err(huffman_error_result(&error)),
            };
            (result, decoded.1)
        },
        None => (
            Err(SymbolTextError::InvalidTableAssociation("no Huffman table bound for field")),
            codec.reader.bit_position,
        ),
    }
}

// Model navigation: spec::huffman_integer_transition [project-result-model]
// Purpose: Preserve field-specific table results/cursors and complete metadata through enum dispatch.
// Audit: symbol-huffman-integer-transition
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_integer_transition<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after: &crate::symbol_text::SymbolCodec<'a>,
    field: IntegerField,
    result: Result<EntropyInteger, SymbolTextError>,
) -> bool
    recommends codec_valid(before),
{
    match *before {
        crate::symbol_text::SymbolCodec::Huffman(prior) => match *after {
            crate::symbol_text::SymbolCodec::Huffman(next) => {
                &&& field != IntegerField::WithinStripT
                    && field != IntegerField::RefinementIndicator ==> {
                        let decoded = huffman_table_integer_state(&prior, field);
                        &&& result == decoded.0
                        &&& next.reader.bit_position == decoded.1
                    }
                &&& next.reader.bytes@ == prior.reader.bytes@
                &&& crate::huffman::huffman_spec::reader_valid(&next.reader)
                &&& next.tables == prior.tables
                &&& next.log_strip_size == prior.log_strip_size
                &&& next.symbol_id_table == prior.symbol_id_table
                &&& next.refinement == prior.refinement
                &&& next.refinement_work == prior.refinement_work
            },
            _ => false,
        },
        _ => true,
    }
}

// Model navigation: spec::huffman_codec_initial [project-result-model]
// Purpose: Fresh Huffman cursor, literal input/strip setup and requested refinement metadata.
// Audit: symbol-huffman-codec-initial
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_codec_initial(
    codec: &crate::symbol_text::HuffmanCodec,
    bytes: Seq<u8>,
    log_strip_size: u8,
    refinement_template: Option<crate::generic_region::RefinementTemplate>,
) -> bool {
    &&& codec.reader.bytes@ == bytes
    &&& codec.reader.bit_position == 0
    &&& codec.log_strip_size == log_strip_size
    &&& codec.symbol_id_table.is_none()
    &&& codec.refinement_work == 0
    &&& match refinement_template {
        Some(template) => match &codec.refinement {
            Some(contexts) => contexts.template == template
                && crate::generic_region::generic_region_spec::refinement_contexts_valid(contexts),
            None => false,
        },
        None => codec.refinement.is_none(),
    }
}

// Model navigation: spec::huffman_error_result [project-result-model]
// Purpose: Reuse the existing symbol/text wrapper around the exact Huffman conversion.
// Audit: symbol-huffman-error-result
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn huffman_error_result(
    error: &crate::huffman::HuffmanError,
) -> crate::symbol_text::SymbolTextError {
    crate::symbol_text::SymbolTextError::Decode(
        crate::huffman::huffman_spec::huffman_error_result(error),
    )
}

pub(in crate::symbol_text) proof fn huffman_error_result_definition(
    error: &crate::huffman::HuffmanError,
)
    ensures huffman_error_result(error) == crate::symbol_text::SymbolTextError::Decode(
        crate::huffman::huffman_spec::huffman_error_result(error),
    ),
{
    reveal(huffman_error_result);
}

// Model navigation: spec::huffman_symbol_id_result [project-result-model]
// Purpose: Exact signed-value, OOB and Huffman-error projection into symbol IDs.
// Audit: symbol-huffman-id-result
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_symbol_id_result(
    decoded: Result<crate::huffman::HuffmanValue, crate::huffman::HuffmanError>,
) -> Result<u32, crate::symbol_text::SymbolTextError> {
    match decoded {
        Ok(crate::huffman::HuffmanValue::Integer(value)) =>
            if value < 0 {
                Err(crate::symbol_text::SymbolTextError::InvalidEntropyValue("symbol ID"))
            } else {
                Ok(value as u32)
            },
        Ok(crate::huffman::HuffmanValue::OutOfBand) =>
            Err(crate::symbol_text::SymbolTextError::UnexpectedOutOfBand("symbol ID")),
        Err(error) => Err(huffman_error_result(&error)),
    }
}

// Model navigation: spec::huffman_symbol_id_state [project-result-model]
// Purpose: Exact table-backed or literal decoded ID/error and consumed reader position.
// Audit: symbol-huffman-id-state
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_symbol_id_state(
    codec: &crate::symbol_text::HuffmanCodec,
    request: crate::symbol_text::SymbolIdRequest,
) -> (Result<u32, crate::symbol_text::SymbolTextError>, usize)
    recommends crate::huffman::huffman_spec::reader_valid(&codec.reader),
{
    match &codec.symbol_id_table {
        Some(table) => {
            let decoded = crate::huffman::huffman_spec::decode_state(
                table, codec.reader.bytes@, codec.reader.bit_position,
            );
            (huffman_symbol_id_result(decoded.0), decoded.1)
        },
        None => {
            let error = crate::spec::readers::bit_read_error(
                codec.reader.bytes@, codec.reader.bit_position, request.code_length,
            );
            let result = match error {
                Some(error) => Err(crate::symbol_text::SymbolTextError::Decode(
                    crate::DecodeError::Malformed(error),
                )),
                None => Ok(crate::spec::readers::bits_value(
                    codec.reader.bytes@, codec.reader.bit_position as nat,
                    request.code_length as nat,
                ) as u32),
            };
            (result, (codec.reader.bit_position as nat
                + if error.is_none() { request.code_length as nat } else { 0nat }) as usize)
        },
    }
}

// Model navigation: spec::huffman_symbol_id_transition [project-result-model]
// Purpose: Preserve complete Huffman ID result/cursor and untouched codec metadata through enum dispatch.
// Audit: symbol-huffman-id-transition
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn huffman_symbol_id_transition<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after: &crate::symbol_text::SymbolCodec<'a>,
    request: crate::symbol_text::SymbolIdRequest,
    result: Result<u32, crate::symbol_text::SymbolTextError>,
) -> bool
    recommends codec_valid(before),
{
    match *before {
        crate::symbol_text::SymbolCodec::Huffman(prior) => match *after {
            crate::symbol_text::SymbolCodec::Huffman(next) => {
                let decoded = huffman_symbol_id_state(&prior, request);
                &&& result == decoded.0
                &&& next.reader.bit_position == decoded.1
                &&& next.reader.bytes@ == prior.reader.bytes@
                &&& crate::huffman::huffman_spec::reader_valid(&next.reader)
                &&& next.tables == prior.tables
                &&& next.log_strip_size == prior.log_strip_size
                &&& next.symbol_id_table == prior.symbol_id_table
                &&& next.refinement == prior.refinement
                &&& next.refinement_work == prior.refinement_work
            },
            _ => false,
        },
        _ => true,
    }
}

}
