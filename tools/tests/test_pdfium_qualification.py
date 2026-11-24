import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


BUNDLE = Path(__file__).resolve().parents[1] / "pdfium-qualification"
sys.path.insert(0, str(BUNDLE))
import configure
import export_run
import qualify


class ConsumerSourceTests(unittest.TestCase):
    def test_external_source_is_hash_checked_and_prepared_reuse_is_offline(self):
        data = b"first-party consumer fixture\n"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "consumer").mkdir()
            (root / "consumer/source.json").write_text(json.dumps({
                "source_url": "https://example.invalid/pinned-consumer",
                "size_bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
            }))
            with patch.object(configure, "BUNDLE", root):
                with patch.object(configure.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                    self.assertEqual(configure.consumer_source(), data)
                for wrong in (data[:-1], data + b"x", b"x" * len(data)):
                    with self.subTest(wrong=wrong), patch.object(
                        configure.urllib.request, "urlopen", return_value=io.BytesIO(wrong),
                    ), self.assertRaisesRegex(ValueError, "differs from the pinned original"):
                        configure.consumer_source()
                existing = root / "jbig2_eval.cpp"
                existing.write_bytes(data)
                with patch.object(configure.urllib.request, "urlopen") as fetch:
                    self.assertEqual(configure.consumer_source(existing), data)
                    fetch.assert_not_called()
                    existing.write_bytes(b"x" * len(data))
                    with self.assertRaisesRegex(ValueError, "differs from the pinned original"):
                        configure.consumer_source(existing)
                    with self.assertRaisesRegex(ValueError, "requires the original consumer"):
                        configure.consumer_source(root / "absent")
                    fetch.assert_not_called()


class QualificationCoverageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.receipts = BUNDLE / "runs/pr241-121bee27-n2"
        cls.rows = json.loads((cls.receipts / "renewed-performance.json").read_text())

    def test_complete_published_measurements_preserve_every_summary_and_interval(self):
        self.assertEqual(
            qualify.summarize_rows(self.rows, 9),
            json.loads((self.receipts / "renewed-summary.json").read_text()),
        )

    def test_missing_workloads_rounds_backends_and_duplicates_fail_closed(self):
        missing_case = [row for row in self.rows if row["id"] != self.rows[0]["id"]]
        for rows, reason in (
            (missing_case, "Incomplete frozen workload/round coverage"),
            (self.rows[:-1], "Incomplete frozen workload/round coverage"),
            (self.rows + [self.rows[0]], "Duplicate qualification measurement"),
        ):
            with self.subTest(reason=reason, rows=len(rows)), self.assertRaisesRegex(ValueError, reason):
                qualify.validate_performance_rows(rows, 9)
        for mode, reason in (
            ("backend", "backend coverage"), ("order", "predeclared rotation"),
            ("ratio", "Recorded ratio"), ("output", "frozen output"),
            ("samples", "Invalid repetitions"), ("nan", "Invalid repetitions"),
        ):
            rows = copy.deepcopy(self.rows)
            if mode == "backend":
                del rows[0]["candidate_strict"]
            elif mode == "order":
                rows[0]["order"].reverse()
            elif mode == "ratio":
                rows[0]["best_candidate_over_base"] = 0.1
            elif mode == "output":
                rows[0]["cpp"]["sha256"] = "0" * 64
            elif mode == "samples":
                rows[0]["candidate_best"]["runs"] = "1"
            else:
                rows[0]["candidate_best"]["p50_us"] = "NaN"
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, reason):
                qualify.validate_performance_rows(rows, 9)
        with self.assertRaisesRegex(ValueError, "at least nine"):
            qualify.validate_performance_rows(self.rows, 3)

    def test_invalid_summary_removes_a_stale_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            summary = root / "renewed-summary.json"
            summary.write_text('{"performance_gate":"PASS"}')
            missing = [row for row in self.rows if row["id"] != self.rows[0]["id"]]
            (root / "renewed-performance.json").write_text(json.dumps(missing))
            with self.assertRaisesRegex(ValueError, "Incomplete frozen workload"):
                qualify.summarize(SimpleNamespace(results=root, rounds=9))
            self.assertFalse(summary.exists())

    def test_export_rejects_missing_workload_and_removes_stale_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "output"
            output.mkdir()
            receipt = output / "receipt.json"
            receipt.write_text('{"performance_gate":"PASS"}')
            missing = [row for row in self.rows if row["id"] != self.rows[0]["id"]]
            (root / "renewed-performance.json").write_text(json.dumps(missing))
            (root / "renewed-summary.json").write_text((self.receipts / "renewed-summary.json").read_text())
            with patch.object(sys, "argv", [
                "export_run.py", "--results", str(root), "--output", str(output),
            ]), self.assertRaisesRegex(ValueError, "Incomplete frozen workload"):
                export_run.main()
            self.assertFalse(receipt.exists())

    def test_export_preserves_original_and_projected_error_and_recovery_diagnostics(self):
        original = {
            "status": "decode_error",
            "stderr": "warning\nJBIG2_ERROR Malformed(original)\nJBIG2_RECOVERY original\n",
            "path": "/temporary/pixels",
        }
        projected = export_run.outcome(original)
        self.assertEqual(projected["diagnostics"], [
            "JBIG2_ERROR Malformed(original)", "JBIG2_RECOVERY original",
        ])
        self.assertEqual(export_run.outcome(projected), projected)
        with self.assertRaisesRegex(ValueError, "neither original stderr"):
            export_run.outcome({"status": "ok"})
        with self.assertRaisesRegex(ValueError, "Invalid projected"):
            export_run.outcome({"status": "ok", "diagnostics": ["discarded warning"]})

    def test_complete_adverse_measurements_keep_failed_data_and_return_failure(self):
        rows = copy.deepcopy(self.rows)
        ident = rows[0]["id"]
        for row in rows:
            if row["id"] != ident:
                continue
            candidate = row["candidate_best"]
            value = float(row["base_best"]["p50_us"]) * 1.2
            candidate["p50_us"] = str(value)
            candidate["p90_us"] = str(value * 1.01)
            row["best_candidate_over_base"] = value / float(row["base_best"]["p50_us"])
            row["best_candidate_over_cpp"] = value / float(row["cpp"]["p50_us"])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "renewed-performance.json").write_text(json.dumps(rows))
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(qualify.summarize(SimpleNamespace(results=root, rounds=9)), 1)
            summary = json.loads((root / "renewed-summary.json").read_text())
            self.assertEqual(summary["performance_gate"], "FAIL")
            self.assertTrue(any(row["id"] == ident for row in summary["adverse_over_5_percent"]))
