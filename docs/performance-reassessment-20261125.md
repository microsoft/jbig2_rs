# Reopened performance assessment: measured results, not closure counts

The blanket final disposition of #227-#240 was too broad. Seven assessments
were reopened: #227, #231, #233, #235, #236, #238 and #240. This report records
new experiments and distinguishes actual gains, failed retention and unfinished
work. No runtime from these trials is adopted by this report.

## Eight completed native trial confirmations

Baseline: accepted main `14aeac451f6491aa3e413fcbc60aafb11fa75b7c`.
The first five trials retain all 123 frozen inputs / 127 configurations. The
last three add six explicitly generated scaling inputs: 129 inputs / 133
configurations, without removing any original case. Every trial uses nine
reversed-order process pairs and 31 adaptive samples targeting 350ms per normal process.
The first-decode large dictionary remains a cold single-decode measurement.

There are **9,306 paired measurements / 1,034 complete case-variant summaries**.
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
| #227 corrected packed bytes, `dfeb892` | Plain templates1/2/3: 0.89476/0.91685/0.98093, none meeting the declared target confidence; large generic 1.98750, 0/9 faster, 95% interval 1.9504-2.1024 | 66 adverse medians, worst 2.00882. Correctness repaired, but reject this measured design on performance. |
| #233 all 15 standard descriptors, `d66db50` | Context-reuse refinement aggregation 0.88429, 8/9 faster, upper interval 0.95352; ordinary Huffman 0.81686 but interval crosses 1 | Two unresolved controls: MMR10bpp 1.07317 and generated depth9 grid 1.05957. Real target gains warrant an independent full-cohort diagnostic; not adopted. |
| #236 narrow/wide index owner, `78440b9` | Generated million-cell depth1 grid 0.93790, 9/9 faster, upper interval 0.95378; ordinary halftone 0.99836 | Generated depth8 grid regresses to 1.03731, 0/9 faster; three controls exceed 1.05. A measured per-byte function boundary is being corrected before any new timing. |

Ratios are paired medians of process medians, not quotients of marginal medians.
The [complete table](../validation/performance/reassessment-20261125.tsv) retains
all 1,034 summaries, intervals and pair counts, not only the examples above.
The [machine report](../validation/performance/reassessment-20261125.json)
binds every binary, source, compiler, protocol, cohort and raw-record hash.
The [complete compressed native records](../validation/performance/native-raw-pairs-20261125.jsonl.gz)
also preserve every original sample and process diagnostic for all 9,306 pairs.

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
corrected source `dfeb892` passes all 512 focused configurations, fresh whole
**1539/0**, native/lint/source audits and **129 inputs / 492 public-API checks**.
Its separate nine-round measurement is now complete and remains adverse as
shown above. This design is discarded based on actual implementation and
measurements, not because packing in general is impossible. Nothing was merged
with the earlier correctness gap.

The all-standard descriptor source passes native and Rust 1.88 suites
**647 tests each**, whole **1577/0**, and **757 direct verified functions plus
20 unchanged reviewed limitations**; all original 750 direct functions remain.
The narrow-owner source passes whole **1538/0**, native/resource-boundary
oracles and the 129/492 public checks. These are not full merge qualification.
Its emitted machine code calls an outlined enum/Result accumulator for every
eight cells. A single ordinary inline hint did not remove that call and was
not benchmarked as an alleged fix. The next substantive variant dispatches
once per plane into typed traversal; its proof/correctness results are recorded
below, and its latency measurement remains pending.

Neither the two descriptor controls nor unrelated adverse controls are
explained merely by observing unchanged source. In particular, the owned
`HuffmanTable` layout did not change; a layout-change explanation would be
incorrect. A portable exact full-cohort hosted diagnostic is being prepared,
with identical-operation calibration and every existing adverse case retained.

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
  Those costs are included in the separate private experiments below; the
  phase bound itself is not a measured candidate speedup.

The other apparent reused source contains a global EndOfPage and correctly
fails actual embedded decoding with `InvalidPageAssociation { segment: 2,
page: 0 }`. Metadata parsing alone accepts it; that does not make it an admissible
cache-performance target. The refusal remains a hard control.

The available gprofng collector failed clock-profile initialization. Its empty
profile was rejected as evidence. No hardware-counter policy or host setting
was changed. The original qualification bundle is public and the previous N2
receipt remains valid for its original heads only; it is not a measurement of
these additional runtimes.

## Bounded decoded-global reuse: actual independent replication

The private, test-only experiment measures actual preparation, fallible copies,
all three outputs and complete cold owner destruction. Local shared-host cold
ratio **0.85414** is promising but only 7/9 pairs are faster, with interval
**0.83320-1.31804**. That weak result was retained, not rerun until passing.
An independent hosted replication with predeclared baseline/baseline calibration
was then executed on two architectures:

| Independent run | Complete cold owner per three-page sequence | Secondary warm same-owner repetition | Identical-operation calibration |
|---|---|---|---|
| x86-64, [45608190010](https://github.com/microsoft/jbig2_rs/actions/runs/45608190010), source `f6aba19` | 0.82814, 9/9 faster, 95% 0.82607-0.83102; p95 ratio 0.83338 | 0.68791, 9/9, 95% 0.68538-0.68993 | 0.99772, 95% 0.99513-1.00645 |
| Native aarch64/N2, [45612105976](https://github.com/microsoft/jbig2_rs/actions/runs/45612105976), source `ec25aa4` | 0.85759, 9/9 faster, 95% 0.85618-0.86142; p95 ratio 0.85918 | 0.74857, 9/9, 95% 0.74594-0.75075 | 0.99951, 95% 0.99761-1.00236 |

Each hosted run has 27 paired comparisons, 54 fresh processes and 54,054 raw
three-page sequence samples. All raw samples, process totals, order, medians,
p95 and bootstrap intervals were independently recomputed for both runs.
Calibration intervals satisfy the predeclared 0.95-1.05 rule. Actual Arm identity
is implementer `0x41`, part `0xd49`, variant/revision 0. Both use native Rust
1.98.1 / LLVM22, **not** the original LLVM24/PDFium qualification.

Important limits:

- The original three-page source is real, but the benchmark normalizes it into
  separate embedded API requests. The existing standalone `Decoder.decode`
  already shares its resources. This is **not** an existing standalone-API
  improvement, an observed production PDF request trace, or full-PDF/C++ gain.
- The experiment allows **65,536 extra bytes** for the owner and copies.
  Separate capacity checks report 22,164 resident bytes / 23,024 peak owner
  plus copy bytes; these are neither RSS nor a same-total-memory-budget claim.
- Warm repetition is diagnostic, not measured production repaint frequency
  or permission for cross-document reuse. The malformed global-EndOfPage
  sequence remains an actual refusing control.
- Production was unchanged in the measured test-only source. Its old
  **1532/750/20** proof receipt does **not** prove a production cache.
  A small explicitly opted-in owner with production contracts is under
  implementation/validation and is not qualified by the private timings.
- Searches of additional actual PDFs have not yet found distinct painted
  images sharing nonempty globals. Single-image global PDFs are controls,
  not invented reuse opportunities.

Experimental manual workflow changes remain unmerged and must not enter a
product PR. No new runtime has been merged or released by this assessment.

The complete original private reports, including all raw sequence samples,
are retained separately for the [shared local host](../validation/performance/prepared-private-local-20261125.json.gz),
[hosted x86](../validation/performance/prepared-private-hosted-x86-20261125.json.gz)
and [hosted Arm](../validation/performance/prepared-private-hosted-arm-20261125.json.gz).

## Separate actual allocation observations

Allocation-instrumented builds are never used for latency. Each of the 129
inputs was checked with baseline, all-standard descriptors and corrected typed
narrow/wide storage: **387 exact-output operations**. The measurement holds
the returned output at the end and excludes preexisting input/expected/global
owners; it reports allocator-request payload, not RSS.

| Case / candidate | Requests | Total requested bytes | Peak live requested bytes |
|---|---:|---:|---:|
| Context-reuse Huffman refinement aggregation, baseline | 146 | 503,864 | 235,980 |
| Same case, all-standard descriptors | 65 | 219,244 | 162,180 |
| Million-cell depth1, baseline | 34 | 4,389,604 | 4,388,312 |
| Same case, typed narrow owner | 34 | 1,243,876 | 1,242,584 |
| Million-cell depth8, baseline | 309 | 5,378,732 | 4,531,496 |
| Same case, typed narrow owner | 309 | 2,233,004 | 1,385,768 |

The 3,145,728-byte depth1/depth8 reduction is an actual requested-owner
observation, not just multiplying a theoretical element width. Depth9 and
the public 10bpp MMR case retain their original allocation totals and peaks.
Descriptor allocation totals/peaks are also unchanged on those MMR controls;
allocator savings cannot explain or waive their adverse timing results.
The [complete allocation receipt](../validation/performance/candidate-allocation-requests-20261125.json.gz)
retains all 387 operations and exact build identities.
The structural narrow source is `e97332c`, with actual fresh whole
**1542/0**, full MSRV tests, CI Clippy/source audit and 129/492 public checks.
Its commit message accidentally says 1539; that stale number is not the machine
receipt. The ordinary hint source was not timed. No timing result for the
structural correction is claimed here.
