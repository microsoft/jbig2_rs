"""Materialize the public fixtures and require exact frozen cohort identity."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

BUNDLE = Path(__file__).resolve().parent


def projection(manifest):
    images = []
    for row in manifest["images"]:
        value = {k: v for k, v in row.items() if k not in ("path", "globals", "original")}
        value["globals"] = {k: v for k, v in row["globals"].items() if k != "path"}
        if "original" in row:
            value["original"] = {k: v for k, v in row["original"].items() if k != "path"}
            value["original"]["file"] = Path(row["original"]["path"]).name
        images.append(value)
    return dict(images=images, pdfs=[{k: v for k, v in row.items() if k != "path"} for row in manifest["pdfs"]])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pdfium", required=True, type=Path, help="Public PDFium base with testing/corpus checkout")
    parser.add_argument("--hayro-tests", required=True, type=Path)
    parser.add_argument("--results", required=True, type=Path)
    args = parser.parse_args()
    results = args.results.resolve()
    results.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment.update(JBIG2_EVAL_RESULTS=str(results), JBIG2_PDFIUM_SOURCE=str(args.pdfium.resolve()),
                       JBIG2_HAYRO_TESTS=str(args.hayro_tests.resolve()))
    subprocess.run([sys.executable, str(BUNDLE / "prepare_corpus.py")], env=environment, check=True)
    manifest = json.loads((results / "manifest.json").read_text())
    expected = json.loads((BUNDLE / "frozen/cohort.json").read_text())
    actual = projection(manifest)
    if actual != expected:
        old = {i["id"]: i for group in ("images", "pdfs") for i in expected[group]}
        new = {i["id"]: i for group in ("images", "pdfs") for i in actual[group]}
        differences = [key for key in sorted(old.keys() | new.keys()) if old.get(key) != new.get(key)]
        raise SystemExit("Frozen cohort differs; do not drop/adapt cases: " + ", ".join(differences))
    digest = hashlib.sha256(json.dumps(actual, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    (results / "materialization-verification.json").write_text(json.dumps(
        dict(result="PASS", image_requests=len(actual["images"]), pdfs=len(actual["pdfs"]),
             portable_cohort_sha256=digest), indent=2) + "\n")
    print("PASS: exact 321-image/140-PDF cohort, including globals/canvases/payload and source hashes.")


if __name__ == "__main__":
    main()
