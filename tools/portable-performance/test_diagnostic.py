import hashlib
import json
import unittest
from unittest.mock import patch
from pathlib import Path

import build
import diagnostic
import fixtures


class SafetyTests(unittest.TestCase):
    def test_http_throttling_respects_retry_after_without_weakening_hashes(self):
        directory = Path("target/portable-performance/test-data")
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / "download-control"
        error = fixtures.urllib.error.HTTPError("https://example.invalid", 429, "slow", {"Retry-After": "60"}, None)
        response = unittest.mock.MagicMock()
        response.__enter__.return_value.read.return_value = base64.b64encode(b"fixture")
        try:
            with patch.object(fixtures, "LAST_FETCH", 0), patch.object(fixtures.time, "sleep") as sleep:
                with patch.object(fixtures.urllib.request, "urlopen", side_effect=[error, response]) as request:
                    result = fixtures.fetch("pdfium", fixtures.PDFIUM, "control.pdf", path)
                sleep.assert_any_call(60)
                self.assertEqual(request.call_count, 2)
            self.assertEqual(result["sha256"], hashlib.sha256(b"fixture").hexdigest())
        finally:
            if path.exists():
                path.unlink()

    def test_self_consistent_metadata_cannot_authorize_unmapped_generated_input(self):
        original_read = Path.read_text
        def altered(path, *args, **kwargs):
            text = original_read(path, *args, **kwargs)
            if path == fixtures.HERE / "cohort.json":
                value = json.loads(text)
                value["original_generated_allowlist"].append("unmapped")
                return json.dumps(value)
            if path == fixtures.HERE / "generated.json":
                value = json.loads(text)
                value["inputs"]["unmapped"] = "not-a-public-fixture"
                return json.dumps(value)
            return text
        with patch.object(Path, "read_text", altered), patch.object(fixtures, "fetch") as fetch:
            with self.assertRaisesRegex(ValueError, "allowlist"):
                fixtures.materialize()
            fetch.assert_not_called()

    def test_template_tool_must_match_frozen_executable_hash(self):
        with patch.object(fixtures, "fetch", return_value={"sha256": "wrong"}) as fetch:
            with self.assertRaisesRegex(ValueError, "template tool hash"):
                fixtures.materialize()
            self.assertEqual(fetch.call_count, 1)

    def test_candidate_cannot_build_a_golden_materializer(self):
        with patch.object(build, "command") as command:
            with self.assertRaisesRegex(ValueError, "Only BASE"):
                build.build("d66db50bca76c8650b29346592151d81427f8323",
                            "candidate", materializer=True)
            command.assert_not_called()

    def test_only_immutable_candidate_and_explicit_latency_authorization(self):
        with patch.object(build, "command") as command:
            for value in ("main", "HEAD", "d66db50", "a" * 39 + ";"):
                with self.assertRaises(ValueError):
                    build.revision(value)
            command.assert_not_called()
        with patch.object(diagnostic, "command") as command:
            with self.assertRaisesRegex(ValueError, "authorize"):
                diagnostic.measure(False)
            command.assert_not_called()

    def test_packed_hash_mismatch_never_regenerates_expected(self):
        directory = Path("target/portable-performance/test-data")
        directory.mkdir(parents=True, exist_ok=True)
        path = directory / "wrong-output"
        path.write_bytes(b"wrong")
        try:
            with self.assertRaisesRegex(ValueError, "hash mismatch"):
                fixtures.checked(path, {"bytes": 5, "sha256": hashlib.sha256(b"right").hexdigest()})
            self.assertEqual(path.read_bytes(), b"wrong")
        finally:
            path.unlink()

    def test_timing_source_must_remain_path_only_port(self):
        actual = (fixtures.HERE / "probe/src/main.rs").read_text()
        original_read = Path.read_text
        def altered(path, *args, **kwargs):
            if path == fixtures.HERE / "probe/src/main.rs":
                return actual.replace("for _ in 0..3", "for _ in 0..2")
            return original_read(path, *args, **kwargs)
        with patch.object(Path, "read_text", altered):
            with self.assertRaisesRegex(ValueError, "machine-path"):
                diagnostic.protocol_sources()

    def test_partial_journal_is_preserved_without_copying_private_payloads(self):
        root = Path("target/portable-performance/test-data/partial")
        results = root / "results"
        results.mkdir(parents=True, exist_ok=True)
        journal = root / "confirmation.sides.jsonl"
        pixels = root / "must-not-upload.packed"
        journal.write_text('{"complete_side":true}\n{"partial":')
        pixels.write_bytes(b"private test bytes")
        try:
            with patch.object(diagnostic, "PRIVATE", root), patch.object(diagnostic, "RESULTS", results):
                with patch("sys.argv", ["diagnostic.py", "stage-partial"]):
                    diagnostic.main()
            self.assertEqual((results / "confirmation-partial.sides.jsonl").read_bytes(),
                             journal.read_bytes())
            self.assertEqual([p.name for p in results.iterdir()], ["confirmation-partial.sides.jsonl"])
        finally:
            journal.unlink()
            pixels.unlink()
            (results / "confirmation-partial.sides.jsonl").unlink()


if __name__ == "__main__":
    unittest.main()
import base64
