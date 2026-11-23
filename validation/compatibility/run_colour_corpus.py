#!/usr/bin/env python3
"""Replay the pinned colour report through both public RGB API shapes."""

import argparse
import hashlib
import json
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path

from run_hayro_corpus import MANIFEST, ROOT, fixture_bytes, validated_manifest

EXPECTED = "fe3ed9ccfa9309b20b5cca0fa612539a6cf3cf14f1782129649ab6f39e1071e4"
RECOVERIES = (
    "[Recovery { page: 0, byte_offset: Some(8), kind: MissingFileColourFlag }, "
    "Recovery { page: 1, byte_offset: Some(40), kind: MissingPageColourFlag }]"
)


def validate(process, output, repaired, compatible):
    lines = process.stdout.splitlines()
    if not repaired and not compatible:
        if process.returncode != 2 or lines != ['ERROR|Malformed(InvalidPageState("missing colour declaration"))']:
            raise ValueError("strict RGB declaration rejection changed")
        if output.exists():
            raise ValueError("strict error returned partial RGB output")
        return {"status": "error", "error": lines[0]}
    expected_recovery = "[]" if repaired else RECOVERIES
    if process.returncode != 0 or lines != ["OK|1", "RECOVERIES|" + expected_recovery]:
        raise ValueError("RGB page count or exact recoveries differ")
    paths = list(output.glob("page-*.rgb"))
    if len(paths) != 1:
        raise ValueError("RGB replay did not return exactly one complete page")
    data = paths[0].read_bytes()
    if len(data) != 16 + 53301 or struct.unpack(">4I", data[:16]) != (1, 163, 109, 489):
        raise ValueError("RGB page extent or channel stride differs")
    if hashlib.sha256(data[16:]).hexdigest() != EXPECTED:
        raise ValueError("complete independent RGB reference hash differs")
    return {"status": "decoded", "rgb_sha256": EXPECTED, "recoveries": expected_recovery}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, default=ROOT / "target/compatibility-fixtures")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--toolchain", default="1.89.0")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    rows = validated_manifest(json.loads(MANIFEST.read_text()))
    row = next(row for row in rows if row["name"] == "lenient_colour_region")
    args.cache.mkdir(parents=True, exist_ok=True)
    source = fixture_bytes(row, args.cache, args.offline)
    records, failures = [], []
    with tempfile.TemporaryDirectory(prefix="jbig2-colour-replay-") as directory:
        directory = Path(directory)
        consumer = directory / "consumer"
        (consumer / "src").mkdir(parents=True)
        (consumer / "Cargo.toml").write_text(
            '[package]\nname="colour-replay"\nversion="0.0.0"\nedition="2021"\n'
            'publish=false\n[workspace]\n[dependencies]\n'
            f'jbig2-decoder={{path={json.dumps(str(ROOT))}}}\n',
        )
        shutil.copyfile(Path(__file__).with_name("colour_consumer.rs"), consumer / "src/main.rs")
        cargo = ["cargo", "+" + args.toolchain]
        subprocess.run(cargo + ["generate-lockfile", "--offline"], cwd=consumer, check=True)
        subprocess.run(cargo + ["build", "--release", "--locked", "--offline"], cwd=consumer, check=True)
        binary = consumer / "target/release/colour-replay"
        repaired_data = bytearray(source.read_bytes())
        repaired_data[8] |= 8
        repaired_data[40] |= 0x80
        repaired = directory / "declared-colour.jbig2"
        repaired.write_bytes(repaired_data)
        for path, corrected in [(source.resolve(), False), (repaired, True)]:
            for mode in ["strict", "strict-policy", "compat"]:
                output = directory / f"output-{corrected}-{mode}"
                record = {"declarations_corrected": corrected, "mode": mode}
                try:
                    process = subprocess.run([str(binary), str(path), str(output), mode],
                        capture_output=True, text=True, timeout=3)
                    record.update(validate(process, output, corrected, mode == "compat"))
                except (ValueError, subprocess.TimeoutExpired) as error:
                    record.update(status="failed", error=str(error))
                    failures.append(record)
                records.append(record)
    result = {
        "status": "failed" if failures else "passed", "fixture_sha256": row["sha256"],
        "source_revision": json.loads(MANIFEST.read_text())["source_revision"],
        "expected_rgb_sha256": EXPECTED, "api_calls": len(records),
        "records": records, "failures": failures,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "api_calls": len(records), "failures": failures}))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
