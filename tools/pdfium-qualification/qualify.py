"""Renew the complete frozen consumer gate for an exact PR base/candidate."""

import argparse
import collections
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import random
import re
import shlex
import statistics
import subprocess

BUNDLE = Path(__file__).resolve().parent
BUILDS = dict(cpp="Cpp", base_strict="BaseStrict", base_best="BaseBest",
              candidate_strict="CandidateStrict", candidate_best="CandidateBest")


def load(path):
    return json.loads(path.read_text())


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def signature(value):
    return tuple(value.get(k) for k in ("status", "width", "height", "channels", "sha256", "bytes"))


def module(name):
    spec = importlib.util.spec_from_file_location("qualification_" + name, BUNDLE / (name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def audit(args):
    receipt = {}
    cpu = subprocess.check_output(["lscpu"], text=True)
    if platform.machine() not in ("aarch64", "arm64") or "Neoverse-N2" not in cpu:
        raise SystemExit("Original matched qualification requires native Neoverse-N2.")
    config = (args.work / "out/BaseStrict/args.gn").read_text()
    clang = Path(re.search(r'clang_base_path = "([^"]+)"', config).group(1))
    rust = Path(re.search(r'rust_sysroot_absolute = "([^"]+)"', config).group(1))
    clang_version = subprocess.check_output([str(clang / "bin/clang"), "--version"], text=True).splitlines()[0]
    rust_version = subprocess.check_output([str(rust / "bin/rustc"), "--version"], text=True).strip()
    rust_verbose = subprocess.check_output([str(rust / "bin/rustc"), "--version", "--verbose"], text=True)
    assert "c3223914b508" in clang_version and "41b321963" in rust_version and "LLVM version: 24." in rust_verbose
    receipt["environment"] = dict(architecture=platform.machine(), cpu_model="Neoverse-N2",
                                  clang=clang_version, rustc=rust_version, affinity_cpu=2,
                                  shared_host=True, host_policy_changed=False)
    for backend, build in BUILDS.items():
        out = args.work / "out" / build
        raw = subprocess.check_output([str(args.work / "third_party/ninja/ninja"), "-C", str(out),
                                       "-t", "commands", "jbig2_eval"]).decode("latin1")
        count = 0
        for line in raw.splitlines():
            tokens = shlex.split(line)
            if "-c" not in tokens or not any("/clang" in t for t in tokens[:2]):
                continue
            opts = [t for t in tokens if t in ("-O0", "-O1", "-O2", "-O3", "-Os", "-Oz", "-Ofast")]
            assert opts[-1] == "-O3" and "-mcpu=native" in tokens and "-flto=thin" in tokens
            assert "-DNDEBUG" in tokens and "-ffast-math" not in tokens and "-D_DEBUG" not in tokens
            count += 1
        link = next(line for line in raw.splitlines() if "--lto-O" in line and "jbig2_eval" in line)
        assert "-Wl,--lto-O3" in shlex.split(link)
        value = dict(c_cpp_o3_native_thinlto_commands=count, final_lto_opt=3,
                     binary_sha256=hashlib.sha256((out / "jbig2_eval").read_bytes()).hexdigest())
        if backend != "cpp":
            for name in ("obj/third_party/jbig2_decoder/lib.rsp.rust", "obj/core/fxcodec/rust_jbig2_ffi.rsp.rust"):
                flags = (out / name).read_text().splitlines()
                for wanted in ("-Copt-level=3", "-Ctarget-cpu=native", "-Ccodegen-units=1",
                               "-Cdebug-assertions=no", "-Coverflow-checks=no", "-Clinker-plugin-lto=yes"):
                    assert wanted in flags, (backend, wanted)
                assert ("--cfg=pdf_eval_jbig2_best_effort" in flags) == (backend.endswith("best") and "ffi" in name)
            value["core_archive_sha256"] = hashlib.sha256(
                (out / "obj/third_party/jbig2_decoder/libjbig2_decoder.rlib").read_bytes()).hexdigest()
        receipt[backend] = value
    for name in ("base", "candidate"):
        assert receipt[name + "_strict"]["core_archive_sha256"] == receipt[name + "_best"]["core_archive_sha256"]
        source = getattr(args, name)
        receipt[name + "_source"] = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
        assert subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True) == ""
    expected_cpp = load(BUNDLE / "frozen/build-audit.json")["cpp"]["binary_sha256"]
    assert receipt["cpp"]["binary_sha256"] == expected_cpp, "Original C++ control identity changed."
    save(args.results / "renewed-build-audit.json", receipt)
    print(json.dumps(receipt, indent=2), flush=True)


def functional(args, compare):
    manifest = load(args.results / "manifest.json")
    historical = {i["id"]: i for i in load(BUNDLE / "frozen/functionality.json")}
    records, mismatches = [], []
    for item in manifest["images"] + manifest["pdfs"]:
        row = dict(id=item["id"], group=item["group"])
        for backend in BUILDS:
            row[backend] = compare.run(item, backend, label="renewed-functional")
        if signature(row["cpp"]) != signature(historical[item["id"]]["cpp"]):
            mismatches.append(dict(id=item["id"], kind="cpp_control"))
        for policy in ("strict", "best"):
            base, candidate = row["base_" + policy], row["candidate_" + policy]
            diagnostics = lambda value: [line for line in value.get("stderr", "").splitlines()
                                         if line.startswith(("JBIG2_ERROR ", "JBIG2_RECOVERY "))]
            if signature(base) != signature(candidate) or diagnostics(base) != diagnostics(candidate):
                mismatches.append(dict(id=item["id"], kind=policy))
        records.append(row)
        if len(records) % 25 == 0:
            save(args.results / "renewed-functionality.json", records)
            print("Qualified", len(records), "requests", flush=True)
    save(args.results / "renewed-functionality.json", records)
    save(args.results / "renewed-functional-gate.json",
         dict(result="PASS" if not mismatches else "FAIL", requests=len(records),
              outcomes=len(records) * len(BUILDS), mismatches=mismatches))
    print("Functional differences:", len(mismatches), flush=True)
    return 1 if mismatches else 0


def references(args):
    helper = module("adjudicate")
    rows = {r["id"]: r for r in load(args.results / "renewed-functionality.json")}
    result = []
    for old in load(BUNDLE / "frozen/reference-adjudication.json")["records"]:
        path = args.hayro / "references" / old["reference"]
        width, height, expected, digest = helper.png_reference(path)
        assert digest == old["reference_sha256"]
        row = dict(id=old["id"], reference=old["reference"], reference_sha256=digest)
        for backend in BUILDS:
            value = rows[old["id"]][backend]
            if value["status"] == "ok":
                actual = Path(value["path"]).read_bytes()
                assert len(actual) == len(expected)
                row[backend] = dict(status="ok", different_pixels=sum((a ^ b).bit_count() for a, b in zip(actual, expected)))
            else:
                row[backend] = dict(status=value["status"])
        for policy in ("strict", "best"):
            assert row["base_" + policy] == row["candidate_" + policy]
        result.append(row)
    save(args.results / "renewed-references.json", result)
    print("Qualified independent-reference decisions:", len(result), flush=True)


def performance(args, compare):
    assert load(args.results / "renewed-functional-gate.json")["result"] == "PASS"
    manifest = load(args.results / "manifest.json")
    items = {i["id"]: i for i in manifest["images"] + manifest["pdfs"]}
    functional_rows = {i["id"]: i for i in load(args.results / "renewed-functionality.json")}
    selection = load(BUNDLE / "frozen/benchmark-selection.json")["selected"]
    historical = {i["id"]: i for i in load(BUNDLE / "frozen/functionality.json")}
    rows = []
    for round_number in range(args.rounds):
        for index, ident in enumerate(selection):
            expected = functional_rows[ident]
            assert expected["cpp"]["status"] == "ok"
            for backend in ("base_best", "candidate_best"):
                assert expected[backend]["status"] == "ok" and expected[backend]["sha256"] == expected["cpp"]["sha256"]
            names = ["cpp", "base_best", "candidate_best"]
            if historical[ident]["cpp"].get("sha256") == historical[ident]["rust_strict"].get("sha256"):
                for backend in ("base_strict", "candidate_strict"):
                    assert expected[backend]["status"] == "ok" and expected[backend]["sha256"] == expected["cpp"]["sha256"]
                names = ["cpp", "base_strict", "candidate_strict", "base_best", "candidate_best"]
            offset = (round_number + index) % len(names)
            order = names[offset:] + names[:offset]
            row = dict(id=ident, group=items[ident]["group"], round=round_number + 1, order=order)
            for backend in order:
                row[backend] = compare.run(items[ident], backend, 11 if row["group"] == "pdf" else 31,
                                           label=f"renewed-benchmark-{round_number + 1}")
                assert signature(row[backend]) == signature(expected[backend]), (ident, backend)
            for policy in ("strict", "best"):
                if "base_" + policy in row:
                    row[policy + "_candidate_over_base"] = float(row["candidate_" + policy]["p50_us"]) / float(row["base_" + policy]["p50_us"])
                    row[policy + "_candidate_over_cpp"] = float(row["candidate_" + policy]["p50_us"]) / float(row["cpp"]["p50_us"])
            rows.append(row)
            save(args.results / "renewed-performance.json", rows)
            print("round", round_number + 1, ident, "candidate/base", round(row["best_candidate_over_base"], 4), flush=True)


def validate_performance_rows(rows, rounds):
    if type(rounds) is not int or rounds < 9:
        raise ValueError("Renewed qualification requires at least nine predeclared rounds.")
    selection = load(BUNDLE / "frozen/benchmark-selection.json")["selected"]
    historical = {row["id"]: row for row in load(BUNDLE / "frozen/functionality.json")}
    positions = {ident: index for index, ident in enumerate(selection)}
    if len(positions) != len(selection):
        raise ValueError("Frozen timing selection contains duplicate workloads.")
    seen = set()
    for row in rows:
        ident, round_number = row["id"], row["round"]
        if ident not in positions or type(round_number) is not int or not 1 <= round_number <= rounds:
            raise ValueError("Unexpected workload or round in qualification measurements.")
        key = (ident, round_number)
        if key in seen:
            raise ValueError(f"Duplicate qualification measurement: {key}")
        seen.add(key)
        original = historical[ident]
        if row["group"] != original["group"]:
            raise ValueError(f"Workload group differs from the frozen selection: {ident}")
        names = ["cpp", "base_best", "candidate_best"]
        if original["cpp"].get("sha256") == original["rust_strict"].get("sha256"):
            names = ["cpp", "base_strict", "candidate_strict", "base_best", "candidate_best"]
        if {name for name in BUILDS if name in row} != set(names):
            raise ValueError(f"Incomplete or unexpected backend coverage: {key}")
        offset = (round_number - 1 + positions[ident]) % len(names)
        if row["order"] != names[offset:] + names[:offset]:
            raise ValueError(f"Backend order differs from the predeclared rotation: {key}")
        if signature(row["cpp"]) != signature(original["cpp"]):
            raise ValueError(f"Timed C++ control differs from the frozen output: {key}")
        for name in names:
            value = row[name]
            expected_runs = 11 if row["group"] == "pdf" else 31
            if value["status"] != "ok" or signature(value) != signature(row["cpp"]):
                raise ValueError(f"Timed backend output differs: {key}, {name}")
            median, tail = float(value["p50_us"]), float(value["p90_us"])
            if (int(value["runs"]) != expected_runs or not math.isfinite(median)
                    or not math.isfinite(tail) or not 0 < median <= tail):
                raise ValueError(f"Invalid repetitions or timing samples: {key}, {name}")
        for policy in ("strict", "best"):
            if "base_" + policy not in row:
                continue
            candidate = float(row["candidate_" + policy]["p50_us"])
            for denominator, backend in (("base", "base_" + policy), ("cpp", "cpp")):
                actual = row[policy + "_candidate_over_" + denominator]
                expected = candidate / float(row[backend]["p50_us"])
                if not math.isfinite(actual) or not math.isclose(actual, expected, rel_tol=1e-12):
                    raise ValueError(f"Recorded ratio differs from measured timings: {key}, {policy}")
    expected = {(ident, number) for ident in selection for number in range(1, rounds + 1)}
    if seen != expected:
        raise ValueError(f"Incomplete frozen workload/round coverage: {len(expected - seen)} measurements missing.")


def summarize_rows(rows, rounds):
    validate_performance_rows(rows, rounds)
    groups = collections.defaultdict(list)
    for row in rows:
        groups[row["id"]].append(row)
    randomizer = random.Random(241)
    result = []
    for ident, rows in groups.items():
        value = dict(id=ident, group=rows[0]["group"], policies={})
        for policy in ("strict", "best"):
            field = policy + "_candidate_over_base"
            ratios = [r[field] for r in rows if field in r]
            if not ratios:
                continue
            boot = sorted(statistics.median(randomizer.choices(ratios, k=len(ratios))) for _ in range(10000))
            value["policies"][policy] = dict(candidate_over_base=statistics.median(ratios),
                bootstrap95=[boot[250], boot[9749]], faster_rounds=sum(v < 1 for v in ratios),
                candidate_over_cpp=statistics.median(r[policy + "_candidate_over_cpp"] for r in rows),
                base_ms=statistics.median(float(r["base_" + policy]["p50_us"]) / 1000 for r in rows),
                candidate_ms=statistics.median(float(r["candidate_" + policy]["p50_us"]) / 1000 for r in rows))
        result.append(value)
    adverse = [dict(id=i["id"], policy=p, **v) for i in result for p, v in i["policies"].items()
               if v["candidate_over_base"] > 1.05]
    return dict(performance_gate="PASS" if not adverse else "FAIL", rounds=rounds,
                cases=len(result), processes=sum(len(r["order"]) for rows in groups.values() for r in rows),
                adverse_over_5_percent=adverse, per_case=result,
                note="All adverse cases retained. Paired percentile bootstrap is descriptive with nine rounds; no universal guarantee.")


def summarize(args):
    path = args.results / "renewed-summary.json"
    path.unlink(missing_ok=True)
    result = summarize_rows(load(args.results / "renewed-performance.json"), args.rounds)
    save(path, result)
    print("Gate", result["performance_gate"], "adverse rows",
          len(result["adverse_over_5_percent"]), flush=True)
    return 0 if result["performance_gate"] == "PASS" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("audit", "functional", "references", "performance", "summarize"))
    for name in ("work", "results", "base", "candidate", "hayro"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--rounds", type=int, default=9)
    args = parser.parse_args()
    for name in ("work", "results", "base", "candidate", "hayro"):
        setattr(args, name, getattr(args, name).resolve())
    os.sched_setaffinity(0, {2})
    os.environ.update(JBIG2_EVAL_RESULTS=str(args.results), JBIG2_PDFIUM_WORK=str(args.work),
                      JBIG2_HAYRO_TESTS=str(args.hayro))
    compare = module("compare")
    compare.BUILDS = BUILDS
    raise SystemExit({"audit": lambda: audit(args), "functional": lambda: functional(args, compare),
     "references": lambda: references(args), "performance": lambda: performance(args, compare),
     "summarize": lambda: summarize(args)}[args.phase]() or 0)
