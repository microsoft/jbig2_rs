import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

ROOT = Path(os.environ["JBIG2_EVAL_RESULTS"]).resolve()
from pypdf import PdfReader
from pypdf.errors import PdfReadError
from pypdf.generic import ArrayObject, DictionaryObject, EncodedStreamObject, IndirectObject, NameObject, StreamObject

PDFIUM = Path(os.environ["JBIG2_PDFIUM_SOURCE"]).resolve()
INPUTS = ROOT / "inputs"
INPUTS.mkdir(exist_ok=True)


def fingerprint(path):
    data = path.read_bytes()
    return dict(path=str(path), size=len(data), sha256=hashlib.sha256(data).hexdigest())


def save_input(name, data):
    path = INPUTS / name
    path.write_bytes(data)
    return path


def image_data(obj):
    filters = obj.get("/Filter", [])
    if not isinstance(filters, (list, tuple)):
        filters = [filters]
    index = filters.index("/JBIG2Decode")
    if index != len(filters) - 1:
        raise ValueError("JBIG2 is not the final filter")
    parms = obj.get("/DecodeParms", [])
    if not isinstance(parms, (list, tuple)):
        parms = [parms]
    parms = [i.get_object() for i in parms]
    params = parms[index] if index < len(parms) else None
    globals_obj = params.get("/JBIG2Globals") if isinstance(params, dict) else None
    globals_bytes = globals_obj.get_object().get_data() if globals_obj is not None else b""
    if not index:
        return obj._data, globals_bytes
    prefix = EncodedStreamObject()
    prefix._data = obj._data
    prefix[NameObject("/Filter")] = ArrayObject(filters[:index])
    if parms:
        prefix[NameObject("/DecodeParms")] = ArrayObject(parms[:index])
    return prefix.get_data(), globals_bytes


def extract_pdf(key, path, images):
    seen = set()
    count = 0

    def walk(obj):
        nonlocal count
        if isinstance(obj, IndirectObject):
            ident = (obj.idnum, obj.generation)
            if ident in seen:
                return
            seen.add(ident)
            obj = obj.get_object()
        if isinstance(obj, StreamObject) and obj.get("/Subtype") == "/Image":
            filters = obj.get("/Filter", [])
            if not isinstance(filters, (list, tuple)):
                filters = [filters]
            if "/JBIG2Decode" in filters:
                encoded, globals_bytes = image_data(obj)
                ident = hashlib.sha256(f"{key}:{count}".encode()).hexdigest()[:16]
                src = save_input(ident + ".jb2", encoded)
                glob = save_input(ident + ".globals", globals_bytes)
                images.append(dict(id=f"embedded/{ident}", group="embedded", pdf=key,
                                   width=int(obj["/Width"]), height=int(obj["/Height"]),
                                   image_mask=bool(obj.get("/ImageMask", False)),
                                   decode=str(obj.get("/Decode", "")),
                                   globals=fingerprint(glob), **fingerprint(src)))
                count += 1
        if isinstance(obj, DictionaryObject):
            for name, value in obj.items():
                if name not in ("/Parent", "/P"):
                    walk(value)
        elif isinstance(obj, ArrayObject):
            for value in obj:
                walk(value)

    walk(PdfReader(path).trailer)
    return count


def header(data, start):
    if start + 6 > len(data):
        raise ValueError("Truncated segment header")
    number = int.from_bytes(data[start:start + 4], "big")
    flags = data[start + 4]
    count = data[start + 5] >> 5
    pos = start + 6
    if count == 7:
        count = int.from_bytes(data[start + 5:start + 9], "big") & 0x1fffffff
        pos = start + 9 + (count + 1 + 7) // 8
    elif count > 4:
        raise ValueError("Reserved referred count")
    if count > 100000:
        raise ValueError("Unbounded reference list")
    pos += count * (1 if number <= 256 else 2 if number <= 65536 else 4)
    page_bytes = 4 if flags & 0x40 else 1
    if pos + page_bytes + 4 > len(data):
        raise ValueError("Truncated segment association/length")
    page = int.from_bytes(data[pos:pos + page_bytes], "big")
    page_offset = pos - start
    pos += page_bytes
    length = int.from_bytes(data[pos:pos + 4], "big")
    pos += 4
    return dict(kind=flags & 63, page=page, start=start, end=pos,
                length=length, header=data[start:pos], page_offset=page_offset,
                page_bytes=page_bytes)


def embedded_pages(data):
    if not data.startswith(b"\x97JB2\r\n\x1a\n"):
        raise ValueError("No standalone signature")
    flags = data[8]
    pos = 9 if flags & 2 else 13
    sequential = bool(flags & 1)
    segments = []
    while pos < len(data):
        value = header(data, pos)
        pos = value["end"]
        if sequential:
            if value["length"] == 0xffffffff:
                if not value["page"]:
                    raise ValueError("Unknown-length global segment")
                value["payload"] = data[pos:]
                value["unknown_tail"] = True
                segments.append(value)
                pos = len(data)
                break
            end = pos + value["length"]
            if end > len(data):
                raise ValueError("Truncated declared payload")
            value["payload"] = data[pos:end]
            pos = end
        segments.append(value)
        if value["kind"] == 51:
            break
    if not sequential:
        for value in segments:
            if value["length"] == 0xffffffff:
                raise ValueError("Unknown-length random-access segment")
            end = pos + value["length"]
            if end > len(data):
                raise ValueError("Truncated random-access payload")
            value["payload"] = data[pos:end]
            pos = end
    if pos != len(data):
        raise ValueError("Trailing container bytes")
    globals_bytes = b"".join(v["header"] + v["payload"] for v in segments if v["page"] == 0 and v["kind"] != 51)
    pages = []
    for info in (v for v in segments if v["kind"] == 48):
        if not info["page"] or len(info["payload"]) < 19:
            raise ValueError("Invalid page information")
        width, height = struct.unpack_from(">II", info["payload"])
        if height == 0xffffffff:
            stripes = [v for v in segments if v["page"] == info["page"] and v["kind"] == 50 and len(v["payload"]) == 4]
            if not stripes:
                raise ValueError("Unknown height without final stripe")
            height = int.from_bytes(stripes[-1]["payload"], "big") + 1
        parts = []
        for value in segments:
            if value["page"] != info["page"] or value["kind"] == 51:
                continue
            if value.get("unknown_tail") and value["page"] != 1:
                raise ValueError("Cannot renumber an unparsed unknown-length multipage tail")
            page_header = bytearray(value["header"])
            begin = value["page_offset"]
            page_header[begin:begin + value["page_bytes"]] = (1).to_bytes(value["page_bytes"], "big")
            parts.append(bytes(page_header) + value["payload"])
        page = b"".join(parts)
        pages.append((info["page"], width, height, page, globals_bytes))
    if not pages:
        raise ValueError("No page information")
    return pages


def wrap(image):
    data = Path(image["path"]).read_bytes()
    globals_bytes = Path(image["globals"]["path"]).read_bytes()
    width, height = image["width"], image["height"]
    content = f"q {width} 0 0 {height} 0 0 cm /Im0 Do Q\n".encode()
    params = " /DecodeParms << /JBIG2Globals 6 0 R >>" if globals_bytes else ""
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        (f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] "
         "/Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>").encode(),
        f"<< /Length {len(content)} >>\nstream\n".encode() + content + b"endstream",
        (f"<< /Type /XObject /Subtype /Image /Width {width} /Height {height} "
         f"/ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode{params} "
         f"/Length {len(data)} >>\nstream\n").encode() + data + b"\nendstream",
    ]
    if globals_bytes:
        objects.append(f"<< /Length {len(globals_bytes)} >>\nstream\n".encode() + globals_bytes + b"\nendstream")
    output = bytearray(b"%PDF-1.7\n")
    offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{index} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode())
    output.extend(f"trailer << /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(output)


def main():
    images, pdfs, errors = [], [], []
    for location in ("corpus", "resources"):
        directory = PDFIUM / "testing" / location
        sources = {}
        for path in sorted(directory.rglob("*.pdf")):
            if b"/JBIG2Decode" in path.read_bytes():
                sources[str(path.relative_to(directory))] = path
        for path in sorted(directory.rglob("*.in")):
            if b"/JBIG2Decode" not in path.read_bytes():
                continue
            key = str(path.relative_to(directory).with_suffix(".pdf"))
            if key in sources:
                continue
            output = INPUTS / "pdfs" / location / path.parent.relative_to(directory)
            output.mkdir(parents=True, exist_ok=True)
            subprocess.run([sys.executable, str(PDFIUM / "testing/tools/fixup_pdf_template.py"),
                            "--output-dir", str(output), str(path)], check=True)
            sources[key] = output / path.with_suffix(".pdf").name
        for key, path in sorted(sources.items()):
            identifier = f"{location}/{key}"
            before = len(images)
            try:
                count = extract_pdf(identifier, path, images)
            except (PdfReadError, ValueError, TypeError, KeyError, NotImplementedError) as error:
                errors.append(dict(id=identifier, stage="pdf-extraction", error=str(error)))
                count = len(images) - before
            pdfs.append(dict(id=identifier, group="pdf", extracted_images=count, **fingerprint(path)))
    hayro = Path(os.environ["JBIG2_HAYRO_TESTS"]).resolve() / "streams"
    for path in sorted(hayro.glob("*.jbig2")):
        try:
            pages = embedded_pages(path.read_bytes())
        except ValueError as error:
            errors.append(dict(id="hayro/" + path.name, stage="standalone-to-embedded", error=str(error), **fingerprint(path)))
            continue
        for number, width, height, page, globals_bytes in pages:
            stem = path.stem + f"-page{number}"
            src = save_input(stem + ".jb2", page)
            glob = save_input(stem + ".globals", globals_bytes)
            images.append(dict(id="hayro/" + stem, group="hayro", family=path.stem.split("_")[0],
                               width=width, height=height, globals=fingerprint(glob),
                               original=fingerprint(path), **fingerprint(src)))
    for image in list(images):
        if image["group"] != "hayro" or image.get("family") not in ("generic", "symbol", "huffman", "halftone", "refine"):
            continue
        if image["width"] * image["height"] > 32000000:
            continue
        selected = sum(i.get("family") == image["family"] and i.get("wrapped") for i in pdfs)
        if selected >= 3:
            continue
        path = save_input("wrapped-" + Path(image["path"]).stem + ".pdf", wrap(image))
        pdfs.append(dict(id="wrapped/" + path.name, group="pdf", wrapped=True,
                         family=image["family"], image_id=image["id"], extracted_images=1, **fingerprint(path)))
    (ROOT / "manifest.json").write_text(json.dumps(dict(images=images, pdfs=pdfs), indent=2) + "\n")
    (ROOT / "extraction-errors.json").write_text(json.dumps(errors, indent=2) + "\n")
    print(f"Frozen {len(images)} image requests and {len(pdfs)} PDF cases; {len(errors)} explicit extraction/adaptation exclusions.")
    for error in errors:
        print(error["id"], error["stage"], error["error"])


if __name__ == "__main__":
    main()
