#!/usr/bin/env python3
"""Check exact decision/context outputs, not complete entropy-stream equivalence."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases


ROOT = Path(__file__).resolve().parents[1]
BASELINE_GROUPS = (("arithmetic", (
    "ArithmeticDecoder::decode", "ArithmeticDecoder::decode_lower_signal",
    "ArithmeticDecoder::decode_signal", "ArithmeticDecoder::decode_upper_signal",
    "ArithmeticDecoder::signal_error",
)),)
CONTEXT_POST = """arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
    old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)),
),
arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
    old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)), arithmetic_spec::context_mps(*old(context)),
),"""
SIGNAL_DECISION = """out <= 1 ==> out == arithmetic_spec::entry_decision(
    old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)), arithmetic_spec::context_mps(*old(context)),
),"""
SIGNAL_POST = SIGNAL_DECISION + "\n" + CONTEXT_POST
CHECKED_POST = """&& decision == arithmetic_spec::entry_decision(
    old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)), arithmetic_spec::context_mps(*old(context)),
)"""
CHECKED_STATE_DECISION = "&& decision == arithmetic_spec::decision_state(old(self), *old(context)).0"
FRAME = """final(self).bytes@ == old(self).bytes@,
final(self).bytes == old(self).bytes,"""
SIGNAL_FRAME = """out <= RASTER_SIGNAL_OVERFLOW,
arithmetic_spec::decoder_state_valid(final(self)),
arithmetic_spec::context_valid(*final(context)),
""" + FRAME + """
out <= 1 ==> arithmetic_spec::decoder_valid(final(self)),
""" + SIGNAL_POST
ERROR_SITE = "return self.decode_lower_signal(context, state, qe, old_mps);"
FAST_SITE = """if self.interval >= MIN_INTERVAL {
    return old_mps;
}"""
STATE_STATUS = "out == arithmetic_spec::decision_state(old(self), *old(context)).0,"
STATE_FIELDS = {
    name: f"final(self).{field} == arithmetic_spec::decision_state(old(self), *old(context)).{index},"
    for name, field, index in (
        ("interval", "interval", 1), ("position", "byte_position", 2),
        ("current", "current_byte", 3), ("code", "code", 4),
        ("bits", "bit_count", 5), ("marker", "marker", 6),
    )
}
STATE_POST = "\n".join((STATE_STATUS, *STATE_FIELDS.values()))
ERROR_POST = "ensures out == arithmetic_spec::error_for_signal(self, signal),"


def state_case(identity, change, *removed):
    clauses = {"signal": STATE_STATUS, **STATE_FIELDS}
    return {
        "id": identity, "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::decode_signal", "site": FAST_SITE,
        "replacement": FAST_SITE.replace("return old_mps;", change + "\nreturn old_mps;"),
        "controls": ((STATE_POST, "\n".join(
            value for name, value in clauses.items() if name not in removed
        )),),
    }


def error_case(identity, change, controls):
    return {
        "id": identity, "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::decode_signal", "site": ERROR_SITE,
        "replacement": (
            "let signal = self.decode_lower_signal(context, state, qe, old_mps);\n"
            f"if signal > 1 {{ {change} }}\nreturn signal;"
        ),
        "controls": controls,
    }


def exchange_post(lower):
    return f"""arithmetic_spec::context_index(*final(context)) == arithmetic_spec::exchange_index(
    {lower}, old(self).interval, qe, state.next_mps, state.next_lps,
),
arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::exchange_mps(
    {lower}, old(self).interval, qe, state.switch_mps, arithmetic_spec::context_mps(*old(context)), old_mps,
),"""


CASES = (
    {
        "id": "signal_returns_wrong_fast_decision",
        "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::decode_signal",
        "site": FAST_SITE,
        "replacement": FAST_SITE.replace("return old_mps;", "return 1 - old_mps;"),
        "controls": (
            (SIGNAL_DECISION, ""),
            (STATE_POST, STATE_POST.replace(STATE_STATUS, "")),
        ),
    },
    {
        "id": "checked_wrapper_returns_wrong_successful_decision",
        "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::decode",
        "site": """let signal = self.decode_signal(context);
if signal <= 1 {
Ok(signal)""",
        "replacement": """let signal = self.decode_signal(context);
if signal <= 1 {
Ok(1 - signal)""",
        "controls": ((CHECKED_POST, ""), (CHECKED_STATE_DECISION, "")),
    },
) + tuple(
    {
        "id": f"{side}_exchange_uses_wrong_error_context",
        "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": f"ArithmeticDecoder::decode_{side}_signal",
        "site": site,
        "replacement": site.replace(
            "if signal == 0 {", "if signal != 0 { context.set_state_and_mps(0, context.mps()); }\nif signal == 0 {",
        ),
        "controls": ((exchange_post(lower), ""),),
    }
    for side, lower, site in (
        ("lower", "true", """self.interval = qe;
let signal = self.renormalize_signal();
if signal == 0 {"""),
        ("upper", "false", """decision = old_mps;
context.set_state_and_mps(state.next_mps, prior_mps);
}
let signal = self.renormalize_signal();
if signal == 0 {"""),
    )
) + (
    error_case(
        "entry_error_discards_context_index", "context.set_state_and_mps(0, context.mps());",
        ((SIGNAL_POST, SIGNAL_POST.replace(
            "arithmetic_spec::context_index(*final(context)) ==",
            "out <= 1 ==> arithmetic_spec::context_index(*final(context)) ==",
        )),),
    ),
    error_case(
        "entry_error_discards_mps", "context.set_state_and_mps(context.state_index(), 0);",
        ((SIGNAL_POST, SIGNAL_POST.replace(
            "arithmetic_spec::context_mps(*final(context)) ==",
            "out <= 1 ==> arithmetic_spec::context_mps(*final(context)) ==",
        )),),
    ),
    error_case(
        "entry_error_replaces_input",
        "if self.marker.is_none() && self.byte_position < self.bytes.len() - 1 { "
        "self.bytes = &self.bytes[..self.bytes.len() - 1]; }",
        ((SIGNAL_FRAME, SIGNAL_FRAME.replace(FRAME, "")),),
    ),
) + (
    state_case("decision_returns_wrong_interval", "self.interval = MIN_INTERVAL;", "interval"),
    state_case("decision_discards_code", "self.code = 0;", "code"),
    state_case("decision_discards_bit_count", "self.bit_count = 0;", "bits"),
    state_case("decision_discards_marker", "self.marker = None;", "marker"),
    state_case(
        "decision_rewinds_consumed_history",
        "self.byte_position = 0; self.current_byte = self.bytes[0]; self.marker = None;",
        "position", "current", "marker",
    ),
    {
        **state_case("decision_rejects_fast_success", "", "signal"),
        "replacement": FAST_SITE.replace("return old_mps;", "return RASTER_SIGNAL_EOF;"),
    },
) + tuple(
    {
        "id": identity, "file": "src/arithmetic.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::signal_error",
        "site": site, "replacement": replacement,
        "controls": ((ERROR_POST, "ensures true,"),),
    }
    for identity, site, replacement in (
        (
            "decision_eof_error_loses_offset",
            """match self.byte_position.checked_add(1) {
Some(offset) => unexpected_eof(offset, 1, 0),""",
            """match self.byte_position.checked_add(1) {
Some(offset) => unexpected_eof(0, 1, 0),""",
        ),
        (
            "decision_overflow_error_misclassified",
            """None => ArithmeticError::Malformed(MalformedError::IntegerOverflow),
}
} else {
ArithmeticError::Malformed(MalformedError::IntegerOverflow)
}""",
            """None => ArithmeticError::Malformed(MalformedError::IntegerOverflow),
}
} else {
unexpected_eof(0, 1, 0)
}""",
        ),
    )
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, args.verus_executable.resolve(), CASES,
            ("src/arithmetic.rs", "specs/arithmetic.rs", "proofs/model_support/arithmetic.rs"),
            "jbig2-arithmetic-decision-",
            "Exact decision signal/register/input-position/current-byte/count/marker and typed failure projection, unconditional context updates and input frame; stream/caller composition and independent normative interpretation remain separate",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"arithmetic decision proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
