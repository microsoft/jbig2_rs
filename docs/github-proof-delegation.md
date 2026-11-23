# Proof contributions

Proof changes are contributions to the same Rust implementation used by Cargo.
They must preserve native behavior and make their actual guarantees clear.
Automated assistance does not replace review or validation.

## Bound the change

Specify the runtime operation, its input/output relation, affected source and
models, preconditions, trust boundaries and exact tests. Prefer one independently
reviewable property, such as a reader cursor frame or a page-state transition,
over a broad claim about an entire codec.

Read the [model guide](../specs/README.md) and
[review procedure](specification-review.md). Compare standard-facing claims
with the pinned reference rather than treating implementation tables as the
normative answer.

## Develop and verify

Use focused verification to diagnose a small change:

```sh
cargo +1.98.1 verus focus --manifest-path verification/Cargo.toml --locked -- \
  --verify-module document --num-threads 1 --rlimit 30 --triggers-mode silent
```

Then run the unfiltered production build:

```sh
cargo +1.98.1 verus build --manifest-path verification/Cargo.toml --locked -- \
  --num-threads 1 --rlimit 30 --triggers-mode silent
```

Focused success is not whole-crate verification. Keep normal Cargo tests,
source accounting, semantic contrasts and native mutation checks in the
change's validation. See [validation](../validation/README.md).

The semantic runner shares locked verification dependency builds between
checkers in one job. Each worker has a separate temporary cache, keyed by the
pinned toolchain and the complete verification dependency graph, and the runner
deletes it on completion. This is not a cache of proof results or production
builds: every selected query still cleans the production package and invokes
the pinned verifier on its current isolated source. Native mutant checks,
strong rejection, weaker acceptance, whole-positive source binding and the
1,050-second actual timing guard are unchanged.

## Review checklist

- The native body, proof-selected body and recorded source identities agree.
- Contracts describe the intended result, not only valid storage or successful
  outcomes.
- Preconditions do not exclude inputs merely to make the proof easy.
- Models, table values, field interpretation and caller obligations are checked
  separately from local algebra.
- Strong/weaker controls compile and distinguish the specific guarantee.
- Tests use independent expected bytes, pixels, values or typed errors.
- Changes do not add assumptions, trusted codec bodies or termination waivers.
- Current-head CI includes the whole proof, compiled library and every
  configured sensitivity group.

Independent changes may proceed in parallel, but edits to shared models,
registries and source bindings must be integrated and revalidated together.
