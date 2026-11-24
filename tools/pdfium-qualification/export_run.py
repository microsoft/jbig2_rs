"""Export reviewable qualification receipts without local paths or pixel blobs."""

import argparse
import csv
import hashlib
import json
from pathlib import Path

from qualify import summarize_rows

BACKENDS = ("cpp", "base_strict", "base_best", "candidate_strict", "candidate_best")


def outcome(value):
    result = {k: v for k, v in value.items() if k in (
        "status", "width", "height", "channels", "sha256", "bytes", "p50_us", "p90_us", "runs")}
    if "stderr" in value:
        diagnostics = [line for line in value["stderr"].splitlines()
                       if line.startswith(("JBIG2_ERROR ", "JBIG2_RECOVERY "))]
    elif "diagnostics" in value:
        diagnostics = value["diagnostics"]
    else:
        raise ValueError("Outcome has neither original stderr nor projected diagnostics.")
    if not isinstance(diagnostics, list) or not all(
        isinstance(line, str) and line.startswith(("JBIG2_ERROR ", "JBIG2_RECOVERY "))
        for line in diagnostics
    ):
        raise ValueError("Invalid projected qualification diagnostics.")
    result["diagnostics"] = diagnostics
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "receipt.json").unlink(missing_ok=True)
    load = lambda name: json.loads((args.results / name).read_text())
    summary = load("renewed-summary.json")
    if summary != summarize_rows(load("renewed-performance.json"), summary["rounds"]):
        raise ValueError("Summary differs from the complete frozen measurements; no receipt exported.")

    def save(name, value):
        (args.output / name).write_text(json.dumps(value, indent=2) + "\n")

    for name in ("materialization-verification.json", "renewed-build-audit.json",
                 "renewed-functional-gate.json", "renewed-references.json", "renewed-summary.json"):
        save(name, load(name))
    for name in ("renewed-functionality.json", "renewed-performance.json"):
        rows = load(name)
        for row in rows:
            for backend in BACKENDS:
                if backend in row:
                    row[backend] = outcome(row[backend])
        save(name, rows)
    fields = ("id", "group", "policy", "base_ms", "candidate_ms", "candidate_over_base",
              "ci95_low", "ci95_high", "faster_rounds", "candidate_over_cpp")
    with (args.output / "per-case.tsv").open("w") as file:
        writer = csv.DictWriter(file, fields, delimiter="\t")
        writer.writeheader()
        for case in summary["per_case"]:
            for policy, value in case["policies"].items():
                row = {k: value[k] for k in ("base_ms", "candidate_ms", "candidate_over_base",
                                             "faster_rounds", "candidate_over_cpp")}
                row.update(id=case["id"], group=case["group"], policy=policy,
                           ci95_low=value["bootstrap95"][0], ci95_high=value["bootstrap95"][1])
                writer.writerow(row)
    functional = load("renewed-functional-gate.json")
    audit = load("renewed-build-audit.json")
    receipt = dict(
        functional_gate=functional["result"], performance_gate=summary["performance_gate"],
        base=audit["base_source"], candidate=audit["candidate_source"],
        rounds=summary["rounds"], cases=summary["cases"], processes=summary["processes"],
        environment=audit["environment"],
        host_policy_changed=False, runtime_modified=False, merge_approval=False,
        method="Original frozen cohort/selection, three warmups,31 image/11 PDF timed repetitions; nine predeclared rotating paired-process rounds. All adverse rows retained.",
        projection="Only machine-local paths and temporary pixel locations removed; logical outcomes, exact hashes, typed diagnostics and timings retained.")
    receipt["files"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in sorted(args.output.iterdir()) if p.is_file() and p.name != "receipt.json"}
    save("receipt.json", receipt)
    print(json.dumps({k: v for k, v in receipt.items() if k != "files"}, indent=2))
    return 0 if receipt["functional_gate"] == receipt["performance_gate"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
