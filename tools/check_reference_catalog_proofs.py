#!/usr/bin/env python3
"""Check exact reference accounting, lookup and structural catalog results."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "src/document.rs", "specs/document.rs", "specs/document/catalogs.rs",
    "specs/document/results.rs", "specs/document/context.rs", "specs/segments.rs",
    "proofs/model_support/document.rs", "proofs/shared/document.rs",
    "specs/document/reference_types.rs",
)
TEST = "document::tests::quotas_and_references::"
TOTALS = TEST + "reference_catalog_totals_and_work_match_u128_admission"
LOOKUP = TEST + "reference_catalog_keyed_get_keeps_first_duplicate_value_and_absence"
PREFIX = TEST + "reference_catalog_ordered_depth_keeps_arbitrary_prefix_and_partial_errors"
GLOBALS = TEST + "reference_catalog_public_globals_keep_matching_type_and_depth_counts"
GRAPH = TEST + "reference_graph_retention_and_budget_precedence_match_scalar_policy"


def case(function, identity, edits, test, *, losses=None):
    row = runtime_function(ROOT, "document", function)
    source = "\n".join(row["attributes"]) + "\n" + row["body_text"]
    mutant = source
    for before, after in edits:
        mutant = replace_once(mutant, before, after)
    weaker = mutant
    if losses is None:
        attributes = [text for text in row["attributes"] if "verus_spec(" in text]
        if len(attributes) != 1 or attributes[0].count("ensures") != 1:
            raise ValueError("new catalog result needs one owning ensures section")
        attribute = attributes[0]
        offset = attribute.index("ensures")
        weaker = replace_once(
            weaker, attribute, attribute[:offset] + "ensures true,\n    ))]",
        )
    else:
        for before, after in losses:
            weaker = replace_once(weaker, before, after)
    return {
        "id": identity, "file": row["file"], "module": row["verifier_module"],
        "function": function, "site": source, "replacement": mutant,
        "controls": ((mutant, weaker),), "native_test": test,
    }


CASES = (
    case("count_total_references", "total_reference_result_discarded",
         (("\n    Ok(total)\n}", "\n    Ok(0)\n}"),), TOTALS),
    case("enforce_work_limit", "work_quota_ignored",
         (("actual > limits.max_work_units", "false"),), TOTALS),
    case("enforce_work_limit", "work_overflow_accepted",
         (("""let Some(partial) = input_work.checked_add(segment_work) else {
        return Err(MalformedError::IntegerOverflow.into());
    };""", """let Some(partial) = input_work.checked_add(segment_work) else {
        return Ok(());
    };"""),), TOTALS),
    case("keyed_index", "first_key_match_skipped_for_adjacent_duplicate",
         (("return Some(index);", """if index + 1 < entries.len() && entries[index + 1].0 == number {
                return Some(index + 1);
            }
            return Some(index);"""),), LOOKUP,
         losses=(("&& forall|prior: int| 0 <= prior < index ==> entries@[prior].0 != number", ""),)),
    case("keyed_get", "matched_key_returns_unrelated_first_value",
         (("Some(&entries[index].1)", "Some(&entries[0].1)"),), LOOKUP),
    case("keyed_get", "existing_key_is_reported_absent",
         (("""let index = keyed_index(entries, number)?;
    Some(&entries[index].1)""", "let _unused = (entries, number);\n    None"),), LOOKUP),
    case("validate_reference_depth_ordered", "ordered_depth_catalog_is_skipped",
         (("while ordered_index < ordered.len()", "while false"),), PREFIX,
         losses=(("out.is_ok() ==> final(depths)@.len() == old(depths)@.len() + ordered@.len(),", ""),)),
    case("validate_reference_depth", "computed_depth_catalog_discarded",
         (("\n    Ok(depths)\n}", "\n    Ok(Vec::new())\n}"),), GRAPH),
    case("validate_references", "successful_type_catalog_discarded",
         (("Ok((types, depths))", "Ok((Vec::new(), depths))"),), GLOBALS),
    case("validate_references", "type_catalog_records_wrong_segment_kind",
         (("segment.header.number,\n            segment.header.segment_type,",
           "segment.header.number,\n            SegmentType::EndOfPage,"),
          ("&& types@[index].1 == segments@[index].header.segment_type", "")), GLOBALS),
    case("validate_references", "type_catalog_records_wrong_segment_number",
         (("""&mut types,
            segment.header.number,""", """&mut types,
            0,"""),
          ("""types@[index].0 == segments@[index].header.number
                && types@[index].1 == segments@[index].header.segment_type""",
           "types@[index].1 == segments@[index].header.segment_type")), GLOBALS),
    case("Decoder::parse_globals", "public_globals_discard_depth_catalog",
         (("""let out = GlobalSegments {
            types,
            depths,
            bytes,
        };""", """let out = GlobalSegments {
            types,
            depths: Vec::new(),
            bytes,
        };"""),), GLOBALS,
         losses=(("""match out {
            Ok(globals) => document_spec::global_segment_count(&globals)
                == document_spec::global_depth_count(&globals),
            Err(_) => true,
        }""", "true"),)),
)
CASE_GROUPS = {"reference-accounting": CASES[:6], "reference-catalog": CASES[6:]}


def baseline_groups(cases):
    modules = {}
    for row in cases:
        functions = modules.setdefault(row["module"], [])
        if row["function"] not in functions:
            functions.append(row["function"])
    return tuple((module, tuple(functions)) for module, functions in modules.items()
                 if len(functions) > 1)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case-group", choices=tuple(CASE_GROUPS), required=True)
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    cases = CASE_GROUPS[arguments.case_group]
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), cases, FILES, "jbig2-reference-catalog-",
            "Exact reference/work accounting and first lookup; successful type identity and"
            " depth catalog cardinality/bounds; ordered-depth prefix/partial-error frames."
            " Domains retain arbitrary existing prefixes and existing in-range index"
            " preconditions. Structural state/value baseline, not complete graph/depth"
            " semantics or normative fidelity; internal metadata is not a compressed witness.",
            baseline_groups=baseline_groups(cases),
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"reference catalog sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    report["case_group"] = arguments.case_group
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
