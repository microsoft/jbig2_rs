#!/usr/bin/env python3
"""Check accepted raw structural metadata and returned typed-data correspondence."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
PAGE_FIELDS = "crate::spec::segments::colour_page_information_fields(payload@, info)"
KNOWN_FIELDS = "document_spec::known_segment_data_fields(header.segment_type, payload@, data)"
PROFILE_PREFIX = """(forall|index: int| 0 <= index < old(profiles)@.len() ==>
    #[trigger] final(profiles)@[index] == old(profiles)@[index])"""
PROFILE_VALUES = """&& #[trigger] final(profiles)@[old(profiles)@.len() + index]
    == crate::spec::segments::be_u32_at(
        old(reader).bytes@, old(reader).position as int + index * 4,
    )"""
PROFILE_LOOP_VALUES = """forall|position: int| 0 <= position < index ==>
    #[trigger] profiles@[original_profiles.len() + position]
        == crate::spec::segments::be_u32_at(
            reader.bytes@, start + position * 4,
        ),"""
BASELINE_GROUPS = (
    ("segment_data", ("parse_page_information_with_colour", "read_profiles_into")),
    ("document", ("parse_known_segment_data", "parse_random_access", "parse_sequential")),
)


def body_case(identity, module, function, before, after, controls):
    original = runtime_function(ROOT, module, function)["body_text"]
    return {
        "id": identity, "file": f"src/{module}.rs", "module": module,
        "function": function, "site": original,
        "replacement": replace_once(original, before, after), "controls": controls,
    }


def page_case(identity, before, after):
    return body_case(
        identity, "segment_data", "parse_page_information_with_colour", before, after,
        ((PAGE_FIELDS, "true"),),
    )


def dispatch_case(identity, before, after):
    return body_case(
        identity, "document", "parse_known_segment_data", before, after,
        ((KNOWN_FIELDS, "true"),),
    )


def container_case(function, contract):
    return body_case(
        function + "_discards_typed_page_metadata", "document", function,
        "Ok(segments)", """if !segments.is_empty() {
            let mut first = segments.remove(0);
            first.data = ParsedData::Other;
            segments.insert(0, first);
        }
        Ok(segments)""",
        ((contract, "true"),),
    )


CASES = (
    page_case(
        "page_width_discarded", "PageInformation { width,",
        "PageInformation { width: 0,",
    ),
    page_case("known_page_height_discarded", "Some(encoded_height)", "Some(0)"),
    page_case(
        "page_resolutions_swapped",
        "x_resolution, y_resolution, eventually_lossless:",
        "x_resolution: y_resolution, y_resolution: x_resolution, eventually_lossless:",
    ),
    body_case(
        "profile_wire_values_discarded", "segment_data", "read_profiles_into",
        "profiles.push(reader.read_u32()?);", "reader.read_u32()?;\nprofiles.push(0);",
        ((PROFILE_VALUES, """&& #[trigger] final(profiles)@[old(profiles)@.len() + index]
            == final(profiles)@[old(profiles)@.len() + index]"""),
         (PROFILE_LOOP_VALUES, "")),
    ),
    body_case(
        "profile_existing_prefix_corrupted", "segment_data", "read_profiles_into",
        "Ok(())", """if count == 0 && !profiles.is_empty() { profiles[0] = 0; }
        Ok(())""",
        ((PROFILE_PREFIX, "true"),),
    ),
    dispatch_case(
        "page_dispatch_returns_wrong_variant",
        "SegmentType::PageInformation => ParsedData::Page(parse_page_information(payload, limits)?),",
        """SegmentType::PageInformation => {
            parse_page_information(payload, limits)?;
            ParsedData::Other
        },""",
    ),
    dispatch_case(
        "profile_dispatch_returns_wrong_variant",
        "SegmentType::Profiles => ParsedData::Profiles(parse_profiles(payload)?),",
        """SegmentType::Profiles => {
            parse_profiles(payload)?;
            ParsedData::Other
        },""",
    ),
    dispatch_case(
        "stripe_dispatch_corrupts_value",
        "ParsedData::EndOfStripe(parse_end_of_stripe(payload)?)",
        "ParsedData::EndOfStripe(parse_end_of_stripe(payload)? ^ 1)",
    ),
    container_case(
        "parse_sequential",
        """document_spec::sequential_container_fields(
            input@, segments@, require_local_references,
        )""",
    ),
    container_case(
        "parse_random_access", "document_spec::random_access_container_fields(input@, segments@)",
    ),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES,
            ("src/segment_data.rs", "src/document.rs", "specs/segments.rs", "specs/document.rs",
             "proofs/production_impl/segment_data.rs", "proofs/production_impl/document.rs",
             "proofs/model_support/document.rs"),
            "jbig2-segment-data-",
            "Accepted raw page fields, profile append contents/frames and typed structural metadata through container results; not complete acceptance/errors, entropy/Huffman/refinement correctness, allocator capacity or normative approval",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"segment data proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
