//! Direct packed-byte generic region arithmetic decoders (Templates 0–3).

use super::fast_support::{copy_prev_row, decode_ctx_cached, row_byte, try_skip_ctx_mps};
use super::generic::probability_signal_error;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{GenericContextDescriptor, GenericRegionError, GenericTemplate, TemplateOffset};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Dispatches direct packed-byte generic region decoding when supported.
/// Returns `Ok(true)` if decoded completely, or `Ok(false)` to fall back.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        match skip {
            Some(mask) =>
                crate::spec::bitmap::storage_valid(mask)
                && mask.width == old(bitmap).width
                && mask.height == old(bitmap).height,
            None => true,
        },
        old(contexts)@.len() == match template {
            GenericTemplate::Template0 => 65_536usize,
            GenericTemplate::Template1 => 8_192usize,
            GenericTemplate::Template2 | GenericTemplate::Template3 => 1_024usize,
        },
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(clippy::too_many_arguments)]
pub(super) fn try_decode_generic(
    bitmap: &mut Bitmap,
    rows: u32,
    template: GenericTemplate,
    extended: bool,
    adaptive: &[TemplateOffset; 12],
    typical_prediction: bool,
    skip: Option<&Bitmap>,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<bool, GenericRegionError> {
    let descriptor = GenericContextDescriptor::new(template, extended, adaptive);
    match descriptor {
        GenericContextDescriptor::NominalTemplate0 => {
            if contexts.len() != 65_536 {
                return Ok(false);
            }
            decode_nominal(bitmap, rows, 0, typical_prediction, skip, decoder, contexts)?;
            Ok(true)
        }
        GenericContextDescriptor::NominalTemplate1 => {
            if contexts.len() != 8_192 {
                return Ok(false);
            }
            decode_nominal(bitmap, rows, 1, typical_prediction, skip, decoder, contexts)?;
            Ok(true)
        }
        GenericContextDescriptor::NominalTemplate2 => {
            if contexts.len() != 1_024 {
                return Ok(false);
            }
            decode_nominal(bitmap, rows, 2, typical_prediction, skip, decoder, contexts)?;
            Ok(true)
        }
        GenericContextDescriptor::NominalTemplate3 => {
            if contexts.len() != 1_024 {
                return Ok(false);
            }
            decode_nominal(bitmap, rows, 3, typical_prediction, skip, decoder, contexts)?;
            Ok(true)
        }
        _ => {
            if matches!(template, GenericTemplate::Template0) && !extended {
                let is_pattern = skip.is_none()
                    && adaptive[0].y == 0
                    && adaptive[0].x < 0
                    && adaptive[0].x >= -128
                    && adaptive[1] == TemplateOffset::new(-3, -1)
                    && adaptive[2] == TemplateOffset::new(2, -2)
                    && adaptive[3] == TemplateOffset::new(-2, -2);
                if is_pattern {
                    #[allow(clippy::cast_sign_loss)]
                    let pattern_width = (-adaptive[0].x) as usize;
                    super::fast_pattern::decode_t0_pattern(
                        bitmap,
                        rows,
                        pattern_width,
                        typical_prediction,
                        decoder,
                        contexts,
                    )?;
                    return Ok(true);
                }
                super::fast_custom::decode_t0_custom(
                    bitmap,
                    rows,
                    adaptive,
                    typical_prediction,
                    skip,
                    decoder,
                    contexts,
                )?;
                return Ok(true);
            }
            Ok(false)
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        template_id <= 3,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == match template_id {
            0u8 => 65_536usize,
            1u8 => 8_192usize,
            _ => 1_024usize,
        },
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
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn decode_nominal(
    bitmap: &mut Bitmap,
    rows: u32,
    template_id: u8,
    typical_prediction: bool,
    skip: Option<&Bitmap>,
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
    let sltp_ctx: usize = match template_id {
        0 => GenericTemplate::Template0.sltp_context(),
        1 => GenericTemplate::Template1.sltp_context(),
        2 => GenericTemplate::Template2.sltp_context(),
        _ => GenericTemplate::Template3.sltp_context(),
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        if let Some(mask) = skip {
            crate::views::bitmap_fields(mask);
        }
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
            bitmap.width == width,
            bitmap.height == height,
            bitmap.stride == stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
            contexts@.len() == match template_id {
                0u8 => 65_536usize,
                1u8 => 8_192usize,
                _ => 1_024usize,
            },
            sltp_ctx < contexts@.len(),
            full_bytes == (width as usize) / 8,
            tail_bits == (width as usize) % 8,
            full_bytes <= stride,
            tail_bits == 0 || full_bytes < stride,
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
        let row_res = match template_id {
            0 => decode_row_nominal_core(
                0, row_m2, row_m1, dst, skip_row, full_bytes, tail_bits, decoder, contexts,
            ),
            1 => decode_row_nominal_core(
                1, row_m2, row_m1, dst, skip_row, full_bytes, tail_bits, decoder, contexts,
            ),
            2 => decode_row_nominal_core(
                2, row_m2, row_m1, dst, skip_row, full_bytes, tail_bits, decoder, contexts,
            ),
            _ => decode_row_nominal_core(
                3,
                &[],
                row_m1,
                dst,
                skip_row,
                full_bytes,
                tail_bits,
                decoder,
                contexts,
            ),
        };
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
        template_id <= 3,
        dst@.len() >= full_bytes,
        tail_bits < 8,
        tail_bits == 0 || full_bytes < dst@.len(),
        match skip_row {
            Some(s) => s@.len() == dst@.len(),
            None => true,
        },
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == match template_id {
            0u8 => 65_536usize,
            1u8 => 8_192usize,
            _ => 1_024usize,
        },
    ensures
        final(dst)@.len() == old(dst)@.len(),
        final(contexts)@.len() == match template_id {
            0u8 => 65_536usize,
            1u8 => 8_192usize,
            _ => 1_024usize,
        },
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::verbose_bit_mask,
    clippy::inline_always
)]
#[inline(always)]
fn decode_row_nominal_core(
    template_id: u8,
    row_m2: &[u8],
    row_m1: &[u8],
    dst: &mut [u8],
    skip_row: Option<&[u8]>,
    full_bytes: usize,
    tail_bits: usize,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let all_ones_ctx: usize = match template_id {
        0 => 0xffff,
        1 => 0x1fff,
        _ => 0x03ff,
    };
    let l2_mask: u32 = match template_id {
        0 => 0x0003_ffc0,
        1 => 0x0001_ffc0,
        2 => 0x0001_ff80,
        _ => 0,
    };
    let l1_mask: u32 = match template_id {
        0 => 0x0007_ffe0,
        1 => 0x0003_ffe0,
        2 => 0x0003_ffc0,
        _ => 0x0007_ffc0,
    };
    let l0_mask: u32 = match template_id {
        0 | 3 => 0x000f,
        1 => 0x0007,
        _ => 0x0003,
    };
    let mut interval = decoder.interval;
    let mut code = decoder.code;
    let mut bit_count = decoder.bit_count;
    let mut l2 = if template_id == 3 {
        0
    } else {
        u32::from(row_byte(row_m2, 0)) << 8
    };
    let mut l1 = u32::from(row_byte(row_m1, 0)) << 8;
    let mut l0 = 0_u32;
    let mut b = 0_usize;

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            b <= full_bytes,
            dst@.len() == old(dst)@.len(),
            full_bytes <= dst@.len(),
            tail_bits < 8,
            tail_bits == 0 || full_bytes < dst@.len(),
            template_id <= 3,
            all_ones_ctx < contexts@.len(),
            match skip_row {
                Some(s) => s@.len() == dst@.len(),
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
            contexts@.len() == match template_id {
                0u8 => 65_536usize,
                1u8 => 8_192usize,
                _ => 1_024usize,
            },
        decreases full_bytes - b,
    ))]
    while b < full_bytes {
        if template_id != 3 {
            l2 |= u32::from(row_byte(row_m2, b + 1));
        }
        l1 |= u32::from(row_byte(row_m1, b + 1));
        let skip_byte = match skip_row {
            Some(s) => s[b],
            None => 0,
        };
        if skip_byte == 0 {
            let all_zero = (l2 & l2_mask) == 0 && (l1 & l1_mask) == 0 && (l0 & l0_mask) == 0;
            if all_zero
                && try_skip_ctx_mps(decoder, contexts, 0, 0, &mut interval, &mut code, bit_count)
            {
                dst[b] = 0;
                l0 <<= 8;
                l1 <<= 8;
                if template_id != 3 {
                    l2 <<= 8;
                }
                b += 1;
                continue;
            }
            let all_one =
                (l2 & l2_mask) == l2_mask && (l1 & l1_mask) == l1_mask && (l0 & l0_mask) == l0_mask;
            if all_one
                && try_skip_ctx_mps(
                    decoder,
                    contexts,
                    all_ones_ctx,
                    1,
                    &mut interval,
                    &mut code,
                    bit_count,
                )
            {
                dst[b] = 0xff;
                l0 = (l0 << 8) | 0xff;
                l1 <<= 8;
                if template_id != 3 {
                    l2 <<= 8;
                }
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
                    template_id <= 3,
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
                    contexts@.len() == match template_id {
                        0u8 => 65_536usize,
                        1u8 => 8_192usize,
                        _ => 1_024usize,
                    },
                decreases 8 - k,
            ))]
            while k < 8 {
                let ctx = nominal_ctx(template_id, l2, l1, l0);
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
                if template_id != 3 {
                    l2 <<= 1;
                }
                k += 1;
            }
            dst[b] = l0 as u8;
        } else if skip_byte == 0xff {
            dst[b] = 0;
            l0 <<= 8;
            l1 <<= 8;
            if template_id != 3 {
                l2 <<= 8;
            }
        } else {
            let mut k = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    k <= 8,
                    b < full_bytes,
                    dst@.len() == old(dst)@.len(),
                    full_bytes <= dst@.len(),
                    template_id <= 3,
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
                    contexts@.len() == match template_id {
                        0u8 => 65_536usize,
                        1u8 => 8_192usize,
                        _ => 1_024usize,
                    },
                decreases 8 - k,
            ))]
            while k < 8 {
                if (skip_byte & (0x80u8 >> k)) != 0 {
                    l0 <<= 1;
                } else {
                    let ctx = nominal_ctx(template_id, l2, l1, l0);
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
                if template_id != 3 {
                    l2 <<= 1;
                }
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
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= tail_bits,
                tail_bits < 8,
                dst@.len() == old(dst)@.len(),
                full_bytes < dst@.len(),
                template_id <= 3,
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
                contexts@.len() == old(contexts)@.len(),
                contexts@.len() == match template_id {
                    0u8 => 65_536usize,
                    1u8 => 8_192usize,
                    _ => 1_024usize,
                },
            decreases tail_bits - k,
        ))]
        while k < tail_bits {
            if (skip_byte & (0x80u8 >> k)) != 0 {
                l0 <<= 1;
            } else {
                let ctx = nominal_ctx(template_id, l2, l1, l0);
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
            if template_id != 3 {
                l2 <<= 1;
            }
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

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires template_id <= 3,
    ensures out < match template_id {
        0u8 => 65_536usize,
        1u8 => 8_192usize,
        _ => 1_024usize,
    },
))]
#[allow(clippy::inline_always)]
#[inline(always)]
fn nominal_ctx(template_id: u8, l2: u32, l1: u32, l0: u32) -> usize {
    let raw = match template_id {
        0 => ((l2 >> 2) & 0xf800) | ((l1 >> 8) & 0x07f0) | (l0 & 0x000f),
        1 => ((l2 >> 4) & 0x1e00) | ((l1 >> 9) & 0x01f8) | (l0 & 0x0007),
        2 => ((l2 >> 7) & 0x0380) | ((l1 >> 11) & 0x007c) | (l0 & 0x0003),
        _ => ((l1 >> 9) & 0x03f0) | (l0 & 0x000f),
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(
            (template_id == 0u8 ==> raw < 65_536u32)
            && (template_id == 1u8 ==> raw < 8_192u32)
            && (template_id >= 2u8 ==> raw < 1_024u32)
        ) by (bit_vector)
            requires
                raw == if template_id == 0u8 {
                    ((l2 >> 2u32) & 0xf800u32) | ((l1 >> 8u32) & 0x07f0u32) | (l0 & 0x000fu32)
                } else if template_id == 1u8 {
                    ((l2 >> 4u32) & 0x1e00u32) | ((l1 >> 9u32) & 0x01f8u32) | (l0 & 0x0007u32)
                } else if template_id == 2u8 {
                    ((l2 >> 7u32) & 0x0380u32) | ((l1 >> 11u32) & 0x007cu32) | (l0 & 0x0003u32)
                } else {
                    ((l1 >> 9u32) & 0x03f0u32) | (l0 & 0x000fu32)
                };
    }
    raw as usize
}
