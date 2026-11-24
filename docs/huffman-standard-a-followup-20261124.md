# Immutable standard-A follow-up: not retained

This is an experimental source branch, not an implementation approved for main.
The prototype has real allocation savings, but fails the complete local latency
control gate. It is not bundled with any composition, reader, canonical-order,
machine-word or split-codebook experiment.

Baseline: main `775e57f2f99d1dbad386e51e094f2994f817ceaf`.
Candidate: `4c6f0cc789b4a665d3a5ace9bdd004d46a8aefbc`.
The original qualified #241 runtime is already present in the baseline.

## What is changed and proved

An immutable sealed four-line standard-A descriptor replaces that table's
per-codec owned construction. The general representation remains for other
standards, custom and long codes; public owned constructors remain unchanged.
No mutable global/input-derived cache is introduced.

The descriptor keeps the four-entry logical symbol limit. Actual removed owners
have no phantom reservation or allocation failure. Consequently, that internal
table can genuinely be admitted below the old owned table's memory floor; this
is explicit, not a claim that every resource refusal is byte-identical.

The constructor bridge proves agreement with the existing canonical assignment
and lookup-validity contracts for the literal standard-A lines. The decoder
tracks exact prefix/suffix EOF, result and bit-cursor state; suffix decoding reuses
the original executing line decoder. The final-strip predicate permits an absent
value only at the actual empty suffix, not arbitrary zero padding.

Fresh current-main validation:

- Unfiltered pinned whole Verus: **1,550 verified / zero errors**.
- Rust 1.88 all-feature tests: **646 native tests plus one doctest pass**,
  16 existing ignored tests. Formatting, Clippy and the unchanged runtime/source
  budget audit pass.
- All 123 frozen public/generated inputs pass 468 exact public-API
  pixel/dimension/metadata/recovery checks.
- Prefix/offset/suffix-EOF comparisons exercise both existing owned constructors;
  range overflow preserves the consumed 32-bit suffix. Native tests cover
  four-entry quota and old/new actual memory-owner boundaries.

These are component proofs and finite native oracles. Existing committed main
attestations/coverage are not refreshed as acceptance for this unretained branch.
No claim is made that the prototype passed the complete project semantic,
coverage/distribution or original N2/PDFium qualification.

## Actual memory effects

Separate allocation-instrumented builds replay **all 123 cases**, not just wins.
On the 25 hit cases, 43 actual standard-A constructions avoid **129 allocation
requests and 447,200 cumulative requested bytes**. This confirms the earlier
attribution on the current merged-main runtime rather than reusing an old
pre-#226 comparison.

| Case | Requests before/after | Requested bytes before/after | Tracked peak before/after |
|---|---:|---:|---:|
| Huffman refagg/context reuse | 146 / 134 | 503,864 / 462,264 | 235,980 / 225,580 |
| Ordinary Huffman symbols/text | 62 / 59 | 145,964 / 135,564 | 110,732 / 110,732 |
| Generated identity4096 | 4,149 / 4,146 | 439,820 / 429,420 | 370,308 / 370,308 |
| Generated aggregate128 | 464 / 458 | 321,744 / 300,944 | 187,304 / 176,904 |

These are allocator-requested payloads, not malloc usable size or process RSS.
Removing a temporary does not necessarily lower the phase that determines the
peak. The [complete allocation TSV](../validation/performance/standard-a-allocations-20261124.tsv)
includes unchanged controls and every measured peak.

## Full nine-round latency confirmation

One predeclared confirmation retains all 127 configurations, nine reversed-order
process pairs and 31 adaptive samples targeting 350ms: **1,143 paired
measurements**, every output checked. The large dictionary remains a single
first decode per fresh process. Input I/O, output verification and final
destruction are outside the decoder timer; whole-process CPU diagnostics are
secondary only.

Same-path clean O3/native/ThinLTO binaries, compiler, lockfile, golden inputs,
output work and CPU2 affinity are matched. The machine is shared x86-64, not an
isolated or matched N2/LLVM24/PDFium runner. No compiler/proof work runs during
the latency window, and no adverse input is omitted.

Two per-case median ratios exceed the unchanged 1.05 limit:

| Control | Paired median ratio | Paired 95% interval | Faster pairs |
|---|---:|---|---:|
| generated-white-512 | 1.0782518 | 1.04239-1.09409 | 1/9 |
| bitmap-symbol-manysyms | 1.05021 | 1.00765-1.16688 | 1/9 |

The many-symbol result is not rounded down to pass. The primary Huffman target
ratios are near flat: ordinary 1.0054, refinement 1.0068 and Huffman refagg 1.0014.
Generated identity4096 is 0.9513, but its interval crosses 1 and only 7/9 pairs
are faster. There is no confirmed general latency win.

The adverse controls are not standard-A allocation-hit cases. That observation
does not identify a hardware cause or excuse a measured regression. Natural
code-generation/layout effects and the shared environment are possible
influences, not reasons to substitute CPU diagnostics or discard records.

The [complete latency TSV](../validation/performance/standard-a-followup-20261124.tsv)
contains every target/control median and paired interval. The
[machine report](../validation/performance/standard-a-followup-20261124.json)
binds raw data, source, binaries, protocol and cohort and preserves both adverse
rows. These intervals are descriptive nine-pair bootstrap estimates, not
proof of zero regressions or portable timing guarantees.

## Decision

**Do not retain this A-only implementation.** Real allocation savings do not
waive the latency nonregression requirement. It remains a useful ownership
experiment, not a reason to close the broader all-standard-descriptor issue.
Main, release/0.2, publication settings and the original N2 receipts are unchanged.
