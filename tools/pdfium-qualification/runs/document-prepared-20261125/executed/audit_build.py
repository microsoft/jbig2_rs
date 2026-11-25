import hashlib
import json
from pathlib import Path
import shlex
import subprocess

ROOT = Path(__file__).resolve().parent
OLD = ROOT.parent / "jbig2-qualification-transfer-20261124"
BUILDS = {
    "strict": ("pdfium", "DocStrict", False),
    "best": ("pdfium", "DocBest", True),
    "trace_strict": ("pdfium", "DocTraceStrict", False),
    "trace_best": ("pdfium", "DocTraceBest", True),
    "trace_cpp": ("pdfium", "TraceCpp", None),
    "accepted_strict": ("reference-pdfium", "AcceptedStrict", False),
    "accepted_best": ("reference-pdfium", "AcceptedBest", True),
    "cpp": ("reference-pdfium", "Cpp", None),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    cpu = subprocess.check_output(["lscpu"], text=True)
    assert "Neoverse-N2" in cpu and "aarch64" in cpu
    clang = subprocess.check_output(
        ["/workspace/chromium/native-toolchains/clang/bin/clang", "--version"], text=True)
    rust = subprocess.check_output(
        ["/workspace/chromium/src/third_party/rust-toolchain/bin/rustc", "-vV"], text=True)
    assert "c3223914b508" in clang and "41b321963" in rust and "LLVM version: 24." in rust
    result = dict(cpu="Neoverse-N2", architecture="aarch64", shared_host=True,
                  clang=clang.splitlines()[0], rustc=rust, binaries={})
    for label, (directory, build, best) in BUILDS.items():
        work = ROOT / directory
        out = work / "out" / build
        commands = subprocess.check_output(
            [str(work / "third_party/ninja/ninja"), "-C", str(out), "-t", "commands", "jbig2_eval"]).decode("latin1")
        count = 0
        for line in commands.splitlines():
            tokens = shlex.split(line)
            if "-c" not in tokens or not any("/clang" in v for v in tokens[:2]):
                continue
            opts = [v for v in tokens if v in ("-O0", "-O1", "-O2", "-O3", "-Os", "-Oz", "-Ofast")]
            assert opts[-1] == "-O3" and "-mcpu=native" in tokens and "-flto=thin" in tokens
            assert "-DNDEBUG" in tokens and "-ffast-math" not in tokens
            if "cpdf_dib.cpp" in line or "cpdf_pageimagecache.cpp" in line:
                assert ("-DJBIG2_DOCUMENT_TRACE" in tokens) == label.startswith("trace_")
            count += 1
        link = next(line for line in commands.splitlines() if "--lto-O" in line and "jbig2_eval" in line)
        assert "-Wl,--lto-O3" in shlex.split(link)
        value = dict(path=str((out / "jbig2_eval").relative_to(ROOT)),
                     sha256=digest(out / "jbig2_eval"),
                     c_cpp_o3_native_thinlto_commands=count, final_link_lto_opt=3,
                     event_tracing=label.startswith("trace_"))
        if best is not None:
            for name in ("third_party/jbig2_decoder/lib.rsp.rust", "core/fxcodec/rust_jbig2_ffi.rsp.rust"):
                flags = (out / "obj" / name).read_text().splitlines()
                for flag in ("-Copt-level=3", "-Ctarget-cpu=native", "-Ccodegen-units=1",
                             "-Cdebug-assertions=no", "-Coverflow-checks=no", "-Clinker-plugin-lto=yes"):
                    assert flag in flags, (label, flag)
                assert ("--cfg=pdf_eval_jbig2_best_effort" in flags) == (best and "ffi" in name)
            value["core_archive_sha256"] = digest(out / "obj/third_party/jbig2_decoder/libjbig2_decoder.rlib")
        result["binaries"][label] = value
    assert result["binaries"]["strict"]["core_archive_sha256"] == result["binaries"]["best"]["core_archive_sha256"]
    assert result["binaries"]["accepted_strict"]["core_archive_sha256"] == result["binaries"]["accepted_best"]["core_archive_sha256"]
    original_audit = json.loads((OLD / "results/renewed-build-audit.json").read_text())
    assert digest(OLD / "pdfium/out/Cpp/jbig2_eval") == original_audit["cpp"]["binary_sha256"]
    result["unchanged_original_cpp_binary_sha256"] = original_audit["cpp"]["binary_sha256"]
    objects = {}
    for original in (OLD / "pdfium/out/Cpp/obj/core/fxcodec/fxcodec").glob("jbig2*.o"):
        candidate = ROOT / "reference-pdfium/out/Cpp/obj/core/fxcodec/fxcodec" / original.name
        assert digest(original) == digest(candidate), original.name
        objects[original.name] = digest(original)
    assert len(objects) > 10
    result["unchanged_cpp_jbig2_object_hashes"] = objects
    result["sources"] = {}
    for label, path, expected in (
        ("candidate", Path("/workspace/pdfium-dev/jbig2-prepared-public-20261125"),
         "6d1bac13480af4eb08916d685013ef1e00b71fac"),
        ("accepted", Path("/workspace/pdfium-dev/jbig2-accepted-reference-20261125"),
         "14aeac451f6491aa3e413fcbc60aafb11fa75b7c"),
    ):
        actual = subprocess.check_output(["git", "-C", str(path), "rev-parse", "HEAD"], text=True).strip()
        assert actual == expected
        assert not subprocess.check_output(["git", "-C", str(path), "status", "--porcelain"], text=True)
        result["sources"][label] = actual
    assert (ROOT / "pdfium/jbig2_eval.cpp").read_bytes() == (ROOT / "reference-pdfium/jbig2_eval.cpp").read_bytes()
    files = ("jbig2_eval.cpp", "core/fxcodec/jbig2/rust_jbig2_ffi.rs",
             "core/fxcodec/jbig2/jbig2_document_context.cpp",
             "core/fxcodec/jbig2/jbig2_document_context.h", "build/rust/allocator/lib.rs")
    result["experimental_consumer_source_hashes"] = {name: digest(ROOT / "pdfium" / name) for name in files}
    result["allocation_accounting"] = "Underlying System allocator unchanged; per-thread construction footprint is admission work. Full allocation counters enabled only in separate memory children; trace defines absent from latency builds."
    (ROOT / "build-audit.json").write_text(json.dumps(result, indent=2) + "\n")
    print("PASS: effective native O3/CGU1/ThinLTO/linkO3, strict/best policy flags,",
          len(objects), "unchanged C++ codec objects, exact clean sources and original C++ binary.")


if __name__ == "__main__":
    main()
