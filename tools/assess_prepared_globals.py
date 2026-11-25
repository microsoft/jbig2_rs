#!/usr/bin/env python3
"""Local, preserved selected/native contrasts for the opt-in prepared owner."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

from proof_sensitivity import replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
API = "src/decode/prepared.rs"
COPY = "src/decode/prepared/copying.rs"
MODEL = "specs/prepared.rs"
TEST = "decode::prepared::tests::"
BINDINGS = (
    "src/lib.rs", "src/decode.rs", API, COPY, MODEL,
    "src/decode/prepared/tests.rs", "src/document.rs",
    "src/symbol_text.rs", "src/symbol_text/bitmap.rs",
    "verification/Cargo.toml", "verification/Cargo.lock", "config/toolchain.json",
)

# Only the listed result component is removed in each weaker control.
CASES = (
    {
        "id": "constructor_discards_requested_extra_allowance",
        "function": "Decoder::prepared_globals", "file": API,
        "edits": (("additional_storage_bytes: allowance,", "additional_storage_bytes: 0,"),),
        "weaker_file": MODEL,
        "weaker": ((
            "&&& owner.additional_storage_bytes == allowance("
            "crate::document::decoder_limits(decoder), requested)",
            "",
        ),),
        "test": TEST + "public_prepared_owner_is_lazy_bound_and_resettable",
    },
    {
        "id": "reset_keeps_resident_resources",
        "function": "PreparedGlobals::reset", "file": API,
        "edits": ((
            "self.resources = None; self.resident_bytes = 0; "
            "self.state = PreparedGlobalsState::Pending;",
            "let _unreleased = &self.resources;",
        ),),
        "weaker_file": API,
        "weaker": ((
            "prepared_spec::info(final(self)).state == PreparedGlobalsState::Pending, "
            "prepared_spec::info(final(self)).resident_bytes == 0,",
            "",
        ),),
        "test": TEST + "public_prepared_owner_is_lazy_bound_and_resettable",
    },
    {
        "id": "copy_budget_rejects_inclusive_equality",
        "function": "CopyBudget::check", "file": COPY,
        "edits": (("if actual > self.limit {", "if actual >= self.limit {"),),
        "weaker_file": MODEL,
        "weaker": ((
            "} else if used as nat + bytes as nat > limit {",
            "} else if used as nat + bytes as nat == limit {"
            " *result == Ok(()) || *result == Err(CopyFailure::Optional("
            "PreparedGlobalsBypass::AdditionalStorage))"
            " } else if used as nat + bytes as nat > limit {",
        ),),
        "test": TEST + "prepared_budget_is_inclusive_and_never_suppresses_internal_errors",
    },
    {
        "id": "bitmap_copy_corrupts_one_pixel_but_keeps_layout",
        "function": "copy_bitmap", "file": COPY,
        "edits": (
            ("let copied = crate::symbol_text::clone_bitmap", "let mut copied = crate::symbol_text::clone_bitmap"),
            ("budget.charge(vector_bytes(&copied.data).map_err(required_copy_failure)?)?;",
             "if !copied.data.is_empty() { copied.data[0] ^= 0x80; }\n"
             "#[cfg(verus_keep_ghost)] proof! { crate::views::bitmap_fields(source); "
             "crate::views::bitmap_fields(&copied); "
             "reveal(crate::spec::bitmap::storage_valid); }\n"
             "budget.charge(vector_bytes(&copied.data).map_err(required_copy_failure)?)?;"),
        ),
        "weaker_file": MODEL,
        "weaker": (("&&& source.data@ == copied.data@", ""),),
        "test": TEST + "production_prepared_copies_exact_packed_frames_and_ids",
    },
    {
        "id": "symbol_copy_discards_resource_identity",
        "function": "copy_resources", "file": COPY,
        "edits": (("symbols.push(( *number, SymbolResource {", "symbols.push(( 0, SymbolResource {"),),
        "weaker_file": MODEL,
        "weaker": (("source.0 == copied.0 && source.1.contexts.is_none()",
                    "source.1.contexts.is_none()"),),
        "test": TEST + "production_prepared_copies_exact_packed_frames_and_ids",
    },
    {
        "id": "internal_overflow_is_suppressed_as_optional_allocation",
        "function": "copy_failure", "file": COPY,
        "edits": (("DecodeError::Allocation { .. } =>",
                   "DecodeError::Allocation { .. } | DecodeError::Malformed("
                   "MalformedError::IntegerOverflow) =>"),),
        "weaker_file": MODEL,
        "weaker": ((
            "_ => *failure == CopyFailure::Decode(*error),",
            "crate::DecodeError::Malformed(crate::MalformedError::IntegerOverflow) => "
            "*failure == CopyFailure::Decode(*error) || *failure == "
            "CopyFailure::Optional(PreparedGlobalsBypass::CopyAllocation), "
            "_ => *failure == CopyFailure::Decode(*error),",
        ),),
        "test": TEST + "prepared_budget_is_inclusive_and_never_suppresses_internal_errors",
    },
    {
        "id": "uncached_transfer_changes_the_consumed_original",
        "function": "PreparedGlobals::transfer_uncached", "file": API,
        "edits": ((
            "Some(resources) => Ok(resources),",
            "Some(mut resources) => { if !resources.symbols.is_empty() {"
            " resources.symbols[0].0 ^= 1; } Ok(resources) },",
        ),),
        "weaker_file": API,
        "weaker": (("Some(resources) => out == Ok(resources),", "Some(_) => true,"),),
        "test": TEST + "prepared_copy_refusal_transfers_the_original_cache_before_page_mutation",
    },
    {
        "id": "retained_contexts_are_admitted",
        "function": "admission", "file": COPY,
        "edits": (("return Some(PreparedGlobalsBypass::RetainedContexts);", "return None;"),),
        "weaker_file": MODEL,
        "weaker": (("Some(PreparedGlobalsBypass::RetainedContexts)", "None"),),
        "test": TEST + "prepared_admission_checks_contexts_tables_and_real_empty_resources",
    },
)


def record(command, directory, environment, prefix):
    """Keep raw diagnostics, including unsuccessful proof/native executions."""
    try:
        process = subprocess.run(
            command, cwd=directory, env=environment, text=True,
            capture_output=True, check=False, timeout=1050,
        )
    except subprocess.TimeoutExpired as error:
        for suffix, value in (("stdout", error.stdout), ("stderr", error.stderr)):
            (prefix.with_suffix("." + suffix)).write_bytes(
                value.encode() if isinstance(value, str) else value or b"",
            )
        raise
    prefix.with_suffix(".stdout").write_text(process.stdout)
    prefix.with_suffix(".stderr").write_text(process.stderr)
    return process


def proof(executable, workspace, environment, row, prefix, rejected=False):
    scope = row["verification_scope"]
    module = row["verifier_module"]
    name = (row["owner"] + "::" if row["owner"] else "") + row["name"]
    selector = name if scope == module else "*::" + name
    command = [
        str(executable.parent / "cargo-verus"), "focus",
        "--manifest-path", str(workspace / "verification/Cargo.toml"),
        "--locked", "--offline", "--",
        "--verify-only-module", scope, "--verify-function", selector,
        "--num-threads", "2", "--rlimit", "30",
        "--triggers-mode", "silent", "--output-json",
    ]
    if rejected:
        command.extend(["--multiple-errors", "0"])
    process = record(command, workspace, environment, prefix)
    report = json.loads(process.stdout)
    result = report["verification-results"]
    expected = f"jbig2_rs_production::{module}::{name}"
    if result["is-verifying-entire-crate"] is not False or expected not in report["func-details"]:
        raise ValueError(f"{prefix.name}: selected proof identity mismatch")
    if rejected:
        if process.returncode == 0 or result["errors"] < 1 or not any(
            text in process.stderr for text in
            ("postcondition not satisfied", "precondition not satisfied",
             "assertion failed", "loop invariant")
        ):
            raise ValueError(f"{prefix.name}: fault did not fail a semantic obligation")
    elif process.returncode != 0 or result["errors"] != 0 or result["verified"] < 1:
        raise ValueError(f"{prefix.name}: baseline/weaker proof did not pass")
    return result


def check(output, executable, selected):
    if output.exists() or not output.is_relative_to(ROOT / "target"):
        raise ValueError("use a fresh output directory under this checkout's target/")
    output.mkdir(parents=True)
    pin = json.loads((ROOT / "config/toolchain.json").read_text())["verus"]
    rust = pin["rust_toolchain"]
    workspace = output / "workspace"
    workspace.mkdir()
    for name in ("src", "specs", "proofs", "verification"):
        shutil.copytree(ROOT / name, workspace / name, ignore=shutil.ignore_patterns("target"))
    for name in ("Cargo.toml", "Cargo.lock", "README.md", "SECURITY.md", "LICENSE"):
        shutil.copyfile(ROOT / name, workspace / name)
    scratch = output / "scratch"
    scratch.mkdir()
    environment = {
        **os.environ, "RUSTUP_TOOLCHAIN": rust,
        "PATH": str(executable.parent) + os.pathsep + os.environ["PATH"],
        "TMPDIR": str(scratch), "CARGO_INCREMENTAL": "0",
        "CARGO_TARGET_DIR": str(output / "proof-target"),
    }
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "CARGO_BUILD_TARGET"):
        environment.pop(name, None)
    version = record([str(executable), "--version"], workspace, environment, output / "version")
    if version.returncode or f"Version: {pin['version']}" not in version.stdout:
        raise ValueError("selected proofs require the unchanged pinned Verus")
    originals = {
        name: (ROOT / name).read_bytes() for name in (*BINDINGS, __file__.removeprefix(str(ROOT) + "/"))
    }
    cases = []
    for case in selected:
        directory = output / case["id"]
        directory.mkdir()
        row = runtime_function(ROOT, "decode", case["function"])
        baseline = proof(executable, workspace, environment, row, directory / "baseline")
        for name in (case["file"], case["weaker_file"]):
            (workspace / name).write_bytes(originals[name])
        text = (workspace / case["file"]).read_text()
        for before, after in case["edits"]:
            text = replace_once(text, before, after)
        (workspace / case["file"]).write_text(text)
        (directory / "strong-mutant.rs").write_text(text)
        strong = proof(executable, workspace, environment, row, directory / "strong", rejected=True)
        native_environment = {**environment, "CARGO_TARGET_DIR": str(output / "native-target")}
        native = record(
            ["cargo", f"+{rust}", "test", "--lib", "--locked", "--offline",
             case["test"], "--", "--exact", "--nocapture"],
            workspace, native_environment, directory / "native",
        )
        if native.returncode == 0 or "test result: FAILED" not in native.stdout:
            raise ValueError(f"{case['id']}: native fault did not fail its exact control")
        text = (workspace / case["weaker_file"]).read_text()
        for before, after in case["weaker"]:
            text = replace_once(text, before, after)
        (workspace / case["weaker_file"]).write_text(text)
        (directory / "weaker-control.rs").write_text(text)
        weaker = proof(executable, workspace, environment, row, directory / "weaker")
        cases.append({
            "id": case["id"], "target": case["function"],
            "native_test": case["test"], "baseline": baseline,
            "strong": strong, "weaker": weaker, "native_fault_rejected": True,
        })
        (output / "case-results.json").write_text(json.dumps(cases, indent=2) + "\n")
        for name in (case["file"], case["weaker_file"]):
            (workspace / name).write_bytes(originals[name])
    return {
        "schema_version": 1, "status": "passed", "scope": "local-prepared-owner-contrasts-only",
        "not_full_stream_or_normative_proof": True, "cases": cases,
        "selected_cases": [case["id"] for case in selected],
        "complete_case_catalog": [case["id"] for case in CASES],
        "source_bindings": {
            name: hashlib.sha256(contents).hexdigest() for name, contents in originals.items()
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--case", action="append", choices=[case["id"] for case in CASES])
    arguments = parser.parse_args()
    output = arguments.output_dir.resolve()
    try:
        selected = [case for case in CASES if not arguments.case or case["id"] in arguments.case]
        report = check(output, arguments.verus_executable.resolve(), selected)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        report = {"schema_version": 1, "status": "failed", "error": str(error)}
    if output.is_dir() and output.is_relative_to(ROOT / "target"):
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
