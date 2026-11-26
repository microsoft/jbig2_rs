// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: ../README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::context_index [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn context_index(context: crate::arithmetic::ArithmeticContext) -> u8 {
    (context.packed / 2) as u8
}

// Model navigation: spec::context_mps [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn context_mps(context: crate::arithmetic::ArithmeticContext) -> u8 {
    (context.packed % 2) as u8
}

// Model navigation: spec::packed_context [representation-invariant]
// Purpose: Context/register/input-position representation and normalization invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn packed_context(
    state_index: u8,
    mps: u8,
) -> crate::arithmetic::ArithmeticContext {
    crate::arithmetic::ArithmeticContext {
        packed: (state_index * 2 + mps) as u8,
    }
}

}
