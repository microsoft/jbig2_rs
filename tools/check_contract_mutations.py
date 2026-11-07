#!/usr/bin/env python3
"""Check that critical API and diagnostic regressions detect isolated mutations."""

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from check_distribution import isolated_environment, run
from check_arithmetic_codec_proofs import CASES as ENTROPY_CODEC_CASES
from check_context_lifecycle_proofs import CASES as CONTEXT_LIFECYCLE_CASES
from check_session_framing_proofs import CASES as SESSION_FRAMING_CASES
from check_clone_result_proofs import CASES as CLONE_RESULT_CASES
from check_public_context_proofs import CASES as PUBLIC_CONTEXT_CASES
from check_public_result_proofs import CASES as PUBLIC_RESULT_CASES
from check_reference_catalog_proofs import CASES as REFERENCE_CATALOG_CASES
from check_reference_type_proofs import CASES as REFERENCE_TYPE_CASES
from check_document_count_proof import CASES as DOCUMENT_CASES
from check_huffman_assignment_proofs import CASES as HUFFMAN_ASSIGNMENT_CASES
from check_iaid_state_proofs import CASES as IAID_STATE_CASES
from check_value_bridge_proofs import CASES as VALUE_BRIDGE_CASES
from check_page_state_proofs import CASES as PAGE_STATE_CASES
from check_segment_data_proofs import CASES as SEGMENT_DATA_CASES
from check_segment_header_proofs import CASES as HEADER_CASES
from check_symbol_cardinality_proofs import CASES as SYMBOL_CARDINALITY_CASES
from check_text_batch_proofs import CASES as TEXT_CASES
from proof_sensitivity import replace_once


ROOT = Path(__file__).resolve().parents[1]
SEGMENT_DATA_TESTS = {
    "page_width_discarded": "segment_data::tests::raw_page_information_preserves_every_literal_field_and_height_encoding",
    "known_page_height_discarded": "segment_data::tests::raw_page_information_preserves_every_literal_field_and_height_encoding",
    "page_resolutions_swapped": "segment_data::tests::raw_page_information_preserves_every_literal_field_and_height_encoding",
    "profile_wire_values_discarded": "segment_data::tests::raw_profile_lists_preserve_literal_values_prefix_extent_and_suffix",
    "profile_existing_prefix_corrupted": "segment_data::tests::raw_profile_lists_preserve_literal_values_prefix_extent_and_suffix",
    "page_dispatch_returns_wrong_variant": "document::tests::payload_windows::raw_known_segment_dispatch_preserves_page_stripe_profile_variants_and_values",
    "profile_dispatch_returns_wrong_variant": "document::tests::payload_windows::raw_known_segment_dispatch_preserves_page_stripe_profile_variants_and_values",
    "stripe_dispatch_corrupts_value": "document::tests::payload_windows::raw_known_segment_dispatch_preserves_page_stripe_profile_variants_and_values",
    "parse_sequential_discards_typed_page_metadata": "document::tests::payload_windows::raw_page_metadata_survives_sequential_random_and_embedded_container_composition",
    "parse_random_access_discards_typed_page_metadata": "document::tests::payload_windows::raw_page_metadata_survives_sequential_random_and_embedded_container_composition",
}
TEXT_RUNTIME_TESTS = {
    "rejected_work_charge_is_silently_rolled_back": "resource_accounting::work_charges_match_wide_results_and_attempted_charge_failure_state",
    "rejected_bitmap_charge_is_silently_rolled_back": "resource_accounting::bitmap_charges_match_wide_priority_and_attempted_charge_failure_state",
    "valid_empty_dimensions_are_rejected": "resource_accounting::bitmap_charges_match_wide_priority_and_attempted_charge_failure_state",
    "preflight_selects_wrong_in_bounds_source_column": "placement_state::placement_preflight_matches_wide_geometry_and_resource_failure_frames",
    "preflight_rejects_valid_origin": "placement_state::placement_preflight_matches_wide_geometry_and_resource_failure_frames",
    "initialization_rejects_valid_zero_instances": "placement_state::text_state_initialization_matches_wide_checked_scaling_and_work_priority",
    "current_placement_rejects_valid_single_pixel": "placement_state::current_text_placement_matches_wide_coordinates_and_partial_failure_state",
}
DOCUMENT_WINDOW_TESTS = {
    "random_access_discards_consumed_segments": "raw_random_access_layout_matches_separate_literal_headers_and_payloads",
    "random_access_rebinds_payload_after_validation": "raw_random_access_layout_matches_separate_literal_headers_and_payloads",
    "random_access_corrupts_header_after_validation": "raw_random_access_layout_matches_separate_literal_headers_and_payloads",
    "random_access_drops_eof_after_validation": "raw_random_access_layout_matches_separate_literal_headers_and_payloads",
    "standalone_random_access_discards_first_body_byte": "standalone_random_access_origins_order_eof_and_trailing_bytes_match_raw_metadata",
    "known_payload_returns_wrong_bytes": "raw_payload_windows_keep_exact_known_and_existing_unknown_extents",
    "unknown_payload_returns_wrong_bytes": "raw_payload_windows_keep_exact_known_and_existing_unknown_extents",
    "sequential_discards_consumed_segments": "raw_sequential_and_embedded_layouts_match_literal_windows_and_order_policy",
    "sequential_rebinds_payload_after_validation": "raw_sequential_and_embedded_layouts_match_literal_windows_and_order_policy",
    "sequential_corrupts_header_after_validation": "raw_sequential_and_embedded_layouts_match_literal_windows_and_order_policy",
    "standalone_disables_sequential_number_order": "standalone_sequential_body_origin_is_exact_and_unordered_headers_are_rejected",
    "embedded_discards_first_input_byte": "raw_sequential_and_embedded_layouts_match_literal_windows_and_order_policy",
}
MUTATIONS = tuple(
    {
        "id": "reference-type-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in REFERENCE_TYPE_CASES
) + tuple(
    {
        "id": "reference-catalog-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in REFERENCE_CATALOG_CASES
) + tuple(
    {
        "id": "public-results-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in PUBLIC_RESULT_CASES
) + tuple(
    {
        "id": "public-context-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in PUBLIC_CONTEXT_CASES
) + tuple(
    {
        "id": "clone-result-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in CLONE_RESULT_CASES
) + tuple(
    {
        "id": "session-framing-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in SESSION_FRAMING_CASES
) + tuple(
    {
        "id": "context-lifecycle-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in CONTEXT_LIFECYCLE_CASES
) + tuple(
    {
        "id": "caller-request-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in SYMBOL_CARDINALITY_CASES if "native_test" in case
) + tuple(
    {
        "id": "checked-id-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in VALUE_BRIDGE_CASES if "native_test" in case
) + tuple(
    {
        "id": "iaid-range-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in IAID_STATE_CASES if "native_test" in case
) + tuple(
    {
        "id": case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["native_test"], "whitespace_flexible": True,
    }
    for case in ENTROPY_CODEC_CASES if "native_test" in case
) + tuple(
    {
        "id": "symbol-cardinality-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib",
        "test": "symbol_text::tests::codec_and_exports::export_runs_match_every_small_selection_with_exact_work_and_input_frames",
        "whitespace_flexible": True,
    }
    for case in SYMBOL_CARDINALITY_CASES if case["function"] == "decode_export_runs"
) + tuple(
    {
        "id": "huffman-assignment-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib",
        "test": case["native_test"] if "native_test" in case else "huffman::tests::table_validation::" + (
            "wide_zero_constructor_matches_default_and_array_initialization"
            if case["function"] == "WideCode::zero"
            else "raw_canonical_assignment_matches_numeric_histogram_and_stable_order"
        ),
        "whitespace_flexible": True,
    }
    for case in HUFFMAN_ASSIGNMENT_CASES
) + tuple(
    {
        "id": "page-state-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": case["test"], "whitespace_flexible": True,
    }
    for case in PAGE_STATE_CASES
) + tuple(
    {
        "id": "segment-data-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": SEGMENT_DATA_TESTS[case["id"]],
        "whitespace_flexible": True,
    }
    for case in SEGMENT_DATA_CASES
) + tuple(
    {
        "id": "text-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": "symbol_text::tests::" + TEXT_RUNTIME_TESTS[case["id"]],
        "whitespace_flexible": True,
    }
    for case in TEXT_CASES if case["id"] in TEXT_RUNTIME_TESTS
) + tuple(
    {
        "id": "header-" + case["id"],
        "file": case["file"], "before": case["site"], "after": case["replacement"],
        "target": "lib",
        "test": "segment_header::tests::complete_headers_at_nonzero_cursors_match_literal_fields_and_exact_extent",
        "whitespace_flexible": True,
    }
    for case in HEADER_CASES
) + tuple(
    {
        "id": "document-window-" + case["id"],
        "file": "src/document.rs", "before": case["site"], "after": case["replacement"],
        "target": "lib", "test": "document::tests::payload_windows::" + DOCUMENT_WINDOW_TESTS[case["id"]],
        "whitespace_flexible": True,
    }
    for case in DOCUMENT_CASES if case["id"] in DOCUMENT_WINDOW_TESTS
) + (
    {
        "id": "refinement-snapshot-template-policy-inverted",
        "file": "src/generic_region.rs",
        "before": (
            "if self.template != snapshot.template {\n"
            "            return Err(GenericRegionError::InvalidFlags(refinement_template_code("
        ),
        "after": (
            "if self.template == snapshot.template {\n"
            "            return Err(GenericRegionError::InvalidFlags(refinement_template_code("
        ),
        "target": "lib",
        "test": "generic_region::tests::sessions_and_snapshots::raw_refinement_snapshots_preserve_all_states_and_failure_frames",
    },
    {
        "id": "generic-template-one-history-bit-discarded",
        "file": "src/generic_region.rs",
        "before": "(((top << 5) | previous) << 4) | (adaptive << 3) | history & 0x07",
        "after": "(((top << 5) | previous) << 4) | (adaptive << 3) | history & 0x03",
        "target": "lib",
        "test": "generic_region::tests::preflight_and_templates::raw_context_descriptors_match_literal_sampling_and_history_masks",
    },
    {
        "id": "halftone-memory-uses-smaller-stage-peak",
        "file": "src/halftone.rs",
        "before": "let peak = if gray_peak >= render_peak {",
        "after": "let peak = if gray_peak <= render_peak {",
        "target": "lib",
        "test": "halftone::tests::geometry_and_render::raw_halftone_memory_peaks_match_wide_stage_arithmetic",
    },
    {
        "id": "collective-slicing-omits-pattern-vector-charge",
        "file": "src/halftone.rs",
        "before": "checked_memory_sum(collective_bytes, retained_pattern_bytes, vector_bytes, 0)",
        "after": "checked_memory_sum(collective_bytes, retained_pattern_bytes, 0, 0)",
        "target": "lib",
        "test": "halftone::tests::geometry_and_render::raw_collective_slicing_matches_literal_pixels_and_exact_budget_boundaries",
    },
    {
        "id": "page-collector-rejects-inclusive-decoded-limit",
        "file": "src/decode.rs",
        "before": "if next_bytes > self.decoded_limit {",
        "after": "if next_bytes >= self.decoded_limit {",
        "target": "lib",
        "test": "decode::tests::dispatch_and_resources::raw_page_collector_guards_preserve_order_pixels_and_rejected_totals",
    },
    {
        "id": "wide-code-final-carry-capacity-inverted",
        "file": "src/huffman.rs",
        "before": "if self.limbs[3] != u64::MAX {",
        "after": "if self.limbs[3] == u64::MAX {",
        "target": "lib",
        "test": "huffman::tests::wide_arithmetic::raw_wide_code_arithmetic_matches_independent_bit_serial_oracles",
    },
    {
        "id": "structural-work-drops-reference-count",
        "file": "src/document.rs",
        "before": "let Some(actual) = partial.checked_add(reference_work) else {",
        "after": "let Some(actual) = partial.checked_add(0) else {",
        "target": "lib",
        "test": "document::tests::payload_windows::raw_structural_work_matches_wide_counts_and_exact_rejection_priority",
    },
    {
        "id": "direct-dispatch-ignores-sealed-stripe",
        "file": "src/decode.rs",
        "before": "|| page.last_stripe_end.is_some()\n        || page.open_stripe_max_row.is_some()",
        "after": "|| false && page.last_stripe_end.is_some()\n        || page.open_stripe_max_row.is_some()",
        "target": "lib",
        "test": "decode::tests::dispatch_and_resources::raw_header_only_direct_dispatch_checks_every_page_gate_without_side_effects",
    },
    {
        "id": "table-reference-gathering-discards-earlier-results",
        "file": "src/decode.rs",
        "before": "result.push(table);",
        "after": "result.clear();\n            result.push(table);",
        "target": "lib",
        "test": "decode::tests::dispatch_and_resources::raw_referred_tables_preserve_order_repeats_missing_keys_and_retention",
    },
    {
        "id": "dictionary-reserved-bit-thirteen-accepted",
        "file": "src/symbol_text.rs",
        "before": (
            "if flags & 0xe000 != 0 {\n"
            "        return Err(SymbolTextError::InvalidFlags(\n"
            "            \"reserved symbol-dictionary flags\","
        ),
        "after": (
            "if flags & 0xc000 != 0 {\n"
            "        return Err(SymbolTextError::InvalidFlags(\n"
            "            \"reserved symbol-dictionary flags\","
        ),
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_dictionary_flag_words_match_independent_field_policy",
    },
    {
        "id": "text-eight-row-strip-shrunk",
        "file": "src/symbol_text.rs",
        "before": "2 => 4,\n        _ => 8,",
        "after": "2 => 4,\n        _ => 4,",
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_text_flag_words_preserve_geometry_and_error_precedence",
    },
    {
        "id": "text-refinement-height-selector-aliased",
        "file": "src/symbol_text.rs",
        "before": "let refinement_height = refinement_choice(text_huffman_selector(flags, 8))?;",
        "after": "let refinement_height = refinement_choice(text_huffman_selector(flags, 6))?;",
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_text_table_selectors_match_complete_word_domain",
    },
    {
        "id": "custom-table-oob-policy-inverted",
        "file": "src/symbol_text.rs",
        "before": "if supplied.has_oob != requires_oob {",
        "after": "if supplied.has_oob == requires_oob {",
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_table_bindings_preserve_selector_order_identity_and_oob_policy",
    },
    {
        "id": "dictionary-bitmap-size-table-aliased-to-refinement",
        "file": "src/symbol_text.rs",
        "before": "IntegerField::DictionaryBitmapSize => self.dictionary_bitmap_size.as_ref(),",
        "after": "IntegerField::DictionaryBitmapSize => self.refinement_size.as_ref(),",
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_table_associations_reject_missing_fields_and_preserve_borrowed_tables",
    },
    {
        "id": "standard-table-d-stored-in-e-slot",
        "file": "src/symbol_text.rs",
        "before": "StandardHuffmanTable::D => self.d = materialized,",
        "after": "StandardHuffmanTable::D => self.e = materialized,",
        "target": "lib",
        "test": "symbol_text::tests::flags_and_tables::raw_standard_table_cache_covers_every_slot_and_failed_materialization",
    },
    {
        "id": "huffman-normal-partition-skips-one-value",
        "file": "src/huffman.rs",
        "before": "if previous.checked_add(1) != Some(line.range_low) {",
        "after": "if previous.checked_add(2) != Some(line.range_low) {",
        "target": "lib",
        "test": "huffman::tests::table_validation::raw_normal_ranges_match_wide_endpoint_and_contiguity_rules",
    },
    {
        "id": "huffman-duplicate-low-line-accepted",
        "file": "src/huffman.rs",
        "before": "if lower.is_some() {",
        "after": "if false && lower.is_some() {",
        "target": "lib",
        "test": "huffman::tests::table_validation::raw_special_line_metadata_rejects_order_duplicates_and_invalid_fields",
    },
    {
        "id": "empty-unaligned-row-blit-rejected",
        "file": "src/bitmap.rs",
        "before": (
            "if width == 0 {\n"
            "            return Ok(());\n"
            "        }\n"
            "        let mut source_bit = source_x;"
        ),
        "after": (
            "if width == 0 {\n"
            "            return Err(MalformedError::IntegerOverflow);\n"
            "        }\n"
            "        let mut source_bit = source_x;"
        ),
        "target": "lib",
        "test": "bitmap::tests::composition::zero_width_unaligned_row_blit_preserves_all_storage",
    },
    {
        "id": "row-blit-source-bit-alignment-discarded",
        "file": "src/bitmap.rs",
        "before": (
            "let source_chunk = read_shifted_chunk(&source.data, source_byte, source_bit % 8)\n"
            "                .ok_or(MalformedError::IntegerOverflow)?;\n"
            "            let destination_chunk =\n"
            "                read_chunk(&self.data, destination_index).ok_or(MalformedError::IntegerOverflow)?;"
        ),
        "after": (
            "let source_chunk = read_shifted_chunk(&source.data, source_byte, 0)\n"
            "                .ok_or(MalformedError::IntegerOverflow)?;\n"
            "            let destination_chunk =\n"
            "                read_chunk(&self.data, destination_index).ok_or(MalformedError::IntegerOverflow)?;"
        ),
        "target": "lib",
        "test": "bitmap::tests::composition::row_batch_chunking_matches_independent_ordered_bit_updates",
    },
    {
        "id": "generic-snapshot-rejects-inclusive-retained-limit",
        "file": "src/generic_region.rs",
        "before": "if storage_bytes > limits.max_retained_state_bytes {",
        "after": "if storage_bytes >= limits.max_retained_state_bytes {",
        "target": "lib",
        "test": "generic_region::tests::sessions_and_snapshots::generic_snapshots_copy_all_states_and_preserve_rejected_targets",
    },
    {
        "id": "generic-typical-work-row-charge-lost",
        "file": "src/generic_region.rs",
        "before": "work = next_work;\n    }\n    Ok(work)\n}",
        "after": "work = next_work - u64::from(rows);\n    }\n    Ok(work)\n}",
        "target": "lib",
        "test": "generic_region::tests::preflight_and_templates::raw_raster_work_guards_cover_exact_addition_overflow_boundaries",
    },
    {
        "id": "reference-role-rejects-inclusive-table-count",
        "file": "src/document.rs",
        "before": "if table_count > maximum_tables {",
        "after": "if table_count >= maximum_tables {",
        "target": "lib",
        "test": "document::tests::quotas_and_references::raw_reference_role_matrix_matches_numeric_tag_policy_locally_and_globally",
    },
    {
        "id": "failed-stripe-seal-clobbers-last-end",
        "file": "src/decode.rs",
        "before": "let next_open = checked_seal_stripe_state(page.open_stripe_max_row, row)?;",
        "after": (
            "page.last_stripe_end = Some(row);\n"
            "    let next_open = checked_seal_stripe_state(page.open_stripe_max_row, row)?;"
        ),
        "target": "lib",
        "test": "decode::tests::budgets_and_pages::raw_stripe_sealing_preserves_pixels_and_rejected_page_state",
    },
    {
        "id": "owned-generic-normalization-appends-wrong-marker",
        "file": "src/generic_region.rs",
        "before": (
            "normalized.push(ARITHMETIC_TERMINATOR[1]);\n"
            "    decode_arithmetic_generic(&normalized, header, rows, None, limits)"
        ),
        "after": (
            "normalized.push(0xab);\n"
            "    decode_arithmetic_generic(&normalized, header, rows, None, limits)"
        ),
        "target": "lib",
        "test": "generic_region::tests::segment_framing::known_length_normalization_preserves_zero_row_bitmap_geometry",
    },
    {
        "id": "direct-generic-normalization-appends-wrong-marker",
        "file": "src/generic_region.rs",
        "before": (
            "normalized.push(ARITHMETIC_TERMINATOR[1]);\n"
            "    decode_arithmetic_generic_into(&normalized, header, rows, limits, bitmap)"
        ),
        "after": (
            "normalized.push(0xab);\n"
            "    decode_arithmetic_generic_into(&normalized, header, rows, limits, bitmap)"
        ),
        "target": "lib",
        "test": "generic_region::tests::segment_framing::known_length_normalization_preserves_zero_row_bitmap_geometry",
    },
    {
        "id": "unknown-generic-extent-includes-following-byte",
        "file": "src/generic_region.rs",
        "before": "consumed: count_end,",
        "after": "consumed: count_end + 1,",
        "target": "lib",
        "test": "generic_region::tests::segment_framing::unknown_length_framing_preserves_count_extent_and_exact_eof_fields",
    },
    {
        "id": "runtime-work-pair-rejects-inclusive-limit",
        "file": "src/decode.rs",
        "before": "Some(used) => used <= limits.max_work_units,",
        "after": "Some(used) => used < limits.max_work_units,",
        "target": "lib",
        "test": "decode::tests::budgets_and_pages::runtime_budgets_match_wide_arithmetic_and_preserve_rejected_state",
    },
    {
        "id": "referred-buffer-retention-polarity-reversed",
        "file": "src/decode.rs",
        "before": (
            "== Some(false)\n"
            "        {\n"
            "            if let Some(buffer_index) = region_buffer_index(buffers, number) {"
        ),
        "after": (
            "== Some(true)\n"
            "        {\n"
            "            if let Some(buffer_index) = region_buffer_index(buffers, number) {"
        ),
        "target": "lib",
        "test": "decode::tests::budgets_and_pages::referred_bitmap_resources_release_only_selected_existing_keys_once",
    },
    {
        "id": "released-symbol-bitmap-bytes-not-deducted",
        "file": "src/decode.rs",
        "before": ".checked_add(bitmap_byte_len(&resource.exported[bitmap_index])?)",
        "after": ".checked_add(0)",
        "target": "lib",
        "test": "decode::tests::budgets_and_pages::referred_bitmap_resources_release_only_selected_existing_keys_once",
    },
    {
        "id": "raw-bitmap-snapshot-discards-source-x",
        "file": "src/decode.rs",
        "before": "snapshot.compose_region(\n            page,\n            region.x,",
        "after": "snapshot.compose_region(\n            page,\n            0,",
        "target": "lib",
        "test": "decode::tests::budgets_and_pages::raw_bitmap_snapshot_matches_literal_scalar_clipping_and_padding",
    },
    {
        "id": "reference-depth-fails-to-count-own-node",
        "file": "src/document.rs",
        "before": "let Some(candidate) = referred_depth.checked_add(1) else {",
        "after": "let Some(candidate) = referred_depth.checked_add(0) else {",
        "target": "lib",
        "test": "document::tests::quotas_and_references::reference_depth_orders_raw_metadata_and_reports_missing_or_overflowed_ancestors",
    },
    {
        "id": "generic-probability-state-validation-skipped",
        "file": "src/generic_region.rs",
        "before": "if let Err(error) = ArithmeticContext::from_state(context.state_index(), context.mps()) {",
        "after": "if let Err(error) = ArithmeticContext::from_state(0, 0) {",
        "target": "lib",
        "test": "generic_region::tests::diagnostics::probability_state_validation_matches_all_raw_fields_and_first_error",
    },
    {
        "id": "generic-packed-eight-bit-window-rejected",
        "file": "src/generic_region.rs",
        "before": "if count == 0 || count > 8 || row < 0 || row >= i64::from(bitmap.height) {",
        "after": "if count == 0 || count > 7 || row < 0 || row >= i64::from(bitmap.height) {",
        "target": "lib",
        "test": "generic_region::tests::diagnostics::packed_rows_match_scalar_msb_windows_padding_and_numeric_edges",
    },
    {
        "id": "generic-probability-context-error-fields-discarded",
        "file": "src/generic_region.rs",
        "before": (
            "if signal == RASTER_SIGNAL_CONTEXT {\n"
            "        GenericRegionError::ContextIndex {\n"
            "            index,\n"
            "            count: context_count,\n"
            "        }"
        ),
        "after": (
            "if signal == RASTER_SIGNAL_CONTEXT {\n"
            "        GenericRegionError::ContextIndex {\n"
            "            index: 0,\n"
            "            count: 0,\n"
            "        }"
        ),
        "target": "lib",
        "test": "generic_region::tests::diagnostics::probability_failure_projection_preserves_context_and_decoder_errors",
    },
    {
        "id": "mmr-reference-cursor-update-discarded",
        "file": "src/mmr.rs",
        "before": "*search_start = index;",
        "after": "*search_start = 0;",
        "target": "lib",
        "test": "mmr::tests::transition_cache::reference_search_matches_scalar_suffix_and_cursor_for_all_small_sorted_vectors",
    },
    {
        "id": "borrowed-symbol-peak-limit-off-by-one",
        "file": "src/decode.rs",
        "before": (
            "let (requested_bytes, peak) = borrowed_symbol_memory(count, active_retained_bytes)?;\n"
            "    if peak > limits.max_decoder_managed_memory_bytes {"
        ),
        "after": (
            "let (requested_bytes, peak) = borrowed_symbol_memory(count, active_retained_bytes)?;\n"
            "    if peak >= limits.max_decoder_managed_memory_bytes {"
        ),
        "target": "lib",
        "test": "decode::tests::symbol_resources::referred_symbol_collection_preserves_order_identity_and_exact_borrow_budget",
    },
    {
        "id": "borrowed-symbol-group-loses-later-bitmaps",
        "file": "src/decode.rs",
        "before": "imported.push(&resource.exported[bitmap_index]);",
        "after": "if bitmap_index == 0 { imported.push(&resource.exported[bitmap_index]); }",
        "target": "lib",
        "test": "decode::tests::symbol_resources::referred_symbol_collection_preserves_order_identity_and_exact_borrow_budget",
    },
    {
        "id": "total-reference-limit-off-by-one",
        "file": "src/document.rs",
        "before": "if total_references > limits.max_total_references {",
        "after": "if total_references >= limits.max_total_references {",
        "target": "lib",
        "test": "document::tests::quotas_and_references::reference_graph_retention_and_budget_precedence_match_scalar_policy",
    },
    {
        "id": "halftone-gray-state-trailing-bytes-ignored",
        "file": "src/halftone.rs",
        "before": "if *offset != coded.len() {",
        "after": "if *offset > coded.len() {",
        "target": "lib",
        "test": "halftone::tests::codec_state::gray_mmr_consumes_exact_prefixes_and_requires_all_declared_bytes",
    },
    {
        "id": "pattern-mmr-single-use-check-disabled",
        "file": "src/halftone.rs",
        "before": "if *used {",
        "after": "if *used && 0_u8 == 1_u8 {",
        "target": "lib",
        "test": "halftone::tests::codec_state::pattern_mmr_is_single_use_and_mode_errors_do_not_consume_it",
    },
    {
        "id": "symbol-code-length-power-boundary-misclassified",
        "file": "src/symbol_text.rs",
        "before": "else if symbol_count <= 65_536 {",
        "after": "else if symbol_count <= 65_535 {",
        "target": "lib",
        "test": "symbol_text::tests::codec_and_exports::symbol_code_length_is_minimal_at_every_u32_power_boundary",
    },
    {
        "id": "packed-symbol-padding-check-weakened",
        "file": "src/symbol_text.rs",
        "before": "if bytes[final_byte] & padding_mask != 0 {",
        "after": "if bytes[final_byte] & padding_mask > 0x80 {",
        "target": "lib",
        "test": "symbol_text::tests::codec_and_exports::packed_symbol_storage_matches_pixels_and_rejects_each_dirty_row",
    },
    {
        "id": "generic-formatter-failure-swallowed",
        "file": "src/generic_region.rs",
        "before": 'formatter.write_str("arithmetic region does not end in FF AC")',
        "after": '{ let _ = formatter.write_str("arithmetic region does not end in FF AC"); Ok(()) }',
        "target": "lib",
        "test": "generic_region::tests::diagnostics::diagnostic_variants_preserve_fields_sources_and_formatter_failures",
    },
    {
        "id": "io-kind-misclassification",
        "file": "src/error.rs",
        "before": "io::ErrorKind::PermissionDenied => Self::PermissionDenied,",
        "after": "io::ErrorKind::PermissionDenied => Self::Other,",
        "target": "error_contract",
        "test": "io_error_conversion_preserves_public_kind_without_retaining_private_message",
    },
    {
        "id": "reader-inclusive-limit-off-by-one",
        "file": "src/document.rs",
        "before": "if new_length_u64 > limits.max_stream_bytes {",
        "after": "if new_length_u64 >= limits.max_stream_bytes {",
        "target": "public_api",
        "test": "stream_byte_limit_is_inclusive_for_slice_and_reader_apis",
    },
    {
        "id": "swallowed-page-sink-error",
        "file": "src/decode.rs",
        "before": "    sink.write_page(page)\n",
        "after": "    let _ = sink.write_page(page);\n    Ok(())\n",
        "target": "public_api",
        "test": "sink_failure_is_preserved_and_does_not_poison_the_decoder",
    },
    {
        "id": "generic-marker-offset-discarded",
        "file": "src/decode.rs",
        "before": "GenericRegionError::UnexpectedArithmeticMarker { offset, code } => {\n            GenericRegionErrorKind::UnexpectedArithmeticMarker { offset, code }\n        }",
        "after": "GenericRegionError::UnexpectedArithmeticMarker { offset: _offset, code } => {\n            GenericRegionErrorKind::UnexpectedArithmeticMarker { offset: 0, code }\n        }",
        "target": "lib",
        "test": "decode::tests::error_adapters::generic_error_conversion_preserves_every_classification_and_field",
    },
    {
        "id": "symbol-availability-discarded",
        "file": "src/decode.rs",
        "before": "SymbolTextError::SymbolIdOutOfRange { id, available } => {\n            DecodeError::Malformed(MalformedError::SymbolIdOutOfRange { id, available })\n        }",
        "after": "SymbolTextError::SymbolIdOutOfRange { id, available: _available } => {\n            DecodeError::Malformed(MalformedError::SymbolIdOutOfRange { id, available: 0 })\n        }",
        "target": "lib",
        "test": "decode::tests::error_adapters::symbol_error_conversion_preserves_reasons_and_structured_fields",
    },
    {
        "id": "huffman-kind-misclassification",
        "file": "src/decode.rs",
        "before": "HuffmanError::InvalidTable(kind) => {\n            DecodeError::Malformed(MalformedError::InvalidHuffman(kind))\n        }",
        "after": "HuffmanError::InvalidTable(_kind) => {\n            DecodeError::Malformed(MalformedError::IntegerOverflow)\n        }",
        "target": "lib",
        "test": "decode::tests::error_adapters::huffman_error_conversion_preserves_every_table_kind_and_field",
    },
    {
        "id": "generic-decode-error-flattened",
        "file": "src/decode.rs",
        "before": "GenericRegionError::Decode(error) => return error,",
        "after": "GenericRegionError::Decode(_error) => return DecodeError::Malformed(MalformedError::IntegerOverflow),",
        "target": "lib",
        "test": "decode::tests::error_adapters::error_adapters_preserve_existing_decode_errors",
    },
    {
        "id": "reference-table-limit-off-by-one",
        "file": "src/document.rs",
        "before": "if table_count > maximum_tables {",
        "after": "if table_count > maximum_tables + 1 {",
        "target": "public_envelope",
        "test": "references::reference_table_limits_count_tables_not_dictionaries_in_both_header_forms",
    },
    {
        "id": "immediate-region-accepted-as-refinement-reference",
        "file": "src/document.rs",
        "before": "Some(kind) => is_intermediate_region(kind),",
        "after": "Some(_kind) => true,",
        "target": "public_envelope",
        "test": "references::only_intermediate_region_roles_can_supply_explicit_refinement_references",
    },
    {
        "id": "halftone-reference-cardinality-weakened",
        "file": "src/document.rs",
        "before": "reference_count == 1\n                && matches!(",
        "after": "reference_count >= 1\n                && matches!(",
        "target": "public_envelope",
        "test": "references::halftone_and_refinement_reference_cardinality_is_exact",
    },
    {
        "id": "global-reference-type-misclassification",
        "file": "src/document.rs",
        "before": next(case["site"] for case in REFERENCE_TYPE_CASES
                       if case["id"] == "resolved_global_type_discarded"),
        "after": next(case["replacement"] for case in REFERENCE_TYPE_CASES
                      if case["id"] == "resolved_global_type_discarded"),
        "target": "public_envelope",
        "test": "references::global_reference_roles_are_rechecked_without_mutating_shared_state",
    },
    {
        "id": "table-body-error-misclassification",
        "file": "src/document.rs",
        "before": "HuffmanError::Decode(error) => error,",
        "after": (
            "HuffmanError::Decode(_error) => "
            "DecodeError::Malformed(MalformedError::IntegerOverflow),"
        ),
        "target": "public_envelope",
        "test": "references::table_body_errors_precede_reference_role_checks_without_losing_error_fields",
    },
    {
        "id": "document-stream-quota-rejects-inclusive-boundary",
        "file": "src/document.rs",
        "before": (
            "fn enforce_stream_limit(length: usize, limits: &Limits) -> Result<(), DecodeError> {\n"
            "    let Ok(actual) = u64::try_from(length) else {\n"
            "        return Err(MalformedError::IntegerOverflow.into());\n"
            "    };\n"
            "    if actual > limits.max_stream_bytes {"
        ),
        "after": (
            "fn enforce_stream_limit(length: usize, limits: &Limits) -> Result<(), DecodeError> {\n"
            "    let Ok(actual) = u64::try_from(length) else {\n"
            "        return Err(MalformedError::IntegerOverflow.into());\n"
            "    };\n"
            "    if actual >= limits.max_stream_bytes {"
        ),
        "target": "lib",
        "test": "document::tests::quotas_and_references::numeric_preflight_quotas_match_wide_results_and_priority",
    },
    {
        "id": "document-page-quota-discards-next-count",
        "file": "src/document.rs",
        "before": (
            "fn enforce_page_limit(count: usize, limits: &Limits) -> Result<(), DecodeError> {\n"
            "    let Some(next_count) = count.checked_add(1) else {"
        ),
        "after": (
            "fn enforce_page_limit(count: usize, limits: &Limits) -> Result<(), DecodeError> {\n"
            "    let Some(next_count) = count.checked_add(0) else {"
        ),
        "target": "lib",
        "test": "document::tests::quotas_and_references::numeric_preflight_quotas_match_wide_results_and_priority",
    },
    {
        "id": "filled-bitmap-loses-black-storage-bit",
        "file": "src/bitmap.rs",
        "before": "data.resize(length, if value { 0xFF } else { 0 });",
        "after": "data.resize(length, if value { 0xFE } else { 0 });",
        "target": "lib",
        "test": "bitmap::tests::layout_and_access::filled_layout_and_padding_are_deterministic",
    },
    {
        "id": "filled-bitmap-keeps-black-padding",
        "file": "src/bitmap.rs",
        "before": "data[final_byte] &= trailing_mask;",
        "after": "data[final_byte] |= trailing_mask;",
        "target": "lib",
        "test": "bitmap::tests::layout_and_access::filled_layout_and_padding_are_deterministic",
    },
    {
        "id": "mmr-raster-reference-pixels-corrupted-after-row-finish",
        "file": "src/mmr.rs",
        "before": "production_mmr_row_termination!(row, height);",
        "after": (
            "production_mmr_row_termination!(row, height);\n"
            "        if row == 0 && height == 1 && width > 0 && !bitmap.data.is_empty() {\n"
            "            bitmap.data[0] ^= 0x80;\n"
            "        }"
        ),
        "target": "lib",
        "test": "mmr::tests::transition_cache::decoded_reference_cache_matches_the_last_physical_row",
    },
    {
        "id": "mmr-raster-reference-cache-corrupted-after-row-swap",
        "file": "src/mmr.rs",
        "before": "production_mmr_row_termination!(row, height);",
        "after": (
            "production_mmr_row_termination!(row, height);\n"
            "        if row == 0 && transitions.len() > 2 { transitions[0] = 0; }"
        ),
        "target": "lib",
        "test": "mmr::tests::transition_cache::decoded_reference_cache_matches_the_last_physical_row",
    },
    {
        "id": "mmr-raster-output-discards-input-extent",
        "file": "src/mmr.rs",
        "before": "production_mmr_trace_output!(data, width, height, bitmap, reader, traced_rows);",
        "after": (
            "production_mmr_trace_output!(data, width, height, bitmap, reader, traced_rows);\n"
            "    reader.bit_position = 0;"
        ),
        "target": "lib",
        "test": "mmr::tests::transition_cache::decoded_reference_cache_matches_the_last_physical_row",
    },
    {
        "id": "mmr-raster-output-blackens-a-white-pixel",
        "file": "src/mmr.rs",
        "before": "production_mmr_trace_output!(data, width, height, bitmap, reader, traced_rows);",
        "after": (
            "production_mmr_trace_output!(data, width, height, bitmap, reader, traced_rows);\n"
            "    if bitmap.width > 0 && bitmap.height > 0 {\n"
            "        bitmap.fill_black_span(0, 0, 1)\n"
            "            .map_err(|_error| MmrError::InvalidTransition)?;\n"
            "    }"
        ),
        "target": "lib",
        "test": "mmr::tests::transition_cache::decoded_reference_cache_matches_the_last_physical_row",
    },
    {
        "id": "mmr-required-prefix-ignores-nonzero-padding",
        "file": "src/mmr.rs",
        "before": (
            "if reader.read_bits(padding_count)? != 0 {\n"
            "            return Err(MmrError::NonZeroEofbPadding);\n"
            "        }"
        ),
        "after": "let _padding_value = reader.read_bits(padding_count)?;",
        "target": "lib",
        "test": "mmr::tests::normative_decode::prefix_decoder_rejects_nonzero_alignment_padding",
    },
    {
        "id": "contiguous-symbol-lookup-uses-first-bitmap",
        "file": "src/symbol_text.rs",
        "before": "Self::Contiguous(symbols) => symbols.get(index),",
        "after": (
            "Self::Contiguous(symbols) => "
            "symbols.get(if index < symbols.len() { 0 } else { index }),"
        ),
        "target": "lib",
        "test": "symbol_text::tests::resource_accounting::exported_symbols_match_exact_clone_oracle",
    },
    {
        "id": "borrowed-symbol-lookup-uses-first-bitmap",
        "file": "src/symbol_text.rs",
        "before": "Self::Borrowed(symbols) => match symbols.get(index) {",
        "after": (
            "Self::Borrowed(symbols) => match symbols.get("
            "if index < symbols.len() { 0 } else { index }) {"
        ),
        "target": "lib",
        "test": "symbol_text::tests::resource_accounting::exported_symbols_match_exact_clone_oracle",
    },
    {
        "id": "new-symbol-lookup-uses-first-bitmap",
        "file": "src/symbol_text.rs",
        "before": "Ok(&new_symbols[new_index])",
        "after": "Ok(&new_symbols[0])",
        "target": "lib",
        "test": "symbol_text::tests::resource_accounting::exported_symbols_match_exact_clone_oracle",
    },
    {
        "id": "exported-symbols-repeat-first-selection",
        "file": "src/symbol_text.rs",
        "before": "let symbol_id = exported_indices[index];",
        "after": "let symbol_id = exported_indices[0];",
        "target": "lib",
        "test": "symbol_text::tests::resource_accounting::exported_symbols_match_exact_clone_oracle",
    },
    {
        "id": "symbol-lookup-rejects-valid-representable-id",
        "file": "src/symbol_text.rs",
        "before": "if id_usize < imported.len() {",
        "after": (
            "if id_usize < available {\n"
            "        return Err(SymbolTextError::InvalidState(\"mutant false valid lookup rejection\"));\n"
            "    }\n"
            "    if id_usize < imported.len() {"
        ),
        "target": "lib",
        "test": "symbol_text::tests::resource_accounting::exported_symbols_match_exact_clone_oracle",
    },
    {
        "id": "single-refinement-rejects-required-instance-count",
        "file": "src/symbol_text.rs",
        "before": "} else {\n            decode_dictionary_single_instance_refinement(",
        "after": """} else {
            if aggregation_instances.is_some() {
                return Err(SymbolTextError::InvalidState(
                    "aggregation count on direct-coded symbol",
                ));
            }
            decode_dictionary_single_instance_refinement(""",
        "target": "lib",
        "test": "symbol_text::tests::dictionary_dispatch::single_instance_refinement_reaches_the_existing_raw_callback",
        "whitespace_flexible": True,
    },
    {
        "id": "direct-symbol-ignores-unexpected-instance-count",
        "file": "src/symbol_text.rs",
        "before": """if aggregation_instances.is_some() {
            return Err(SymbolTextError::InvalidState(
                "aggregation count on direct-coded symbol",
            ));
        }""",
        "after": "let _ignored_aggregation_count = aggregation_instances;",
        "target": "lib",
        "test": "symbol_text::tests::dictionary_dispatch::direct_coded_symbol_rejects_an_aggregation_count_before_decoding",
        "whitespace_flexible": True,
    },
    {
        "id": "text-count-skips-final-strip-terminator",
        "file": "src/symbol_text.rs",
        "before": "let delta_s = if first {\n                None\n            } else {",
        "after": """if decoded >= number_of_instances { break; }
            let delta_s = if first {
                None
            } else {""",
        "target": "lib",
        "test": "symbol_text::tests::text_termination::final_strip_terminator_precedes_following_dictionary_fields",
        "whitespace_flexible": True,
    },
    {
        "id": "text-count-delays-overfull-strip-rejection",
        "file": "src/symbol_text.rs",
        "before": """if decoded >= number_of_instances {
                return Err(SymbolTextError::InvalidCount("too many text instances"));
            }""",
        "after": "let _ignored_declared_text_limit = decoded >= number_of_instances;",
        "target": "lib",
        "test": "symbol_text::tests::text_termination::declared_instance_count_does_not_hide_a_missing_or_overfull_strip_end",
        "whitespace_flexible": True,
    },
    {
        "id": "generated-text-omits-final-strip-terminator",
        "file": "tests/support/codec_scenarios.rs",
        "before": "bits.put(1, 2); // Table H OOB terminates the final strip.",
        "after": "let _omitted_final_strip = self.instances;",
        "target": "fuzz_codec_families",
        "test": "tiny_generated_families_have_literal_packed_oracles",
    },
)


def mutate(source: str, mutation: dict) -> str:
    if mutation.get("whitespace_flexible", False):
        return replace_once(source, mutation["before"], mutation["after"])
    if source.count(mutation["before"]) != 1:
        raise ValueError(f"mutation site is missing or ambiguous: {mutation['id']}")
    return source.replace(mutation["before"], mutation["after"], 1)


def build_test_binary(cargo: list[str], directory: Path, environment: dict, target: str) -> Path:
    selector = ["--lib"] if target == "lib" else ["--test", target]
    output = run(cargo + [
        "test", "--offline", "--locked", *selector, "--no-run", "--message-format=json",
    ], directory, environment, capture=True)
    executables = []
    for line in output.splitlines():
        message = json.loads(line)
        if not isinstance(message, dict):
            raise ValueError("invalid Cargo artifact message")
        if message.get("reason") != "compiler-artifact":
            continue
        metadata, profile = message.get("target", {}), message.get("profile", {})
        if not isinstance(metadata, dict) or not isinstance(profile, dict):
            raise ValueError("invalid Cargo test artifact metadata")
        selected = (
            metadata.get("kind") == (["lib"] if target == "lib" else ["test"])
            and metadata.get("name") == ("jbig2_decoder" if target == "lib" else target)
        )
        if not selected or profile.get("test") is not True:
            continue
        executable = message.get("executable")
        if not isinstance(executable, str):
            raise ValueError(f"missing Cargo test executable: {target}")
        path = Path(executable).resolve()
        if not path.is_relative_to(directory / "target") or not path.is_file():
            raise ValueError(f"missing or out-of-tree Cargo test executable: {target}")
        executables.append(path)
    if len(executables) != 1:
        raise ValueError(f"missing or ambiguous Cargo test executable: {target}")
    return executables[0]


def check_mutations(toolchain: str) -> dict:
    from source_group import bound_source_groups

    if not MUTATIONS or len({case["id"] for case in MUTATIONS}) != len(MUTATIONS):
        raise ValueError("missing or duplicate contract mutations")
    results = []
    cargo = ["cargo", f"+{toolchain}"]
    groups, sources, _ = bound_source_groups(ROOT, tuple(dict.fromkeys(
        mutation["file"] for mutation in MUTATIONS
    )))
    with tempfile.TemporaryDirectory(prefix="jbig2-contract-mutations-") as temporary:
        directory = Path(temporary).resolve()
        if directory.is_relative_to(ROOT):
            raise ValueError("mutation checks must run outside the checkout")
        for name in ("src", "tests"):
            shutil.copytree(ROOT / name, directory / name)
        (directory / "tools").mkdir()
        shutil.copyfile(ROOT / "tools/distribution_consumer.rs",
                        directory / "tools/distribution_consumer.rs")
        for name in ("Cargo.toml", "Cargo.lock", "README.md", "SECURITY.md", "LICENSE"):
            shutil.copyfile(ROOT / name, directory / name)
        environment = isolated_environment(directory)
        environment["CARGO_INCREMENTAL"] = "1"
        baseline_directory = directory / "baseline-tests"
        baseline_directory.mkdir()
        baselines = {}
        for target in dict.fromkeys(mutation["target"] for mutation in MUTATIONS):
            executable = build_test_binary(cargo, directory, environment, target)
            baseline = baseline_directory / (target + executable.suffix)
            shutil.copy2(executable, baseline)
            baselines[target] = baseline
        for mutation in MUTATIONS:
            baseline = run(
                [str(baselines[mutation["target"]]), mutation["test"], "--exact"],
                directory, environment, capture=True,
            )
            if not re.search(r"test result: ok\. 1 passed; 0 failed;", baseline):
                raise ValueError(f"baseline did not run exactly one passing test: {mutation['id']}")
            group = groups[mutation["file"]]
            try:
                group.write(directory, mutate(group.text, mutation))
                executable = build_test_binary(cargo, directory, environment, mutation["target"])
                process = subprocess.run(
                    [str(executable), mutation["test"], "--exact"], cwd=directory, env=environment,
                    capture_output=True, text=True, check=False,
                )
                if process.returncode == 0 or not re.search(
                    r"test result: FAILED\. 0 passed; 1 failed;", process.stdout
                ):
                    raise ValueError(
                        f"mutation survived or failed for a non-test reason: {mutation['id']}\n"
                        + process.stdout + process.stderr
                    )
                results.append({
                    "id": mutation["id"], "test_target": mutation["target"],
                    "test_name": mutation["test"], "baseline": "passed",
                    "mutant_compiles": True, "mutant": "detected_by_failing_test",
                })
            finally:
                group.restore(directory)
    if any((ROOT / name).read_bytes() != data for name, data in sources.items()):
        raise ValueError("source changed during native mutation checks")
    return {
        "schema_version": 1, "status": "passed", "toolchain": toolchain,
        "scope": "Targeted public API, reference-role, typed-error, formatter-failure, symbol storage and halftone state contracts, not exhaustive codec mutation coverage",
        "runtime_source_modified": False, "mutations": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--toolchain", default="1.89.0")
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    try:
        result = check_mutations(arguments.toolchain)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"contract mutation validation failed: {error}", file=sys.stderr)
        result = {"schema_version": 1, "status": "failed", "error": str(error)}
    text = json.dumps(result, indent=2) + "\n"
    if arguments.output:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
