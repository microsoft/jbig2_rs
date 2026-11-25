"""Materialize only the frozen public sources; never export their bytes."""

import base64
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from freeze_local import GENERATED, GRIDS

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
PUBLIC = REPO / "tools/pdfium-qualification"
PRIVATE = REPO / "target/portable-performance/private"
RESULTS = REPO / "target/portable-performance/results"
PDFIUM = "68f43efa71ac66ba843ba286bb76d075aa1c49df"
CORPUS = "2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5"
FIXUP_SHA256 = "0a05d1e213a3c72fbddeed263e8e08f4a51de35515a906e286d4c1bd2fa11d52"
FETCH_LOCK = threading.Lock()
LAST_FETCH = 0.0


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def checked(path, expected):
    actual = {"bytes": path.stat().st_size, "sha256": sha(path)}
    if actual != expected:
        raise ValueError(f"Frozen bytes/hash mismatch: {path}; expected {expected}; actual {actual}")


def fetch(repository, revision, name, destination):
    global LAST_FETCH
    parts = PurePosixPath(name)
    if parts.is_absolute() or ".." in parts.parts:
        raise ValueError("unsafe public source path")
    url = (f"https://pdfium.googlesource.com/{repository}/+/{revision}/"
           + urllib.parse.quote(name, safe="/") + "?format=TEXT")
    if not destination.exists():
        for attempt in range(4):
            try:
                with FETCH_LOCK:
                    time.sleep(max(0, LAST_FETCH + 0.25 - time.monotonic()))
                    LAST_FETCH = time.monotonic()
                with urllib.request.urlopen(url, timeout=60) as response:
                    data = base64.b64decode(response.read(), validate=True)
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
                break
            except urllib.error.HTTPError as error:
                if error.code == 404 or attempt == 3:
                    error.add_note("Immutable source: " + url)
                    raise
                retry_after = error.headers.get("Retry-After", "")
                delay = max(2 ** attempt, int(retry_after) if retry_after.isdigit() else 0)
                if error.code == 429:
                    delay = max(delay, 15 * (attempt + 1))
                print(f"Retry pinned source: {url}; HTTP {error.code}; delay {delay}s", file=sys.stderr)
                time.sleep(delay)
            except (TimeoutError, urllib.error.URLError) as error:
                if attempt == 3:
                    error.add_note("Immutable source: " + url)
                    raise
                time.sleep(2 ** attempt)
    return {"url": url, "sha256": sha(destination), "bytes": destination.stat().st_size}


def prepare_pdf(source, tool):
    name = source["pdf"]
    prefix, relative = name.split("/", 1)
    if prefix == "corpus":
        repository, revision, remote = "pdfium_tests", CORPUS, relative
    elif prefix == "resources":
        repository, revision, remote = "pdfium", PDFIUM, "testing/resources/" + relative
    else:
        raise ValueError("unknown frozen public origin")
    pdf = PRIVATE / "pdfs" / name
    try:
        origin = fetch(repository, revision, remote, pdf)
    except urllib.error.HTTPError as error:
        if error.code != 404:
            raise
        template = PRIVATE / "templates" / str(PurePosixPath(name).with_suffix(".in"))
        origin = fetch(repository, revision, str(PurePosixPath(remote).with_suffix(".in")), template)
        pdf.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [sys.executable, str(tool), "--output-dir", str(pdf.parent), str(template)],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        origin["template"] = True
    checked(pdf, {"bytes": source["pdf_bytes"], "sha256": source["pdf_sha256"]})
    return name, pdf, origin


def materialize():
    import pypdf
    if pypdf.__version__ != "7.1.1":
        raise ValueError("Require exact pypdf==7.1.1")
    cohort = json.loads((HERE / "cohort.json").read_text())
    generated = json.loads((HERE / "generated.json").read_text())["inputs"]
    allowed = set(cohort["original_generated_allowlist"] + cohort["grid_generated_allowlist"])
    if set(generated) != allowed or allowed != GENERATED | GRIDS or len(cohort["cases"]) != 129:
        raise ValueError("generated allowlist or full denominator changed")
    public = json.loads((PUBLIC / "frozen/cohort.json").read_text())
    frozen_images = {row["id"]: row for row in public["images"]}
    frozen_pdfs = {row["id"]: row for row in public["pdfs"]}
    sources = {}
    for row in cohort["cases"]:
        source = row["source"]
        if source["kind"] == "pinned-public-pdf":
            image = frozen_images[source["image_id"]]
            glob_sha = row["globals"]["sha256"] if row["globals"] else hashlib.sha256(b"").hexdigest()
            pdf = frozen_pdfs[source["pdf"]]
            if (image["sha256"], image["globals"]["sha256"], image["pdf"]) != (
                row["input"]["sha256"], glob_sha, source["pdf"],
            ) or (pdf["sha256"], pdf["size"]) != (source["pdf_sha256"], source["pdf_bytes"]):
                raise ValueError("public mapping no longer matches pinned qualification cohort")
            sources[source["pdf"]] = source
        elif row["id"] not in allowed or source["kind"] != "explicit-first-party-generated":
            raise ValueError("unapproved generated input")
    if len(sources) != 108:
        raise ValueError("Do not drop public sources")
    tool = PRIVATE / "tools/fixup_pdf_template.py"
    tool_source = fetch("pdfium", PDFIUM, "testing/tools/fixup_pdf_template.py", tool)
    if tool_source["sha256"] != FIXUP_SHA256:
        raise ValueError(f"Pinned PDFium template tool hash differs: expected {FIXUP_SHA256}, actual {tool_source}")
    os.environ.update(JBIG2_EVAL_RESULTS=str(PRIVATE), JBIG2_PDFIUM_SOURCE=str(PRIVATE))
    spec = importlib.util.spec_from_file_location("qualification_extract", PUBLIC / "prepare_corpus.py")
    extraction = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(extraction)
    receipts, extracted = {}, {}
    with ThreadPoolExecutor(max_workers=4) as pool:
        ready = list(pool.map(lambda value: prepare_pdf(value, tool), sources.values()))
    for name, pdf, origin in sorted(ready):
        images = []
        extraction.extract_pdf(name, pdf, images)
        for image in images:
            key = image["sha256"], image["globals"]["sha256"]
            extracted.setdefault(key, []).append(image)
        receipts[name] = {**origin, "pdf_sha256": sha(pdf)}
    manifest = []
    for row in cohort["cases"]:
        if row["id"] in allowed:
            payload = gzip.decompress(base64.b64decode(generated[row["id"]], validate=True))
            input_path = PRIVATE / "inputs" / (row["id"] + ".jb2")
            input_path.parent.mkdir(parents=True, exist_ok=True)
            input_path.write_bytes(payload)
            globals_path = None
        else:
            key = row["input"]["sha256"], (
                row["globals"]["sha256"] if row["globals"] else hashlib.sha256(b"").hexdigest()
            )
            matches = extracted.get(key, [])
            if not matches:
                raise ValueError("Missing EXACT extracted encoded/globals tuple: " + row["id"])
            image = next((v for v in matches if v["pdf"] == row["source"]["pdf"]), None)
            if image is None:
                raise ValueError("Wrong pinned PDF supplied matching payload")
            input_path = Path(image["path"])
            globals_path = Path(image["globals"]["path"]) if row["globals"] else None
        checked(input_path, row["input"])
        if globals_path:
            checked(globals_path, row["globals"])
        manifest.append({
            **{k: row[k] for k in ("id", "family", "width", "height", "pages", "policy", "standalone")},
            "input": str(input_path), "globals": str(globals_path) if globals_path else None,
            "expected": str(PRIVATE / "goldens" / (row["id"] + ".packed")),
        })
    save(PRIVATE / "expanded-cases.json", manifest)
    save(RESULTS / "materialization.json", {
        "status": "inputs-verified", "inputs": len(manifest), "public_inputs": 110,
        "public_sources": receipts, "first_party_generated_inputs": sorted(allowed),
        "fixup_tool": tool_source, "pypdf": pypdf.__version__,
        "frozen_cohort_sha256": sha(HERE / "cohort.json"),
        "qualification_cohort_sha256": sha(PUBLIC / "frozen/cohort.json"),
        "helpers_sha256": sha(PUBLIC / "prepare_corpus.py"),
    })
    return manifest


if __name__ == "__main__":
    print("Verified", len(materialize()), "inputs; packed goldens still require BASE materialization")
