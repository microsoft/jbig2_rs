"""Record emitted first-party loop code without instrumenting latency binaries."""

import json
import re
import subprocess

from fixtures import PRIVATE, RESULTS, save, sha


TARGETS = (
    "halftone::planes::accumulate_gray_byte",
    "halftone::planes::accumulate_narrow_plane",
    "halftone::planes::accumulate_wide_plane",
    "halftone::planes::decode_gray_planes",
)


def command(args):
    return subprocess.check_output(args, text=True)


def main():
    records = []
    for label in ("baseline", "candidate"):
        binary = PRIVATE / "binaries" / label
        build = json.loads(binary.with_suffix(".json").read_text())
        if build["instrumented_allocations"] or sha(binary) != build["binary_sha256"]:
            raise ValueError("Need the exact uninstrumented latency binary")
        sections = {}
        for line in command(["size", "-A", "-d", str(binary)]).splitlines():
            fields = line.split()
            if len(fields) == 3 and fields[0].startswith(".") and fields[1].isdigit():
                sections[fields[0]] = int(fields[1])
        if ".text" not in sections:
            raise ValueError("Missing native text-section size")
        functions = []
        text = []
        for line in command(["nm", "-SC", str(binary)]).splitlines():
            match = re.fullmatch(r"([0-9a-f]+) ([0-9a-f]+) [tT] (.+)", line)
            if match is None or not any(match[3].endswith(name) for name in TARGETS):
                continue
            start, length = int(match[1], 16), int(match[2], 16)
            functions.append({"symbol": match[3], "bytes": length})
            text.append(command([
                "objdump", "-dC", f"--start-address={start}",
                f"--stop-address={start + length}", str(binary),
            ]))
        disassembly = RESULTS / (label + "-halftone-disassembly.txt")
        disassembly.write_text("\n".join(text))
        if sha(binary) != build["binary_sha256"]:
            raise ValueError("Binary changed during inspection")
        records.append({
            "label": label,
            "source_revision": build["source_revision"],
            "binary_sha256": build["binary_sha256"],
            "sections": sections,
            "functions": functions,
            "targets_without_separate_symbol": [
                target for target in TARGETS
                if not any(row["symbol"].endswith(target) for row in functions)
            ],
            "disassembly": disassembly.name,
            "disassembly_sha256": sha(disassembly),
        })
    save(RESULTS / "native-code-inspection.json", {
        "scope": "ELF sizes and selected emitted first-party loops, not executed instruction counts, hardware counters or a latency waiver. Missing separate symbols are explicit and may reflect inlining; no vector-arithmetic claim follows from a symbol name.",
        "objdump": command(["objdump", "--version"]).splitlines()[0],
        "binaries": records,
    })


if __name__ == "__main__":
    main()
