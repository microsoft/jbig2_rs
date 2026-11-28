use super::support::sample_bitmap;
use super::{
    GenericContextDescriptor, GenericRowContext, GenericSlot, GenericTemplate, TemplateOffset,
};
use crate::Bitmap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive.is_none(),
))]
pub(super) const fn generic_slot(x: i32, y: i32) -> GenericSlot {
    GenericSlot {
        nominal: TemplateOffset::new(x, y),
        adaptive: None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive == Some(number),
))]
pub(super) const fn generic_at(x: i32, y: i32, number: u8) -> GenericSlot {
    GenericSlot {
        nominal: TemplateOffset::new(x, y),
        adaptive: Some(number),
    }
}

macro_rules! generic_slot_const {
    ($x:expr, $y:expr) => {
        GenericSlot {
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: None,
        }
    };
}

macro_rules! generic_at_const {
    ($x:expr, $y:expr, $number:expr) => {
        GenericSlot {
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: Some($number),
        }
    };
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const GENERIC_TEMPLATE_0: [GenericSlot; 16] = [
    generic_at_const!(-2, -2, 4),
    generic_slot_const!(-1, -2),
    generic_slot_const!(0, -2),
    generic_slot_const!(1, -2),
    generic_at_const!(2, -2, 3),
    generic_at_const!(-3, -1, 2),
    generic_slot_const!(-2, -1),
    generic_slot_const!(-1, -1),
    generic_slot_const!(0, -1),
    generic_slot_const!(1, -1),
    generic_slot_const!(2, -1),
    generic_at_const!(3, -1, 1),
    generic_slot_const!(-4, 0),
    generic_slot_const!(-3, 0),
    generic_slot_const!(-2, 0),
    generic_slot_const!(-1, 0),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const GENERIC_TEMPLATE_0_EXTENDED: [GenericSlot; 16] = [
    generic_at_const!(-2, -2, 11),
    generic_at_const!(-1, -2, 4),
    generic_at_const!(0, -2, 2),
    generic_at_const!(1, -2, 5),
    generic_at_const!(2, -2, 9),
    generic_at_const!(-3, -1, 12),
    generic_at_const!(-2, -1, 3),
    generic_slot_const!(-1, -1),
    generic_slot_const!(0, -1),
    generic_slot_const!(1, -1),
    generic_at_const!(2, -1, 6),
    generic_at_const!(3, -1, 10),
    generic_at_const!(-4, 0, 8),
    generic_at_const!(-3, 0, 7),
    generic_at_const!(-2, 0, 1),
    generic_slot_const!(-1, 0),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const GENERIC_TEMPLATE_1: [GenericSlot; 13] = [
    generic_slot_const!(-1, -2),
    generic_slot_const!(0, -2),
    generic_slot_const!(1, -2),
    generic_slot_const!(2, -2),
    generic_slot_const!(-2, -1),
    generic_slot_const!(-1, -1),
    generic_slot_const!(0, -1),
    generic_slot_const!(1, -1),
    generic_slot_const!(2, -1),
    generic_at_const!(3, -1, 1),
    generic_slot_const!(-3, 0),
    generic_slot_const!(-2, 0),
    generic_slot_const!(-1, 0),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const GENERIC_TEMPLATE_2: [GenericSlot; 10] = [
    generic_slot_const!(-1, -2),
    generic_slot_const!(0, -2),
    generic_slot_const!(1, -2),
    generic_slot_const!(-2, -1),
    generic_slot_const!(-1, -1),
    generic_slot_const!(0, -1),
    generic_slot_const!(1, -1),
    generic_at_const!(2, -1, 1),
    generic_slot_const!(-2, 0),
    generic_slot_const!(-1, 0),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const GENERIC_TEMPLATE_3: [GenericSlot; 10] = [
    generic_slot_const!(-3, -1),
    generic_slot_const!(-2, -1),
    generic_slot_const!(-1, -1),
    generic_slot_const!(0, -1),
    generic_slot_const!(1, -1),
    generic_at_const!(2, -1, 1),
    generic_slot_const!(-4, 0),
    generic_slot_const!(-3, 0),
    generic_slot_const!(-2, 0),
    generic_slot_const!(-1, 0),
];

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == if 1 <= number && number <= 12 { adaptive@[(number - 1) as int] } else { nominal },
))]
pub(super) fn generic_adaptive_offset(
    adaptive: &[TemplateOffset; 12],
    number: u8,
    nominal: TemplateOffset,
) -> TemplateOffset {
    match number {
        1 => adaptive[0],
        2 => adaptive[1],
        3 => adaptive[2],
        4 => adaptive[3],
        5 => adaptive[4],
        6 => adaptive[5],
        7 => adaptive[6],
        8 => adaptive[7],
        9 => adaptive[8],
        10 => adaptive[9],
        11 => adaptive[10],
        12 => adaptive[11],
        _ => nominal,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires crate::spec::bitmap::storage_valid(bitmap),
))]
#[allow(clippy::bool_to_int_with_if)]
pub(super) fn generic_context(
    bitmap: &Bitmap,
    x: u32,
    y: u32,
    template: GenericTemplate,
    extended: bool,
    adaptive: &[TemplateOffset; 12],
) -> usize {
    let count = match template {
        GenericTemplate::Template0 => 16usize,
        GenericTemplate::Template1 => 13usize,
        GenericTemplate::Template2 | GenericTemplate::Template3 => 10usize,
    };
    let mut context = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= count,
            count <= 16,
            count == match template {
                GenericTemplate::Template0 => 16usize,
                GenericTemplate::Template1 => 13usize,
                GenericTemplate::Template2 | GenericTemplate::Template3 => 10usize,
            },
            crate::spec::bitmap::storage_valid(bitmap),
        decreases count - index,
    ))]
    while index < count {
        let slot = match (template, extended) {
            (GenericTemplate::Template0, true) => GENERIC_TEMPLATE_0_EXTENDED[index],
            (GenericTemplate::Template0, false) => GENERIC_TEMPLATE_0[index],
            (GenericTemplate::Template1, _) => GENERIC_TEMPLATE_1[index],
            (GenericTemplate::Template2, _) => GENERIC_TEMPLATE_2[index],
            (GenericTemplate::Template3, _) => GENERIC_TEMPLATE_3[index],
        };
        let offset = match slot.adaptive {
            Some(number) => generic_adaptive_offset(adaptive, number, slot.nominal),
            None => slot.nominal,
        };
        let bit = if sample_bitmap(bitmap, x, y, offset) {
            1
        } else {
            0
        };
        context = (context << 1) | bit;
        index += 1;
    }
    context
}

impl GenericContextDescriptor {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn new(
        template: GenericTemplate,
        extended: bool,
        adaptive: &[TemplateOffset; 12],
    ) -> Self {
        match (template, extended) {
            (GenericTemplate::Template0, true) => {
                if *adaptive
                    == [
                        TemplateOffset::new(-2, 0),
                        TemplateOffset::new(0, -2),
                        TemplateOffset::new(-2, -1),
                        TemplateOffset::new(-1, -2),
                        TemplateOffset::new(1, -2),
                        TemplateOffset::new(2, -1),
                        TemplateOffset::new(-3, 0),
                        TemplateOffset::new(-4, 0),
                        TemplateOffset::new(2, -2),
                        TemplateOffset::new(3, -1),
                        TemplateOffset::new(-2, -2),
                        TemplateOffset::new(-3, -1),
                    ]
                {
                    return Self::NominalTemplate0;
                }
                let mut offsets = [TemplateOffset { x: 0, y: 0 }; 16];
                let mut index = 0_usize;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant index <= offsets@.len(),
                    decreases offsets@.len() - index,
                ))]
                while index < offsets.len() {
                    let slot = GENERIC_TEMPLATE_0_EXTENDED[index];
                    offsets[index] = match slot.adaptive {
                        Some(number) => generic_adaptive_offset(adaptive, number, slot.nominal),
                        None => slot.nominal,
                    };
                    index += 1;
                }
                Self::Template0Extended(offsets)
            }
            (GenericTemplate::Template0, false) => {
                let values = [adaptive[0], adaptive[1], adaptive[2], adaptive[3]];
                if values
                    == [
                        TemplateOffset::new(3, -1),
                        TemplateOffset::new(-3, -1),
                        TemplateOffset::new(2, -2),
                        TemplateOffset::new(-2, -2),
                    ]
                {
                    Self::NominalTemplate0
                } else {
                    Self::Template0(values)
                }
            }
            (GenericTemplate::Template1, _) => {
                if adaptive[0] == TemplateOffset::new(3, -1) {
                    Self::NominalTemplate1
                } else {
                    Self::Template1(adaptive[0])
                }
            }
            (GenericTemplate::Template2, _) => {
                if adaptive[0] == TemplateOffset::new(2, -1) {
                    Self::NominalTemplate2
                } else {
                    Self::Template2(adaptive[0])
                }
            }
            (GenericTemplate::Template3, _) => {
                if adaptive[0] == TemplateOffset::new(2, -1) {
                    Self::NominalTemplate3
                } else {
                    Self::Template3(adaptive[0])
                }
            }
        }
    }

    #[allow(clippy::similar_names)]
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn context(&self, bitmap: &Bitmap, x: u32, y: u32, history: usize) -> usize {
        match self {
            Self::NominalTemplate0 => {
                let top = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -2, 5);
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -3, 7);
                (top << 11) | (previous << 4) | history & 0x0f
            }
            Self::Template0(adaptive) => {
                let top_fixed = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -1, 3);
                let previous_fixed = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -2, 5);
                let a4 = usize::from(sample_cached(bitmap, x, y, adaptive[3]));
                let a3 = usize::from(sample_cached(bitmap, x, y, adaptive[2]));
                let a2 = usize::from(sample_cached(bitmap, x, y, adaptive[1]));
                let a1 = usize::from(sample_cached(bitmap, x, y, adaptive[0]));
                let mut context = (a4 << 3) | top_fixed;
                context = (context << 1) | a3;
                context = (context << 1) | a2;
                context = (context << 5) | previous_fixed;
                context = (context << 1) | a1;
                (context << 4) | history & 0x0f
            }
            Self::Template0Extended(offsets) => {
                let mut context = 0_usize;
                let mut index = 0_usize;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant index <= offsets@.len(),
                    decreases offsets@.len() - index,
                ))]
                while index < offsets.len() {
                    context =
                        (context << 1) | usize::from(sample_cached(bitmap, x, y, offsets[index]));
                    index += 1;
                }
                context
            }
            Self::NominalTemplate1 => {
                let top = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -1, 4);
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -2, 6);
                (top << 9) | (previous << 3) | history & 0x07
            }
            Self::Template1(adaptive) => {
                let top = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -1, 4);
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -2, 5);
                let adaptive = usize::from(sample_cached(bitmap, x, y, *adaptive));
                (((top << 5) | previous) << 4) | (adaptive << 3) | history & 0x07
            }
            Self::NominalTemplate2 => {
                let top = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -1, 3);
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -2, 5);
                (top << 7) | (previous << 2) | history & 0x03
            }
            Self::Template2(adaptive) => {
                let top = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 2, -1, 3);
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -2, 4);
                let adaptive = usize::from(sample_cached(bitmap, x, y, *adaptive));
                (((top << 4) | previous) << 3) | (adaptive << 2) | history & 0x03
            }
            Self::NominalTemplate3 => {
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -3, 6);
                (previous << 4) | history & 0x0f
            }
            Self::Template3(adaptive) => {
                let previous = packed_row_bits(bitmap, i64::from(x), i64::from(y) - 1, -3, 5);
                let adaptive = usize::from(sample_cached(bitmap, x, y, *adaptive));
                (previous << 5) | (adaptive << 4) | history & 0x0f
            }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        *final(remaining) == if *old(remaining) == 0 { 0u8 } else { (*old(remaining) - 1) as u8 },
        out == (*final(remaining) == 0),
))]
#[inline]
pub(super) fn queue_exhausted(remaining: &mut u8) -> bool {
    if *remaining == 0 {
        return true;
    }
    *remaining -= 1;
    *remaining == 0
}

impl GenericRowContext {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn new(descriptor: GenericContextDescriptor, bitmap: &Bitmap, y: u32) -> Self {
        let y = i64::from(y);
        match descriptor {
            GenericContextDescriptor::NominalTemplate0 => Self::Template0 {
                top: packed_row_bits(bitmap, 0, y - 2, -2, 5),
                previous: packed_row_bits(bitmap, 0, y - 1, -3, 7),
                top_row: packed_row_start(bitmap, y - 2),
                previous_row: packed_row_start(bitmap, y - 1),
                top_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 2), 3),
                previous_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 1), 4),
                remaining: 8,
            },
            GenericContextDescriptor::NominalTemplate1 => Self::Template1 {
                top: packed_row_bits(bitmap, 0, y - 2, -1, 4),
                previous: packed_row_bits(bitmap, 0, y - 1, -2, 6),
                top_row: packed_row_start(bitmap, y - 2),
                previous_row: packed_row_start(bitmap, y - 1),
                top_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 2), 3),
                previous_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 1), 4),
                remaining: 8,
            },
            GenericContextDescriptor::NominalTemplate2 => Self::Template2 {
                top: packed_row_bits(bitmap, 0, y - 2, -1, 3),
                previous: packed_row_bits(bitmap, 0, y - 1, -2, 5),
                top_row: packed_row_start(bitmap, y - 2),
                previous_row: packed_row_start(bitmap, y - 1),
                top_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 2), 2),
                previous_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 1), 3),
                remaining: 8,
            },
            GenericContextDescriptor::NominalTemplate3 => Self::Template3 {
                previous: packed_row_bits(bitmap, 0, y - 1, -3, 6),
                previous_row: packed_row_start(bitmap, y - 1),
                previous_queue: packed_row_byte(bitmap, packed_row_start(bitmap, y - 1), 3),
                remaining: 8,
            },
            _ => Self::Adaptive(descriptor),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn context_and_advance(
        &mut self,
        bitmap: &Bitmap,
        x: u32,
        y: u32,
        history: usize,
    ) -> usize {
        let sample_x = i64::from(x);
        match self {
            Self::Template0 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                let context = (*top << 11) | (*previous << 4) | history & 0x0f;
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x1f;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x7f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, sample_x + 4);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, sample_x + 5);
                    *remaining = 8;
                }
                context
            }
            Self::Template1 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                let context = (*top << 9) | (*previous << 3) | history & 0x07;
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x0f;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x3f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, sample_x + 4);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, sample_x + 5);
                    *remaining = 8;
                }
                context
            }
            Self::Template2 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                let context = (*top << 7) | (*previous << 2) | history & 0x03;
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x07;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x1f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, sample_x + 3);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, sample_x + 4);
                    *remaining = 8;
                }
                context
            }
            Self::Template3 {
                previous,
                previous_row,
                previous_queue,
                remaining,
            } => {
                let context = (*previous << 4) | history & 0x0f;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x3f;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *previous_queue = packed_row_byte(bitmap, *previous_row, sample_x + 4);
                    *remaining = 8;
                }
                context
            }
            Self::Adaptive(descriptor) => descriptor.context(bitmap, x, y, history),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn advance(&mut self, bitmap: &Bitmap, x: u32) {
        let x = i64::from(x);
        match self {
            Self::Template0 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x1f;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x7f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, x + 4);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, x + 5);
                    *remaining = 8;
                }
            }
            Self::Template1 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x0f;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x3f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, x + 4);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, x + 5);
                    *remaining = 8;
                }
            }
            Self::Template2 {
                top,
                previous,
                top_row,
                previous_row,
                top_queue,
                previous_queue,
                remaining,
            } => {
                *top = ((*top << 1) | usize::from(*top_queue & 0x80 != 0)) & 0x07;
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x1f;
                *top_queue <<= 1;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *top_queue = packed_row_byte(bitmap, *top_row, x + 3);
                    *previous_queue = packed_row_byte(bitmap, *previous_row, x + 4);
                    *remaining = 8;
                }
            }
            Self::Template3 {
                previous,
                previous_row,
                previous_queue,
                remaining,
            } => {
                *previous = ((*previous << 1) | usize::from(*previous_queue & 0x80 != 0)) & 0x3f;
                *previous_queue <<= 1;
                if queue_exhausted(remaining) {
                    *previous_queue = packed_row_byte(bitmap, *previous_row, x + 4);
                    *remaining = 8;
                }
            }
            Self::Adaptive(_) => {}
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn sample_cached(bitmap: &Bitmap, x: u32, y: u32, offset: TemplateOffset) -> bool {
    packed_pixel(
        bitmap,
        i64::from(x) + i64::from(offset.x),
        i64::from(y) + i64::from(offset.y),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures (x < 0 || y < 0 || x >= bitmap.width as i64 || y >= bitmap.height as i64) ==> !out,
))]
pub(super) fn packed_pixel(bitmap: &Bitmap, x: i64, y: i64) -> bool {
    if x < 0 || y < 0 || x >= i64::from(bitmap.width) || y >= i64::from(bitmap.height) {
        return false;
    }
    let row = y as usize;
    let column = x as usize / 8;
    let Some(index) = row
        .checked_mul(bitmap.stride)
        .and_then(|start| start.checked_add(column))
    else {
        return false;
    };
    let mask = match x as u32 % 8 {
        0 => 0x80,
        1 => 0x40,
        2 => 0x20,
        3 => 0x10,
        4 => 0x08,
        5 => 0x04,
        6 => 0x02,
        _ => 0x01,
    };
    match bitmap.data.get(index) {
        Some(byte) => *byte & mask != 0,
        None => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        (row < 0 || row >= bitmap.height as i64) ==> out.is_none(),
        match out {
            Some(start) => 0 <= row < bitmap.height as i64 && start as nat == row as nat * bitmap.stride as nat,
            None => true,
        },
))]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn packed_row_start(bitmap: &Bitmap, row: i64) -> Option<usize> {
    if row < 0 || row >= i64::from(bitmap.height) {
        return None;
    }
    (row as usize).checked_mul(bitmap.stride)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures (row_start.is_none() || x < 0 || x >= bitmap.width as i64) ==> out == 0u8,
))]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn packed_row_byte(bitmap: &Bitmap, row_start: Option<usize>, x: i64) -> u8 {
    let Some(row_start) = row_start else {
        return 0;
    };
    if x < 0 || x >= i64::from(bitmap.width) {
        return 0;
    }
    let x = x as usize;
    let Some(index) = row_start.checked_add(x / 8) else {
        return 0;
    };
    let Some(first) = bitmap.data.get(index) else {
        return 0;
    };
    let second = match index.checked_add(1) {
        Some(next) => match bitmap.data.get(next) {
            Some(byte) => *byte,
            None => 0,
        },
        None => 0,
    };
    let combined = (u16::from(*first) << 8) | u16::from(second);
    let shift = match x & 7 {
        0 => 8,
        1 => 7,
        2 => 6,
        3 => 5,
        4 => 4,
        5 => 3,
        6 => 2,
        _ => 1,
    };
    let shifted = (combined >> shift) & 0xff;
    let Ok(value) = u8::try_from(shifted) else {
        return 0;
    };
    let remaining = usize::try_from(bitmap.width)
        .ok()
        .and_then(|width| width.checked_sub(x))
        .unwrap_or(0);
    let mask = match remaining {
        0 => 0x00,
        1 => 0x80,
        2 => 0xc0,
        3 => 0xe0,
        4 => 0xf0,
        5 => 0xf8,
        6 => 0xfc,
        7 => 0xfe,
        _ => 0xff,
    };
    value & mask
}

// Specialize fixed-width refinement neighborhoods at their call sites.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::inline_always
)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[inline(always)]
pub(super) fn packed_row_bits(
    bitmap: &Bitmap,
    center_x: i64,
    row: i64,
    left: i32,
    count: u8,
) -> usize {
    if count == 0 || count > 8 || row < 0 || row >= i64::from(bitmap.height) {
        return 0;
    }
    let Some(start) = center_x.checked_add(i64::from(left)) else {
        return 0;
    };
    let Some(end) = start.checked_add(i64::from(count)) else {
        return 0;
    };
    if start < 0 || end > i64::from(bitmap.width) {
        let mut bits = 0_usize;
        let mut offset = 0_u8;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant offset <= count,
            decreases count - offset,
        ))]
        while offset < count {
            let Some(sample_x) = start.checked_add(i64::from(offset)) else {
                return 0;
            };
            let Some(shifted) = bits.checked_mul(2) else {
                return 0;
            };
            bits = shifted | usize::from(packed_pixel(bitmap, sample_x, row));
            offset += 1;
        }
        return bits;
    }

    let row = row as usize;
    let start = start as usize;
    let Some(index) = row
        .checked_mul(bitmap.stride)
        .and_then(|base| base.checked_add(start / 8))
    else {
        return 0;
    };
    let Some(first) = bitmap.data.get(index) else {
        return 0;
    };
    let first = *first;
    let bit_offset = start & 7;
    let count = usize::from(count);
    let mask = match count {
        1 => 0x01,
        2 => 0x03,
        3 => 0x07,
        4 => 0x0f,
        5 => 0x1f,
        6 => 0x3f,
        7 => 0x7f,
        _ => 0xff,
    };
    let Some(total_bits) = bit_offset.checked_add(count) else {
        return 0;
    };
    if total_bits <= 8 {
        return (usize::from(first) >> (8 - total_bits)) & mask;
    }
    let second = match index.checked_add(1) {
        Some(next) => match bitmap.data.get(next) {
            Some(byte) => *byte,
            None => 0,
        },
        None => 0,
    };
    let combined = (usize::from(first) << 8) | usize::from(second);
    let Some(shift) = 16_usize.checked_sub(total_bits) else {
        return 0;
    };
    (combined >> shift) & mask
}
