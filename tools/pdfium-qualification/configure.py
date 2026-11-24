"""Configure the original consumer in an explicitly disposable PDFium checkout."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import urllib.request

BUNDLE = Path(__file__).resolve().parent
PUBLIC_BASE = "68f43efa71ac66ba843ba286bb76d075aa1c49df"


def consumer_source(existing=None):
    source = json.loads((BUNDLE / "consumer/source.json").read_text())
    if existing is not None:
        if not existing.is_file():
            raise ValueError("--prepared requires the original consumer in the disposable checkout")
        data = existing.read_bytes()
    else:
        with urllib.request.urlopen(source["source_url"], timeout=60) as response:
            data = response.read(source["size_bytes"] + 1)
    if len(data) != source["size_bytes"] or hashlib.sha256(data).hexdigest() != source["sha256"]:
        raise ValueError("consumer source differs from the pinned original; do not build or relabel it")
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pdfium", required=True, type=Path)
    parser.add_argument("--crate", required=True, type=Path)
    parser.add_argument("--clang", required=True, type=Path, help="Clang prefix containing bin/clang")
    parser.add_argument("--rust", required=True, type=Path, help="Chromium Rust prefix containing bin/rustc")
    parser.add_argument("--prefix", default="Rust")
    parser.add_argument("--prepared", action="store_true", help="Reuse a disposable original consumer copy")
    args = parser.parse_args()
    pdfium, crate, clang, rust = (p.resolve() for p in (args.pdfium, args.crate, args.clang, args.rust))
    if platform.system() != "Linux" or platform.machine() not in ("aarch64", "arm64"):
        raise SystemExit("Original N2 consumer qualification requires native Linux ARM64; other hosts are additional evidence.")
    if "Neoverse-N2" not in subprocess.check_output(["lscpu"], text=True):
        raise SystemExit("This is not the original Neoverse-N2 host class; report other CPUs separately.")
    if not args.prefix.isalnum():
        raise SystemExit("Build prefix must be alphanumeric.")
    clang_version = subprocess.check_output([str(clang / "bin/clang"), "--version"], text=True)
    rust_verbose = subprocess.check_output([str(rust / "bin/rustc"), "--version", "--verbose"], text=True)
    if "clang version 24." not in clang_version or "LLVM version: 24." not in rust_verbose:
        raise SystemExit("Use the original matching LLVM24 compiler/Rust toolchains; do not relabel another toolchain.")
    if "c3223914b508" not in clang_version or "41b321963" not in rust_verbose:
        raise SystemExit("LLVM24 alone is insufficient: use the frozen Clang c3223914 / Chromium Rust41b321963 revisions.")
    consumer = consumer_source(pdfium / "jbig2_eval.cpp" if args.prepared else None)
    if not args.prepared:
        head = subprocess.check_output(["git", "-C", str(pdfium), "rev-parse", "HEAD"], text=True).strip()
        dirty = subprocess.check_output(["git", "-C", str(pdfium), "status", "--porcelain", "--untracked-files=no"], text=True)
        if head != PUBLIC_BASE or dirty:
            raise SystemExit("Expected a clean disposable public PDFium base, never a user's working checkout.")
        if not (pdfium / "build/config/compiler/BUILD.gn").exists():
            raise SystemExit("First complete PDFium dependencies with Rust/CXX enabled using the pinned DEPS.")
        subprocess.run(["git", "-C", str(pdfium), "apply", "--check", str(BUNDLE / "consumer/integration.patch")], check=True)
        subprocess.run(["git", "-C", str(pdfium), "apply", str(BUNDLE / "consumer/integration.patch")], check=True)
        subprocess.run(["git", "-C", str(pdfium), "apply", str(BUNDLE / "consumer/compiler.patch")], check=True)
    (pdfium / "jbig2_eval.cpp").write_bytes(consumer)
    shutil.copy2(BUNDLE / "consumer/jbig2_eval_sources.py", pdfium / "jbig2_eval_sources.py")
    shutil.copy2(BUNDLE / "consumer/jbig2_decoder.BUILD.gn", pdfium / "third_party/jbig2_decoder/BUILD.gn")
    rust_version = subprocess.check_output([str(rust / "bin/rustc"), "--version"], text=True).strip()
    common = f'''
target_cpu = "arm64"
use_remoteexec = false
is_debug = false
is_component_build = false
pdf_is_standalone = true
pdf_use_skia = false
pdf_enable_fontations = false
pdf_enable_xfa = false
pdf_enable_v8 = false
pdf_enable_rust_png = false
clang_base_path = "{clang}"
clang_use_chrome_plugins = false
use_mold = false
use_lld = true
use_thin_lto = true
optimize_for_size = false
symbol_level = 0
dcheck_always_on = false
enable_rust = true
enable_rust_cxx = true
rust_sysroot_absolute = "{rust}"
rust_bindgen_root = "{rust}"
rustc_version = "{rust_version}"
added_rust_stdlib_libs = [ "addr2line", "adler2", "gimli", "miniz_oxide", "panic_unwind" ]
'''
    for name, enabled, best in (("Cpp", False, False),
                                 (args.prefix + "Strict", True, False),
                                 (args.prefix + "Best", True, True)):
        text = common + f"\npdf_enable_rust_jbig2 = {str(enabled).lower()}\njbig2_eval_best_effort = {str(best).lower()}\n"
        if enabled:
            text += f'jbig2_eval_core_root = "{crate}"\n'
        output = pdfium / "out" / name
        subprocess.run([str(pdfium / "buildtools/linux64/gn"), "gen", str(output), "--args=" + text],
                       cwd=pdfium, check=True)
    print("Configured", args.prefix, "against", crate, "; run Ninja and audit actual commands before timing.")


if __name__ == "__main__":
    main()
