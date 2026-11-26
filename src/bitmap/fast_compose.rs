//! Operator- and alignment-specialized SIMD/NEON-friendly packed bitmap composition.

use super::{Bitmap, CombinationOperator};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out <= 255,
))]
#[allow(clippy::inline_always)]
#[inline(always)]
fn apply_op_byte(operator: CombinationOperator, dst: u8, src: u8) -> u8 {
    match operator {
        CombinationOperator::Or => dst | src,
        CombinationOperator::And => dst & src,
        CombinationOperator::Xor => dst ^ src,
        CombinationOperator::Xnor => !(dst ^ src),
        CombinationOperator::Replace => src,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        dst@.len() == src@.len(),
    ensures
        final(dst)@.len() == old(dst)@.len(),
))]
#[allow(clippy::inline_always)]
#[inline(always)]
fn combine_aligned_slice(operator: CombinationOperator, dst: &mut [u8], src: &[u8]) {
    let len = dst.len();
    let mut i = 0_usize;
    match operator {
        CombinationOperator::Or =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant i <= len, dst@.len() == len, src@.len() == len,
                decreases len - i,
            ))]
            while i < len {
                dst[i] |= src[i];
                i += 1;
            }
        }
        CombinationOperator::And =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant i <= len, dst@.len() == len, src@.len() == len,
                decreases len - i,
            ))]
            while i < len {
                dst[i] &= src[i];
                i += 1;
            }
        }
        CombinationOperator::Xor =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant i <= len, dst@.len() == len, src@.len() == len,
                decreases len - i,
            ))]
            while i < len {
                dst[i] ^= src[i];
                i += 1;
            }
        }
        CombinationOperator::Xnor =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant i <= len, dst@.len() == len, src@.len() == len,
                decreases len - i,
            ))]
            while i < len {
                dst[i] = !(dst[i] ^ src[i]);
                i += 1;
            }
        }
        CombinationOperator::Replace =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant i <= len, dst@.len() == len, src@.len() == len,
                decreases len - i,
            ))]
            while i < len {
                dst[i] = src[i];
                i += 1;
            }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        1 <= s_shift < 8,
        dst@.len() == count,
        count + 1 <= src@.len(),
    ensures
        final(dst)@.len() == old(dst)@.len(),
))]
#[allow(clippy::inline_always)]
#[inline(always)]
fn combine_shifted_slice(
    operator: CombinationOperator,
    dst: &mut [u8],
    src: &[u8],
    count: usize,
    s_shift: u32,
) {
    let rshift = 8 - s_shift;
    let mut i = 0_usize;
    match operator {
        CombinationOperator::Or =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    i <= count, dst@.len() == count, count + 1 <= src@.len(),
                    1 <= s_shift < 8, rshift == 8 - s_shift,
                decreases count - i,
            ))]
            while i < count {
                let sv = (src[i] << s_shift) | (src[i + 1] >> rshift);
                dst[i] |= sv;
                i += 1;
            }
        }
        CombinationOperator::And =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    i <= count, dst@.len() == count, count + 1 <= src@.len(),
                    1 <= s_shift < 8, rshift == 8 - s_shift,
                decreases count - i,
            ))]
            while i < count {
                let sv = (src[i] << s_shift) | (src[i + 1] >> rshift);
                dst[i] &= sv;
                i += 1;
            }
        }
        CombinationOperator::Xor =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    i <= count, dst@.len() == count, count + 1 <= src@.len(),
                    1 <= s_shift < 8, rshift == 8 - s_shift,
                decreases count - i,
            ))]
            while i < count {
                let sv = (src[i] << s_shift) | (src[i + 1] >> rshift);
                dst[i] ^= sv;
                i += 1;
            }
        }
        CombinationOperator::Xnor =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    i <= count, dst@.len() == count, count + 1 <= src@.len(),
                    1 <= s_shift < 8, rshift == 8 - s_shift,
                decreases count - i,
            ))]
            while i < count {
                let sv = (src[i] << s_shift) | (src[i + 1] >> rshift);
                dst[i] = !(dst[i] ^ sv);
                i += 1;
            }
        }
        CombinationOperator::Replace =>
        {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    i <= count, dst@.len() == count, count + 1 <= src@.len(),
                    1 <= s_shift < 8, rshift == 8 - s_shift,
                decreases count - i,
            ))]
            while i < count {
                let sv = (src[i] << s_shift) | (src[i + 1] >> rshift);
                dst[i] = sv;
                i += 1;
            }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        width > 0,
        dst_bit < 8,
        src_bit < 8,
        dst_row@.len() == ((dst_bit as nat + width as nat - 1) / 8 + 1),
        src_row@.len() == ((src_bit as nat + width as nat - 1) / 8 + 1),
    ensures
        final(dst_row)@.len() == old(dst_row)@.len(),
))]
#[allow(
    clippy::inline_always,
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::range_plus_one
)]
#[inline(always)]
fn compose_one_row(
    operator: CombinationOperator,
    dst_row: &mut [u8],
    src_row: &[u8],
    dst_bit: u32,
    src_bit: u32,
    width: usize,
) {
    if dst_bit == 0 && src_bit == 0 {
        let full = width / 8;
        let tail = (width % 8) as u32;
        if full != 0 {
            combine_aligned_slice(operator, &mut dst_row[..full], &src_row[..full]);
        }
        if tail != 0 {
            let mask = 0xff_u8 << (8 - tail);
            let dv = dst_row[full];
            let sv = src_row[full];
            dst_row[full] = (dv & !mask) | (apply_op_byte(operator, dv, sv) & mask);
        }
        return;
    }

    if dst_bit == src_bit {
        let max_head = (8 - dst_bit) as usize;
        let head = if width < max_head { width } else { max_head };
        let head_u32 = head as u32;
        let head_mask = (0xff_u8 >> dst_bit) & (0xff_u8 << (8 - dst_bit - head_u32));
        let dv0 = dst_row[0];
        let sv0 = src_row[0];
        dst_row[0] = (dv0 & !head_mask) | (apply_op_byte(operator, dv0, sv0) & head_mask);
        let rem = width - head;
        if rem == 0 {
            return;
        }
        let mid = rem / 8;
        let tail = (rem % 8) as u32;
        if mid != 0 {
            combine_aligned_slice(operator, &mut dst_row[1..1 + mid], &src_row[1..1 + mid]);
        }
        if tail != 0 {
            let mask = 0xff_u8 << (8 - tail);
            let dv = dst_row[1 + mid];
            let sv = src_row[1 + mid];
            dst_row[1 + mid] = (dv & !mask) | (apply_op_byte(operator, dv, sv) & mask);
        }
        return;
    }

    let (d_off, s_off, s_shift, rem) = if dst_bit != 0 {
        let max_head = (8 - dst_bit) as usize;
        let head = if width < max_head { width } else { max_head };
        let head_u32 = head as u32;
        let head_mask = (0xff_u8 >> dst_bit) & (0xff_u8 << (8 - dst_bit - head_u32));
        let raw = if src_bit + head_u32 <= 8 {
            src_row[0] << src_bit
        } else {
            (src_row[0] << src_bit) | (src_row[1] >> (8 - src_bit))
        };
        let sv0 = raw >> dst_bit;
        let dv0 = dst_row[0];
        dst_row[0] = (dv0 & !head_mask) | (apply_op_byte(operator, dv0, sv0) & head_mask);
        let rem = width - head;
        if rem == 0 {
            return;
        }
        let s_total = (src_bit as usize) + head;
        (1_usize, s_total / 8, (s_total % 8) as u32, rem)
    } else {
        (0_usize, 0_usize, src_bit, width)
    };

    let mid = rem / 8;
    let tail = (rem % 8) as u32;
    if mid != 0 {
        if s_shift == 0 {
            combine_aligned_slice(
                operator,
                &mut dst_row[d_off..d_off + mid],
                &src_row[s_off..s_off + mid],
            );
        } else {
            combine_shifted_slice(
                operator,
                &mut dst_row[d_off..d_off + mid],
                &src_row[s_off..s_off + mid + 1],
                mid,
                s_shift,
            );
        }
    }
    if tail != 0 {
        let mask = 0xff_u8 << (8 - tail);
        let si = s_off + mid;
        let sv = if s_shift == 0 {
            src_row[si]
        } else if s_shift + tail <= 8 {
            src_row[si] << s_shift
        } else {
            (src_row[si] << s_shift) | (src_row[si + 1] >> (8 - s_shift))
        };
        let dv = dst_row[d_off + mid];
        dst_row[d_off + mid] = (dv & !mask) | (apply_op_byte(operator, dv, sv) & mask);
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        crate::spec::bitmap::storage_valid(old(dst)),
        crate::spec::bitmap::storage_valid(src),
        width > 0,
        height > 0,
        src_x as nat + width as nat <= src.width as nat,
        src_y as nat + height as nat <= src.height as nat,
        dst_x as nat + width as nat <= old(dst).width as nat,
        dst_y as nat + height as nat <= old(dst).height as nat,
    ensures
        crate::spec::bitmap::storage_valid(final(dst)),
        final(dst).width == old(dst).width,
        final(dst).height == old(dst).height,
        final(dst).stride == old(dst).stride,
        final(dst).data@.len() == old(dst).data@.len(),
        forall|idx: int|
            0 <= idx < old(dst).data@.len()
            && old(dst).stride > 0
            && (
                idx / (old(dst).stride as int) < (dst_y as int)
                || idx / (old(dst).stride as int) >= (dst_y as int) + (height as int)
                || idx % (old(dst).stride as int) < (dst_x as int) / 8
                || idx % (old(dst).stride as int) >= (dst_x as int) / 8
                    + (((dst_x as int) % 8 + width as int - 1) / 8 + 1)
            )
            ==> final(dst).data@[idx] == old(dst).data@[idx],
))]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::inline_always
)]
#[inline(always)]
fn compose_rect_for_op(
    operator: CombinationOperator,
    dst: &mut Bitmap,
    src: &Bitmap,
    src_x: u32,
    src_y: u32,
    dst_x: u32,
    dst_y: u32,
    width: u32,
    height: u32,
) {
    let dst_stride = dst.stride;
    let src_stride = src.stride;
    let dst_col = (dst_x as usize) / 8;
    let dst_bit = dst_x % 8;
    let src_col = (src_x as usize) / 8;
    let src_bit = src_x % 8;
    let width_usize = width as usize;
    let dst_span = ((dst_bit as usize) + width_usize - 1) / 8 + 1;
    let src_span = ((src_bit as usize) + width_usize - 1) / 8 + 1;
    let height_usize = height as usize;
    let dst_y_usize = dst_y as usize;
    let src_y_usize = src_y as usize;
    #[allow(unused_variables)]
    let dst_len = dst.data.len();
    #[allow(unused_variables)]
    let src_len = src.data.len();
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost orig = dst.data@; }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(dst);
        crate::views::bitmap_fields(src);
        assert(dst_col + dst_span <= dst_stride);
        assert(src_col + src_span <= src_stride);
    }

    let mut row = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            row <= height_usize,
            height_usize == height as usize,
            dst_y_usize == dst_y as usize,
            src_y_usize == src_y as usize,
            width_usize == width as usize,
            width > 0,
            height > 0,
            dst_bit == dst_x % 8,
            dst_bit < 8,
            src_bit == src_x % 8,
            src_bit < 8,
            dst_col == (dst_x as usize) / 8,
            src_col == (src_x as usize) / 8,
            dst_span == ((dst_bit as usize) + width_usize - 1) / 8 + 1,
            src_span == ((src_bit as usize) + width_usize - 1) / 8 + 1,
            dst_col + dst_span <= dst_stride,
            src_col + src_span <= src_stride,
            dst.width == old(dst).width,
            dst.height == old(dst).height,
            dst.stride == dst_stride,
            dst_stride == old(dst).stride,
            dst.data@.len() == dst_len,
            dst_len == old(dst).data@.len(),
            orig == old(dst).data@,
            src.stride == src_stride,
            src.data@.len() == src_len,
            crate::spec::bitmap::storage_valid(dst),
            crate::spec::bitmap::storage_valid(src),
            dst_y as nat + height as nat <= dst.height as nat,
            src_y as nat + height as nat <= src.height as nat,
            forall|idx: int|
                0 <= idx < orig.len()
                && dst_stride > 0
                && (
                    idx / (dst_stride as int) < (dst_y as int)
                    || idx / (dst_stride as int) >= (dst_y as int) + (row as int)
                    || idx % (dst_stride as int) < (dst_col as int)
                    || idx % (dst_stride as int) >= (dst_col as int) + (dst_span as int)
                )
                ==> dst.data@[idx] == orig[idx],
        decreases height_usize - row,
    ))]
    while row < height_usize {
        let dy = dst_y_usize + row;
        let sy = src_y_usize + row;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(dst);
            crate::views::bitmap_fields(src);
            assert(dy * dst_stride + dst_stride <= dst_len) by (nonlinear_arith)
                requires
                    dy < dst.height as usize,
                    dst_len == dst_stride * (dst.height as usize);
            assert(sy * src_stride + src_stride <= src_len) by (nonlinear_arith)
                requires
                    sy < src.height as usize,
                    src_len == src_stride * (src.height as usize);
        }
        let d_start = dy * dst_stride + dst_col;
        let s_start = sy * src_stride + src_col;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before_row = dst.data@;
            let ghost mut after_slice: Seq<u8> = Seq::empty();
        }
        {
            let (prefix, rest) = dst.data.as_mut_slice().split_at_mut(d_start);
            let (dst_slice, suffix) = rest.split_at_mut(dst_span);
            let src_slice = &src.data[s_start..s_start + src_span];
            compose_one_row(
                operator,
                dst_slice,
                src_slice,
                dst_bit,
                src_bit,
                width_usize,
            );
            #[cfg(verus_keep_ghost)]
            proof! {
                after_slice = dst_slice@;
                assert(ext_equal(prefix@, before_row.subrange(0, d_start as int)));
                assert(ext_equal(
                    suffix@,
                    before_row.subrange((d_start + dst_span) as int, dst_len as int),
                ));
            }
            let _ = prefix;
            let _ = suffix;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(dst);
            let pfx = before_row.subrange(0, d_start as int);
            let sfx = before_row.subrange((d_start + dst_span) as int, dst_len as int);
            let right = after_slice + sfx;
            assert(pfx.len() == d_start as int);
            assert(after_slice.len() == dst_span as int);
            assert(sfx.len() == dst_len as int - (d_start + dst_span) as int);
            assert(right.len() == dst_len as int - d_start as int);
            assert(ext_equal(dst.data@, pfx + right));
            assert(dst.data@.len() == dst_len as int);
            assert forall|idx: int|
                0 <= idx < orig.len()
                && dst_stride > 0
                && (
                    idx / (dst_stride as int) < (dst_y as int)
                    || idx / (dst_stride as int) >= (dst_y as int) + ((row + 1) as int)
                    || idx % (dst_stride as int) < (dst_col as int)
                    || idx % (dst_stride as int) >= (dst_col as int) + (dst_span as int)
                )
                implies dst.data@[idx] == orig[idx]
            by {
                if d_start as int <= idx && idx < (d_start + dst_span) as int {
                    let s = dst_stride as int;
                    let off = idx - (dy as int) * s;
                    assert(dst_col as int <= off < (dst_col + dst_span) as int);
                    assert(0 <= off < s);
                    vstd::arithmetic::div_mod::lemma_fundamental_div_mod_converse(
                        idx, s, dy as int, off,
                    );
                    assert(idx / s == dy as int);
                    assert(idx % s == off);
                    assert(false);
                }
                if idx < d_start as int {
                    assert((pfx + right)[idx] == pfx[idx]);
                    assert(dst.data@[idx] == pfx[idx]);
                } else {
                    assert(idx >= (d_start + dst_span) as int);
                    assert((pfx + right)[idx] == right[idx - d_start as int]);
                    assert((after_slice + sfx)[idx - d_start as int]
                        == sfx[idx - (d_start + dst_span) as int]);
                }
                assert(dst.data@[idx] == before_row[idx]);
                if idx / (dst_stride as int) == (dst_y as int) + (row as int) {
                    assert(idx / (dst_stride as int) < (dst_y as int) + ((row + 1) as int));
                    assert(
                        idx % (dst_stride as int) < (dst_col as int)
                        || idx % (dst_stride as int) >= (dst_col as int) + (dst_span as int)
                    );
                }
                assert(before_row[idx] == orig[idx]);
            };
        }
        row += 1;
    }
}

/// Composes a validated non-empty rectangle from `src` into `dst`, dispatching
/// outside row loops on `operator` and alignment for SIMD/NEON vectorization.
#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        crate::spec::bitmap::storage_valid(old(dst)),
        crate::spec::bitmap::storage_valid(src),
        width > 0,
        height > 0,
        src_x as nat + width as nat <= src.width as nat,
        src_y as nat + height as nat <= src.height as nat,
        dst_x as nat + width as nat <= old(dst).width as nat,
        dst_y as nat + height as nat <= old(dst).height as nat,
    ensures
        crate::spec::bitmap::storage_valid(final(dst)),
        final(dst).width == old(dst).width,
        final(dst).height == old(dst).height,
        final(dst).stride == old(dst).stride,
        final(dst).data@.len() == old(dst).data@.len(),
        forall|idx: int|
            0 <= idx < old(dst).data@.len()
            && old(dst).stride > 0
            && (
                idx / (old(dst).stride as int) < (dst_y as int)
                || idx / (old(dst).stride as int) >= (dst_y as int) + (height as int)
                || idx % (old(dst).stride as int) < (dst_x as int) / 8
                || idx % (old(dst).stride as int) >= (dst_x as int) / 8
                    + (((dst_x as int) % 8 + width as int - 1) / 8 + 1)
            )
            ==> final(dst).data@[idx] == old(dst).data@[idx],
))]
#[allow(clippy::too_many_arguments)]
pub(super) fn compose_rect_fast(
    dst: &mut Bitmap,
    src: &Bitmap,
    src_x: u32,
    src_y: u32,
    dst_x: u32,
    dst_y: u32,
    width: u32,
    height: u32,
    operator: CombinationOperator,
) {
    match operator {
        CombinationOperator::Or => compose_rect_for_op(
            CombinationOperator::Or,
            dst,
            src,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        ),
        CombinationOperator::And => compose_rect_for_op(
            CombinationOperator::And,
            dst,
            src,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        ),
        CombinationOperator::Xor => compose_rect_for_op(
            CombinationOperator::Xor,
            dst,
            src,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        ),
        CombinationOperator::Xnor => compose_rect_for_op(
            CombinationOperator::Xnor,
            dst,
            src,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        ),
        CombinationOperator::Replace => compose_rect_for_op(
            CombinationOperator::Replace,
            dst,
            src,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        ),
    }
}
