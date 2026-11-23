import contextlib
import io
import json
import runpy
import tempfile
import unittest
from pathlib import Path
import sys
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from source_group import source_text
from check_integer_state_proofs import CASES
from proof_sensitivity import replace_once, weakened_contract


class IntegerStateProofTests(unittest.TestCase):
    def test_all_cases_have_unique_production_sites_and_related_controls(self):
        source = source_text(TOOLS.parent, "src/arithmetic/integers.rs")
        self.assertEqual(len(CASES), 9)
        self.assertEqual(len({case["id"] for case in CASES}), 9)
        for case in CASES:
            with self.subTest(case=case["id"]):
                changed = replace_once(source, case["site"], case["replacement"])
                self.assertNotEqual(changed, source)
                self.assertNotEqual(weakened_contract(changed, case), changed)
                self.assertIn("arithmetic_spec::decoder_state_valid(final(self))", changed)

    def test_missing_or_duplicate_sites_fail_closed(self):
        for case in CASES:
            for source in ("", case["site"] + "\n" + case["site"]):
                with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                    replace_once(source, case["site"], case["replacement"])

    def test_missing_control_fails_closed(self):
        for case in CASES:
            with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                weakened_contract("", case)

    def test_cli_propagates_rejected_verification_as_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed.json"
            arguments = [
                str(TOOLS / "check_integer_state_proofs.py"),
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


if __name__ == "__main__":
    unittest.main()
