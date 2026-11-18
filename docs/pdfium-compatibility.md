# Embedded page completion and PDFium

## Maintenance scope

This patch is based on the published 0.2.0 source,
`4f52fd1a31a2c3e37cff9d898401b6e4a6729e6b`. It changes only embedded final-page
completion, not the public API, package version, entropy decoding, dependency
pin, test expectations, feature support or resource limits.

Both embedded inspection and decoding previously required an explicit
end-of-page segment. Removing that check alone was insufficient: the decode
dispatcher delivered pages only on explicit end-of-page segments and rejected
any remaining active page.

At the end of a completely parsed embedded input, only the final page may now
be completed implicitly. Earlier pages still require explicit boundaries.
Known-height pages use their declared dimensions. Initially unknown-height
pages require a final end-of-stripe, no subsequent region, and no preceding
immediate region extending below its stripe's end row. The terminal stripe
determines the inclusive final row; dimensions are never inferred from a PDF
image dictionary. This prevents a region from being silently clipped to an
incomplete final stripe.

The parser must first consume every complete header and payload. The decode
path must still decode each compressed bitmap successfully. Missing or partial
headers, partial declared payloads, incomplete entropy data, invalid references
and over-limit images remain errors. No synthetic segment is added, so segment
counts, reference identities and structural resource accounting are preserved.
Standalone sequential and random-access validation remains strict.

The generated public-API regressions exercise exact nonuniform 9x3 bitmaps,
padding, initially known/unknown heights, explicit and implicit boundaries,
optional end-of-file, parsed globals (including actual reference resolution),
reusable decoder contexts, standalone rejection, truncated input, unfinished
stripes and the unchanged height limit.

The public API documentation explicitly identifies this as PDF/PDFium
integration behavior. First-party, generated regressions live in
[`page_lifecycle.rs`](../tests/public_envelope/page_lifecycle.rs), including
`pdfium_embedded_stream_end_completes_the_final_page_with_and_without_globals`,
`pdfium_embedded_completion_rejects_truncated_segments_and_unfinished_stripes`,
and `pdf_integration_does_not_relax_standalone_or_earlier_page_boundaries`.
They verify dimensions and packed pixels rather than just successful returns.
No external PDF fixture is required to run these crate regressions.

## Local results

The initial nine generated lifecycle tests were run against the original 0.2.0
implementation with only the test file replaced: six passed and three failed
(exit 101), including the expected `MissingEndOfPage(1)` rejections. The patched
implementation passes those cases. Further coverage adds clipped/empty regions,
an intermediate region after the final stripe, a truncated region header, and
direct checks that the standalone dispatcher never enables implicit completion.

| Candidate check | Exit status | Result |
|---|---|---|
| `cargo fmt --all --check` | 0 | Formatted |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Passed |
| `cargo test --all-features` | 0 | 575 native tests and one doctest passed; 14 benchmarks ignored |
| Rust 1.88 public lifecycle tests | 0 | All ten passed |
| Whole pinned Verus build and compilation | 0 | 1,316 obligations verified; zero errors |
| Exact production proof accounting | 0 | 649 verified functions; 20 existing reviewed limitations |
| Expanded verifier inventory reconciliation | 0 | New helper and existing functions reconciled |
| Count/parser and page-finish proof sensitivity | 0 | 13 and seven compiling strong/weaker contrasts passed |
| Audit-tool validation gates | 0 | All 817 passed |
| Aggregate and per-file runtime coverage | 0 | Existing floors retained; 11,907/12,895 lines and 2,320/2,658 branches covered |
| Rust 1.88 actual-archive distribution check | 0 | Extracted tests, doctest, downstream output and API isolation passed |
| Existing standalone conformance replay | 0 | Unchanged: two bitmap passes, seven reviewed rejections, one unsupported case |

These are local results, not fresh hosted CI or complete formal output proofs.
The existing reader/callback trust-boundary dispositions are preserved.

### Verus non-regression gate

The maintenance candidate must pass the complete
[Verus workflow](../.github/workflows/verus.yml): whole-crate verification and
compilation, exact production accounting, original expanded-verifier inventory,
source-bound traceability, all 61 semantic groups / 494 compiling strong/weaker
contrasts, and the complete same-run measured-profile guard. A successful
whole build or selected group alone does not satisfy this gate.

The existing contracts, assumptions, trust boundaries, contrast cases, native
mutation controls, shard count and resource limits are retained. The initial
PDF/PDFium documentation/naming follow-up was checked for identical non-comment
runtime/model/proof/contract/cfg tokens across all 217 Rust files under `src/`,
`specs/` and `proofs/`. Subsequent coverage additions extend only tests, without
weakening assertions or changing production logic or contracts.

Hosted proof results remain separate from the unavailable PDFium GN/Ninja
integration and full baseline described below. Passing one does not establish
the other.

### Independent public-fixture API comparisons

The three requested corpus fixtures were fetched and hash-checked at
[pdfium_tests `2bcc4fd`](https://pdfium.googlesource.com/pdfium_tests/+/2bcc4fdc9abe4c4c115c82c486ac4c46f1d474c5/third_party/jbig2/).
Their extracted streams were decoded through the original and candidate public
crate APIs, with parsed globals where applicable. Native image pixels came
from the independently versioned C++ PDFium build described below:

| Fixture | Original crate exit | Candidate exit | Candidate bitmap |
|---|---|---|---|
| `bitmap.pdf` | 1, missing end-of-page | 0 | 399x400; all native pixels match |
| `bitmap-symbol-global.pdf` | 1, missing end-of-page | 0 | 399x400; all native pixels match |
| `bitmap-stripe-initially-unknown-height.pdf` | 1, missing end-of-page | 0 | 399x400; all native pixels match |

The six named recoverable pixel fixtures were also inspected at the public
PDFium snapshot below. Templates were expanded with that snapshot's unmodified
template tool. All seven extracted JBIG2 image uses (the transfer-function
fixture uses its image twice) change from missing-end-of-page rejection
(exit 1) to successful crate decode (exit 0).

For `bug_1087`, both composite fixtures, and the three corpus cases, dimensions
and every native image pixel match directly. The JBIG2 streams in `bug_1236`,
`bug_1478366` and `transfer_function` declare 3562x851, while their PDF image
dictionaries request 400x400, 4000x4000 and 400x400 respectively. Every pixel
inside the shared image extent matches native PDFium. Cropping produces the
native 400x400 images; the larger native 4000x4000 image has black pixels
outside the declared JBIG2 extent. Black padding empirically matches that native
image, but it is **not** justified by the JBIG2 default-pixel flag (which is zero).
No padding or PDF-dimension substitution is added to the crate. The actual
integration must check its own PDF canvas handling and complete-page output.

These API probes are not GN/Ninja builds, embedder tests, or the prescribed
pixel/corpus test runners; they must not be reported as those suites passing.

## Zero-length page information: no recovery

The accessible public PDFium fixture and expected image were inspected at
[`a843234`](https://github.com/chromium/pdfium/tree/a84323421e94f484faca52dd9d027934eba42ab8):

- [PDF fixture](https://github.com/chromium/pdfium/blob/a84323421e94f484faca52dd9d027934eba42ab8/testing/resources/pixel/bug_867501.pdf)
- [Expected image](https://github.com/chromium/pdfium/blob/a84323421e94f484faca52dd9d027934eba42ab8/testing/resources/pixel/bug_867501_expected.pdf.0.png)
- [C++ decoder](https://github.com/chromium/pdfium/blob/a84323421e94f484faca52dd9d027934eba42ab8/core/fxcodec/jbig2/JBig2_Context.cpp)

The PDF image dictionary specifies 1x3 pixels, but the JBIG2 page-information
segment declares **zero** payload bytes and page association zero. Its next
segment also declares zero length. The C++ page-information handler nevertheless
reads 19 bytes from the shared stream, crossing into the following segment.
Those bytes do not specify a meaningful page width/height. `GetFirstPage`
supplies the output bitmap using dimensions from the PDF caller, and the
sequential decoder resets its cursor to the declared payload end afterward.
This is not equivalent to ignoring an optional empty metadata segment.

The expectation is not blank: the 94x450 PNG has 150 white rows, 150 black rows
and 150 white rows. An independently installed native PDFium
`159.0.8149.0@sourcebuild-toolchained` (pypdfium2 5.15.2) also renders the black
middle band. Its default page-size rounding produces 95x450 instead of the
test runner's 94x450; this is not a reproduction of the requested integration
binary or its test runner. The raw decoded image is exactly 1x3, with white,
black, white pixels. Cropping its page rendering to the expected 94x450 bounds
matches every expected pixel. The PDF parser recovers a 77-byte image stream
despite the PDF stream's declared length of 17; the JBIG2 segment-length
overreads described above occur within those recovered bytes.

The crate has neither the missing valid page information nor a PDF image
dictionary in its API. Emulating the C++ behavior would require reading outside
a declared segment, reinterpreting following bytes and substituting external
dimensions. There is no unambiguous bounded recovery in the existing API.
Strict `InvalidSegmentDataLength { segment_type: 48, expected: 19, actual: 0 }`
is therefore retained and tested. This PDFium compatibility failure remains.

## Integration validation limitation

The requested `/workspace/pdfium-dev/pdfium` checkout, build outputs and
`jbig2-pdfium-regressions.md` report were absent in the execution environment.
The requested PDFium revision
`68f43efa71ac66ba843ba286bb76d075aa1c49df` was not available from the public
source endpoints inspected. The C++ source/fixture snapshot above is explicitly
a different revision, not a substitute for the prescribed baseline.

Consequently, neither the original-pin PDFium targeted commands, candidate
GN/Ninja build, targeted suites nor full Rust-enabled PDFium baseline could be
run. Their exit statuses are **not available**, not zero. The reported
112 corpus, six pixel and two embedder missing-end-of-page failures have not
been independently counted or confirmed eliminated. No full-PDFium-green or
0.2.1 release-readiness claim follows from the crate regressions.

The implementation is a narrowly scoped 0.2.1 backport candidate, but the
integration acceptance target remains blocked until the same prepared checkout
can run the original and candidate binaries, confirm GN's dependency source,
and execute all requested targeted and full baseline commands.
