#!/usr/bin/env python3
"""Replay pinned external compatibility cases through the ordinary public API."""

import argparse
import hashlib
import json
import re
import shutil
import struct
import subprocess
import tempfile
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path(__file__).with_name("hayro-0.4.json")
MODES = (
    "strict", "strict-context", "strict-reader",
    "compat", "compat-context",
)


def validated_manifest(document):
    rows = document["fixtures"]
    if (
        document["schema_version"] != 1
        or document["fixture_count"] != len(rows)
        or len(rows) != 32
        or len({row["name"] for row in rows}) != len(rows)
        or {issue for row in rows for issue in row["issues"]} != set(range(191, 219))
    ):
        raise ValueError("compatibility manifest must account for all28 reports/32 files")
    revision = document["source_revision"]
    if len(revision) != 40 or any(char not in "0123456789abcdef" for char in revision):
        raise ValueError("fixture revision is not immutable")
    for row in rows:
        name = row["name"]
        if not name or any(char not in "abcdefghijklmnopqrstuvwxyz0123456789_" for char in name):
            raise ValueError("unsafe fixture name")
        url = urlsplit(row["url"])
        expected = f"/LaurenzV/hayro/{revision}/hayro-jbig2/tests/streams/{name}.jbig2"
        if (
            url.scheme != "https" or url.netloc != "raw.githubusercontent.com"
            or url.path != expected or url.query or url.fragment
            or not 0 < row["bytes"] <= 16 * 1024 * 1024
            or len(row["sha256"]) != 64
            or any(char not in "0123456789abcdef" for char in row["sha256"])
        ):
            raise ValueError("fixture source or size/hash is not pinned")
        if row["issues"] in ([215], [216]):
            default = row.get("default_work")
            if (
                not isinstance(default, dict)
                or set(default) != {"timeout_seconds", "strict_error_debug", "compatibility_error_debug"}
                or default["timeout_seconds"] != 3
                or not all(
                    isinstance(default[key], str) and default[key].startswith(
                        "LimitExceeded(LimitExceeded { kind: Work,"
                    )
                    for key in ("strict_error_debug", "compatibility_error_debug")
                )
            ):
                raise ValueError("nonprogress fixtures require exact default-work errors and a3-second deadline")
    return rows


def fixture_bytes(row, cache, offline):
    path = cache / f"{row['name']}.jbig2"
    if path.is_symlink():
        raise ValueError("fixture cache must not contain symlinks")
    if path.exists():
        data = path.read_bytes()
    else:
        if offline:
            raise ValueError(f"offline fixture unavailable: {path.name}")
        request = urllib.request.Request(
            row["url"], headers={"User-Agent": "jbig2-decoder-compatibility-validation"},
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read(row["bytes"] + 1)
    if len(data) != row["bytes"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
        raise ValueError(f"fixture size/hash mismatch: {path.name}")
    path.write_bytes(data)
    return path


def decoded_pages(directory):
    pages = []
    for path in directory.glob("page-*.raw"):
        data = path.read_bytes()
        if len(data) < 16:
            raise ValueError("consumer output has a truncated header")
        number, width, height, stride = struct.unpack(">4I", data[:16])
        if stride != (width + 7) // 8 or len(data) != 16 + stride * height:
            raise ValueError("consumer output has an invalid bitmap extent")
        pages.append({
            "number": number, "width": width, "height": height,
            "packed_sha256": hashlib.sha256(data[16:]).hexdigest(),
        })
    return sorted(pages, key=lambda page: page["number"])


def validate_output(row, mode, process, directory):
    strict = mode.startswith("strict")
    expected = row["strict_status" if strict else "compatibility_status"]
    lines = process.stdout.splitlines()
    if expected == "error":
        if process.returncode != 2 or len(lines) != 1 or not lines[0].startswith("ERROR|"):
            raise ValueError("expected a complete typed rejection, not success/crash/partial output")
        error = lines[0].removeprefix("ERROR|")
        expected_error = row["strict_error_debug" if strict else "compatibility_error_debug"]
        if error != expected_error:
            raise ValueError(f"typed diagnostic changed: {error}")
        return {"status": "error", "error": error, "pages": []}
    if process.returncode != 0 or len(lines) != 2:
        raise ValueError(f"expected a complete decoded document: {process.stdout} {process.stderr}")
    if lines[0] != f"OK|{len(row['expected_pages'])}" or not lines[1].startswith("RECOVERIES|"):
        raise ValueError("consumer did not report complete page count and recovery diagnostics")
    pages = decoded_pages(directory)
    if not pages or pages != row["expected_pages"]:
        raise ValueError("exact all-page dimensions/packed-pixel hashes differ")
    recoveries = lines[1].removeprefix("RECOVERIES|")
    if strict or row["strict_status"] == "decoded":
        if recoveries != "[]":
            raise ValueError("conforming input unexpectedly reports a recovery")
    elif row["recovery_contains"] not in recoveries:
        raise ValueError("required observable compatibility recovery is missing")
    return {"status": "decoded", "pages": pages, "recoveries": recoveries}


def comparison_signature(row, mode, result):
    error = result.get("error", "")
    if row["max_work_units"] is not None and error.startswith("LimitExceeded"):
        match = re.fullmatch(
            r"LimitExceeded\(LimitExceeded \{ kind: Work, limit: (\d+), actual: (\d+) \}\)",
            error,
        )
        if match is None:
            raise ValueError("work-budget case lost its precise typed limit fields")
        limit, actual = map(int, match.groups())
        if actual != limit + 1 or limit > row["max_work_units"]:
            raise ValueError("work-budget failure has incorrect inclusive threshold")
        return {
            "status": "error", "kind": "Work",
            "remaining_decode_budget": limit,
        }
    return result


def work_cases(row):
    cases = [(row, "configured", 10)]
    if "default_work" in row:
        default = row["default_work"]
        cases.append(({
            **row,
            "max_work_units": None,
            "strict_error_debug": default["strict_error_debug"],
            "compatibility_error_debug": default["compatibility_error_debug"],
        }, "default", default["timeout_seconds"]))
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, default=ROOT / "target/compatibility-fixtures")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--toolchain", default="1.89.0")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    document = json.loads(MANIFEST.read_text())
    rows = validated_manifest(document)
    args.cache.mkdir(parents=True, exist_ok=True)
    records, failures = [], []
    with tempfile.TemporaryDirectory(prefix="jbig2-compatibility-") as temporary:
        temporary = Path(temporary)
        consumer = temporary / "consumer"
        (consumer / "src").mkdir(parents=True)
        (consumer / "Cargo.toml").write_text(
            '[package]\nname="jbig2-compatibility-consumer"\nversion="0.0.0"\n'
            'edition="2021"\npublish=false\n[workspace]\n[dependencies]\n'
            f'jbig2-decoder={{path={json.dumps(str(ROOT))}}}\n',
        )
        shutil.copyfile(Path(__file__).with_name("fixture_consumer.rs"), consumer / "src/main.rs")
        cargo = ["cargo", "+" + args.toolchain]
        subprocess.run(cargo + ["generate-lockfile", "--offline"], cwd=consumer, check=True)
        subprocess.run(cargo + ["build", "--release", "--locked", "--offline"], cwd=consumer, check=True)
        binary = consumer / "target/release/jbig2-compatibility-consumer"
        for row in rows:
            input_path = fixture_bytes(row, args.cache, args.offline).resolve()
            for case, budget, timeout in work_cases(row):
                policy_results = {}
                for mode in MODES:
                    directory = temporary / row["name"] / budget / mode
                    command = [
                        str(binary), str(input_path), str(directory), mode,
                        str(case["max_work_units"]) if case["max_work_units"] is not None else "default",
                    ]
                    record = {
                        "fixture": row["name"], "issues": row["issues"],
                        "mode": mode, "budget": budget,
                    }
                    try:
                        process = subprocess.run(
                            command, text=True, capture_output=True, timeout=timeout,
                        )
                        result = validate_output(case, mode, process, directory)
                        policy = mode.split("-", 1)[0]
                        signature = comparison_signature(case, mode, result)
                        if policy in policy_results and policy_results[policy] != signature:
                            raise ValueError("byte/reader/fresh/context results or recovery reports differ")
                        policy_results[policy] = signature
                        record.update(result)
                        record["exit"] = process.returncode
                    except (ValueError, subprocess.TimeoutExpired) as error:
                        record["status"] = "failed"
                        record["diagnostic"] = str(error)
                        failures.append(record)
                    records.append(record)
    report = {
        "schema_version": 1, "status": "failed" if failures else "passed",
        "manifest_sha256": hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),
        "source_revision": document["source_revision"],
        "fixture_count": len(rows), "api_runs": len(records),
        "decoder_calls": len(records) + sum("context" in record["mode"] for record in records),
        "scope": document["scope"], "records": records, "failures": failures,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": report["status"], "api_runs": len(records), "failures": failures}))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
