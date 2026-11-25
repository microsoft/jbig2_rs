# Independent hosted follow-up: actual gains and measured rejection

This follows the [earlier eight-trial report](performance-reassessment-20261125.md),
normally merged through #244 as `e763d7a` after all 34 protected checks passed.
No runtime optimization is adopted by either evidence report.

Four additional full-cohort diagnostics retain **129 inputs / 133
configurations**, nine paired rounds and every earlier adverse control.
They add **4,788 candidate pairs / 532 complete summaries** and **288 separate
identical-baseline calibration pairs**. Every raw sample, process median/p95,
pair order, side journal and paired bootstrap interval was independently
recomputed. All eight calibration intervals pass the predeclared 0.95-1.05
rule in each run. Setup, compilation and correctness checks finish before
timing. Every run completed within its unchanged 20-minute job budget.

These are native Rust 1.98.1 / LLVM22 diagnostics, **not** the original
LLVM24/PDFium gate, a full-PDF gain, independent normative approval, or a
reason to erase earlier shared-host results. Actual CPU/compiler identities
are recorded rather than inferred from runner labels.

## Complete native target/control results

Ratios below are paired medians of process medians, against accepted runtime
`14aeac451f6491aa3e413fcbc60aafb11fa75b7c` within each actual host context.
They are not cross-host absolute-time comparisons.

| Candidate / run | Actual target gains | Adverse medians / disposition |
|---|---|---|
| All 15 standard descriptors `d66db50`, [hosted x86 45627390231](https://github.com/microsoft/jbig2_rs/actions/runs/45627390231), AMD EPYC 9V45 | Ordinary Huffman **0.85231**, refinement aggregation **0.83914**; both 9/9 faster, 95% upper ratios 0.86172/0.85509 | **Zero** medians above +5%, worst +1.80%. Genuine benefit on this context; not universal qualification. |
| Same immutable source, [native N2 45635188596](https://github.com/microsoft/jbig2_rs/actions/runs/45635188596) | Ordinary Huffman **0.82960**, refinement aggregation **0.87321**; both 9/9 faster, upper ratios 0.82999/0.87642 | **30** adverse medians: arithmetic symbols 1.12322, run-code controls 1.15411, TPG **1.32500**, 0/9 faster, 95% 1.31842-1.32715. **Reject this design.** |
| Corrected typed narrow/wide owner `e97332c`, [hosted x86 45628320647](https://github.com/microsoft/jbig2_rs/actions/runs/45628320647) | Actual requested payload decreases by 3,145,728 bytes on million-cell depth1/depth8 grids | **20** adverse medians: depth8 **3.89820**, 0/9 faster, 95% 3.89427-3.92197; depth1 1.76945, wide depth9 1.51373, actual MMR 1.29559. **Reject both tested narrowing designs.** |
| Dedicated pattern kernel `ab8e8be`, [hosted x86 45635182696](https://github.com/microsoft/jbig2_rs/actions/runs/45635182696) | Ordinary halftone **0.63877**, 10bpp **0.53571**, globals **0.63485**, all 9/9 faster with upper intervals below 1 | Original three adverse controls are now flat/faster, but **two predeclared generated MMR controls** regress: depth1 1.07429, 0/9, 95% 1.07405-1.07524; depth0 1.06787. **Existing rejection remains.** |

The [complete 532-row table](../validation/performance/hosted-followup-20261125.tsv)
includes every configuration, not just targets.
The [machine report](../validation/performance/hosted-followup-20261125.json)
binds sources, binaries, actual CPUs, protocols, all adverse rows and audits.
The [compressed raw native records](../validation/performance/hosted-raw-pairs-20261125.jsonl.gz)
retain all 5,076 new candidate/calibration pairs and their original samples.
There is no fabricated within-process tail for the declared single-cold-decode
large-dictionary exception.

### Why the positive x86 descriptor result does not authorize adoption

The descriptor mechanism really improves target latency and allocation work.
It is not dismissed as providing no gain. However, the separately completed
native N2 run has stable, supported-platform regressions with good calibration.
Proof counts, smaller allocation owners and another CPU passing cannot waive
them. The tested implementation is discarded, not the entire optimization
family.

The owned `HuffmanTable` layout did not change. Targeted MMR hot-function
instruction shapes and allocator requests were also unchanged in the earlier
local diagnostic, but those observations do not prove equal referenced data or
an identical whole execution. They do not establish the cause of the timing
differences. An attempted software instruction profile failed on the exact
native SSE4a instruction and yielded zero events; it was rejected as evidence,
not reported as a successful profile.

Likewise, the pattern dispatch delta adds no shared MMR state layout. The
remaining control regressions are not explained merely by observing unchanged
source. No artificial alignment, randomized build padding, dropped scaling
case, blind inline-hint retry or threshold change is used to obtain acceptance.

## Complete ordinary-API preservation for descriptors

The [separate source-bound 321-request check](https://github.com/microsoft/jbig2_rs/blob/f4f5bcd133927f177f37a8f18f16f414251c89ae/tools/ordinary-cohort-check/evidence/receipt.json)
compares accepted main and `d66db50` under both strict and best-effort policy:

- **642 outcomes per revision; zero mismatches.**
- Strict: 275 decoded / 46 typed errors; best-effort: 291 / 30.
- Complete metadata, recoveries, typed errors and **9,480,096 packed bytes**
  per revision, not just dimensions or page counts.
- Baseline expectations are sealed before candidate construction; raw pixels
  are streamed into hashing and never persisted or uploaded.

This preserves an accepted baseline, not a new independent-reference decision
or a rerun of the original 2,305-outcome / 183-reference / 140-PDF qualification.
The earlier fresh **1577/0, 757 direct functions / original 750 retained /
20 unchanged limitations**, native/MSRV and actual allocation receipts remain
separate. Correctness is not a speedup.

## Actual public prepared-owner API, not a relabelled private cache

Production source `6d1bac1` implements an explicitly opted-in, lazy owner bound
to one immutable decoder/global identity and strict policy. Exact fallible
bitmap/resource copies, visible bypasses, reset/transfer lifecycle and
additional-storage checks have actual production contracts:
**1567/0; 769 direct functions, original 750 retained and the same 20 reviewed
limitations**. All 321 frozen requests and 15 limit frontiers have separate
controls. Ordinary APIs and all original declaration bodies remain unchanged.

The new public probe calls that actual API. It does not time the private
experimental cache. Each hosted run has 27 pairs, 54 fresh processes and
54,054 raw three-page sequences:

| Context | Complete cold owner per three-page sequence | Secondary warm repetition | Baseline calibration |
|---|---|---|---|
| [x86 45635207229](https://github.com/microsoft/jbig2_rs/actions/runs/45635207229), AMD EPYC 7763 | **0.87070**, 9/9, 95% 0.85996-0.87746; p95 0.87470 | 0.74767, 9/9 | 0.99899, 95% 0.99649-1.00249 |
| [Native N2 45636382537](https://github.com/microsoft/jbig2_rs/actions/runs/45636382537), actual implementer 0x41 / part 0xd49 | **0.85948**, 9/9, 95% 0.85746-0.86173; p95 0.86186 | 0.75355, 9/9 | 0.99968, 95% 0.99754-1.00092 |

Both independent calibration intervals pass the declared rule. All raw samples,
totals, orders, medians, p95 and bootstrap intervals were independently checked.
The weaker shared-host public result is also retained: cold 0.84708, 8/9,
but calibration 95% 0.96776-1.16592 fails the rule. It is not replaced or
relabeled by the independent runs.

Original raw public reports: [x86](../validation/performance/prepared-public-x86-20261125.json.gz),
[N2](../validation/performance/prepared-public-n2-20261125.json.gz),
[unresolved shared host](../validation/performance/prepared-public-local-20261125.json.gz).

Important boundaries remain:

- The genuine original three-page source is normalized into separate embedded
  API requests. Existing standalone decoding already shares globals. This is
  **not** an existing standalone/default-API improvement, observed production
  PDF reuse trace, full-PDF gain or C++ comparison.
- Each cold sample includes actual construction, global preparation, resource
  copies, every output drop and complete owner destruction. The public info
  snapshot cost is included; assertions/aggregation/printing are outside samples.
- The extra allowance is explicitly **65,536 bytes**. Observed resident
  capacities are 22,164 bytes, not RSS or a same-total-memory-budget claim.
  No private peak-copy counter is fabricated for the public API.
- Warm repetition is diagnostic, not measured production repaint frequency or
  permission for cross-document reuse. Custom tables and retained contexts
  remain explicit bypasses; malformed globals remain typed failures.
- The additional archival PDF has 58 attached JBIG2 masks as well as 116 JPX
  images; the earlier painted-XObject-only traversal missed those masks.
  All 58 have no globals, so they are not a shared-global target.

This is a **positive but unmerged, conditional-domain candidate**. Actual
caller/domain evidence and required complete qualification remain outstanding.
The original native N2/LLVM24/PDFium execution must not be substituted by these
LLVM22 runs or by changing compiler/host/C++ binary identity checks. Source
assets are public; an unavailable execution environment is not a missing-assets
claim. No unqualified runtime, experimental workflow, release or backport is
adopted by this report.
