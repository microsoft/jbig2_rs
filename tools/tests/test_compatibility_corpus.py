import copy
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "validation/compatibility/run_hayro_corpus.py"
SPEC = importlib.util.spec_from_file_location("compatibility_corpus", PATH)
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class CompatibilityCorpusTests(unittest.TestCase):
    def test_complete_pinned_manifest_rejects_missing_reports_and_unsafe_sources(self):
        manifest = json.loads(runner.MANIFEST.read_text())
        self.assertEqual(len(runner.validated_manifest(manifest)), 32)
        for field, value in (
            ("name", "../outside"),
            ("url", "https://example.invalid/fixture"),
            ("url", manifest["fixtures"][0]["url"] + "?mutable=1"),
            ("sha256", "not-a-checksum"),
            ("bytes", 16 * 1024 * 1024 + 1),
        ):
            changed = copy.deepcopy(manifest)
            changed["fixtures"][0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                runner.validated_manifest(changed)
        manifest["fixtures"].pop()
        with self.assertRaises(ValueError):
            runner.validated_manifest(manifest)

    def test_success_requires_every_exact_page_and_observable_recovery(self):
        import hashlib

        page = {
            "number": 1, "width": 8, "height": 1,
            "packed_sha256": hashlib.sha256(b"\xa5").hexdigest(),
        }
        row = {
            "strict_status": "error", "compatibility_status": "decoded",
            "expected_pages": [page], "recovery_contains": "ObservedPageCount",
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "page-1.raw").write_bytes(struct.pack(">4I", 1, 8, 1, 1) + b"\xa5")
            output = subprocess.CompletedProcess([], 0, "OK|1\nRECOVERIES|[ObservedPageCount]\n", "")
            self.assertEqual(runner.validate_output(row, "compat", output, directory)["pages"], [page])
            output.stdout = "OK|1\nRECOVERIES|[]\n"
            with self.assertRaises(ValueError):
                runner.validate_output(row, "compat", output, directory)
            output.stdout = "OK|1\nRECOVERIES|[ObservedPageCount]\n"
            (directory / "page-1.raw").write_bytes(struct.pack(">4I", 1, 8, 1, 1) + b"\xa4")
            with self.assertRaises(ValueError):
                runner.validate_output(row, "compat", output, directory)
            (directory / "page-1.raw").write_bytes(b"short")
            with self.assertRaises(ValueError):
                runner.validate_output(row, "compat", output, directory)

    def test_work_exhaustion_checks_the_effective_budget_and_inclusive_threshold(self):
        row = {"max_work_units": 100000, "bytes": 115}
        before = {
            "status": "error",
            "error": "LimitExceeded(LimitExceeded { kind: Work, limit: 100000, actual: 100001 })",
        }
        reader = {
            "status": "error",
            "error": before["error"],
        }
        self.assertEqual(
            runner.comparison_signature(row, "strict", before),
            runner.comparison_signature(row, "strict-reader", reader),
        )
        reader["error"] = "LimitExceeded(LimitExceeded { kind: Work, limit: 99140, actual: 99141 })"
        self.assertNotEqual(
            runner.comparison_signature(row, "strict", before),
            runner.comparison_signature(row, "compat", reader),
        )
        reader["error"] = reader["error"].replace("99141", "99140")
        with self.assertRaises(ValueError):
            runner.comparison_signature(row, "strict-reader", reader)


if __name__ == "__main__":
    unittest.main()
