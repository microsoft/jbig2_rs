# Remaining embedded/PDFium compatibility work

Comparison baseline: PR #176 at
`51e5fb05bdfbf5d7d8cf2378d197afd7892d7bfb`. Its clean embedded final-page
completion, standalone validation and all original tests are preserved.
Tested implementation revision: `d368ccf89dbaf9ab352e254840a8211452c1824e`;
subsequent evidence/documentation commits do not change production Rust.

## Strict remains the default

`Decoder::new`, all existing inspection/decoding methods and standalone APIs
retain their original strict policy. There is no implicit retry or C++ fallback.
The additional embedded-only entry points are:

```rust,ignore
use jbig2_decoder::{Decoder, EmbeddedPolicy};

let result = Decoder::default().decode_embedded_with_policy(
    page_stream,
    validated_globals.as_ref(), // None without JBIG2Globals.
    EmbeddedPolicy::BestEffort,
)?;
for recovery in &result.recoveries {
    // Record recovery.segment, recovery.page, recovery.byte_offset and recovery.kind.
}
let document = result.value;
```

`inspect_embedded_with_policy` has the same arguments. Both methods also exist
on `DecoderContext`; normal cleanup occurs on success and error. Explicit
`EmbeddedPolicy::Strict` uses the existing validation with no recovery
preparation. `EmbeddedResult<T>` contains the output and every applied recovery.
An empty recovery list means no source metadata/marker was overridden, not a
certificate of complete standards conformance. Inspection still does not decode
entropy data; its success does not imply a subsequent decode will succeed.

### Initial best-effort allowlist

| Recovery | Boundary and justification |
|---|---|
| `AuxiliaryBuffersFlag` | Set only page-information bit 5 when an associated intermediate region needs the already implemented auxiliary-buffer path. T.88 7.4.8.5 prohibits this combination when the original bit is clear, so strict rejection remains correct. Actual allocation/work limits still apply. |
| `RefinementsFlag` | Set only page-information bit 1 when an associated generic refinement region uses that already implemented path. This is inconsistent metadata under 7.4.8.5, not a strict-mode decoding fix. No geometry, combination operator, prediction or compressed data is changed. |
| `ArithmeticTerminatorCode` | Normalize an existing **FF D0** to **FF AC** only at the exact end of a complete, known-length arithmetic generic-region payload. The complete generic header is first validated, and the marker must be in the coding body, not an AT field. T.88 E.2.9/Figure E.11 emits AC; E.3.3 BYTEIN handles both marker codes identically as markers above 8F. The normal decoder must still complete every row and pass its terminal/interior-marker checks. |

The allowlist inserts or removes no bytes. The existing strict known-length
final-FF convention is unchanged; no additional missing-data synthesis is added.
Unknown-length markers/row counts, absent marker prefixes, other nonstandard
marker codes, early markers, malformed headers, truncation, invalid references,
unsupported syntax and resource exhaustion
remain fatal. Zero-length page information is never repaired. Dictionary/table
selectors are not part of this initial allowlist.

The original stream is first fully framed and reference/page validated.
Only the selected bytes in a fallibly allocated, bounded private copy change;
caller input and globals remain immutable. Byte offsets refer to the supplied
page stream, not the globals stream. The ordinary decoder executes once on that
prepared input; errors never return a partial image or blank success.

Preparation charges an additional structural/reference pass, a bounded
segment-by-page scan and any copy against the work budget. Copy and diagnostic
storage debit the decoder-managed and retained-state budgets. Decoding uses the
remaining effective budgets, which are also the limits reported by a subsequent
resource error. Consequently, best effort may exhaust a tight budget sooner
than strict mode. Width, height, pixel, symbol, reference, depth, stream and
segment limits are never increased.

## Strict decoder fixes

### Retained Huffman refinement statistics

`bitmap-symbol-context-reuse-huffman-refagg.pdf` was not an adapter error.
The Huffman dictionary path validated the context-used flag but never restored
the supplied refinement statistics. T.88 7.4.2.2 steps 3/7 and E.3.8 require
these statistics to carry across compatible dictionaries.

A test-only production-API trace of every exported symbol isolates the first
different bitmap to **dictionary segment 3, exported symbol 0, 100x100**.
Earlier exports match. Its packed-byte hashes are:

- Baseline: `e86fd7565d71eda7c75b3d762ccca424a1a4cd3496690a8c04715c9397f18ecd`
- Corrected: `272852115fa63eed32321348525181128f74cc3561a06752006a9f9407da4d25`

Restoring the validated snapshot before the first bitmap decode corrects the
final 399x400 image to every native-oracle pixel. A generated regression retains
nonzero valid statistics for both templates, verifies imported output pixels
and complete statistics preservation, and rejects an incompatible template.
The new preservation test fails on the unchanged baseline (exit 101).

### Individual versus collective symbol widths

`bitmap-symbol-manysyms.pdf` uses arithmetic symbol coding: there is **no**
collective bitmap. The old class helper nevertheless limited the sum of all
symbol widths as though it were a materialized image. T.88 6.5.5/6.5.8 decode
these bitmaps individually; 6.5.9's collective bitmap is only used for
SDHUFF=1, SDREFAGG=0.

The individual path now checks each real symbol's width, height and pixel
count. It preserves symbol counts, work, cumulative bytes and retained memory.
The existing collective method and its width contract remain unchanged.
Arithmetic and Huffman aggregation callers use the individual method.
Generated tests cover two individually valid symbols whose combined width
exceeds the limit, an actually too-wide symbol, and the still-rejected
collective-width case. An encoded two-aggregate dictionary verifies exact pixels
without constructing a fictitious collective bitmap.

The arithmetic many-symbol fixture now decodes to the native 399x400 bitmap in
both modes. In contrast, `bitmap-symbol-symhuff-texthuff-manysyms.pdf` really
needs a **1,000,008-wide collective bitmap** and still exceeds the unchanged
1,000,000 width cap in both modes. A caller limit review is separate from
leniency.

## Accounting for all 24 reported remaining cases

| Cases | Count | Disposition |
|---|---:|---|
| Huffman context reuse pixel mismatch | 1 | **Strict fix**: restore retained statistics. |
| Arithmetic many-symbol width rejection | 1 | **Strict fix**: limit actual individual bitmaps, not an unallocated sum. |
| Inconsistent auxiliary/refinement page flags | 15 | **Lenient recovery** only; strict rejection retained under 7.4.8.5. |
| `pdfium/bug_880920.pdf` | 1 | **Lenient recovery** of the existing known-length FF D0 tail; strict rejection retained. Output is 3300x2550 and matches the independent native image. |
| Huffman collective many-symbol width | 1 | **Intentional rejection** in both modes: the actual temporary bitmap exceeds the configured width limit. |
| `bitmap-symbol-symhuffcustom-texthuffcustom.pdf` | 1 | **Intentional rejection** in this allowlist: flags FD set the custom aggregation-table selector with SDREFAGG=0, contrary to 7.4.2.1.1 bit 7. There are four referenced custom tables; clearing the bit alone changes table association. A separately reviewed typed table-binding recovery could support this later; no table/reference is silently dropped here. |
| `bug_867501.pdf` | 1 | **Intentional rejection** in both modes: zero page-information payload, no safe source metadata to recover. Legacy cross-segment reads and PDF-supplied dimensions are not emulated. |
| `bug_1236.in`, `bug_1478366.in`, `transfer_function.in` | 3 | **PDFium adapter follow-up**: preserve the native 3562x851 bitmap. The adapter owns 400x400/4000x4000 clipping, padding and canvas composition. Neither policy changes native dimensions. |
| **Total** | **24** | No full-compatibility claim. |

The 15 page-flag selectors, all under `third_party/jbig2/`, are:

- `bitmap-composite-and-xnor-refine.pdf`
- `bitmap-composite-or-xor-replace-refine.pdf`
- `bitmap-halftone-refine.pdf`
- `bitmap-refine-customat-tpgron.pdf`
- `bitmap-refine-customat.pdf`
- `bitmap-refine-lossless.pdf`
- `bitmap-refine-page-subrect.pdf`
- `bitmap-refine-page.pdf`
- `bitmap-refine-refine.pdf`
- `bitmap-refine-template1-tpgron.pdf`
- `bitmap-refine-template1.pdf`
- `bitmap-refine-tpgron.pdf`
- `bitmap-refine.pdf`
- `bitmap-symbol-refine.pdf`
- `bitmap-trailing-7fff-stripped-harder-refine.pdf`

## Reproduction scope and results

The prepared ARM64 PDFium trees, triage TSV and runner logs are absent on this
machine; access was requested but unavailable. The reported **97/121 fixed**,
passing unit/embedder/JavaScript suites and **20 corpus + four pixel** failures
are the user's integration baseline, not a newly executed local runner result.

Independent crate-API comparisons used every image in the 111 public JBIG2
corpus PDFs plus `pdfium/bug_880920.pdf`, hash-checked at immutable
`pdfium_tests` revision `2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5`.
The image oracle is native PDFium
`159.0.8149.0@sourcebuild-toolchained` through pypdfium2 5.15.2, not the prepared
ARM64 build. Each process exit, image dimension, packed-byte hash, recovery
receipt and exact native-image comparison is recorded separately.

| Extracted-stream API configuration | Exact native-image matches | Typed errors | Wrong pixels |
|---|---:|---:|---:|
| Validated PR #176 strict baseline | 92 | 19 | 1 |
| Candidate strict | 94 | 18 | 0 |
| Candidate explicit best effort | 110 | 2 | 0 |

Every previously correct baseline image retains identical packed bytes; the
context-reuse error is corrected. These measurements do not certify whole-page
PDF rendering, the complete PDFium suites, or preservation of the 97 runner
fixes on ARM64. The three canvas cases and zero-length page-information case
remain separately qualified as above.

The new public API file `src/document/embedded.rs` must be included in the local
GN Rust source list when staging a disposable override. Wire
`decode_embedded_with_policy(..., EmbeddedPolicy::BestEffort)` explicitly in a
separate adapter patch, and consume `recoveries`; keep the dependency pin,
expectations and suppressions unchanged. No PDFium adapter file is changed by
this crate follow-up.

The requested C++ control, PR-head, strict-candidate and lenient-candidate
GN/Ninja/targeted runs and all five full suites in both candidate modes were
**not run** here. Their exit statuses are unavailable, not zero. Before merge
or 0.2.1 publication, run those commands in the prepared environment and record
the exact GN dependency revision, selectors and all remaining failures.

Local crate validation uses the CI-pinned Rust 1.89.0. Formatting, strict locked
Clippy and locked all-feature tests pass: 581 native tests plus one doctest,
with 14 existing ignored benchmarks. Whole pinned Verus verification and
compilation pass with 1,339 obligations, zero errors, 665 directly verified
functions and the same 20 reviewed limitations. Expanded verifier inventory
reconciliation also passes. Existing aggregate/per-file coverage floors pass
without modification (12,314/13,259 lines; 2,365/2,704 branches). These local
results are not a fresh full hosted semantic profile or a PDFium suite result.
All 818 audit-tool tests pass. Replaying all ten previously approved standalone
conformance inputs reproduces byte-identical outcomes: two bitmap passes,
seven existing reviewed strict rejections and one unsupported colour case.
This renews neither historical normative approval nor full-conformance claims.

The first hosted candidate run verified/compiled all 61 semantic groups and 494
contrasts, but its collector correctly rejected a 1,080.353-second shard against
the unchanged 1,050-second guard. One complete `region-dispatch` group is moved
from shard 10 to shard 0, using complete measured profiles and a regression.
No check, contrast, worker count, whole/collector role or guard is removed or
relaxed. A fresh complete hosted profile is required after that allocation change.
