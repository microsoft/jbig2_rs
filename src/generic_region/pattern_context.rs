use super::generic_context::{packed_row_bits, packed_row_byte, packed_row_start, queue_exhausted};
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::TemplateOffset;
use super::{decode_probability_signal, probability_signal_error, GenericRegionError};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(lag) => lag >= 8
            && adaptive[0].x == -(lag as i32) && adaptive[0].y == 0
            && adaptive[1].x == -3 && adaptive[1].y == -1
            && adaptive[2].x == 2 && adaptive[2].y == -2
            && adaptive[3].x == -2 && adaptive[3].y == -2,
        None => true,
    },
))]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn pattern_lag(adaptive: &[TemplateOffset; 4]) -> Option<u8> {
    if adaptive[0].x < -255
        || adaptive[0].x > -8
        || adaptive[0].y != 0
        || adaptive[1].x != -3
        || adaptive[1].y != -1
        || adaptive[2].x != 2
        || adaptive[2].y != -2
        || adaptive[3].x != -2
        || adaptive[3].y != -2
    {
        return None;
    }
    Some((-adaptive[0].x) as u8)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out < 65536,
        out & 15 == history & 15,
        (out >> 4) & 1 == if lag_bit { 1usize } else { 0usize },
        (out >> 5) & 63 == previous & 63,
        out >> 11 == top & 31,
))]
fn combine(top: usize, previous: usize, lag_bit: bool, history: usize) -> usize {
    let lag_mask = if lag_bit { 16_usize } else { 0 };
    let context = ((top & 31) << 11) | ((previous & 63) << 5) | lag_mask | (history & 15);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(context < 65536
            && context & 15 == history & 15
            && (context >> 4) & 1 == if lag_bit { 1usize } else { 0usize }
            && (context >> 5) & 63 == previous & 63
            && context >> 11 == top & 31) by(bit_vector)
            requires lag_mask == if lag_bit { 16usize } else { 0usize },
                context == ((top & 31) << 11) | ((previous & 63) << 5)
                    | lag_mask | (history & 15);
    }
    context
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) struct PatternRowContext {
    top: usize,
    previous: usize,
    top_row: Option<usize>,
    previous_row: Option<usize>,
    current_row: Option<usize>,
    top_queue: u8,
    previous_queue: u8,
    remaining: u8,
    lag: u8,
}

impl PatternRowContext {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn new(bitmap: &Bitmap, y: u32, lag: u8) -> Self {
        let y = i64::from(y);
        let top_row = packed_row_start(bitmap, y - 2);
        let previous_row = packed_row_start(bitmap, y - 1);
        Self {
            top: packed_row_bits(bitmap, 0, y - 2, -2, 5),
            previous: packed_row_bits(bitmap, 0, y - 1, -3, 6),
            top_row,
            previous_row,
            current_row: packed_row_start(bitmap, y),
            top_queue: packed_row_byte(bitmap, top_row, 3),
            previous_queue: packed_row_byte(bitmap, previous_row, 3),
            remaining: 8,
            lag,
        }
    }
}

#[allow(clippy::ptr_arg)]
#[inline(never)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
            requires
                crate::spec::bitmap::storage_valid(old(bitmap)),
                rows <= old(bitmap).height,
                crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
                generic_region_spec::contexts_valid(old(contexts)@),
            ensures
                crate::spec::bitmap::storage_valid(final(bitmap)),
                final(bitmap).width == old(bitmap).width,
                final(bitmap).height == old(bitmap).height,
                final(bitmap).stride == old(bitmap).stride,
                generic_region_spec::contexts_valid(final(contexts)@),
                out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
        ))]
pub(super) fn try_decode(
    bitmap: &mut Bitmap,
    rows: u32,
    adaptive: &[TemplateOffset; 12],
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<bool, GenericRegionError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    let Some(lag) = pattern_lag(&[adaptive[0], adaptive[1], adaptive[2], adaptive[3]]) else {
        return Ok(false);
    };
    let width = bitmap.width;
    #[cfg(verus_keep_ghost)]
    let height = bitmap.height;
    #[cfg(verus_keep_ghost)]
    let stride = bitmap.stride;
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    y <= rows <= bitmap.height,
                    bitmap.width == width,
                    bitmap.height == height,
                    bitmap.stride == stride,
                    width == old(bitmap).width,
                    height == old(bitmap).height,
                    stride == old(bitmap).stride,
                    crate::spec::bitmap::storage_valid(bitmap),
                    crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                    generic_region_spec::contexts_valid(contexts@),
                decreases rows - y,
            ))]
    while y < rows {
        let mut row = PatternRowContext::new(bitmap, y, lag);
        let mut history = 0_usize;
        let mut x = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        x <= width,
                        y < rows <= bitmap.height,
                        bitmap.width == width,
                        bitmap.height == height,
                        bitmap.stride == stride,
                        width == old(bitmap).width,
                        height == old(bitmap).height,
                        stride == old(bitmap).stride,
                        crate::spec::bitmap::storage_valid(bitmap),
                        crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                        generic_region_spec::contexts_valid(contexts@),
                    decreases width - x,
                ))]
        while x < width {
            let context = row.context_and_advance(bitmap, x, history);
            let signal = decode_probability_signal(decoder, contexts, context);
            if signal > 1 {
                return Err(probability_signal_error(
                    decoder,
                    context,
                    contexts.len(),
                    signal,
                ));
            }
            history = ((history << 1) | usize::from(signal)) & 15;
            if signal != 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(bitmap);
                    reveal(crate::spec::bitmap::coordinates_in_bounds);
                }
                bitmap.set_pixel(x, y, true)?;
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(bitmap);
                }
            }
            x += 1;
        }
        y += 1;
    }
    Ok(true)
}

impl PatternRowContext {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn context_and_advance(&mut self, bitmap: &Bitmap, x: u32, history: usize) -> usize {
        let lag = packed_row_byte(bitmap, self.current_row, i64::from(x) - i64::from(self.lag));
        let context = combine(self.top, self.previous, lag & 0x80 != 0, history);
        self.advance(bitmap, x);
        context
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn advance(&mut self, bitmap: &Bitmap, x: u32) {
        self.top = ((self.top << 1) | usize::from(self.top_queue & 0x80 != 0)) & 31;
        self.previous = ((self.previous << 1) | usize::from(self.previous_queue & 0x80 != 0)) & 63;
        self.top_queue <<= 1;
        self.previous_queue <<= 1;
        if queue_exhausted(&mut self.remaining) {
            self.top_queue = packed_row_byte(bitmap, self.top_row, i64::from(x) + 4);
            self.previous_queue = packed_row_byte(bitmap, self.previous_row, i64::from(x) + 4);
            self.remaining = 8;
        }
    }
}
