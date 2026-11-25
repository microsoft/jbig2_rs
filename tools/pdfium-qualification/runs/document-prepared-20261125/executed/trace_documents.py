import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess

from pypdf import PdfReader
from pypdf.errors import PdfReadError
from pypdf.generic import IndirectObject

ROOT = Path(__file__).resolve().parent
ORIGINAL = ROOT.parent / "jbig2-qualification-transfer-20261124/results"


def limits():
    resource.setrlimit(resource.RLIMIT_DATA, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def inventory(path):
    reader = PdfReader(path)
    images = {}
    mask_parents = collections.defaultdict(list)
    for generation, table in reader.xref.items():
        for number in table:
            if not number:
                continue
            value = reader.get_object(IndirectObject(number, generation, reader))
            if not isinstance(value, dict) or value.get("/Subtype") != "/Image":
                continue
            for kind in ("/Mask", "/SMask"):
                target = value.raw_get(kind) if kind in value else None
                if isinstance(target, IndirectObject) and hasattr(target.get_object(), "get_data"):
                    mask_parents[target.idnum].append(dict(parent=number, kind=kind))
            filters = value.get("/Filter")
            filters = filters.get_object() if hasattr(filters, "get_object") else filters
            if isinstance(filters, list):
                filters = [item.get_object() for item in filters]
            if "/JBIG2Decode" in str(filters):
                images[number] = dict(image_mask=bool(value.get("/ImageMask", False)))
    for number, image in images.items():
        image["attached_as_mask"] = mask_parents[number]
    return dict(pages=len(reader.pages), jbig2_objects=images)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=ROOT / "pdfium/out/TraceCpp/jbig2_eval")
    parser.add_argument("--output", default="traces-cpp")
    parser.add_argument("--reference", default="cpp")
    parser.add_argument("--mode", default="pdf")
    args = parser.parse_args()
    os.sched_setaffinity(0, {3})
    output = ROOT / args.output
    output.mkdir(exist_ok=True)
    manifest = json.loads((ORIGINAL / "manifest.json").read_text())
    historical = {v["id"]: v for v in json.loads(
        (ROOT.parent / "jbig2-comparison-20261123/functionality.json").read_text())}
    items = list(manifest["pdfs"])
    for path in sorted((ROOT / "extra").glob("*.pdf")):
        items.append(dict(id="extra/" + path.name, path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    rows = []
    for item in items:
        ident = item["id"]
        path = Path(item["path"])
        assert hashlib.sha256(path.read_bytes()).hexdigest() == item["sha256"]
        name = hashlib.sha256(ident.encode()).hexdigest()[:20]
        target = output / (name + ".bin")
        command = [str(args.binary), args.mode, str(path), "-", str(target), "0", "0", "0"]
        run = subprocess.run(command, capture_output=True, text=True, timeout=30, preexec_fn=limits)
        assert run.returncode == 0, (ident, run.returncode, run.stderr[-2000:])
        (output / (name + ".log")).write_text(run.stderr)
        if args.mode == "pdf":
            status, width, height, channels, _, _, _ = run.stdout.strip().split("\t")
        else:
            response = json.loads(run.stdout)
            status = response["status"]
            width, height, channels = str(response["width"]), str(response["height"]), "4"
        result = dict(status=status, width=width, height=height, channels=channels)
        if status == "ok":
            result.update(sha256=hashlib.sha256(target.read_bytes()).hexdigest(), bytes=target.stat().st_size)
        if ident in historical and not args.mode.startswith("repeat_"):
            expected = historical[ident][args.reference]
            assert all(result.get(k) == expected.get(k) for k in result), (ident, result, expected)
        invocations, cache, owner_events, returns = [], [], [], []
        current_page = {}
        for line in run.stderr.splitlines():
            if line.startswith("JBIG2_PAGE_BEGIN "):
                current_page = dict(field.split("=", 1) for field in line.split()[1:])
            if line.startswith(("JBIG2_INVOKE ", "JBIG2_IMAGE_CACHE ")):
                kind, *fields = line.split()
                record = dict(field.split("=", 1) for field in fields)
                record.update(current_page)
                (invocations if kind == "JBIG2_INVOKE" else cache).append(record)
            elif line.startswith(("JBIG2_OWNER ", "JBIG2_OWNER_BYPASS ", "JBIG2_OWNER_DROP ", "JBIG2_RETURN ")):
                kind, *fields = line.split()
                record = dict(field.split("=", 1) for field in fields)
                record["kind"] = kind
                (returns if kind == "JBIG2_RETURN" else owner_events).append(record)
        try:
            inv = inventory(path)
        except PdfReadError as error:
            print("INVENTORY_ERROR", ident, type(error).__name__, str(error), flush=True)
            inv = dict(pages=None, jbig2_objects={},
                       inventory_error=dict(type=type(error).__name__, message=str(error)))
        groups = collections.defaultdict(list)
        for call in invocations:
            if int(call["global_bytes"]):
                groups[call["global_sha"]].append(call)
        sharing = []
        for digest, calls in groups.items():
            sharing.append(dict(global_sha256=digest, encoded_bytes=int(calls[0]["global_bytes"]),
                                invocations=len(calls),
                                distinct_image_objects=len({c["image"] for c in calls}),
                                distinct_encoded_images=len({c["source_sha"] for c in calls}),
                                after_first_invocations=max(0, len(calls) - 1)))
        jbig_ids = set(inv["jbig2_objects"]) | {int(c["image"]) for c in invocations}
        hit_events = [c for c in cache if int(c["image"]) in jbig_ids
                      or int(c["attached_mask"]) in jbig_ids]
        rows.append(dict(id=ident, input_sha256=item["sha256"], result=result,
                         inventory=inv, invocations=invocations,
                         jbig2_image_cache_events=hit_events, globals=sharing,
                         owner_events=owner_events, decoder_returns=returns))
        (output / "documents.json").write_text(json.dumps(rows, indent=2) + "\n")
        print(ident, "invocations", len(invocations), "image-cache", len(hit_events),
              "nonempty-global groups", len(sharing), flush=True)
    summary = dict(documents=len(rows), original_documents=len(manifest["pdfs"]),
                   invocations=sum(len(v["invocations"]) for v in rows),
                   image_mask_invocations=sum(int(c["mask"]) for v in rows for c in v["invocations"]),
                   attached_mask_invocations=sum(bool(v["inventory"]["jbig2_objects"]
                                                       .get(int(c["image"]), {}).get("attached_as_mask"))
                                                 for v in rows for c in v["invocations"]),
                   jbig2_image_cache_events=sum(len(v["jbig2_image_cache_events"]) for v in rows),
                   global_invocations=sum(g["invocations"] for v in rows for g in v["globals"]),
                   successful_resource_reuses=sum(int(event.get("resource_hit", 0))
                                                  for v in rows for event in v["owner_events"]),
                   parsed_owner_reuses=sum(int(event.get("parsed_hit", 0))
                                           for v in rows for event in v["owner_events"]),
                   admission_states=dict(collections.Counter(event["after"] for v in rows
                                         for event in v["owner_events"] if event["kind"] == "JBIG2_OWNER")),
                   bypass_reasons=dict(collections.Counter(event["reason"] for v in rows
                                       for event in v["owner_events"] if event["kind"] == "JBIG2_OWNER_BYPASS")),
                   repeated_global_groups=[dict(document=v["id"], **g) for v in rows for g in v["globals"]
                                           if g["invocations"] > 1],
                   note="Actual rendering callbacks, including masks. Image-cache returns are not decoder calls or prepared-resource hits.")
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
