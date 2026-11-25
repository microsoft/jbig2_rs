import collections
import hashlib
import json
import math
import os
from pathlib import Path
import random
import statistics

from measure_documents import COMPARISONS, VARIANTS, quantile

ROOT = Path(__file__).resolve().parent


def load(path):
    return json.loads(path.read_text())


def records(name):
    with (ROOT / name).open() as stream:
        return [json.loads(line) for line in stream]


def interval(values, seed):
    generator = random.Random(seed)
    samples = sorted(statistics.median(generator.choices(values, k=len(values)))
                     for _ in range(10000))
    return [quantile(samples, 0.025), quantile(samples, 0.975)]


def timings(values):
    samples = [v for row in values for v in row["result"]["samples_us"]]
    return dict(processes=len(values), document_samples=len(samples),
                median_process_p50_us=statistics.median(v["result"]["p50_us"] for v in values),
                median_process_p95_us=statistics.median(v["result"]["p95_us"] for v in values),
                pooled_median_us=statistics.median(samples),
                pooled_p95_us=quantile(samples, 0.95))


def memory(values):
    names = ("before_rust_bytes", "after_document_rust_bytes",
             "peak_rust_requested_bytes", "rust_allocation_calls",
             "rust_requested_total", "maxrss_kib")
    return {name: dict(median=statistics.median(v["result"][name] for v in values),
                       minimum=min(v["result"][name] for v in values),
                       maximum=max(v["result"][name] for v in values))
            for name in names}


def trace(value):
    invocations = value["invocations"]
    returned = value["decoder_returns"]
    owner = value["owner_events"]
    states = collections.Counter(v["after"] for v in owner if v["kind"] == "JBIG2_OWNER")
    retained = [int(v["metadata"]) + int(v["resident"])
                for v in owner if v["kind"] == "JBIG2_OWNER"]
    retained.extend(int(v["retained"]) for v in owner
                    if v["kind"] == "JBIG2_OWNER_BYPASS" and "retained" in v)
    for row in owner:
        if row["kind"] == "JBIG2_OWNER":
            assert int(row["metadata"]) + int(row["resident"]) <= int(row["budget"])
            assert int(row["reserved"]) <= int(row["budget"])
        elif row["kind"] == "JBIG2_OWNER_DROP":
            assert int(row["retained"]) == 0
    return dict(
        decoder_invocations=len(invocations),
        stencil_mask_invocations=sum(int(v["mask"]) for v in invocations),
        attached_mask_invocations=sum(bool(value["inventory"]["jbig2_objects"]
                                             .get(v["image"], {}).get("attached_as_mask"))
                                      for v in invocations),
        image_cache_events=len(value["jbig2_image_cache_events"]),
        nonempty_global_invocations=sum(v["invocations"] for v in value["globals"]),
        global_groups=value["globals"],
        resource_reuses=sum(int(v.get("resource_hit", 0)) for v in owner),
        parsed_owner_reuses=sum(int(v.get("parsed_hit", 0)) for v in owner),
        admission_states=dict(states),
        bypass_reasons=dict(collections.Counter(v["reason"] for v in owner
                           if v["kind"] == "JBIG2_OWNER_BYPASS")),
        max_retained_owner_bytes=max(retained, default=0),
        complete_jbig2=bool(invocations) and len(returned) == len(invocations)
                       and all(v["status"] == "4" for v in returned),
        decoder_statuses=dict(collections.Counter(v["status"] for v in returned)),
        inventory_error=value["inventory"].get("inventory_error"),
    )


def investigate(comparison, metric, value, context, memory_delta, calibration):
    statements = []
    if not context["complete_jbig2"]:
        statements.append("No successful complete-JBIG2-document speedup is credited: "
                          "this is a no-JBIG2 or unchanged-refusal/omission control.")
    if comparison == "calibration":
        statements.append("Both roles execute identical operations in the identical binary. "
                          "This is observed calibration variability, not a decoder source regression.")
    elif comparison == "best_policy_control":
        statements.append("Actual tracing confirms strict prepared decoding is bypassed for best effort. "
                          "There is no prepared-resource execution to attribute this observation to.")
    elif not context["nonempty_global_invocations"]:
        statements.append("All actual requests bypass the owner for empty globals. "
                          "The resource comparison uses the same binary/mode and never reads its allowance here.")
    elif comparison == "resources":
        statements.append(
            f"Actual successful resource reuses: {context['resource_reuses']}; "
            f"admission states: {context['admission_states']}. "
            "Cold preparation/copies/destruction are included, not amortized by invented calls."
        )
    else:
        statements.append(
            f"Observed prepared-mode parsed-owner reuses: {context['parsed_owner_reuses']}. "
            "The metadata comparison holds context/scratch lifetime and packing equal; "
            "it includes key/owner construction and retained parsed metadata."
        )
    statements.append(
        "Separate three-child memory observations: candidate-base "
        f"Rust allocation calls {memory_delta['rust_allocation_calls']:+g}, "
        f"requested total {memory_delta['rust_requested_total']:+g} bytes, "
        f"requested live peak {memory_delta['peak_rust_requested_bytes']:+g} bytes. "
        "These observations do not exclusively localize the latency cause."
    )
    statements.append(
        f"Same-document identical-operation {metric} calibration ratio "
        f"{calibration['ratio']:.6f}, descriptive95 {calibration['bootstrap95']}. "
        "Neither calibration nor unchanged source excuses deleting a negative row."
    )
    if value["bootstrap95"][0] > 1:
        statements.append("The paired interval excludes parity in this run; keep as a supported-context adverse observation.")
    else:
        statements.append("The paired interval includes parity; retain as an adverse point estimate, not established causal slowdown.")
    statements.append("No post-selection replacement trials, input drops, alignment tuning or automatic owner waiver.")
    return " ".join(statements)


def main():
    settings = load(ROOT / "protocol.json")
    raw = records("timings.jsonl")
    mem = records("memory.jsonl")
    jobs = [(name, role, variant) for name, pair in COMPARISONS.items()
            for role, variant in zip(("base", "candidate"), pair)]
    jobs.extend(("reference", v, v) for v in settings["reference_variants"])
    expected = {(scenario, ident, round_, comparison, role, variant)
                for scenario in ("cold", "repeat")
                for ident in settings[scenario + "_documents"]
                for round_ in range(1, settings["rounds"] + 1)
                for comparison, role, variant in jobs}
    keyed = {(v["scenario"], v["id"], v["round"], v["comparison"], v["role"], v["variant"]): v
             for v in raw}
    assert len(keyed) == len(raw) == len(expected) and set(keyed) == expected
    expected_memory = {(scenario, ident, round_, variant)
                       for scenario in ("cold", "repeat")
                       for ident in settings[scenario + "_documents"]
                       for round_ in range(1, settings["memory_rounds"] + 1)
                       for variant in VARIANTS}
    memory_keys = {(v["scenario"], v["id"], v["round"], v["variant"]) for v in mem}
    assert len(memory_keys) == len(mem) == len(expected_memory) and memory_keys == expected_memory
    for v in raw:
        result = v["result"]
        samples = result["samples_us"]
        assert len(samples) == settings[v["scenario"] + "_samples"]
        assert result["p50_us"] == statistics.median(samples)
        assert result["p95_us"] == quantile(samples, 0.95)
    assert all(v["result"]["after_document_rust_bytes"] == 0 for v in mem)
    mem_groups = collections.defaultdict(list)
    for row in mem:
        mem_groups[(row["scenario"], row["id"], row["variant"])].append(row)
    traces = {}
    for scenario in ("cold", "repeat"):
        for name in ("prepared", "best", "ordinary", "cpp"):
            directory = "traces-cpp" if scenario == "cold" and name == "cpp" else "traces-" + name + "-" + scenario
            traces[(scenario, name)] = {v["id"]: v for v in load(ROOT / directory / "documents.json")}
    output = []
    adverse = []
    for scenario in ("cold", "repeat"):
        for ident in settings[scenario + "_documents"]:
            value = dict(id=ident, scenario=scenario, comparisons={}, references={},
                         trace={name: trace(traces[(scenario, name)][ident])
                                for name in ("prepared", "best", "ordinary", "cpp")},
                         memory={variant: memory(mem_groups[(scenario, ident, variant)])
                                 for variant in VARIANTS})
            for comparison, (baseline, candidate) in COMPARISONS.items():
                base = [keyed[(scenario, ident, r, comparison, "base", baseline)]
                        for r in range(1, settings["rounds"] + 1)]
                cand = [keyed[(scenario, ident, r, comparison, "candidate", candidate)]
                        for r in range(1, settings["rounds"] + 1)]
                row = dict(base_variant=baseline, candidate_variant=candidate,
                           base=timings(base), candidate=timings(cand))
                for metric in ("p50", "p95"):
                    ratios = [c["result"][metric + "_us"] / b["result"][metric + "_us"]
                              for b, c in zip(base, cand)]
                    seed = int(hashlib.sha256((scenario + ident + comparison + metric).encode()).hexdigest()[:16], 16)
                    row[metric] = dict(ratio=statistics.median(ratios),
                                       bootstrap95=interval(ratios, seed),
                                       per_round_ratios=ratios,
                                       faster_rounds=sum(v < 1 for v in ratios))
                value["comparisons"][comparison] = row
            value["calibration_interval_within_5_percent"] = {
                metric: value["comparisons"]["calibration"][metric]["bootstrap95"][0] >= 0.95
                and value["comparisons"]["calibration"][metric]["bootstrap95"][1] <= 1.05
                for metric in ("p50", "p95")
            }
            for variant in settings["reference_variants"]:
                values = [keyed[(scenario, ident, r, "reference", variant, variant)]
                          for r in range(1, settings["rounds"] + 1)]
                value["references"][variant] = timings(values)
            for comparison, row in value["comparisons"].items():
                delta = {name: value["memory"][row["candidate_variant"]][name]["median"] -
                         value["memory"][row["base_variant"]][name]["median"]
                         for name in ("rust_allocation_calls", "rust_requested_total",
                                      "peak_rust_requested_bytes", "maxrss_kib")}
                for metric in ("p50", "p95"):
                    if row[metric]["ratio"] > 1.05:
                        adverse.append(dict(id=ident, scenario=scenario, comparison=comparison,
                                            metric=metric, **row[metric],
                                            memory_delta=delta,
                                            investigation=investigate(
                                                comparison, metric, row[metric],
                                                value["trace"]["best" if comparison == "best_policy_control" else "prepared"],
                                                delta, value["comparisons"]["calibration"][metric])))
            output.append(value)
    summary = dict(
        result="MEASURED_NOT_ADOPTED", timed_processes=len(raw),
        timed_fresh_document_samples=sum(len(v["result"]["samples_us"]) for v in raw),
        separate_memory_processes=len(mem),
        cold_documents=len(settings["cold_documents"]),
        repeat_documents=len(settings["repeat_documents"]),
        every_post_document_rust_requested_byte_balance_zero=True,
        calibration_intervals_outside_5_percent=[
            dict(id=v["id"], scenario=v["scenario"], metric=metric,
                 **v["comparisons"]["calibration"][metric])
            for v in output for metric in ("p50", "p95")
            if not v["calibration_interval_within_5_percent"][metric]
        ],
        per_document=output, adverse_over_5_percent=adverse,
        note="All original documents retained. No rejected/omitted image is a speedup. "
             "Every paired median or p95 ratio over1.05, including controls, is retained and investigated. "
             "Intervals resample nine fresh-process pairs; shared-host, finite-sample and secondary p95 limits remain.")
    (ROOT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Complete:", len(raw), "timed processes;", len(mem), "memory processes;",
          len(adverse), "adverse median/p95 comparison rows retained.")


if __name__ == "__main__":
    os.sched_setaffinity(0, {31})
    main()
