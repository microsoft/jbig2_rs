import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import resource
import statistics
import subprocess

ROOT = Path(__file__).resolve().parent
OLD = ROOT.parent / "jbig2-qualification-transfer-20261124"
VARIANTS = {
    "ordinary": ("pdfium", "DocStrict", "ordinary", 65536),
    "parsed": ("pdfium", "DocStrict", "parsed", 65536),
    "off": ("pdfium", "DocStrict", "prepared", 0),
    "on": ("pdfium", "DocStrict", "prepared", 65536),
    "best": ("pdfium", "DocBest", "ordinary", 65536),
    "best_requested": ("pdfium", "DocBest", "prepared", 65536),
    "accepted_strict": ("reference-pdfium", "AcceptedStrict", "ordinary", 65536),
    "accepted_best": ("reference-pdfium", "AcceptedBest", "ordinary", 65536),
    "cpp": ("reference-pdfium", "Cpp", "ordinary", 65536),
}
COMPARISONS = {
    "metadata": ("ordinary", "parsed"),
    "resources": ("off", "on"),
    "best_policy_control": ("best", "best_requested"),
    "calibration": ("ordinary", "ordinary"),
}


def load(path):
    return json.loads(path.read_text())


def limits():
    resource.setrlimit(resource.RLIMIT_DATA, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def quantile(values, q):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * q) - 1)]


def run(item, variant, scenario, samples, memory=False):
    directory, build, mode, allowance = VARIANTS[variant]
    binary = ROOT / directory / "out" / build / "jbig2_eval"
    output = ROOT / "measured-pixels" / (variant + ".bin")
    environment = dict(os.environ, JBIG2_DOC_MODE=mode, JBIG2_DOC_ALLOWANCE=str(allowance),
                       JBIG2_DOC_BUDGET=str(4 * 1024 * 1024))
    for name in ("JBIG2_OWNER_TRACE", "JBIG2_EVAL_RECOVERIES"):
        environment.pop(name, None)
    command = [str(binary), ("repeat_document" if scenario == "repeat" else "document") +
               ("_memory" if memory else ""), item["path"], "-", str(output), "0", "0", str(samples)]
    process = subprocess.run(command, capture_output=True, text=True, timeout=180,
                             env=environment, preexec_fn=limits)
    assert process.returncode == 0, (item["id"], variant, scenario, process.returncode, process.stderr)
    value = json.loads(process.stdout)
    value["diagnostics"] = process.stderr
    if not memory:
        assert len(value["samples_us"]) == samples
        assert all(v > 0 for v in value["samples_us"])
        value["p50_us"] = statistics.median(value["samples_us"])
        value["p95_us"] = quantile(value["samples_us"], 0.95)
        value["sha256"] = hashlib.sha256(output.read_bytes()).hexdigest()
        assert output.stat().st_size == value["bytes"]
    return value


def manifest():
    rows = list(load(OLD / "results/manifest.json")["pdfs"])
    for path in sorted((ROOT / "extra").glob("*.pdf")):
        rows.append(dict(id="extra/" + path.name, path=str(path),
                         sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    return rows


def protocol():
    docs = manifest()
    traces = {v["id"]: v for v in load(ROOT / "traces-prepared-cold/documents.json")}
    repeat_ids = [v["id"] for v in docs if traces[v["id"]]["globals"] or
                  any(int(c["mask"]) for c in traces[v["id"]]["invocations"]) or
                  any(image.get("attached_as_mask") for image in traces[v["id"]]["inventory"]["jbig2_objects"].values())]
    for ident in ("corpus/third_party/jbig2/bitmap-symbol-manysyms.pdf",
                  "extra/ocrmypdf-multipage.pdf"):
        if ident not in repeat_ids:
            assert ident in traces
            repeat_ids.append(ident)
    value = dict(rounds=9, cold_samples=31, repeat_samples=11, memory_rounds=3,
                 cold_documents=[v["id"] for v in docs], repeat_documents=repeat_ids,
                 comparisons=COMPARISONS, reference_variants=["accepted_strict", "accepted_best", "cpp"],
                 cpu=2, rlimit_data=2 * 1024**3, warmups=3,
                 candidate="6d1bac13480af4eb08916d685013ef1e00b71fac",
                 accepted="14aeac451f6491aa3e413fcbc60aafb11fa75b7c",
                 memory_note="Separate fresh children; Rust Layout requests and allocation-boundary live peak, not C++ heap or allocator usable sizes. RSS includes a possible launcher floor.",
                 timing_note="Cold primary: complete fresh document per sample, including preparation/copies/packing/document and output destruction. Pixel assertions excluded. Repeat secondary: two paints per still-open page, no forced image-cache eviction.",
                 storage_note="One entry per document; aggregate4MiB requested/capacity ledger includes owner, entry, exact key, observed parsed heap, borrowing box and resource allowance. Default extra resource allowance65536; off0. Underlying System allocator unchanged; construction-only thread-local footprint accounting is part of admission; full allocation metering and event tracing disabled in latency runs.",
                 uncertainty_note="Nine independently ordered fresh-process pairs per document/comparison. Paired bootstrap of process-median and process-p95 ratios is descriptive;11-sample repeat p95 is its maximum. Calibration retains the predeclared0.95-1.05 interval criterion; failures do not trigger replacement trials.",
                 regressions="Retain and investigate every candidate/control median or p95 ratio above1.05. No input omissions counted as speedups, no dropped observations, alignment tuning or rerun-until-pass.")
    path = ROOT / "protocol.json"
    assert not path.exists(), "Do not replace a declared protocol."
    path.write_text(json.dumps(value, indent=2) + "\n")
    print("Declared", len(docs), "cold documents and", len(repeat_ids), "repeat documents.")


def measure(memory):
    settings = load(ROOT / "protocol.json")
    audit = load(ROOT / "build-audit.json")
    for key, binary in audit["binaries"].items():
        path = ROOT / binary["path"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == binary["sha256"], key
    assert load(ROOT / "functional-gate.json")["result"] == "PASS"
    docs = {v["id"]: v for v in manifest()}
    expected = {}
    for scenario in ("cold", "repeat"):
        for policy, name in (("strict", "prepared"), ("best", "best"), ("cpp", "cpp")):
            path = ROOT / ("traces-" + name + "-" + scenario) / "documents.json"
            if scenario == "cold" and policy == "cpp":
                path = ROOT / "traces-cpp/documents.json"
            expected[(scenario, policy)] = {row["id"]: row["result"] for row in load(path)}
    for item in docs.values():
        assert hashlib.sha256(Path(item["path"]).read_bytes()).hexdigest() == item["sha256"]
    (ROOT / "measured-pixels").mkdir(exist_ok=True)
    journal = ROOT / ("memory.jsonl" if memory else "timings.jsonl")
    assert not journal.exists(), "Refusing to overwrite or silently repeat a study."
    with journal.open("x") as stream:
        for scenario in ("cold", "repeat"):
            ids = settings[scenario + "_documents"]
            rounds = settings["memory_rounds"] if memory else settings["rounds"]
            for round_number in range(1, rounds + 1):
                for index, ident in enumerate(ids):
                    if memory:
                        jobs = [("memory", variant, variant) for variant in VARIANTS]
                    else:
                        names = list(COMPARISONS)
                        shift = (round_number + index) % len(names)
                        names = names[shift:] + names[:shift]
                        jobs = []
                        for ci, comparison in enumerate(names):
                            baseline, candidate = COMPARISONS[comparison]
                            order = [("base", baseline), ("candidate", candidate)]
                            if (round_number + index + ci) % 2:
                                order.reverse()
                            jobs.extend((comparison, role, variant) for role, variant in order)
                        jobs.extend(("reference", variant, variant) for variant in settings["reference_variants"])
                    for comparison, role, variant in jobs:
                        result = run(docs[ident], variant, scenario, settings[scenario + "_samples"], memory)
                        if not memory:
                            policy = "best" if variant in ("best", "best_requested", "accepted_best") else "strict"
                            if variant == "cpp":
                                policy = "cpp"
                            target = expected[(scenario, policy)][ident]
                            assert all(str(result[key]) == str(target[key])
                                       for key in ("sha256", "width", "height", "bytes")), (ident, variant, scenario)
                        row = dict(id=ident, scenario=scenario, round=round_number,
                                   comparison=comparison, role=role, variant=variant, result=result)
                        stream.write(json.dumps(row) + "\n")
                        stream.flush()
                    print("memory" if memory else "timing", scenario, round_number, ident, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("phase", choices=("declare", "memory", "timing"))
    args = parser.parse_args()
    os.sched_setaffinity(0, {2})
    {"declare": protocol, "memory": lambda: measure(True), "timing": lambda: measure(False)}[args.phase]()
