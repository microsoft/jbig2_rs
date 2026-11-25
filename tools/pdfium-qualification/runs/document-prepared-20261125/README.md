# JBIG2 prepared globals: actual document-level adoption assessment

## Decision

**Do not adopt a prepared-resource cache into PDFium on this evidence.**
The public API is useful conditionally, but this actual document workload
does not exercise successful decoded-resource reuse. Keep the candidate
opt-in/held rather than closing the optimization as universally useless.

The requested "#238 PR" is actually
[issue #238](https://github.com/microsoft/jbig2_rs/issues/238).
This study executes the actual public `PreparedGlobals` implementation at
[`6d1bac13480af4eb08916d685013ef1e00b71fac`](https://github.com/microsoft/jbig2_rs/commit/6d1bac13480af4eb08916d685013ef1e00b71fac),
not its private prototype. Accepted Rust reference:
`14aeac451f6491aa3e413fcbc60aafb11fa75b7c`.

All work is experimental consumer/evidence work. No production decoder,
default policy, source proof, release, merge or adoption was changed.

## 1. Real opportunities, including masks and image-cache interception

The study retains all original **321 image requests / 140 PDFs** and adds
three pre-existing public controls: OCRmyPDF's `jbig2.pdf` and
`multipage.pdf`, and PDF.js's `JBIG2Globals.pdf` regression attachment.
Exact public URLs and input hashes are in `additional-public-inputs.json`.
No PDF was generated to manufacture a positive reuse target.

Actual renderer tracing is at PDFium's real `CPDF_DIB` decoder call site,
including mask loading, with document/image identity, exact source/global
SHA256, canvas, page/pass and decoder return status. Image-cache returns
are recorded at the successful `CPDF_PageImageCache` bitmap-return branch,
including an attached mask only when its cached bitmap actually exists.
Inventory alone is not counted as a decoder call.

| Actual schedule, all 143 documents | Decoder starts | Stencil-mask starts | Attached-mask starts | JBIG2-related image-cache return events | Successful prepared-resource reuses |
|---|---:|---:|---:|---:|---:|
| Cold, strict Rust | 144 | 4 | 1 | 0 | **0** |
| Two paints of each still-open page, strict Rust | 170 | 8 | 2 | 118 | **0** |
| Two paints, unchanged best-effort Rust | 154 | 8 | 2 | 134 | **0** |
| Two paints, C++ reference | 153 | 8 | 2 | 135 | not the experimental Rust API |

Cold traffic has **nine nonempty-global calls in nine documents**:
each exact global identity has **one distinct image object, one distinct
encoded image and one invocation**. There is no observed pair of distinct
painted images sharing nonempty globals in a document.

The repeat schedule retains normal PDFium caching; it does not flush images
to force decoder work. Its two repeated-global groups are retries of the
same failed image, not productive cross-image reuse. One failed PDF.js image
reuses parsed metadata on retry; it never prepares usable decoded resources.
That omission is not credited as a speedup. The global-bearing attached-mask
case, `bug_527174.pdf`, retains its malformed/height-limit refusal.

A bounded search of existing public fixtures and three additional public
documents did not supply a real positive shared-global PDF. This is a
statement about the evaluated domain, not proof that such PDFs do not exist.
The previously demonstrated benefit for three distinct normalized pages of
the genuine standalone `file_pages.jbig2` remains separate evidence:
standalone decoding already shares globals, and normalized embedded calls
are not an observed full-PDF gain. They were used here for correctness/quota
controls only, never as a fabricated production-PDF timing target.

### Actual admission and retained storage

Prepared decoding below is **strict-only**. Numeric states and every event
are retained in `traces.json.gz`; per-document data is in `summary.json`.

| Cold global-bearing document | API disposition | Encoded globals | Retained decoded-resource capacities | Total experimental owner retained bytes |
|---|---|---:|---:|---:|
| `bitmap-halftone-global.pdf` | Ready, first use only | 313 | 17,528 | 18,594 |
| `bitmap-p32-eof.pdf` | NoResources bypass | 131 | 0 | 702 |
| `bitmap-symbol-global.pdf` | Ready, first use only | 287 | 17,062 | 18,076 |
| `bitmap-symbol-texthuffrefinecustompos-global.pdf` | CustomTables bypass | 46 | 0 | 532 |
| `bug_631912.pdf` | Ready, first use only | 189 | 12,469 | 13,287 |
| `bug_1087.pdf` | Ready, first use only | 77 | 11,421 | 12,015 |
| `bug_527174.pdf` | Global parse/height-limit refusal | 59 | 0 | 24 |
| OCRmyPDF `jbig2.pdf` | AdditionalStorage bypass | 28,846 | 0 | 58,132 |
| PDF.js `JBIG2Globals.pdf` | Pending; original image decode fails | 314,990 | 0 | 630,420 |

The largest owner footprint is therefore unused parsed metadata on a failed
image, not a successful cache hit. The `24` bytes in the parse-refusal row
are the empty C++ owner; the failed temporary parsed entry is not retained.

## 2. Experimental document-scoped owner, not a process cache

The owner lives in `JBig2_DocumentContext`, reached through the actual
`CPDF_Document::GetOrCreateCodecContext()` path, and is destroyed with the
document. It binds exact global bytes, one immutable decoder/limit profile,
and strict policy. Content equality is checked, not just stream keys.
Changed global content or fuzzing limits bypass the existing entry.

This conservative experiment has **one slot per document** and an
**aggregate 4 MiB requested/capacity ledger**, not a per-entry-only limit.
It accounts for owner/entry storage, exact key capacity, observed parsed
heap, the borrowing box and the resource allowance. The ordinary codec
context and PDFium's existing image cache remain separate baseline storage.
The one-slot choice loses no observed cold sharing group in this corpus;
it is not claimed to be an optimal general multi-global replacement policy.

Default additional resource allowance is **65,536 bytes**; the disabled
control uses **zero**. Existing per-image decoder limits are unchanged.
Optional resource/copy refusals retain the actual public API's uncached
behavior. Reset releases resources without changing decoder/global identity.

CXX explicitly marks its lifetime-exposing constructor unsafe. The C++
entry holds stable immutable parsed/decoder boxes and destroys the borrowing
prepared box first. This is a new experimental interoperability boundary,
not a claim that the core Verus receipt proves the C++ owner.

Best effort always bypasses strict preparation and calls its original
best-effort decoder. Both best-effort cache-request settings are measured
separately. There is no silent substitution of strict decoding.

The cap is retained requested/capacity storage, **not an OS RSS ceiling**,
allocator usable-size bound or same-total-memory-budget claim. The public
API itself documents possible transient over-reservation before an optional
copy is rejected. Fixed bridge metadata uses PDFium/Rust's normal allocation
machinery; this experiment is not a new general OS-allocation-failure proof.

## 3. Isolated complete-document timing

All configurations use the original **native Neoverse-N2 / matching LLVM24**
PDFium setup. Effective C/C++ and Rust commands verify O3/native/ThinLTO,
one Rust codegen unit, disabled debug/overflow checks and final `--lto-O3`.
Strict/best-effort same-revision core archives are identical.
The old C++ qualification binary remains unchanged; all **18 C++ JBIG2
codec object files** linked into the new measurement harness are byte-identical.

Preparation-enabled and disabled resource comparisons use the **same
executable, actual public prepared API, parsed-global lifetime, strict
policy, decoder limits and rendering work**, with only resource allowance
65,536 versus zero. Thus reuse-disabled does not reparse globals per image.
The separate metadata comparison uses shared parsed globals versus the
ordinary per-request parser, preserving context/scratch lifetime and packing.
Accepted strict/best Rust and unchanged C++ are measured as references,
not substituted controls.

Primary: all **143 complete documents**, nine balanced fresh-process pairs
per comparison, three warmups and **31 fresh-document samples** per child.
Secondary: **13 predeclared documents**, two paints of each still-open page,
nine pairs and 11 samples. Cold means a new document/owner each sample;
process/library/input caches are warmed as in the canonical protocol.

Samples include document loading, real preparation, copies, packing,
rendering, document/owner destruction and output-buffer destruction.
Pixel assertions, input loading, library initialization and output-file
writing are outside latency. Tracing is compiled into separate binaries.
Full allocation metering is enabled only in separate memory children;
construction-only footprint accounting is part of actual cache admission.
The underlying System allocator is not replaced with a different allocator.

**15,444 timed children / 453,024 fresh-document samples** completed.
Every timed bitmap matches the corresponding policy/backend capture.
All observations and pair order are retained in `timings.jsonl.gz`.

### Population summaries, not universal guarantees

Ratios are candidate/base elapsed time; lower is better. These are medians
across the declared per-document paired ratios, not a production-document
population estimate.

| Comparison | Cold median ratio | Cold per-document median range | Cold median p95 ratio | Repeat median ratio |
|---|---:|---:|---:|---:|
| Decoded resources enabled / disabled, same prepared API | 1.0002 | 0.9867-1.0204 | 1.0000 | 0.9999 |
| Shared parsed metadata / ordinary parser | 1.0007 | 0.9854-1.0113 | 1.0003 | 0.9993 |
| Best effort, preparation requested / not requested | 0.9996 | 0.9805-1.0376 | 0.9983 | 1.0008 |

There is **no demonstrated material document-level reuse gain**.
Small positive or negative observations are not attributed to saved global
decoding when the actual successful resource-reuse count is zero.

Two representative first-use costs are visible:

- Halftone-global: disabled/enabled process-median medians
  **1.404650 -> 1.413992 ms**; paired median ratio
  **1.00906 [1.00543, 1.01623]**, p95 ratio
  **1.00951 [0.99840, 1.01304]**.
- Symbol-global: **1.050166 -> 1.068302 ms**; paired median ratio
  **1.01022 [1.00179, 1.01727]**, p95 ratio
  **1.00338 [0.98626, 1.02051]**.

Ratios of paired medians need not equal ratios of the displayed aggregate
times. All per-document median/p95 values, paired intervals, individual
ratios and accepted Rust/C++ references are provided in `per-document.tsv`,
`references.tsv` and `summary.json`. A faster incomplete/white raster,
including the failed large PDF.js JBIG2 image, is never a decoder speedup.

### Every >5% observation and uncertainty limitation

**No candidate resource, metadata or best-effort control median or p95
point estimate exceeds a 5% slowdown.** The sole over-threshold observation
is identical-operation calibration for
`wrapped-generic_template0-page1.pdf`: p95 ratio
**1.06722 [0.97545, 1.10883]**.

Both roles execute the same binary and operations, with no global owner
execution and identical separate allocation observations. All nine ratios,
including the 1.38020 round, remain. The interval includes parity; this is
an adverse calibration observation, not an attributed decoder regression.
No replacement trials or alignment/padding changes were made.

**16 calibration intervals** extend outside the declared 0.95-1.05 range:
two median and 14 p95 intervals, including tiny/refusal controls. They are
all enumerated in `summary.json`. Therefore this is not a universally
passing performance-qualification claim. Nine paired process trials give
descriptive uncertainty; the secondary 11-sample p95 is its maximum sample.
The complete adverse-row investigation retains trace, memory-delta and
calibration evidence rather than waiving any negative result.

## 4. Actual allocation and peak-memory observations

**4,212 separate fresh memory children** completed: three observations per
document/scenario/variant. Every child has **zero tracked Rust bytes after
document destruction**. The requested-live peak maximum is 14,350,256 bytes;
observed process peak RSS ranges from 21,656 to 112,040 KiB.

The Rust counters observe actual Layout requests and allocation-boundary
live peaks for complete document work, not C++ allocations or allocator
usable sizes. C++ references have process RSS observations; zero Rust
allocation calls must not be interpreted as zero C++ allocations.
Small RSS figures can be dominated by the launcher/startup floor, so no
codec heap or small memory saving is inferred from them.

Cold enabled-minus-disabled observations for the four Ready owners:

| Document | Extra Rust allocation calls | Extra requested total bytes | Requested-live peak delta | Process peak-RSS median delta |
|---|---:|---:|---:|---:|
| halftone-global | +91 | +7,200 | +17,528 | 0 KiB |
| symbol-global | +10 | +6,734 | 0 | 0 KiB |
| bug_631912 | +8 | +2,141 | +12,469 | 0 KiB |
| bug_1087 | +7 | +1,093 | 0 | 0 KiB |

These are real first-use copying/retention costs with no observed successful
amortization. Earlier work can dominate peak memory, so a zero peak delta
does not imply that copies or retained owners are free.
`memory.tsv` and `memory.jsonl.gz` retain every document and variant.

## 5. Preserved acceptance, pixels, errors and resource controls

- All **321 requests / 140 PDFs x seven profiles = 3,227 outcomes** retain
  exact original status, dimensions, pixels and Rust error/recovery receipts.
  This includes strict ordinary/parsed/prepared-zero/prepared-enabled,
  both best-effort request settings and the unchanged original C++ binary.
- **183 independent reference decisions** were freshly executed against
  C++, strict prepared and best effort. C++ retains164 exact/11 different/
  8 rejected; each Rust policy retains166 exact/0 different/17 rejected.
  The15 original explicit mapping exclusions remain, not invented oracles.
- **60 actual-consumer native controls** pass across enabled, zero allowance,
  zero/small aggregate budget and best-effort profiles. They exercise
  exact-content versus stream-key binding, changed limits, invalid canvas,
  reset, retained contexts, document isolation and destruction.
- Eight focused public-API tests plus the pinned external-fixture control
  pass. The latter covers all **15 quota frontiers**, their neighbours,
  malformed globals and actual distinct-page warm-copy equivalence.
  The original custom-table, retained-context, single-use/no-global and
  allocation/copy-refusal controls are retained.
- Same per-image policies/limits and the original **2 GiB RLIMIT_DATA**, not
  an address-space cap. No image retry after page mutation, blank-image
  success credit, fallback to C++, or quota relaxation.

The public component's conditional normalized-page benefit remains real.
What is missing for PDFium adoption is an observed productive caller
schedule, not access to the original canonical execution environment.

## Files and reproduction scope

`receipt.json` addresses the report, complete raw records, matrices,
source/optimization audit, protocol and experimental consumer patches.
No third-party PDFs, reference pixels, toolchains or decoder binaries are
redistributed. Public originals are fetched by URL and exact hash.

`consumer/document-owner.patch` is relative to the original prepared
consumer; `consumer/allocator-accounting.patch` applies to a **private copy**
of Chromium's allocator support, never a shared dependency checkout.
`consumer/document-harness.patch` supplies the common exact executed document
harness as a delta to the original public consumer, without committing native
source files or bypassing the repository's native-source provenance guard.
The accepted/C++ reference consumer keeps the original bridge/codec objects
and uses only this common harness and allocator accounting.

The actual evaluator scripts are retained under `executed/`. They describe
the executed canonical directory layout, not a promise of a one-command
portable SDK installation. The existing public qualification bundle supplies
the original materializer, source overlay and compiler configuration.
All source/runtime identities were rechecked after measurement.
