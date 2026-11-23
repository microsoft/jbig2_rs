from pathlib import Path
import contextlib
import io
import json
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
from source_group import source_text
from check_arithmetic_integer_proofs import CASES
from proof_sensitivity import replace_once, weakened_contract


class ArithmeticIntegerProofFixtureTests(unittest.TestCase):
    def setUp(self):
        self.source = source_text(TOOLS.parent, "src/arithmetic/integers.rs")

    def test_nine_integer_mutations_have_unique_sites(self):
        self.assertEqual(len({case["id"] for case in CASES}), 9)
        for case in CASES:
            with self.subTest(case=case["id"]):
                self.assertNotEqual(replace_once(self.source, case["site"], case["replacement"]), self.source)

    def test_weaker_controls_keep_validity_and_the_other_relations(self):
        for case in CASES:
            with self.subTest(case=case["id"]):
                changed = weakened_contract(self.source, case)
                self.assertIn("arithmetic_spec::decoder_state_valid(final(self))", changed)
                self.assertIn("*final(previous) <= 511", changed)
                self.assertIn("arithmetic_spec::context_after_decision", changed)
                self.assertIn("out.is_ok() == (arithmetic_spec::integer_bit_state", changed)

    def test_missing_ambiguous_and_repeated_controls_fail_closed(self):
        for case in CASES:
            with self.subTest(case=case["id"]):
                for invalid in ("", self.source + "\n" + case["site"]):
                    with self.assertRaises(ValueError):
                        replace_once(invalid, case["site"], case["replacement"])
                with self.assertRaises(ValueError):
                    weakened_contract(weakened_contract(self.source, case), case)

    def test_cli_returns_failure_for_a_rejected_verification(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed.json"
            arguments = [
                str(TOOLS / "check_arithmetic_integer_proofs.py"),
                "--verus-executable", str(Path(directory) / "verus"),
                "--output", str(output),
            ]
            with patch.object(sys, "argv", arguments), patch(
                "proof_sensitivity.check_semantic_cases", side_effect=ValueError("rejected control"),
            ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as stopped:
                    runpy.run_path(arguments[0], run_name="__main__")
            self.assertEqual(stopped.exception.code, 1)
            self.assertEqual(json.loads(output.read_text())["status"], "failed")
