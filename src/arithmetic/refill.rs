//! Refill implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(test)]
use super::{unexpected_eof, MalformedError};
#[cfg(not(verus_keep_ghost))]
use super::{
    ArithmeticDecoder, ArithmeticError, ArithmeticMarker, MIN_INTERVAL, RASTER_SIGNAL_EOF,
    RASTER_SIGNAL_OVERFLOW,
};

impl ArithmeticDecoder<'_> {
    /// Returns a recognized marker without consuming it.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == arithmetic_spec::decoder_marker(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn marker(&self) -> Option<ArithmeticMarker> {
        self.marker
    }

    /// Returns the offset of the current byte.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == arithmetic_spec::decoder_byte_position(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn byte_position(&self) -> usize {
        self.byte_position
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_state_valid(old(self)),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            match out {
                Ok(()) => arithmetic_spec::decoder_valid(final(self)),
                Err(_) => true,
            },
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out.is_ok() == (arithmetic_spec::normalized_state(old(self)).0 == 0),
            final(self).interval == arithmetic_spec::normalized_state(old(self)).1,
            final(self).byte_position == arithmetic_spec::normalized_state(old(self)).2,
            final(self).current_byte == arithmetic_spec::normalized_state(old(self)).3,
            final(self).code == arithmetic_spec::normalized_state(old(self)).4,
            final(self).bit_count == arithmetic_spec::normalized_state(old(self)).5,
            final(self).marker == arithmetic_spec::normalized_state(old(self)).6,
    ))]
    pub(super) fn renormalize(&mut self) -> Result<(), ArithmeticError> {
        let signal = self.renormalize_signal();
        if signal == 0 {
            Ok(())
        } else {
            Err(self.signal_error(signal))
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_state_valid(old(self)),
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            out == 0 || out == RASTER_SIGNAL_EOF || out == RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            out == 0 ==> arithmetic_spec::decoder_valid(final(self)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out == arithmetic_spec::normalized_state(old(self)).0,
            final(self).interval == arithmetic_spec::normalized_state(old(self)).1,
            final(self).byte_position == arithmetic_spec::normalized_state(old(self)).2,
            final(self).current_byte == arithmetic_spec::normalized_state(old(self)).3,
            final(self).code == arithmetic_spec::normalized_state(old(self)).4,
            final(self).bit_count == arithmetic_spec::normalized_state(old(self)).5,
            final(self).marker == arithmetic_spec::normalized_state(old(self)).6,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn renormalize_signal(&mut self) -> u8 {
        #[cfg(verus_keep_ghost)]
        production_reveal_decoder_valid!();
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                arithmetic_spec::decoder_state_valid(self),
                arithmetic_spec::decoder_state_valid(old(self)),
                self.bytes@ == old(self).bytes@,
                self.bytes == old(self).bytes,
                arithmetic_spec::normalized_state(old(self))
                    == arithmetic_spec::normalized_state(self),
            decreases arithmetic_spec::renormalization_steps(
                self.interval as nat,
            ),
        ))]
        while self.interval < MIN_INTERVAL {
            #[cfg(verus_keep_ghost)]
            proof_decl! {
                let ghost previous_interval = self.interval;
                let ghost before = *self;
            }
            if self.interval < MIN_INTERVAL / 2 && self.bit_count >= 2 && self.code <= u32::MAX / 4
            {
                #[cfg(verus_keep_ghost)]
                proof_decl! {
                    let ghost middle = ArithmeticDecoder {
                        bytes: before.bytes,
                        byte_position: before.byte_position,
                        current_byte: before.current_byte,
                        code: (before.code * 2) as u32,
                        interval: (before.interval * 2) as u32,
                        bit_count: (before.bit_count - 1) as u8,
                        marker: before.marker,
                    };
                }
                self.interval *= 4;
                self.code *= 4;
                self.bit_count -= 2;
                #[cfg(verus_keep_ghost)]
                proof! {
                    reveal_with_fuel(arithmetic_spec::normalized_state, 2);
                    assert(arithmetic_spec::normalized_state(&before)
                        == arithmetic_spec::normalized_state(&middle));
                    assert(arithmetic_spec::normalized_state(&middle)
                        == arithmetic_spec::normalized_state(self));
                    reveal_with_fuel(arithmetic_spec::renormalization_steps, 2);
                    assert(arithmetic_spec::renormalization_steps(self.interval as nat)
                        < arithmetic_spec::renormalization_steps(before.interval as nat));
                }
                continue;
            }
            if self.bit_count == 0 {
                let signal = self.byte_in_signal();
                if signal != 0 {
                    return signal;
                }
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(self.interval < 0x8000u32);
                assert(self.interval <= u32::MAX / 2);
                assert(self.interval * 2 <= 0xffffu32);
                assert(self.bit_count > 0);
                assert(self.bit_count <= 8);
                assert(
                    MIN_INTERVAL - self.interval * 2
                        <                         MIN_INTERVAL - self.interval
                );
            }
            self.interval *= 2;
            let Some(code) = self.code.checked_mul(2) else {
                return RASTER_SIGNAL_OVERFLOW;
            };
            self.code = code;
            self.bit_count -= 1;
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal_with_fuel(arithmetic_spec::normalized_state, 1);
                assert(
                    arithmetic_spec::normalized_state(&before)
                        == arithmetic_spec::normalized_state(self)
                );
                assert(self.interval == previous_interval * 2);
                reveal(arithmetic_spec::renormalization_steps);
                assert(
                    arithmetic_spec::renormalization_steps(
                        self.interval as nat,
                    ) < arithmetic_spec::renormalization_steps(
                        previous_interval as nat,
                    )
                );
            }
        }
        0
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_state_valid(old(self)),
        ensures
            arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes@ == old(self).bytes@,
            final(self).interval == old(self).interval,
            final(self).byte_position == old(self).byte_position
                || final(self).byte_position == old(self).byte_position + 1,
            final(self).bytes == old(self).bytes,
            out.is_ok() == (arithmetic_spec::refill_step(old(self)).0 == 0),
            final(self).byte_position == arithmetic_spec::refill_step(old(self)).1,
            final(self).current_byte == arithmetic_spec::refill_step(old(self)).2,
            final(self).code == arithmetic_spec::refill_step(old(self)).3,
            final(self).bit_count == arithmetic_spec::refill_step(old(self)).4,
            final(self).marker == arithmetic_spec::refill_step(old(self)).5,
            match out {
                Ok(()) =>
                    old(self).byte_position + 1 < old(self).bytes@.len()
                    && final(self).bit_count >= 7,
                Err(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn byte_in(&mut self) -> Result<(), ArithmeticError> {
        let signal = self.byte_in_signal();
        if signal == 0 {
            Ok(())
        } else {
            Err(self.signal_error(signal))
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires arithmetic_spec::decoder_state_valid(old(self)),
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            out == 0 || out == RASTER_SIGNAL_EOF || out == RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            final(self).bytes@ == old(self).bytes@,
            final(self).interval == old(self).interval,
            final(self).byte_position == old(self).byte_position
                || final(self).byte_position == old(self).byte_position + 1,
            final(self).bytes == old(self).bytes,
            out == arithmetic_spec::refill_step(old(self)).0,
            final(self).byte_position == arithmetic_spec::refill_step(old(self)).1,
            final(self).current_byte == arithmetic_spec::refill_step(old(self)).2,
            final(self).code == arithmetic_spec::refill_step(old(self)).3,
            final(self).bit_count == arithmetic_spec::refill_step(old(self)).4,
            final(self).marker == arithmetic_spec::refill_step(old(self)).5,
            out == 0 ==>
                    old(self).byte_position + 1 < old(self).bytes@.len()
                    && final(self).bit_count >= 7
                    && (
                        old(self).marker.is_none()
                            && final(self).marker.is_some()
                        ==> final(self).byte_position
                            == old(self).byte_position
                    ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn byte_in_signal(&mut self) -> u8 {
        #[cfg(verus_keep_ghost)]
        production_reveal_decoder_valid!();
        let next_position = self.byte_position + 1;
        if next_position >= self.bytes.len() {
            return RASTER_SIGNAL_EOF;
        }
        let next_byte = self.bytes[next_position];

        if self.current_byte == 0xff {
            if next_byte > 0x8f {
                let Some(code) = self.code.checked_add(0xff00) else {
                    return RASTER_SIGNAL_OVERFLOW;
                };
                self.code = code;
                self.bit_count = 8;
                self.marker = Some(ArithmeticMarker {
                    offset: self.byte_position,
                    code: next_byte,
                });
                return 0;
            }

            self.byte_position = next_position;
            self.current_byte = next_byte;
            let Some(code) = self.code.checked_add(u32::from(next_byte) << 9) else {
                return RASTER_SIGNAL_OVERFLOW;
            };
            self.code = code;
            self.bit_count = 7;
            return 0;
        }

        self.byte_position = next_position;
        self.current_byte = next_byte;
        let Some(code) = self.code.checked_add(u32::from(next_byte) << 8) else {
            return RASTER_SIGNAL_OVERFLOW;
        };
        self.code = code;
        self.bit_count = 8;
        0
    }

    #[cfg(test)]
    pub(super) fn renormalize_checked_oracle(&mut self) -> Result<(), ArithmeticError> {
        while self.interval < MIN_INTERVAL {
            if self.bit_count == 0 {
                self.byte_in_checked_oracle()?;
            }
            self.interval = self
                .interval
                .checked_mul(2)
                .ok_or(MalformedError::IntegerOverflow)?;
            self.code = self
                .code
                .checked_mul(2)
                .ok_or(MalformedError::IntegerOverflow)?;
            self.bit_count = self
                .bit_count
                .checked_sub(1)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn byte_in_checked_oracle(&mut self) -> Result<(), ArithmeticError> {
        let next_position = self
            .byte_position
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        if next_position >= self.bytes.len() {
            return Err(unexpected_eof(next_position, 1, 0));
        }
        let next_byte = self.bytes[next_position];

        if self.current_byte == 0xff {
            if next_byte > 0x8f {
                self.code = self
                    .code
                    .checked_add(0xff00)
                    .ok_or(MalformedError::IntegerOverflow)?;
                self.bit_count = 8;
                self.marker = Some(ArithmeticMarker {
                    offset: self.byte_position,
                    code: next_byte,
                });
                return Ok(());
            }

            self.byte_position = next_position;
            self.current_byte = next_byte;
            self.code = self
                .code
                .checked_add(u32::from(next_byte) << 9)
                .ok_or(MalformedError::IntegerOverflow)?;
            self.bit_count = 7;
            return Ok(());
        }

        self.byte_position = next_position;
        self.current_byte = next_byte;
        self.code = self
            .code
            .checked_add(u32::from(next_byte) << 8)
            .ok_or(MalformedError::IntegerOverflow)?;
        self.bit_count = 8;
        Ok(())
    }
}
