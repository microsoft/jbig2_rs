//! Direct packed-byte Template 0 decoder for pattern-dictionary (`AT0 = (-pw, 0)`) layouts.

use super::fast_support::{copy_prev_row, decode_ctx_cached, row_byte, try_skip_ctx_mps};
use super::generic::probability_signal_error;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{GenericRegionError, GenericTemplate};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        1 <= pattern_width <= 128,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == 65_536usize,
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation
)]
pub(super) fn decode_t0_pattern(
    bitmap: &mut Bitmap,
    rows: u32,
    pattern_width: usize,
    typical_prediction: bool,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let width = bitmap.width;
    #[allow(unused_variables)]
    let height = bitmap.height;
    let stride = bitmap.stride;
    let rows_usize = rows as usize;
    let full_bytes = (width as usize) / 8;
    let tail_bits = (width as usize) % 8;
    let sltp_ctx = GenericTemplate::Template0.sltp_context();
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        assert(full_bytes <= stride);
        assert(tail_bits == 0 || full_bytes < stride);
    }

    let mut ltp = false;
    let mut y = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= rows_usize,
            rows_usize == rows as usize,
            rows <= height,
            1 <= pattern_width <= 128,
            bitmap.width == width,
            bitmap.height == height,
            bitmap.stride == stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
            contexts@.len() == 65_536usize,
            sltp_ctx < contexts@.len(),
            full_bytes == (width as usize) / 8,
            tail_bits == (width as usize) % 8,
            full_bytes <= stride,
            tail_bits == 0 || full_bytes < stride,
        decreases rows_usize - y,
    ))]
    while y < rows_usize {
        if typical_prediction {
            let sltp = decoder.decode_signal(&mut contexts[sltp_ctx]);
            if sltp > 1 {
                return Err(probability_signal_error(
                    decoder,
                    sltp_ctx,
                    contexts.len(),
                    sltp,
                ));
            }
            ltp ^= sltp != 0;
        }
        if stride == 0 {
            y += 1;
            continue;
        }
        let data_len = bitmap.data.len();
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(bitmap);
            assert(y * stride + stride <= data_len) by (nonlinear_arith)
                requires
                    y < rows_usize,
                    rows_usize <= height as usize,
                    data_len == stride * (height as usize);
        }
        let _ = data_len;
        let row_start = y * stride;
        #[cfg(verus_keep_ghost)]
        proof! {
            if y > 0 {
                assert(row_start >= stride) by (nonlinear_arith)
                    requires y >= 1, row_start == y * stride;
            }
            if y >= 2 {
                assert(row_start >= 2 * stride) by (nonlinear_arith)
                    requires y >= 2, row_start == y * stride;
            }
        }
        let (prev_rows, cur_and_after) = bitmap.data.as_mut_slice().split_at_mut(row_start);
        let dst = &mut cur_and_after[..stride];
        if ltp {
            copy_prev_row(prev_rows, dst, y, stride, row_start);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(bitmap);
            }
            y += 1;
            continue;
        }
        let row_m1: &[u8] = if y >= 1 {
            &prev_rows[row_start - stride..row_start]
        } else {
            &[]
        };
        let row_m2: &[u8] = if y >= 2 {
            &prev_rows[row_start - 2 * stride..row_start - stride]
        } else {
            &[]
        };
        let row_res = decode_row_t0_pattern(
            row_m2,
            row_m1,
            dst,
            pattern_width,
            full_bytes,
            tail_bits,
            decoder,
            contexts,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(bitmap);
        }
        row_res?;
        y += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        1 <= pattern_width <= 128,
        dst@.len() >= full_bytes,
        tail_bits < 8,
        tail_bits == 0 || full_bytes < dst@.len(),
        full_bytes <= (u32::MAX as usize) / 8,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == 65_536usize,
    ensures
        final(dst)@.len() == old(dst)@.len(),
        final(contexts)@.len() == 65_536usize,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::verbose_bit_mask
)]
fn decode_row_t0_pattern(
    row_m2: &[u8],
    row_m1: &[u8],
    dst: &mut [u8],
    pattern_width: usize,
    full_bytes: usize,
    tail_bits: usize,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let mut interval = decoder.interval;
    let mut code = decoder.code;
    let mut bit_count = decoder.bit_count;
    let mut l2 = u32::from(row_byte(row_m2, 0)) << 8;
    let mut l1 = u32::from(row_byte(row_m1, 0)) << 8;
    let mut l0 = 0_u32;
    let mut b = 0_usize;

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            b <= full_bytes,
            dst@.len() == old(dst)@.len(),
            full_bytes <= dst@.len(),
            full_bytes <= (u32::MAX as usize) / 8,
            tail_bits < 8,
            tail_bits == 0 || full_bytes < dst@.len(),
            1 <= pattern_width <= 128,
            crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                bytes: decoder.bytes,
                byte_position: decoder.byte_position,
                current_byte: decoder.current_byte,
                code,
                interval,
                bit_count,
                marker: decoder.marker,
            }),
            generic_region_spec::contexts_valid(contexts@),
            contexts@.len() == 65_536usize,
        decreases full_bytes - b,
    ))]
    while b < full_bytes {
        l2 |= u32::from(row_byte(row_m2, b + 1));
        l1 |= u32::from(row_byte(row_m1, b + 1));
        let cur_x = b * 8;
        if pattern_width <= 24 {
            let pw_mask = 0x00ff_ffffu32 >> (24 - pattern_width);
            let all_zero = (l2 & 0x0003_ffc0) == 0
                && (l1 & 0x0007_ffc0) == 0
                && (l0 & 0x000f) == 0
                && (l0 & pw_mask) == 0;
            if all_zero
                && try_skip_ctx_mps(decoder, contexts, 0, 0, &mut interval, &mut code, bit_count)
            {
                dst[b] = 0;
                l0 <<= 8;
                l1 <<= 8;
                l2 <<= 8;
                b += 1;
                continue;
            }
            let all_one = cur_x >= pattern_width
                && (l2 & 0x0003_ffc0) == 0x0003_ffc0
                && (l1 & 0x0007_ffc0) == 0x0007_ffc0
                && (l0 & 0x000f) == 0x000f
                && (l0 & pw_mask) == pw_mask;
            if all_one
                && try_skip_ctx_mps(
                    decoder,
                    contexts,
                    0xffff,
                    1,
                    &mut interval,
                    &mut code,
                    bit_count,
                )
            {
                dst[b] = 0xff;
                l0 = (l0 << 8) | 0xff;
                l1 <<= 8;
                l2 <<= 8;
                b += 1;
                continue;
            }
            let pw_shift = pattern_width - 1;
            let mut k = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    k <= 8,
                    b < full_bytes,
                    dst@.len() == old(dst)@.len(),
                    full_bytes <= dst@.len(),
                    pw_shift < 24,
                    crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                        bytes: decoder.bytes,
                        byte_position: decoder.byte_position,
                        current_byte: decoder.current_byte,
                        code,
                        interval,
                        bit_count,
                        marker: decoder.marker,
                    }),
                    generic_region_spec::contexts_valid(contexts@),
                    contexts@.len() == 65_536usize,
                decreases 8 - k,
            ))]
            while k < 8 {
                let at0 = ((l0 >> pw_shift) & 1) << 4;
                let ctx =
                    (((l2 >> 2) & 0xf800) | ((l1 >> 8) & 0x07e0) | at0 | (l0 & 0x000f)) as usize;
                let sig = decode_ctx_cached(
                    decoder,
                    contexts,
                    ctx,
                    &mut interval,
                    &mut code,
                    &mut bit_count,
                );
                if sig > 1 {
                    return Err(probability_signal_error(decoder, ctx, contexts.len(), sig));
                }
                l0 = (l0 << 1) | u32::from(sig);
                l1 <<= 1;
                l2 <<= 1;
                k += 1;
            }
            dst[b] = l0 as u8;
        } else if cur_x >= pattern_width {
            let px = cur_x - pattern_width;
            let pb = px / 8;
            let ps = px % 8;
            let b0 = u32::from(dst[pb]);
            let b1 = if pb + 1 < b {
                u32::from(dst[pb + 1])
            } else {
                0
            };
            let pbyte = (((b0 << 8) | b1) >> (8 - ps)) as u8;
            if pbyte == 0
                && (l2 & 0x0003_ffc0) == 0
                && (l1 & 0x0007_ffc0) == 0
                && (l0 & 0x000f) == 0
                && try_skip_ctx_mps(decoder, contexts, 0, 0, &mut interval, &mut code, bit_count)
            {
                dst[b] = 0;
                l0 <<= 8;
                l1 <<= 8;
                l2 <<= 8;
                b += 1;
                continue;
            }
            if pbyte == 0xff
                && (l2 & 0x0003_ffc0) == 0x0003_ffc0
                && (l1 & 0x0007_ffc0) == 0x0007_ffc0
                && (l0 & 0x000f) == 0x000f
                && try_skip_ctx_mps(
                    decoder,
                    contexts,
                    0xffff,
                    1,
                    &mut interval,
                    &mut code,
                    bit_count,
                )
            {
                dst[b] = 0xff;
                l0 = (l0 << 8) | 0xff;
                l1 <<= 8;
                l2 <<= 8;
                b += 1;
                continue;
            }
            let mut k = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    k <= 8,
                    b < full_bytes,
                    dst@.len() == old(dst)@.len(),
                    full_bytes <= dst@.len(),
                    crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                        bytes: decoder.bytes,
                        byte_position: decoder.byte_position,
                        current_byte: decoder.current_byte,
                        code,
                        interval,
                        bit_count,
                        marker: decoder.marker,
                    }),
                    generic_region_spec::contexts_valid(contexts@),
                    contexts@.len() == 65_536usize,
                decreases 8 - k,
            ))]
            while k < 8 {
                let at0 = u32::from((pbyte >> (7 - k)) & 1) << 4;
                let ctx =
                    (((l2 >> 2) & 0xf800) | ((l1 >> 8) & 0x07e0) | at0 | (l0 & 0x000f)) as usize;
                let sig = decode_ctx_cached(
                    decoder,
                    contexts,
                    ctx,
                    &mut interval,
                    &mut code,
                    &mut bit_count,
                );
                if sig > 1 {
                    return Err(probability_signal_error(decoder, ctx, contexts.len(), sig));
                }
                l0 = (l0 << 1) | u32::from(sig);
                l1 <<= 1;
                l2 <<= 1;
                k += 1;
            }
            dst[b] = l0 as u8;
        } else {
            let mut k = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    k <= 8,
                    b < full_bytes,
                    dst@.len() == old(dst)@.len(),
                    full_bytes <= dst@.len(),
                    cur_x == b * 8,
                    cur_x < pattern_width,
                    24 < pattern_width <= 128,
                    crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                        bytes: decoder.bytes,
                        byte_position: decoder.byte_position,
                        current_byte: decoder.current_byte,
                        code,
                        interval,
                        bit_count,
                        marker: decoder.marker,
                    }),
                    generic_region_spec::contexts_valid(contexts@),
                    contexts@.len() == 65_536usize,
                decreases 8 - k,
            ))]
            while k < 8 {
                let x = cur_x + k;
                let at0 = if x >= pattern_width {
                    let qx = x - pattern_width;
                    let qb = qx / 8;
                    if qb < b {
                        u32::from((dst[qb] >> (7 - (qx % 8))) & 1) << 4
                    } else {
                        0
                    }
                } else {
                    0
                };
                let ctx =
                    (((l2 >> 2) & 0xf800) | ((l1 >> 8) & 0x07e0) | at0 | (l0 & 0x000f)) as usize;
                let sig = decode_ctx_cached(
                    decoder,
                    contexts,
                    ctx,
                    &mut interval,
                    &mut code,
                    &mut bit_count,
                );
                if sig > 1 {
                    return Err(probability_signal_error(decoder, ctx, contexts.len(), sig));
                }
                l0 = (l0 << 1) | u32::from(sig);
                l1 <<= 1;
                l2 <<= 1;
                k += 1;
            }
            dst[b] = l0 as u8;
        }
        b += 1;
    }

    if tail_bits != 0 {
        let cur_x = full_bytes * 8;
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= tail_bits,
                tail_bits < 8,
                dst@.len() == old(dst)@.len(),
                full_bytes < dst@.len(),
                full_bytes <= (u32::MAX as usize) / 8,
                cur_x == full_bytes * 8,
                1 <= pattern_width <= 128,
                crate::arithmetic::arithmetic_spec::decoder_valid(&ArithmeticDecoder {
                    bytes: decoder.bytes,
                    byte_position: decoder.byte_position,
                    current_byte: decoder.current_byte,
                    code,
                    interval,
                    bit_count,
                    marker: decoder.marker,
                }),
                generic_region_spec::contexts_valid(contexts@),
                contexts@.len() == 65_536usize,
            decreases tail_bits - k,
        ))]
        while k < tail_bits {
            let x = cur_x + k;
            let at0 = if pattern_width <= 24 {
                ((l0 >> (pattern_width - 1)) & 1) << 4
            } else if x >= pattern_width {
                let qx = x - pattern_width;
                let qb = qx / 8;
                if qb < full_bytes {
                    u32::from((dst[qb] >> (7 - (qx % 8))) & 1) << 4
                } else {
                    0
                }
            } else {
                0
            };
            let ctx = (((l2 >> 2) & 0xf800) | ((l1 >> 8) & 0x07e0) | at0 | (l0 & 0x000f)) as usize;
            let sig = decode_ctx_cached(
                decoder,
                contexts,
                ctx,
                &mut interval,
                &mut code,
                &mut bit_count,
            );
            if sig > 1 {
                return Err(probability_signal_error(decoder, ctx, contexts.len(), sig));
            }
            l0 = (l0 << 1) | u32::from(sig);
            l1 <<= 1;
            l2 <<= 1;
            k += 1;
        }
        let tail_mask = 0xffu32 >> (8 - tail_bits);
        dst[full_bytes] = ((l0 & tail_mask) << (8 - tail_bits)) as u8;
    }

    decoder.interval = interval;
    decoder.code = code;
    decoder.bit_count = bit_count;
    Ok(())
}
