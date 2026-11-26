import sys
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_reset_proof import ASSIGNMENT, mutated_reset, validity_only_contract


class ResetProofFixtureTests(unittest.TestCase):
    def setUp(self):
        self.source = (TOOLS.parent / "src/arithmetic.rs").read_text(encoding="utf-8")

    def test_mutants_are_valid_nonzero_contexts_at_one_assignment(self):
        self.assertEqual(self.source.count(ASSIGNMENT), 1)
        for index, mps in ((1, 0), (0, 1)):
            changed = mutated_reset(self.source, index, mps)
            self.assertNotEqual(changed, self.source)
            self.assertNotIn(ASSIGNMENT, changed)
            self.assertIn(f"packed: {(index << 1) | mps}", changed)
            self.assertIn(
                "arithmetic_spec::context_index(final(contexts)@[index]) == 0",
                changed,
            )

    def test_control_only_removes_the_two_zero_quantifiers(self):
        control = validity_only_contract(self.source)
        self.assertNotIn(
            "arithmetic_spec::context_index(final(contexts)@[index]) == 0", control,
        )
        self.assertNotIn(
            "arithmetic_spec::context_index(contexts@[completed]) == 0", control,
        )
        self.assertIn("arithmetic_spec::context_valid(final(contexts)@[index])", control)
        self.assertIn("contexts@.len() == old(contexts)@.len()", control)
        self.assertIn(ASSIGNMENT, control)
        self.assertIn("arithmetic_spec::context_index(*final(self)) == 0", control)

    def test_missing_or_duplicate_sensitivity_sites_fail_closed(self):
        for source in ("", self.source + ASSIGNMENT):
            with self.assertRaises(ValueError):
                mutated_reset(source, 1, 0)
        with self.assertRaises(ValueError):
            validity_only_contract(validity_only_contract(self.source))


if __name__ == "__main__":
    unittest.main()
