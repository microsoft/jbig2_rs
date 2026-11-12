from pathlib import Path
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_contract_mutations import MUTATIONS
from check_reference_type_proofs import CASES, CASE_GROUPS, FILES, ROOT, baseline_groups
from proof_sensitivity import replace_once, weakened_contract
from source_group import SourceGroup, bound_source_groups, relocate_cases
import run_semantic_proofs as runner


class ReferenceTypeProofTests(unittest.TestCase):
    def test_all_twelve_exact_sites_have_actual_native_and_selective_weaker_recipes(self):
        self.assertEqual({name: len(cases) for name, cases in CASE_GROUPS.items()}, {
            "reference-type-lookup": 6, "reference-type-classes": 6,
        })
        self.assertEqual(len({case["id"] for case in CASES}), 12)
        for case in CASES:
            with self.subTest(case=case["id"]):
                source = SourceGroup(ROOT, case["file"])
                mutant = replace_once(source.text, case["site"], case["replacement"])
                weaker = weakened_contract(mutant, case)
                self.assertNotEqual(mutant, source.text)
                self.assertNotEqual(mutant, weaker)
                self.assertIn("requires", weaker)
                self.assertIn("indexes@", weaker)
                self.assertEqual(sum(
                    mutation["id"] == "reference-type-" + case["id"] for mutation in MUTATIONS
                ), 1)

    def test_real_scope_and_private_global_projection_are_source_bound(self):
        _, sources, _ = bound_source_groups(ROOT, FILES)
        self.assertEqual(sources["specs/document/reference_types.rs"],
                         (ROOT / "specs/document/reference_types.rs").read_bytes())
        for group, cases in CASE_GROUPS.items():
            with self.subTest(group=group):
                relocated, _ = relocate_cases(ROOT, cases, baseline_groups(cases), sources)
                self.assertEqual(len(relocated), 6)

    def test_new_groups_do_not_redistribute_any_old_case_or_add_roles(self):
        added = set(CASE_GROUPS)
        with patch.object(runner, "CHECKS", tuple(row for row in runner.CHECKS if row[0] not in added)):
            before = (runner.configured_whole_checks(), runner.configured_collector_checks(),
                      *runner.configured_shards())
        after = (runner.configured_whole_checks(), runner.configured_collector_checks(),
                 *runner.configured_shards())
        self.assertEqual(len(after), 16)
        for old, current in zip(before, after):
            self.assertEqual(old, tuple(row for row in current if row[0] not in added))
        self.assertIn("reference-type-lookup", {row[0] for row in after[12]})
        self.assertIn("reference-type-classes", {row[0] for row in after[15]})

    @patch.dict(runner.ADDED_SHARD_GROUPS, {"reference-type-lookup": 3})
    def test_complete_original_timing_profile_rejects_first_assignment_and_fits_corrected_one(self):
        # Same-run42310047778/attempt1 passed all61groups; seconds rounded up.
        costs = {
            "arithmetic-codec": 719.378, "clone-results": 252.298, "context-callers": 175.058,
            "arithmetic-decision": 336.722, "reset": 67.227,
            "mmr-run": 135.701, "mmr-reference": 338.320, "default-limits-1": 105.424,
            "reader-error": 251.590, "document-count": 311.917, "default-decoder-2": 92.852,
            "iaid-state": 207.335, "mmr-span": 207.626, "default-limits-2": 103.200,
            "error-wrappers-symbol": 218.833, "symbol-cardinality": 572.860,
            "reference-type-lookup": 128.358, "arithmetic-refill": 125.288,
            "page-state": 261.596, "huffman-codec": 261.506, "public-decode-results": 208.263,
            "public-global-results": 122.834, "text-batch": 278.158, "arithmetic-integer": 221.958,
            "retention": 86.771, "error-wrappers-regions": 199.346, "document-layout": 281.306,
            "halftone-header": 204.643, "mmr-horizontal": 218.879, "black-span": 202.371,
            "segment-data": 296.206, "arithmetic-initialization": 184.928,
            "public-context": 281.848, "error-wrappers-basic": 270.086, "bitmap-copy": 232.589,
            "default-limits-3": 107.060, "mmr-prefix": 111.237, "bitmap-chunk": 107.701,
            "default-decoder-3": 88.271, "bitmap-compose": 170.999, "context-lifecycle": 273.073,
            "huffman-assignment": 867.915, "collector-sink": 82.433,
            "reference-accounting": 172.684, "segment-header": 269.749, "region-dispatch": 104.134,
            "integer-state": 266.336, "reference-type-classes": 187.887, "operator-bit": 130.749,
            "error-conversion": 284.341, "value-bridge": 480.106,
            "reference-catalog": 153.090, "default-decoder-4": 89.550, "page-public": 213.135,
            "default-decoder-1": 89.200, "default-decoder-5": 89.180,
            "arithmetic-normalization": 148.680, "session-framing": 332.845,
            "default-limits-4": 108.449, "page-finish": 208.850, "default-limits-5": 106.371,
        }
        self.assertEqual(set(costs), {row[0] for row in runner.CHECKS})
        rows = [{"id": name, "status": "passed", "seconds": seconds}
                for name, seconds in costs.items()]
        with patch.dict(runner.ADDED_SHARD_GROUPS, {"reference-type-classes": 11}):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)
        measured = runner.measured_shard_seconds(rows)
        self.assertEqual(len(measured), 14)
        self.assertLess(max(measured), 1000)


if __name__ == "__main__":
    unittest.main()
