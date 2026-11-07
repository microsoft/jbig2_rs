#!/usr/bin/env python3
"""Check actual decoded metadata and owned-global result guarantees."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "src/document.rs", "src/decode.rs", "specs/document.rs",
    "specs/document/results.rs", "specs/document/context.rs", "specs/decode.rs",
    "specs/document/catalogs.rs",
    "specs/document/reference_types.rs",
    "proofs/shared/document.rs", "proofs/shared/decode.rs",
)
DECODE_TEST = "decode::tests::container_routes::public_results_preserve_exact_standalone_and_embedded_metadata"
GLOBAL_TEST = "document::tests::container_inspection::public_global_results_"
STANDALONE = """match out {
            Ok(document) => crate::document::standalone_metadata(
                input@, decode_spec::document_info(&document),
            ),
            Err(_) => true,
        }"""
EMBEDDED = """match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        }"""
GLOBAL_DECODER = """document_spec::global_parse_result(
            input@, document_spec::decoder_limits(self), out,
        )"""
GLOBAL_CONTEXT = """document_spec::global_parse_result(
            input@, document_spec::decoder_context_state(old(self)).0, out,
        )"""


def case(component, function, identity, before, after, losses, test, *, support_edits=()):
    row = runtime_function(ROOT, component, function)
    source = "\n".join(row["attributes"]) + "\n" + row["body_text"]
    mutant = replace_once(source, before, after)
    for existing, replacement in support_edits:
        mutant = replace_once(mutant, existing, replacement)
    weaker = mutant
    for existing, replacement in losses:
        weaker = replace_once(weaker, existing, replacement)
    return {
        "id": identity, "file": row["file"], "module": row["verifier_module"],
        "function": function, "site": source, "replacement": mutant,
        "controls": ((mutant, weaker),), "native_test": test,
    }


DECODE_CASES = tuple(
    case("decode", function, identity, before, after, ((post, "true"),), DECODE_TEST)
    for function, identity, before, after, post in (
        ("Decoder::decode", "decoder_standalone_uses_embedded_entry",
         "self.context().decode(input)", "self.context().decode_embedded(input)", STANDALONE),
        ("Decoder::decode_embedded", "decoder_embedded_uses_standalone_entry",
         "self.context().decode_embedded(input)", "self.context().decode(input)", EMBEDDED),
        ("Decoder::decode_embedded_with_globals", "decoder_globals_uses_standalone_entry",
         "self.context().decode_embedded_with_globals(input, globals)",
         "self.context().decode(input)", EMBEDDED),
        ("Decoder::decode_embedded_internal", "embedded_helper_parses_standalone",
         "parse_embedded_document(input, self.limits(), globals)",
         "parse_standalone_document(input, self.limits())", EMBEDDED),
        ("DecoderContext::decode_internal", "standalone_helper_parses_embedded",
         "parse_standalone_document(input, self.decoder.limits())",
         "parse_embedded_document(input, self.decoder.limits(), None)", STANDALONE),
        ("DecoderContext::decode", "context_standalone_uses_embedded_helper",
         "let result = self.decode_internal(input);",
         "let result = self.decoder.decode_embedded_internal(input, None, &mut self.mmr);",
         STANDALONE),
        ("DecoderContext::decode_embedded", "context_embedded_uses_standalone_helper",
         """let result = self
            .decoder
            .decode_embedded_internal(input, None, &mut self.mmr);""",
         "let result = self.decode_internal(input);", EMBEDDED),
        ("DecoderContext::decode_embedded_with_globals", "context_globals_uses_standalone_helper",
         """let result = self
            .decoder
            .decode_embedded_internal(input, Some(globals), &mut self.mmr);""",
         "let result = self.decode_internal(input);", EMBEDDED),
    )
)
GLOBAL_CASES = (
    case("document", "Decoder::parse_globals", "global_input_bytes_discarded",
         "while byte_index < input.len()", "while false",
         ((GLOBAL_DECODER, "true"),),
         GLOBAL_TEST + "own_complete_metadata_input_for_decoder_and_context"),
    case("document", "Decoder::parse_globals", "global_first_stream_guard_omitted",
         "enforce_stream_limit(input.len(), &self.limits)?;", "let _unbounded = input.len();",
         ((GLOBAL_DECODER, "true"),),
         GLOBAL_TEST + "preserve_first_stream_error_even_for_malformed_input",
         support_edits=(
          ("segment_index <= segments@.len(),\n                input@.len() <= self.limits.max_stream_bytes,",
           "segment_index <= segments@.len(),"),
          ("bytes@ == input@.subrange(0, byte_index as int),\n                types@.len() == depths@.len(),\n                input@.len() <= self.limits.max_stream_bytes,",
           "bytes@ == input@.subrange(0, byte_index as int),\n                types@.len() == depths@.len(),"))),
    case("document", "Decoder::parse_globals", "global_retention_guard_omitted",
         "input_bytes > self.limits.max_retained_state_bytes", "false",
         ((GLOBAL_DECODER, "true"),),
         GLOBAL_TEST + "match_exact_retained_admission_boundaries",
         support_edits=(("input@.len() <= self.limits.max_retained_state_bytes,", ""),)),
    case("document", "DecoderContext::parse_globals", "context_global_input_discarded",
         "self.decoder.parse_globals(input)", "self.decoder.parse_globals(&[])",
         ((GLOBAL_CONTEXT, "true"),),
         GLOBAL_TEST + "own_complete_metadata_input_for_decoder_and_context"),
)
CASES = DECODE_CASES + GLOBAL_CASES
CASE_GROUPS = {"public-decode-results": DECODE_CASES, "public-global-results": GLOBAL_CASES}


def baseline_groups(cases):
    modules = {}
    for row in cases:
        functions = modules.setdefault(row["module"], [])
        if row["function"] not in functions:
            functions.append(row["function"])
    return tuple((module, tuple(functions)) for module, functions in modules.items()
                 if len(functions) > 1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case-group", choices=tuple(CASE_GROUPS), required=True)
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    cases = CASE_GROUPS[arguments.case_group]
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), cases, FILES, "jbig2-public-results-",
            "All standalone/embedded owning decode results retain existing metadata relations;"
            " globals retain exact owned input, successful stream/retention admission and the"
            " first stream-limit error. No full raster/entropy or normative-fidelity claim."
            " Global native positives are accepted metadata containers, not entropy witnesses.",
            baseline_groups=baseline_groups(cases),
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"public result sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    report["case_group"] = arguments.case_group
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
