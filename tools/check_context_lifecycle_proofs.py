#!/usr/bin/env python3
"""Check actual generic/refinement context API state and frame contracts."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
BASELINE_GROUPS = (("generic_region", tuple(
    owner + "::" + method
    for owner in ("GenericContexts", "RefinementContexts")
    for method in ("new", "reset", "try_snapshot", "restore")
)),)
TEST_PREFIX = "generic_region::tests::sessions_and_snapshots::"
RESET_ZERO = """forall |index: int| 0 <= index < final(self).contexts@.len() ==>
    crate::arithmetic::arithmetic_spec::context_index(final(self).contexts@[index]) == 0
        && crate::arithmetic::arithmetic_spec::context_mps(final(self).contexts@[index]) == 0"""
NEW_ZERO = """&& (forall |index: int| 0 <= index < contexts.contexts@.len() ==>
    crate::arithmetic::arithmetic_spec::context_index(contexts.contexts@[index]) == 0
        && crate::arithmetic::arithmetic_spec::context_mps(contexts.contexts@[index]) == 0)"""
PREFIX = """forall |completed: int| 0 <= completed < index ==>
    self.contexts@[completed] == snapshot.contexts@[completed],"""
COPY_ASSERT = "assert(self.contexts@ == snapshot.contexts@);"


def case(owner, method, identity, site, replacement, lost, weaker="", *, test, drop_copy=False):
    row = runtime_function(ROOT, "generic_region", owner + "::" + method)
    attributes = [attribute for attribute in row["attributes"] if "verus_spec(" in attribute]
    if len(attributes) != 1:
        raise ValueError("context API needs one exact attached contract")
    body = row["body_text"]
    mutant = replace_once(body, site, replacement)
    controls = ((attributes[0], replace_once(attributes[0], lost, weaker)),)
    if drop_copy:
        weak_body = replace_once(mutant, PREFIX, "")
        weak_body = replace_once(weak_body, COPY_ASSERT, "")
        controls += ((mutant, weak_body),)
    return {
        "id": identity, "file": row["file"], "module": "generic_region",
        "function": owner + "::" + method, "site": body, "replacement": mutant,
        "controls": controls, "native_test": TEST_PREFIX + test,
    }


CASES = ()
for owner, view, offset, test in (
    ("GenericContexts", "generic_snapshot_view", 2,
     "generic_snapshots_copy_all_states_and_preserve_rejected_targets"),
    ("RefinementContexts", "refinement_snapshot_view", 1,
     "raw_refinement_snapshots_preserve_all_states_and_failure_frames"),
):
    label = "generic" if owner == "GenericContexts" else "refinement"
    success = f"""final(self).contexts@ == generic_region_spec::{view}(snapshot).{offset}
        && generic_region_spec::contexts_valid(final(self).contexts@)"""
    snapshot_equal = f"""&& generic_region_spec::{view}(&snapshot).{offset} == self.contexts@"""
    CASES += (
        case(
            owner, "reset", label + "_reset_keeps_old_states",
            "reset_contexts(&mut self.contexts);", "let _unreset = &self.contexts;",
            RESET_ZERO, "true", test=test,
        ),
        case(
            owner, "try_snapshot", label + "_snapshot_discards_states",
            "try_copy_contexts(&self.contexts, self.storage_bytes, limits)?",
            "try_copy_contexts(&self.contexts[..0], self.storage_bytes, limits)?",
            snapshot_equal, test=test,
        ),
        case(
            owner, "restore", label + "_restore_repeats_first_state",
            "self.contexts[index] = snapshot.contexts[index];",
            "self.contexts[index] = snapshot.contexts[0];",
            success, "true", test=test, drop_copy=True,
        ),
        case(
            owner, "restore", label + "_restore_writes_before_rejection",
            "if self.template != snapshot.template",
            """if !self.contexts.is_empty() {
                self.contexts[0] = ArithmeticContext::new();
            }
            if self.template != snapshot.template""",
            "Err(_) => final(self).contexts@ == old(self).contexts@,",
            "Err(_) => true,", test=test,
        ),
        case(
            owner, "new", label + "_new_poisons_first_probability",
            "let contexts = allocate_contexts(count, 0, limits)?;",
            """let mut contexts = allocate_contexts(count, 0, limits)?;
            if !contexts.is_empty() {
                contexts[0] = ArithmeticContext { packed: 1 };
            }""",
            NEW_ZERO, test="fresh_context_banks_have_exact_metadata_and_zero_probability_states",
        ),
    )
    metadata = (
        ("extended_template: self.extended_template,", "extended_template: !self.extended_template,",
         "&& generic_region_spec::generic_snapshot_view(&snapshot).1 == self.extended_template")
        if label == "generic" else
        ("template: self.template,", "template: RefinementTemplate::Template0,",
         "generic_region_spec::refinement_snapshot_view(&snapshot).0 == self.template")
    )
    CASES += (case(
        owner, "try_snapshot", label + "_snapshot_changes_configuration",
        metadata[0], metadata[1], metadata[2],
        "" if label == "generic" else "true", test=test,
    ),)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES,
            ("src/generic_region.rs", "src/arithmetic.rs", "specs/generic_region.rs",
             "specs/arithmetic.rs", "proofs/shared/generic_region.rs"),
            "jbig2-context-lifecycle-",
            "Actual generic/refinement fresh/reset/snapshot/restore values and metadata/failure "
            "frames. No new preconditions, allocator-capacity bound, normative caller reset timing "
            "or full raster/stream proof.",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"context lifecycle sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
