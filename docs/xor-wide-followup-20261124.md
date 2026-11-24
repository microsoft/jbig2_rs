# Pristine-zero XOR on the original wide input: not retained

The original wide target confirms a substantial physical memory benefit, but
the complete local latency controls do not qualify this implementation for
retention. This experimental branch is not a main-target implementation PR.

Baseline: main `775e57f2f99d1dbad386e51e094f2994f817ceaf`.
Candidate: `82610c93d2f290f7d3bb6d9d24065f0710dc9ded`.
No composition, standard-table, reader or other performance prototype is bundled.

## Exact original input, not two independent entropy samples

The pinned public PDFium templates are:

| Original request | Template | PDF image canvas |
|---|---|---:|
| embedded/cad55441ee2af1dd | testing/resources/bug_674771.in | 400x400 |
| embedded/e434f28150343037 | testing/resources/pixel/bug_1478366.in | 4000x4000 |

Both contain the same 94-byte encoded input, native dimensions **3562x851**,
SHA256 `6f99e5992948d2595bcd907c8226ce0a91b2e341a0dcb369681596f702dd6abd`.
The templates are pinned to public PDFium
`68f43efa71ac66ba843ba286bb76d075aa1c49df`.
The original 123-case native corpus is retained intact, with **one** additional
native-raster task. The two distinct caller canvases remain in provenance;
their different clipping/padding/full-PDF work is not measured or conflated.

The native packed raster is 379,546 bytes with independent published Hayro0.4.0
and MuPDF agreement, SHA256
`e44aad16c4c8385d0b0f7f624ba7660a5db7055ff4ae10a07e9e71e04f422695`.
This is not a substitute for the original matched N2/PDFium 321-image/140-PDF
qualification on a changed runtime.

**Classification erratum:** the raw manifest/pair `family` annotation says
template1, but the unchanged encoded region has generic flags0: **template0**,
no MMR or typical prediction. The decoder reads those actual wire bytes; the
descriptive family string does not select an operation, input, case order or
statistic. The source/golden hashes, decoded pixels and measurements are correct
and preserved. The machine report corrects the classification without rewriting
the raw annotation or input.

## Narrow behavior and component checks

Only full-page arithmetic XOR on the existing pristine/default-zero route is
newly admitted. All previous origin/dimension/stripe/override/work guards remain.
An actual full-storage zero reduction, including padding, is required rather
than trusting the pristine flag alone. Other cases retain the buffered path.

The zero predicate and universal bytewise XOR identity are verified. No unsafe,
new dependency, trust interface or input-derived mutable cache is introduced.
The existing direct-page accounting and page-write path is reused.

Fresh current-main checks pass:

- Unfiltered pinned whole Verus: **1,536 verified / zero errors**.
- Rust 1.88 all-feature: **643 native tests plus one doctest**, 16 existing
  ignored tests. Formatting, Clippy and the unchanged runtime/source-budget
  audit pass.
- Both normal binaries pass all **124 inputs / 472 public-API checks**.
- The wide target additionally passes ten decodes through the same reusable
  context in each binary, with exact output and empty recovery receipts.
  Those diagnostic timings are discarded.
- Native direct/buffered oracles cover full pixels, work, partial bytes,
  actual nonzero storage/padding and default-one/nonpristine fallbacks.

These are component proofs/native checks, not a full decoded-output theorem or
claims that all semantic/coverage/distribution and external qualification gates
were completed for the unretained prototype. Main attestations/coverage are not
relabelled as acceptance of these source changes.

## Actual allocation and peak-payload benefit

Separate allocator-instrumented builds replay all 124 cases. On the original
wide native target:

| Metric | Baseline | Candidate |
|---|---:|---:|
| Allocation requests | 21 | 20 |
| Total requested bytes | 914,852 | 535,306 |
| Peak live tracked payload bytes | 914,228 | 534,682 |

One genuine 379,546-byte temporary disappears; peak tracked payload falls
**41.5%**. These are allocator requests/live payloads, **not process RSS**.
The six earlier 399x400 eligible cases each still remove one 20,000-byte
request, but five of their peaks are unchanged. Per-case peaks are not summed
or inferred from cumulative bytes.

The [complete allocation TSV](../validation/performance/xor-wide-allocations-20261124.tsv)
keeps every unchanged control and observed peak.

## Complete nine-round latency decision

One predeclared run contains all **128 configurations / 1,152 paired
measurements**, nine reversed-order pairs, 31 adaptive samples targeting350ms
per normal record. Every output is checked. The large dictionary remains one
first decode per fresh process; no unfavorable case is warmed or omitted.

Same-path native O3/ThinLTO binaries, compiler, lockfile and output/cache work
are matched. CPU2 affinity is not isolation on this shared x86-64 host. No
compiler/proof jobs run during the latency window. Decoder wall time is primary;
whole-process CPU diagnostics include startup/validation/destruction and are
never substituted for it.

The original wide task has paired median ratio **0.97719**, descriptive paired
95% interval **0.95281-0.99051**, and 8/9 faster pairs: a small local latency
improvement in addition to the much larger memory benefit.

Four complete-cohort median ratios still exceed1.05:

| Control | Ratio | Paired95% interval | Faster pairs |
|---|---:|---|---:|
| Huffman uncompressed symbols/text | 1.068606 | 0.91567-1.15496 | 4/9 |
| Template1 typical prediction | 1.057785 | 0.88526-1.07067 | 4/9 |
| Custom-AT typical prediction | 1.053941 | 0.97538-1.44811 | 2/9 |
| Huffman refagg/context reuse | 1.053081 | 0.96160-1.12673 | 3/9 |

Their uncertainty does not authorize discarding them or waiving the declared
per-case median gate. The [complete latency TSV](../validation/performance/xor-wide-followup-20261124.tsv)
retains all 128 medians/intervals and per-process tail summaries; the cold
single-decode task has no within-process tail estimate. The
[machine report](../validation/performance/xor-wide-followup-20261124.json)
binds all inputs, binaries, protocol and raw data, including all adverse rows.

## Decision

**Do not retain this implementation.** The wide-input memory opportunity is
confirmed, but it does not waive complete latency nonregression or the original
external qualifier. No extra XOR variant or identical-source pass-seeking rerun
is part of this bounded follow-up. Issue #239 remains open; main, release/0.2,
ordinary Cargo settings, publication and the original N2 receipts are unchanged.
