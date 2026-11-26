import contextlib
import hashlib
import importlib.util
import io
import json
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


package = load_module("audit_package", ROOT / "tools/audit_package.py")
itu = load_module("run_itu", ROOT / "validation/conformance/run_itu.py")


def generated_bmp(pixels, white_first, top_down):
    height, width = len(pixels), len(pixels[0])
    packed = bytearray()
    rows = []
    for row in pixels:
        expected = bytearray((width + 7) // 8)
        encoded = bytearray((width + 31) // 32 * 4)
        for x, black in enumerate(row):
            if black:
                expected[x // 8] |= 0x80 >> (x % 8)
            if bool(black) == white_first:
                encoded[x // 8] |= 0x80 >> (x % 8)
        for x in range(width, len(encoded) * 8):
            encoded[x // 8] |= 0x80 >> (x % 8)
        packed.extend(expected)
        rows.append(bytes(encoded))
    palette = (b"\xff\xff\xff\x00", b"\x00\x00\x00\x00")
    if not white_first:
        palette = palette[::-1]
    body = b"".join(rows if top_down else rows[::-1])
    header = (
        struct.pack("<2sIHHI", b"BM", 62 + len(body), 0, 0, 62)
        + struct.pack("<IiiHHIIiiII", 40, width, -height if top_down else height,
                      1, 1, 0, len(body), 0, 0, 2, 0)
    )
    return header + b"".join(palette) + body, bytes(packed)


class BitmapNormalizationTests(unittest.TestCase):
    def test_both_runners_match_independent_pixels_for_palette_padding_orientation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            vectors = root / "generated"
            vectors.mkdir()
            wanted = {}
            for width in (1, 8, 9, 32, 33):
                pixels = [[(x + 2 * y) % 3 == 0 for x in range(width)] for y in range(3)]
                for white_first in (False, True):
                    for top_down in (False, True):
                        identity = f"fixture-{width}-{white_first}-{top_down}"
                        encoded, packed = generated_bmp(pixels, white_first, top_down)
                        path = vectors / f"{identity}_TT00.bmp"
                        path.write_bytes(encoded)
                        (vectors / f"{identity}.jb2").write_bytes(b"original fixture; decoder is mocked")
                        wanted[identity] = (width, 3, packed)
                        self.assertEqual(itu.parse_bmp(path), wanted[identity])
            native, poppler = root / "oracle", root / "pdfimages"
            native.write_bytes(b"first-party placeholder, not executed")
            poppler.write_bytes(b"first-party placeholder, not executed")
            output = root / "result.json"

            def fake_oracle(command, **kwargs):
                if command[-1] in ("--version", "-v"):
                    return subprocess.CompletedProcess(command, 0, "fixture version", "")
                if command[0] == str(native):
                    identity = Path(command[-1]).stem
                    destination = Path(command[2])
                else:
                    prefix = Path(command[-1])
                    identity = prefix.name.removesuffix("-image")
                    destination = prefix.with_name(prefix.name + "-000.pbm")
                width, height, packed = wanted[identity]
                destination.write_bytes(f"P4\n{width} {height}\n".encode() + packed)
                return subprocess.CompletedProcess(command, 0, "", "")

            with (
                patch.object(sys, "argv", [
                    "run_oracles.py", "--vectors", str(root), "--jbig2dec", str(native),
                    "--pdfimages", str(poppler), "--output", str(output),
                ]),
                patch.object(subprocess, "run", side_effect=fake_oracle),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                differential = load_module(
                    "normalization_oracles", ROOT / "validation/differential/run_oracles.py"
                )
            report = json.loads(output.read_text())
            self.assertEqual(len(report["cases"]), 20)
            for case in report["cases"]:
                with self.subTest(case=case["id"]):
                    expected = wanted[case["id"]]
                    self.assertEqual(
                        differential.parse_bmp(vectors / f"{case['id']}_TT00.bmp"), expected
                    )
                    self.assertEqual(case["expected_bitmap_sha256"], [
                        hashlib.sha256(expected[2]).hexdigest()
                    ])
                    self.assertTrue(all(entry["match"] for entry in case["oracles"].values()))

    def test_page_pairing_is_numeric_and_checks_each_page_number_and_bitmap(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stream = root / "case_TT1.jb2"
            stream.write_bytes(b"original first-party fixture")
            expected = {}
            for page in reversed(range(11)):
                pixels = [
                    [bool(page & (1 << x)) if x < 4 else y == 1 for x in range(9)]
                    for y in range(2)
                ]
                bmp, packed = generated_bmp(pixels, page % 2 == 0, page % 3 == 0)
                (root / f"case_TT1_TT{page}.bmp").write_bytes(bmp)
                expected[page] = packed
            (root / "case_TT10_TT00.bmp").write_bytes(b"unrelated sibling is not a page")
            self.assertEqual(
                [path.name for path in itu.expected_pages(stream)],
                [f"case_TT1_TT{page}.bmp" for page in range(11)],
            )

            def decode(command, **kwargs):
                output = Path(command[2])
                output.mkdir()
                for page, packed in expected.items():
                    (output / f"page-{page + 1:08}.raw").write_bytes(
                        b"JB2R" + struct.pack(">IIII", page + 1, 9, 2, 2) + packed
                    )
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(itu.subprocess, "run", side_effect=decode):
                case = itu.run_case(stream, root / "mock-decoder", root / "output", 1, None)
            self.assertEqual(case["status"], "passed")
            self.assertEqual([page["index"] for page in case["pages"]], list(range(11)))


class PackageAuditTests(unittest.TestCase):
    def test_complete_inventory(self):
        self.assertEqual(package.audit(sorted(package.REQUIRED)), [])

    def test_archive_root(self):
        paths = [f"jbig2-decoder-0.0.1/{path}" for path in sorted(package.REQUIRED)]
        self.assertEqual(package.audit(paths), [])

    def test_windows_cargo_paths(self):
        paths = [path.replace("/", "\\") for path in sorted(package.REQUIRED)]
        self.assertEqual(package.audit(paths), [])
        prefixed = [f"jbig2-decoder-0.0.1\\{path}" for path in paths]
        self.assertEqual(package.audit(prefixed), [])
        self.assertTrue(package.audit(paths + ["src/lib.rs"]))

    def test_reject_empty_and_missing(self):
        self.assertTrue(package.audit([]))
        self.assertTrue(package.audit(["Cargo.toml"]))
        self.assertTrue(package.audit(sorted(package.REQUIRED - {"src/generic_region/fast.rs"})))

    def test_reject_unexpected_and_malformed(self):
        for path in (
            "proofs/hidden.rs", "specs/model.rs", "validation/stream.jb2",
            "src/extra.rs", "docs/standard.pdf", "../src/lib.rs",
            "/src/lib.rs", "C:\\src\\lib.rs", "src\\..\\lib.rs",
            "src//lib.rs", "./src/lib.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(package.audit(sorted(package.REQUIRED) + [path]))

    def test_reject_duplicates_and_mixed_roots(self):
        self.assertTrue(package.audit(sorted(package.REQUIRED) + ["src/lib.rs"]))
        self.assertTrue(package.audit(sorted(package.REQUIRED) + ["jbig2-decoder-0.0.1/LICENSE"]))
        self.assertTrue(package.audit([
            "jbig2-decoder-0.0.1/src/lib.rs", "jbig2-decoder-0.0.2/Cargo.toml",
        ]))

    def test_cli_exit_status(self):
        for manifest, status in (("", 1), ("\n".join(sorted(package.REQUIRED)), 0)):
            process = subprocess.run(
                [sys.executable, str(ROOT / "tools/audit_package.py")],
                input=manifest, capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, status, process.stderr)


class ConformanceGateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.vectors = self.root / "generated"
        self.vectors.mkdir()
        self.stream = self.vectors / "case.jb2"
        self.stream.write_bytes(b"generated fixture, not an ITU vector")
        bitmap = (
            struct.pack("<2sIHHI", b"BM", 66, 0, 0, 62)
            + struct.pack("<IiiHHIIiiII", 40, 1, 1, 1, 1, 0, 4, 0, 0, 2, 0)
            + b"\xff\xff\xff\x00\x00\x00\x00\x00"
            + b"\x80\x00\x00\x00"
        )
        (self.vectors / "case_TT00.bmp").write_bytes(bitmap)
        self.manifest = self.root / "manifest.json"
        self.output = self.root / "result.json"
        self.write_manifest()

    def write_manifest(self):
        files = []
        for path in sorted(self.vectors.iterdir()):
            data = path.read_bytes()
            files.append({
                "path": f"generated/{path.name}",
                "sha256": hashlib.sha256(data).hexdigest(),
                "size_bytes": len(data),
                "kind": "encoded_stream" if path.suffix == ".jb2" else "expected_bitmap",
            })
        self.manifest.write_text(json.dumps({"files": files}), encoding="utf-8")

    def invoke(self, *extra, decoder=None, vectors=None):
        argv = [
            "run_itu.py", "--vectors", str(vectors or self.root),
            "--manifest", str(self.manifest), "--decoder", str(self.root / "decoder"),
            "--output", str(self.output), *extra,
        ]
        with (
            patch.object(sys, "argv", argv),
            patch.object(itu.subprocess, "run", side_effect=decoder or self.decode),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            return itu.main()

    def decode(self, command, **kwargs):
        self.assertGreater(kwargs["timeout"], 0)
        output = Path(command[2])
        output.mkdir()
        (output / "page-00000001.raw").write_bytes(
            b"JB2R" + struct.pack(">IIII", 1, 1, 1, 1) + b"\x80"
        )
        return subprocess.CompletedProcess(command, 0, "", "")

    def test_generated_exact_output_both_layouts(self):
        for vectors in (self.root, self.vectors):
            self.assertEqual(self.invoke(vectors=vectors), 0)
            report = json.loads(self.output.read_text())
            self.assertEqual(report["passed"], 1)
            self.assertEqual(report["unresolved_failures"], 0)

    def test_reject_empty_manifest(self):
        self.manifest.write_text('{"files":[]}', encoding="utf-8")
        self.assertEqual(self.invoke(), 1)

    def test_reject_no_streams(self):
        self.stream.unlink()
        self.write_manifest()
        self.assertEqual(self.invoke(), 1)

    def test_reject_duplicate_json_and_manifest_paths(self):
        original = self.manifest.read_text()
        self.manifest.write_text('{"files":[], "files":[]}', encoding="utf-8")
        self.assertEqual(self.invoke(), 1)
        document = json.loads(original)
        document["files"].append(document["files"][0])
        self.manifest.write_text(json.dumps(document), encoding="utf-8")
        self.assertEqual(self.invoke(), 1)

    def test_reject_missing_unexpected_and_modified_vectors(self):
        original = self.stream.read_bytes()
        self.stream.unlink()
        self.assertEqual(self.invoke(), 1)
        self.stream.write_bytes(original + b"changed")
        self.assertEqual(self.invoke(), 1)
        self.stream.write_bytes(original)
        (self.vectors / "extra.jb2").write_bytes(b"extra")
        self.assertEqual(self.invoke(), 1)

    def test_reject_manifest_traversal(self):
        document = json.loads(self.manifest.read_text())
        document["files"][0]["path"] = "../case.jb2"
        self.manifest.write_text(json.dumps(document), encoding="utf-8")
        self.assertEqual(self.invoke(), 1)

    def test_reject_archive_mismatch(self):
        document = json.loads(self.manifest.read_text())
        document["container_sha256"] = "0" * 64
        self.manifest.write_text(json.dumps(document), encoding="utf-8")
        archive = self.root / "archive.zip"
        archive.write_bytes(b"wrong archive")
        self.assertEqual(self.invoke("--archive", str(archive)), 1)

    def test_reject_missing_expected_pages(self):
        (self.vectors / "case_TT00.bmp").unlink()
        self.write_manifest()
        self.assertEqual(self.invoke(), 1)

    def test_reject_noncontiguous_expected_pages(self):
        (self.vectors / "case_TT00.bmp").rename(self.vectors / "case_TT02.bmp")
        self.write_manifest()
        self.assertEqual(self.invoke(), 1)

    def test_reject_mismatching_pixels(self):
        def decode(command, **kwargs):
            result = self.decode(command, **kwargs)
            path = Path(command[2]) / "page-00000001.raw"
            path.write_bytes(path.read_bytes()[:-1] + b"\x00")
            return result
        self.assertEqual(self.invoke(decoder=decode), 1)
        self.assertEqual(json.loads(self.output.read_text())["cases"][0]["status"], "bitmap_mismatch")

    def test_reject_page_count_mismatch(self):
        def decode(command, **kwargs):
            return subprocess.CompletedProcess(command, 0, "", "")
        self.assertEqual(self.invoke(decoder=decode), 1)
        self.assertEqual(json.loads(self.output.read_text())["cases"][0]["status"], "page_count_mismatch")

    def test_reject_malformed_raw_output(self):
        def decode(command, **kwargs):
            result = self.decode(command, **kwargs)
            (Path(command[2]) / "page-00000001.raw").write_bytes(b"truncated")
            return result
        self.assertEqual(self.invoke(decoder=decode), 1)

    def test_reject_invalid_raw_layout_and_page_number(self):
        for page, width, height, stride in ((2, 1, 1, 1), (1, 0, 1, 1), (1, 1, 1, 2)):
            def decode(command, **kwargs):
                result = self.decode(command, **kwargs)
                (Path(command[2]) / "page-00000001.raw").write_bytes(
                    b"JB2R" + struct.pack(">IIII", page, width, height, stride) + b"\x80"
                )
                return result
            self.assertEqual(self.invoke(decoder=decode), 1)

    def test_reject_timeout_and_decoder_errors(self):
        for failure in (
            subprocess.TimeoutExpired("decoder", 60),
            subprocess.CompletedProcess([], 1, "", "unexpected failure"),
        ):
            def decode(command, **kwargs):
                if isinstance(failure, Exception):
                    raise failure
                return failure
            self.assertEqual(self.invoke(decoder=decode), 1)
            self.assertEqual(json.loads(self.output.read_text())["unresolved_failures"], 1)

    def difference_file(self, **overrides):
        entry = {
            "id": "case", "classification": "reviewed_strict_rejection",
            "expected_error_exact": "reviewed error", "rationale": "generated test expectation",
            "stream_sha256": hashlib.sha256(self.stream.read_bytes()).hexdigest(),
            "source_files_sha256": {
                "src/lib.rs": hashlib.sha256((ROOT / "src/lib.rs").read_bytes()).hexdigest(),
            },
        }
        entry.update(overrides)
        if entry["classification"] == "unsupported_profile":
            for key in ("expected_error_exact", "stream_sha256", "source_files_sha256"):
                del entry[key]
            entry["expected_error_contains"] = "reviewed error"
        path = self.root / "differences.json"
        path.write_text(json.dumps({"schema_version": 1, "cases": [entry]}), encoding="utf-8")
        return path

    def pending_file(self, **overrides):
        entry = {
            "classification": "pending_normative_review",
            "expected_error_exact": "reviewed error",
            "stream_sha256": hashlib.sha256(self.stream.read_bytes()).hexdigest(),
            "source_files_sha256": {
                "src/lib.rs": hashlib.sha256((ROOT / "src/lib.rs").read_bytes()).hexdigest(),
            },
        }
        entry.update(overrides)
        path = self.difference_file(**entry)
        document = json.loads(path.read_text())
        document["cases"][0].pop("expected_error_contains", None)
        path.write_text(json.dumps(document))
        return path

    def test_pending_exact_rejection_remains_unresolved(self):
        path = self.pending_file()
        def decode(command, **kwargs):
            return subprocess.CompletedProcess(command, 1, "", "reviewed error")
        self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 1)
        report = json.loads(self.output.read_text())
        self.assertEqual(report["pending_normative_reviews"], 1)
        self.assertEqual(report["reviewed_strict_rejections"], 0)
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["unresolved_failures"], 1)

    def test_pending_review_accepts_exact_nested_source_bindings_without_granting_credit(self):
        relative = "src/bitmap/storage.rs"
        path = self.pending_file(source_files_sha256={
            relative: hashlib.sha256((ROOT / relative).read_bytes()).hexdigest(),
        })
        def decode(command, **kwargs):
            return subprocess.CompletedProcess(command, 1, "", "reviewed error")
        self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 1)
        report = json.loads(self.output.read_text())
        self.assertEqual(report["pending_normative_reviews"], 1)
        self.assertEqual(report["reviewed_strict_rejections"], 0)
        self.assertEqual(report["unresolved_failures"], 1)

    def test_pending_diagnosis_does_not_match_substrings_crashes_or_changed_success(self):
        path = self.pending_file()
        for code, error in ((1, "prefix reviewed error"), (1, "reviewed error suffix"),
                            (-11, "reviewed error"), (2, "reviewed error"), (0, "")):
            with self.subTest(code=code, error=error):
                def decode(command, **kwargs):
                    return subprocess.CompletedProcess(command, code, "", error)
                self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 1)
                report = json.loads(self.output.read_text())
                self.assertEqual(report["pending_normative_reviews"], 0)
                self.assertEqual(report["unresolved_failures"], 1)

    def test_pending_schema_stream_and_source_fail_closed_and_remove_stale_report(self):
        for overrides in (
            {"stream_sha256": "0" * 64}, {"stream_sha256": 1},
            {"expected_error_exact": ""}, {"source_files_sha256": {}},
            {"source_files_sha256": {"../src/lib.rs": "0" * 64}},
            {"source_files_sha256": {"src/../lib.rs": "0" * 64}},
            {"source_files_sha256": {"/src/lib.rs": "0" * 64}},
            {"source_files_sha256": {"src/lib.rs": "0" * 64}},
            {"source_files_sha256": {"src/absent.rs": "0" * 64}},
            {"source_files_sha256": {"src/lib.rs": True}}, {"unexpected": True},
        ):
            with self.subTest(overrides=overrides):
                path = self.pending_file(**overrides)
                self.output.write_text('{"stale_success": true}')
                self.assertEqual(self.invoke("--expected-differences", str(path)), 1)
                self.assertFalse(self.output.exists())

    def test_expected_difference_schema_and_non_json_constants_fail_closed(self):
        for text in (
            '{"schema_version":true,"cases":[]}',
            '{"schema_version":2,"cases":[]}',
            '{"schema_version":1,"cases":null}',
            '{"schema_version":1,"cases":[],"extra":0}',
            '{"schema_version":1,"cases":[],"extra":NaN}',
        ):
            path = self.root / "invalid.json"
            path.write_text(text)
            self.output.write_text('{"stale_success": true}')
            self.assertEqual(self.invoke("--expected-differences", str(path)), 1)
            self.assertFalse(self.output.exists())

    def test_expected_difference_is_separate_from_exact_pass(self):
        path = self.difference_file()
        def decode(command, **kwargs):
            return subprocess.CompletedProcess(command, 1, "", "reviewed error")
        self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 0)
        report = json.loads(self.output.read_text())
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["reviewed_strict_rejections"], 1)

    def test_reviewed_rejections_keep_exact_error_exit_stream_and_source_guards(self):
        path = self.difference_file()
        for code, error in ((1, "prefix reviewed error"), (1, "reviewed error suffix"),
                            (-11, "reviewed error"), (2, "reviewed error"), (0, "")):
            with self.subTest(code=code, error=error):
                def decode(command, **kwargs):
                    return subprocess.CompletedProcess(command, code, "", error)
                self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 1)
                report = json.loads(self.output.read_text())
                self.assertEqual(report["reviewed_strict_rejections"], 0)
                self.assertEqual(report["unresolved_failures"], 1)
        for overrides in (
            {"stream_sha256": "0" * 64}, {"source_files_sha256": {}},
            {"source_files_sha256": {"src/lib.rs": "0" * 64}},
            {"expected_error_contains": "reviewed error"},
        ):
            with self.subTest(overrides=overrides):
                path = self.difference_file(**overrides)
                self.output.write_text('{"stale_success": true}')
                self.assertEqual(self.invoke("--expected-differences", str(path)), 1)
                self.assertFalse(self.output.exists())

    def test_unsupported_profile_does_not_require_bilevel_expected_bitmap(self):
        (self.vectors / "case_TT00.bmp").write_bytes(b"generated unsupported colour fixture")
        self.write_manifest()
        path = self.difference_file(classification="unsupported_profile")
        def decode(command, **kwargs):
            return subprocess.CompletedProcess(command, 1, "", "reviewed error")
        self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 0)
        self.assertEqual(json.loads(self.output.read_text())["unsupported_profile"], 1)

    def test_expected_difference_does_not_hide_crash_or_changed_behavior(self):
        path = self.difference_file()
        for code in (-11, 2):
            def decode(command, **kwargs):
                return subprocess.CompletedProcess(command, code, "", "reviewed error")
            self.assertEqual(self.invoke("--expected-differences", str(path), decoder=decode), 1)
        self.assertEqual(self.invoke("--expected-differences", str(path)), 1)

    def test_reject_invalid_or_unknown_expectations(self):
        for overrides in (
            {"classification": "passed"}, {"expected_error_contains": ""},
            {"id": "unknown"}, {"rationale": ""},
        ):
            path = self.difference_file(**overrides)
            self.assertEqual(self.invoke("--expected-differences", str(path)), 1)

    def test_cli_empty_inventory_is_nonzero(self):
        self.manifest.write_text('{"files":[]}', encoding="utf-8")
        process = subprocess.run(
            [
                sys.executable, str(ROOT / "validation/conformance/run_itu.py"),
                "--vectors", str(self.root), "--manifest", str(self.manifest),
                "--decoder", "missing-decoder", "--output", str(self.output),
            ],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(process.returncode, 1)
        self.assertIn("empty conformance manifest", process.stderr)


if __name__ == "__main__":
    unittest.main()
