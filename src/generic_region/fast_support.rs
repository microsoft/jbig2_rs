//! Shared Verus-verified helpers for direct packed-byte generic and refinement decoders.

use super::generic::RASTER_SIGNAL_CONTEXT;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out <= 255,
))]
#[inline]
pub(super) fn row_byte(row: &[u8], index: usize) -> u8 {
    if index < row.len() {
        row[index]
    } else {
        0
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        stride <= prev_rows@.len() || y == 0,
        row_start == y * stride,
        row_start == prev_rows@.len(),
        dst@.len() == stride,
        y > 0 ==> row_start >= stride,
    ensures
        final(dst)@.len() == stride,
))]
#[inline]
pub(super) fn copy_prev_row(
    prev_rows: &[u8],
    dst: &mut [u8],
    y: usize,
    stride: usize,
    row_start: usize,
) {
    if y != 0 {
        let src = &prev_rows[row_start - stride..row_start];
        let mut i = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                i <= stride,
                src@.len() == stride,
                dst@.len() == stride,
            decreases stride - i,
        ))]
        while i < stride {
            dst[i] = src[i];
            i += 1;
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
            bytes: old(decoder).bytes,
            byte_position: old(decoder).byte_position,
            current_byte: old(decoder).current_byte,
            code: *old(code),
            interval: *old(interval),
            bit_count: *old(bit_count),
            marker: old(decoder).marker,
        }),
        generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        out <= RASTER_SIGNAL_CONTEXT,
        final(contexts)@.len() == old(contexts)@.len(),
        generic_region_spec::contexts_valid(final(contexts)@),
        final(decoder).bytes@ == old(decoder).bytes@,
        final(decoder).bytes == old(decoder).bytes,
        out <= 1 ==> crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
            bytes: final(decoder).bytes,
            byte_position: final(decoder).byte_position,
            current_byte: final(decoder).current_byte,
            code: *final(code),
            interval: *final(interval),
            bit_count: *final(bit_count),
            marker: final(decoder).marker,
        }),
        out > 1 && out < RASTER_SIGNAL_CONTEXT ==>
            crate::arithmetic::arithmetic_spec::decoder_state_valid(final(decoder))
            && final(decoder).interval == *final(interval)
            && final(decoder).code == *final(code)
            && final(decoder).bit_count == *final(bit_count),
))]
#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn decode_ctx_cached(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
    ctx: usize,
    interval: &mut u32,
    code: &mut u32,
    bit_count: &mut u8,
) -> u8 {
    if ctx >= contexts.len() {
        return RASTER_SIGNAL_CONTEXT;
    }
    decoder.decode_bit_cached(&mut contexts[ctx], interval, code, bit_count)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: decoder.byte_position,
            current_byte: decoder.current_byte,
            code: *old(code),
            interval: *old(interval),
            bit_count,
            marker: decoder.marker,
        }),
        generic_region_spec::contexts_valid(contexts@),
        ctx < contexts@.len(),
        expected_mps <= 1,
    ensures
        crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
            bytes: decoder.bytes,
            byte_position: decoder.byte_position,
            current_byte: decoder.current_byte,
            code: *final(code),
            interval: *final(interval),
            bit_count,
            marker: decoder.marker,
        }),
))]
#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn try_skip_ctx_mps(
    decoder: &ArithmeticDecoder<'_>,
    contexts: &[ArithmeticContext],
    ctx: usize,
    expected_mps: u8,
    interval: &mut u32,
    code: &mut u32,
    bit_count: u8,
) -> bool {
    let _ = (decoder, bit_count);
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(crate::arithmetic::arithmetic_spec::decoder_state_valid);
        reveal(crate::arithmetic::arithmetic_spec::decoder_valid);
    }
    ArithmeticDecoder::try_skip_byte_mps(contexts[ctx], expected_mps, interval, code)
}
