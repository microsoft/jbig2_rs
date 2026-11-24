# Aligned composition follow-up: experiments not retained

The referenced runtime variants are experimental and **not approved for merge**.
None of the three meets the full local control gate. This report does not retain
them on main. Existing committed main attestations/coverage are not refreshed
as acceptance evidence for these unretained prototypes.

The baseline is main `775e57f2f99d1dbad386e51e094f2994f817ceaf`, including the
qualified #225/#226 changes. Its executing source/model/proof and ordinary Cargo
closure is identical to the existing benchmark baseline
`f2ded09787e983edf76ed05a7487fabcbedfe728`. Historical #241 N2 qualification
does not qualify these additional composition changes.

## Three isolated mechanisms

1. `261da64`: the earlier byte-aligned OR/REPLACE helper, equivalent to
   `3ba22e8`'s executing source. Other operators, shifts, geometry validation,
   zero areas and error framing retain their original paths.
2. `f1540e5`: validate full-row spans once, then use safe slice splitting rather
   than recalculating checked row products and full-storage ranges on every
   iteration. A shared checked span helper proves its endpoints and exact length;
   the OR slice loop proves every byte result.
3. `6e98cc0`: keep only the new private aligned helper out of line. Final-code
   inspection found the general compositor grew from 1,643 to 3,044 bytes when
   the fast path was inlined into it. Outlining reduces that general function to
   1,904 bytes, with a separate 1,247-byte helper. No padding, artificial
   alignment, build randomization or broad arithmetic inlining hint is used.

These are different implementations, not repeated identical-source attempts
substituted to obtain a pass. The original implementation already hoists the
operator choice in generated code; a const-generic rewrite was not invented
as an extra optimization for work the compiler already eliminates.

## Complete paired confirmation

Each variant has one complete nine-round run over all 123 frozen inputs /
127 API configurations: **1,143 paired measurements each, 3,429 total**.
Every process validates exact packed output, dimensions, metadata and recovery
receipts. Each normal record has 31 adaptive-batch samples targeting 350ms;
the large dictionary retains one first decode per fresh process.

Both sides use the same prior same-path clean native O3/ThinLTO probe, compiler,
dependency lockfile, output work and CPU2 affinity. The host is shared x86-64,
not isolated or the original matched N2/LLVM24/PDFium environment. No agent
compiler/proof jobs run during these latency windows.

Ratios are medians of paired after/before process medians, not quotients of
the marginal before/after medians. Intervals use the existing paired percentile
bootstrap; nine pairs do not establish a universal guarantee. Whole-process CPU
diagnostics include startup/validation/destruction and never replace decoder
wall time.

| Variant | Ordinary halftone | Global halftone | Controls above 1.05 |
|---|---:|---:|---|
| Initial | 0.8998, 9/9 faster | 0.9154, 8/9 | white-512 1.0629; Huffman B10/B13 1.0573; identity4096 1.0521 |
| Streamed rows | 0.8557, 8/9 | 0.8566, 9/9 | many-symbol 1.0731; template1 prediction 1.0683 |
| Outlined streamed rows | 0.8716, 8/9 | 0.8394, 9/9 | white-512 1.0568 |

The final white-512 control is faster in only 1/9 pairs, with paired 95% interval
**1.03369-1.09959**. It is not discarded as noise. Its secondary whole-process
CPU ratio is about 1.0063, but that is not decoder-only CPU time and cannot
cancel the primary wall-clock regression.

The [complete machine report](../validation/performance/composition-followup-20261124.json)
retains every adverse row and source/binary/protocol/cohort identity.
The [complete per-case TSV](../validation/performance/composition-followup-20261124.tsv)
includes all 381 case/variant summaries, including paired intervals and tails.
The cold single-decode case has no within-process tail estimate.

## Attribution and correctness

The prior untimed whole-cohort traversal records no eligible aligned-region
request on white-512. Huffman B10/B13 has unaligned REPLACE and XOR region
requests, which remain on the fallback. In contrast, identity4096 has 4,096
eligible one-byte REPLACE requests plus one eligible OR request. They must not
all be described as unadmitted controls. Many-symbol's region widths are 9/3,
and template1 prediction uses the separate clipped-composition path.

These observations distinguish actual optimized-path reach from source-level
guesses. They do not prove a cache/branch/hardware cause for a timing difference.
Code placement and the shared environment remain possible influences, not a
reason to waive the predeclared control rule.

The streamed algorithm passes all existing native/MSRV tests (641 native tests
plus one doctest), the independent bitmap scalar oracles, and Clippy. The
final outlined shape passes the 32 bitmap tests and all 123-case /
468-call public output checks. Fresh unfiltered pinned whole Verus reports
**1,537 verified / zero errors** for both streamed and outlined variants.
All selected aligned-module proofs pass after discharging exact span arithmetic
with existing standard lemmas; no new trusted interface, assumption or lint
exception is added. Contracts cover storage/metadata/zero-area frames, checked
span endpoints and pointwise OR, not a full JBIG2/PDF output theorem.

No ownership, logical work/byte/retained-state quota, policy, reader or entropy
state change is intended. Existing errors and full storage are covered by the
scalar/error oracles. No claim is made that these unretained branches completed
all project semantic/coverage/distribution gates or the original 321-image /
140-PDF N2 qualification.

## Decision

**Do not retain any of these three variants.** The halftone gains are real local
observations, but at least one complete-cohort median exceeds the unchanged 5%
control threshold for every variant. No further composition variants or
unchanged-source pass-seeking reruns are part of this bounded follow-up.
Issue #230 remains open for a different justified design and fresh qualification.
