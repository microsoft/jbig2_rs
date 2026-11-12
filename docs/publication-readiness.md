# Release validation

## 0.2.0 scope

The 0.2.0 release includes the fixed-width Huffman aggregate-dictionary
correction from #173 and stronger selected contracts for symbol IDs, reusable
contexts, decoded metadata, globals and references. Compared with 0.1.2,
application obligations increase from 1,287 to 1,314 and semantic contrasts
from 357 to 494. These are bounded contract and sensitivity results, not
percentages of standards coverage or complete decoded-output proofs.

Rust 1.88.0 remains the runtime MSRV and the crate has no runtime dependencies.
The early-release warning and partial-Verus/conformance limitations below
remain in effect. The [versioned release](https://github.com/microsoft/jbig2_rs/releases/tag/v0.2.0)
records the final source, registry checksum and publication evidence.

## Initial 0.1.0 release evidence

`jbig2-decoder` 0.1.0 is published on
[crates.io](https://crates.io/crates/jbig2-decoder/0.1.0).
Its Rust import is `jbig2_decoder`, its runtime MSRV is Rust 1.88.0, and it has
no normal dependencies.

Versioned archives and validation receipts are available in
[GitHub releases](https://github.com/microsoft/jbig2_rs/releases).
The table below records the initial 0.1.0 release; later patches have their own
source revisions and archive checksums.

This is an early bi-level decoder release with **partial Verus coverage**,
not full JBIG2 or Annex F profile 1 conformance. It is not yet suitable for
untrusted production documents. Colour is unsupported and encoding is deferred.

## Reproducible release identity

| Item | 0.1.0 value |
|---|---|
| Source commit | `ace4fae35afc4e17830d339cc61484e64d2d5870` |
| Tag | [`v0.1.0`](https://github.com/microsoft/jbig2_rs/releases/tag/v0.1.0) |
| Registry archive SHA-256 | `bd5b195921364f39d316fe9b91a9edbd44a836f6fbe9aa300a9b3269e28a444a` |
| Registry archive inventory | 153 files |
| Runtime package/import | `jbig2-decoder` / `jbig2_decoder` |

The registry checksum, downloaded archive, packaged Git identity and runtime,
documentation and license bytes were checked against this source. A fresh
Cargo home fetched `jbig2-decoder = "=0.1.0"` from the registry, then ran the
Rust 1.88 exact-output consumer offline. The check used no local path override
and also confirmed that internal validation hooks are not exported.

Later source or documentation changes do not modify that immutable archive.
The release notes describe version-specific limitations.

## Validation results for 0.1.0

| Check | Result and scope |
|---|---|
| Whole Verus build | 1,287 application obligations, zero errors and a nonempty production library |
| Native function accounting | 642 directly verified declarations and 20 explicit standard-library/callback limitations; no native assumptions, forwarders, divergences or incomplete functions |
| Contract sensitivity | 49 semantic groups / 357 compiling strong/weaker contrasts; 174 detected native mutations |
| Native tests | 514 library/integration tests and one doctest passed; 14 microbenchmarks remained ignored |
| Audit tooling | 632 tests passed |
| Runtime coverage | 11,647 / 12,776 lines and 2,265 / 2,640 branch outcomes; aggregate and per-file floors passed |
| Distribution | Linux, Windows and macOS with MSRV/stable: extracted tests, fresh consumers and compiler safety controls passed |
| Publication rehearsal | Six actual Cargo dry runs completed verification and explicitly aborted upload |
| Generated fuzzing | 65,451,779 executions in the source-bound smoke campaign |
| Official corpus | Two bitmap passes, seven expected strict rejections and one unsupported colour case |

These results describe this release, not every later checkout. Each run binds
its source, tests, tools and raw artifacts. Counts are not a percentage of
normative requirements or proof of every decoded-output property.

## Evidence

The [GitHub release](https://github.com/microsoft/jbig2_rs/releases/tag/v0.1.0)
contains:

- [Release evidence index](https://github.com/microsoft/jbig2_rs/releases/download/v0.1.0/release-evidence.json).
- [Release attestation](https://github.com/microsoft/jbig2_rs/releases/download/v0.1.0/release-attestation.json).
- [Registry and fresh-consumer verification](https://github.com/microsoft/jbig2_rs/releases/download/v0.1.0/registry-verification.json).
- [Archive checksum](https://github.com/microsoft/jbig2_rs/releases/download/v0.1.0/registry-archive.sha256).
- [Fuzz report](https://github.com/microsoft/jbig2_rs/releases/download/v0.1.0/fuzz-report.json).

Retained machine records preserve their original source revisions. Current-head
CI and archive checks are separate from those historical record identities.
An attestation is not a certificate of complete functional verification.

## What remains outside the results

- Complete standard-to-model correspondence, exhaustive requirement enumeration
  and end-to-end output/caller/lifecycle proofs.
- Universal panic freedom, allocator usable-size bounds and a formal bound on
  total process memory.
- Exhaustive coding/template/resource combinations and all failure paths.
- Representative PDF-corpus latency guarantees; hosted timing and RSS
  observations are informational.

See [verification limitations](verifier-limitations.md),
[runtime coverage](runtime-coverage.md), the
[validation guide](../validation/README.md) and the
[development roadmap](open-source-readiness-plan.md).
