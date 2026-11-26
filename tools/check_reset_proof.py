#!/usr/bin/env python3
"""Demonstrate semantic strength of the selected batch-reset proof."""

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from proof_sensitivity import compile_native_mutant, positive_baseline, proof_workspace, selected_proof as verify_selected


ROOT = Path(__file__).resolve().parents[1]
ASSIGNMENT = "contexts[index] = ArithmeticContext::new();"
ZERO_CLAUSES = (
    """forall|index: int| 0 <= index < final(contexts)@.len() ==>
        arithmetic_spec::context_index(final(contexts)@[index]) == 0
        && arithmetic_spec::context_mps(final(contexts)@[index]) == 0,""",
    """forall|completed: int| 0 <= completed < index ==>
        arithmetic_spec::context_index(contexts@[completed]) == 0
        && arithmetic_spec::context_mps(contexts@[completed]) == 0,""",
)


def mutated_reset(source: str, state_index: int, mps: int) -> str:
    if source.count(ASSIGNMENT) != 1:
        raise ValueError("batch-reset assignment is missing or ambiguous")
    packed = (state_index << 1) | mps
    replacement = f"contexts[index] = ArithmeticContext {{ packed: {packed} }};"
    return source.replace(ASSIGNMENT, replacement, 1)


def validity_only_contract(source: str) -> str:
    for clause in ZERO_CLAUSES:
        pattern = re.compile(r"\s*".join(re.escape(token) for token in clause.split()))
        if len(pattern.findall(source)) != 1:
            raise ValueError("zero-state contract/invariant changed; review sensitivity fixtures")
        source = pattern.sub("", source, count=1)
    return source


def selected_proof(executable: Path, directory: Path, environment: dict, expect_failure: bool):
    return verify_selected(
        executable, directory, environment, expect_failure, "arithmetic", "reset_contexts",
    )


def check_reset_proof(executable: Path) -> dict:
    production_bytes = (ROOT / "src/arithmetic.rs").read_bytes()
    with proof_workspace(ROOT, executable, "jbig2-reset-proof-") as (
        directory, environment, expected, rust,
    ):
        baseline = positive_baseline(
            ROOT, executable, directory, environment, "arithmetic", "reset_contexts",
        )
        path = directory / "src/arithmetic.rs"
        original = path.read_text(encoding="utf-8")
        results = []
        for state_index, mps in ((1, 0), (0, 1)):
            source = mutated_reset(original, state_index, mps)
            path.write_text(source, encoding="utf-8")
            compile_native_mutant(directory, environment, rust)
            rejected = selected_proof(executable, directory, environment, expect_failure=True)
            path.write_text(validity_only_contract(source), encoding="utf-8")
            control = selected_proof(executable, directory, environment, expect_failure=False)
            results.append({
                "state_index": state_index, "mps": mps,
                "normal_rust_compiles": True,
                "strong_zero_contract": "rejected",
                "validity_only_control": "accepted",
                "strong_verifier_errors": rejected["errors"],
                "control_verifier_errors": control["errors"],
            })
            path.write_text(original, encoding="utf-8")
    if (ROOT / "src/arithmetic.rs").read_bytes() != production_bytes:
        raise ValueError("production source changed during proof sensitivity checks")
    return {
        "schema_version": 1, "status": "passed", "verus_version": expected,
        "scope": "Selected reset proof only; normal whole-crate verification is required separately",
        "production_sources_modified": False, "counterexamples": results,
        "source_sha256": hashlib.sha256(production_bytes).hexdigest(),
        "baseline_selected_verification": baseline,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        result = check_reset_proof(arguments.verus_executable.resolve())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"reset proof sensitivity failed: {error}", file=sys.stderr)
        result = {"schema_version": 1, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
