import argparse
import collections
import csv
import hashlib
import json
import os
from pathlib import Path
import resource
import shlex
import statistics
import subprocess

ROOT = Path(os.environ["JBIG2_EVAL_RESULTS"]).resolve()
WORK = Path(os.environ["JBIG2_PDFIUM_WORK"]).resolve()
BUILDS = dict(cpp="Cpp", rust_strict="RustStrict", rust_best="RustBest")


def load(name):
    return json.loads((ROOT / name).read_text())


def save(name, value):
    (ROOT / name).write_text(json.dumps(value, indent=2) + "\n")


def limits():
    resource.setrlimit(resource.RLIMIT_DATA, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run(item, backend, repetitions=0, label="functional"):
    directory = ROOT / "pixels" / label / backend
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / (hashlib.sha256(item["id"].encode()).hexdigest()[:20] + ".bin")
    command = [str(WORK / "out" / BUILDS[backend] / "jbig2_eval"),
               "pdf" if item["group"] == "pdf" else "image", item["path"],
               item.get("globals", {}).get("path", "-"), str(target),
               str(item.get("width", 0)), str(item.get("height", 0)), str(repetitions)]
    environment = os.environ.copy()
    if repetitions:
        environment.pop("JBIG2_EVAL_RECOVERIES", None)
    else:
        environment["JBIG2_EVAL_RECOVERIES"] = "1"
    try:
        process = subprocess.run(command, capture_output=True, text=True, preexec_fn=limits,
                                 timeout=120 if repetitions else 20, env=environment)
    except subprocess.TimeoutExpired as error:
        print(f"TIMEOUT {backend} {item['id']}", flush=True)
        return dict(status="timeout", stderr=str(error), path="")
    if process.returncode:
        print(f"PROCESS FAILURE {backend} {item['id']}: {process.returncode}", flush=True)
        return dict(status=f"exit_{process.returncode}", stderr=process.stderr,
                    stdout=process.stdout, path="")
    values = process.stdout.strip().split("\t")
    if len(values) != 7:
        raise ValueError(f"Unexpected harness response {backend} {item['id']}: {process.stdout!r}")
    result = dict(zip(("status", "width", "height", "channels", "p50_us", "p90_us", "runs"), values))
    result.update(stderr=process.stderr, path=str(target))
    if result["status"] == "ok":
        data = target.read_bytes()
        result.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
    return result


def comparison(a, b, bitmap):
    if a["status"] != "ok" or b["status"] != "ok":
        return dict(comparison="not_both_accepted")
    if (a["width"], a["height"], a["channels"], a["bytes"]) != (b["width"], b["height"], b["channels"], b["bytes"]):
        return dict(comparison="layout_differs")
    if a["sha256"] == b["sha256"]:
        return dict(comparison="exact", different_pixels=0, different_samples=0, max_delta=0)
    x, y = Path(a["path"]).read_bytes(), Path(b["path"]).read_bytes()
    if bitmap:
        return dict(comparison="pixels_differ", different_pixels=sum((p ^ q).bit_count() for p, q in zip(x, y)))
    delta = [abs(p - q) for p, q in zip(x, y)]
    return dict(comparison="pixels_differ", different_samples=sum(d != 0 for d in delta),
                max_delta=max(delta), mae=sum(delta) / len(delta))


def audit():
    result = {}
    for backend, build in BUILDS.items():
        output = WORK / "out" / build
        raw = subprocess.check_output([str(WORK / "third_party/ninja/ninja"), "-C", str(output),
                                       "-t", "commands", "jbig2_eval"])
        (ROOT / f"commands-{backend}.txt").write_bytes(raw)
        counts = collections.Counter()
        for line in raw.decode("latin1").splitlines():
            args = shlex.split(line)
            if "-c" not in args or not any("/clang" in i for i in args[:2]):
                continue
            opts = [i for i in args if i in ("-O0", "-O1", "-O2", "-O3", "-Os", "-Oz", "-Ofast")]
            assert opts[-1] == "-O3" and "-mcpu=native" in args and "-flto=thin" in args
            assert "-DNDEBUG" in args and "-D_DEBUG" not in args and "-ffast-math" not in args
            counts[opts[-1]] += 1
        link = next(line for line in raw.decode("latin1").splitlines() if "--lto-O" in line and "jbig2_eval" in line)
        assert "-Wl,--lto-O3" in shlex.split(link)
        value = dict(c_cpp_commands=dict(counts), final_lto_opt=3,
                     binary_sha256=hashlib.sha256((output / "jbig2_eval").read_bytes()).hexdigest())
        if backend != "cpp":
            for name in ("obj/third_party/jbig2_decoder/lib.rsp.rust", "obj/core/fxcodec/rust_jbig2_ffi.rsp.rust"):
                flags = (output / name).read_text().splitlines()
                for wanted in ("-Copt-level=3", "-Ctarget-cpu=native", "-Ccodegen-units=1",
                               "-Cdebug-assertions=no", "-Coverflow-checks=no", "-Clinker-plugin-lto=yes"):
                    assert wanted in flags, (backend, name, wanted)
                assert ("--cfg=pdf_eval_jbig2_best_effort" in flags) == (backend == "rust_best" and "ffi" in name)
            value["core_archive_sha256"] = hashlib.sha256(
                (output / "obj/third_party/jbig2_decoder/libjbig2_decoder.rlib").read_bytes()).hexdigest()
        result[backend] = value
    assert result["rust_strict"]["core_archive_sha256"] == result["rust_best"]["core_archive_sha256"]
    result["jbig2_source"] = subprocess.check_output(
        ["git", "-C", os.environ["JBIG2_RS_SOURCE"], "rev-parse", "HEAD"], text=True).strip()
    save("build-audit.json", result)
    print(json.dumps(result, indent=2))


def functional():
    manifest = load("manifest.json")
    result = []
    for item in manifest["images"] + manifest["pdfs"]:
        assert hashlib.sha256(Path(item["path"]).read_bytes()).hexdigest() == item["sha256"]
        row = {k: item[k] for k in ("id", "group")}
        if "pdf" in item:
            row["source_pdf"] = item["pdf"]
        for backend in BUILDS:
            row[backend] = run(item, backend)
        for backend in ("rust_strict", "rust_best"):
            row["cpp_vs_" + backend] = comparison(row["cpp"], row[backend], item["group"] != "pdf")
        row["strict_vs_best"] = comparison(row["rust_strict"], row["rust_best"], item["group"] != "pdf")
        result.append(row)
        if len(result) % 25 == 0:
            save("functionality.json", result)
            print("Compared", len(result), "requests", flush=True)
    save("functionality.json", result)
    print("Functional comparison complete:", len(result), "requests", flush=True)


def performance():
    manifest = load("manifest.json")
    rows = {r["id"]: r for r in load("functionality.json")}
    candidates = []
    for item in manifest["images"]:
        row = rows[item["id"]]
        if row["cpp_vs_rust_best"]["comparison"] == "exact":
            candidates.append(item)
    selected, selected_ids, hashes = [], set(), set()

    def add(item):
        key = (item["sha256"], item.get("globals", {}).get("sha256"), item.get("width"), item.get("height"))
        if item["id"] not in selected_ids and key not in hashes:
            selected.append(item)
            selected_ids.add(item["id"])
            hashes.add(key)

    for family in ("generic", "refine", "symbol", "huffman", "halftone", "text", "composite", "page"):
        matching = [i for i in candidates if i.get("family") == family]
        for item in sorted(matching, key=lambda i: i["width"] * i["height"], reverse=True)[:3]:
            add(item)
    for stem in ("generic_template1", "generic_template2", "generic_template3",
                 "generic_template0_tpgdon", "halftone_10_bitplanes_mmr"):
        for item in candidates:
            if item["id"] == "hayro/" + stem + "-page1":
                add(item)
    embedded = [i for i in candidates if i["group"] == "embedded"]
    for item in sorted(embedded, key=lambda i: i["width"] * i["height"], reverse=True)[:12]:
        add(item)
    for item in [i for i in embedded if i["globals"]["size"]][:3]:
        add(item)
    healthy = []
    exclusions = []
    for item in manifest["pdfs"]:
        payloads = [i for i in manifest["images"] if i.get("pdf") == item["id"]]
        if "image_id" in item:
            payloads = [next(i for i in manifest["images"] if i["id"] == item["image_id"])]
        if not payloads or rows[item["id"]]["cpp_vs_rust_best"]["comparison"] != "exact" or any(
                rows[i["id"]]["cpp_vs_rust_best"]["comparison"] != "exact" for i in payloads):
            exclusions.append(item["id"])
        else:
            healthy.append(item)
    for item in sorted(healthy, key=lambda i: float(rows[i["id"]]["cpp"]["p50_us"]) if float(rows[i["id"]]["cpp"]["p50_us"]) else i["size"], reverse=True)[:24]:
        add(item)
    shared_global_pdfs = [i for i in healthy if any(
        p.get("pdf") == i["id"] and p["globals"]["size"] for p in manifest["images"])]
    for item in shared_global_pdfs[:3]:
        add(item)
    for item in [i for i in healthy if i["extracted_images"] > 1][:2]:
        add(item)
    save("benchmark-selection.json", dict(selected=[i["id"] for i in selected], excluded_pdfs=exclusions,
         primary="cpp versus rust_best; strict measured additionally only where its pixels match",
         image_runs=31, pdf_runs=11, warmups=3, rounds=3, cpu=2))
    timed = []
    names = list(BUILDS)
    for round_number in range(3):
        for index, item in enumerate(selected):
            row = rows[item["id"]]
            eligible = ["cpp", "rust_best"]
            if row["cpp_vs_rust_strict"]["comparison"] == "exact":
                eligible.insert(1, "rust_strict")
            offset = (round_number + index) % len(eligible)
            order = eligible[offset:] + eligible[:offset]
            value = dict(id=item["id"], group=item["group"], family=item.get("family", item["group"]),
                         round=round_number + 1, order=order)
            for backend in order:
                value[backend] = run(item, backend, 11 if item["group"] == "pdf" else 31,
                                     label=f"benchmark-{round_number + 1}")
                assert value[backend]["status"] == "ok", value
                assert value[backend]["sha256"] == row[backend]["sha256"], item["id"]
            value["best_over_cpp"] = float(value["rust_best"]["p50_us"]) / float(value["cpp"]["p50_us"])
            if "rust_strict" in value:
                value["strict_over_cpp"] = float(value["rust_strict"]["p50_us"]) / float(value["cpp"]["p50_us"])
            timed.append(value)
            save("performance.json", timed)
            print(round_number + 1, item["id"], f"best/cpp {value['best_over_cpp']:.3f}x", flush=True)
    print("Matched timing complete",len(timed), "case-rounds", flush=True)


def summarize():
    manifest = load("manifest.json")
    items = {i["id"]: i for i in manifest["images"] + manifest["pdfs"]}
    rows = load("functionality.json")
    summary = {}
    for group in ("embedded", "hayro", "pdf"):
        values = [i for i in rows if i["group"] == group]
        summary[group] = dict(cases=len(values), statuses={b: dict(collections.Counter(i[b]["status"] for i in values)) for b in BUILDS},
            comparisons={b:dict(collections.Counter(i["cpp_vs_" + b]["comparison"] for i in values)) for b in ("rust_strict","rust_best")})
    summary["differences"] = [dict(id=i["id"], group=i["group"], comparison=i["cpp_vs_rust_best"],
            cpp=i["cpp"]["status"], strict=i["rust_strict"]["status"], best=i["rust_best"]["status"],
            best_error=i["rust_best"]["stderr"]) for i in rows if i["cpp_vs_rust_best"]["comparison"] != "exact"]
    fields = ["id","group","cpp","rust_strict","rust_best","cpp_vs_strict","cpp_vs_best","different_pixels","different_samples","max_delta"]
    with (ROOT / "functionality.tsv").open("w") as file:
        writer=csv.DictWriter(file,fields,delimiter="\t");writer.writeheader()
        for i in rows:
            writer.writerow(dict(id=i["id"],group=i["group"],**{b:i[b]["status"] for b in BUILDS},
                cpp_vs_strict=i["cpp_vs_rust_strict"]["comparison"],cpp_vs_best=i["cpp_vs_rust_best"]["comparison"],
                **{f:i["cpp_vs_rust_best"].get(f,"") for f in ("different_pixels","different_samples","max_delta")}))
    if (ROOT / "performance.json").exists():
        timed = load("performance.json")
        by_id=collections.defaultdict(list)
        for i in timed:by_id[i["id"]].append(i)
        measurements=[]
        for ident,values in by_id.items():
            assert sorted(i["round"] for i in values)==[1,2,3]
            row=dict(id=ident,group=values[0]["group"],family=values[0]["family"],
                     best_over_cpp=statistics.median(i["best_over_cpp"] for i in values))
            for b in BUILDS:
                if all(b in i for i in values):row[b+"_ms"]=statistics.median(float(i[b]["p50_us"])/1000 for i in values)
            if all("strict_over_cpp" in i for i in values):row["strict_over_cpp"]=statistics.median(i["strict_over_cpp"] for i in values)
            measurements.append(row)
        fields=["id","group","family","cpp_ms","rust_strict_ms","rust_best_ms","best_over_cpp","strict_over_cpp"]
        with (ROOT / "performance.tsv").open("w") as file:
            writer=csv.DictWriter(file,fields,delimiter="\t");writer.writeheader();writer.writerows(measurements)
        summary["performance"]={}
        for group in sorted({i["family"] if i["group"]=="hayro" else i["group"] for i in measurements}):
            values=[i for i in measurements if (i["family"] if i["group"]=="hayro" else i["group"])==group]
            summary["performance"][group]=dict(cases=len(values),best_over_cpp=statistics.median(i["best_over_cpp"] for i in values),
                minimum=min(i["best_over_cpp"] for i in values),maximum=max(i["best_over_cpp"] for i in values))
    save("summary.json",summary)
    print(json.dumps({k:v for k,v in summary.items() if k!="differences"},indent=2))


if __name__ == "__main__":
    os.sched_setaffinity(0,{2})
    parser=argparse.ArgumentParser()
    parser.add_argument("phase",choices=("audit","functional","performance","summarize"))
    args=parser.parse_args()
    dict(audit=audit,functional=functional,performance=performance,summarize=summarize)[args.phase]()
