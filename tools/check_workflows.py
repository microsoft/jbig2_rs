#!/usr/bin/env python3
"""Validate repository workflow policy, not upstream action code or branch rules."""

import argparse
import hashlib
import json
import re
import shlex
import subprocess
import sys
from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[1]
ACTION = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*@[0-9a-f]{40}")
TOOLCHAIN = re.compile(r"(?:[0-9]+\.[0-9]+\.[0-9]+|nightly-[0-9]{4}-[0-9]{2}-[0-9]{2})")
FUZZ_TRUST = (
    "(github.event_name == 'schedule' || github.event_name == 'workflow_dispatch')"
    " && github.ref == 'refs/heads/main' && github.event.repository.default_branch == 'main'"
)
FUZZ_PRODUCER_GROUP = (
    "${{ " + FUZZ_TRUST
    + " && format('fuzz-generated-v2-trusted-{0}-main', github.repository)"
    " || format('fuzz-generated-v2-untrusted-{0}-{1}-{2}',"
    " github.repository, github.run_id, github.run_attempt) }}"
)
VALIDATION_WORKFLOWS = {"ci", "coverage", "fuzz", "performance", "verus"}
OFFICIAL_TRUST = (
    "github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main'"
    " && github.repository == 'microsoft/jbig2_rs'"
)
RELEASE_VALIDATE = (
    "github.repository == 'microsoft/jbig2_rs' && (github.event_name == 'release'"
    " || (github.event_name == 'workflow_dispatch'"
    " && (github.ref == 'refs/heads/release/0.1' || github.ref == 'refs/heads/release/0.2')))"
)
RELEASE_POLICY_REF = (
    "${{ startsWith(github.event.release.tag_name || inputs.tag, 'v0.2.')"
    " && 'release/0.2' || 'release/0.1' }}"
)
RELEASE_PUBLISH = (
    "needs.validate.outputs.publish_needed == 'true'"
    " && (github.event_name == 'release' || inputs.dry_run == false)"
)
RELEASE_RECHECK = "steps.recheck.outputs.publish_needed == 'true'"
RELEASE_READ = {"contents": "read", "actions": "read", "checks": "read", "pull-requests": "read"}
RELEASE_OIDC = {"contents": "read", "id-token": "write"}
RELEASE_TAG = "${{ github.event.release.tag_name || inputs.tag }}"
COLLECTOR_READY = "needs.whole.result == 'success' && needs.semantic.result == 'success'"
DISABLED_VERUS_CHECK = re.compile(
    r"(?<![\w-])--(?:no-verify|no-lifetime|no-erasure-check|no-trait-conflicts"
    r"|no-auto-recommends-check)(?![\w-])"
)


class UniqueLoader(yaml.BaseLoader):
    """Keep GitHub's `on` key as a string without YAML 1.1 boolean coercion."""

    def construct_object(self, node, deep=False):
        if node.tag not in {
            yaml.resolver.BaseResolver.DEFAULT_SCALAR_TAG,
            yaml.resolver.BaseResolver.DEFAULT_SEQUENCE_TAG,
            yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
        }:
            raise ValueError(f"unsupported explicit YAML tag: {node.tag}")
        return super().construct_object(node, deep=deep)


def mapping(loader, node):
    result = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=True)
        if not isinstance(key, str) or key in result or key == "<<":
            raise ValueError(f"duplicate, merged or non-scalar YAML key: {key!r}")
        result[key] = loader.construct_object(value_node, deep=True)
    return result


UniqueLoader.add_constructor(yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, mapping)


def load_workflow(text):
    document = yaml.load(text, Loader=UniqueLoader)
    if not isinstance(document, dict) or not document:
        raise ValueError("workflow/configuration must be a nonempty mapping")
    return document


def check_proof_agent(text):
    if not text.startswith("---\n"):
        raise ValueError("proof agent needs YAML frontmatter")
    frontmatter, separator, body = text[4:].partition("\n---\n")
    if not separator or not body.strip():
        raise ValueError("proof agent needs closed frontmatter and instructions")
    profile = load_workflow(frontmatter)
    expected = {
        "name": "verus-proof-worker", "target": "github-copilot",
        "model": "gpt-6.1-sol", "tools": ["read", "edit", "search", "execute"],
        "disable-model-invocation": "true", "user-invocable": "true",
    }
    if any(profile.get(key) != value for key, value in expected.items()):
        raise ValueError("proof agent must retain the exact model, explicit dispatch and bounded tools")
    if not isinstance(profile.get("description"), str) or not profile["description"].strip():
        raise ValueError("proof agent needs a meaningful description")
    return profile


def check_specification_auditor(text):
    if not text.startswith("---\n"):
        raise ValueError("specification auditor needs YAML frontmatter")
    frontmatter, separator, body = text[4:].partition("\n---\n")
    if not separator or not body.strip():
        raise ValueError("specification auditor needs closed frontmatter and instructions")
    profile = load_workflow(frontmatter)
    expected = {
        "name": "specification-auditor", "target": "github-copilot",
        "model": "gpt-6.1-sol", "tools": ["read", "search"],
        "disable-model-invocation": "true", "user-invocable": "true",
    }
    if (
        set(profile) != {*expected, "description"}
        or any(profile.get(key) != value for key, value in expected.items())
        or not isinstance(profile.get("description"), str) or not profile["description"].strip()
    ):
        raise ValueError("specification auditor must retain read-only explicit dispatch and exact model")
    return profile


def read_permissions(permissions):
    if not isinstance(permissions, dict) or set(permissions) - {"contents"}:
        raise ValueError("only contents read/none permissions are allowed by current CI policy")
    if any(not isinstance(value, str) or value not in {"read", "none"} for value in permissions.values()):
        raise ValueError("workflow permissions must not grant writes")


def check_release_workflow(document):
    jobs = document.get("jobs", {})
    events = document.get("on", {})
    if not isinstance(events, dict) or not isinstance(events.get("workflow_dispatch"), dict):
        raise ValueError("release workflow requires explicit event and manual-input mappings")
    inputs = events["workflow_dispatch"].get("inputs", {})
    if not isinstance(inputs, dict):
        raise ValueError("release workflow requires explicit manual inputs")
    if (
        set(document) != {"name", "on", "permissions", "jobs"}
        or set(events) != {"release", "workflow_dispatch"}
        or events["release"] != {"types": ["published"]}
        or set(inputs) != {"tag", "dry_run"}
        or set(jobs) != {"validate", "publish"}
    ):
        raise ValueError("release workflow requires published releases and bounded manual inputs only")
    for name, required in (
        ("tag", {"required": "true", "type": "string"}),
        ("dry_run", {"required": "true", "default": "true", "type": "boolean"}),
    ):
        if (
            not isinstance(inputs[name], dict) or set(inputs[name]) != {*required, "description"}
            or any(inputs[name].get(key) != value for key, value in required.items())
            or not isinstance(inputs[name]["description"], str) or not inputs[name]["description"].strip()
        ):
            raise ValueError("manual release runs must default to credential-free dry runs")
    validate, publish = jobs["validate"], jobs["publish"]
    if not isinstance(validate, dict) or not isinstance(publish, dict):
        raise ValueError("release jobs must be mappings")
    if (
        set(validate) != {"if", "runs-on", "timeout-minutes", "permissions", "outputs", "env", "steps"}
        or validate["if"] != RELEASE_VALIDATE or validate["permissions"] != RELEASE_READ
        or validate["timeout-minutes"] != "120" or validate["runs-on"] != "ubuntu-latest"
        or validate["env"] != {"TAG_NAME": RELEASE_TAG}
        or validate["outputs"] != {
            "commit": "${{ steps.source.outputs.commit }}", "version": "${{ steps.source.outputs.version }}",
            "archive_sha256": "${{ steps.registry.outputs.archive_sha256 }}",
            "publish_needed": "${{ steps.registry.outputs.publish_needed }}",
        }
        or set(publish) != {"needs", "if", "runs-on", "timeout-minutes", "environment",
                           "concurrency", "permissions", "env", "steps"}
        or publish["needs"] != "validate" or publish["if"] != RELEASE_PUBLISH
        or publish["permissions"] != RELEASE_OIDC or publish["environment"] != "crates-io"
        or publish["timeout-minutes"] != "15" or publish["runs-on"] != "ubuntu-latest"
        or publish["concurrency"] != {
            "group": "crates-io-jbig2-decoder-${{ github.repository }}", "cancel-in-progress": "false",
        }
        or publish["env"] != {
            "TAG_NAME": RELEASE_TAG, "COMMIT": "${{ needs.validate.outputs.commit }}",
            "VERSION": "${{ needs.validate.outputs.version }}",
            "ARCHIVE_SHA256": "${{ needs.validate.outputs.archive_sha256 }}",
        }
        or not isinstance(validate["steps"], list) or not isinstance(publish["steps"], list)
        or len(validate["steps"]) != 9 or len(publish["steps"]) != 11
        or any(not isinstance(step, dict) for step in validate["steps"] + publish["steps"])
    ):
        raise ValueError("release source validation, dry-run isolation or publisher-only OIDC changed")
    source_command = (
        'cp tools/check_release.py "$RUNNER_TEMP/check-release.py"\n'
        'python -B "$RUNNER_TEMP/check-release.py" --tag "$TAG_NAME" --wait-for-checks \\\n'
        '  --output "$RUNNER_TEMP/release-source.json" --github-output "$GITHUB_OUTPUT"'
    )
    registry_command = (
        'cp "$RUNNER_TEMP/release-source.json" target/release-validation/source.json\n'
        'python -B "$RUNNER_TEMP/check-release.py" --tag "$TAG_NAME" --commit "$COMMIT" \\\n'
        '  --archive "target/package/jbig2-decoder-${VERSION}.crate" \\\n'
        '  --output target/release-validation/registry.json --github-output "$GITHUB_OUTPUT"'
    )
    commands = (
        (validate, 2, {"name": "Validate exact merged source and required checks", "id": "source",
                       "env": {"GH_TOKEN": "${{ github.token }}"}, "run": source_command}),
        (validate, 5, {"name": "Rehearse isolated publication without credentials",
                       "run": "python -B tools/check_distribution.py --toolchain 1.88.0"
                              " --publish-dry-run --output target/release-validation/distribution.json"}),
        (validate, 6, {"name": "Build final registry archive", "run": "cargo +1.88.0 package --locked"}),
        (validate, 7, {"name": "Verify archive and existing registry version", "id": "registry",
                       "env": {"COMMIT": "${{ steps.source.outputs.commit }}",
                               "VERSION": "${{ steps.source.outputs.version }}"}, "run": registry_command}),
        (publish, 1, {"name": "Preserve trusted validation helper",
                      "run": 'cp tools/check_release.py "$RUNNER_TEMP/check-release.py"'}),
        (publish, 5, {"name": "Rebuild immutable candidate", "run": "cargo +1.88.0 package --locked"}),
        (publish, 6, {"name": "Recheck tag archive and registry before authentication", "id": "recheck",
                      "run": 'python -B "$RUNNER_TEMP/check-release.py" --tag "$TAG_NAME" --commit "$COMMIT"'
                             ' --archive "target/package/jbig2-decoder-${VERSION}.crate"'
                             ' --expected-sha256 "$ARCHIVE_SHA256"'
                             ' --output target/release-publication/candidate.json --github-output "$GITHUB_OUTPUT"'}),
        (publish, 8, {"name": "Publish to crates.io", "if": RELEASE_RECHECK,
                      "env": {"CARGO_REGISTRY_TOKEN": "${{ steps.auth.outputs.token }}"},
                      "run": 'cargo +1.88.0 --config \'registry.global-credential-providers=["cargo:token"]\''
                             ' --config \'registries.crates-io.index="sparse+https://index.crates.io/"\''
                             " publish --locked --registry crates-io"}),
        (publish, 9, {"name": "Verify published registry checksum and download",
                      "run": 'python -B "$RUNNER_TEMP/check-release.py" --tag "$TAG_NAME" --commit "$COMMIT"'
                             ' --archive "target/package/jbig2-decoder-${VERSION}.crate"'
                             ' --expected-sha256 "$ARCHIVE_SHA256"'
                             " --verify-published --output target/release-publication/registry.json"}),
    )
    for job, index, expected in commands:
        actual = {**job["steps"][index]}
        if isinstance(actual.get("run"), str):
            actual["run"] = actual["run"].strip()
        if actual != expected:
            raise ValueError("release command, order, credentials or verification changed")
    actions = (
        (validate, 0, "actions/checkout", {"ref": RELEASE_POLICY_REF, "fetch-depth": "0", "persist-credentials": "false"}),
        (validate, 1, "actions/setup-python", {"python-version": "3.13"}),
        (validate, 3, "actions/checkout", {"ref": "${{ steps.source.outputs.commit }}",
                                        "fetch-depth": "0", "persist-credentials": "false"}),
        (validate, 4, "dtolnay/rust-toolchain", {"toolchain": "1.88.0"}),
        (validate, 8, "actions/upload-artifact", {
            "name": "release-validation-${{ github.run_id }}-${{ github.run_attempt }}",
            "path": "target/release-validation/*.json", "if-no-files-found": "error",
        }),
        (publish, 0, "actions/checkout", {"ref": RELEASE_POLICY_REF, "fetch-depth": "0", "persist-credentials": "false"}),
        (publish, 2, "actions/checkout", {"ref": "${{ needs.validate.outputs.commit }}",
                                       "fetch-depth": "0", "persist-credentials": "false"}),
        (publish, 3, "actions/setup-python", {"python-version": "3.13"}),
        (publish, 4, "dtolnay/rust-toolchain", {"toolchain": "1.88.0"}),
        (publish, 10, "actions/upload-artifact", {
            "name": "release-publication-${{ github.run_id }}-${{ github.run_attempt }}",
            "path": "target/release-publication/*.json", "if-no-files-found": "error",
        }),
    )
    for job, index, action, settings in actions:
        step = job["steps"][index]
        if (
            set(step) != {"uses", "with"} or not isinstance(step["uses"], str)
            or step["uses"].split("@")[0] != action or step["with"] != settings
        ):
            raise ValueError("release action, trusted checkout or metadata-only artifact policy changed")
    if publish["steps"][7] != {
        "name": "Authenticate with crates.io Trusted Publishing", "id": "auth", "if": RELEASE_RECHECK,
        "uses": "rust-lang/crates-io-auth-action@3268eb413133e2960fa4ae0e963ca76380c54cd9",
    }:
        raise ValueError("OIDC authentication must use the reviewed pin only after the registry recheck")


def selected_toolchain(step, job):
    settings = step.get("with", {})
    value = settings.get("toolchain") if isinstance(settings, dict) else None
    if not isinstance(value, str):
        raise ValueError("Rust setup requires an explicit toolchain input")
    if TOOLCHAIN.fullmatch(value):
        return value
    if re.fullmatch(r"\$\{\{\s*matrix\.toolchain\s*\}\}", value):
        strategy = job.get("strategy", {})
        matrix = strategy.get("matrix", {}) if isinstance(strategy, dict) else {}
        versions = matrix.get("toolchain") if isinstance(matrix, dict) else None
        if isinstance(versions, list) and versions and all(
            isinstance(version, str) and (version == "stable" or TOOLCHAIN.fullmatch(version))
            for version in versions
        ):
            return value
    raise ValueError("Rust setup requires a fixed version/dated nightly or reviewed stable matrix")


def check_fuzz_producer_serialization(document):
    jobs = document.get("jobs", {})
    producer = jobs.get("fuzz")
    if not isinstance(producer, dict) or not isinstance(producer.get("env"), dict):
        raise ValueError("fuzz corpus producer and exact trust predicate are required")
    if producer["env"].get("TRUSTED_CORPUS") != "${{ " + FUZZ_TRUST + " }}":
        raise ValueError("fuzz corpus trust predicate must remain unchanged")
    if producer.get("concurrency") != {
        "group": FUZZ_PRODUCER_GROUP, "cancel-in-progress": "false",
    }:
        raise ValueError("fuzz corpus producer serialization or untrusted isolation changed")
    if "concurrency" in document or any(
        isinstance(job, dict) and "concurrency" in job
        for name, job in jobs.items() if name != "fuzz"
    ):
        raise ValueError("fuzz concurrency belongs only to the corpus producer job")


def check_verus_sharding(document):
    jobs = document.get("jobs", {})
    if set(jobs) != {"whole", "semantic", "verify"} or not all(isinstance(job, dict) for job in jobs.values()):
        raise ValueError("Verus requires whole, semantic and stable verify collector jobs")
    whole, semantic, verify = (jobs[name] for name in ("whole", "semantic", "verify"))
    if (
        whole.get("timeout-minutes") != "20"
        or semantic.get("timeout-minutes") != "30"
        or verify.get("timeout-minutes") != "20"
    ):
        raise ValueError("Verus job caps must remain 20 minutes, except 30 minutes for semantic shards")
    if (
        semantic.get("needs") != "whole"
        or semantic.get("strategy") != {
            "fail-fast": "false", "max-parallel": "4",
            "matrix": "${{ fromJSON(needs.whole.outputs.matrix) }}",
        }
        or whole.get("outputs") != {"matrix": "${{ steps.groups.outputs.matrix }}"}
        or verify.get("if") != "always()" or verify.get("needs") != ["whole", "semantic"]
        or "name" in verify
    ):
        raise ValueError("Verus dynamic complete matrix/stable fail-closed verify contract changed")
    whole_steps, semantic_steps, verify_steps = (
        job.get("steps", []) for job in (whole, semantic, verify)
    )
    setups = [
        [{key: step[key] for key in ("uses", "with") if key in step}
         for step in steps if step.get("uses", "").split("@")[0] in {
             "actions/checkout", "actions/setup-python", "dtolnay/rust-toolchain",
         }]
        for steps in (whole_steps, semantic_steps, verify_steps)
    ]
    downloads = [
        [{key: value for key, value in step.items() if key != "if"}
         for step in steps if step.get("name") == "Download pinned Verus"]
        for steps in (whole_steps, semantic_steps, verify_steps)
    ]
    if (
        not all(setup == setups[0] for setup in setups) or len(setups[0]) != 3
        or not all(download == downloads[0] for download in downloads) or len(downloads[0]) != 1
    ):
        raise ValueError("all proof jobs must use identical pinned tool setup")
    pins = json.loads((ROOT / "config/toolchain.json").read_text())["verus"]
    download_run = downloads[0][0].get("run", "").replace("\\\n", " ")
    download_run = re.sub(r"\|\s*\n\s*", "| ", download_run)
    download_commands = [
        shlex.split(line, comments=True)
        for line in download_run.splitlines()
        if line.strip()
    ]
    if download_commands != [
        ["curl", "--fail", "--location", "--retry", "3", "--retry-all-errors",
         "--retry-delay", "2", "--retry-max-time", "60", "--connect-timeout", "10",
         "--max-time", "30", "--output", "verus.zip", pins["linux_archive_url"]],
        ["echo", f"{pins['linux_archive_sha256']}  verus.zip", "|", "sha256sum", "--check"],
        ["unzip", "-q", "verus.zip"],
    ]:
        raise ValueError("pinned compiler download requires bounded retries and integrity verification")
    runs = ["\n".join(step.get("run", "") for step in steps) for steps in (
        whole_steps, semantic_steps, verify_steps,
    )]
    commands = [
        [shlex.split(line, comments=True) for line in text.replace("\\\n", " ").splitlines()]
        for text in runs
    ]
    environments = [
        document.get("env", {}), *(job.get("env", {}) for job in jobs.values()),
        *(step.get("env", {}) for steps in (whole_steps, semantic_steps, verify_steps) for step in steps),
    ]
    if not all(isinstance(environment, dict) for environment in environments):
        raise ValueError("Verus validation environments must be mappings")
    if any(
        DISABLED_VERUS_CHECK.search(token)
        for command_list in commands for command in command_list for token in command
    ) or any(
        DISABLED_VERUS_CHECK.search(str(value))
        for environment in environments for value in environment.values()
    ):
        raise ValueError("Verus check-disabling flags are forbidden in validation commands/environment")
    cargo_invocations = [command for command in commands[0] if command[:2] == ["cargo", "verus"]]
    if cargo_invocations != [[
        "cargo", "verus", "build", "--manifest-path", "verification/Cargo.toml",
        "--locked", "--offline", "--", "--num-threads", "4", "--rlimit", "30",
        "--triggers-mode", "silent", "--log", "vir", "--log-dir", "$PWD/target/verus-expanded-raw",
        "--output-json", ">",
        "target/verus-production/cargo-reports.jsons",
    ]]:
        raise ValueError("whole-crate gate requires the exact pinned Cargo verification-and-build invocation")
    report_commands = {
        "tools/collect_proof_mode_inventory.py": [
            "python", "tools/collect_proof_mode_inventory.py",
            "--vir", "target/verus-expanded-raw/crate.vir",
            "--verus-json", "target/verification/verus-production.json",
            "--coverage-json", "target/verification/production-proof-coverage-ci.json",
            "--compiled-library", "target/verus-cargo/debug/libjbig2_rs_production.rlib",
            "--output-dir", "target/traceability/expanded-verifier",
        ],
        "tools/extract_cargo_verus_report.py": [
            "python", "tools/extract_cargo_verus_report.py",
            "--input", "target/verus-production/cargo-reports.jsons",
            "--output", "target/verification/verus-production.json",
            "--compiled-library", "target/verus-cargo/debug/libjbig2_rs_production.rlib",
        ],
        "tools/audit_production_coverage.py": [
            "python", "tools/audit_production_coverage.py",
            "--verus-json", "target/verification/verus-production.json",
            "--output", "target/verification/production-proof-coverage-ci.json",
            "--require-complete", "--require-exact",
        ],
        "tools/check_traceability.py": [
            "python", "tools/check_traceability.py",
            "--verus-json", "target/verification/verus-production.json",
            "--discover-tests", "--require-current-scope",
            "--output", "target/traceability/verifier-report.json",
        ],
    }
    if any(
        [command for command in commands[0] if command[:2] == ["python", script]] != [expected]
        for script, expected in report_commands.items()
    ) or [command for command in commands[0] if command[:2] == ["diff", "-u"]] != [[
        "diff", "-u", "verification/production-proof-coverage.json",
        "target/verification/production-proof-coverage-ci.json",
    ]]:
        raise ValueError("whole proof reports must stay under target with exact extraction/audit/comparison")
    for text in (
        "--require-complete", "--require-exact",
        "diff -u verification/production-proof-coverage.json", "tools/check_traceability.py",
        "--seal-whole target/semantic-ci/whole", "tools/run_semantic_proofs.py --matrix",
        "--whole-groups", "--whole-baseline target/semantic-ci/whole",
        "--output-dir target/semantic-ci/whole/semantic-groups/target/traceability",
    ):
        if text not in runs[0]:
            raise ValueError(f"whole-crate verification gate missing: {text}")
    if '--shard "$SEMANTIC_SHARD"' not in runs[1] or not any(
        step.get("env") == {"SEMANTIC_SHARD": "${{ matrix.shard }}"} for step in semantic_steps
    ):
        raise ValueError("semantic matrix must execute its exact complete selected shard")
    positive_download = [
        (index, step) for index, step in enumerate(semantic_steps)
        if step.get("uses", "").startswith("actions/download-artifact@")
    ]
    semantic_execution = [
        (index, step) for index, step in enumerate(semantic_steps)
        if "tools/run_semantic_proofs.py" in step.get("run", "")
    ]
    if (
        len(positive_download) != 1 or len(semantic_execution) != 1
        or positive_download[0][0] >= semantic_execution[0][0]
        or positive_download[0][1].get("uses")
            != "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333"
        or positive_download[0][1].get("with") != {
            "name": "verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-whole",
            "path": "target/semantic-whole",
        }
        or "--whole-baseline target/semantic-whole"
            not in semantic_execution[0][1].get("run", "")
    ):
        raise ValueError("semantic shards require the pinned current-run whole-positive baseline before execution")
    download = [step for step in verify_steps if step.get("uses", "").startswith("actions/download-artifact@")]
    if len(download) != 1 or download[0].get("with") != {
        "pattern": "verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-*",
        "path": "target/semantic-inputs", "merge-multiple": "false",
    } or download[0].get("uses") != "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333":
        raise ValueError("collector must download unmerged, pinned, current-run/current-attempt artifacts")
    collector_runs = [
        (index, step) for index, step in enumerate(verify_steps)
        if "--collector-groups" in step.get("run", "")
    ]
    if len(collector_runs) != 1:
        raise ValueError("collector must execute every assigned complete semantic group")
    collector_index, collector = collector_runs[0]
    if (
        collector.get("if") != COLLECTOR_READY
        or collector.get("env") != {
            "WHOLE_BASELINE": "target/semantic-inputs/verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-whole",
            "COLLECTOR_OUTPUT": "target/semantic-inputs/verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-collector/target/traceability",
        }
        or shlex.split(collector.get("run", "")) != [
            "python", "-B", "tools/run_semantic_proofs.py", "--collector-groups",
            "--verus-executable", "verus-x86-linux/verus",
            "--whole-baseline", "$WHOLE_BASELINE", "--output-dir", "$COLLECTOR_OUTPUT",
        ]
        or verify_steps.index(download[0]) >= collector_index
        or not any(
            "--aggregate target/semantic-inputs" in step.get("run", "")
            for step in verify_steps[collector_index + 1:]
        )
    ):
        raise ValueError("collector semantic groups require exact current-run inputs before aggregation")
    if not any(
        step.get("env") == {
            "WHOLE_RESULT": "${{ needs.whole.result }}", "SEMANTIC_RESULT": "${{ needs.semantic.result }}",
        } and all(text in step.get("run", "") for text in (
            "--aggregate target/semantic-inputs",
            '--whole-result "$WHOLE_RESULT" --semantic-result "$SEMANTIC_RESULT"',
            "--output-dir target/semantic-canonical",
        )) for step in verify_steps
    ):
        raise ValueError("collector must check both upstream results and all evidence")
    uploads = {}
    for name, steps in (("whole", whole_steps), ("semantic", semantic_steps), ("verify", verify_steps)):
        matches = [step for step in steps if step.get("uses", "").startswith("actions/upload-artifact@")]
        if len(matches) != (2 if name == "verify" else 1) or any(
            step.get("if") != "always()" for step in matches
        ):
            raise ValueError("each Verus job must retain failed diagnostics")
        uploads[name] = [step.get("with") for step in matches]
        if any(
            "if" in step and not (
                name == "verify"
                and step.get("if") == COLLECTOR_READY
                and (step.get("name") in {
                    "Download pinned Verus",
                    "Check every complete semantic group assigned to the collector",
                } or step.get("uses", "").startswith("dtolnay/rust-toolchain@"))
            )
            for step in steps
            if step.get("name") != "Stage whole-crate outputs including failed diagnostics"
            and not step.get("uses", "").startswith("actions/upload-artifact@")
        ):
            raise ValueError("Verus evidence-producing/collecting steps must not be conditional")
    if uploads != {
        "whole": [{"name": "verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-whole",
                   "path": "target/semantic-ci/whole/"}],
        "semantic": [{"name": "verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-shard-${{ matrix.shard }}",
                      "path": "target/semantic-ci/group/"}],
        "verify": [
            {"name": "verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-collector",
             "path": "target/semantic-inputs/verus-partial-${{ github.run_id }}-${{ github.run_attempt }}-collector/"},
            {"name": "verus-bootstrap-log", "path": "target/semantic-canonical/"},
        ],
    }:
        raise ValueError("Verus immutable partial/canonical artifact names or layout changed")


def check_native_mutation_lane(document):
    job = document.get("jobs", {}).get("cargo", {})
    steps = job.get("steps", [])
    nightly = json.loads((ROOT / "config/toolchain.json").read_text())["coverage"]["rust_toolchain"]
    expected = (
        ("Install pinned native mutation compiler",
         f"rustup toolchain install {nightly} --profile minimal"),
        ("Reconcile actual production compiler inventory",
         "python -B tools/collect_compiler_inventory.py "
         "--output-dir target/traceability/compiler-inventory"),
        ("Check critical public-API mutation sensitivity",
         f"python -B tools/check_contract_mutations.py --toolchain {nightly} "
         "--output target/traceability/mutations.json"),
        ("Freeze approved internal declaration profile",
         "python -B tools/check_internal_profile.py "
         "--compiler-inventory target/traceability/compiler-inventory "
         "--output target/traceability/internal-profile.json"),
    )
    positions = []
    for name, command in expected:
        matches = [(index, step) for index, step in enumerate(steps) if step.get("name") == name]
        if (
            len(matches) != 1 or matches[0][1].get("if") != "runner.os == 'Linux'"
            or shlex.split(matches[0][1].get("run", "")) != shlex.split(command)
        ):
            raise ValueError("native mutation checks require the exact pinned Linux-only lane")
        positions.append(matches[0][0])
    uploads = [
        (index, step) for index, step in enumerate(steps)
        if step.get("uses", "").startswith("actions/upload-artifact@")
    ]
    if (
        job.get("timeout-minutes") != "20"
        or len(uploads) != 1 or uploads[0][1].get("if") != "always()"
        or uploads[0][1].get("with") != {
            "name": "traceability-${{ matrix.os }}", "path": "target/traceability/",
        }
        or positions != sorted(set(positions))
        or positions[-1] >= uploads[0][0]
    ):
        raise ValueError("native mutations require the existing 20-minute job and retained raw artifact")


def check_workflow(document, name):
    if document.get("permissions") != {"contents": "read"}:
        raise ValueError(f"{name}: workflow must default to contents: read only")
    events = document.get("on")
    if not isinstance(events, (dict, list, str)) or not events or "pull_request_target" in events:
        raise ValueError(f"{name}: missing or unsupported privileged workflow trigger")
    if isinstance(events, list) and any(not isinstance(event, str) for event in events):
        raise ValueError(f"{name}: workflow event names must be strings")
    if Path(name).stem in VALIDATION_WORKFLOWS and (
        not isinstance(events, dict) or "pull_request" not in events
        or events.get("pull_request") not in ("", None, {})
        or "workflow_dispatch" not in events
        or events.get("push") != {"branches": ["main", "release/0.1", "release/0.2"], "tags": ["**"]}
    ):
        raise ValueError("validation runs on every PR, main/release/0.1/release/0.2/tag pushes and manual dispatch")
    jobs = document.get("jobs")
    if not isinstance(jobs, dict) or not jobs:
        raise ValueError(f"{name}: workflow must contain jobs")
    if name in {"fuzz.yml", "fuzz.yaml"}:
        check_fuzz_producer_serialization(document)
    if name in {"verus.yml", "verus.yaml"}:
        check_verus_sharding(document)
    if name in {"ci.yml", "ci.yaml"}:
        check_native_mutation_lane(document)
        distribution = jobs.get("distribution", {})
        rehearsals = [
            step for step in distribution.get("steps", [])
            if step.get("name") == "Test isolated proof-free distribution"
        ]
        if (
            len(rehearsals) != 1
            or "if" in rehearsals[0]
            or shlex.split(rehearsals[0].get("run", "")) != shlex.split(
                "python -B tools/check_distribution.py --toolchain ${{ matrix.toolchain }}"
                " --publish-dry-run --output target/distribution/report.json"
            )
        ):
            raise ValueError("distribution checks require the actual isolated publication dry run")
    if name in {"release.yml", "release.yaml"}:
        check_release_workflow(document)
    if name in {"conformance.yml", "conformance.yaml"}:
        if document["on"] != {"workflow_dispatch": ""} or set(jobs) != {"official"}:
            raise ValueError("official conformance requires manual trusted-main execution only")
        official = jobs["official"]
        steps = official.get("steps", [])
        private = [step for step in steps if step.get("name") == "Require private repository"]
        fetches = [index for index, step in enumerate(steps)
                   if step.get("name") == "Fetch exact approved conformance vectors"]
        if (
            official.get("if") != OFFICIAL_TRUST
            or official.get("timeout-minutes") != "10"
            or len(private) != 1 or len(fetches) != 1
            or "if" in private[0]
            or private[0].get("env") != {"GH_TOKEN": "${{ github.token }}"}
            or private[0].get("run") != 'test "$(gh api repos/microsoft/jbig2_rs --jq .private)" = true'
            or steps.index(private[0]) >= fetches[0]
            or steps[fetches[0]].get("run") != (
                "python -B validation/conformance/fetch_itu.py"
                " --download --output target/official-vectors"
            )
        ):
            raise ValueError("official conformance requires private identity before pinned acquisition")
        uploads = [step for step in steps
                   if step.get("uses", "").startswith("actions/upload-artifact@")]
        if len(uploads) != 1 or uploads[0].get("with") != {
            "name": "official-conformance-${{ github.run_id }}-${{ github.run_attempt }}",
            "path": "target/official-results/results.json\n"
                    "target/official-results/source-revision.txt\n",
        }:
            raise ValueError("official conformance may upload only metadata reports, never assets")
        runs = [step for step in steps if step.get("name") == "Run all original official streams"]
        if len(runs) != 1 or "if" in runs[0] or shlex.split(runs[0].get("run", "")) != [
            "python", "-B", "validation/conformance/run_itu.py",
            "--vectors", "target/official-vectors",
            "--decoder", "target/release/examples/decode_vector",
            "--expected-differences", "validation/conformance/expected-differences.json",
            "--output", "target/official-results/results.json",
        ]:
            raise ValueError("official conformance must execute the complete fail-closed runner")
    actions = []
    for job_name, job in jobs.items():
        if not isinstance(job, dict):
            raise ValueError(f"{name}/{job_name}: job must be a mapping")
        if "permissions" in job:
            if name not in {"release.yml", "release.yaml"}:
                read_permissions(job["permissions"])
        timeout = job.get("timeout-minutes")
        maximum = 120 if name in {"release.yml", "release.yaml"} and job_name == "validate" else 60
        if (
            not isinstance(timeout, str) or not timeout.isascii() or not timeout.isdecimal()
            or not 1 <= int(timeout) <= maximum
        ):
            raise ValueError(f"{name}/{job_name}: explicit timeout must be within 1..{maximum} minutes")
        steps = job.get("steps")
        if "uses" in job or not isinstance(steps, list) or not steps:
            raise ValueError(f"{name}/{job_name}: explicit steps required; reusable jobs need policy review")
        if job.get("continue-on-error", "false") != "false":
            raise ValueError(f"{name}/{job_name}: failures must not be masked")
        for step in steps:
            if not isinstance(step, dict):
                raise ValueError(f"{name}/{job_name}: step must be a mapping")
            if "uses" in step and "run" in step:
                raise ValueError(f"{name}/{job_name}: a step must choose either uses or run")
            if step.get("continue-on-error", "false") != "false" or "permissions" in step:
                raise ValueError(f"{name}/{job_name}: step failure/permission override is unsupported")
            if "uses" not in step:
                continue
            reference = step["uses"]
            if not isinstance(reference, str) or not ACTION.fullmatch(reference):
                raise ValueError(f"{name}/{job_name}: action requires a full commit SHA: {reference}")
            entry = {"workflow": name, "job": job_name, "uses": reference}
            if reference.split("@")[0] == "dtolnay/rust-toolchain":
                entry["toolchain"] = selected_toolchain(step, job)
            actions.append(entry)
    if not actions:
        raise ValueError(f"{name}: no pinned actions were checked")
    return actions


def check_updates(document):
    updates = document.get("updates")
    if document.get("version") != "2" or not isinstance(updates, list):
        raise ValueError("Dependabot version 2 action-update policy is required")
    matching = [
        entry for entry in updates if isinstance(entry, dict)
        and entry.get("package-ecosystem") == "github-actions" and entry.get("directory") == "/"
    ]
    if len(matching) != 1 or matching[0].get("schedule") != {"interval": "weekly"}:
        raise ValueError("exactly one weekly root GitHub Actions update entry is required")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        arguments.output.unlink(missing_ok=True)
        directory = ROOT / ".github/workflows"
        paths = sorted([*directory.glob("*.yml"), *directory.glob("*.yaml")])
        if not paths:
            raise ValueError("no workflow files were found")
        actions = []
        bindings = {}
        for path in paths:
            actions.extend(check_workflow(load_workflow(path.read_text()), path.name))
            bindings[path.relative_to(ROOT).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
        policy = ROOT / ".github/dependabot.yml"
        check_updates(load_workflow(policy.read_text()))
        bindings[policy.relative_to(ROOT).as_posix()] = hashlib.sha256(policy.read_bytes()).hexdigest()
        agent = ROOT / ".github/agents/verus-proof-worker.agent.md"
        profile = check_proof_agent(agent.read_text())
        bindings[agent.relative_to(ROOT).as_posix()] = hashlib.sha256(agent.read_bytes()).hexdigest()
        auditor = ROOT / ".github/agents/specification-auditor.agent.md"
        check_specification_auditor(auditor.read_text())
        bindings[auditor.relative_to(ROOT).as_posix()] = hashlib.sha256(auditor.read_bytes()).hexdigest()
        report = {
            "schema_version": 1, "status": "workflow_policy_passed",
            "workflow_count": len(paths), "action_count": len(actions), "actions": actions,
            "source_bindings": bindings,
            "configured_proof_agent_model": profile["model"],
            "source_revision": subprocess.run(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True,
            ).stdout.strip(),
            "note": "Checks configuration pins, explicit toolchains, read-only permissions, job budgets, bounded complete Verus sharding/fail-closed collection, fuzz producer serialization/isolation, update configuration and requested proof-agent profile. Not serving-model confirmation, upstream code approval, protected-branch enforcement, trigger execution or full readiness evidence.",
        }
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({key: report[key] for key in ("status", "workflow_count", "action_count")}))
        return 0
    except (OSError, ValueError, yaml.YAMLError, subprocess.CalledProcessError) as error:
        print(f"workflow policy validation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
