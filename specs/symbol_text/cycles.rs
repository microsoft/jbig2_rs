//! Component model of exact zero-progress entropy orbits, not complete output correctness.

use super::*;
use crate::symbol_text::dictionary::cycles::{Checkpoint, CycleMode, IntegerCycle};

verus! {

pub(in crate::symbol_text) type CycleState<'a> = (
    crate::arithmetic::ArithmeticDecoder<'a>,
    Seq<crate::arithmetic::ArithmeticContext>,
    Seq<crate::arithmetic::ArithmeticContext>,
    (u32, u32, bool),
    Seq<u8>,
);

// Model navigation: spec::fields [representation-invariant]
// Purpose: Complete banks consulted by each zero-progress dictionary procedure.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn fields(mode: CycleMode) -> (usize, usize) {
    match mode {
        CycleMode::ExportRuns => (
            integer_field_slot(crate::symbol_text::IntegerField::ExportRun),
            integer_field_slot(crate::symbol_text::IntegerField::ExportRun),
        ),
        CycleMode::EmptyHeightClasses => (
            integer_field_slot(crate::symbol_text::IntegerField::HeightDelta),
            integer_field_slot(crate::symbol_text::IntegerField::WidthDelta),
        ),
    }
}

// Model navigation: spec::state [representation-invariant]
// Purpose: Exact decoder, complete procedure banks and logical dictionary cursor.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn state<'a>(
    codec: &crate::symbol_text::ArithmeticCodec<'a>, mode: CycleMode, cursor: (u32, u32, bool),
) -> CycleState<'a> {
    (codec.decoder, codec.integers.slots[fields(mode).0 as int]@,
        codec.integers.slots[fields(mode).1 as int]@, cursor, codec.decoder.bytes@)
}

// Model navigation: spec::source_state [representation-invariant]
// Purpose: Arithmetic-only state projection through existing entropy dispatch.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn source_state<'a>(
    codec: &crate::symbol_text::SymbolCodec<'a>, mode: CycleMode, cursor: (u32, u32, bool),
) -> Option<CycleState<'a>> {
    match *codec {
        crate::symbol_text::SymbolCodec::Arithmetic(arithmetic) => Some(state(&arithmetic, mode, cursor)),
        _ => None,
    }
}

// Model navigation: spec::checkpoint_state [representation-invariant]
// Purpose: Every native checkpoint field, including immutable entropy input.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn checkpoint_state<'a>(checkpoint: &Checkpoint<'a>) -> CycleState<'a> {
    (checkpoint.decoder, checkpoint.banks[0]@, checkpoint.banks[1]@,
        checkpoint.cursor, checkpoint.decoder.bytes@)
}

// Model navigation: spec::state_valid [representation-invariant]
// Purpose: Valid arithmetic registers and both complete512-entry procedure banks.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn state_valid(value: CycleState) -> bool {
    crate::arithmetic::arithmetic_spec::decoder_valid(&value.0)
        && value.4 == value.0.bytes@
        && value.1.len() == 512 && value.2.len() == 512
        && arithmetic_contexts_valid(value.1) && arithmetic_contexts_valid(value.2)
}

// Model navigation: spec::prefix_keys [representation-invariant]
// Purpose: Checkpoint-refresh keys only; recurrence still compares complete state.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn prefix_keys(mode: CycleMode, bank: int) -> Seq<int> {
    if bank == 0 || mode == CycleMode::ExportRuns { seq![1, 2, 4, 8] }
    else { seq![1, 3, 6, 12] }
}

// Model navigation: spec::prefix_equal [representation-invariant]
// Purpose: Exact equality of refresh keys, not an approximate cycle witness.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn prefix_equal(
    checkpoint: &Checkpoint, codec: &crate::symbol_text::ArithmeticCodec, mode: CycleMode,
) -> bool {
    forall|bank: int, key: int| 0 <= bank < 2 && 0 <= key < 4 ==>
        checkpoint.banks[bank][prefix_keys(mode, bank)[key]]
            == codec.integers.slots[if bank == 0 { fields(mode).0 as int }
                else { fields(mode).1 as int }][prefix_keys(mode, bank)[key]]
}

// Model navigation: spec::step [project-result-model]
// Purpose: Existing AnnexA integer transitions for a zero run or empty class.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn step<'a>(
    mode: CycleMode, value: CycleState<'a>,
) -> Option<CycleState<'a>> {
    let first = crate::arithmetic::arithmetic_spec::integer_state(value.0, value.1);
    if first.0 != Ok(crate::arithmetic::ArithmeticInteger::Value(0)) {
        None
    } else {
        match mode {
            CycleMode::ExportRuns => Some((
                first.1, first.2, first.2, (value.3.0, value.3.1, !value.3.2), value.4,
            )),
            CycleMode::EmptyHeightClasses => {
                let second = crate::arithmetic::arithmetic_spec::integer_state(first.1, value.2);
                if second.0 == Ok(crate::arithmetic::ArithmeticInteger::OutOfBand) {
                    Some((second.1, first.2, second.2, value.3, value.4))
                } else { None }
            },
        }
    }
}

// Model navigation: spec::chain [representation-invariant]
// Purpose: A finite trace of genuinely executed, valid zero-progress transitions.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn chain(mode: CycleMode, values: Seq<CycleState>) -> bool {
    (forall|index: int| 0 <= index < values.len() ==> state_valid(values[index]))
        && (forall|index: int| 0 <= index < values.len() ==>
            (#[trigger] values[index]).0.bytes == values[0].0.bytes
                && values[index].4 == values[0].4)
        && (forall|index: int| 0 <= index && index + 1 < values.len() ==>
            #[trigger] step(mode, values[index]) == Some(values[index + 1]))
}

// Model navigation: spec::detector_valid [representation-invariant]
// Purpose: Checkpoint provenance within the actual trace; no guessed recurrence.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn detector_valid(cycle: &IntegerCycle) -> bool {
    chain(cycle.mode, cycle.history@)
        && match cycle.checkpoint {
            None => cycle.history@.len() <= 1,
            Some(ref checkpoint) =>
                0 <= cycle.anchor@ < cycle.history@.len()
                && checkpoint_state(checkpoint) == cycle.history@[cycle.anchor@],
        }
}

// Model navigation: spec::repetition [representation-invariant]
// Purpose: Equality of an earlier and current state within a genuine zero-only trace.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn repetition(mode: CycleMode, values: Seq<CycleState>) -> bool {
    chain(mode, values) && exists|first: int|
        0 <= first < values.len() - 1 && values[first] == values.last()
}

pub(in crate::symbol_text) proof fn record<'a>(
    cycle: &IntegerCycle<'a>, before: Option<CycleState<'a>>, after: CycleState<'a>,
) -> (updated: (Seq<CycleState<'a>>, int))
    requires
        detector_valid(cycle), state_valid(after),
        cycle.checkpoint.is_some() ==> (
            before == Some(cycle.history@.last())
                && step(cycle.mode, before.unwrap()) == Some(after)
                && after.0.bytes == before.unwrap().0.bytes
                && after.4 == before.unwrap().4
        ),
    ensures
        chain(cycle.mode, updated.0), updated.0.len() > 0,
        updated.0.last() == after,
        match cycle.checkpoint {
            None => updated.0.len() == 1 && updated.1 == 0,
            Some(ref checkpoint) =>
                0 <= updated.1 < updated.0.len() - 1
                && checkpoint_state(checkpoint) == updated.0[updated.1],
        },
{
    if cycle.checkpoint.is_some() {
        let values = cycle.history@;
        let appended = values.push(after);
        assert forall|index: int| 0 <= index && index + 1 < appended.len()
            implies step(cycle.mode, appended[index]) == Some(appended[index + 1]) by {
            if index + 1 == values.len() {
                assert(values[index] == values.last());
            }
        }
        (appended, cycle.anchor@)
    } else {
        (seq![after], 0)
    }
}

pub(in crate::symbol_text) proof fn export_zero_step<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after: &crate::symbol_text::SymbolCodec<'a>, cursor: (u32, u32, bool),
)
    requires
        codec_valid(before),
        source_state(before, CycleMode::ExportRuns, cursor).is_some(),
        arithmetic_integer_transition(before, after, crate::symbol_text::IntegerField::ExportRun,
            Ok(crate::symbol_text::EntropyInteger::Value(0))),
    ensures
        source_state(after, CycleMode::ExportRuns, (cursor.0, cursor.1, !cursor.2)).is_some(),
        step(CycleMode::ExportRuns, source_state(before, CycleMode::ExportRuns, cursor).unwrap())
            == source_state(after, CycleMode::ExportRuns, (cursor.0, cursor.1, !cursor.2)),
{
    if let crate::symbol_text::SymbolCodec::Arithmetic(prior) = *before {
        if let crate::symbol_text::SymbolCodec::Arithmetic(next) = *after {
            let decoded = crate::arithmetic::arithmetic_spec::integer_state(
                prior.decoder, prior.integers.slots[fields(CycleMode::ExportRuns).0 as int]@,
            );
            assert(decoded.0 == Ok(crate::arithmetic::ArithmeticInteger::Value(0)));
        }
    }
}

pub(in crate::symbol_text) proof fn height_zero_step<'a>(
    before: &crate::symbol_text::SymbolCodec<'a>,
    after_height: &crate::symbol_text::SymbolCodec<'a>,
    after_width: &crate::symbol_text::SymbolCodec<'a>, cursor: (u32, u32, bool),
)
    requires
        codec_valid(before), codec_valid(after_height),
        source_state(before, CycleMode::EmptyHeightClasses, cursor).is_some(),
        arithmetic_integer_transition(before, after_height, crate::symbol_text::IntegerField::HeightDelta,
            Ok(crate::symbol_text::EntropyInteger::Value(0))),
        arithmetic_integer_transition(after_height, after_width, crate::symbol_text::IntegerField::WidthDelta,
            Ok(crate::symbol_text::EntropyInteger::OutOfBand)),
    ensures
        source_state(after_width, CycleMode::EmptyHeightClasses, cursor).is_some(),
        step(CycleMode::EmptyHeightClasses,
            source_state(before, CycleMode::EmptyHeightClasses, cursor).unwrap())
            == source_state(after_width, CycleMode::EmptyHeightClasses, cursor),
{
    if let crate::symbol_text::SymbolCodec::Arithmetic(prior) = *before {
        if let crate::symbol_text::SymbolCodec::Arithmetic(middle) = *after_height {
            if let crate::symbol_text::SymbolCodec::Arithmetic(next) = *after_width {
                let height = crate::arithmetic::arithmetic_spec::integer_state(
                    prior.decoder, prior.integers.slots[fields(CycleMode::EmptyHeightClasses).0 as int]@,
                );
                assert(height.0 == Ok(crate::arithmetic::ArithmeticInteger::Value(0)));
                assert(middle.integers.slots[fields(CycleMode::EmptyHeightClasses).1 as int]@
                    == prior.integers.slots[fields(CycleMode::EmptyHeightClasses).1 as int]@);
                let width = crate::arithmetic::arithmetic_spec::integer_state(
                    middle.decoder, middle.integers.slots[fields(CycleMode::EmptyHeightClasses).1 as int]@,
                );
                assert(width.0 == Ok(crate::arithmetic::ArithmeticInteger::OutOfBand));
                assert(next.integers.slots[fields(CycleMode::EmptyHeightClasses).0 as int]@
                    == middle.integers.slots[fields(CycleMode::EmptyHeightClasses).0 as int]@);
            }
        }
    }
}

// Model navigation: spec::zero_run [project-result-model]
// Purpose: Any finite continuation of the same deterministic zero-progress procedure.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn zero_run<'a>(
    mode: CycleMode, value: CycleState<'a>, count: nat,
) -> Option<CycleState<'a>>
    decreases count,
{
    if count == 0 { Some(value) }
    else {
        match step(mode, value) {
            Some(next) => zero_run(mode, next, (count - 1) as nat),
            None => None,
        }
    }
}

pub(in crate::symbol_text) proof fn closed_cycle_never_progresses<'a>(
    mode: CycleMode, values: Seq<CycleState<'a>>, first: int, last: int,
    node: int, count: nat,
)
    requires
        chain(mode, values), 0 <= first < last < values.len(),
        values[first] == values[last], first <= node < last,
    ensures
        zero_run(mode, values[node], count).is_some(),
        zero_run(mode, values[node], count).unwrap().3.0 == values[node].3.0,
        zero_run(mode, values[node], count).unwrap().3.1 == values[node].3.1,
    decreases count,
{
    if count > 0 {
        let next = if node + 1 == last { first } else { node + 1 };
        assert(step(mode, values[node]) == Some(values[next]));
        assert(values[next].3.0 == values[node].3.0);
        assert(values[next].3.1 == values[node].3.1);
        closed_cycle_never_progresses(mode, values, first, last, next, (count - 1) as nat);
    }
}

pub(in crate::symbol_text) proof fn repetition_never_progresses<'a>(
    mode: CycleMode, values: Seq<CycleState<'a>>,
)
    requires repetition(mode, values),
    ensures forall|count: nat|
        zero_run(mode, values.last(), count).is_some()
            && zero_run(mode, values.last(), count).unwrap().3.0 == values.last().3.0
            && zero_run(mode, values.last(), count).unwrap().3.1 == values.last().3.1,
{
    let first = choose|first: int| 0 <= first < values.len() - 1 && values[first] == values.last();
    assert forall|count: nat|
        zero_run(mode, values.last(), count).is_some()
            && zero_run(mode, values.last(), count).unwrap().3.0 == values.last().3.0
            && zero_run(mode, values.last(), count).unwrap().3.1 == values.last().3.1 by {
        closed_cycle_never_progresses(mode, values, first, values.len() - 1, first, count);
    }
}

// Model navigation: spec::work_exhaustion_error [project-policy]
// Purpose: Existing Work overshoot or integer-overflow boundary after accelerated work.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::symbol_text) open spec fn work_exhaustion_error(limits: &crate::Limits)
    -> crate::symbol_text::SymbolTextError {
    if limits.max_work_units == u64::MAX {
        crate::symbol_text::SymbolTextError::Decode(crate::DecodeError::Malformed(
            crate::MalformedError::IntegerOverflow,
        ))
    } else {
        crate::symbol_text::SymbolTextError::Decode(crate::DecodeError::LimitExceeded(
            crate::LimitExceeded { kind: crate::LimitKind::Work, limit: limits.max_work_units,
                actual: (limits.max_work_units + 1) as u64 },
        ))
    }
}

}
