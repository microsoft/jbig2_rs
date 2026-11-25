import contextlib
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from source_group import source_text
from check_arithmetic_codec_proofs import CASES, FIELD_CASES
from proof_sensitivity import replace_once, weakened_contract
from rust_source import parse


class ArithmeticCodecProofTests(unittest.TestCase):
    def test_field_result_controls_keep_literal_semantics_frames_and_the_other_state_component(self):
        for case in FIELD_CASES:
            if case["function"] != "HuffmanCodec::decode_integer":
                continue
            _, weaker = case["controls"][0]
            with self.subTest(case=case["id"]):
                self.assertIn("bit_read_error", weaker)
                self.assertIn("reader_valid(&old(self).reader)", weaker)
                self.assertIn("reader_valid(&final(self).reader)", weaker)
                for field in ("tables", "log_strip_size", "symbol_id_table", "refinement", "refinement_work"):
                    self.assertIn(f"final(self).{field} == old(self).{field}", weaker)
                retained = "1" if case["id"].endswith("substitutes_zero") else "0"
                removed = "0" if retained == "1" else "1"
                self.assertIn(f"huffman_table_integer_state(old(self), field).{retained}", weaker)
                self.assertNotIn(f"huffman_table_integer_state(old(self), field).{removed}", weaker)

    def test_literal_controls_retain_each_original_validity_boundary(self):
        literals = [
            case for case in CASES
            if case.get("native_test", "").startswith("symbol_text::tests::huffman_fields::")
        ]
        self.assertEqual(len(literals), 6)
        self.assertEqual(len({case["function"] for case in literals}), 5)
        for case in literals:
            with self.subTest(case=case["id"]):
                strong, weaker = case["controls"][0]
                self.assertIn("bit_read_error", strong)
                self.assertNotIn("bit_read_error", weaker)
                if case["function"].startswith("SymbolCodec::"):
                    self.assertIn("requires symbol_text_spec::codec_valid(old(self))", weaker)
                    self.assertIn("Ok(_) => symbol_text_spec::codec_valid(final(self))", weaker)
                    self.assertIn("symbol_text_spec::arithmetic_", weaker)
                else:
                    self.assertIn("reader_valid(&old(self).reader)", weaker)
                    self.assertIn("reader_valid(&final(self).reader)", weaker)

    def test_unique_sites_and_relation_specific_controls(self):
        self.assertEqual(len(CASES), 45)
        self.assertEqual(len({case["id"] for case in CASES}), 45)
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]):
                changed = replace_once(source, case["site"], case["replacement"])
                self.assertNotEqual(changed, source)
                self.assertNotEqual(weakened_contract(changed, case), changed)

    def test_literal_id_controls_retain_table_result_cursor_and_all_metadata(self):
        cases = [case for case in CASES
                 if case.get("native_test", "").startswith("symbol_text::tests::huffman_fields::")
                 and case["function"].endswith("decode_symbol_id")]
        self.assertEqual(len(cases), 2)
        for case in cases:
            weaker = case["controls"][0][1]
            with self.subTest(case=case["id"]):
                self.assertIn("symbol_id_table.is_some()", weaker)
                for field in ("tables", "log_strip_size", "symbol_id_table", "refinement", "refinement_work"):
                    self.assertIn(f".{field} ==", weaker)
                self.assertIn(
                    "huffman_symbol_id_transition" if case["function"].startswith("SymbolCodec::")
                    else "huffman_symbol_id_state",
                    weaker,
                )

    def test_table_controls_preserve_literal_contract_and_untouched_guarantees(self):
        cases = [case for case in CASES if case["id"].startswith("huffman_table_")]
        self.assertEqual(len(cases), 7)
        for case in cases:
            strong, weaker = case["controls"][0]
            with self.subTest(case=case["id"]):
                self.assertIn("bit_read_error", weaker)
                if case["function"].startswith("SymbolCodec::"):
                    self.assertIn("arithmetic_symbol_id_transition", weaker)
                else:
                    self.assertIn("reader_valid(&old(self).reader)", weaker)
                    self.assertIn("reader_valid(&final(self).reader)", weaker)
                    self.assertIn("final(self).reader.bytes@ == old(self).reader.bytes@", weaker)
                self.assertTrue(strong.startswith("#[cfg_attr"))

    def test_context_validation_controls_preserve_original_success_guarantee(self):
        for case in CASES:
            if case["function"] != "validate_probability_states":
                continue
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]):
                weaker = weakened_contract(source, case)
                self.assertIn("Ok(()) => forall|index: int|", weaker)
                self.assertIn("Err(_) => true,", weaker)
                self.assertIn("decreases contexts@.len() - index,", weaker)

    def test_mutants_preserve_ordinary_rust_syntax(self):
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]):
                changed = replace_once(source, case["site"], case["replacement"])
                self.assertFalse(parse(changed.encode()).has_error)

    def test_missing_or_duplicate_sites_fail_closed(self):
        for case in CASES:
            for source in ("", case["site"] + "\n" + case["site"]):
                with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                    replace_once(source, case["site"], case["replacement"])

    def test_missing_or_repeated_controls_fail_closed(self):
        for case in CASES:
            with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                weakened_contract("", case)
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                weakened_contract(weakened_contract(source, case), case)

    def test_cli_propagates_rejected_verification_as_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed.json"
            arguments = [
                str(TOOLS / "check_arithmetic_codec_proofs.py"),
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
