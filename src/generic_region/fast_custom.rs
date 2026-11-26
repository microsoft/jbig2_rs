//! Direct packed-byte Template 0 decoder for general 4-AT layouts.

use super::fast_support::{copy_prev_row, decode_ctx_cached, row_byte};
use super::generic::probability_signal_error;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{GenericRegionError, GenericTemplate, TemplateOffset};
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
fn sample_at_pixel(
    prev_rows: &[u8],
    dst: &[u8],
    l0: u32,
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
            return if ((l0 >> (dist - 1)) & 1) != 0 { 1 } else { 0 };
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
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == 65_536usize,
        match skip {
            Some(mask) =>
                crate::spec::bitmap::storage_valid(mask)
                && mask.width == old(bitmap).width
                && mask.height == old(bitmap).height,
            None => true,
        },
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
pub(super) fn decode_t0_custom(
    bitmap: &mut Bitmap,
    rows: u32,
    adaptive: &[TemplateOffset; 12],
    typical_prediction: bool,
    skip: Option<&Bitmap>,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let width = bitmap.width;
    let width_usize = width as usize;
    #[allow(unused_variables)]
    let height = bitmap.height;
    let stride = bitmap.stride;
    let rows_usize = rows as usize;
    let full_bytes = width_usize / 8;
    let tail_bits = width_usize % 8;
    let sltp_ctx = GenericTemplate::Template0.sltp_context();
    let at1 = adaptive[0];
    let at2 = adaptive[1];
    let at3 = adaptive[2];
    let at4 = adaptive[3];
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        if let Some(mask) = skip {
            crate::views::bitmap_fields(mask);
        }
        assert(full_bytes <= stride);
        assert(tail_bits == 0 || full_bytes < stride);
        assert((width_usize as nat) <= (stride as nat) * 8);
    }

    let mut ltp = false;
    let mut y = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= rows_usize,
            rows_usize == rows as usize,
            rows <= height,
            bitmap.width == width,
            width_usize == width as usize,
            bitmap.height == height,
            bitmap.stride == stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
            contexts@.len() == 65_536usize,
            sltp_ctx < contexts@.len(),
            full_bytes == width_usize / 8,
            tail_bits == width_usize % 8,
            full_bytes <= stride,
            tail_bits == 0 || full_bytes < stride,
            (width_usize as nat) <= (stride as nat) * 8,
            match skip {
                Some(mask) =>
                    crate::spec::bitmap::storage_valid(mask)
                    && mask.width == width
                    && mask.height == height
                    && mask.stride == stride
                    && mask.data@.len() == bitmap.data@.len(),
                None => true,
            },
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
        let y_u32 = y as u32;
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
        let skip_row: Option<&[u8]> = match skip {
            Some(mask) => Some(&mask.data[row_start..row_start + stride]),
            None => None,
        };
        let row_res = decode_row_t0_custom(
            prev_rows, row_m2, row_m1, dst, skip_row, y_u32, width, stride, full_bytes, tail_bits,
            at1, at2, at3, at4, decoder, contexts,
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
        prev_rows@.len() == (y_u32 as usize) * stride,
        dst@.len() == stride,
        full_bytes == (width as usize) / 8,
        tail_bits == (width as usize) % 8,
        full_bytes <= stride,
        tail_bits == 0 || full_bytes < stride,
        (width as nat) <= (stride as nat) * 8,
        match skip_row {
            Some(s) => s@.len() == stride,
            None => true,
        },
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == 65_536usize,
    ensures
        final(dst)@.len() == stride,
        final(contexts)@.len() == 65_536usize,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation
)]
fn decode_row_t0_custom(
    prev_rows: &[u8],
    row_m2: &[u8],
    row_m1: &[u8],
    dst: &mut [u8],
    skip_row: Option<&[u8]>,
    y_u32: u32,
    width: u32,
    stride: usize,
    full_bytes: usize,
    tail_bits: usize,
    at1: TemplateOffset,
    at2: TemplateOffset,
    at3: TemplateOffset,
    at4: TemplateOffset,
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
            full_bytes <= stride,
            dst@.len() == stride,
            dst@.len() == old(dst)@.len(),
            prev_rows@.len() == (y_u32 as usize) * stride,
            tail_bits < 8,
            tail_bits == 0 || full_bytes < stride,
            full_bytes == (width as usize) / 8,
            tail_bits == (width as usize) % 8,
            (width as nat) <= (stride as nat) * 8,
            match skip_row {
                Some(s) => s@.len() == stride,
                None => true,
            },
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
        let skip_byte = match skip_row {
            Some(s) => s[b],
            None => 0,
        };
        if skip_byte == 0xff {
            dst[b] = 0;
            l0 <<= 8;
            l1 <<= 8;
            l2 <<= 8;
        } else {
            let cur_x = (b * 8) as u32;
            let mut k = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    k <= 8,
                    b < full_bytes,
                    full_bytes <= stride,
                    dst@.len() == stride,
                    prev_rows@.len() == (y_u32 as usize) * stride,
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
                    contexts@.len() == 65_536usize,
                decreases 8 - k,
            ))]
            while k < 8 {
                if (skip_byte & (0x80u8 >> k)) != 0 {
                    l0 <<= 1;
                } else {
                    let x = cur_x + (k as u32);
                    let p1 = sample_at_pixel(prev_rows, dst, l0, b, x, y_u32, width, stride, at1);
                    let p2 = sample_at_pixel(prev_rows, dst, l0, b, x, y_u32, width, stride, at2);
                    let p3 = sample_at_pixel(prev_rows, dst, l0, b, x, y_u32, width, stride, at3);
                    let p4 = sample_at_pixel(prev_rows, dst, l0, b, x, y_u32, width, stride, at4);
                    let ctx = (((l2 >> 2) & 0x7000)
                        | ((l1 >> 8) & 0x03e0)
                        | (l0 & 0x000f)
                        | (p1 << 4)
                        | (p2 << 10)
                        | (p3 << 11)
                        | (p4 << 15)) as usize;
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
                }
                l1 <<= 1;
                l2 <<= 1;
                k += 1;
            }
            dst[b] = l0 as u8;
        }
        b += 1;
    }
    if tail_bits != 0 {
        let skip_byte = match skip_row {
            Some(s) => s[full_bytes],
            None => 0,
        };
        let cur_x = (full_bytes * 8) as u32;
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= tail_bits,
                tail_bits < 8,
                full_bytes < stride,
                dst@.len() == stride,
                prev_rows@.len() == (y_u32 as usize) * stride,
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
                contexts@.len() == 65_536usize,
            decreases tail_bits - k,
        ))]
        while k < tail_bits {
            if (skip_byte & (0x80u8 >> k)) != 0 {
                l0 <<= 1;
            } else {
                let x = cur_x + (k as u32);
                let p1 =
                    sample_at_pixel(prev_rows, dst, l0, full_bytes, x, y_u32, width, stride, at1);
                let p2 =
                    sample_at_pixel(prev_rows, dst, l0, full_bytes, x, y_u32, width, stride, at2);
                let p3 =
                    sample_at_pixel(prev_rows, dst, l0, full_bytes, x, y_u32, width, stride, at3);
                let p4 =
                    sample_at_pixel(prev_rows, dst, l0, full_bytes, x, y_u32, width, stride, at4);
                let ctx = (((l2 >> 2) & 0x7000)
                    | ((l1 >> 8) & 0x03e0)
                    | (l0 & 0x000f)
                    | (p1 << 4)
                    | (p2 << 10)
                    | (p3 << 11)
                    | (p4 << 15)) as usize;
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
            }
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
