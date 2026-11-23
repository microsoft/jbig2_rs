import json
import shutil
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from subprocess import CalledProcessError, CompletedProcess
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from proof_sensitivity import compile_native_mutant, check_semantic_cases, dependency_build_root, positive_baseline, proof_workspace, selected_proof, weakened_contract
from source_group import whole_binds_unique_implementation


class SelectedProofEvidenceTests(unittest.TestCase):
    def test_dependency_cache_binds_the_pinned_graph_not_mutated_production_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root, cache = Path(temporary).resolve() / "source", Path(temporary).resolve() / "cache"
            for name in ("config", "verification/stdlib/interface", "src"):
                (root / name).mkdir(parents=True)
            for name in (
                "config/toolchain.json", "verification/Cargo.toml",
                "verification/Cargo.lock", "verification/stdlib/interface/lib.rs",
                "src/lib.rs",
            ):
                (root / name).write_text(name)
            first = dependency_build_root(root, cache)
            (root / "src/lib.rs").write_text("a different production mutant")
            self.assertEqual(dependency_build_root(root, cache), first)
            for name in (
                "config/toolchain.json", "verification/Cargo.lock",
                "verification/stdlib/interface/lib.rs",
            ):
                path = root / name
                original = path.read_bytes()
                path.write_bytes(original + b"\nchanged")
                self.assertNotEqual(dependency_build_root(root, cache), first)
                path.write_bytes(original)
            for invalid in (Path("relative-cache"), root / "cache", cache / ".." / "other"):
                with self.subTest(path=invalid), self.assertRaisesRegex(ValueError, "outside the checkout"):
                    dependency_build_root(root, invalid)

    def test_cached_dependencies_never_skip_either_root_invalidation_or_selected_proof(self):
        with tempfile.TemporaryDirectory() as temporary:
            build_root = Path(temporary).resolve()
            directory = build_root / "source"
            environment = {
                "JBIG2_SELECTED_CARGO_MANIFEST": str(directory / "verification/Cargo.toml"),
                "JBIG2_SELECTED_BUILD_ROOT": str(build_root),
                "CARGO_HOME": str(build_root / "cargo-home"),
            }
            process = CompletedProcess([], 0, json.dumps(self.report()), "")
            with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
                for _ in range(2):
                    selected_proof(
                        Path("/tools/verus"), directory, environment, False,
                        "document", "finalize_pages",
                    )
            self.assertEqual(run.call_count, 4)
            for clean, verify in zip(run.call_args_list[::2], run.call_args_list[1::2]):
                command = clean.args[0]
                self.assertEqual(command[:4], ["cargo", "clean", "--package", "jbig2_rs_verification"])
                self.assertEqual(command[command.index("--target-dir") + 1],
                                 str(build_root / "proof-target/verus-partial"))
                command = verify.args[0]
                self.assertIn("focus", command)
                self.assertEqual(command[command.index("--target-dir") + 1],
                                 str(build_root / "proof-target"))
            with patch("proof_sensitivity.subprocess.run") as run:
                with self.assertRaisesRegex(ValueError, "isolated Cargo home"):
                    selected_proof(
                        Path("/tools/verus"), directory,
                        {**environment, "CARGO_HOME": str(build_root / "foreign")},
                        False, "document", "finalize_pages",
                    )
                run.assert_not_called()

    def test_workspace_copies_the_actual_verification_dependency_graph(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in (
                "src", "specs", "proofs", "config",
                "verification/proposals/interface", "verification/stdlib/interface",
            ):
                (root / name).mkdir(parents=True)
            for name in ("Cargo.toml", "Cargo.lock", "README.md", "SECURITY.md", "LICENSE",
                         "verification/Cargo.toml", "verification/Cargo.lock",
                         "verification/proposals/interface/lib.rs",
                         "verification/stdlib/interface/lib.rs"):
                (root / name).write_text(name)
            (root / "config/toolchain.json").write_text(json.dumps({
                "verus": {"version": "pinned", "rust_toolchain": "rust"},
            }))
            with patch("proof_sensitivity.run", return_value="Version: pinned") as run:
                with proof_workspace(root, Path("/tools/verus"), "jbig2-workspace-test-") as (
                    directory, environment, expected, rust,
                ):
                    self.assertEqual(expected, "pinned")
                    self.assertEqual(rust, "rust")
                    for name in ("verification/Cargo.toml", "verification/Cargo.lock",
                                 "verification/proposals/interface/lib.rs",
                                 "verification/stdlib/interface/lib.rs"):
                        self.assertEqual((directory / name).read_bytes(), (root / name).read_bytes())
                    self.assertEqual(environment["JBIG2_SELECTED_CARGO_MANIFEST"],
                                     str(directory / "verification/Cargo.toml"))
                    self.assertEqual(environment["VERUS_PATH"], str(Path("/tools/verus").resolve()))
                    self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
                    command, fetched_directory, fetched_environment = run.call_args.args
                    self.assertEqual(command, [
                        "cargo", "+rust", "fetch", "--locked", "--manifest-path",
                        str(directory / "verification/Cargo.toml"),
                    ])
                    self.assertEqual(fetched_directory, directory)
                    self.assertEqual(fetched_environment["CARGO_NET_OFFLINE"], "false")
                    self.assertFalse(directory.is_relative_to(root))
            self.assertFalse(directory.exists())

    def test_selected_cargo_uses_focus_and_the_isolated_graph_without_changing_targets(self):
        directory = Path("/isolated")
        environment = {"JBIG2_SELECTED_CARGO_MANIFEST": "/isolated/verification/Cargo.toml"}
        process = CompletedProcess([], 0, json.dumps(self.report()), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(Path("/tools/verus"), directory, environment, False,
                           "document", "finalize_pages")
        command = run.call_args.args[0]
        self.assertEqual(command[:2], [str(Path("/tools/verus").resolve().parent / "cargo-verus"), "focus"])
        self.assertEqual(command[command.index("--manifest-path") + 1],
                         environment["JBIG2_SELECTED_CARGO_MANIFEST"])
        self.assertEqual(command[command.index("--target-dir") + 1], str(directory / "proof-target"))
        self.assertEqual(command[command.index("--verify-function") + 1], "finalize_pages")
        self.assertEqual(command[command.index("--verify-only-module") + 1], "document")
        self.assertEqual(command.count("--verify-function"), 1)
        self.assertIn("--offline", command)
        self.assertIn("--locked", command)
        clean = run.call_args_list[0].args[0]
        self.assertEqual(clean, [
            "cargo", "clean", "--package", "jbig2_rs_verification",
            "--manifest-path", environment["JBIG2_SELECTED_CARGO_MANIFEST"],
            "--target-dir", str(directory / "proof-target/verus-partial"), "--offline", "--locked",
        ])
        self.assertEqual(run.call_count, 2)

    def test_selected_cargo_does_not_verify_if_root_invalidation_fails(self):
        with patch("proof_sensitivity.run", side_effect=CalledProcessError(101, ["cargo", "clean"])):
            with patch("proof_sensitivity.subprocess.run") as verify:
                with self.assertRaises(CalledProcessError):
                    selected_proof(
                        Path("/tools/verus"), Path("/isolated"),
                        {"JBIG2_SELECTED_CARGO_MANIFEST": "/isolated/verification/Cargo.toml"},
                        False, "document", "finalize_pages",
                    )
                verify.assert_not_called()

    def test_selected_cargo_rejects_a_foreign_source_graph_before_running(self):
        with patch("proof_sensitivity.subprocess.run") as run:
            with self.assertRaisesRegex(ValueError, "isolated proof workspace"):
                selected_proof(
                    Path("/verus"), Path("/isolated"),
                    {"JBIG2_SELECTED_CARGO_MANIFEST": "/foreign/verification/Cargo.toml"},
                    False, "document", "finalize_pages",
                )
            run.assert_not_called()

    def test_native_cache_is_workspace_local_and_does_not_change_the_proof_environment(self):
        environment = {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": "/unrelated", "PATH": "/tools"}
        original = dict(environment)
        directory = Path("/isolated")
        with patch("proof_sensitivity.run", return_value="checked") as native:
            self.assertEqual(compile_native_mutant(directory, environment, "pinned"), "checked")
        command, checkout, native_environment = native.call_args.args
        self.assertEqual(command, ["cargo", "+pinned", "check", "--offline", "--locked", "--lib"])
        self.assertEqual(checkout, directory)
        self.assertEqual(native_environment["CARGO_TARGET_DIR"], str(directory / "target"))
        self.assertEqual(native_environment["CARGO_INCREMENTAL"], "1")
        self.assertEqual(environment, original)
        self.assertTrue(native.call_args.kwargs["capture"])

    def test_native_compile_errors_are_not_hidden_by_incremental_reuse(self):
        error = CalledProcessError(101, ["cargo", "check"])
        with patch("proof_sensitivity.run", side_effect=error):
            with self.assertRaises(CalledProcessError):
                compile_native_mutant(Path("/isolated"), {}, "pinned")

    def test_whole_trait_binding_requires_one_exact_verified_nondivergent_declaration(self):
        case = {
            "module": "value", "function": "Value::default", "file": "src/value.rs",
            "verification_module": "value", "impl_trait": "Default",
            "selected_impl_traits": ["Default"],
        }
        row = {
            "verifier_module": "value", "owner": "Value", "name": "default",
            "file": "src/value.rs", "verification_scope": "value", "impl_trait": "Default",
            "verification_requested": True, "runtime_proof_divergence": False,
        }
        self.assertTrue(whole_binds_unique_implementation(case, [row]))
        for field, value in (
            ("file", "src/other.rs"), ("verification_scope", "other"),
            ("impl_trait", "Other"), ("verification_requested", False),
            ("runtime_proof_divergence", True),
        ):
            with self.subTest(field=field):
                self.assertFalse(whole_binds_unique_implementation(case, [{**row, field: value}]))
        self.assertFalse(whole_binds_unique_implementation(case, []))
        self.assertFalse(whole_binds_unique_implementation(case, [row, {**row, "verification_scope": "other"}]))
        self.assertFalse(whole_binds_unique_implementation(
            {**case, "selected_impl_traits": ["Default", "Other"]}, [row],
        ))

    def test_unique_trait_reuses_actual_whole_count_but_runs_both_case_queries(self):
        import hashlib

        with tempfile.TemporaryDirectory() as temporary:
            root, directory = Path(temporary) / "source", Path(temporary) / "proof"
            (root / "src").mkdir(parents=True)
            (root / "src/lib.rs").write_text("mod value;\n")
            source = (
                "struct Value;\n// guarantee\n"
                "#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
                "impl Default for Value { fn default() -> Self { Self } }\n"
            )
            (root / "src/value.rs").write_text(source)
            shutil.copytree(root, directory)
            case = {
                "id": "unique", "file": "src/value.rs", "module": "value",
                "verification_module": "value", "function": "Value::default",
                "impl_trait": "Default", "site": "fn default() -> Self { Self }",
                "replacement": "fn default() -> Self { let _ = 0; Self }",
                "controls": (("// guarantee", "// weak"),),
            }
            snapshot = {"source_bindings": {
                f"src/{name}": hashlib.sha256((root / "src" / name).read_bytes()).hexdigest()
                for name in ("lib.rs", "value.rs")
            }}
            whole = {
                "receipt_sha256": "1" * 64, "report_sha256": "2" * 64,
                "manifest": snapshot, "files": {"baseline": "3" * 64},
                "report": {
                    "verification-results": {
                        "success": True, "is-verifying-entire-crate": True,
                        "verified": 999, "errors": 0,
                    },
                    "func-details": {"jbig2_rs_production::value::Value::default": {}},
                },
            }

            @contextmanager
            def workspace(*args):
                yield directory, {}, "pinned", "rust"

            def verify(executable, checkout, environment, failure, module, function):
                return {"verified": 0 if failure else 1, "errors": int(failure)}

            with patch.dict("os.environ", {"JBIG2_WHOLE_BASELINE": "/receipt"}), patch(
                "run_semantic_proofs.load_whole_baseline", return_value=whole,
            ), patch("run_semantic_proofs.manifest", return_value=snapshot), patch(
                "run_semantic_proofs.whole_files", return_value=whole["files"],
            ), patch("run_semantic_proofs.digest", return_value="1" * 64), patch(
                "proof_sensitivity.proof_workspace", workspace,
            ), patch("proof_sensitivity.selected_proof", side_effect=verify) as queries, patch(
                "proof_sensitivity.run",
            ):
                report = check_semantic_cases(
                    root, Path("/verus"), (case,), ("src/value.rs",), "fixture-", "scope",
                )
            self.assertEqual(queries.call_count, 2)
            record = report["counterexamples"][0]
            self.assertEqual(record["baseline_selected_verification"]["verified"], 999)
            self.assertTrue(record["baseline_selected_verification"]["is-verifying-entire-crate"])
            self.assertEqual(record["strong_verified_functions"], 0)
            self.assertEqual(record["control_verified_functions"], 1)

    def test_same_scope_impls_share_only_a_complete_selected_peer_baseline(self):
        with tempfile.TemporaryDirectory() as temporary:
            root, directory = Path(temporary) / "source", Path(temporary) / "proof"
            (root / "src").mkdir(parents=True)
            (root / "src/lib.rs").write_text("mod owner;\nmod left;\n")
            (root / "src/owner.rs").write_text("pub struct Failure;\n")
            source, cases = "", []
            for kind in ("u8", "u16"):
                source += (
                    f"// guarantee {kind}\n#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
                    f"impl From<{kind}> for crate::owner::Failure {{ fn from(value: {kind}) -> Self {{ Self }} }}\n"
                )
                cases.append({
                    "id": kind, "file": "src/left.rs", "module": "owner",
                    "verification_module": "left", "function": "Failure::from",
                    "impl_trait": f"From<{kind}>",
                    "site": f"fn from(value: {kind}) -> Self {{ Self }}",
                    "replacement": f"fn from(value: {kind}) -> Self {{ let _ = value; Self }}",
                    "controls": ((f"// guarantee {kind}", f"// weak {kind}"),),
                })
            (root / "src/left.rs").write_text(source)
            shutil.copytree(root, directory)

            @contextmanager
            def workspace(*args):
                yield directory, {}, "pinned", "rust"

            for broken in (None, "baseline", "strong", "control"):
                calls = []

                def verify(executable, checkout, environment, failure, module, function, *, verification_module):
                    position = len(calls)
                    calls.append((verification_module, failure))
                    verified = 1 if failure else 2
                    if (broken == "baseline" and position == 0) or (broken == "control" and position == 2):
                        verified = 1
                    if broken == "strong" and failure:
                        verified = 0
                    return {"verified": verified, "errors": int(failure), "is-verifying-entire-crate": False}

                with patch("proof_sensitivity.proof_workspace", workspace), patch(
                    "proof_sensitivity.selected_proof", side_effect=verify,
                ), patch(
                    "proof_sensitivity.positive_baseline", side_effect=AssertionError("name-only inheritance"),
                ), patch("proof_sensitivity.run"):
                    if broken:
                        with self.assertRaises(ValueError):
                            check_semantic_cases(root, Path("/verus"), tuple(cases), ("src/left.rs",), "fixture-", "scope")
                    else:
                        report = check_semantic_cases(root, Path("/verus"), tuple(cases), ("src/left.rs",), "fixture-", "scope")
                        self.assertEqual(len(calls), 5)
                        self.assertEqual(
                            [row["impl_trait"] for row in report["counterexamples"]],
                            ["From<u8>", "From<u16>"],
                        )
                        self.assertTrue(all(
                            row["selected_impl_traits"] == ["From<u16>", "From<u8>"]
                            and row["strong_verified_functions"] == 1
                            and row["control_verified_functions"] == 2
                            for row in report["counterexamples"]
                        ))

    def test_colliding_trait_names_get_separate_selected_baselines_and_scoped_receipts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "source"
            directory = Path(temporary) / "proof"
            (root / "src").mkdir(parents=True)
            (root / "src/lib.rs").write_text("mod owner;\nmod left;\nmod right;\n")
            (root / "src/owner.rs").write_text("pub struct Failure;\n")
            cases = []
            for scope, kind in (("left", "u8"), ("right", "u16")):
                (root / f"src/{scope}.rs").write_text(
                    "// guarantee\n#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
                    f"impl From<{kind}> for crate::owner::Failure {{ fn from(value: {kind}) -> Self {{ Self }} }}\n"
                )
                cases.append({
                    "id": scope, "file": f"src/{scope}.rs", "module": "owner",
                    "verification_module": scope, "function": "Failure::from",
                    "site": f"fn from(value: {kind}) -> Self {{ Self }}",
                    "replacement": f"fn from(value: {kind}) -> Self {{ let _ = value; Self }}",
                    "controls": (("// guarantee", "// weakened guarantee"),),
                })
            shutil.copytree(root, directory)
            calls = []

            @contextmanager
            def workspace(*args):
                yield directory, {}, "pinned", "rust"

            def verify(executable, checkout, environment, failure, module, function, *, verification_module):
                calls.append((verification_module, failure))
                return {"verified": 1, "errors": int(failure), "is-verifying-entire-crate": False}

            with patch("proof_sensitivity.proof_workspace", workspace), patch(
                "proof_sensitivity.selected_proof", side_effect=verify,
            ), patch(
                "proof_sensitivity.positive_baseline", side_effect=AssertionError("ambiguous inheritance"),
            ), patch("proof_sensitivity.run"):
                result = check_semantic_cases(
                    root, Path("/verus"), tuple(cases),
                    ("src/left.rs", "src/right.rs"), "fixture-", "scope",
                )
            self.assertEqual(calls, [
                ("left", False), ("left", True), ("left", False),
                ("right", False), ("right", True), ("right", False),
            ])
            self.assertEqual(
                [(row["verification_module"], row["source_file"]) for row in result["counterexamples"]],
                [("left", "src/left.rs"), ("right", "src/right.rs")],
            )

    def test_whole_metadata_is_validated_once_and_snapshot_changes_fail_closed(self):
        import hashlib

        for broken in (None, "copy", "manifest", "files", "receipt"):
            with self.subTest(broken=broken), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary) / "source"
                directory = Path(temporary) / "proof"
                root.mkdir()
                directory.mkdir()
                original = "first second contract"
                (root / "fixture.rs").write_text(original)
                (directory / "fixture.rs").write_text("wrong" if broken == "copy" else original)
                cases = tuple({
                    "id": name, "file": "fixture.rs", "module": "module", "function": name,
                    "site": name, "replacement": "bad_" + name,
                    "controls": (("contract", "weak"),),
                } for name in ("first", "second"))
                snapshot = {"source_bindings": {
                    "fixture.rs": hashlib.sha256(original.encode()).hexdigest(),
                }}
                whole = {
                    "receipt_sha256": "1" * 64, "report_sha256": "2" * 64,
                    "manifest": snapshot, "files": {"baseline": "3" * 64},
                    "report": {
                        "verification-results": {
                            "success": True, "is-verifying-entire-crate": True,
                            "verified": 2, "errors": 0,
                        },
                        "func-details": {
                            f"jbig2_rs_production::module::{name}": {}
                            for name in ("first", "second")
                        },
                    },
                }

                @contextmanager
                def workspace(*args):
                    yield directory, {}, "pinned", "rust"

                def verify(executable, checkout, environment, failure, module, function):
                    return {"verified": 1, "errors": int(failure)}

                with patch.dict("os.environ", {"JBIG2_WHOLE_BASELINE": "/receipt"}), patch(
                    "run_semantic_proofs.load_whole_baseline", return_value=whole,
                ) as load, patch(
                    "run_semantic_proofs.manifest", return_value={} if broken == "manifest" else snapshot,
                ), patch(
                    "run_semantic_proofs.whole_files", return_value={} if broken == "files" else whole["files"],
                ), patch(
                    "run_semantic_proofs.digest", return_value="0" * 64 if broken == "receipt" else "1" * 64,
                ), patch("proof_sensitivity.proof_workspace", workspace), patch(
                    "proof_sensitivity.selected_proof", side_effect=verify,
                ) as queries, patch("proof_sensitivity.run"):
                    if broken:
                        with self.assertRaisesRegex(ValueError, "differs|changed"):
                            check_semantic_cases(root, Path("/verus"), cases, ("fixture.rs",), "fixture-", "scope")
                    else:
                        result = check_semantic_cases(root, Path("/verus"), cases, ("fixture.rs",), "fixture-", "scope")
                        self.assertEqual(result["status"], "passed")
                        self.assertEqual(queries.call_count, 4)
                    load.assert_called_once()
                    if broken == "copy":
                        queries.assert_not_called()

    def test_source_inventory_cache_tracks_exact_bytes_and_returns_independent_records(self):
        from rust_source import source_functions

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            path = root / "src/lib.rs"
            path.write_text("pub fn cached_inventory_probe() -> u32 { 1 }")
            first = source_functions(root)
            first[0]["attributes"].append("must not leak")
            again = source_functions(root)
            self.assertEqual(again[0]["attributes"], [])
            original_hash = again[0]["file_sha256"]
            path.write_text("pub fn cached_inventory_probe() -> u32 { 2 }")
            changed = source_functions(root)
            self.assertNotEqual(changed[0]["file_sha256"], original_hash)
            self.assertIn("{ 2 }", changed[0]["body_text"])
            (root / "src/child.rs").write_text("pub fn child_probe() {}")
            path.write_text("mod child; pub fn cached_inventory_probe() -> u32 { 2 }")
            self.assertEqual(len(source_functions(root)), 2)
            (root / "src/child.rs").unlink()
            with self.assertRaisesRegex(ValueError, "missing"):
                source_functions(root)

    def test_projection_cache_is_content_and_mode_bound_not_verification_evidence(self):
        from rust_source import production_code

        production_code.cache_clear()
        source = b"fn projection_cache_probe() { #[cfg(verus_keep_ghost)] proof! { assert(true); } let value = 1; } // marker\n"
        hidden = production_code(source)
        self.assertEqual(production_code(source), hidden)
        self.assertEqual(production_code.cache_info().hits, 1)
        self.assertEqual(production_code.cache_info().maxsize, 2048)
        self.assertNotEqual(production_code(source, exclude_proofs=False), hidden)
        self.assertNotEqual(production_code(source, mask_noncode=False), hidden)
        changed = source.replace(b"value = 1", b"value = 2")
        self.assertNotEqual(production_code(changed), hidden)
        before = production_code.cache_info().misses
        for _ in range(2):
            with self.assertRaisesRegex(ValueError, "syntax"):
                production_code(b"fn projection_cache_invalid( {")
        self.assertEqual(production_code.cache_info().misses, before + 2)

    def test_receipt_replaces_only_pristine_baseline_not_negative_or_control_queries(self):
        from run_semantic_proofs import baseline_from_whole

        whole = {
            "receipt_sha256": "1" * 64, "report_sha256": "2" * 64,
            "report": {
                "verification-results": {
                    "success": True, "is-verifying-entire-crate": True, "verified": 1, "errors": 0,
                },
                "func-details": {"jbig2_rs_production::document::finalize_pages": {}},
            },
        }
        with patch.dict("os.environ", {"JBIG2_WHOLE_BASELINE": "/receipt"}), patch(
            "run_semantic_proofs.load_whole_baseline", return_value=whole,
        ) as load, patch("proof_sensitivity.selected_proof") as verify:
            result = positive_baseline(
                TOOLS.parent, Path("/verus"), TOOLS.parent, {}, "document", "finalize_pages",
            )
            self.assertEqual(result, baseline_from_whole(
                whole, ["jbig2_rs_production::document::finalize_pages"],
            ))
            verify.assert_not_called()
            load.assert_called_once_with(TOOLS.parent, Path("/receipt"))
        for failure in (True, False):
            with patch.dict("os.environ", {"JBIG2_WHOLE_BASELINE": "/invalid"}), patch(
                "run_semantic_proofs.load_whole_baseline", side_effect=AssertionError("must not reuse"),
            ):
                report = self.report()
                report["verification-results"]["errors"] = int(failure)
                process = CompletedProcess(
                    [], int(failure), json.dumps(report),
                    "postcondition not satisfied" if failure else "",
                )
                with patch("proof_sensitivity.subprocess.run", return_value=process) as verify:
                    selected_proof(
                        Path("/verus"), TOOLS.parent, {}, failure, "document", "finalize_pages",
                    )
                verify.assert_called_once()
        with patch.dict("os.environ", {"JBIG2_WHOLE_BASELINE": "/invalid"}), patch(
            "run_semantic_proofs.load_whole_baseline", side_effect=ValueError("stale receipt"),
        ), patch("proof_sensitivity.selected_proof") as verify, self.assertRaisesRegex(ValueError, "stale"):
            positive_baseline(TOOLS.parent, Path("/verus"), TOOLS.parent, {}, "document", "finalize_pages")
        verify.assert_not_called()

    def test_multi_clause_controls_are_explicit_and_nonempty(self):
        self.assertEqual(
            weakened_contract("first second", {"controls": (("first", "one"), ("second", "two"))}),
            "one two",
        )
        with self.assertRaises(ValueError):
            weakened_contract("unchanged", {"controls": ()})

    def verify(self, report, returncode=0, stderr="", expect_failure=False, function="finalize_pages"):
        process = CompletedProcess([], returncode, json.dumps(report), stderr)
        with patch("proof_sensitivity.subprocess.run", return_value=process):
            return selected_proof(
                Path("/verus"), TOOLS.parent, {}, expect_failure, "document", function,
            )

    def report(self):
        return {
            "verification-results": {
                "is-verifying-entire-crate": False, "verified": 1, "errors": 0,
            },
            "func-details": {"jbig2_rs_production::document::finalize_pages": {}},
        }

    def test_baseline_requires_nonzero_proofs_and_exact_target(self):
        self.assertEqual(self.verify(self.report())["errors"], 0)
        for key, value in [("verified", 0), ("errors", 1), ("is-verifying-entire-crate", True)]:
            report = self.report()
            report["verification-results"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.verify(report)
        report = self.report()
        report["func-details"] = {"other::finalize_pages": {}}
        with self.assertRaises(ValueError):
            self.verify(report)

    def test_missing_scope_reports_the_actual_verifier_diagnostic(self):
        report = self.report()
        del report["verification-results"]["is-verifying-entire-crate"]
        with self.assertRaisesRegex(ValueError, "trigger diagnostic"):
            self.verify(report, 1, "trigger diagnostic")

    def test_selection_module_can_differ_from_reported_type_namespace(self):
        report = self.report()
        process = CompletedProcess([], 0, json.dumps(report), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(
                Path("/verus"), TOOLS.parent, {}, False, "document", "finalize_pages",
                verification_module="document::pages",
            )
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--verify-only-module") + 1], "document::pages")
        report["func-details"] = {"jbig2_rs_production::document::pages::finalize_pages": {}}
        with patch(
            "proof_sensitivity.subprocess.run",
            return_value=CompletedProcess([], 0, json.dumps(report), ""),
        ), self.assertRaisesRegex(ValueError, "did not target"):
            selected_proof(
                Path("/verus"), TOOLS.parent, {}, False, "document", "finalize_pages",
                verification_module="document::pages",
            )

    def test_only_expected_failure_skips_additional_error_searches(self):
        for expected_failure in (False, True):
            report = self.report()
            report["verification-results"]["errors"] = int(expected_failure)
            process = CompletedProcess(
                [], int(expected_failure), json.dumps(report),
                "postcondition not satisfied" if expected_failure else "",
            )
            with self.subTest(expected_failure=expected_failure), patch(
                "proof_sensitivity.subprocess.run", return_value=process,
            ) as run:
                selected_proof(
                    Path("/verus"), TOOLS.parent, {}, expected_failure,
                    "document", "finalize_pages",
                )
            command = run.call_args.args[0]
            self.assertEqual("--multiple-errors" in command, expected_failure)
            if expected_failure:
                self.assertEqual(command[-2:], ["--multiple-errors", "0"])
            self.assertIn("--verify-function", command)
            self.assertIn("--rlimit", command)

    def test_compiler_or_timeout_diagnostics_do_not_count_as_semantic_rejections(self):
        report = self.report()
        report["verification-results"]["errors"] = 1
        for diagnostic in ["type mismatch", "resource limit exceeded", ""]:
            with self.subTest(diagnostic=diagnostic), self.assertRaises(ValueError):
                self.verify(report, 1, diagnostic, True)

    def test_semantic_diagnostic_alone_does_not_count_without_failed_obligations(self):
        with self.assertRaises(ValueError):
            self.verify(self.report(), 1, "postcondition not satisfied", True)
        report = self.report()
        report["verification-results"]["errors"] = 1
        self.assertEqual(
            self.verify(report, 1, "loop invariant not satisfied", True)["errors"], 1,
        )

    def test_explicit_caller_assertions_require_failed_obligations_and_exact_target(self):
        report = self.report()
        report["verification-results"]["errors"] = 1
        self.assertEqual(self.verify(report, 1, "assertion failed", True)["errors"], 1)
        for invalid, code in ((self.report(), 1), (report, 0)):
            with self.subTest(report=invalid, code=code), self.assertRaises(ValueError):
                self.verify(invalid, code, "assertion failed", True)
        report["func-details"] = {"other::finalize_pages": {}}
        with self.assertRaisesRegex(ValueError, "did not target"):
            self.verify(report, 1, "assertion failed", True)

    def test_shared_positive_module_baseline_requires_every_exact_target_and_partial_proofs(self):
        functions = ("finalize_pages", "build_document_info")
        report = self.report()
        report["verification-results"]["verified"] = 2
        report["func-details"]["jbig2_rs_production::document::build_document_info"] = {}
        process = CompletedProcess([], 0, json.dumps(report), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(Path("/verus"), TOOLS.parent, {}, False, "document", functions)
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--verify-function") + 1], "*")
        self.assertEqual(command[command.index("--verify-only-module") + 1], "document")
        self.assertEqual(command[command.index("--num-threads") + 1], "1")
        self.assertEqual(command[command.index("--rlimit") + 1], "30")
        for changed in ("missing-target", "too-few-proofs", "whole-report"):
            broken = json.loads(json.dumps(report))
            if changed == "missing-target":
                del broken["func-details"]["jbig2_rs_production::document::build_document_info"]
            elif changed == "too-few-proofs":
                broken["verification-results"]["verified"] = 1
            else:
                broken["verification-results"]["is-verifying-entire-crate"] = True
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                self.verify(broken, function=functions)

    def test_shared_baseline_rejects_empty_duplicate_and_failure_groups_before_execution(self):
        for functions, failure in (((), False), (("same", "same"), False), (("one",), True)):
            with self.subTest(functions=functions, failure=failure), patch(
                "proof_sensitivity.subprocess.run",
            ) as run, self.assertRaises(ValueError):
                selected_proof(Path("/verus"), TOOLS.parent, {}, failure, "document", functions)
            run.assert_not_called()

    def test_shared_baseline_uses_narrow_common_prefix_and_keeps_individual_failures(self):
        report = self.report()
        names = ("widen_u8_i32", "widen_u8_i64")
        report["verification-results"]["verified"] = 2
        report["func-details"] = {
            f"jbig2_rs_production::symbol_text::{name}": {} for name in names
        }
        process = CompletedProcess([], 0, json.dumps(report), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(Path("/verus"), TOOLS.parent, {}, False, "symbol_text", names)
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--verify-function") + 1], "widen_u8_i*")
        for name in names:
            with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
                selected_proof(Path("/verus"), TOOLS.parent, {}, False, "symbol_text", name)
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--verify-function") + 1], name)

    def test_grouped_methods_keep_the_lexical_scope_suffix_selector(self):
        names = ("ArithmeticCodec::decode_integer", "ArithmeticCodec::decode_symbol_id")
        report = self.report()
        report["verification-results"]["verified"] = 2
        report["func-details"] = {
            f"jbig2_rs_production::symbol_text::{name}": {} for name in names
        }
        process = CompletedProcess([], 0, json.dumps(report), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(
                Path("/verus"), TOOLS.parent, {}, False, "symbol_text", names,
                verification_module="symbol_text::codec",
            )
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--verify-function") + 1], "*::ArithmeticCodec::decode_*")
        self.assertEqual(command[command.index("--verify-only-module") + 1], "symbol_text::codec")

    def test_single_method_selector_is_suffix_anchored_against_longer_method_names(self):
        report = self.report()
        report["func-details"] = {"jbig2_rs_production::bitmap::Bitmap::compose": {}}
        process = CompletedProcess([], 0, json.dumps(report), "")
        with patch("proof_sensitivity.subprocess.run", return_value=process) as run:
            selected_proof(
                Path("/verus"), TOOLS.parent, {}, False,
                "bitmap", "Bitmap::compose", verification_module="bitmap::compose",
            )
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("--verify-function") + 1], "*::Bitmap::compose")
        self.assertEqual(command[command.index("--verify-only-module") + 1], "bitmap::compose")
        report["func-details"] = {"jbig2_rs_production::bitmap::Bitmap::compose_chunked": {}}
        with patch(
            "proof_sensitivity.subprocess.run",
            return_value=CompletedProcess([], 0, json.dumps(report), ""),
        ), self.assertRaisesRegex(ValueError, "did not target"):
            selected_proof(
                Path("/verus"), TOOLS.parent, {}, False,
                "bitmap", "Bitmap::compose", verification_module="bitmap::compose",
            )

    def test_shared_baseline_runs_once_on_pristine_source_and_keeps_every_case_query(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path) / "source"
            directory = Path(path) / "proof"
            root.mkdir()
            directory.mkdir()
            original = "first second contract"
            (root / "fixture.rs").write_text(original)
            shutil.copyfile(root / "fixture.rs", directory / "fixture.rs")
            cases = tuple({
                "id": name, "file": "fixture.rs", "module": "module", "function": name,
                "site": name, "replacement": "bad_" + name, "controls": (("contract", "weak"),),
            } for name in ("first", "second"))
            calls = []

            @contextmanager
            def workspace(*args):
                yield directory, {}, "pinned", "rust"

            def verify(executable, checkout, environment, failure, module, function):
                calls.append((failure, module, function))
                if isinstance(function, tuple):
                    self.assertEqual((directory / "fixture.rs").read_text(), original)
                return {"verified": 2, "errors": int(failure)}

            groups = (("module", ("first", "second")),)
            with patch("proof_sensitivity.proof_workspace", workspace), patch(
                "proof_sensitivity.selected_proof", side_effect=verify,
            ), patch("proof_sensitivity.run"):
                result = check_semantic_cases(
                    root, Path("/verus"), cases, ("fixture.rs",), "fixture-", "scope",
                    baseline_groups=groups,
                )
            self.assertEqual(calls, [
                (False, "module", ("first", "second")),
                (True, "module", "first"), (False, "module", "first"),
                (True, "module", "second"), (False, "module", "second"),
            ])
            self.assertEqual((directory / "fixture.rs").read_text(), original)
            self.assertTrue(all(row["baseline_selected_functions"] == ["first", "second"]
                                for row in result["counterexamples"]))
            for invalid in (
                (("module", ("first", "unknown")),),
                (("module", ("first", "first")),),
                groups + groups,
            ):
                with self.subTest(invalid=invalid), patch(
                    "proof_sensitivity.proof_workspace",
                ) as workspace, self.assertRaises(ValueError):
                    check_semantic_cases(
                        root, Path("/verus"), cases, ("fixture.rs",), "fixture-", "scope",
                        baseline_groups=invalid,
                    )
                workspace.assert_not_called()


if __name__ == "__main__":
    unittest.main()
