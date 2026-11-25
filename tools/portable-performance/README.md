# Exact129 portable native diagnostic

This is manual measurement infrastructure, not a product change or an adoption
decision. It preserves the local129-input/133-configuration investigation.
No local latency run, GitHub push or dispatch is part of its setup validation.
The original shared-host adverse results are not replaced or erased.

## Frozen inputs and independent output identity

`cohort.json` preserves all original123 cases in order, followed by the six
controlled grid cases. All policies, envelopes, globals, page dimensions/counts
and independently frozen packed SHA256/lengths are retained. The four context
controls are additional configurations, not replacements: `bitmap-mmr`,
`generated-transitions-64`, `generated-transitions-512`, `generated-white-2048`.
The generated grid's canvas is **one white pixel**, not a million-pixel page;
its1024x1024/depth labels describe internal halftone-grid work.

The110 public payload/global tuples map back to the existing pinned
`tools/pdfium-qualification/frozen/cohort.json`. The mapping chooses an exact
tuple and the original frozen case's PDF hash; it does not infer a downloadable
asset from an image/PDF identifier alone. This selected route has108 distinct
source containers. Each is actually fetched and verified individually:

* `corpus/` sources: `pdfium_tests`
  `2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5`, prefix removed.
* `resources/` sources: PDFium
  `68f43efa71ac66ba843ba286bb76d075aa1c49df`, under `testing/resources/`.
* If a PDF exists only as an `.in` template, the exact pinned PDFium
  `testing/tools/fixup_pdf_template.py` is downloaded into disposable storage,
  never copied into this repository. Its SHA256 is
  `0a05d1e213a3c72fbddeed263e8e08f4a51de35515a906e286d4c1bd2fa11d52`.
  The resulting **PDF hash/length must still match** the frozen source.
* Extraction reuses the qualification producer's `extract_pdf`/`image_data`
  helpers with **pypdf7.1.1**. Every exact encoded/global hash/length is checked;
  no new candidate output, source substitution or denominator reduction is an
  escape hatch. A genuinely missing source/normalization route is a hard setup
  failure, with the immutable URL and expected/actual identity reported.
  Acquisition is paced, bounded to four attempts and records HTTP throttling
  and numeric `Retry-After` delays; a rate limit is not a reason to drop a case.

Only the13 explicitly allowlisted original first-party positive fixtures and
six first-party controlled-grid inputs are packaged as deterministic
gzip/base64 encoded inputs in `generated.json`. An arbitrary unmapped input is
never classified as generated. No public raw stream, PDF or packed golden is
shipped. `original/*.txt` holds the original **first-party sources and metadata
manifests**, not third-party fixture bytes. `provenance.json` preserves their
original paths/hashes, including the grid generator.

The separate `jbig2-golden-materializer` is built **only at BASE**
`14aeac451f6491aa3e413fcbc60aafb11fa75b7c`. It writes packed pages outside timing.
Python requires every output's shape, byte count and hash to equal the
independently frozen expectations; it never regenerates those expectations.
The candidate cannot build this materializer through the supported entry point.
Candidate validation only reads goldens, and hashes are rechecked afterward.
Multipage cases must match the frozen per-page bytes on every page.

## Build and correctness protocol

`diagnostic.py setup --candidate FULL_SHA`:

1. Verifies all129 inputs and records exact immutable acquisition identities.
2. Uses one owned disposable clone at `target/portable-performance/private/matched`.
3. Explicitly cleans both runtime/probe packages, builds BASE, materializes
   independently hash-verified goldens, switches that **same path** to the one
   candidate, then explicitly cleans and builds it.
4. Uses Rust **1.98.1**, nativeCPU, O3, ThinLTO, CGU1, debug1, incremental off,
   with the original probe lockfile. No latency binary enables allocations.
5. Runs **492 API configurations per revision**, including exact packed output,
   inspection, globals and policy/recovery metadata. Each validation route is
   exercised twice, covering context reuse. Baseline/candidate records must
   match. This validator has no timers and cannot write goldens.

The492-route denominator is19 standalone cases times four APIs,94 strict
embedded cases times four APIs,16 best-effort cases times two APIs, plus eight
explicit globals API checks. No inspection route is invented for a
best-effort-only input, and no existing route is removed.

All fetching/compilation/validation finishes before timing can start. The
timing binary is the original first-party Rust harness; its **only source
change is the generated-case module's machine path**, shown in
`harness-path-only.diff`. The original two pixel/page-count rejection tests
remain in that source. No sample, warmup, pilot, batch, decoder or timer code
was rewritten.

The Python build driver necessarily has a larger recorded diff for portable
paths, extra correctness binaries and receipts. The confirmation driver's
explicit deltas are CPU selection plus append-only completed-side journals
outside the child measurements; `measure_probes.py` only changes ROOT.
`*.diff` and the originals make these changes reviewable. The **Python driver
is not claimed byte-identical** to its local original, nor is inter-process
spacing claimed identical. Sampling inside the native harness is unchanged.

## Manual invocation and outputs

The existing `performance.yml` performance job, permissions, steps,20-minute
cap,1050 guard and proof/required-CI roles remain unchanged. The added job is
manual-only, `needs: performance`, on fresh `ubuntu-24.04` hosted x86, with its
own unchanged **20-minute cap**. It accepts exactly one immutable full
`candidate_sha` (default `d66db50bca76c8650b29346592151d81427f8323`).
Other published, full candidate SHAs use the same path; no d66 product edit is
required.

The parent may dispatch after publishing this infrastructure:

```sh
gh workflow run performance.yml --ref INFRASTRUCTURE_REF \
  -f portable_diagnostic=true -f candidate_sha=FULL_CANDIDATE_SHA
```

For **setup/correctness only**, in a fresh disposable checkout:

```sh
python3 -m venv target/portable-performance/venv
target/portable-performance/venv/bin/python -m pip install \
  -r tools/pdfium-qualification/requirements.txt
target/portable-performance/venv/bin/python -B tools/portable-performance/diagnostic.py \
  setup --candidate FULL_CANDIDATE_SHA
```

Latency is a separate explicit `measure --authorize-latency` action. It first
runs the predeclared identical-BASE-binary calibration on six named cases
(eight configurations), then **all133 configurations for nine rounds:1197
pairs**, with31 samples/350ms adaptive target settings. The original frozen
`bitmap-symbol-manysyms` exception remains a single cold decode per side/round;
it is not silently described as31 samples. Order is the original seeded,
per-round shuffled case order and alternating before/after order. Every child
process uses one deterministic available CPU, recorded alongside the actual
hardware, affinity, compiler, sources, binaries and configuration.

All raw samples, medians, p95s, paired bootstrap95% intervals, faster/slower
counts, process-CPU diagnostics, target/control/adverse rows and calibration
results are retained. No >5% row is automatically discarded. Process CPU is
secondary scheduling evidence, never decoder wall time or hardware cycles.
Setup creates no latency results. A failed/incomplete run cannot create a
`completion.json` success receipt.

The first diagnostic step declares a19-minute execution deadline within the
unchanged20-minute job, reserving finalization/upload time. Toolchain/setup
consume that deadline too. A timeout preserves completed-side JSONL journals,
even mid-round, and fails without cutting samples, rounds, cases or controls.
The350ms target is the original total adaptive target across the31 samples,
not350ms per sample. Slow first decodes/validation can still make a full run
exceed the hosted budget: that is an explicit incomplete diagnostic, not a
weaker successful run. No successful hosted timing or budget-fit claim is made
by local setup validation.

Only `target/portable-performance/results/` is uploaded as
`portable-exact129-RUN_ID-RUN_ATTEMPT`: receipts, configuration, logs, correctness
metadata and timing JSON/JSONL. All corpus/raw pixel files, binaries and
toolchains remain in ignored disposable directories and are **never uploaded
or committed**. A setup rerun refuses to overwrite a completed setup; preserve
it and use a fresh disposable checkout.

This is **not Native_N2/LLVM24/PDFium qualification**. Actual host/compiler
identities and all earlier shared-host evidence remain necessary context.
`validation/` contains the local setup/correctness receipts only. They are not
measurement inputs or hosted timing evidence; every dispatch performs setup
and validation again for its own candidate and actual runner.
