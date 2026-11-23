#!/usr/bin/env python3
"""Check terminal uncertainty, saved-prefix provenance and final-DS errors."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases


ROOT = Path(__file__).resolve().parents[1]
INITIAL_SPREAD = "out.spread == certainty_spec::initial_spread(decoder),"
INITIAL_TRACE = "certainty_spec::observer_valid(&out, decoder),"
INITIAL_RETURN = """observer
    }

    /// Call only for a real, successful decision"""
DECISION_POST = """old(self).active ==> (
    final(self).active
        == certainty_spec::decision_spread(before, context, old(self).spread,
            old(self).entropy_end).is_some()
    && (final(self).active ==> final(self).spread
        == certainty_spec::decision_spread(before, context, old(self).spread,
            old(self).entropy_end).unwrap())
),"""
CROSSING = """if self.spread > before.code
    || (before.code >= threshold && before.code - self.spread < threshold)
{
    self.active = false;
    return;
}"""
CAPTURE_RETURN = "self.active\n    }\n}"
CAPTURE_ACTIVE = "out == old(self).active,"
CAPTURE_PREFIX = "out ==> certainty_spec::saved_prefix(final(self), instances),"
DS_CALL = """match self.decode_integer(IntegerField::SubsequentS)? {
    EntropyInteger::OutOfBand => return Ok(false),
    EntropyInteger::Value(_) => {}
}"""
DS_ERROR = """Err(error) => out == Err(error),
Ok(EntropyInteger::OutOfBand) => out == Ok(false),"""
INIT_TEST = (
    "arithmetic::tests::prefix_certainty::"
    "certificate_initialization_accounts_for_prefix_and_fill_separately"
)
DS_TEST = (
    "symbol_text::tests::text_termination::"
    "final_arithmetic_ds_errors_propagate_despite_native_recovery_gates"
)
FILES = (
    "src/arithmetic/certainty.rs", "specs/arithmetic_certainty.rs",
    "src/symbol_text/codec/arithmetic.rs", "specs/arithmetic.rs",
    "specs/symbol_text.rs",
)


def case(identity, file, module, function, site, replacement, controls, native_test):
    return {
        "id": identity, "file": file, "module": module, "function": function,
        "site": site, "replacement": replacement, "controls": controls,
        "native_test": native_test,
    }


CASES = (
    case(
        "initial_terminal_prefix_contribution_discarded",
        "src/arithmetic/certainty.rs", "arithmetic::certainty", "PrefixCertainty::new",
        INITIAL_RETURN,
        """let mut observer = observer;
if decoder.byte_position >= entropy_end && decoder.marker.is_none() {
    observer.spread = 0;
}
""" + INITIAL_RETURN,
        ((INITIAL_SPREAD, ""), (INITIAL_TRACE, "")), INIT_TEST,
    ),
    case(
        "threshold_crossing_falsely_certified",
        "src/arithmetic/certainty.rs", "arithmetic::certainty", "PrefixCertainty::observe",
        CROSSING, CROSSING.replace(
            "self.active = false;", "self.active = self.spread <= before.code;",
        ),
        ((DECISION_POST, ""),), INIT_TEST,
    ),
    case(
        "inactive_trace_reported_as_saved_prefix",
        "src/arithmetic/certainty.rs", "arithmetic::certainty", "PrefixCertainty::capture",
        CAPTURE_RETURN, CAPTURE_RETURN.replace("self.active", "true"),
        ((CAPTURE_ACTIVE, ""), (CAPTURE_PREFIX, "")), INIT_TEST,
    ),
    case(
        "final_ds_error_reclassified",
        "src/symbol_text/codec/arithmetic.rs", "symbol_text",
        "ArithmeticCodec::decode_final_strip", DS_CALL,
        """match self.decode_integer(IntegerField::SubsequentS) {
    Ok(EntropyInteger::OutOfBand) => return Ok(false),
    Ok(EntropyInteger::Value(_)) => {}
    Err(_error) => return Err(SymbolTextError::InvalidCount("too many text instances")),
}""",
        ((DS_ERROR, DS_ERROR.replace("Err(error) => out == Err(error),", "Err(_) => true,")),),
        DS_TEST,
    ),
)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--verus-executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        report = check_semantic_cases(
            ROOT, arguments.verus_executable.resolve(), CASES, FILES,
            "jbig2-arithmetic-certainty-",
            "Initialized terminal-prefix numeric contribution; threshold-crossing "
            "certainty/enclosure; real saved-prefix provenance; unchanged final-DS "
            "errors. Ordinary entropy results and immutable frames remain enforced.",
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"arithmetic certainty proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
