"""Same-path, explicitly cleaned builds; latency never runs in this module."""

import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

from fixtures import HERE, PRIVATE, REPO, RESULTS, save, sha

MANIFEST = HERE / "probe/Cargo.toml"
MATCHED = PRIVATE / "matched"
BASE = "14aeac451f6491aa3e413fcbc60aafb11fa75b7c"


def command(args, **kwargs):
    try:
        return subprocess.run(args, check=True, text=True, **kwargs)
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, file=sys.stderr)
        raise


def revision(value):
    if not re.fullmatch(r"[0-9a-f]{40}", value):
        raise ValueError("Require one immutable, full lowercase candidate SHA")
    try:
        actual = command(["git", "rev-parse", "--verify", value + "^{commit}"],
                         cwd=REPO, capture_output=True).stdout.strip()
    except subprocess.CalledProcessError:
        command(["git", "fetch", "--no-tags", "origin", value], cwd=REPO)
        actual = command(["git", "rev-parse", "--verify", value + "^{commit}"],
                         cwd=REPO, capture_output=True).stdout.strip()
    if actual != value:
        raise ValueError("Revision did not resolve exactly")
    return actual


def environment():
    env = {
        key: value for key, value in os.environ.items()
        if key not in {
            "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_ENCODED_RUSTDOCFLAGS", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET",
            "RUSTUP_TOOLCHAIN",
        } and not key.startswith("CARGO_PROFILE_")
    }
    env.update({
        "CARGO_TARGET_DIR": str(PRIVATE / "build-target"),
        "CARGO_INCREMENTAL": "0",
        "RUSTFLAGS": "-C target-cpu=native -C embed-bitcode=yes",
        "CARGO_TERM_COLOR": "never",
    })
    return env


def build(source, label, materializer=False):
    if materializer and (source != BASE or label != "baseline"):
        raise ValueError("Only BASE may build the golden materializer")
    if not MATCHED.exists():
        command(["git", "clone", "--shared", str(REPO), str(MATCHED)])
    if command(["git", "status", "--porcelain"], cwd=MATCHED, capture_output=True).stdout:
        raise ValueError("Disposable matched worktree is dirty")
    command(["git", "switch", "--detach", source], cwd=MATCHED)
    env = environment()
    inputs = {str(path.relative_to(REPO)): sha(path) for path in (
        HERE / "probe/src/main.rs", REPO / "benchmarks/cases.rs", MANIFEST,
        HERE / "probe/Cargo.lock", HERE / "probe/src/api.rs",
        HERE / "probe/src/validate.rs", HERE / "probe/src/materialize.rs",
    )}
    command(["cargo", "+1.98.1", "fetch", "--locked", "--manifest-path", str(MANIFEST)], env=env)
    command([
        "cargo", "+1.98.1", "clean", "--release", "--manifest-path", str(MANIFEST),
        "--package", "jbig2-decoder", "--package", "jbig2-roadmap-perf-probe",
    ], env=env)
    binaries = ["jbig2-roadmap-perf-probe", "jbig2-api-validation"]
    if materializer:
        binaries.append("jbig2-golden-materializer")
    log = RESULTS / (label + "-build.log")
    args = ["cargo", "+1.98.1", "build", "--release", "--locked", "--offline", "-vv",
            "--manifest-path", str(MANIFEST)]
    for binary in binaries:
        args += ["--bin", binary]
    with log.open("w") as stream:
        command(args, env=env, stdout=stream, stderr=subprocess.STDOUT)
    for path, digest in inputs.items():
        if sha(REPO / path) != digest:
            raise ValueError("Harness input changed during build")
    runtime = hashlib.sha256()
    for path in sorted((MATCHED / "src").rglob("*.rs")):
        runtime.update(path.relative_to(MATCHED).as_posix().encode() + b"\0"
                       + path.read_bytes() + b"\0")
    paths = {}
    (PRIVATE / "binaries").mkdir(exist_ok=True)
    for binary in binaries:
        destination = PRIVATE / "binaries" / (
            label if binary == "jbig2-roadmap-perf-probe" else label + "-" + binary
        )
        shutil.copyfile(PRIVATE / "build-target/release" / binary, destination)
        destination.chmod(0o755)
        paths[binary] = {"path": str(destination), "sha256": sha(destination)}
    record = {
        "source_revision": source, "source_tree": command(
            ["git", "rev-parse", "HEAD^{tree}"], cwd=MATCHED, capture_output=True).stdout.strip(),
        "runtime_sha256": runtime.hexdigest(),
        "binary_sha256": paths["jbig2-roadmap-perf-probe"]["sha256"],
        "cargo_lock_sha256": sha(HERE / "probe/Cargo.lock"), "harness_sha256": inputs,
        "same_build_directory": str(MATCHED), "binaries": paths,
        "rustc": command(["rustc", "+1.98.1", "-vV"], capture_output=True).stdout,
        "rustflags": env["RUSTFLAGS"], "profile": MANIFEST.read_text(),
        "instrumented_allocations": False, "build_log": str(log),
        "qualification": "Hosted/local native diagnostic; not N2/LLVM24/PDFium qualification.",
    }
    save(PRIVATE / "binaries" / (label + ".json"), record)
    save(RESULTS / (label + "-build.json"), record)
    return record
