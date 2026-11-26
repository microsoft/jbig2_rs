import contextlib
import io
import json
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_owned_decode_proofs import (
    BANK_RESULT, CASES, DECODER_ONLY, EXPORT_CONTENTS, EXPORT_COUNT, ROOT, contract, export_target,
)
from proof_sensitivity import replace_once, weakened_contract
from source_group import bound_source_groups, relocate_cases, runtime_function, source_text


class OwnedDecodeProofTests(unittest.TestCase):
    def test_two_real_mutations_have_native_oracles_and_relocate_to_their_bodies(self):
        self.assertEqual(len(CASES), 2)
        self.assertEqual(len({case["id"] for case in CASES}), 2)
        files = tuple(case["file"] for case in CASES)
        _, sources, _ = bound_source_groups(ROOT, files)
        relocated, groups = relocate_cases(ROOT, CASES, (), sources)
        self.assertEqual(groups, ())
        self.assertEqual(
            [case["verification_module"] for case in relocated],
            ["halftone", "symbol_text::dictionary::exports"],
        )
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = source_text(ROOT, case["file"])
                mutant = replace_once(source, case["site"], case["replacement"])
                weaker = weakened_contract(mutant, case)
                self.assertNotEqual(mutant, source)
                self.assertNotEqual(weaker, mutant)
                self.assertTrue(case["native_test"])

    def test_probability_control_changes_only_constructor_bank_guarantee(self):
        case = CASES[0]
        original, weaker = case["controls"][0]
        self.assertEqual(weaker, replace_once(original, BANK_RESULT, DECODER_ONLY))
        self.assertIn("arithmetic_spec::decoder_valid(&session.decoder)", weaker)
        self.assertIn("matches!(state, GenericRegionState::Arithmetic { .. })", weaker)
        self.assertIn("Err(_) => true", weaker)
        self.assertIn("packed: 94", case["replacement"])
        self.assertIn("if !session.contexts.contexts.is_empty()", case["replacement"])
        self.assertIn("owned_arithmetic_planes_match_checked_banks", case["native_test"])

    def test_identity_control_keeps_storage_inputs_and_bounds_but_weakens_result_relation(self):
        case = CASES[1]
        original, weaker = case["controls"][0]
        self.assertEqual(
            weaker, replace_once(replace_once(original, EXPORT_COUNT, "true"), EXPORT_CONTENTS, ""),
        )
        self.assertIn("symbol_text_spec::bitmaps_valid(exported@)", weaker)
        self.assertIn("symbol_text_spec::codebook_valid(imported)", weaker)
        self.assertIn("symbol_text_spec::bitmaps_valid(new_symbols@)", weaker)
        self.assertIn("forall|index: int|", weaker)
        self.assertNotIn("symbol_text_spec::exported_contents_match(", weaker)
        self.assertIn("Ok(Vec::new())", case["replacement"])
        self.assertIn("owned_identity_exports_transfer_vector", case["native_test"])

    def test_probability_control_does_not_weaken_either_calling_constructor(self):
        source = source_text(ROOT, CASES[0]["file"])
        weaker = weakened_contract(source, CASES[0])
        for name in ("new_pattern_arithmetic_state", "new_halftone_arithmetic_state"):
            with self.subTest(name=name):
                original = contract(runtime_function(ROOT, "halftone", name))
                self.assertIn(original, source)
                self.assertIn(original, weaker)

    def test_missing_ambiguous_and_repeated_sites_or_controls_fail_closed(self):
        for case in CASES:
            source = source_text(ROOT, case["file"])
            with self.subTest(case=case["id"]):
                for bad in ("", case["site"] + "\n" + case["site"]):
                    with self.assertRaises(ValueError):
                        replace_once(bad, case["site"], case["replacement"])
                with self.assertRaises(ValueError):
                    weakened_contract("", case)
                with self.assertRaises(ValueError):
                    weakened_contract(weakened_contract(source, case), case)

    def test_export_name_alias_requires_one_actual_helper(self):
        row = dict(name="select_exports_owned", owner=None,
                   file_module="symbol_text::dictionary::exports")
        for rows in ([], [row, {**row, "name": "assemble_owned_exported_symbols"}]):
            with self.subTest(rows=rows), patch(
                "check_owned_decode_proofs.source_functions", return_value=rows,
            ), self.assertRaises(ValueError):
                export_target(ROOT)
        with patch("check_owned_decode_proofs.source_functions", return_value=[row]):
            self.assertEqual(export_target(ROOT), row)

    def test_cli_failure_is_nonzero_without_writing_a_report_file(self):
        output = ROOT / "target/owned-decode-proof-test/failed.json"
        arguments = [
            str(TOOLS / "check_owned_decode_proofs.py"),
            "--verus-executable", str(ROOT / "target/missing-verus"),
            "--output", str(output),
        ]
        with patch.object(sys, "argv", arguments), patch(
            "proof_sensitivity.check_semantic_cases", side_effect=ValueError("rejected control"),
        ), patch("pathlib.Path.mkdir"), patch("pathlib.Path.write_text") as write, \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as stopped:
                runpy.run_path(arguments[0], run_name="__main__")
        self.assertEqual(stopped.exception.code, 1)
        self.assertEqual(json.loads(write.call_args.args[0])["status"], "failed")


if __name__ == "__main__":
    unittest.main()
