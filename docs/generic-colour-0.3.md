# Generic-region RGB8 profile (main/0.3 development)

`Decoder::decode_rgb(bytes)` and
`Decoder::decode_rgb_with_policy(bytes, DecodePolicy::Compatibility)` return a
separate `DecodedColourDocument`. Existing bi-level decoder, reader, context,
sink and image-companion APIs are unchanged and still reject colour.
There is no publication or release/0.2 backport.

## Output and supported syntax

Each `ColourPage` has its positive source association and an opaque `RgbBitmap`.
Pixels are row-major red/green/blue octets; stride is exactly `3 * width`, with
no padding. Pages remain in page-information order. No partial document is
returned on failure.

This deliberately limited profile supports:

- sequential standalone files, consecutive known-height unstriped pages;
- immediate generic and immediate lossless generic regions, arithmetic or MMR;
- the 32 default colours of T.88 (08/2018), 6.8.5/Table 30;
- referenced 8-bit, three-component RGB/sRGB colour palettes in reference order;
- normal reference retention, without deferred non-retention;
- monochrome OR and REPLACE and coloured REPLACE.

Other region kinds, random-access organization, unknown lengths/heights,
striping, deferred retention, Adobe RGB, extended palette flags and 16/32-bit
palette components remain typed `Unsupported`. Palette counts, references,
lifetimes and IDs are validated; IDs are never clamped. This API is not a claim
of full colour-extension or T.45 coloured-text support, colour management, or
independently approved normative conformance.

The RGB presentation contract interprets monochrome OR as opaque black ink
where the source mask is one, leaving other pixels unchanged. Monochrome
REPLACE writes black or white throughout the clipped source rectangle.
Coloured REPLACE paints the selected foreground where the mask is one and
leaves the background transparent, as specified by 6.2.7. Other monochrome
operators are not assigned speculative RGB semantics.

## Strict validation and Compatibility

Strict RGB decoding requires file-header colour bit 3 (D.4.2) and page colour
bit 7 (7.4.8.5) for every coloured region. Its operator must be REPLACE
(7.4.1.5); the page's override policy is still enforced. The final four bytes
of a fixed-length coloured generic payload are GBFGCOLID and are excluded from
its entropy window.

Compatibility can report only `MissingFileColourFlag` and
`MissingPageColourFlag`, at the original declaration-byte offsets. Source
bytes are immutable; no colour flag, palette ID or entropy data is discarded.
This profile does not inherit the bi-level Compatibility API's other
exceptions. Framing, all region entropy, every page boundary and the declared
page count must remain complete and valid.

The original #212 fixture needs both declaration repairs, at bytes 8 and 40.
Its GBFGCOLID is 4 (red), with a transparent background. Its complete 163x109
RGB output has 53,301 bytes and SHA-256
`fe3ed9ccfa9309b20b5cca0fa612539a6cf3cf14f1782129649ab6f39e1071e4`.
The independently decoded contributing masks and scalar RGB composition are
the [reference](../validation/compatibility/colour-reference.json); the upstream
bi-level honeycomb image is not a colour oracle.
Setting both declarations gives a strict RGB regression with the same pixels.

## Resources and proof scope

RGB page storage debits its observed vector capacity from memory and
retained-state limits; cumulative output counts all three channels. Page fills
and region rendering reserve conservative work before execution. Source-derived
header, palette, page-vector and diagnostic capacities use a prechecked
`64 * input_bytes` scratch reservation and are observed collectively.
Input, segment, page, reference and reference-depth limits remain enforced.
These are abstract decoder budgets, not operating-system RSS limits.

The pinned verifier checks the executing sources. Contracts cover RGB storage,
exact channel writes and transparency/ink presentation, palette wire triples,
foreground IDs, declaration recovery, and complete positive returned pages.
Focused native tests and complete RGB references supplement these component
contracts. Existing reviewed I/O/formatting limitations remain explicit;
there is no new assumed reader or algorithm body. This is not a complete
decoded-output or full standards-conformance proof.
