#!/usr/bin/env python3
"""Check exact collected cardinality, symbol content selection and export order."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
BASELINE_GROUPS = (
    ("decode", (
        "CollectedSymbols::codebook", "collect_referred_symbols",
        "get_symbol_resource", "symbol_index",
    )),
    ("symbol_text", (
        "SymbolCodebook::get", "assemble_exported_symbols", "symbol_by_id",
        "decode_export_runs",
    )),
)
GET = runtime_function(ROOT, "symbol_text", "SymbolCodebook::get")["body_text"]
LOOKUP = runtime_function(ROOT, "symbol_text", "symbol_by_id")["body_text"]
ASSEMBLE_TARGET = runtime_function(ROOT, "symbol_text::dictionary", "assemble_exported_symbols")
ASSEMBLE = ASSEMBLE_TARGET["body_text"]
EXPORT_RUNS = runtime_function(ROOT, "symbol_text::dictionary", "decode_export_runs")["body_text"]
CODEBOOK_COUNT = """crate::symbol_text::symbol_text_spec::codebook_len(out)
    == decode_spec::collected_symbol_count(self),"""
GROUP_COUNT = """&& resource.exported@.len()
    == decode_spec::symbol_group_count(symbols@, number)"""
COLLECTED_COUNT = """&& decode_spec::collected_symbol_count(&bitmaps)
    == decode_spec::referred_symbol_count(
        symbols@, segment.header.referred_segments@,
        segment.header.referred_segments@.len(),
    )"""
IMPORT_OUTER = """imported@.len() == decode_spec::referred_symbol_count(
    symbols@, segment.header.referred_segments@, reference_index as nat,
),"""
IMPORT_INNER = """imported@.len() == decode_spec::referred_symbol_count(
    symbols@, segment.header.referred_segments@, reference_index as nat,
) + bitmap_index as nat,"""
GET_CONTENT = "&& symbol_text_spec::codebook_selects(self, index as nat, bitmap)"
LOOKUP_CONTENT = """&& symbol_text_spec::symbol_id_selects(
    imported, new_symbols@, id, bitmap
)"""
EXPORT_CONTENT = """&& symbol_text_spec::exported_contents_match(
    imported, new_symbols@, exported_indices@, exported@,
)"""
EXPORT_PREFIX = """forall|offset: int| 0 <= offset < index ==>
    #[trigger] symbol_text_spec::symbol_id_selects(
        imported, new_symbols@, exported_indices@[offset], &exported_symbols@[offset],
    ),"""
LOOKUP_ACCEPTS = """(symbol_text_spec::codebook_len(imported) + new_symbols@.len() <= usize::MAX
    && symbol_text_spec::codebook_len(imported) + new_symbols@.len() <= u32::MAX
    && (id as nat) < symbol_text_spec::codebook_len(imported) + new_symbols@.len())
    ==> out.is_ok(),"""
RUN_ORDER = """forall|left: int, right: int|
    0 <= left < right < exported@.len() ==>
        #[trigger] exported@[left] < #[trigger] exported@[right]"""
RUN_OUTER_BOUND = """forall|offset: int| 0 <= offset < exported@.len()
    ==> #[trigger] exported@[offset] < index,"""
RUN_INNER_BOUND = """forall|offset: int| 0 <= offset < exported@.len()
    ==> #[trigger] exported@[offset] < candidate,"""


def contract(row):
    attributes = [value for value in row["attributes"] if "verus_spec(" in value]
    if len(attributes) != 1:
        raise ValueError("symbol export target needs one exact attached contract")
    visibility = "" if row["visibility"] == "private" else row["visibility"] + " "
    return attributes[0] + "\n" + visibility + "fn " + row["name"]


ASSEMBLE_CONTRACT = contract(ASSEMBLE_TARGET)


def case(identity, function, site, replacement, controls, *,
         file="src/decode.rs", module="decode"):
    return {
        "id": identity, "file": file, "module": module,
        "function": function, "site": site, "replacement": replacement,
        "controls": controls,
    }


CASES = (
    case(
        "contiguous_codebook_loses_count", "CollectedSymbols::codebook",
        "Self::Contiguous(symbols) => SymbolCodebook::Contiguous(symbols),",
        "Self::Contiguous(symbols) => SymbolCodebook::Contiguous(&symbols[..0]),",
        ((CODEBOOK_COUNT, ""),),
    ),
    case(
        "borrowed_codebook_loses_count", "CollectedSymbols::codebook",
        "Self::Borrowed(symbols) => SymbolCodebook::Borrowed(symbols),",
        "Self::Borrowed(symbols) => SymbolCodebook::Borrowed(&symbols[..0]),",
        ((CODEBOOK_COUNT, ""),),
    ),
    case(
        "symbol_index_skips_first_duplicate", "symbol_index",
        "if symbols[index].0 == number { return Some(index); }",
        """if symbols[index].0 == number {
            if index + 1 < symbols.len() && symbols[index + 1].0 == number {
                return Some(index + 1);
            }
            return Some(index);
        }""",
        (("&& decode_spec::first_symbol_index(symbols@, number, index as int)", ""),),
    ),
    case(
        "resource_projection_uses_wrong_group_count", "get_symbol_resource",
        "Some(&symbols[index].1)", "Some(&symbols[0].1)",
        ((GROUP_COUNT, ""),),
    ),
    case(
        "borrowed_collection_drops_later_group_bitmaps", "collect_referred_symbols",
        "imported.push(&resource.exported[bitmap_index]);",
        "if bitmap_index == 0 { imported.push(&resource.exported[bitmap_index]); }",
        ((COLLECTED_COUNT, ""), (IMPORT_OUTER, ""), (IMPORT_INNER, "")),
    ),
    case(
        "contiguous_collection_discards_nonempty_group", "collect_referred_symbols",
        """let direct = match contiguous {
            Some(bitmaps) => bitmaps,
            None => &[],
        };""",
        "let direct: &[Bitmap] = &[];",
        ((COLLECTED_COUNT, ""),),
    ),
    case(
        "contiguous_lookup_returns_first_instead_of_requested_bitmap",
        "SymbolCodebook::get", GET,
        replace_once(
            GET,
            "Self::Contiguous(symbols) => symbols.get(index),",
            """Self::Contiguous(symbols) =>
                symbols.get(if index < symbols.len() { 0 } else { index }),""",
        ),
        ((GET_CONTENT, ""),),
        file="src/symbol_text.rs", module="symbol_text",
    ),
    case(
        "borrowed_lookup_returns_first_instead_of_requested_bitmap",
        "SymbolCodebook::get", GET,
        replace_once(
            GET,
            "Self::Borrowed(symbols) => match symbols.get(index) {",
            """Self::Borrowed(symbols) => match symbols.get(
                if index < symbols.len() { 0 } else { index }
            ) {""",
        ),
        ((GET_CONTENT, ""),),
        file="src/symbol_text.rs", module="symbol_text",
    ),
    case(
        "new_symbol_lookup_returns_first_instead_of_selected_bitmap",
        "symbol_by_id", LOOKUP,
        replace_once(LOOKUP, "Ok(&new_symbols[new_index])", "Ok(&new_symbols[0])"),
        ((LOOKUP_CONTENT, ""),),
        file="src/symbol_text.rs", module="symbol_text",
    ),
    case(
        "export_assembly_repeats_first_selection_instead_of_requested_order",
        "assemble_exported_symbols", ASSEMBLE,
        replace_once(
            ASSEMBLE, "let symbol_id = exported_indices[index];",
            "let symbol_id = exported_indices[0];",
        ),
        ((ASSEMBLE_CONTRACT, replace_once(ASSEMBLE_CONTRACT, EXPORT_CONTENT, "")),
         (EXPORT_PREFIX, "")),
        file="src/symbol_text.rs", module="symbol_text",
    ),
    case(
        "symbol_lookup_falsely_rejects_valid_representable_id",
        "symbol_by_id", LOOKUP,
        replace_once(
            LOOKUP, "if id_usize < imported.len() {",
            """if id_usize < available {
                return Err(SymbolTextError::InvalidState("mutant false valid lookup rejection"));
            }
            if id_usize < imported.len() {""",
        ),
        ((LOOKUP_ACCEPTS, ""),),
        file="src/symbol_text.rs", module="symbol_text",
    ),
    case(
        "export_run_repeats_first_candidate_instead_of_strict_order",
        "decode_export_runs", EXPORT_RUNS,
        replace_once(EXPORT_RUNS, "exported.push(candidate);", "exported.push(index);"),
        (
            ("&& (" + RUN_ORDER + ")", ""),
            (RUN_OUTER_BOUND + "\n" + RUN_ORDER + ",", RUN_OUTER_BOUND),
            (RUN_INNER_BOUND + "\n" + RUN_ORDER + ",", RUN_INNER_BOUND),
        ),
        file="src/symbol_text.rs", module="symbol_text",
    ),
)

def caller_request_case(function, field, replacement, native_test=None):
    text = function == "decode_text_region_body_with_end"
    count = "available_count" if text else "available"
    mode = "config.huffman" if text else "huffman"
    extent = (
        "symbol_text_spec::codebook_len(available_symbols)"
        if text else "symbol_text_spec::codebook_len(imported) + new_symbols.len()"
    )
    request = f"""let id_request = SymbolIdRequest {{
        available_symbols: {count},
        code_length,
        huffman: {mode},
    }};""" if text else f"""let id_request = SymbolIdRequest {{
        available_symbols: {count},
        code_length,
        huffman,
    }};"""
    witness = f"""#[cfg(verus_keep_ghost)]
        production_symbol_id_request!(
            id_request,
            {extent},
            code_length,
            {mode}
        );"""
    row = case(
        f"{'text' if text else 'dictionary'}_id_request_changes_{field}",
        function, request, replace_once(request, {
            "extent": f"available_symbols: {count}",
            "width": "code_length,",
            "mode": f"huffman: {mode}" if text else "huffman,",
        }[field], replacement),
        ((witness, ""),),
        file=runtime_function(ROOT, "symbol_text", function)["file"], module="symbol_text",
    )
    if native_test is not None:
        row["native_test"] = native_test
    return row


CASES += tuple(
    caller_request_case(function, field, replacement, native_test)
    for function, test_module, test_name in (
        ("decode_text_region_body_with_end", "text_termination",
         "actual_text_id_request_preserves_book_extent_segment_width_and_continuation"),
        ("decode_dictionary_single_instance_refinement", "dictionary_dispatch",
         "actual_dictionary_id_request_counts_imported_and_new_with_enclosing_width"),
    )
    for field, replacement, native_test in (
        ("extent", "available_symbols: 0", f"symbol_text::tests::{test_module}::{test_name}"),
        ("width", "code_length: 0,", f"symbol_text::tests::{test_module}::{test_name}"),
        ("mode", "huffman: false" if test_module == "text_termination" else "huffman: !huffman,", None),
    )
)

def segment_width_case(identity, function, site, replacement):
    text = function == "decode_text_region"
    count = "symbol_count" if text else "total_symbols"
    extent = (
        "symbol_text_spec::codebook_len(input.symbols)" if text else
        "symbol_text_spec::codebook_len(input.imported_symbols) + header.new_symbols"
    )
    witness = f"""#[cfg(verus_keep_ghost)]
        production_segment_symbol_width!(
            {count},
            code_length,
            {extent}
        );""" if text else """#[cfg(verus_keep_ghost)]
        production_segment_symbol_width!(total_symbols, code_length, input, header);"""
    row = case(
        identity, function, site, replacement, ((witness, ""),),
        file=runtime_function(ROOT, "symbol_text", function)["file"], module="symbol_text",
    )
    row["native_test"] = (
        "symbol_text::tests::codec_and_exports::actual_text_segment_width_uses_full_contiguous_or_borrowed_extent"
        if text else
        "symbol_text::tests::codec_and_exports::actual_dictionary_segment_width_uses_imported_plus_parsed_new_count"
    )
    return row


CASES += (
    segment_width_case(
        "text_segment_loses_input_count", "decode_text_region",
        "u32::try_from(input.symbols.len())", "u32::try_from(0_usize)",
    ),
    segment_width_case(
        "text_segment_uses_zero_width", "decode_text_region",
        "let code_length = symbol_id_code_length(symbol_count)?;",
        "let code_length = symbol_id_code_length(0)?;",
    ),
    segment_width_case(
        "dictionary_segment_uses_zero_width", "decode_symbol_dictionary",
        "let code_length = symbol_id_code_length(total_symbols)?;",
        "let code_length = symbol_id_code_length(0)?;",
    ),
    segment_width_case(
        "dictionary_segment_ignores_new_width_extent", "decode_symbol_dictionary",
        "let code_length = symbol_id_code_length(total_symbols)?;",
        "let code_length = symbol_id_code_length(imported_count)?;",
    ),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES,
            ("src/decode.rs", "specs/decode.rs", "src/symbol_text.rs",
             "specs/symbol_text.rs", "specs/symbol_text/symbol_ids.rs",
             "proofs/production_impl/symbol_text.rs", "src/bitmap.rs", "specs/bitmap.rs"),
            "jbig2-symbol-cardinality-",
            "Exact project cardinality/content/export order plus actual text and single-instance dictionary request witnesses: availability equals supplied codebook extent, while enclosing-segment width and entropy mode are forwarded unchanged. Original caller/lookup/storage/resource/loop guarantees remain in weaker controls. No derivation of every enclosing-segment parameter, lifecycle/reset timing, independent normative approval or full-stream/raster claim",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Symbol cardinality proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
