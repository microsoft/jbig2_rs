#!/usr/bin/env python3
"""Check owned halftone probability validity and nonempty identity-export results."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from rust_source import source_functions
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
EXPORT_NAMES = ("select_exports_owned", "assemble_owned_exported_symbols")
FILES = (
    "src/halftone.rs", "specs/halftone.rs",
    "src/generic_region.rs", "specs/generic_region.rs",
    "src/arithmetic.rs", "specs/arithmetic.rs",
    "src/symbol_text.rs", "specs/symbol_text.rs",
) + (("specs/symbol_text/codebook.rs",)
     if (ROOT / "specs/symbol_text/codebook.rs").is_file() else ())
BANK_RESULT = "halftone_spec::decoder_state_valid(&state)"
DECODER_ONLY = """(match &state {
    GenericRegionState::Arithmetic { session, .. } =>
        crate::arithmetic::arithmetic_spec::decoder_valid(&session.decoder),
    _ => halftone_spec::decoder_state_valid(&state),
})"""
EXPORT_COUNT = "exported@.len() == exported_indices@.len()"
EXPORT_CONTENTS = """&& symbol_text_spec::exported_contents_match(
    imported, new_symbols@, exported_indices@, exported@,
)"""


def export_target(root):
    rows = [
        row for row in source_functions(root)
        if row["name"] in EXPORT_NAMES and row["owner"] is None
        and (row["file_module"] == "symbol_text::dictionary"
             or row["file_module"].startswith("symbol_text::dictionary::"))
    ]
    if len(rows) != 1:
        raise ValueError("owned export sensitivity requires one unambiguous production helper")
    return rows[0]


def contract(row):
    attributes = [value for value in row["attributes"] if "verus_spec(" in value]
    if len(attributes) != 1:
        raise ValueError("owned decoder target needs one exact attached contract")
    visibility = "" if row["visibility"] == "private" else row["visibility"] + " "
    return attributes[0] + "\n" + visibility + "fn " + row["name"]


def cases(root):
    constructor = runtime_function(root, "halftone", "new_arithmetic_state")
    original = constructor["body_text"]
    poisoned = replace_once(original, "let session = match", "let mut session = match")
    poisoned = replace_once(
        poisoned, "Ok(GenericRegionState::Arithmetic {",
        """if !session.contexts.contexts.is_empty() {
            session.contexts.contexts[0] = crate::arithmetic::ArithmeticContext {
                packed: 94,
            };
        }
        Ok(GenericRegionState::Arithmetic {""",
    )
    constructor_contract = contract(constructor)
    exports = export_target(root)
    export_contract = contract(exports)
    weakened_exports = replace_once(export_contract, EXPORT_COUNT, "true")
    weakened_exports = replace_once(weakened_exports, EXPORT_CONTENTS, "")
    return (
        {
            "id": "halftone_new_state_poisons_probability",
            "file": constructor["file"], "module": "halftone",
            "function": constructor["name"], "site": original, "replacement": poisoned,
            "controls": ((constructor_contract, replace_once(
                constructor_contract, BANK_RESULT, DECODER_ONLY,
            )),),
            "native_test": "halftone::tests::codec_state::"
                           "owned_arithmetic_planes_match_checked_banks_and_complete_state_across_reuse",
        },
        {
            "id": "owned_exports_discard_nonempty_identity",
            "file": exports["file"], "module": "symbol_text",
            "function": exports["name"], "site": exports["body_text"],
            "replacement": replace_once(exports["body_text"], "Ok(new_symbols)", "Ok(Vec::new())"),
            "controls": ((export_contract, weakened_exports),),
            "native_test": "symbol_text::tests::resource_accounting::"
                           "owned_identity_exports_transfer_vector_and_nonempty_payload_owners",
        },
    )


CASES = cases(ROOT)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES, FILES,
            "jbig2-owned-decode-",
            "Actual halftone constructor probability-bank validity and owned identity-export "
            "count/content relation. Native-compiling mutants; controls retain decoder validity, "
            "state kind, bitmap storage validity and original input preconditions. "
            "No ownership/allocation-count proof, normative approval or full raster/stream proof.",
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"owned decode sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
