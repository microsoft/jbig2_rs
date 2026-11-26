//! Direct packed-byte generic-refinement decoders (Templates 0–1) with 8-pixel LTP and MPS skips.

use super::fast_support::{decode_ctx_cached, row_byte, try_skip_ctx_mps};
use super::generic::probability_signal_error;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{
    GenericRegionError, RefinementContextDescriptor, RefinementParameters, RefinementTemplate,
    TemplateOffset,
};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        ref_stride == row@.len() || row@.len() == 0,
    ensures out <= 255,
))]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn ref_byte(row: &[u8], idx: i64, ref_stride: usize, ref_tail_mask: u8) -> u8 {
    if row.is_empty() || idx < 0 {
        return 0;
    }
    let u = idx as usize;
    if u >= row.len() {
        return 0;
    }
    if u + 1 == ref_stride {
        row[u] & ref_tail_mask
    } else {
        row[u]
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        ref_stride == row@.len() || row@.len() == 0,
        rs < 8,
    ensures out <= 255,
))]
#[allow(clippy::cast_possible_truncation)]
#[inline]
pub(super) fn ref_vbyte(row: &[u8], rb: i64, rs: u32, ref_stride: usize, ref_tail_mask: u8) -> u8 {
    let b0 = u32::from(ref_byte(row, rb, ref_stride, ref_tail_mask));
    if rs == 0 {
        return b0 as u8;
    }
    let Some(next_rb) = rb.checked_add(1) else {
        return ((b0 << 8) >> (8 - rs)) as u8;
    };
    let b1 = u32::from(ref_byte(row, next_rb, ref_stride, ref_tail_mask));
    (((b0 << 8) | b1) >> (8 - rs)) as u8
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(reference),
    ensures out@.len() == reference.stride || out@.len() == 0,
))]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn reference_row_slice(reference: &Bitmap, ry: i64) -> &[u8] {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(reference);
    }
    if ry < 0 || ry >= i64::from(reference.height) || reference.stride == 0 {
        return &[];
    }
    let row_y = ry as usize;
    let stride = reference.stride;
    let data_len = reference.data.len();
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(row_y * stride + stride <= data_len) by (nonlinear_arith)
            requires
                row_y < reference.height as usize,
                data_len == stride * (reference.height as usize);
    }
    let _ = data_len;
    let start = row_y * stride;
    &reference.data[start..start + stride]
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        template_id <= 1,
    ensures
        out < match template_id {
            0u8 => 8_192usize,
            _ => 1_024usize,
        },
))]
#[allow(clippy::inline_always)]
#[inline(always)]
fn nominal_ref_ctx(template_id: u8, t1: u32, t0: u32, rp: u32, rc: u32, rn: u32) -> usize {
    let raw = if template_id == 0 {
        ((t1 >> 4) & 0x1c00)
            | ((t0 & 1) << 9)
            | ((rp >> 8) & 0x01c0)
            | ((rc >> 11) & 0x0038)
            | ((rn >> 14) & 0x0007)
    } else {
        ((t1 >> 7) & 0x0380)
            | ((t0 & 1) << 6)
            | ((rp >> 10) & 0x0020)
            | ((rc >> 12) & 0x001c)
            | ((rn >> 14) & 0x0003)
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(
            (template_id == 0u8 ==> raw < 8_192u32)
            && (template_id != 0u8 ==> raw < 1_024u32)
        ) by (bit_vector)
            requires
                raw == if template_id == 0u8 {
                    ((t1 >> 4u32) & 0x1c00u32)
                        | ((t0 & 1u32) << 9u32)
                        | ((rp >> 8u32) & 0x01c0u32)
                        | ((rc >> 11u32) & 0x0038u32)
                        | ((rn >> 14u32) & 0x0007u32)
                } else {
                    ((t1 >> 7u32) & 0x0380u32)
                        | ((t0 & 1u32) << 6u32)
                        | ((rp >> 10u32) & 0x0020u32)
                        | ((rc >> 12u32) & 0x001cu32)
                        | ((rn >> 14u32) & 0x0003u32)
                };
    }
    raw as usize
}

/// Direct packed-byte refinement decoder covering Nominal T0, T1 (aligned and
/// shifted), and General T0 with 8-pixel LTP and MPS fast-skips.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        crate::spec::bitmap::storage_valid(reference),
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == match template {
            RefinementTemplate::Template0 => 8_192usize,
            RefinementTemplate::Template1 => 1_024usize,
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
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names
)]
pub(super) fn decode_refinement_fast(
    bitmap: &mut Bitmap,
    reference: &Bitmap,
    template: RefinementTemplate,
    adaptive: [TemplateOffset; 2],
    typical_prediction: bool,
    parameters: RefinementParameters,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let descriptor = RefinementContextDescriptor::new(template, adaptive);
    let width = bitmap.width;
    let height = bitmap.height;
    let stride = bitmap.stride;
    let height_usize = height as usize;
    let full_bytes = (width as usize) / 8;
    let tail_bits = (width as usize) % 8;
    let ref_stride = reference.stride;
    let ref_rem = (reference.width as usize) % 8;
    let ref_tail_mask: u8 = if ref_rem == 0 {
        0xff
    } else {
        0xffu8 << (8 - ref_rem)
    };
    let neg_dx = -i64::from(parameters.reference_dx);
    let q = neg_dx / 8;
    let r = neg_dx % 8;
    let (base_rb, rs): (i64, u32) = if r < 0 {
        (q - 1, (r + 8) as u32)
    } else {
        (q, r as u32)
    };
    let aligned = parameters.reference_dx == 0;
    let sltp_ctx = template.sltp_context();
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        crate::views::bitmap_fields(reference);
        assert(full_bytes <= stride);
        assert(tail_bits == 0 || full_bytes < stride);
        assert(rs < 8);
    }

    let mut ltp = false;
    let mut y = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= height_usize,
            height_usize == height as usize,
            bitmap.width == width,
            bitmap.height == height,
            bitmap.stride == stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::spec::bitmap::storage_valid(reference),
            reference.stride == ref_stride,
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
            contexts@.len() == match template {
                RefinementTemplate::Template0 => 8_192usize,
                RefinementTemplate::Template1 => 1_024usize,
            },
            sltp_ctx < contexts@.len(),
            full_bytes == (width as usize) / 8,
            tail_bits == (width as usize) % 8,
            full_bytes <= stride,
            tail_bits == 0 || full_bytes < stride,
            (width as nat) <= (stride as nat) * 8,
            -1_000_000_000i64 <= base_rb <= 1_000_000_000i64,
            rs < 8,
            aligned ==> (base_rb == 0 && rs == 0),
        decreases height_usize - y,
    ))]
    while y < height_usize {
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
                    y < height as usize,
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
        }
        let ry = i64::from(y as u32) - i64::from(parameters.reference_dy);
        let row_rp = reference_row_slice(reference, ry - 1);
        let row_rc = reference_row_slice(reference, ry);
        let row_rn = reference_row_slice(reference, ry + 1);
        let (prev_rows, cur_and_after) = bitmap.data.as_mut_slice().split_at_mut(row_start);
        let dst = &mut cur_and_after[..stride];
        let row_t1: &[u8] = if y >= 1 {
            &prev_rows[row_start - stride..row_start]
        } else {
            &[]
        };

        let row_res = match (descriptor, template) {
            (RefinementContextDescriptor::NominalTemplate0, RefinementTemplate::Template0) => {
                if aligned {
                    decode_row_nominal_ref(
                        0,
                        true,
                        row_t1,
                        row_rp,
                        row_rc,
                        row_rn,
                        dst,
                        0,
                        0,
                        ref_stride,
                        ref_tail_mask,
                        full_bytes,
                        tail_bits,
                        ltp,
                        decoder,
                        contexts,
                    )
                } else {
                    decode_row_nominal_ref(
                        0,
                        false,
                        row_t1,
                        row_rp,
                        row_rc,
                        row_rn,
                        dst,
                        base_rb,
                        rs,
                        ref_stride,
                        ref_tail_mask,
                        full_bytes,
                        tail_bits,
                        ltp,
                        decoder,
                        contexts,
                    )
                }
            }
            (RefinementContextDescriptor::Template1, RefinementTemplate::Template1) => {
                if aligned {
                    decode_row_nominal_ref(
                        1,
                        true,
                        row_t1,
                        row_rp,
                        row_rc,
                        row_rn,
                        dst,
                        0,
                        0,
                        ref_stride,
                        ref_tail_mask,
                        full_bytes,
                        tail_bits,
                        ltp,
                        decoder,
                        contexts,
                    )
                } else {
                    decode_row_nominal_ref(
                        1,
                        false,
                        row_t1,
                        row_rp,
                        row_rc,
                        row_rn,
                        dst,
                        base_rb,
                        rs,
                        ref_stride,
                        ref_tail_mask,
                        full_bytes,
                        tail_bits,
                        ltp,
                        decoder,
                        contexts,
                    )
                }
            }
            (
                RefinementContextDescriptor::Template0 {
                    target: target_at,
                    reference: ref_at,
                },
                RefinementTemplate::Template0,
            ) => super::fast_refinement_general::decode_row_general_t0_ref(
                prev_rows,
                row_t1,
                reference,
                row_rp,
                row_rc,
                row_rn,
                dst,
                y as u32,
                width,
                stride,
                base_rb,
                rs,
                ref_stride,
                ref_tail_mask,
                full_bytes,
                tail_bits,
                ltp,
                parameters,
                target_at,
                ref_at,
                decoder,
                contexts,
            ),
            _ => Ok(()),
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
        template_id <= 1,
        -1_000_000_000i64 <= base_rb <= 1_000_000_000i64,
        rs < 8,
        aligned ==> (base_rb == 0 && rs == 0),
        ref_stride == row_rp@.len() || row_rp@.len() == 0,
        ref_stride == row_rc@.len() || row_rc@.len() == 0,
        ref_stride == row_rn@.len() || row_rn@.len() == 0,
        dst@.len() >= full_bytes,
        tail_bits < 8,
        tail_bits == 0 || full_bytes < dst@.len(),
        full_bytes <= (u32::MAX as usize) / 8,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        old(contexts)@.len() == match template_id {
            0u8 => 8_192usize,
            _ => 1_024usize,
        },
    ensures
        final(dst)@.len() == old(dst)@.len(),
        final(contexts)@.len() == match template_id {
            0u8 => 8_192usize,
            _ => 1_024usize,
        },
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::inline_always,
    clippy::similar_names
)]
#[inline(always)]
fn decode_row_nominal_ref(
    template_id: u8,
    aligned: bool,
    row_t1: &[u8],
    row_rp: &[u8],
    row_rc: &[u8],
    row_rn: &[u8],
    dst: &mut [u8],
    base_rb: i64,
    rs: u32,
    ref_stride: usize,
    ref_tail_mask: u8,
    full_bytes: usize,
    tail_bits: usize,
    ltp: bool,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let all_ones_ctx: usize = if template_id == 0 { 0x1fff } else { 0x03ff };
    let rp_mps_mask: u32 = if template_id == 0 {
        0x0001_ff80
    } else {
        0x0000_ff00
    };
    let rn_mps_mask: u32 = if template_id == 0 {
        0x0001_ff80
    } else {
        0x0000_ff80
    };

    let mut interval = decoder.interval;
    let mut code = decoder.code;
    let mut bit_count = decoder.bit_count;
    let mut t1 = u32::from(row_byte(row_t1, 0)) << 8;
    let mut t0 = 0_u32;
    let (mut rp, mut rc, mut rn) = if aligned {
        (
            u32::from(ref_byte(row_rp, 0, ref_stride, ref_tail_mask)) << 8,
            u32::from(ref_byte(row_rc, 0, ref_stride, ref_tail_mask)) << 8,
            u32::from(ref_byte(row_rn, 0, ref_stride, ref_tail_mask)) << 8,
        )
    } else {
        (
            (u32::from(ref_vbyte(
                row_rp,
                base_rb - 1,
                rs,
                ref_stride,
                ref_tail_mask,
            )) << 16)
                | (u32::from(ref_vbyte(row_rp, base_rb, rs, ref_stride, ref_tail_mask)) << 8),
            (u32::from(ref_vbyte(
                row_rc,
                base_rb - 1,
                rs,
                ref_stride,
                ref_tail_mask,
            )) << 16)
                | (u32::from(ref_vbyte(row_rc, base_rb, rs, ref_stride, ref_tail_mask)) << 8),
            (u32::from(ref_vbyte(
                row_rn,
                base_rb - 1,
                rs,
                ref_stride,
                ref_tail_mask,
            )) << 16)
                | (u32::from(ref_vbyte(row_rn, base_rb, rs, ref_stride, ref_tail_mask)) << 8),
        )
    };

    let mut b = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            b <= full_bytes,
            dst@.len() == old(dst)@.len(),
            full_bytes <= dst@.len(),
            full_bytes <= (u32::MAX as usize) / 8,
            tail_bits < 8,
            tail_bits == 0 || full_bytes < dst@.len(),
            template_id <= 1,
            -1_000_000_000i64 <= base_rb <= 1_000_000_000i64,
            rs < 8,
            aligned ==> (base_rb == 0 && rs == 0),
            ref_stride == row_rp@.len() || row_rp@.len() == 0,
            ref_stride == row_rc@.len() || row_rc@.len() == 0,
            ref_stride == row_rn@.len() || row_rn@.len() == 0,
            all_ones_ctx < contexts@.len(),
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
                0u8 => 8_192usize,
                _ => 1_024usize,
            },
        decreases full_bytes - b,
    ))]
    while b < full_bytes {
        t1 |= u32::from(row_byte(row_t1, b + 1));
        if aligned {
            let next_i = (b + 1) as i64;
            rp |= u32::from(ref_byte(row_rp, next_i, ref_stride, ref_tail_mask));
            rc |= u32::from(ref_byte(row_rc, next_i, ref_stride, ref_tail_mask));
            rn |= u32::from(ref_byte(row_rn, next_i, ref_stride, ref_tail_mask));
        } else {
            let next_rb = base_rb + (b as i64) + 1;
            rp |= u32::from(ref_vbyte(row_rp, next_rb, rs, ref_stride, ref_tail_mask));
            rc |= u32::from(ref_vbyte(row_rc, next_rb, rs, ref_stride, ref_tail_mask));
            rn |= u32::from(ref_vbyte(row_rn, next_rb, rs, ref_stride, ref_tail_mask));
        }

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
        } else {
            let all_zero = (t1 & 0x0001_ff80) == 0
                && (t0 & 1) == 0
                && (rp & rp_mps_mask) == 0
                && (rc & 0x0001_ff80) == 0
                && (rn & rn_mps_mask) == 0;
            if all_zero
                && try_skip_ctx_mps(decoder, contexts, 0, 0, &mut interval, &mut code, bit_count)
            {
                dst[b] = 0;
                t0 <<= 8;
                t1 <<= 8;
                rp <<= 8;
                rc <<= 8;
                rn <<= 8;
                b += 1;
                continue;
            }
            let all_one = (t1 & 0x0001_ff80) == 0x0001_ff80
                && (t0 & 1) == 1
                && (rp & rp_mps_mask) == rp_mps_mask
                && (rc & 0x0001_ff80) == 0x0001_ff80
                && (rn & rn_mps_mask) == rn_mps_mask;
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
                t0 = (t0 << 8) | 0xff;
                t1 <<= 8;
                rp <<= 8;
                rc <<= 8;
                rn <<= 8;
                b += 1;
                continue;
            }
        }

        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= 8,
                b < full_bytes,
                dst@.len() == old(dst)@.len(),
                full_bytes <= dst@.len(),
                template_id <= 1,
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
                    0u8 => 8_192usize,
                    _ => 1_024usize,
                },
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
                    let ctx = nominal_ref_ctx(template_id, t1, t0, rp, rc, rn);
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
                let ctx = nominal_ref_ctx(template_id, t1, t0, rp, rc, rn);
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
        if aligned {
            let next_i = (full_bytes + 1) as i64;
            rp |= u32::from(ref_byte(row_rp, next_i, ref_stride, ref_tail_mask));
            rc |= u32::from(ref_byte(row_rc, next_i, ref_stride, ref_tail_mask));
            rn |= u32::from(ref_byte(row_rn, next_i, ref_stride, ref_tail_mask));
        } else {
            let next_rb = base_rb + (full_bytes as i64) + 1;
            rp |= u32::from(ref_vbyte(row_rp, next_rb, rs, ref_stride, ref_tail_mask));
            rc |= u32::from(ref_vbyte(row_rc, next_rb, rs, ref_stride, ref_tail_mask));
            rn |= u32::from(ref_vbyte(row_rn, next_rb, rs, ref_stride, ref_tail_mask));
        }
        let mut k = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                k <= tail_bits,
                tail_bits < 8,
                dst@.len() == old(dst)@.len(),
                full_bytes < dst@.len(),
                template_id <= 1,
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
                    0u8 => 8_192usize,
                    _ => 1_024usize,
                },
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
                    let ctx = nominal_ref_ctx(template_id, t1, t0, rp, rc, rn);
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
                let ctx = nominal_ref_ctx(template_id, t1, t0, rp, rc, rn);
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
