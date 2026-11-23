# Clean standard-A-only benchmark revision

Base: `3cfff21b3bc157a5908e32199c1e94b2811ad806`.
This revision applies the library, specification, proof and regression-test
portion of the validated A-only patch, without #232/#234/#235, diagnostic
observer code, diagnostic examples or their historical documentation.
No compilation, test/proof run or timing was performed to create this revision.

Only codec-selected standard A uses the sealed immutable descriptor. All other
standards and custom/long tables retain the original backend. Public owned
constructors retain their original allocations and limits. The descriptor
checks all four logical entries, but genuinely removes A's two Vec allocation
sites and one Box site. Its private zero-owned-memory admission is intentional;
it does not waive any other owner, page, stream or work limit.

## Exact scope of the existing proof artifact

The existing A-only whole run reported **1547 verified,0 errors** with
`is-verifying-entire-crate: true`, pinned Rust1.98.1/Verus, rlimit30 and
numthreads2. It belongs to the earlier A-only export, not to a newly executed
verification of this clean commit. It has not been resealed or relabeled.

Proof-time source tree:
`3a2fdb3b1c68fc31c8dc3c61578e54a5bc1e14d3`.
The later validated source export used for the patch is
`749cbd97e7a3c2523effc99bbde53eb80fcaac78`; only historical docs and two
native-only test files changed between those trees (identifier/format fixes).
The retained production, specification and proof inputs are unchanged.

Artifacts remain in the original worktree:

* Receipt: `/workspace/jbig2-perf-huffman-20261123/target/huffman-a-validation/standalone-whole-proof.jsons`
* Diagnostics: `/workspace/jbig2-perf-huffman-20261123/target/huffman-a-validation/standalone-whole-proof.stderr`
* Compiled library: `/workspace/jbig2-perf-huffman-20261123/target/huffman-a-isolated/verus-target/debug/libjbig2_rs_production.rlib`
* Exact proof-source archive: `/workspace/jbig2-perf-huffman-20261123/target/huffman-a-validation/proof-source-3a2fdb3b1c68fc31c8dc3c61578e54a5bc1e14d3.tar`
* Prior production golden qualification: `/workspace/jbig2-perf-huffman-20261123/target/huffman-a-validation/standalone-goldens.json`

Receipt SHA256:
`6d05c45dcf2fd22ee65de1e3739715468b5546e3203c384129c5ccca5e38116d`.
Archive SHA256:
`6661456443cd9eee60a6830a1dfdec2b4ec92627fb1764f0be10f64be99b6de2`.

## Actual standard-A hit cases

These are **observed baseline construction/decode hits**, not a new
descriptor-instrumentation or timing run:25 of the frozen123 cases,43 A
constructions and321 A decode attempts. All25 have both construction and
decode hits; the other98 have neither.

Evidence:
`/workspace/jbig2-perf-huffman-20261123/target/huffman-standard-233/attribution.json`.
Input manifest SHA256:
`5a38ebb41d92617525801c95c159d894c32a5f7b8015bb16283b5264c2422f6f`.

| Case ID | A constructions | A decode attempts |
|---|---:|---:|
| bitmap-symbol-context-reuse-huffman-refagg | 4 | 15 |
| bitmap-symbol-symhuff-texthuff | 1 | 5 |
| bitmap-symbol-symhuff-texthuffB10B13 | 1 | 5 |
| bitmap-symbol-symhuffB5B3-texthuffB7B9B12 | 1 | 5 |
| bitmap-symbol-symhuffrefine-textrefine-export | 3 | 19 |
| bitmap-symbol-symhuffrefine-textrefine | 3 | 15 |
| bitmap-symbol-symhuffrefineone | 2 | 10 |
| bitmap-symbol-symhuffrefineseveral | 2 | 8 |
| bitmap-symbol-symhuffuncompressed-texthuff | 1 | 5 |
| bitmap-symbol-texthuffrefine | 2 | 7 |
| bitmap-symbol-texthuffrefineB15 | 2 | 7 |
| bitmap-symbol-texthuffrefinecustom | 1 | 6 |
| bitmap-symbol-texthuffrefinecustomdims | 2 | 7 |
| bitmap-symbol-texthuffrefinecustompos-global | 2 | 7 |
| bitmap-symbol-texthuffrefinecustompos | 2 | 7 |
| bitmap-symbol-texthuffrefinecustomposdims | 2 | 7 |
| bitmap-symbol-texthuffrefinecustomsize | 1 | 6 |
| bug_880920 | 1 | 7 |
| symbol-identity-1 | 1 | 3 |
| symbol-identity-16 | 1 | 3 |
| symbol-identity-256 | 1 | 3 |
| symbol-identity-4096 | 1 | 3 |
| symbol-aggregate-2 | 2 | 7 |
| symbol-aggregate-16 | 2 | 21 |
| symbol-aggregate-128 | 2 | 133 |
