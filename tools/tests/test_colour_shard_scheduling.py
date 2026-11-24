import json
import math
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import run_semantic_proofs as runner


CURRENT_CHECKS = runner.CHECKS
COLOUR_CERTAINTY_CHECKS = tuple(
    row for row in CURRENT_CHECKS if row[0] not in {"owned-decode"}
)
TIMINGS = json.loads(
    (Path(__file__).parent / "fixtures/colour-rgb-semantic-timings.json").read_text()
)
OWNED_REJECTED = json.loads(
    (Path(__file__).parent / "fixtures/owned-decode-rejected-shard-timings.json").read_text()
)
OWNED_COMPLETE_REJECTED = json.loads(
    (Path(__file__).parent / "fixtures/owned-decode-complete-rejected-profile.json").read_text()
)
PORTABLE_REJECTED = json.loads(
    (Path(__file__).parent / "fixtures/portable-qualification-rejected-profile.json").read_text()
)
PORTABLE_BALANCED_REJECTED = json.loads(
    (Path(__file__).parent / "fixtures/portable-balanced-rejected-profile.json").read_text()
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
    result = {name: max(profile[name] for profile in profiles) for name in profiles[0]}
    result["arithmetic-certainty"] = TIMINGS["certainty"]["budget_seconds"]
    return result


def configured_roles():
    return {
        "whole": runner.configured_whole_checks(),
        "collector": runner.configured_collector_checks(),
        **{f"shard-{index}": shard for index, shard in enumerate(runner.configured_shards())},
    }


class RobustPortableShardSchedulingTests(unittest.TestCase):
    @patch.object(runner, "CHECKS", CURRENT_CHECKS)
    def test_complete_rejection_is_preserved_and_only_whole_groups_are_rebalanced(self):
        profile = PORTABLE_BALANCED_REJECTED
        self.assertEqual(profile["run_identity"], {
            "GITHUB_REPOSITORY": "microsoft/jbig2_rs",
            "GITHUB_RUN_ID": "45391179475",
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_SHA": "eb1a5dc7bb9a84b199803f501fec8f95b3029b51",
        })
        self.assertTrue(profile["all_groups_passed"])
        self.assertEqual(profile["counterexamples"], 505)
        seconds = profile["seconds"]
        self.assertEqual(set(seconds), {row[0] for row in runner.configured_checks()})
        with patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.PORTABLE_SHARD_ALLOCATION,
        ):
            previous = configured_roles()
            self.assertEqual(
                {role: [row[0] for row in groups] for role, groups in previous.items()},
                profile["role_groups"],
            )
            for role, groups in previous.items():
                self.assertAlmostEqual(
                    math.fsum(seconds[row[0]] for row in groups),
                    profile["role_seconds"][role], places=9,
                )
            self.assertAlmostEqual(profile["role_seconds"]["shard-4"], 1063.714668023, places=9)
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(timing_rows(seconds))
        current = configured_roles()
        moved = {
            "reference-catalog", "mmr-span", "bitmap-copy", "reader-error",
            "public-decode-results", "black-span", "error-wrappers-basic",
        }
        for role in previous:
            self.assertEqual(
                [row for row in previous[role] if row[0] not in moved],
                [row for row in current[role] if row[0] not in moved],
            )
        self.assertEqual(current["whole"], previous["whole"])
        self.assertEqual(current["collector"], previous["collector"])
        assigned = [row for groups in current.values() for row in groups]
        self.assertEqual(len(assigned), 64)
        self.assertEqual(set(assigned), set(runner.configured_checks()))
        self.assertEqual(sum(row[2] for row in assigned), 505)
        projected = runner.measured_shard_seconds(timing_rows(seconds))
        self.assertAlmostEqual(max(projected), 1019.920750152, places=9)
        self.assertEqual(len(projected), 14)

    @patch.object(runner, "CHECKS", CURRENT_CHECKS)
    def test_complete_component_envelope_keeps_all_adverse_observations(self):
        seconds = dict(OWNED_COMPLETE_REJECTED["seconds"])
        for rows in OWNED_REJECTED["shards"].values():
            for row in rows:
                seconds[row["id"]] = max(seconds[row["id"]], row["seconds"])
        for source in (
            PORTABLE_REJECTED["prior_accepted"]["seconds"],
            PORTABLE_REJECTED["seconds"], PORTABLE_BALANCED_REJECTED["seconds"],
        ):
            self.assertEqual(set(source), set(seconds))
            for name, elapsed in source.items():
                seconds[name] = max(seconds[name], elapsed)
        projected = runner.measured_shard_seconds(timing_rows(seconds))
        self.assertAlmostEqual(max(projected), 1025.695969442, places=9)
        collector = math.fsum(seconds[row[0]] for row in runner.configured_collector_checks())
        self.assertAlmostEqual(collector, 1049.066534463, places=9)
        with patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.PORTABLE_SHARD_ALLOCATION,
        ), self.assertRaisesRegex(ValueError, "1050-second guard"):
            runner.measured_shard_seconds(timing_rows(seconds))


class PortableShardSchedulingTests(unittest.TestCase):
    @patch.object(runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.PORTABLE_SHARD_ALLOCATION)
    @patch.object(runner, "CHECKS", CURRENT_CHECKS)
    def test_complete_portable_rejection_moves_only_one_complete_group(self):
        profile = PORTABLE_REJECTED
        self.assertEqual(profile["run_identity"], {
            "GITHUB_REPOSITORY": "microsoft/jbig2_rs",
            "GITHUB_RUN_ID": "45375474932",
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_SHA": "f329ffc076846491a711c18befa3db567d7dccc9",
        })
        self.assertTrue(profile["all_groups_passed"])
        self.assertEqual(profile["counterexamples"], 505)
        seconds = profile["seconds"]
        self.assertEqual(set(seconds), {row[0] for row in runner.configured_checks()})
        with patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.OWNED_BALANCED_SHARD_ALLOCATION,
        ):
            previous = configured_roles()
            self.assertEqual(
                {name: [row[0] for row in groups] for name, groups in previous.items()},
                profile["role_groups"],
            )
            for name, groups in previous.items():
                self.assertAlmostEqual(
                    math.fsum(seconds[row[0]] for row in groups),
                    profile["role_seconds"][name], places=9,
                )
            self.assertAlmostEqual(profile["role_seconds"]["shard-13"], 1126.437413592, places=9)
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(timing_rows(seconds))
        current = configured_roles()
        moved = "default-limits-3"
        self.assertEqual(runner.PORTABLE_SHARD_ALLOCATION, {
            **runner.OWNED_BALANCED_SHARD_ALLOCATION, moved: 4,
        })
        self.assertEqual(runner.OWNED_BALANCED_SHARD_ALLOCATION[moved], 13)
        for name in previous:
            self.assertEqual(
                [row for row in previous[name] if row[0] != moved],
                [row for row in current[name] if row[0] != moved],
            )
        self.assertIn(moved, [row[0] for row in current["shard-4"]])
        self.assertNotIn(moved, [row[0] for row in current["shard-13"]])
        self.assertEqual(current["whole"], previous["whole"])
        self.assertEqual(current["collector"], previous["collector"])
        projected = runner.measured_shard_seconds(timing_rows(seconds))
        self.assertAlmostEqual(projected[13], 1034.037577418, places=9)
        self.assertAlmostEqual(projected[4], 875.938735740, places=9)
        self.assertAlmostEqual(max(projected), 1045.988282866, places=9)

    @patch.object(runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.PORTABLE_SHARD_ALLOCATION)
    @patch.object(runner, "CHECKS", CURRENT_CHECKS)
    def test_portable_plan_preserves_all_known_owned_components_as_planning_only(self):
        seconds = dict(OWNED_COMPLETE_REJECTED["seconds"])
        for rows in OWNED_REJECTED["shards"].values():
            for row in rows:
                seconds[row["id"]] = max(seconds[row["id"]], row["seconds"])
        for source in (PORTABLE_REJECTED["prior_accepted"]["seconds"], PORTABLE_REJECTED["seconds"]):
            self.assertEqual(set(source), set(seconds))
            for name, elapsed in source.items():
                seconds[name] = max(seconds[name], elapsed)
        self.assertEqual(len(seconds), 64)
        self.assertEqual(sum(row[2] for row in runner.configured_checks()), 505)
        projected = runner.measured_shard_seconds(timing_rows(seconds))
        self.assertAlmostEqual(projected[0], 1006.845207815, places=9)
        self.assertAlmostEqual(projected[4], 1021.836501295, places=9)
        self.assertAlmostEqual(projected[13], 1034.037577418, places=9)
        self.assertAlmostEqual(max(projected), 1045.988282866, places=9)
        with patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.OWNED_BALANCED_SHARD_ALLOCATION,
        ), self.assertRaisesRegex(ValueError, "1050-second guard"):
            runner.measured_shard_seconds(timing_rows(seconds))


class ColourShardSchedulingTests(unittest.TestCase):
    def setUp(self):
        historical = patch.object(runner, "CHECKS", COLOUR_CERTAINTY_CHECKS)
        historical.start()
        self.addCleanup(historical.stop)
        allocation = patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.OWNED_BALANCED_SHARD_ALLOCATION,
        )
        allocation.start()
        self.addCleanup(allocation.stop)

    def test_owned_relocation_preserves_rejected_partial_run_and_exact_projection(self):
        self.assertEqual(OWNED_REJECTED["run_identity"], {
            "GITHUB_REPOSITORY": "microsoft/jbig2_rs",
            "GITHUB_RUN_ID": "45280486638",
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_SHA": "030ec395517a6e9a9ca8091a6c0a02aa4776165f",
        })
        self.assertEqual(OWNED_REJECTED["proposal_head"], "12cc1f5")
        observed = {
            row["id"]: row["seconds"]
            for rows in OWNED_REJECTED["shards"].values() for row in rows
        }
        counts = {row[0]: row[2] for row in CURRENT_CHECKS}
        for rows in OWNED_REJECTED["shards"].values():
            for row in rows:
                self.assertEqual(row["status"], "passed")
                self.assertEqual(row["counterexamples"], counts[row["id"]])
        with patch.object(runner, "CHECKS", CURRENT_CHECKS):
            with patch.object(runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", {
                **runner.OWNED_DECODE_SHARD_ALLOCATION, "owned-decode": 0,
            }):
                original = runner.configured_shards()
                for index, rows in OWNED_REJECTED["shards"].items():
                    self.assertEqual(
                        [row[0] for row in original[int(index)]],
                        [row["id"] for row in rows],
                    )
                previous = [
                    math.fsum(observed[row[0]] for row in original[index])
                    for index in range(4)
                ]
            self.assertAlmostEqual(previous[0], 1075.4300956399998, places=9)
            self.assertGreater(previous[0], 1050)
            self.assertAlmostEqual(previous[1], 811.748944268, places=9)
            current = runner.configured_shards()
            projected = [
                math.fsum(observed[row[0]] for row in current[index])
                for index in range(4)
            ]
            self.assertAlmostEqual(projected[0], 1006.8452078149999, places=9)
            self.assertAlmostEqual(projected[1], 880.3338320929998, places=9)
            self.assertEqual(projected[2:], previous[2:])
            partial_rows = [
                {"id": name, "status": "passed", "seconds": seconds}
                for name, seconds in observed.items()
            ]
            with self.assertRaisesRegex(ValueError, "every complete current-profile group"):
                runner.measured_shard_seconds(partial_rows)

    def test_owned_relocation_keeps_complete_historical_maxima_as_planning_not_acceptance(self):
        # No setup-saving deductions or acceptance inferred from different-source runs.
        with patch.object(runner, "CHECKS", CURRENT_CHECKS), patch.object(
            runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.OWNED_DECODE_SHARD_ALLOCATION,
        ):
            current = runner.configured_shards()
            historical_sharded = {
                row[0] for shard in current for row in shard if row[0] != "owned-decode"
            }
            cache = TIMINGS["dependency_reuse"]
            profiles = list(cache["shard_profiles"].values())
            self.assertEqual(len(profiles), 5)
            self.assertEqual(set().union(*(set(row) for row in profiles)), historical_sharded)
            seconds = {
                name: max(row[name] for row in profiles if name in row)
                for name in historical_sharded
            }
            for group in (*runner.configured_whole_checks(), *runner.configured_collector_checks()):
                seconds[group[0]] = max(
                    row["seconds"][group[0]] for row in TIMINGS["profiles"].values()
                )
            seconds["arithmetic-certainty"] = max(
                seconds["arithmetic-certainty"], cache["certainty_budget_seconds"],
            )
            for rows in OWNED_REJECTED["shards"].values():
                for row in rows:
                    seconds[row["id"]] = max(seconds.get(row["id"], 0), row["seconds"])
            self.assertEqual(set(seconds), {row[0] for row in runner.configured_checks()})
            self.assertEqual(len(seconds), 64)
            projected = [
                math.fsum(seconds[row[0]] for row in shard) for shard in current
            ]
            self.assertAlmostEqual(projected[0], 1023.021545111, places=9)
            self.assertAlmostEqual(projected[1], 1117.426081810, places=9)
            self.assertGreater(projected[1], 1050)
            rows = timing_rows(seconds)
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(rows)
            with patch.object(runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", {
                **runner.OWNED_DECODE_SHARD_ALLOCATION, "owned-decode": 0,
            }):
                old_bound = math.fsum(
                    seconds[row[0]] for row in runner.configured_shards()[0]
                )
                self.assertGreaterEqual(old_bound, 1075.4300956399998)
                with self.assertRaisesRegex(ValueError, "1050-second guard"):
                    runner.measured_shard_seconds(rows)

    def test_complete_owned_profile_rejection_requires_only_the_smallest_group_overlay(self):
        profile = OWNED_COMPLETE_REJECTED
        self.assertEqual(profile["run_identity"], {
            "GITHUB_REPOSITORY": "microsoft/jbig2_rs",
            "GITHUB_RUN_ID": "45289712820",
            "GITHUB_RUN_ATTEMPT": "1",
            "GITHUB_SHA": "a8fc8d65fe429fa6374483be0cdfd33ece3a6006",
        })
        self.assertTrue(profile["all_groups_passed"])
        self.assertEqual(profile["counterexamples"], 505)
        seconds = profile["seconds"]
        with patch.object(runner, "CHECKS", CURRENT_CHECKS):
            self.assertEqual(set(seconds), {row[0] for row in runner.configured_checks()})
            self.assertEqual(len(seconds), 64)
            self.assertEqual(sum(row[2] for row in runner.configured_checks()), 505)
            rows = timing_rows(seconds)
            with patch.object(
                runner, "PORTABLE_ROBUST_SHARD_ALLOCATION", runner.OWNED_DECODE_SHARD_ALLOCATION,
            ):
                original = configured_roles()
                self.assertEqual(len(original), 16)
                self.assertEqual(
                    {name: [row[0] for row in groups] for name, groups in original.items()},
                    profile["role_groups"],
                )
                observed = {
                    name: math.fsum(seconds[row[0]] for row in groups)
                    for name, groups in original.items()
                }
                for name, elapsed in profile["role_seconds"].items():
                    self.assertAlmostEqual(observed[name], elapsed, places=9)
                self.assertAlmostEqual(max(observed.values()), 1054.647908281, places=9)
                with self.assertRaisesRegex(ValueError, "1050-second guard"):
                    runner.measured_shard_seconds(rows)
            current = configured_roles()
            moved = "default-limits-3"
            self.assertEqual(
                min((row[0] for row in original["shard-12"]), key=seconds.get), moved,
            )
            for name in original:
                self.assertEqual(
                    [row for row in original[name] if row[0] != moved],
                    [row for row in current[name] if row[0] != moved],
                )
            self.assertIn(moved, [row[0] for row in current["shard-13"]])
            self.assertNotIn(moved, [row[0] for row in current["shard-12"]])
            # Frozen-input regrouping is a plan, not a fresh same-run qualification.
            projection = runner.measured_shard_seconds(rows)
            self.assertAlmostEqual(projection[12], 963.695956135, places=9)
            self.assertAlmostEqual(projection[13], 829.459011706, places=9)
            self.assertAlmostEqual(max(projection), 1029.196259829, places=9)
            self.assertEqual(current["whole"], original["whole"])
            self.assertEqual(current["collector"], original["collector"])

    def test_balanced_owned_plan_conservatively_envelopes_all_owned_run_components(self):
        # Complete 45289712820/1 plus every earlier 45280486638/1 component;
        # no setup subtraction, discarded adverse case or acceptance credit.
        seconds = dict(OWNED_COMPLETE_REJECTED["seconds"])
        for rows in OWNED_REJECTED["shards"].values():
            for row in rows:
                self.assertIn(row["id"], seconds)
                seconds[row["id"]] = max(seconds[row["id"]], row["seconds"])
        with patch.object(runner, "CHECKS", CURRENT_CHECKS):
            self.assertEqual(set(seconds), {row[0] for row in runner.configured_checks()})
            bounds = runner.measured_shard_seconds(timing_rows(seconds))
            self.assertAlmostEqual(bounds[0], 1006.845207815, places=9)
            self.assertAlmostEqual(bounds[1], 1002.128138983, places=9)
            self.assertAlmostEqual(bounds[3], 951.948942690, places=9)
            self.assertAlmostEqual(max(bounds), 1029.196259829, places=9)
            self.assertLess(max(bounds), 1050)

    def test_current_inventory_requires_owned_costs_without_changing_the_original_guard(self):
        historical_rows = timing_rows(maximum_seconds())
        with patch.object(runner, "CHECKS", CURRENT_CHECKS):
            checks = runner.configured_checks()
            self.assertEqual(len(checks), 64)
            self.assertEqual(sum(row[2] for row in checks), 505)
            owned = next(row for row in checks if row[0] == "owned-decode")
            self.assertEqual(owned[2], 2)
            self.assertIn(owned, runner.configured_shards()[1])
            with self.assertRaisesRegex(ValueError, "every complete current-profile group"):
                runner.measured_shard_seconds(historical_rows)
            # Synthetic refusal control, not a measured cost for the new group.
            rows = [
                {"id": row[0], "status": "passed",
                 "seconds": 1050.001 if row[0] == "owned-decode" else 1}
                for row in checks
            ]
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(rows)

    def test_complete_actual_profiles_keep_every_group_and_rejected_timing(self):
        expected = {row[0] for row in runner.CHECKS if row[0] != "arithmetic-certainty"}
        self.assertEqual(set(TIMINGS["profiles"]), {"45092228281/1", "45102932751/1"})
        self.assertEqual(len(expected), 62)
        self.assertEqual(sum(row[2] for row in runner.CHECKS if row[0] in expected), 499)
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

    @patch.object(runner, "COLOUR_SHARD_ALLOCATION", TIMINGS["pre_dependency_cache_allocation"])
    def test_previous_maxima_prediction_preserves_its_original_scope_and_guard(self):
        measured = TIMINGS["certainty"]["measured_seconds"]
        budget = TIMINGS["certainty"]["budget_seconds"]
        self.assertEqual(measured, 264.47)
        self.assertEqual(budget, 320)
        self.assertGreaterEqual(budget, 1.2 * measured)
        seconds = maximum_seconds()
        excluded = set(runner.WHOLE_GROUPS) | set(runner.COLLECTOR_GROUPS)
        actual_total = math.fsum(
            value for name, value in seconds.items()
            if name not in excluded | {"arithmetic-certainty"}
        )
        self.assertAlmostEqual(actual_total, 14149.248929882, places=8)
        bounds = runner.measured_shard_seconds(timing_rows(seconds))
        self.assertEqual(len(bounds), 14)
        for actual, expected in zip(bounds, EXPECTED_BOUNDS):
            self.assertAlmostEqual(actual, expected, places=7)
        self.assertLess(max(bounds), 1050)
        for profile in TIMINGS["profiles"].values():
            rows = timing_rows({
                **profile["seconds"], "arithmetic-certainty": budget,
            })
            self.assertLess(max(runner.measured_shard_seconds(rows)), 1050)
        with patch.object(runner, "COLOUR_SHARD_ALLOCATION", {}):
            for profile in TIMINGS["profiles"].values():
                with self.assertRaisesRegex(ValueError, "1050-second guard"):
                    runner.measured_shard_seconds(timing_rows({
                        **profile["seconds"], "arithmetic-certainty": budget,
                    }))

    def test_every_membership_role_and_order_is_preserved_once(self):
        checks = runner.configured_checks()
        self.assertEqual(len(checks), 63)
        self.assertEqual(sum(row[2] for row in checks), 503)
        self.assertEqual(runner.CI_SHARDS, 14)
        self.assertEqual(runner.WHOLE_GROUPS, ("arithmetic-codec",))
        self.assertEqual(
            runner.COLLECTOR_GROUPS,
            ("arithmetic-decision", "reset", "clone-results", "context-callers"),
        )
        excluded = set(runner.WHOLE_GROUPS) | set(runner.COLLECTOR_GROUPS)
        sharded = tuple(row for row in checks if row[0] not in excluded)
        self.assertEqual(set(runner.COLOUR_SHARD_ALLOCATION), {row[0] for row in sharded})
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

    def test_pre_rgb_scopes_keep_exact_legacy_slots_and_order(self):
        for remove in ({"colour-rgb"}, {"colour-rgb", "arithmetic-certainty"}):
            checks = tuple(row for row in runner.CHECKS if row[0] not in remove)
            with patch.object(runner, "CHECKS", checks):
                before = runner.configured_shards()
                with patch.object(
                    runner, "COLOUR_SHARD_ALLOCATION", {row[0]: 13 for row in checks},
                ):
                    self.assertEqual(runner.configured_shards(), before)
            self.assertEqual(len(before), 14)

    def test_insufficient_headroom_still_fails_the_original_guard(self):
        with patch.object(
            runner, "COLOUR_SHARD_ALLOCATION", TIMINGS["pre_dependency_cache_allocation"],
        ):
            self.check_previous_insufficient_headroom()
        with self.assertRaisesRegex(ValueError, "1050-second guard"):
            runner.measured_shard_seconds(timing_rows({
                **maximum_seconds(), "huffman-assignment": 1050.001,
            }))

    def check_previous_insufficient_headroom(self):
        for name, value in (("arithmetic-certainty", 330), ("huffman-assignment", 1050.001)):
            with self.subTest(group=name), self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(timing_rows({**maximum_seconds(), name: value}))

    def test_dependency_reuse_prediction_uses_all_five_real_profiles_and_keeps_cold_setup(self):
        cache = TIMINGS["dependency_reuse"]
        profiles = list(cache["shard_profiles"].values())
        sharded = {
            row[0] for shard in runner.configured_shards() for row in shard
        }
        self.assertEqual(len(profiles), 5)
        for profile in profiles:
            self.assertIn(set(profile), (sharded, sharded - {"arithmetic-certainty"}))
        seconds = {name: max(profile.get(name, 0) for profile in profiles) for name in sharded}
        self.assertGreater(
            cache["measured_warm_setup_saving_seconds"], cache["planned_warm_setup_saving_seconds"],
        )
        self.assertLess(cache["cached_two_groups_seconds"], 0.9 * cache["cold_two_groups_seconds"])
        self.assertGreater(cache["certainty_budget_seconds"], 1.5 * seconds["arithmetic-certainty"])
        seconds["arithmetic-certainty"] = cache["certainty_budget_seconds"]
        saving = cache["planned_warm_setup_saving_seconds"]
        predicted = [
            sum(seconds[row[0]] for row in shard) - saving * (len(shard) - 1)
            for shard in runner.configured_shards()
        ]
        self.assertEqual(len(predicted), 14)
        self.assertLess(max(predicted), 1024)


if __name__ == "__main__":
    unittest.main()
