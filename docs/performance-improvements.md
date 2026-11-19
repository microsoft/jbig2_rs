# Measured dictionary/context improvements

Baseline: main `96684fef6bd22502095858a02c1e5716c49f8c4d`.
Retained runtime: `69bd3d896b41aea98313126bbc67a29266caf073`.
Later documentation/evidence changes do not change the measured runtime.

The retained PR #182 changes are also backported to `release/0.2`. Its decoder,
models and proofs are byte-identical to main; branch-specific version,
publication scope and lockfiles stay on the 0.2 line. The timings below remain
the original main measurements, not a new release-branch measurement. Published
0.2.1 archives are immutable; this backport does not publish a new crate.

## Retained scope

1. Reuse an established probability-bank invariant across internal symbol
   decodes and owned arithmetic sessions. The original arbitrary-bank checked
   kernel remains and rejects invalid states, including states not touched by
   the decoded pixels. A separate verified kernel requires valid probability
   states. Creation/checked restore, private caller contracts and dictionary
   loop invariants discharge that precondition; arithmetic updates preserve it.
   No assumption, trusted algorithm body or runtime/proof substitution is added.
2. Do not allocate a generic statistics snapshot when the dictionary's
   context-retained flag is clear. Requested snapshots still use the existing
   checked copy/restore path and memory limits.

Public API signatures, strict/recovery policies, dimensions, entropy decoding,
reference validation, truncation checks and configured limits are unchanged.
Avoiding an unused snapshot can admit a dictionary under a tighter memory cap:
the allocation no longer occurs, rather than a limit being increased or an
allocation failure being ignored. Tests cover unused/requested snapshots at
both tight and exact memory boundaries for two context-bank sizes.

## Paired measurements

Rust 1.89.0, optimized Cargo release, portable x86-64 target, no native tuning
or LTO, with debug information. Both binaries use the same probe source and
dependency lockfile. The host is a **shared** AMD EPYC 7763 VM; affinity to CPU 2
is not CPU isolation.

Public inputs are extracted JBIG2 streams (and optional `JBIG2Globals`) from
[`pdfium_tests` at `2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5`](https://pdfium.googlesource.com/pdfium_tests/+/2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5/third_party/jbig2/).
Fixture names below identify the corresponding PDFs. PDF parsing/rendering is
not timed. The existing first-party generated MMR cases supply scale and
multipage controls.

The final comparison uses nine paired process launches, alternating before/after
order and randomizing case order, for 41 fixtures / 45 API configurations.
Normal runs use 15 adaptive-batch samples targeting 350 ms per record. The
large dictionary uses one first decode per fresh process to bound execution.
Every decoded bitmap is checked outside timing against the same previously
native-PDFium-matched packed bytes and dimensions. File I/O, output verification,
final output destruction and preparation of parsed globals are outside timing.

| Fixture | Baseline median | Retained median | Median paired time ratio | Faster pairs |
|---|---:|---:|---:|---:|
| `bitmap-symbol-manysyms` | 9465.18 ms | 143.30 ms | 0.0151 | 9/9 |
| `bitmap-symbol` | 0.6559 ms | 0.2708 ms | 0.4085 | 9/9 |
| `bitmap-symbol-global` | 0.6557 ms | 0.2758 ms | 0.4182 | 9/9 |

The large case is about **66x faster**; ordinary arithmetic-symbol cases take
about **59% less time** in this matrix. These are workload-specific observations,
not universal speedups or latency guarantees. That large dictionary declares
262,144 symbols with a 65,536-entry bank: the old per-symbol scan visited about
17.2 billion states. Avoiding that repeated validation is the primary win.

The generic-snapshot change was also measured incrementally, separately from
bank-validation reuse: the basic/global symbol targets were faster in all nine
pairs, with median time ratios around 0.66. Independent allocator tracking
confirms **one fewer allocation and 131,072 fewer requested bytes** on each
template-0 dictionary without requested generic retention. Ordinary bitmap and
Huffman-refinement controls have unchanged allocation counts and requested bytes.
Allocator-requested payloads are not total RSS or allocator usable sizes.

All 45 final configuration median paired ratios are at most 1.05. Individual
pairs can be slower, and smaller differences remain inconclusive on this shared
host; this does not prove zero regressions. The public fixtures are
coding-feature/regression vectors, often sharing a synthetic 399x400 image,
not a representative distribution of scanned books or office PDFs.

## Experiments excluded

- Arithmetic/Huffman refinement snapshot variants are not retained. The smaller
  Huffman gain failed a predeclared confirmation requirement: both pure targets
  needed at least 14/15 faster pairs and a median ratio at most 0.95. One target
  achieved only 12/15. Arithmetic-refinement savings were not isolated from the
  generic snapshot benefit. The original refinement behavior is preserved.
- An inlining hint for the new helper is not retained: it improved one control
  but left a measured high-bit-depth halftone regression.
- A partial combination that regressed halftones is not the final candidate.
  Using the owning session's existing proved invariant removes that repeated
  bank validation without the inlining hint; the broad matrix was rerun on this
  exact retained source.
- MMR prefix tables, refinement row-window changes and allocator pooling are
  deferred, not bundled as unmeasured improvements.

## Correctness and proof validation

- 520 public/generated API replays: 426 exact bitmap results and 94 metadata
  inspections, across fresh/reusable contexts, strict/best effort, globals and
  standalone reader paths. Both original fatal corpus cases still reject in
  both policies.
- Regression tests compare repeated checked/validated calls, every resulting
  probability state, arithmetic registers and work, and exact guard/error frames
  on checked, validated and caller-owned-bitmap routes.
- Rust 1.89 locked all-feature tests pass: 584 native tests plus one doctest,
  with 14 existing ignored microbenchmarks.
- Fresh whole pinned Verus builds pass for the isolated and retained candidates:
  **1340 obligations, zero errors, 666 directly verified functions**, the same
  20 reviewed limitations, and no new assumptions, incomplete functions or
  runtime/proof divergences. Expanded verifier inventory reconciliation passes.
- All ten previously approved standalone conformance outcomes are byte-identical:
  two bitmap passes, seven reviewed strict rejections and one unsupported colour
  case. Historical normative approval/dispositions are not renewed.
- Existing aggregate/per-file runtime coverage floors pass unchanged.

All existing semantic/contrast/native-mutation, workflow, distribution, coverage,
fuzz and timing gates remain required on the exact PR head. Component proofs and
finite bitmap regressions do not establish full JBIG2/PDFium conformance or a
complete decoded-output proof. No crate publication is part of this work.
