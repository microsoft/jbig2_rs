import io
import struct
import unittest

from run import Frames


def number(value):
    return struct.pack(">Q", value)


def text(value):
    value = value.encode()
    return number(len(value)) + value


def frame(pixel=b"\0", resolution=0):
    prefix = b"CASE" + text("first-party-control") + b"\0\0\1"
    prefix += b"\0\0" + number(4) + number(1)
    prefix += b"".join(number(value) for value in (1, 1, 1, resolution, 0)) + b"\0"
    prefix += number(1) + b"".join(number(value) for value in (1, 1, 1, 1))
    return prefix + number(len(pixel)) + pixel + number(0) + b"DONE"


class LosslessFrameTests(unittest.TestCase):
    def test_pixel_and_metadata_changes_cannot_pass_as_shape_only_matches(self):
        baseline = Frames(io.BytesIO(frame())).outcome()
        wrong_pixel = Frames(io.BytesIO(frame(b"\x80"))).outcome()
        wrong_resolution = Frames(io.BytesIO(frame(resolution=1))).outcome()
        self.assertEqual(baseline["page_info"][0]["width"], wrong_pixel["page_info"][0]["width"])
        self.assertNotEqual(baseline["pages"][0]["packed"], wrong_pixel["pages"][0]["packed"])
        self.assertNotEqual(baseline["canonical_frame_sha256"], wrong_pixel["canonical_frame_sha256"])
        self.assertNotEqual(baseline["canonical_frame_sha256"], wrong_resolution["canonical_frame_sha256"])

    def test_truncated_frame_never_becomes_a_success_or_empty_bitmap(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            Frames(io.BytesIO(frame()[:-6])).outcome()

    def test_complete_error_payload_is_framed_not_display_projected(self):
        def error(needed):
            return (b"CASE" + text("error-control") + b"\1\0\0"
                    + text(f"Malformed(UnexpectedEof {{ offset: 7, needed: {needed}, remaining: 0 }})")
                    + b"DONE")
        before = Frames(io.BytesIO(error(1))).outcome()
        after = Frames(io.BytesIO(error(2))).outcome()
        self.assertEqual(before["status"], "error")
        self.assertEqual(before["phase"], "decode")
        self.assertNotEqual(before["error_debug"], after["error_debug"])
        self.assertNotEqual(before["canonical_frame_sha256"], after["canonical_frame_sha256"])


if __name__ == "__main__":
    unittest.main()
