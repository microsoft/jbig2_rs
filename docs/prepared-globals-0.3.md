# Opt-in prepared globals on unpublished 0.3

**Implementation/qualification work in progress.** The new public owner must
receive fresh native/MSRV, exact Verus, contrast and inventory qualification.
The earlier private-experiment proof receipt does not prove this new API.
No release, normative approval, external-readiness or same-memory-budget claim
is made. Ordinary decoder defaults and existing function bodies are unchanged.
The optimization domain is callers already providing **separate embedded
streams with one global owner**, not standalone decoding or full PDF rendering.

## Interface and ownership

```rust,no_run
use jbig2_decoder::{Decoder, PreparedGlobalsState};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let global_bytes: &[u8] = &[];
# let page_bytes: &[u8] = &[];
let decoder = Decoder::default();
let globals = decoder.parse_globals(global_bytes)?;
let mut prepared = decoder.prepared_globals(&globals, 65_536);
assert_eq!(prepared.info().state, PreparedGlobalsState::Pending);
let document = prepared.decode_embedded(page_bytes)?;
let state = prepared.info();
prepared.reset(); // Release owned resources; the next image prepares lazily.
# let _ = (document, state);
# Ok(())
# }
```

`PreparedGlobals` borrows one exact immutable `Decoder` and `GlobalSegments`.
Its strict `decode_embedded` method cannot receive replacement globals, keys,
limits or a recovery policy. There is no process-global cache, cross-document
lookup, eviction map, shared mutable bitmap, or probability-context alias.
Dropping `PreparedGlobals` releases its allocations; `reset` releases them early and
returns to `Pending` without changing the borrowed inputs or allowance.

The measured `file_pages.jbig2` source is genuinely three-page **standalone**
JBIG2, normalized by qualification into separate embedded requests.
`Decoder::decode(original)` already shares its resources across those pages.
The normalized sequence is not an observed production PDF request trace, and
its speedup is not a full-PDF gain. The original seven PDF global groups each
had only one unique image; independent real-PDF evidence of distinct painted
image objects sharing globals remains a qualification caveat.

The constructor is lazy and allocation-free. Every image first performs normal
embedded page/reference validation, then reaches global preparation. A global
EOP that `parse_globals` accepts as metadata still yields
`InvalidPageAssociation { segment: 2, page: 0 }` at actual decoding; it is never
admitted, cached as a success or reclassified as an optimization refusal.

The initial subset admits pattern bitmaps and exported symbol bitmaps **without
retained probability contexts or custom tables**. Original global preparation
must succeed under this owner's exact immutable limits. Each image gets owned,
fallibly allocated copies with the same resource order/IDs and exact bitmap
width, height, stride and packed bytes. The existing clone helper is reused.
Page retention/release consumes those copies normally and cannot mutate the
resident source.

## Explicit extra storage and visible bypasses

The requested additional owner/copy allowance is capped by the decoder's memory
and retained-state limits. This remains **additional bounded storage**, not a
proved same-total-memory-budget public cache or zero-cost ownership. Existing
per-image limits are not enlarged, subtracted from, replaced or disabled.
Successful global validation is reused at those exact limits; the page decoder
retains its original structural-work/global-byte inputs and all original guards.
Copying has no infallible vector clones or hidden retained-context allocations.

Accounting observes actual vector/bitmap payload capacities, including outer
stores and nested bitmap vectors, checks requested copies before allocation,
checks observed capacities after allocation, and reobserves completed copied
stores before page mutation. These are not allocator usable sizes or total
process-memory measurements. An allocator may over-reserve before the runtime
post-allocation check refuses and drops an optional copy, so this is not an
absolute attempted-allocation/process-peak theorem.

`info()` is a counter-free, allocation-free snapshot:

- `additional_storage_bytes`: effective additional owner/copy allowance.
- `resident_bytes`: checked observed payload capacities currently resident.
- `state`: `Pending`, `Ready`, or `Uncached(reason)`.

Concrete bypass reasons distinguish no resources, custom tables, retained
contexts, additional storage refusal, and optional copy allocation failure.
Bypass persists until `reset`. It is not a success-shaped substitute for input
errors: malformed/unsupported data, integer overflow, original quota failures
and required global preparation allocation errors propagate normally.

If optional copying cannot be admitted, all partial copies are dropped and the
original immutable resource owner is transferred into that image's uncached
resource path, before page mutation. Its resident count becomes zero. Later
images prepare ordinary uncached resources. Unsupported first preparations
likewise proceed with their already decoded owned resources rather than retrying
the image. In particular, a cold unsupported/budget bypass does **not** decode
globals a second time for that first image, and an optional clone refusal
consumes the original cache once instead of redecoding globals. There is no retry
or fallback after page mutation.

## New proof and native obligations

The new source is part of the ordinary and pinned-Verus production roots,
not `cfg(test)`. New contracts concern:

- Exact borrowed owner/configuration frames, effective allowance, lazy initial
  state, explicit bypass admission and reset/release state.
- The constructor-established sealed owner invariant: valid bitmap stores,
  context/table exclusion when ready and bounded resident metadata. Image bytes
  acquire no new validity precondition; malformed inputs remain unrestricted
  typed-error inputs.
- Inclusive checked budget arithmetic, overflow versus optional refusal, and
  allocation-only optional error classification.
- Exact fallible bitmap/resource copy contents, dimensions/stride, resource IDs
  and order, preserved source frames and valid owned stores.

The existing reviewed `Vec::capacity` interface supplies runtime observations,
not new allocator equality/lower-bound axioms. No additional trusted interface,
proof exclusion, runtime/proof substitution or old precondition is added.
These are project ownership/copy/lifecycle results, **not** a complete decoded
stream/raster theorem or independent interpretation of T.88.

Native controls include exact public/private/baseline outputs for the actual
three-page source, repeated reuse, truncations, refusal priority, reset,
context/table bypass, partial-copy cleanup and all fifteen original logical-limit
frontiers and neighbours. New selective local contrasts target discarded
constructor allowance, retained reset state, rejected budget equality, one
corrupted copied pixel, a changed symbol ID, suppressed internal overflow and
modification of the consumed original on the uncached-transfer path.
Each contrast keeps failed source/diagnostics and requires a native failure,
a strong semantic rejection and a passing selectively weaker control.

The ignored `prepared_globals_external_request_controls` test accepts a
`JBIG2_PREPARED_REQUEST_LIST` TSV of `id`, image-stream path and global-stream
path (`-` means no globals). Paths are absolute or relative to the list.
The caller supplies the already hash-checked frozen normalized streams; the
test never normalizes, downloads or edits them. Its required count defaults to
321 (`JBIG2_PREPARED_EXPECTED_REQUESTS` can select an explicitly different
control list). It compares complete outputs or exact typed errors at additional
allowances zero/65,536, twice each, with separate owners per request. Unchanged
global-metadata parse refusals are counted before owner construction. The
three-page fixture control separately exercises the real repeated-document
ownership sequence; no identity/key cache is inferred from this replay.

`prepared_globals_external_cold_controls` uses the same TSV format through
`JBIG2_PREPARED_COLD_REQUEST_LIST`, requires exactly one request and performs only
one decode per newly created owner at each allowance. It is the no-timer
single-use control interface, not a warm target. The predeclared real-PDF
negative is pikepdf's MPL-2.0 `jbig2global.pdf`, blob
`bea83752185a462951e459afc6a3c98654d69da9`, materialized separately as
`pikepdf-jbig2global.pdf`: one painted image object 17, 4000×2864, and 47,864
global bytes. Supply its extracted unchanged streams after input/hash review;
the test does not download, rewrite, redistribute or pretend to time the PDF.
Any future latency comparison must include cold preparation and visible bypass
reasons, not fabricate repeated requests for that object.

## Qualification and integration still required

Do not run compilers during a parent-owned quiet timing window. After explicit
authorization:

1. Run focused public-owner and actual-fixture controls, then all native/MSRV,
   formatting, strict lint, documentation and distribution checks.
2. Run fresh pinned whole-crate verification with Rust 1.98.1, two threads,
   `--rlimit 30` and the existing 1,050-second guard. Retain the original 750
   directly verified functions and 20 reviewed limitations; report actual new
   coverage separately. No historical 1532/750/20 receipt is a cache theorem.
3. Execute `tools/assess_prepared_globals.py` with the pinned executable
   and a fresh `target/` output directory. Its preserved local receipts do not
   silently expand historical semantic-profile membership, costs or CI scope.
4. Renew source-bound coverage and the existing 20 reviewed dispositions only
   after confirming unchanged reviewed bodies. The module/reexport-only changes
   affect `src/decode.rs` and `src/document.rs` hashes; the reused bitmap helper
   changes crate-internal visibility, not its body or existing precondition.
5. Regenerate actual compiler and proof-mode inventories, then replay
   `check_internal_profile.py` using those compiler artifacts. The new
   `PreparedGlobals*` declarations and methods must enter the derived API
   inventory; do not hand-edit an accepted public-name list or approved support
   rows to conceal them.
6. Renew `validation/traceability/requirements.json` source bindings and register
   `specs/prepared.rs` as a project ownership/copy/resource policy family, linked
   to actual contracts, exact native tests and contrast receipts. This adds no
   normative requirement, independent approval or readiness grant. Refresh
   source-review/provenance packets through their existing reviewed process,
   rather than flipping approval/fidelity/readiness flags.

Parent owns complete protected CI, any profile/ledger review, public opt-in
measurement and final disposition. No experimental workflow override belongs
in the product.

## Evidence motivating, not qualifying, the public owner

The private prototype's independent hosted run
[45608190010, attempt 1](https://github.com/microsoft/jbig2_rs/actions/runs/45608190010/attempts/1)
at `f6aba19` passed the existing performance steps and preregistered replication/
calibration. Primary cold paired ratio: 0.828137, 9/9 faster, 95% interval
0.826068–0.831016, p95 ratio 0.833380. Secondary warm ratio: 0.687912, 9/9 faster,
interval 0.685382–0.689933, p95 ratio 0.701974. Baseline/baseline calibration:
0.997722, interval 0.995134–1.006449, within the preregistered ±5%.

These are independent native x86 private-prototype observations, not N2,
PDFium, public-owner results, an observed PDF reuse trace, full-PDF gains or full
shipping qualification. The original
shared-host weak-confidence result remains part of the record. The earlier
15.98% after-first phase opportunity is attribution, not a measured cache gain.
