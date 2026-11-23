use super::chunks::{
    read_chunk, read_shifted_byte, read_shifted_chunk, read_shifted_tail, write_chunk,
};
use super::{Bitmap, CombinationOperator, RowBlit};
use crate::MalformedError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl Bitmap {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::spec::bitmap::storage_valid(old(self)),
            crate::spec::bitmap::storage_valid(source),
        ensures
            crate::spec::bitmap::storage_valid(final(self)),
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            !crate::spec::bitmap::region_request_valid(
                old(self), source, source_x, source_y, destination_x, destination_y, width, height,
            ) ==> matches!(out, Err(MalformedError::IntegerOverflow)),
            (!crate::spec::bitmap::region_request_valid(
                old(self), source, source_x, source_y, destination_x, destination_y, width, height,
            ) || width == 0 || height == 0) ==> final(self).data@ == old(self).data@,
            (crate::spec::bitmap::region_request_valid(
                old(self), source, source_x, source_y, destination_x, destination_y, width, height,
            ) && (width == 0 || height == 0)) ==> out.is_ok(),
    ))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compose_region(
        &mut self,
        source: &Self,
        source_x: u32,
        source_y: u32,
        destination_x: u32,
        destination_y: u32,
        width: u32,
        height: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(source);
            crate::views::bitmap_fields(old(self));
        }
        let Some(source_end_x) = source_x.checked_add(width) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(source_end_y) = source_y.checked_add(height) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(destination_end_x) = destination_x.checked_add(width) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(destination_end_y) = destination_y.checked_add(height) else {
            return Err(MalformedError::IntegerOverflow);
        };
        if source_end_x > source.width
            || source_end_y > source.height
            || destination_end_x > self.width
            || destination_end_y > self.height
        {
            return Err(MalformedError::IntegerOverflow);
        }
        if source_x % 8 == 0
            && destination_x % 8 == 0
            && width % 8 == 0
            && matches!(
                operator,
                CombinationOperator::Or | CombinationOperator::Replace
            )
        {
            return super::aligned::compose_region(
                self,
                source,
                source_x,
                source_y,
                destination_x,
                destination_y,
                width,
                height,
                operator == CombinationOperator::Replace,
            );
        }
        let result = self.compose_region_chunked(
            source,
            source_x,
            source_y,
            destination_x,
            destination_y,
            width,
            height,
            operator,
        );
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
        }
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::spec::bitmap::storage_valid(old(self)),
            crate::spec::bitmap::storage_valid(source),
            source_x as nat + width as nat <= source.width as nat,
            source_y as nat + height as nat <= source.height as nat,
            destination_x as nat + width as nat <= old(self).width as nat,
            destination_y as nat + height as nat <= old(self).height as nat,
        ensures
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            (width == 0 || height == 0) ==> final(self).data@ == old(self).data@,
            (width == 0 || height == 0) ==> out.is_ok(),
    ))]
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn compose_region_chunked(
        &mut self,
        source: &Self,
        source_x: u32,
        source_y: u32,
        destination_x: u32,
        destination_y: u32,
        width: u32,
        height: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(source);
        }
        let Some(source_end_x) = source_x.checked_add(width) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(source_end_y) = source_y.checked_add(height) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(destination_end_x) = destination_x.checked_add(width) else {
            return Err(MalformedError::IntegerOverflow);
        };
        let Some(destination_end_y) = destination_y.checked_add(height) else {
            return Err(MalformedError::IntegerOverflow);
        };
        if source_end_x > source.width
            || source_end_y > source.height
            || destination_end_x > self.width
            || destination_end_y > self.height
        {
            return Err(MalformedError::IntegerOverflow);
        }
        if width == 0 || height == 0 {
            return Ok(());
        }
        let destination_column =
            usize::try_from(destination_x / 8).map_err(|_error| MalformedError::IntegerOverflow)?;
        let destination_shift = destination_x % 8;
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::div_mod::lemma_mod_bound(destination_x as int, 8);
            assert(destination_shift < 8);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                crate::spec::bitmap::storage_valid(source),
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
                row <= height,
                width > 0,
                height > 0,
                destination_shift < 8,
                source_x as nat + width as nat <= source.width as nat,
                source_y as nat + height as nat <= source.height as nat,
                destination_x as nat + width as nat <= self.width as nat,
                destination_y as nat + height as nat <= self.height as nat,
            decreases height - row,
        ))]
        for row in 0..height {
            let source_row = usize::try_from(
                source_y
                    .checked_add(row)
                    .ok_or(MalformedError::IntegerOverflow)?,
            )
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(source.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
            let destination_row = usize::try_from(
                destination_y
                    .checked_add(row)
                    .ok_or(MalformedError::IntegerOverflow)?,
            )
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .and_then(|offset| offset.checked_add(destination_column))
            .ok_or(MalformedError::IntegerOverflow)?;

            let mut source_bit = source_x;
            let mut remaining = width;
            let mut destination_index = destination_row;
            if destination_shift != 0 {
                let head_bits = remaining.min(8 - destination_shift);
                let source_byte = source_row
                    .checked_add(
                        usize::try_from(source_bit / 8)
                            .map_err(|_error| MalformedError::IntegerOverflow)?,
                    )
                    .ok_or(MalformedError::IntegerOverflow)?;
                let source_value =
                    read_shifted_tail(&source.data, source_byte, source_bit % 8, head_bits)
                        .ok_or(MalformedError::IntegerOverflow)?
                        >> destination_shift;
                let destination_value = *self
                    .data
                    .get(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let mask = (0xFF_u8 >> destination_shift)
                    & (0xFF_u8 << (8 - destination_shift - head_bits));
                let combined = operator.apply_byte(destination_value, source_value);
                let destination_byte = self
                    .data
                    .get_mut(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                *destination_byte = (destination_value & !mask) | (combined & mask);
                source_bit = source_bit
                    .checked_add(head_bits)
                    .ok_or(MalformedError::IntegerOverflow)?;
                remaining = remaining
                    .checked_sub(head_bits)
                    .ok_or(MalformedError::IntegerOverflow)?;
                destination_index = destination_index
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?;
            }

            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    remaining <= width,
                    self.data@.len() == old(self).data@.len(),
                    width > 0,
                    height > 0,
                decreases remaining,
            ))]
            while remaining >= 64 {
                let source_byte = source_row
                    .checked_add(
                        usize::try_from(source_bit / 8)
                            .map_err(|_error| MalformedError::IntegerOverflow)?,
                    )
                    .ok_or(MalformedError::IntegerOverflow)?;
                let source_chunk = read_shifted_chunk(&source.data, source_byte, source_bit % 8)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let destination_chunk = read_chunk(&self.data, destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                if !write_chunk(
                    &mut self.data,
                    destination_index,
                    operator.apply_chunk(destination_chunk, source_chunk),
                ) {
                    return Err(MalformedError::IntegerOverflow);
                }
                source_bit = source_bit
                    .checked_add(64)
                    .ok_or(MalformedError::IntegerOverflow)?;
                remaining -= 64;
                destination_index = destination_index
                    .checked_add(8)
                    .ok_or(MalformedError::IntegerOverflow)?;
            }
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    remaining <= width,
                    self.data@.len() == old(self).data@.len(),
                    width > 0,
                    height > 0,
                decreases remaining,
            ))]
            while remaining >= 8 {
                let source_byte = source_row
                    .checked_add(
                        usize::try_from(source_bit / 8)
                            .map_err(|_error| MalformedError::IntegerOverflow)?,
                    )
                    .ok_or(MalformedError::IntegerOverflow)?;
                let source_value = read_shifted_byte(&source.data, source_byte, source_bit % 8)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let destination_value = *self
                    .data
                    .get(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let destination_byte = self
                    .data
                    .get_mut(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                *destination_byte = operator.apply_byte(destination_value, source_value);
                source_bit = source_bit
                    .checked_add(8)
                    .ok_or(MalformedError::IntegerOverflow)?;
                remaining -= 8;
                destination_index = destination_index
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?;
            }
            if remaining != 0 {
                let source_byte = source_row
                    .checked_add(
                        usize::try_from(source_bit / 8)
                            .map_err(|_error| MalformedError::IntegerOverflow)?,
                    )
                    .ok_or(MalformedError::IntegerOverflow)?;
                let source_value =
                    read_shifted_tail(&source.data, source_byte, source_bit % 8, remaining)
                        .ok_or(MalformedError::IntegerOverflow)?;
                let destination_value = *self
                    .data
                    .get(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let mask = 0xFF_u8 << (8 - remaining);
                let combined = operator.apply_byte(destination_value, source_value);
                let destination_byte = self
                    .data
                    .get_mut(destination_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                *destination_byte = (destination_value & !mask) | (combined & mask);
            }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            destination_shift < 8,
        ensures
            width == 0 ==> out.is_ok(),
            width == 0 ==> final(self).data@ == old(self).data@,
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
    ))]
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn compose_row_blit_chunked(
        &mut self,
        source: &Self,
        source_row: usize,
        destination_row: usize,
        source_x: u32,
        destination_shift: u32,
        width: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        if width == 0 {
            return Ok(());
        }
        let mut source_bit = source_x;
        let mut remaining = width;
        let mut destination_index = destination_row;
        if destination_shift != 0 {
            let head_bits = remaining.min(8 - destination_shift);
            let source_byte = source_row
                .checked_add(
                    usize::try_from(source_bit / 8)
                        .map_err(|_error| MalformedError::IntegerOverflow)?,
                )
                .ok_or(MalformedError::IntegerOverflow)?;
            let source_value =
                read_shifted_tail(&source.data, source_byte, source_bit % 8, head_bits)
                    .ok_or(MalformedError::IntegerOverflow)?
                    >> destination_shift;
            let destination_value = *self
                .data
                .get(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            let mask =
                (0xFF_u8 >> destination_shift) & (0xFF_u8 << (8 - destination_shift - head_bits));
            let combined = operator.apply_byte(destination_value, source_value);
            let destination_byte = self
                .data
                .get_mut(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *destination_byte = (destination_value & !mask) | (combined & mask);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                crate::views::bitmap_fields(old(self));
                reveal(crate::spec::bitmap::storage_valid);
            }
            source_bit = source_bit
                .checked_add(head_bits)
                .ok_or(MalformedError::IntegerOverflow)?;
            remaining = remaining
                .checked_sub(head_bits)
                .ok_or(MalformedError::IntegerOverflow)?;
            destination_index = destination_index
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
                remaining <= width,
            decreases remaining,
        ))]
        while remaining >= 64 {
            let source_byte = source_row
                .checked_add(
                    usize::try_from(source_bit / 8)
                        .map_err(|_error| MalformedError::IntegerOverflow)?,
                )
                .ok_or(MalformedError::IntegerOverflow)?;
            let source_chunk = read_shifted_chunk(&source.data, source_byte, source_bit % 8)
                .ok_or(MalformedError::IntegerOverflow)?;
            let destination_chunk =
                read_chunk(&self.data, destination_index).ok_or(MalformedError::IntegerOverflow)?;
            if !write_chunk(
                &mut self.data,
                destination_index,
                operator.apply_chunk(destination_chunk, source_chunk),
            ) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(self);
                    crate::views::bitmap_fields(old(self));
                    reveal(crate::spec::bitmap::storage_valid);
                }
                return Err(MalformedError::IntegerOverflow);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                crate::views::bitmap_fields(old(self));
                reveal(crate::spec::bitmap::storage_valid);
            }
            source_bit = source_bit
                .checked_add(64)
                .ok_or(MalformedError::IntegerOverflow)?;
            remaining -= 64;
            destination_index = destination_index
                .checked_add(8)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
                remaining <= width,
            decreases remaining,
        ))]
        while remaining >= 8 {
            let source_byte = source_row
                .checked_add(
                    usize::try_from(source_bit / 8)
                        .map_err(|_error| MalformedError::IntegerOverflow)?,
                )
                .ok_or(MalformedError::IntegerOverflow)?;
            let source_value = read_shifted_byte(&source.data, source_byte, source_bit % 8)
                .ok_or(MalformedError::IntegerOverflow)?;
            let destination_value = *self
                .data
                .get(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            let destination_byte = self
                .data
                .get_mut(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *destination_byte = operator.apply_byte(destination_value, source_value);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                crate::views::bitmap_fields(old(self));
                reveal(crate::spec::bitmap::storage_valid);
            }
            source_bit = source_bit
                .checked_add(8)
                .ok_or(MalformedError::IntegerOverflow)?;
            remaining -= 8;
            destination_index = destination_index
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        if remaining != 0 {
            let source_byte = source_row
                .checked_add(
                    usize::try_from(source_bit / 8)
                        .map_err(|_error| MalformedError::IntegerOverflow)?,
                )
                .ok_or(MalformedError::IntegerOverflow)?;
            let source_value =
                read_shifted_tail(&source.data, source_byte, source_bit % 8, remaining)
                    .ok_or(MalformedError::IntegerOverflow)?;
            let destination_value = *self
                .data
                .get(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            let mask = 0xFF_u8 << (8 - remaining);
            let combined = operator.apply_byte(destination_value, source_value);
            let destination_byte = self
                .data
                .get_mut(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *destination_byte = (destination_value & !mask) | (combined & mask);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                reveal(crate::spec::bitmap::storage_valid);
            }
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            reveal(crate::spec::bitmap::storage_valid);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
    ))]
    fn compose_aligned_row_chunked(
        &mut self,
        source: &Self,
        mut source_index: usize,
        mut destination_index: usize,
        mut byte_count: usize,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
            decreases byte_count,
        ))]
        while byte_count >= 8 {
            let source_chunk =
                read_chunk(&source.data, source_index).ok_or(MalformedError::IntegerOverflow)?;
            let destination_chunk =
                read_chunk(&self.data, destination_index).ok_or(MalformedError::IntegerOverflow)?;
            if !write_chunk(
                &mut self.data,
                destination_index,
                operator.apply_chunk(destination_chunk, source_chunk),
            ) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(self);
                    crate::views::bitmap_fields(old(self));
                    reveal(crate::spec::bitmap::storage_valid);
                }
                return Err(MalformedError::IntegerOverflow);
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                crate::views::bitmap_fields(old(self));
                reveal(crate::spec::bitmap::storage_valid);
            }
            source_index = source_index
                .checked_add(8)
                .ok_or(MalformedError::IntegerOverflow)?;
            destination_index = destination_index
                .checked_add(8)
                .ok_or(MalformedError::IntegerOverflow)?;
            byte_count -= 8;
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
            decreases byte_count,
        ))]
        while byte_count != 0 {
            let source_value = *source
                .data
                .get(source_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            let destination_value = *self
                .data
                .get(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            let destination_byte = self
                .data
                .get_mut(destination_index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *destination_byte = operator.apply_byte(destination_value, source_value);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(self);
                crate::views::bitmap_fields(old(self));
                reveal(crate::spec::bitmap::storage_valid);
            }
            source_index = source_index
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
            destination_index = destination_index
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
            byte_count -= 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            reveal(crate::spec::bitmap::storage_valid);
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            height == 0 ==> final(self).data@ == old(self).data@,
            height == 0 ==> out.is_ok(),
    ))]
    pub(crate) fn compose_row_batch_chunked(
        &mut self,
        blits: &[RowBlit<'_>],
        source_y: u32,
        destination_y: u32,
        height: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                row <= height,
                self.width == old(self).width,
                self.height == old(self).height,
                self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(),
                height == 0 ==> self.data@ == old(self).data@,
            decreases height - row,
        ))]
        for row in 0..height {
            let destination_base = usize::try_from(
                destination_y
                    .checked_add(row)
                    .ok_or(MalformedError::IntegerOverflow)?,
            )
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
            let mut blit_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    blit_index <= blits@.len(),
                    self.width == old(self).width,
                    self.height == old(self).height,
                    self.stride == old(self).stride,
                    self.data@.len() == old(self).data@.len(),
                    height > 0,
                decreases blits@.len() - blit_index,
            ))]
            while blit_index < blits.len() {
                let blit = &blits[blit_index];
                let source_row = usize::try_from(
                    source_y
                        .checked_add(row)
                        .ok_or(MalformedError::IntegerOverflow)?,
                )
                .map_err(|_error| MalformedError::IntegerOverflow)?
                .checked_mul(blit.source.stride)
                .ok_or(MalformedError::IntegerOverflow)?;
                let destination_column = usize::try_from(blit.destination_x / 8)
                    .map_err(|_error| MalformedError::IntegerOverflow)?;
                let destination_row = destination_base
                    .checked_add(destination_column)
                    .ok_or(MalformedError::IntegerOverflow)?;
                if blit.source_x % 8 == 0 && blit.destination_x % 8 == 0 && blit.width % 8 == 0 {
                    let source_index = source_row
                        .checked_add(
                            usize::try_from(blit.source_x / 8)
                                .map_err(|_error| MalformedError::IntegerOverflow)?,
                        )
                        .ok_or(MalformedError::IntegerOverflow)?;
                    self.compose_aligned_row_chunked(
                        blit.source,
                        source_index,
                        destination_row,
                        usize::try_from(blit.width / 8)
                            .map_err(|_error| MalformedError::IntegerOverflow)?,
                        operator,
                    )?;
                } else {
                    self.compose_row_blit_chunked(
                        blit.source,
                        source_row,
                        destination_row,
                        blit.source_x,
                        blit.destination_x % 8,
                        blit.width,
                        operator,
                    )?;
                }
                blit_index += 1;
            }
        }
        Ok(())
    }
}
