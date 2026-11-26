//! A one-sided interval observer for the genuinely executed Annex E decisions.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{ArithmeticContext, ArithmeticDecoder, ArithmeticError, MIN_INTERVAL, QE_STATES};

#[cfg(verus_keep_ghost)]
#[path = "../../specs/arithmetic_certainty.rs"]
pub(crate) mod certainty_spec;

/// The actual code is the upper endpoint; `spread` is its possible reduction.
/// Marker bytes remain immutable control input, but their numeric contributions
/// (including the first marker prefix) are allowed to range down to zero.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct PrefixCertainty<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) spread: u32,
    pub(crate) active: bool,
    pub(crate) entropy_end: usize,
    #[cfg(verus_keep_ghost)]
    #[verifier::spec]
    pub(crate) initial: Ghost<ArithmeticDecoder<'a>>,
    #[cfg(verus_keep_ghost)]
    #[verifier::spec]
    pub(crate) history: Ghost<Seq<certainty_spec::Step<'a>>>,
    #[cfg(verus_keep_ghost)]
    #[verifier::spec]
    pub(crate) saved: Ghost<Option<certainty_spec::Snapshot<'a>>>,
}

impl<'a> PrefixCertainty<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(decoder),
            decoder.bytes@.len() >= 2,
            decoder.interval == MIN_INTERVAL,
            decoder.byte_position == arithmetic_spec::initialized_state(decoder.bytes).unwrap().0,
            decoder.current_byte == arithmetic_spec::initialized_state(decoder.bytes).unwrap().1,
            decoder.code == arithmetic_spec::initialized_state(decoder.bytes).unwrap().2,
            decoder.bit_count == arithmetic_spec::initialized_state(decoder.bytes).unwrap().3,
            decoder.marker == arithmetic_spec::initialized_state(decoder.bytes).unwrap().4,
        ensures
            out.active,
            out.entropy_end == decoder.bytes@.len() - 2,
            out.spread == certainty_spec::initial_spread(decoder),
            out.spread <= decoder.code,
            certainty_spec::observer_valid(&out, decoder),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(decoder: &ArithmeticDecoder<'a>) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! {
            certainty_spec::initial_spread_bound(decoder);
        }
        let entropy_end = decoder.bytes.len() - 2;
        let spread = if entropy_end == 0 {
            decoder.code
        } else if decoder.marker.is_some() {
            0xff00 * 128
        } else if decoder.byte_position >= entropy_end {
            let next = u32::from(decoder.current_byte);
            let increment = if decoder.bytes[0] == 0xff {
                next << 9
            } else {
                next << 8
            };
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(increment <= 0x1fe00u32) by(bit_vector)
                    requires next <= 255,
                        increment == next << 9 || increment == next << 8;
            }
            increment * 128
        } else {
            0
        };
        let observer = Self {
            bytes: decoder.bytes,
            spread,
            active: true,
            entropy_end,
            #[cfg(verus_keep_ghost)]
            initial: verus_exec_expr!(Ghost(*decoder)),
            #[cfg(verus_keep_ghost)]
            history: verus_exec_expr!(Ghost(Seq::empty())),
            #[cfg(verus_keep_ghost)]
            saved: verus_exec_expr!(Ghost(None)),
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            certainty_spec::observer_start(&observer, decoder);
        }
        observer
    }

    /// Call only for a real, successful decision, with its *pre-decode* frame.
    /// This observer never produces entropy bits or changes the decoder.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires
            arithmetic_spec::decoder_valid(before),
            arithmetic_spec::context_valid(context),
            old(self).entropy_end + 2 == before.bytes@.len(),
        ensures
            final(self).entropy_end == old(self).entropy_end,
            final(self).bytes == old(self).bytes,
            final(self).bytes@ == old(self).bytes@,
            final(self).initial == old(self).initial,
            final(self).history == old(self).history,
            final(self).saved == old(self).saved,
            !old(self).active ==> !final(self).active,
            old(self).active ==> (
                final(self).active
                    == certainty_spec::decision_spread(before, context, old(self).spread,
                        old(self).entropy_end).is_some()
                && (final(self).active ==> final(self).spread
                    == certainty_spec::decision_spread(before, context, old(self).spread,
                        old(self).entropy_end).unwrap())
            ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(in crate::arithmetic) fn observe(
        &mut self,
        before: &ArithmeticDecoder<'_>,
        context: ArithmeticContext,
    ) {
        #[cfg(verus_keep_ghost)]
        production_reveal_decoder_valid!();
        #[cfg(verus_keep_ghost)]
        proof! {
            super::arithmetic_shared::packed_context_bounds(context);
            reveal(certainty_spec::decision_spread);
            reveal(certainty_spec::decision_frame);
            super::arithmetic_shared::qe_transition_valid(
                arithmetic_spec::context_index(context) as int,
            );
        }
        if !self.active {
            return;
        }
        let qe = u32::from(QE_STATES[usize::from(context.state_index())].qe);
        let threshold = qe << 16;
        if self.spread > before.code
            || (before.code >= threshold && before.code - self.spread < threshold)
        {
            self.active = false;
            return;
        }
        let lower = before.code < threshold;
        let mut frame = ArithmeticDecoder {
            bytes: before.bytes,
            byte_position: before.byte_position,
            current_byte: before.current_byte,
            code: if lower {
                before.code
            } else {
                before.code - threshold
            },
            interval: if lower { qe } else { before.interval - qe },
            bit_count: before.bit_count,
            marker: before.marker,
        };
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                arithmetic_spec::decoder_state_valid(&frame),
                frame.bytes == before.bytes,
                frame.bytes@ == before.bytes@,
                self.active,
                self.entropy_end == old(self).entropy_end,
                self.spread <= frame.code,
                certainty_spec::normalization_spread(&frame, self.spread, self.entropy_end)
                    == certainty_spec::decision_spread(before, context, old(self).spread,
                        old(self).entropy_end),
            decreases arithmetic_spec::renormalization_steps(frame.interval as nat),
        ))]
        while frame.interval < MIN_INTERVAL {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost previous_frame = frame;
                let ghost previous_spread = self.spread;
            }
            if frame.bit_count == 0 {
                let old_code = frame.code;
                if frame.byte_in_signal() != 0 {
                    self.active = false;
                    return;
                }
                if frame.marker.is_some() || frame.byte_position >= self.entropy_end {
                    let Some(spread) = self.spread.checked_add(frame.code - old_code) else {
                        self.active = false;
                        return;
                    };
                    self.spread = spread;
                }
            }
            frame.interval *= 2;
            let Some(code) = frame.code.checked_mul(2) else {
                self.active = false;
                return;
            };
            frame.code = code;
            let Some(spread) = self.spread.checked_mul(2) else {
                self.active = false;
                return;
            };
            self.spread = spread;
            frame.bit_count -= 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(certainty_spec::normalization_spread, 1);
                reveal(arithmetic_spec::renormalization_steps);
            }
        }
    }

    /// Observes the existing decoder, never a substitute decoder.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
            requires
                arithmetic_spec::decoder_valid(old(decoder)),
                arithmetic_spec::context_valid(*old(context)),
                certainty_spec::observer_valid(old(self), old(decoder)),
            ensures
                arithmetic_spec::decoder_state_valid(final(decoder)),
                arithmetic_spec::context_valid(*final(context)),
                final(decoder).bytes == old(decoder).bytes,
                final(decoder).bytes@ == old(decoder).bytes@,
                *final(decoder) == certainty_spec::after_decision(old(decoder), *old(context)),
                arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
                    old(decoder).interval,
                    old(decoder).code,
                    arithmetic_spec::context_index(*old(context)),
                ),
                arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
                    old(decoder).interval,
                    old(decoder).code,
                    arithmetic_spec::context_index(*old(context)),
                    arithmetic_spec::context_mps(*old(context)),
                ),
                out == certainty_spec::checked_decision(old(decoder), *old(context)),
                out.is_ok() ==> arithmetic_spec::decoder_valid(final(decoder)),
                certainty_spec::observer_valid(final(self), final(decoder)),
                !old(self).active ==> !final(self).active,
                final(self).saved == old(self).saved,
        ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(in crate::arithmetic) fn decode(
        &mut self,
        decoder: &mut ArithmeticDecoder<'a>,
        context: &mut ArithmeticContext,
    ) -> Result<u8, ArithmeticError> {
        let before = ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: decoder.byte_position,
            current_byte: decoder.current_byte,
            code: decoder.code,
            interval: decoder.interval,
            bit_count: decoder.bit_count,
            marker: decoder.marker,
        };
        let before_context = *context;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost prior = *self; }
        let result = decoder.decode(context);
        if let Ok(_bit) = result {
            self.observe(&before, before_context);
            if self.active {
                #[cfg(verus_keep_ghost)]
                proof! {
                    self.history = Ghost(prior.history@.push(certainty_spec::Step {
                        before, context: before_context, after: *decoder,
                        context_after: *context, bit: _bit,
                        width_before: prior.spread, width_after: self.spread,
                    }));
                    vstd::assert_seqs_equal!(self.history@.drop_last(), prior.history@);
                    certainty_spec::certify_step(self.history@.last(), self.entropy_end);
                    certainty_spec::observer_append(&prior, self, decoder);
                }
            }
        } else {
            self.active = false;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            if !self.active {
                certainty_spec::observer_inactive(&prior, self, &before, decoder);
            }
        }
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires certainty_spec::observer_valid(old(self), decoder),
        ensures
            out == old(self).active,
            certainty_spec::observer_valid(final(self), decoder),
            final(self).bytes == old(self).bytes,
            final(self).bytes@ == old(self).bytes@,
            final(self).entropy_end == old(self).entropy_end,
            final(self).spread == old(self).spread,
            final(self).active == old(self).active,
            final(self).history == old(self).history,
            final(self).initial == old(self).initial,
            out ==> certainty_spec::saved_prefix(final(self), instances),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn capture(&mut self, decoder: &ArithmeticDecoder<'a>, instances: u32) -> bool {
        let _ = (decoder, instances);
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost prior = *self; }
        if self.active {
            #[cfg(verus_keep_ghost)]
            proof! {
                self.saved = Ghost(Some(certainty_spec::Snapshot {
                    decoder: *decoder, spread: self.spread, history: self.history@, instances,
                }));
                certainty_spec::observer_save(&prior, self, decoder, instances);
            }
        }
        self.active
    }
}
