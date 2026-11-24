import collections
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import sys
import zlib

ROOT = Path(os.environ["JBIG2_EVAL_RESULTS"]).resolve()
HAYRO = Path(os.environ["JBIG2_HAYRO_TESTS"]).resolve()
from pypdf.filters import FlateDecode
from pypdf.generic import DictionaryObject, NameObject, NumberObject


def png_reference(path):
    source = path.read_bytes()
    assert source[:8] == b"\x89PNG\r\n\x1a\n"
    offset, compressed, palette = 8, bytearray(), None
    while offset < len(source):
        size = struct.unpack_from(">I", source, offset)[0]
        kind = source[offset + 4:offset + 8]
        data = source[offset + 8:offset + 8 + size]
        assert zlib.crc32(kind + data) == struct.unpack_from(">I", source, offset + 8 + size)[0]
        if kind == b"IHDR":
            width, height, bits, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", data)
            assert (compression, filtering, interlace) == (0, 0, 0)
        elif kind == b"PLTE":
            palette = data
        elif kind == b"IDAT":
            compressed.extend(data)
        offset += size + 12
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color]
    assert bits in (1, 2, 4, 8, 16)
    parameters = DictionaryObject({NameObject(k): NumberObject(v) for k, v in
        {"/Predictor": 15, "/Columns": width, "/Colors": channels, "/BitsPerComponent": bits}.items()})
    raw = FlateDecode.decode(bytes(compressed), parameters)
    stride = (width * channels * bits + 7) // 8
    assert len(raw) == stride * height
    packed = bytearray(b"\xff" * (((width + 7) // 8) * height))
    for y in range(height):
        for x in range(width):
            position = x * channels * bits
            byte = raw[y * stride + position // 8]
            value = (byte >> (8 - bits - position % 8)) & ((1 << bits) - 1) if bits < 8 else byte
            if color == 3:
                rgb = palette[3 * value:3 * value + 3]
                assert len(rgb) == 3 and rgb[0] == rgb[1] == rgb[2]
                value = rgb[0]
            elif color in (2, 6):
                step = bits // 8
                rgb = raw[y * stride + position // 8:y * stride + position // 8 + 3 * step:step]
                assert rgb[0] == rgb[1] == rgb[2]
            elif bits < 8:
                value = value * 255 // ((1 << bits) - 1)
            if value < 128:
                packed[y * ((width + 7) // 8) + x // 8] &= ~(0x80 >> (x % 8))
    return width, height, bytes(packed), hashlib.sha256(source).hexdigest()


def main():
    mapping = {}
    for path in HAYRO.glob("*.rs"):
        text = path.read_text()
        for name, reference in re.findall(r'(?:check|check_lenient|check_with_memory_limit)\s*\(\s*"([^"]+)"\s*,\s*"([^"]+)"', text):
            mapping[name] = [reference]
        for name, refs in re.findall(r'check_pages(?:_lenient)?\s*\(\s*"([^"]+)"\s*,\s*&\[(.*?)\]', text, re.S):
            mapping[name] = re.findall(r'"([^"]+)"', refs)
    manifest = json.loads((ROOT / "manifest.json").read_text())
    records = {r["id"]: r for r in json.loads((ROOT / "functionality.json").read_text())}
    results = []
    skipped = []
    for item in manifest["images"]:
        if item["group"] != "hayro":
            continue
        name = Path(item["original"]["path"]).stem
        refs = mapping.get(name)
        if not refs:
            skipped.append(dict(id=item["id"], reason="No direct uncropped reference mapping"))
            continue
        siblings = [i for i in manifest["images"] if i.get("original", {}).get("path") == item["original"]["path"]]
        index = siblings.index(item)
        if index >= len(refs):
            skipped.append(dict(id=item["id"], reason="Page/reference mapping differs"))
            continue
        path = HAYRO / "references" / (refs[index] + ".png")
        width, height, expected, digest = png_reference(path)
        if (width, height) != (item["width"], item["height"]):
            skipped.append(dict(id=item["id"], reason="Reference dimensions differ from requested canvas"))
            continue
        row = dict(id=item["id"], reference=str(path), reference_sha256=digest,
                   expected_packed_sha256=hashlib.sha256(expected).hexdigest())
        for backend in ("cpp", "rust_strict", "rust_best"):
            result = records[item["id"]][backend]
            if result["status"] != "ok":
                row[backend] = dict(status=result["status"])
                continue
            actual = Path(result["path"]).read_bytes()
            assert len(actual) == len(expected)
            row[backend] = dict(status="ok", different_pixels=sum((a ^ b).bit_count() for a, b in zip(actual, expected)))
        results.append(row)
    output = dict(records=results, skipped=skipped, summary={
        b: dict(collections.Counter("rejected" if i[b]["status"] != "ok" else
                                   "exact" if i[b]["different_pixels"] == 0 else "wrong_pixels" for i in results))
        for b in ("cpp", "rust_strict", "rust_best")})
    (ROOT / "reference-adjudication.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(output["summary"], indent=2), "references", len(results), "explicit skips", len(skipped))


if __name__ == "__main__":
    main()
