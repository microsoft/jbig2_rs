# Validation guide

The crate is checked through native tests, Verus contracts, source audits,
coverage, generated fuzzing, external conformance/differential tests and
performance/resource measurements. These establish different properties.
None alone proves complete JBIG2 conformance.

The [release results](../docs/publication-readiness.md) describe 0.1.0 evidence.
The [repository layout](../docs/repository-layout.md) identifies source,
retained baselines and generated reports. Validation tools and external assets
are not runtime dependencies or part of the published crate.

## Validation records

| Location | Purpose |
|---|---|
| [Source provenance](source-provenance.json) | Source identities, hashes, versions and external-asset terms |
| `conformance/` | Official manifest, exact expected outcomes and source-bound results |
| `differential/` | External decoder identities and comparison expectations |
| `negative/`, `metamorphic/` | Exact executable Cargo test catalogs |
| `traceability/` | Selected requirements, contracts, models, proofs, tests and gaps |
| `coverage/` | Runtime aggregate/per-file non-regression baseline |
| `performance/` | Generated benchmark scenarios and deterministic expectations |
| `schemas/` | Report and release-attestation structure |

Routine reports go under ignored `target/` directories or CI artifacts.
Source, test, tool and input hashes identify the run they describe; loading an
old successful report is not a new execution.

External standards, vectors, corpora and decoder tools have separate terms.
Keep restricted inputs out of the crate and public failure artifacts. A schema
or hash audit validates identities and structure, not redistribution rights or
semantic correctness.

## Local correctness and audit checks

```sh
cargo test --all-features
python3 -m pip install -r tools/requirements.txt
python3 -B -m unittest discover -s tools/tests -v
cargo package --locked --list | python3 -B tools/audit_package.py
python3 -B tools/check_distribution.py --toolchain 1.88.0 --publish-dry-run \
  --output target/distribution/msrv.json
python3 -B tools/check_test_catalogs.py --discover-tests \
  --output target/traceability/test-catalogs.json
python3 -B tools/check_workflows.py \
  --output target/traceability/workflow-policy.json
```

The package audit requires a complete nonempty real archive inventory.
The distribution checker tests extracted source, a fresh exact-output consumer,
internal-hook isolation and the compiler-enforced unsafe prohibition. It uses
credential-free temporary directories and never uploads, including when the
source manifest permits crates.io. See [distribution](../docs/distribution.md).

Catalog entries name exact Cargo targets and non-ignored test names. Discovery
compiles/lists tests; it does not execute them. Native CI executes them
separately. Missing, duplicate, ignored and wrong-target entries fail checks.

## Official conformance

The [manifest](conformance/itu-t88-2018-manifest.json) pins archive/file hashes
and exact case membership. The corpus is external and is not redistributed
with the crate.

Version 0.1.0 records:

| Outcome | Cases |
|---|---|
| Exact bitmap passes | `F01_200_TT9`, `F01_200_TT10` |
| Expected strict rejection | `codeStreamTest1_TT1`, `TT2`, `TT3`, `TT4`, `TT5`, `TT7`, and `codeStreamTest2_TT6` |
| Unsupported colour profile | `codeStreamTest3_TT8` |

The seven rejections are not bitmap passes or evidence that these files were
intended as negative tests. The recorded diagnostics include EOF during text
or bounded collective-bitmap decoding and negative export-run rejection.
The [case review](conformance/seven-rejection-review.json) contains the detailed
observations and reference-program discrepancies; the
[expectations](conformance/expected-differences.json) pin exact errors,
original streams and relevant source identities.

Reference software can recover inputs, ignore fields or disagree with other
decoders. Matching one reference result does not justify changing wire-format
interpretation, skipping exports or weakening resource/input bounds.

### Run the original corpus

Build the strict adapter, then point the runner at a local copy:

```sh
cargo build --locked --release --example decode_vector
python3 -B validation/conformance/run_itu.py \
  --vectors /path/to/extracted-package \
  --archive /path/to/conformance.zip \
  --decoder target/release/examples/decode_vector \
  --expected-differences validation/conformance/expected-differences.json \
  --output target/conformance/results.json
```

`--vectors` accepts either the extraction parent or
`JBIG2_ConformanceData-A20180829` itself. The decoder timeout is 60 seconds by
default, configurable with `--timeout-seconds`.

The runner checks file membership, sizes/hashes, expected page pairing,
dimensions and every packed bitmap byte. Changed inputs, missing images,
timeouts, crashes, changed diagnostics and unexpected success fail. Reviewed
rejections require exact typed errors and stream/source hashes; they are
counted separately from bitmap passes. Pending diagnoses remain unresolved
even when their recorded error matches.

The normal runner succeeds only when all outcomes match the recorded policy.
`tools/verify_provenance.py --require-exact-conformance` is stricter: it requires
every recorded case to be a bitmap pass and therefore does not pass for 0.1.0.
The ordinary provenance audit checks records without executing the corpus.

### Acquire and isolate external inputs

```sh
python3 -B validation/conformance/fetch_itu.py \
  --package /path/to/official-package.zip --output target/official-vectors
```

`--download` selects the pinned official HTTPS package instead. Extraction
reads only its conformance ZIP member, verifies package/archive/file identities,
rejects links and unexpected members, bounds sizes and stages into a new
directory under `target/`. Reference sources and standard documents are not
extracted by this tool.

The [official workflow](../.github/workflows/conformance.yml) is manual,
main-only and guarded for a private repository. It must not be used to process
restricted vectors in this public repository. Public CI uses generated
regressions and runner/manifest tests; external corpus execution is recorded
separately. Reports contain metadata and outcomes, not copies of restricted
streams or images.

## Differential comparisons

Pinned external tools are listed in
[`differential/oracles.json`](differential/oracles.json). They are validation-only
programs, never decoder dependencies. Expected differences and full-byte/page
comparison matter more than a tool's exit code alone.

An error, warning or recovered output does not establish normative validity.
The optional Hayro wrapper uses a single-image API and FNV64 summaries; those
do not establish full-byte multi-page equality. See
[comparison reproduction](comparison/README.md).

Bitmap-normalizer tests independently exercise palette order, top-down/bottom-up
rows, packed and four-byte padding, and numeric multi-page pairing. Those tests
cover the listed layouts, not universal equivalence between image formats.

## Formal and native sensitivity

The primary Cargo-Verus command needs no Python. To generate the strict
whole-proof report used by audit tools:

```sh
mkdir -p target/verus-production target/verification
cargo +1.98.1 verus build --manifest-path verification/Cargo.toml --locked \
  --target-dir target/verus-cargo -- \
  --num-threads 1 --rlimit 30 --triggers-mode silent --output-json \
  > target/verus-production/cargo-reports.jsons
python3 -B tools/extract_cargo_verus_report.py \
  --input target/verus-production/cargo-reports.jsons \
  --output target/verification/verus-production.json \
  --compiled-library target/verus-cargo/debug/libjbig2_rs_production.rlib
python3 -B tools/audit_production_coverage.py \
  --verus-json target/verification/verus-production.json \
  --output target/verification/production-proof-coverage-ci.json \
  --require-complete --require-exact
python3 -B tools/check_traceability.py \
  --verus-json target/verification/verus-production.json --discover-tests \
  --output target/traceability/combined-report.json
```

Inventory completeness includes explicit limitations; it is not complete
functional proof. Exactness rejects divergent or incomplete implementations.
Tools validate syntax/module/cfg identity and source hashes rather than merely
matching function names.

Semantic contrasts require an exact positive baseline, an ordinarily compiling
mutant, strong proof rejection and success under only the selected weakened
guarantee. Every configured group is required.

The current portable profile uses an additive overlay on the frozen owned-balanced
allocation: only the complete three-case `default-limits-3` group moves from
shard 13 to 4. The preceding owned-balanced correction from shard 12 to 13 is
preserved as historical data, not reinterpreted.
All 64 groups / 505 contrasts remain; the two-case `owned-decode` group stays on
shard 1. Historical colour/certainty and original owned maps retain their
membership and timing interpretation. Fourteen shards, whole/collector roles,
two local workers, at most four concurrent CI semantic jobs and the actual
1,050-second role/shard guard are unchanged. Current-profile collection requires
the new group's complete measured cost; historical timings are not new acceptance.
The runner's `--only owned-decode` selector runs both contrasts through
`check_owned_decode_proofs.py` without case-group or mode flags.

The initial shard-0 assignment was rejected: run `45280486638/1`, checkout
`030ec395517a6e9a9ca8091a6c0a02aa4776165f` (proposal head `12cc1f5`), recorded
1,006.845207815 seconds for `huffman-assignment` plus 68.584887825 seconds for
`owned-decode`, a 1,075.430095640-second lower bound above the unchanged guard.
That first correction moved only the complete new group, with older groups and
roles fixed under its original owned allocation.
Reusing those partial receipts projects source/destination totals of
1,006.845207815 / 880.333832093 seconds. This rejected partial run gives no
acceptance credit. Complete prior-source per-group maxima remain conservative
planning inputs, not current measurements; their raw shard-1 bound is still
1,117.426081810 seconds under that original allocation. Those different-source
colour profiles remain planning only, not current qualification.

The corrected run `45289712820/1`, checkout
`a8fc8d65fe429fa6374483be0cdfd33ece3a6006`, completed all 64 groups / 505
contrasts but was rejected at shard 12's 1,054.647908281 seconds. Its original
correction moved only its smallest timed complete group, `default-limits-3`
(90.951952146 seconds), to shard 13. Reusing the complete rejected profile
projects shard 12 / 13 at 963.695956135 / 829.459011706 seconds and the full
16-role maximum at 1,029.196259829 seconds. A per-group conservative envelope
including every earlier owned-run component has the same prospective maximum,
without setup deductions or discarded adverse timings. These are planning
calculations, not acceptance credit. Qualification still requires a fresh
complete same-run result under the unchanged guard.

The portable-assets run `45375474932/1`, checkout
`f329ffc076846491a711c18befa3db567d7dccc9`, also passed all 64 groups / 505
contrasts, but its canonical profile rejected shard 13 at 1,126.437413592 seconds.
The current overlay moves only the complete `default-limits-3` group
(92.399836174 seconds) from shard 13 to 4. Its measured projection is
1,034.037577418 / 875.938735740 seconds there. A per-group envelope including
the earlier complete rejected profile, every partial owned-run component and
the accepted `45304768601/1` profile projects a maximum of
1,045.988282866 seconds; destination shard 4 projects 1,021.836501295 seconds.
All rejected and accepted observations retain their original identities and
interpretation. These calculations are planning only, not a latency guarantee
or acceptance credit. A fresh complete same-run profile is required; no case,
role, compiler/resource setting, parallelism or guard is changed.

Native mutations separately require an original passing test and failure of
that exact test after a compiling change:

```sh
python3 -B tools/check_contract_mutations.py --toolchain nightly-2026-10-12 \
  --output target/traceability/mutations.json
```

Compile failures, zero-test passes and unrelated failures are not detection.
Temporary isolated workspaces may reuse native incremental compilation, not
verification outcomes. Runtime files in the checkout are not mutated.
See [verification limitations](../docs/verifier-limitations.md).

## Coverage

[Runtime coverage](../docs/runtime-coverage.md) describes the active-code
denominator, LLVM metrics, aggregate/per-file floors and changed-line reports.
Line/region/branch coverage is not proof coverage or complete test adequacy.

## Bounded fuzzing and replay

```sh
python3 -B tools/run_fuzz.py run --profile smoke \
  --corpus-dir target/fuzz-corpus --output-dir target/fuzz-run
python3 -B tools/run_fuzz.py replay --targets document_arbitrary \
  --artifact /path/to/reproducer --output-dir target/fuzz-replay
```

Use fresh output directories and the nightly/cargo-fuzz pins in
[`config/toolchain.json`](../config/toolchain.json).
The nine targets include public-context sequences and generated MMR/Huffman
codec families. Smoke campaigns use 60 seconds per target; extended campaigns
use 300. Per-input bounds include a five-second timeout, 1,024 MiB RSS and
16,384 input bytes, with an additional total runner deadline.

Reports bind source, toolchain, fixtures, raw logs, counters and reproducers.
Missing or duplicate execution counters, target failures, crashes and timeouts
fail validation. Execution counts are not unique-input or source-coverage counts.
Seeds are first-party generated fixtures/constants, not official or private
corpora. Public PRs cannot write trusted corpus caches.

Failure controls inject a generated assertion/input into an isolated copy,
require a reproducer and failing exact replay, then require restoration to pass.
Minimization, promotion rollback and cache serialization are described in the
[corpus lifecycle guide](../docs/fuzz-corpus-lifecycle.md).

### Existing structured target boundaries

- Context programs exercise standalone/embedded inspection/decode, readers,
  sinks, malformed input, resource errors, scratch release and valid exact-pixel
  recovery with bounded MMR fixtures.
- Codec programs exercise MMR pattern/halftone and Huffman symbol/text fixtures,
  with independent scalar pixel expectations, clipping/operators, text strips,
  globals and recovery/error/limit paths.
- Those families do not cover every arithmetic, refinement, aggregation or
  encoded custom-table combination. Direct error construction is not proof
  that every defensive state is reachable through valid decoding.

## Base/head performance

```sh
python3 -B tools/run_performance.py --base-ref origin/main \
  --output-dir target/performance-comparison
python3 -B tools/check_performance_controls.py \
  --output-dir target/performance-controls
python3 -B tools/run_allocation.py --base-ref origin/main --measure-phases \
  --output-dir target/allocation-comparison
```

The same generated fixtures and driver link independently built base/head
release libraries. Paired runs alternate order and validate dimensions,
page numbers, bitmap bytes and exact errors before timing. Decode timing
excludes fixture construction, output hashing and owned-result destruction.
Raw samples retain median/p95 and variability.

The workload is bounded MMR/page/error data, not a representative PDF corpus.
**Hosted timing is informational.** Optional local `--enforce-timing` uses the
configured thresholds and rejects unstable/inconclusive results rather than
calling them a pass. Exact-output and deterministic work-quota regressions
remain failures independently of timing.

`--measure-rss` on supported Linux hosts samples a fresh process's `VmHWM`;
startup, fixture and measurement overhead are included. The isolated allocation
probe records requested payload counts/peaks and can separate decode, validation
and destruction phases. Vector capacity, requested payloads, allocator usable
size and process RSS are not interchangeable memory bounds.

Controls inject actual slowdown, changed output, work charges, RSS or live
allocation inflation and require the corresponding check to detect it.
See [optional comparisons](comparison/README.md) for additional timing limits.

## Workflow contract

The [workflow audit](../tools/check_workflows.py) parses YAML and checks immutable
action pins, permissions, budgets, compiler selections and fail-closed collectors.
Native platform tests execute rather than merely cross-compile.

Proof CI requires whole verification/compilation and every same-source/run/attempt
semantic receipt. Fourteen serial shards have at most four concurrent producers;
whole/collector roles retain their assigned groups. Missing, duplicate, stale,
misrouted, partial or canceled evidence fails aggregation.

Native mutations run in the Linux native lane; LLVM coverage is a separate
required lane. Distribution CI always performs no-upload dry runs. Source-bound
reports and failure diagnostics remain available as Actions artifacts.
