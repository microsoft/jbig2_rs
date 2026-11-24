# Renewed original N2 / LLVM24 / PDFium qualification

Date: 2026-11-24.

- Exact PR base: `3cfff21b3bc157a5908e32199c1e94b2811ad806`.
- Exact candidate: `121bee27be8080a6de3642375d2f84d1967f24dd`.
- Original C++ binary identity retained:
  `a854099a628a7ef303fcb9a68f8a6186d6e4021acd8471a6aea3f6a0181e638d`.
- Native Linux ARM64 Neoverse-N2, CPU2, shared host, original matched
  Clang c3223914 / Chromium Rust41b321963 LLVM24 toolchains.
- Actual commands audited as O3/native/ThinLTO/final-link-O3; strict and best
  core archives are identical within each source revision. Base and candidate
  archives/binaries are distinct. See [build audit](renewed-build-audit.json).

## Result

**Functional gate PASS. Performance median-regression gate PASS.**

Exact materialization reproduced every original source/payload/globals/canvas
identity: **321 image requests + 140 PDFs**. All **2,305 outcomes** across C++,
base strict/best and candidate strict/best preserve the original C++ control
and base/candidate statuses, pixels, typed diagnostics and recovery receipts.
All **183 mapped independent-reference decisions** remain equal between base
and candidate. All **88 targeted tests** across the four Rust consumers pass.

Performance is **nine predeclared rotating process rounds**, the original
68-case frozen selection, three warmups and31 image/11 PDF timed repetitions.
There are **2,934 timed processes**, with strict variants additionally measured
on the original61 shared eligible cases. No adverse case or process was
dropped, no alignment/padding/compiler/layout knob changed, and no repeated
run was substituted to obtain a pass.

Across all129 case/policy medians, none exceeds the unchanged +5% regression
threshold. The worst is **1.0432609** (strict `refine_refined_region`).
That is not a guarantee of zero regression or hardware-independent behavior.
No new allocator/peak-payload claim is inferred from this timing run.

Primary best-effort paired candidate/base medians:

| Target | Ratio | Descriptive paired95% interval | Faster rounds | Candidate/C++ |
|---|---:|---|---:|---:|
| AND/XNOR halftone image | 0.41636 | 0.41373-0.41846 | 9/9 | 1.60964 |
| OR/XOR/REPLACE halftone image | 0.42695 | 0.42525-0.42973 | 9/9 | 1.53356 |
| Many-symbol complete PDF | 0.83600 | 0.83506-0.83832 | 9/9 | 1.03490 |
| Hayro Huffman-many-symbols | 0.63409 | 0.63160-0.64406 | 9/9 | 0.66836 |

These are different workloads; do not relabel the mixed `bug_880920` image as
the many-symbol document. C++ is not beaten on every family. The paired
percentile bootstrap has only nine rounds and is descriptive; full raw data
and all per-case tails remain available.

## Reviewable evidence

- [Exact materialization receipt](materialization-verification.json)
- [Actual compiler/source/binary audit](renewed-build-audit.json)
- [Complete functional gate](renewed-functional-gate.json)
- [All functional outcomes and output hashes](renewed-functionality.json)
- [Independent-reference decisions](renewed-references.json)
- [Every process-round timing](renewed-performance.json)
- [Complete summary, intervals and adverse list](renewed-summary.json)
- [All case/policy medians in TSV](per-case.tsv)
- [Content-addressed receipt](receipt.json)

JSON receipts remove machine-local paths and temporary pixel locations.
They retain exact source and binary identities, input identity via the frozen
cohort, output hashes, typed diagnostics, original refusals and measurements.
Public fixture licensing remains at the pinned sources; no PDF/oracle pixels,
toolchain or compiled third-party binary is redistributed.

This resolves the unavailable-original-evaluator qualification blocker for the
tested candidate. It does not merge/approve PR #241, change its source, replace
project CI or authorize a release. Retain normal review and exact-head checks.
