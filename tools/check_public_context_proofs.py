#!/usr/bin/env python3
"""Check actual public reusable-context state/cleanup and caller return frames."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from rust_source import direct_contract_sections
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "src/document.rs", "src/decode.rs", "src/mmr.rs", "specs/document.rs",
    "specs/document/context.rs", "specs/document/results.rs", "specs/decode.rs", "specs/mmr.rs",
    "specs/document/catalogs.rs",
    "specs/document/reference_types.rs",
    "proofs/shared/document.rs", "proofs/shared/decode.rs", "proofs/shared/mmr.rs",
)
TEST = "document::tests::container_inspection::"
FRESH = TEST + "public_context_starts_empty_with_exact_limits_and_retained_metadata"
RESET = TEST + "public_context_reset_keeps_configuration_capacity_and_arbitrary_stored_bytes"
RELEASE = TEST + "public_context_release_resets_all_storage_metadata_and_configuration_frame"
FINISH = TEST + "public_context_finish_matches_strict_wide_threshold_and_empty_state"
ERRORS = TEST + "public_context_all_error_returns_clear_scratch_and_keep_limits"
STATE = "document_spec::decoder_context_state"
FINAL_COUNTER = f"""{STATE}(final(self)).3 ==
    if {STATE}(old(self)).3 > {STATE}(old(self)).0.max_retained_state_bytes {{
        0u64
    }} else {{ {STATE}(old(self)).3 }}"""


def case(component, function, identity, edits, losses, test, *, legacy=False):
    row = runtime_function(ROOT, component, function)
    attributes = "\n".join(row["attributes"])
    source = attributes + "\n" + row["body_text"]
    mutant = source
    for before, after in edits:
        mutant = replace_once(mutant, before, after)
    weaker = mutant
    if legacy:
        clauses = direct_contract_sections(row, "ensures")
        if len(clauses) != 1:
            raise ValueError("fresh scratch needs one actual owning result clause")
        weaker = replace_once(weaker, "ensures" + clauses[0], "ensures(|_out: Self| true)")
    else:
        for before, after in losses:
            weaker = replace_once(weaker, before, after)
    return {
        "id": identity, "file": row["file"], "module": row["verifier_module"],
        "function": function, "site": source, "replacement": mutant,
        "controls": ((mutant, weaker),), "native_test": test,
    }


BASIC_CASES = (
    case("mmr", "MmrScratch::default", "fresh_scratch_starts_charged",
         (("retained_bytes: 0,", "retained_bytes: 1,"),), (), FRESH, legacy=True),
    case("mmr", "MmrScratch::default", "fresh_scratch_contains_transitions",
         (("let out = Self {\n            transitions: Vec::new(),",
           "let out = Self {\n            transitions: vec![0_u32],"),),
         (), FRESH, legacy=True),
    case("document", "Decoder::context", "fresh_public_context_starts_charged",
         (("mmr: MmrScratch::default(),",
           "mmr: MmrScratch { retained_bytes: 1, ..MmrScratch::default() },"),),
         ((f"{STATE}(&out).3 == 0,", ""),), FRESH),
    case("document", "DecoderContext::retained_scratch_bytes", "stored_bytes_discarded",
         (("self.mmr.retained_bytes()", "0"),),
         ((f"out == {STATE}(self).3", "true"),), RESET),
    case("document", "DecoderContext::reset", "reset_keeps_old_contents",
         (("self.mmr.clear();", "let _uncleared = &self.mmr;"),),
         ((f"{STATE}(final(self)).1.len() == 0,", ""),
          (f"{STATE}(final(self)).2.len() == 0,", "")), RESET),
    case("document", "DecoderContext::reset", "reset_releases_stored_capacity_metadata",
         (("self.mmr.clear();", "self.mmr.release();"),),
         ((f"{STATE}(final(self)).3 == {STATE}(old(self)).3,", ""),), RESET),
    case("document", "DecoderContext::release_scratch", "release_only_clears",
         (("self.mmr.release();", "self.mmr.clear();"),),
         ((f"{STATE}(final(self)).3 == 0,", ""),), RELEASE),
    case("document", "DecoderContext::finish_call", "finish_releases_at_equality",
         (("> self.limits().max_retained_state_bytes", ">= self.limits().max_retained_state_bytes"),),
         ((FINAL_COUNTER, "true"),), FINISH),
    case("document", "DecoderContext::finish_call", "finish_ignores_retained_limit",
         (("self.retained_scratch_bytes() > self.limits().max_retained_state_bytes", "false"),),
         ((FINAL_COUNTER, "true"),), FINISH),
    case("mmr", "MmrScratch::clear", "clear_discards_stored_bytes_metadata",
         (("self.row_transitions.clear();",
           "self.row_transitions.clear(); self.retained_bytes = 0;"),),
         (("final(self).retained_bytes == old(self).retained_bytes,", ""),), RESET),
)
CALLER_CASES = tuple(
    case(
        component, "DecoderContext::" + method, method + "_omits_return_cleanup",
        (("self.finish_call();", "let _uncleaned = &self.mmr;"),),
        ((prefix + "decoder_context_finished(old(self), final(self))", "true"),),
        ERRORS,
    )
    for component, prefix, method in (
        ("document", "document_spec::", "inspect"),
        ("document", "document_spec::", "inspect_embedded"),
        ("document", "document_spec::", "inspect_embedded_with_globals"),
        ("document", "document_spec::", "parse_globals"),
        ("decode", "crate::document::", "decode"),
        ("decode", "crate::document::", "decode_embedded"),
        ("decode", "crate::document::", "decode_embedded_with_globals"),
    )
)
CASES = BASIC_CASES + CALLER_CASES
CASE_GROUPS = {"public-context": BASIC_CASES, "context-callers": CALLER_CASES}


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
            ROOT, arguments.verus_executable.resolve(), cases, FILES, "jbig2-public-context-",
            "Actual fresh/configuration/stored-byte/clear/release/threshold state and all seven"
            " public return cleanup frames, preserving existing metadata guarantees. Internal"
            " arbitrary stored counters are not physical capacity/process-memory bounds or"
            " public compressed-image witnesses. No new preconditions or assumptions.",
            baseline_groups=baseline_groups(cases),
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"public context sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    report["case_group"] = arguments.case_group
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
