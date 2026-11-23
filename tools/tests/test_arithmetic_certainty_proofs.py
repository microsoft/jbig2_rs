from pathlib import Path
import sys
import unittest


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from check_arithmetic_certainty_proofs import CASES
from check_contract_mutations import MUTATIONS
from proof_sensitivity import replace_once, weakened_contract
from source_group import relocate_cases, source_text
from rust_tests import test_inventory


class ArithmeticCertaintyProofFixtureTests(unittest.TestCase):
    def test_four_unique_real_mutations_and_native_controls_are_registered(self):
        self.assertEqual(len(CASES), 4)
        self.assertEqual(len({case["id"] for case in CASES}), 4)
        self.assertEqual(len({case["function"] for case in CASES}), 4)
        tests = test_inventory(TOOLS.parent)
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = source_text(TOOLS.parent, case["file"])
                mutant = replace_once(source, case["site"], case["replacement"])
                weaker = weakened_contract(mutant, case)
                self.assertNotEqual(source, mutant)
                self.assertNotEqual(mutant, weaker)
                registered = [
                    row for row in MUTATIONS
                    if row["id"] == "arithmetic-certainty-" + case["id"]
                ]
                self.assertEqual(len(registered), 1)
                self.assertEqual(registered[0]["before"], case["site"])
                self.assertEqual(registered[0]["after"], case["replacement"])
                self.assertEqual(registered[0]["test"], case["native_test"])
                self.assertIn(("lib", case["native_test"]), tests)
                self.assertFalse(tests[("lib", case["native_test"])]["ignored"])

    def test_weaker_controls_keep_safety_frames_and_success_validity(self):
        remaining = {
            "initial_terminal_prefix_contribution_discarded": (
                "out.spread <= decoder.code", "out.entropy_end == decoder.bytes@.len() - 2",
                "certainty_spec::observer_start(&observer, decoder)",
            ),
            "threshold_crossing_falsely_certified": (
                "arithmetic_spec::decoder_valid(before)",
                "final(self).saved == old(self).saved",
                "self.spread <= frame.code",
            ),
            "inactive_trace_reported_as_saved_prefix": (
                "certainty_spec::observer_valid(final(self), decoder)",
                "final(self).initial == old(self).initial",
                "certainty_spec::observer_save(&prior, self, decoder, instances)",
            ),
            "final_ds_error_reclassified": (
                "out.is_ok() ==> symbol_text_spec::arithmetic_codec_valid(final(self))",
                "out == Ok(true) ==> symbol_text_spec::arithmetic_end_certificate",
                "Ok(EntropyInteger::OutOfBand) => out == Ok(false)",
            ),
        }
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = source_text(TOOLS.parent, case["file"])
                weaker = weakened_contract(
                    replace_once(source, case["site"], case["replacement"]), case,
                )
                for clause in remaining[case["id"]]:
                    self.assertIn(clause, weaker)

    def test_real_owner_and_execution_scopes_remain_distinct(self):
        prepared, _ = relocate_cases(
            TOOLS.parent, CASES, (),
            tuple(dict.fromkeys(case["file"] for case in CASES)),
        )
        error = next(case for case in prepared if case["id"] == "final_ds_error_reclassified")
        self.assertEqual(error["module"], "symbol_text")
        self.assertEqual(error["verification_module"], "symbol_text::codec::arithmetic")
        for case in prepared:
            self.assertEqual(case["file"], next(
                original["file"] for original in CASES if original["id"] == case["id"]
            ))

    def test_missing_ambiguous_and_repeated_controls_fail_closed(self):
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]):
                for invalid in ("", source + "\n" + case["site"]):
                    with self.assertRaises(ValueError):
                        replace_once(invalid, case["site"], case["replacement"])
                with self.assertRaises(ValueError):
                    weakened_contract(weakened_contract(source, case), case)


if __name__ == "__main__":
    unittest.main()
