#!/usr/bin/env python3
"""Check complete IAID result, decoder and context-bank composition."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases


ROOT = Path(__file__).resolve().parents[1]
RESULT = "out == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).0,"
DECODER = "*final(self) == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).1,"
CONTEXTS = "final(contexts)@ == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).2,"
RANGE = """match out {
    Ok(value) => code_length <= MAX_IAID_CODE_LENGTH
        && (value as u64) < (1u64 << code_length),
    Err(_) => true,
},"""
ZERO = """code_length == 0 && old(contexts)@.len() >= 1 ==> {
    &&& out == Ok(0)
    &&& *final(self) == *old(self)
    &&& final(contexts)@ == old(contexts)@
},"""
FRAME = """code_length <= MAX_IAID_CODE_LENGTH ==>
    forall|index: int| 0 <= index < final(contexts)@.len()
        && (index == 0 || index >= (1usize << code_length)) ==>
            final(contexts)@[index] == old(contexts)@[index],"""
RETURN = """if value > u32::MAX as usize {
    return Err(ArithmeticError::Malformed(MalformedError::IntegerOverflow));
}
Ok(value as u32)"""
CALL = """Err(error) => {
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(ext_equal(
            ctxs@ + _rest@,
            before_contexts.update(
                before_previous as int,
                arithmetic_spec::context_after_decision(
                    &before,
                    before_contexts[before_previous as int],
                ),
            ),
        ));
    }
    loop_error = Some(error);
}"""


ERR_INV = """&&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required)
    == (Err(loop_error.unwrap()), *self, ctxs@ + _rest@)"""
ERR_INV_NO_RESULT = """&&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required).1 == *self
&&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required).2 == ctxs@ + _rest@"""
ERR_INV_NO_DECODER = """&&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required).0 == Err(loop_error.unwrap())
&&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required).2 == ctxs@ + _rest@"""


def case(identity, site, replacement, control, *overlapping, native_test=None, extra_controls=()):
    return {
        "id": identity, "file": "src/arithmetic/integers.rs", "module": "arithmetic",
        "function": "ArithmeticDecoder::decode_iaid",
        "site": site, "replacement": replacement,
        "controls": tuple((clause, "") for clause in (control, *overlapping)) + extra_controls,
        **({"native_test": native_test} if native_test is not None else {}),
    }


CASES = (
    case("iaid_discards_the_decoded_value", RETURN, RETURN.replace("Ok(value as u32)", "Ok(0)"), RESULT),
    case(
        "iaid_nonzero_empty_value", RETURN,
        RETURN.replace("Ok(value as u32)", "if code_length == 0 { return Ok(1); }\nOk(value as u32)"),
        RESULT, RANGE, ZERO,
    ),
    case(
        "iaid_discards_final_code_register", RETURN,
        RETURN.replace("Ok(value as u32)", "self.code = 0;\nOk(value as u32)"), DECODER, ZERO,
    ),
    case(
        "iaid_changes_unvisited_root_context", RETURN,
        RETURN.replace("Ok(value as u32)", "if !contexts.is_empty() { contexts[0] = ArithmeticContext::new(); }\nOk(value as u32)"),
        CONTEXTS, ZERO, FRAME,
    ),
    case(
        "iaid_masks_decision_eof_as_overflow", CALL,
        CALL.replace(
            "Err(error) => {",
            "Err(_error) => {",
        ).replace(
            "loop_error = Some(error);",
            "loop_error = Some(ArithmeticError::Malformed(MalformedError::IntegerOverflow));",
        ),
        RESULT,
        extra_controls=((ERR_INV, ERR_INV_NO_RESULT),),
    ),
    case(
        "iaid_rewinds_partial_error_input", CALL,
        CALL.replace(
            "loop_error = Some(error);",
            """self.byte_position = 0;
            self.current_byte = self.bytes[0];
            self.marker = None;
            loop_error = Some(error);""",
        ),
        DECODER,
        extra_controls=((ERR_INV, ERR_INV_NO_DECODER),),
    ),
    case(
        "iaid_returns_the_excluded_upper_bound", RETURN,
        RETURN.replace("Ok(value as u32)", """if code_length != 0 {
    return Ok(1u32 << code_length);
}
Ok(value as u32)"""),
        RESULT, RANGE,
        native_test="arithmetic::tests::integer_procedures::iaid_prefix_range_covers_both_endpoint_values_and_unvisited_contexts",
    ),
    case(
        "iaid_changes_outside_tree_context", RETURN,
        RETURN.replace("Ok(value as u32)", """if contexts.len() > required {
    contexts[required] = ArithmeticContext::new();
}
Ok(value as u32)"""),
        CONTEXTS, ZERO, FRAME,
        native_test="arithmetic::tests::integer_procedures::iaid_prefix_range_covers_both_endpoint_values_and_unvisited_contexts",
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
            ("src/arithmetic/integers.rs", "specs/arithmetic.rs", "proofs/model_support/arithmetic.rs",
             "proofs/production_impl/arithmetic.rs"),
            "jbig2-iaid-state-",
            "Exact project IAID result/state plus mathematical prefix/output range, zero-length identity and root/outside-tree frames; normative caller selection and standard interpretation remain separate",
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"IAID state proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
