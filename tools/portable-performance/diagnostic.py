"""Explicit setup and separately authorized full-cohort latency entry points."""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import sys
import time

from build import BASE, build, command, revision
from fixtures import HERE, PRIVATE, REPO, RESULTS, checked, materialize, save, sha


def protocol_sources():
    provenance = json.loads((HERE / "provenance.json").read_text())
    for name, row in provenance.items():
        path = HERE / "original" / (name + ".txt")
        if path.exists() and sha(path) != row["sha256"]:
            raise ValueError("Original source/metadata identity changed: " + name)
    if (HERE / "probe/Cargo.lock").read_bytes() != (HERE / "original/probe-Cargo.lock.txt").read_bytes():
        raise ValueError("Original probe dependency lockfile changed")
    original = (HERE / "original/harness.rs.txt").read_text()
    actual = (HERE / "probe/src/main.rs").read_text()
    if actual != original.replace("/workspace/jbig2/benchmarks/cases.rs",
                                   "../../../../benchmarks/cases.rs"):
        raise ValueError("Timing harness differs beyond the documented machine-path port")
    result = {
        str(path.relative_to(REPO)): sha(path)
        for path in sorted(HERE.rglob("*")) if path.is_file()
        and not {"__pycache__", "validation", "target"} & set(path.relative_to(HERE).parts)
    }
    for name in (".github/workflows/performance.yml", "benchmarks/cases.rs",
                 "tools/pdfium-qualification/prepare_corpus.py",
                 "tools/pdfium-qualification/frozen/cohort.json",
                 "tools/pdfium-qualification/requirements.txt"):
        result[name] = sha(REPO / name)
    return result


def verify_files(cohort):
    runtime = json.loads((PRIVATE / "expanded-cases.json").read_text())
    if len(runtime) != len(cohort["cases"]):
        raise ValueError("Runtime manifest lost cases")
    for case, frozen in zip(runtime, cohort["cases"]):
        if any(case[key] != frozen[key] for key in (
            "id", "family", "width", "height", "pages", "policy", "standalone",
        )):
            raise ValueError("Runtime case identity/shape/policy changed")
        for field in ("input", "globals", "expected"):
            if frozen[field] is None:
                if case[field] is not None:
                    raise ValueError("Unexpected globals")
            else:
                checked(Path(case[field]), frozen[field])


def setup(candidate):
    start = time.monotonic()
    RESULTS.mkdir(parents=True, exist_ok=True)
    if (RESULTS / "setup.json").exists():
        raise FileExistsError("Preserve the previous setup; use a fresh disposable checkout")
    candidate = revision(candidate)
    frozen = json.loads((HERE / "cohort.json").read_text())
    sources = protocol_sources()
    allowed = sorted(os.sched_getaffinity(0))
    if not allowed:
        raise ValueError("No available CPU for deterministic affinity")
    cpu = allowed[0]
    host = {
        "platform": platform.platform(), "machine": platform.machine(), "available_cpus": allowed,
        "selected_cpu": cpu, "lscpu": command(["lscpu"], capture_output=True).stdout,
        "uname": command(["uname", "-a"], capture_output=True).stdout,
        "runner": {key: os.environ.get(key) for key in (
            "RUNNER_NAME", "RUNNER_OS", "RUNNER_ARCH", "ImageOS", "ImageVersion",
            "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_SHA",
        )},
    }
    save(RESULTS / "configuration.json", {
        "baseline": BASE, "candidate": candidate, "inputs": 129, "configs": 133,
        "correctness_checks_per_binary": 492, "rounds": 9, "samples": 31, "target_ms": 350,
        "calibration_cases": frozen["calibration_cases"], "context_controls": frozen["context_controls"],
        "host": host, "sources_sha256": sources,
        "artifact_policy": "Results/configuration/logs only. NEVER upload private corpus/pixels/binaries.",
        "scope": "Native hosted diagnostic, not Native_N2/LLVM24/PDFium qualification.",
        "setup_started_epoch": time.time(),
        "workflow_deadline_epoch": os.environ.get("PORTABLE_DEADLINE"),
        "job_timeout_minutes": 20, "reserved_finalization_seconds": 60,
    })
    cases = materialize()
    context_controls = {"bitmap-mmr", "generated-transitions-64",
                        "generated-transitions-512", "generated-white-2048"}
    if len({case["id"] for case in cases}) != 129 or (
        len(cases) + sum(case["id"] in context_controls for case in cases) != 133
    ) or set(frozen["context_controls"]) != context_controls:
        raise ValueError("All129 inputs and all four context controls are mandatory")
    baseline = build(BASE, "baseline", materializer=True)
    materializer = Path(baseline["binaries"]["jbig2-golden-materializer"]["path"])
    if sha(materializer) != baseline["binaries"]["jbig2-golden-materializer"]["sha256"]:
        raise ValueError("BASE materializer changed")
    result = command([str(materializer), str(PRIVATE / "expanded-cases.json"),
                      str(PRIVATE / "goldens")], capture_output=True)
    shapes = json.loads(result.stdout)
    if len(shapes) != 129:
        raise ValueError("Incomplete BASE materialization")
    for shape, case in zip(shapes, frozen["cases"]):
        if any(shape[field] != case[field] for field in ("id", "width", "height", "pages")):
            raise ValueError("BASE shape differs from independently frozen shape")
    verify_files(frozen)
    for path in (PRIVATE / "goldens").glob("*.packed"):
        path.chmod(0o444)
    save(RESULTS / "golden-verification.json", {
        "status": "all-129-frozen-hashes-and-shapes-match", "baseline": BASE,
        "materializer_sha256": sha(materializer),
        "expected": [{"id": row["id"], **row["expected"]} for row in frozen["cases"]],
        "scope": "BASE only materialized bytes; expected hashes were independently frozen, never regenerated.",
    })
    records = {"baseline": baseline, "candidate": build(candidate, "candidate")}
    validations = {}
    for label, record in records.items():
        executable = record["binaries"]["jbig2-api-validation"]
        if sha(executable["path"]) != executable["sha256"]:
            raise ValueError("Correctness binary changed")
        process = command(["taskset", "-c", str(cpu), executable["path"],
                           str(PRIVATE / "expanded-cases.json")], capture_output=True)
        rows = json.loads(process.stdout)
        if len(rows) != 492 or not all(row["exact_output_checked"] for row in rows):
            raise ValueError("All 492 API checks are mandatory")
        validations[label] = rows
        save(RESULTS / (label + "-correctness.json"), rows)
        verify_files(frozen)
    if validations["baseline"] != validations["candidate"]:
        raise ValueError("Candidate output/recovery/metadata differs from BASE")
    files = {}
    for case in cases:
        for field in ("input", "globals", "expected"):
            if case[field] is not None:
                path = Path(case[field])
                files[str(path)] = {"sha256": sha(path), "bytes": path.stat().st_size}
    path = PRIVATE / "expanded-cases.json"
    files[str(path)] = {"sha256": sha(path), "bytes": path.stat().st_size}
    save(PRIVATE / "expanded-cohort.json", {"manifest": str(path), "files": files})
    if sources != protocol_sources():
        raise ValueError("Protocol source changed during setup")
    save(RESULTS / "setup.json", {
        "status": "prepared-and-correctness-verified", "baseline": BASE, "candidate": candidate,
        "inputs": 129, "configs": 133, "api_checks_per_binary": 492,
        "cpu": cpu, "setup_seconds": time.monotonic() - start,
        "cohort_sha256": sha(PRIVATE / "expanded-cohort.json"),
        "sources_sha256": sources, "latency_executed": False,
        "builds": {label: {
            "record_sha256": sha(PRIVATE / "binaries" / (label + ".json")),
            "binary_sha256": row["binary_sha256"], "revision": row["source_revision"],
        } for label, row in records.items()},
    })
    print("Prepared all129 inputs/133 configurations; both492 API checks pass. No latency executed.")


def measure(authorized):
    if not authorized:
        raise ValueError("Latency requires explicit --authorize-latency")
    ready = json.loads((RESULTS / "setup.json").read_text())
    frozen = json.loads((HERE / "cohort.json").read_text())
    if ready["status"] != "prepared-and-correctness-verified" or ready["sources_sha256"] != protocol_sources():
        raise ValueError("Run unchanged complete setup before any timing")
    if sha(PRIVATE / "expanded-cohort.json") != ready["cohort_sha256"]:
        raise ValueError("Prepared cohort changed")
    for label, row in ready["builds"].items():
        if sha(PRIVATE / "binaries" / (label + ".json")) != row["record_sha256"] \
                or sha(PRIVATE / "binaries" / label) != row["binary_sha256"]:
            raise ValueError("Prepared build record or binary changed: " + label)
    verify_files(frozen)
    env = {**os.environ, "PERF_DIAGNOSTIC_ROOT": str(PRIVATE),
           "PERF_DIAGNOSTIC_CPU": str(ready["cpu"])}
    common = [sys.executable, "-B", str(HERE / "confirm_probes.py"),
              "--rounds", "9", "--samples", "31", "--target-ms", "350"]
    calibration = common + ["--label", "calibration", "--pair", "baseline-calibration:baseline:baseline",
                            "--calibration"]
    for case in frozen["calibration_cases"]:
        calibration += ["--case", case]
    for args in (calibration, common + ["--label", "confirmation", "--pair", "candidate:baseline:candidate"]):
        command(args, env=env)
        for path, row in json.loads((PRIVATE / "expanded-cohort.json").read_text())["files"].items():
            checked(Path(path), row)
    for name in ("calibration", "confirmation"):
        path = PRIVATE / (name + ".json")
        report = json.loads(path.read_text())
        expected = 133 if name == "confirmation" else sum(
            1 + int(row["id"] in frozen["context_controls"])
            for row in frozen["cases"] if row["id"] in frozen["calibration_cases"]
        )
        status = "completed-calibration" if name == "calibration" else "completed-confirmation"
        if report["tasks"] != expected or len(report["summary"]) != expected or (
            len(report["measurements"]) != expected * 9
            or report["status"] != status
            or (report["rounds"], report["samples"], report["target_ms"]) != (9, 31, 350)
        ):
            raise ValueError("Never accept an incomplete or filtered confirmation")
        journal = PRIVATE / (name + ".sides.jsonl")
        if len(journal.read_text().splitlines()) != expected * 18:
            raise ValueError("Incomplete completed-side journal")
        shutil.copyfile(journal, RESULTS / (name + ".sides.jsonl"))
        for row in report["summary"]:
            pairs = [p for p in report["measurements"] if (p["id"], p["mode"]) == (row["id"], row["mode"])]
            row["over_5pct_slower_pairs"] = sum(p["ratio"] > 1.05 for p in pairs)
            row["p95_before_ns"] = [p["before"].get("p95_ns", p["before"]["median_ns"]) for p in pairs]
            row["p95_after_ns"] = [p["after"].get("p95_ns", p["after"]["median_ns"]) for p in pairs]
        save(RESULTS / (name + ".json"), report)
    verify_files(frozen)
    if ready["sources_sha256"] != protocol_sources():
        raise ValueError("Protocol changed during the diagnostic")
    save(RESULTS / "completion.json", {
        "status": "complete", "inputs": 129, "configs": 133, "pairs": 1197,
        "baseline": BASE, "candidate": ready["candidate"], "calibration": "all-predeclared-controls-retained",
        "confirmation_sha256": sha(RESULTS / "confirmation.json"),
        "qualification": "Diagnostic only; all adverse shared-host and hosted records remain evidence.",
    })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("setup", "measure", "stage-partial"))
    parser.add_argument("--candidate", default="d66db50bca76c8650b29346592151d81427f8323")
    parser.add_argument("--authorize-latency", action="store_true")
    args = parser.parse_args()
    if args.action == "setup":
        setup(args.candidate)
    elif args.action == "measure":
        measure(args.authorize_latency)
    else:
        # Preserve partial raw records on timeout/failure without ever copying private payloads.
        for name in ("calibration", "confirmation"):
            for suffix in (".json", ".sides.jsonl"):
                path = PRIVATE / (name + suffix)
                if path.exists():
                    shutil.copyfile(path, RESULTS / (name + "-partial" + suffix))


if __name__ == "__main__":
    main()
