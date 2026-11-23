import json
import math
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import run_semantic_proofs as runner


TIMINGS = json.loads(
    (Path(__file__).parent / "fixtures/colour-rgb-semantic-timings.json").read_text()
)
EXPECTED_BOUNDS = (
    1037.521383123, 1040.108429994, 1040.703310944, 1030.325156962,
    1037.335416414, 1039.711085949, 1039.446731117, 1043.280104190,
    1038.452690862, 959.121906907, 1042.168769594, 1043.547973186,
    1036.347691940, 1041.178278700,
)


def timing_rows(seconds):
    return [
        {"id": row[0], "status": "passed", "seconds": seconds[row[0]]}
        for row in runner.configured_checks()
    ]


def maximum_seconds():
    profiles = [row["seconds"] for row in TIMINGS["profiles"].values()]
    return {name: max(profile[name] for profile in profiles) for name in profiles[0]}


def reserved_bounds(seconds):
    bounds = runner.measured_shard_seconds(timing_rows(seconds))
    bounds[runner.COLOUR_SHARD_ALLOCATION["arithmetic-certainty"]] += (
        TIMINGS["certainty"]["budget_seconds"]
    )
    return bounds


class ColourShardSchedulingTests(unittest.TestCase):
    def test_complete_actual_profiles_keep_every_group_and_rejected_timing(self):
        expected = {row[0] for row in runner.CHECKS}
        self.assertEqual(set(TIMINGS["profiles"]), {"45092228281/1", "45102932751/1"})
        self.assertEqual(len(expected), 62)
        self.assertEqual(sum(row[2] for row in runner.CHECKS), 499)
        self.assertNotIn("arithmetic-certainty", expected)
        for profile in TIMINGS["profiles"].values():
            self.assertEqual(set(profile["seconds"]), expected)
            self.assertTrue(all(
                type(value) in (int, float) and math.isfinite(value) and value > 0
                for value in profile["seconds"].values()
            ))
            self.assertGreater(profile["rejected_max_shard_seconds"], 1050)
        self.assertAlmostEqual(
            TIMINGS["profiles"]["45092228281/1"]["rejected_max_shard_seconds"],
            1130.134626096, places=8,
        )
        self.assertAlmostEqual(
            TIMINGS["profiles"]["45102932751/1"]["rejected_max_shard_seconds"],
            1261.259997126, places=8,
        )

    def test_maxima_and_reserved_headroom_fit_the_unchanged_actual_data_guard(self):
        measured = TIMINGS["certainty"]["measured_seconds"]
        budget = TIMINGS["certainty"]["budget_seconds"]
        self.assertEqual(measured, 264.47)
        self.assertEqual(budget, 320)
        self.assertGreaterEqual(budget, 1.2 * measured)
        seconds = maximum_seconds()
        excluded = set(runner.WHOLE_GROUPS) | set(runner.COLLECTOR_GROUPS)
        actual_total = math.fsum(
            value for name, value in seconds.items() if name not in excluded
        )
        self.assertAlmostEqual(actual_total, 14149.248929882, places=8)
        bounds = reserved_bounds(seconds)
        self.assertEqual(len(bounds), 14)
        for actual, expected in zip(bounds, EXPECTED_BOUNDS):
            self.assertAlmostEqual(actual, expected, places=7)
        self.assertLess(max(bounds), 1050)
        for profile in TIMINGS["profiles"].values():
            self.assertLess(max(reserved_bounds(profile["seconds"])), 1050)
        with patch.object(runner, "COLOUR_SHARD_ALLOCATION", {}):
            for profile in TIMINGS["profiles"].values():
                with self.assertRaisesRegex(ValueError, "1050-second guard"):
                    runner.measured_shard_seconds(timing_rows(profile["seconds"]))

    def test_every_membership_role_and_order_is_preserved_once(self):
        checks = runner.configured_checks()
        self.assertEqual(len(checks), 62)
        self.assertEqual(sum(row[2] for row in checks), 499)
        self.assertEqual(runner.CI_SHARDS, 14)
        self.assertEqual(runner.WHOLE_GROUPS, ("arithmetic-codec",))
        self.assertEqual(
            runner.COLLECTOR_GROUPS,
            ("arithmetic-decision", "reset", "clone-results", "context-callers"),
        )
        excluded = set(runner.WHOLE_GROUPS) | set(runner.COLLECTOR_GROUPS)
        sharded = tuple(row for row in checks if row[0] not in excluded)
        self.assertEqual(
            set(runner.COLOUR_SHARD_ALLOCATION),
            {row[0] for row in sharded} | {"arithmetic-certainty"},
        )
        shards = runner.configured_shards()
        self.assertEqual(len(shards), 14)
        self.assertTrue(all(shards))
        assigned = [row for shard in shards for row in shard]
        self.assertEqual(len(assigned), len(sharded))
        self.assertEqual(set(assigned), set(sharded))
        for index, shard in enumerate(shards):
            self.assertEqual(shard, tuple(row for row in checks if row in shard))
            for row in shard:
                self.assertEqual(runner.COLOUR_SHARD_ALLOCATION[row[0]], index)

    def test_pre_rgb_scopes_restore_exact_legacy_slots_and_order(self):
        checks = tuple(row for row in runner.CHECKS if row[0] != "colour-rgb")
        self.assertEqual(len(checks), 61)
        self.assertEqual(sum(row[2] for row in checks), 494)
        self.assertEqual(runner.DEFAULT_SHARD_REBALANCE["default-limits-2"], 2)
        self.assertEqual(runner.ADDED_SHARD_GROUPS["public-global-results"], 4)
        with patch.object(runner, "CHECKS", checks):
            before = runner.configured_shards()
            self.assertIn("default-limits-2", {row[0] for row in before[2]})
            self.assertIn("public-global-results", {row[0] for row in before[4]})
            with patch.object(
                runner, "COLOUR_SHARD_ALLOCATION", {row[0]: 13 for row in checks},
            ):
                self.assertEqual(runner.configured_shards(), before)
        self.assertEqual(len(before), 14)

    def test_insufficient_headroom_still_fails_the_original_guard(self):
        with self.assertRaisesRegex(ValueError, "1050-second guard"):
            runner.measured_shard_seconds(timing_rows({
                **maximum_seconds(), "huffman-assignment": 1050.001,
            }))


if __name__ == "__main__":
    unittest.main()
