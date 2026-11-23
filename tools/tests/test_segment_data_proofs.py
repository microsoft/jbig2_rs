import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from source_group import source_text
from check_segment_data_proofs import CASES, PAGE_FIELDS, PROFILE_PREFIX
from proof_sensitivity import replace_once, weakened_contract
from rust_source import parse


class SegmentDataProofFixtureTests(unittest.TestCase):
    def test_all_ten_mutations_are_unique_valid_source_edits(self):
        self.assertEqual(len(CASES), 10)
        self.assertEqual(len({case["id"] for case in CASES}), 10)
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = source_text(TOOLS.parent, case["file"])
                mutated = replace_once(source, case["site"], case["replacement"])
                self.assertNotEqual(mutated, source)
                self.assertFalse(parse(mutated.encode()).has_error)
                control = weakened_contract(mutated, case)
                self.assertNotEqual(control, mutated)
                self.assertFalse(parse(control.encode()).has_error)

    def test_page_controls_keep_original_flag_dimension_and_payload_guarantees(self):
        source = source_text(TOOLS.parent, "src/segment_data.rs")
        for case in CASES:
            if case["function"] != "parse_page_information_with_colour":
                continue
            control = weakened_contract(source, case)
            self.assertNotIn(PAGE_FIELDS, control)
            self.assertIn("crate::spec::segments::page_flags_match(", control)
            self.assertIn("crate::spec::segments::dimensions_within_limits(", control)
            self.assertIn("payload@.len() == 19", control)
            self.assertIn("info.default_operator != CombinationOperator::Replace", control)

    def test_profile_value_control_keeps_prefix_extent_count_and_reader_frame(self):
        source = source_text(TOOLS.parent, "src/segment_data.rs")
        case = next(case for case in CASES if case["id"] == "profile_wire_values_discarded")
        control = weakened_contract(source, case)
        self.assertNotEqual(replace_once(control, PROFILE_PREFIX, "true"), control)
        self.assertIn("final(profiles)@.len() == old(profiles)@.len() + count", control)
        self.assertIn("final(reader).bytes@ == old(reader).bytes@", control)
        self.assertIn("original_profiles == old(profiles)@", control)
        self.assertIn("profiles@.len() == original_profiles.len() + index", control)
        self.assertIn("old(reader).position as int + index * 4 + 4 <= old(reader).bytes@.len()", control)

    def test_prefix_control_retains_new_wire_values_and_loop_history(self):
        source = source_text(TOOLS.parent, "src/segment_data.rs")
        case = next(case for case in CASES if case["id"] == "profile_existing_prefix_corrupted")
        control = weakened_contract(source, case)
        self.assertIn("final(profiles)@[old(profiles)@.len() + index]", control)
        self.assertIn("profiles@[original_profiles.len() + position]", control)
        self.assertIn("final(reader).position == old(reader).position as int + count as int * 4", control)

    def test_container_controls_keep_internal_typed_and_byte_window_proofs(self):
        source = source_text(TOOLS.parent, "src/document.rs")
        for case in CASES:
            if case["function"] not in {"parse_sequential", "parse_random_access"}:
                continue
            control = weakened_contract(source, case)
            self.assertIn("document_spec::segment_metadata(segments@)", control)
            self.assertIn("document_spec::known_segment_data_fields(header.segment_type, payload@, data)", control)
            self.assertIn("document_spec::sequential_windows(", control)
            self.assertIn("document_spec::random_payload_windows(", control)
            self.assertIn("production_random_access_complete!", control)
            self.assertIn("production_sequential_complete!", control)


if __name__ == "__main__":
    unittest.main()
