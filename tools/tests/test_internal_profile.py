import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import check_internal_profile as checker
import check_workflows


class InternalProfileTests(unittest.TestCase):
    def test_ci_requires_original_compiler_and_profile_before_raw_artifact_upload(self):
        workflow = check_workflows.load_workflow((TOOLS.parent / ".github/workflows/ci.yml").read_text())
        check_workflows.check_native_mutation_lane(workflow)
        for name in ("Freeze approved internal declaration profile",
                     "Reconcile actual production compiler inventory"):
            changed = copy.deepcopy(workflow)
            changed["jobs"]["cargo"]["steps"] = [
                row for row in changed["jobs"]["cargo"]["steps"] if row.get("name") != name
            ]
            with self.subTest(step=name), self.assertRaisesRegex(ValueError, "pinned Linux"):
                check_workflows.check_native_mutation_lane(changed)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        for name in checker.INPUTS:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(TOOLS.parent / name, path)
        self.directory = self.root / "compiler"
        self.directory.mkdir()
        (self.directory / "report.json").write_text('{"fixture": true}\n')
        self.compiler = {
            "source_bindings": {"src/lib.rs": "fixture"},
            "artifacts": {"fixture": "bound by mocked compiler replay"},
            "counts": {"owned_callable_and_abstract_identities": 2},
            "profile": {"production_library": True},
            "public_root_exports": [{"name": "Decoder"}],
            "api_declaration_surfaces": {"categories": {"public_inherent_method_on_root_type": 1}},
            "records": [{
                "id": "document::Decoder::decode", "compiler_id": 1,
                "source_span": {"filename": "src/lib.rs", "begin": [1, 1], "end": [1, 9]},
                "compiler_has_body": True,
                "declaration_surface": {
                    "category": "public_inherent_method_on_root_type", "root_aliases": ["Decoder"],
                    "compiler_signature": {"sig": {}, "generics": {}, "header": {}},
                },
            }, {
                "id": "private", "declaration_surface": {"category": "non_root_function_or_method"},
            }],
        }
        mock = patch.object(checker, "validate_existing", return_value=self.compiler)
        self.validator = mock.start()
        self.addCleanup(mock.stop)

    def test_snapshot_preserves_all_support_rows_policies_and_compiler_signatures(self):
        report = checker.snapshot(self.root, self.directory)
        self.validator.assert_called_once_with(self.root, self.directory)
        original = json.loads((self.root / "config/support-matrix.json").read_text())
        self.assertEqual(report["approved_support_matrix"], original)
        self.assertEqual(report["api_entry_count"], 1)
        self.assertEqual(report["declared_api_entries"][0]["declaration_surface"],
                         self.compiler["records"][0]["declaration_surface"])
        self.assertFalse(report["ready_for_external_validation"])
        self.assertFalse(report["source_statement_inventory_qualified"])
        self.assertFalse(report["code_domains_and_contract_baseline_qualified"])

    def test_release_and_next_development_metadata_preserve_the_frozen_api_scope(self):
        manifest = self.root / "Cargo.toml"
        for version, publish, accepted in (
            ("0.3.0-dev", "false", True),
            ("0.2.0", '["crates-io"]', True),
            ("0.3.0-dev", '["crates-io"]', False),
            ("0.2.0", "false", False),
            ("0.4.0-dev", "false", False),
        ):
            manifest.write_text(
                f'[package]\nname = "jbig2-decoder"\nversion = "{version}"\npublish = {publish}\n',
            )
            with self.subTest(version=version, publish=publish):
                if accepted:
                    report = checker.snapshot(self.root, self.directory)
                    self.assertEqual(report["declared_api_entries"][0]["declaration_surface"],
                                     self.compiler["records"][0]["declaration_surface"])
                    self.assertFalse(report["ready_for_external_validation"])
                else:
                    with self.assertRaisesRegex(ValueError, "approved"):
                        checker.snapshot(self.root, self.directory)

    def test_replay_rejects_rehashed_flags_scope_omissions_and_signature_changes(self):
        report = checker.snapshot(self.root, self.directory)
        path = self.root / "profile.json"
        for mutate in (
            lambda value: value.update(ready_for_external_validation=True),
            lambda value: value["approved_support_matrix"]["rows"].pop(),
            lambda value: value["declared_api_entries"].clear(),
            lambda value: value.update(api_entry_count=0),
            lambda value: value["declared_api_entries"][0]["declaration_surface"].update(
                compiler_signature={"invented": True}),
        ):
            changed = copy.deepcopy(report)
            mutate(changed)
            path.write_text(json.dumps(changed))
            with self.subTest(mutation=mutate), self.assertRaisesRegex(ValueError, "altered"):
                checker.validate_snapshot(self.root, self.directory, path)
        path.write_text(json.dumps(report))
        self.assertEqual(checker.validate_snapshot(self.root, self.directory, path), report)

    def test_duplicate_and_unclassified_feature_rows_are_rejected(self):
        path = self.root / "config/support-matrix.json"
        original = json.loads(path.read_text())
        for modify in (
            lambda matrix: matrix["rows"].append(matrix["rows"][0]),
            lambda matrix: matrix["rows"][0].update(status="omitted"),
            lambda matrix: matrix["rows"][0].update(clauses=[]),
        ):
            matrix = copy.deepcopy(original)
            modify(matrix)
            path.write_text(json.dumps(matrix))
            with self.assertRaisesRegex(ValueError, "feature scope"):
                checker.snapshot(self.root, self.directory)

    def test_unapproved_scope_publication_and_wrong_editions_are_rejected(self):
        path = self.root / "config/support-matrix.json"
        original = json.loads(path.read_text())
        for modify in (
            lambda matrix: matrix["approval"].update(status="pending"),
            lambda matrix: matrix["public_api"].update(status="pending"),
            lambda matrix: matrix["normative_baseline"].update(itu="ITU-T T.88 (02/2000)"),
        ):
            matrix = copy.deepcopy(original)
            modify(matrix)
            path.write_text(json.dumps(matrix))
            with self.assertRaisesRegex(ValueError, "approved"):
                checker.snapshot(self.root, self.directory)

    def test_scope_cut_cannot_reuse_the_existing_approval_label(self):
        path = self.root / "config/support-matrix.json"
        matrix = json.loads(path.read_text())
        matrix["rows"].pop()
        path.write_text(json.dumps(matrix))
        with self.assertRaisesRegex(ValueError, "explicit reviewed profile update"):
            checker.snapshot(self.root, self.directory)

    def test_crlf_scope_checkout_keeps_exact_raw_binding_and_same_approved_content(self):
        for name in checker.FROZEN_SCOPE_INPUTS:
            path = self.root / name
            path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        report = checker.snapshot(self.root, self.directory)
        for name, expected in checker.FROZEN_SCOPE_INPUTS.items():
            self.assertEqual(checker.scope_digest(self.root / name), expected)
            self.assertEqual(report["source_bindings"][name], checker.digest(self.root / name))
        matrix = report["approved_support_matrix"]
        matrix["rows"].pop()
        (self.root / "config/support-matrix.json").write_text(json.dumps(matrix))
        with self.assertRaisesRegex(ValueError, "reviewed profile update"):
            checker.snapshot(self.root, self.directory)

    def test_missing_or_duplicate_api_declarations_are_rejected(self):
        for modified in (
            self.compiler["records"][:1] * 2,
            self.compiler["records"][1:],
        ):
            with patch.object(checker, "validate_existing", return_value={
                **self.compiler, "records": modified,
            }), self.assertRaises(ValueError):
                checker.snapshot(self.root, self.directory)

    def test_compiler_replay_failure_never_grants_a_profile(self):
        with patch.object(checker, "validate_existing", side_effect=ValueError("stale compiler")):
            with self.assertRaisesRegex(ValueError, "stale compiler"):
                checker.snapshot(self.root, self.directory)

    def test_modified_original_compiler_receipt_invalidates_existing_snapshot(self):
        path = self.root / "profile.json"
        path.write_text(json.dumps(checker.snapshot(self.root, self.directory)))
        (self.directory / "report.json").write_text('{"fixture": false}\n')
        with self.assertRaisesRegex(ValueError, "stale"):
            checker.validate_snapshot(self.root, self.directory, path)


if __name__ == "__main__":
    unittest.main()
