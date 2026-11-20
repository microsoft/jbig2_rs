#!/usr/bin/env python3
"""Check that MMR mode/run table witnesses match the actually consumed bits."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases


ROOT = Path(__file__).resolve().parents[1]
BITS = """&& code.bits as nat == crate::spec::readers::bits_value(
    old(reader).bytes@, old(reader).bit_position as nat, code.length as nat,
)"""
MODE_POST = """Ok(mode) => exists|code: ModeCode|
    mmr_spec::mode_code_in_implementation_table(code)
    && code.mode == mode
    && final(reader).bit_position == old(reader).bit_position + code.length as usize
""" + BITS + ","
RUN_POST = """Ok(code) =>
    mmr_spec::run_code_in_implementation_tables(black, code)
    && final(reader).bit_position == old(reader).bit_position + code.length as usize
""" + BITS + ","
MODE_SITE = """if code.length == length && code.bits == prefix {
    #[cfg(verus_keep_ghost)]
    production_mmr_mode_entry!(code, index, prefix, length);
    return Ok(code.mode);
}"""
RUN_SITE = "if let Some(code) = find_run_code(black, prefix, length, run_index) {"


def mode_case(identity, expression):
    return {
        "id": identity, "file": "src/mmr.rs", "module": "mmr",
        "function": "decode_mode", "site": MODE_SITE,
        "replacement": MODE_SITE.replace(
            "if code.length", f"let lookup_prefix = {expression};\nif code.length",
        ).replace("code.bits == prefix", "code.bits == lookup_prefix").replace(
            "code, index, prefix, length", "code, index, lookup_prefix, length",
        ),
        "controls": ((MODE_POST, MODE_POST.replace(BITS, "")),),
    }


def run_case(identity, expression):
    return {
        "id": identity, "file": "src/mmr.rs", "module": "mmr",
        "function": "decode_run_code", "site": RUN_SITE,
        "replacement": RUN_SITE.replace("black, prefix, length", f"black, {expression}, length"),
        "controls": ((RUN_POST, RUN_POST.replace(BITS, "")),),
    }


CASES = (
    mode_case("mode_lookup_flips_the_consumed_low_bit", "prefix ^ 1"),
    mode_case("mode_lookup_discards_the_consumed_low_bit", "prefix / 2"),
    run_case("run_lookup_flips_the_consumed_low_bit", "prefix ^ 1"),
    run_case("run_lookup_discards_the_consumed_low_bit", "prefix / 2"),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES,
            ("src/mmr.rs", "specs/mmr.rs", "specs/readers.rs",
             "proofs/spec_properties/readers.rs", "proofs/production_impl/mmr.rs"),
            "jbig2-mmr-prefix-",
            "Exact consumed bit-prefix witnesses for project MMR mode/run lookup; implementation membership, length and bounds remain in weaker controls; independent T.6 table fidelity/full raster semantics remain separate",
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"MMR prefix proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
