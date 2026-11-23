import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from check_colour_proofs import CASES
from proof_sensitivity import weakened_contract
from source_group import source_text
from rust_source import parse


class ColourProofTests(unittest.TestCase):
    def test_every_colour_mutant_and_control_is_executable_source(self):
        self.assertEqual(len(CASES), 5)
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]):
                self.assertIn(case["site"], source)
                changed = source.replace(case["site"], case["replacement"], 1)
                self.assertFalse(parse(changed.encode()).has_error)
                weaker = weakened_contract(changed, case)
                self.assertFalse(parse(weaker.encode()).has_error)

    def test_colour_controls_preserve_unrelated_safety_and_policy_frames(self):
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            weaker = weakened_contract(source, case)
            with self.subTest(case=case["id"]):
                if case["function"] == "RgbBitmap::set":
                    self.assertIn("colour_spec::storage_valid(final(self))", weaker)
                    self.assertIn("final(self).stride == old(self).stride", weaker)
                if case["function"] == "ensure_declaration":
                    self.assertIn("out.is_ok() ==> *final(declared)", weaker)
                    self.assertIn("old(recoveries)@.push(recovery)", weaker)
                if case["function"] == "decode_generic_with_colour":
                    self.assertIn("decoded.consumed_bytes == payload@.len()", weaker)


if __name__ == "__main__":
    unittest.main()
