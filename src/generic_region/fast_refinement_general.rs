//! Direct packed-byte general Template 0 generic-refinement row decoder.

use super::fast_refinement::ref_vbyte;
use super::fast_support::{decode_ctx_cached, row_byte};
use super::generic::probability_signal_error;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{GenericRegionError, RefinementParameters, TemplateOffset};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        prev_rows@.len() == (y as usize) * stride,
        dst@.len() == stride,
        b <= stride,
        x < width,
        (width as nat) <= (stride as nat) * 8,
    ensures out <= 1,
))]
#[allow(
    clippy::too_many_arguments,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::bool_to_int_with_if
)]
#[inline]
fn sample_target_at(
    prev_rows: &[u8],
    dst: &[u8],
    t0: u32,
    b: usize,
    x: u32,
    y: u32,
    width: u32,
    stride: usize,
    at: TemplateOffset,
) -> u32 {
    let ay = i64::from(y) + i64::from(at.y);
    let ax = i64::from(x) + i64::from(at.x);
    if ax < 0 || ax >= i64::from(width) || ay < 0 || ay > i64::from(y) {
        return 0;
    }
    if at.y == 0 {
        if at.x >= 0 {
            return 0;
        }
        let dist_i64 = -i64::from(at.x);
        if dist_i64 <= 32 {
            let dist = dist_i64 as u32;
            return if ((t0 >> (dist - 1)) & 1) != 0 { 1 } else { 0 };
        }
        let uax = ax as usize;
        let ab = uax / 8;
        if ab < b {
            return if ((dst[ab] >> (7 - (uax % 8))) & 1) != 0 {
                1
            } else {
                0
            };
        }
        return 0;
    }
    let uax = ax as usize;
    let uay = ay as usize;
    let byte_col = uax / 8;
    if let Some(row_off) = uay.checked_mul(stride) {
        if let Some(idx) = row_off.checked_add(byte_col) {
            if idx < prev_rows.len() {
                return if ((prev_rows[idx] >> (7 - (uax % 8))) & 1) != 0 {
                    1
                } else {
                    0
                };
            }
        }
    }
    0
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(reference),
    ensures out <= 1,
))]
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::bool_to_int_with_if,
    clippy::similar_names
)]
#[inline]
fn sample_ref_at(
    reference: &Bitmap,
    x: u32,
    y: u32,
    parameters: RefinementParameters,
    at: TemplateOffset,
) -> u32 {
    let rx = i64::from(x) - i64::from(parameters.reference_dx) + i64::from(at.x);
    let ry = i64::from(y) - i64::from(parameters.reference_dy) + i64::from(at.y);
    if rx < 0 || rx >= i64::from(reference.width) || ry < 0 || ry >= i64::from(reference.height) {
        return 0;
    }
    let urx = rx as usize;
    let ury = ry as usize;
    let stride = reference.stride;
    if let Some(row_off) = ury.checked_mul(stride) {
        if let Some(idx) = row_off.checked_add(urx / 8) {
            if idx < reference.data.len() {
                return if ((reference.data[idx] >> (7 - (urx % 8))) & 1) != 0 {
                    1
                } else {
                    0
                };
            }
        }
    }
    0
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        prev_rows@.len() == (y_u32 as usize) * stride,
        dst@.len() == stride,
        crate::spec::bitmap::storage_valid(reference),
        ref_stride == row_rp@.len() || row_rp@.len() == 0,
        ref_stride == row_rc@.len() || row_rc@.len() == 0,
        ref_stride == row_rn@.len() || row_rn@.len() == 0,
        -1_000_000_000i64 <= base_rb <= 1_000_000_000i64,
        rs < 8,
        full_bytes == (width as usize) / 8,
        tail_bits == (width as usize) % 8,
        full_bytes <= stride,
        tail_bits == 0 || full_bytes < stride,
        (width as nat) <= (stride as nat) * 8,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == 8_192usize,
    ensures
        final(dst)@.len() == stride,
        final(contexts)@.len() == 8_192usize,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::similar_names
)]
pub(super) fn decode_row_general_t0_ref(
    prev_rows: &[u8],
    row_t1: &[u8],
    reference: &Bitmap,
    row_rp: &[u8],
    row_rc: &[u8],
    row_rn: &[u8],
    dst: &mut [u8],
    y_u32: u32,
    width: u32,
    stride: usize,
    base_rb: i64,
    rs: u32,
    ref_stride: usize,
    ref_tail_mask: u8,
    full_bytes: usize,
    tail_bits: usize,
    ltp: bool,
    parameters: RefinementParameters,
    target_at: TemplateOffset,
    ref_at: TemplateOffset,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let mut interval = decoder.interval;
    let mut code = decoder.code;
    let mut bit_count = decoder.bit_count;
    let mut t1 = u32::from(row_byte(row_t1, 0)) << 8;
    let mut t0 = 0_u32;
    let mut rp = (u32::from(ref_vbyte(
        row_rp,
        base_rb - 1,
        rs,
        ref_stride,
        ref_tail_mask,
    )) << 16)
        | (u32::from(ref_vbyte(row_rp, base_rb, rs, ref_stride, ref_tail_mask)) << 8);
    let mut rc = (u32::from(ref_vbyte(
        row_rc,
        base_rb - 1,
        rs,
        ref_stride,
        ref_tail_mask,
    )) << 16)
        | (u32::from(ref_vbyte(row_rc, base_rb, rs, ref_stride, ref_tail_mask)) << 8);
    let mut rn = (u32::from(ref_vbyte(
        row_rn,
        base_rb - 1,
        rs,
        ref_stride,
        ref_tail_mask,
    )) << 16)
        | (u32::from(ref_vbyte(row_rn, base_rb, rs, ref_stride, ref_tail_mask)) << 8);

    let mut b = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            b <= full_bytes,
            full_bytes <= stride,
            dst@.len() == stride,
            prev_rows@.len() == (y_u32 as usize) * stride,
            crate::spec::bitmap::storage_valid(reference),
            ref_stride == row_rp@.len() || row_rp@.len() == 0,
            ref_stride == row_rc@.len() || row_rc@.len() == 0,
            ref_stride == row_rn@.len() || row_rn@.len() == 0,
            -1_000_000_000i64 <= base_rb <= 1_000_000_000i64,
            rs < 8,
            full_bytes == (width as usize) / 8,
            tail_bits == (width as usize) % 8,
            tail_bits < 8,
            tail_bits == 0 || full_bytes < stride,
            (width as nat) <= (stride as nat) * 8,
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
            contexts@.len() == 8_192usize,
        decreases full_bytes - b,
    ))]
    while b < full_bytes {
        t1 |= u32::from(row_byte(row_t1, b + 1));
        let next_rb = base_rb + (b as i64) + 1;
        rp |= u32::from(ref_vbyte(row_rp, next_rb, rs, ref_stride, ref_tail_mask));
        rc |= u32::from(ref_vbyte(row_rc, next_rb, rs, ref_stride, ref_tail_mask));
        rn |= u32::from(ref_vbyte(row_rn, next_rb, rs, ref_stride, ref_tail_mask));
        if ltp {
            if (rp & 0x0001_ff80) == 0 && (rc & 0x0001_ff80) == 0 && (rn & 0x0001_ff80) == 0 {
                dst[b] = 0;
                t0 <<= 8;
                t1 <<= 8;
                rp <<= 8;
                rc <<= 8;
                rn <<= 8;
                b += 1;
                continue;
            }
            if (rp & 0x0001_ff80) == 0x0001_ff80
                && (rc & 0x0001_ff80) == 0x0001_ff80
                && (rn & 0x0001_ff80) == 0x0001_ff80
            {
                dst[b] = 0xff;
                t0 = (t0 << 8) | 0xff;
                t1 <<= 8;
                rp <<= 8;
                rc <<= 8;
                rn <<= 8;
                b += 1;
                continue;
            }
        }
        let cur_x = (b * 8) as u32;
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= 8,
                b < full_bytes,
                full_bytes <= stride,
                dst@.len() == stride,
                prev_rows@.len() == (y_u32 as usize) * stride,
                crate::spec::bitmap::storage_valid(reference),
                cur_x as usize == b * 8,
                (cur_x as usize) + 8 <= width as usize,
                (width as nat) <= (stride as nat) * 8,
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
                contexts@.len() == 8_192usize,
            decreases 8 - k,
        ))]
        while k < 8 {
            let bit: u32 = if ltp {
                let r9 = (rp & 0x0001_c000) | ((rc & 0x0001_c000) >> 3) | ((rn & 0x0001_c000) >> 6);
                if r9 == 0 {
                    0
                } else if r9 == 0x0001_ff00 {
                    1
                } else {
                    let x = cur_x + (k as u32);
                    let ta =
                        sample_target_at(prev_rows, dst, t0, b, x, y_u32, width, stride, target_at);
                    let ra = sample_ref_at(reference, x, y_u32, parameters, ref_at);
                    let ctx = ((ta << 12)
                        | ((t1 >> 4) & 0x0c00)
                        | ((t0 & 1) << 9)
                        | (ra << 8)
                        | ((rp >> 8) & 0x00c0)
                        | ((rc >> 11) & 0x0038)
                        | ((rn >> 14) & 0x0007)) as usize;
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
                    u32::from(sig)
                }
            } else {
                let x = cur_x + (k as u32);
                let ta =
                    sample_target_at(prev_rows, dst, t0, b, x, y_u32, width, stride, target_at);
                let ra = sample_ref_at(reference, x, y_u32, parameters, ref_at);
                let ctx = ((ta << 12)
                    | ((t1 >> 4) & 0x0c00)
                    | ((t0 & 1) << 9)
                    | (ra << 8)
                    | ((rp >> 8) & 0x00c0)
                    | ((rc >> 11) & 0x0038)
                    | ((rn >> 14) & 0x0007)) as usize;
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
                u32::from(sig)
            };
            t0 = (t0 << 1) | bit;
            t1 <<= 1;
            rp <<= 1;
            rc <<= 1;
            rn <<= 1;
            k += 1;
        }
        dst[b] = t0 as u8;
        b += 1;
    }

    if tail_bits != 0 {
        let next_rb = base_rb + (full_bytes as i64) + 1;
        rp |= u32::from(ref_vbyte(row_rp, next_rb, rs, ref_stride, ref_tail_mask));
        rc |= u32::from(ref_vbyte(row_rc, next_rb, rs, ref_stride, ref_tail_mask));
        rn |= u32::from(ref_vbyte(row_rn, next_rb, rs, ref_stride, ref_tail_mask));
        let cur_x = (full_bytes * 8) as u32;
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= tail_bits,
                tail_bits < 8,
                full_bytes < stride,
                dst@.len() == stride,
                prev_rows@.len() == (y_u32 as usize) * stride,
                crate::spec::bitmap::storage_valid(reference),
                cur_x as usize == full_bytes * 8,
                (cur_x as usize) + tail_bits == width as usize,
                (width as nat) <= (stride as nat) * 8,
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
                contexts@.len() == 8_192usize,
            decreases tail_bits - k,
        ))]
        while k < tail_bits {
            let bit: u32 = if ltp {
                let r9 = (rp & 0x0001_c000) | ((rc & 0x0001_c000) >> 3) | ((rn & 0x0001_c000) >> 6);
                if r9 == 0 {
                    0
                } else if r9 == 0x0001_ff00 {
                    1
                } else {
                    let x = cur_x + (k as u32);
                    let ta = sample_target_at(
                        prev_rows, dst, t0, full_bytes, x, y_u32, width, stride, target_at,
                    );
                    let ra = sample_ref_at(reference, x, y_u32, parameters, ref_at);
                    let ctx = ((ta << 12)
                        | ((t1 >> 4) & 0x0c00)
                        | ((t0 & 1) << 9)
                        | (ra << 8)
                        | ((rp >> 8) & 0x00c0)
                        | ((rc >> 11) & 0x0038)
                        | ((rn >> 14) & 0x0007)) as usize;
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
                    u32::from(sig)
                }
            } else {
                let x = cur_x + (k as u32);
                let ta = sample_target_at(
                    prev_rows, dst, t0, full_bytes, x, y_u32, width, stride, target_at,
                );
                let ra = sample_ref_at(reference, x, y_u32, parameters, ref_at);
                let ctx = ((ta << 12)
                    | ((t1 >> 4) & 0x0c00)
                    | ((t0 & 1) << 9)
                    | (ra << 8)
                    | ((rp >> 8) & 0x00c0)
                    | ((rc >> 11) & 0x0038)
                    | ((rn >> 14) & 0x0007)) as usize;
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
                u32::from(sig)
            };
            t0 = (t0 << 1) | bit;
            t1 <<= 1;
            rp <<= 1;
            rc <<= 1;
            rn <<= 1;
            k += 1;
        }
        let tail_mask = 0xffu32 >> (8 - tail_bits);
        dst[full_bytes] = ((t0 & tail_mask) << (8 - tail_bits)) as u8;
    }

    decoder.interval = interval;
    decoder.code = code;
    decoder.bit_count = bit_count;
    Ok(())
}
