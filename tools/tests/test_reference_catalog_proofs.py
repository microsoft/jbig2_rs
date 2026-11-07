from pathlib import Path
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

from check_contract_mutations import MUTATIONS
from check_reference_catalog_proofs import CASES, CASE_GROUPS, FILES, ROOT, baseline_groups
from proof_sensitivity import replace_once, weakened_contract
from source_group import SourceGroup, bound_source_groups, relocate_cases
import run_semantic_proofs as runner


class ReferenceCatalogProofTests(unittest.TestCase):
    def test_new_groups_use_only_existing_shard_ten_and_twelve_headroom(self):
        added = set(CASE_GROUPS)
        current = tuple(row for row in runner.CHECKS if row[0] not in {
            "reference-type-lookup", "reference-type-classes",
        })
        old_checks = tuple(row for row in current if row[0] not in added)
        moved = {
            "retention": 3, "default-limits-3": 8, "collector-sink": 11, "mmr-span": 13,
            "arithmetic-initialization": 7, "default-decoder-4": 4,
            "default-decoder-3": 5,
        }
        previous = {**runner.DEFAULT_SHARD_REBALANCE, **moved}
        with patch.object(runner, "CHECKS", old_checks), patch.object(
            runner, "DEFAULT_SHARD_REBALANCE", previous,
        ):
            before = (
                runner.configured_whole_checks(), runner.configured_collector_checks(),
                *runner.configured_shards(),
            )
        with patch.object(runner, "CHECKS", current):
            after = (
                runner.configured_whole_checks(), runner.configured_collector_checks(),
                *runner.configured_shards(),
            )
        self.assertEqual(len(after), 16)
        for old, current in zip(before, after):
            self.assertEqual(
                tuple(row for row in old if row[0] not in moved),
                tuple(row for row in current if row[0] not in added | moved.keys()),
            )
        self.assertIn("reference-accounting", {row[0] for row in after[12]})
        self.assertIn("reference-catalog", {row[0] for row in after[14]})
        for name, shard in {
            "retention": 5, "default-limits-3": 7, "collector-sink": 9, "mmr-span": 1,
            "arithmetic-initialization": 6, "default-decoder-4": 12,
            "default-decoder-3": 8,
        }.items():
            self.assertIn(name, {row[0] for row in after[shard + 2]})

    def test_twelve_actual_scoped_sites_have_native_and_weaker_controls(self):
        self.assertEqual({name: len(cases) for name, cases in CASE_GROUPS.items()}, {
            "reference-accounting": 6, "reference-catalog": 6,
        })
        self.assertEqual(len(CASES), 12)
        for case in CASES:
            with self.subTest(case=case["id"]):
                group = SourceGroup(ROOT, case["file"])
                mutant = replace_once(group.text, case["site"], case["replacement"])
                weaker = weakened_contract(mutant, case)
                self.assertNotEqual(mutant, group.text)
                self.assertNotEqual(weaker, mutant)
                self.assertEqual(sum(
                    row["id"] == "reference-catalog-" + case["id"] for row in MUTATIONS
                ), 1)

    def test_groups_bind_real_catalog_models_and_private_field_bridges(self):
        _, sources, _ = bound_source_groups(ROOT, FILES)
        for file in ("specs/document/catalogs.rs", "proofs/model_support/document.rs"):
            self.assertEqual(sources[file], (ROOT / file).read_bytes())
        for name, cases in CASE_GROUPS.items():
            with self.subTest(group=name):
                relocated, _ = relocate_cases(ROOT, cases, baseline_groups(cases), sources)
                self.assertEqual(len(relocated), 6)

    def test_weaker_controls_keep_existing_domains_frames_and_global_policy(self):
        for case in CASES:
            weaker = weakened_contract(case["replacement"], case)
            if case["function"] in {"validate_reference_depth", "validate_reference_depth_ordered"}:
                self.assertIn("requires", weaker)
                self.assertIn("indexes@[entry].1 < segments@.len()", weaker)
            if case["function"] == "validate_reference_depth_ordered":
                self.assertIn("final(depths)@[entry] == old(depths)@[entry]", weaker)
                self.assertIn("limits.max_reference_graph_depth", weaker)
            if case["function"] == "keyed_index":
                self.assertIn("entries@[index as int].0 == number", weaker)
                self.assertIn("None =>", weaker)
            if case["function"] == "Decoder::parse_globals":
                self.assertIn("document_spec::global_parse_result(", weaker)
                self.assertNotIn("== document_spec::global_depth_count(&globals)", weaker)


if __name__ == "__main__":
    unittest.main()
