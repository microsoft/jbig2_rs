#[cfg(verus_keep_ghost)]
use super::bitmap_shared;
use super::chunks::write_chunk;
use super::Bitmap;
use crate::MalformedError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == crate::spec::bitmap::bit_mask(x),
))]
fn pixel_mask(x: u32) -> u8 {
    match x % 8 {
        0 => 0x80,
        1 => 0x40,
        2 => 0x20,
        3 => 0x10,
        4 => 0x08,
        5 => 0x04,
        6 => 0x02,
        _ => 0x01,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires old(destination)@.len() == source@.len(),
    ensures final(destination)@ == source@,
))]
#[inline]
pub(super) fn copy_row_bytes(destination: &mut [u8], source: &[u8]) {
    let length = source.len();
    if length <= 2 {
        if length != 0 {
            destination[0] = source[0];
        }
        if length == 2 {
            destination[1] = source[1];
        }
    } else if length > 64 && length <= 128 {
        // Avoid the slow short-copy path with a fixed block and a small tail.
        destination[..64].copy_from_slice(&source[..64]);
        destination[64..].copy_from_slice(&source[64..]);
    } else {
        destination.copy_from_slice(source);
    }
}

impl Bitmap {
    /// Tests every stored byte, including padding.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out == crate::spec::bitmap::zeroed_storage(self),
            out ==> forall|index: int, value: u8|
                0 <= index < crate::views::bitmap_data(self).len() ==>
                    #[trigger] crate::spec::bitmap::operator_byte(
                        crate::bitmap::CombinationOperator::Xor,
                        crate::views::bitmap_data(self)[index],
                        value,
                    ) == value,
    ))]
    pub(crate) fn is_zeroed_storage(&self) -> bool {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            reveal(crate::spec::bitmap::zeroed_storage);
        }
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                index <= self.data@.len(),
                forall|before: int| 0 <= before < index ==>
                    #[trigger] self.data@[before] == 0,
            decreases self.data@.len() - index,
        ))]
        while index < self.data.len() {
            if self.data[index] != 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(self);
                    reveal(crate::spec::bitmap::zeroed_storage);
                    assert(crate::views::bitmap_data(self)[index as int] != 0);
                    assert(!crate::spec::bitmap::zeroed_storage(self));
                }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert forall|position: int, value: u8|
                0 <= position < crate::views::bitmap_data(self).len()
                implies crate::spec::bitmap::operator_byte(
                    crate::bitmap::CombinationOperator::Xor,
                    crate::views::bitmap_data(self)[position],
                    value,
                ) == value by {
                assert(crate::views::bitmap_data(self)[position] == 0);
                assert(0u8 ^ value == value) by(bit_vector);
            }
        }
        true
    }

    /// Returns a pixel, or `None` when the coordinates are outside the bitmap.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::spec::bitmap::storage_valid(self),
        ensures out == crate::spec::bitmap::pixel_result(self, x, y),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub fn pixel(&self, x: u32, y: u32) -> Option<bool> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            reveal(crate::spec::bitmap::pixel_result);
            reveal(crate::spec::bitmap::coordinates_in_bounds);
        }
        if x >= self.width || y >= self.height {
            return None;
        }
        #[cfg(verus_keep_ghost)]
        production_bitmap_in_bounds_byte!(self, x, y);
        #[cfg(verus_keep_ghost)]
        production_bitmap_pixel_index_bounds!(self, x, y);
        let byte_index = usize::try_from(y)
            .ok()?
            .checked_mul(self.stride)?
            .checked_add(usize::try_from(x / 8).ok()?)?;
        let byte = self.data.get(byte_index)?;
        let value = *byte & pixel_mask(x) != 0;
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::spec::bitmap::stored_pixel);
            reveal(crate::spec::bitmap::byte_bit);
            reveal(crate::spec::bitmap::bit_mask);
            assert(byte_index as nat
                == crate::spec::bitmap::byte_index(self.stride, x, y));
        }
        Some(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            row < old(self).height,
            start <= end <= old(self).width,
        ensures
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            start == end ==> final(self).data@ == old(self).data@,
            start == end ==> out.is_ok(),
            crate::spec::bitmap::storage_valid(old(self)) ==> out.is_ok(),
            crate::spec::bitmap::storage_valid(old(self)) ==>
                crate::spec::bitmap::black_span_result(old(self), final(self), row, start, end),
    ))]
    pub(crate) fn fill_black_span(
        &mut self,
        row: u32,
        start: u32,
        end: u32,
    ) -> Result<(), MalformedError> {
        #[cfg(verus_keep_ghost)]
        production_black_span_init!(self, row, start, end, original, valid);
        if start > end || end > self.width || row >= self.height {
            return Err(MalformedError::IntegerOverflow);
        }
        if start == end {
            return Ok(());
        }
        #[cfg(verus_keep_ghost)]
        production_black_span_bounds!(self, row, end, valid);
        let row_start = usize::try_from(row)
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let first_byte =
            usize::try_from(start / 8).map_err(|_error| MalformedError::IntegerOverflow)?;
        let first_index = row_start
            .checked_add(first_byte)
            .ok_or(MalformedError::IntegerOverflow)?;
        let start_shift = start % 8;
        let first_bits = (end - start).min(8 - start_shift);
        let first_mask = (0xFF_u8 >> start_shift) & (0xFF_u8 << (8 - start_shift - first_bits));
        let first = self
            .data
            .get_mut(first_index)
            .ok_or(MalformedError::IntegerOverflow)?;
        *first |= first_mask;
        #[cfg(verus_keep_ghost)]
        production_black_span_first!(self, valid, original, row_start, start, first_bits);

        let mut x = start + first_bits;
        let mut index = first_index
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        #[cfg_attr(verus_keep_ghost, verus_spec(invariant
                start < x <= end, end <= self.width,
                self.width == old(self).width, self.height == old(self).height, self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(), self.data@.len() <= usize::MAX,
                valid == crate::spec::bitmap::storage_valid(old(self)), original == old(self).data@,
                row_start as int == row as int * self.stride as int,
                index as int == row_start as int + (x as int + 7) / 8, x < end ==> x % 8 == 0,
                valid ==> row_start as int + (end as int + 7) / 8 <= self.data@.len(),
                valid ==> crate::spec::bitmap::black_span_bits(original, self.data@, row_start as int, start, x),
            decreases end - x,
        ))]
        while end - x >= 64 {
            #[cfg(verus_keep_ghost)]
            production_black_span_access!(self, index, row_start, x, 8, valid, before);
            if !write_chunk(&mut self.data, index, u64::MAX) {
                return Err(MalformedError::IntegerOverflow);
            }
            #[cfg(verus_keep_ghost)]
            production_black_span_word!(self, valid, original, before, row_start, start, x, index);
            x += 64;
            index = index
                .checked_add(8)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(invariant
                start < x <= end, end <= self.width,
                self.width == old(self).width, self.height == old(self).height, self.stride == old(self).stride,
                self.data@.len() == old(self).data@.len(), self.data@.len() <= usize::MAX,
                valid == crate::spec::bitmap::storage_valid(old(self)), original == old(self).data@,
                row_start as int == row as int * self.stride as int,
                index as int == row_start as int + (x as int + 7) / 8, x < end ==> x % 8 == 0,
                valid ==> row_start as int + (end as int + 7) / 8 <= self.data@.len(),
                valid ==> crate::spec::bitmap::black_span_bits(original, self.data@, row_start as int, start, x),
            decreases end - x,
        ))]
        while end - x >= 8 {
            #[cfg(verus_keep_ghost)]
            production_black_span_access!(self, index, row_start, x, 1, valid, before);
            let byte = self
                .data
                .get_mut(index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *byte = 0xFF;
            #[cfg(verus_keep_ghost)]
            production_black_span_byte!(self, valid, original, before, row_start, start, x);
            x += 8;
            index = index
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        if x < end {
            #[cfg(verus_keep_ghost)]
            proof_decl! { let ghost before = self.data@; }
            let mask = 0xFF_u8 << (8 - (end - x));
            let byte = self
                .data
                .get_mut(index)
                .ok_or(MalformedError::IntegerOverflow)?;
            *byte |= mask;
            #[cfg(verus_keep_ghost)]
            production_black_span_tail!(
                self, original, before, row_start, start, x, end, index, mask
            );
        }
        #[cfg(verus_keep_ghost)]
        production_black_span_result!(self, valid, row, start, end);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::spec::bitmap::storage_valid(old(self)),
            source < old(self).height,
            destination < old(self).height,
        ensures
            crate::spec::bitmap::storage_valid(final(self)),
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            crate::spec::bitmap::row_copy_result(old(self), final(self), source, destination),
            out.is_ok(),
    ))]
    pub(crate) fn copy_row(&mut self, source: u32, destination: u32) -> Result<(), MalformedError> {
        let data_length = self.data.len();
        #[cfg(verus_keep_ghost)]
        production_bitmap_row_copy_bounds!(self, source, destination);
        if source >= self.height || destination >= self.height {
            return Err(MalformedError::IntegerOverflow);
        }
        if source == destination || self.stride == 0 {
            return Ok(());
        }
        let source_start = usize::try_from(source)
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let destination_start = usize::try_from(destination)
            .map_err(|_error| MalformedError::IntegerOverflow)?
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let source_end = source_start
            .checked_add(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let destination_end = destination_start
            .checked_add(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        if source_end > data_length || destination_end > data_length {
            return Err(MalformedError::IntegerOverflow);
        }
        if source < destination {
            let (before, after) = self.data.split_at_mut(destination_start);
            copy_row_bytes(&mut after[..self.stride], &before[source_start..source_end]);
        } else {
            let (before, after) = self.data.split_at_mut(source_start);
            copy_row_bytes(
                &mut before[destination_start..destination_end],
                &after[..self.stride],
            );
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
        }
        Ok(())
    }

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
            match out {
                Ok(()) => byte_count <= old(self).data@.len()
                    && byte_count <= source.data@.len()
                    && crate::spec::bitmap::prefix_copy_result(
                        old(self), final(self), source, byte_count,
                    ),
                Err(error) =>
                    error == MalformedError::IntegerOverflow
                    && (byte_count > old(self).data@.len() || byte_count > source.data@.len())
                    && final(self).data@ == old(self).data@,
            },
    ))]
    pub(crate) fn copy_prefix_from(
        &mut self,
        source: &Self,
        byte_count: usize,
    ) -> Result<(), MalformedError> {
        if byte_count > self.data.len() || byte_count > source.data.len() {
            return Err(MalformedError::IntegerOverflow);
        }
        // Keep fixed-word prefix copies inline without changing the row kernel.
        if byte_count == 8 {
            self.data.as_mut_slice()[..8].copy_from_slice(&source.data.as_slice()[..8]);
        } else {
            copy_row_bytes(
                &mut self.data.as_mut_slice()[..byte_count],
                &source.data.as_slice()[..byte_count],
            );
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            crate::views::bitmap_fields(source);
            reveal(crate::spec::bitmap::storage_valid);
            reveal(crate::spec::bitmap::prefix_copy_result);
        }
        Ok(())
    }
}

impl Bitmap {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::spec::bitmap::storage_valid(old(self)),
        ensures
            crate::views::bitmap_width(final(self))
                == crate::views::bitmap_width(old(self)),
            crate::views::bitmap_height(final(self))
                == crate::views::bitmap_height(old(self)),
            crate::views::bitmap_stride(final(self))
                == crate::views::bitmap_stride(old(self)),
            crate::spec::bitmap::storage_valid(final(self)),
            match out {
                Ok(()) =>
                    crate::spec::bitmap::coordinates_in_bounds(
                        crate::views::bitmap_width(old(self)),
                        crate::views::bitmap_height(old(self)),
                        x,
                        y,
                    )
                    && crate::views::bitmap_data(final(self)).len()
                        == crate::views::bitmap_data(old(self)).len()
                    && crate::spec::bitmap::stored_pixel(
                        final(self),
                        x,
                        y,
                    ) == value
                    && forall|index: int|
                        0 <= index
                            < crate::views::bitmap_data(old(self)).len()
                        ==> crate::views::bitmap_data(final(self))[index]
                            == if index
                                == crate::spec::bitmap::byte_index(
                                    crate::views::bitmap_stride(old(self)),
                                    x,
                                    y,
                                ) as int
                            {
                                crate::spec::bitmap::set_byte_bit(
                                    crate::views::bitmap_data(old(self))[index],
                                    x,
                                    value,
                                )
                            } else {
                                crate::views::bitmap_data(old(self))[index]
                            },
                Err(_) =>
                    !crate::spec::bitmap::coordinates_in_bounds(
                        crate::views::bitmap_width(old(self)),
                        crate::views::bitmap_height(old(self)),
                        x,
                        y,
                    )
                    && crate::views::bitmap_data(final(self))
                        == crate::views::bitmap_data(old(self)),
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn set_pixel(&mut self, x: u32, y: u32, value: bool) -> Result<(), MalformedError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            reveal(crate::spec::bitmap::coordinates_in_bounds);
        }
        if x >= self.width || y >= self.height {
            return Err(MalformedError::IntegerOverflow);
        }
        #[cfg(verus_keep_ghost)]
        production_bitmap_in_bounds_byte!(self, x, y);
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(crate::spec::bitmap::byte_index);
            assert(self.data@.len() == self.data.len());
            assert(self.data@.len() <= usize::MAX);
            assert(y as nat <= u32::MAX as nat);
            assert(u32::MAX as nat <= usize::MAX) by (compute);
            assert(y as nat * self.stride as nat <= usize::MAX);
        }
        let row = usize::try_from(y).map_err(|_error| MalformedError::IntegerOverflow)?;
        let row_offset = row
            .checked_mul(self.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let column = usize::try_from(x / 8).map_err(|_error| MalformedError::IntegerOverflow)?;
        let byte_index = row_offset
            .checked_add(column)
            .ok_or(MalformedError::IntegerOverflow)?;
        if byte_index >= self.data.len() {
            return Err(MalformedError::IntegerOverflow);
        }
        let byte = &mut self.data[byte_index];
        let mask = pixel_mask(x);
        if value {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
        #[cfg(verus_keep_ghost)]
        production_bitmap_set_pixel_result!(self, x, y, value);
        Ok(())
    }
}
