# jbig2-image

Experimental companion [`image`](https://crates.io/crates/image) 0.25 adapter for
[`jbig2-decoder`](https://crates.io/crates/jbig2-decoder). Requires Rust 1.88.
This companion is **not published**. It follows the separate-crate model of
[`jpeg2000-image`](https://github.com/microsoft/jpeg2000_rs/tree/9a568fa615f8bfdfb73f773eda4576ae653bf22f/adapters/image).

The core remains dependency-free and its executable, model and proof sources
are unchanged. This adapter is **not covered by the decoder's Verus proofs**.
The early-release warning still applies: not yet suitable for untrusted
production documents, and not a claim of full JBIG2 or PDF compatibility.

## Use directly

Use path dependencies on this directory and the same `image` 0.25 release:

```toml
[dependencies]
image = { version = "0.25.10", default-features = false }
jbig2-image = { path = "/path/to/jbig2_rs/adapters/image" }
```

```no_run
use image::DynamicImage;
use jbig2_image::Jbig2Decoder;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let input = std::fs::File::open("page.jb2")?;
let image = DynamicImage::from_decoder(Jbig2Decoder::new(input)?)?;
assert_eq!(image.color(), image::ColorType::L8);
# Ok(())
# }
```

The initial scope is **exactly one standalone JBIG2 page**, in sequential or
random-access organization, including bounded, completed stripes. Output is
row-major `L8`: a packed JBIG2 **one bit is black (0)**, a zero bit is white
(255). Row-end padding is discarded. The converted color type is L8; the
default `original_color_type` describes that output, not encoded bit depth.
Multi-page/empty documents are rejected, not silently reduced to a first page.
Embedded streams, `JBIG2Globals`, PDF parsing/canvas composition, best-effort
recovery, animation and encoding are outside this adapter. Use the core APIs
for embedded data or explicit page selection. There is no decoder fallback.

## Register with image

```no_run
use jbig2_image::{register_decoding_hooks, DecoderOptions};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
register_decoding_hooks(DecoderOptions::default())?;
let image = image::ImageReader::open("page.jbig2")?.decode()?;
# Ok(())
# }
```

Hooks enable `.jb2`/`.jbig2`, full eight-byte standalone signature detection,
`ImageReader::with_guessed_format` and `image::load_from_memory`. No changes to
`image` itself are needed. There is no JBIG2 `ImageFormat` variant;
`ImageReader::format()` can be `None` for this external decoder even though
decoding works. `.jbg` (also used for JBIG1) is intentionally not registered.

Call once at startup. Hooks are process-global, first-registration-wins and
cannot be unregistered. Conflicts return the owning extension without replacing
it. Pre-existing conflicts are checked before any registration; concurrent
competing registrations can still leave partial extension registration.
Use direct construction to avoid global state or to select per-image options.
Hook options apply to all inputs loaded through those hooks.

## Limits, allocation and errors

Construction reads compressed input and inspects the full envelope, but defers
entropy/pixel decoding. Inspection success does not imply decoding success.
Configure `DecoderOptions::limits` **before construction** to bound compressed
input, dimensions, references, work and decoder resources:

```no_run
use jbig2_image::{DecoderOptions, Jbig2Decoder};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let mut options = DecoderOptions::default();
options.limits.max_stream_bytes = 16 * 1024 * 1024;
options.limits.max_work_units = 10_000_000;
let decoder = Jbig2Decoder::with_options(std::fs::File::open("page.jb2")?, options)?;
# Ok(())
# }
```

The core's default compressed-input cap is 1 GiB, **not** `image`'s 512 MiB
default allocation budget. The adapter buffers at most the smaller of the codec
input and managed-memory caps (plus a one-byte stack probe to detect overflow),
uses fallible reservation and charges retained compressed capacity against
both its configured core managed-memory cap and any installed `image` budget.
I/O errors propagate, and interrupted reads are retried.

`ImageReader` calls `set_limits` only **after construction**: its limits cannot
retroactively constrain buffering or envelope inspection. Strict image
width/height limits are checked before pixel decoding. In `image` 0.25.10,
`ImageReader::decode` reserves the L8 output before passing the remaining memory
budget to the decoder. The adapter subtracts compressed-buffer capacity and caps
core managed memory by the remainder, without reserving caller-owned output
twice. Core width/height/pixel/work and all other limits remain enforced;
disabling `image` limits never disables codec limits.

Direct `DynamicImage::from_decoder` does not install `ImageReader`'s budget.
For direct use, set codec caps and call `set_limits` with the budget remaining
after your output allocation if needed. Conversion writes straight into that
output; no extra L8 conversion buffer is allocated by the adapter.
As with the core and `image`, memory accounting is best-effort/stage-specific,
not a formal total-memory, allocator-overhead or peak-RSS guarantee. Envelope
metadata, library/allocator overhead and third-party internals are not a process
memory bound; use appropriate external isolation.

Image dimension/memory failures use `ImageError::Limits`; unsupported core
profiles/page counts use `Unsupported`. Other codec failures are `Decoding`
with the original typed `DecodeError` as the source. Reader I/O errors retain
their original `io::Error`. No partial bitmap is reported as successful.

## Build and validate

Like existing isolated helper crates, this companion has its **own workspace
and lockfile**. This differs from JPEG2000's shared root workspace so existing
root commands, proof inventories, decoder lockfiles and proof-free distribution
gates retain their exact scope. Root commands do not build this adapter.

```sh
cargo +1.88.0 test --manifest-path adapters/image/Cargo.toml --all-targets --locked
cargo +1.88.0 test --manifest-path adapters/image/Cargo.toml --doc --locked
cargo +1.88.0 check --manifest-path adapters/image/Cargo.toml --release --all-targets --locked
cargo +1.89.0 fmt --manifest-path adapters/image/Cargo.toml -- --check
cargo +1.89.0 clippy --manifest-path adapters/image/Cargo.toml --all-targets --locked -- -D warnings
```

The dependency requirement covers the published 0.2.1-compatible API and the
unpublished 0.3.0-dev branch. Local builds use the path decoder; packaged
companion tests use registry 0.2.x. Publication remains disabled in both branches.
The original decoder publication gates are neither replaced nor redirected.
Separate CI tests the companion on Linux/macOS/Windows, docs, release compilation
and its packaged registry boundary. Generated first-party fixtures test literal
pixels, packed bit order/padding/polarity, sequential/random/striped inputs,
strict/deferred errors, input probing, image/codec limits and hook conflicts.
