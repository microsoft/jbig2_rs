#[cfg(verus_keep_ghost)]
use super::colour_spec;
use crate::bitmap::CombinationOperator;
use crate::{Bitmap, DecodeError, LimitKind, Limits, MalformedError, UnsupportedFeature};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Packed, row-major, opaque RGB8 pixels, in red/green/blue channel order.
///
/// Stride is exactly three bytes per pixel. No colour-space conversion or
/// bi-level approximation is performed.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct RgbBitmap {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: usize,
    pub(crate) bytes: Vec<u8>,
}

impl RgbBitmap {
    /// Width in pixels.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == colour_spec::width(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn width(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        self.width
    }

    /// Height in pixels.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == colour_spec::height(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn height(&self) -> u32 {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        self.height
    }

    /// Bytes in one row, exactly `width * 3`.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out == colour_spec::stride(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub const fn stride(&self) -> usize {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        self.stride
    }

    /// RGB8 bytes, without row padding.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out => ensures out@ == colour_spec::bytes(self)))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn bytes(&self) -> &[u8] {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        &self.bytes
    }

    /// Returns the red/green/blue channels, or `None` outside the bitmap.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires colour_spec::storage_valid(self),
        ensures out == colour_spec::pixel(self, x, y),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        if x >= self.width || y >= self.height {
            return None;
        }
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::pixel_offset_bounds(self, x, y); }
        let offset = usize::try_from(y)
            .ok()?
            .checked_mul(self.stride)?
            .checked_add(usize::try_from(x).ok()?.checked_mul(3)?)?;
        Some([
            *self.bytes.get(offset)?,
            *self.bytes.get(offset + 1)?,
            *self.bytes.get(offset + 2)?,
        ])
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(bitmap) => colour_spec::storage_valid(&bitmap)
                && bitmap.width == width && bitmap.height == height
                && bitmap.bytes@.len() <= limits.max_work_units
                && forall|index: int| 0 <= index < bitmap.bytes@.len() ==>
                    bitmap.bytes@[index] == if black { 0u8 } else { 255u8 },
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn filled(
        width: u32,
        height: u32,
        black: bool,
        limits: &Limits,
    ) -> Result<Self, DecodeError> {
        crate::segment_data::enforce_dimension_limits(width, height, limits)?;
        let stride = usize::try_from(width)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(3)
            .ok_or(MalformedError::IntegerOverflow)?;
        let size = stride
            .checked_mul(usize::try_from(height).map_err(|_| MalformedError::IntegerOverflow)?)
            .ok_or(MalformedError::IntegerOverflow)?;
        let requested = u64::try_from(size).map_err(|_| MalformedError::IntegerOverflow)?;
        super::document::limit(LimitKind::Work, requested, limits.max_work_units)?;
        super::document::limit(
            LimitKind::Memory,
            requested,
            limits.max_decoder_managed_memory_bytes,
        )?;
        super::document::limit(
            LimitKind::RetainedState,
            requested,
            limits.max_retained_state_bytes,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve(size)
            .map_err(|_| DecodeError::Allocation {
                requested_bytes: requested,
            })?;
        let allocated =
            u64::try_from(bytes.capacity()).map_err(|_| MalformedError::IntegerOverflow)?;
        super::document::limit(
            LimitKind::Memory,
            allocated,
            limits.max_decoder_managed_memory_bytes,
        )?;
        super::document::limit(
            LimitKind::RetainedState,
            allocated,
            limits.max_retained_state_bytes,
        )?;
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant index <= size, bytes@.len() == index,
                forall|done: int| 0 <= done < index ==> bytes@[done] == if black {0u8} else {255u8},
            decreases size - index,
        ))]
        while index < size {
            bytes.push(if black { 0 } else { 255 });
            index += 1;
        }
        let bitmap = Self {
            width,
            height,
            stride,
            bytes,
        };
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(&bitmap); }
        Ok(bitmap)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires colour_spec::storage_valid(old(self)),
        ensures colour_spec::storage_valid(final(self)),
            final(self).width == old(self).width, final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            out.is_ok() ==> x < final(self).width && y < final(self).height
                && forall|channel:int| 0 <= channel < 3 ==>
                    final(self).bytes@[y as int * final(self).stride as int + x as int * 3 + channel] == value@[channel],
            out.is_ok() ==> forall|index:int| 0 <= index < final(self).bytes@.len()
                && !(y as int * final(self).stride as int + x as int * 3 <= index
                    < y as int * final(self).stride as int + x as int * 3 + 3)
                ==> final(self).bytes@[index] == old(self).bytes@[index],
    ))]
    pub(super) fn set(&mut self, x: u32, y: u32, value: [u8; 3]) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        if x >= self.width || y >= self.height {
            return Err(MalformedError::InvalidPageState("RGB pixel outside page bounds").into());
        }
        let row = usize::try_from(y)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let column = usize::try_from(x)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(3)
            .ok_or(MalformedError::IntegerOverflow)?;
        let offset = row
            .checked_add(column)
            .ok_or(MalformedError::IntegerOverflow)?;
        if offset
            .checked_add(2)
            .ok_or(MalformedError::IntegerOverflow)?
            >= self.bytes.len()
        {
            return Err(MalformedError::InvalidPageState("invalid RGB storage").into());
        }
        self.bytes[offset] = value[0];
        self.bytes[offset + 1] = value[1];
        self.bytes[offset + 2] = value[2];
        #[cfg(verus_keep_ghost)]
        proof! {
            colour_spec::bitmap_fields(self);
            assert(value@ == seq![value[0], value[1], value[2]]);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires colour_spec::storage_valid(old(self)),
            crate::spec::bitmap::storage_valid(mask),
        ensures colour_spec::storage_valid(final(self)),
            final(self).width == old(self).width, final(self).height == old(self).height,
            final(self).stride == old(self).stride,
    ))]
    pub(super) fn compose(
        &mut self,
        mask: &Bitmap,
        x: u32,
        y: u32,
        foreground: Option<[u8; 3]>,
        operator: CombinationOperator,
    ) -> Result<(), DecodeError> {
        let replace = matches!(operator, CombinationOperator::Replace);
        if foreground.is_some() && !replace
            || foreground.is_none() && !replace && !matches!(operator, CombinationOperator::Or)
        {
            return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
        }
        let width = self.width.saturating_sub(x).min(mask.width());
        let height = self.height.saturating_sub(y).min(mask.height());
        let mut row = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant row <= height, colour_spec::storage_valid(self),
                crate::spec::bitmap::storage_valid(mask),
                self.width == old(self).width, self.height == old(self).height, self.stride == old(self).stride,
            decreases height - row,
        ))]
        while row < height {
            let mut column = 0_u32;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant column <= width, colour_spec::storage_valid(self),
                    crate::spec::bitmap::storage_valid(mask),
                    self.width == old(self).width, self.height == old(self).height, self.stride == old(self).stride,
                decreases width - column,
            ))]
            while column < width {
                let ink = mask
                    .pixel(column, row)
                    .ok_or(MalformedError::InvalidPageState(
                        "RGB mask coordinate outside source",
                    ))?;
                self.paint(
                    x.checked_add(column)
                        .ok_or(MalformedError::IntegerOverflow)?,
                    y.checked_add(row).ok_or(MalformedError::IntegerOverflow)?,
                    ink,
                    foreground,
                    replace,
                )?;
                column += 1;
            }
            row += 1;
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires colour_spec::storage_valid(old(self)),
        ensures colour_spec::storage_valid(final(self)),
            final(self).width == old(self).width, final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            out.is_ok() ==> x < final(self).width && y < final(self).height
                && forall|channel:int| 0 <= channel < 3 ==>
                    final(self).bytes@[y as int * final(self).stride as int + x as int * 3 + channel]
                        == #[trigger] colour_spec::paint_channel(
                            old(self).bytes@[y as int * old(self).stride as int + x as int * 3 + channel],
                            ink, foreground, replace, channel,
                        ),
            out.is_ok() ==> forall|index:int| 0 <= index < final(self).bytes@.len()
                && !(y as int * final(self).stride as int + x as int * 3 <= index
                    < y as int * final(self).stride as int + x as int * 3 + 3)
                ==> final(self).bytes@[index] == old(self).bytes@[index],
    ))]
    fn paint(
        &mut self,
        x: u32,
        y: u32,
        ink: bool,
        foreground: Option<[u8; 3]>,
        replace: bool,
    ) -> Result<(), DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::bitmap_fields(self); }
        if x >= self.width || y >= self.height {
            return Err(MalformedError::InvalidPageState("RGB paint outside page bounds").into());
        }
        #[cfg(verus_keep_ghost)]
        proof! { colour_spec::pixel_offset_bounds(self, x, y); }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = self.bytes@; }
        match foreground {
            Some(rgb) if ink => {
                self.set(x, y, rgb)?;
            }
            None if ink => {
                self.set(x, y, [0, 0, 0])?;
            }
            None if replace => {
                self.set(x, y, [255, 255, 255])?;
            }
            Some(_) | None => {}
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            colour_spec::bitmap_fields(self);
            colour_spec::pixel_offset_bounds(self, x, y);
            reveal(colour_spec::paint_channel);
            assert forall|channel:int| 0 <= channel < 3 implies
                self.bytes@[y as int * self.stride as int + x as int * 3 + channel]
                    == #[trigger] colour_spec::paint_channel(
                        before[y as int * self.stride as int + x as int * 3 + channel],
                        ink, foreground, replace, channel,
                    ) by {
                let offset = y as int * self.stride as int + x as int * 3 + channel;
                match foreground {
                    Some(rgb) => {
                        if ink { assert(self.bytes@[offset] == rgb@[channel]); }
                        else { assert(self.bytes@[offset] == before[offset]); }
                    },
                    None => {
                        if ink { assert(self.bytes@[offset] == 0); }
                        else if replace { assert(self.bytes@[offset] == 255); }
                        else { assert(self.bytes@[offset] == before[offset]); }
                    },
                }
            }
        }
        Ok(())
    }
}
