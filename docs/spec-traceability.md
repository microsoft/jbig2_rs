# T.88/T.6 statement traceability

The [requirements ledger](../validation/traceability/requirements.json) connects
selected standard requirements with runtime contracts, project models, proof
support and exact executable tests. The selected denominator and functional
coverage are partial; this is not a full-standard conformance map.

The 0.1 release uses a partial-Verus scope. The current 0.2 milestone is
consistent code-contract coverage and complete statement representation for
external validation, tracked in [#135](https://github.com/microsoft/jbig2_rs/issues/135).
Deeper full functional proofs remain longer-term work, not a blanket
prerequisite for that handoff.
See [release validation](publication-readiness.md) for measured results.

## Internal preparation versus validator execution

Internal preparation uses the approved broad bi-level decoder scope in the
[support matrix](../config/support-matrix.json), T.88 (08/2018) /
ISO/IEC 14492:2019 Edition 2 and the distinct T.6 (1988) MMR reference.
Standalone sequential/random and embedded/global APIs remain in scope;
the existing encoder, colour and container exclusions are not expanded to
improve coverage. The actual native root API and compiler profile come from
the source-bound compiler export, not a hand-edited list of callable names.

Internal completion requires native/Verus/generated identity reconciliation,
explicit caller-stage or admitted-input domains, consistent meaningful
contracts and the complete applicable statement-to-obligation-to-consumer
matrix. Missing local syntax alone does not identify an absent trait contract;
line coverage and verified obligation counts do not establish those criteria.
Approved source-statement processing or an original rights-cleared inventory
is needed for exhaustive enumeration. That dependency is distinct from
selecting, invoking and adapting inputs to an external validator.

Generic reproducibility evidence can be prepared before a validator is chosen.
Validator identity/version/input adaptation, execution and adjudication are
separate work. The stronger full external-readiness and independent-review
claims remain false until their own requirements are met; internal preparation
does not require every deeper end-to-end/raster theorem.

## Records and scope

- [`config/support-matrix.json`](../config/support-matrix.json) defines supported,
  unsupported and deferred behavior, including resource policy.
- [`source-provenance.json`](../validation/source-provenance.json) identifies
  T.88 (08/2018), edition 2.0, and the distinct T.6 (1988) MMR reference.
  Their document identities are not interchangeable.
- [`production-proof-coverage.json`](../verification/production-proof-coverage.json)
  accounts for qualified native declarations; it is not normative coverage.
- Each requirement records a stable ID, locator, original paraphrase, runtime
  symbols, models/proofs, actual contract evidence, exact tests, assumptions,
  gaps and source identities.

Project resource limits and typed Rust error policy are separate from normative
rules. A policy row does not increase the standard-requirement count.
The crate decodes bi-level JBIG2 streams, not PDF containers or colour images.

## What an evidence label means

| Evidence | Establishes | Does not establish |
|---|---|---|
| Safety/bounds | Valid storage, ranges and checked arithmetic under stated preconditions | Intended decoded output or every rejection condition |
| State preservation | Selected invariants and unchanged-state frames | A transformation merely because state stays valid |
| Termination | A local decreasing measure, conditional on callees terminating | A global CPU/service-availability bound |
| Functional project contract | The stated relation between inputs/state and results | Independent fidelity of the model to the standard |
| Runtime test | Executed assertions on particular inputs | Universal proof or exhaustive coverage |
| Strong/weaker contrast | A compiling fault is rejected by a selected guarantee and accepted when only that guarantee is removed | Full correctness of the model or decoder |
| Reference/hash audit | Exact definitions, identities and declared links | Semantic correctness, test execution or adequate test assertions |

Functional evidence comes from actual production `ensures` clauses and proved
support. A model's name, a precondition, `true`, or a same-named unrelated
function is not a result guarantee.

## Models and their limits

### Actual compiled native inventory

The [compiler collector](../tools/collect_compiler_inventory.py) uses the already
pinned coverage/fuzz nightly in an isolated offline native-library build. It
exports private/hidden compiler items, source-spanned MIR and a nonempty compiled
library. The [reconciler](../tools/compiler_inventory.py) checks every explicit
source declaration, source derive, executable body, closure and ADT constructor.
Copy/equality/internal derive marker implementations and duplicate CTFE bodies
remain visible but are not counted as additional executable functions.

The Linux CI artifact contains the original exports, toolchain/command identities,
source/tool hashes and a replayable report. Recheck it without rebuilding:

```sh
python -B tools/collect_compiler_inventory.py \
  --output-dir target/traceability/compiler-inventory --check-existing
```

The report's `api_declaration_surfaces` classifies every owned native callable
or abstract identity against the actual root reexports: reexported free
functions, public inherent methods, trait callbacks, trait implementations,
ADT constructors, other functions/methods and internal closures. Root aliases
and exact compiler owners are retained. Exposed function records also retain
the compiler signature, generic bounds and function header. Native-cfg source
reexports must match the compiler export list; omitted exports, new public
declaration forms and unsupported glob forms require explicit review.

A public method on a private type is not thereby a root declaration. A tuple
struct's exported name does not expose a constructor with private fields.
Conversely, an enum variant constructor or trait implementation may be exposed
without having publicly constructible arguments. Closures remain internal
bodies even beneath exported functions; their enclosing declaration category
does not assert runtime reachability.

These are declaration-surface classifications, not admissible input domains
or a public call graph. Private helpers may execute under public callers.
Public data fields, associated constants, external blanket implementations and
other platform/proof profiles still need separate inventory/domain review.
Both reachability and constructible-domain qualification flags remain false.

This complements the conservative explicit-source inventory; it does not replace
it with an obligation count. Native compiler ownership is not proof-mode identity,
semantic contract quality, a public call graph or an admissible stage domain.
Those qualifications remain explicit open obligations. No readiness or coverage
percentage follows from this compiler-profile reconciliation.

Five previously derived non-`Copy` clones now have explicit, checked result
contracts: decoder configuration, generic/refinement headers, retention bytes
and complete segment-header metadata. Closed associated logical views preserve
private runtime fields. These contracts retain the original value/Vec-cloning
behavior and make no new capacity, allocation-limit or normative claims.
The [clone controls](../tools/check_clone_result_proofs.py) exercise actual
compiling faults against the owning guarantees and their weaker controls.
Header tests also cover internal metadata states, not public compressed-image
witnesses. New explicit source declarations are kept distinct from formerly
compiler-derived methods; moving a clone into source is not an increase in the
number of native callable bodies.

Public reusable-context contracts additionally preserve the governing limits,
describe fresh empty scratch, copy the stored retained-byte observation, and
prove reset/release/strict-threshold cleanup. All seven existing inspection,
global-parsing and decoding return paths now carry the cleanup frame on success
and error, preserving their earlier metadata guarantees. The
[state/return models](../specs/document/context.rs) are separately classified
project API helpers without a fabricated normative requirement link.
Stored-byte metadata is not a new proof of vector capacity or process memory;
native capacity observations remain distinct tests.

All eight owning standalone/embedded decode, helper and context result paths
also retain the existing header/envelope `DocumentInfo` guarantees. Context
cleanup and configuration frames remain intact. This is not a proof that the
decoded page/raster list corresponds to all document metadata or source bits.
The existing document-count record traces these actual consumers without
claiming a new normative statement or completed normative review.

Both global-parser APIs prove exact owned input bytes and successful
stream/retention admission, with the exact first stream-quota error. The
[global result relation](../specs/document/results.rs) is separately classified
project API support, not a normative requirement. Global native positives are
accepted metadata containers, not entropy-valid dictionary witnesses. Real
compiling [result contrasts](../tools/check_public_result_proofs.py) preserve
existing context frames; quota-bypass controls also remove the now-invalid
new quota-preservation invariants, so rejection comes from the actual result
guarantee rather than an unrelated loop-proof failure.
The original hosted profile rejected an existing shard at 1,051.79 seconds;
its framing group was moved from shard 12 to existing shard 13 using the same
run's measurements. Every case and the 14-shard/16-role/max-four topology and
1,050-second guards remain unchanged; the failed profile grants no acceptance.
The next original run verified the whole crate but its two contrast groups
finished at the unchanged 20-minute job cancellation. Their 1,047.40 seconds
plus setup and whole verification exceeded that job's budget. The existing
ten-case context group therefore moves from the whole role to shard 7;
the whole proof itself, every query and all job/measurement caps remain.

### Reference catalogs and project work

The concrete catalog baseline counts actual `ParsedSegment` referred-entry
lengths and proves exact checked work-quota outcomes. Its prefix fold has a
different input domain from the existing abstract `Segment` reference/work
models; no native-to-abstract correspondence is credited without a bridge.
Lookups retain first-match values and exact absence, including duplicate keys.

Successful reference validation returns the input-order type/number catalog
and equally sized depth metadata with positive, admitted new depths.
Ordered depth processing preserves arbitrary incoming prefixes on success and
error, and describes bounded partial appends without requiring the incoming
prefix itself to satisfy the new-depth limit. Existing index-range
preconditions remain unchanged. Both global parsers retain matching type/depth
counts in addition to their earlier owned-byte and quota/error guarantees.

These are structural value/state/accounting guarantees, not complete graph
depth, reference-class, retention, raster or normative-fidelity proofs.
The [catalog contrasts](../tools/check_reference_catalog_proofs.py) and wide
scalar native oracles keep these domains explicit; internal length metadata
and seeded prefixes are not public compressed-image witnesses. An affected
window-composition proof uses checked per-window/header-length projections
instead of globally expanding adjacency quantifiers. Its original guarantees,
preconditions and verifier resource limits are retained.
The first hosted catalog profile passed all individual queries but rejected
an existing shard at 1,103.60 seconds. Its complete original measurements
and two further complete rejected profiles drive conservative per-group maxima.
Seven existing groups (`mmr-span`, `retention`, `default-limits-3`,
`collector-sink`, `arithmetic-initialization`, `default-decoder-4`,
`default-decoder-3`) move into
existing headroom. No case, producer, pin, concurrency or measurement/job
limit is removed or raised. Combining timings predicts capacity only; neither
rejected run is acceptance evidence.

### Source-only purpose inventory

The [concrete reference-type models](../specs/document/reference_types.rs)
describe first-local/global lookup, intermediate kinds, symbol/table limits
and exact type-stage acceptance/error results. They preserve all existing
index-range preconditions and do not introduce public field access or trust.
Missing symbol/text references are deliberately ignored by this classifier;
the preceding link validator rejects them. Treating the classifier alone as
complete reference validity would misstate its admitted stage domain.

Independent native oracles cover all 22 owner kinds against every resolved
kind and selected reference counts, first duplicate-key matches, local/global
precedence and exact owner-number diagnostics. Twelve real compiling
[type contrasts](../tools/check_reference_type_proofs.py) remove only the
selected owning result guarantee; table/class faults also remove invalid
new preservation invariants while keeping existing domains. These are
implementation-stage results, not independent normative approval or a proved
bridge to the distinct abstract reference-class model.
The first complete hosted profile passed all individual groups but rejected
shard 11 at 1,083.08 seconds. Only the new class group moves to existing shard
13; maxima with the accepted baseline fit the unchanged guards. The original
failed profile is preserved and gives no acceptance credit.

`model_classifications` identifies exact declaration families as project
results, policies, representation invariants, proof support or candidate
standard-facing models. These categories describe purpose, not proof strength.
The bounded inventory includes registered definitions and spec functions in
`specs/` and `proofs/model_support/`; other helpers are not silently counted.

Registry and project-source digests bind exact declaration/module identities
and all Rust source under `src/`, `specs/` and `proofs/`. Missing, duplicate,
renamed, ambiguous or stale declarations fail checks. Reports expose unresolved
purpose ambiguity separately from known partial scope or fidelity caveats.

`Model navigation`, `Audit` and `REQUIREMENT CONTEXT` comments are local reading
aids. They do not prove the cited standard translation. Imported implementation
tables/constants remain dependencies to check independently.

### Arithmetic, integer and IAID

Project contracts cover initialization, BYTEIN, normalization and selected
decision/register/context results. Integer and IAID relations compose decision
history, sign/length/value or OOB and context state. Codec adapters preserve
selected/unselected banks and exact project errors.

Runtime probability arrays are model dependencies, not independent evidence
of standard-table fidelity. Procedure-bank placement, field selection, reset
timing and full caller/stream interpretation require their own guarantees.
The IAID 31-bit cap is project policy.

Local generic/refinement framing contracts describe the existing strict policy:
bad or short tails take priority over scanning; interior marker rejection reports
the first matching pair and its exact code/offset. Session queries return stored
decoder metadata, and final-marker results compose observed-marker validation
with checked length subtraction. Successful session construction preserves the
supplied bytes, configuration, limits and zero work counter.
The [framing models](../specs/generic_region/framing.rs) are project-result
relations, not independent approval of every normative framing exception.
In particular, an unobserved marker is accepted by the observation helper even
for a short internal coded length; final session validation still rejects that
length. The [native oracles](../src/generic_region/tests/segment_framing.rs)
exercise all byte pairs and arbitrary stored metadata without treating those
internal states as public compressed-image witnesses. Full driver/raster
composition and exhaustive source-statement coverage remain unfinished.

### Readers, bitmap and raster primitives

Byte/bit readers have exact input/cursor/value/error frames. Packed copying,
fill/padding, spans and Boolean operators describe contents and untouched
frames, including partial bytes. A white-span no-op does not prove arbitrary
old pixels were white.

Helper results do not alone prove template/prediction timing, edge behavior,
clipping, placement or complete generic/refinement/page composition.
Standard slice operations and the compiler/library remain trust boundaries.

### MMR/T.6

Mode/run lookup binds consumed input bits and lengths to implementation tables.
`complete_run` additionally describes prefix extent, make-up/terminating
structure and sums; `horizontal_pair` composes opposite-colour runs.
Reference models describe changing-element parity, initial-position equality
and width-sentinel fallback.

Previous-row pixel/cache relations and finite raster histories connect input,
mode/run/reference state and visible rows. Required framing describes immediate
EOFB and zero alignment bits.

Independent table/coordinate fidelity, complete acceptance/errors, applicable
T.88 framing exceptions and full caller/resource composition remain separate.
Tests sharing implementation arrays are not independent table oracles.

### Containers, metadata and lifecycle

Sequential and random-access models connect original header/payload windows,
offsets, ordering and complete accepted body extents. Embedded relations preserve
supplied order without importing standalone ordering rules.

Typed metadata relates accepted dimensions, height encoding, resolutions,
flags and profile IDs to original bytes. Opaque entropy/table payload metadata
is not a codec-validity theorem.

Page-state and retention field results do not complete unknown-height/stripe
lifecycle, reference resolution, lifetime/discard or decoded-document semantics.
The retention model uses forward wire-byte order, least-significant bit first
within a byte; the conflicting worked example is not an alternate-order
recovery heuristic.

### Huffman, symbols/text and halftone

Canonical assignment and cached/fallback lookup have operational project result
contracts. They do not independently establish predefined-table translation,
encoded caller selection or whole-stream conformance.

Symbol selection/export relations bind geometry, stride, packed bytes and
order. They do not alone identify encoded export flags or prove complete
refinement/aggregation/text output. Pattern/halftone header contracts bind raw
fields and parser extent, not the complete grid/raster/caller interpretation.

## Checker contract

`tools/check_traceability.py` checks schema/IDs, exact feature membership,
standard identities, source/module hashes, runtime proof accounting,
qualified model/proof symbols, proof-macro wiring, actual contract sections
and exact non-ignored Cargo test references.

It rejects unsupported or ambiguous bindings, vacuous/commented-out evidence,
unused definitions and claims stronger than their recorded evidence.
`--discover-tests` compiles/lists the recorded Cargo targets; native CI separately
executes tests.

`decomposition_review_complete` describes a reviewed exhaustive map that may
still contain unproved transformations. `semantic_claims_complete` additionally
requires functional evidence and no semantic gaps. The corresponding
`--require-reviewed-decomposition` and `--require-complete` gates remain
distinct from the limited 0.1 release scope.

### Readiness gap assessment

The JSON report's `validation_readiness` section joins the existing cfg/module
source inventory to local attached and direct legacy postcondition syntax,
verifier-name matches and actual ledger/evidence owners. It reports per-module
and per-declaration identities, source hashes and gaps. Nested closure contracts
are not credited to the enclosing function; comments and result-string contents
cannot manufacture contract-section markers. Shared verifier names remain an
explicit disambiguation queue rather than separate qualified proof matches.

This first assessment counts selected explicit production declarations, not
every generated/derived item, macro expansion or closure. Local syntax does not
establish contract meaning, trait ownership, caller reachability or admissible
input domains. Direct ledger links do not measure transitive caller coverage.
These are review queues, not automatically missing proofs or decoder defects.

Selected ledger records and partial feature worklists are not an exhaustive
statement inventory. The report therefore leaves statement/code qualification
and readiness false, and percentage/unknown statement denominator fields null.
Even a declared reviewed feature decomposition does not qualify the new
source-statement inventory. The stronger existing completion gates and all
quality/source-bound evidence requirements retain their meanings.
Whole-artifact sealing/loading and final semantic collection recompute this
assessment from current source, verifier names, accounting and ledger data.
Missing/truncated inventories, altered readiness/percentage claims and invented
statement denominators are rejected even if artifact hashes are refreshed.

### Post-semantic implementation identity qualification

The final `semantic-proof-summary.json` includes `implementation_identities`.
After every same-run semantic report has passed its existing source/tool/receipt
checks, this section binds shared verifier names to exact file, lexical proof
scope, owning type, method and implementation-trait identities.

Each shared implementation needs a fresh selected pristine proof covering every
selected peer, a compiling strong mutant that fails only that implementation,
and an accepted weaker control preserving the peers. An aggregate whole-proof
name cannot substitute for that selected evidence. Source hashes, exact peer
sets and proof/error cardinalities are checked; missing evidence fails collection.
Existing conversion controls are reused, with two additional root error-wrapper
controls where evidence was absent. The native source and its contracts are
unchanged.

The immutable whole-stage readiness report still exposes the raw shared-name
queue. The final semantic section supplies its evidence-backed resolution;
it does not rewrite the earlier artifact or qualify contract quality,
reachability, generated code, complete statements or external-validation readiness.

### Existing inherited result-contract ownership

The final semantic summary also contains `trait_contract_ownership`. Its bounded
rule recognizes existing `From<T>` result contracts only when the exact native
implementation has a concrete, enabled `FromSpecImpl<T>` model in the same file
and lexical scope, and the configured weaker control disables that exact model's
enabling condition. Actual source-bound positive/strong/weak evidence must still
preserve every selected peer.

The standard contract is the existing pinned vstd `ExFrom::from` conditional
result equality. Its package version/checksum and source-file digest identify
the inspected rule; manifest/lock equality is checked on each assessment. This
is fixed source-rule provenance, not a fresh dependency download, a new standard
specification/assumption or independent normative approval. Changing the pin
requires reviewing the ownership rule rather than silently carrying credit.

Model parsing distinguishes real Verus implementation/function tokens from
comments and literals. It requires the concrete typed input/result methods,
literal `obeys_from_spec = true`, a result definition referencing its input,
and no direct arbitrary/assume/admit/external/opaque markers in the model.
These are bounded ownership/syntax checks, not a transitive dependency-closure
audit or independent semantic-quality/source-fidelity judgments.

The immutable whole-stage no-local-postcondition queue is retained. The final
section records which members already own an inherited result guarantee and
the remaining review count, without adding contracts solely to improve syntax
figures. Unique native implementations may use the existing exact source-bound
whole positive; shared names continue to require fresh selected positives.
Readiness, public/stage reachability, generated code and exhaustive statement
coverage remain unqualified.

## Reproduction

```sh
python3 -m pip install -r tools/requirements.txt
python3 -B tools/check_traceability.py --discover-tests \
  --output target/traceability/reference-report.json
python3 -B tools/check_traceability.py \
  --verus-json target/verification/verus-production.json --discover-tests \
  --require-v01-scope --output target/traceability/v01-scope.json
```

The supplied verifier report must come from the pinned whole build of the same
source. Loading a successful JSON file does not run the solver.
For a local reference document, its identity can also be checked:

```sh
python3 -B tools/check_traceability.py --discover-tests \
  --standard-document 'ITU-T T.6 (1988)=/path/to/t6.pdf' \
  --output target/traceability/t6-report.json
```

Unknown/duplicate document selections and cross-standard substitutions fail.
Identity checking does not imply reacquisition or reproduction rights.
See [reviewing models](specification-review.md) and
[verification limitations](verifier-limitations.md).
