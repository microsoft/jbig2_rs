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
from check_symbol_cardinality_proofs import CASES
from proof_sensitivity import replace_once, weakened_contract


class SymbolCardinalityProofTests(unittest.TestCase):
    def test_segment_controls_retain_original_output_consumption_and_input_contracts(self):
        segments = [case for case in CASES if "_segment_" in case["id"]]
        self.assertEqual(len(segments), 4)
        self.assertTrue(all("native_test" in case for case in segments))
        for case in segments:
            source = source_text(TOOLS.parent, case["file"])
            weaker = weakened_contract(source, case)
            with self.subTest(case=case["id"]):
                self.assertIn("symbol_text_spec::codebook_valid(input.", weaker)
                self.assertIn("output.consumed_bytes <= input.payload@.len()", weaker)
                self.assertNotIn("production_segment_symbol_width!(", weaker)
                self.assertIn("let code_length = symbol_id_code_length(", weaker)

    def test_caller_controls_retain_count_invariants_and_resource_storage_contracts(self):
        callers = [case for case in CASES if "_id_request_changes_" in case["id"]]
        self.assertEqual(len(callers), 6)
        self.assertEqual(sum("native_test" in case for case in callers), 4)
        for case in callers:
            source = source_text(TOOLS.parent, case["file"])
            weaker = weakened_contract(source, case)
            with self.subTest(case=case["id"]):
                self.assertIn("symbol_text_spec::codec_valid(old(codec))", weaker)
                self.assertIn("crate::spec::bitmap::storage_valid(&bitmap)", weaker)
                self.assertIn("symbol_text_spec::work_units", weaker)
                if case["function"] == "decode_text_region_body_with_end":
                    self.assertIn(
                        "available_count as nat == symbol_text_spec::codebook_len(available_symbols)",
                        weaker,
                    )
                else:
                    self.assertIn("equal(value as u128, count as u128)", weaker)

    def test_export_order_control_keeps_count_bounds_and_resource_frame(self):
        case = next(case for case in CASES if case["function"] == "decode_export_runs")
        source = source_text(TOOLS.parent, case["file"])
        changed = replace_once(source, case["site"], case["replacement"])
        weaker = weakened_contract(changed, case)
        self.assertEqual(source.count("forall|left: int, right: int|"), 3)
        self.assertNotIn("forall|left: int, right: int|", weaker)
        self.assertIn("exported@.len() == expected_exports", weaker)
        self.assertIn("symbol_text_spec::export_indices_valid(", weaker)
        self.assertIn("exported@[offset] < index", weaker)
        self.assertIn("exported@[offset] < candidate", weaker)
        self.assertIn("symbol_text_spec::bitmap_bytes(final(resources))", weaker)
        self.assertIn("exported.push(index);", weaker)

    def test_unique_mutations_retain_existing_storage_and_lookup_bounds(self):
        self.assertEqual(len(CASES), 22)
        self.assertEqual(len({case["id"] for case in CASES}), 22)
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = source_text(TOOLS.parent, case["file"])
                changed = replace_once(source, case["site"], case["replacement"])
                weaker = weakened_contract(changed, case)
                self.assertNotEqual(source, changed)
                self.assertNotEqual(changed, weaker)
                if "_segment_" in case["id"]:
                    self.assertIn("storage_valid(&output.bitmap)" if case["function"] == "decode_text_region"
                                  else "bitmaps_valid(output.exported_symbols@)", weaker)
                    self.assertNotIn("production_segment_symbol_width!(", weaker)
                    self.assertIn("output.consumed_bytes <= input.payload@.len()", weaker)
                elif "_id_request_changes_" in case["id"]:
                    self.assertIn("symbol_text_spec::codec_valid(old(codec))", weaker)
                    self.assertIn("crate::spec::bitmap::storage_valid(&bitmap)", weaker)
                    self.assertNotIn("production_symbol_id_request!(", weaker)
                    self.assertIn("let id_request = SymbolIdRequest", weaker)
                elif case["module"] == "decode":
                    self.assertIn("decode_spec::collected_symbols_valid(&bitmaps)", weaker)
                    self.assertIn("decode_spec::symbol_store_valid(symbols)", weaker)
                    self.assertIn("index < symbols@.len()", weaker)
                    self.assertIn("crate::symbol_text::symbol_text_spec::codebook_valid(out)", weaker)
                else:
                    self.assertIn("symbol_text_spec::codebook_valid(imported)", weaker)
                    self.assertIn("symbol_text_spec::bitmaps_valid(new_symbols@)", weaker)
                    self.assertIn("symbol_text_spec::bitmaps_valid(exported@)", weaker)
                    self.assertIn("exported@.len() == exported_indices@.len()", weaker)
                    self.assertIn("None => index as nat >= symbol_text_spec::codebook_len(self)", weaker)

    def test_missing_or_duplicate_sites_fail_closed(self):
        for case in CASES:
            for source in ("", case["site"] + "\n" + case["site"]):
                with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                    replace_once(source, case["site"], case["replacement"])

    def test_missing_or_repeated_controls_fail_closed(self):
        for case in CASES:
            source = source_text(TOOLS.parent, case["file"])
            with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                weakened_contract("", case)
            with self.subTest(case=case["id"]), self.assertRaises(ValueError):
                weakened_contract(weakened_contract(source, case), case)

    def test_cli_failure_is_nonzero(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "failed.json"
            arguments = [
                str(TOOLS / "check_symbol_cardinality_proofs.py"),
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
