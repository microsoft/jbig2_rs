import copy
import io
import math
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_workflows import check_proof_agent, check_specification_auditor, check_updates, check_workflow, load_workflow, main


class WorkflowPolicyTests(unittest.TestCase):
    def setUp(self):
        self.document = load_workflow(
            "name: generated policy fixture\n"
            "on: [pull_request, push]\n"
            "permissions: {contents: read}\n"
            "jobs:\n"
            "  checks:\n"
            "    runs-on: ubuntu-latest\n"
            "    timeout-minutes: 20\n"
            "    steps:\n"
            f"      - uses: owner/action@{'a' * 40}\n"
            "      - run: |\n"
            "          echo 'uses: owner/action@v4 is only text'\n"
        )

    def test_actual_yaml_actions_are_checked_without_matching_run_text(self):
        actions = check_workflow(self.document, "fixture.yml")
        self.assertEqual(len(actions), 1)
        self.assertEqual(actions[0]["uses"], "owner/action@" + "a" * 40)
        self.assertIn("on", self.document)
        self.assertNotIn(True, self.document)

    def test_official_conformance_is_private_main_only_and_uploads_no_assets(self):
        actual = load_workflow(
            (TOOLS.parent / ".github/workflows/conformance.yml").read_text()
        )
        self.assertEqual(len(check_workflow(actual, "conformance.yml")), 4)
        for mutation in ("pr", "trust", "private", "skip_private", "order", "fetch",
                         "upload", "extra_job", "skip_run", "change_runner"):
            document = copy.deepcopy(actual)
            job = document["jobs"]["official"]
            steps = job["steps"]
            if mutation == "pr":
                document["on"]["pull_request"] = ""
            elif mutation == "trust":
                job["if"] = "always()"
            elif mutation == "private":
                next(step for step in steps if step.get("name") == "Require private repository")["run"] = "true"
            elif mutation == "skip_private":
                next(step for step in steps if step.get("name") == "Require private repository")["if"] = "false"
            elif mutation == "order":
                private = next(step for step in steps if step.get("name") == "Require private repository")
                steps.remove(private)
                steps.append(private)
            elif mutation == "fetch":
                next(step for step in steps if step.get("name") == "Fetch exact approved conformance vectors")["run"] += " --unverified"
            elif mutation == "upload":
                steps[-1]["with"]["path"] = "target/"
            elif mutation == "extra_job":
                document["jobs"]["untrusted"] = copy.deepcopy(job)
            elif mutation == "skip_run":
                next(step for step in steps if step.get("name") == "Run all original official streams")["if"] = "false"
            elif mutation == "change_runner":
                next(step for step in steps if step.get("name") == "Run all original official streams")["run"] = "true"
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                check_workflow(document, "conformance.yml")

    def test_release_permissions_and_commands_are_narrowly_scoped(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/release.yml").read_text())
        self.assertEqual(len(check_workflow(actual, "release.yml")), 11)
        for mutation in ("trigger", "dispatch", "default", "trust", "oidc_validation", "contents_write",
                         "publish_condition", "environment", "cancel", "source_ref", "target_ref",
                         "credentials", "source_command", "dry_run_command", "recheck", "auth_condition",
                         "auth_pin", "auth_url", "auth_order", "publish_command", "static_secret",
                         "post_verify", "artifact", "extra_step", "global_env", "extra_job",
                         "wait_flag", "wait_timeout"):
            document = copy.deepcopy(actual)
            validate, publish = document["jobs"]["validate"], document["jobs"]["publish"]
            if mutation == "trigger":
                document["on"]["pull_request"] = ""
            elif mutation == "dispatch":
                document["on"]["workflow_dispatch"] = ""
            elif mutation == "default":
                document["on"]["workflow_dispatch"]["inputs"]["dry_run"]["default"] = "false"
            elif mutation == "trust":
                validate["if"] = "always()"
            elif mutation == "oidc_validation":
                validate["permissions"]["id-token"] = "write"
            elif mutation == "contents_write":
                publish["permissions"]["contents"] = "write"
            elif mutation == "publish_condition":
                publish["if"] = "always()"
            elif mutation == "environment":
                publish["environment"] = "other"
            elif mutation == "cancel":
                publish["concurrency"]["cancel-in-progress"] = "true"
            elif mutation == "source_ref":
                validate["steps"][0]["with"]["ref"] = "${{ inputs.tag }}"
            elif mutation == "target_ref":
                publish["steps"][2]["with"]["ref"] = "${{ inputs.tag }}"
            elif mutation == "credentials":
                publish["steps"][2]["with"]["persist-credentials"] = "true"
            elif mutation == "source_command":
                validate["steps"][2]["run"] = "true"
            elif mutation == "dry_run_command":
                validate["steps"][5]["run"] = "cargo publish"
            elif mutation == "recheck":
                publish["steps"][6]["if"] = "false"
            elif mutation == "auth_condition":
                publish["steps"][7]["if"] = "always()"
            elif mutation == "auth_pin":
                publish["steps"][7]["uses"] = "rust-lang/crates-io-auth-action@" + "a" * 40
            elif mutation == "auth_url":
                publish["steps"][7]["with"] = {"url": "https://other.example"}
            elif mutation == "auth_order":
                publish["steps"][6], publish["steps"][7] = publish["steps"][7], publish["steps"][6]
            elif mutation == "publish_command":
                publish["steps"][8]["run"] = "cargo publish --registry other"
            elif mutation == "static_secret":
                publish["steps"][8]["env"]["CARGO_REGISTRY_TOKEN"] = "${{ secrets.CARGO_REGISTRY_TOKEN }}"
            elif mutation == "post_verify":
                publish["steps"][9]["run"] = "true"
            elif mutation == "artifact":
                publish["steps"][10]["with"]["path"] = "target/"
            elif mutation == "extra_step":
                publish["steps"].insert(7, {"run": "echo unexpected"})
            elif mutation == "global_env":
                document["env"] = {"CARGO_HOME": "/other"}
            elif mutation == "extra_job":
                document["jobs"]["extra"] = copy.deepcopy(publish)
            elif mutation == "wait_flag":
                validate["steps"][2]["run"] = validate["steps"][2]["run"].replace(" --wait-for-checks", "")
            elif mutation == "wait_timeout":
                validate["timeout-minutes"] = "121"
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                check_workflow(document, "release.yml")
        with self.assertRaises(ValueError):
            check_workflow(actual, "unrelated.yml")

    def test_proof_agent_pins_model_and_cannot_silently_inherit_or_fallback(self):
        path = TOOLS.parent / ".github/agents/verus-proof-worker.agent.md"
        text = path.read_text()
        self.assertEqual(check_proof_agent(text)["model"], "gpt-6.1-sol")
        for changed in (
            text.replace("model: gpt-6.1-sol\n", ""),
            text.replace("model: gpt-6.1-sol", "model: default"),
            text.replace("model: gpt-6.1-sol", "model: [gpt-6.1-sol, default]"),
            text.replace("disable-model-invocation: true", "disable-model-invocation: false"),
            text.replace("tools: [read, edit, search, execute]", "tools: [read, agent]"),
        ):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                check_proof_agent(changed)

    def test_specification_auditor_is_read_only_explicit_and_has_no_model_fallback(self):
        text = (TOOLS.parent / ".github/agents/specification-auditor.agent.md").read_text()
        self.assertEqual(check_specification_auditor(text)["tools"], ["read", "search"])
        for changed in (
            text.replace("tools: [read, search]", "tools: [read, edit, search]"),
            text.replace("tools: [read, search]", "tools: [read, search, execute]"),
            text.replace("tools: [read, search]", "tools: [read, search, web]"),
            text.replace("\n---\n", "\nhooks: {}\n---\n", 1),
            text.replace("\n---\n", "\nmcp-servers: {}\n---\n", 1),
            text.replace("\n---\n", "\nhandoffs: []\n---\n", 1),
            text.replace("model: gpt-6.1-sol", "model: [gpt-6.1-sol, default]"),
            text.replace("disable-model-invocation: true", "disable-model-invocation: false"),
            text.removeprefix("---\n"), text.split("\n---\n", 1)[0] + "\n---\n",
        ):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                check_specification_auditor(changed)
    def test_proof_agent_rejects_invalid_frontmatter_missing_description_and_empty_body(self):
        text = (TOOLS.parent / ".github/agents/verus-proof-worker.agent.md").read_text()
        frontmatter = text.split("\n---\n", 1)[0] + "\n---\n"
        for changed in (
            text.removeprefix("---\n"),
            text.replace("\n---\n", "\n", 1),
            frontmatter,
            text.replace('description: "', 'description: ""\nignored: "', 1),
            text.replace("model: gpt-6.1-sol", "model: gpt-6.1-sol\nmodel: default"),
        ):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                check_proof_agent(changed)

    def test_empty_nonmapping_and_duplicate_key_yaml_is_rejected(self):
        for text in ("", "{}", "[]", "name: one\nname: two\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                load_workflow(text)

    def test_yaml_merges_and_custom_tags_cannot_hide_policy_fields(self):
        for text in (
            "base: &base {permissions: write-all}\n<<: *base\n",
            "!custom {permissions: read-all}\n",
        ):
            with self.subTest(text=text), self.assertRaises(ValueError):
                load_workflow(text)

    def test_floating_short_dynamic_local_and_container_uses_are_rejected(self):
        for reference in (
            "owner/action@v4", "owner/action@master", "owner/action@" + "a" * 7,
            "owner/action@${{ inputs.ref }}", "./.github/actions/local",
            "docker://example/image@sha256:" + "a" * 64,
        ):
            document = copy.deepcopy(self.document)
            document["jobs"]["checks"]["steps"][0]["uses"] = reference
            with self.subTest(reference=reference), self.assertRaisesRegex(ValueError, "full commit SHA"):
                check_workflow(document, "fixture.yml")

    def test_flow_style_actions_cannot_hide_a_floating_ref(self):
        document = load_workflow(
            "on: push\npermissions: {contents: read}\n"
            "jobs: {check: {runs-on: ubuntu-latest, timeout-minutes: 20, "
            "steps: [{uses: owner/action@v4}]}}\n"
        )
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            check_workflow(document, "flow.yml")

    def test_job_and_step_write_or_broad_permission_overrides_are_rejected(self):
        for permission in ("write-all", {"contents": "write"}, {"actions": "read"}, {"contents": []}):
            document = copy.deepcopy(self.document)
            document["jobs"]["checks"]["permissions"] = permission
            with self.subTest(permission=permission), self.assertRaises(ValueError):
                check_workflow(document, "fixture.yml")
        document = copy.deepcopy(self.document)
        document["permissions"] = {"contents": "write"}
        with self.assertRaises(ValueError):
            check_workflow(document, "fixture.yml")
        document = copy.deepcopy(self.document)
        document["jobs"]["checks"]["steps"][0]["permissions"] = {}
        with self.assertRaises(ValueError):
            check_workflow(document, "fixture.yml")

    def test_privileged_trigger_is_rejected_in_all_supported_yaml_shapes(self):
        for events in ("pull_request_target", ["pull_request_target"], {"pull_request_target": {}}):
            document = copy.deepcopy(self.document)
            document["on"] = events
            with self.subTest(events=events), self.assertRaises(ValueError):
                check_workflow(document, "fixture.yml")

    def test_job_budgets_are_explicit_and_bounded(self):
        for value in (None, "0", "61", "1.5", "${{ inputs.timeout }}"):
            document = copy.deepcopy(self.document)
            document["jobs"]["checks"]["timeout-minutes"] = value
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "timeout"):
                check_workflow(document, "fixture.yml")
        document = copy.deepcopy(self.document)
        document["jobs"]["checks"]["timeout-minutes"] = "60"
        self.assertTrue(check_workflow(document, "fixture.yml"))

    def test_failure_masking_and_unreviewed_reusable_jobs_are_rejected(self):
        for level in ("job", "step"):
            document = copy.deepcopy(self.document)
            job = document["jobs"]["checks"]
            target = job if level == "job" else job["steps"][0]
            target["continue-on-error"] = "true"
            with self.subTest(level=level), self.assertRaises(ValueError):
                check_workflow(document, "fixture.yml")
        document = copy.deepcopy(self.document)
        document["jobs"]["checks"]["uses"] = "owner/repo/workflow.yml@" + "a" * 40
        with self.assertRaises(ValueError):
            check_workflow(document, "fixture.yml")

    def test_rust_action_requires_an_explicit_fixed_or_dated_toolchain(self):
        for version in (None, "", "stable", "nightly", "1.89"):
            document = copy.deepcopy(self.document)
            step = document["jobs"]["checks"]["steps"][0]
            step["uses"] = "dtolnay/rust-toolchain@" + "a" * 40
            step["with"] = {"toolchain": version}
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, "toolchain|Rust setup"):
                check_workflow(document, "fixture.yml")
        for version in ("1.89.0", "nightly-2026-10-12"):
            document = copy.deepcopy(self.document)
            step = document["jobs"]["checks"]["steps"][0]
            step["uses"] = "dtolnay/rust-toolchain@" + "a" * 40
            step["with"] = {"toolchain": version}
            self.assertEqual(check_workflow(document, "fixture.yml")[0]["toolchain"], version)

    def test_stable_matrix_requires_an_explicit_reviewed_version_list(self):
        document = copy.deepcopy(self.document)
        job = document["jobs"]["checks"]
        job["steps"][0].update({
            "uses": "dtolnay/rust-toolchain@" + "a" * 40,
            "with": {"toolchain": "${{ matrix.toolchain }}"},
        })
        for versions in (None, [], ["nightly"], ["1.89.0", "stable"]):
            job["strategy"] = {"matrix": {"toolchain": versions}}
            if versions == ["1.89.0", "stable"]:
                self.assertTrue(check_workflow(document, "fixture.yml"))
            else:
                with self.subTest(versions=versions), self.assertRaises(ValueError):
                    check_workflow(document, "fixture.yml")

    def test_weekly_root_action_updates_are_configured_exactly_once(self):
        document = load_workflow(
            "version: 2\nupdates:\n"
            "  - package-ecosystem: github-actions\n    directory: /\n"
            "    schedule: {interval: weekly}\n"
        )
        check_updates(document)
        for change in ("missing", "duplicate", "wrong_interval"):
            changed = copy.deepcopy(document)
            if change == "missing":
                changed["updates"] = []
            elif change == "duplicate":
                changed["updates"] *= 2
            else:
                changed["updates"][0]["schedule"]["interval"] = "never"
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_updates(changed)

    def test_repository_workflows_preserve_declared_toolchain_selections(self):
        root = TOOLS.parent
        toolchains = json.loads((root / "config/toolchain.json").read_text())
        selections = {}
        for path in sorted((root / ".github/workflows").glob("*.yml")):
            for entry in check_workflow(load_workflow(path.read_text()), path.name):
                if "toolchain" in entry:
                    selections[(path.name, entry["job"])] = entry["toolchain"]
        self.assertEqual(selections, {
            ("ci.yml", "cargo"): toolchains["cargo_rust_toolchain"],
            ("ci.yml", "distribution"): "${{ matrix.toolchain }}",
            ("conformance.yml", "official"): "1.88.0",
            ("coverage.yml", "coverage"): toolchains["coverage"]["rust_toolchain"],
            ("fuzz.yml", "controls"): toolchains["fuzz_rust_toolchain"],
            ("fuzz.yml", "fuzz"): toolchains["fuzz_rust_toolchain"],
            ("performance.yml", "performance"): toolchains["cargo_rust_toolchain"],
            ("release.yml", "validate"): "1.88.0",
            ("release.yml", "publish"): "1.88.0",
            ("verus.yml", "whole"): toolchains["verus"]["rust_toolchain"],
            ("verus.yml", "semantic"): toolchains["verus"]["rust_toolchain"],
            ("verus.yml", "verify"): toolchains["verus"]["rust_toolchain"],
        })
        check_updates(load_workflow((root / ".github/dependabot.yml").read_text()))

    def test_required_validation_runs_on_main_and_release_branch_without_trusting_fuzz_corpus(self):
        for name in ("ci", "coverage", "fuzz", "performance", "verus"):
            original = load_workflow((TOOLS.parent / f".github/workflows/{name}.yml").read_text())
            self.assertTrue(check_workflow(original, f"{name}.yml"))
            for branches in (["main"], ["release/0.1"], ["main", "release/0.1"],
                             ["main", "release/**"], ["**"]):
                document = copy.deepcopy(original)
                document["on"]["push"]["branches"] = branches
                with self.subTest(workflow=name, branches=branches), self.assertRaises(ValueError):
                    check_workflow(document, f"{name}.yml")
        fuzz = load_workflow((TOOLS.parent / ".github/workflows/fuzz.yml").read_text())
        self.assertIn("github.ref == 'refs/heads/main'", fuzz["jobs"]["fuzz"]["env"]["TRUSTED_CORPUS"])
        self.assertNotIn("release/0.1", fuzz["jobs"]["fuzz"]["env"]["TRUSTED_CORPUS"])

    def test_distribution_lane_cannot_skip_the_actual_publication_dry_run(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/ci.yml").read_text())
        check_workflow(actual, "ci.yml")
        for mutation in ("omit", "skip", "replace"):
            document = copy.deepcopy(actual)
            step = next(
                item for item in document["jobs"]["distribution"]["steps"]
                if item.get("name") == "Test isolated proof-free distribution"
            )
            if mutation == "omit":
                step["run"] = step["run"].replace("--publish-dry-run", "")
            elif mutation == "skip":
                step["if"] = "false"
            else:
                step["run"] = "true"
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, "dry run"):
                check_workflow(document, "ci.yml")

    def test_current_verus_scope_cannot_skip_checked_report_or_test_discovery(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        check_workflow(actual, "verus.yml")
        for flag in ("--require-current-scope", "--discover-tests"):
            document = copy.deepcopy(actual)
            step = next(
                item for item in document["jobs"]["whole"]["steps"]
                if item.get("name") == "Verify proof build and audit production coverage"
            )
            step["run"] = step["run"].replace(flag, "")
            with self.subTest(flag=flag), self.assertRaises(ValueError):
                check_workflow(document, "verus.yml")

    def test_verus_stable_collector_and_bounded_dynamic_shards_are_policy_protected(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        self.assertTrue(check_workflow(actual, "verus.yml"))
        mutations = [
            lambda j: j["verify"].pop("if"),
            lambda j: j["verify"].update(needs=["semantic"]),
            lambda j: j["verify"].update(name="renamed"),
            lambda j: j["whole"].update(**{"timeout-minutes": "21"}),
            lambda j: j["semantic"].update(**{"timeout-minutes": "20"}),
            lambda j: j["semantic"]["strategy"].update(**{"fail-fast": "true"}),
            lambda j: j["semantic"]["strategy"].update(**{"max-parallel": "8"}),
            lambda j: j["semantic"]["strategy"].update(matrix={"group": ["reset"]}),
            lambda j: j["whole"].pop("outputs"),
            lambda j: j["semantic"]["steps"][3]["with"].update(toolchain="1.89.0"),
            lambda j: j["whole"]["steps"][7].update(run="echo skipped"),
            lambda j: j["verify"]["steps"][3]["with"].update(**{"merge-multiple": "true"}),
            lambda j: j["verify"]["steps"][3]["with"].update(**{"run-id": "previous"}),
            lambda j: j["verify"]["steps"][4].update(run="true"),
            lambda j: j["verify"]["steps"][4].update(**{"if": "false"}),
            lambda j: j["whole"]["steps"][-1]["with"].update(name="verus-bootstrap-log"),
            lambda j: j["semantic"]["steps"][-1].pop("if"),
        ]
        for mutate in mutations:
            document = copy.deepcopy(actual)
            mutate(document["jobs"])
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                check_workflow(document, "verus.yml")

    def test_verus_generated_reports_cannot_return_to_root_or_skip_compiled_library_checks(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for before, after in (
            ("target/verification/verus-production.json", "verus-production.json"),
            ("target/verification/production-proof-coverage-ci.json", "coverage.json"),
            ("--compiled-library", "--unused-library"),
            ("--require-exact", ""),
        ):
            document = copy.deepcopy(actual)
            for step in document["jobs"]["whole"]["steps"]:
                if "run" in step:
                    step["run"] = step["run"].replace(before, after)
            with self.subTest(before=before), self.assertRaisesRegex(ValueError, "reports must stay"):
                check_workflow(document, "verus.yml")

    def test_semantic_positive_baseline_transport_is_current_exact_and_required(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        self.assertTrue(check_workflow(actual, "verus.yml"))
        for change in (
            "missing-download", "old-run", "wrong-name", "wrong-path",
            "missing-argument", "wrong-argument", "late-download",
        ):
            document = copy.deepcopy(actual)
            steps = document["jobs"]["semantic"]["steps"]
            download = next(
                step for step in steps
                if step.get("uses", "").startswith("actions/download-artifact@")
            )
            execution = next(
                step for step in steps
                if "tools/run_semantic_proofs.py" in step.get("run", "")
            )
            if change == "missing-download":
                steps.remove(download)
            elif change == "old-run":
                download["with"]["run-id"] = "previous"
            elif change == "wrong-name":
                download["with"]["name"] = "verus-partial-previous-whole"
            elif change == "wrong-path":
                download["with"]["path"] = "target/previous"
            elif change == "missing-argument":
                execution["run"] = execution["run"].replace("--whole-baseline target/semantic-whole", "")
            elif change == "wrong-argument":
                execution["run"] = execution["run"].replace("--whole-baseline target/semantic-whole", "--whole-baseline target/previous")
            else:
                steps.remove(download)
                steps.insert(steps.index(execution) + 1, download)
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_workflow(document, "verus.yml")

    def test_whole_job_retains_its_serial_semantic_groups_inside_the_same_artifact(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for before, after in (
            ("--whole-groups", "--skip-groups"),
            ("--whole-baseline target/semantic-ci/whole", "--whole-baseline target/previous"),
            ("--output-dir target/semantic-ci/whole/semantic-groups/target/traceability",
             "--output-dir target/semantic-ci/whole/target/traceability"),
        ):
            document = copy.deepcopy(actual)
            for step in document["jobs"]["whole"]["steps"]:
                if "run" in step:
                    step["run"] = step["run"].replace(before, after)
            with self.subTest(before=before), self.assertRaises(ValueError):
                check_workflow(document, "verus.yml")

    def test_collector_groups_and_raw_evidence_cannot_be_skipped_or_misrouted(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for change in ("missing", "condition", "baseline", "output", "upload", "late"):
            document = copy.deepcopy(actual)
            steps = document["jobs"]["verify"]["steps"]
            execution = next(step for step in steps if "--collector-groups" in step.get("run", ""))
            if change == "missing":
                steps.remove(execution)
            elif change == "condition":
                execution["if"] = "false"
            elif change == "baseline":
                execution["env"]["WHOLE_BASELINE"] = "previous"
            elif change == "output":
                execution["env"]["COLLECTOR_OUTPUT"] = "previous"
            elif change == "upload":
                steps.remove(next(step for step in steps if step.get("with", {}).get("name", "").endswith("-collector")))
            else:
                steps.remove(execution)
                steps.append(execution)
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_workflow(document, "verus.yml")

    def test_native_mutations_keep_the_exact_compiler_cases_artifact_and_existing_job(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/ci.yml").read_text())
        for change in ("missing", "condition", "compiler", "command", "artifact", "timeout"):
            document = copy.deepcopy(actual)
            job = document["jobs"]["cargo"]
            steps = job["steps"]
            execution = next(step for step in steps if step.get("name") == "Check critical public-API mutation sensitivity")
            if change == "missing":
                steps.remove(execution)
            elif change == "condition":
                execution["if"] = "false"
            elif change == "compiler":
                next(step for step in steps if step.get("name") == "Install pinned native mutation compiler")["run"] = "true"
            elif change == "command":
                execution["run"] += " --only-one"
            elif change == "artifact":
                steps[-1]["with"]["path"] = "previous"
            else:
                job["timeout-minutes"] = "21"
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_workflow(document, "ci.yml")

    def test_validation_events_avoid_duplicate_pushes_without_filtering_prs_or_tags(self):
        for name in ("ci", "coverage", "fuzz", "performance", "verus"):
            actual = load_workflow((TOOLS.parent / f".github/workflows/{name}.yml").read_text())
            self.assertTrue(check_workflow(actual, f"{name}.yml"))
            for change in ("branch-pushes", "missing-pr", "filtered-pr", "missing-manual", "missing-tags"):
                document = copy.deepcopy(actual)
                if change == "branch-pushes":
                    document["on"]["push"] = ""
                elif change == "missing-pr":
                    document["on"].pop("pull_request")
                elif change == "filtered-pr":
                    document["on"]["pull_request"] = {"paths": ["src/**"]}
                elif change == "missing-manual":
                    document["on"].pop("workflow_dispatch")
                else:
                    document["on"]["push"].pop("tags")
                with self.subTest(name=name, change=change), self.assertRaisesRegex(ValueError, "validation runs"):
                    check_workflow(document, f"{name}.yml")

    def test_complete_pr_profile_uses_twenty_nine_jobs_without_more_peak_concurrency(self):
        from run_semantic_proofs import configured_shards

        count = 0
        for name in ("ci", "coverage", "fuzz", "performance", "verus"):
            document = load_workflow((TOOLS.parent / f".github/workflows/{name}.yml").read_text())
            for job_name, job in document["jobs"].items():
                matrix = job.get("strategy", {}).get("matrix")
                if matrix is None:
                    count += 1
                elif isinstance(matrix, dict):
                    count += math.prod(len(values) for values in matrix.values())
                else:
                    self.assertEqual((name, job_name), ("verus", "semantic"))
                    count += len(configured_shards())
            if name == "ci":
                expected_os = ["ubuntu-latest", "windows-latest", "macos-latest"]
                self.assertEqual(document["jobs"]["cargo"]["strategy"]["matrix"]["os"], expected_os)
                self.assertEqual(document["jobs"]["distribution"]["strategy"]["matrix"], {
                    "os": expected_os, "toolchain": ["1.88.0", "stable"],
                })
            if name == "verus":
                self.assertEqual(document["jobs"]["semantic"]["strategy"]["max-parallel"], "4")
        self.assertEqual(count, 29)
        self.assertEqual(count - len(configured_shards()) + 8, 23)

    def test_verus_download_retries_remain_bounded_and_hash_checked(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for original, replacement in (
            ("--retry 3", "--retry 30"),
            ("--retry-max-time 60", ""),
            ("--connect-timeout 10", ""),
            ("--max-time 30", ""),
            ("sha256sum --check", "true"),
        ):
            document = copy.deepcopy(actual)
            for job in ("whole", "semantic", "verify"):
                step = next(
                    step for step in document["jobs"][job]["steps"]
                    if step.get("name") == "Download pinned Verus"
                )
                self.assertIn(original, step["run"])
                step["run"] = step["run"].replace(original, replacement)
            with self.subTest(original=original), self.assertRaisesRegex(
                ValueError, "bounded retries and integrity",
            ):
                check_workflow(document, "verus.yml")

    def test_verus_compile_gate_cannot_be_satisfied_by_a_substring_or_comment(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for replacement in ("cargo verus verify", "echo cargo verus build", "# cargo verus build"):
            document = copy.deepcopy(actual)
            step = next(
                step for step in document["jobs"]["whole"]["steps"]
                if step.get("name") == "Verify proof build and audit production coverage"
            )
            step["run"] = step["run"].replace("cargo verus build", replacement)
            self.assertIn("--compiled-library", step["run"])
            with self.subTest(replacement=replacement), self.assertRaisesRegex(ValueError, "exact pinned Cargo"):
                check_workflow(document, "verus.yml")

    def test_verus_check_disabling_flags_and_environment_overrides_are_rejected(self):
        actual = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        for flag in (
            "--no-verify", "--no-lifetime", "--no-erasure-check",
            "--no-trait-conflicts", "--no-auto-recommends-check",
        ):
            for surface in ("command", "workflow-env", "job-env", "step-env", "export"):
                document = copy.deepcopy(actual)
                job = document["jobs"]["whole"]
                step = next(
                    step for step in job["steps"]
                    if step.get("name") == "Verify proof build and audit production coverage"
                )
                if surface == "command":
                    step["run"] = step["run"].replace("--rlimit 30", f"--rlimit 30 {flag}")
                elif surface == "workflow-env":
                    document["env"] = {"VERUS_ARGS": flag}
                elif surface == "job-env":
                    job["env"] = {"VERUS_ARGS": flag}
                elif surface == "step-env":
                    step["env"] = {"VERUS_ARGS": flag}
                else:
                    step["run"] = f'export VERUS_ARGS="{flag}"\n' + step["run"]
                with self.subTest(flag=flag, surface=surface), self.assertRaisesRegex(ValueError, "check-disabling"):
                    check_workflow(document, "verus.yml")

    def test_verus_policy_ignores_comment_flags_but_rejects_extra_invocations(self):
        document = load_workflow((TOOLS.parent / ".github/workflows/verus.yml").read_text())
        step = next(
            step for step in document["jobs"]["whole"]["steps"]
            if step.get("name") == "Verify proof build and audit production coverage"
        )
        step["run"] += "\n# --no-lifetime and --no-erasure-check are forbidden\n"
        self.assertTrue(check_workflow(document, "verus.yml"))
        step["run"] += "\ncargo verus verify --manifest-path verification/Cargo.toml\n"
        with self.assertRaisesRegex(ValueError, "exact pinned Cargo"):
            check_workflow(document, "verus.yml")

    def test_cli_failure_removes_prior_success_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / ".github/workflows"
            directory.mkdir(parents=True)
            (directory / "bad.yml").write_text("permissions: write-all\n")
            output = root / "report.json"
            output.write_text('{"status":"workflow_policy_passed"}')
            with patch("check_workflows.ROOT", root), patch.object(
                sys, "argv", ["check_workflows", "--output", str(output)],
            ), redirect_stderr(io.StringIO()) as errors:
                self.assertEqual(main(), 1)
            self.assertIn("workflow policy validation failed", errors.getvalue())
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
