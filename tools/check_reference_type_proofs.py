#!/usr/bin/env python3
"""Check exact reference lookup and staged type acceptance guarantees."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "src/document.rs", "specs/document.rs", "specs/document/reference_types.rs",
    "specs/document/catalogs.rs", "specs/document/results.rs", "specs/document/context.rs",
    "proofs/model_support/document.rs", "proofs/shared/document.rs", "specs/segments.rs",
)
TEST = "document::tests::reference_types::reference_types_"
LOOKUP = TEST + "lookup_keeps_bounds_first_local_precedence_and_global_first_match"
INTERMEDIATE = TEST + "intermediate_classifies_every_resolved_kind_and_missing"
SYMBOLS = TEST + "symbol_resources_keep_table_boundary_and_stage_missing_behavior"
DISPATCH = TEST + "dispatch_matches_every_owner_kind_cardinality_and_resolved_class"


def case(function, identity, edits, test):
    row = runtime_function(ROOT, "document", function)
    source = "\n".join(row["attributes"]) + "\n" + row["body_text"]
    mutant = source
    for before, after in edits:
        mutant = replace_once(mutant, before, after)
    attributes = [text for text in row["attributes"] if "verus_spec(" in text]
    if len(attributes) != 1 or attributes[0].count("ensures") != 1:
        raise ValueError("reference-type result needs one exact owning clause")
    attribute = attributes[0]
    weaker = replace_once(mutant, attribute,
                         attribute[:attribute.index("ensures")] + "ensures true,\n    ))]")
    return {
        "id": identity, "file": row["file"], "module": row["verifier_module"],
        "function": function, "site": source, "replacement": mutant,
        "controls": ((mutant, weaker),), "native_test": test,
    }


LOOKUP_CASES = (
    case("known_reference_type", "local_resolution_overridden_by_global",
         (("if let Some(entry) = keyed_index(indexes, number) {",
           "if let Some(entry) = if globals.is_none() { keyed_index(indexes, number) } else { None } {"),), LOOKUP),
    case("known_reference_type", "resolved_local_type_discarded",
         (("return Some(segments[segment_index].header.segment_type);",
           "return Some(SegmentType::SymbolDictionary);"),), LOOKUP),
    case("known_reference_type", "resolved_global_type_discarded",
         (("Some(*kind)", "Some(SegmentType::SymbolDictionary)"),), LOOKUP),
    case("known_reference_type", "missing_reference_manufactures_a_type",
         (("""if reference_index >= header.referred_segments.len() {
        return None;
    }""", """if reference_index >= header.referred_segments.len() {
        return Some(SegmentType::SymbolDictionary);
    }"""),), LOOKUP),
    case("known_reference_is_intermediate", "all_resolved_types_are_intermediate",
         (("Some(kind) => is_intermediate_region(kind),", "Some(_kind) => true,"),), INTERMEDIATE),
    case("known_reference_is_intermediate", "missing_type_is_intermediate",
         (("None => false,", "None => true,"),), INTERMEDIATE),
)
CLASS_CASES = (
    case("references_are_symbol_resources", "table_limit_admits_one_extra",
         (("table_count > maximum_tables", "table_count > maximum_tables.saturating_add(1)"),
          ("table_count <= maximum_tables,", "")), SYMBOLS),
    case("references_are_symbol_resources", "table_count_not_charged",
         (("table_count += 1;", "let _uncharged = table_count;"),
          ("""table_count as nat == document_spec::resolved_table_count(
                header.referred_segments@, segments@, indexes@, globals, index as nat,
            ),""", "")), SYMBOLS),
    case("references_are_symbol_resources", "invalid_known_kind_accepted",
         (("_ => return false,", "_ => {}"),
          ("""forall|prior: int| 0 <= prior < index ==> match
                #[trigger] document_spec::resolved_reference_type(
                    header.referred_segments@, prior as nat, segments@, indexes@, globals,
                ) {
                    Some(kind) => kind == SegmentType::SymbolDictionary || kind == SegmentType::Tables,
                    None => true,
                },""", "")), SYMBOLS),
    case("validate_reference_types", "dictionary_uses_text_table_limit",
         (("references_are_symbol_resources(header, segments, indexes, globals, 4)",
           "references_are_symbol_resources(header, segments, indexes, globals, 8)"),), DISPATCH),
    case("validate_reference_types", "halftone_accepts_symbol_dictionary",
         (("Some(SegmentType::PatternDictionary)", "Some(SegmentType::SymbolDictionary)"),), DISPATCH),
    case("validate_reference_types", "rejection_discards_owner_number",
         (("return Err(MalformedError::InvalidSegmentReferences(header.number).into());",
           "return Err(MalformedError::InvalidSegmentReferences(0).into());"),), DISPATCH),
)
CASES = LOOKUP_CASES + CLASS_CASES
CASE_GROUPS = {"reference-type-lookup": LOOKUP_CASES, "reference-type-classes": CLASS_CASES}


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
            ROOT, arguments.verus_executable.resolve(), cases, FILES, "jbig2-reference-types-",
            "Exact first local/global resolution and out-of-range absence; intermediate kinds,"
            " symbol/table eligibility and exact staged type acceptance/error owner."
            " Unresolved symbol/text references remain skipped here because preceding link"
            " validation owns missing-reference rejection. Existing index-range domains remain;"
            " no complete graph/reference/normative-fidelity or input-witness claim.",
            baseline_groups=baseline_groups(cases),
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"reference type sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    report["case_group"] = arguments.case_group
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
