//! Decoding implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    unexpected_eof, ArithmeticContext, ArithmeticDecoder, ArithmeticError, MalformedError, QeState,
    MIN_INTERVAL, QE_PACKED_VALS, QE_STATES, RASTER_SIGNAL_EOF, RASTER_SIGNAL_OVERFLOW,
};

impl<'a> ArithmeticDecoder<'a> {
    /// Initializes the Annex E decoder at the start of one arithmetic stream.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
        out.is_ok() == (bytes@.len() >= 2),
        match out {
            Ok(decoder) =>
                arithmetic_spec::decoder_valid(&decoder)
                && decoder.bytes@ == bytes@
                && decoder.interval == MIN_INTERVAL
                && decoder.byte_position == arithmetic_spec::initialized_state(bytes).unwrap().0
                && decoder.current_byte == arithmetic_spec::initialized_state(bytes).unwrap().1
                && decoder.code == arithmetic_spec::initialized_state(bytes).unwrap().2
                && decoder.bit_count == arithmetic_spec::initialized_state(bytes).unwrap().3
                && decoder.marker == arithmetic_spec::initialized_state(bytes).unwrap().4,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self, ArithmeticError> {
        #[cfg(verus_keep_ghost)]
        production_reveal_decoder_valid!();
        if bytes.is_empty() {
            return Err(unexpected_eof(0, 1, 0));
        }
        let first = bytes[0];
        let mut decoder = Self {
            bytes,
            byte_position: 0,
            current_byte: first,
            code: u32::from(first) << 16,
            interval: MIN_INTERVAL,
            bit_count: 0,
            marker: None,
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            if bytes@.len() >= 2 {
                arithmetic_spec::initialization_code_bound(bytes@[0], bytes@[1]);
                assert(arithmetic_spec::refill_step(&decoder).0 == 0);
                assert(arithmetic_spec::refill_step(&decoder).3 <= u32::MAX / 128);
            }
        }
        decoder.byte_in()?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(bytes@.len() >= 2);
            assert(decoder.code <= u32::MAX / 128);
            // Relate the native shift constant to the model's arithmetic scale.
            assert((1u32 << 7) == 128u32) by(bit_vector);
        }
        decoder.code = decoder
            .code
            .checked_mul(1 << 7)
            .ok_or(MalformedError::IntegerOverflow)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(decoder.interval == MIN_INTERVAL);
            assert(decoder.bit_count >= 7);
            assert(decoder.bit_count <= 8);
        }
        decoder.bit_count = decoder
            .bit_count
            .checked_sub(7)
            .ok_or(MalformedError::IntegerOverflow)?;
        Ok(decoder)
    }

    /// Decodes one binary decision and updates its probability context.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            arithmetic_spec::context_valid(*old(context)),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
                old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)),
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
                old(self).interval,
                old(self).code,
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
            match out {
                Ok(decision) =>
                    decision <= 1
                    && arithmetic_spec::decoder_valid(final(self))
                    && decision == arithmetic_spec::entry_decision(
                        old(self).interval,
                        old(self).code,
                        arithmetic_spec::context_index(*old(context)),
                        arithmetic_spec::context_mps(*old(context)),
                    )
                    && decision == arithmetic_spec::decision_state(old(self), *old(context)).0,
                Err(_) => true,
            },
            out.is_ok() == (arithmetic_spec::decision_state(old(self), *old(context)).0 <= 1),
            final(self).interval == arithmetic_spec::decision_state(old(self), *old(context)).1,
            final(self).byte_position == arithmetic_spec::decision_state(old(self), *old(context)).2,
            final(self).current_byte == arithmetic_spec::decision_state(old(self), *old(context)).3,
            final(self).code == arithmetic_spec::decision_state(old(self), *old(context)).4,
            final(self).bit_count == arithmetic_spec::decision_state(old(self), *old(context)).5,
            final(self).marker == arithmetic_spec::decision_state(old(self), *old(context)).6,
            match out {
                Err(error) => error == arithmetic_spec::error_for_signal(
                    final(self), arithmetic_spec::decision_state(old(self), *old(context)).0,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(crate) fn decode(
        &mut self,
        context: &mut ArithmeticContext,
    ) -> Result<u8, ArithmeticError> {
        let signal = self.decode_signal(context);
        if signal <= 1 {
            Ok(signal)
        } else {
            Err(self.signal_error(signal))
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            arithmetic_spec::context_valid(*old(context)),
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out <= 1 ==> arithmetic_spec::decoder_valid(final(self)),
            out <= 1 ==> out == arithmetic_spec::entry_decision(
                old(self).interval,
                old(self).code,
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
                old(self).interval, old(self).code, arithmetic_spec::context_index(*old(context)),
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
                old(self).interval,
                old(self).code,
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
            out == arithmetic_spec::decision_state(old(self), *old(context)).0,
            final(self).interval == arithmetic_spec::decision_state(old(self), *old(context)).1,
            final(self).byte_position == arithmetic_spec::decision_state(old(self), *old(context)).2,
            final(self).current_byte == arithmetic_spec::decision_state(old(self), *old(context)).3,
            final(self).code == arithmetic_spec::decision_state(old(self), *old(context)).4,
            final(self).bit_count == arithmetic_spec::decision_state(old(self), *old(context)).5,
            final(self).marker == arithmetic_spec::decision_state(old(self), *old(context)).6,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(crate) fn decode_signal(&mut self, context: &mut ArithmeticContext) -> u8 {
        #[cfg(verus_keep_ghost)]
        production_reveal_decoder_valid!();
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::arithmetic::arithmetic_shared::packed_context_bounds(*context);
            crate::arithmetic::arithmetic_shared::packed_tables_match(context.packed);
            arithmetic_spec::decision_model_equations(self, *context);
            arithmetic_spec::entry_model_equations(
                self.interval,
                self.code,
                arithmetic_spec::context_index(*context),
                arithmetic_spec::context_mps(*context),
            );
        }
        let packed = context.packed();
        #[cfg(verus_keep_ghost)]
        proof! {
            let p_usize = packed as usize;
            assert((p_usize & 0x7fusize) == p_usize) by (bit_vector)
                requires p_usize < 94usize;
        }
        let qe = u32::from(QE_PACKED_VALS[usize::from(packed) & 0x7f]);
        let old_mps = context.mps();

        #[cfg(verus_keep_ghost)]
        production_decode_interval_subtraction!(self, qe);
        self.interval -= qe;

        if self.code >> 16 < qe {
            let state_index = context.state_index();
            let state = QE_STATES[usize::from(state_index)];
            #[cfg(verus_keep_ghost)]
            production_qe_transition_valid!(state_index, state);
            return self.decode_lower_signal(context, state, qe, old_mps);
        }

        #[cfg(verus_keep_ghost)]
        proof! {
            let value = self.code;
            assert(value >= qe << 16) by (bit_vector)
                requires value >> 16 >= qe, qe <= 0xffff;
        }
        let Some(code) = self.code.checked_sub(qe << 16) else {
            return RASTER_SIGNAL_OVERFLOW;
        };
        self.code = code;
        if self.interval >= MIN_INTERVAL {
            return old_mps;
        }

        let state_index = context.state_index();
        let state = QE_STATES[usize::from(state_index)];
        #[cfg(verus_keep_ghost)]
        production_qe_transition_valid!(state_index, state);
        self.decode_upper_signal(context, state, qe, old_mps)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_state_valid(old(self)),
            arithmetic_spec::context_valid(*old(context)),
            state.next_mps < 47,
            state.next_lps < 47,
            0 < qe <= 0x5601,
            old_mps <= 1,
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out <= 1 ==> arithmetic_spec::decoder_valid(final(self)),
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::exchange_index(
                true, old(self).interval, qe, state.next_mps, state.next_lps,
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::exchange_mps(
                true,
                old(self).interval,
                qe,
                state.switch_mps,
                arithmetic_spec::context_mps(*old(context)),
                old_mps,
            ),
            out <= 1 ==> out == arithmetic_spec::exchange_decision(
                true, old(self).interval, qe, old_mps,
            ),
            out == if arithmetic_spec::exchange_normalization(old(self), true, qe).0 == 0 {
                arithmetic_spec::exchange_decision(true, old(self).interval, qe, old_mps)
            } else {
                arithmetic_spec::exchange_normalization(old(self), true, qe).0
            },
            final(self).interval == arithmetic_spec::exchange_normalization(old(self), true, qe).1,
            final(self).byte_position == arithmetic_spec::exchange_normalization(old(self), true, qe).2,
            final(self).current_byte == arithmetic_spec::exchange_normalization(old(self), true, qe).3,
            final(self).code == arithmetic_spec::exchange_normalization(old(self), true, qe).4,
            final(self).bit_count == arithmetic_spec::exchange_normalization(old(self), true, qe).5,
            final(self).marker == arithmetic_spec::exchange_normalization(old(self), true, qe).6,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[cold]
    #[inline(never)]
    pub(super) fn decode_lower_signal(
        &mut self,
        context: &mut ArithmeticContext,
        state: QeState,
        qe: u32,
        old_mps: u8,
    ) -> u8 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::arithmetic::arithmetic_shared::packed_context_bounds(*context);
            reveal(arithmetic_spec::exchange_decision);
            reveal(arithmetic_spec::exchange_is_mps);
            reveal(arithmetic_spec::exchange_index);
            reveal(arithmetic_spec::exchange_mps);
        }
        let prior_mps = context.mps();
        let decision;
        if self.interval < qe {
            decision = old_mps;
            context.set_state_and_mps(state.next_mps, prior_mps);
        } else {
            decision = 1 - old_mps;
            let next_mps = if state.switch_mps {
                decision
            } else {
                prior_mps
            };
            context.set_state_and_mps(state.next_lps, next_mps);
        }
        self.interval = qe;
        let signal = self.renormalize_signal();
        if signal == 0 {
            decision
        } else {
            signal
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_state_valid(old(self)),
            arithmetic_spec::context_valid(*old(context)),
            state.next_mps < 47,
            state.next_lps < 47,
            0 < qe <= 0x5601,
            old_mps <= 1,
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out <= 1 ==> arithmetic_spec::decoder_valid(final(self)),
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::exchange_index(
                false, old(self).interval, qe, state.next_mps, state.next_lps,
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::exchange_mps(
                false,
                old(self).interval,
                qe,
                state.switch_mps,
                arithmetic_spec::context_mps(*old(context)),
                old_mps,
            ),
            out <= 1 ==> out == arithmetic_spec::exchange_decision(
                false, old(self).interval, qe, old_mps,
            ),
            out == if arithmetic_spec::exchange_normalization(old(self), false, qe).0 == 0 {
                arithmetic_spec::exchange_decision(false, old(self).interval, qe, old_mps)
            } else {
                arithmetic_spec::exchange_normalization(old(self), false, qe).0
            },
            final(self).interval == arithmetic_spec::exchange_normalization(old(self), false, qe).1,
            final(self).byte_position == arithmetic_spec::exchange_normalization(old(self), false, qe).2,
            final(self).current_byte == arithmetic_spec::exchange_normalization(old(self), false, qe).3,
            final(self).code == arithmetic_spec::exchange_normalization(old(self), false, qe).4,
            final(self).bit_count == arithmetic_spec::exchange_normalization(old(self), false, qe).5,
            final(self).marker == arithmetic_spec::exchange_normalization(old(self), false, qe).6,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[cold]
    #[inline(never)]
    pub(super) fn decode_upper_signal(
        &mut self,
        context: &mut ArithmeticContext,
        state: QeState,
        qe: u32,
        old_mps: u8,
    ) -> u8 {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::arithmetic::arithmetic_shared::packed_context_bounds(*context);
            reveal(arithmetic_spec::exchange_decision);
            reveal(arithmetic_spec::exchange_is_mps);
            reveal(arithmetic_spec::exchange_index);
            reveal(arithmetic_spec::exchange_mps);
        }
        let prior_mps = context.mps();
        let decision;
        if self.interval < qe {
            decision = 1 - old_mps;
            let next_mps = if state.switch_mps {
                decision
            } else {
                prior_mps
            };
            context.set_state_and_mps(state.next_lps, next_mps);
        } else {
            decision = old_mps;
            context.set_state_and_mps(state.next_mps, prior_mps);
        }
        let signal = self.renormalize_signal();
        if signal == 0 {
            decision
        } else {
            signal
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == arithmetic_spec::error_for_signal(self, signal),
    ))]
    pub(crate) fn signal_error(&self, signal: u8) -> ArithmeticError {
        if signal == RASTER_SIGNAL_EOF {
            match self.byte_position.checked_add(1) {
                Some(offset) => unexpected_eof(offset, 1, 0),
                None => ArithmeticError::Malformed(MalformedError::IntegerOverflow),
            }
        } else {
            ArithmeticError::Malformed(MalformedError::IntegerOverflow)
        }
    }

    #[cfg(test)]
    pub(super) fn decode_checked_oracle(
        &mut self,
        context: &mut ArithmeticContext,
    ) -> Result<u8, ArithmeticError> {
        let state = QE_STATES[usize::from(context.state_index())];
        let qe = u32::from(state.qe);
        let old_mps = context.mps();
        self.interval = self
            .interval
            .checked_sub(qe)
            .ok_or(MalformedError::IntegerOverflow)?;

        if self.code >> 16 < qe {
            let decision;
            if self.interval < qe {
                decision = old_mps;
                context.set_state_and_mps(state.next_mps, old_mps);
            } else {
                decision = 1 - old_mps;
                let next_mps = if state.switch_mps { decision } else { old_mps };
                context.set_state_and_mps(state.next_lps, next_mps);
            }
            self.interval = qe;
            self.renormalize_checked_oracle()?;
            return Ok(decision);
        }

        self.code = self
            .code
            .checked_sub(qe << 16)
            .ok_or(MalformedError::IntegerOverflow)?;
        if self.interval >= MIN_INTERVAL {
            return Ok(old_mps);
        }

        let decision;
        if self.interval < qe {
            decision = 1 - old_mps;
            let next_mps = if state.switch_mps { decision } else { old_mps };
            context.set_state_and_mps(state.next_lps, next_mps);
        } else {
            decision = old_mps;
            context.set_state_and_mps(state.next_mps, old_mps);
        }
        self.renormalize_checked_oracle()?;
        Ok(decision)
    }
}
