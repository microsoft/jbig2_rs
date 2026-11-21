// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

#[path = "generic_region/framing.rs"]
mod framing;
pub(crate) use framing::{
    arithmetic_terminator_validation, bounded_arithmetic_tail, early_arithmetic_marker,
    final_marker_result, generic_header_length, observed_marker_result,
};

verus! {

// Model navigation: spec::contexts_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn contexts_valid(
    contexts: Seq<crate::arithmetic::ArithmeticContext>,
) -> bool {
    forall|index: int| 0 <= index < contexts.len() ==>
        crate::arithmetic::arithmetic_spec::context_valid(contexts[index])
}

// Project diagnostic policy, not a T.88 arithmetic-decoding equation.
// Model navigation: spec::probability_validation_result [project-policy]
// Purpose: Project storage/work estimates and typed context/decoder error policy.
// Audit: generic-probability-validation-result
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn probability_validation_result(
    contexts: Seq<crate::arithmetic::ArithmeticContext>,
    result: Result<(), super::GenericRegionError>,
) -> bool {
    match result {
        Ok(()) => contexts_valid(contexts),
        Err(error) => exists|index: int| 0 <= index < contexts.len()
            && !crate::arithmetic::arithmetic_spec::context_valid(contexts[index])
            && (forall|before: int| 0 <= before < index ==>
                crate::arithmetic::arithmetic_spec::context_valid(contexts[before]))
            && error == super::GenericRegionError::Decode(
                crate::DecodeError::Malformed(crate::MalformedError::InvalidArithmeticContextState {
                    state_index: contexts[index].state_index,
                    mps: contexts[index].mps,
                }),
            ),
    }
}

// Model navigation: spec::probability_error_for_signal [project-policy]
// Purpose: Project storage/work estimates and typed context/decoder error policy.
// Audit: generic-probability-signal-error
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn probability_error_for_signal(
    decoder: &crate::arithmetic::ArithmeticDecoder,
    index: usize,
    context_count: usize,
    signal: u8,
) -> super::GenericRegionError {
    if signal == super::RASTER_SIGNAL_CONTEXT {
        super::GenericRegionError::ContextIndex { index, count: context_count }
    } else {
        super::GenericRegionError::Decode(
            crate::arithmetic::arithmetic_spec::decode_error(
                crate::arithmetic::arithmetic_spec::error_for_signal(decoder, signal),
            ),
        )
    }
}

// Model navigation: spec::generic_contexts_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn generic_contexts_valid(
    contexts: &crate::generic_region::GenericContexts,
) -> bool {
    contexts_valid(contexts.contexts@)
}

// Model navigation: spec::refinement_contexts_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_contexts_valid(
    contexts: &crate::generic_region::RefinementContexts,
) -> bool {
    contexts_valid(contexts.contexts@)
}

// Model navigation: spec::generic_snapshot_view [project-result-model]
// Purpose: Logical snapshot metadata and exact probability bank without widening runtime fields.
// Audit: generic-snapshot-view
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn generic_snapshot_view(
    snapshot: &super::GenericContextSnapshot,
) -> (super::GenericTemplate, bool, Seq<crate::arithmetic::ArithmeticContext>, u64) {
    (snapshot.template, snapshot.extended_template, snapshot.contexts@, snapshot.storage_bytes)
}

pub(super) proof fn generic_snapshot_view_definition()
    ensures forall|snapshot: &super::GenericContextSnapshot|
        #[trigger] generic_snapshot_view(snapshot)
            == (snapshot.template, snapshot.extended_template, snapshot.contexts@, snapshot.storage_bytes),
{
    reveal(generic_snapshot_view);
}

// Model navigation: spec::refinement_snapshot_view [project-result-model]
// Purpose: Logical snapshot metadata and exact probability bank without widening runtime fields.
// Audit: refinement-snapshot-view
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn refinement_snapshot_view(
    snapshot: &super::RefinementContextSnapshot,
) -> (super::RefinementTemplate, Seq<crate::arithmetic::ArithmeticContext>, u64) {
    (snapshot.template, snapshot.contexts@, snapshot.storage_bytes)
}

pub(super) proof fn refinement_snapshot_view_definition()
    ensures forall|snapshot: &super::RefinementContextSnapshot|
        #[trigger] refinement_snapshot_view(snapshot)
            == (snapshot.template, snapshot.contexts@, snapshot.storage_bytes),
{
    reveal(refinement_snapshot_view);
}

// Model navigation: spec::generic_session_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn generic_session_valid(
    session: &crate::generic_region::GenericArithmeticSession,
) -> bool {
    crate::arithmetic::arithmetic_spec::decoder_valid(&session.decoder)
        && generic_contexts_valid(&session.contexts)
}

// Model navigation: spec::generic_template_code [proof-support]
// Purpose: Template identity, powers of two and remaining-extent measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_template_code(
    template: super::GenericTemplate,
) -> nat {
    match template {
        super::GenericTemplate::Template0 => 0,
        super::GenericTemplate::Template1 => 1,
        super::GenericTemplate::Template2 => 2,
        super::GenericTemplate::Template3 => 3,
    }
}

// Model navigation: spec::generic_context_bits [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Audit: template-bits
// REQUIREMENT CONTEXT: T88-GENERIC-CONTEXT-SIZE | ITU-T T.88 (08/2018) | 6.2.5.3; Figures 3-6
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn generic_context_bits(
    template: super::GenericTemplate,
) -> nat {
    match template {
        super::GenericTemplate::Template0 => 16,
        super::GenericTemplate::Template1 => 13,
        super::GenericTemplate::Template2 => 10,
        super::GenericTemplate::Template3 => 10,
    }
}

// Model navigation: spec::generic_sltp_context [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Audit: sltp-index
// REQUIREMENT CONTEXT: T88-GENERIC-SLTP-CONTEXT | ITU-T T.88 (08/2018) | 6.2.5.7 step 3b; Figures 8-11
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn generic_sltp_context(
    template: super::GenericTemplate,
) -> nat {
    match template {
        super::GenericTemplate::Template0 => 0x9b25,
        super::GenericTemplate::Template1 => 0x0795,
        super::GenericTemplate::Template2 => 0x00e5,
        super::GenericTemplate::Template3 => 0x0195,
    }
}

// Model navigation: spec::refinement_context_bits [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Audit: refinement-context-bits
// REQUIREMENT CONTEXT: T88-REFINEMENT-CONTEXT-SIZE | ITU-T T.88 (08/2018) | 6.3.5.3; Figures 12-13
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_context_bits(
    template: super::RefinementTemplate,
) -> nat {
    match template {
        super::RefinementTemplate::Template0 => 13,
        super::RefinementTemplate::Template1 => 10,
    }
}

// Model navigation: spec::refinement_sltp_context [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Audit: refinement-sltp-context
// REQUIREMENT CONTEXT: T88-REFINEMENT-SLTP-CONTEXT | ITU-T T.88 (08/2018) | 6.3.5.6 step 3b; Figures 14-15
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_sltp_context(
    template: super::RefinementTemplate,
) -> nat {
    match template {
        super::RefinementTemplate::Template0 => 0x10,
        super::RefinementTemplate::Template1 => 0x08,
    }
}

// Model navigation: spec::pow2 [proof-support]
// Purpose: Template identity, powers of two and remaining-extent measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pow2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * pow2((exponent - 1) as nat)
    }
}

// Model navigation: spec::target_adaptive_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn target_adaptive_valid(
    offset: super::TemplateOffset,
) -> bool {
    offset.y < 0 || (offset.y == 0 && offset.x < 0)
}

// Model navigation: spec::generic_slot_count [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_slot_count(
    template: super::GenericTemplate,
) -> nat {
    generic_context_bits(template)
}

// Model navigation: spec::generic_adaptive_count [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_adaptive_count(
    template: super::GenericTemplate,
    extended: bool,
) -> nat {
    if template == super::GenericTemplate::Template0 {
        if extended { 12 } else { 4 }
    } else {
        1
    }
}

/// The adaptive-pixel number occupying a geometric context slot. `None`
/// denotes a fixed slot. The slot index is the context bit's MSB-first index.
// Model navigation: spec::generic_slot_adaptive [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_slot_adaptive(
    template: super::GenericTemplate,
    extended: bool,
    slot: nat,
) -> Option<nat> {
    match template {
        super::GenericTemplate::Template0 if extended => {
            if slot == 0 { Some(11) } else if slot == 1 { Some(4) }
            else if slot == 2 { Some(2) } else if slot == 3 { Some(5) }
            else if slot == 4 { Some(9) } else if slot == 5 { Some(12) }
            else if slot == 6 { Some(3) } else if slot == 10 { Some(6) }
            else if slot == 11 { Some(10) } else if slot == 12 { Some(8) }
            else if slot == 13 { Some(7) } else if slot == 14 { Some(1) }
            else { None }
        },
        super::GenericTemplate::Template0 => {
            if slot == 0 { Some(4) } else if slot == 4 { Some(3) }
            else if slot == 5 { Some(2) } else if slot == 11 { Some(1) }
            else { None }
        },
        super::GenericTemplate::Template1 => {
            if slot == 9 { Some(1) } else { None }
        },
        super::GenericTemplate::Template2 => {
            if slot == 7 { Some(1) } else { None }
        },
        super::GenericTemplate::Template3 => {
            if slot == 5 { Some(1) } else { None }
        },
    }
}

// Model navigation: spec::generic_slot_x [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_slot_x(
    template: super::GenericTemplate,
    slot: int,
) -> int {
    match template {
        super::GenericTemplate::Template0 => {
            if slot < 5 { slot - 2 }
            else if slot < 12 { slot - 8 }
            else { slot - 16 }
        },
        super::GenericTemplate::Template1 => {
            if slot < 4 { slot - 1 }
            else if slot < 10 { slot - 6 }
            else { slot - 13 }
        },
        super::GenericTemplate::Template2 => {
            if slot < 3 { slot - 1 }
            else if slot < 8 { slot - 5 }
            else { slot - 10 }
        },
        super::GenericTemplate::Template3 => {
            if slot < 6 { slot - 3 } else { slot - 10 }
        },
    }
}

// Model navigation: spec::generic_slot_y [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_slot_y(
    template: super::GenericTemplate,
    slot: int,
) -> int {
    match template {
        super::GenericTemplate::Template0 => {
            if slot < 5 { -2 } else if slot < 12 { -1 } else { 0 }
        },
        super::GenericTemplate::Template1 => {
            if slot < 4 { -2 } else if slot < 10 { -1 } else { 0 }
        },
        super::GenericTemplate::Template2 => {
            if slot < 3 { -2 } else if slot < 8 { -1 } else { 0 }
        },
        super::GenericTemplate::Template3 => {
            if slot < 6 { -1 } else { 0 }
        },
    }
}

// Model navigation: spec::generic_slot_matches [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_slot_matches(
    actual: super::GenericSlot,
    template: super::GenericTemplate,
    extended: bool,
    slot: int,
) -> bool {
    actual.nominal.x as int == generic_slot_x(template, slot)
        && actual.nominal.y as int == generic_slot_y(template, slot)
        && match (actual.adaptive, generic_slot_adaptive(
            template,
            extended,
            slot as nat,
        )) {
            (Some(actual_number), Some(spec_number)) =>
                actual_number as nat == spec_number,
            (None, None) => true,
            _ => false,
        }
}

// Model navigation: spec::refinement_slot_count [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_count(
    template: super::RefinementTemplate,
) -> nat {
    refinement_context_bits(template)
}

// Model navigation: spec::refinement_slot_x [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_x(
    template: super::RefinementTemplate,
    slot: int,
) -> int {
    if slot < 4 {
        if slot == 0 { -1 } else if slot == 1 { 0 }
        else if slot == 2 { 1 } else { -1 }
    } else {
        match template {
            super::RefinementTemplate::Template0 => {
                (slot - 4) % 3 - 1
            },
            super::RefinementTemplate::Template1 => {
                if slot == 4 { 0 } else if slot == 5 { -1 }
                else if slot == 6 { 0 } else if slot == 7 { 1 }
                else if slot == 8 { 0 } else { 1 }
            },
        }
    }
}

// Model navigation: spec::refinement_slot_y [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_y(
    template: super::RefinementTemplate,
    slot: int,
) -> int {
    if slot < 3 {
        -1
    } else if slot == 3 {
        0
    } else {
        match template {
            super::RefinementTemplate::Template0 => (slot - 4) / 3 - 1,
            super::RefinementTemplate::Template1 => {
                if slot == 4 { -1 } else if slot == 5 { 0 }
                else if slot == 6 { 0 } else if slot == 7 { 0 }
                else { 1 }
            },
        }
    }
}

// Model navigation: spec::refinement_slot_matches [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_matches(
    actual: super::RefinementSlot,
    template: super::RefinementTemplate,
    slot: int,
) -> bool {
    actual.nominal.x as int == refinement_slot_x(template, slot)
        && actual.nominal.y as int == refinement_slot_y(template, slot)
        && (actual.plane == super::RefinementPlane::Target)
            == refinement_slot_is_target(template, slot as nat)
        && match (actual.adaptive, refinement_slot_adaptive(
            template,
            slot as nat,
        )) {
            (Some(actual_number), Some(spec_number)) =>
                actual_number as nat == spec_number,
            (None, None) => true,
            _ => false,
        }
}

// Model navigation: spec::refinement_slot_is_target [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_is_target(
    template: super::RefinementTemplate,
    slot: nat,
) -> bool {
    match template {
        super::RefinementTemplate::Template0 => slot < 4,
        super::RefinementTemplate::Template1 => slot < 4,
    }
}

// Model navigation: spec::refinement_slot_adaptive [candidate-standard-facing-model]
// Purpose: Template context sizes, slots, adaptive coordinates and plane candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_slot_adaptive(
    template: super::RefinementTemplate,
    slot: nat,
) -> Option<nat> {
    match template {
        super::RefinementTemplate::Template0 => {
            if slot == 0 { Some(1) }
            else if slot == 4 { Some(2) }
            else { None }
        },
        super::RefinementTemplate::Template1 => None,
    }
}

// Model navigation: spec::context_value [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn context_value(bits: Seq<bool>) -> nat
    decreases bits.len(),
{
    if bits.len() == 0 {
        0
    } else {
        2 * context_value(bits.drop_last())
            + if bits.last() { 1nat } else { 0nat }
    }
}

// Model navigation: spec::sample_or_zero [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn sample_or_zero(
    pixels: Seq<Seq<bool>>,
    x: int,
    y: int,
) -> bool {
    if 0 <= y < pixels.len()
        && 0 <= x < pixels[y].len()
    {
        pixels[y][x]
    } else {
        false
    }
}

// Model navigation: spec::tpgd_next [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn tpgd_next(ltp: bool, sltp: bool) -> bool {
    ltp != sltp
}

// Model navigation: spec::generic_pixel_decisions [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_pixel_decisions(
    width: nat,
    ltp: bool,
    skipped: nat,
) -> nat {
    if ltp {
        0
    } else if skipped <= width {
        (width - skipped) as nat
    } else {
        0
    }
}

// Model navigation: spec::tpgr_uses_prediction [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn tpgr_uses_prediction(
    ltp: bool,
    neighbourhood_uniform: bool,
) -> bool {
    ltp && neighbourhood_uniform
}

// Model navigation: spec::refinement_pixel_decisions [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_pixel_decisions(
    pixels: nat,
    predictable: nat,
    ltp: bool,
) -> nat {
    if ltp && predictable <= pixels {
        (pixels - predictable) as nat
    } else if ltp {
        0
    } else {
        pixels
    }
}

// Model navigation: spec::generic_work_spec [project-policy]
// Purpose: Project storage/work estimates and typed context/decoder error policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn generic_work_spec(
    width: nat,
    rows: nat,
    typical_prediction: bool,
) -> nat {
    width * rows * 17 + if typical_prediction { rows } else { 0 }
}

// Model navigation: spec::refinement_work_spec [project-policy]
// Purpose: Project storage/work estimates and typed context/decoder error policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn refinement_work_spec(
    width: nat,
    height: nat,
    typical_prediction: bool,
) -> nat {
    width * height * (if typical_prediction { 23nat } else { 14nat }) + height
}

// Model navigation: spec::bitmap_storage_spec [project-policy]
// Purpose: Project storage/work estimates and typed context/decoder error policy.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bitmap_storage_spec(width: nat, height: nat) -> nat {
    ((width + 7) / 8) * height
}

// Model navigation: spec::contexts_compatible [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn contexts_compatible(
    template: nat,
    extended: bool,
    context_count: nat,
    snapshot_template: nat,
    snapshot_extended: bool,
    snapshot_count: nat,
) -> bool {
    template == snapshot_template
        && extended == snapshot_extended
        && context_count == snapshot_count
}

// Model navigation: spec::probability_states_valid [representation-invariant]
// Purpose: Context banks, decoder sessions, snapshots and preceding adaptive-pixel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn probability_states_valid(
    states: Seq<(nat, nat)>,
) -> bool {
    forall|index: int| 0 <= index < states.len() ==>
        states[index].0 < 47 && states[index].1 <= 1
}

// Model navigation: spec::restore_context_states [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn restore_context_states(
    current: Seq<(nat, nat)>,
    snapshot: Seq<(nat, nat)>,
    compatible: bool,
) -> Seq<(nat, nat)> {
    if compatible && current.len() == snapshot.len() {
        snapshot
    } else {
        current
    }
}

// Model navigation: spec::reset_context_states [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn reset_context_states(count: nat) -> Seq<(nat, nat)> {
    Seq::new(count, |_index: int| (0nat, 0nat))
}

// Model navigation: spec::decoded_pixels [project-result-model]
// Purpose: Supplied decision prefix only; does not decode bytes or inspect context values.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn decoded_pixels(
    decisions: Seq<bool>,
    context_indexes: Seq<nat>,
) -> Seq<bool> {
    Seq::new(
        if decisions.len() < context_indexes.len() {
            decisions.len()
        } else {
            context_indexes.len()
        },
        |index: int| decisions[index],
    )
}

// Model navigation: spec::unknown_length_accounting [project-result-model]
// Purpose: Context-bit folds, white boundary sampling, restored states and marker extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn unknown_length_accounting(
    marker_offset: nat,
    mmr: bool,
) -> (nat, nat) {
    // Arithmetic includes FF AC in its coded slice; MMR excludes 00 00.
    (if mmr { marker_offset } else { marker_offset + 2 }, marker_offset + 6)
}

// Model navigation: spec::row_measure [proof-support]
// Purpose: Template identity, powers of two and remaining-extent measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn row_measure(row: nat, rows: nat) -> nat {
    if row <= rows { (rows - row) as nat } else { 0 }
}

// Model navigation: spec::pixel_measure [proof-support]
// Purpose: Template identity, powers of two and remaining-extent measures.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn pixel_measure(x: nat, width: nat) -> nat {
    if x <= width { (width - x) as nat } else { 0 }
}

}
