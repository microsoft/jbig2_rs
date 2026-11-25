# Performance roadmap evaluation: #225-#240

## Final status (2026-11-25)

All sixteen reported investigations have a final disposition:

- #225/#226 are completed through [#241](https://github.com/microsoft/jbig2_rs/pull/241),
  normally merged as `30a1bd9825556441edf38f84c14d95173fd478d4`.
- #227-#240 are closed **not planned**, with individual evidence and concrete
  reopen criteria. No unqualified prototype is retained. These are decisions
  about the evaluated proposals, not proofs that every future design is futile.
- The original N2 / LLVM24 / PDFium availability blocker is resolved.
  [#242](https://github.com/microsoft/jbig2_rs/pull/242), normally merged as
  `775e57f2f99d1dbad386e51e094f2994f817ceaf`, publishes the portable harness,
  manifests and original qualification receipts. All 2,305 outcomes, 183 reference
  decisions and 88 targeted tests pass/preserve behavior. Nine rounds over 68
  workloads and 2,934 processes have no per-case median regression above 5%;
  the worst is +4.33%. See the [frozen receipt](../tools/pdfium-qualification/runs/pr241-121bee27-n2/receipt.json).

The later independent current-main follow-up covers five variants, 5,724 paired
measurements and 636 complete case/variant summaries. Each misses the unchanged
control gate. Sources, raw summaries, proofs and adverse records remain public:

- [#230: three aligned-composition variants](https://github.com/microsoft/jbig2_rs/blob/4771da8888a698314bebaec72a101560bef780d7/docs/composition-followup-20261124.md).
- [#233: isolated standard-A ownership](https://github.com/microsoft/jbig2_rs/blob/3a4219d3c44c1e78c446a3bca6f0c4fbd3f78ac4/docs/huffman-standard-a-followup-20261124.md).
- [#239: original wide XOR target](https://github.com/microsoft/jbig2_rs/blob/232aec09d44cb70e74c17a7d876aa7a4b27f1630/docs/xor-wide-followup-20261124.md),
  including the explicit template-0 classification erratum. The real 41.5%
  tracked-peak-payload reduction is not RSS and does not waive four adverse
  latency controls.

No additional implementation or repeated confirmation is scheduled for the
closed investigations. Reopening requires a materially different isolated
mechanism or representative workload/phase attribution addressing the recorded
retention failure; speculative alternatives alone are not unfinished deliverables.

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
| #238 | The original 123-case subset had four distinct external-global identities. A later complete 321-request qualification replay finds a three-page standalone source normalized into separate shared-global requests; see below. | Reopened for explicit opt-in ownership; independent private replication motivates, but does not qualify, the new public owner or establish a production PDF reuse trace. |
| #239 | Checked pristine-zero XOR admission, then a vectorizable zero reduction. Both remove one20,000-byte request on six actual targets. Reduction latency remains mixed, with11 adverse control medians; five measured peaks are unchanged. | Do not retain yet; transient bytes removed are not peak/RSS savings. |
| #240 | Actual29,111,339 arithmetic decisions:89.56% require no normalization;79.49% of normalizing decisions shift once. | Keep the shared engine unchanged pending actual cycle/state-traffic evidence. Counts alone do not justify a rewrite. |

Rejecting one implementation does not prove that an entire optimization family
cannot help. The initial evaluation left deferred investigations open; the final
dispositions above now supersede those provisional statuses without presenting
unretained experiments as completed improvements or weakening qualification.

## Private prepared-global experiment (#238, 2026-11-25)

The complete portable qualification replay contains 321 image requests,
18 nonempty globals and 14 exact global identities. The pinned Hayro
`file_pages.jbig2` is a three-page standalone source normalized into three
embedded requests sharing 143 global bytes. `Decoder::decode(original)` already
shares resources across its pages; this normalization is not an observed
production PDF request trace or a full-PDF gain. The original seven PDF global
groups each contained only one unique image, leaving production reuse as a
qualification caveat.
Its source SHA-256 is
`036b0c61419ff2c31c0ee41483d9d539fd79b2a48b55ac127ca75c0a2aabd866`.
The other apparent three-page group,
`lenient_end_of_page_of_previous_page`, is **not** an admitted reuse opportunity:
metadata parsing accepts its global EOP, but actual embedded decoding rejects
`InvalidPageAssociation { segment: 2, page: 0 }`.

Five separately instrumented 1001-sequence runs attribute about 23.96% of the
three-page sequence to global-resource decoding. Removing only its two later
invocations gives a 15.98% optimistic opportunity, **not an observed cache speedup
or hardware-cycle measurement**.

`src/decode/tests/prepared_globals/` provides a private experiment only.
No public API, default, runtime decoder body, proof contract, dependency or
verification configuration changes. The test owner borrows one exact immutable
`Decoder` and `GlobalSegments`; it cannot accept another identity, policy or set
of limits. It admits patterns and symbol resources without retained probability
contexts. Custom tables and other unsupported states use the original path
before page mutation. Page parsing still precedes global preparation on every
call. The ready resources remain immutable; each image receives independently
owned, fallibly allocated packed-bitmap copies. Normal page retention/release
and all existing logical global/page checks retain their original ordering.
A successful original global decode at the borrowed decoder's exact limits is reused, not
replaced by weaker logical work or decoded-byte checks.

There is an **explicit additional 65,536-byte experimental owner/copy allowance**,
capped by the image memory and retained-state limits. Resident vector/bitmap
capacities and simultaneous copy capacities count against this allowance;
allocation/copy failure drops the optional cache and takes the original path
before page mutation. Reported capacity bytes are neither RSS nor allocator
usable size. The allowance is additional to the unchanged image limits, not
zero-cost shared ownership or proof of a combined shipping memory bound.
This is an additional bounded storage tradeoff, **not** a proved
same-total-memory-budget public cache or an observed speedup.
Integrating such an owner into a public aggregate memory contract would require
separate implementation and proof; this experiment deliberately does not do so.

With the already materialized, hash-checked fixtures (never downloaded or
rewritten by this probe):

```sh
export JBIG2_PREPARED_FIXTURE_DIR=/path/to/materialized/globals
cargo test --lib prepared_globals_fixture_controls -- --ignored --nocapture
cargo test --lib prepared_globals_probe_functional_controls -- --ignored --nocapture
cargo +1.98.1 test --release --lib prepared_globals_sequence_probe --no-run
# Use the executable printed by the build; do not invoke compilers during timing.
PROBE=/path/to/target/release/deps/jbig2_decoder-REPORTED_HASH
# Parent-owned quiet timing only; alternate baseline/cold/warm process order.
JBIG2_PREPARED_MODE=baseline JBIG2_PREPARED_ITERATIONS=1001 \
  "$PROBE" --exact decode::tests::prepared_globals::probe::prepared_globals_sequence_probe \
  --ignored --nocapture --test-threads=1
JBIG2_PREPARED_MODE=cold JBIG2_PREPARED_ITERATIONS=1001 \
  "$PROBE" --exact decode::tests::prepared_globals::probe::prepared_globals_sequence_probe \
  --ignored --nocapture --test-threads=1
JBIG2_PREPARED_MODE=warm JBIG2_PREPARED_ITERATIONS=1001 \
  "$PROBE" --exact decode::tests::prepared_globals::probe::prepared_globals_sequence_probe \
  --ignored --nocapture --test-threads=1
```

`cold` includes one original preparation plus three checked physical copies and
all three page decodes per document sequence; `warm` keeps one owner across
sequences after explicit untimed warmup. File reads and parsed-global preparation
are outside all three modes. Timed outputs are consumed and dropped.
The sample vector is preallocated before sampling. Each raw `samples_ns` entry
covers one complete three-page sequence, including cold owner construction,
preparation, copies, all full decodes/output drops and explicit cold-owner
destruction. Sample pushes and uniform counter aggregation occur after the
sample timer stops; assertions and printing occur after all sampling.
`elapsed_ns` is exactly the sum of those samples, not an enclosing wall time.
Per-process medians and tails must be computed from the raw array, not inferred
from the aggregate or a 1001-sequence mean. Warm preparation stays outside warm
samples by definition; cold is the primary comparison. The functional control
executes the same sequence body in all modes without a timer.
Exact candidate/baseline outputs and original standalone pixels are checked outside
timing. Correctness controls also compare all fifteen logical-limit frontiers
and their neighbours, truncations, subsequent reuse and the refused global EOP.
The probe reports elapsed nanoseconds and resident/owner-plus-copy capacity
bytes; it does not itself qualify performance or permit a shipping cache.

Seven focused native controls pass on Rust 1.88.0 and 1.98.1; the actual fixture
and fifteen-limit controls pass on MSRV and in a Rust 1.98.1 release build.
MSRV all-target strict Clippy and the runtime/800-line audit pass.
The original symbol-only subset correctly fell back on the actual document:
its globals also contain a pattern dictionary. The completed experiment copies
both context-free symbols and patterns. Its measured allocation capacities are
22,164 resident bytes and 23,024 simultaneous owner-plus-copy bytes, not a latency
or process-memory result. At that private-experiment revision, all 770 existing
production function files remained source-identical to their source-bound proof
coverage. The private helper itself is test-only and does not acquire a new
Verus or public-memory-contract claim.
Fresh pinned Verus whole-crate verification succeeds with 1,532 obligations,
zero errors, and exact coverage of 750 directly verified functions plus the
unchanged 20 reviewed limitations. It uses Rust 1.98.1, two verification threads,
`--rlimit 30`, and the unchanged 1,050-second guard/settings.
That receipt covers the existing unchanged production proof surface, not a new
proof of this test-only cache experiment. Public design/proof and its eventual
integration remained separate subsequent work.

The later independent hosted replication motivates an explicit opt-in public
owner, now implemented and locally source-validated, with protected integration
and opt-in measurements still pending. Its interface, evidence,
additional-storage tradeoff, proof boundaries and required inventory/ledger
renewal are tracked in [prepared globals on unpublished 0.3](prepared-globals-0.3.md).
The historical private receipts above are not relabelled as public-cache proofs.

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
