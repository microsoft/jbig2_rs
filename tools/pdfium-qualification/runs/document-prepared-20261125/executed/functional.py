import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess

ROOT = Path(__file__).resolve().parent
OLD = ROOT.parent / "jbig2-qualification-transfer-20261124"
PROFILES = {
    "cpp": (OLD / "pdfium/out/Cpp/jbig2_eval", "ordinary", "65536", "cpp"),
    "ordinary": (ROOT / "pdfium/out/DocStrict/jbig2_eval", "ordinary", "65536", "rust_strict"),
    "parsed": (ROOT / "pdfium/out/DocStrict/jbig2_eval", "parsed", "65536", "rust_strict"),
    "prepared0": (ROOT / "pdfium/out/DocStrict/jbig2_eval", "prepared", "0", "rust_strict"),
    "prepared": (ROOT / "pdfium/out/DocStrict/jbig2_eval", "prepared", "65536", "rust_strict"),
    "best": (ROOT / "pdfium/out/DocBest/jbig2_eval", "ordinary", "65536", "rust_best"),
    "best_prepared_requested": (ROOT / "pdfium/out/DocBest/jbig2_eval", "prepared", "65536", "rust_best"),
}
SIGNATURE = ("status", "width", "height", "channels", "bytes", "sha256")


def limits():
    resource.setrlimit(resource.RLIMIT_DATA, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def diagnostics(value):
    return [line for line in value.get("stderr", "").splitlines()
            if line.startswith(("JBIG2_ERROR ", "JBIG2_RECOVERY "))]


def run(item, profile, output):
    binary, mode, allowance, _ = PROFILES[profile]
    target = output / (hashlib.sha256(item["id"].encode()).hexdigest()[:20] + ".bin")
    environment = dict(os.environ, JBIG2_DOC_MODE=mode, JBIG2_DOC_ALLOWANCE=allowance,
                       JBIG2_EVAL_RECOVERIES="1")
    for name in ("JBIG2_OWNER_TRACE", "JBIG2_DOC_BUDGET"):
        environment.pop(name, None)
    command = [str(binary), "pdf" if item["group"] == "pdf" else "image", item["path"],
               item.get("globals", {}).get("path", "-"), str(target),
               str(item.get("width", 0)), str(item.get("height", 0)), "0"]
    try:
        process = subprocess.run(command, capture_output=True, text=True, timeout=20,
                                 env=environment, preexec_fn=limits)
    except subprocess.TimeoutExpired:
        return dict(status="timeout", stderr="bounded20s")
    if process.returncode:
        raise RuntimeError((item["id"], profile, process.returncode, process.stderr))
    fields = process.stdout.strip().split("\t")
    assert len(fields) == 7, (profile, item["id"], process.stdout)
    result = dict(zip(("status", "width", "height", "channels", "p50_us", "p90_us", "runs"), fields))
    result["stderr"] = process.stderr
    if result["status"] == "ok":
        data = target.read_bytes()
        result.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    return result


def main():
    os.sched_setaffinity(0, {3})
    manifest = json.loads((OLD / "results/manifest.json").read_text())
    historical = {v["id"]: v for v in json.loads(
        (ROOT.parent / "jbig2-comparison-20261123/functionality.json").read_text())}
    output = ROOT / "functional-pixels"
    output.mkdir(exist_ok=True)
    rows, differences = [], []
    for item in manifest["images"] + manifest["pdfs"]:
        assert hashlib.sha256(Path(item["path"]).read_bytes()).hexdigest() == item["sha256"]
        row = dict(id=item["id"], group=item["group"])
        for profile in PROFILES:
            row[profile] = run(item, profile, output)
            expected = historical[item["id"]][PROFILES[profile][3]]
            if tuple(row[profile].get(k) for k in SIGNATURE) != tuple(expected.get(k) for k in SIGNATURE):
                differences.append(dict(id=item["id"], profile=profile, kind="signature",
                                        actual=row[profile], expected=expected))
            if profile != "cpp" and diagnostics(row[profile]) != diagnostics(expected):
                differences.append(dict(id=item["id"], profile=profile, kind="diagnostic",
                                        actual=diagnostics(row[profile]), expected=diagnostics(expected)))
        rows.append(row)
        if len(rows) % 25 == 0:
            (ROOT / "functionality.json").write_text(json.dumps(rows, indent=2) + "\n")
            print("Completed", len(rows), "differences", len(differences), flush=True)
    (ROOT / "functionality.json").write_text(json.dumps(rows, indent=2) + "\n")
    gate = dict(result="PASS" if not differences else "REVIEW_REQUIRED", requests=len(rows),
                profiles=len(PROFILES), outcomes=len(rows) * len(PROFILES), differences=differences)
    (ROOT / "functional-gate.json").write_text(json.dumps(gate, indent=2) + "\n")
    print(json.dumps({k: v for k, v in gate.items() if k != "differences"}, indent=2))
    assert not differences


if __name__ == "__main__":
    main()
