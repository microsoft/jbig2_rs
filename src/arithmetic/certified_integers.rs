//! Compatibility-only Annex A drivers. Ordinary arithmetic hot paths are untouched.

#[cfg(verus_keep_ghost)]
use super::certainty::certainty_spec;
#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    ArithmeticContext, ArithmeticDecoder, ArithmeticError, ArithmeticInteger, MalformedError,
    PrefixCertainty, INTEGER_CONTEXT_COUNT, MAX_IAID_CODE_LENGTH,
};

impl<'a> ArithmeticDecoder<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_valid(old(self)),
            certainty_spec::observer_valid(old(observer), old(self)),
            forall|i: int| 0 <= i < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[i]),
        ensures arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes, final(self).bytes@ == old(self).bytes@,
            certainty_spec::observer_valid(final(observer), final(self)),
            !old(observer).active ==> !final(observer).active,
            final(observer).saved == old(observer).saved,
            forall|i: int| 0 <= i < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[i]),
            out.is_ok() ==> arithmetic_spec::decoder_valid(final(self)),
            out == arithmetic_spec::integer_state(*old(self), old(contexts)@).0,
            *final(self) == arithmetic_spec::integer_state(*old(self), old(contexts)@).1,
            final(contexts)@ == arithmetic_spec::integer_state(*old(self), old(contexts)@).2,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::cast_possible_wrap)]
    pub(crate) fn decode_integer_certified(
        &mut self,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
        observer: &mut PrefixCertainty<'a>,
    ) -> Result<ArithmeticInteger, ArithmeticError> {
        #[cfg(verus_keep_ghost)]
        proof! { reveal_with_fuel(arithmetic_spec::integer_selection_state, 6); }
        let mut previous = 1_u16;
        let sign = self.decode_integer_bit_certified(&mut previous, contexts, observer)?;
        let (bit_count, offset) =
            if self.decode_integer_bit_certified(&mut previous, contexts, observer)? == 0 {
                (2, 0_u64)
            } else if self.decode_integer_bit_certified(&mut previous, contexts, observer)? == 0 {
                (4, 4)
            } else if self.decode_integer_bit_certified(&mut previous, contexts, observer)? == 0 {
                (6, 20)
            } else if self.decode_integer_bit_certified(&mut previous, contexts, observer)? == 0 {
                (8, 84)
            } else if self.decode_integer_bit_certified(&mut previous, contexts, observer)? == 0 {
                (12, 340)
            } else {
                (32, 4436)
            };
        let magnitude_bits =
            self.decode_integer_bits_certified(&mut previous, bit_count, contexts, observer)?;
        let magnitude = offset
            .checked_add(u64::from(magnitude_bits))
            .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
        if sign == 1 && magnitude == 0 {
            return Ok(ArithmeticInteger::OutOfBand);
        }
        #[cfg(verus_keep_ghost)]
        production_integer_magnitude!(magnitude_bits, offset);
        let magnitude = magnitude as i64;
        if sign == 0 {
            Ok(ArithmeticInteger::Value(magnitude))
        } else {
            Ok(ArithmeticInteger::Value(-magnitude))
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_valid(old(self)),
            certainty_spec::observer_valid(old(observer), old(self)),
            *old(previous) <= 511,
            forall|i: int| 0 <= i < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[i]),
        ensures arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes, final(self).bytes@ == old(self).bytes@,
            certainty_spec::observer_valid(final(observer), final(self)),
            !old(observer).active ==> !final(observer).active,
            final(observer).saved == old(observer).saved,
            *final(previous) <= 511,
            forall|i: int| 0 <= i < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[i]),
            out.is_ok() ==> arithmetic_spec::decoder_valid(final(self)),
            out == arithmetic_spec::integer_step_state(*old(self), old(contexts)@, *old(previous)).0,
            *final(self) == arithmetic_spec::integer_step_state(*old(self), old(contexts)@, *old(previous)).1,
            final(contexts)@ == arithmetic_spec::integer_step_state(*old(self), old(contexts)@, *old(previous)).2,
            *final(previous) == arithmetic_spec::integer_step_state(*old(self), old(contexts)@, *old(previous)).3,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn decode_integer_bit_certified(
        &mut self,
        previous: &mut u16,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
        observer: &mut PrefixCertainty<'a>,
    ) -> Result<u8, ArithmeticError> {
        let prior = *previous;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(prior & 511 == prior) by(bit_vector) requires prior <= 511;
        }
        let context_index = usize::from(prior & 0x01ff);
        #[cfg(verus_keep_ghost)]
        production_integer_history_index!(prior);
        let decision = observer.decode(self, &mut contexts[context_index])?;
        let shifted = prior * 2 + u16::from(decision);
        *previous = if prior < 256 {
            shifted
        } else {
            (shifted & 511) | 256
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            if prior < 256 {
                assert(shifted <= 511);
            } else {
                assert((shifted & 511) | 256 <= 511) by(bit_vector);
            }
        }
        Ok(decision)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_valid(old(self)),
            certainty_spec::observer_valid(old(observer), old(self)),
            *old(previous) <= 511, bit_count <= 32,
            forall|i: int| 0 <= i < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[i]),
        ensures arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes, final(self).bytes@ == old(self).bytes@,
            certainty_spec::observer_valid(final(observer), final(self)),
            !old(observer).active ==> !final(observer).active,
            final(observer).saved == old(observer).saved,
            *final(previous) <= 511,
            forall|i: int| 0 <= i < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[i]),
            out.is_ok() ==> arithmetic_spec::decoder_valid(final(self)),
            out == arithmetic_spec::integer_bits_run(*old(self), old(contexts)@, *old(previous), bit_count, 0).0,
            *final(self) == arithmetic_spec::integer_bits_run(*old(self), old(contexts)@, *old(previous), bit_count, 0).1,
            final(contexts)@ == arithmetic_spec::integer_bits_run(*old(self), old(contexts)@, *old(previous), bit_count, 0).2,
            *final(previous) == arithmetic_spec::integer_bits_run(*old(self), old(contexts)@, *old(previous), bit_count, 0).3,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn decode_integer_bits_certified(
        &mut self,
        previous: &mut u16,
        bit_count: u8,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
        observer: &mut PrefixCertainty<'a>,
    ) -> Result<u32, ArithmeticError> {
        let mut value = 0_u32;
        let mut remaining = bit_count;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant arithmetic_spec::decoder_valid(self),
                self.bytes == old(self).bytes, self.bytes@ == old(self).bytes@,
                certainty_spec::observer_valid(observer, self),
                !old(observer).active ==> !observer.active,
                observer.saved == old(observer).saved,
                *previous <= 511, remaining <= bit_count,
                forall|i: int| 0 <= i < contexts@.len() ==>
                    arithmetic_spec::context_valid(contexts@[i]),
                arithmetic_spec::integer_bits_run(*old(self), old(contexts)@, *old(previous), bit_count, 0)
                    == arithmetic_spec::integer_bits_run(*self, contexts@, *previous, remaining, value),
            decreases remaining,
        ))]
        while remaining != 0 {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = *self;
                let ghost before_contexts = contexts@;
                let ghost before_previous = *previous;
                let ghost before_remaining = remaining;
                let ghost before_value = value;
            }
            #[cfg(verus_keep_ghost)]
            proof! { reveal_with_fuel(arithmetic_spec::integer_bits_run, 1); }
            let bit = self.decode_integer_bit_certified(previous, contexts, observer)?;
            let shifted = value
                .checked_mul(2)
                .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
            value = shifted
                .checked_add(u32::from(bit))
                .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
            remaining -= 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(arithmetic_spec::integer_bits_run(before, before_contexts,
                    before_previous, before_remaining, before_value)
                    == arithmetic_spec::integer_bits_run(*self, contexts@, *previous, remaining, value));
            }
        }
        Ok(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_valid(old(self)),
            certainty_spec::observer_valid(old(observer), old(self)),
            forall|i: int| 0 <= i < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[i]),
        ensures arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes, final(self).bytes@ == old(self).bytes@,
            certainty_spec::observer_valid(final(observer), final(self)),
            !old(observer).active ==> !final(observer).active,
            final(observer).saved == old(observer).saved,
            forall|i: int| 0 <= i < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[i]),
            out.is_ok() ==> arithmetic_spec::decoder_valid(final(self)),
            out == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).0,
            *final(self) == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).1,
            final(contexts)@ == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).2,
            out.is_ok() ==> code_length <= MAX_IAID_CODE_LENGTH
                && (out.unwrap() as u64) < (1u64 << code_length),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn decode_iaid_certified(
        &mut self,
        code_length: u8,
        contexts: &mut [ArithmeticContext],
        observer: &mut PrefixCertainty<'a>,
    ) -> Result<u32, ArithmeticError> {
        if code_length > MAX_IAID_CODE_LENGTH {
            return Err(ArithmeticError::InvalidIaidCodeLength(code_length));
        }
        let required = 1_usize << code_length;
        let context_count = contexts.len();
        if context_count < required {
            return Err(ArithmeticError::InsufficientContexts {
                required,
                actual: context_count,
            });
        }
        let mut previous = 1_usize;
        let mut remaining = code_length;
        #[cfg(verus_keep_ghost)]
        production_iaid_prefix_bounds!(initial, previous, code_length, remaining);
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant arithmetic_spec::decoder_valid(self),
                self.bytes == old(self).bytes, self.bytes@ == old(self).bytes@,
                certainty_spec::observer_valid(observer, self),
                !old(observer).active ==> !observer.active,
                observer.saved == old(observer).saved,
                code_length <= MAX_IAID_CODE_LENGTH,
                required == 1usize << code_length, required <= context_count,
                context_count == contexts@.len(), contexts@.len() == old(contexts)@.len(),
                remaining <= code_length,
                (previous as u64) >> ((code_length - remaining) as u64) == 1,
                forall|i: int| 0 <= i < contexts@.len() ==>
                    arithmetic_spec::context_valid(contexts@[i]),
                arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required)
                    == arithmetic_spec::iaid_run(*self, contexts@, previous, remaining, required),
            decreases remaining,
        ))]
        while remaining != 0 {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = *self;
                let ghost before_contexts = contexts@;
                let ghost before_previous = previous;
                let ghost before_remaining = remaining;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(arithmetic_spec::iaid_run, 1);
                assert(0 < previous && previous < required) by(bit_vector)
                    requires 0 < remaining <= code_length, code_length <= 31,
                        required == 1usize << code_length,
                        (previous as u64) >> ((code_length - remaining) as u64) == 1;
            }
            if previous >= context_count {
                return Err(ArithmeticError::InsufficientContexts {
                    required,
                    actual: context_count,
                });
            }
            let decision = observer.decode(self, &mut contexts[previous])?;
            let shifted = previous
                .checked_mul(2)
                .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
            previous = shifted
                .checked_add(usize::from(decision))
                .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
            remaining -= 1;
            #[cfg(verus_keep_ghost)]
            production_iaid_prefix_bounds!(
                advance,
                previous,
                before_previous,
                decision,
                code_length,
                remaining,
                before_remaining
            );
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(arithmetic_spec::iaid_run(before, before_contexts,
                    before_previous, before_remaining, required)
                    == arithmetic_spec::iaid_run(*self, contexts@, previous, remaining, required));
            }
        }
        let value = previous
            .checked_sub(required)
            .ok_or(ArithmeticError::Malformed(MalformedError::IntegerOverflow))?;
        #[cfg(verus_keep_ghost)]
        production_iaid_prefix_bounds!(result, value, previous, required, code_length);
        if value > u32::MAX as usize {
            return Err(ArithmeticError::Malformed(MalformedError::IntegerOverflow));
        }
        Ok(value as u32)
    }
}
