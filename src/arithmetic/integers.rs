//! Integers implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    ArithmeticContext, ArithmeticDecoder, ArithmeticError, ArithmeticInteger, MalformedError,
    INTEGER_CONTEXT_COUNT, MAX_IAID_CODE_LENGTH,
};

impl ArithmeticDecoder<'_> {
    /// Decodes one Annex A integer using exactly 512 procedure-specific contexts.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            forall|index: int| 0 <= index < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[index]),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes,
            final(self).bytes@ == old(self).bytes@,
            forall|index: int| 0 <= index < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[index]),
            match out {
                Ok(_) => arithmetic_spec::decoder_valid(final(self)),
                Err(_) => true,
            },
            out == arithmetic_spec::integer_state(*old(self), old(contexts)@).0,
            *final(self) == arithmetic_spec::integer_state(*old(self), old(contexts)@).1,
            final(contexts)@ == arithmetic_spec::integer_state(*old(self), old(contexts)@).2,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::cast_possible_wrap, clippy::too_many_lines)]
    pub(crate) fn decode_integer(
        &mut self,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    ) -> Result<ArithmeticInteger, ArithmeticError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal_with_fuel(arithmetic_spec::integer_selection_state, 6);
        }
        let mut previous = 1_u16;
        let mut interval = self.interval;
        let mut code = self.code;
        let mut mq_bit_count = self.bit_count;
        let sign = self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )?;

        let (bit_count, offset) = if self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )? == 0
        {
            (2, 0_u64)
        } else if self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )? == 0
        {
            (4, 4)
        } else if self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )? == 0
        {
            (6, 20)
        } else if self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )? == 0
        {
            (8, 84)
        } else if self.decode_integer_bit_cached(
            &mut previous,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )? == 0
        {
            (12, 340)
        } else {
            (32, 4436)
        };

        let magnitude_bits = self.decode_integer_bits_cached(
            &mut previous,
            bit_count,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )?;
        self.interval = interval;
        self.code = code;
        self.bit_count = mq_bit_count;
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
            let negative = -magnitude;
            Ok(ArithmeticInteger::Value(negative))
        }
    }

    /// Decodes one fixed-length Annex A IAID value.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            forall|index: int| 0 <= index < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[index]),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            forall|index: int| 0 <= index < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[index]),
            match out {
                Ok(_) => arithmetic_spec::decoder_valid(final(self)),
                Err(_) => true,
            },
            code_length > MAX_IAID_CODE_LENGTH ==> {
                &&& out == Err(ArithmeticError::InvalidIaidCodeLength(code_length))
                &&& *final(self) == *old(self)
                &&& final(contexts)@ == old(contexts)@
            },
            code_length <= MAX_IAID_CODE_LENGTH
                && old(contexts)@.len() < (1usize << code_length) ==> {
                &&& out == Err(ArithmeticError::InsufficientContexts {
                    required: 1usize << code_length, actual: old(contexts)@.len() as usize,
                })
                &&& *final(self) == *old(self)
                &&& final(contexts)@ == old(contexts)@
            },
            out == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).0,
            *final(self) == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).1,
            final(contexts)@ == arithmetic_spec::iaid_state(*old(self), old(contexts)@, code_length).2,
            match out {
                Ok(value) => code_length <= MAX_IAID_CODE_LENGTH
                    && (value as u64) < (1u64 << code_length),
                Err(_) => true,
            },
            code_length == 0 && old(contexts)@.len() >= 1 ==> {
                &&& out == Ok(0)
                &&& *final(self) == *old(self)
                &&& final(contexts)@ == old(contexts)@
            },
            code_length <= MAX_IAID_CODE_LENGTH ==>
                forall|index: int| 0 <= index < final(contexts)@.len()
                    && (index == 0 || index >= (1usize << code_length)) ==>
                        final(contexts)@[index] == old(contexts)@[index],
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    pub(crate) fn decode_iaid(
        &mut self,
        code_length: u8,
        contexts: &mut [ArithmeticContext],
    ) -> Result<u32, ArithmeticError> {
        if code_length > MAX_IAID_CODE_LENGTH {
            return Err(ArithmeticError::InvalidIaidCodeLength(code_length));
        }
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
        }

        let (ctxs, _rest) = contexts.split_at_mut(required);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(required >= 1) by (bit_vector)
                requires
                    code_length <= 31,
                    required == 1usize << code_length;
            assert(ext_equal(ctxs@ + _rest@, old(contexts)@));
            assert(ctxs@[0] == old(contexts)@[0]);
            assert(
                ArithmeticDecoder {
                    bytes: self.bytes,
                    byte_position: self.byte_position,
                    current_byte: self.current_byte,
                    code: self.code,
                    interval: self.interval,
                    bit_count: self.bit_count,
                    marker: self.marker,
                } == *old(self)
            );
        }
        let mut interval = self.interval;
        let mut code = self.code;
        let mut mq_bit_count = self.bit_count;
        let mut previous = 1_usize;
        let mut remaining = code_length;
        let mut loop_error: Option<ArithmeticError> = None;
        #[cfg(verus_keep_ghost)]
        production_iaid_prefix_bounds!(initial, previous, code_length, remaining);
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                loop_error.is_none() ==> arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                    bytes: self.bytes,
                    byte_position: self.byte_position,
                    current_byte: self.current_byte,
                    code,
                    interval,
                    bit_count: mq_bit_count,
                    marker: self.marker,
                }),
                loop_error.is_some() ==> {
                    &&& arithmetic_spec::decoder_state_valid(self)
                    &&& self.interval == interval
                    &&& self.code == code
                    &&& self.bit_count == mq_bit_count
                    &&& arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required)
                        == (Err(loop_error.unwrap()), *self, ctxs@ + _rest@)
                },
                self.bytes == old(self).bytes,
                self.bytes@ == old(self).bytes@,
                code_length <= MAX_IAID_CODE_LENGTH,
                required == 1usize << code_length,
                required >= 1,
                context_count == old(contexts)@.len(),
                ctxs@.len() == required,
                _rest@.len() == context_count - required,
                _rest@ == old(contexts)@.subrange(required as int, context_count as int),
                forall|index: int| 0 <= index < ctxs@.len() ==>
                    arithmetic_spec::context_valid(ctxs@[index]),
                forall|index: int| 0 <= index < _rest@.len() ==>
                    arithmetic_spec::context_valid(_rest@[index]),
                remaining <= code_length,
                required <= context_count,
                loop_error.is_none() ==> (previous as u64) >> ((code_length - remaining) as u64) == 1,
                ctxs@[0] == old(contexts)@[0],
                loop_error.is_none() ==>
                    arithmetic_spec::iaid_run(*old(self), old(contexts)@, 1, code_length, required)
                        == arithmetic_spec::iaid_run(
                            ArithmeticDecoder {
                                bytes: self.bytes,
                                byte_position: self.byte_position,
                                current_byte: self.current_byte,
                                code,
                                interval,
                                bit_count: mq_bit_count,
                                marker: self.marker,
                            },
                            ctxs@ + _rest@,
                            previous,
                            remaining,
                            required,
                        ),
            decreases remaining, if loop_error.is_none() { 1int } else { 0int },
        ))]
        while remaining != 0 && loop_error.is_none() {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost before = ArithmeticDecoder {
                    bytes: self.bytes,
                    byte_position: self.byte_position,
                    current_byte: self.current_byte,
                    code,
                    interval,
                    bit_count: mq_bit_count,
                    marker: self.marker,
                };
                let ghost before_contexts = ctxs@ + _rest@;
                let ghost before_previous = previous;
                let ghost before_remaining = remaining;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(arithmetic_spec::iaid_run, 1);
                assert(0 < previous && previous < required) by(bit_vector)
                    requires
                        0 < remaining <= code_length,
                        code_length <= 31,
                        required == 1usize << code_length,
                        (previous as u64) >> ((code_length - remaining) as u64) == 1;
            }
            match self.decode_bit_cached_result(
                &mut ctxs[previous],
                &mut interval,
                &mut code,
                &mut mq_bit_count,
            ) {
                Ok(decision) => {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(
                            (previous as u64) * 2 + (decision as u64) <= u32::MAX as u64
                            && (((previous << 1usize) | (decision as usize)) as u64)
                                == (previous as u64) * 2 + (decision as u64)
                        ) by (bit_vector)
                            requires
                                0 < previous && previous < required,
                                code_length <= 31,
                                required == 1usize << code_length,
                                decision <= 1u8;
                    }
                    previous = (previous << 1) | usize::from(decision);
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
                        let after = ArithmeticDecoder {
                            bytes: self.bytes,
                            byte_position: self.byte_position,
                            current_byte: self.current_byte,
                            code,
                            interval,
                            bit_count: mq_bit_count,
                            marker: self.marker,
                        };
                        let after_contexts = ctxs@ + _rest@;
                        assert(ext_equal(
                            after_contexts,
                            before_contexts.update(
                                before_previous as int,
                                arithmetic_spec::context_after_decision(&before, before_contexts[before_previous as int]),
                            ),
                        ));
                        assert(
                            arithmetic_spec::iaid_run(
                                before, before_contexts, before_previous, before_remaining, required,
                            ) == arithmetic_spec::iaid_run(after, after_contexts, previous, remaining, required)
                        );
                    }
                }
                Err(error) => {
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
                }
            }
        }
        if let Some(error) = loop_error {
            return Err(error);
        }
        self.interval = interval;
        self.code = code;
        self.bit_count = mq_bit_count;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(remaining == 0);
            assert(
                ArithmeticDecoder {
                    bytes: self.bytes,
                    byte_position: self.byte_position,
                    current_byte: self.current_byte,
                    code,
                    interval,
                    bit_count: mq_bit_count,
                    marker: self.marker,
                } == *self
            );
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

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            *old(previous) <= 511,
            forall|index: int| 0 <= index < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[index]),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            *final(previous) <= 511,
            forall|index: int| 0 <= index < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[index]),
            match out {
                Ok(decision) =>
                    decision <= 1
                    && arithmetic_spec::decoder_valid(final(self))
                    && decision == arithmetic_spec::integer_bit_state(
                        old(self), old(contexts)@, *old(previous),
                    ).0
                    && *final(previous) == arithmetic_spec::integer_history_step(*old(previous), decision),
                Err(error) =>
                    *final(previous) == *old(previous)
                    && error == arithmetic_spec::error_for_signal(
                        final(self), arithmetic_spec::integer_bit_state(
                            old(self), old(contexts)@, *old(previous),
                        ).0,
                    ),
            },
            out.is_ok() == (arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).0 <= 1),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            final(self).interval == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).1,
            final(self).byte_position == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).2,
            final(self).current_byte == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).3,
            final(self).code == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).4,
            final(self).bit_count == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).5,
            final(self).marker == arithmetic_spec::integer_bit_state(old(self), old(contexts)@, *old(previous)).6,
            final(contexts)@[( *old(previous) & 511) as int]
                == arithmetic_spec::context_after_decision(
                    old(self), old(contexts)@[( *old(previous) & 511) as int],
                ),
            forall|index: int| 0 <= index < old(contexts)@.len()
                && index != (*old(previous) & 511) as int ==>
                    final(contexts)@[index] == old(contexts)@[index],
            out == arithmetic_spec::integer_step_state(
                *old(self), old(contexts)@, *old(previous),
            ).0,
            *final(self) == arithmetic_spec::integer_step_state(
                *old(self), old(contexts)@, *old(previous),
            ).1,
            final(contexts)@ == arithmetic_spec::integer_step_state(
                *old(self), old(contexts)@, *old(previous),
            ).2,
            *final(previous) == arithmetic_spec::integer_step_state(
                *old(self), old(contexts)@, *old(previous),
            ).3,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn decode_integer_bit(
        &mut self,
        previous: &mut u16,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    ) -> Result<u8, ArithmeticError> {
        let prior = *previous;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(prior & 511 == prior) by (bit_vector)
                requires prior <= 511;
        }
        let context_index = usize::from(prior & 0x01ff);
        #[cfg(verus_keep_ghost)]
        production_integer_history_index!(prior);
        let decision = self.decode(&mut contexts[context_index])?;
        let shifted = prior * 2 + u16::from(decision);
        *previous = if prior < 256 {
            shifted
        } else {
            (shifted & 511) | 256
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            if prior < 256 {
                assert(prior <= 255);
                assert(decision <= 1);
                assert(shifted == prior * 2 + decision as u16);
                assert(shifted <= 511);
            } else {
                assert((shifted & 511) | 256 <= 511) by (bit_vector);
            }
        }
        Ok(decision)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(old(self)),
            *old(previous) <= 511,
            bit_count <= 32,
            forall|index: int| 0 <= index < old(contexts)@.len() ==>
                arithmetic_spec::context_valid(old(contexts)@[index]),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes == old(self).bytes,
            final(self).bytes@ == old(self).bytes@,
            *final(previous) <= 511,
            forall|index: int| 0 <= index < final(contexts)@.len() ==>
                arithmetic_spec::context_valid(final(contexts)@[index]),
            match out {
                Ok(_) => arithmetic_spec::decoder_valid(final(self)),
                Err(_) => true,
            },
            out == arithmetic_spec::integer_bits_run(
                *old(self), old(contexts)@, *old(previous), bit_count, 0,
            ).0,
            *final(self) == arithmetic_spec::integer_bits_run(
                *old(self), old(contexts)@, *old(previous), bit_count, 0,
            ).1,
            final(contexts)@ == arithmetic_spec::integer_bits_run(
                *old(self), old(contexts)@, *old(previous), bit_count, 0,
            ).2,
            *final(previous) == arithmetic_spec::integer_bits_run(
                *old(self), old(contexts)@, *old(previous), bit_count, 0,
            ).3,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn decode_integer_bits(
        &mut self,
        previous: &mut u16,
        bit_count: u8,
        contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    ) -> Result<u32, ArithmeticError> {
        let mut interval = self.interval;
        let mut code = self.code;
        let mut mq_bit_count = self.bit_count;
        let value = self.decode_integer_bits_cached(
            previous,
            bit_count,
            contexts,
            &mut interval,
            &mut code,
            &mut mq_bit_count,
        )?;
        self.interval = interval;
        self.code = code;
        self.bit_count = mq_bit_count;
        Ok(value)
    }
}
