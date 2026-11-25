# Untimed ordinary-API all321 preservation check

This is one bounded14ae versus d66 functional differential, not a new
optimization, timing run, independent reference or replacement for the
183reference decisions/2305outcomes/full-PDF/LLVM24 qualification.

The supplied `requests-all321.tsv` and materialization receipt are read only.
Every request ID, encoded length/SHA256 and globals length/SHA256 is checked
against the existing frozen qualification cohort. All321 normalized embedded
envelopes are passed unchanged to the ordinary
`decode_embedded_with_policy` API, in both strict and best-effort modes:
**642 outcomes per revision**. No standalone admission, PDF canvas adaptation,
image omission or success-shaped fallback is introduced.

The existing `adapters/image` adapter deliberately exposes only strict,
single-page standalone decoding and maps errors into `image` classifications.
The original PDFium consumer performs renderer/canvas integration and is not
a lossless ordinary Rust error/document serializer. The prepared-owner test
consumer also requires a different API not present at14ae. Therefore this
small standalone serializer uses the accepted ordinary core directly.

## Lossless semantic serialization

`src/main.rs` is source-identical for both builds at one clean task-local
path. It writes canonical, length-prefixed binary frames to a pipe:

* Request ID, explicit policy, optional globals state and parsed segment count.
* Complete typed globals/decode failure, or the complete returned document.
* Document organization, optional declared page count, segment count and all
  page metadata: number, width, height, both resolutions and striped flag.
* Every decoded page's number and complete bitmap width/height/stride/byte
  length, followed by **every returned packed byte**, including row padding.
* Every recovery's segment, page, byte offset and kind, in returned order.

The inspected `DecodedDocument`, `DocumentInfo`, `PageInfo`, `Page`, `Bitmap`
and `EmbeddedRecovery` declarations contain exactly those value fields.
Vector allocation capacity/addresses are not semantic output values.
Organization and recovery tags are matched explicitly. Pixels never use
`Debug`, PNG conversion, shape-only checks or a candidate-generated oracle.

Only the error value uses Rust's derived `Debug`, following an explicit audit
of the entire `DecodeError` closure in accepted `src/error.rs`:
`DecodeError`, `MalformedError`, `UnsupportedFeature`, `LimitExceeded`,
`LimitKind`, `IoErrorKind`, `HuffmanErrorKind`, `MmrErrorKind`,
`HalftoneErrorKind` and `GenericRegionErrorKind`. All use standard
`#[derive(Debug)]`, without custom Debug implementations. Their fields are
integers, bools, static strings and those derived enums/structs; there are no
raw pointers, hidden dynamic I/O errors, floats, hash maps, byte buffers,
callbacks or custom lossy formatters. Default `{:?}` exposes every variant,
field and complete escaped static string; it does not use Display or a
precision/truncation setting. Both revisions use the same Rust1.98.1.
The driver pins the exact audited error source and all relevant public value
schema/getter files for **both** builds. An unreviewed schema change is a tool
failure, not an accepted comparison.

The Python consumer hashes each complete frame and every packed byte while
streaming. No raw third-party pixels are persisted or uploaded. It records
all scalar metadata, error values and per-page packed hashes. After all642
BASE results exist, the baseline expectation file is sealed read-only
**before switching to or building d66**. Only then are candidate frames
collected and compared, with inputs rehashed before/after. A native crash,
truncation, missing frame or mismatch fails; no case is dropped.

## Execute

From a task-local checkout with both exact source commits available:

```sh
python3 -B tools/ordinary-cohort-check/run.py \
  --requests /path/to/materialized/requests-all321.tsv \
  --materialization-receipt /path/to/materialized/complete-report.json
```

The pair is deliberately fixed to accepted
`14aeac451f6491aa3e413fcbc60aafb11fa75b7c` and descriptor candidate
`d66db50bca76c8650b29346592151d81427f8323`. This is not a new general benchmark
framework. No product source/model/manifest, shared fixture, default limit or
CI/proof budget is changed.

Task-local builds, binaries and stderr logs stay under
`target/ordinary-cohort-check/private/`. Only first-party tooling and
metadata/hash receipts are retained; result metadata is under
`target/ordinary-cohort-check/results/`. The `evidence/` directory, when
present, contains the actual completed differential receipts, not an
independent-reference approval. There is no latency clock or timing mode.
