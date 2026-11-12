# jbig2-decoder

A dependency-free Rust decoder for JBIG2 black-and-white images.

**This is an early release under active development and is not yet suitable
for untrusted production documents.** Verus coverage is partial; implemented
features are not a claim of full JBIG2 or Annex F profile 1 conformance.
Colour decoding is unsupported and encoding is outside the crate's scope.

## Decode a document

The Cargo package is `jbig2-decoder`; the Rust import is `jbig2_decoder`.
The runtime MSRV is **Rust 1.88.0**.

```toml
[dependencies]
jbig2-decoder = "0.2"
```

```rust,no_run
use std::fs::File;
use jbig2_decoder::{Decoder, Limits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let decoder = Decoder::new(Limits::default());
    let mut input = File::open("document.jb2")?;
    let document = decoder.decode_reader(&mut input)?;

    for page in document.pages() {
        let bitmap = page.bitmap();
        println!("page {}: {} x {}", page.number(), bitmap.width(), bitmap.height());
    }
    Ok(())
}
```

Consumers need only ordinary Rust and Cargo, not Verus or the repository's
proof files. Choose application-appropriate `Limits` before accepting input.

## Decoder scope

- Standalone sequential and random-access JBIG2 files.
- Embedded page streams with reusable validated global segments.
- Known-height, unknown-height, multi-page and striped pages.
- Arithmetic decisions, integers and IAID; standard/custom Huffman; MMR.
- Generic/refinement regions, symbol dictionaries, text, patterns and halftones.
- Clipping, combination operators, retention, and owned/reader/page-sink APIs.

The crate decodes JBIG2 segment streams; it does **not** parse PDF containers.
The [support matrix](config/support-matrix.json) describes feature handling
and resource limits. Unsupported syntax, malformed input, resource exhaustion,
allocation failure and I/O failure have distinct typed errors.

### Choose the right API

| Need | Entry point and behavior |
|---|---|
| Inspect structure and dimensions | `inspect` / `inspect_reader` validate the envelope and page lifecycle, not compressed region payloads. Successful inspection does not guarantee successful decoding. |
| Collect decoded pages | `decode` / `decode_reader` return an owned `DecodedDocument`. |
| Receive completed pages | `decode_with_sink` / `decode_reader_with_sink` transfer each `Page` to a `PageSink`; closures can serve as sinks. Earlier callback effects are not rolled back after a later error. |
| Decode embedded streams | `decode_embedded`, or `parse_globals` followed by `decode_embedded_with_globals`. Callers supply the extracted JBIG2 streams. |
| Reuse scratch storage | `Decoder::context` returns a `DecoderContext`; reuse does not retain previous document semantics. |

Reader methods buffer bounded compressed input through EOF before parsing or
delivering pages. A sink avoids collecting all decoded pages, but does not make
compressed input incremental or guarantee constant process memory. Reader
position and sink side effects are not restored on failure. I/O errors,
including `Interrupted`, remain typed errors rather than being silently retried.

Packed pixels use **1 / `true` for black and 0 / `false` for white**.
The first pixel is the most significant bit of its byte; row stride is in
bytes. Read only the declared width, not padding bits. An 8-bit grayscale
adapter must expand the bits and map black to 0 and white to 255.

Decoded pages and sink callbacks use stream order, which need not be page-number
order. `DocumentInfo::pages()` is sorted by page number; match pages using
`Page::number()`, not slice position.

See the [API documentation](https://docs.rs/jbig2-decoder) and
[crate-level guide](src/lib.rs) for more examples.

## How it is validated

Validation combines complementary checks:

- Native tests exercise public APIs, exact pixel/error results, coding
  primitives, malformed inputs and resource boundaries.
- Verus proves selected contracts on the same Rust source compiled by Cargo.
- Compiling strong/weaker contract tests and native mutations check sensitivity
  to deliberately broken results and guards.
- LLVM coverage measures ordinary executing Rust, with aggregate and per-file
  non-regression checks.
- Bounded generated fuzz campaigns retain reproducible failures.
- Distribution CI tests the actual archive and a fresh exact-output consumer
  on Linux, Windows and macOS with MSRV/stable toolchains.
- External conformance and differential tests record exact inputs, bitmap
  outcomes and expected rejections separately.

These are not interchangeable guarantees. A verified storage invariant is not
a complete decoded-output proof; successful tests are not exhaustive input
coverage. Official-case rejections are not bitmap passes.
See the [validation guide](validation/README.md),
[release evidence](docs/publication-readiness.md) and
[threat model](docs/threat-model.md).

## Build and test

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
cargo package --locked
```

For explicit MSRV checks:

```sh
cargo +1.88.0 test --locked --all-targets
cargo +1.88.0 test --locked --doc
cargo +1.88.0 check --locked --release --all-targets
```

Compiler and development-tool pins are in
[`config/toolchain.json`](config/toolchain.json).
The optional [distribution check](docs/distribution.md) uses Python 3.11+:

```sh
python3 -B tools/check_distribution.py --toolchain 1.88.0 --publish-dry-run \
  --output target/distribution/msrv.json
```

It tests the archive outside the checkout, without proof assets or inherited
credentials. The publication rehearsal never uploads a crate.

## Implementation and proof boundary

[`src/`](src) is the single executable implementation. [`specs/`](specs)
contains project models and [`proofs/`](proofs) contains supporting lemmas and
proof bodies attached to that source. Separate proof/model directories are
excluded from the Cargo package; inline ghost code and proof attributes remain
cfg-inactive in ordinary builds.

The crate forbids handwritten unsafe Rust. Compiler-generated code, standard
library internals and the Rust/Verus/vstd toolchains remain trust boundaries.
This is not a binary-wide no-unsafe or universal no-panic proof. The
[proof-only capacity interface](verification/stdlib/STATUS.txt) is an additional
explicit boundary; it supplies no allocator or memory-bound facts.

### Run Verus through Cargo

Put the pinned Verus release and bundled `cargo-verus` on `PATH`, install its
proof Rust toolchain, then run:

```sh
cargo +1.98.1 verus build --manifest-path verification/Cargo.toml --locked -- \
  --num-threads 1 --rlimit 30 --triggers-mode silent
```

This verifies and builds the production library. **Python is not required for
this command.** The proof manifest targets the same `src/lib.rs` as ordinary
Cargo; `vstd` and proof-interface dependencies are not runtime dependencies.

### Interpret the result

A zero-error run establishes the submitted contracts under their preconditions
and trusted interfaces. Obligation counts are not percentages of the JBIG2
standard, and selected-module success does not replace whole-crate verification
and compilation. Some contracts establish bounds or state preservation rather
than complete decoded output.

Start with the [model reading guide](specs/README.md). For detail, see
[verification limitations](docs/verifier-limitations.md),
[statement traceability](docs/spec-traceability.md) and the
[auditor index](docs/verus-auditor-index.md). Remaining coverage work is listed
in the [development roadmap](docs/open-source-readiness-plan.md).

## License and security

Licensed under the [MIT license](LICENSE).
External standards, validation vectors and comparison tools are not distributed
with the crate and have separate terms.

Report vulnerabilities through the channels in [SECURITY.md](SECURITY.md).
