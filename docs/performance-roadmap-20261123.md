# Performance roadmap evaluation: #225-#240

## Final 0.3 release disposition

All sixteen assessments are complete. Only #225/#226 were retained through
merged #241. The tested #227-#240 proposals are not adopted; measured target
gains are not denied by that decision. The [complete hosted follow-up](https://github.com/microsoft/jbig2_rs/blob/20abc4dfcee35f9db4e51317a475d89a1bfba48e/docs/performance-hosted-followup-20261125.md)
preserves subsequent target/control results and raw samples.

The [canonical #238 document assessment](https://github.com/microsoft/jbig2_rs/tree/e0f2031ac6c34a35faade027d5d939367fe1c36d/tools/pdfium-qualification/runs/document-prepared-20261125)
resolved the final execution prerequisite: 143 PDFs produced no successful
prepared-resource reuse or material document-level gain. The opt-in component
remains experimental and unmerged, not an API shipped by 0.3. Calibration
limitations and the positive controlled-sequence results remain explicit.

## Historical reassessment status (earlier on 2026-11-25)

The initial blanket closure was too broad. Seven assessments were reopened
to finish implementation and measurement rather than infer value from counts:

- #225/#226 are completed through [#241](https://github.com/microsoft/jbig2_rs/pull/241),
  normally merged as `30a1bd9825556441edf38f84c14d95173fd478d4`.
- #228/#229/#230/#232/#234/#237/#239 retain their earlier measured negative
  dispositions.
- #227/#231/#235/#240 have now received new actual implementation trials and
  nine-round confirmations. Those designs are not retained, with measured
  target gains and adverse controls explicitly reported; this is not a claim
  that every optimization in those families is futile.
- #233/#236/#238 remain open. All-standard descriptors have real target
  latency/allocation gains but unresolved controls; narrowing has real
  allocator savings and a hot-loop correction awaiting timing; bounded global
  reuse has confirmed private-sequence gains on hosted x86 and native N2, but
  production/API/domain qualification remains unfinished.
- The [new reassessment report](performance-reassessment-20261125.md) preserves
  **9,306 native paired measurements / 1,034 summaries**, including every
  adverse case, corrected-source identities and separate private reuse results.
  No new runtime has been adopted by that report.
- The original N2 / LLVM24 / PDFium availability blocker is resolved.
  [#242](https://github.com/microsoft/jbig2_rs/pull/242), normally merged as
  `775e57f2f99d1dbad386e51e094f2994f817ceaf`, publishes the portable harness,
  manifests and original qualification receipts. All 2,305 outcomes, 183 reference
  decisions and 88 targeted tests pass/preserve behavior. Nine rounds over 68
  workloads and 2,934 processes have no per-case median regression above 5%;
  the worst is +4.33%. See the [frozen receipt](../tools/pdfium-qualification/runs/pr241-121bee27-n2/receipt.json).

An earlier independent current-main follow-up covers five variants, 5,724 paired
measurements and 636 complete case/variant summaries. Each misses the unchanged
control gate. Sources, raw summaries, proofs and adverse records remain public:

- [#230: three aligned-composition variants](https://github.com/microsoft/jbig2_rs/blob/4771da8888a698314bebaec72a101560bef780d7/docs/composition-followup-20261124.md).
- [#233: isolated standard-A ownership](https://github.com/microsoft/jbig2_rs/blob/3a4219d3c44c1e78c446a3bca6f0c4fbd3f78ac4/docs/huffman-standard-a-followup-20261124.md).
- [#239: original wide XOR target](https://github.com/microsoft/jbig2_rs/blob/232aec09d44cb70e74c17a7d876aa7a4b27f1630/docs/xor-wide-followup-20261124.md),
  including the explicit template-0 classification erratum. The real 41.5%
  tracked-peak-payload reduction is not RSS and does not waive four adverse
  latency controls.

The historical screens below remain unchanged. Current implementation,
independent-runner investigation and final issue dispositions are tracked in the
reassessment report; they must not be inferred from this earlier closure list.

## Historical screening boundary

The remaining measurements preserve the preliminary evaluation unchanged. Its
issue baseline is `f8774dd4d09bec588f88cd4750334b398030057b`, its then-main
baseline is `3cfff21b3bc157a5908e32199c1e94b2811ad806`, and its proposed runtime
is `f2ded09787e983edf76ed05a7487fabcbedfe728`. Original qualification assets were
unavailable during that screen; their later availability and accepted results
above supersede that blocker, not the historical observations. Local x86 Rust
comparisons are not relabelled as N2 results or a win over C++.

## Experiment discipline

The local cohort contains 116 existing public/generated cases and seven generated
complete symbol documents: 123 inputs, 154 output pages, and 127 latency
case/API configurations including four reusable-context controls. Every normal
binary passes 468 public-API output/metadata/recovery checks. Fixture identities,
probe source, dependency lockfile, compiler identity and binary hashes are frozen.

Both revisions are rebuilt in the same detached worktree, explicitly cleaning the
runtime and probe release packages. Local builds use Rust 1.98.1 / LLVM22.1.8,
O3, CPU-native, ThinLTO and one codegen unit. This is a shared x86-64 host;
affinity to CPU2 is not CPU isolation. No host permissions, frequency settings,
build alignment, allocator or thread budget is changed.

Three preliminary five-round screens contain 6,985, 4,445 and 2,540 paired
measurements, respectively.
Each normal process uses 15 adaptive-batch samples targeting 100ms. The
timed samples follow one validation decode, three additional warmups and one
pilot decode. All five are included in whole-process CPU diagnostics. The
262,144-symbol case uses one first decode per fresh process to bound execution.
Case order is shuffled and before/after order alternates. Every decoded result
is checked against the same frozen packed bytes outside timing. File I/O,
validation and final output destruction are outside the operation timer.

These are **screening results, not adoption confidence**. Adverse records are
retained, including slowdowns in apparently unrelated controls. Confirmation
requires longer nine-round paired runs and investigation of every per-case
median regression above5%. Whole-process CPU diagnostics include startup,
parsing, validation and destruction; they are secondary scheduling evidence,
never a replacement for measured decoder wall time. Hardware performance
counters are unavailable, so software work counts are not reported as cycles.

An identical-binary calibration uses nine paired rounds over six configurations.
Median ratios range from 0.978 to 1.051; the cold many-symbol control has a wide
95% interval of 0.904-1.244. This quantifies measurement uncertainty, not a
performance improvement, and does not excuse adverse candidate measurements.

## Disposition of every report

Ratios below are candidate/baseline decode time from the preliminary screens.
They are workload-specific and do not establish portable speedups.

| Report | Experiment and observed evidence | Preliminary disposition (historical) |
|---|---|---|
| #225 | Carry full bank validity across privately owned arithmetic halftone planes. The direct-call variant gives about0.30/0.36/0.63 on three halftone targets, all five pairs faster. | Proposed with #226; complete control confirmation and external qualification remain required. |
| #226 | Move the existing all-new, identity-order export vector. Many-symbol incremental ratio0.733, all five pairs faster; actual allocation and peak-payload savings below. | Proposed with #225; no general shared-bitmap or sparse-export redesign. |
| #227 | Accumulate template-0 row words locally; exact state/error/pixel tests include widths through3562. The stripe control is1.53x slower in all five pairs and broad target benefit is absent. | Reject this variant; other packing designs are not proved unhelpful. |
| #228 | Rolling refinement windows, then an independently guarded nominal-template variant. Nominal targets improve17-25%, but guarded refagg remains1.379x slower in all five pairs; custom-AT also regresses. | Reject both variants. Geometry guards do not resolve the wider generated-code/control effect. |
| #229 | Owned refinement-bank invariant wrapper. Actual refagg targets make three calls over24,576 bank entries but have ratios near1.003. | Do not retain without material target benefit. |
| #230 | Proved byte-aligned OR/REPLACE composition, retaining the existing fallback. Initial halftone targets are about0.75/0.77; incremental results over #225/#226 are0.85/0.96, with13 adverse control medians. | Unadopted; not bundled to conceal control regressions. |
| #231 | Exact pattern-specific template-0 cursor, including lag129/255 and all fallback geometries. Pattern targets improve, but custom-AT/control regressions persist. | Do not retain the current enum-dispatch variant. A separate kernel remains a different experiment. |
| #232 | Bounded machine-word canonical Huffman fallback. Relevant Huffman target ratios are1.0048/1.0181; unrelated faster cases do not qualify it. | Do not retain this variant. |
| #233 | Actual attribution finds245 standard-table constructions in28 cases, not exclusive CPU time. An A-only descriptor removes129 actual requests/447,200 cumulative bytes over25 hit cases, but relevant latency gains are unconvincing and20 control medians exceed5%. | Do not retain yet; the isolated prototype has component proofs, not performance qualification. |
| #234 | Stable linear canonical Huffman construction. No convincing broad target gain and adverse controls. | Do not retain this variant. |
| #235 | Aligned fixed-width reads; other reads unchanged. No convincing broad target gain. A reservoir would be a separate state/error-sensitive change. | Do not retain this variant; reservoir not justified by these data. |
| #236 | Actual31 halftone grids contain14,841 cells; largest1,763 cells/7,052 index bytes. Potential narrow-index savings total40,773 bytes across the replay, not live memory. | Defer representation/fusion redesign until a real large-grid bandwidth target exists. |
| #237 | Borrow split aggregate codebooks instead of flattening references. Generated128-aggregate ratio0.9521 and66,096 fewer requested bytes; only82 references in six actual public cases, plus adverse controls. | Scaling opportunity, not locally qualified for retention. |
| #238 | Four external-global requests have four distinct exact identities and one use each: zero observed warm-reuse opportunities. | Defer document-cache/API redesign pending representative repeated-document ownership/lifetime traces. |
| #239 | Checked pristine-zero XOR admission, then a vectorizable zero reduction. Both remove one20,000-byte request on six actual targets. Reduction latency remains mixed, with11 adverse control medians; five measured peaks are unchanged. | Do not retain yet; transient bytes removed are not peak/RSS savings. |
| #240 | Actual29,111,339 arithmetic decisions:89.56% require no normalization;79.49% of normalizing decisions shift once. | Keep the shared engine unchanged pending actual cycle/state-traffic evidence. Counts alone do not justify a rewrite. |

Rejecting one implementation does not prove that an entire optimization family
cannot help. The initial evaluation left deferred investigations open; the final
dispositions above now supersede those provisional statuses without presenting
unretained experiments as completed improvements or weakening qualification.

## Actual allocation measurements

These are separate allocation-instrumented builds, never latency binaries.
Numbers are allocation requests and requested payload bytes within the tracked
operation, **not allocator usable sizes or total process RSS**.

| Many-symbol decode | Current main | #225/#226 |
|---|---:|---:|
| Allocation requests | 786,464 | 524,319 |
| Total requested bytes | 31,823,996 | 19,241,084 |
| Peak live tracked payload bytes | 26,933,168 | 14,350,256 |

The input actually decodes262,144 nonempty symbols. Declared counts alone were
not used as allocation measurements. The identity4096 generated case changes
8,246 to4,149 allocations and636,428 to439,820 requested bytes. Empty-symbol
controls are unchanged. Halftone allocation counts/peaks are unchanged.

For #239, both zero-check implementations remove exactly one20,000-byte request
on each of the six identified399x400 cases. Only `bitmap-tpgdon` lowers the
observed peak, from214,620 to204,460 bytes; the other five peaks are unchanged
because a different phase determines the maximum. These observations do not
support claiming a20,000-byte peak or RSS reduction per case.
The originally reported two3562x851 inputs are unavailable; these results are
not extrapolated to them.

The #233 A-only allocation replay covers all123 cases, not just its25 hits.
Its43 constructions remove three requests and10,400 requested bytes each.
The unchanged production/model/dependency closure matches the archived1547/0
whole-proof input across251 files; two later test-only differences are explicitly
recorded rather than relabelling that receipt as a new proof run.

Whole-consumer `.text` is636,782 bytes on current main and636,958 bytes for
#225/#226 (+176 bytes); `.rodata` is unchanged at81,856 bytes. These are native
x86-64 consumer sections, not an ARM result or a debug-containing file-size claim.

## Proposed runtime: correctness and proof scope

- #225 strengthens the private constructor/state predicate to include the whole
  probability bank. The existing validated generic kernel reuses that invariant.
  Arbitrary banks still take the checked entry; skip, prediction, geometry, work,
  failure/reuse states, registers and every bank entry have native comparisons.
- #226 transfers only import-free, all-new identity exports. Every current
  per-payload memory check remains. General imports, sparse selections, duplicates
  and reordered exports retain the original independent-copy path. Removed
  allocations have no phantom reservations or allocation failures; logical work,
  decoded-byte and retained-state charges remain unchanged.
- Fresh unfiltered pinned whole Verus: **1,532 obligations, zero errors**;
  exact **750 directly verified functions and the same20 reviewed limitations**.
  Every reviewed limitation's source-file identity is unchanged. No new assumptions,
  trusted bodies, divergences or incomplete functions.
- Native and expanded verifier inventories reconcile. Rust 1.88 all-feature tests
  pass: 641 native tests plus one doctest, with 16 existing ignored tests.
  Formatting, Clippy, image companion, isolated package/dry-run and unchanged
  aggregate/per-file runtime coverage gates pass.
- All 852 audit-tool tests pass. Historical timing-profile tests retain their
  original membership and cost scopes; new groups do not acquire historical
  timing credit. The original export-order contrast still passes its actual
  native/strong/weaker execution after precise declaration anchoring.
- The prior 32-fixture compatibility corpus passes all 170 configurations /
  238 decoder calls, including complete pixels, recovery receipts, strict reader
  behavior and default-budget nonprogress deadlines. All six RGB oracle calls pass.
- All ten approved original strict conformance outcomes are byte-identical:
  two bitmap passes, seven reviewed refusals and one unsupported profile.
  Historical classifications and normative approvals are not renewed.
- Two new real contrasts compile natively, fail their original strong contracts,
  and pass precisely weaker controls: an invalid returned probability entry and
  an empty result for a nonempty identity export. These are component-contract
  checks, not a physical-allocation/aliasing theorem or full decoded-output proof.
- The complete current semantic inventory is64 groups/505 contrasts, retaining
  all prior63/503. Existing14 shards, roles, concurrency and the1,050-second
  guard are unchanged. Fresh complete protected CI remains mandatory.

The frozen bi-level support matrix, public APIs, strict/Compatibility behavior,
RGB profile, dependency/unsafe policies and release/0.2 are unchanged.
Main remains unpublished 0.3 development; no crate publication is part of this work.
