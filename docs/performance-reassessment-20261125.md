# Reopened performance assessment: measured results, not closure counts

The blanket final disposition of #227-#240 was too broad. Seven assessments
were reopened: #227, #231, #233, #235, #236, #238 and #240. This report records
new experiments and distinguishes actual gains, failed retention and unfinished
work. No runtime from these trials is adopted by this report.

## Five completed native trial confirmations

Baseline: accepted main `14aeac451f6491aa3e413fcbc60aafb11fa75b7c`.
Every trial retains all 123 frozen inputs / 127 configurations, nine reversed-order
process pairs and 31 adaptive samples targeting 350ms per normal process.
The first-decode large dictionary remains a cold single-decode measurement.

There are **5,715 paired measurements / 635 complete case-variant summaries**.
Both sides use clean same-path O3/native/ThinLTO builds, one compiler/lockfile,
equal output work and CPU2 affinity. This is a shared x86-64 host, not an isolated
or matched N2/PDFium runner. All our compiler/proof work was stopped during
latency collection; no adverse case was dropped or build padding introduced.

| Issue / implementation | Actual target observation | Retention result |
|---|---|---|
| #231 isolated cursor, `5f1994e` | Ordinary arithmetic halftone 0.70157; composite 0.64802; 10bpp 0.69464 | Three adverse control medians: MMR10bpp 1.06065, refinement/export 1.10011, text refinement 1.05253. Not retained. |
| #231 dedicated kernel, `ab8e8be` | Ordinary 0.67846, 10bpp 0.54865, global 0.65748; each 9/9 faster with paired 95% upper ratio below 1 | Three unresolved controls: MMR10bpp 1.06525, refinement/export 1.08894, Huffman custom dimensions 1.09092. Not retained despite real gains. |
| #235 arbitrary-offset byte groups, `f3e4140` | Ordinary Huffman 0.96277, but 7/9 faster and upper interval 1.00043 | Three adverse controls, including MMR10bpp 1.05447 (0/9 faster) and uncompressed Huffman 1.12626. Not retained; no reservoir claim. |
| #240 guarded two shifts, `5c8d9b8` | No consistent broad material gain: large generic 0.98217, 6/9 faster, upper interval 1.00857 | White512 control 1.07172. Keep the existing engine; no general impossibility claim. |
| #227 direct packed bytes, `453fff5` | Plain templates1/2/3: 0.97132/0.98227/0.92632, none meeting the declared target confidence; T0 large generic about1.97 | 67 adverse medians, plus the correctness caveat below. This earlier source is not retainable. |

Ratios are paired medians of process medians, not quotients of marginal medians.
The [complete table](../validation/performance/reassessment-20261125.tsv) retains
all 635 summaries, intervals and pair counts, not only the examples above.
The [machine report](../validation/performance/reassessment-20261125.json)
binds every binary, source, compiler, protocol, cohort and raw-record hash.

The pattern mechanisms therefore **do provide actual local gains**. Their
unresolved regressions prevent adoption under the existing retention discipline;
they were not discarded merely because no speedup was observed. Possible
code-generation/shared-host effects are not an established explanation or a
reason to waive a control.

## Correctness/proof scope and the #227 correction

The isolated cursor passes 11 focused MSRV native tests, all 123/468 public-API
oracles and fresh whole **1541/0**. The dedicated kernel passes the same public
oracles and whole **1543/0**, retaining the original general descriptor enums.
The byte-group reader retains its exact result/error/cursor contract and
passes exhaustive reader oracles, whole **1542/0**, MSRV lint/source audits and
123/468 public checks. Bounded normalization passes the existing exhaustive
numeric state/partial-error oracles, whole **1532/0** and the public checks.
These are component/whole proofs, not all project acceptance gates.

The initial packed-byte source passed whole **1537/0**, generic-helper parity
and positive public outputs, but those contracts/tests did not establish the
production ring's complete-row error-commit semantics on every initial target.
Further examination found that its immediate partial-row writes and handling
of preexisting nonzero storage differed from that route. This is an incomplete
property/test boundary, not a false theorem about a proved full-raster contract.

The initial binary and its adverse timing data are preserved unchanged.
A separate correction restores dirty-target fallback and zero-row rollback;
160 additional production-route/error/dirty-target comparisons pass. Fresh
corrected-source proof/measurement is separate and must not be inferred from
the earlier receipt or timing table. Nothing was merged with that gap.

## New attribution that warrants further work

Five coarse instrumented rounds replay all 123 existing golden cases:
**615 case/round records**. This measures wall-clock phase estimates, not
hardware cycles or uninstrumented candidate gains; categories can nest.

- **#233:** real Huffman standard-construction fractions are about 5-13.6%:
  ordinary 10.44%, mixed-standard 10.99%, aggregate refinement 13.63%.
  The all-standard descriptor/reuse experiment is justified; the old A-only
  result did not complete that scope.
- **#236:** the actual public 10bpp MMR case spends about 1.956% of total
  instrumented decode time in plane XOR/index accumulation. Controlled real-MMR
  scaling fixtures with about one million cells spend about 96% of the
  plane-reconstruction time there. These are deliberately generated scaling
  controls, not newly discovered representative PDFs. Narrow/wide storage is
  being tested rather than rejected from the old small-grid counts.
- **#238:** original `file_pages.jbig2` has three pages sharing actual global
  bytes. Five processes, each with 1001 complete three-page sequences, measure
  global resource reconstruction at 23.96% of a 475086ns median instrumented
  sequence. Avoiding only two after-first reconstructions gives an optimistic
  15.98% phase bound **before** real preparation/copy/resident-memory costs.
  The private explicit-owner copy experiment must measure those costs.

The other apparent reused source contains a global EndOfPage and correctly
fails actual embedded decoding with `InvalidPageAssociation { segment: 2,
page: 0 }`. Metadata parsing alone accepts it; that does not make it an admissible
cache-performance target. The refusal remains a hard control.

The available gprofng collector failed clock-profile initialization. Its empty
profile was rejected as evidence. No hardware-counter policy or host setting
was changed. The original qualification bundle is public and the previous N2
receipt remains valid for its original heads only; it is not a measurement of
these additional runtimes.
