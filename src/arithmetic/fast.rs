//! Register-cached arithmetic decoding fast paths and 8-pixel MPS byte skip.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{ArithmeticContext, ArithmeticDecoder, MIN_INTERVAL, NEXT_MPS_PACKED, QE_PACKED_VALS};

impl ArithmeticDecoder<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                bytes: old(self).bytes,
                byte_position: old(self).byte_position,
                current_byte: old(self).current_byte,
                code,
                interval,
                bit_count,
                marker: old(self).marker,
            }),
            arithmetic_spec::context_valid(*old(context)),
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::decoder_state_valid(final(self)),
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out <= 1 ==> arithmetic_spec::decoder_valid(final(self)),
            out <= 1 ==> out == arithmetic_spec::entry_decision(
                interval,
                code,
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
                interval, code, arithmetic_spec::context_index(*old(context)),
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
                interval,
                code,
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
            ({
                let snap = ArithmeticDecoder {
                    bytes: old(self).bytes,
                    byte_position: old(self).byte_position,
                    current_byte: old(self).current_byte,
                    code,
                    interval,
                    bit_count,
                    marker: old(self).marker,
                };
                let ds = arithmetic_spec::decision_state(&snap, *old(context));
                &&& out == ds.0
                &&& final(self).interval == ds.1
                &&& final(self).byte_position == ds.2
                &&& final(self).current_byte == ds.3
                &&& final(self).code == ds.4
                &&& final(self).bit_count == ds.5
                &&& final(self).marker == ds.6
            }),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[cold]
    #[inline(never)]
    pub(crate) fn decode_signal_slow_bc(
        &mut self,
        context: &mut ArithmeticContext,
        interval: u32,
        code: u32,
        bit_count: u8,
    ) -> u8 {
        self.interval = interval;
        self.code = code;
        self.bit_count = bit_count;
        self.decode_signal(context)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                bytes: old(self).bytes,
                byte_position: old(self).byte_position,
                current_byte: old(self).current_byte,
                code: *old(code),
                interval: *old(interval),
                bit_count: *old(bit_count),
                marker: old(self).marker,
            }),
            arithmetic_spec::context_valid(*old(context)),
        ensures
            out <= RASTER_SIGNAL_OVERFLOW,
            arithmetic_spec::context_valid(*final(context)),
            final(self).bytes@ == old(self).bytes@,
            final(self).bytes == old(self).bytes,
            out <= 1 ==> arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                bytes: final(self).bytes,
                byte_position: final(self).byte_position,
                current_byte: final(self).current_byte,
                code: *final(code),
                interval: *final(interval),
                bit_count: *final(bit_count),
                marker: final(self).marker,
            }),
            out > 1 ==> arithmetic_spec::decoder_state_valid(final(self))
                && final(self).interval == *final(interval)
                && final(self).code == *final(code)
                && final(self).bit_count == *final(bit_count),
            ({
                let snap = ArithmeticDecoder {
                    bytes: old(self).bytes,
                    byte_position: old(self).byte_position,
                    current_byte: old(self).current_byte,
                    code: *old(code),
                    interval: *old(interval),
                    bit_count: *old(bit_count),
                    marker: old(self).marker,
                };
                let ds = arithmetic_spec::decision_state(&snap, *old(context));
                &&& out == ds.0
                &&& *final(interval) == ds.1
                &&& final(self).byte_position == ds.2
                &&& final(self).current_byte == ds.3
                &&& *final(code) == ds.4
                &&& *final(bit_count) == ds.5
                &&& final(self).marker == ds.6
            }),
            arithmetic_spec::context_index(*final(context)) == arithmetic_spec::entry_index(
                *old(interval), *old(code), arithmetic_spec::context_index(*old(context)),
            ),
            arithmetic_spec::context_mps(*final(context)) == arithmetic_spec::entry_mps(
                *old(interval),
                *old(code),
                arithmetic_spec::context_index(*old(context)),
                arithmetic_spec::context_mps(*old(context)),
            ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::inline_always, dead_code)]
    #[inline(always)]
    pub(crate) fn decode_bit_cached(
        &mut self,
        context: &mut ArithmeticContext,
        interval: &mut u32,
        code: &mut u32,
        bit_count: &mut u8,
    ) -> u8 {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost snapshot = ArithmeticDecoder {
                bytes: self.bytes,
                byte_position: self.byte_position,
                current_byte: self.current_byte,
                code: *code,
                interval: *interval,
                bit_count: *bit_count,
                marker: self.marker,
            };
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(arithmetic_spec::decoder_state_valid);
            reveal(arithmetic_spec::decoder_valid);
            crate::arithmetic::arithmetic_shared::packed_context_bounds(*context);
            crate::arithmetic::arithmetic_shared::packed_tables_match(context.packed);
            arithmetic_spec::decision_model_equations(&snapshot, *context);
            arithmetic_spec::entry_model_equations(
                *interval,
                *code,
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
        let qe_shifted = qe << 16;
        let next_a = *interval - qe;
        let c = *code;
        if c >= qe_shifted {
            let next_c = c - qe_shifted;
            if next_a >= MIN_INTERVAL {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert((c >> 16u32) >= qe) by (bit_vector)
                        requires 0 < qe <= 0x5601u32, c >= (qe << 16u32);
                    assert((packed & 1u8) == packed % 2u8) by (bit_vector);
                }
                *interval = next_a;
                *code = next_c;
                return packed & 1;
            }
            if next_a >= 0x4000 && next_a >= qe && *bit_count != 0 && (next_c & 0x8000_0000) == 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::arithmetic::arithmetic_shared::inline_mps_renorm_matches(
                        &snapshot, *context, qe, next_a, next_c,
                    );
                    assert((packed & 1u8) == packed % 2u8) by (bit_vector);
                }
                *interval = next_a << 1;
                *code = next_c << 1;
                *bit_count -= 1;
                *context =
                    ArithmeticContext::from_packed(NEXT_MPS_PACKED[usize::from(packed) & 0x7f]);
                return packed & 1;
            }
        }
        let signal = self.decode_signal_slow_bc(context, *interval, c, *bit_count);
        *interval = self.interval;
        *code = self.code;
        *bit_count = self.bit_count;
        signal
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            MIN_INTERVAL <= *old(interval) <= 0xffff,
            arithmetic_spec::context_valid(context),
            expected_mps <= 1,
        ensures
            MIN_INTERVAL <= *final(interval) <= 0xffff,
            !out ==> *final(interval) == *old(interval) && *final(code) == *old(code),
            out ==> {
                let qe = QE_PACKED_VALS@[context.packed as int] as u32;
                let qe8 = qe * 8;
                &&& arithmetic_spec::context_mps(context) == expected_mps
                &&& (*old(interval) as int) >= (MIN_INTERVAL as int) + (qe8 as int)
                &&& (*old(code) >> 16) as int >= qe8 as int
                &&& *final(interval) as int == *old(interval) as int - qe8 as int
                &&& *final(code) as int == *old(code) as int - ((qe8 as int) * 65536)
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::inline_always, dead_code)]
    #[inline(always)]
    pub(crate) fn try_skip_byte_mps(
        context: ArithmeticContext,
        expected_mps: u8,
        interval: &mut u32,
        code: &mut u32,
    ) -> bool {
        let packed = context.packed();
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::arithmetic::arithmetic_shared::packed_context_bounds(context);
            crate::arithmetic::arithmetic_shared::packed_tables_match(packed);
            let p_usize = packed as usize;
            assert((p_usize & 0x7fusize) == p_usize) by (bit_vector)
                requires p_usize < 94usize;
            assert((packed & 1u8) == packed % 2u8) by (bit_vector);
        }
        if (packed & 1) != expected_mps {
            return false;
        }
        let qe = u32::from(QE_PACKED_VALS[usize::from(packed) & 0x7f]);
        let qe8 = qe << 3;
        let a = *interval;
        let c = *code;
        #[cfg(verus_keep_ghost)]
        proof! {
            assert((qe << 3u32) == mul(qe, 8u32) && (qe << 3u32) <= 0x2b008u32) by (bit_vector)
                requires 0 < qe <= 0x5601u32;
        }
        if a >= MIN_INTERVAL + qe8 && (c >> 16) >= qe8 {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(
                    qe8 <= 0x7fffu32
                    && ((qe8 << 16u32) as int) == (qe8 as int) * 65536
                    && c >= (qe8 << 16u32)
                ) by (bit_vector)
                    requires
                        qe8 <= 0x2b008u32,
                        a <= 0xffffu32,
                        a >= add(0x8000u32, qe8),
                        (c >> 16u32) >= qe8;
            }
            *interval = a - qe8;
            *code = c - (qe8 << 16);
            true
        } else {
            false
        }
    }
}
