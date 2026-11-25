# All-standard Huffman descriptor trial (#233)

## Scope and status

This isolated, unpublished 0.3 development trial starts at
`14aeac451f6491aa3e413fcbc60aafb11fa75b7c`. It does not change `release/0.2`.
It is **prepared for parent measurement, not qualified for retention**.
The native, MSRV, lint and whole-crate proof checks below pass. Performance,
independent review/provenance renewal and normal integration gates remain
separate. No latency or allocation result is claimed here.

The earlier A-only prototype
`4c6f0cc789b4a665d3a5ace9bdd004d46a8aefbc` removed 129 requests and 447,200
cumulative requested bytes, but did not satisfy latency/control gates. This is
a different all-fifteen-table experiment, not a relabelled A-only result.
The supplied five-round coarse observer evidence
`coarse-phases-five-round-v2.json` estimates standard setup at approximately
5–13.6% of instrumented wall time on real Huffman targets, including 10.44% on
ordinary, 10.99% on mixed B5/B3/B7/B9/B12 and 13.63% on refagg-several inputs.
Those are observer estimates, not actual speedups or exclusive cycle counts.

## Executing mechanism

Each Annex B standard table has immutable, compile-time canonical metadata:
ten `u16` first codes, ten `u8` counts, ten `u8` starts, at most 22 ordered line
indices and its maximum prefix length. A private source tag selects the
existing normative line constants. The codec's existing per-region standard
slots now hold references to these descriptors rather than allocated owned
tables. Selection performs the original logical line-count limit check but
does not validate/copy/sort the same fixed lines or construct the 256-entry
wide-code metadata on every region.

There is no mutable global cache, lazy initialization, synchronization,
dependency, unsafe code or persistent document state. Only immutable standard
metadata is shared. Custom and symbol-ID owned tables retain their original
construction, storage and `HuffmanTable::decode` implementation. Public owned
standard constructors also retain their original allocation/refusal behavior.

The resolved codec view distinguishes descriptor and general-table references
at field selection. The getter and dispatch wrappers deliberately force
inlining to fuse custom-field resolution with decoding. Optimized Rust 1.98.1
IR confirms that the custom binding branch calls the unchanged owned decoder
directly, without a second resolved-descriptor tag test or wrapper call. An
initial hint-only build did retain that avoidable dispatch; its IR is preserved
separately. No representation test was inserted into the general/custom decoder.
This is not evidence that all generated-code/control effects are harmless.
Actual control measurements remain mandatory.

Descriptor prefix decoding still reads exactly one bit at a time and uses the
existing checked `WideCode` operations and `decode_line` suffix implementation.
It does not batch prefix reads, advance on failed suffix reads or suppress
range overflow. Final-strip padding retains the original strict sub-byte
zero-padding test and one-bit EOF guard.

## Limits and ownership

Every newly selected standard table still checks `max_symbols` against its
complete original line count, with the same Symbols error payload. Reusing an
already materialized per-region slot preserves the existing behavior.
No stream, bitmap, decoded symbol, work or custom-table limit is relaxed.

The descriptor path no longer requests the owned line/order vectors or box,
or constructs the owned table's large canonical arrays. Consequently it does
not charge the memory estimate for that removed ownership and cannot return
the allocation failures for those removed requests. An artificially small
`max_decoder_managed_memory_bytes` can therefore admit standard-descriptor
setup where the unchanged owned constructor refuses it. This is an intentional
physical ownership/refusal change, not identical low-memory error behavior.
Read-only executable data and unrelated allocator/runtime overhead are not
claimed as decoder-managed heap savings or RSS savings.

## Model and test boundary

The shared owned constructor now proves canonical assignment for all fifteen
literal standard sources, not only borrowed raw lines. The descriptor checks
reuse that same histogram, stable-order and first-prefix recurrence model.
The bridge relates descriptor results and consumed cursors to any successful
owned canonical constructor for those supplied lines, including prefix/suffix
EOF, OOB and integer overflow. A separate padding bridge relates final-strip
eligibility.

The exact field getter models continue to carry actual complete supplied
contents, not only a standard tag. Missing slots remain missing; native tests
deliberately put another descriptor in slot A to exercise this distinction.
Concrete and enum-level integer/ID contracts retain their exact results,
cursors, input identity and unchanged codec metadata.

Focused descriptor tests compare all fifteen tables, every nine-bit prefix,
all available bit positions, each line's endpoint/overflow suffixes, every
truncation boundary, continuation and final-strip behavior with the owned
canonical path. Logical-limit versus removed-memory refusal behavior is
explicitly tested. Existing independent scalar owned/custom Huffman tests
remain in place.

These are local representation/result bridges, not independent normative
approval or an end-to-end rendering/resource theorem. The inherited 750 direct declarations remain directly verified, and the same
20 reviewed limitation records are byte-for-byte unchanged. Seven new runtime
declarations are directly verified. No coverage flag, trust boundary,
assumption or precondition waiver is part of this trial.

## Validation receipts and remaining gates

- Whole pinned Verus: **1,577 verified obligations, zero errors**, entire crate.
  Exact-source production accounting: **757 directly verified, 20 reviewed
  limitations; zero incomplete, divergences, native assumptions or forwarders**.
  No original direct declaration is removed.
- Full `cargo test --locked --offline --all-features` on Rust **1.98.1** and
  MSRV **1.88.0**: each passes **646 native tests plus one doc test**; 16 existing
  tests remain ignored. No ignored benchmark is executed.
- Strict repository-pinned **Clippy 1.89.0**, all targets/features with
  `-D warnings`, and formatting checks pass. An exploratory Clippy 1.98 run
  encountered pre-existing new-version lints outside this change; no unrelated
  code or lint/build thresholds were changed to suppress them.
- Runtime/trust/source-budget audit passes, including the 800-line file bound.
  Native test-catalog discovery passes all 92 catalog cases, 162 references and
  157 distinct referenced tests.
- Historical provenance review is **not renewed**. `verify_provenance.py`
  correctly rejects the old `codeStreamTest1_TT1` review hash for changed
  `src/huffman.rs`. The original approval bytes are retained. The candidate is
  not claimed ready for protected integration merely because local proofs pass.
- Strict traceability also correctly refuses its old standard-getter contract
  fragment: that ledger still names the owned-only `optional_table_contents`
  projection, whereas the executing getter now proves complete descriptor
  contents through `optional_standard_contents`. The exact selected-slot and
  selected-field guarantees are proved, not removed; updating the ledger's
  representation references/current-scope classification remains an integration
  gate. No traceability flag or historical review record was weakened to hide
  this boundary.

The whole command uses `/workspace/jbig2-pinned-verus/{verus,cargo-verus}`,
Verus `0.2026.10.04.426d8b0`, commit
`426d8b01e7ffb910a36c15e1f870ff985bb61eef`, Rust 1.98.1
(`48a229ceaefd4985c50990b14116b6d856af0985`, LLVM 22.1.8), and:

```text
cargo verus build --manifest-path verification/Cargo.toml --locked --offline -- \
  --num-threads 2 --rlimit 30 --triggers-mode silent --output-json
```

The actual invocation also emits VIR into the isolated worktree's target
directory. The existing verification-manifest host-build override remains 1.
`tools/extract_cargo_verus_report.py` separates the production report from the
raw JSON stream; `audit_production_coverage.py --require-complete --require-exact`
binds its declarations to the actual runtime source. Python audits use the
supplied `jbig2-compat28-20261120/target/compat28/audit-venv/bin/python`
environment. Raw streams, extracted report, compiled proof library, test/lint
logs, code-generation evidence and a hash-bound local receipt remain under
`target/descriptor-evidence/` (the library is under `target/verus-whole/`).

The rejected constant-slice representation, its source and its Verus
constant-array-to-slice mode error are preserved separately under
`target/descriptor-evidence/slice-descriptor-*`. They are prototype diagnostics,
not evidence for the final representation. Earlier failed shape/padding proofs
and unsupported constructor-function-value / insufficiently specified map
attempts are also retained, separately named from the successful whole report.
All timing, full paired allocation evaluation, gate decisions and integration
remain outside this implementation preparation.
