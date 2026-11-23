from pathlib import Path
import sys
import unittest


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from source_group import source_text
from check_iaid_state_proofs import CASES
from proof_sensitivity import replace_once, weakened_contract


class IaidStateProofFixtureTests(unittest.TestCase):
    def setUp(self):
        self.source = source_text(TOOLS.parent, "src/arithmetic/integers.rs")

    def test_eight_state_and_range_mutations_have_unique_sites(self):
        self.assertEqual(len({case["id"] for case in CASES}), 8)
        for case in CASES:
            with self.subTest(case=case["id"]):
                self.assertNotEqual(replace_once(self.source, case["site"], case["replacement"]), self.source)

    def test_controls_keep_safety_preflight_and_loop_relation(self):
        for case in CASES:
            with self.subTest(case=case["id"]):
                changed = weakened_contract(self.source, case)
                self.assertIn("arithmetic_spec::decoder_state_valid(final(self))", changed)
                self.assertIn("InvalidIaidCodeLength(code_length)", changed)
                self.assertIn("arithmetic_spec::iaid_run(*old(self)", changed)
                self.assertIn("decreases remaining", changed)

    def test_missing_ambiguous_and_repeated_controls_fail_closed(self):
        for case in CASES:
            with self.subTest(case=case["id"]):
                for invalid in ("", self.source + "\n" + case["site"]):
                    with self.assertRaises(ValueError):
                        replace_once(invalid, case["site"], case["replacement"])
                with self.assertRaises(ValueError):
                    weakened_contract(weakened_contract(self.source, case), case)
