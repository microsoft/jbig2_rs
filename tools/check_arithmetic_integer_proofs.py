#!/usr/bin/env python3
"""Check exact integer-bit history, context frames and inherited step state."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

from proof_sensitivity import check_semantic_cases, replace_once


ROOT = Path(__file__).resolve().parents[1]
BASELINE_GROUPS = (("arithmetic", (
    "ArithmeticDecoder::decode_iaid", "ArithmeticDecoder::decode_integer_bit",
)),)
CALL = "let decision = self.decode(&mut contexts[context_index])?;"
HISTORY = """*previous = if prior < 256 {
    shifted
} else {
    (shifted & 511) | 256
};"""
HISTORY_POST = "&& *final(previous) == arithmetic_spec::integer_history_step(*old(previous), decision)"
BIT_POST = """&& decision == arithmetic_spec::integer_bit_state(
    old(self), old(contexts)@, *old(previous),
).0"""
FRAME_POST = """forall|index: int| 0 <= index < old(contexts)@.len()
    && index != (*old(previous) & 511) as int ==>
        final(contexts)@[index] == old(contexts)@[index],"""
ERROR_HISTORY = """*final(previous) == *old(previous)
    && error == arithmetic_spec::error_for_signal("""
CODE_POST = "final(self).code == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).4,"
LENGTH_RESULT = "out == Err(ArithmeticError::InvalidIaidCodeLength(code_length))"
LENGTH_POST = """code_length > MAX_IAID_CODE_LENGTH ==> {
    &&& out == Err(ArithmeticError::InvalidIaidCodeLength(code_length))
    &&& *final(self) == *old(self)
    &&& final(contexts)@ == old(contexts)@
},"""
STORAGE_RESULT = """out == Err(ArithmeticError::InsufficientContexts {
    required: 1usize << code_length, actual: old(contexts)@.len() as usize,
})"""
STORAGE_POST = """code_length <= MAX_IAID_CODE_LENGTH
    && old(contexts)@.len() < (1usize << code_length) ==> {
    &&& out == Err(ArithmeticError::InsufficientContexts {
        required: 1usize << code_length, actual: old(contexts)@.len() as usize,
    })
    &&& *final(self) == *old(self)
    &&& final(contexts)@ == old(contexts)@
},"""
IAID_HEAD = """pub(crate) fn decode_iaid(
    &mut self, code_length: u8, contexts: &mut [ArithmeticContext],
) -> Result<u32, ArithmeticError> {"""
LENGTH_SITE = IAID_HEAD + """
if code_length > MAX_IAID_CODE_LENGTH {
    return Err(ArithmeticError::InvalidIaidCodeLength(code_length));
}"""
STORAGE_SITE = LENGTH_SITE + """
let required = 1_usize << code_length;
#[cfg(verus_keep_ghost)]
proof! {
    assert(code_length == 0 ==> required == 1) by(bit_vector)
        requires required == 1usize << code_length;
}
let context_count = contexts.len();
if context_count < required {
    return Err(ArithmeticError::InsufficientContexts {
        required,
        actual: context_count,
    });
}"""
IAID_RESULT = "out == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).0,"
IAID_CONTEXTS = "final(contexts)@ == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).2,"
STEP_POSTS = tuple(
    f"""{target} == arithmetic_spec::integer_step_state(
        *old(self), old(contexts)@, *old(previous),
    ).{index},"""
    for index, target in enumerate(("out", "*final(self)", "final(contexts)@", "*final(previous)"))
)


def case(identity, site, replacement, control, weaker="", function="ArithmeticDecoder::decode_integer_bit"):
    controls = [(control, weaker)]
    affected = {
        "growing_history_discards_the_decision": (3,),
        "rolling_history_discards_the_retained_high_bit": (3,),
        "integer_bit_returns_the_wrong_decision": (0, 3),
        "integer_bit_changes_an_unselected_context": (2,),
        "integer_error_discards_history": (3,),
        "integer_bit_discards_the_exact_code_register": (1,),
    }
    controls.extend((STEP_POSTS[index], "") for index in affected.get(identity, ()))
    if identity in {
        "iaid_invalid_length_loses_the_supplied_value",
        "iaid_insufficient_storage_loses_required_count",
    }:
        controls.append((IAID_RESULT, ""))
    elif identity == "iaid_invalid_length_mutates_context_storage":
        controls.append((IAID_CONTEXTS, ""))
    return {
        "id": identity, "file": "src/arithmetic/integers.rs", "module": "arithmetic",
        "function": function,
        "site": site, "replacement": replacement,
        "controls": tuple(controls),
    }


CASES = (
    case(
        "growing_history_discards_the_decision", HISTORY,
        HISTORY + "\nif prior < 256 { *previous = 0; }", HISTORY_POST,
    ),
    case(
        "rolling_history_discards_the_retained_high_bit", HISTORY,
        HISTORY + "\nif prior >= 256 { *previous = shifted % 256; }", HISTORY_POST,
    ),
    case(
        "integer_bit_returns_the_wrong_decision", CALL,
        CALL + "\nlet decision = 1 - decision;", BIT_POST,
    ),
    case(
        "integer_bit_changes_an_unselected_context", CALL,
        CALL + "\ncontexts[(context_index + 1) % INTEGER_CONTEXT_COUNT] = ArithmeticContext::new();",
        FRAME_POST,
    ),
    case(
        "integer_error_discards_history", CALL,
        """let decision = match self.decode(&mut contexts[context_index]) {
            Ok(decision) => decision,
            Err(error) => { *previous = 0; return Err(error); }
        };""",
        ERROR_HISTORY, "error == arithmetic_spec::error_for_signal(",
    ),
    case(
        "integer_bit_discards_the_exact_code_register", CALL,
        CALL + "\nself.code = 0;", CODE_POST,
    ),
    case(
        "iaid_invalid_length_loses_the_supplied_value", LENGTH_SITE,
        LENGTH_SITE.replace("InvalidIaidCodeLength(code_length)", "InvalidIaidCodeLength(0)"),
        LENGTH_POST, LENGTH_POST.replace(LENGTH_RESULT, "true"),
        "ArithmeticDecoder::decode_iaid",
    ),
    case(
        "iaid_insufficient_storage_loses_required_count", STORAGE_SITE,
        STORAGE_SITE.replace("required,", "required: 0,"),
        STORAGE_POST, replace_once(STORAGE_POST, STORAGE_RESULT, "true"),
        "ArithmeticDecoder::decode_iaid",
    ),
    case(
        "iaid_invalid_length_mutates_context_storage", LENGTH_SITE,
        LENGTH_SITE.replace(
            "return Err",
            "if !contexts.is_empty() { contexts[0] = ArithmeticContext::new(); }\nreturn Err",
        ),
        LENGTH_POST, LENGTH_POST.replace("&&& final(contexts)@ == old(contexts)@", ""),
        "ArithmeticDecoder::decode_iaid",
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
            "jbig2-arithmetic-integer-",
            "Exact single integer-bit decision/history, context frames and decoder/error step; exact IAID preflight errors and unchanged state; complete integer/IAID folds and normative interpretation remain separate",
            baseline_groups=BASELINE_GROUPS,
        )
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"arithmetic integer proof sensitivity failed: {error}", file=sys.stderr)
        report = {"schema_version": 2, "status": "failed", "error": str(error)}
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
