"""Actual public-API replication, separate from the frozen private experiment."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import re
import statistics
import subprocess

from prepared_globals_replication import ROOT, prepare, select_binary


TEST = "decode::tests::prepared_globals::probe::prepared_globals_public_sequence_probe"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def operation(binary, fixture, mode, cpu, directory, label):
    process = subprocess.run(
        ["taskset", "-c", str(cpu), str(binary), "--ignored", "--exact", TEST,
         "--nocapture", "--test-threads=1"],
        text=True, capture_output=True, timeout=180, check=False,
        env={**os.environ, "JBIG2_PREPARED_FIXTURE_DIR": str(fixture),
             "JBIG2_PREPARED_MODE": mode, "JBIG2_PREPARED_ITERATIONS": "1001"},
    )
    (directory / (label + ".stdout")).write_text(process.stdout)
    (directory / (label + ".stderr")).write_text(process.stderr)
    process.check_returncode()
    totals = re.findall(
        r"prepared-public-sequence mode=(\w+) iterations=(\d+) pages=(\d+) "
        r"elapsed_ns=(\d+) exact=true public_info=(.*?) owner_info_snapshot_cost_included=(true|false)",
        process.stdout,
    )
    raw = re.findall(r"^samples_ns=(\[[^\r\n]*\])$", process.stdout, re.M)
    if len(totals) != 1 or len(raw) != 1 or "test result: ok. 1 passed;" not in process.stdout:
        raise ValueError("Missing exact successful public-production probe result")
    actual, count, pages, total, info, included = totals[0]
    samples = json.loads(raw[0])
    if (
        actual != mode or int(count) != 1001 or int(pages) != 3003
        or len(samples) != 1001 or min(samples) <= 0 or sum(samples) != int(total)
        or included != ("false" if mode == "baseline" else "true")
    ):
        raise ValueError("Public probe identity/sample/lifetime mismatch")
    if mode == "baseline":
        if info != "None":
            raise ValueError("Baseline unexpectedly used prepared resources")
        resident = 0
    else:
        match = re.fullmatch(
            r"Some\(PreparedGlobalsInfo \{ state: Ready, additional_storage_bytes: 65536, "
            r"resident_bytes: (\d+) \}\)", info,
        )
        if match is None or not 0 < int(match[1]) <= 65_536:
            raise ValueError("Actual public owner did not report admitted bounded resources")
        resident = int(match[1])
    return {
        "mode": mode, "samples_ns": samples, "median_ns": statistics.median(samples),
        "p95_ns": sorted(samples)[math.ceil(len(samples) * 0.95) - 1],
        "public_info": info, "observed_resident_bytes": resident,
        "owner_info_snapshot_cost_included": included == "true",
    }


def measure(build, fixture, output):
    if output.exists():
        raise FileExistsError("Preserve the previous public-production measurement")
    binary = select_binary(build)
    cpu = min(os.sched_getaffinity(0))
    directory = output.with_suffix(".processes")
    directory.mkdir(parents=True)
    for control in ("prepared_globals_fixture_controls",
                    "prepared_globals_public_probe_functional_controls"):
        process = subprocess.run(
            [str(binary), "--ignored", "--exact",
             "decode::tests::prepared_globals::probe::" + control,
             "--nocapture", "--test-threads=1"],
            env={**os.environ, "JBIG2_PREPARED_FIXTURE_DIR": str(fixture)},
            text=True, capture_output=True, timeout=180, check=False,
        )
        (directory / (control + ".stdout")).write_text(process.stdout)
        (directory / (control + ".stderr")).write_text(process.stderr)
        process.check_returncode()
        if "test result: ok. 1 passed;" not in process.stdout:
            raise ValueError("The exact public/limit/refusal control did not execute")
    bindings = {path.name: sha(path) for path in fixture.iterdir() if path.is_file()}
    report = {
        "scope": "Actual production PreparedGlobals API versus the unchanged ordinary API in one matched native binary. Cold includes construction/preparation/copies/all output drops/complete owner destruction and a public info snapshot. Warm is secondary same-owner repetition. Normalized original standalone pages are not an observed production PDF request trace, standalone/default-API improvement, full-PDF gain or LLVM24/PDFium qualification.",
        "memory_scope": "Explicit additional65536-byte owner/copy allowance; info reports observed resident payload capacities, not peak copy capacity, RSS or equal total budgets. Public snapshot cost is included only in candidate sequence timing; assertions/aggregation/printing are outside samples.",
        "calibration_rule": "Identical ordinary-API baseline/baseline paired95 interval must lie within0.95–1.05 before labeling timing uncertainty resolved. All results, including unsuccessful calibration, remain evidence.",
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True,
        ).strip(),
        "binary_sha256": sha(binary), "protocol_sha256": sha(Path(__file__)),
        "fixture_producer_sha256": sha(ROOT / "tools/prepared_globals_replication.py"),
        "fixture_sha256": bindings, "cpu_affinity": cpu, "rounds": 9,
        "architecture": platform.machine(),
        "cpu_identity": {
            key: sorted({line.split(":", 1)[1].strip()
                         for line in Path("/proc/cpuinfo").read_text().splitlines()
                         if ":" in line and line.split(":", 1)[0].strip() == key})
            for key in ("model name", "CPU implementer", "CPU architecture",
                        "CPU part", "CPU variant", "CPU revision")
        },
        "rustc": subprocess.check_output(["rustc", "+1.98.1", "-vV"], text=True),
        "build_profile": {key: os.environ[key] for key in (
            "RUSTFLAGS", "CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
            "CARGO_PROFILE_RELEASE_DEBUG",
        )},
        "measurements": [],
    }
    output.write_text(json.dumps(report, indent=2) + "\n")
    rng = random.Random(20261125)
    for round_index in range(9):
        comparisons = [("cold", "cold"), ("warm", "warm"), ("calibration", "baseline")]
        rng.shuffle(comparisons)
        for name, mode in comparisons:
            order = [("before", "baseline"), ("after", mode)]
            if round_index % 2:
                order.reverse()
            pair = {"round": round_index, "comparison": name, "order": [side for side, _ in order]}
            for side, actual_mode in order:
                pair[side] = operation(
                    binary, fixture, actual_mode, cpu, directory, f"{name}-{round_index}-{side}",
                )
            pair["ratio"] = pair["after"]["median_ns"] / pair["before"]["median_ns"]
            pair["p95_ratio"] = pair["after"]["p95_ns"] / pair["before"]["p95_ns"]
            report["measurements"].append(pair)
        print(f"public-production paired round {round_index + 1}/9", flush=True)
        output.write_text(json.dumps(report, indent=2) + "\n")
    summary = []
    for name in ("cold", "warm", "calibration"):
        pairs = [row for row in report["measurements"] if row["comparison"] == name]
        ratios = [row["ratio"] for row in pairs]
        bootstrap = sorted(statistics.median(rng.choices(ratios, k=9)) for _ in range(5000))
        summary.append({
            "comparison": name, "paired_median_ratio": statistics.median(ratios),
            "paired_bootstrap95": [bootstrap[125], bootstrap[4874]],
            "faster_pairs": sum(value < 1 for value in ratios),
            "paired_p95_ratio": statistics.median(row["p95_ratio"] for row in pairs),
        })
    if sha(binary) != report["binary_sha256"] or bindings != {
        path.name: sha(path) for path in fixture.iterdir() if path.is_file()
    }:
        raise ValueError("Public-production binary or fixtures changed")
    report.update(status="complete-public-production-sequence-replication", summary=summary)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "measure"))
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--build-json", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.fixtures.resolve())
    elif args.build_json is None or args.output is None:
        parser.error("Public measurement requires --build-json and --output")
    else:
        measure(args.build_json, args.fixtures.resolve(), args.output)
