# jbig2-decoder 0.3.0 candidate

This prepares the 0.3 release line; it is not a publication receipt. The crate
remains an early decoder with partial Verus coverage and is not yet suitable
for untrusted production documents. Full JBIG2/Annex F conformance and external
validation readiness are not claimed. Encoding remains outside scope.

## Changes from 0.2.2

### Strict decoding corrections

- Validate the complete generic-region header before scanning an unknown-length
  coding body; adaptive-coordinate bytes are not delimiters (#191).
- Select fresh or prescribed retained dictionary probability contexts from the
  actual flags and references, with retained-model validation (#192).
- Decode one-pattern halftones without initializing nonexistent gray planes,
  preserving geometry and resource checks (#193).
- Construct custom Huffman ranges using the Annex B.2 stop-at-or-above-high
  procedure, without clamping, wrapping or moving the declared upper base (#194).
- Accelerate proved non-progressing dictionary cycles to the same eventual
  work-limit/overflow error, without a new repetition cutoff or relaxed limits
  (#215/#216, implemented through #220).

### Explicit standalone compatibility

The byte-slice `Decoder::decode_with_policy` and context equivalent accept an
explicit `DecodePolicy::Compatibility` and return complete output with recovery
diagnostics. Existing default, reader and sink behavior remains strict;
embedded policy remains independent.

Only the documented bounded exceptions apply: witnessed page-end association
or closure, validated observed page count, exact final CR LF, selected reserved
syntax and independently certified final text-strip termination. No partial
bitmap, guessed resource, cross-segment entropy or implicit strict-failure retry
is returned. See the [complete allowlist](standalone-compatibility.md) and
[per-report dispositions](compatibility-0.3.md).

### Separate generic-region RGB8 output

`Decoder::decode_rgb` and `decode_rgb_with_policy` return separate
`DecodedColourDocument` / `RgbBitmap` values with actual RGB channels.
The [limited profile](generic-colour-0.3.md) covers supported sequential,
known-height generic regions, default colours and supported RGB palettes.
It does not add colour to existing bi-level, reader, context, sink or image
companion APIs, and is not general colour-extension support.

### Retained performance work

The accepted #241 runtime reuses established validity across privately owned
halftone probability banks and transfers all-new identity-order symbol exports
without redundant deep copies. Other resource/export paths retain their checked
behavior and original accounting.

The [original N2/LLVM24/PDFium receipt](../tools/pdfium-qualification/runs/pr241-121bee27-n2/README.md)
records its exact historical source and workloads. Results are not universal
speedup guarantees or fresh measurements of a later release archive.
Previously published 0.2.2 optimizations are not new 0.3 improvements.

The broader performance investigations do not authorize shipping their
prototypes. In particular #238's opt-in prepared-global implementation remains
experimental and unmerged: its controlled repeated-request gains did not
produce useful reuse or material document-level gain in the actual PDFium
assessment. No process-global or document cache is added by this release.

## Compatibility, validation and publication boundary

- The core runtime has no normal dependencies and retains Rust 1.88.
- Packed bi-level pixels remain one for black and zero for white; existing
  typed errors, policies and logical resource limits remain explicit.
- The separate `jbig2-image` companion remains unpublished. Its existing
  single-page bi-level scope is not silently expanded.
- Existing proof, semantic/native sensitivity, coverage, fuzz, output/resource
  and actual-package distribution checks remain mandatory. No proof count is a
  percentage of standard coverage or a complete decoded-output theorem.
- The original 750 directly verified functions and 20 reviewed limitations
  remain the inherited baseline. [Issue #135](https://github.com/microsoft/jbig2_rs/issues/135)
  remains open; release preparation does not approve normative fidelity or
  external-validation readiness.
- The exact final release source, reviewed PR tree, native/Verus results and
  package checksum must be bound by the normal protected release process.
  Until a future explicit publication, there is no 0.3.0 registry checksum or
  published-release claim.

The 0.2 and 0.1 maintenance lines remain independent. Selected fixes require
their own minimal backport, tests and source-bound qualification.
