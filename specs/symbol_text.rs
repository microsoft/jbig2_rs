// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

#[path = "symbol_text/cycles.rs"]
pub(super) mod cycles;

#[path = "symbol_text/arithmetic_codec.rs"]
mod arithmetic_codec;
pub(super) use arithmetic_codec::{
    arithmetic_integer_transition, arithmetic_symbol_id_transition, integer_contexts_initial,
};

#[path = "symbol_text/huffman_codec.rs"]
mod huffman_codec;
pub(super) use huffman_codec::{
    huffman_codec_initial, huffman_error_result, huffman_error_result_definition,
    huffman_symbol_id_state, huffman_symbol_id_transition,
};

#[path = "symbol_text/symbol_ids.rs"]
mod symbol_ids;
pub(crate) use symbol_ids::{
    checked_symbol_decode_result, checked_symbol_id_result, pow2,
    symbol_code_length_expected, symbol_code_length_valid, symbol_code_length_bounds,
};
pub(super) use symbol_ids::{symbol_code_length_expected_bounds, symbol_code_length_least};
pub(super) use symbol_ids::{checked_symbol_id_transition, symbol_id_request_matches};
pub(super) use symbol_ids::segment_symbol_width_matches;

#[path = "symbol_text/dictionary.rs"]
mod dictionary;
pub(super) use dictionary::{
    bounded_arithmetic_source_tail, bounded_arithmetic_terminator_result,
    dictionary_context_models_match, dictionary_context_validation, normalized_arithmetic_bytes,
};
pub(crate) use dictionary::{dictionary_config_safe, dictionary_state_valid, symbol_count_result};

verus! {

// Model navigation: spec::text_end_matches_count [project-policy]
// Purpose: Final Huffman recovery is gated by complete count and a sub-byte endpoint.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn text_end_matches_count(
    end: super::TextRegionEnd, codec: &super::SymbolCodec,
    count: u32, refinement: bool,
) -> bool {
    match end {
        super::TextRegionEnd::RecoveredHuffman { instances, padding_start } =>
            instances == count && count > 0 && !refinement
            && match *codec {
                super::SymbolCodec::Huffman(h) =>
                    padding_start <= h.reader.bit_position
                    && h.reader.bit_position - padding_start < 8
                    && h.reader.bit_position == h.reader.bytes@.len() * 8,
                _ => false,
            },
        super::TextRegionEnd::RecoveredArithmetic { instances } =>
            instances == count && count > 0 && !refinement
            && match *codec {
                super::SymbolCodec::Arithmetic(a) => arithmetic_end_certificate(&a, count),
                _ => false,
            },
        _ => false,
    }
}

// Model navigation: spec::work_units [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn work_units(tracker: &super::ResourceTracker) -> u64 {
    tracker.work_units
}

// Model navigation: spec::bitmap_bytes [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmap_bytes(tracker: &super::ResourceTracker) -> u64 {
    tracker.decoded_bitmap_bytes
}

// Model navigation: spec::work_charge_effect [project-policy]
// Purpose: Resource charges/frames, requested dimensions, work and cursor arithmetic policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn work_charge_effect(
    before: &super::ResourceTracker,
    after: &super::ResourceTracker,
    units: u64,
    limits: &crate::Limits,
    accepted: bool,
) -> bool {
    let total = work_units(before) as nat + units as nat;
    bitmap_bytes(after) == bitmap_bytes(before)
        && work_units(after) == (if total <= u64::MAX {
            total as u64
        } else {
            work_units(before)
        })
        && accepted == (total <= u64::MAX && total <= limits.max_work_units)
}

// Model navigation: spec::bitmap_charge_effect [project-policy]
// Purpose: Resource charges/frames, requested dimensions, work and cursor arithmetic policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmap_charge_effect(
    before: &super::ResourceTracker,
    after: &super::ResourceTracker,
    width: u32,
    height: u32,
    limits: &crate::Limits,
    accepted: bool,
) -> bool {
    let dimensions_fit = width <= limits.max_width
        && height <= limits.max_height
        && width as nat * height as nat <= limits.max_pixels_per_page;
    let requested = ((width as nat + 7) / 8) * height as nat;
    let total = bitmap_bytes(before) as nat + requested;
    work_units(after) == work_units(before)
        && bitmap_bytes(after) == (if dimensions_fit && total <= u64::MAX {
            total as u64
        } else {
            bitmap_bytes(before)
        })
        && accepted == (
            dimensions_fit
                && total <= u64::MAX
                && total <= limits.max_cumulative_decoded_bitmap_bytes
        )
}

// Model navigation: spec::text_initialization_allowed [project-policy]
// Purpose: Resource charges/frames, requested dimensions, work and cursor arithmetic policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn text_initialization_allowed(
    config: super::TextRegionConfig,
    instances: u32,
    initial_strip: i64,
    limits: &crate::Limits,
) -> bool {
    let scaled = initial_strip as int * config.strip_size as int;
    instances as nat <= limits.max_work_units
        && i64::MIN <= scaled <= i64::MAX
        && i64::MIN <= -scaled <= i64::MAX
}

// Model navigation: spec::text_placement_allowed [project-policy]
// Purpose: Resource charges/frames, requested dimensions, work and cursor arithmetic policy.
// Audit: text-current-placement-acceptance
// REQUIREMENT CONTEXT: T88-TEXT-INSTANCE-COORDINATES | ITU-T T.88 (08/2018) | 6.4.5 step 3c; 6.4.7-6.4.9
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn text_placement_allowed(
    state: &super::TextRegionState,
    within_strip_t: u8,
    width: u32,
    height: u32,
    resources: &super::ResourceTracker,
    limits: &crate::Limits,
) -> bool {
    match state.strip {
        None => false,
        Some(strip) => {
            let extent = if state.config.transposed {
                height as int - 1
            } else {
                width as int - 1
            };
            let next_s = strip.cursor_s as int + extent;
            let symbol_t = state.strip_t as int + within_strip_t as int;
            let next_work = work_units(resources) as nat + 1;
            width > 0 && height > 0
                && within_strip_t < state.config.strip_size
                && state.decoded_instances < state.declared_instances
                && next_work <= u64::MAX
                && next_work <= limits.max_work_units
                && i64::MIN <= next_s <= i64::MAX
                && i64::MIN <= symbol_t <= i64::MAX
                && exists|placement: super::Placement|
                    placement_for_current(state, within_strip_t, width, height, placement)
        },
    }
}

// Model navigation: spec::placement_overlap [project-result-model]
// Purpose: Corner/transposition placement, clipping and prepared-content/resource frames.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn placement_overlap(
    destination: &crate::bitmap::Bitmap,
    source: &crate::bitmap::Bitmap,
    placement: super::Placement,
) -> (int, int, nat, nat) {
    let left = clipped_start(placement.x as int);
    let top = clipped_start(placement.y as int);
    let right = clipped_end(
        placement.x as int, source.width as nat, destination.width as nat,
    );
    let bottom = clipped_end(
        placement.y as int, source.height as nat, destination.height as nat,
    );
    (left, top, if right > left { (right - left) as nat } else { 0 },
        if bottom > top { (bottom - top) as nat } else { 0 })
}

// Model navigation: spec::placement_preflight_result [project-result-model]
// Purpose: Corner/transposition placement, clipping and prepared-content/resource frames.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn placement_preflight_result(
    destination: &crate::bitmap::Bitmap,
    source: &crate::bitmap::Bitmap,
    placement: super::Placement,
    before: &super::ResourceTracker,
    after: &super::ResourceTracker,
    limits: &crate::Limits,
    result: Result<Option<super::PreparedPlacement>, super::SymbolTextError>,
) -> bool {
    let (left, top, width, height) = placement_overlap(destination, source, placement);
    let work = width * height;
    if placement.x == i64::MIN || placement.y == i64::MIN || work > u64::MAX {
        result.is_err()
            && work_units(after) == work_units(before)
            && bitmap_bytes(after) == bitmap_bytes(before)
    } else {
        work_charge_effect(before, after, work as u64, limits, result.is_ok())
            && match result {
                Ok(Some(prepared)) =>
                    width > 0 && height > 0
                        && bitmap_contents_match(prepared.blit.source, source)
                        && prepared.blit.source_x as int == left - placement.x as int
                        && prepared.source_y as int == top - placement.y as int
                        && prepared.blit.destination_x as int == left
                        && prepared.destination_y as int == top
                        && prepared.blit.width as nat == width
                        && prepared.height as nat == height,
                Ok(None) => width == 0 || height == 0,
                Err(_) => true,
            }
    }
}

// Model navigation: spec::integer_contexts_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn integer_contexts_valid(
    contexts: &super::IntegerContexts,
) -> bool {
    forall|slot: int, index: int|
        0 <= slot < super::INTEGER_FIELD_COUNT
            && 0 <= index < 512
            ==> crate::arithmetic::arithmetic_spec::context_valid(
                contexts.slots[slot][index]
            )
}

// Model navigation: spec::arithmetic_contexts_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn arithmetic_contexts_valid(
    contexts: Seq<crate::arithmetic::ArithmeticContext>,
) -> bool {
    forall|index: int| 0 <= index < contexts.len()
        ==> crate::arithmetic::arithmetic_spec::context_valid(contexts[index])
}

// Model navigation: spec::arithmetic_codec_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn arithmetic_codec_valid(
    arithmetic: &super::ArithmeticCodec,
) -> bool {
    crate::arithmetic::arithmetic_spec::decoder_valid(&arithmetic.decoder)
        && integer_contexts_valid(&arithmetic.integers)
        && arithmetic_contexts_valid(arithmetic.iaid@)
        && prefix_valid(arithmetic)
}

pub(super) open spec fn prefix_valid(a: &super::ArithmeticCodec) -> bool {
    match a.prefix {
        Some(p) => crate::arithmetic::certainty_spec::observer_valid(&p, &a.decoder),
        None => true,
    }
}

pub(super) open spec fn arithmetic_end_certificate(a: &super::ArithmeticCodec, count: u32) -> bool {
    arithmetic_codec_valid(a)
    && match a.prefix {
        Some(p) => crate::arithmetic::certainty_spec::saved_prefix(&p, count)
            && match a.decoder.marker {
                Some(m) => m.offset == p.entropy_end && m.code == 0xac,
                None => false,
            },
        None => false,
    }
}

pub(super) open spec fn prefix_saved_unchanged(before: &super::ArithmeticCodec,
    after: &super::ArithmeticCodec) -> bool {
    match before.prefix {
        Some(p) => after.prefix.is_some() && after.prefix.unwrap().saved == p.saved,
        None => after.prefix.is_none(),
    }
}
// Model navigation: spec::integer_field_slot [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Audit: symbol-integer-field-slot
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn integer_field_slot(field: super::IntegerField) -> usize {
    field as usize
}

// Model navigation: spec::arithmetic_integer_result [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Audit: symbol-arithmetic-integer-result
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn arithmetic_integer_result(
    result: Result<crate::arithmetic::ArithmeticInteger, crate::arithmetic::ArithmeticError>,
) -> Result<super::EntropyInteger, super::SymbolTextError> {
    match result {
        Ok(crate::arithmetic::ArithmeticInteger::Value(value)) =>
            Ok(super::EntropyInteger::Value(value)),
        Ok(crate::arithmetic::ArithmeticInteger::OutOfBand) =>
            Ok(super::EntropyInteger::OutOfBand),
        Err(error) => Err(super::SymbolTextError::Decode(
            crate::arithmetic::arithmetic_spec::decode_error(error),
        )),
    }
}

// Model navigation: spec::arithmetic_symbol_id_result [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Audit: symbol-arithmetic-iaid-result
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS | ITU-T T.88 (08/2018) | A.1; A.2, context-separation paragraph (pp. 89-91)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn arithmetic_symbol_id_result(
    result: Result<u32, crate::arithmetic::ArithmeticError>,
) -> Result<u32, super::SymbolTextError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(super::SymbolTextError::Decode(
            crate::arithmetic::arithmetic_spec::decode_error(error),
        )),
    }
}

// Model navigation: spec::codec_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn codec_valid(codec: &super::SymbolCodec) -> bool {
    match codec {
        super::SymbolCodec::Arithmetic(arithmetic) =>
            arithmetic_codec_valid(&arithmetic),
        super::SymbolCodec::Huffman(huffman) => {
            crate::huffman::huffman_spec::reader_valid(&huffman.reader)
        },
    }
}

// Model navigation: spec::table_bindings_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn table_bindings_valid(
    bindings: &super::TableBindings,
) -> bool {
    bindings.len <= super::MAX_TABLE_BINDINGS
        && forall|index: int| 0 <= index < super::MAX_TABLE_BINDINGS ==>
            bindings.entries[index].is_some() == (index < bindings.len)
}

// Model navigation: spec::export_indices_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Audit: export-indices-valid
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-COUNT | ITU-T T.88 (08/2018) | 6.5.10; 7.4.2.1.4
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn export_indices_valid(
    exported: Seq<u32>,
    total_candidates: u32,
) -> bool {
    forall|index: int| 0 <= index < exported.len()
        ==> #[trigger] exported[index] < total_candidates
}

// Model navigation: spec::bitmaps_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Audit: symbol-bitmaps-valid
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmaps_valid(bitmaps: Seq<crate::bitmap::Bitmap>) -> bool {
    forall|index: int| 0 <= index < bitmaps.len()
        ==> crate::spec::bitmap::storage_valid(#[trigger] &bitmaps[index])
}

// Model navigation: spec::bitmap_refs_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmap_refs_valid(
    bitmaps: Seq<&crate::bitmap::Bitmap>,
) -> bool {
    forall|index: int| 0 <= index < bitmaps.len()
        ==> crate::spec::bitmap::storage_valid(#[trigger] bitmaps[index])
}

// Model navigation: spec::codebook_len [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Audit: project-symbol-codebook-length
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn codebook_len(codebook: super::SymbolCodebook) -> nat {
    match codebook {
        super::SymbolCodebook::Contiguous(symbols) => symbols@.len(),
        super::SymbolCodebook::Borrowed(symbols) => symbols@.len(),
    }
}

// Model navigation: spec::codebook_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn codebook_valid(codebook: super::SymbolCodebook) -> bool {
    match codebook {
        super::SymbolCodebook::Contiguous(symbols) => bitmaps_valid(symbols@),
        super::SymbolCodebook::Borrowed(symbols) => bitmap_refs_valid(symbols@),
    }
}

// Model navigation: spec::bitmap_contents_match [project-result-model]
// Purpose: Exact bitmap content, symbol selection and ordered repeated-export results.
// Audit: symbol-bitmap-contents-match
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmap_contents_match(
    actual: &crate::bitmap::Bitmap, expected: &crate::bitmap::Bitmap,
) -> bool {
    actual.width == expected.width
        && actual.height == expected.height
        && actual.stride == expected.stride
        && actual.data@ == expected.data@
}

// Model navigation: spec::codebook_selects [project-result-model]
// Purpose: Exact bitmap content, symbol selection and ordered repeated-export results.
// Audit: symbol-codebook-selects
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn codebook_selects(
    codebook: super::SymbolCodebook, index: nat, bitmap: &crate::bitmap::Bitmap,
) -> bool {
    index < codebook_len(codebook)
        && match codebook {
            super::SymbolCodebook::Contiguous(symbols) =>
                bitmap_contents_match(bitmap, &symbols@[index as int]),
            super::SymbolCodebook::Borrowed(symbols) =>
                bitmap_contents_match(bitmap, symbols@[index as int]),
        }
}

// Model navigation: spec::symbol_id_selects [project-result-model]
// Purpose: Exact bitmap content, symbol selection and ordered repeated-export results.
// Audit: symbol-id-selects
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn symbol_id_selects(
    imported: super::SymbolCodebook, new_symbols: Seq<crate::bitmap::Bitmap>,
    id: u32, bitmap: &crate::bitmap::Bitmap,
) -> bool {
    if (id as nat) < codebook_len(imported) {
        codebook_selects(imported, id as nat, bitmap)
    } else {
        let index = id as int - codebook_len(imported) as int;
        0 <= index < new_symbols.len()
            && bitmap_contents_match(bitmap, &new_symbols[index])
    }
}

// Model navigation: spec::exported_contents_match [project-result-model]
// Purpose: Exact bitmap content, symbol selection and ordered repeated-export results.
// Audit: symbol-exported-contents-match
// REQUIREMENT CONTEXT: T88-DICTIONARY-EXPORT-ORDER | ITU-T T.88 (08/2018) | 6.5.10 step 8
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn exported_contents_match(
    imported: super::SymbolCodebook, new_symbols: Seq<crate::bitmap::Bitmap>,
    indices: Seq<u32>, exported: Seq<crate::bitmap::Bitmap>,
) -> bool {
    exported.len() == indices.len()
        && forall|index: int| 0 <= index < indices.len() ==>
            #[trigger] symbol_id_selects(imported, new_symbols, indices[index], &exported[index])
}

// Model navigation: spec::two_bit_choice_result [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn two_bit_choice_result(
    encoded: u8,
    first: crate::huffman::StandardHuffmanTable,
    second: crate::huffman::StandardHuffmanTable,
    choice: super::TableChoice,
) -> bool {
    match encoded {
        0 => choice == super::TableChoice::Standard(first),
        1 => choice == super::TableChoice::Standard(second),
        3 => choice == super::TableChoice::Custom,
        _ => false,
    }
}

// Model navigation: spec::four_way_choice_result [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn four_way_choice_result(
    encoded: u8,
    first: crate::huffman::StandardHuffmanTable,
    second: crate::huffman::StandardHuffmanTable,
    third: crate::huffman::StandardHuffmanTable,
    choice: super::TableChoice,
) -> bool {
    match encoded {
        0 => choice == super::TableChoice::Standard(first),
        1 => choice == super::TableChoice::Standard(second),
        2 => choice == super::TableChoice::Standard(third),
        _ => choice == super::TableChoice::Custom,
    }
}

// Model navigation: spec::text_config_safe [candidate-standard-facing-model]
// Purpose: Selected dictionary/text flags, strip/refinement choices and OOB candidates.
// Audit: text-config-safe
// REQUIREMENT CONTEXT: T88-TEXT-CODING-FLAGS | ITU-T T.88 (08/2018) | 7.4.3.1.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn text_config_safe(
    flags: u16,
    huffman_flags: Option<u16>,
    colour_extension: bool,
    config: super::TextRegionConfig,
) -> bool {
    !colour_extension
        && config.huffman == (flags & 1 != 0)
        && config.refinement == (flags & 2 != 0)
        && (config.huffman ==> huffman_flags.is_some())
        && (!config.huffman ==> huffman_flags.is_none())
        && valid_strip_size(config.strip_size)
}

// Model navigation: spec::text_huffman_choices_safe [candidate-standard-facing-model]
// Purpose: Selected dictionary/text flags, strip/refinement choices and OOB candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn text_huffman_choices_safe(
    flags: u16,
    refinement: bool,
    choices: super::TextHuffmanChoices,
) -> bool {
    flags & 0x8000 == 0
        && (!refinement ==> flags & 0x7fc0 == 0)
        && (flags & 0x4000 == 0 ==> (
            choices.refinement_size
                == super::TableChoice::Standard(
                    crate::huffman::StandardHuffmanTable::A
                )
        ))
        && (flags & 0x4000 != 0 ==> (
            choices.refinement_size == super::TableChoice::Custom
        ))
}

// Model navigation: spec::refinement_choice_result [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_choice_result(
    encoded: u8,
    choice: super::TableChoice,
) -> bool {
    match encoded {
        0 => choice == super::TableChoice::Standard(
            crate::huffman::StandardHuffmanTable::N
        ),
        1 => choice == super::TableChoice::Standard(
            crate::huffman::StandardHuffmanTable::O
        ),
        3 => choice == super::TableChoice::Custom,
        _ => false,
    }
}

// Model navigation: spec::floor_half [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Audit: text-floor-half
// REQUIREMENT CONTEXT: T88-AGGREGATION-TEXT-REFINEMENT-GEOMETRY | ITU-T T.88 (08/2018) | 6.5.8.2 step 2; 6.4.11 step 6; Table 12
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn floor_half(value: int) -> int {
    value / 2
}

// Model navigation: spec::signed_byte [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn signed_byte(value: u8) -> int {
    if value <= 127 { value as int } else { value as int - 256 }
}

// Model navigation: spec::text_state_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn text_state_valid(state: &super::TextRegionState) -> bool {
    state.decoded_instances <= state.declared_instances
}

// Model navigation: spec::placement_within_extents [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn placement_within_extents(
    placement: super::Placement,
    width: u32,
    height: u32,
) -> bool {
    width > 0
        && height > 0
        && placement.x <= placement.x + width as int - 1
        && placement.y <= placement.y + height as int - 1
}

// Model navigation: spec::prepared_placement_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn prepared_placement_valid(
    prepared: super::PreparedPlacement,
    destination: &crate::bitmap::Bitmap,
) -> bool {
    crate::spec::bitmap::storage_valid(prepared.blit.source)
        && prepared.blit.source_x as nat + prepared.blit.width as nat
            <= prepared.blit.source.width as nat
        && prepared.source_y as nat + prepared.height as nat
            <= prepared.blit.source.height as nat
        && prepared.blit.destination_x as nat + prepared.blit.width as nat
            <= destination.width as nat
        && prepared.destination_y as nat + prepared.height as nat
            <= destination.height as nat
}

// Model navigation: spec::placement_batch_valid [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn placement_batch_valid(
    batch: &super::PlacementBatch,
    destination: &crate::bitmap::Bitmap,
) -> bool {
    batch.blits@.len() <= super::MAX_PLACEMENT_BATCH
        && forall|index: int| 0 <= index < batch.blits@.len() ==> {
            let blit = #[trigger] batch.blits@[index];
            crate::spec::bitmap::storage_valid(blit.source)
                && blit.source_x as nat + blit.width as nat
                    <= blit.source.width as nat
                && batch.source_y as nat + batch.height as nat
                    <= blit.source.height as nat
                && blit.destination_x as nat + blit.width as nat
                    <= destination.width as nat
                && batch.destination_y as nat + batch.height as nat
                    <= destination.height as nat
        }
}

// Model navigation: spec::placement_for_current [project-result-model]
// Purpose: Corner/transposition placement, clipping and prepared-content/resource frames.
// Audit: text-current-placement
// REQUIREMENT CONTEXT: T88-TEXT-INSTANCE-COORDINATES | ITU-T T.88 (08/2018) | 6.4.5 step 3c; 6.4.7-6.4.9
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn placement_for_current(
    state: &super::TextRegionState,
    within_strip_t: u8,
    width: u32,
    height: u32,
    placement: super::Placement,
) -> bool {
    match state.strip {
        None => false,
        Some(strip) => {
            let width_extent = width as int - 1;
            let height_extent = height as int - 1;
            let advances_before =
                (!state.config.transposed
                    && (state.config.reference_corner
                        == super::ReferenceCorner::TopRight
                        || state.config.reference_corner
                            == super::ReferenceCorner::BottomRight))
                || (state.config.transposed
                    && (state.config.reference_corner
                        == super::ReferenceCorner::BottomLeft
                        || state.config.reference_corner
                            == super::ReferenceCorner::BottomRight));
            let extent = if state.config.transposed {
                height_extent
            } else {
                width_extent
            };
            let symbol_s = strip.cursor_s as int
                + if advances_before { extent } else { 0 };
            let symbol_t = state.strip_t as int + within_strip_t as int;
            let reference_x = if state.config.transposed { symbol_t } else { symbol_s };
            let reference_y = if state.config.transposed { symbol_s } else { symbol_t };
            placement.x as int == if state.config.reference_corner
                    == super::ReferenceCorner::TopRight
                    || state.config.reference_corner
                        == super::ReferenceCorner::BottomRight
                { reference_x - width_extent } else { reference_x }
                && placement.y as int == if state.config.reference_corner
                    == super::ReferenceCorner::BottomLeft
                    || state.config.reference_corner
                        == super::ReferenceCorner::BottomRight
                { reference_y - height_extent } else { reference_y }
        }
    }
}

// Model navigation: spec::generic_template_matches [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn generic_template_matches(
    value: u8,
    template: crate::generic_region::GenericTemplate,
) -> bool {
    match value {
        0 => template == crate::generic_region::GenericTemplate::Template0,
        1 => template == crate::generic_region::GenericTemplate::Template1,
        2 => template == crate::generic_region::GenericTemplate::Template2,
        _ => template == crate::generic_region::GenericTemplate::Template3,
    }
}

// Model navigation: spec::refinement_template_matches [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_template_matches(
    value: u8,
    template: crate::generic_region::RefinementTemplate,
) -> bool {
    if value == 0 {
        template == crate::generic_region::RefinementTemplate::Template0
    } else {
        template == crate::generic_region::RefinementTemplate::Template1
    }
}

// Model navigation: spec::valid_strip_size [representation-invariant]
// Purpose: Codec banks/bitmaps, table slots, export counters and placement-bound invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn valid_strip_size(size: u8) -> bool {
    size == 1 || size == 2 || size == 4 || size == 8
}

// Model navigation: spec::strip_log_valid [project-result-model]
// Purpose: Arithmetic codec result/error translation and encoded choice/length results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn strip_log_valid(size: u8, log: u8) -> bool {
    (size == 1 && log == 0)
        || (size == 2 && log == 1)
        || (size == 4 && log == 2)
        || (size == 8 && log == 3)
}

// Model navigation: spec::export_progress_measure [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn export_progress_measure(
    index: nat,
    work: nat,
    total: nat,
    work_limit: nat,
) -> int
    recommends index <= total, work <= work_limit,
{
    (work_limit - work) * (total + 1) + (total - index)
}

// Model navigation: spec::custom_oob_compatible [candidate-standard-facing-model]
// Purpose: Selected dictionary/text flags, strip/refinement choices and OOB candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn custom_oob_compatible(
    requires_oob: bool,
    supplied_oob: bool,
) -> bool {
    requires_oob == supplied_oob
}

// Model navigation: spec::clipped_start [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn clipped_start(origin: int) -> int {
    if origin < 0 { 0 } else { origin }
}

// Model navigation: spec::clipped_end [proof-support]
// Purpose: Private codec/bank projections, signed/clipped vocabulary and export measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn clipped_end(origin: int, extent: nat, bound: nat) -> int {
    let far = origin + extent as int;
    if far < bound as int { far } else { bound as int }
}

}
