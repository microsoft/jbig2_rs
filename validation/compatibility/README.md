# Public compatibility replay

The [manifest](hayro-0.4.json) accounts for reports #191 through #218 and all
32 referenced files. Streams are downloaded only from the pinned public
revision, size/SHA-256 checked, cached outside tracked source and never
executed. Expected packed-image hashes come from the pinned reference PNGs.
No external stream or standards document is redistributed here.

Run the same check as the Linux crate CI lane:

```sh
python3 -B validation/compatibility/run_hayro_corpus.py \
  --output target/compatibility/results.json
```

For an already populated cache, add `--offline`. Rust 1.89.0 is selected by
default; `--toolchain` can select an installed alternative. The temporary
dependency-free consumer uses the ordinary crate API, not hidden validation
hooks or a patched decoder.

Each file is exercised in strict mode through fresh/reusable byte contexts and
the original `Decoder` reader API, and in explicit compatibility mode through
the two byte APIs: 160 API configurations. Every successful
document must match every expected page number, dimension and packed bitmap
hash, and report the required recovery. Negative inputs must remain typed
errors, not crashes, partial documents or blank success.
Context modes decode twice with the same context and compare complete output
or errors, giving 224 decoder calls across the 160 configurations.
The existing reader APIs remain strict. No new opaque reader interface,
assumed I/O contract or unverified compatibility-reader wrapper is introduced.
Compatibility callers acquire bounded bytes before selecting the byte API.

The two nonprogress fixtures use an explicit 100,000-unit work cap; that
bounded failure is distinct from a default-budget wall-clock timeout.
Compatibility preparation debits work before decoding, so its exact remaining
threshold is checked rather than forced to equal strict mode's budget. The
original strict reader and byte APIs retain identical outcomes. Pixel output
and recovery reports must be identical across
the supported API shapes.

Comparator acceptance is not validity evidence. See the complete
[0.3 disposition report](../../docs/compatibility-0.3.md) and
[standalone recovery policy](../../docs/standalone-compatibility.md).
