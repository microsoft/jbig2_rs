"""One untimed BASE14ae versus d66 ordinary-API differential over all321 requests."""

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
PRIVATE = REPO / "target/ordinary-cohort-check/private"
RESULTS = REPO / "target/ordinary-cohort-check/results"
BASE = "14aeac451f6491aa3e413fcbc60aafb11fa75b7c"
CANDIDATE = "d66db50bca76c8650b29346592151d81427f8323"
AUDITED = {
    "src/error.rs": "35e87338ed815bdc4b77a519217d5b2a14cf547af689ab2d00d9e4d2222bf737",
    "src/decode.rs": "bf6372596293d462737e3cdd3b9d8b15fbf6af6f8afc0ebd9bd4c6b5ccccab55",
    "src/document.rs": "e9ac0a112e6e6b195c7c7a22af5f3da1e685a5f19380109ea906770dd7aca6a5",
    "src/document/embedded.rs": "6a2f1e1abe8c00d52cd3fb75c460658d36da9a9d90585caafe3ecf0a82359bef",
    "src/bitmap.rs": "7151eb22ac6ebf41ed4cb75d3698e43eab4277dbf6667dc8c7fd699a48988e94",
    "src/bitmap/access.rs": "ce458c26068422e717e32672c57eda03168d8118806d39ffaed278622e74fae3",
    "src/bitmap/storage.rs": "8f6ce10586869f2b0ec85214156dcd6ead236b88f7465f107f54df90698524e4",
    "src/file_header.rs": "9011f761698f3772c953980e2d2d56ed5ece4f792c8e318ca89a0e0f8d326e11",
}


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


class Frames:
    """Hash every frame byte; only scalar metadata and pixel hashes are retained."""

    def __init__(self, stream):
        self.stream = stream
        self.frame = None
        self.whole = hashlib.sha256()

    def read(self, length):
        chunks = []
        remaining = length
        while remaining:
            data = self.stream.read(remaining)
            if not data:
                raise ValueError(f"incomplete native frame: missing {remaining} bytes")
            chunks.append(data)
            self.whole.update(data)
            if self.frame is not None:
                self.frame.update(data)
            remaining -= len(data)
        return b"".join(chunks)

    def expect(self, value):
        if self.read(len(value)) != value:
            raise ValueError("invalid native frame marker")

    def byte(self):
        return self.read(1)[0]

    def flag(self):
        value = self.byte()
        if value not in (0, 1):
            raise ValueError("invalid native boolean/tag")
        return value

    def number(self):
        return struct.unpack(">Q", self.read(8))[0]

    def count(self):
        value = self.number()
        if value > 1_000_000:
            raise ValueError("native metadata count is outside tool integrity bounds")
        return value

    def text(self):
        return self.read(self.count()).decode("utf-8")

    def pixels(self):
        length = self.number()
        digest = hashlib.sha256()
        remaining = length
        while remaining:
            data = self.read(min(remaining, 65536))
            digest.update(data)
            remaining -= len(data)
        return {"bytes": length, "sha256": digest.hexdigest()}

    def outcome(self):
        self.frame = hashlib.sha256()
        self.expect(b"CASE")
        row = {"id": self.text(), "policy": ("strict", "best-effort")[self.flag()]}
        if self.flag():
            if not self.flag():
                row.update(status="error", phase="parse_globals", error_debug=self.text())
                return self.finish(row)
            row["global_segments"] = self.number()
        else:
            row["global_segments"] = None
        if not self.flag():
            row.update(status="error", phase="decode", error_debug=self.text())
            return self.finish(row)
        row.update(status="decoded", organization=("sequential", "random-access")[self.flag()])
        row["declared_page_count"] = self.number() if self.flag() else None
        row["segment_count"] = self.number()
        row["page_info"] = [
            {**dict(zip(("number", "width", "height", "x_resolution", "y_resolution"),
                        (self.number() for _ in range(5)))), "striped": bool(self.flag())}
            for _ in range(self.count())
        ]
        row["pages"] = []
        for _ in range(self.count()):
            page = dict(zip(("number", "width", "height", "stride"),
                            (self.number() for _ in range(4))))
            page["packed"] = self.pixels()
            row["pages"].append(page)
        row["recoveries"] = []
        for _ in range(self.count()):
            value = dict(zip(("segment", "page", "byte_offset"),
                             (self.number() for _ in range(3))))
            kind = self.byte()
            if kind not in (0, 1, 2):
                raise ValueError("unknown recovery kind; never silently omit it")
            value["kind"] = ("AuxiliaryBuffersFlag", "RefinementsFlag", "ArithmeticTerminatorCode")[kind]
            row["recoveries"].append(value)
        return self.finish(row)

    def finish(self, row):
        self.expect(b"DONE")
        row["canonical_frame_sha256"] = self.frame.hexdigest()
        self.frame = None
        return row


def inputs(path):
    frozen_path = REPO / "tools/pdfium-qualification/frozen/cohort.json"
    frozen = json.loads(frozen_path.read_text())["images"]
    lookup = {row["id"]: row for row in frozen}
    lines = Path(path).read_text().splitlines()
    if len(lines) != 321:
        raise ValueError("all321 request envelopes are mandatory")
    rows = []
    for line in lines:
        fields = line.split("\t")
        if len(fields) != 3 or fields[0] not in lookup:
            raise ValueError("unknown or malformed frozen request")
        identity, payload, globals_path = fields
        expected = lookup[identity]
        payload = Path(payload)
        if (payload.stat().st_size, sha(payload)) != (expected["size"], expected["sha256"]):
            raise ValueError("frozen encoded input differs: " + identity)
        if globals_path == "-":
            size, digest = 0, hashlib.sha256(b"").hexdigest()
        else:
            size, digest = Path(globals_path).stat().st_size, sha(globals_path)
        if (size, digest) != (expected["globals"]["size"], expected["globals"]["sha256"]):
            raise ValueError("frozen globals differ: " + identity)
        rows.append({
            "id": identity, "input": str(payload), "globals": globals_path,
            "envelope": "normalized-embedded; unchanged supplied bytes/globals",
            "frozen": expected,
        })
    if len({row["id"] for row in rows}) != 321 or {row["id"] for row in rows} != set(lookup):
        raise ValueError("duplicate or missing normalized request")
    return rows


def command(args, **kwargs):
    return subprocess.run(args, text=True, check=True, **kwargs)


def build(source, label):
    matched = PRIVATE / "matched"
    if not matched.exists():
        command(["git", "clone", "--shared", str(REPO), str(matched)])
    if command(["git", "status", "--porcelain"], cwd=matched, capture_output=True).stdout:
        raise ValueError("owned serializer build path is dirty")
    command(["git", "switch", "--detach", source], cwd=matched)
    for name, digest in AUDITED.items():
        if sha(matched / name) != digest:
            raise ValueError("public-value/derived-Debug schema audit no longer applies: " + name)
    manifest = HERE / "Cargo.toml"
    env = {key: value for key, value in os.environ.items()
           if key not in {"RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                          "CARGO_ENCODED_RUSTDOCFLAGS", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER",
                          "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS",
                          "CARGO_BUILD_TARGET", "RUSTUP_TOOLCHAIN"}
           and not key.startswith("CARGO_PROFILE_")}
    env.update(CARGO_TARGET_DIR=str(PRIVATE / "build-target"), CARGO_INCREMENTAL="0",
               CARGO_TERM_COLOR="never")
    if not (HERE / "Cargo.lock").exists():
        command(["cargo", "+1.98.1", "generate-lockfile", "--offline",
                 "--manifest-path", str(manifest)], env=env)
    common = ["cargo", "+1.98.1"]
    command(common + ["clean", "--release", "--manifest-path", str(manifest),
                      "--package", "jbig2-decoder", "--package", "ordinary-cohort-serialize"], env=env)
    with (PRIVATE / (label + "-build.log")).open("w") as log:
        command(common + ["build", "--release", "--locked", "--offline",
                          "--manifest-path", str(manifest)], env=env,
                stdout=log, stderr=subprocess.STDOUT)
    binary = PRIVATE / label
    shutil.copyfile(PRIVATE / "build-target/release/ordinary-cohort-serialize", binary)
    binary.chmod(0o755)
    runtime = hashlib.sha256()
    for path in sorted((matched / "src").rglob("*.rs")):
        runtime.update(path.relative_to(matched).as_posix().encode() + b"\0"
                       + path.read_bytes() + b"\0")
    record = {
        "revision": source, "binary_sha256": sha(binary), "same_clean_build_path": str(matched),
        "serializer_sha256": sha(HERE / "src/main.rs"), "lockfile_sha256": sha(HERE / "Cargo.lock"),
        "manifest_sha256": sha(manifest), "public_value_schema_audit": AUDITED,
        "runtime_sha256": runtime.hexdigest(), "driver_sha256": sha(Path(__file__)),
        "source_tree": command(["git", "rev-parse", "HEAD^{tree}"], cwd=matched,
                               capture_output=True).stdout.strip(),
        "rustc": command(["rustc", "+1.98.1", "-Vv"], capture_output=True).stdout,
        "build": "cargo +1.98.1 build --release --locked --offline; no timing/instrumentation",
    }
    save(RESULTS / (label + "-build.json"), record)
    return binary, record


def collect(binary, label, rows):
    manifest = PRIVATE / "requests.tsv"
    with (PRIVATE / (label + "-stderr.log")).open("w") as log:
        process = subprocess.Popen([str(binary), str(manifest)], stdout=subprocess.PIPE, stderr=log)
        frames = Frames(process.stdout)
        result = []
        try:
            frames.expect(b"J321API1")
            for row in rows:
                for policy in ("strict", "best-effort"):
                    outcome = frames.outcome()
                    if (outcome["id"], outcome["policy"]) != (row["id"], policy):
                        raise ValueError("native serializer changed request/policy order")
                    result.append(outcome)
            frames.expect(b"END!")
            if frames.number() != 642 or process.stdout.read(1):
                raise ValueError("incomplete or trailing canonical output")
            if process.wait() != 0:
                raise ValueError("native serializer failed")
        except Exception:
            process.kill()
            process.wait()
            save(RESULTS / (label + "-incomplete.json"), {
                "status": "blocked; not a successful partial cohort",
                "completed_frames": len(result), "last_complete": result[-1] if result else None,
                "stderr": str(PRIVATE / (label + "-stderr.log")),
            })
            raise
        finally:
            process.stdout.close()
    save(RESULTS / (label + "-outcomes.json"), {
        "frames": result, "stream_sha256": frames.whole.hexdigest(),
        "raw_pixels_persisted": False, "ordinary_decode_differential_not_independent_reference": True,
    })
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--requests", type=Path, required=True)
    parser.add_argument("--materialization-receipt", type=Path, required=True)
    args = parser.parse_args()
    if RESULTS.exists():
        raise FileExistsError("preserve the previous complete/failed attempt")
    PRIVATE.mkdir(parents=True, exist_ok=True)
    RESULTS.mkdir()
    materialization = json.loads(args.materialization_receipt.read_text())
    if materialization.get("count") != 321 or materialization.get("status") != "all321_frozen_source_and_payload_hashes_match":
        raise ValueError("require the complete supplied normalized materialization receipt")
    rows = inputs(args.requests)
    manifest = PRIVATE / "requests.tsv"
    manifest.write_text("\n".join("\t".join((r["id"], r["input"], r["globals"])) for r in rows) + "\n")
    snapshot_hash = sha(manifest)
    manifest.chmod(0o444)
    save(RESULTS / "requests.json", {
        "requests_tsv_sha256": sha(args.requests),
        "native_request_snapshot_sha256": snapshot_hash,
        "materialization_receipt_sha256": sha(args.materialization_receipt),
        "frozen_cohort_sha256": sha(REPO / "tools/pdfium-qualification/frozen/cohort.json"),
        "requests": rows, "envelopes_changed": False,
    })
    baseline, base_build = build(BASE, "baseline")
    expected = collect(baseline, "baseline", rows)
    expected_file = RESULTS / "baseline-outcomes.json"
    expected_hash = sha(expected_file)
    expected_file.chmod(0o444)
    # Expectations are complete and sealed BEFORE switching/building the candidate.
    candidate, candidate_build = build(CANDIDATE, "candidate")
    if inputs(args.requests) != rows or sha(expected_file) != expected_hash or sha(manifest) != snapshot_hash:
        raise ValueError("frozen requests or sealed baseline expectations changed")
    actual = collect(candidate, "candidate", rows)
    if inputs(args.requests) != rows or sha(expected_file) != expected_hash or sha(manifest) != snapshot_hash:
        raise ValueError("frozen requests or sealed baseline expectations changed")
    if sha(baseline) != base_build["binary_sha256"] or sha(candidate) != candidate_build["binary_sha256"]:
        raise ValueError("serialized executable changed after source binding")
    mismatches = [{"id": a["id"], "policy": a["policy"], "baseline": a, "candidate": b}
                  for a, b in zip(expected, actual) if a != b]
    save(RESULTS / "comparison.json", {
        "status": "exact-all321-both-policies-match" if not mismatches else "mismatch",
        "requests": 321, "policy_outcomes": 642, "mismatches": mismatches,
        "baseline": base_build, "candidate": candidate_build,
        "baseline_outcomes_sha256_sealed_before_candidate": expected_hash,
        "baseline_counts": dict(Counter(r["policy"] + "/" + r["status"] for r in expected)),
        "candidate_counts": dict(Counter(r["policy"] + "/" + r["status"] for r in actual)),
        "scope": "Ordinary-API preservation of accepted14ae only; not new independent reference,183 decisions,2305 outcomes,full-PDF or LLVM24 qualification.",
        "latency_executed": False, "raw_pixels_persisted": False,
    })
    print(json.dumps({"requests": 321, "policy_outcomes": 642, "mismatches": len(mismatches),
                      "baseline_counts": dict(Counter(r["policy"] + "/" + r["status"] for r in expected))}))
    if mismatches:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
