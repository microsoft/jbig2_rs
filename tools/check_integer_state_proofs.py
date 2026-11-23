#!/usr/bin/env python3
"""Check complete signed-integer and payload-fold result/state relations."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases
from source_group import runtime_function


ROOT = Path(__file__).resolve().parents[1]
BASELINE_GROUPS = (("arithmetic", (
    "ArithmeticDecoder::decode_integer", "ArithmeticDecoder::decode_integer_bits",
)),)
RESULT = "out == arithmetic_spec::integer_state(*old(self), old(contexts)@).0,"
DECODER = "*final(self) == arithmetic_spec::integer_state(*old(self), old(contexts)@).1,"
CONTEXTS = "final(contexts)@ == arithmetic_spec::integer_state(*old(self), old(contexts)@).2,"
PAYLOAD_POSTS = tuple(
    f"""{target} == arithmetic_spec::integer_bits_run(
        *old(self), old(contexts)@, *old(previous), bit_count, 0,
    ).{index},"""
    for index, target in enumerate(("out", "*final(self)", "final(contexts)@", "*final(previous)"))
)
MAGNITUDE = """let magnitude = offset
    .checked_add(u64::from(magnitude_bits))
    .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;"""
SIGNED_RESULT = """let magnitude = magnitude as i64;
if sign == 0 {
    Ok(ArithmeticInteger::Value(magnitude))
} else {
    let negative = -magnitude;
    Ok(ArithmeticInteger::Value(negative))
}"""
PAYLOAD_RETURN = runtime_function(
    ROOT, "arithmetic", "ArithmeticDecoder::decode_integer_bits",
)["body_text"]
SIGN = "let sign = self.decode_integer_bit(&mut previous, contexts)?;"


def case(identity, function, site, replacement, control):
    return {
        "id": identity, "file": "src/arithmetic/integers.rs", "module": "arithmetic",
        "function": f"ArithmeticDecoder::{function}",
        "site": site, "replacement": replacement, "controls": ((control, ""),),
    }


CASES = (
    case(
        "integer_positive_magnitude_discarded", "decode_integer", SIGNED_RESULT,
        SIGNED_RESULT.replace("Value(magnitude)", "Value(0)"), RESULT,
    ),
    case(
        "integer_negative_sign_discarded", "decode_integer", SIGNED_RESULT,
        SIGNED_RESULT.replace("Value(negative)", "Value(magnitude)"), RESULT,
    ),
    case(
        "integer_out_of_band_becomes_zero", "decode_integer",
        MAGNITUDE + "\nif sign == 1 && magnitude == 0 {\nreturn Ok(ArithmeticInteger::OutOfBand);\n}",
        MAGNITUDE + "\nif sign == 1 && magnitude == 0 {\nreturn Ok(ArithmeticInteger::Value(0));\n}",
        RESULT,
    ),
    case(
        "integer_final_code_discarded", "decode_integer", MAGNITUDE,
        MAGNITUDE + "\nself.code = 0;", DECODER,
    ),
    case(
        "integer_unvisited_context_changed", "decode_integer", MAGNITUDE,
        MAGNITUDE + "\ncontexts[0] = ArithmeticContext::new();", CONTEXTS,
    ),
    case(
        "integer_partial_error_rewinds_input", "decode_integer", SIGN,
        """let sign = match self.decode_integer_bit(&mut previous, contexts) {
            Ok(sign) => sign,
            Err(error) => {
                self.byte_position = 0;
                self.current_byte = self.bytes[0];
                self.marker = None;
                return Err(error);
            }
        };""",
        DECODER,
    ),
    case(
        "integer_payload_value_discarded", "decode_integer_bits", PAYLOAD_RETURN,
        PAYLOAD_RETURN.replace("Ok(value)", "Ok(0)"), PAYLOAD_POSTS[0],
    ),
    case(
        "integer_payload_history_discarded", "decode_integer_bits", PAYLOAD_RETURN,
        PAYLOAD_RETURN.replace("Ok(value)", "*previous = 0;\nOk(value)"), PAYLOAD_POSTS[3],
    ),
    case(
        "integer_payload_context_changed", "decode_integer_bits", PAYLOAD_RETURN,
        PAYLOAD_RETURN.replace("Ok(value)", "contexts[0] = ArithmeticContext::new();\nOk(value)"),
        PAYLOAD_POSTS[2],
    ),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES,
            ("src/arithmetic/integers.rs", "specs/arithmetic.rs", "proofs/model_support/arithmetic.rs"),
            "jbig2-integer-state-",
            "Complete project signed-integer and payload result, history, decoder and bank composition; independent normative procedure/table/caller interpretation remains separate",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"integer state proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
