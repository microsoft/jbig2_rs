//! Numeric uncertainty, not marker recognition or instance-count evidence.

use super::super::arithmetic_spec;
use super::super::{ArithmeticContext, ArithmeticDecoder, MIN_INTERVAL, QE_STATES};
use super::PrefixCertainty;
use vstd::prelude::*;

verus! {

pub(crate) struct Step<'a> {
    pub(crate) before: ArithmeticDecoder<'a>,
    pub(crate) context: ArithmeticContext,
    pub(crate) after: ArithmeticDecoder<'a>,
    pub(crate) context_after: ArithmeticContext,
    pub(crate) bit: u8,
    pub(crate) width_before: u32,
    pub(crate) width_after: u32,
}

pub(crate) struct Snapshot<'a> {
    pub(crate) decoder: ArithmeticDecoder<'a>,
    pub(crate) spread: u32,
    pub(crate) history: Seq<Step<'a>>,
    pub(crate) instances: u32,
}

// Model navigation: spec::saved_valid [representation-invariant]
// Purpose: A saved certificate belongs to the actual initialized immutable trace.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn saved_valid(o: &PrefixCertainty) -> bool {
    match o.saved@ {
        Some(s) => trace_valid(o.initial@, s.history, &s.decoder, s.spread, o.entropy_end)
            && s.decoder.bytes == o.bytes && s.decoder.bytes@ == o.bytes@,
        None => true,
    }
}

// Model navigation: spec::saved_prefix [representation-invariant]
// Purpose: Bind saved decision provenance to the completed instance count.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn saved_prefix(o: &PrefixCertainty, instances: u32) -> bool {
    o.saved@.is_some() && o.saved@.unwrap().instances == instances && saved_valid(o)
}

// Model navigation: spec::after_decision [project-result-model]
// Purpose: Project the real arithmetic decision register transition.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn after_decision<'a>(d: &ArithmeticDecoder<'a>, c: ArithmeticContext)
    -> ArithmeticDecoder<'a> {
    let r = arithmetic_spec::decision_state(d, c);
    ArithmeticDecoder {
        bytes: d.bytes, byte_position: r.2, current_byte: r.3,
        code: r.4, interval: r.1, bit_count: r.5, marker: r.6,
    }
}

// Model navigation: spec::checked_decision [project-result-model]
// Purpose: Preserve the real decision value or its ordinary arithmetic error.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn checked_decision(d: &ArithmeticDecoder, c: ArithmeticContext)
    -> Result<u8, super::super::ArithmeticError> {
    let r = arithmetic_spec::decision_state(d, c);
    if r.0 <= 1 { Ok(r.0) }
    else { Err(arithmetic_spec::error_for_signal(&after_decision(d, c), r.0)) }
}

// Model navigation: spec::initialized [representation-invariant]
// Purpose: Require genuine initialization from the immutable coding bytes.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn initialized(d: &ArithmeticDecoder) -> bool {
    arithmetic_spec::decoder_valid(d) && d.bytes@.len() >= 2
    && d.interval == MIN_INTERVAL
    && d.byte_position == arithmetic_spec::initialized_state(d.bytes).unwrap().0
    && d.current_byte == arithmetic_spec::initialized_state(d.bytes).unwrap().1
    && d.code == arithmetic_spec::initialized_state(d.bytes).unwrap().2
    && d.bit_count == arithmetic_spec::initialized_state(d.bytes).unwrap().3
    && d.marker == arithmetic_spec::initialized_state(d.bytes).unwrap().4
}

// Model navigation: spec::step_valid [representation-invariant]
// Purpose: Bind output, context and uncertainty to one actually executed decision.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn step_valid(s: Step, end: usize) -> bool {
    arithmetic_spec::decoder_valid(&s.before)
    && arithmetic_spec::decoder_valid(&s.after)
    && arithmetic_spec::context_valid(s.context)
    && s.after.bytes == s.before.bytes && s.after.bytes@ == s.before.bytes@
    && s.after == after_decision(&s.before, s.context)
    && checked_decision(&s.before, s.context) == Ok(s.bit)
    && s.context_after.state_index == arithmetic_spec::entry_index(
        s.before.interval, s.before.code, s.context.state_index)
    && s.context_after.mps == arithmetic_spec::entry_mps(
        s.before.interval, s.before.code, s.context.state_index, s.context.mps)
    && decision_spread(&s.before, s.context, s.width_before, end) == Some(s.width_after)
}

pub(in crate::arithmetic) proof fn certify_step(s: Step, end: usize)
    requires
        arithmetic_spec::decoder_valid(&s.before), arithmetic_spec::decoder_valid(&s.after),
        arithmetic_spec::context_valid(s.context),
        s.after.bytes == s.before.bytes, s.after.bytes@ == s.before.bytes@,
        s.after == after_decision(&s.before, s.context),
        checked_decision(&s.before, s.context) == Ok(s.bit),
        s.context_after.state_index == arithmetic_spec::entry_index(
            s.before.interval, s.before.code, s.context.state_index),
        s.context_after.mps == arithmetic_spec::entry_mps(
            s.before.interval, s.before.code, s.context.state_index, s.context.mps),
        decision_spread(&s.before, s.context, s.width_before, end) == Some(s.width_after),
    ensures step_valid(s, end),
{
    reveal(step_valid);
}

// Model navigation: spec::trace_valid [representation-invariant]
// Purpose: Chain genuine initialized steps and their numeric enclosure widths.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn trace_valid(initial: ArithmeticDecoder, history: Seq<Step>,
    current: &ArithmeticDecoder, w: u32, end: usize) -> bool {
    initialized(&initial) && initial.bytes@.len() <= usize::MAX
    && initial.bytes == current.bytes && initial.bytes@ == current.bytes@
    && end + 2 == initial.bytes@.len()
    && (forall|i: int| 0 <= i < history.len() ==> (
        #[trigger] step_valid(history[i], end)
        && history[i].before == if i == 0 { initial } else { history[i-1].after }
        && history[i].width_before == if i == 0 { initial_spread(&initial) } else { history[i-1].width_after }
        && history[i].before.bytes == initial.bytes && history[i].before.bytes@ == initial.bytes@
    ))
    && *current == if history.len() == 0 { initial } else { history.last().after }
    && w == if history.len() == 0 { initial_spread(&initial) } else { history.last().width_after }
}

// Model navigation: spec::observer_valid [representation-invariant]
// Purpose: Relate observer storage and history to the unchanged actual decoder.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn observer_valid(o: &PrefixCertainty, d: &ArithmeticDecoder) -> bool {
    o.bytes == d.bytes && o.bytes@ == d.bytes@
    && o.entropy_end + 2 == o.bytes@.len()
    && saved_valid(o)
    && (o.active ==> trace_valid(o.initial@, o.history@, d, o.spread, o.entropy_end))
}

pub(crate) proof fn observer_start(o: &PrefixCertainty, d: &ArithmeticDecoder)
    requires initialized(d), d.bytes@.len() <= usize::MAX,
        o.active, o.bytes == d.bytes, o.bytes@ == d.bytes@,
        o.initial@ == *d, o.history@.len() == 0,
        o.entropy_end + 2 == d.bytes@.len(), o.spread == initial_spread(d),
        o.saved@.is_none(),
    ensures observer_valid(o, d),
{
    reveal(trace_valid);
    reveal(saved_valid);
}

pub(crate) proof fn observer_save(prior: &PrefixCertainty, o: &PrefixCertainty,
    d: &ArithmeticDecoder, instances: u32)
    requires prior.active, observer_valid(prior, d),
        o.bytes == prior.bytes, o.bytes@ == prior.bytes@,
        o.entropy_end == prior.entropy_end, o.spread == prior.spread,
        o.active == prior.active, o.history == prior.history, o.initial == prior.initial,
        o.saved@ == Some(Snapshot { decoder: *d, spread: prior.spread,
            history: prior.history@, instances }),
    ensures observer_valid(o, d), saved_prefix(o, instances),
{
    reveal(saved_valid);
}

pub(crate) proof fn observer_inactive(prior: &PrefixCertainty, o: &PrefixCertainty,
    before: &ArithmeticDecoder, after: &ArithmeticDecoder)
    requires observer_valid(prior, before), !o.active,
        o.bytes == prior.bytes, o.bytes@ == prior.bytes@,
        o.initial == prior.initial, o.entropy_end == prior.entropy_end,
        o.saved == prior.saved,
        after.bytes == before.bytes, after.bytes@ == before.bytes@,
    ensures observer_valid(o, after),
{
    reveal(saved_valid);
}

pub(in crate::arithmetic) proof fn observer_append(prior: &PrefixCertainty, o: &PrefixCertainty,
    d: &ArithmeticDecoder)
    requires prior.active, o.active,
        o.bytes == prior.bytes, o.bytes@ == prior.bytes@,
        o.initial == prior.initial, o.entropy_end == prior.entropy_end,
        o.saved == prior.saved,
        o.history@.len() == prior.history@.len() + 1,
        o.history@.drop_last() == prior.history@,
        observer_valid(prior, &o.history@.last().before),
        step_valid(o.history@.last(), o.entropy_end),
        o.history@.last().width_before == prior.spread,
        o.history@.last().width_after == o.spread,
        o.history@.last().after == *d,
    ensures observer_valid(o, d),
{
    reveal(trace_valid);
    assert forall|i: int| 0 <= i < o.history@.len() implies (
        #[trigger] step_valid(o.history@[i], o.entropy_end)
        && o.history@[i].before == if i == 0 { o.initial@ } else { o.history@[i-1].after }
        && o.history@[i].width_before == if i == 0 { initial_spread(&o.initial@) } else { o.history@[i-1].width_after }
        && o.history@[i].before.bytes == o.initial@.bytes
        && o.history@[i].before.bytes@ == o.initial@.bytes@
    ) by {
        if i < prior.history@.len() {
            assert(o.history@[i] == prior.history@[i]);
            assert(step_valid(prior.history@[i], prior.entropy_end));
        } else {
            assert(i == o.history@.len() - 1);
            assert(o.history@[i] == o.history@.last());
            assert(trace_valid(prior.initial@, prior.history@,
                &o.history@.last().before, prior.spread, prior.entropy_end));
            if i > 0 {
                assert(o.history@[i-1] == prior.history@.last());
            }
        }
    }
}

// Model navigation: spec::initial_spread [project-result-model]
// Purpose: Include terminal-prefix and BYTEIN uncertainty during initialization.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn initial_spread(d: &ArithmeticDecoder) -> u32
    recommends d.bytes@.len() >= 2,
{
    let end = d.bytes@.len() - 2;
    if end == 0 { d.code }
    else if d.marker.is_some() { 8355840u32 }
    else if d.byte_position >= end {
        let increment = if d.bytes@[0] == 0xff {
            (d.current_byte as u32) << 9
        } else {
            (d.current_byte as u32) << 8
        };
        (increment as nat * 128) as u32
    } else { 0 }
}

// Model navigation: spec::normalization_spread [project-result-model]
// Purpose: Bound variable numeric source contributions before each doubling.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn normalization_spread(d: &ArithmeticDecoder, w: u32, end: usize)
    -> Option<u32>
    recommends arithmetic_spec::decoder_state_valid(d),
    decreases if 0 < d.interval < MIN_INTERVAL { MIN_INTERVAL as int - d.interval as int } else { 0 },
{
    if d.interval == 0 || w > d.code { None }
    else if d.interval >= MIN_INTERVAL { Some(w) }
    else {
        let refill = if d.bit_count == 0 { arithmetic_spec::refill_step(d) }
        else { (0u8, d.byte_position, d.current_byte, d.code, d.bit_count, d.marker) };
        let unknown = d.bit_count == 0 && (refill.5.is_some() || refill.1 >= end);
        let added = if unknown { refill.3 as int - d.code as int } else { 0 };
        let next_width = (w as int + added) * 2;
        if refill.0 != 0 || refill.3 as nat * 2 > u32::MAX
            || next_width > u32::MAX || next_width < 0 { None }
        else {
            normalization_spread(&ArithmeticDecoder {
                bytes: d.bytes, byte_position: refill.1, current_byte: refill.2,
                code: (refill.3 as nat * 2) as u32,
                interval: (d.interval as nat * 2) as u32,
                bit_count: (refill.4 as int - 1) as u8, marker: refill.5,
            }, next_width as u32, end)
        }
    }
}

// Model navigation: spec::decision_frame [project-result-model]
// Purpose: Project the fixed-control register frame before renormalization.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn decision_frame<'a>(d: &ArithmeticDecoder<'a>, context: ArithmeticContext)
    -> ArithmeticDecoder<'a>
    recommends arithmetic_spec::decoder_valid(d), arithmetic_spec::context_valid(context),
{
    let qe = QE_STATES@[context.state_index as int].qe as u32;
    let lower = d.code < (qe << 16);
    ArithmeticDecoder {
        bytes: d.bytes, byte_position: d.byte_position, current_byte: d.current_byte,
        code: if lower { d.code } else { (d.code as int - (qe << 16) as int) as u32 },
        interval: if lower { qe } else { (d.interval as int - qe as int) as u32 },
        bit_count: d.bit_count, marker: d.marker,
    }
}

// Model navigation: spec::decision_spread [project-result-model]
// Purpose: Reject any enclosure crossing the arithmetic decision threshold.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn decision_spread(d: &ArithmeticDecoder, context: ArithmeticContext,
    w: u32, end: usize) -> Option<u32>
    recommends arithmetic_spec::decoder_valid(d), arithmetic_spec::context_valid(context),
{
    let qe = QE_STATES@[context.state_index as int].qe as u32;
    if w > d.code || (d.code >= (qe << 16) && (d.code as int - w as int) < (qe << 16)) {
        None
    } else {
        normalization_spread(&decision_frame(d, context), w, end)
    }
}

pub(in crate::arithmetic) proof fn initial_spread_bound(d: &ArithmeticDecoder)
    requires
        arithmetic_spec::decoder_valid(d), d.bytes@.len() >= 2,
        d.byte_position == arithmetic_spec::initialized_state(d.bytes).unwrap().0,
        d.current_byte == arithmetic_spec::initialized_state(d.bytes).unwrap().1,
        d.code == arithmetic_spec::initialized_state(d.bytes).unwrap().2,
        d.marker == arithmetic_spec::initialized_state(d.bytes).unwrap().4,
    ensures initial_spread(d) <= d.code,
{
    let first = d.bytes@[0];
    let next = d.bytes@[1];
    arithmetic_spec::initialization_code_bound(first, next);
    let seed = ArithmeticDecoder {
        bytes: d.bytes, byte_position: 0, current_byte: first,
        code: (first as u32) << 16, interval: MIN_INTERVAL,
        bit_count: 0, marker: None,
    };
    let refill = arithmetic_spec::refill_step(&seed);
    assert(refill.3 <= u32::MAX / 128);
    assert(d.code == refill.3 as nat * 128);
    if d.bytes@.len() > 2 && d.marker.is_none() && d.byte_position >= d.bytes@.len() - 2 {
        let contribution = if first == 0xff { (next as u32) << 9 } else { (next as u32) << 8 };
        assert(contribution <= refill.3);
        assert(initial_spread(d) == contribution as nat * 128);
    }
}

/// BYTEIN's control frame is immutable. Only the numeric contribution varies.
// Model navigation: spec::refill_contribution [project-result-model]
// Purpose: Project the actual BYTEIN numeric contribution without producing bits.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn refill_contribution(d: &ArithmeticDecoder) -> nat {
    (arithmetic_spec::refill_step(d).3 as int - d.code as int) as nat
}

// Model navigation: spec::source_unknown [project-policy]
// Purpose: Vary both first terminal-prefix and repeated marker-fill contributions.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn source_unknown(d: &ArithmeticDecoder, end: usize) -> bool {
    let refill = arithmetic_spec::refill_step(d);
    refill.5.is_some() || refill.1 >= end
}

// Model navigation: spec::normalized_next [project-result-model]
// Purpose: Preserve immutable bytes and the real normalization control schedule.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn normalized_next<'a>(d: &ArithmeticDecoder<'a>)
    -> ArithmeticDecoder<'a>
{
    let r = if d.bit_count == 0 { arithmetic_spec::refill_step(d) }
        else { (0u8, d.byte_position, d.current_byte, d.code, d.bit_count, d.marker) };
    ArithmeticDecoder {
        bytes: d.bytes, byte_position: r.1, current_byte: r.2,
        code: (r.3 as nat * 2) as u32, interval: (d.interval as nat * 2) as u32,
        bit_count: (r.4 as int - 1) as u8, marker: r.5,
    }
}

// Model navigation: spec::admissible_sources [project-policy]
// Purpose: Keep genuine coded contributions fixed and vary uncertain ones downward.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn admissible_sources(
    d: &ArithmeticDecoder, sources: Seq<nat>, end: usize) -> bool
    decreases if 0 < d.interval < MIN_INTERVAL { MIN_INTERVAL as int - d.interval as int } else { 0 },
{
    if d.interval == 0 { false }
    else if d.interval >= MIN_INTERVAL { true }
    else if d.bit_count == 0 {
        let r = arithmetic_spec::refill_step(d);
        r.0 == 0 && sources.len() > 0
            && sources[0] <= refill_contribution(d)
            && (!source_unknown(d, end) ==> sources[0] == refill_contribution(d))
            && admissible_sources(&normalized_next(d), sources.drop_first(), end)
    } else {
        admissible_sources(&normalized_next(d), sources, end)
    }
}

/// Exact numeric execution with arbitrary admissible fills and the original
/// source/control frames. The interval and CT schedule do not depend on code.
// Model navigation: spec::alternative_normalization [project-result-model]
// Purpose: Construct alternative numeric codes on the immutable control schedule.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn alternative_normalization(
    d: &ArithmeticDecoder, code: nat, sources: Seq<nat>, end: usize) -> nat
    recommends admissible_sources(d, sources, end),
    decreases if 0 < d.interval < MIN_INTERVAL { MIN_INTERVAL as int - d.interval as int } else { 0 },
{
    if d.interval == 0 || d.interval >= MIN_INTERVAL { code }
    else if d.bit_count == 0 {
        alternative_normalization(&normalized_next(d), (code + sources[0]) * 2,
            sources.drop_first(), end)
    } else {
        alternative_normalization(&normalized_next(d), code * 2, sources, end)
    }
}

pub(in crate::arithmetic) proof fn normalization_enclosure(
    d: &ArithmeticDecoder, w: u32, end: usize, code: nat, sources: Seq<nat>)
    requires
        arithmetic_spec::decoder_state_valid(d),
        d.bytes@.len() <= usize::MAX,
        normalization_spread(d, w, end).is_some(),
        d.code as int - w as int <= code <= d.code,
        admissible_sources(d, sources, end),
    ensures
        arithmetic_spec::normalized_state(d).0 == 0,
        normalization_spread(d, w, end).unwrap() <= arithmetic_spec::normalized_state(d).4,
        arithmetic_spec::normalized_state(d).4 as int
            - normalization_spread(d, w, end).unwrap() as int
            <= alternative_normalization(d, code, sources, end)
            <= arithmetic_spec::normalized_state(d).4,
    decreases if 0 < d.interval < MIN_INTERVAL { MIN_INTERVAL as int - d.interval as int } else { 0 },
{
    reveal(arithmetic_spec::decoder_state_valid);
    reveal(arithmetic_spec::decoder_position_valid);
    reveal_with_fuel(normalization_spread, 1);
    reveal_with_fuel(arithmetic_spec::normalized_state, 1);
    if d.interval < MIN_INTERVAL {
        let r = if d.bit_count == 0 { arithmetic_spec::refill_step(d) }
            else { (0u8, d.byte_position, d.current_byte, d.code, d.bit_count, d.marker) };
        let unknown = d.bit_count == 0 && (r.5.is_some() || r.1 >= end);
        let added = if unknown { r.3 as int - d.code as int } else { 0 };
        let next_width = ((w as int + added) * 2) as u32;
        let next = normalized_next(d);
        assert(arithmetic_spec::decoder_position_valid(&next));
        assert(0 < next.interval <= 0xffff);
        assert(next.bit_count <= 8);
        assert(match next.marker {
            Some(marker) =>
                marker.offset == next.byte_position
                && marker.offset + 1 < next.bytes@.len()
                && next.bytes@[marker.offset as int] == 0xff
                && next.bytes@[(marker.offset + 1) as int] == marker.code
                && marker.code > 0x8f,
            None => true,
        });
        assert(arithmetic_spec::decoder_state_valid(&next));
        if d.bit_count == 0 {
            assert(r.3 as int == d.code as int + refill_contribution(d));
            normalization_enclosure(&next, next_width, end,
                (code + sources[0]) * 2, sources.drop_first());
        } else {
            normalization_enclosure(&next, next_width, end, code * 2, sources);
        }
    }
}

pub(in crate::arithmetic) proof fn decision_independent(
    d: &ArithmeticDecoder, context: ArithmeticContext, w: u32, end: usize, code: u32)
    requires
        arithmetic_spec::decoder_valid(d), arithmetic_spec::context_valid(context),
        decision_spread(d, context, w, end).is_some(),
        d.code as int - w as int <= code <= d.code,
    ensures
        arithmetic_spec::entry_decision(d.interval, code, context.state_index, context.mps)
            == arithmetic_spec::entry_decision(d.interval, d.code, context.state_index, context.mps),
        arithmetic_spec::entry_index(d.interval, code, context.state_index)
            == arithmetic_spec::entry_index(d.interval, d.code, context.state_index),
        arithmetic_spec::entry_mps(d.interval, code, context.state_index, context.mps)
            == arithmetic_spec::entry_mps(d.interval, d.code, context.state_index, context.mps),
{
    let qe = QE_STATES@[context.state_index as int].qe as u32;
    super::super::arithmetic_shared::qe_transition_valid(context.state_index as int);
    assert((code >> 16 < qe) == (code < (qe << 16))) by(bit_vector)
        requires qe <= 0xffff;
    let upper = d.code;
    assert((upper >> 16 < qe) == (upper < (qe << 16))) by(bit_vector)
        requires qe <= 0xffff;
    arithmetic_spec::entry_model_equations(d.interval, code, context.state_index, context.mps);
    arithmetic_spec::entry_model_equations(d.interval, d.code, context.state_index, context.mps);
}

// Model navigation: spec::admissible_initial [project-policy]
// Purpose: Bound initialization sources without assuming decision agreement.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn admissible_initial(d: &ArithmeticDecoder,
    first: nat, refill: nat, end: usize) -> bool {
    let seed = ArithmeticDecoder {
        bytes: d.bytes, byte_position: 0, current_byte: d.bytes@[0],
        code: (d.bytes@[0] as u32) << 16, interval: MIN_INTERVAL,
        bit_count: 0, marker: None,
    };
    first <= seed.code && (end > 0 ==> first == seed.code)
    && refill <= refill_contribution(&seed)
    && (!source_unknown(&seed, end) ==> refill == refill_contribution(&seed))
}

pub(in crate::arithmetic) proof fn initial_enclosure(d: &ArithmeticDecoder,
    first: nat, refill: nat, end: usize)
    requires initialized(d), end + 2 == d.bytes@.len(),
        admissible_initial(d, first, refill, end),
    ensures d.code as int - initial_spread(d) as int
        <= (first + refill) * 128 <= d.code,
{
    let seed = ArithmeticDecoder {
        bytes: d.bytes, byte_position: 0, current_byte: d.bytes@[0],
        code: (d.bytes@[0] as u32) << 16, interval: MIN_INTERVAL,
        bit_count: 0, marker: None,
    };
    arithmetic_spec::initialization_code_bound(d.bytes@[0], d.bytes@[1]);
    assert(arithmetic_spec::refill_step(&seed).3 as int
        == seed.code as int + refill_contribution(&seed));
    assert(d.code == (seed.code as nat + refill_contribution(&seed)) * 128);
    if end > 0 {
        if source_unknown(&seed, end) {
            assert(initial_spread(d) == refill_contribution(&seed) * 128);
        } else {
            assert(initial_spread(d) == 0);
        }
    }
}

// Model navigation: spec::alternative_step [project-result-model]
// Purpose: Propagate an alternative numeric code through one fixed-control step.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(in crate::arithmetic) open spec fn alternative_step(s: Step, code: nat,
    sources: Seq<nat>, end: usize) -> nat {
    let qe = QE_STATES@[s.context.state_index as int].qe as u32;
    let residual = if code < (qe << 16) { code } else { (code - (qe << 16)) as nat };
    alternative_normalization(&decision_frame(&s.before, s.context), residual, sources, end)
}

pub(in crate::arithmetic) proof fn step_enclosure(s: Step, end: usize,
    code: u32, sources: Seq<nat>)
    requires
        step_valid(s, end), s.before.bytes@.len() <= usize::MAX,
        s.before.code as int - s.width_before as int <= code <= s.before.code,
        admissible_sources(&decision_frame(&s.before, s.context), sources, end),
    ensures
        s.after.code as int - s.width_after as int
            <= alternative_step(s, code as nat, sources, end) <= s.after.code,
        arithmetic_spec::entry_decision(s.before.interval, code,
            s.context.state_index, s.context.mps) == s.bit,
        arithmetic_spec::entry_index(s.before.interval, code,
            s.context.state_index) == s.context_after.state_index,
        arithmetic_spec::entry_mps(s.before.interval, code,
            s.context.state_index, s.context.mps) == s.context_after.mps,
{
    reveal(step_valid);
    let frame = decision_frame(&s.before, s.context);
    let qe = QE_STATES@[s.context.state_index as int].qe as u32;
    super::super::arithmetic_shared::qe_transition_valid(s.context.state_index as int);
    let upper = s.before.code;
    assert((upper >> 16 < qe) == (upper < (qe << 16))) by(bit_vector)
        requires qe <= 0xffff;
    arithmetic_spec::decision_model_equations(&s.before, s.context);
    arithmetic_spec::entry_model_equations(s.before.interval, s.before.code,
        s.context.state_index, s.context.mps);
    decision_independent(&s.before, s.context, s.width_before, end, code);
    assert(arithmetic_spec::decoder_state_valid(&frame));
    let residual = if (code as nat) < (qe << 16) { code as nat }
        else { (code as nat - (qe << 16)) as nat };
    normalization_enclosure(&frame, s.width_before, end, residual, sources);
    assert(arithmetic_spec::normalized_state(&frame).4 == s.after.code);
}

/// An alternative execution chooses *every* permitted numeric fill, including
/// initialization. Its context selections are the recorded, actually executed
/// accesses; equal decisions preserve the integer/IAID selection histories.
// Model navigation: spec::admissible_prefix [project-policy]
// Purpose: Describe all source-bounded alternatives without an agreement premise.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn admissible_prefix(o: &PrefixCertainty, codes: Seq<nat>,
    sources: Seq<Seq<nat>>, first: nat, refill: nat) -> bool {
    codes.len() == o.history@.len() + 1 && sources.len() == o.history@.len()
    && admissible_initial(&o.initial@, first, refill, o.entropy_end)
    && codes[0] == (first + refill) * 128
    && forall|i: int| 0 <= i < o.history@.len() ==> (
        #[trigger] admissible_sources(&decision_frame(&o.history@[i].before,
            o.history@[i].context), sources[i], o.entropy_end)
        && codes[i+1] == alternative_step(o.history@[i], codes[i], sources[i], o.entropy_end)
    )
}

// Model navigation: spec::trace_upper [project-result-model]
// Purpose: Project the actual upper code endpoint at a trace boundary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn trace_upper(o: &PrefixCertainty, k: nat) -> u32 {
    if k == 0 { o.initial@.code } else { o.history@[(k-1) as int].after.code }
}

// Model navigation: spec::trace_width [project-result-model]
// Purpose: Project the enclosure width at a trace boundary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn trace_width(o: &PrefixCertainty, k: nat) -> u32 {
    if k == 0 { initial_spread(&o.initial@) } else { o.history@[(k-1) as int].width_after }
}

pub(crate) proof fn prefix_enclosure(o: &PrefixCertainty, d: &ArithmeticDecoder,
    codes: Seq<nat>, sources: Seq<Seq<nat>>, first: nat, refill: nat, k: nat)
    requires o.active, observer_valid(o, d), k <= o.history@.len(),
        admissible_prefix(o, codes, sources, first, refill),
    ensures
        trace_upper(o, k) as int - trace_width(o, k) as int <= codes[k as int] <= trace_upper(o, k),
    decreases k,
{
    reveal(trace_valid);
    reveal(admissible_prefix);
    if k == 0 {
        initial_enclosure(&o.initial@, first, refill, o.entropy_end);
    } else {
        prefix_enclosure(o, d, codes, sources, first, refill, (k-1) as nat);
        let i = (k-1) as int;
        let s = o.history@[i];
        assert(step_valid(s, o.entropy_end));
        assert(s.before.code == trace_upper(o, (k-1) as nat));
        assert(s.width_before == trace_width(o, (k-1) as nat));
        step_enclosure(s, o.entropy_end, codes[i] as u32, sources[i]);
    }
}

/// Universal prefix certainty for the one initialized, immutable actual trace.
/// The guarantee covers binary decisions *and* both probability-context fields.
pub(crate) proof fn active_trace_sound(o: &PrefixCertainty, d: &ArithmeticDecoder,
    codes: Seq<nat>, sources: Seq<Seq<nat>>, first: nat, refill: nat)
    requires o.active, observer_valid(o, d),
        admissible_prefix(o, codes, sources, first, refill),
    ensures
        forall|i: int| 0 <= i < o.history@.len() ==> (
            arithmetic_spec::entry_decision(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].bit
            && arithmetic_spec::entry_index(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index) == o.history@[i].context_after.state_index
            && arithmetic_spec::entry_mps(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].context_after.mps
        ),
{
    reveal(trace_valid);
    reveal(admissible_prefix);
    assert forall|i: int| 0 <= i < o.history@.len() implies (
        arithmetic_spec::entry_decision(o.history@[i].before.interval, codes[i] as u32,
            o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].bit
        && arithmetic_spec::entry_index(o.history@[i].before.interval, codes[i] as u32,
            o.history@[i].context.state_index) == o.history@[i].context_after.state_index
        && arithmetic_spec::entry_mps(o.history@[i].before.interval, codes[i] as u32,
            o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].context_after.mps
    ) by {
        prefix_enclosure(o, d, codes, sources, first, refill, i as nat);
        let s = o.history@[i];
        assert(step_valid(s, o.entropy_end));
        assert(s.before.code == trace_upper(o, i as nat));
        assert(s.width_before == trace_width(o, i as nat));
        step_enclosure(s, o.entropy_end, codes[i] as u32, sources[i]);
    }
}

// Model navigation: spec::source_choices_admissible [project-policy]
// Purpose: Quantify bounded source choices, not assumed matching decoder outputs.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn source_choices_admissible(o: &PrefixCertainty,
    sources: Seq<Seq<nat>>, first: nat, refill: nat) -> bool {
    sources.len() == o.history@.len()
    && admissible_initial(&o.initial@, first, refill, o.entropy_end)
    && forall|i: int| 0 <= i < o.history@.len() ==>
        #[trigger] admissible_sources(&decision_frame(&o.history@[i].before,
            o.history@[i].context), sources[i], o.entropy_end)
}

// Model navigation: spec::alternative_codes [project-result-model]
// Purpose: Construct the alternative code history recursively from source choices.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn alternative_codes(o: &PrefixCertainty,
    sources: Seq<Seq<nat>>, first: nat, refill: nat, k: nat) -> Seq<nat>
    decreases k,
{
    if k == 0 { seq![(first + refill) * 128] }
    else {
        let prior = alternative_codes(o, sources, first, refill, (k-1) as nat);
        prior.push(alternative_step(o.history@[(k-1) as int], prior.last(),
            sources[(k-1) as int], o.entropy_end))
    }
}

pub(in crate::arithmetic) proof fn alternative_codes_exist(o: &PrefixCertainty,
    sources: Seq<Seq<nat>>, first: nat, refill: nat, k: nat)
    requires k <= o.history@.len(), source_choices_admissible(o, sources, first, refill),
    ensures ({
        let codes = alternative_codes(o, sources, first, refill, k);
        codes.len() == k + 1 && codes[0] == (first + refill) * 128
        && forall|i: int| 0 <= i < k ==>
            codes[i+1] == alternative_step(o.history@[i], codes[i], sources[i], o.entropy_end)
    }),
    decreases k,
{
    reveal_with_fuel(alternative_codes, 1);
    if k > 0 {
        alternative_codes_exist(o, sources, first, refill, (k-1) as nat);
        let codes = alternative_codes(o, sources, first, refill, k);
        assert forall|i: int| 0 <= i < k implies
            codes[i+1] == #[trigger] alternative_step(o.history@[i], codes[i], sources[i], o.entropy_end)
        by {
            if i < k - 1 {
                let prior = alternative_codes(o, sources, first, refill, (k-1) as nat);
                assert(prior[i+1] == alternative_step(o.history@[i], prior[i], sources[i], o.entropy_end));
            }
        }
    }
}

/// No agreement/bounds premise is imposed on alternative code values: they are
/// constructed from any independently chosen admissible numeric contributions.
pub(crate) proof fn arbitrary_fill_sound(o: &PrefixCertainty, d: &ArithmeticDecoder,
    sources: Seq<Seq<nat>>, first: nat, refill: nat)
    requires o.active, observer_valid(o, d),
        source_choices_admissible(o, sources, first, refill),
    ensures ({
        let codes = alternative_codes(o, sources, first, refill, o.history@.len());
        forall|i: int| 0 <= i < o.history@.len() ==>
            arithmetic_spec::entry_decision(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].bit
            && arithmetic_spec::entry_index(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index) == o.history@[i].context_after.state_index
            && arithmetic_spec::entry_mps(o.history@[i].before.interval, codes[i] as u32,
                o.history@[i].context.state_index, o.history@[i].context.mps) == o.history@[i].context_after.mps
    }),
{
    reveal(source_choices_admissible);
    reveal(admissible_prefix);
    alternative_codes_exist(o, sources, first, refill, o.history@.len());
    let codes = alternative_codes(o, sources, first, refill, o.history@.len());
    assert(admissible_prefix(o, codes, sources, first, refill));
    active_trace_sound(o, d, codes, sources, first, refill);
}

// Model navigation: spec::saved_observer [project-result-model]
// Purpose: Recover the saved pre-final-DS trace independently of later decoding.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn saved_observer<'a>(o: &PrefixCertainty<'a>) -> PrefixCertainty<'a> {
    let s = o.saved@.unwrap();
    PrefixCertainty {
        bytes: o.bytes, entropy_end: o.entropy_end,
        active: true, spread: s.spread,
        initial: o.initial, history: Ghost(s.history), saved: Ghost(None),
    }
}

/// The complete placement-prefix snapshot remains sound even when final DS
/// disables the live observer or returns an entropy error.
pub(crate) proof fn saved_arbitrary_fill_sound(o: &PrefixCertainty, count: u32,
    sources: Seq<Seq<nat>>, first: nat, refill: nat)
    requires saved_prefix(o, count),
        source_choices_admissible(&saved_observer(o), sources, first, refill),
    ensures ({
        let snapshot = saved_observer(o);
        let codes = alternative_codes(&snapshot, sources, first, refill, snapshot.history@.len());
        forall|i: int| 0 <= i < snapshot.history@.len() ==>
            arithmetic_spec::entry_decision(snapshot.history@[i].before.interval, codes[i] as u32,
                snapshot.history@[i].context.state_index, snapshot.history@[i].context.mps) == snapshot.history@[i].bit
            && arithmetic_spec::entry_index(snapshot.history@[i].before.interval, codes[i] as u32,
                snapshot.history@[i].context.state_index) == snapshot.history@[i].context_after.state_index
            && arithmetic_spec::entry_mps(snapshot.history@[i].before.interval, codes[i] as u32,
                snapshot.history@[i].context.state_index, snapshot.history@[i].context.mps) == snapshot.history@[i].context_after.mps
    }),
{
    reveal(saved_valid);
    let snapshot = saved_observer(o);
    assert(observer_valid(&snapshot, &o.saved@.unwrap().decoder));
    arbitrary_fill_sound(&snapshot, &o.saved@.unwrap().decoder, sources, first, refill);
}

}
