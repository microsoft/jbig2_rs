use super::generic_context::{packed_row_bits, packed_row_byte, packed_row_start, queue_exhausted};
use super::TemplateOffset;
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

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn context_at(bitmap: &Bitmap, x: u32, y: u32, history: usize, lag: u8) -> usize {
    let x = i64::from(x);
    let y = i64::from(y);
    combine(
        packed_row_bits(bitmap, x, y - 2, -2, 5),
        packed_row_bits(bitmap, x, y - 1, -3, 6),
        packed_row_byte(bitmap, packed_row_start(bitmap, y), x - i64::from(lag)) & 0x80 != 0,
        history,
    )
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
