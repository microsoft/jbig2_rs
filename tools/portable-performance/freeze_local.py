"""One-time export of explicitly allowlisted first-party inputs and frozen metadata."""

import argparse
import base64
import difflib
import gzip
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
GENERATED = {
    "generated-white-64", "generated-white-512", "generated-white-2048",
    "generated-transitions-64", "generated-transitions-512", "generated-many-pages-32",
    "symbol-identity-1", "symbol-identity-16", "symbol-identity-256", "symbol-identity-4096",
    "symbol-aggregate-2", "symbol-aggregate-16", "symbol-aggregate-128",
}
GRIDS = {
    f"controlled-halftone-grid-{width}x{height}-depth{depth}"
    for width, height, depth in (
        (43, 41, 0), (1024, 1024, 1), (1024, 1024, 8),
        (1024, 1024, 9), (1024, 1024, 10), (1025, 1023, 8),
    )
}
EMPTY = hashlib.sha256(b"").hexdigest()


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--roadmap", type=Path, required=True)
    parser.add_argument("--benchmark", type=Path, required=True)
    parser.add_argument("--resumed", type=Path, required=True)
    args = parser.parse_args()
    originals = json.loads((args.roadmap / "expanded-cases.json").read_text())
    old = json.loads((args.roadmap / "expanded-cohort.json").read_text())
    extended = json.loads((args.resumed / "grid-cohort/expanded-cohort.json").read_text())
    assert all(extended["files"][path] == row for path, row in old["files"].items())
    cases = json.loads(Path(extended["manifest"]).read_text())
    assert len(originals) == 123 and len(cases) == 129 and cases[:123] == originals
    assert {row["id"] for row in cases[123:]} == GRIDS
    public = json.loads((REPO / "tools/pdfium-qualification/frozen/cohort.json").read_text())
    images = {}
    for row in public["images"]:
        images.setdefault((row["sha256"], row["globals"]["sha256"]), []).append(row)
    pdfs = {row["id"]: row for row in public["pdfs"]}
    packed, frozen, used = {}, [], set()
    for case in cases:
        identity = case["id"]
        row = {k: case[k] for k in (
            "id", "family", "width", "height", "pages", "policy", "standalone",
        )}
        for field in ("input", "expected", "globals"):
            path = case[field]
            if path is None:
                row[field] = None
                continue
            before = extended["files"][path]
            assert sha(path) == before["sha256"] and Path(path).stat().st_size == before["bytes"]
            row[field] = {"sha256": before["sha256"], "bytes": before["bytes"]}
        assert row["expected"]["bytes"] == ((case["width"] + 7) // 8) * case["height"]
        for field, key in (("input", "input_sha256"), ("expected", "expected_packed_sha256")):
            if key in case:
                assert row[field]["sha256"] == case[key]
        matches = images.get((row["input"]["sha256"],
                              row["globals"]["sha256"] if row["globals"] else EMPTY), [])
        if identity in GENERATED | GRIDS:
            assert not matches and row["globals"] is None and case["standalone"]
            payload = Path(case["input"]).read_bytes()
            packed[identity] = base64.b64encode(gzip.compress(payload, mtime=0)).decode()
            row["source"] = {"kind": "explicit-first-party-generated"}
        else:
            assert matches, f"Unmapped input is NOT automatically generated: {identity}"
            eligible = [
                value for value in matches
                if pdfs[value["pdf"]]["sha256"] == case.get("source_pdf_sha256")
            ]
            assert eligible, f"Original frozen PDF hash has no exact public mapping: {identity}"
            selected = sorted(eligible, key=lambda value: value["id"])[0]
            assert selected["group"] == "embedded"
            source = pdfs[selected["pdf"]]
            row["source"] = {
                "kind": "pinned-public-pdf", "pdf": selected["pdf"],
                "pdf_sha256": source["sha256"], "pdf_bytes": source["size"],
                "image_id": selected["id"],
            }
            used.add(source["id"])
        row["original_paths"] = {field: case[field] for field in ("input", "globals", "expected")}
        frozen.append(row)
    assert set(packed) == GENERATED | GRIDS
    assert sum(row["source"]["kind"] == "pinned-public-pdf" for row in frozen) == 110
    assert len(used) == 108
    write(HERE / "cohort.json", {
        "baseline": "14aeac451f6491aa3e413fcbc60aafb11fa75b7c",
        "default_candidate": "d66db50bca76c8650b29346592151d81427f8323",
        "cases": frozen, "original_generated_allowlist": sorted(GENERATED),
        "grid_generated_allowlist": sorted(GRIDS), "public_sources": 108,
        "context_controls": [
            "bitmap-mmr", "generated-transitions-64", "generated-transitions-512",
            "generated-white-2048",
        ],
        "calibration_cases": [
            "bitmap-halftone-10bpp-mmr", "controlled-halftone-grid-1024x1024-depth9",
            "bitmap-symbol-symhuff-texthuff", "bitmap-mmr", "generated-transitions-512",
            "bitmap-symbol-manysyms",
        ],
    })
    write(HERE / "generated.json", {
        "encoding": "base64(gzip(encoded input)); gzip mtime=0",
        "provenance": "Explicit first-party positive-fixture allowlists only; no public streams or pixels.",
        "inputs": packed,
    })
    sources = {
        "harness.rs": args.benchmark / "src/main.rs",
        "build_probe.py": args.roadmap / "build_probe.py",
        "confirm_probes.py": args.roadmap / "confirm_probes.py",
        "measure_probes.py": args.roadmap / "measure_probes.py",
        "probe-Cargo.toml": args.roadmap / "probe/Cargo.toml",
        "probe-Cargo.lock": args.roadmap / "probe/Cargo.lock",
        "prepare_grid_cases.py": args.resumed / "prepare_grid_cases.py",
    }
    receipts = {}
    for name, path in sources.items():
        target = HERE / "original" / (name + ".txt")
        target.parent.mkdir(exist_ok=True)
        target.write_bytes(path.read_bytes())
        receipts[name] = {"original_path": str(path), "sha256": sha(path)}
    for name, path in (
        ("expanded-cases.json", args.roadmap / "expanded-cases.json"),
        ("expanded-cohort.json", args.roadmap / "expanded-cohort.json"),
        ("grid-expanded-cases.json", args.resumed / "grid-cohort/expanded-cases.json"),
        ("grid-expanded-cohort.json", args.resumed / "grid-cohort/expanded-cohort.json"),
    ):
        (HERE / "original" / (name + ".txt")).write_bytes(path.read_bytes())
        receipts[name] = {"original_path": str(path), "sha256": sha(path)}
    receipts["frozen_public_cohort"] = {
        "path": "tools/pdfium-qualification/frozen/cohort.json",
        "sha256": sha(REPO / "tools/pdfium-qualification/frozen/cohort.json"),
    }
    write(HERE / "provenance.json", receipts)
    harness = sources["harness.rs"].read_text()
    ported = harness.replace('/workspace/jbig2/benchmarks/cases.rs',
                             '../../../../benchmarks/cases.rs')
    assert harness != ported
    (HERE / "probe/src").mkdir(parents=True, exist_ok=True)
    (HERE / "probe/src/main.rs").write_text(ported)
    (HERE / "harness-path-only.diff").write_text("".join(difflib.unified_diff(
        harness.splitlines(True), ported.splitlines(True),
        fromfile="original/harness.rs", tofile="probe/src/main.rs", n=0,
    )))
    (HERE / "probe/Cargo.lock").write_bytes(sources["probe-Cargo.lock"].read_bytes())
    for name in ("measure_probes.py", "confirm_probes.py"):
        before = sources[name].read_text()
        after = before
        if name == "measure_probes.py":
            after = after.replace(
                "ROOT = Path(__file__).resolve().parent",
                'ROOT = Path(os.environ["PERF_DIAGNOSTIC_ROOT"]).resolve()',
            )
        else:
            after = after.replace(
                '"cpu_affinity": 2,',
                '"cpu_affinity": int(os.environ["PERF_DIAGNOSTIC_CPU"]),',
            ).replace(
                '["taskset", "-c", "2", str(ROOT / "binaries" / name),',
                '["taskset", "-c", os.environ["PERF_DIAGNOSTIC_CPU"], str(ROOT / "binaries" / name),',
            )
            after = after.replace(
                "if output.exists():",
                'if output.exists() or output.with_suffix(".sides.jsonl").exists():',
            ).replace(
                '                    pair[side] = row',
                '                    pair[side] = row\n'
                '                    with output.with_suffix(".sides.jsonl").open("a") as journal:\n'
                '                        journal.write(json.dumps({"comparison": comparison, "id": case["id"],\n'
                '                            "mode": mode, "round": round_index, "order": order, "side": side,\n'
                '                            "measurement": row}, separators=(",", ":")) + "\\n")',
            )
        assert before != after
        (HERE / name).write_text(after)
        (HERE / (name + ".diff")).write_text("".join(difflib.unified_diff(
            before.splitlines(True), after.splitlines(True),
            fromfile="original/" + name, tofile=name, n=0,
        )))
    before = sources["build_probe.py"].read_text()
    after = (HERE / "build.py").read_text()
    (HERE / "build_probe.py.diff").write_text("".join(difflib.unified_diff(
        before.splitlines(True), after.splitlines(True),
        fromfile="original/build_probe.py", tofile="build.py", n=0,
    )))
    print(f"Frozen {len(frozen)} inputs, {len(used)} public PDFs, {len(packed)} generated payloads")


if __name__ == "__main__":
    main()
