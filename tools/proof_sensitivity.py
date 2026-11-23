"""Shared isolated, pinned selected-proof sensitivity checks."""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from os.path import commonprefix
from pathlib import Path

from check_distribution import isolated_environment, run
from rust_source import source_functions
from source_group import SourceGroup, bound_source_groups, relocate_cases, whole_binds_unique_implementation


def replace_once(source: str, original: str, replacement: str) -> str:
    pattern = re.compile(r"\s*".join(re.escape(token) for token in original.split()))
    if len(pattern.findall(source)) != 1:
        raise ValueError("proof sensitivity site is missing or ambiguous")
    return pattern.sub(lambda _: replacement, source, count=1)


def dependency_build_root(root: Path, cache: Path) -> Path:
    if not cache.is_absolute() or cache.resolve() != cache or cache.is_relative_to(root.resolve()):
        raise ValueError("selected dependency cache must be outside the checkout without symlinks")
    paths = [
        root / "config/toolchain.json",
        root / "verification/Cargo.toml",
        root / "verification/Cargo.lock",
    ]
    for name in ("stdlib", "proposals"):
        paths.extend(path for path in (root / "verification" / name).rglob("*") if path.is_file())
    identity = hashlib.sha256()
    for path in sorted(paths):
        if path.is_symlink() or path.resolve() != path.absolute():
            raise ValueError("selected dependency inputs must not contain symlinks")
        identity.update(path.relative_to(root).as_posix().encode() + b"\0" + path.read_bytes() + b"\0")
    directory = cache / identity.hexdigest()
    directory.mkdir(parents=True, exist_ok=True)
    if directory.resolve() != directory:
        raise ValueError("selected dependency cache must not contain symlinks")
    return directory


@contextmanager
def proof_workspace(root: Path, executable: Path, prefix: str):
    manifest = json.loads((root / "config/toolchain.json").read_text())
    expected = manifest["verus"]["version"]
    rust = manifest["verus"]["rust_toolchain"]
    with tempfile.TemporaryDirectory(prefix=prefix) as temporary:
        directory = Path(temporary).resolve()
        if directory.is_relative_to(root):
            raise ValueError("proof sensitivity checks must run outside the checkout")
        for name in ("src", "specs", "proofs"):
            shutil.copytree(root / name, directory / name)
        for name in ("Cargo.toml", "Cargo.lock", "README.md", "SECURITY.md", "LICENSE"):
            shutil.copyfile(root / name, directory / name)
        (directory / "verification").mkdir()
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copyfile(root / "verification" / name, directory / "verification" / name)
        for name in ("stdlib", "proposals"):
            dependency_root = root / "verification" / name
            if dependency_root.is_dir():
                shutil.copytree(dependency_root, directory / "verification" / name)
        environment = isolated_environment(directory)
        environment.pop("JBIG2_SELECTED_BUILD_ROOT", None)
        cache = environment.pop("JBIG2_SELECTED_DEPENDENCIES", None)
        if cache is not None:
            build_root = dependency_build_root(root, Path(cache))
            for name in ("cargo-home", "proof-target"):
                if (build_root / name).resolve() != build_root / name:
                    raise ValueError("selected dependency cache must not contain symlinks")
            environment["CARGO_HOME"] = str(build_root / "cargo-home")
            environment["JBIG2_SELECTED_BUILD_ROOT"] = str(build_root)
        environment["RUSTUP_TOOLCHAIN"] = rust
        environment["VERUS_PATH"] = str(executable.resolve())
        environment["JBIG2_SELECTED_CARGO_MANIFEST"] = str(directory / "verification/Cargo.toml")
        version = run([str(executable), "--version"], directory, environment, capture=True)
        if f"Version: {expected}" not in version:
            raise ValueError("proof sensitivity requires the pinned Verus version")
        run(
            ["cargo", f"+{rust}", "fetch", "--locked", "--manifest-path",
             environment["JBIG2_SELECTED_CARGO_MANIFEST"]],
            directory, {**environment, "CARGO_NET_OFFLINE": "false"}, capture=True,
        )
        yield directory, environment, expected, rust


def selected_proof(
    executable: Path, directory: Path, environment: dict, expect_failure: bool,
    module: str, function: str | tuple[str, ...],
    *, verification_module=None,
):
    functions = (function,) if isinstance(function, str) else function
    if (
        not isinstance(functions, tuple) or not functions
        or any(not isinstance(name, str) or not name for name in functions)
        or len(set(functions)) != len(functions)
        or expect_failure and not isinstance(function, str)
    ):
        raise ValueError("selected proof needs unique targets; failures must select one function")
    # Verus cannot strip a lexical scope from a method reported under its parent type.
    selector = function if isinstance(function, str) else commonprefix(functions) + "*"
    if verification_module and verification_module != module:
        selector = "*::" + selector
    arguments = [
        "--verify-only-module", verification_module or module, "--verify-function",
        selector,
        "--num-threads", "1", "--rlimit", "30", "--triggers-mode", "silent", "--output-json",
    ]
    manifest = environment.get("JBIG2_SELECTED_CARGO_MANIFEST")
    if manifest is not None:
        if Path(manifest).resolve() != (directory / "verification/Cargo.toml").resolve():
            raise ValueError("selected Cargo manifest must belong to the isolated proof workspace")
        build_root = Path(environment.get("JBIG2_SELECTED_BUILD_ROOT", directory))
        if (
            not build_root.is_absolute() or build_root.resolve() != build_root
            or "JBIG2_SELECTED_BUILD_ROOT" in environment and (
                build_root.is_relative_to(directory)
                or environment.get("CARGO_HOME") != str(build_root / "cargo-home")
            )
        ):
            raise ValueError("selected dependency build root must match its isolated Cargo home")
        # The pinned focus mode appends verus-partial to its requested target.
        run(
            ["cargo", "clean", "--package", "jbig2_rs_verification",
             "--manifest-path", manifest,
             "--target-dir", str(build_root / "proof-target/verus-partial"),
             "--offline", "--locked"],
            directory, environment, capture=True,
        )
        command = [
            str(executable.resolve().parent / "cargo-verus"), "focus",
            "--manifest-path", manifest, "--offline", "--locked",
            "--target-dir", str(build_root / "proof-target"), "--", *arguments,
        ]
    else:
        command = [
            str(executable), "src/lib.rs", "--crate-name", "jbig2_rs_production",
            "--crate-type", "rlib", "--cfg", "verus_keep_ghost", *arguments,
        ]
    if expect_failure:
        # Zero skips additional error searches, not the first semantic failure.
        command.extend(["--multiple-errors", "0"])
    process = subprocess.run(
        command,
        cwd=directory, env=environment, text=True, capture_output=True, check=False, timeout=180,
    )
    try:
        report = json.loads(process.stdout)
    except ValueError as error:
        raise ValueError("verifier did not produce semantic proof evidence:\n" + process.stderr) from error
    result = report.get("verification-results")
    identities = {f"jbig2_rs_production::{module}::{name}" for name in functions}
    if (
        not isinstance(result, dict)
        or result.get("is-verifying-entire-crate") is not False
        or not isinstance(report.get("func-details"), dict)
        or not identities <= set(report["func-details"])
    ):
        raise ValueError(
            f"selected verification did not target {sorted(identities)}:\n{process.stderr}"
        )
    if expect_failure:
        if (
            process.returncode == 0 or result["errors"] < 1
            or not any(text in process.stderr for text in (
                "loop invariant not preserved", "postcondition not satisfied",
                "invariant not satisfied at end of loop body", "loop invariant not satisfied",
                "assertion failed",
            ))
        ):
            raise ValueError("mutant was not rejected by semantic obligations:\n" + process.stderr)
    elif (
        process.returncode != 0 or result["errors"] != 0
        or type(result["verified"]) is not int or result["verified"] < len(functions)
    ):
        raise ValueError("baseline/control proof did not pass:\n" + process.stderr)
    return result


def compile_native_mutant(directory: Path, environment: dict, rust: str):
    native_environment = {
        **environment,
        "CARGO_TARGET_DIR": str(directory / "target"),
        "CARGO_INCREMENTAL": "1",
    }
    return run(
        ["cargo", f"+{rust}", "check", "--offline", "--locked", "--lib"],
        directory, native_environment, capture=True,
    )


def weakened_contract(source: str, case: dict) -> str:
    controls = case["controls"] if "controls" in case else ((case["control"], ""),)
    if not controls:
        raise ValueError("semantic counterexample has no weaker control")
    for original, replacement in controls:
        source = replace_once(source, original, replacement)
    return source


def positive_baseline(
    root, executable, directory, environment, module, functions, *,
    whole=None, verification_module=None,
):
    receipt = os.environ.get("JBIG2_WHOLE_BASELINE")
    if receipt is not None:
        from run_semantic_proofs import baseline_from_whole, load_whole_baseline

        names = (functions,) if isinstance(functions, str) else functions
        if whole is None:
            whole = load_whole_baseline(root, Path(receipt))
        return baseline_from_whole(
            whole, [f"jbig2_rs_production::{module}::{name}" for name in names],
        )
    options = {"verification_module": verification_module} if verification_module and verification_module != module else {}
    return selected_proof(executable, directory, environment, False, module, functions, **options)


def check_semantic_cases(
    root: Path, executable: Path, cases: tuple, files: tuple, prefix: str, scope: str,
    *, baseline_groups: tuple = (),
) -> dict:
    targets = {(case["module"], case["function"]) for case in cases}
    covered = set()
    for module, functions in baseline_groups:
        group = {(module, function) for function in functions}
        if (
            not isinstance(functions, tuple) or len(functions) < 2
            or len(group) != len(functions) or not group <= targets or covered & group
        ):
            raise ValueError("shared baseline needs disjoint existing function targets")
        covered.update(group)
    groups, sources, modules = bound_source_groups(root, files)
    cases, baseline_groups = relocate_cases(root, cases, baseline_groups, sources)
    scopes = {
        (case["module"], case["function"]): case["verification_module"] for case in cases
    }
    scoped_targets = {}
    for case in cases:
        scoped_targets.setdefault((case["module"], case["function"]), set()).add(
            (case["verification_module"], case.get("impl_trait"))
        )
    collisions = {target for target, scopes in scoped_targets.items() if len(scopes) > 1}
    if any((module, function) in collisions for module, functions in baseline_groups for function in functions):
        raise ValueError("shared baseline cannot alias distinct native implementation scopes")
    whole = None
    receipt = os.environ.get("JBIG2_WHOLE_BASELINE")
    if receipt is not None:
        from run_semantic_proofs import load_whole_baseline

        whole = load_whole_baseline(root, Path(receipt))
    inventory = source_functions(root) if whole is not None and any("impl_trait" in case for case in cases) else []
    results = []
    with proof_workspace(root, executable, prefix) as (
        directory, environment, expected, rust,
    ):
        if whole is not None:
            copied_names = {
                "Cargo.toml", "Cargo.lock", "README.md", "SECURITY.md", "LICENSE",
                "verification/Cargo.toml", "verification/Cargo.lock",
            }
            for name, expected_hash in whole["manifest"]["source_bindings"].items():
                if name in files or name in copied_names or name.startswith((
                    "src/", "specs/", "proofs/", "verification/stdlib/", "verification/proposals/",
                )):
                    if hashlib.sha256((directory / name).read_bytes()).hexdigest() != expected_hash:
                        raise ValueError("isolated source differs from the whole-positive snapshot")
        baselines = {}
        baseline_targets = {}
        for module, functions in baseline_groups:
            baseline = positive_baseline(
                root, executable, directory, environment, module, functions, whole=whole,
                verification_module=scopes[(module, functions[0])],
            )
            for function in functions:
                target = (module, function, scopes[(module, function)])
                baselines[target] = baseline
                baseline_targets[target] = list(functions)
        for case in cases:
            print(f"semantic contrast: {case['module']}::{case['function']} / {case['id']}",
                  file=sys.stderr, flush=True)
            if case["file"] not in sources:
                raise ValueError("semantic counterexample edits an unbound source")
            if case["file"] not in groups:
                groups[case["file"]] = SourceGroup(root, case["file"], modules)
            group = groups[case["file"]]
            original = group.text
            identity = (case["module"], case["function"])
            target = (*identity, case["verification_module"])
            if target not in baselines:
                if identity in collisions or "impl_trait" in case and not (
                    whole is not None and whole_binds_unique_implementation(case, inventory)
                ):
                    baselines[target] = selected_proof(
                        executable, directory, environment, False, *identity,
                        verification_module=case["verification_module"],
                    )
                else:
                    baselines[target] = positive_baseline(
                        root, executable, directory, environment, *identity, whole=whole,
                        verification_module=case["verification_module"],
                    )
                baseline_targets[target] = [case["function"]]
            if (
                "impl_trait" in case and "whole_positive" not in baselines[target]
                and baselines[target]["verified"] != len(case["selected_impl_traits"])
            ):
                raise ValueError("selected pristine baseline must prove every colliding implementation")
            source = replace_once(original, case["site"], case["replacement"])
            try:
                group.write(directory, source)
                compile_native_mutant(directory, environment, rust)
                options = (
                    {"verification_module": case["verification_module"]}
                    if case["verification_module"] != case["module"] else {}
                )
                rejected = selected_proof(executable, directory, environment, True, *identity, **options)
                group.write(directory, weakened_contract(source, case))
                control = selected_proof(executable, directory, environment, False, *identity, **options)
                if "impl_trait" in case and (
                    rejected["errors"] != 1
                    or rejected["verified"] != len(case["selected_impl_traits"]) - 1
                    or control["verified"] != len(case["selected_impl_traits"])
                ):
                    raise ValueError("mutant must fail only its implementation while peers and the weaker control verify")
                record = {
                    "id": case["id"], "function": "::".join(identity),
                    "normal_rust_compiles": True, "strong_contract": "rejected",
                    "weaker_control": "accepted", "strong_verifier_errors": rejected["errors"],
                    "control_verifier_errors": control["errors"],
                    "baseline_selected_verification": baselines[target],
                    "baseline_selected_functions": baseline_targets[target],
                }
                if identity in collisions or "impl_trait" in case:
                    record.update(
                        verification_module=case["verification_module"], source_file=case["file"],
                    )
                if "impl_trait" in case:
                    record.update(
                        impl_trait=case["impl_trait"], selected_impl_traits=case["selected_impl_traits"],
                        strong_verified_functions=rejected["verified"],
                        control_verified_functions=control["verified"],
                    )
                results.append(record)
            finally:
                group.restore(directory)
    if any((root / name).read_bytes() != data for name, data in sources.items()):
        raise ValueError("production source changed during semantic sensitivity checks")
    if whole is not None:
        from run_semantic_proofs import digest, manifest, whole_files, WHOLE_SUMMARY

        if (
            manifest(root) != whole["manifest"]
            or whole_files(Path(receipt)) != whole["files"]
            or digest(Path(receipt) / WHOLE_SUMMARY) != whole["receipt_sha256"]
        ):
            raise ValueError("whole-positive inputs changed during semantic checks")
    return {
        "schema_version": 2, "status": "passed", "verus_version": expected,
        "scope": scope, "production_sources_modified": False, "counterexamples": results,
        "baseline_groups": [
            {"module": module, "functions": list(functions)}
            for module, functions in baseline_groups
        ],
        "source_bindings": {
            name: hashlib.sha256(data).hexdigest() for name, data in sources.items()
        },
    }
