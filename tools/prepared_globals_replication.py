"""Manual, unmerged independent-runner replication of the private cache probe."""

import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import random
import re
import statistics
import subprocess
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
PIN = "e7ada1354dff4041cdeed3d8ba56fe01b6186132"
TEST = "decode::tests::prepared_globals::probe::prepared_globals_sequence_probe"
NAMES = ("file_pages", "lenient_end_of_page_of_previous_page")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    os.environ.update(JBIG2_EVAL_RESULTS=str(directory), JBIG2_PDFIUM_SOURCE=str(ROOT))
    spec = importlib.util.spec_from_file_location(
        "qualification_prepare", ROOT / "tools/pdfium-qualification/prepare_corpus.py",
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    cohort = json.loads((ROOT / "tools/pdfium-qualification/frozen/cohort.json").read_text())
    for name in NAMES:
        rows = [row for row in cohort["images"] if row["id"].startswith("hayro/" + name + "-page")]
        url = f"https://raw.githubusercontent.com/LaurenzV/hayro/{PIN}/hayro-jbig2/tests/streams/{name}.jbig2"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(rows[0]["original"]["size"] + 1)
        if hashlib.sha256(data).hexdigest() != rows[0]["original"]["sha256"]:
            raise ValueError("original fixture identity changed")
        (directory / (name + ".jbig2")).write_bytes(data)
        for number, width, height, page, globals_bytes in module.embedded_pages(data):
            row = next(row for row in rows if row["id"] == f"hayro/{name}-page{number}")
            if (
                (width, height) != (row["width"], row["height"])
                or hashlib.sha256(page).hexdigest() != row["sha256"]
                or hashlib.sha256(globals_bytes).hexdigest() != row["globals"]["sha256"]
            ):
                raise ValueError("normalized fixture identity changed")
            (directory / f"{name}-page{number}.jb2").write_bytes(page)
            (directory / (name + ".globals")).write_bytes(globals_bytes)
    print("Both original sources and all six normalized page/global identities match.")


def select_binary(build):
    rows = [json.loads(line) for line in build.read_text().splitlines()]
    binaries = [Path(row["executable"]).resolve() for row in rows
                if row.get("executable") and row["target"]["name"] == "jbig2_decoder"
                and Path(row["manifest_path"]).resolve() == ROOT / "Cargo.toml"
                and row["profile"]["test"] is True]
    if len(binaries) != 1 or not binaries[0].is_relative_to(ROOT / "target/prepared-native-matched"):
        raise ValueError("need exactly one actual matched library-test binary")
    return binaries[0]


def operation(binary, fixture, mode, cpu, directory, label):
    env = {
        **os.environ, "JBIG2_PREPARED_FIXTURE_DIR": str(fixture),
        "JBIG2_PREPARED_MODE": mode, "JBIG2_PREPARED_ITERATIONS": "1001",
    }
    process = subprocess.run(
        ["taskset", "-c", str(cpu), str(binary), "--ignored", "--exact", TEST,
         "--nocapture", "--test-threads=1"],
        text=True, capture_output=True, env=env, timeout=180, check=False,
    )
    (directory / (label + ".stdout")).write_text(process.stdout)
    (directory / (label + ".stderr")).write_text(process.stderr)
    process.check_returncode()
    totals = re.findall(
        r"prepared-sequence mode=(\w+) iterations=(\d+) pages=(\d+) elapsed_ns=(\d+) "
        r"exact=true resident_bytes=(\d+) peak_owner_plus_copy_bytes=(\d+)", process.stdout,
    )
    raw = re.findall(r"^samples_ns=(\[[^\r\n]*\])$", process.stdout, re.M)
    if len(totals) != 1 or len(raw) != 1 or "test result: ok. 1 passed;" not in process.stdout:
        raise ValueError("missing exact successful probe result")
    actual, count, pages, total, resident, peak = totals[0]
    samples = json.loads(raw[0])
    if (
        actual != mode or int(count) != 1001 or int(pages) != 3003
        or len(samples) != 1001 or min(samples) <= 0 or sum(samples) != int(total)
    ):
        raise ValueError("probe identity/sample total mismatch")
    return {
        "mode": mode, "samples_ns": samples, "median_ns": statistics.median(samples),
        "p95_ns": sorted(samples)[math.ceil(len(samples) * 0.95) - 1],
        "checked_resident_capacity_bytes": int(resident),
        "checked_owner_plus_copy_capacity_bytes": int(peak),
    }


def measure(build, fixture, output):
    if output.exists():
        raise FileExistsError("preserve the existing independent-runner result")
    binary = select_binary(build)
    cpu = min(os.sched_getaffinity(0))
    directory = output.with_suffix(".processes")
    directory.mkdir(parents=True)
    for control in ("prepared_globals_fixture_controls", "prepared_globals_probe_functional_controls"):
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
            raise ValueError("the exact fixture/limit/refusal control did not execute")
    bindings = {path.name: sha(path) for path in fixture.iterdir() if path.is_file()}
    report = {
        "scope": "Independent hosted x86-64 replication, not N2/PDFium, shipping qualification or a cache proof. Cold includes complete owner preparation/copies/output/owner destruction per original three-page document. Warm is secondary same-owner repetition, not observed repaint frequency.",
        "memory_scope": "Additional65536-byte document-owner/copy allowance, not a same-total-memory-budget claim. Capacities are from a separate checked sequence, not process RSS.",
        "calibration_rule": "Identical baseline/baseline paired95 interval must lie within0.95–1.05 before treating this runner as a resolved timing diagnostic.",
        "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "binary_sha256": sha(binary), "protocol_sha256": sha(Path(__file__)),
        "fixture_sha256": bindings, "cpu_affinity": cpu, "rounds": 9,
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
        print(f"independent paired round {round_index + 1}/9", flush=True)
        output.write_text(json.dumps(report, indent=2) + "\n")
    summary = []
    for name in ("cold", "warm", "calibration"):
        pairs = [row for row in report["measurements"] if row["comparison"] == name]
        ratios = [row["ratio"] for row in pairs]
        boot = sorted(statistics.median(rng.choices(ratios, k=9)) for _ in range(5000))
        summary.append({
            "comparison": name, "paired_median_ratio": statistics.median(ratios),
            "paired_bootstrap95": [boot[125], boot[4874]],
            "faster_pairs": sum(value < 1 for value in ratios),
            "paired_p95_ratio": statistics.median(row["p95_ratio"] for row in pairs),
        })
    if sha(binary) != report["binary_sha256"] or bindings != {
        path.name: sha(path) for path in fixture.iterdir() if path.is_file()
    }:
        raise ValueError("independent-runner binary or fixture changed")
    report.update(status="complete-independent-private-replication", summary=summary)
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
        parser.error("measurement requires --build-json and --output")
    else:
        measure(args.build_json, args.fixtures.resolve(), args.output)
