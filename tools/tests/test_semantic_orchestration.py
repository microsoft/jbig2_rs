import copy
import json
import io
import os
from pathlib import Path
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest.mock import patch


TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import run_semantic_proofs as runner


LIVE_CHECKS = runner.CHECKS
PRE_REFERENCE_TYPE_CHECKS = tuple(
    row for row in LIVE_CHECKS if row[0] not in {"reference-type-lookup", "reference-type-classes"}
)
PRE_REFERENCE_CATALOG_CHECKS = tuple(
    row for row in PRE_REFERENCE_TYPE_CHECKS if row[0] not in {"reference-accounting", "reference-catalog"}
)
PRE_PUBLIC_RESULT_CHECKS = tuple(
    row for row in PRE_REFERENCE_CATALOG_CHECKS if row[0] not in {"public-decode-results", "public-global-results"}
)
PRE_PUBLIC_CONTEXT_CHECKS = tuple(
    row for row in PRE_PUBLIC_RESULT_CHECKS if row[0] not in {"public-context", "context-callers"}
)
PRE_CLONE_RESULT_CHECKS = tuple(
    row for row in PRE_PUBLIC_CONTEXT_CHECKS if row[0] != "clone-results"
)
PRE_SESSION_FRAMING_CHECKS = tuple(
    row for row in PRE_CLONE_RESULT_CHECKS if row[0] != "session-framing"
)
PRE_CONTEXT_BASELINE_CHECKS = tuple(
    row for row in PRE_SESSION_FRAMING_CHECKS if row[0] != "context-lifecycle"
)
PRE_IMPL_IDENTITY_CHECKS = tuple(
    (name, script, 6 if name == "error-wrappers-basic" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_CONTEXT_BASELINE_CHECKS
)
PRE_ENTROPY_PARTITION_CHECKS = tuple(
    (name, script, 40 if name == "arithmetic-codec" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_IMPL_IDENTITY_CHECKS if name != "huffman-codec"
)
LIVE_TOPOLOGY = {
    "CI_SHARDS": runner.CI_SHARDS,
    "WHOLE_GROUPS": runner.WHOLE_GROUPS,
    "COLLECTOR_GROUPS": runner.COLLECTOR_GROUPS,
    "DEFAULT_SHARD_REBALANCE": runner.DEFAULT_SHARD_REBALANCE,
}
PRE_REFERENCE_CATALOG_TOPOLOGY = {
    **LIVE_TOPOLOGY,
    "DEFAULT_SHARD_REBALANCE": {
        **runner.DEFAULT_SHARD_REBALANCE,
        "retention": 3, "default-limits-3": 8, "collector-sink": 11, "mmr-span": 13,
        "arithmetic-initialization": 7, "default-decoder-4": 4,
        "default-decoder-3": 5,
    },
}
CURRENT_TOPOLOGY = {
    **PRE_REFERENCE_CATALOG_TOPOLOGY,
    "WHOLE_GROUPS": tuple(name for name in runner.WHOLE_GROUPS if name != "public-context"),
    "COLLECTOR_GROUPS": tuple(name for name in runner.COLLECTOR_GROUPS
                              if name not in {"clone-results", "context-callers"}),
}
CAPACITY_TOPOLOGY = {
    **CURRENT_TOPOLOGY,
    "DEFAULT_SHARD_REBALANCE": {
        **CURRENT_TOPOLOGY["DEFAULT_SHARD_REBALANCE"], "reader-error": 9, "halftone-header": 9,
    },
}
EIGHT_SHARD_TOPOLOGY = {
    "CI_SHARDS": 8,
    "WHOLE_GROUPS": ("arithmetic-codec", "value-bridge", "collector-sink"),
    "COLLECTOR_GROUPS": ("arithmetic-decision", "document-count", "reset"),
    "DEFAULT_SHARD_REBALANCE": {
        "mmr-reference": 0, "error-wrappers-regions": 0, "arithmetic-normalization": 0,
        "region-dispatch": 0, "default-decoder-5": 0,
        "document-layout": 1, "mmr-span": 1, "iaid-state": 1,
        "bitmap-chunk": 1, "default-limits-2": 1,
        "page-state": 2, "bitmap-copy": 2, "black-span": 2, "operator-bit": 2,
        "default-decoder-2": 2, "default-limits-5": 2,
        "integer-state": 3, "page-public": 3, "page-finish": 3,
        "arithmetic-initialization": 3, "default-decoder-1": 3,
        "symbol-cardinality": 4, "halftone-header": 4, "bitmap-compose": 4,
        "arithmetic-refill": 4, "retention": 4,
        "text-batch": 5, "reader-error": 5, "error-wrappers-symbol": 5,
        "mmr-run": 5, "default-limits-3": 5,
        "segment-data": 6, "error-conversion": 6, "error-wrappers-basic": 6,
        "huffman-assignment": 6, "default-decoder-4": 6, "default-limits-4": 6,
        "segment-header": 7, "arithmetic-integer": 7, "mmr-horizontal": 7,
        "mmr-prefix": 7, "default-decoder-3": 7, "default-limits-1": 7,
    },
}
CAPACITY_GROUP_SECONDS = {
    "arithmetic-codec": 711, "value-bridge": 219, "collector-sink": 119,
    "arithmetic-decision": 518, "document-count": 453, "reset": 120,
    "mmr-reference": 532, "error-wrappers-regions": 287, "arithmetic-normalization": 235,
    "region-dispatch": 148, "default-decoder-5": 147, "bitmap-chunk": 221,
    "document-layout": 419, "mmr-span": 309, "iaid-state": 236, "default-limits-2": 146,
    "operator-bit": 187, "page-state": 438, "bitmap-copy": 348, "black-span": 243,
    "default-decoder-2": 148, "default-limits-5": 144, "integer-state": 352,
    "page-public": 312, "page-finish": 257, "arithmetic-initialization": 233,
    "default-decoder-1": 148, "symbol-cardinality": 436, "halftone-header": 307,
    "bitmap-compose": 279, "arithmetic-refill": 231, "retention": 175, "mmr-run": 184,
    "text-batch": 405, "reader-error": 328, "error-wrappers-symbol": 262,
    "default-limits-3": 144, "huffman-assignment": 212, "segment-data": 350,
    "error-conversion": 321, "error-wrappers-basic": 264, "default-decoder-4": 138,
    "default-limits-4": 136, "mmr-prefix": 199, "segment-header": 332,
    "arithmetic-integer": 301, "mmr-horizontal": 250, "default-decoder-3": 138,
    "default-limits-1": 134,
}
SIX_SHARD_WHOLE_GROUPS = (
    "arithmetic-codec", "value-bridge", "error-conversion",
    "default-limits-1", "default-limits-4", "default-limits-5", "collector-sink",
)
SIX_SHARD_COLLECTOR_GROUPS = (
    "arithmetic-decision", "document-count", "retention", "error-wrappers-regions",
    "default-limits-2", "default-limits-3",
)
SIX_SHARD_MOVES = {
    "mmr-reference": 0, "arithmetic-refill": 0, "symbol-cardinality": 0,
    "error-wrappers-basic": 0, "operator-bit": 0,
    "default-decoder-3": 0, "default-decoder-4": 0,
    "document-layout": 1, "arithmetic-normalization": 1, "arithmetic-integer": 1,
    "bitmap-chunk": 1, "black-span": 1, "reset": 1,
    "region-dispatch": 2, "reader-error": 2, "mmr-horizontal": 2, "iaid-state": 2,
    "default-decoder-2": 2, "bitmap-compose": 2, "default-decoder-5": 2,
    "huffman-assignment": 3, "text-batch": 3, "arithmetic-initialization": 3,
    "default-decoder-1": 3, "mmr-prefix": 3, "mmr-run": 3, "page-public": 3,
    "integer-state": 4, "bitmap-copy": 4, "error-wrappers-symbol": 4,
    "segment-data": 4, "page-finish": 4,
    "segment-header": 5, "mmr-span": 5, "halftone-header": 5, "page-state": 5,
}
PRE_PRODUCTION_CACHE_CHECKS = tuple(
    (name, script, {
        "huffman-assignment": 21, "arithmetic-codec": 33, "value-bridge": 12,
        "symbol-cardinality": 12,
    }.get(name, count), strong, weaker)
    for name, script, count, strong, weaker in PRE_ENTROPY_PARTITION_CHECKS
)
PRE_FALLBACK_STATE_CHECKS = tuple(
    (name, script, 15 if name == "huffman-assignment" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_PRODUCTION_CACHE_CHECKS
)
PRE_CACHE_DISPATCH_CHECKS = tuple(
    (name, script, 11 if name == "huffman-assignment" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_FALLBACK_STATE_CHECKS
)
PRE_PREFIX_PEEK_CHECKS = tuple(
    (name, script, 8 if name == "huffman-assignment" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_CACHE_DISPATCH_CHECKS
)
PRE_LINE_RESULT_CHECKS = tuple(
    (name, script, 4 if name == "huffman-assignment" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_PREFIX_PEEK_CHECKS
)
PRE_LITERAL_FIELDS_CHECKS = tuple(
    (name, script, 19 if name == "arithmetic-codec" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_LINE_RESULT_CHECKS
)
PRE_EXPORT_ORDER_CHECKS = tuple(
    (name, script, 11 if name == "symbol-cardinality" else count, strong, weaker)
    for name, script, count, strong, weaker in PRE_LITERAL_FIELDS_CHECKS
)
PRE_SINK_CHECKS = tuple(row for row in PRE_EXPORT_ORDER_CHECKS if row[0] != "collector-sink")
PRE_DEFAULT_CHECKS = tuple(row for row in PRE_SINK_CHECKS if not row[0].startswith("default-"))
PRE_WRAPPER_CHECKS = tuple(row for row in PRE_DEFAULT_CHECKS if not row[0].startswith("error-wrappers-"))
PRE_CONVERSION_CHECKS = tuple(row for row in PRE_WRAPPER_CHECKS if row[0] != "error-conversion")
PRE_DEFAULT_WHOLE_GROUPS = ("arithmetic-codec", "value-bridge", "error-conversion", "retention")
PRE_DEFAULT_COLLECTOR_GROUPS = ("arithmetic-decision", "document-count", "reset", "error-wrappers-regions")
PRE_WRAPPER_COLLECTOR_GROUPS = ("arithmetic-decision", "document-count", "reset")
PRE_COLLECTOR_REBALANCE_GROUPS = (
    "arithmetic-decision", "document-count", "retention", "error-wrappers-regions", "default-decoder-1",
)
PRE_COLLECTOR_REBALANCE_ADDED = {
    name: index for name, index in runner.ADDED_SHARD_GROUPS.items() if name != "default-decoder-1"
}
PRE_COLLECTOR_REBALANCE_MOVES = {
    "symbol-cardinality": 0, "default-decoder-4": 1, "operator-bit": 1,
    "bitmap-compose": 2, "default-decoder-5": 2, "error-wrappers-basic": 3,
    "page-public": 3, "default-limits-4": 4, "segment-data": 4,
    "default-limits-5": 5, "halftone-header": 5, "page-state": 5,
}
PRE_ENVELOPE_WHOLE_GROUPS = (
    "arithmetic-codec", "value-bridge", "reset", "error-conversion", "default-limits-1",
)
PRE_ENVELOPE_SHARD_MOVES = {**PRE_COLLECTOR_REBALANCE_MOVES, "default-limits-5": 4}
PRE_NATIVE_LOAD_COLLECTOR_GROUPS = (
    "arithmetic-decision", "document-count", "retention", "error-wrappers-regions",
)
PRE_NATIVE_LOAD_SHARD_MOVES = {
    **SIX_SHARD_MOVES, "default-limits-2": 5,
    "default-limits-3": 1, "default-decoder-3": 2, "default-decoder-4": 4,
}

DOCUMENT_PARTITION_GROUPS = (
    ("iaid-state", 6), ("arithmetic-initialization", 6), ("text-batch", 11),
    ("black-span", 6), ("segment-data", 10), ("value-bridge", 5), ("mmr-span", 8),
    ("reader-error", 9), ("arithmetic-refill", 6), ("mmr-reference", 11),
    ("bitmap-copy", 8), ("retention", 3), ("bitmap-chunk", 4),
    ("symbol-cardinality", 11), ("segment-header", 10), ("document-count", 13),
    ("mmr-run", 4), ("arithmetic-codec", 19), ("arithmetic-decision", 15),
    ("halftone-header", 9), ("reset", 2), ("arithmetic-normalization", 6),
    ("arithmetic-integer", 9), ("mmr-horizontal", 6), ("integer-state", 9),
    ("operator-bit", 4), ("document-layout", 12), ("region-dispatch", 3),
    ("bitmap-compose", 7), ("mmr-prefix", 4),
)


def document_partition_checks():
    checks = {row[0]: row for row in runner.configured_checks()}
    return tuple(
        (*checks[name][:2], count, *checks[name][3:])
        for name, count in DOCUMENT_PARTITION_GROUPS
    )


def document_partition_shards():
    with patch.object(runner, "CHECKS", document_partition_checks()), patch.object(
        runner, "WHOLE_GROUPS", (),
    ), patch.object(
        runner, "COLLECTOR_GROUPS", (),
    ):
        return runner.configured_shards()


def evidence():
    return {
        "schema_version": 2, "status": "passed", "verus_version": "pinned", "production_sources_modified": False,
        "counterexamples": [
            {"id": "case", "function": "module::function",
             "normal_rust_compiles": True, "strong_contract": "rejected",
             "weaker_control": "accepted", "strong_verifier_errors": 1,
             "control_verifier_errors": 0,
             "baseline_selected_verification": {
                 "success": True, "errors": 0, "verified": 1,
                 "encountered-error": False, "encountered-vir-error": False,
                 "is-verifying-entire-crate": False,
             }},
        ],
    }


class SemanticOrchestrationTests(unittest.TestCase):
    @patch.object(runner, "CHECKS", LIVE_CHECKS)
    @patch.multiple(runner, **LIVE_TOPOLOGY)
    def test_dictionary_fix_profile_preserves_checks_within_the_original_guard(self):
        # Complete rejected run 43244487615/attempt1, rounded to milliseconds.
        costs = {
            "clone-results": 276.333, "context-callers": 212.532,
            "arithmetic-decision": 416.912, "reset": 85.768,
            "mmr-run": 134.377, "mmr-reference": 335.612,
            "default-limits-1": 103.152, "reader-error": 248.683,
            "document-count": 288.516, "default-decoder-2": 87.769,
            "iaid-state": 195.804, "mmr-span": 189.675,
            "reference-accounting": 155.796, "segment-header": 238.212,
            "region-dispatch": 92.809, "integer-state": 230.156,
            "operator-bit": 134.713, "error-conversion": 297.505,
            "value-bridge": 530.211, "reference-catalog": 128.659,
            "default-decoder-4": 69.719, "page-public": 172.827,
            "default-decoder-1": 73.365, "default-decoder-5": 72.423,
            "arithmetic-normalization": 129.193, "reference-type-classes": 173.126,
            "session-framing": 303.066, "default-limits-4": 95.521,
            "page-finish": 188.999, "default-limits-5": 97.001,
            "default-limits-2": 103.079, "error-wrappers-symbol": 211.399,
            "symbol-cardinality": 561.671, "arithmetic-refill": 178.347,
            "page-state": 354.072, "huffman-codec": 353.474,
            "public-decode-results": 237.117, "public-global-results": 137.558,
            "text-batch": 309.546, "arithmetic-integer": 265.637,
            "retention": 105.142, "error-wrappers-regions": 233.958,
            "document-layout": 329.283, "halftone-header": 242.266,
            "mmr-horizontal": 203.594, "black-span": 186.773,
            "segment-data": 282.094, "arithmetic-initialization": 177.080,
            "public-context": 275.664, "error-wrappers-basic": 266.548,
            "bitmap-copy": 228.732, "default-limits-3": 104.771,
            "mmr-prefix": 129.761, "bitmap-chunk": 126.226,
            "default-decoder-3": 102.741, "bitmap-compose": 198.195,
            "context-lifecycle": 316.887, "reference-type-lookup": 180.709,
            "huffman-assignment": 862.111, "collector-sink": 83.043,
            "arithmetic-codec": 846.407,
        }
        rows = [{"id": name, "status": "passed", "seconds": seconds}
                for name, seconds in costs.items()]
        previous = {**runner.ADDED_SHARD_GROUPS, "reference-type-lookup": 9}
        with patch.object(runner, "ADDED_SHARD_GROUPS", previous):
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(rows)
        measured = runner.measured_shard_seconds(rows)
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(),
                 *runner.configured_shards()]
        assigned = [check for role in roles for check in role]
        self.assertEqual(len(measured), 14)
        self.assertEqual(len(assigned), len(costs))
        self.assertEqual({check[0] for check in assigned}, set(costs))
        self.assertEqual(sum(check[2] for check in assigned), 494)
        self.assertLess(max(measured), 1050)

    def test_entropy_partition_preserves_every_recipe_and_valid_selected_baseline(self):
        from check_arithmetic_codec_proofs import CASES, CASE_GROUPS, CASE_BASELINE_GROUPS

        self.assertEqual({name: len(cases) for name, cases in CASE_GROUPS.items()}, {
            "arithmetic-codec": 27, "huffman-codec": 13,
        })
        partitioned = [case for cases in CASE_GROUPS.values() for case in cases]
        self.assertEqual(len(partitioned), len(CASES))
        self.assertEqual(len({case["id"] for case in partitioned}), len(CASES))
        self.assertEqual({case["id"] for case in partitioned}, {case["id"] for case in CASES})
        original = {case["id"]: case for case in CASES}
        for case in partitioned:
            self.assertIs(case, original[case["id"]])
        for name, cases in CASE_GROUPS.items():
            targets = {(case["module"], case["function"]) for case in cases}
            grouped = set()
            for module, functions in CASE_BASELINE_GROUPS[name]:
                group = {(module, function) for function in functions}
                self.assertGreaterEqual(len(functions), 2)
                self.assertEqual(len(group), len(functions))
                self.assertTrue(group <= targets)
                self.assertFalse(group & grouped)
                grouped.update(group)
        self.assertEqual({case["function"] for case in CASE_GROUPS["huffman-codec"]}, {
            "HuffmanCodec::read_raw_bits", "HuffmanCodec::decode_integer",
            "HuffmanCodec::decode_symbol_id", "SymbolCodec::decode_integer",
            "SymbolCodec::decode_symbol_id",
        })

    @patch.object(runner, "CHECKS", LIVE_CHECKS)
    @patch.multiple(runner, **LIVE_TOPOLOGY)
    def test_entropy_partition_keeps_all_queries_on_the_existing_sixteen_roles(self):
        shards = runner.configured_shards()
        whole = runner.configured_whole_checks()
        collector = runner.configured_collector_checks()
        assigned = [row for role in (whole, collector, *shards) for row in role]
        self.assertEqual(len(shards), 14)
        self.assertEqual(len(assigned), 61)
        self.assertEqual(len({row[0] for row in assigned}), 61)
        self.assertEqual(sum(row[2] for row in assigned), 494)
        self.assertIn("clone-results", {row[0] for row in collector})
        self.assertEqual({row[0]: row[2] for row in whole}, {
            "arithmetic-codec": 27,
        })
        self.assertIn("public-context", {row[0] for row in shards[7]})
        self.assertIn("context-callers", {row[0] for row in collector})
        self.assertIn("huffman-codec", {row[0] for row in shards[3]})
        self.assertEqual(set(assigned), set(LIVE_CHECKS))

    def test_live_huffman_id_counts_match_the_executed_case_sets(self):
        from check_arithmetic_codec_proofs import CASE_GROUPS as codec_cases
        from check_value_bridge_proofs import CASES as value_cases
        from check_symbol_cardinality_proofs import CASES as cardinality_cases

        with patch.object(runner, "CHECKS", LIVE_CHECKS):
            checks = runner.configured_checks()
        counts = {row[0]: row[2] for row in checks}
        for identity, cases in codec_cases.items():
            self.assertEqual(counts[identity], len(cases))
        self.assertEqual(counts["value-bridge"], len(value_cases))
        self.assertEqual(counts["symbol-cardinality"], len(cardinality_cases))
        self.assertEqual(sum(row[2] for row in checks), 494)
        self.assertEqual(len(checks), 61)

    @patch.object(runner, "CHECKS", PRE_REFERENCE_TYPE_CHECKS)
    @patch.multiple(runner, **LIVE_TOPOLOGY)
    def test_rejected_catalog_profile_rebalances_complete_original_measurements(self):
        # Complete per-group maxima of42244087583/42259441787/42278989577; none accepted.
        costs = {
            "arithmetic-codec": 745.498, "clone-results": 276.639,
            "context-callers": 205.625, "arithmetic-decision": 398.436, "reset": 81.797,
            "mmr-run": 137.043, "mmr-reference": 351.223, "default-limits-1": 117.461,
            "reader-error": 266.655, "document-count": 338.304,
            "default-decoder-2": 102.568, "iaid-state": 225.386,
            "default-limits-2": 109.748, "error-wrappers-symbol": 225.275,
            "symbol-cardinality": 595.849, "retention": 112.754,
            "arithmetic-refill": 192.993, "page-state": 380.689, "huffman-codec": 382.086,
            "public-decode-results": 235.845, "public-global-results": 136.820,
            "default-decoder-4": 107.972, "text-batch": 311.379,
            "arithmetic-integer": 256.360, "error-wrappers-regions": 241.842,
            "document-layout": 343.588, "default-decoder-3": 110.586,
            "halftone-header": 251.133, "mmr-horizontal": 225.362,
            "black-span": 209.040, "segment-data": 315.899, "public-context": 301.467,
            "error-wrappers-basic": 283.911, "arithmetic-initialization": 194.041,
            "bitmap-copy": 246.218, "mmr-prefix": 141.710, "bitmap-chunk": 136.563,
            "bitmap-compose": 217.094, "default-limits-3": 109.597,
            "context-lifecycle": 345.878, "huffman-assignment": 855.678,
            "reference-accounting": 185.181, "segment-header": 299.433,
            "region-dispatch": 113.660, "integer-state": 289.452,
            "operator-bit": 129.463, "error-conversion": 288.027,
            "collector-sink": 86.421, "value-bridge": 473.643,
            "reference-catalog": 182.543, "page-public": 254.937,
            "default-decoder-1": 107.688, "default-decoder-5": 107.395,
            "arithmetic-normalization": 178.643, "session-framing": 374.002,
            "mmr-span": 257.861, "default-limits-4": 117.115,
            "page-finish": 234.713, "default-limits-5": 119.913,
        }
        self.assertEqual(set(costs), {row[0] for row in PRE_REFERENCE_TYPE_CHECKS})
        rows = [{"id": name, "status": "passed", "seconds": seconds}
                for name, seconds in costs.items()]
        with patch.object(runner, "DEFAULT_SHARD_REBALANCE",
                          PRE_REFERENCE_CATALOG_TOPOLOGY["DEFAULT_SHARD_REBALANCE"]):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)
        first_correction = {
            **PRE_REFERENCE_CATALOG_TOPOLOGY["DEFAULT_SHARD_REBALANCE"],
            "retention": 5, "default-limits-3": 2, "collector-sink": 9, "mmr-span": 0,
        }
        with patch.object(runner, "DEFAULT_SHARD_REBALANCE", first_correction):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)
        second_correction = {
            **runner.DEFAULT_SHARD_REBALANCE, "default-limits-3": 5, "default-decoder-3": 5,
        }
        with patch.object(runner, "DEFAULT_SHARD_REBALANCE", second_correction):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)
        measured = runner.measured_shard_seconds(rows)
        self.assertEqual(len(measured), 14)
        self.assertLess(max(measured), 1000)
        self.assertLess(measured[13], 900)

    @patch.multiple(runner, **PRE_REFERENCE_CATALOG_TOPOLOGY)
    def test_public_results_keep_every_old_role_and_use_existing_shard_four_headroom(self):
        previous = {
            **runner.DEFAULT_SHARD_REBALANCE, "session-framing": 12,
        }
        with patch.object(runner, "CHECKS", PRE_PUBLIC_RESULT_CHECKS), patch.object(
            runner, "DEFAULT_SHARD_REBALANCE", previous,
        ), patch.object(
            runner, "WHOLE_GROUPS", ("arithmetic-codec", "public-context"),
        ):
            before = (
                runner.configured_whole_checks(), runner.configured_collector_checks(),
                *runner.configured_shards(),
            )
        with patch.object(runner, "CHECKS", PRE_REFERENCE_CATALOG_CHECKS):
            after = (
                runner.configured_whole_checks(), runner.configured_collector_checks(),
                *runner.configured_shards(),
            )
        added = {"public-decode-results", "public-global-results"}
        moved = {"session-framing", "public-context"}
        self.assertEqual(len(after), 16)
        for old, current in zip(before, after):
            self.assertEqual(
                tuple(row for row in old if row[0] not in moved),
                tuple(row for row in current if row[0] not in added | moved),
            )
        self.assertEqual({row[0] for row in after[6] if row[0] in added}, added)
        self.assertIn("session-framing", {row[0] for row in before[14]})
        self.assertIn("session-framing", {row[0] for row in after[15]})
        self.assertIn("public-context", {row[0] for row in before[0]})
        self.assertIn("public-context", {row[0] for row in after[9]})

    @patch.object(runner, "CHECKS", PRE_REFERENCE_CATALOG_CHECKS)
    @patch.multiple(runner, **PRE_REFERENCE_CATALOG_TOPOLOGY)
    def test_completed_whole_queries_still_need_the_original_twenty_minute_job_headroom(self):
        # Original42218240883/attempt1 finished both queries at the job cancellation.
        completed = {"arithmetic-codec": 736.550, "public-context": 310.854}
        setup_and_whole_proof = 35 + 121
        previous_whole = sum(completed.values())
        self.assertLess(previous_whole, 1050)
        self.assertGreater(setup_and_whole_proof + previous_whole, 20 * 60)
        whole = runner.configured_whole_checks()
        self.assertEqual({row[0] for row in whole}, {"arithmetic-codec"})
        self.assertLess(setup_and_whole_proof + sum(completed[row[0]] for row in whole), 20 * 60)
        self.assertIn("public-context", {row[0] for row in runner.configured_shards()[7]})
        self.assertLess(621.185 + completed["public-context"], 1050)

    @patch.object(runner, "CHECKS", PRE_REFERENCE_CATALOG_CHECKS)
    @patch.multiple(runner, **PRE_REFERENCE_CATALOG_TOPOLOGY)
    def test_failed_public_result_profile_requires_measured_framing_redistribution(self):
        # Original run42206783028/attempt1, all57groups; seconds rounded up.
        costs = {
            "arithmetic-codec": 585.286, "public-context": 233.702,
            "clone-results": 236.057, "context-callers": 175.428,
            "arithmetic-decision": 341.531, "reset": 66.723,
            "mmr-run": 138.161, "mmr-reference": 339.983,
            "default-limits-1": 106.768, "reader-error": 260.346,
            "document-count": 347.816, "default-decoder-2": 105.893, "iaid-state": 232.553,
            "default-limits-2": 69.961, "error-wrappers-symbol": 137.611,
            "symbol-cardinality": 383.246, "retention": 96.322,
            "arithmetic-refill": 154.483, "page-state": 310.099, "huffman-codec": 330.024,
            "public-decode-results": 172.496, "public-global-results": 98.134,
            "default-decoder-4": 81.900, "text-batch": 225.435, "arithmetic-integer": 175.385,
            "error-wrappers-regions": 169.028, "document-layout": 229.495,
            "default-decoder-3": 72.984, "halftone-header": 165.258,
            "mmr-horizontal": 212.305, "black-span": 194.358, "segment-data": 288.289,
            "error-wrappers-basic": 242.081, "arithmetic-initialization": 164.230,
            "bitmap-copy": 214.874, "mmr-prefix": 110.864, "bitmap-chunk": 108.928,
            "bitmap-compose": 170.383, "default-limits-3": 86.152,
            "context-lifecycle": 272.088, "huffman-assignment": 783.528,
            "segment-header": 289.914, "region-dispatch": 111.824, "integer-state": 288.828,
            "operator-bit": 91.178, "error-conversion": 195.742,
            "collector-sink": 59.713, "value-bridge": 332.749,
            "page-public": 277.047, "default-decoder-1": 116.105,
            "default-decoder-5": 114.585, "arithmetic-normalization": 190.493,
            "session-framing": 353.560, "mmr-span": 167.583,
            "default-limits-4": 75.452, "page-finish": 155.786, "default-limits-5": 76.329,
        }
        self.assertEqual(set(costs), {row[0] for row in PRE_REFERENCE_CATALOG_CHECKS})
        rows = [{"id": name, "status": "passed", "seconds": seconds} for name, seconds in costs.items()]
        previous = {**runner.DEFAULT_SHARD_REBALANCE, "session-framing": 12}
        with patch.object(runner, "DEFAULT_SHARD_REBALANCE", previous):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)
        measured = runner.measured_shard_seconds(rows)
        self.assertEqual(len(measured), 14)
        self.assertLess(max(measured), 1000)
        self.assertAlmostEqual(measured[12], 698.230)
        self.assertAlmostEqual(measured[13], 828.710)

    @patch.multiple(runner, **CURRENT_TOPOLOGY)
    def test_complete_rejected_huffman_state_profile_keeps_original_guards_and_roles(self):
        # Actual same-run49-group costs at2991b46, run41334856554/attempt1.
        costs = {
            "arithmetic-decision": 394.815, "reset": 84.221,
            "mmr-run": 99.392, "mmr-reference": 235.157, "default-limits-1": 82.920,
            "document-count": 345.749, "default-decoder-2": 110.154, "iaid-state": 180.819,
            "segment-header": 239.867, "region-dispatch": 96.666, "integer-state": 240.870,
            "operator-bit": 122.386, "error-conversion": 254.990,
            "collector-sink": 77.621, "value-bridge": 137.876,
            "page-public": 185.349, "default-decoder-1": 75.200,
            "default-decoder-5": 77.560, "arithmetic-normalization": 133.717,
            "mmr-span": 192.593, "default-limits-4": 90.477,
            "page-finish": 172.937, "default-limits-5": 93.529,
            "default-limits-2": 111.434, "error-wrappers-symbol": 223.348,
            "symbol-cardinality": 333.642, "retention": 115.752,
            "arithmetic-refill": 180.679, "page-state": 350.707,
            "default-decoder-4": 117.599, "text-batch": 306.242, "arithmetic-integer": 252.224,
            "error-wrappers-regions": 172.976, "document-layout": 226.767,
            "default-decoder-3": 73.509, "mmr-horizontal": 218.060,
            "black-span": 205.376, "segment-data": 300.192,
            "error-wrappers-basic": 195.987, "arithmetic-initialization": 164.186,
            "bitmap-copy": 203.517, "mmr-prefix": 146.140, "bitmap-chunk": 141.179,
            "bitmap-compose": 217.234, "default-limits-3": 116.558,
            "huffman-assignment": 557.924, "reader-error": 255.934,
            "halftone-header": 252.475, "arithmetic-codec": 534.684,
        }
        rows = [{"id": name, "status": "passed", "seconds": seconds} for name, seconds in costs.items()]
        original = {**runner.DEFAULT_SHARD_REBALANCE, "reader-error": 9, "halftone-header": 9}
        with patch.object(runner, "DEFAULT_SHARD_REBALANCE", original):
            original_groups = runner.configured_shards()
            self.assertAlmostEqual(sum(costs[row[0]] for row in original_groups[9]), 1066.333)
            with self.assertRaisesRegex(ValueError, "1050-second guard"):
                runner.measured_shard_seconds(rows)
        current = runner.configured_shards()
        self.assertEqual(len(current), 14)
        assigned = [row for shard in current for row in shard]
        self.assertEqual(len(assigned) + len(runner.configured_whole_checks()) +
                         len(runner.configured_collector_checks()), 49)
        self.assertEqual({row[0] for row in current[9]}, {"huffman-assignment"})
        self.assertAlmostEqual(max(runner.measured_shard_seconds(rows)), 725.727)
        self.assertLessEqual(max(runner.measured_shard_seconds(rows)), 1050)

    def setUp(self):
        legacy = patch.multiple(
            runner, CI_SHARDS=6, WHOLE_GROUPS=SIX_SHARD_WHOLE_GROUPS,
            COLLECTOR_GROUPS=SIX_SHARD_COLLECTOR_GROUPS, DEFAULT_SHARD_REBALANCE=SIX_SHARD_MOVES,
            CHECKS=PRE_ENTROPY_PARTITION_CHECKS,
        )
        legacy.start()
        self.addCleanup(legacy.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        (self.root / "tools").mkdir()
        (self.root / "src").mkdir()
        (self.root / "config").mkdir()
        (self.root / "src/lib.rs").write_text(
            '#[path = "example.rs"] mod module;\nmod document;\n'
        )
        (self.root / "src/example.rs").write_text(
            "// source fixture\nfn function() {}\nfn first() {}\nfn second() {}\n"
        )
        (self.root / "src/document.rs").write_text(
            "fn function() {}\nfn first() {}\nfn second() {}\n"
        )
        (self.root / "config/toolchain.json").write_text(json.dumps({"verus": {"version": "pinned"}}))
        self.checks = tuple((str(index), f"{index}.py", 1, "strong_contract", "weaker_control") for index in range(8))
        for _, script, *_ in self.checks:
            (self.root / "tools" / script).write_text(
                'CASES = ({"id": "case", "module": "module", "function": "function"},)\n'
            )
        self.output = self.root / "reports"
        self.environment = patch.dict(os.environ, {
            name: "local" for name in (
                "GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_SHA",
            )
        })
        self.environment.start()
        self.addCleanup(self.environment.stop)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    def test_eight_shard_profile_preserves_all_queries_and_bounded_concurrency(self):
        with patch.multiple(runner, **EIGHT_SHARD_TOPOLOGY):
            shards = runner.configured_shards()
            whole = runner.configured_whole_checks()
            collector = runner.configured_collector_checks()
            self.assertEqual(len(shards), 8)
            self.assertEqual([row[0] for row in whole], ["arithmetic-codec", "value-bridge"])
            self.assertEqual(
                [row[0] for row in collector], ["arithmetic-decision", "document-count", "reset"],
            )
            assigned = [row for role in (whole, collector, *shards) for row in role]
            self.assertEqual(len(assigned), 48)
            self.assertEqual(len({row[0] for row in assigned}), 48)
            self.assertEqual(sum(row[2] for row in assigned), 336)
            self.assertEqual(set(assigned), set(runner.configured_checks()))
            costs = {
                "arithmetic-decision": 355, "document-count": 298, "retention": 71,
                "error-wrappers-regions": 198, "default-decoder-4": 73,
                "integer-state": 259, "bitmap-copy": 201, "error-wrappers-symbol": 188,
                "segment-data": 252, "page-finish": 173, "bitmap-chunk": 97,
                "document-layout": 307, "arithmetic-normalization": 149,
                "default-limits-3": 72, "arithmetic-integer": 221, "black-span": 153,
                "reset": 50, "default-limits-2": 72, "segment-header": 246,
                "mmr-span": 201, "halftone-header": 211, "page-state": 306,
                "operator-bit": 91, "mmr-reference": 333, "arithmetic-refill": 149,
                "symbol-cardinality": 258, "error-wrappers-basic": 169,
                "huffman-assignment": 111, "mmr-prefix": 102, "mmr-run": 102,
                "text-batch": 255, "arithmetic-initialization": 144,
                "default-decoder-1": 71, "page-public": 210, "arithmetic-codec": 400,
                "value-bridge": 111, "error-conversion": 221, "default-limits-1": 62,
                "default-limits-4": 62, "default-limits-5": 63, "region-dispatch": 75,
                "reader-error": 212, "mmr-horizontal": 183, "iaid-state": 151,
                "default-decoder-2": 73, "default-decoder-3": 73,
                "bitmap-compose": 171, "default-decoder-5": 73,
            }
            measured = [sum(costs[row[0]] for row in role) for role in (whole, collector, *shards)]
            self.assertEqual(measured, [511, 703, 828, 828, 887, 857, 860, 829, 888, 887])
            self.assertLessEqual(max(measured), 1050)
            self.assertLessEqual(measured[0] + 208, 1200)

    def test_capacity_profile_preserves_all_queries_and_measured_headroom(self):
        with patch.multiple(runner, **CAPACITY_TOPOLOGY):
            shards = runner.configured_shards()
            whole = runner.configured_whole_checks()
            collector = runner.configured_collector_checks()
            self.assertEqual(len(shards), 14)
            self.assertEqual([row[0] for row in whole], ["arithmetic-codec"])
            self.assertEqual([row[0] for row in collector], ["arithmetic-decision", "reset"])
            assigned = [row for role in (whole, collector, *shards) for row in role]
            self.assertEqual(len(assigned), 49)
            self.assertEqual(len({row[0] for row in assigned}), 49)
            self.assertEqual(sum(row[2] for row in assigned), 405)
            self.assertEqual(set(assigned), set(runner.configured_checks()))
            rows = [
                {"id": row[0], "status": "passed", "seconds": CAPACITY_GROUP_SECONDS[row[0]]}
                for row in runner.configured_checks()
            ]
            measured = runner.measured_shard_seconds(rows)
            self.assertEqual(measured, [850, 837, 844, 844, 844, 844, 843, 845, 843, 847, 832, 846, 842, 846])
            self.assertEqual(sum(CAPACITY_GROUP_SECONDS[row[0]] for row in whole), 711)
            self.assertEqual(sum(CAPACITY_GROUP_SECONDS[row[0]] for row in collector), 638)
            self.assertGreater(sum(measured), 13 * 850)
            self.assertLessEqual(max(measured), 850)
            exact = copy.deepcopy(rows)
            next(row for row in exact if row["id"] == "mmr-reference")["seconds"] += 200
            self.assertEqual(max(runner.measured_shard_seconds(exact)), 1050)
            next(row for row in exact if row["id"] == "mmr-reference")["seconds"] += 1
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(exact)
            with patch.object(sys, "argv", ["runner", "--matrix"]), redirect_stdout(io.StringIO()) as stream:
                self.assertEqual(runner.main(), 0)
            self.assertEqual(json.loads(stream.getvalue()), {"shard": list(range(14))})
        with patch.multiple(runner, **EIGHT_SHARD_TOPOLOGY):
            with self.assertRaisesRegex(ValueError, "1050-second"):
                runner.measured_shard_seconds(rows)

    def run_fixture(self, process):
        with patch.object(runner, "CHECKS", self.checks), patch.object(runner.subprocess, "run", side_effect=process):
            return runner.run_checks(self.root, self.root / "verus", self.output)

    def passing_process(self, command, **kwargs):
        report = evidence()
        report["source_bindings"] = {"src/example.rs": runner.digest(self.root / "src/example.rs")}
        Path(command[-1]).write_text(json.dumps(report))
        return SimpleNamespace(returncode=0)

    def test_colliding_trait_receipts_reject_missing_or_swapped_native_scopes(self):
        (self.root / "src/lib.rs").write_text("mod owner;\nmod left;\nmod right;\n")
        (self.root / "src/owner.rs").write_text("pub struct Failure;\n")
        cases = []
        for scope, kind in (("left", "u8"), ("right", "u16")):
            (self.root / f"src/{scope}.rs").write_text(
                "#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
                f"impl From<{kind}> for crate::owner::Failure {{ fn from(value: {kind}) -> Self {{ Self }} }}\n"
            )
            cases.append({
                "id": scope, "file": f"src/{scope}.rs", "module": "owner",
                "verification_module": scope, "function": "Failure::from",
            })
        (self.root / "tools/conversion.py").write_text("CASES = " + repr(cases) + "\n")
        bindings = {
            f"src/{scope}.rs": runner.digest(self.root / f"src/{scope}.rs")
            for scope in ("left", "right")
        }
        report = evidence()
        report["source_bindings"] = bindings
        report["counterexamples"] = []
        for case in cases:
            record = copy.deepcopy(evidence()["counterexamples"][0])
            record.update(
                id=case["id"], function="owner::Failure::from",
                verification_module=case["verification_module"], source_file=case["file"],
            )
            report["counterexamples"].append(record)
        check = ("conversions", "conversion.py", 2, "strong_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, bindings)
        for field, value in (("verification_module", None), ("verification_module", "right"), ("source_file", "src/right.rs")):
            broken = copy.deepcopy(report)
            broken["counterexamples"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, "exact source scope"):
                runner.check_bound_report(self.root, check, broken, bindings)

    def test_same_scope_trait_receipts_require_exact_type_parameters_and_all_peers(self):
        (self.root / "src/lib.rs").write_text("mod owner;\nmod left;\n")
        (self.root / "src/owner.rs").write_text("pub struct Failure;\n")
        source, cases = "", []
        for kind in ("u8", "u16"):
            source += (
                "#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
                f"impl From<{kind}> for crate::owner::Failure {{ fn from(value: {kind}) -> Self {{ Self }} }}\n"
            )
            cases.append({
                "id": kind, "file": "src/left.rs", "module": "owner",
                "verification_module": "left", "function": "Failure::from",
                "impl_trait": f"From<{kind}>",
            })
        (self.root / "src/left.rs").write_text(source)
        (self.root / "tools/conversion.py").write_text("CASES = " + repr(cases) + "\n")
        bindings = {"src/left.rs": runner.digest(self.root / "src/left.rs")}
        report = evidence()
        report["source_bindings"] = bindings
        report["counterexamples"] = []
        for case in cases:
            record = copy.deepcopy(evidence()["counterexamples"][0])
            record.update(
                id=case["id"], function="owner::Failure::from", verification_module="left",
                source_file="src/left.rs", impl_trait=case["impl_trait"],
                selected_impl_traits=["From<u16>", "From<u8>"],
                strong_verified_functions=1, control_verified_functions=2,
            )
            record["baseline_selected_verification"]["verified"] = 2
            report["counterexamples"].append(record)
        check = ("conversions", "conversion.py", 2, "strong_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, bindings)
        for field, value in (
            ("impl_trait", "From<u16>"), ("selected_impl_traits", ["From<u8>"]),
            ("strong_verified_functions", 0), ("control_verified_functions", 1),
        ):
            broken = copy.deepcopy(report)
            broken["counterexamples"][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "selected peer bodies"):
                runner.check_bound_report(self.root, check, broken, bindings)

    def test_whole_trait_receipt_needs_global_uniqueness_and_the_exact_whole_identity(self):
        (self.root / "src/example.rs").write_text(
            "struct Value;\n#[cfg_attr(verus_keep_ghost, verus_verify)]\n"
            "impl Default for Value { fn default() -> Self { Self } }\n"
        )
        case = {
            "id": "case", "module": "module", "function": "Value::default",
            "file": "src/example.rs", "verification_module": "module", "impl_trait": "Default",
        }
        (self.root / "tools/unique.py").write_text("CASES = " + repr((case,)) + "\n")
        bindings = {"src/example.rs": runner.digest(self.root / "src/example.rs")}
        report = evidence()
        report["source_bindings"] = bindings
        whole = {
            "receipt_sha256": "1" * 64, "report_sha256": "2" * 64,
            "report": {
                "verification-results": {
                    **report["counterexamples"][0]["baseline_selected_verification"],
                    "is-verifying-entire-crate": True, "verified": 999,
                },
                "func-details": {"jbig2_rs_production::module::Value::default": {}},
            },
        }
        row = report["counterexamples"][0]
        row.update(
            function="module::Value::default", verification_module="module",
            source_file="src/example.rs", impl_trait="Default", selected_impl_traits=["Default"],
            strong_verified_functions=0, control_verified_functions=1,
            baseline_selected_verification=runner.baseline_from_whole(
                whole, ["jbig2_rs_production::module::Value::default"],
            ),
        )
        check = ("unique", "unique.py", 1, "strong_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, bindings, whole)
        self.assertEqual(row["baseline_selected_verification"]["verified"], 999)
        broken = copy.deepcopy(report)
        broken["counterexamples"][0]["baseline_selected_verification"]["whole_positive"]["receipt_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "selected cases/baselines"):
            runner.check_bound_report(self.root, check, broken, bindings, whole)
        (self.root / "src/example.rs").write_text(
            (self.root / "src/example.rs").read_text()
            + "impl Value { fn default() -> Self { Self } }\n"
        )
        bindings["src/example.rs"] = runner.digest(self.root / "src/example.rs")
        with self.assertRaisesRegex(ValueError, "exact source scope"):
            runner.check_bound_report(self.root, check, report, bindings, whole)

    def test_wrapper_group_selection_is_forwarded_to_its_shared_checker(self):
        script = "check_error_wrapper_proofs.py"
        (self.root / "tools" / script).write_text(
            "CASES = ({'id':'case','module':'module','function':'function'},)\n"
            "CASE_GROUPS = {'error-wrappers-basic': CASES}\n"
        )
        check = ("error-wrappers-basic", script, 1, "strong_contract", "weaker_control")

        def process(command, **kwargs):
            self.assertEqual(command[command.index("--case-group") + 1], "error-wrappers-basic")
            report = evidence()
            report["case_group"] = "error-wrappers-basic"
            report["source_bindings"] = {"src/example.rs": runner.digest(self.root / "src/example.rs")}
            Path(command[-1]).write_text(json.dumps(report))
            return SimpleNamespace(returncode=0)

        with patch.object(runner, "CHECKS", (check,)), patch.object(runner.subprocess, "run", side_effect=process):
            result = runner.run_checks(self.root, self.root / "verus", self.output, only="error-wrappers-basic")
        self.assertEqual(result["status"], "passed")

    @patch.object(runner, "CHECKS", PRE_DEFAULT_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", PRE_DEFAULT_WHOLE_GROUPS)
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_DEFAULT_COLLECTOR_GROUPS)
    def test_new_wrapper_groups_rebalance_two_measured_memberships_without_more_producers(self):
        shards = runner.configured_shards()
        with patch.object(runner, "CHECKS", PRE_WRAPPER_CHECKS), patch.object(
            runner, "WHOLE_GROUPS", ("arithmetic-codec", "value-bridge", "error-conversion"),
        ):
            before = runner.configured_shards()
        symbol = next(row for row in runner.configured_checks() if row[0] == "error-wrappers-symbol")
        basic = next(row for row in runner.configured_checks() if row[0] == "error-wrappers-basic")
        expected = [list(shard) for shard in before]
        expected[4].append(symbol)
        expected[5].append(basic)
        retention = next(row for row in expected[2] if row[0] == "retention")
        expected[2].remove(retention)
        for name, old, new in (
            ("huffman-assignment", 5, 1), ("operator-bit", 1, 2),
            ("mmr-prefix", 4, 3), ("mmr-run", 4, 3), ("bitmap-chunk", 3, 4),
        ):
            row = next(row for row in expected[old] if row[0] == name)
            expected[old].remove(row)
            expected[new].append(row)
        self.assertEqual(shards, tuple(tuple(shard) for shard in expected))
        self.assertNotIn("error-wrappers-basic", [row[0] for row in runner.configured_whole_checks()])
        self.assertIn("retention", [row[0] for row in runner.configured_whole_checks()])
        self.assertIn("error-wrappers-regions", [row[0] for row in runner.configured_collector_checks()])
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *shards]
        self.assertEqual(len(roles), 8)
        self.assertEqual(sum(len(role) for role in roles), 38)
        self.assertEqual(sum(row[2] for role in roles for row in role), 306)

    @patch.object(runner, "CHECKS", PRE_DEFAULT_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", PRE_DEFAULT_WHOLE_GROUPS)
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_DEFAULT_COLLECTOR_GROUPS)
    def test_completed_rejected_wrapper_profiles_rebalance_below_original_guards(self):
        # All producers in40760950278 completed; its guard rejection grants no acceptance credit.
        # Combine both complete rejected profiles with accepted #111; round maxima up.
        costs = {
            "mmr-reference": 352, "reader-error": 228, "black-span": 163,
            "arithmetic-refill": 160, "region-dispatch": 81,
            "document-layout": 314, "page-public": 223, "page-finish": 169,
            "arithmetic-normalization": 147, "operator-bit": 94,
            "page-state": 331, "arithmetic-integer": 235, "mmr-horizontal": 192,
            "iaid-state": 159, "retention": 77,
            "text-batch": 266, "halftone-header": 215, "bitmap-compose": 172,
            "arithmetic-initialization": 145, "bitmap-chunk": 96,
            "symbol-cardinality": 265, "integer-state": 265, "bitmap-copy": 195,
            "mmr-run": 105, "mmr-prefix": 108, "error-wrappers-symbol": 185,
            "segment-data": 270, "segment-header": 266, "mmr-span": 222,
            "huffman-assignment": 126, "error-wrappers-basic": 217,
            "arithmetic-codec": 448, "value-bridge": 117, "error-conversion": 238,
            "arithmetic-decision": 398, "document-count": 326, "reset": 52,
            "error-wrappers-regions": 213,
        }
        rejected = ("segment-data", "segment-header", "mmr-span", "huffman-assignment", "error-wrappers-basic")
        self.assertGreater(sum(costs[name] for name in rejected), 1050)
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *runner.configured_shards()]
        measured = [sum(costs[row[0]] for row in role) for role in roles]
        self.assertLessEqual(max(measured), 1020)
        self.assertLessEqual(measured[0] + 211, 1200)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    def test_default_groups_use_existing_roles_and_the_complete_measured_rebalance(self):
        shards = runner.configured_shards()
        with patch.object(runner, "CHECKS", PRE_DEFAULT_CHECKS), patch.object(
            runner, "WHOLE_GROUPS", PRE_DEFAULT_WHOLE_GROUPS,
        ), patch.object(runner, "COLLECTOR_GROUPS", PRE_DEFAULT_COLLECTOR_GROUPS):
            expected = {row[0]: index for index, shard in enumerate(runner.configured_shards()) for row in shard}
        whole = runner.configured_whole_checks()
        collector = runner.configured_collector_checks()
        self.assertEqual(whole[-1][0], "default-limits-5")
        self.assertEqual(collector[-1][0], "default-limits-3")
        self.assertNotIn("reset", [row[0] for row in whole])
        self.assertIn("retention", [row[0] for row in collector])
        for name, index in (
            ("default-limits-2", 0), ("default-limits-3", 1),
            ("default-limits-4", 1), ("default-limits-5", 1),
            ("default-decoder-1", 3),
            ("default-decoder-2", 2), ("default-decoder-3", 2),
            ("default-decoder-4", 4), ("default-decoder-5", 5),
        ):
            expected[name] = index
        expected.update({
            "symbol-cardinality": 0, "default-decoder-4": 1, "operator-bit": 1,
            "bitmap-compose": 2, "default-decoder-5": 2, "error-wrappers-basic": 3,
            "page-public": 3, "default-limits-4": 4, "segment-data": 4,
            "default-limits-5": 4, "halftone-header": 5, "page-state": 5,
        })
        expected.pop("default-limits-4")
        expected.pop("default-limits-5")
        expected.pop("default-limits-2")
        expected.pop("default-limits-3")
        expected.update({
            "region-dispatch": 2,
            "error-wrappers-basic": 0, "huffman-assignment": 3,
            "default-decoder-3": 0, "default-decoder-4": 0, "reader-error": 2, "operator-bit": 0,
            "arithmetic-integer": 1, "bitmap-chunk": 1,
            "black-span": 1, "reset": 1, "page-finish": 4,
        })
        self.assertIn("default-limits-4", [row[0] for row in whole])
        self.assertEqual(
            {row[0]: index for index, shard in enumerate(shards) for row in shard}, expected,
        )
        roles = [whole, collector, *shards]
        self.assertEqual(len(roles), 8)
        self.assertEqual(sum(len(role) for role in roles), 48)
        self.assertEqual(sum(row[2] for role in roles for row in role), 336)

    @patch.multiple(runner, **EIGHT_SHARD_TOPOLOGY)
    @patch.object(runner, "CHECKS", PRE_EXPORT_ORDER_CHECKS)
    def test_owned_sink_uses_existing_whole_role_without_changing_shards(self):
        actual = runner.configured_shards()
        with patch.object(runner, "CHECKS", PRE_SINK_CHECKS):
            previous = runner.configured_shards()
        self.assertEqual(actual, previous)
        self.assertEqual([row[0] for row in runner.configured_whole_checks()].count("collector-sink"), 1)
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *actual]
        self.assertEqual(len(roles), 10)
        self.assertEqual(sum(len(role) for role in roles), 49)
        self.assertEqual(sum(row[2] for role in roles for row in role), 338)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_COLLECTOR_REBALANCE_GROUPS)
    @patch.object(runner, "ADDED_SHARD_GROUPS", PRE_COLLECTOR_REBALANCE_ADDED)
    @patch.object(runner, "DEFAULT_SHARD_REBALANCE", PRE_COLLECTOR_REBALANCE_MOVES)
    @patch.object(runner, "WHOLE_GROUPS", PRE_ENVELOPE_WHOLE_GROUPS)
    def test_complete_rejected_default_profile_fits_the_unchanged_guards_after_rebalance(self):
        # All 40861606251 queries passed; its measured-profile rejection grants no acceptance credit.
        costs = {
            "arithmetic-decision": 360, "document-count": 305, "reset": 49,
            "error-wrappers-regions": 204, "default-decoder-1": 87,
            "symbol-cardinality": 267, "integer-state": 255, "bitmap-copy": 192,
            "error-wrappers-symbol": 182, "default-decoder-4": 84, "bitmap-chunk": 95,
            "page-state": 328, "arithmetic-integer": 237, "mmr-horizontal": 194,
            "iaid-state": 163, "default-decoder-2": 94, "default-decoder-3": 94,
            "operator-bit": 101, "document-layout": 316, "page-public": 226,
            "page-finish": 176, "arithmetic-normalization": 162,
            "default-limits-3": 89, "default-limits-4": 90, "default-limits-5": 87,
            "huffman-assignment": 118, "segment-data": 232, "segment-header": 229,
            "mmr-span": 186, "error-wrappers-basic": 187, "default-decoder-5": 80,
            "text-batch": 262, "halftone-header": 208, "bitmap-compose": 167,
            "arithmetic-initialization": 144, "mmr-prefix": 104, "mmr-run": 105,
            "mmr-reference": 231, "reader-error": 164, "black-span": 128,
            "arithmetic-refill": 128, "region-dispatch": 62, "default-limits-2": 64,
            "arithmetic-codec": 461, "value-bridge": 120, "retention": 72,
            "error-conversion": 245, "default-limits-1": 84,
        }
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *runner.configured_shards()]
        measured = [sum(costs[row[0]] for row in role) for role in roles]
        self.assertEqual({row[0] for role in roles for row in role}, set(costs))
        self.assertLessEqual(max(measured), 1050)
        self.assertLessEqual(measured[0] + 208, 1200)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", PRE_ENVELOPE_WHOLE_GROUPS)
    @patch.object(runner, "DEFAULT_SHARD_REBALANCE", PRE_ENVELOPE_SHARD_MOVES)
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_NATIVE_LOAD_COLLECTOR_GROUPS)
    def test_completed_unique_reuse_profile_rebalances_the_remaining_collector_overload(self):
        # Complete 40873336428 inputs, rounded up; its 1078.132 collector rejection is not acceptance.
        costs = {
            "arithmetic-decision": 395, "document-count": 323, "retention": 76,
            "error-wrappers-regions": 212, "default-decoder-1": 74,
            "mmr-reference": 231, "reader-error": 153, "black-span": 109,
            "arithmetic-refill": 107, "region-dispatch": 53, "default-limits-2": 52,
            "symbol-cardinality": 194, "document-layout": 312, "page-finish": 171,
            "arithmetic-normalization": 152, "default-limits-3": 73,
            "huffman-assignment": 115, "operator-bit": 97, "default-decoder-4": 75,
            "arithmetic-integer": 221, "mmr-horizontal": 180, "iaid-state": 149,
            "default-decoder-2": 72, "default-decoder-3": 72, "bitmap-compose": 168,
            "default-decoder-5": 72, "text-batch": 220, "arithmetic-initialization": 120,
            "mmr-prefix": 86, "mmr-run": 86, "error-wrappers-basic": 142,
            "page-public": 176, "integer-state": 196, "bitmap-copy": 150,
            "error-wrappers-symbol": 143, "bitmap-chunk": 77, "default-limits-4": 57,
            "segment-data": 192, "segment-header": 245, "mmr-span": 198,
            "default-limits-5": 72, "halftone-header": 217, "page-state": 309,
            "arithmetic-codec": 333, "value-bridge": 84, "reset": 34,
            "error-conversion": 171, "default-limits-1": 49,
        }
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *runner.configured_shards()]
        measured = [sum(costs[row[0]] for row in role) for role in roles]
        self.assertEqual({row[0] for role in roles for row in role}, set(costs))
        self.assertLessEqual(max(measured), 1010)
        self.assertLessEqual(measured[0] + 208, 1200)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_NATIVE_LOAD_COLLECTOR_GROUPS)
    @patch.object(runner, "DEFAULT_SHARD_REBALANCE", PRE_NATIVE_LOAD_SHARD_MOVES)
    def test_same_harness_per_group_maxima_fit_without_relaxing_any_guard(self):
        # Per-group maxima of complete 40873336428/40883755135, rounded up; neither run is accepted.
        costs = {
            "arithmetic-decision": 395, "document-count": 328, "retention": 77,
            "error-wrappers-regions": 215, "default-decoder-1": 74,
            "segment-header": 245, "mmr-span": 198, "default-limits-5": 72,
            "halftone-header": 217, "page-state": 309, "integer-state": 238,
            "bitmap-copy": 172, "error-wrappers-symbol": 165, "bitmap-chunk": 87,
            "default-limits-4": 62, "segment-data": 219, "mmr-reference": 350,
            "reader-error": 227, "black-span": 159, "arithmetic-refill": 151,
            "region-dispatch": 77, "default-limits-2": 75, "symbol-cardinality": 274,
            "text-batch": 266, "arithmetic-initialization": 147, "mmr-prefix": 105,
            "mmr-run": 104, "error-wrappers-basic": 171, "page-public": 213,
            "arithmetic-integer": 221, "mmr-horizontal": 180, "iaid-state": 150,
            "default-decoder-2": 74, "default-decoder-3": 75, "bitmap-compose": 172,
            "default-decoder-5": 72, "document-layout": 312, "page-finish": 171,
            "arithmetic-normalization": 152, "default-limits-3": 73,
            "huffman-assignment": 115, "operator-bit": 97, "default-decoder-4": 75,
            "arithmetic-codec": 392, "value-bridge": 102, "reset": 42,
            "error-conversion": 211, "default-limits-1": 61,
        }
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *runner.configured_shards()]
        measured = [sum(costs[row[0]] for row in role) for role in roles]
        self.assertEqual({row[0] for role in roles for row in role}, set(costs))
        self.assertEqual(measured, [900, 1015, 1043, 1046, 1027, 1024, 1040, 1044])
        self.assertLessEqual(max(measured), 1050)
        self.assertLessEqual(measured[0] + 208, 1200)

    @patch.object(runner, "CHECKS", PRE_SINK_CHECKS)
    def test_complete_native_cache_profile_uses_existing_cohort_headroom(self):
        # Complete 40906782428 inputs, rounded up; its 1091.433 shard rejection is not acceptance.
        costs = {
            "arithmetic-decision": 313, "document-count": 273, "retention": 64,
            "error-wrappers-regions": 178, "default-decoder-4": 73,
            "integer-state": 248, "bitmap-copy": 189, "error-wrappers-symbol": 179,
            "segment-data": 240, "page-finish": 165, "bitmap-chunk": 97,
            "document-layout": 307, "arithmetic-normalization": 149,
            "default-limits-3": 72, "arithmetic-integer": 221, "black-span": 153,
            "reset": 50, "default-limits-2": 72, "segment-header": 243,
            "mmr-span": 200, "halftone-header": 211, "page-state": 306,
            "operator-bit": 75, "mmr-reference": 244, "arithmetic-refill": 112,
            "symbol-cardinality": 202, "error-wrappers-basic": 129,
            "huffman-assignment": 110, "mmr-prefix": 101, "mmr-run": 102,
            "text-batch": 255, "arithmetic-initialization": 140,
            "default-decoder-1": 70, "page-public": 206, "arithmetic-codec": 377,
            "value-bridge": 100, "error-conversion": 201, "default-limits-1": 58,
            "default-limits-4": 58, "default-limits-5": 57, "region-dispatch": 74,
            "reader-error": 212, "mmr-horizontal": 183, "iaid-state": 151,
            "default-decoder-2": 73, "default-decoder-3": 73,
            "bitmap-compose": 168, "default-decoder-5": 73,
        }
        roles = [runner.configured_whole_checks(), runner.configured_collector_checks(), *runner.configured_shards()]
        measured = [sum(costs[row[0]] for row in role) for role in roles]
        self.assertEqual({row[0] for role in roles for row in role}, set(costs))
        self.assertEqual(measured, [851, 972, 908, 977, 934, 984, 1021, 960])
        self.assertLessEqual(max(measured), 1050)
        self.assertLessEqual(measured[0] + 208, 1200)

    def test_group_selection_is_forwarded_to_its_shared_checker(self):
        for identity in (
            "default-limits-1", "default-limits-2", "default-limits-3",
            "default-limits-4", "default-limits-5",
            "default-decoder-1", "default-decoder-2", "default-decoder-3",
            "default-decoder-4", "default-decoder-5",
            "arithmetic-codec", "huffman-codec",
        ):
            with self.subTest(group=identity):
                script = (
                    "check_arithmetic_codec_proofs.py"
                    if identity in {"arithmetic-codec", "huffman-codec"}
                    else "check_default_proofs.py"
                )
                (self.root / "tools" / script).write_text(
                    "CASES = ({'id':'case','module':'module','function':'function'},)\n"
                    f"CASE_GROUPS = {{{identity!r}: CASES}}\n"
                )
                check = (identity, script, 1, "strong_contract", "weaker_control")

                def process(command, **kwargs):
                    self.assertEqual(command[command.index("--case-group") + 1], identity)
                    report = evidence()
                    report["case_group"] = identity
                    report["source_bindings"] = {"src/example.rs": runner.digest(self.root / "src/example.rs")}
                    Path(command[-1]).write_text(json.dumps(report))
                    return SimpleNamespace(returncode=0)

                with patch.object(runner, "CHECKS", (check,)), patch.object(
                    runner.subprocess, "run", side_effect=process,
                ):
                    result = runner.run_checks(self.root, self.root / "verus", self.output, only=identity)
                self.assertEqual(result["status"], "passed")

    def test_manifest_covers_every_semantic_checker_without_fixed_group_or_case_totals(self):
        with patch.object(runner, "CHECKS", LIVE_CHECKS):
            checks = runner.configured_checks()
        self.assertEqual(len({row[0] for row in checks}), len(checks))
        self.assertEqual(
            {row[1] for row in checks},
            {path.name for path in TOOLS.glob("check_*proof*.py")},
        )
        self.assertTrue(all(type(row[2]) is int and row[2] > 0 for row in checks))

    def test_manifest_binds_verification_cargo_inputs_without_build_outputs(self):
        verification = self.root / "verification"
        verification.mkdir()
        for name in ("Cargo.toml", "Cargo.lock"):
            (verification / name).write_text("pinned verification fixture\n")
        dependency = verification / "proposals/capacity-interface"
        dependency.mkdir(parents=True)
        for name in ("Cargo.toml", "Cargo.lock", "lib.rs"):
            (dependency / name).write_text("pinned interface fixture\n")
        (verification / "proposals/validate_interface.py").write_text("# checked control\n")
        (verification / "proposals/STATUS.txt").write_text("unapproved\n")
        (verification / "target").mkdir()
        (verification / "target/Cargo.toml").write_text("not an input\n")
        (verification / "target/lib.rs").write_text("not an input\n")
        with patch.object(runner, "CHECKS", self.checks):
            bindings = runner.manifest(self.root)["source_bindings"]
            original = bindings["verification/proposals/capacity-interface/lib.rs"]
            (dependency / "lib.rs").write_text("changed interface fixture\n")
            changed = runner.manifest(self.root)["source_bindings"]
        for name in ("Cargo.toml", "Cargo.lock", "lib.rs"):
            self.assertIn(f"verification/proposals/capacity-interface/{name}", bindings)
        self.assertIn("verification/proposals/validate_interface.py", bindings)
        self.assertIn("verification/proposals/STATUS.txt", bindings)
        self.assertNotEqual(original, changed["verification/proposals/capacity-interface/lib.rs"])
        self.assertIn("verification/Cargo.toml", bindings)
        self.assertIn("verification/Cargo.lock", bindings)
        self.assertNotIn("verification/target/Cargo.toml", bindings)
        self.assertNotIn("verification/target/lib.rs", bindings)

    @patch.object(runner, "CHECKS", PRE_WRAPPER_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", ("arithmetic-codec", "value-bridge", "error-conversion"))
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_WRAPPER_COLLECTOR_GROUPS)
    def test_error_conversion_uses_existing_whole_role_without_reassigning_prior_shards(self):
        actual = runner.configured_shards()
        previous = tuple(row for row in runner.CHECKS if row[0] != "error-conversion")
        with patch.object(runner, "CHECKS", previous):
            self.assertEqual(actual, runner.configured_shards())
        self.assertEqual(
            [row[0] for row in runner.configured_whole_checks()],
            ["arithmetic-codec", "value-bridge", "error-conversion"],
        )
        self.assertEqual(len(actual), 6)
        self.assertEqual(sum(row[2] for row in runner.configured_checks()), 286)
        self.assertEqual(len(runner.configured_checks()), 35)

    def test_manifest_binds_relocated_metadata_and_opaque_agent_profiles_not_generated_json(self):
        (self.root / "verification/target").mkdir(parents=True)
        (self.root / "verification/proof-dispositions.json").write_text("{}")
        (self.root / "verification/target/generated.json").write_text("{}")
        profiles = {".github/agents/worker.agent.md": "1" * 40}
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner, "agent_profile_blobs", return_value=profiles,
        ):
            manifest = runner.manifest(self.root)
            bindings = manifest["source_bindings"]
        self.assertIn("config/toolchain.json", bindings)
        self.assertIn("verification/proof-dispositions.json", bindings)
        self.assertEqual(manifest["agent_profile_blobs"], profiles)
        self.assertNotIn("verification/target/generated.json", bindings)

    def test_agent_profile_identity_uses_git_metadata_without_reading_instructions(self):
        (self.root / ".git").mkdir()
        outputs = [
            SimpleNamespace(stdout=""),
            SimpleNamespace(stdout="100644 blob " + "1" * 40 + "\t.github/agents/worker.agent.md\n"),
        ]
        with patch.object(runner.subprocess, "run", side_effect=outputs):
            self.assertEqual(runner.agent_profile_blobs(self.root), {
                ".github/agents/worker.agent.md": "1" * 40,
            })
        with patch.object(runner.subprocess, "run", return_value=SimpleNamespace(stdout=" M profile")):
            with self.assertRaisesRegex(ValueError, "committed opaque"):
                runner.agent_profile_blobs(self.root)

    def test_shared_document_baseline_requires_all_configured_function_identities(self):
        cases = [{"id": name, "function": name} for name in ("first", "second")]
        (self.root / "tools/document.py").write_text("CASES = " + repr(cases) + "\n")
        report = evidence()
        report.update(
            schema_version=1,
            baseline_verification_scope="document::*",
            baseline_selected_functions=["first", "second"],
            source_bindings={"src/example.rs": runner.digest(self.root / "src/example.rs")},
        )
        report["counterexamples"] = []
        for case in cases:
            record = copy.deepcopy(evidence()["counterexamples"][0])
            record.update(id=case["id"], function=case["function"], strong_count_contract="rejected")
            record["baseline_selected_verification"]["verified"] = 2
            report["counterexamples"].append(record)
        check = ("document-count", "document.py", 2, "strong_count_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, report["source_bindings"])
        for mutation in ("missing-scope", "missing-function", "duplicate-function", "too-few-proofs"):
            broken = copy.deepcopy(report)
            if mutation == "missing-scope":
                del broken["baseline_verification_scope"]
            elif mutation == "missing-function":
                broken["baseline_selected_functions"].pop()
            elif mutation == "duplicate-function":
                broken["baseline_selected_functions"] = ["first", "first"]
            else:
                broken["counterexamples"][0]["baseline_selected_verification"]["verified"] = 1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                runner.check_bound_report(self.root, check, broken, report["source_bindings"])

    def test_document_partition_rejects_missing_wrong_or_cross_group_evidence(self):
        cases = [{"id": name, "function": name} for name in ("first", "second")]
        (self.root / "tools/document.py").write_text(
            "CASES = " + repr(cases) + "\nCASE_GROUPS = "
            + repr({"document-count": cases[:1], "document-layout": cases[1:]}) + "\n"
        )
        report = evidence()
        report.update(
            schema_version=1, case_group="document-count",
            baseline_verification_scope="document::*", baseline_selected_functions=["first"],
            source_bindings={"src/example.rs": runner.digest(self.root / "src/example.rs")},
        )
        report["counterexamples"][0].update(
            id="first", function="first", strong_count_contract="rejected",
        )
        check = ("document-count", "document.py", 1, "strong_count_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, report["source_bindings"])
        for mutation in ("missing-group", "wrong-group", "cross-group", "duplicate"):
            broken = copy.deepcopy(report)
            if mutation == "missing-group":
                del broken["case_group"]
            elif mutation == "wrong-group":
                broken["case_group"] = "document-layout"
            elif mutation == "cross-group":
                broken["counterexamples"][0].update(id="second", function="second")
            else:
                broken["counterexamples"].append(copy.deepcopy(broken["counterexamples"][0]))
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                runner.check_bound_report(self.root, check, broken, report["source_bindings"])

    def test_document_partition_is_explicitly_forwarded_to_the_shared_checker(self):
        script = "check_document_count_proof.py"
        (self.root / "tools" / script).write_text(
            "CASES = ({'id': 'case', 'function': 'function'},)\n"
            "CASE_GROUPS = {'document-count': CASES}\n"
        )
        check = ("document-count", script, 1, "strong_count_contract", "weaker_control")

        def process(command, **_keywords):
            self.assertEqual(command[command.index("--case-group") + 1], "document-count")
            report = evidence()
            report.update(
                schema_version=1, case_group="document-count",
                baseline_verification_scope="document::*", baseline_selected_functions=["function"],
                source_bindings={"src/example.rs": runner.digest(self.root / "src/example.rs")},
            )
            report["counterexamples"][0].update(function="function", strong_count_contract="rejected")
            Path(command[-1]).write_text(json.dumps(report))
            return SimpleNamespace(returncode=0)

        with patch.object(runner, "CHECKS", (check,)), patch.object(
            runner.subprocess, "run", side_effect=process,
        ):
            result = runner.run_checks(self.root, self.root / "verus", self.output, only="document-count")
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["checks"][0]["counterexamples"], 1)

    def test_shared_group_baselines_cannot_omit_or_invent_target_coverage(self):
        cases = [{"id": name, "module": "module", "function": name} for name in ("first", "second")]
        groups = (("module", ("first", "second")),)
        (self.root / "tools/shared.py").write_text(
            "CASES = " + repr(cases) + "\nBASELINE_GROUPS = " + repr(groups) + "\n"
        )
        report = evidence()
        report.update(
            baseline_groups=[{"module": "module", "functions": ["first", "second"]}],
            source_bindings={"src/example.rs": runner.digest(self.root / "src/example.rs")},
        )
        report["counterexamples"] = []
        for case in cases:
            row = copy.deepcopy(evidence()["counterexamples"][0])
            row.update(id=case["id"], function="module::" + case["function"],
                       baseline_selected_functions=["first", "second"])
            row["baseline_selected_verification"]["verified"] = 2
            report["counterexamples"].append(row)
        check = ("shared", "shared.py", 2, "strong_contract", "weaker_control")
        runner.check_bound_report(self.root, check, report, report["source_bindings"])
        for mutation in ("missing-group", "missing-function", "extra-function", "too-few-proofs"):
            broken = copy.deepcopy(report)
            if mutation == "missing-group":
                del broken["baseline_groups"]
            elif mutation == "missing-function":
                broken["counterexamples"][0]["baseline_selected_functions"].pop()
            elif mutation == "extra-function":
                broken["baseline_groups"][0]["functions"].append("unknown")
            else:
                broken["counterexamples"][0]["baseline_selected_verification"]["verified"] = 1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                runner.check_bound_report(self.root, check, broken, report["source_bindings"])

    def test_actual_work_overlaps_but_never_exceeds_two_children(self):
        lock = threading.Lock()
        active = 0
        maximum = 0
        barrier = threading.Barrier(2)

        def process(command, **kwargs):
            nonlocal active, maximum
            with lock:
                active += 1
                maximum = max(maximum, active)
            barrier.wait(timeout=10)
            result = self.passing_process(command, **kwargs)
            with lock:
                active -= 1
            return result

        result = self.run_fixture(process)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(maximum, 2)
        self.assertEqual([row["id"] for row in result["checks"]], [row[0] for row in self.checks])

    def test_failed_child_cannot_reuse_a_stale_passing_report(self):
        self.output.mkdir()
        for identity, *_ in self.checks:
            (self.output / f"{identity}-proof-sensitivity.json").write_text(json.dumps(evidence()))
        result = self.run_fixture(lambda *args, **kwargs: SimpleNamespace(returncode=1))
        self.assertEqual(result["status"], "failed")
        self.assertTrue(all(row["status"] == "failed" for row in result["checks"]))
        self.assertFalse(any(self.output.glob("*-proof-sensitivity.json")))

    def test_exit_zero_without_report_fails(self):
        result = self.run_fixture(lambda *args, **kwargs: SimpleNamespace(returncode=0))
        self.assertEqual(result["status"], "failed")

    def test_invalid_evidence_shapes_and_weakened_results_fail(self):
        mutations = [
            lambda r: r.update(status="failed"),
            lambda r: r.update(verus_version="floating"),
            lambda r: r.update(production_sources_modified=True),
            lambda r: r.update(counterexamples=[]),
            lambda r: r["counterexamples"][0].update(normal_rust_compiles=False),
            lambda r: r["counterexamples"][0].update(strong_contract="accepted"),
            lambda r: r["counterexamples"][0].update(weaker_control="rejected"),
            lambda r: r["counterexamples"][0].update(strong_verifier_errors=0),
            lambda r: r["counterexamples"][0].update(control_verifier_errors=1),
            lambda r: r["counterexamples"][0].update(control_verifier_errors=False),
        ]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                report = copy.deepcopy(evidence())
                mutate(report)
                with self.assertRaises(ValueError):
                    runner.check_report(report, 1, "strong_contract", "weaker_control", "pinned")

    def test_duplicate_records_are_rejected(self):
        report = evidence()
        report["counterexamples"] *= 2
        with self.assertRaises(ValueError):
            runner.check_report(report, 2, "strong_contract", "weaker_control", "pinned")

    def test_malformed_json_and_launch_failures_are_explicit(self):
        def malformed(command, **kwargs):
            Path(command[-1]).write_text("{")
            return SimpleNamespace(returncode=0)

        for process in (malformed, lambda *args, **kwargs: (_ for _ in ()).throw(OSError("launch denied"))):
            with self.subTest(process=process):
                result = self.run_fixture(process)
                self.assertEqual(result["status"], "failed")
                self.assertTrue(all("error" in row for row in result["checks"]))

    def test_empty_and_duplicate_manifest_fail_and_remove_stale_summary(self):
        self.output.mkdir()
        summary = self.output / "semantic-proof-summary.json"
        for checks in ((), self.checks + self.checks):
            summary.write_text('{"status":"passed"}')
            with patch.object(runner, "CHECKS", checks), self.assertRaises(ValueError):
                runner.run_checks(self.root, self.root / "verus", self.output)
            self.assertFalse(summary.exists())

    def test_only_runs_one_known_complete_group_and_unknown_selector_fails(self):
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner.subprocess, "run", side_effect=self.passing_process,
        ) as process:
            result = runner.run_checks(self.root, self.root / "verus", self.output, only="2")
            self.assertEqual(result["selected_groups"], ["2"])
            self.assertEqual(result["workers"], 1)
            self.assertEqual(len(result["manifest"]["groups"]), len(self.checks))
            self.assertEqual(process.call_count, 1)
            self.assertEqual(result["status"], "passed")
            with self.assertRaisesRegex(ValueError, "unknown"):
                runner.run_checks(self.root, self.root / "verus", self.output, only="../missing")
            self.assertFalse((self.output / runner.SUMMARY).exists())

    def test_whole_job_groups_are_serial_and_not_omitted_or_duplicated_by_shards(self):
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner, "WHOLE_GROUPS", ("0", "1"),
        ), patch.object(runner.subprocess, "run", side_effect=self.passing_process):
            whole = runner.run_checks(
                self.root, self.root / "verus", self.output, whole_groups=True,
            )
            self.assertEqual(whole["selected_groups"], ["0", "1"])
            self.assertEqual(whole["workers"], 1)
            self.assertIsNone(whole["shard"])
            assigned = [row[0] for shard in runner.configured_shards() for row in shard]
            self.assertEqual(assigned, [str(index) for index in range(2, 8)])
            self.assertEqual(len(runner.configured_shards()), 6)
            self.assertEqual(set(assigned) | set(whole["selected_groups"]), {row[0] for row in self.checks})
            with self.assertRaisesRegex(ValueError, "mutually exclusive"):
                runner.run_checks(self.root, self.root / "verus", self.output, only="0", whole_groups=True)
            rows = [{"id": row[0], "status": "passed", "seconds": 1.0} for row in self.checks]
            self.assertEqual(runner.measured_shard_seconds(rows), [1] * 6)
            rows[0]["seconds"] = 1050
            with self.assertRaisesRegex(ValueError, "1050"):
                runner.measured_shard_seconds(rows)

    @patch.object(runner, "CHECKS", PRE_CONVERSION_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", ("arithmetic-codec", "value-bridge"))
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_WRAPPER_COLLECTOR_GROUPS)
    def test_pre_conversion_roles_replay_qualified_timing_inputs_without_more_jobs_or_raised_guards(self):
        # Planning only: completed f294 groups; completed shard-5 ZIP timestamps
        # plus allowance; current local value/Huffman runs plus 10%, rounded up.
        # These inputs cannot grant current-head or interrupted-run acceptance.
        weights = {
            "arithmetic-codec": 381, "arithmetic-decision": 358,
            "arithmetic-initialization": 139, "arithmetic-integer": 212,
            "arithmetic-normalization": 113, "arithmetic-refill": 103,
            "bitmap-chunk": 99, "bitmap-compose": 171, "bitmap-copy": 202,
            "black-span": 151, "document-count": 308, "document-layout": 309,
            "halftone-header": 214, "huffman-assignment": 120, "iaid-state": 144,
            "integer-state": 241, "mmr-horizontal": 169, "mmr-prefix": 102,
            "mmr-reference": 318, "mmr-run": 101, "mmr-span": 193,
            "operator-bit": 98, "page-finish": 169, "page-public": 220,
            "page-state": 309, "reader-error": 210, "region-dispatch": 56,
            "reset": 37, "retention": 70, "segment-data": 237,
            "segment-header": 174, "symbol-cardinality": 272,
            "text-batch": 293, "value-bridge": 142,
        }
        checks = runner.configured_checks()
        whole = runner.configured_whole_checks()
        collector = runner.configured_collector_checks()
        shards = runner.configured_shards()
        self.assertEqual(
            [row[0] for row in whole],
            ["arithmetic-codec", "value-bridge"],
        )
        self.assertEqual(len(shards), 6)
        assigned = list(whole) + list(collector) + [row for shard in shards for row in shard]
        self.assertEqual(len(assigned), len(checks))
        self.assertEqual({row[0] for row in assigned}, {row[0] for row in checks})
        self.assertEqual(sum(row[2] for row in assigned), sum(row[2] for row in checks))
        self.assertLessEqual(sum(weights[row[0]] for row in whole), 1000)
        self.assertLessEqual(sum(weights[row[0]] for row in collector), 1000)
        self.assertLessEqual(max(sum(weights[row[0]] for row in shard) for shard in shards), 1000)
        rows = [{"id": row[0], "status": "passed", "seconds": weights[row[0]]} for row in checks]
        self.assertLessEqual(max(runner.measured_shard_seconds(rows)), 1050)

    @patch.object(runner, "CHECKS", PRE_CONVERSION_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", ("arithmetic-codec", "value-bridge"))
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_WRAPPER_COLLECTOR_GROUPS)
    def test_completed_f53_costs_rebalance_without_accepting_its_rejected_collection(self):
        # All 40412207379 producers completed, but its collector rejected the
        # overloaded shard. These costs are scheduling inputs, not acceptance.
        observed = {
            "arithmetic-codec": 354.261, "value-bridge": 98.024,
            "arithmetic-decision": 345.218, "page-public": 212.728,
            "segment-header": 231.228, "arithmetic-refill": 138.842,
            "retention": 69.331, "region-dispatch": 73.249,
            "mmr-reference": 228.469, "arithmetic-integer": 161.789,
            "bitmap-copy": 144.227, "huffman-assignment": 78.843,
            "operator-bit": 79.065, "reset": 37.332,
            "document-layout": 302.450, "segment-data": 242.248,
            "mmr-span": 192.194, "iaid-state": 142.176, "mmr-prefix": 98.857,
            "page-state": 306.773, "halftone-header": 213.203,
            "reader-error": 201.498, "arithmetic-normalization": 140.035,
            "arithmetic-initialization": 138.166, "document-count": 288.756,
            "integer-state": 224.280, "mmr-horizontal": 157.141,
            "page-finish": 155.159, "mmr-run": 93.525,
            "text-batch": 278.817, "symbol-cardinality": 267.925,
            "bitmap-compose": 166.873, "black-span": 149.415,
            "bitmap-chunk": 96.925,
        }
        checks = runner.configured_checks()
        self.assertEqual({row[0] for row in checks}, set(observed))
        overloaded = (
            "arithmetic-decision", "page-public", "segment-header",
            "arithmetic-refill", "retention", "region-dispatch",
        )
        self.assertGreater(sum(observed[name] for name in overloaded), 1050)
        self.assertLessEqual(sum(
            observed[row[0]] for row in runner.configured_whole_checks()
        ), 1000)
        self.assertLessEqual(max(
            sum(observed[row[0]] for row in shard)
            for shard in runner.configured_shards()
        ), 1000)
        rows = [
            {"id": row[0], "status": "passed", "seconds": observed[row[0]]}
            for row in checks
        ]
        self.assertLessEqual(max(runner.measured_shard_seconds(rows)), 1050)

    def test_measured_complete_profile_leaves_timeout_headroom(self):
        # Accepted current baseline 40222911541, rounded up.
        # The new metadata group uses its complete 269-second local run plus 10%.
        # Document partitions project the same 25-case family, charging 20 extra
        # seconds for a duplicated baseline measured at 15.45 seconds.
        # Scheduling estimates are not hosted acceptance.
        # Huffman: complete local four-case run 236.25 seconds, plus 10%.
        measured = {
            "reset": (2, 62), "document-count": (25, 511), "document-layout": (25, 511),
            "bitmap-copy": (8, 259), "retention": (3, 86),
            "value-bridge": (5, 104), "mmr-span": (8, 183),
            "region-dispatch": (3, 66), "text-batch": (11, 388),
            "bitmap-chunk": (4, 123), "bitmap-compose": (7, 192),
            "black-span": (6, 137), "arithmetic-decision": (15, 370),
            "arithmetic-refill": (6, 112), "arithmetic-initialization": (6, 150),
            "arithmetic-normalization": (6, 160), "operator-bit": (4, 120),
            "arithmetic-integer": (9, 157), "iaid-state": (6, 155),
            "integer-state": (9, 165), "arithmetic-codec": (19, 531),
            "halftone-header": (9, 189), "reader-error": (9, 270),
            "mmr-prefix": (4, 96), "mmr-run": (4, 93),
            "mmr-horizontal": (6, 180), "symbol-cardinality": (11, 271),
            "mmr-reference": (11, 371), "segment-header": (10, 171),
            "segment-data": (10, 296),
        }
        checks = document_partition_checks()
        self.assertEqual({check[0] for check in checks}, set(measured))
        estimates = []
        for shard in document_partition_shards():
            seconds = 0
            for identity, _, cases, *_ in shard:
                observed_cases, observed_seconds = measured[identity]
                seconds += (cases * observed_seconds + observed_cases - 1) // observed_cases
                if identity == "document-layout":
                    seconds += 20
            estimates.append(seconds)
        self.assertLessEqual(max(estimates), 1000)

    @patch.object(runner, "CHECKS", PRE_CONVERSION_CHECKS)
    @patch.object(runner, "WHOLE_GROUPS", ("arithmetic-codec", "value-bridge"))
    @patch.object(runner, "COLLECTOR_GROUPS", PRE_WRAPPER_COLLECTOR_GROUPS)
    def test_pre_conversion_job_capacity_covers_three_profile_per_group_maxima(self):
        # Planning only: prior qualified estimates and completed f53/7ae groups,
        # taking each group's maximum and rounding up. Failed collections gain
        # no acceptance credit; current measured receipts remain mandatory.
        weights = {
            "arithmetic-codec": 500, "value-bridge": 142,
            "arithmetic-decision": 358, "document-count": 345, "reset": 46,
            "mmr-reference": 328, "document-layout": 309, "page-state": 309,
            "text-batch": 293, "symbol-cardinality": 272, "segment-data": 265,
            "reader-error": 210, "page-public": 220, "arithmetic-integer": 212,
            "halftone-header": 228, "integer-state": 241, "segment-header": 252,
            "black-span": 169, "page-finish": 169, "mmr-horizontal": 169,
            "bitmap-compose": 186, "bitmap-copy": 205, "mmr-span": 194,
            "arithmetic-refill": 139, "arithmetic-normalization": 141,
            "iaid-state": 147, "arithmetic-initialization": 139,
            "mmr-prefix": 106, "mmr-run": 113, "retention": 71,
            "region-dispatch": 74, "operator-bit": 98, "bitmap-chunk": 99,
            "huffman-assignment": 120,
        }
        roles = [
            runner.configured_whole_checks(), runner.configured_collector_checks(),
            *runner.configured_shards(),
        ]
        assigned = [row for role in roles for row in role]
        self.assertEqual(len(roles), 8)
        self.assertEqual(len(assigned), len(runner.configured_checks()))
        self.assertEqual(
            {row[0] for row in assigned},
            {row[0] for row in runner.configured_checks()},
        )
        self.assertLessEqual(max(sum(weights[row[0]] for row in role) for role in roles), 1000)
        self.assertEqual(sum(row[2] for row in assigned), sum(
            row[2] for row in runner.configured_checks()
        ))

    def test_completed_profile_has_measured_job_headroom_without_relaxed_limits(self):
        # Completed 40303484174 attempt-2 groups, with conservative ZIP-time
        # bounds for its three leading shard-5 groups; canceled jobs get no
        # acceptance credit. Document partitions use fresh complete local runs.
        observed = {
            "arithmetic-codec": 540, "arithmetic-decision": 308,
            "arithmetic-initialization": 160, "arithmetic-integer": 227,
            "arithmetic-normalization": 153, "arithmetic-refill": 155,
            "bitmap-chunk": 128, "bitmap-compose": 179, "bitmap-copy": 256,
            "black-span": 169, "document-count": 332, "document-layout": 327,
            "halftone-header": 225, "iaid-state": 163, "integer-state": 265,
            "mmr-horizontal": 155, "mmr-prefix": 124, "mmr-reference": 300,
            "mmr-run": 118, "mmr-span": 177, "operator-bit": 134,
            "reader-error": 169, "region-dispatch": 89, "reset": 46,
            "retention": 91, "segment-header": 207, "symbol-cardinality": 322,
            "text-batch": 312, "value-bridge": 120, "segment-data": 266,
        }
        self.assertEqual({row[0] for row in document_partition_checks()}, set(observed))
        self.assertEqual(sum(row[2] for row in document_partition_checks()), 236)
        self.assertLessEqual(max(
            sum(observed[row[0]] for row in shard)
            for shard in document_partition_shards()
        ), 1050)

    def test_document_partitions_use_different_shards_without_more_jobs_or_cases(self):
        shards = document_partition_shards()
        self.assertEqual(len(shards), 6)
        self.assertEqual(sum(row[2] for row in document_partition_checks()), 236)
        groups = [{row[0] for row in shard} for shard in shards]
        self.assertNotEqual(
            next(i for i, names in enumerate(groups) if "document-count" in names),
            next(i for i, names in enumerate(groups) if "document-layout" in names),
        )
        self.assertLessEqual(max(sum(row[2] for row in shard) for shard in shards), 42)

    def test_historical_snapshot_does_not_freeze_growth_within_existing_groups(self):
        current = runner.configured_checks()
        counts = {row[0]: row[2] for row in current}
        for name, count in DOCUMENT_PARTITION_GROUPS:
            self.assertGreaterEqual(counts[name], count)
        future = tuple(
            (*row[:2], row[2] + 1, *row[3:]) if row[0] == "iaid-state" else row
            for row in current
        )
        before = document_partition_shards()
        with patch.object(runner, "CHECKS", future):
            self.assertEqual(document_partition_shards(), before)
            self.assertEqual(sum(row[2] for row in runner.configured_checks()), sum(counts.values()) + 1)

    def test_complete_measurement_guard_uses_all_current_groups_without_a_global_case_limit(self):
        checks = self.checks + (("future-decision", "future.py", 100, "strong", "weak"),)
        rows = [{"id": check[0], "status": "passed", "seconds": 1.0} for check in checks]
        with patch.object(runner, "CHECKS", checks):
            self.assertEqual(runner.measured_shard_seconds(rows), [2, 2, 2, 1, 1, 1])
            for value in (None, True, 0, -1, float("nan"), float("inf"), 1050):
                broken = copy.deepcopy(rows)
                broken[0]["seconds"] = value
                with self.subTest(seconds=value), self.assertRaises(ValueError):
                    runner.measured_shard_seconds(broken)
            for broken in (rows[:-1], rows + [rows[0]], [dict(row, status="failed") for row in rows]):
                with self.assertRaises(ValueError):
                    runner.measured_shard_seconds(broken)

    def test_matrix_is_generated_from_current_complete_checks(self):
        future_checks = self.checks + (("future-decision", "future.py", 4, "strong_contract", "weaker_control"),)
        with patch.object(runner, "CHECKS", future_checks), patch.object(
            sys, "argv", ["runner", "--matrix"],
        ), redirect_stdout(io.StringIO()) as stream:
            self.assertEqual(runner.main(), 0)
        self.assertEqual(json.loads(stream.getvalue()), {"shard": [0, 1, 2, 3, 4, 5]})
        with patch.object(runner, "CHECKS", future_checks):
            assigned = [check[0] for shard in runner.configured_shards() for check in shard]
        self.assertEqual(set(assigned), {row[0] for row in future_checks})
        self.assertEqual(len(assigned), len(future_checks))

    def test_shard_runs_every_assigned_group_with_one_worker(self):
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner.subprocess, "run", side_effect=self.passing_process,
        ) as process:
            result = runner.run_checks(self.root, self.root / "verus", self.output, shard=0)
        self.assertEqual(result["selected_groups"], ["0", "6"])
        self.assertEqual(result["shard"], 0)
        self.assertEqual(result["workers"], 1)
        self.assertEqual(process.call_count, 2)
        self.assertEqual(result["manifest"]["shards"], [["0", "6"], ["1", "7"], ["2"], ["3"], ["4"], ["5"]])

    def test_invalid_or_mixed_shard_selectors_fail_without_stale_success(self):
        self.output.mkdir()
        for shard in (-1, 6, True):
            runner.write_json(self.output / runner.SUMMARY, {"status": "passed"})
            with patch.object(runner, "CHECKS", self.checks), self.subTest(shard=shard), self.assertRaises(ValueError):
                runner.run_checks(self.root, self.root / "verus", self.output, shard=shard)
            self.assertFalse((self.output / runner.SUMMARY).exists())
        with patch.object(runner, "CHECKS", self.checks), self.assertRaises(ValueError):
            runner.run_checks(self.root, self.root / "verus", self.output, only="0", shard=0)

    def test_smaller_configurations_do_not_create_empty_shards(self):
        with patch.object(runner, "CHECKS", self.checks[:2]):
            self.assertEqual(runner.configured_shards(), ((self.checks[0],), (self.checks[1],)))

    def test_clean_shards_and_whole_job_outputs_have_identical_source_manifests(self):
        with patch.object(runner, "CHECKS", self.checks):
            before = runner.manifest(self.root)
            runner.write_json(self.root / "verus-production.json", {"generated": True})
            runner.write_json(self.root / "production-proof-coverage-ci.json", {"generated": True})
            self.assertEqual(runner.manifest(self.root), before)

    def test_selected_verus_baselines_do_not_require_whole_crate_only_success_field(self):
        baseline = evidence()["counterexamples"][0]["baseline_selected_verification"]
        baseline.pop("success")
        self.assertTrue(runner.baseline_passed(baseline))
        for field, value in (
            ("encountered-error", True), ("encountered-vir-error", True),
            ("errors", 1), ("verified", 0), ("is-verifying-entire-crate", True),
        ):
            changed = dict(baseline, **{field: value})
            with self.subTest(field=field):
                self.assertFalse(runner.baseline_passed(changed))

    def whole_fixture(self, whole):
        from check_traceability import runtime_inventory
        from readiness_assessment import assess

        details = {"module::function": {"failed_proof_notes": []}}
        coverage = {
            "schema_version": 1, "source": "fixture", "verus_verified_obligations": 1,
            "native_assumptions": [],
            "counts": {"verified": 1, "incomplete": 0, "audited-forwarder": 0,
                       "reviewed-verifier-limitation": 0, "native-assumption": 0},
            "functions": [{"id": "fixture", "file": "src/example.rs",
                           "file_sha256": runner.digest(self.root / "src/example.rs"),
                           "status": "verified", "visibility": "pub", "disposition_reason": None,
                           "verus_functions": list(details)}],
        }
        runner.write_json(self.root / "verification/production-proof-coverage.json", coverage)
        runner.write_json(whole / "production-proof-coverage-ci.json", coverage)
        runner.write_json(whole / "verus-production.json", {
            "verification-results": {
                "success": True, "errors": 0, "verified": 1, "is-verifying-entire-crate": True,
            },
            "func-details": details, "verus": {"version": "pinned"},
        })
        ledger = self.root / "validation/traceability/requirements.json"
        ledger_data = {
            "requirements": [{
                "id": "fixture", "runtime": [], "functional_evidence": [],
                "invariant_evidence": [], "strength": "safety_only", "gaps": [],
            }],
            "features": [{
                "feature": "fixture", "decomposition": "partial",
                "requirement_ids": ["fixture"], "remaining_scope": ["Other statements."],
            }],
        }
        runner.write_json(ledger, ledger_data)
        runner.write_json(whole / runner.TRACE / "verifier-report.json", {
            "status": "references_valid_scope_incomplete", "verifier_report_checked": True,
            "requirements": ledger_data["requirements"], "requirement_count": 1, "feature_count": 1,
            "ledger_sha256": runner.digest(ledger),
            "validation_readiness": assess(
                ledger_data, runtime_inventory(self.root), set(details),
                {record["id"]: record for record in coverage["functions"]},
            ),
        })
        native = whole / "target/verus-production/libfixture.rlib"
        native.parent.mkdir(parents=True)
        native.write_bytes(b"native compilation fixture")

    def test_whole_recomputes_readiness_instead_of_accepting_claims_or_partial_inventory(self):
        whole = self.root / "whole"
        self.whole_fixture(whole)
        path = whole / runner.TRACE / "verifier-report.json"
        original = runner.read_json(path)
        for mutation in ("ready", "percentage", "declarations", "statements", "missing"):
            changed = copy.deepcopy(original)
            assessment = changed["validation_readiness"]
            if mutation == "ready":
                assessment["ready_for_external_validation"] = True
            elif mutation == "percentage":
                assessment["readiness_percentage"] = 100
            elif mutation == "declarations":
                assessment["code"]["declarations"].pop()
            elif mutation == "statements":
                assessment["statements"]["exhaustive_statement_count"] = 1
            else:
                del changed["validation_readiness"]
            runner.write_json(path, changed)
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, "readiness gap"):
                runner.validate_whole(self.root, whole)
        runner.write_json(path, original)
        runner.validate_whole(self.root, whole)

    def collection_fixture(self):
        inputs = self.root / "inputs"
        whole = inputs / (runner.artifact_prefix() + "whole")
        self.whole_fixture(whole)
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner.subprocess, "run", side_effect=self.passing_process,
        ):
            runner.seal_whole(self.root, whole)
            if runner.configured_whole_checks():
                runner.run_checks(
                    self.root, self.root / "verus",
                    whole / "semantic-groups" / runner.TRACE, whole_groups=True,
                )
            if runner.configured_collector_checks():
                runner.run_checks(
                    self.root, self.root / "verus",
                    inputs / (runner.artifact_prefix() + "collector") / runner.TRACE,
                    collector_groups=True,
                )
            for index, _ in enumerate(runner.configured_shards()):
                runner.run_checks(
                    self.root, self.root / "verus",
                    inputs / (runner.artifact_prefix() + f"shard-{index}") / runner.TRACE, shard=index,
                )
        return inputs

    def test_collector_groups_are_serial_exclusive_and_required_before_aggregation(self):
        with patch.object(runner, "COLLECTOR_GROUPS", ("0", "1")):
            inputs = self.collection_fixture()
            path = inputs / (runner.artifact_prefix() + "collector") / runner.TRACE / runner.SUMMARY
            original = runner.read_json(path)
            self.assertEqual(original["workers"], 1)
            self.assertEqual(original["selected_groups"], ["0", "1"])
            with patch.object(runner, "CHECKS", self.checks):
                result = runner.aggregate(self.root, inputs, self.output, "success", "success")
                self.assertGreater(result["measured_collector_group_seconds"], 0)
                for change in ("missing", "workers", "selection", "role", "run", "timing"):
                    modified = copy.deepcopy(original)
                    if change == "missing":
                        path.unlink()
                    elif change == "workers":
                        modified["workers"] = 2
                    elif change == "selection":
                        modified["selected_groups"] = ["0"]
                    elif change == "role":
                        modified["shard"] = 0
                    elif change == "run":
                        modified["run_identity"]["GITHUB_RUN_ATTEMPT"] = "previous"
                    else:
                        modified["checks"][0]["seconds"] = 1050
                    if change != "missing":
                        runner.write_json(path, modified)
                    with self.subTest(change=change), self.assertRaises((ValueError, OSError)):
                        runner.aggregate(self.root, inputs, self.output, "success", "success")
                    runner.write_json(path, original)
                for options in ({"only": "0"}, {"shard": 0}, {"whole_groups": True}):
                    with self.subTest(options=options), self.assertRaisesRegex(ValueError, "mutually exclusive"):
                        runner.run_checks(
                            self.root, self.root / "verus", self.output,
                            collector_groups=True, **options,
                        )

    def test_whole_job_semantic_receipts_are_required_and_source_bound(self):
        with patch.object(runner, "WHOLE_GROUPS", ("0", "1")):
            inputs = self.collection_fixture()
            whole = inputs / (runner.artifact_prefix() + "whole")
            summary_path = whole / "semantic-groups" / runner.TRACE / runner.SUMMARY
            original = runner.read_json(summary_path)
            with patch.object(runner, "CHECKS", self.checks):
                result = runner.aggregate(self.root, inputs, self.output, "success", "success")
                self.assertEqual(result["selected_groups"], [row[0] for row in self.checks])
                self.assertEqual(len(result["checks"]), 8)
                self.assertGreater(result["measured_whole_group_seconds"], 0)
                for changed in ("missing", "workers", "selection", "role", "run", "timing"):
                    summary = copy.deepcopy(original)
                    if changed == "missing":
                        summary_path.unlink()
                    elif changed == "workers":
                        summary["workers"] = 2
                    elif changed == "selection":
                        summary["selected_groups"] = ["0"]
                    elif changed == "role":
                        summary["shard"] = 0
                    elif changed == "run":
                        summary["run_identity"]["GITHUB_RUN_ID"] = "previous"
                    else:
                        summary["checks"][0]["seconds"] = 1050
                    if changed != "missing":
                        runner.write_json(summary_path, summary)
                    with self.subTest(changed=changed), self.assertRaises((ValueError, OSError)):
                        runner.aggregate(self.root, inputs, self.output, "success", "success")
                    runner.write_json(summary_path, original)

    def test_whole_positive_receipt_is_source_tool_run_and_artifact_bound(self):
        whole = self.root / "whole"
        self.whole_fixture(whole)
        with patch.object(runner, "CHECKS", self.checks):
            runner.seal_whole(self.root, whole)
            baseline = runner.load_whole_baseline(self.root, whole)
            selected = runner.baseline_from_whole(baseline, ["module::function"])
            self.assertTrue(runner.baseline_passed(selected, baseline, ["module::function"]))
            self.assertFalse(runner.baseline_passed(selected))
            self.assertFalse(runner.baseline_passed(selected, baseline, ["other::function"]))
            for names in ([], ["unknown"], ["module::function"] * 2):
                with self.subTest(names=names), self.assertRaises(ValueError):
                    runner.baseline_from_whole(baseline, names)
            for field in ("receipt_sha256", "report_sha256", "functions"):
                changed = copy.deepcopy(selected)
                changed["whole_positive"][field] = [] if field == "functions" else "0" * 64
                self.assertFalse(runner.baseline_passed(changed, baseline, ["module::function"]))
            summary = whole / runner.WHOLE_SUMMARY
            original = summary.read_bytes()
            for field, value in (
                ("status", "failed"), ("schema_version", 0), ("manifest", {}),
                ("run_identity", {"GITHUB_RUN_ATTEMPT": "other"}), ("files", {}),
            ):
                document = json.loads(original)
                document[field] = value
                runner.write_json(summary, document)
                with self.subTest(field=field), self.assertRaises(ValueError):
                    runner.load_whole_baseline(self.root, whole)
                summary.write_bytes(original)
            library = whole / "target/verus-production/libfixture.rlib"
            library.write_bytes(b"changed library")
            with self.assertRaisesRegex(ValueError, "modified"):
                runner.load_whole_baseline(self.root, whole)
            library.write_bytes(b"native compilation fixture")
            (self.root / "src/example.rs").write_text("changed source")
            with self.assertRaisesRegex(ValueError, "source/tool identity"):
                runner.load_whole_baseline(self.root, whole)

    def test_invalid_explicit_whole_baseline_never_falls_back_or_runs_a_checker(self):
        with patch.object(runner, "CHECKS", self.checks), patch.object(
            runner.subprocess, "run",
        ) as process, self.assertRaises(FileNotFoundError):
            runner.run_checks(
                self.root, self.root / "verus", self.output,
                whole_baseline=self.root / "missing",
            )
        process.assert_not_called()
        self.assertFalse((self.output / runner.SUMMARY).exists())

    def test_baseline_option_cannot_be_silently_ignored_in_nonexecution_modes(self):
        for mode in (["--matrix"], ["--seal-whole", "/whole"], ["--aggregate", "/inputs"]):
            with self.subTest(mode=mode), patch.object(
                sys, "argv", ["runner", *mode, "--whole-baseline", "/stale"],
            ), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                runner.main()
            self.assertEqual(stopped.exception.code, 2)

    def test_runner_passes_only_explicit_whole_baseline_to_isolated_checkers(self):
        whole = self.root / "whole"
        self.whole_fixture(whole)
        with patch.object(runner, "CHECKS", self.checks):
            runner.seal_whole(self.root, whole)
            for supplied in (None, whole):
                with patch.dict(os.environ, {"JBIG2_WHOLE_BASELINE": "/stale"}), patch.object(
                    runner.subprocess, "run", side_effect=self.passing_process,
                ) as process:
                    result = runner.run_checks(
                        self.root, self.root / "verus", self.output,
                        only="0", whole_baseline=supplied,
                    )
                self.assertEqual(result["status"], "passed")
                environment = process.call_args.kwargs["env"]
                if supplied is None:
                    self.assertNotIn("JBIG2_WHOLE_BASELINE", environment)
                else:
                    self.assertEqual(environment["JBIG2_WHOLE_BASELINE"], str(whole.resolve()))

    def collect(self, inputs, whole="success", semantic="success"):
        with patch.object(runner, "CHECKS", self.checks):
            return runner.aggregate(self.root, inputs, self.output, whole, semantic)

    def test_full_aggregate_preserves_canonical_report_layout(self):
        inputs = self.collection_fixture()
        result = self.collect(inputs)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["selected_groups"], [row[0] for row in self.checks])
        self.assertEqual(len(result["checks"]), len(self.checks))
        self.assertEqual(result["matrix_shards"], 6)
        self.assertEqual(result["matrix_max_parallel"], 4)
        self.assertTrue((self.output / "verus-production.json").is_file())
        self.assertTrue((self.output / "production-proof-coverage-ci.json").is_file())
        self.assertTrue((self.output / runner.TRACE / runner.SUMMARY).is_file())
        for identity, *_ in self.checks:
            self.assertTrue((self.output / runner.TRACE / f"{identity}-proof-sensitivity.json").is_file())

    def test_eight_shard_aggregate_requires_each_exact_producer(self):
        with patch.multiple(runner, **EIGHT_SHARD_TOPOLOGY):
            inputs = self.collection_fixture()
            result = self.collect(inputs)
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["matrix_shards"], 8)
            self.assertEqual(result["matrix_max_parallel"], 4)
            self.assertEqual(result["workers"], 1)
            for index in range(8):
                path = inputs / (runner.artifact_prefix() + f"shard-{index}")
                moved = self.root / "removed"
                path.rename(moved)
                try:
                    with self.subTest(shard=index), self.assertRaises(ValueError):
                        self.collect(inputs)
                    self.assertFalse((self.output / runner.TRACE / runner.SUMMARY).exists())
                finally:
                    moved.rename(path)

    def test_capacity_aggregate_requires_all_sixteen_exact_producers(self):
        self.checks = tuple(
            (str(index), f"{index}.py", 1, "strong_contract", "weaker_control")
            for index in range(16)
        )
        for _, script, *_ in self.checks:
            (self.root / "tools" / script).write_text(
                'CASES = ({"id": "case", "module": "module", "function": "function"},)\n'
            )
        with patch.multiple(runner, **CURRENT_TOPOLOGY), patch.object(
            runner, "WHOLE_GROUPS", ("0",),
        ), patch.object(runner, "COLLECTOR_GROUPS", ("1",)):
            inputs = self.collection_fixture()
            result = self.collect(inputs)
            self.assertEqual(result["status"], "passed")
            self.assertEqual(result["matrix_shards"], 14)
            self.assertEqual(result["matrix_max_parallel"], 4)
            self.assertEqual(result["workers"], 1)
            for identity in ("whole", "collector", *(f"shard-{index}" for index in range(14))):
                path = inputs / (runner.artifact_prefix() + identity)
                moved = self.root / "removed"
                path.rename(moved)
                try:
                    with self.subTest(producer=identity), self.assertRaises(ValueError):
                        self.collect(inputs)
                    self.assertFalse((self.output / runner.TRACE / runner.SUMMARY).exists())
                finally:
                    moved.rename(path)

    def test_missing_whole_or_each_shard_fails_and_removes_stale_success(self):
        inputs = self.collection_fixture()
        for identity in ("whole", *(f"shard-{index}" for index in range(6))):
            path = inputs / (runner.artifact_prefix() + identity)
            moved = self.root / "removed"
            path.rename(moved)
            runner.write_json(self.output / runner.TRACE / runner.SUMMARY, {"status": "passed"})
            with self.subTest(identity=identity), self.assertRaises(ValueError):
                self.collect(inputs)
            self.assertFalse((self.output / runner.TRACE / runner.SUMMARY).exists())
            moved.rename(path)

    def test_job_results_fail_closed_even_with_complete_passing_artifacts(self):
        inputs = self.collection_fixture()
        for status in (None, "failure", "cancelled", "skipped", ""):
            for job in ("whole", "semantic"):
                with self.subTest(status=status, job=job), self.assertRaisesRegex(ValueError, "both succeed"):
                    self.collect(inputs, **{job: status})

    def test_failed_stale_duplicate_manifest_tool_and_source_evidence_are_rejected(self):
        inputs = self.collection_fixture()
        summary_path = inputs / (runner.artifact_prefix() + "shard-0") / runner.TRACE / runner.SUMMARY
        original = runner.read_json(summary_path)
        mutations = [
            lambda r: r.update(status="failed"),
            lambda r: r.update(schema_version=1),
            lambda r: r.update(schema_version=2),
            lambda r: r.update(shard=1),
            lambda r: r.update(shard=False),
            lambda r: r.update(workers=True),
            lambda r: r.update(workers=2),
            lambda r: r["checks"][0].update(status="failed"),
            lambda r: r["checks"][0].update(exit_code=1),
            lambda r: r["checks"][0].update(exit_code=False),
            lambda r: r["checks"][0].update(checker_sha256="0" * 64),
            lambda r: r["checks"][0].update(counterexamples=2),
            lambda r: r["checks"][0].update(counterexamples=True),
            lambda r: r["checks"][0].update(report_sha256="0" * 64),
            lambda r: r["checks"].extend(r["checks"]),
            lambda r: r["checks"].pop(),
            lambda r: r.update(selected_groups=["0", "0"]),
            lambda r: r["manifest"]["groups"].pop(),
            lambda r: r["manifest"]["verus"].update(version="floating"),
            lambda r: r["manifest"]["source_bindings"].update({"src/example.rs": "0" * 64}),
            lambda r: r["run_identity"].update(GITHUB_RUN_ATTEMPT="previous"),
        ]
        for mutate in mutations:
            report = copy.deepcopy(original)
            mutate(report)
            runner.write_json(summary_path, report)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                self.collect(inputs)
        runner.write_json(summary_path, original)
        (self.root / "src/example.rs").write_text("// changed source\n")
        with self.assertRaises(ValueError):
            self.collect(inputs)

    def test_success_shaped_report_rejected_even_with_updated_file_hash(self):
        inputs = self.collection_fixture()
        directory = inputs / (runner.artifact_prefix() + "shard-0") / runner.TRACE
        report_path = directory / "0-proof-sensitivity.json"
        summary_path = directory / runner.SUMMARY
        original_report = runner.read_json(report_path)
        original_summary = runner.read_json(summary_path)
        mutations = [
            lambda r: r.update(schema_version=1),
            lambda r: r.pop("source_bindings"),
            lambda r: r.update(source_bindings={"src/example.rs": "0" * 64}),
            lambda r: r["counterexamples"][0].pop("baseline_selected_verification"),
            lambda r: r["counterexamples"][0].update(id="other-case"),
            lambda r: r["counterexamples"][0].update(function="other::target"),
            lambda r: r["counterexamples"][0].update(strong_contract="accepted"),
            lambda r: r["counterexamples"][0]["baseline_selected_verification"].update(success=False),
        ]
        for mutate in mutations:
            report = copy.deepcopy(original_report)
            mutate(report)
            runner.write_json(report_path, report)
            summary = copy.deepcopy(original_summary)
            summary["checks"][0]["report_sha256"] = runner.digest(report_path)
            runner.write_json(summary_path, summary)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                self.collect(inputs)

    def test_whole_failures_and_native_inventory_traceability_are_required(self):
        inputs = self.collection_fixture()
        whole = inputs / (runner.artifact_prefix() + "whole")
        for relative in (
            runner.WHOLE_SUMMARY, "verus-production.json", "production-proof-coverage-ci.json",
            "target/traceability/verifier-report.json", "target/verus-production/libfixture.rlib",
        ):
            path = whole / relative
            original = path.read_bytes()
            path.unlink()
            with self.subTest(missing=relative), self.assertRaises((ValueError, OSError)):
                self.collect(inputs)
            path.write_bytes(original)
        report = runner.read_json(whole / "verus-production.json")
        for mutate in (
            lambda r: r["verification-results"].update(success=False),
            lambda r: r["verification-results"].update(**{"is-verifying-entire-crate": False}),
            lambda r: r.update(**{"func-details": {}}),
            lambda r: r["verus"].update(version="floating"),
        ):
            changed = copy.deepcopy(report)
            mutate(changed)
            runner.write_json(whole / "verus-production.json", changed)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                self.collect(inputs)
        runner.write_json(whole / "verus-production.json", report)
        inventory = whole / "production-proof-coverage-ci.json"
        inventory.write_text(inventory.read_text() + " ")
        with self.assertRaisesRegex(ValueError, "inventory differs"):
            self.collect(inputs)
        inventory.write_bytes((self.root / "verification/production-proof-coverage.json").read_bytes())
        trace = whole / runner.TRACE / "verifier-report.json"
        changed = runner.read_json(trace)
        changed["ledger_sha256"] = "0" * 64
        runner.write_json(trace, changed)
        with self.assertRaisesRegex(ValueError, "traceability"):
            self.collect(inputs)

    def test_extra_artifacts_and_duplicate_json_keys_fail(self):
        inputs = self.collection_fixture()
        extra = inputs / (runner.artifact_prefix() + "duplicate")
        extra.mkdir()
        with self.assertRaises(ValueError):
            self.collect(inputs)
        extra.rmdir()
        summary = inputs / (runner.artifact_prefix() + "shard-0") / runner.TRACE / runner.SUMMARY
        summary.write_text('{"status":"failed","status":"passed"}')
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            self.collect(inputs)

    def test_missing_or_unexpected_group_files_inside_shard_are_rejected(self):
        inputs = self.collection_fixture()
        shard = inputs / (runner.artifact_prefix() + "shard-0")
        report = shard / runner.TRACE / "0-proof-sensitivity.json"
        original = report.read_bytes()
        report.unlink()
        with self.assertRaises((ValueError, OSError)):
            self.collect(inputs)
        report.write_bytes(original)
        extra = shard / "unbound.json"
        extra.write_text("{}")
        with self.assertRaisesRegex(ValueError, "outside"):
            self.collect(inputs)

    def test_aggregate_cli_missing_inputs_exits_nonzero_with_failure_diagnostic(self):
        with patch.object(runner, "ROOT", self.root), patch.object(sys, "argv", [
            "runner", "--aggregate", str(self.root / "missing"), "--output-dir", str(self.output),
            "--whole-result", "success", "--semantic-result", "success",
        ]), redirect_stderr(io.StringIO()):
            self.assertEqual(runner.main(), 1)
        self.assertEqual(runner.read_json(self.output / "aggregation-failure.json")["status"], "failed")

    def test_unknown_selector_cli_is_nonzero(self):
        with patch.object(runner, "ROOT", self.root), patch.object(sys, "argv", [
            "runner", "--only", "not-configured", "--output-dir", str(self.output),
            "--verus-executable", str(self.root / "verus"),
        ]), redirect_stderr(io.StringIO()):
            self.assertEqual(runner.main(), 1)


if __name__ == "__main__":
    unittest.main()
