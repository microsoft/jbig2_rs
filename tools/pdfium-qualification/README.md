# Original N2 / PDFium qualification assets

This directory transfers the previously evaluator-local source, cohort,
expected outcomes and receipts required by PR #241. It is not a decoder
optimization, release, gate waiver or a claim that the candidate has passed.
Fresh candidate results, when available, are stored under `runs/`.

The held original consumer gate has now been executed for PR #241:
[complete N2 results and raw receipts](runs/pr241-121bee27-n2/README.md).
The functional and per-case median performance gates pass for the exact
tested base/head. This is separate from project proof/CI and not merge approval.

The original narrative and TSVs are under `frozen/`; JSON outcomes are portable
projections, so their file hashes deliberately differ from the original
machine-local JSON hashes recorded in the historical verification receipt.

## What is actually supplied

- A content-addressed public download of the original real-PDFium C++ consumer,
  its complete integration patch
  against public PDFium, and its exact native-O3/final-ThinLTO-O3 patch.
- Portable corpus extraction/normalization, comparison and PNG-adjudication
  helpers. Machine-local paths are supplied through environment variables.
- The complete frozen 321-image/140-PDF cohort projection, with IDs, source and
  payload hashes, globals, canvas dimensions and wrapper identities.
- All original backend outcomes/output hashes, independent-reference decisions,
  timing measurements, selection, compiler/binary audit and verification
  receipts. These are historical observations, not current PR measurements.

The original manifest byte SHA-256 is in `frozen/provenance.json`. The portable
`cohort.json` intentionally removes absolute paths; do not claim its file hash
is the original manifest hash. `materialize.py` requires exact logical equality
with all frozen source/payload/canvas/global fields, not just matching counts.

Fixtures, oracle images, toolchains and compiled third-party binaries are not
redistributed. Public fixtures are materialized from their original projects.
The source/data receipts are now publicly available, not behind a local path.
The native consumer itself is fetched from the immutable public source URL in
`consumer/source.json`, not vendored in this repository's current tree.
`configure.py` checks its exact 5,395-byte length and SHA-256 before writing it
only into the disposable PDFium checkout. This preserves the existing ban on
committed native/oracle source without an audit exception or renamed source.
The historical public source commit remains reachable in this branch's history;
the consumer bytes and original binary/measurement receipts are unchanged.

## Materialize the exact public inputs on any host

Use Python 3 and the pinned dependency in `requirements.txt` in a virtualenv.
No native decoder or N2 host is needed to verify materialization.

```sh
git clone https://pdfium.googlesource.com/pdfium.git pdfium-source
git -C pdfium-source checkout 68f43efa71ac66ba843ba286bb76d075aa1c49df
git clone https://pdfium.googlesource.com/pdfium_tests pdfium-source/testing/corpus
git -C pdfium-source/testing/corpus checkout 2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5
git clone https://github.com/LaurenzV/hayro.git hayro-source
git -C hayro-source checkout e7ada1354dff4041cdeed3d8ba56fe01b6186132
python3 -m venv qualification-venv
qualification-venv/bin/python -m pip install -r tools/pdfium-qualification/requirements.txt
qualification-venv/bin/python tools/pdfium-qualification/materialize.py \
  --pdfium pdfium-source \
  --hayro-tests hayro-source/hayro-jbig2/tests \
  --results qualification-results
```

The expected result is `PASS: exact 321-image/140-PDF cohort`.
Warnings from intentionally damaged PDFs and the four recorded extraction/
adaptation exclusions are not silently discarded. The two damaged original
PDFs still participate in full-page rendering. Standalone Hayro file
admission is not relabelled embedded-page admission.

## Build the matched consumer

This step requires native Linux ARM64 / Neoverse-N2 for the original
performance qualification. Other hosts supply additional evidence only.
Prepare a **disposable** public PDFium checkout at the revision above with
its DEPS, Linux sysroot and Rust/CXX dependencies (`checkout_rust = True`),
following PDFium's normal gclient instructions. Do not apply this overlay to
a user's working checkout. The fixture-only clone above does not by itself
contain all compiler/build dependencies.

Original toolchain identities:

- Clang24, LLVM commit `c3223914b50811fb3be915840f06456acb83d096`.
- Chromium Rust `1.101.0-nightly`, rustc commit
  `41b321963cf979d564d4bb447d9a09347664659f`, matching LLVM24.
- `configure.py` checks these identities. The actual generated compiler and
  final-link commands must also pass `compare.py audit`.

The public PDFium base is sufficient: the formerly local integration commit
`5932789adab7df4d03de4d8fdff2077845b13682` is supplied as
`consumer/integration.patch`, including the subsequent clipping and evaluation
changes. No fetch of that local commit from Google's remote is required.

```sh
python3 tools/pdfium-qualification/configure.py \
  --pdfium /absolute/disposable/pdfium \
  --crate /absolute/jbig2-checkout \
  --clang /absolute/clang-prefix \
  --rust /absolute/chromium-rust-prefix

cd /absolute/disposable/pdfium
third_party/ninja/ninja -C out/Cpp -j 4 jbig2_eval pdfium_unittests
third_party/ninja/ninja -C out/RustStrict -j 4 jbig2_eval pdfium_unittests
third_party/ninja/ninja -C out/RustBest -j 4 jbig2_eval pdfium_unittests
```

`--prepared` is only for a disposable copy of the exact original evaluator.
It verifies the existing consumer against the same digest without downloading
it again; it is not permission to modify
an arbitrary checkout. `--prefix Base` / `--prefix Candidate` permits both
Rust source revisions in one consumer tree, via per-build core-source roots.
The C++ consumer/renderer remains identical.

## Original three-backend protocol

Set absolute roots; these helpers no longer require a particular home,
workspace, session, sibling JPEG2000 checkout or private source location.

```sh
export JBIG2_EVAL_RESULTS=/absolute/qualification-results
export JBIG2_PDFIUM_SOURCE=/absolute/fixture-source/pdfium
export JBIG2_PDFIUM_WORK=/absolute/disposable/pdfium
export JBIG2_HAYRO_TESTS=/absolute/hayro-source/hayro-jbig2/tests
export JBIG2_RS_SOURCE=/absolute/jbig2-checkout
python3 tools/pdfium-qualification/compare.py audit
python3 tools/pdfium-qualification/compare.py functional
python3 tools/pdfium-qualification/adjudicate.py
python3 tools/pdfium-qualification/compare.py performance
python3 tools/pdfium-qualification/compare.py summarize
```

All timed C/C++ must be O3/native/ThinLTO/NDEBUG; Rust opt3/native/one codegen
unit/no debug assertions or overflow instrumentation/linker-plugin LTO; final
link explicitly `--lto-O3`. No fast-math, policy/limit change or C++ fallback.
Record strict and explicit best-effort separately. Recovery receipts are
enabled during functionality and disabled during timing.

The frozen timing selection is 68 workloads, three rotated rounds with three
warmups, 31 timed image repetitions or 11 timed PDF repetitions, CPU2.
Input loading/startup/writes/library initialization are excluded from the
latency clock. Decoder/context construction, packing and destruction are not.
Use RLIMIT_DATA=2GiB, not RLIMIT_AS: the allocator reserves large virtual ranges.
Fresh image contexts and fresh PDF documents preserve real cache lifetimes.

For renewed PR qualification compare its exact base/head as well as C++.
`f8774dd4` is the historical Rust source, not a substitute for a later main
control. Preserve every adverse row and the original intentional refusals.
Do not count blank/omitted images as decoder successes or speedups.

## Renew the exact base/candidate qualification

Configure one disposable consumer for the exact PR base with `--prefix Base`,
then configure its candidate with `--prepared --prefix Candidate`. Build all
four `BaseStrict`, `BaseBest`, `CandidateStrict`, `CandidateBest` executables
and their targeted tests, retaining the original C++ control. `qualify.py`
audits the effective commands and requires the recorded C++ binary identity.

```sh
for phase in audit functional references performance summarize; do
  python3 tools/pdfium-qualification/qualify.py "$phase" --rounds 9 \
    --work /absolute/disposable/pdfium \
    --results /absolute/qualification-results \
    --base /absolute/exact-pr-base \
    --candidate /absolute/exact-pr-head \
    --hayro /absolute/hayro-source/hayro-jbig2/tests
done
python3 tools/pdfium-qualification/export_run.py \
  --results /absolute/qualification-results \
  --output /absolute/reviewable-receipts
```

Declare the round count and full frozen selection before measurement; do not
repeat favorable subsets or mix runs. The summary retains every case and
flags each paired median above1.05. Exported receipts remove local paths but
preserve the complete measured data. A failed gate remains a failed gate.
The summary and exporter require every frozen workload/round/backend, the
prescribed rotation and repetition counts, matching outputs and consistent
ratios. Missing or duplicate measurements fail rather than producing a partial
PASS. Invalid summaries remove any stale receipt; complete adverse results
remain exportable as FAIL, with a failing command exit status.

## Interpretation and licensing

`frozen/functionality.json` records final logical outcomes, including three
C++ 20-second malformed-input timeouts; it does not assert permanent hangs.
The 183 mapped PNG decisions establish that some C++ outputs are not oracles.
The three original C++-only real-PDF requests remain limit/policy dispositions,
not optimization targets. Changed main features must be separated from the
two retained optimization mechanisms.

The added harness/helpers are covered by this repository's license.
The integration patch modifies PDFium's BSD-licensed source and retains its
notices; consult the upstream PDFium license. Fixture licenses remain with
their public source projects; no fixture pixels/PDFs are vendored here.
