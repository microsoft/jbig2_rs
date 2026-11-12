import base64
import copy
from contextlib import redirect_stderr, redirect_stdout
import hashlib
from io import StringIO
import json
import re
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from pathlib import Path

from jsonschema import ValidationError


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_traceability import check, enforce_completion, local_document_checks, policy_row_hash, runtime_inventory
from evidence import load_json, validate
from rust_tests import test_inventory as inventory_tests
import check_traceability as checker


class OwnerReviewConfirmationTests(unittest.TestCase):
    def setUp(self):
        self.root = TOOLS.parent
        self.ledger = load_json(self.root / "validation/traceability/requirements.json")
        self.ledger["owner_review_confirmation"] = copy.deepcopy(
            self.ledger["historical_owner_review_confirmation"]
        )
        self.ledger["owner_review_confirmation"]["project_source_sha256"] = (
            checker.classification_evidence(self.root, self.ledger)["project_source_sha256"]
        )
        self.ledger["owner_review_confirmation"]["requirements_sha256"] = hashlib.sha256(
            json.dumps(self.ledger["requirements"], sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        self.legacy = copy.deepcopy(self.ledger)
        self.legacy["release_scope"] = self.legacy.pop("maintenance_release_scope")
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.legacy_root = Path(temporary.name)
        (self.legacy_root / "Cargo.toml").write_text('[package]\nversion = "0.1.1"\n')
        self.development = copy.deepcopy(self.ledger)
        self.development.pop("publication_scope", None)
        self.development["development_scope"] = {
            "profile": "0.3-development", "crate_version": "0.3.0-dev",
            "maintenance_branch": "release/0.2",
            "proof_completion_issue": "https://github.com/microsoft/jbig2_rs/issues/135",
            "publication_enabled": False,
        }
        self.development_root = self.legacy_root / "development"
        self.development_root.mkdir()
        (self.development_root / "Cargo.toml").write_text(
            '[package]\nversion = "0.3.0-dev"\npublish = false\n',
        )

    def test_owner_confirmation_is_bound_without_approving_missing_guarantees(self):
        confirmation = checker.check_owner_review_confirmation(self.root, self.ledger)
        self.assertTrue(confirmation["normative_model_review_validated"])
        self.assertFalse(confirmation["exhaustive_requirement_coverage"])
        self.assertFalse(confirmation["complete_functional_proof"])
        self.assertFalse(confirmation["independent_certificate"])
        self.assertEqual(len(self.ledger["requirements"]), 266)
        self.assertEqual(sum(bool(row["gaps"]) for row in self.ledger["requirements"]), 257)
        confirmation["instruction"] = "local copy mutation"
        self.assertNotEqual(
            confirmation["instruction"], self.ledger["owner_review_confirmation"]["instruction"],
        )

    def test_historical_review_is_preserved_but_not_renewed_for_new_proofs(self):
        actual = load_json(self.root / "validation/traceability/requirements.json")
        self.assertNotIn("owner_review_confirmation", actual)
        self.assertIsNone(checker.check_owner_review_confirmation(self.root, actual))
        historical = actual["historical_owner_review_confirmation"]
        self.assertEqual(historical["source_revision"], "33c4b3e267a3b6aa9bec0833ecca6bc468bc1c44")
        self.assertEqual(historical["confirmed_at"], "2026-11-02T09:48:42.675+00:00")
        actual["owner_review_confirmation"] = copy.deepcopy(historical)
        with self.assertRaisesRegex(ValueError, "snapshot"):
            checker.check_owner_review_confirmation(self.root, actual)

    def test_source_or_requirement_changes_invalidate_confirmation_match(self):
        for mutation in ("source", "requirements", "gap_removed"):
            document = copy.deepcopy(self.ledger)
            if mutation == "source":
                document["owner_review_confirmation"]["project_source_sha256"] = "0" * 64
            elif mutation == "requirements":
                document["requirements"][0]["review"] = "approved"
            else:
                row = next(row for row in document["requirements"] if row["gaps"])
                row["gaps"] = []
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, "snapshot"):
                checker.check_owner_review_confirmation(self.root, document)

    def test_confirmation_timestamp_and_scope_are_not_success_shaped_defaults(self):
        for timestamp in ("", "not-a-date", "2026-11-02T09:48:42", True):
            document = copy.deepcopy(self.ledger)
            document["owner_review_confirmation"]["confirmed_at"] = timestamp
            with self.subTest(timestamp=timestamp), self.assertRaisesRegex(ValueError, "timezone"):
                checker.check_owner_review_confirmation(self.root, document)
        for field in ("exhaustive_requirement_coverage", "complete_functional_proof",
                      "independent_certificate"):
            document = copy.deepcopy(self.ledger)
            document["owner_review_confirmation"][field] = True
            with self.subTest(field=field), self.assertRaises(ValidationError):
                validate(document, "traceability")
        document = copy.deepcopy(self.ledger)
        del document["owner_review_confirmation"]
        self.assertIsNone(checker.check_owner_review_confirmation(self.root, document))

    def scoped_report(self):
        return {
            "verifier_report_checked": True, "cargo_test_discovery_checked": True,
            "owner_review_confirmation": self.ledger["owner_review_confirmation"],
            "decomposition_review_complete": False, "semantic_claims_complete": False,
        }

    def test_v01_scope_accepts_deferred_coverage_without_passing_stronger_gates(self):
        report = self.scoped_report()
        scoped = checker.check_v01_scope(self.legacy_root, self.legacy, report)
        self.assertEqual(scoped["crate_version"], "0.1.1")
        self.assertTrue(scoped["requirement_coverage_policy_satisfied"])
        self.assertFalse(scoped["exhaustive_requirement_coverage"])
        self.assertFalse(scoped["complete_functional_proof"])
        self.assertFalse(scoped["publication_performed"])
        self.assertTrue(scoped["remaining_quality_and_release_checks_required"])
        for flags in ({"require_decomposition": True}, {"require_complete": True}):
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                enforce_completion(report, **flags)

    def test_v01_scope_still_requires_real_proof_discovery_owner_and_version(self):
        for field in ("verifier_report_checked", "cargo_test_discovery_checked",
                      "owner_review_confirmation"):
            report = self.scoped_report()
            report[field] = None
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "evidence"):
                checker.check_v01_scope(self.legacy_root, self.legacy, report)
        document = copy.deepcopy(self.legacy)
        del document["release_scope"]
        with self.assertRaisesRegex(ValueError, "owner"):
            checker.check_v01_scope(self.legacy_root, document, self.scoped_report())
        with patch.object(checker.tomllib, "loads", return_value={"package": {"version": "0.2.0"}}):
            with self.assertRaisesRegex(ValueError, "version"):
                checker.check_v01_scope(self.legacy_root, self.legacy, self.scoped_report())

    def test_v01_scope_cannot_disable_existing_gates_or_claim_full_conformance(self):
        for field, value in (
            ("existing_proof_and_quality_gates_required", False),
            ("full_spec_conformance_claim", True),
            ("complete_functional_proof_required", True),
            ("exhaustive_requirement_coverage_required", True),
            ("crate_version", "0.2.0"),
        ):
            document = copy.deepcopy(self.legacy)
            document["release_scope"][field] = value
            with self.subTest(field=field), self.assertRaises(ValidationError):
                validate(document, "traceability")
        document = copy.deepcopy(self.legacy)
        document["release_scope"]["approved_at"] = "2026-11-02"
        with self.assertRaisesRegex(ValueError, "timezone"):
            checker.check_v01_scope(self.legacy_root, document, self.scoped_report())

    def test_development_scope_requires_proof_discovery_and_disabled_publication(self):
        report = self.scoped_report()
        report["owner_review_confirmation"] = None
        scoped = checker.check_development_scope(self.development_root, self.development, report)
        self.assertEqual(scoped["crate_version"], "0.3.0-dev")
        self.assertFalse(scoped["publication_enabled"])
        self.assertFalse(scoped["complete_functional_proof"])
        self.assertFalse(scoped["exhaustive_requirement_coverage"])
        self.assertFalse(scoped["publication_performed"])
        for field in ("verifier_report_checked", "cargo_test_discovery_checked"):
            altered = {**report, field: False}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "evidence"):
                checker.check_development_scope(self.development_root, self.development, altered)
        for package in ({"version": "0.1.2", "publish": False},
                        {"version": "0.3.0-dev", "publish": ["crates-io"]},
                        {"version": "0.3.0-dev"}):
            with (
                self.subTest(package=package),
                patch.object(checker.tomllib, "loads", return_value={"package": package}),
                self.assertRaises(ValueError),
            ):
                checker.check_development_scope(self.development_root, self.development, report)
        for flags in ({"require_decomposition": True}, {"require_complete": True}):
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                enforce_completion(report, **flags)

    def test_development_scope_schema_rejects_release_or_claim_overrides(self):
        for field, value in (("crate_version", "0.2.0"), ("publication_enabled", True),
                             ("maintenance_branch", "main"), ("profile", "0.1-partial-verus"),
                             ("proof_completion_issue", "https://example.com/waiver")):
            ledger = copy.deepcopy(self.development)
            ledger["development_scope"][field] = value
            with self.subTest(field=field), self.assertRaises(ValidationError):
                validate(ledger, "traceability")
        with self.assertRaisesRegex(ValueError, "owner"):
            checker.check_v01_scope(self.root, self.ledger, self.scoped_report())

    def test_publication_scope_is_explicit_and_retains_all_quality_requirements(self):
        ledger = copy.deepcopy(self.ledger)
        ledger.pop("development_scope", None)
        ledger["publication_scope"] = {
            "profile": "0.2-partial-verus", "crate_version": "0.2.0",
            "release_branch": "release/0.2", "authority": "repository_owner_release_scope",
            "approved_at": "2026-11-12T20:19:28.161+00:00",
            "instruction": "Publish the 0.2.0 crate with the existing partial coverage boundaries.",
            "decision_reference": "agent-host-session://ahp-session/0940700e-faa1-4257-a786-58cf3bd4a573",
            "exhaustive_requirement_coverage_required": False,
            "complete_functional_proof_required": False,
            "existing_proof_and_quality_gates_required": True,
            "full_spec_conformance_claim": False,
        }
        root = self.legacy_root / "publication"
        root.mkdir()
        (root / "Cargo.toml").write_text(
            '[package]\nversion = "0.2.0"\npublish = ["crates-io"]\n',
        )
        validate(ledger, "traceability")
        report = self.scoped_report()
        scoped = checker.check_current_scope(root, ledger, report)
        self.assertFalse(scoped["complete_functional_proof"])
        self.assertFalse(scoped["publication_performed"])
        for field in ("verifier_report_checked", "cargo_test_discovery_checked"):
            altered = {**report, field: False}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "evidence"):
                checker.check_current_scope(root, ledger, altered)
        for package in ({"version": "0.3.0-dev", "publish": False},
                        {"version": "0.2.0", "publish": False}):
            with (
                self.subTest(package=package),
                patch.object(checker.tomllib, "loads", return_value={"package": package}),
                self.assertRaisesRegex(ValueError, "approved scope"),
            ):
                checker.check_current_scope(root, ledger, report)
        unqualified = copy.deepcopy(ledger)
        unqualified["publication_scope"]["approved_at"] = "2026-11-12"
        with self.assertRaisesRegex(ValueError, "timezone"):
            checker.check_current_scope(root, unqualified, report)
        for field, value in (("existing_proof_and_quality_gates_required", False),
                             ("complete_functional_proof_required", True),
                             ("full_spec_conformance_claim", True),
                             ("release_branch", "main"), ("crate_version", "0.3.0")):
            altered = copy.deepcopy(ledger)
            altered["publication_scope"][field] = value
            with self.subTest(field=field), self.assertRaises(ValidationError):
                validate(altered, "traceability")
        ledger["development_scope"] = self.development["development_scope"]
        with self.assertRaisesRegex(ValueError, "exactly one"):
            checker.check_current_scope(root, ledger, report)
        ledger.pop("publication_scope")
        ledger.pop("development_scope")
        with self.assertRaisesRegex(ValueError, "exactly one"):
            checker.check_current_scope(root, ledger, report)


class SourceReviewDispositionTests(unittest.TestCase):
    def setUp(self):
        self.revision = "a" * 40
        self.pin = "c" * 64
        self.authority = {"enabled": True, "human_user_ids": [22]}
        self.pull = {
            "number": 4, "state": "open", "user": {"id": 11},
            "base": {"ref": "main", "sha": "b" * 40, "repo": {"full_name": checker.REVIEW_REPOSITORY}},
            "head": {"sha": self.revision, "repo": {"full_name": checker.REVIEW_REPOSITORY}},
        }
        self.review = {
            "id": 41, "state": "APPROVED", "commit_id": self.revision,
            "user": {"id": 22, "type": "User"},
            "body": f"JBIG2-SOURCE-REVIEW-SHA256: {self.pin}",
            "submitted_at": "2026-10-27T18:00:00Z",
        }
        self.responses = {
            "": {"full_name": checker.REVIEW_REPOSITORY, "default_branch": "main"},
            "git/ref/heads/main": {
                "ref": "refs/heads/main", "object": {"sha": "b" * 40, "type": "commit"},
            },
            "pulls/4": self.pull,
            "pulls/4/reviews?per_page=100&page=1": [self.review],
        }
        self.calls = []

    def fetch(self, endpoint):
        self.calls.append(endpoint)
        if endpoint == f"contents/validation/source-provenance.json?ref={'b' * 40}":
            return {
                "type": "file", "encoding": "base64",
                "content": base64.b64encode(json.dumps({
                    "policy": {"source_review_authority": self.authority},
                }).encode("utf-8")).decode("ascii"),
            }
        return copy.deepcopy(self.responses[endpoint])

    def validate(self, **overrides):
        return checker.source_review_disposition(
            **({"pr_number": 4, "review_id": 41, "revision": self.revision,
                "packet_pin": self.pin, "fetch": self.fetch} | overrides),
        )

    def test_authenticates_only_source_disposition_from_default_branch_authority(self):
        report = self.validate()
        self.assertEqual(report["status"], "authenticated_source_snapshot_disposition")
        self.assertEqual(report["authority_revision"], "b" * 40)
        self.assertEqual(report["human_user_id"], 22)
        self.assertFalse(report["normative_approval"])
        self.assertFalse(report["document_processing_authorized"])
        self.assertIn(f"contents/validation/source-provenance.json?ref={'b' * 40}", self.calls)
        self.assertNotIn(f"contents/validation/source-provenance.json?ref={self.revision}", self.calls)

    def test_authority_is_absent_disabled_or_malformed_by_default(self):
        for authority in (
            None, {}, {"enabled": False, "human_user_ids": [22]},
            {"enabled": True, "human_user_ids": []},
            {"enabled": True, "human_user_ids": [22, 22]},
            {"enabled": True, "human_user_ids": ["22"]},
            {"enabled": True, "human_user_ids": [True]},
            {"enabled": True, "human_user_ids": [0]},
            {"enabled": True, "human_user_ids": [22], "normative_approval": True},
        ):
            with self.subTest(authority=authority):
                self.authority = authority
                with self.assertRaisesRegex(ValueError, "authority is not configured"):
                    self.validate()

    def test_rejects_bad_explicit_ids_and_source_pins_before_network(self):
        for change in (
            {"pr_number": 0}, {"pr_number": True}, {"review_id": -1},
            {"revision": None}, {"revision": "A" * 40},
            {"packet_pin": "c" * 63}, {"packet_pin": True},
        ):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "pinned source"):
                self.validate(**change)
        self.assertEqual(self.calls, [])

    def test_rejects_other_pr_branch_repository_or_revision(self):
        original = copy.deepcopy(self.pull)
        for path, replacement in (
            (("state",), "closed"), (("number",), 5),
            (("base", "ref"), "topic"), (("base", "sha"), "d" * 40),
            (("base", "repo", "full_name"), "other/repository"),
            (("head", "repo", "full_name"), "other/repository"),
            (("head", "sha"), "d" * 40),
        ):
            self.pull.clear()
            self.pull.update(copy.deepcopy(original))
            target = self.pull
            for name in path[:-1]:
                target = target[name]
            target[path[-1]] = replacement
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "identity differs"):
                self.validate()

    def test_current_default_branch_revocation_cannot_use_an_old_base(self):
        self.responses["git/ref/heads/main"]["object"]["sha"] = "d" * 40
        with self.assertRaisesRegex(ValueError, "identity differs"):
            self.validate()
        self.assertFalse(any(path.startswith("contents/") for path in self.calls))

    def test_rejects_nonhuman_self_authored_stale_or_nonexplicit_dispositions(self):
        original = copy.deepcopy(self.review)
        for change in (
            {"state": "COMMENTED"}, {"state": "DISMISSED"}, {"state": "CHANGES_REQUESTED"},
            {"commit_id": "d" * 40}, {"user": {"id": 23, "type": "User"}},
            {"user": {"id": 22, "type": "Bot"}}, {"user": {"id": True, "type": "User"}},
            {"body": None}, {"body": f"> {original['body']}"},
            {"body": f"Not a disposition:\n{original['body']}"},
            {"body": f"```\n{original['body']}\n```"}, {"body": original["body"].replace("c", "d")},
        ):
            self.review.clear()
            self.review.update(copy.deepcopy(original) | change)
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "stale, unauthorized"):
                self.validate()
        self.review.clear()
        self.review.update(original)
        self.pull["user"]["id"] = 22
        with self.assertRaisesRegex(ValueError, "self-authored"):
            self.validate()

    def test_submission_time_not_creation_id_controls_superseding_reviews(self):
        for state in ("APPROVED", "CHANGES_REQUESTED", "DISMISSED"):
            later = copy.deepcopy(self.review) | {
                "id": 40, "state": state, "submitted_at": "2026-10-27T18:00:01Z",
            }
            self.responses["pulls/4/reviews?per_page=100&page=1"] = [later, self.review]
            with self.subTest(state=state), self.assertRaisesRegex(ValueError, "superseded"):
                self.validate()
        later["submitted_at"] = self.review["submitted_at"]
        with self.assertRaisesRegex(ValueError, "superseded"):
            self.validate()

    def test_old_decisions_and_new_comments_do_not_supersede_explicit_approval(self):
        self.responses["pulls/4/reviews?per_page=100&page=1"] += [
            copy.deepcopy(self.review) | {
                "id": 42, "state": "CHANGES_REQUESTED", "submitted_at": "2026-10-27T17:59:59Z",
            },
            copy.deepcopy(self.review) | {
                "id": 43, "state": "COMMENTED", "submitted_at": "2026-10-27T18:01:00Z",
            },
        ]
        self.assertEqual(self.validate()["review_id"], 41)

    def test_rejects_missing_duplicate_or_malformed_review_evidence(self):
        for rows in (
            [], [self.review, self.review], [self.review | {"id": True}],
            [self.review | {"submitted_at": None}],
            [self.review | {"submitted_at": "2026-02-31T18:00:00Z"}],
            [self.review | {"submitted_at": "2026-10-27T18:00:00"}],
        ):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                self.responses["pulls/4/reviews?per_page=100&page=1"] = rows
                self.validate()

    def test_reads_paginated_history_and_fails_closed_at_bound(self):
        first = [self.review | {"id": index, "state": "COMMENTED"} for index in range(100, 200)]
        self.responses["pulls/4/reviews?per_page=100&page=1"] = first
        self.responses["pulls/4/reviews?per_page=100&page=2"] = [self.review]
        self.assertEqual(self.validate()["review_id"], 41)
        self.assertIn("pulls/4/reviews?per_page=100&page=2", self.calls)
        for page in range(1, 21):
            self.responses[f"pulls/4/reviews?per_page=100&page={page}"] = [
                self.review | {"id": 100 + (page - 1) * 100 + index, "state": "COMMENTED"}
                for index in range(100)
            ]
        self.responses["pulls/4/reviews?per_page=100&page=20"][-1] = self.review
        self.responses["pulls/4/reviews?per_page=100&page=21"] = []
        self.assertEqual(self.validate()["review_id"], 41)
        self.responses["pulls/4/reviews?per_page=100&page=21"] = [self.review | {"id": 3000}]
        with self.assertRaisesRegex(ValueError, "exceeds the supported bound"):
            self.validate()

    def test_github_reads_are_utf8_strict_json_noninteractive_and_bounded(self):
        for document in ('{"duplicate":1,"duplicate":2}', '{"invalid":NaN}'):
            with patch.object(checker.subprocess, "check_output", return_value=document):
                with self.assertRaises(ValueError):
                    checker.github_review_json("pulls/4")
        with patch.object(checker.subprocess, "check_output", return_value='{"value":"é"}') as process:
            self.assertEqual(checker.github_review_json(""), {"value": "é"})
        self.assertEqual(process.call_args.kwargs["encoding"], "utf-8")
        self.assertEqual(process.call_args.kwargs["timeout"], 60)
        self.assertEqual(process.call_args.kwargs["stdin"], checker.subprocess.DEVNULL)
        self.assertEqual(process.call_args.args[0][-1], f"repos/{checker.REVIEW_REPOSITORY}")

    def test_base_authority_transport_and_embedded_json_are_strict(self):
        original_fetch = self.fetch
        for content in (
            "!invalid base64!",
            base64.b64encode(b'{"policy":{},"policy":{}}').decode("ascii"),
            base64.b64encode(b'{"policy":NaN}').decode("ascii"),
        ):
            def fetch(endpoint):
                result = original_fetch(endpoint)
                if endpoint.startswith("contents/"):
                    result["content"] = content
                return result
            with self.subTest(content=content), self.assertRaises(ValueError):
                self.validate(fetch=fetch)


class TraceabilityTests(unittest.TestCase):
    def setUp(self):
        fixture_directory = TOOLS.parent / "target" / "traceability-test-fixtures"
        fixture_directory.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=fixture_directory)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.write("src/lib.rs", """
            mod leaf;
            #[cfg(verus_keep_ghost)]
            #[path = "../specs/model.rs"]
            mod spec;
        """)
        self.write("src/leaf.rs", """
            #[cfg_attr(verus_keep_ghost, verus_spec(out =>
                ensures out == crate::spec::identity(x),
            ))]
            pub fn identity(x: u8) -> u8 { x }
            #[cfg(test)]
            mod tests {
                #[test] fn identity_returns_input() {}
                #[test] #[ignore] fn ignored_probe() {}
                #[cfg(verus_keep_ghost)]
                #[test] fn ghost_only() {}
            }
            #[cfg_attr(verus_keep_ghost, verus_verify)]
            fn after_tests() {}
        """)
        self.write("specs/model.rs", """
            verus! {
                pub open spec fn identity(x: u8) -> u8 { x }
            }
        """)
        self.write_json("config/support-matrix.json", {"rows": [{"feature": "decoder", "status": "required"}]})
        self.standard = {"id": "ITU-T T.88 (08/2018)", "edition": "2.0", "document_sha256": "a" * 64}
        self.consultation = {
            "standard": self.standard["id"],
            "acquisition_kind": "official_package",
            "acquisition_url": "https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.88-201808-I!!SOFT-ZST-E&type=items",
            "package_sha256": "b" * 64, "document_sha256": self.standard["document_sha256"],
            "clause_families": ["5.4 (synthetic fixture)"],
        }
        self.write_json("validation/source-provenance.json", {"normative_documents": [self.standard | {
            "package_sha256": "b" * 64, "source_url": "https://www.itu.int/rec/T-REC-T.88-201808-I/en",
        }]})
        runtime = runtime_inventory(self.root)
        coverage = {
            "schema_version": 1, "source": "src/*.rs", "verus_verified_obligations": 2,
            "native_assumptions": [],
            "counts": {
                "verified": 2, "incomplete": 0, "native-assumption": 0,
                "audited-forwarder": 0, "reviewed-verifier-limitation": 0,
            },
            "functions": [{
                "id": name, "file": entry["file"], "file_sha256": entry["file_sha256"],
                "visibility": entry["visibility"], "status": "verified",
                "disposition_reason": None,
                "verus_functions": [f"jbig2_rs_production::leaf::{entry['name']}"],
            } for name, entry in runtime.items()],
        }
        self.write_json("verification/production-proof-coverage.json", coverage)
        self.ledger = {
            "schema_version": 1, "review_policy": "provisional fixture",
            "standards": [self.standard],
            "source_consultations": [self.consultation],
            "features": [{
                "feature": "decoder", "support_status": "required", "decomposition": "partial",
                "requirement_ids": ["T88-FIXTURE"],
                "remaining_scope": ["Synthetic fixture is not a full normative decomposition."],
            }],
            "definitions": {
                "identity-model": {
                    "kind": "spec", "file": "specs/model.rs", "module_path": ["spec"],
                    "name": "identity", "role": "output model",
                },
            },
            "bindings": {
                path: hashlib.sha256((self.root / path).read_bytes()).hexdigest()
                for path in ("src/lib.rs", "src/leaf.rs", "specs/model.rs")
            },
            "requirements": [{
                "id": "T88-FIXTURE", "standard": self.standard["id"], "clause": "5.4",
                "title": "synthetic identity fixture", "feature": "decoder",
                "runtime": ["leaf::identity"], "models": ["identity-model"], "proof_macros": [],
                "tests": [{"target": "lib", "name": "leaf::tests::identity_returns_input"}],
                "strength": "functional_contract", "review": "provisional",
                "scope_of_formal_result": "fixture only", "assumptions": ["fixture translation"],
                "gaps": [], "invariant_evidence": [],
                "functional_evidence": [{
                    "runtime": "leaf::identity", "section": "ensures",
                    "tokens": ["out == crate::spec::identity(x)"],
                }],
            }],
        }
        self.names = {
            "jbig2_rs_production::spec::identity",
            "jbig2_rs_production::leaf::identity",
        }
        self.discovered = {("lib", "leaf::tests::identity_returns_input")}
        self.classification_fixture()

    def classification_fixture(self):
        # Give compact synthetic declarations an unambiguous standalone anchor.
        for directory in ("specs", "proofs"):
            for path in (self.root / directory).rglob("*.rs"):
                text = path.read_text(encoding="utf-8")
                text = re.sub(r"(?<=[{;}]) *(?=pub open spec fn|pub proof fn)", "\n", text)
                path.write_text(text, encoding="utf-8")
        declarations, _ = checker.audit_declarations(self.root, self.ledger)
        families = []
        for file in sorted({key[0] for key in declarations}):
            families.append({
                "file": file,
                "members": [f"{kind}::{name}" for path, kind, name in sorted(declarations) if path == file],
                "classification": "classification-pending",
                "justification": "Synthetic exact-membership fixture.",
                "ambiguity": "Body-purpose review pending.",
            })
        self.ledger["model_classifications"] = {
            "review": "author-navigation-only-not-independent-normative-approval",
            **checker.classification_evidence(self.root, self.ledger),
            "families": families,
        }
        self.inline_fixture()

    def inline_fixture(self):
        declarations, registered = checker.audit_declarations(self.root, self.ledger)
        classifications = {
            (family["file"], *member.split("::")): family["classification"]
            for family in self.ledger["model_classifications"]["families"]
            for member in family["members"]
        }
        for file in sorted({key[0] for key in declarations}):
            path = self.root / file
            source = path.read_bytes()
            source = re.sub(
                rb"(?m)^// Model navigation:.*\n// Purpose: Synthetic.*\n"
                rb"(?:// Audit:.*\n)?// (?:REQUIREMENT CONTEXT|Mapping):.*\n"
                rb"// Fidelity:.*\n", b"", source,
            )
            insertions = []
            for (kind, name), offset in checker.navigation_declarations(source).items():
                key = (file, kind, name)
                if key not in declarations:
                    continue
                context = checker.navigation_context(self.ledger, registered.get(key))
                lines = [
                    f"// Model navigation: {kind}::{name} [{classifications[key]}]",
                    "// Purpose: Synthetic source-only navigation fixture.",
                ]
                if key in registered:
                    lines.append(f"// Audit: {registered[key]}")
                lines.append(
                    f"// REQUIREMENT CONTEXT: {context['id']} | {context['standard']} | {context['clause']}"
                    if context else "// Mapping: no registered requirement/locator mapping."
                )
                lines.append("// Fidelity: unresolved; no independent normative approval.")
                insertions.append((offset, ("\n".join(lines) + "\n").encode()))
            for offset, block in reversed(insertions):
                source = source[:offset] + block + source[offset:]
            path.write_bytes(source)
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        for file in self.ledger["bindings"]:
            self.ledger["bindings"][file] = hashlib.sha256((self.root / file).read_bytes()).hexdigest()

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write_json(self, relative, document):
        self.write(relative, json.dumps(document))

    def run_check(self, ledger=None, names=None, discovered=None):
        return check(
            ledger or self.ledger, self.root,
            self.names if names is None else names,
            self.discovered if discovered is None else discovered,
        )

    def with_reference_standard(self):
        ledger = copy.deepcopy(self.ledger)
        reference = {
            "id": "ITU-T T.6 (1988)", "document_sha256": "c" * 64,
            "source_url": "https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.6-198811-I!!PDF-E&type=items",
        }
        provenance = load_json(self.root / "validation/source-provenance.json")
        provenance["normative_references"] = [reference]
        self.write_json("validation/source-provenance.json", provenance)
        ledger["standards"].append({
            "id": reference["id"], "edition": None,
            "provenance_collection": "normative_references",
            "document_sha256": reference["document_sha256"],
        })
        ledger["source_consultations"].append({
            "standard": reference["id"], "acquisition_kind": "official_pdf",
            "acquisition_url": reference["source_url"],
            "document_sha256": reference["document_sha256"], "clause_families": ["2.2.1"],
        })
        requirement = copy.deepcopy(ledger["requirements"][0])
        requirement.update(id="T6-FIXTURE", standard=reference["id"], clause="2.2.1")
        ledger["requirements"].append(requirement)
        ledger["features"][0]["requirement_ids"].append(requirement["id"])
        return ledger

    def test_links_are_valid_without_claiming_scope_or_owner_approval(self):
        validate(self.ledger, "traceability")
        report = self.run_check()
        self.assertEqual(report["requirement_count"], 1)
        self.assertFalse(report["normative_decomposition_complete"])
        self.assertEqual(report["status"], "references_valid_scope_incomplete")
        self.assertTrue(report["verifier_report_checked"])
        self.assertTrue(report["cargo_test_discovery_checked"])
        feature = report["feature_decomposition"][0]
        self.assertEqual(feature["requirement_ids"], ["T88-FIXTURE"])
        self.assertEqual(feature["requirement_count"], 1)
        self.assertEqual(feature["evidence_strength_counts"]["functional_contract"], 1)
        self.assertTrue(feature["remaining_scope"])

    def packet_fixture(self):
        relative = "validation/traceability/requirements.json"
        self.write_json(relative, self.ledger)
        self.write_json("config/toolchain.json", {"fixture": "pinned"})
        self.write("tools/check_traceability.py", "fixture verifier identity\n")
        self.write_json("tools/evidence.schema.json", {"fixture": "schema"})
        for name in ("Cargo.toml", "Cargo.lock", "verification/Cargo.toml", "verification/Cargo.lock"):
            self.write(name, "fixture Cargo identity\n")
        packet = checker.audit_packet(self.root, self.ledger, ["T88-FIXTURE"], relative)
        return packet, relative

    def test_inventory_exposes_unclassified_and_unregistered_models_without_approval(self):
        self.write("specs/extra.rs", """
            verus! {
                // pub open spec fn not_a_declaration() -> bool { true }
                pub open spec fn undiscovered() -> bool { super::EXAMPLE_TABLE.len() > 0 }
            }
        """)
        self.classification_fixture()
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertEqual(report["registered_definition_count"], 1)
        self.assertEqual(report["registered_spec_count"], 1)
        self.assertEqual(report["definitions"][0]["classification"], "classification-pending")
        self.assertEqual(report["definitions"][0]["requirements"][0]["clause"], "5.4")
        self.assertEqual([
            {key: row[key] for key in ("file", "name")}
            for row in report["unregistered_spec_functions"]
        ], [
            {"file": "specs/extra.rs", "name": "undiscovered"},
        ])
        self.assertEqual(report["implementation_constant_references"], {
            "specs/extra.rs": ["EXAMPLE_TABLE"],
        })
        self.assertFalse(report["classification_complete"])
        self.assertFalse(report["normative_approval"])

    def test_inline_audit_references_bind_existing_physical_definition_files(self):
        source = (self.root / "specs/model.rs").read_text()
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertTrue(report["definitions"][0]["inline_audit_reference"])
        for path, annotation in (
            ("specs/model.rs", "// Audit: missing\n"),
            ("specs/model.rs", "// Audit: identity-model\n// Audit: identity-model\n"),
            ("specs/extra.rs", "// Audit: identity-model\n"),
        ):
            self.write("specs/model.rs", source)
            self.write(path, annotation + source)
            self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
            with self.subTest(path=path, annotation=annotation), self.assertRaises(ValueError):
                checker.audit_inventory(self.root, self.ledger)
            if path != "specs/model.rs":
                (self.root / path).unlink()

    def test_inline_navigation_rejects_missing_duplicate_unknown_and_hidden_markers(self):
        path = self.root / "specs/model.rs"
        original = path.read_text()
        marker = "// Model navigation: spec::identity [classification-pending]"
        for changed in (
            original.replace(marker + "\n", ""),
            original.replace(marker, marker + "\n" + marker),
            original.replace("spec::identity [", "spec::unknown ["),
            original.replace("// Audit: identity-model\n", ""),
            original.replace("// Audit: identity-model", "// Audit: unknown"),
            original.replace(marker, 'const HIDDEN: &str = r#"\n' + marker + '\n"#;'),
            original.replace("// Audit: identity-model", 'const HIDDEN: &str = r#"\n// Audit: identity-model\n"#;'),
            original.replace("// REQUIREMENT CONTEXT: T88-FIXTURE", "// REQUIREMENT CONTEXT: T88-UNKNOWN"),
            original.replace("| 5.4", "| 5.5"),
            original.replace("// Purpose: Synthetic source-only navigation fixture.\n", ""),
            original.replace("// Fidelity: unresolved; no independent normative approval.\n", ""),
        ):
            path.write_text(changed)
            self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                checker.audit_inventory(self.root, self.ledger)

    def test_inline_navigation_rejects_same_file_misplacement_and_wrong_support_file(self):
        path = self.root / "specs/model.rs"
        original = path.read_text()
        marker = "// Audit: identity-model"
        for file in ("specs/model.rs", "proofs/shared/unregistered.rs", "src/leaf.rs"):
            path.write_text(original.replace(marker + "\n", ""))
            destination = self.root / file
            old = destination.read_text() if destination.exists() else ""
            self.write(file, old + "\n" + marker + "\nverus! {\npub open spec fn unrelated() -> bool { true }\n}")
            self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
            with self.subTest(file=file), self.assertRaises(ValueError):
                checker.audit_inventory(self.root, self.ledger)
            if file != "specs/model.rs":
                if old:
                    destination.write_text(old)
                else:
                    destination.unlink()

    def test_inline_navigation_covers_registered_proof_support_and_growth_without_fixed_total(self):
        self.write("proofs/shared/support.rs", """
            verus! {
                pub open spec fn support() -> bool { true }
                pub proof fn witness() {}
            }
        """)
        self.write("specs/model.rs", (self.root / "specs/model.rs").read_text()
                   + '\ninclude!("../proofs/shared/support.rs");\n')
        for kind, name in (("spec", "support"), ("proof", "witness")):
            self.ledger["definitions"][f"{name}-model"] = {
                "kind": kind, "file": "proofs/shared/support.rs", "module_path": ["spec"],
                "name": name, "role": "proof support",
            }
        self.classification_fixture()
        report = checker.audit_inventory(self.root, self.ledger)["inline_navigation"]
        self.assertEqual(report["declared_count"], 3)
        self.assertEqual(report["covered_count"], 3)
        self.assertEqual(report["uncovered_count"], 0)
        self.assertTrue(report["structural_coverage_complete"])
        self.assertFalse(report["independently_verified_citations"])
        self.assertFalse(report["document_processing_authorized"])
        self.assertFalse(report["normative_approval"])
        path = self.root / "proofs/shared/support.rs"
        source = path.read_text()
        path.write_text(source.replace("// Audit: witness-model\n", ""))
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "missing inline navigation"):
            checker.audit_inventory(self.root, self.ledger)
        path.write_text(source)
        self.write("proofs/model_support/new.rs", "verus! {\npub open spec fn added() -> bool { true }\n}")
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "membership differs"):
            checker.audit_inventory(self.root, self.ledger)
        self.classification_fixture()
        self.assertEqual(checker.audit_inventory(self.root, self.ledger)["inline_navigation"]["covered_count"], 4)

    def test_ambiguous_inline_placement_fails_instead_of_granting_coverage(self):
        path = self.root / "specs/model.rs"
        source = path.read_text()
        self.write("specs/model.rs", source.replace(
            "pub open spec fn identity", "struct Intervening;\npub open spec fn identity",
        ))
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "ambiguous or misplaced"):
            checker.audit_inventory(self.root, self.ledger)

    def test_same_named_declaration_cannot_inherit_a_different_owner(self):
        path = self.root / "specs/model.rs"
        original = path.read_text()
        for owner in ("mod wrong", "impl Wrong"):
            path.write_text(original.replace("verus! {", f"verus! {{\n{owner} {{") + "\n}\n")
            self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
            with self.subTest(owner=owner), self.assertRaisesRegex(ValueError, "unsupported inline navigation owner"):
                checker.audit_inventory(self.root, self.ledger)

    def test_cfg_excluded_comment_cannot_supply_a_production_reference(self):
        self.write("proofs/shared/support.rs", "macro_rules! helper { () => {} }\n")
        self.write("specs/model.rs", (self.root / "specs/model.rs").read_text()
                   + '\ninclude!("../proofs/shared/support.rs");\n')
        self.ledger["definitions"]["helper-model"] = {
            "kind": "macro", "file": "proofs/shared/support.rs", "module_path": ["spec"],
            "name": "helper", "role": "proof support",
        }
        self.classification_fixture()
        self.assertTrue(checker.audit_inventory(self.root, self.ledger)["inline_navigation"]["structural_coverage_complete"])
        path = self.root / "proofs/shared/support.rs"
        path.write_text(
            "#[cfg(test)]\nfn excluded() {\n// Audit: helper-model\n}\n"
            + path.read_text().replace("// Audit: helper-model\n", "")
        )
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "missing inline navigation"):
            checker.audit_inventory(self.root, self.ledger)

    def test_unscoped_associated_names_do_not_make_navigation_ambiguous(self):
        self.write("proofs/shared/unregistered.rs", """
            verus! {
                impl A {
                    spec fn unscoped() -> bool { true }
                }
                impl B {
                    spec fn unscoped() -> bool { false }
                }
            }
        """)
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        self.assertEqual(checker.audit_inventory(self.root, self.ledger)["inline_navigation"]["covered_count"], 1)

    def test_unicode_comments_and_literal_decoys_do_not_shift_navigation_identity(self):
        path = self.root / "specs/model.rs"
        original = path.read_text()
        path.write_text(
            '// Source-only notation: α → β.\nconst DECOY: &str = r#"\n'
            '// Audit: unknown\n// Model navigation: spec::phantom [proof-support]\n"#;\n'
            + original.replace("// Purpose: Synthetic", "// Purpose: α → β; Synthetic"),
            encoding="utf-8",
        )
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        self.assertTrue(checker.audit_inventory(self.root, self.ledger)["inline_navigation"]["structural_coverage_complete"])

    def test_crlf_navigation_uses_logical_lines_without_normalizing_source_hashes(self):
        before = self.ledger["model_classifications"]["project_source_sha256"]
        for directory in ("src", "specs", "proofs"):
            for path in (self.root / directory).rglob("*.rs"):
                path.write_bytes(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        for file in self.ledger["bindings"]:
            self.ledger["bindings"][file] = hashlib.sha256((self.root / file).read_bytes()).hexdigest()
        self.assertNotEqual(before, self.ledger["model_classifications"]["project_source_sha256"])
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertTrue(report["inline_navigation"]["structural_coverage_complete"])
        self.assertEqual(report["inline_navigation"]["covered_count"], 1)
        path = self.root / "specs/model.rs"
        path.write_bytes(path.read_bytes().replace(b"// Audit: identity-model", b"// Audit: missing"))
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "audit reference"):
            checker.audit_inventory(self.root, self.ledger)

    def test_authored_purposes_preserve_registration_and_never_approve_candidates(self):
        self.write("specs/extra.rs", "verus! { pub open spec fn helper() -> bool { true } }")
        self.classification_fixture()
        families = self.ledger["model_classifications"]["families"]
        for family in families:
            family["classification"] = "candidate-standard-facing-model"
            family["ambiguity"] = ""
            family["caveats"] = "Candidate only; independent normative review unresolved."
        self.inline_fixture()
        validate(self.ledger, "traceability")
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertTrue(report["classification_complete"])
        self.assertFalse(report["normative_approval"])
        self.assertEqual(report["classification_counts"], {"candidate-standard-facing-model": 2})
        self.assertEqual(report["unresolved_declarations"], [])
        self.assertEqual(report["definitions"][0]["caveats"], families[0]["caveats"])
        self.assertEqual(report["definitions"][0]["requirements"][0]["id"], "T88-FIXTURE")
        helper = report["unregistered_spec_functions"][0]
        self.assertEqual(helper["requirements"], [])
        self.assertFalse(helper["normative_approval"])
        self.assertEqual(helper["classification_review"], "author-navigation-only-not-independent-normative-approval")

    def test_nonpending_category_cannot_hide_unresolved_purpose(self):
        family = self.ledger["model_classifications"]["families"][0]
        for purpose in checker.CLASSIFICATION_SCHEMA["properties"]["families"]["items"]["properties"]["classification"]["enum"]:
            family.update(
                classification=purpose,
                ambiguity="Purpose unresolved: could instead be a representation invariant.",
            )
            self.inline_fixture()
            with self.subTest(purpose=purpose):
                validate(self.ledger, "traceability")
                report = checker.audit_inventory(self.root, self.ledger)
                self.assertFalse(report["classification_complete"])
                self.assertEqual(report["unresolved_declarations"], [{
                    "file": "specs/model.rs", "kind": "spec", "name": "identity",
                    "ambiguity": family["ambiguity"],
                }])
                self.assertFalse(report["normative_approval"])

    def test_scope_and_normative_caveats_do_not_make_purpose_unresolved(self):
        family = self.ledger["model_classifications"]["families"][0]
        family.update(
            classification="project-result-model", ambiguity="",
            caveats="Only the identity result; full output scope and independent normative fidelity remain unreviewed.",
        )
        self.inline_fixture()
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertTrue(report["classification_complete"])
        self.assertEqual(report["unresolved_declarations"], [])
        self.assertFalse(report["normative_approval"])
        self.assertEqual(report["definitions"][0]["caveats"], family["caveats"])
        family["ambiguity"] = "Purpose unresolved despite the separate caveat."
        self.assertFalse(checker.audit_inventory(self.root, self.ledger)["classification_complete"])

    def test_unregistered_purpose_ambiguity_is_visible_without_requirement_credit(self):
        self.write("specs/extra.rs", "verus! { pub open spec fn helper() -> bool { true } }")
        self.classification_fixture()
        for family in self.ledger["model_classifications"]["families"]:
            family.update(classification="project-result-model", ambiguity="")
            if family["file"] == "specs/extra.rs":
                family["ambiguity"] = "Purpose unresolved: no caller distinguishes result from invariant."
                family["caveats"] = "Independent normative review is also pending."
        self.inline_fixture()
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertFalse(report["classification_complete"])
        self.assertEqual(report["unresolved_declarations"], [{
            "file": "specs/extra.rs", "kind": "spec", "name": "helper",
            "ambiguity": "Purpose unresolved: no caller distinguishes result from invariant.",
        }])
        helper = report["unregistered_spec_functions"][0]
        self.assertEqual(helper["requirements"], [])
        self.assertEqual(helper["caveats"], "Independent normative review is also pending.")
        self.assertFalse(helper["normative_approval"])

    def test_pending_classification_requires_visible_ambiguity_and_is_not_complete(self):
        report = checker.audit_inventory(self.root, self.ledger)
        self.assertFalse(report["classification_complete"])
        self.assertEqual(report["unresolved_declarations"], [{
            "file": "specs/model.rs", "kind": "spec", "name": "identity",
            "ambiguity": "Body-purpose review pending.",
        }])
        authored = self.ledger["model_classifications"]
        authored["families"][0]["ambiguity"] = ""
        with self.assertRaises(ValidationError):
            checker.audit_inventory(self.root, self.ledger)
        authored["families"][0]["ambiguity"] = "Unresolved."
        authored["review"] = "independently-approved"
        with self.assertRaises(ValidationError):
            checker.audit_inventory(self.root, self.ledger)

    def test_classification_membership_rejects_missing_unknown_duplicate_and_wrong_kind(self):
        original = copy.deepcopy(self.ledger["model_classifications"]["families"])
        for members in ([], ["spec::unknown"], ["proof::identity"], ["spec::identity"] * 2):
            self.ledger["model_classifications"]["families"] = copy.deepcopy(original)
            self.ledger["model_classifications"]["families"][0]["members"] = members
            with self.subTest(members=members), self.assertRaises((ValueError, ValidationError)):
                checker.audit_inventory(self.root, self.ledger)
        self.ledger["model_classifications"]["families"] = original * 2
        with self.assertRaisesRegex(ValueError, "duplicate navigation"):
            checker.audit_inventory(self.root, self.ledger)

    def test_new_and_deleted_unregistered_declarations_surface_even_with_rebound_sources(self):
        self.write("specs/extra.rs", "verus! { pub open spec fn helper() -> bool { true } }")
        self.classification_fixture()
        original = (self.root / "specs/extra.rs").read_text()
        for changed in (
            original.replace("helper", "renamed"),
            "verus! { pub open spec fn helper() -> bool { true } pub open spec fn added() -> bool { true } }",
            "verus! {}",
        ):
            self.write("specs/extra.rs", changed)
            self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
            with self.subTest(changed=changed), self.assertRaisesRegex(ValueError, "membership differs"):
                checker.audit_inventory(self.root, self.ledger)

    def test_new_file_and_deleted_registered_declaration_fail_closed(self):
        self.write("specs/new.rs", "verus! { pub open spec fn added() -> bool { true } }")
        with self.assertRaisesRegex(ValueError, "stale navigation"):
            checker.audit_inventory(self.root, self.ledger)
        self.ledger["model_classifications"].update(checker.classification_evidence(self.root, self.ledger))
        with self.assertRaisesRegex(ValueError, "membership differs"):
            checker.audit_inventory(self.root, self.ledger)
        self.write("specs/model.rs", "verus! {}")
        with self.assertRaisesRegex(ValueError, "missing or ambiguous"):
            checker.audit_inventory(self.root, self.ledger)

    def test_classifications_reject_stale_bodies_dependencies_and_registry_bindings(self):
        for name in ("specs/model.rs", "src/lib.rs", "src/leaf.rs"):
            path = self.root / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n// changed dependency\n")
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "stale navigation"):
                checker.audit_inventory(self.root, self.ledger)
            path.write_bytes(original)
        self.ledger["definitions"]["identity-model"]["role"] = "changed role"
        with self.assertRaisesRegex(ValueError, "stale navigation"):
            checker.audit_inventory(self.root, self.ledger)

    def test_duplicate_declarations_and_registry_aliases_are_not_interchangeable(self):
        self.write("specs/extra.rs", """
            verus! {
                pub open spec fn repeated() -> bool { true }
                pub open spec fn repeated() -> bool { false }
            }
        """)
        with self.assertRaisesRegex(ValueError, "ambiguous audit declaration"):
            checker.audit_inventory(self.root, self.ledger)
        (self.root / "specs/extra.rs").unlink()
        self.ledger["definitions"]["alias"] = copy.deepcopy(self.ledger["definitions"]["identity-model"])
        with self.assertRaisesRegex(ValueError, "duplicate audit declaration"):
            checker.audit_inventory(self.root, self.ledger)

    def test_discovery_masks_noncode_and_does_not_infer_purpose_from_names(self):
        self.write("specs/extra.rs", '''
            const TEXT: &str = "pub open spec fn phantom() -> bool { true }";
            // pub open spec fn commented() -> bool { true }
            verus! { pub open spec fn output_valid() -> bool { true } }
        ''')
        self.classification_fixture()
        family = self.ledger["model_classifications"]["families"][0]
        self.assertEqual(family["members"], ["spec::output_valid"])
        family.update(classification="project-result-model", justification="Synthetic output relation despite its name.")
        self.inline_fixture()
        report = checker.audit_inventory(self.root, self.ledger)
        helper = report["unregistered_spec_functions"][0]
        self.assertEqual(helper["name"], "output_valid")
        self.assertEqual(helper["classification"], "project-result-model")
        self.assertFalse(report["classification_complete"])

    def test_classification_binding_tampering_and_module_rerouting_fail(self):
        authored = self.ledger["model_classifications"]
        for field in ("registry_sha256", "project_source_sha256"):
            original = authored[field]
            authored[field] = "0" * 64
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "stale navigation"):
                checker.audit_inventory(self.root, self.ledger)
            authored[field] = original
        self.ledger["definitions"]["identity-model"]["module_path"] = ["leaf"]
        with self.assertRaisesRegex(ValueError, "module points to another file"):
            checker.audit_inventory(self.root, self.ledger)

    def test_audit_snapshot_is_explicitly_unreviewed_and_requires_an_external_pin(self):
        packet, relative = self.packet_fixture()
        self.assertEqual(packet["state"], "unreviewed")
        self.assertIs(packet["normative_approval"], False)
        self.assertIs(packet["document_processing_authorized"], False)
        self.assertIsNone(packet["human_adjudicator"])
        checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)
        for pin in (None, "", "bad", "a" * 64):
            with self.subTest(pin=pin), self.assertRaises(ValueError):
                checker.check_audit_packet(self.root, self.ledger, packet, pin, relative)
        packet["requirements"][0]["gaps"].append("local report edit")
        self.assertEqual(self.ledger["requirements"][0]["gaps"], [])

    def test_audit_snapshot_rejects_source_dependency_and_validator_drift(self):
        packet, relative = self.packet_fixture()
        for name in (
            "src/leaf.rs", "specs/model.rs", "config/toolchain.json",
            "validation/source-provenance.json", "tools/check_traceability.py",
            "tools/evidence.schema.json", "Cargo.toml", "verification/Cargo.lock",
        ):
            path = self.root / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.subTest(name=name), self.assertRaises(ValueError):
                checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)
            path.write_bytes(original)
        self.write("specs/new.rs", "verus! { pub open spec fn added() -> bool { true } }")
        with self.assertRaises(ValueError):
            checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)

    def test_packet_binds_local_standard_sources_graph_review_and_controls(self):
        names = {
            "verification/stdlib/fixture/lib.rs": "pub fn trusted_fixture() {}\n",
            "verification/stdlib/fixture/Cargo.toml": "[package]\nname = 'fixture'\n",
            "verification/stdlib/fixture/Cargo.lock": "version = 4\n",
            "verification/stdlib/review.json": '{"normative_approval": false}\n',
            "verification/stdlib/check_controls.py": "print('control')\n",
            "verification/stdlib/STATUS.txt": "Unreviewed fixture; no approval.\n",
        }
        for name, contents in names.items():
            self.write(name, contents)
        self.write("verification/stdlib/fixture/target/generated.rs", "generated build output")
        packet, relative = self.packet_fixture()
        for name in names:
            self.assertEqual(
                packet["source_bindings"][name], hashlib.sha256((self.root / name).read_bytes()).hexdigest(),
            )
        self.assertNotIn(
            "verification/stdlib/fixture/target/generated.rs", packet["source_bindings"],
        )
        checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)
        self.assertIs(packet["normative_approval"], False)
        self.assertIs(packet["document_processing_authorized"], False)
        for name in names:
            path = self.root / name
            original = path.read_bytes()
            path.write_bytes(original + b"\n")
            with self.subTest(name=name), self.assertRaises(ValueError):
                checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)
            path.write_bytes(original)

    def test_packet_rejects_added_or_removed_local_standard_inputs(self):
        name = "verification/stdlib/fixture/lib.rs"
        self.write(name, "pub fn trusted_fixture() {}\n")
        packet, relative = self.packet_fixture()
        path = self.root / name
        path.unlink()
        with self.assertRaises(ValueError):
            checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)
        self.write(name, "pub fn trusted_fixture() {}\n")
        self.write("verification/stdlib/fixture/extra.rs", "pub fn added() {}\n")
        with self.assertRaises(ValueError):
            checker.check_audit_packet(self.root, self.ledger, packet, packet["packet_sha256"], relative)

    def test_packet_rejects_symlinked_local_standard_source_directories(self):
        packet, relative = self.packet_fixture()
        self.write("external/hidden.rs", "hidden source")
        (self.root / "verification/stdlib").mkdir(parents=True)
        link = self.root / "verification/stdlib/hidden"
        try:
            link.symlink_to(self.root / "external", target_is_directory=True)
        except OSError as error:
            self.skipTest(f"symlink unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlinks"):
            checker.audit_packet(self.root, self.ledger, ["T88-FIXTURE"], relative)

    def test_audit_snapshot_cannot_forge_approval_scope_types_or_selected_rows(self):
        packet, relative = self.packet_fixture()
        for key, value in (
            ("state", "approved"), ("normative_approval", True),
            ("document_processing_authorized", True), ("human_adjudicator", "self"),
            ("schema_version", True), ("requirement_ids", []),
            ("requirements", []), ("source_bindings", {}),
        ):
            changed = copy.deepcopy(packet)
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                checker.check_audit_packet(self.root, self.ledger, changed, packet["packet_sha256"], relative)
        forged = copy.deepcopy(packet)
        forged["normative_approval"] = True
        forged.pop("packet_sha256")
        forged["packet_sha256"] = hashlib.sha256(json.dumps(
            forged, sort_keys=True, separators=(",", ":"), allow_nan=False,
        ).encode("utf-8")).hexdigest()
        with self.assertRaises(ValueError):
            checker.check_audit_packet(self.root, self.ledger, forged, forged["packet_sha256"], relative)

    def test_packet_selection_missing_sources_and_stale_in_memory_ledger_fail(self):
        packet, relative = self.packet_fixture()
        for selected in (None, [], "T88-FIXTURE", ["unknown"], ["T88-FIXTURE"] * 2, [{}]):
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                checker.audit_packet(self.root, self.ledger, selected, relative)
        changed = copy.deepcopy(self.ledger)
        changed["requirements"][0]["title"] = "changed"
        with self.assertRaisesRegex(ValueError, "ledger changed"):
            checker.audit_packet(self.root, changed, ["T88-FIXTURE"], relative)
        (self.root / "config/toolchain.json").unlink()
        with self.assertRaises(ValueError):
            checker.audit_packet(self.root, self.ledger, ["T88-FIXTURE"], relative)

    def test_packet_rejects_symlinked_source_directories(self):
        packet, relative = self.packet_fixture()
        self.write("external/hidden.rs", "hidden source")
        link = self.root / "specs/hidden"
        try:
            link.symlink_to(self.root / "external", target_is_directory=True)
        except OSError as error:
            self.skipTest(f"symlink unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlinks"):
            checker.audit_packet(self.root, self.ledger, ["T88-FIXTURE"], relative)

    def test_audit_cli_does_not_ignore_missing_pins_or_mixed_modes(self):
        for options in (
            ["--audit-packet", "packet"],
            ["--audit-requirement", "T88-FIXTURE"],
            ["--check-audit-packet", "packet"],
            ["--expected-audit-sha256", "a" * 64],
            ["--audit-packet", "packet", "--check-audit-packet", "other"],
            ["--source-review-pr", "4"],
            ["--source-review-id", "41"],
            ["--source-review-pr", "4", "--source-review-id", "41"],
        ):
            with self.subTest(options=options), patch.object(
                sys, "argv", ["checker", "--output", str(self.root / "result.json"), *options],
            ), redirect_stderr(StringIO()), self.assertRaises(SystemExit) as stopped:
                checker.main()
            self.assertEqual(stopped.exception.code, 2)

    def test_failed_audit_validation_removes_stale_success_reports(self):
        packet, relative = self.packet_fixture()
        stored = self.root / "packet.json"
        stored.write_text(json.dumps(packet))
        output = self.root / "target/result.json"
        output.parent.mkdir()
        output.write_text('{"status":"passed"}')
        output.with_suffix(".md").write_text("stale success")
        with patch.object(checker, "ROOT", self.root), patch.object(
            sys, "argv", [
                "checker", "--ledger", str(self.root / relative), "--output", str(output),
                "--check-audit-packet", str(stored), "--expected-audit-sha256", "a" * 64,
            ],
        ), redirect_stderr(StringIO()):
            self.assertEqual(checker.main(), 1)
        self.assertFalse(output.exists())
        self.assertFalse(output.with_suffix(".md").exists())
        self.assertTrue(stored.is_file())

    def test_source_disposition_cli_requires_valid_packet_clean_head_and_live_approval(self):
        for outcome in (
            "accepted", "dirty", "untracked_source", "unauthorized", "api_failure", "timeout", "wrong_pin",
        ):
            with self.subTest(outcome=outcome):
                packet, relative = self.packet_fixture()
                stored = self.root / "target/packet.json"
                stored.parent.mkdir(exist_ok=True)
                stored.write_text(json.dumps(packet), encoding="utf-8")
                output = self.root / "target/result.json"
                output.write_text('{"status":"stale success"}', encoding="utf-8")
                output.with_suffix(".md").write_text("stale success", encoding="utf-8")
                pin = "f" * 64 if outcome == "wrong_pin" else packet["packet_sha256"]
                disposition = {
                    "status": "authenticated_source_snapshot_disposition",
                    "review_id": 41, "human_user_id": 22,
                    "normative_approval": False, "document_processing_authorized": False,
                }
                failures = {
                    "unauthorized": ValueError("base-revision human authority is not configured"),
                    "api_failure": checker.subprocess.CalledProcessError(1, ["gh", "api"]),
                    "timeout": checker.subprocess.TimeoutExpired(["gh", "api"], 60),
                }

                def git(command, **kwargs):
                    if command == ["git", "rev-parse", "HEAD"]:
                        return SimpleNamespace(stdout="a" * 40 + "\n")
                    if command[:2] == ["git", "ls-files"]:
                        self.assertEqual(command, [
                            "git", "ls-files", "--error-unmatch", "--", *packet["source_bindings"],
                        ])
                        if outcome == "untracked_source":
                            raise checker.subprocess.CalledProcessError(1, command)
                        return SimpleNamespace(stdout="")
                    self.assertEqual(command, ["git", "status", "--porcelain", "--untracked-files=all"])
                    return SimpleNamespace(stdout=" M source.rs\n" if outcome == "dirty" else "")

                with patch.object(checker, "ROOT", self.root), patch.object(
                    sys, "argv", [
                        "checker", "--ledger", str(self.root / relative), "--output", str(output),
                        "--check-audit-packet", str(stored), "--expected-audit-sha256", pin,
                        "--source-review-pr", "4", "--source-review-id", "41",
                    ],
                ), patch.object(checker.subprocess, "run", side_effect=git), patch.object(
                    checker, "source_review_disposition", return_value=disposition,
                    side_effect=failures.get(outcome),
                ) as gate, redirect_stderr(StringIO()), redirect_stdout(StringIO()):
                    self.assertEqual(checker.main(), 0 if outcome == "accepted" else 1)
                if outcome == "accepted":
                    report = load_json(output)
                    self.assertTrue(report["audit_packet_identity_checked"])
                    self.assertEqual(report["source_review_disposition"], disposition)
                    self.assertIn("Not normative approval", output.with_suffix(".md").read_text())
                    gate.assert_called_once_with(4, 41, "a" * 40, packet["packet_sha256"])
                else:
                    self.assertFalse(output.exists())
                    self.assertFalse(output.with_suffix(".md").exists())
                    if outcome in {"dirty", "untracked_source", "wrong_pin"}:
                        gate.assert_not_called()
                self.assertEqual(load_json(stored)["state"], "unreviewed")
                self.assertFalse(load_json(stored)["normative_approval"])

    def test_audit_output_aliases_cannot_delete_input_or_snapshot_files(self):
        packet, relative = self.packet_fixture()
        stored = self.root / "target/packet.md"
        stored.parent.mkdir()
        original = json.dumps(packet)
        stored.write_text(original)
        for output in (stored, stored.with_suffix(".json"), self.root / relative, self.root / "outside.json"):
            with self.subTest(output=output), patch.object(checker, "ROOT", self.root), patch.object(
                sys, "argv", [
                    "checker", "--ledger", str(self.root / relative), "--output", str(output),
                    "--check-audit-packet", str(stored), "--expected-audit-sha256", packet["packet_sha256"],
                ],
            ), redirect_stderr(StringIO()), self.assertRaises(SystemExit) as stopped:
                checker.main()
            self.assertEqual(stopped.exception.code, 2)
            self.assertEqual(stored.read_text(), original)
            self.assertEqual(json.loads((self.root / relative).read_text()), self.ledger)

    def test_test_inventory_respects_test_and_ghost_scopes(self):
        self.write("tests/hidden.rs", "#![cfg(verus_keep_ghost)]\n#[test] fn hidden() {}")
        tests = inventory_tests(self.root)
        self.assertIn(("lib", "leaf::tests::identity_returns_input"), tests)
        self.assertTrue(tests[("lib", "leaf::tests::ignored_probe")]["ignored"])
        self.assertNotIn(("lib", "leaf::tests::ghost_only"), tests)
        self.assertNotIn(("hidden", "hidden"), tests)
        self.assertIn("leaf::after_tests", runtime_inventory(self.root))

    def test_external_unit_tests_keep_their_declared_module_names(self):
        library = self.root / "src/lib.rs"
        library.write_text(library.read_text() + "\n#[cfg(test)] mod external_tests;\n")
        self.write("src/external_tests.rs", "#![cfg(test)]\n#[test] fn separate_case() {}")
        tests = inventory_tests(self.root)
        self.assertIn(("lib", "external_tests::separate_case"), tests)
        self.assertEqual(tests[("lib", "external_tests::separate_case")]["file"], "src/external_tests.rs")

    def test_external_tests_follow_inline_module_path_attributes(self):
        library = self.root / "src/lib.rs"
        library.write_text(library.read_text() + """
            #[cfg(test)]
            mod audit {
                #[path = "renamed.rs"]
                mod checks;
            }
        """)
        self.write("src/audit/renamed.rs", "#[test] #[ignore] fn named_case() {}")
        tests = inventory_tests(self.root)
        self.assertIn(("lib", "audit::checks::named_case"), tests)
        self.assertTrue(tests[("lib", "audit::checks::named_case")]["ignored"])
        self.assertNotIn(("lib", "audit::renamed::named_case"), tests)

    def test_unreachable_test_files_do_not_supply_coverage(self):
        self.write("src/orphan.rs", "#[test] fn unreachable_case() {}")
        self.assertNotIn(("lib", "orphan::unreachable_case"), inventory_tests(self.root))

    def test_external_test_module_cycles_and_ambiguity_fail(self):
        library = self.root / "src/lib.rs"
        original = library.read_text()
        library.write_text(original + '\n#[cfg(test)] #[path = "lib.rs"] mod cycle;\n')
        with self.assertRaisesRegex(ValueError, "cyclic"):
            inventory_tests(self.root)
        library.write_text(original + "\n#[cfg(test)] mod ambiguous;\n")
        self.write("src/ambiguous.rs", "#[test] fn first() {}")
        self.write("src/ambiguous/mod.rs", "#[test] fn second() {}")
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            inventory_tests(self.root)

    def test_integration_crate_root_does_not_add_its_filename_to_module_paths(self):
        self.write("tests/consumer.rs", "mod support;")
        self.write("tests/support.rs", "#[test] fn helper_case() {}")
        tests = inventory_tests(self.root)
        self.assertIn(("consumer", "support::helper_case"), tests)
        self.assertIn(("support", "helper_case"), tests)

    def test_stale_runtime_and_model_hashes_fail(self):
        for path in ("src/leaf.rs", "specs/model.rs"):
            original = (self.root / path).read_bytes()
            (self.root / path).write_bytes(original + b"\n")
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "stale"):
                self.run_check()
            (self.root / path).write_bytes(original)

    def test_same_named_wrong_namespace_does_not_satisfy_model(self):
        wrong = {"jbig2_rs_production::other::identity", "jbig2_rs_production::leaf::identity"}
        with self.assertRaisesRegex(ValueError, "definition absent"):
            self.run_check(names=wrong)

    def test_commented_declaration_is_not_a_model(self):
        self.write("specs/model.rs", "verus! {\n// pub open spec fn identity(x:u8)->u8 {x}\n}")
        with self.assertRaisesRegex(ValueError, "missing or ambiguous Verus"):
            self.run_check()

    def test_module_alias_must_resolve_to_the_recorded_file(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["definitions"]["identity-model"]["module_path"] = ["leaf"]
        with self.assertRaisesRegex(ValueError, "another file"):
            self.run_check(ledger)

    def included_ledger(self):
        self.write("specs/model.rs", 'include!("../proofs/model_support/model.rs");\n')
        self.write("proofs/model_support/model.rs", """
            verus! {
                pub open spec fn identity(x: u8) -> u8 { x }
            }
        """)
        ledger = copy.deepcopy(self.ledger)
        ledger["definitions"]["identity-model"]["file"] = "proofs/model_support/model.rs"
        for path in ("specs/model.rs", "proofs/model_support/model.rs"):
            ledger["bindings"][path] = hashlib.sha256((self.root / path).read_bytes()).hexdigest()
        return ledger

    def test_literal_include_preserves_logical_owner_and_binds_physical_file(self):
        ledger = self.included_ledger()
        self.assertEqual(self.run_check(ledger)["status"], "references_valid_scope_incomplete")
        wrong = {"jbig2_rs_production::model_support::identity"}
        with self.assertRaisesRegex(ValueError, "definition absent"):
            self.run_check(ledger, names=wrong)

    def test_included_source_must_be_bound_and_fresh(self):
        ledger = self.included_ledger()
        included = "proofs/model_support/model.rs"
        ledger["bindings"].pop(included)
        with self.assertRaisesRegex(ValueError, "unbound"):
            self.run_check(ledger)
        ledger = self.included_ledger()
        self.write(included, (self.root / included).read_text() + "\n")
        with self.assertRaisesRegex(ValueError, "stale"):
            self.run_check(ledger)

    def test_unused_included_support_is_still_in_the_module_binding_closure(self):
        self.write("proofs/model_support/extra.rs", """
            verus! {
                proof fn helper() {}
            }
        """)
        self.write(
            "specs/model.rs",
            (self.root / "specs/model.rs").read_text()
            + '\ninclude!("../proofs/model_support/extra.rs");\n',
        )
        ledger = copy.deepcopy(self.ledger)
        for path in ("specs/model.rs", "proofs/model_support/extra.rs"):
            ledger["bindings"][path] = hashlib.sha256((self.root / path).read_bytes()).hexdigest()
        self.assertEqual(self.run_check(ledger)["status"], "references_valid_scope_incomplete")

    def test_include_cycles_duplicates_and_ambiguous_declarations_fail(self):
        for content, message in (
            ('include!("model.rs");\n', "cyclic or duplicate"),
            ('include!("extra.rs");\ninclude!("extra.rs");\n', "cyclic or duplicate"),
            ("""
                verus! {
                    pub open spec fn identity(x: u8) -> u8 { x }
                }
                include!("extra.rs");
            """, "missing or ambiguous"),
        ):
            ledger = self.included_ledger()
            self.write("proofs/model_support/extra.rs", """
                verus! {
                    pub open spec fn identity(x: u8) -> u8 { x }
                }
            """)
            self.write("proofs/model_support/model.rs", content)
            with self.subTest(content=content), self.assertRaisesRegex(ValueError, message):
                self.run_check(ledger)

    def test_dynamic_nested_and_external_includes_fail_closed(self):
        for source, message in (
            ('include!(concat!("model", ".rs"));\n', "literal relative"),
            ('verus! { include!("model.rs"); }\n', "module-scope"),
            ('include!("/external.rs");\n', "literal relative"),
            ('include!("../../external.rs");\n', "external included"),
        ):
            self.write("specs/model.rs", source)
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, message):
                self.run_check()

    def test_child_modules_in_included_source_need_explicit_support(self):
        self.included_ledger()
        self.write("proofs/model_support/model.rs", "mod nested;\n")
        with self.assertRaisesRegex(ValueError, "included module declarations"):
            self.run_check()

    def test_ignored_missing_and_undiscovered_tests_fail(self):
        for name in ("leaf::tests::ignored_probe", "leaf::tests::nonexistent"):
            ledger = copy.deepcopy(self.ledger)
            ledger["requirements"][0]["tests"][0]["name"] = name
            with self.subTest(name=name), self.assertRaises((ValueError, KeyError)):
                self.run_check(ledger)
        with self.assertRaisesRegex(ValueError, "Cargo discovery"):
            self.run_check(discovered=set())

    def test_weak_evidence_cannot_be_promoted_without_postcondition(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["functional_evidence"] = []
        with self.assertRaisesRegex(ValueError, "explicit postcondition"):
            self.run_check(ledger)
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["functional_evidence"][0]["tokens"] = ["out == 0"]
        with self.assertRaisesRegex(ValueError, "not present"):
            self.run_check(ledger)

    def test_preconditions_are_not_functional_output_evidence(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["functional_evidence"][0]["section"] = "requires"
        with self.assertRaisesRegex(ValueError, "preconditions cannot"):
            self.run_check(ledger)

    def test_unverified_body_cannot_back_a_functional_claim(self):
        path = self.root / "verification/production-proof-coverage.json"
        coverage = load_json(path)
        entry = next(item for item in coverage["functions"] if item["id"] == "leaf::identity")
        entry.update(
            status="runtime-proof-divergence", verus_functions=[],
            disposition_reason="synthetic divergent-body fixture",
        )
        coverage["counts"]["verified"] -= 1
        coverage["counts"]["runtime-proof-divergence"] = 1
        self.write_json("verification/production-proof-coverage.json", coverage)
        with self.assertRaisesRegex(ValueError, "unverified or divergent"):
            self.run_check()

    def test_unrelated_existing_proof_macro_cannot_back_a_runtime_claim(self):
        original = (self.root / "src/lib.rs").read_text()
        self.write("src/lib.rs", original + """
            #[cfg(verus_keep_ghost)]
            #[path = "../proofs/helper.rs"] mod helper;
        """)
        self.write("proofs/helper.rs", "macro_rules! unrelated { () => {}; }")
        ledger = copy.deepcopy(self.ledger)
        ledger["definitions"]["unrelated"] = {
            "kind": "macro", "file": "proofs/helper.rs", "module_path": ["helper"],
            "name": "unrelated", "role": "synthetic proof support",
        }
        ledger["requirements"][0]["proof_macros"] = ["unrelated"]
        with self.assertRaisesRegex(ValueError, "not wired"):
            self.run_check(ledger)

    def test_required_feature_cannot_disappear_or_be_excluded(self):
        for features in ([], [{"feature": "decoder", "support_status": "required", "decomposition": "excluded"}]):
            ledger = copy.deepcopy(self.ledger)
            ledger["features"] = features
            with self.subTest(features=features), self.assertRaises(ValueError):
                self.run_check(ledger)

    def test_complete_feature_cannot_be_an_empty_declaration(self):
        self.write_json("config/support-matrix.json", {"rows": [
            {"feature": "decoder", "status": "required"},
            {"feature": "other", "status": "required"},
        ]})
        ledger = copy.deepcopy(self.ledger)
        ledger["features"].append({
            "feature": "other", "support_status": "required", "decomposition": "complete",
        })
        with self.assertRaisesRegex(ValueError, "complete feature has no requirement"):
            self.run_check(ledger)

    def test_feature_accounting_fields_are_required_by_schema(self):
        for field in ("requirement_ids", "remaining_scope"):
            ledger = copy.deepcopy(self.ledger)
            del ledger["features"][0][field]
            with self.subTest(field=field), self.assertRaises(ValidationError):
                validate(ledger, "traceability")

    def test_missing_stale_and_duplicate_feature_requirement_ids_fail(self):
        for references in ([], ["T88-STALE"], ["T88-FIXTURE", "T88-FIXTURE"]):
            ledger = copy.deepcopy(self.ledger)
            ledger["features"][0]["requirement_ids"] = references
            with self.subTest(references=references), self.assertRaisesRegex(ValueError, "accounting"):
                self.run_check(ledger)

    def test_feature_cannot_claim_another_features_requirement(self):
        self.write_json("config/support-matrix.json", {"rows": [
            {"feature": "decoder", "status": "required"},
            {"feature": "other", "status": "required"},
        ]})
        ledger = copy.deepcopy(self.ledger)
        ledger["features"].append({
            "feature": "other", "support_status": "required", "decomposition": "partial",
            "requirement_ids": ["T88-FIXTURE"], "remaining_scope": ["Still unenumerated."],
        })
        with self.assertRaisesRegex(ValueError, "accounting"):
            self.run_check(ledger)

    def test_recorded_feature_cannot_stay_not_started(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0]["decomposition"] = "not_started"
        with self.assertRaisesRegex(ValueError, "state disagrees"):
            self.run_check(ledger)

    def test_partial_feature_cannot_have_no_requirement_records(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"] = []
        ledger["definitions"] = {}
        ledger["bindings"] = {}
        ledger["features"][0]["requirement_ids"] = []
        with self.assertRaisesRegex(ValueError, "state disagrees"):
            self.run_check(ledger)

    def test_unfinished_or_excluded_feature_needs_remaining_scope(self):
        for status, state, selected in (
            ("required", "partial", True),
            ("required", "not_started", False),
            ("deferred", "excluded", False),
        ):
            self.write_json("config/support-matrix.json", {"rows": [{"feature": "decoder", "status": status}]})
            for notes in ([], ["   "]):
                ledger = copy.deepcopy(self.ledger)
                ledger["features"][0].update(
                    support_status=status, decomposition=state, remaining_scope=notes,
                    requirement_ids=["T88-FIXTURE"] if selected else [],
                )
                if not selected:
                    ledger.update(requirements=[], definitions={}, bindings={})
                with self.subTest(state=state, notes=notes), self.assertRaisesRegex(ValueError, "remaining scope"):
                    self.run_check(ledger)

    def test_complete_decomposition_still_rejects_unreviewed_records(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0].update(decomposition="complete", remaining_scope=[])
        with self.assertRaisesRegex(ValueError, "unresolved scope or evidence"):
            self.run_check(ledger)

    def test_reviewed_decomposition_can_account_for_explicit_semantic_gaps(self):
        for change in (
            {"gaps": ["Unproved output semantics."]},
            {"strength": "safety_only", "functional_evidence": []},
            {"strength": "safety_only", "functional_evidence": [], "gaps": ["Unproved output semantics."]},
        ):
            ledger = copy.deepcopy(self.ledger)
            ledger["features"][0].update(decomposition="complete", remaining_scope=[])
            ledger["requirements"][0]["review"] = "approved"
            ledger["requirements"][0].update(change)
            with self.subTest(change=change):
                report = self.run_check(ledger)
                self.assertTrue(report["decomposition_review_complete"])
                self.assertFalse(report["semantic_claims_complete"])
                self.assertFalse(report["normative_decomposition_complete"])
                self.assertEqual(report["status"], "references_valid_scope_incomplete")
                self.assertEqual(report["release_gate"], "semantic_claims_review_required")
                enforce_completion(report, require_decomposition=True)
                with self.assertRaisesRegex(ValueError, "semantic claims"):
                    enforce_completion(report, require_complete=True)

    def test_both_gates_pass_only_when_reviewed_scope_and_functional_claims_are_closed(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0].update(decomposition="complete", remaining_scope=[])
        ledger["requirements"][0]["review"] = "approved"
        report = self.run_check(ledger)
        self.assertTrue(report["decomposition_review_complete"])
        self.assertTrue(report["semantic_claims_complete"])
        self.assertTrue(report["normative_decomposition_complete"])
        enforce_completion(report, require_decomposition=True, require_complete=True)

    def test_reviewed_functional_rows_cannot_hide_unenumerated_feature_scope(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["review"] = "approved"
        report = self.run_check(ledger)
        self.assertFalse(report["decomposition_review_complete"])
        self.assertFalse(report["semantic_claims_complete"])
        for flags in ({"require_decomposition": True}, {"require_complete": True}):
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                enforce_completion(report, **flags)

    def test_cli_decomposition_gate_does_not_make_the_full_semantic_gate_pass(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0].update(decomposition="complete", remaining_scope=[])
        ledger["requirements"][0].update(
            review="approved", strength="safety_only", functional_evidence=[],
            gaps=["The output transformation is unproved."],
        )
        self.write_json("validation/traceability/requirements.json", ledger)
        original_read = checker.read_evidence

        def read(path, kind):
            if kind == "verus":
                return {"func-details": {name: {} for name in self.names}}
            return original_read(path, kind)

        for flag, expected in (("--require-reviewed-decomposition", 0), ("--require-complete", 1)):
            with self.subTest(flag=flag):
                output = self.root / ("result-" + flag[2:] + ".json")
                arguments = [
                    "check_traceability.py", flag, "--verus-json", "fixture.json",
                    "--discover-tests", "--output", str(output),
                ]
                with patch.object(checker, "ROOT", self.root), patch.object(sys, "argv", arguments),\
                        patch.object(checker, "read_evidence", side_effect=read),\
                        patch.object(checker, "discover_tests", return_value=self.discovered),\
                        patch.object(checker.subprocess, "run", return_value=SimpleNamespace(stdout="fixture\n")),\
                        redirect_stdout(StringIO()), redirect_stderr(StringIO()):
                    self.assertEqual(checker.main(), expected)
                result = load_json(output)
                self.assertTrue(result["decomposition_review_complete"])
                self.assertFalse(result["semantic_claims_complete"])
                self.assertFalse(result["normative_decomposition_complete"])

    def test_both_cli_gates_require_verifier_and_real_discovery_inputs(self):
        for flag in ("--require-reviewed-decomposition", "--require-complete"):
            with self.subTest(flag=flag):
                with patch.object(sys, "argv", [
                    "check_traceability.py", flag, "--output", str(self.root / "unused.json"),
                ]), redirect_stderr(StringIO()), self.assertRaises(SystemExit) as error:
                    checker.main()
                self.assertEqual(error.exception.code, 2)

    def test_complete_feature_cannot_hide_remaining_scope(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["features"][0]["decomposition"] = "complete"
        ledger["requirements"][0]["review"] = "approved"
        with self.assertRaisesRegex(ValueError, "unresolved scope or evidence"):
            self.run_check(ledger)

    def test_duplicate_runtime_model_test_and_contract_evidence_fails(self):
        for field in ("runtime", "models", "tests", "functional_evidence"):
            ledger = copy.deepcopy(self.ledger)
            ledger["requirements"][0][field].append(copy.deepcopy(ledger["requirements"][0][field][0]))
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, f"duplicate {field}"):
                self.run_check(ledger)
            with self.subTest(schema=field), self.assertRaises(ValidationError):
                validate(ledger, "traceability")

    def test_duplicate_contract_tokens_fail_even_when_whitespace_differs(self):
        for token in ("out == crate::spec::identity(x)", "out==crate::spec::identity(x)"):
            ledger = copy.deepcopy(self.ledger)
            ledger["requirements"][0]["functional_evidence"][0]["tokens"].append(token)
            with self.subTest(token=token), self.assertRaisesRegex(ValueError, "duplicate contract token"):
                self.run_check(ledger)

    def test_unused_and_aliased_duplicate_model_definitions_fail(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["definitions"]["alias"] = copy.deepcopy(ledger["definitions"]["identity-model"])
        with self.assertRaisesRegex(ValueError, "unused or missing"):
            self.run_check(ledger)
        ledger["requirements"][0]["models"].append("alias")
        with self.assertRaisesRegex(ValueError, "duplicate model/proof definition"):
            self.run_check(ledger)

    def test_weakened_contract_fails_after_source_hashes_are_refreshed(self):
        path = self.root / "src/leaf.rs"
        path.write_text(path.read_text().replace(
            "ensures out == crate::spec::identity(x)", "ensures out <= 255",
        ))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        ledger = copy.deepcopy(self.ledger)
        ledger["bindings"]["src/leaf.rs"] = digest
        coverage = load_json(self.root / "verification/production-proof-coverage.json")
        for function in coverage["functions"]:
            if function["file"] == "src/leaf.rs":
                function["file_sha256"] = digest
        self.write_json("verification/production-proof-coverage.json", coverage)
        with self.assertRaisesRegex(ValueError, "not present"):
            self.run_check(ledger)

    def test_commented_out_postcondition_cannot_satisfy_a_refreshed_binding(self):
        path = self.root / "src/leaf.rs"
        path.write_text(path.read_text().replace(
            "ensures out == crate::spec::identity(x),",
            "ensures /* out == crate::spec::identity(x) */ true,",
        ))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        ledger = copy.deepcopy(self.ledger)
        ledger["bindings"]["src/leaf.rs"] = digest
        coverage = load_json(self.root / "verification/production-proof-coverage.json")
        for function in coverage["functions"]:
            if function["file"] == "src/leaf.rs":
                function["file_sha256"] = digest
        self.write_json("verification/production-proof-coverage.json", coverage)
        with self.assertRaisesRegex(ValueError, "not present"):
            self.run_check(ledger)

    def test_documentation_cannot_replace_a_removed_ghost_contract(self):
        path = self.root / "src/leaf.rs"
        path.write_text(path.read_text().replace(
            """#[cfg_attr(verus_keep_ghost, verus_spec(out =>
                ensures out == crate::spec::identity(x),
            ))]""",
            """#[doc = "verus_spec(ensures out == crate::spec::identity(x))"]
            #[cfg_attr(verus_keep_ghost, verus_verify)]""",
        ))
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        ledger = copy.deepcopy(self.ledger)
        ledger["bindings"]["src/leaf.rs"] = digest
        coverage = load_json(self.root / "verification/production-proof-coverage.json")
        for function in coverage["functions"]:
            if function["file"] == "src/leaf.rs":
                function["file_sha256"] = digest
        self.write_json("verification/production-proof-coverage.json", coverage)
        with self.assertRaisesRegex(ValueError, "not present"):
            self.run_check(ledger)

    def test_true_cannot_be_promoted_into_functional_or_state_evidence(self):
        for strength in ("functional_contract", "state_preservation"):
            ledger = copy.deepcopy(self.ledger)
            requirement = ledger["requirements"][0]
            requirement["strength"] = strength
            claim = {"runtime": "leaf::identity", "section": "ensures", "tokens": ["( true )"]}
            requirement["functional_evidence"] = [claim] if strength == "functional_contract" else []
            requirement["invariant_evidence"] = [claim] if strength == "state_preservation" else []
            with self.subTest(strength=strength), self.assertRaisesRegex(ValueError, "tautological"):
                self.run_check(ledger)

    def test_state_preservation_cannot_use_a_precondition(self):
        ledger = copy.deepcopy(self.ledger)
        requirement = ledger["requirements"][0]
        requirement["strength"] = "state_preservation"
        requirement["invariant_evidence"] = requirement["functional_evidence"]
        requirement["functional_evidence"] = []
        requirement["invariant_evidence"][0]["section"] = "requires"
        with self.assertRaisesRegex(ValueError, "preconditions cannot satisfy a state"):
            self.run_check(ledger)

    def test_functional_evidence_cannot_hide_under_a_weaker_strength(self):
        for strength in ("state_preservation", "safety_only"):
            ledger = copy.deepcopy(self.ledger)
            ledger["requirements"][0]["strength"] = strength
            ledger["requirements"][0]["invariant_evidence"] = copy.deepcopy(
                ledger["requirements"][0]["functional_evidence"],
            )
            with self.subTest(strength=strength), self.assertRaisesRegex(ValueError, "weaker strength classification"):
                self.run_check(ledger)
    def test_duplicate_requirements_and_unused_bindings_fail(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"].append(copy.deepcopy(ledger["requirements"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate requirement"):
            self.run_check(ledger)
        ledger = copy.deepcopy(self.ledger)
        ledger["bindings"]["README.md"] = "a" * 64
        with self.assertRaisesRegex(ValueError, "unused or missing"):
            self.run_check(ledger)

    def test_normative_edition_or_hash_change_invalidates_review(self):
        for change in ({"edition": "old"}, {"document_sha256": "b" * 64}):
            ledger = copy.deepcopy(self.ledger)
            ledger["standards"][0] = self.standard | change
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "normative"):
                self.run_check(ledger)

    def test_source_consultations_are_required_and_cannot_disappear_or_duplicate(self):
        ledger = copy.deepcopy(self.ledger)
        del ledger["source_consultations"]
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")
        for consultations in ([], [self.consultation, self.consultation]):
            ledger = copy.deepcopy(self.ledger)
            ledger["source_consultations"] = consultations
            with self.subTest(consultations=consultations), self.assertRaisesRegex(ValueError, "exact normative standards once"):
                self.run_check(ledger)

    def test_stale_document_or_package_consultation_hash_fails(self):
        for field in ("document_sha256", "package_sha256"):
            ledger = copy.deepcopy(self.ledger)
            ledger["source_consultations"][0][field] = "c" * 64
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "consultation .* differs"):
                self.run_check(ledger)

    def test_consultation_cannot_point_to_a_mirror_or_credentialed_url(self):
        for url in (
            "https://mirror.invalid/T88.zip",
            "http://www.itu.int/rec/dologin_pub.asp?id=fixture",
            "https://user@www.itu.int/rec/dologin_pub.asp?id=fixture",
            "https://www.itu.int/unrecorded/path",
        ):
            ledger = copy.deepcopy(self.ledger)
            ledger["source_consultations"][0]["acquisition_url"] = url
            with self.subTest(url=url), self.assertRaisesRegex(ValueError, "official publication host"):
                self.run_check(ledger)

    def test_consultation_cannot_name_another_official_publication(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["source_consultations"][0]["acquisition_url"] = (
            "https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.6-198811-I!!SOFT-ZST-E&type=items"
        )
        with self.assertRaisesRegex(ValueError, "different publication/package"):
            self.run_check(ledger)

    def test_consultation_clause_families_cannot_be_empty_or_repeated(self):
        for families in ([], [" "], ["6.3", "6.3 "], ["6.3", "6.3"]):
            ledger = copy.deepcopy(self.ledger)
            ledger["source_consultations"][0]["clause_families"] = families
            with self.subTest(families=families), self.assertRaisesRegex(ValueError, "nonempty distinct clause families"):
                self.run_check(ledger)

    def test_explicit_project_policy_is_counted_separately_from_normative_rules(self):
        policy = {"feature": "decoder", "status": "required", "note": "Synthetic API policy."}
        self.write_json("config/support-matrix.json", {"rows": [policy]})
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0].update(
            obligation_kind="project_profile_policy", policy_row_sha256=policy_row_hash(policy),
        )
        validate(ledger, "traceability")
        report = self.run_check(ledger)
        self.assertEqual(report["obligation_kind_counts"], {"normative_rule": 0, "project_profile_policy": 1})
        self.assertEqual(report["requirements"][0]["obligation_kind"], "project_profile_policy")
        self.assertEqual(report["requirements"][0]["policy_row_sha256"], policy_row_hash(policy))

    def test_stale_missing_or_unstated_policy_basis_fails(self):
        policy = {"feature": "decoder", "status": "required", "note": "Synthetic API policy."}
        self.write_json("config/support-matrix.json", {"rows": [policy]})
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["obligation_kind"] = "project_profile_policy"
        with self.assertRaisesRegex(ValueError, "stale or missing project policy"):
            self.run_check(ledger)
        ledger["requirements"][0]["policy_row_sha256"] = policy_row_hash(policy)
        self.write_json("config/support-matrix.json", {"rows": [policy | {"note": "Weakened API policy."}]})
        with self.assertRaisesRegex(ValueError, "stale or missing project policy"):
            self.run_check(ledger)
        self.write_json("config/support-matrix.json", {"rows": [{"feature": "decoder", "status": "required"}]})
        with self.assertRaisesRegex(ValueError, "no explicit support-matrix basis"):
            self.run_check(ledger)

    def test_project_policy_hash_cannot_be_mislabeled_normative(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["policy_row_sha256"] = "c" * 64
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")
        with self.assertRaisesRegex(ValueError, "cannot hide a project policy"):
            self.run_check(ledger)

    def test_direct_reference_pdf_preserves_missing_edition_and_source_namespace(self):
        ledger = self.with_reference_standard()
        validate(ledger, "traceability")
        report = self.run_check(ledger)
        self.assertIsNone(report["standards"][1]["edition"])
        self.assertEqual(report["standards"][1]["provenance_collection"], "normative_references")
        self.assertEqual(report["requirements"][1]["standard"], "ITU-T T.6 (1988)")
        self.assertNotIn("package_sha256", report["source_consultations"][1])

    def test_reference_cannot_silently_resolve_from_another_collection(self):
        ledger = self.with_reference_standard()
        del ledger["standards"][1]["provenance_collection"]
        with self.assertRaisesRegex(ValueError, "provenance collection differs"):
            self.run_check(ledger)

    def test_reference_edition_cannot_be_invented_from_the_year(self):
        for edition in ("1988", "1.0", "2.0"):
            ledger = self.with_reference_standard()
            ledger["standards"][1]["edition"] = edition
            with self.subTest(edition=edition), self.assertRaisesRegex(ValueError, "normative edition differs"):
                self.run_check(ledger)

    def test_ambiguous_cross_collection_identity_is_rejected(self):
        ledger = self.with_reference_standard()
        provenance = load_json(self.root / "validation/source-provenance.json")
        provenance["normative_documents"].append(copy.deepcopy(provenance["normative_references"][0]))
        self.write_json("validation/source-provenance.json", provenance)
        with self.assertRaisesRegex(ValueError, "ambiguous normative provenance"):
            self.run_check(ledger)

    def test_reference_requires_an_approved_document_hash_not_only_metadata(self):
        ledger = self.with_reference_standard()
        provenance = load_json(self.root / "validation/source-provenance.json")
        del provenance["normative_references"][0]["document_sha256"]
        self.write_json("validation/source-provenance.json", provenance)
        with self.assertRaisesRegex(ValueError, "no approved document identity"):
            self.run_check(ledger)

    def test_reference_cannot_reuse_another_standards_hash_or_prefix(self):
        ledger = self.with_reference_standard()
        ledger["standards"][1]["document_sha256"] = self.standard["document_sha256"]
        with self.assertRaisesRegex(ValueError, "normative document identity differs"):
            self.run_check(ledger)
        ledger = self.with_reference_standard()
        ledger["requirements"][1]["standard"] = self.standard["id"]
        with self.assertRaisesRegex(ValueError, "prefix differs from normative source"):
            self.run_check(ledger)

    def test_direct_pdf_cannot_invent_a_package_or_use_an_unrecorded_url(self):
        ledger = self.with_reference_standard()
        ledger["source_consultations"][1]["package_sha256"] = "b" * 64
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")
        with self.assertRaisesRegex(ValueError, "cannot invent a package"):
            self.run_check(ledger)
        ledger = self.with_reference_standard()
        ledger["source_consultations"][1]["acquisition_url"] += "&unrecorded=1"
        with self.assertRaisesRegex(ValueError, "URL differs from recorded provenance"):
            self.run_check(ledger)

    def test_direct_reference_cannot_masquerade_as_the_other_standards_package(self):
        ledger = self.with_reference_standard()
        ledger["source_consultations"][1].update(
            acquisition_kind="official_package", package_sha256=self.consultation["package_sha256"],
            acquisition_url=self.consultation["acquisition_url"],
        )
        with self.assertRaisesRegex(ValueError, "package_sha256 differs from provenance"):
            self.run_check(ledger)

    def test_missing_reference_edition_or_acquisition_kind_is_not_guessed(self):
        ledger = self.with_reference_standard()
        del ledger["standards"][1]["edition"]
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")
        ledger = self.with_reference_standard()
        del ledger["source_consultations"][1]["acquisition_kind"]
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")

    def test_local_document_hashes_are_named_and_not_inferred_from_the_first_standard(self):
        ledger = self.with_reference_standard()
        self.write("fixture-t88.pdf", "synthetic first document")
        self.write("fixture-t6.pdf", "synthetic reference document")
        for standard, path in zip(ledger["standards"], ("fixture-t88.pdf", "fixture-t6.pdf")):
            standard["document_sha256"] = hashlib.sha256((self.root / path).read_bytes()).hexdigest()
        t6 = f"ITU-T T.6 (1988)={self.root / 'fixture-t6.pdf'}"
        checks = local_document_checks(ledger, standard_documents=[t6])
        self.assertEqual(checks, {self.standard["id"]: False, "ITU-T T.6 (1988)": True})
        self.assertFalse(all(checks.values()))
        checks = local_document_checks(ledger, self.root / "fixture-t88.pdf", [t6])
        self.assertTrue(all(checks.values()))
        with self.assertRaisesRegex(ValueError, "hash mismatch: ITU-T T.6"):
            local_document_checks(ledger, standard_documents=[
                f"ITU-T T.6 (1988)={self.root / 'fixture-t88.pdf'}",
            ])

    def test_named_local_documents_reject_duplicates_unknown_ids_and_missing_names(self):
        for selection in ("missing separator", "=fixture.pdf", "ITU-T T.6 (1988)="):
            with self.subTest(selection=selection), self.assertRaisesRegex(ValueError, "needs STANDARD_ID=PATH"):
                local_document_checks(self.ledger, standard_documents=[selection])
        with self.assertRaisesRegex(ValueError, "unknown local normative document"):
            local_document_checks(self.ledger, standard_documents=["unknown=fixture.pdf"])
        ledger = copy.deepcopy(self.ledger)
        self.write("fixture.pdf", "synthetic document")
        path = self.root / "fixture.pdf"
        ledger["standards"][0]["document_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        selection = f"{self.standard['id']}={path}"
        with self.assertRaisesRegex(ValueError, "duplicate local normative document"):
            local_document_checks(ledger, path, [selection])

    def test_typed_error_policy_has_a_separate_approved_public_api_binding(self):
        matrix = load_json(self.root / "config/support-matrix.json")
        matrix["public_api"] = {
            "status": "approved",
            "requirements": ["Typed malformed, unsupported, resource-limit, input/output, and allocation errors."],
        }
        self.write_json("config/support-matrix.json", matrix)
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0].update(
            obligation_kind="project_profile_policy", policy_source="public_api",
            policy_row_sha256=policy_row_hash(matrix["public_api"]),
        )
        validate(ledger, "traceability")
        report = self.run_check(ledger)
        self.assertEqual(report["requirements"][0]["policy_source"], "public_api")
        matrix["public_api"]["requirements"].append("Changed policy scope.")
        self.write_json("config/support-matrix.json", matrix)
        with self.assertRaisesRegex(ValueError, "stale or missing project policy"):
            self.run_check(ledger)
        matrix["public_api"]["status"] = "pending"
        self.write_json("config/support-matrix.json", matrix)
        with self.assertRaisesRegex(ValueError, "no approved public API basis"):
            self.run_check(ledger)

    def test_public_api_policy_cannot_omit_the_typed_error_basis_or_change_selector(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0].update(
            obligation_kind="project_profile_policy", policy_source="public_api", policy_row_sha256="c" * 64,
        )
        self.write_json("config/support-matrix.json", {
            "rows": [{"feature": "decoder", "status": "required"}],
            "public_api": {"status": "approved", "requirements": ["Unrelated API policy."]},
        })
        with self.assertRaisesRegex(ValueError, "no approved public API basis"):
            self.run_check(ledger)
        ledger["requirements"][0]["policy_source"] = "unrecorded"
        with self.assertRaisesRegex(ValueError, "unknown project policy source"):
            self.run_check(ledger)

    def test_policy_selector_cannot_be_hidden_on_a_normative_record(self):
        ledger = copy.deepcopy(self.ledger)
        ledger["requirements"][0]["policy_source"] = "public_api"
        with self.assertRaises(ValidationError):
            validate(ledger, "traceability")
        with self.assertRaisesRegex(ValueError, "cannot hide a project policy"):
            self.run_check(ledger)

    def test_checked_repository_ledger_is_schema_valid_and_provisional(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        validate(ledger, "traceability")
        self.assertEqual(len(ledger["requirements"]), 266)
        self.assertTrue(all(entry["review"] == "provisional" for entry in ledger["requirements"]))

    def test_selected_header_fields_are_exact_and_still_provisional(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {
            row["id"]: row for row in ledger["requirements"]
            if row["id"].startswith(("T88-PATTERN-HEADER-", "T88-HALFTONE-HEADER-"))
        }
        expected = {
            **{f"T88-PATTERN-HEADER-{name}": "pattern_dictionaries" for name in
               ("MMR", "TEMPLATE", "RESERVED", "WIDTH", "HEIGHT", "GRAYMAX")},
            **{f"T88-HALFTONE-HEADER-{name}": "halftone_regions" for name in (
                "MMR", "TEMPLATE", "SKIP", "OPERATOR", "DEFAULT-PIXEL",
                "GRID-WIDTH", "GRID-HEIGHT", "GRID-X", "GRID-Y", "VECTOR-X", "VECTOR-Y",
            )},
        }
        self.assertEqual(set(rows), set(expected))
        for identity, feature in expected.items():
            with self.subTest(requirement=identity):
                row = rows[identity]
                self.assertEqual(row["standard"], "ITU-T T.88 (08/2018)")
                self.assertEqual(row["feature"], feature)
                self.assertEqual(row["strength"], "functional_contract")
                self.assertEqual(row["review"], "provisional")
                self.assertTrue(row["gaps"])
                self.assertTrue(row["functional_evidence"])
                self.assertTrue(row["clause"].startswith(
                    "7.4.4.1." if feature == "pattern_dictionaries" else "7.4.5.1."
                ))
                worklist = next(row for row in ledger["features"] if row["feature"] == feature)
                self.assertEqual(worklist["decomposition"], "partial")
                self.assertIn(identity, worklist["requirement_ids"])

    def test_selected_arithmetic_algorithm_membership_is_exact_and_unapproved(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"].removeprefix("T88-ARITHMETIC-"): row
                for row in ledger["requirements"] if row["feature"] == "arithmetic_coding"}
        groups = {
            "INTEGER": [
                "PROCEDURE-BANKS", "HISTORY-SEED", "CONTEXT-SELECT", "HISTORY-GROW",
                "HISTORY-ROLL", "CONTEXT-STORAGE", "SIGN-DECISION", "PAYLOAD-ORDER",
                "RESULT-NONNEGATIVE", "RESULT-NEGATIVE", "RESULT-OOB",
            ] + [f"MAGNITUDE-LEAF-{i}" for i in range(1, 7)]
              + [f"TABLE-ROW-{i:02}" for i in range(1, 15)],
            "IAID": [
                "LENGTH", "SEED", "CONTEXT", "HISTORY", "ITERATIONS",
                "CLEAR-SENTINEL", "RESULT", "STORAGE",
            ],
            "DRIVER": ["INIT", "CONTEXT-SEQUENCE"],
            "REGISTER": ["HIGH-LOW", "LOW-INPUT", "UNSIGNED"],
            "DECODE": [
                "QE-LOOKUP", "INTERVAL-SUBTRACT", "CODE-PARTITION",
                "UPPER-CODE-SUBTRACT", "FAST-MPS", "UPPER-DISPATCH",
                "LOWER-DISPATCH", "RETURN-DECISION",
            ],
            "EXCHANGE-MPS": ["COMPARE", "MPS-RESULT", "LPS-RESULT", "SWITCH", "NLPS"],
            "EXCHANGE-LPS": ["COMPARE", "MPS-RESULT", "LPS-RESULT", "SWITCH", "NLPS", "INTERVAL"],
            "RENORM": ["REFILL", "INTERVAL-SHIFT", "CODE-SHIFT", "COUNTER", "STOP"],
            "BYTEIN": [
                "BRANCH", "MARKER-THRESHOLD", "MARKER-POINTER", "MARKER-FILL",
                "MARKER-COUNTER", "STUFFED-POINTER", "STUFFED-ALIGN",
                "STUFFED-COUNTER", "ORDINARY-POINTER", "ORDINARY-ALIGN", "ORDINARY-COUNTER",
            ],
            "INITDEC": ["POINTER", "CODE-SEED", "BYTEIN", "CODE-ALIGN", "COUNTER", "INTERVAL"],
            "QE": ["ESTIMATE-SCALE", "UPDATE-TIMING", "NMPS", "NLPS-SWITCH", "FIXED-STATE"]
                  + [f"TABLE-INDEX-{i:02}" for i in range(47)],
        }
        original = {"RESET-ONE", "RESET-SET", "DECISION"}
        added = {"INTERFACE"} | {f"{group}-{suffix}"
                                 for group, suffixes in groups.items() for suffix in suffixes}
        self.assertEqual(set(rows), original | added)
        self.assertEqual(len(added), 138)
        functional = {
            "REGISTER-LOW-INPUT",
            *(f"RENORM-{suffix}" for suffix in groups["RENORM"]),
            *(f"DECODE-{suffix}" for suffix in groups["DECODE"] if suffix != "QE-LOOKUP"),
            "INTEGER-CONTEXT-SELECT", "INTEGER-HISTORY-GROW", "INTEGER-HISTORY-ROLL",
            "INTEGER-HISTORY-SEED", "INTEGER-SIGN-DECISION", "INTEGER-PAYLOAD-ORDER",
            *(f"INTEGER-MAGNITUDE-LEAF-{index}" for index in range(1, 7)),
            "INTEGER-RESULT-NONNEGATIVE", "INTEGER-RESULT-NEGATIVE", "INTEGER-RESULT-OOB",
            "IAID-SEED", "IAID-CONTEXT", "IAID-HISTORY", "IAID-ITERATIONS",
            "IAID-CLEAR-SENTINEL", "IAID-RESULT",
            "INTEGER-PROCEDURE-BANKS", "IAID-STORAGE",
            "DRIVER-CONTEXT-SEQUENCE",
            *(f"EXCHANGE-MPS-{suffix}" for suffix in groups["EXCHANGE-MPS"]),
            *(f"EXCHANGE-LPS-{suffix}" for suffix in groups["EXCHANGE-LPS"]),
        }
        for name in added:
            with self.subTest(requirement=name):
                row = rows[name]
                self.assertEqual(row["review"], "provisional")
                self.assertEqual(
                    row["strength"],
                    "functional_contract" if name in functional else "state_preservation",
                )
                if name in functional:
                    self.assertTrue(row["functional_evidence"])
                    self.assertTrue(
                        {
                            "arithmetic-normalized-state", "arithmetic-integer-bit-state",
                            "arithmetic-iaid-state", "arithmetic-integer-state",
                            "arithmetic-integer-bits-run",
                            "arithmetic-exchange-normalization",
                            "generic-snapshot-view", "refinement-snapshot-view",
                        } & set(row["models"])
                    )
                else:
                    self.assertEqual(row["functional_evidence"], [])
                self.assertTrue(row["invariant_evidence"])
                self.assertTrue(row["gaps"])
                self.assertEqual(row["standard"], "ITU-T T.88 (08/2018)")
                callers = {
                    "symbol_text::IntegerField::slot",
                    'symbol_text::codec::arithmetic::ArithmeticCodec::decode_integer',
                    'symbol_text::codec::arithmetic::ArithmeticCodec::decode_symbol_id',
                } if name in {
                    "INTEGER-PROCEDURE-BANKS", "IAID-LENGTH", "IAID-STORAGE",
                } else set()
                if name in {"INTEGER-PROCEDURE-BANKS", "IAID-STORAGE"}:
                    callers |= {
                        "symbol_text::codec::arithmetic::new_arithmetic_codec",
                        "symbol_text::codec::arithmetic::IntegerContexts::new",
                        "symbol_text::codec::SymbolCodec::decode_integer",
                        "symbol_text::codec::SymbolCodec::decode_symbol_id",
                    }
                if name == "IAID-RESULT":
                    callers = {
                        "symbol_text::codec::arithmetic::ArithmeticCodec::decode_symbol_id",
                        "symbol_text::codec::SymbolCodec::decode_symbol_id",
                    }
                if name == "DRIVER-CONTEXT-SEQUENCE":
                    callers = {
                        "generic_region::contexts::validate_probability_states",
                        "generic_region::generic::probability_signal_error",
                        "generic_region::support::unexpected_eof",
                        "generic_region::support::integer_overflow",
                    }
                    callers |= {
                        "generic_region::contexts::" + owner + "::" + method
                        for owner in ("GenericContexts", "RefinementContexts")
                        for method in ("new", "reset", "try_snapshot", "restore")
                    }
                    callers |= {
                        "generic_region::generic::GenericArithmeticSession::" + method
                        for method in ("new", "consumed_byte_offset",
                                       "observed_marker_offset", "validate_final_marker")
                    } | {
                        "generic_region::segments::validate_arithmetic_terminator",
                        "generic_region::segments::validate_observed_marker",
                    }
                    self.assertTrue({
                        "generic-probability-validation-result",
                        "generic-probability-signal-error",
                    }.issubset(row["models"]))
                    self.assertIn("Auxiliary project contracts", row["scope_of_formal_result"])
                self.assertTrue(callers.issubset(row["runtime"]))
                self.assertTrue(all(
                    runtime.startswith("arithmetic::") or runtime in callers
                    for runtime in row["runtime"]
                ))
        feature = next(f for f in ledger["features"] if f["feature"] == "arithmetic_coding")
        self.assertEqual(feature["decomposition"], "partial")
        self.assertTrue(feature["remaining_scope"])
        self.assertEqual(rows["RESET-ONE"]["strength"], "functional_contract")
        self.assertEqual(rows["RESET-SET"]["strength"], "functional_contract")
        self.assertEqual(rows["DECISION"]["strength"], "state_preservation")

    def test_session_framing_is_local_project_result_support_not_normative_closure(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        row = next(row for row in ledger["requirements"]
                   if row["id"] == "T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE")
        expected = {
            "generic_region::generic::GenericArithmeticSession::" + method
            for method in ("new", "consumed_byte_offset",
                           "observed_marker_offset", "validate_final_marker")
        } | {
            "generic_region::segments::validate_arithmetic_terminator",
            "generic_region::segments::validate_observed_marker",
        }
        self.assertTrue(expected <= {claim["runtime"] for claim in row["functional_evidence"]})
        models = {
            "generic-" + name for name in (
                "bounded-arithmetic-tail", "early-arithmetic-marker",
                "arithmetic-terminator-validation", "observed-marker-result", "final-marker-result",
            )
        }
        self.assertTrue(models <= set(row["models"]))
        self.assertTrue(all(ledger["definitions"][name]["file"] ==
                            "specs/generic_region/framing.rs" for name in models))
        family = next(f for f in ledger["model_classifications"]["families"]
                      if f["file"] == "specs/generic_region/framing.rs")
        self.assertEqual(family["classification"], "project-result-model")
        self.assertEqual(row["strength"], "functional_contract")
        self.assertEqual(row["review"], "provisional")
        self.assertTrue(row["gaps"])
        self.assertIn("not public", row["scope_of_formal_result"])
        self.assertIn("not independent framing", row["scope_of_formal_result"])

    def test_context_api_baseline_is_functional_support_not_complete_driver_timing(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        row = next(row for row in ledger["requirements"]
                   if row["id"] == "T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE")
        expected = {
            "generic_region::contexts::" + owner + "::" + method
            for owner in ("GenericContexts", "RefinementContexts")
            for method in ("new", "reset", "try_snapshot", "restore")
        }
        self.assertTrue(expected <= {claim["runtime"] for claim in row["functional_evidence"]})
        self.assertEqual(row["strength"], "functional_contract")
        self.assertEqual(row["review"], "provisional")
        self.assertTrue(row["gaps"])
        self.assertIn("not complete E.3.8/caller", row["scope_of_formal_result"])
        for identity in ("generic-snapshot-view", "refinement-snapshot-view"):
            self.assertIn(identity, row["models"])
            self.assertEqual(ledger["definitions"][identity]["file"], "specs/generic_region.rs")

    def test_selected_arithmetic_table_locators_and_checked_links_are_not_shifted(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for index in range(47):
            row = rows[f"T88-ARITHMETIC-QE-TABLE-INDEX-{index:02}"]
            with self.subTest(index=index):
                self.assertEqual(
                    row["clause"],
                    f"E.2.5, Table E.1, index {index}, Qe/NMPS/NLPS/SWITCH columns (p. 116)",
                )
                self.assertEqual(row["proof_macros"], ["arithmetic-qe-transition-bounds"])
                self.assertIn({
                    "target": "lib",
                    "name": "arithmetic::tests::qe_table_matches_annex_e_table_e_1",
                }, row["tests"])
                self.assertEqual(row["runtime"], [
                    'arithmetic::decoding::ArithmeticDecoder::decode_signal',
                    'arithmetic::decoding::ArithmeticDecoder::decode_lower_signal',
                    'arithmetic::decoding::ArithmeticDecoder::decode_upper_signal',
                ])
        for index in range(1, 15):
            row = rows[f"T88-ARITHMETIC-INTEGER-TABLE-ROW-{index:02}"]
            with self.subTest(integer_table_row=index):
                self.assertTrue(row["clause"].startswith(f"A.2, Table A.1, row {index} ("))
                self.assertTrue(row["clause"].endswith("(p. 91)"))
                self.assertEqual(row["runtime"], [
                    'arithmetic::integers::ArithmeticDecoder::decode_integer',
                    'arithmetic::integers::ArithmeticDecoder::decode_integer_bits',
                ])
        for name in ("LENGTH", "SEED", "CONTEXT", "HISTORY", "ITERATIONS",
                     "CLEAR-SENTINEL", "RESULT", "STORAGE"):
            row = rows[f"T88-ARITHMETIC-IAID-{name}"]
            self.assertTrue({"arithmetic-binary-value", "arithmetic-pow2"} <= set(row["models"]))
            self.assertTrue(any("assumed decision sequence" in text for text in row["assumptions"]))

    def test_exchange_evidence_is_reconciled_without_claiming_new_theorems_or_closing_gaps(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = [row for row in ledger["requirements"]
                if row["id"].startswith("T88-ARITHMETIC-EXCHANGE-")]
        self.assertEqual(len(rows), 11)
        for row in rows:
            with self.subTest(requirement=row["id"]):
                self.assertEqual(row["strength"], "functional_contract")
                self.assertTrue(row["functional_evidence"])
                self.assertTrue(row["invariant_evidence"])
                self.assertTrue(row["gaps"])
                self.assertEqual(row["review"], "provisional")
                self.assertIn("already-proved helper contracts", row["scope_of_formal_result"])
                self.assertIn("not a newly authored", row["scope_of_formal_result"])
                self.assertIn("supplied", row["scope_of_formal_result"])
                self.assertIn("arithmetic-exchange-decision", row["models"])
        self.assertEqual(len(ledger["requirements"]), 266)
        self.assertEqual(sum(bool(row["gaps"]) for row in ledger["requirements"]), 257)

    def test_fresh_banks_and_enum_composition_have_actual_caller_and_literal_test_bindings(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for identity in ("T88-ARITHMETIC-INTEGER-PROCEDURE-BANKS", "T88-ARITHMETIC-IAID-STORAGE"):
            row = rows[identity]
            with self.subTest(requirement=identity):
                self.assertEqual(row["strength"], "functional_contract")
                self.assertIn("symbol_text::codec::arithmetic::new_arithmetic_codec", row["runtime"])
                self.assertIn("symbol_text::codec::SymbolCodec::decode_integer", row["runtime"])
                self.assertIn("symbol_text::codec::SymbolCodec::decode_symbol_id", row["runtime"])
                self.assertEqual(len(row["functional_evidence"]), 4)
                self.assertTrue(any("arithmetic_composition::fresh_" in test["name"]
                                    for test in row["tests"]))
                self.assertIn("when a caller must construct versus reuse", row["gaps"][0])
        self.assertIn("specs/symbol_text/arithmetic_codec.rs", ledger["bindings"])
        self.assertIn("src/symbol_text/tests/arithmetic_composition.rs", ledger["bindings"])

    def test_checked_id_caller_guarantees_bind_table_extraction_and_remaining_caller_scope(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for identity in ("T88-PROFILE-SYMBOL-ID-PRESERVATION", "T88-TEXT-SYMBOL-ID"):
            row = rows[identity]
            with self.subTest(requirement=identity):
                self.assertIn("symbol_text::codec::symbols::decode_checked_symbol_id", row["runtime"])
                self.assertTrue({
                    "symbol-checked-id-result", "symbol-checked-id-decode-result",
                    "symbol-checked-id-transition",
                    "symbol-huffman-id-result", "symbol-huffman-id-state",
                    "symbol-huffman-id-transition", "huffman-error-result",
                    "symbol-huffman-error-result",
                } <= set(row["models"]))
                self.assertTrue(any(
                    claim["runtime"] == "symbol_text::codec::symbols::decode_checked_symbol_id"
                    for claim in row["functional_evidence"]
                ))
                self.assertIn(
                    "symbol_text::codec::huffman::HuffmanCodec::decode_symbol_id",
                    row["runtime"],
                )
                self.assertIn("symbol_text::codec::SymbolCodec::decode_symbol_id", row["runtime"])
                self.assertTrue(any(
                    "huffman_symbol_id_transition" in " ".join(claim["tokens"])
                    for claim in row["functional_evidence"]
                ))
                self.assertEqual(row["review"], "provisional")
                self.assertTrue(row["gaps"])
                self.assertIn("extent", row["gaps"][0])
                self.assertIn("rendered", row["gaps"][0])
                self.assertTrue(any("checked_literal_ids_" in test["name"] for test in row["tests"]))
                self.assertTrue(any("huffman_symbol_ids::" in test["name"] for test in row["tests"]))
        self.assertEqual(len(ledger["requirements"]), 266)
        self.assertEqual(sum(bool(row["gaps"]) for row in ledger["requirements"]), 257)

    def test_actual_caller_request_witnesses_preserve_explicit_upstream_gaps(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        for row in ledger["requirements"]:
            if row["id"] not in {"T88-PROFILE-SYMBOL-ID-PRESERVATION", "T88-TEXT-SYMBOL-ID"}:
                continue
            with self.subTest(requirement=row["id"]):
                self.assertIn("symbol-id-request-matches", row["models"])
                self.assertIn("symbol-id-request-witness", row["proof_macros"])
                self.assertIn("symbol_text::text::decode_text_region_body", row["runtime"])
                self.assertIn(
                    "symbol_text::dictionary::arithmetic::decode_dictionary_single_instance_refinement",
                    row["runtime"],
                )
                self.assertIn("codec-kind", row["gaps"][0])
                self.assertIn("flag forwarding alone", row["gaps"][0].lower())
                self.assertEqual(row["review"], "provisional")

    def test_reached_segment_count_width_models_are_bound_without_full_stream_claim(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        for row in ledger["requirements"]:
            if row["id"] not in {"T88-PROFILE-SYMBOL-ID-PRESERVATION", "T88-TEXT-SYMBOL-ID"}:
                continue
            with self.subTest(requirement=row["id"]):
                self.assertIn("segment-symbol-width-matches", row["models"])
                self.assertIn("segment-symbol-width-witness", row["proof_macros"])
                self.assertIn("symbol_text::text::decode_text_region", row["runtime"])
                self.assertIn("symbol_text::dictionary::decode_symbol_dictionary", row["runtime"])
                self.assertIn("extent", row["gaps"][0])
                self.assertIn("codec-kind", row["gaps"][0])
                self.assertIn("rendered", row["gaps"][0])
                self.assertEqual(row["review"], "provisional")

    def test_fresh_huffman_setup_is_bound_without_claiming_upstream_selection(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        for row in ledger["requirements"]:
            if row["id"] not in {"T88-PROFILE-SYMBOL-ID-PRESERVATION", "T88-TEXT-SYMBOL-ID"}:
                continue
            with self.subTest(requirement=row["id"]):
                self.assertIn("symbol-huffman-codec-initial", row["models"])
                self.assertIn("huffman-segment-setup-witness", row["proof_macros"])
                self.assertIn("symbol_text::codec::huffman::new_huffman_codec", row["runtime"])
                self.assertIn(
                    "symbol_text::dictionary::huffman::decode_symbol_dictionary_huffman",
                    row["runtime"],
                )
                self.assertIn("field-table bindings", row["scope_of_formal_result"])
                self.assertIn("table/codec-kind selection", row["gaps"][0])
                self.assertIn("lifecycle", row["gaps"][0])
                self.assertEqual(row["review"], "provisional")

    def test_complete_header_results_are_linked_without_normative_approval(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for identity in [
            "T88-SEGMENT-TYPE-FIELD", "T88-SEGMENT-FLAG-FIELDS",
            "T88-SEGMENT-PAGE-ASSOCIATION", "T88-SEGMENT-KNOWN-LENGTH",
            "T88-SEGMENT-UNKNOWN-LENGTH", "T88-SEGMENT-RETENTION-WIRE-ORDER",
            "T88-SEGMENT-RETENTION-UNUSED-BITS", "T88-SEGMENT-REFERENCE-WIDTH",
            "T88-SEGMENT-REFERENCE-PRECEDENCE",
        ]:
            row = rows[identity]
            with self.subTest(requirement=identity):
                self.assertEqual(row["review"], "provisional")
                self.assertEqual(row["strength"], "functional_contract")
                self.assertIn("segment-header-fields", row["models"])
                self.assertIn("segment-header-field-proof", row["proof_macros"])
                self.assertTrue(row["gaps"])
                claims = [claim for claim in row["functional_evidence"]
                          if claim["runtime"] == "segment_header::parse_segment_header"]
                self.assertEqual(len(claims), 1)
                self.assertTrue(any("segment_header_fields(" in token for token in claims[0]["tokens"]))
        definition = ledger["definitions"]["segment-header-fields"]
        self.assertEqual(definition["module_path"], ["spec", "segments"])
        self.assertEqual(definition["name"], "segment_header_fields")
        self.assertEqual(len(ledger["requirements"]), 266)

    def test_repository_feature_accounting_is_exact_but_still_incomplete(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        features = {feature["feature"]: feature for feature in ledger["features"]}
        for name, feature in features.items():
            expected = {row["id"] for row in ledger["requirements"] if row["feature"] == name}
            with self.subTest(feature=name):
                self.assertEqual(set(feature["requirement_ids"]), expected)
                self.assertTrue(feature["remaining_scope"])
                self.assertNotEqual(feature["decomposition"], "complete")
        for name in (
            "sequential_organization", "random_access_organization", "embedded_organization",
            "standard_huffman_tables", "custom_huffman_tables", "mmr_coding",
        ):
            self.assertEqual(features[name]["decomposition"], "partial")

    def test_parser_only_records_stay_weak_and_mmr_requires_input_raster_evidence(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for name in ("T88-HUFFMAN-STANDARD-TABLES",):
            self.assertEqual(rows[name]["strength"], "safety_only")
            self.assertEqual(rows[name]["functional_evidence"], [])
            self.assertTrue(rows[name]["gaps"])
        prefix = rows["T88-HUFFMAN-PREFIX-MATCH"]
        self.assertEqual(prefix["strength"], "functional_contract")
        self.assertEqual(prefix["review"], "provisional")
        self.assertTrue(prefix["gaps"])
        self.assertIn("supplied table metadata", prefix["scope_of_formal_result"])
        self.assertTrue({"huffman-fallback-run", "huffman-decode-state"} <= set(prefix["models"]))
        self.assertEqual(
            {claim["runtime"] for claim in prefix["functional_evidence"]},
            {"huffman::decoding::HuffmanTable::decode",
             "huffman::decoding::HuffmanTable::decode_fallback"},
        )
        self.assertIn("canonical", " ".join(prefix["gaps"]))
        assignment = rows["T88-HUFFMAN-CANONICAL-ASSIGNMENT"]
        self.assertEqual(assignment["strength"], "functional_contract")
        self.assertEqual(assignment["review"], "provisional")
        self.assertTrue(assignment["gaps"])
        self.assertTrue({
            "huffman-assignment-contents", "huffman-assignment-codes",
            "huffman-assignment-starts", "huffman-assignment-order",
            "huffman-fast-shape", "huffman-fast-slot", "huffman-table-lookup-valid",
        } <= set(assignment["models"]))
        self.assertEqual(
            {claim["runtime"] for claim in assignment["functional_evidence"]},
            {'huffman::construction::HuffmanTable::from_line_source', 'huffman::construction::HuffmanTable::new',
             'huffman::construction::HuffmanTable::new_symbol_ids', 'huffman::construction::HuffmanTable::standard',
             'huffman::construction::HuffmanTable::from_encoded', 'huffman::decoding::build_prefix_lookup',
             'huffman::wide::WideCode::zero'},
        )
        for name in (
            "T6-REFERENCE-FIRST-WHITE", "T6-REFERENCE-NEXT-LINE", "T6-PASS-TRANSITION",
            "T6-VERTICAL-TRANSITION", "T6-FIRST-IMAGINARY-WHITE", "T6-EOFB-PADDING",
            "T88-MMR-BINARY-POLARITY",
        ):
            row = rows[name]
            self.assertEqual(row["strength"], "functional_contract")
            self.assertEqual(row["review"], "provisional")
            self.assertTrue(row["functional_evidence"])
            self.assertTrue(row["gaps"])
        polarity = rows["T88-MMR-BINARY-POLARITY"]
        self.assertTrue({"mmr-input-bitmap", "mmr-bounded-bitmap"} <= set(polarity["models"]))
        claims = {claim["runtime"]: claim["tokens"] for claim in polarity["functional_evidence"]}
        self.assertTrue(any("mmr_spec::bitmap_decodes(" in token for token in claims['mmr::raster::decode_bitmap']))
        self.assertTrue(any("mmr_spec::bounded_bitmap_decodes(" in token for token in claims["mmr::decode_mmr"]))
        self.assertIn("T.6", " ".join(polarity["assumptions"]))

    def test_sequential_records_require_exact_payload_layout_and_conditional_caller_evidence(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for name, caller in (
            ("T88-SEQUENTIAL-SEGMENT-LAYOUT", "document::parsing::parse_standalone_document"),
            ("T88-SEQUENTIAL-NUMBER-ORDER", "document::parsing::parse_standalone_document"),
            ("T88-EMBEDDED-SEGMENT-LAYOUT", "document::parsing::parse_embedded_document"),
        ):
            row = rows[name]
            with self.subTest(requirement=name):
                self.assertEqual(row["strength"], "functional_contract")
                self.assertEqual(row["review"], "provisional")
                self.assertIn("document-sequential-fields", row["models"])
                self.assertIn("document-sequential-complete-proof", row["proof_macros"])
                claims = {claim["runtime"]: claim["tokens"] for claim in row["functional_evidence"]}
                self.assertIn("document::parsing::parse_sequential", claims)
                self.assertTrue(any("sequential_container_fields(" in token for token in claims[caller]))
                self.assertTrue(row["gaps"])
        for name in ("T88-SEGMENT-KNOWN-LENGTH", "T88-SEGMENT-UNKNOWN-LENGTH"):
            claims = {
                claim["runtime"]: claim["tokens"]
                for claim in rows[name]["functional_evidence"]
            }
            self.assertTrue(any("payload_window(" in token for token in claims["document::parsing::read_payload"]))
        self.assertTrue(any(
            "terminator" in gap for gap in rows["T88-SEGMENT-UNKNOWN-LENGTH"]["gaps"]
        ))
        self.assertEqual(len(rows), 266)

    def test_random_access_records_require_separate_windows_order_eof_and_caller_evidence(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for name in (
            "T88-RANDOM-HEADER-PAYLOAD-LAYOUT", "T88-RANDOM-EOF-DELIMITER", "T88-RANDOM-NUMBER-ORDER",
        ):
            row = rows[name]
            with self.subTest(requirement=name):
                self.assertEqual(row["strength"], "functional_contract")
                self.assertEqual(row["review"], "provisional")
                self.assertIn("document-random-access-fields", row["models"])
                self.assertIn("document-random-access-segment-fields", row["models"])
                self.assertIn("document-random-complete-proof", row["proof_macros"])
                claims = {claim["runtime"]: claim["tokens"] for claim in row["functional_evidence"]}
                for function in ("document::parsing::parse_random_access", "document::parsing::parse_standalone_document"):
                    self.assertTrue(any("random_access_container_fields(" in token for token in claims[function]))
                self.assertTrue(row["gaps"])
        self.assertEqual(len(rows), 266)

    def test_page_metadata_fields_require_input_correspondence_and_stay_provisional(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for name in (
            "T88-PAGE-KNOWN-DIMENSIONS", "T88-PAGE-RESOLUTION-FIELDS",
            "T88-PAGE-UNKNOWN-HEIGHT-SENTINEL",
        ):
            row = rows[name]
            with self.subTest(requirement=name):
                self.assertEqual(row["strength"], "functional_contract")
                self.assertEqual(row["review"], "provisional")
                self.assertIn("page-information-fields", row["models"])
                self.assertIn("known-segment-data-fields", row["models"])
                claims = {claim["runtime"]: claim["tokens"] for claim in row["functional_evidence"]}
                self.assertTrue(any(
                    "page_information_fields(" in token
                    for token in claims["segment_data::parse_page_information"]
                ))
                self.assertTrue(row["gaps"])
        for name in (
            "T88-SEQUENTIAL-SEGMENT-LAYOUT", "T88-RANDOM-HEADER-PAYLOAD-LAYOUT",
            "T88-EMBEDDED-SEGMENT-LAYOUT",
        ):
            self.assertIn("container-segment-metadata", rows[name]["models"])
            self.assertIn("profiles-payload-fields", rows[name]["models"])
            self.assertEqual(rows[name]["review"], "provisional")
        self.assertEqual(len(rows), 266)

    def test_all_feature_worklists_have_started_without_claiming_full_decomposition(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        self.assertEqual(sum(feature["decomposition"] == "partial" for feature in ledger["features"]), 29)
        self.assertEqual(sum(feature["decomposition"] == "excluded" for feature in ledger["features"]), 1)
        self.assertTrue(all(feature["decomposition"] != "complete" for feature in ledger["features"]))
        self.assertTrue(all(feature["remaining_scope"] for feature in ledger["features"]))
        policy = [row for row in ledger["requirements"] if row.get("obligation_kind") == "project_profile_policy"]
        self.assertEqual(len(policy), 9)
        self.assertEqual({row["feature"] for row in policy}, {
            "pdf_global_and_page_segments", "colour_palette", "lossy_symbol_substitution_by_decoder", "mmr_coding",
        })
        for row in policy:
            self.assertTrue(row["policy_row_sha256"])
            self.assertTrue(row["gaps"])

    def test_repository_t6_source_and_records_are_distinct_and_provisional(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        standard = next(entry for entry in ledger["standards"] if entry["id"] == "ITU-T T.6 (1988)")
        self.assertIsNone(standard["edition"])
        self.assertEqual(standard["provenance_collection"], "normative_references")
        self.assertEqual(standard["document_sha256"], "8ccbe2d203e03ee2ac37478d50d2d946bab49dfb45376eb0f773009d183a7767")
        consultation = next(entry for entry in ledger["source_consultations"] if entry["standard"] == standard["id"])
        self.assertEqual(consultation["acquisition_kind"], "official_pdf")
        self.assertNotIn("package_sha256", consultation)
        rows = [row for row in ledger["requirements"] if row["id"].startswith("T6-")]
        self.assertEqual(len(rows), 17)
        self.assertTrue(all(row["standard"] == standard["id"] and row["review"] == "provisional" and row["gaps"] for row in rows))
        self.assertEqual(sum(row.get("obligation_kind") == "project_profile_policy" for row in rows), 3)

    def test_new_page_records_do_not_claim_transitive_kernel_or_pixel_equivalence(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        rows = {row["id"]: row for row in ledger["requirements"]}
        for name in ("T88-IMMEDIATE-DIRECT-PAGE", "T88-IMMEDIATE-CLIPPED-PAGE"):
            self.assertTrue(all(runtime.startswith("decode::rendering::compose_immediate") for runtime in rows[name]["runtime"]))
            self.assertEqual(rows[name]["strength"], "state_preservation")
            self.assertEqual(rows[name]["functional_evidence"], [])
            self.assertTrue(rows[name]["invariant_evidence"])
            self.assertIn("audited inventory", " ".join(rows[name]["assumptions"]))
            self.assertTrue(rows[name]["gaps"])

    def test_retention_records_disclose_the_worked_example_conflict(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        for identity in ("T88-SEGMENT-RETENTION-WIRE-ORDER", "T88-SEGMENT-RETENTION-UNUSED-BITS"):
            requirement = next(entry for entry in ledger["requirements"] if entry["id"] == identity)
            with self.subTest(requirement=identity):
                self.assertEqual(requirement["strength"], "functional_contract")
                self.assertEqual(requirement["review"], "provisional")
                self.assertIn("7.2.8", " ".join(requirement["assumptions"]))
                self.assertTrue(requirement["gaps"])
        definition = ledger["definitions"]["retention-wire-bit"]
        self.assertEqual(definition["module_path"], ["spec", "segments"])
        self.assertEqual(definition["name"], "retention_bit")

    def test_typical_row_ledger_records_exact_bytes_and_visible_pixels(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        requirement = next(entry for entry in ledger["requirements"]
                           if entry["id"] == "T88-GENERIC-TYPICAL-ROW")
        self.assertEqual(requirement["strength"], "functional_contract")
        self.assertEqual(requirement["review"], "provisional")
        self.assertEqual(set(requirement["runtime"]), {
            "generic_region::support::copy_bitmap_row", "bitmap::access::Bitmap::copy_row", "bitmap::access::copy_row_bytes",
            "bitmap::storage::Bitmap::blank", "bitmap::storage::Bitmap::filled",
        })
        tokens = " ".join(
            token for claim in requirement["functional_evidence"] for token in claim["tokens"]
        )
        self.assertIn("row_copy_result(old(bitmap), final(bitmap)", tokens)
        self.assertIn("stored_pixel(final(bitmap), x, destination)", tokens)
        self.assertIn("copied-row-pixels", requirement["proof_macros"])
        self.assertIn("zeroed-storage", requirement["models"])
        self.assertIn("zeroed_storage(&bitmap)", tokens)

    def test_batch_reset_ledger_records_the_proved_zero_result(self):
        ledger = load_json(TOOLS.parent / "validation/traceability/requirements.json")
        requirement = next(entry for entry in ledger["requirements"]
                           if entry["id"] == "T88-ARITHMETIC-RESET-SET")
        self.assertEqual(requirement["strength"], "functional_contract")
        self.assertEqual(requirement["review"], "provisional")
        tokens = " ".join(
            token for claim in requirement["functional_evidence"] for token in claim["tokens"]
        )
        self.assertIn("state_index == 0", tokens)
        self.assertIn("mps == 0", tokens)


if __name__ == "__main__":
    unittest.main()
