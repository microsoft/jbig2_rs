// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: ../../specs/README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

pub(super) proof fn context_components_are_bounded(
    context: super::ArithmeticContext,
)
    requires super::arithmetic_spec::context_valid(context),
    ensures
        super::arithmetic_spec::context_index(context) < 47,
        super::arithmetic_spec::context_mps(context) <= 1,
{
    super::arithmetic_shared::packed_context_bounds(context);
}

pub(super) proof fn binary_history_index_is_valid(previous: u16)
    ensures (previous & 0x01ff) < 512,
{
    assert((previous & 0x01ff) < 512) by (bit_vector);
}

pub(super) proof fn iaid_value_bound(previous: usize, required: usize)
    requires
        required > 0,
        required <= previous,
        previous < required * 2,
        required <= 0x8000_0000,
    ensures
        previous - required < required,
        previous - required <= u32::MAX,
{
}

// Model navigation: spec::pow2 [proof-support]
// Purpose: Doubling and binary-prefix vocabulary for arithmetic properties.
// Audit: arithmetic-pow2
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PAYLOAD-ORDER | ITU-T T.88 (08/2018) | A.2, step 4; Figure A.1; Table A.1 payload fields (pp. 90-91)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn pow2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * pow2((exponent - 1) as nat)
    }
}

pub(super) proof fn pow2_positive(exponent: nat)
    ensures pow2(exponent) > 0,
    decreases exponent,
{
    if exponent > 0 {
        pow2_positive((exponent - 1) as nat);
    }
}

pub(super) proof fn pow2_step(exponent: nat)
    ensures pow2(exponent + 1) == 2 * pow2(exponent),
{
}

pub(super) proof fn pow2_monotonic(lower: nat, upper: nat)
    requires lower <= upper,
    ensures pow2(lower) <= pow2(upper),
    decreases upper - lower,
{
    if lower < upper {
        pow2_monotonic(lower, (upper - 1) as nat);
        pow2_step((upper - 1) as nat);
        pow2_positive((upper - 1) as nat);
    }
}

// Model navigation: spec::binary_value [proof-support]
// Purpose: Doubling and binary-prefix vocabulary for arithmetic properties.
// Audit: arithmetic-binary-value
// REQUIREMENT CONTEXT: T88-ARITHMETIC-INTEGER-PAYLOAD-ORDER | ITU-T T.88 (08/2018) | A.2, step 4; Figure A.1; Table A.1 payload fields (pp. 90-91)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn binary_value(
    decisions: Seq<u8>,
    count: nat,
) -> nat
    recommends count <= decisions.len(),
    decreases count,
{
    if count == 0 {
        0
    } else {
        binary_value(decisions, (count - 1) as nat) * 2
            + decisions[(count - 1) as int] as nat
    }
}

pub(super) proof fn binary_value_bound(
    decisions: Seq<u8>,
    count: nat,
)
    requires
        count <= decisions.len(),
        forall|index: int| 0 <= index < count ==>
            decisions[index] <= 1,
    ensures binary_value(decisions, count) < pow2(count),
    decreases count,
{
    if count > 0 {
        binary_value_bound(decisions, (count - 1) as nat);
        pow2_step((count - 1) as nat);
    }
}

pub(super) proof fn iaid_context_index_is_valid(
    decisions: Seq<u8>,
    completed: nat,
    code_length: nat,
)
    requires
        completed < code_length,
        code_length <= decisions.len(),
        forall|index: int| 0 <= index < completed ==>
            decisions[index] <= 1,
    ensures
        pow2(completed) + binary_value(decisions, completed)
            < pow2(code_length),
{
    binary_value_bound(decisions, completed);
    pow2_step(completed);
    pow2_monotonic(completed + 1, code_length);
}

pub(super) proof fn iaid_recurrence_matches_binary_value(
    decisions: Seq<u8>,
    completed: nat,
)
    requires
        completed < decisions.len(),
        decisions[completed as int] <= 1,
    ensures
        (pow2(completed) + binary_value(decisions, completed)) * 2
            + decisions[completed as int] as nat
            == pow2(completed + 1)
                + binary_value(decisions, completed + 1),
{
    pow2_step(completed);
}

pub(super) proof fn iaid_loop_indexes_and_output(
    decisions: Seq<u8>,
    code_length: nat,
) -> (value: nat)
    requires
        code_length <= 31,
        decisions.len() == code_length,
        forall|index: int| 0 <= index < decisions.len() ==>
            decisions[index] <= 1,
    ensures
        value < pow2(code_length),
        value <= u32::MAX,
{
    binary_value_bound(decisions, code_length);
    assert(pow2(31) == 0x8000_0000) by (compute);
    pow2_monotonic(code_length, 31);
    let required = pow2(code_length);
    assert(required <= 0x8000_0000);
    let value = binary_value(decisions, code_length);
    assert(value <= u32::MAX);
    value
}

pub(super) proof fn integer_bits_loop_bound(
    decisions: Seq<u8>,
) -> (value: nat)
    requires
        decisions.len() <= 32,
        forall|index: int| 0 <= index < decisions.len() ==>
            decisions[index] <= 1,
    ensures
        value < pow2(decisions.len()),
        value <= u32::MAX,
{
    binary_value_bound(decisions, decisions.len());
    assert(pow2(32) == 0x1_0000_0000) by (compute);
    pow2_monotonic(decisions.len(), 32);
    let value = binary_value(decisions, decisions.len());
    assert(value <= u32::MAX);
    value
}

pub(super) proof fn integer_magnitude_fits_i64(
    magnitude_bits: u32,
    offset: u64,
)
    requires offset <= 4436,
    ensures offset + magnitude_bits <= i64::MAX,
{
}

}
