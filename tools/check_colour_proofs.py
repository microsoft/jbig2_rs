#!/usr/bin/env python3
"""Check exact RGB channels, foreground IDs and explicit declaration recovery."""

import argparse
import json
from pathlib import Path
import sys

from proof_sensitivity import check_semantic_cases, replace_once
from source_group import runtime_function

ROOT = Path(__file__).resolve().parents[1]


def case(identity, module, function, site, replacement, controls, test):
    row = runtime_function(ROOT, module, function)
    body = row["body_text"]
    return {
        "id": identity, "file": row["file"], "module": module, "function": function,
        "site": body, "replacement": replace_once(body, site, replacement), "controls": controls,
        "native_test": test,
    }


SET_CHANNELS = """out.is_ok() ==> x < final(self).width && y < final(self).height
    && forall|channel:int| 0 <= channel < 3 ==>
        final(self).bytes@[y as int * final(self).stride as int + x as int * 3 + channel] == value@[channel],"""
PALETTE_PREFIX = """forall|done:int, channel:int| 0 <= done < index && 0 <= channel < 3 ==>
    colours@[done]@[channel] == payload@[7 + done * 3 + channel],"""
FOREGROUND = """&& (foreground.is_some() ==> payload@.len() >= 4
    && foreground.unwrap() == crate::spec::segments::be_u32_at(
        payload@, payload@.len() - 4,
    ))"""
CASES = (
    case("rgb_red_channel_replaced_by_blue", "colour", "RgbBitmap::set",
         "self.bytes[offset] = value[0];", "self.bytes[offset] = value[2];",
         ((SET_CHANNELS, ""),), "rgb_palette_references_and_truncation_remain_semantic_errors"),
    case("rgb_stride_has_four_channels", "colour", "RgbBitmap::filled",
         ".checked_mul(3)", ".checked_mul(4)",
         (("colour_spec::storage_valid(&bitmap) &&", ""),),
         "rgb_keeps_real_foreground_channels_and_repairs_only_missing_declarations"),
    case("palette_green_and_blue_swapped", "colour", "parse",
         "colours.push([red, green, blue]);", "colours.push([red, blue, green]);",
         (("super::colour_spec::palette_contents(payload@, palette.colours@) &&", ""),
          (PALETTE_PREFIX, "")),
         "rgb_palette_references_and_truncation_remain_semantic_errors"),
    case("strict_colour_declaration_silently_repaired", "colour", "ensure_declaration",
         "if !compatible {", "if compatible && false {",
         (("!*old(declared) && !compatible ==> out.is_err(),", ""),),
         "rgb_keeps_real_foreground_channels_and_repairs_only_missing_declarations"),
    case("generic_foreground_id_changed", "generic_region", "decode_generic_with_colour",
         "Some(ByteReader::new(&payload[coded_end..]).read_u32()?)",
         "Some(ByteReader::new(&payload[coded_end..]).read_u32()? ^ 1)",
         ((FOREGROUND, ""),),
         "rgb_keeps_real_foreground_channels_and_repairs_only_missing_declarations"),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, args.verus_executable.resolve(), CASES,
            ("src/colour.rs", "src/colour/bitmap.rs", "src/colour/palette.rs",
             "src/colour/document.rs", "specs/colour.rs", "src/generic_region/colour.rs"),
            "jbig2-colour-", "Exact RGB storage/channel and encoded palette/foreground/declaration "
            "contracts, retaining storage, framing and resource controls. "
            "Not a full decoded-output or independently approved normative proof.",
        )
    except (OSError, ValueError, KeyError) as error:
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
