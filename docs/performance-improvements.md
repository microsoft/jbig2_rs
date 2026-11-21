# Measured decoder performance improvements

Baseline: main `96684fef6bd22502095858a02c1e5716c49f8c4d`.
Retained runtime: `69bd3d896b41aea98313126bbc67a29266caf073`.
Later documentation/evidence changes do not change the measured runtime.

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

## Follow-up: fixed-width refinement windows

Baseline: main `a65907cdcff89af1bf0f0022c0c1e94c62f155db`, including the
dictionary/context improvements above.
Retained runtime: `0f3ca201a8102a206b628eee7f7e97999a05b3fa`.

The only additional runtime change is `#[inline(always)]` on the existing
`packed_row_bits` helper, following the existing row-ring helper pattern.
Refinement callers supply fixed neighborhood widths and offsets; exposing them
to the optimizer avoids an out-of-line, general-purpose window extraction.
The helper's executing body, scalar boundary fallback, checked coordinates,
storage checks and verifier contracts are byte-for-byte unchanged. No public
API, recovery policy, bitmap dimensions, allocation or configured limit changes.

The same Rust 1.89.0 release probe, dependency lockfile and shared host were used.
Baseline and candidate were rebuilt in the **same directory**, explicitly
cleaning release artifacts between revisions and checking distinct binary
hashes. Nine randomized, alternating paired rounds cover 43 fixtures / 47 API
configurations, including two extra Huffman controls. Each normal record uses
15 adaptive-batch samples targeting 500 ms; the large dictionary remains a
single-decode record. Exact-output validation stays outside timing.

| Fixture | Baseline median | Retained median | Median paired time ratio | Faster pairs |
|---|---:|---:|---:|---:|
| `bitmap-refine` | 3.5216 ms | 2.5455 ms | 0.7260 | 9/9 |
| `bitmap-refine-page` | 3.4888 ms | 2.6002 ms | 0.7400 | 9/9 |
| `bitmap-refine-refine` | 6.2567 ms | 4.4282 ms | 0.7043 | 9/9 |
| `bitmap-refine-template1-tpgron` | 3.4350 ms | 2.6075 ms | 0.7559 | 8/9 |
| `bitmap-symbol-context-reuse-huffman-refagg` | 0.7826 ms | 0.5520 ms | 0.7069 | 9/9 |
| `bitmap-symbol-context-reuse-refagg` | 0.7330 ms | 0.5112 ms | 0.7005 | 8/9 |

These refinement targets take about **24-30% less time**. Independent Callgrind
collection over three `bitmap-refine-page` decodes falls from 184,617,513 to
121,981,209 instructions, about 34% fewer; this is not a hardware-cycle count.
Every configuration's median paired time ratio is at most 1.05. Some individual
pairs regress, and template-3 controls have smaller slowdowns: the shared-host
matrix does not establish zero regressions or a portable latency guarantee.

### Additional experiments not retained

- An MMR one-pixel terminating-code shortcut made generated dense transitions
  about seven times faster and passed exact prefix/tail/error tests and whole
  Verus verification. It nevertheless repeatedly slowed unrelated halftone and
  refinement controls, including same-directory rebuild comparisons. Neither
  that shortcut nor its test-only scaffolding is retained.
- An ordinary `#[inline]` hint did not establish the refinement win. Combining
  forced specialization with the MMR shortcut still left control regressions;
  the final matrix above measures specialization **without** the MMR change.
- A stale-artifact diagnostic produced identical before/after binary hashes and
  was excluded, not counted as performance evidence. Explicit release cleaning
  and hash checks were used for all reported same-directory comparisons.

### Follow-up correctness and proofs

The existing release-mode packed-window scalar oracle covers padding,
boundaries and numeric edges; the randomized refinement oracle covers both
specialized templates. Both pass without changing the tests. All 520 API
replays preserve exact packed pixels/dimensions, metadata and recovery reports;
the original two fatal fixtures still reject in both policies.

Fresh pinned whole Verus passes with the **same 1340 obligations, 666 directly
verified functions and 20 reviewed limitations**, zero errors and no new trusted
bodies or assumptions. Expanded inventory reconciliation passes. All 584 native
tests and one doctest pass, with the same 14 ignored microbenchmarks.
All ten previously approved original standalone outcomes remain byte-identical.
Runtime coverage passes unchanged floors: 12,339/13,283 lines and 2,370/2,708
branches. Source-bound coverage, traceability and conformance identities are
refreshed without renewing historical normative approvals.
All 818 audit-tool tests, the isolated Rust 1.88 MSRV distribution/dry-run check
and the image companion's tests/doctests also pass.

All existing CI checks, including the complete 61-group / 494-contrast Verus
pipeline and unchanged 1050-second timing guard, remain required before merging.
The full PDFium integration runners were not available and are not claimed.

## Follow-up: borrow adaptive context descriptors

Baseline: main `6eb85ee7fdfbaecf969f14b4f9e216fd29db52af`, including both
optimizations above.
Retained runtime: `88ade916215fffc50badf23d3d063f93e2c0c5a2`.

The generic context descriptor contains an extended array of template offsets.
Its context method previously took `self` by value on the per-pixel adaptive
path. It now borrows `&self`; three scalar adaptive offsets are dereferenced
where needed. No context formula, template order, arithmetic operation, storage
check, resource budget or public API changes. This is not a new decoding kernel.
The existing optimized refinement and checked probability-bank paths are intact.

### Measurements

The same probe, Rust 1.89.0 release settings, dependency lockfile and shared host
were used. Both exact revisions were built at the same path, with explicit
release cleaning and distinct binary-hash checks. Nine randomized, alternating
paired rounds cover 43 fixtures / 47 API configurations. Normal records use
15 adaptive-batch samples targeting 500 ms; the large dictionary remains a
single-decode record. Every output is checked outside timing.

| Fixture | Baseline median | Retained median | Median paired time ratio | Faster pairs |
|---|---:|---:|---:|---:|
| `bitmap-customat` | 3.2488 ms | 2.6583 ms | 0.8182 | 9/9 |
| `bitmap-halftone-10bpp` | 3.8620 ms | 3.2157 ms | 0.8372 | 9/9 |
| `bitmap-halftone-global` | 0.9079 ms | 0.8122 ms | 0.8942 | 9/9 |
| `bitmap-halftone-skip-grid` | 1.5012 ms | 1.3434 ms | 0.9080 | 9/9 |
| `bitmap-halftone` | 0.8882 ms | 0.8255 ms | 0.9287 | 8/9 |

Custom adaptive-template decoding takes about **18% less time** and the
high-bit-depth halftone about **16% less time**. Every configuration's median
paired time ratio is below 1.03. A separate 15-pair screen of 14 target/control
configurations, targeting 650 ms per normal record, confirms the high-bit-depth
halftone gain: 3.9946 ms to 3.3133 ms, paired ratio 0.8281, faster in 14/15 pairs.
Every confirmation median ratio is below 1.02.

Independent Callgrind collection for three high-bit-depth halftone decodes
falls from 188,982,518 to 167,038,274 instructions, about 12% fewer.
Instruction counts are not hardware-cycle measurements. Individual pairs can
still regress substantially on this shared host; small control differences are
inconclusive. These coding-feature fixtures are not representative production
PDFs, and these results are not latency guarantees or proof of zero regressions.

### Rejected experiment

Length-gating the original MMR run-table scans was also tested. It preserves
bitwise reads and passes exhaustive prefix/cursor checks and whole Verus.
Dense transitions took about 57% less time, but several unrelated halftone,
stripe and arithmetic controls repeatedly regressed by 6-10%. The candidate was
reverted completely before measuring the retained descriptor change. No MMR
lookup, table, reader, test scaffolding or proof change is retained.

### Correctness and proofs

The existing release-mode literal-template/history oracle and all 11 raster
parity tests pass unchanged, including randomized generic/refinement contexts,
skip masks, retained entropy state, memory boundaries and typed error paths.
All 520 public/generated API replays preserve exact packed pixels/dimensions,
metadata and recovery reports; the two original fatal fixtures still reject in
both policies. The full 584 native tests and one doctest pass with the same
14 ignored microbenchmarks.

Fresh pinned whole Verus passes with the same **1340 obligations, 666 directly
verified functions and 20 reviewed limitations**, zero errors and no new trusted
bodies or assumptions. Expanded inventory reconciliation passes. All ten
previously approved original standalone outcomes remain byte-identical.
Runtime coverage passes unchanged floors: 12,339/13,283 lines and 2,370/2,708
branches. The isolated Rust 1.88 MSRV distribution/no-upload check and image
companion tests/doctests pass. Source identities are rebound without renewing
historical normative approval or conformance claims.
All 818 audit-tool regression tests also pass.

All existing checks, especially the complete 61-group / 494-contrast Verus
pipeline and unchanged 1050-second timing guard, remain required before merging.
No crate publication or release-branch backport is included in this follow-up.

## Follow-up: compact indexed MMR run lookup

Baseline: main `f929feab4ce13139db1d620580f4c83a3b12ac89`.
Measured runtime: `18a6650072e7f813e874dc01821e6a1c92bfff74`.
Later documentation/evidence changes preserve that runtime.

Run-table scans are replaced by two compile-time-derived ordinal indexes.
Each key combines the code length with its significant bits. All current
canonical codes have at most eight significant bits despite their leading-zero
lengths; exhaustive exact-key tests check this representation against every
canonical entry. The two 14-by-256-byte indexes occupy **7 KiB total**, rather
than the earlier prototype's 32 KiB. There is no per-decoder allocation or
increase to any resource budget.

The scalar one-bit reader loop is unchanged: no wide peek or changed shortest
prefix, physical-EOF, cursor or malformed-input behavior. The immutable colour
index is selected once per run code. The actual lookup body also accepts
controlled invalid indexes in tests, exercising its defensive checks rather
than substituting a test-only decoder. Selected ordinals are always checked
against the actual canonical table entry's length/bits. Checked length and
eight-bit fields prove the computed index is below 3584 without overflow.

The generator unrolls table ordinals because pinned Verus ghost loop markers
cannot execute during Rust constant evaluation. No normative bitstrings are
duplicated. Both generator functions and both constant initializers are checked.
Expanded inventory qualification requires exact source kind, coordinates,
module/name, checked body and whole-report identity; initializers are not
counted as native callables. Tests reject forged/missing identities or bodies.

### Measurements and the earlier regression

The previous 32 KiB coverage-complete prototypes showed repeatable 8-13%
arithmetic/halftone control slowdowns. For five unchanged halftone decodes,
baseline, original indexed and held indexed binaries each execute exactly
68,961,621 Callgrind instructions. The hot context function has identical
normalized instructions but different placement. This indicates sensitivity
to compiler placement/cache effects, not extra decoding work; it does not
establish a specific hardware-cache diagnosis. Merely forcing the MMR routine
out of line did not resolve the slowdown and is not retained.

The smaller index is a structural footprint reduction, not alignment padding
or a consumer-specific compiler flag. Both compared revisions use the same
Rust 1.89.0 release probe, lockfile, build directory and flags. Explicit release
cleaning and distinct binary hashes guard stale builds. Nine randomized,
alternating paired rounds cover 43 fixtures / 47 API configurations, with
15 adaptive-batch samples targeting 500 ms per normal record. The large
dictionary remains one decode per fresh process; exact-output validation,
file I/O, output drop and parsed-globals preparation are outside timing.

| Fixture | Baseline median | Compact median | Median paired ratio |
|---|---:|---:|---:|
| `bitmap-mmr` | 0.0430 ms | 0.0340 ms | 0.7924 |
| `bitmap-halftone-10bpp-mmr` | 0.3883 ms | 0.2727 ms | 0.6812 |
| `generated-transitions-512` | 11.6929 ms | 1.8586 ms | 0.1616 |

An independent 15-pair confirmation over 21 configurations, targeting 650 ms,
gives basic/MMR-halftone ratios 0.8033/0.6569 and dense-transition ratio 0.1591.
All MMR targets are faster in 15/15 confirmation pairs. The worst median
slowdown is **2.22% in the broad matrix and 1.41% in confirmation**, within the
unchanged 5% acceptance cutoff. Public MMR cases take about 20-34% less time;
dense transitions are about 6.3 times faster. Individual pairs still regress
on this shared host; these finite feature vectors do not prove zero regressions
or portable latency guarantees, nor represent full PDF rendering.

### Correctness and proof validation

All 520 public/generated API replays preserve packed pixels/dimensions,
metadata and recovery reports; the original two fatal fixtures still reject
in both policies. Exact-key, invalid-ordinal/key and exhaustive short-tail
tests supplement every 13-bit window, colour/alignment and horizontal/makeup
oracle. All 587 native tests and one doctest pass with 14 existing ignored
microbenchmarks. All ten previously approved standalone outcomes are unchanged.

Fresh whole Verus passes **1340 obligations, zero errors, 668 directly verified
functions**, the same 20 reviewed limitations and two separately qualified
checked constant initializers. The original runtime coverage floors pass
without reduction. Table-source fidelity and complete decoded output are not
relabeled independent normative proofs. All existing audit, compiler inventory,
mutation/fuzz, coverage, distribution and companion gates remain required on
the exact final head, including all 61 groups / 494 contrasts and the unchanged
1050-second timing guard. No crate publication is part of this work.

## Follow-up: exact non-progressing dictionary cycles

Baseline: main `7db4da60177d8c227d39315d265d44ac692f7142`.
This follow-up targets main/0.3 only.

Reports #215/#216 contain framed arithmetic dictionaries that repeat zero
export runs or empty height classes. Legal zero runs and empty classes remain
supported. An observed marker or repeated value alone is not a rejection rule.

The rare path checkpoints every arithmetic register, the immutable input,
both complete procedure banks and the logical dictionary cursor, including
export-flag phase. Equality is exact, not a hash or partial-state guess.
Checkpoint-refresh prefix keys are only an optimization; the final match
still checks every bank entry and input byte. The proved, actually executed
zero-only trace establishes a closed deterministic orbit: no finite
continuation can advance the dictionary cursor. Only then is work accelerated
to the identical existing failure boundary. Finite budgets still report
`Work` at limit plus one; a maximum `u64` budget retains integer overflow
without wrapping. No partial image or success fallback is returned.

Normal single zero runs do not allocate or copy a checkpoint. Fixed-size
stack checkpoints introduce no heap allocation. Small-budget activation
thresholds only avoid unprofitable checkpoint work; they neither reject input
nor modify the configured budget.

On the shared Linux host, the two unchanged-default-budget failures now
return in roughly four milliseconds; the previous binaries exceed the
three-second reproduction deadline. Alternating-order paired measurements
at one million units show about 88-94% less time. The original 100,000-unit
export case avoids checkpoint overhead; empty-height cases take about
32-35% less time. These are fixture-specific observations, not universal
speedups. Exact typed outcomes are checked on every timed repetition.

Unlike the rejected integer-inlining experiments, fixed-work valid controls
do not show the earlier 15-20% dense-MMR slowdown. The measured control
medians remain within the unchanged five-percent acceptance cutoff; shared
host noise and smaller differences do not establish zero regressions.

The executing code has a fresh pinned whole Verus result of 1,411 obligations,
zero errors, 703 directly verified functions and the same 20 reviewed
limitations, with no incomplete functions or new assumptions. Its recurrence
proof binds full native checkpoints to the genuine trace and proves
arbitrarily long zero continuations; it is not a complete decoded-output
or independent normative proof. Full-state/cursor/input mismatch controls,
legal interleaved zero runs, empty classes and exact default/maximum work
boundaries supplement the contracts.

The pinned corpus now includes default-budget checks through every supported
API shape: 170 configurations / 238 decoder calls, with a three-second
deadline and exact Work errors for both nonprogress samples. Every existing
native, audit, compiler, coverage, distribution, semantic/contrast and timing
gate remains mandatory. No release backport or publication is included.
