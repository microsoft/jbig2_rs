use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::generic_region::{
    decode_probability_signal, enforce_managed_memory, integer_overflow, probability_signal_error,
    GenericRegionError, GenericTemplate,
};
use crate::{Bitmap, Limits};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct GenericRowRing {
    width: u32,
    stride_words: usize,
    row_slots: usize,
    words: Vec<u32>,
    charged_bytes: u64,
}

impl GenericRowRing {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.is_some() ==> {
            let ring = out.unwrap();
            &&& ring.row_slots <= 3
            &&& ring.words@.len()
                == ring.stride_words as nat * ring.row_slots as nat
            &&& ring.width as nat <= ring.stride_words as nat * 32
            &&& ring.width == width
            &&& ring.row_slots == (if rows < 3 { rows } else { 3 }) as usize
            &&& ring.charged_bytes <= available_bytes
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[cold]
    #[inline(never)]
    fn try_new(width: u32, rows: u32, available_bytes: u64) -> Option<Self> {
        let stride_words = usize::try_from(width.checked_add(31)? / 32).ok()?;
        let row_slots = usize::try_from(rows.min(3)).ok()?;
        let word_count = stride_words.checked_mul(row_slots)?;
        let requested_bytes = u64::try_from(word_count).ok()?.checked_mul(4)?;
        if requested_bytes > available_bytes {
            return None;
        }
        let mut words = Vec::new();
        if words.try_reserve(word_count).is_err() {
            return None;
        }
        let charged_bytes = requested_bytes;
        words.resize(word_count, 0);
        Some(Self {
            width,
            stride_words,
            row_slots,
            words,
            charged_bytes,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            self.row_slots <= 3,
            self.words@.len()
                == self.stride_words as nat * self.row_slots as nat,
            self.width as nat <= self.stride_words as nat * 32,
        ensures out.is_some() ==> out.unwrap() < self.row_slots,
    ))]
    #[inline]
    fn slot_for_row(&self, row: u32) -> Option<usize> {
        if self.row_slots == 0 {
            return None;
        }
        let row = usize::try_from(row).ok()?;
        Some(row % self.row_slots)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            self.row_slots <= 3,
            self.words@.len()
                == self.stride_words as nat * self.row_slots as nat,
            self.width as nat <= self.stride_words as nat * 32,
    ))]
    #[inline]
    fn row_start(&self, slot: Option<usize>) -> Option<usize> {
        let slot = slot?;
        if slot >= self.row_slots {
            return None;
        }
        slot.checked_mul(self.stride_words)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            self.row_slots <= 3,
            self.words@.len()
                == self.stride_words as nat * self.row_slots as nat,
            self.width as nat <= self.stride_words as nat * 32,
    ))]
    fn row_bits(&self, slot: Option<usize>, start_x: i64, count: u8) -> usize {
        let Some(row_start) = self.row_start(slot) else {
            return 0;
        };
        let mut bits = 0_usize;
        let mut offset = 0_u8;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant offset <= count,
            decreases count - offset,
        ))]
        while offset < count {
            let Some(x) = start_x.checked_add(i64::from(offset)) else {
                return bits;
            };
            let bit = if x < 0 || x >= i64::from(self.width) {
                false
            } else {
                let Ok(x) = u32::try_from(x) else {
                    return bits;
                };
                let Ok(column) = usize::try_from(x / 32) else {
                    return bits;
                };
                match row_start
                    .checked_add(column)
                    .and_then(|index| self.words.get(index))
                {
                    Some(word) => *word & (0x8000_0000_u32 >> (x % 32)) != 0,
                    None => false,
                }
            };
            bits = (bits << 1) | usize::from(bit);
            offset += 1;
        }
        bits
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires
            self.row_slots <= 3,
            self.words@.len()
                == self.stride_words as nat * self.row_slots as nat,
            self.width as nat <= self.stride_words as nat * 32,
    ))]
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn row_word(&self, row_start: Option<usize>, x: u64) -> u32 {
        let Some(row_start) = row_start else {
            return 0;
        };
        let Ok(column) = usize::try_from(x / 32) else {
            return 0;
        };
        if column >= self.stride_words {
            return 0;
        }
        let Some(first_index) = row_start.checked_add(column) else {
            return 0;
        };
        let Some(first) = self.words.get(first_index) else {
            return 0;
        };
        let first = *first;
        let shift = (x % 32) as u32;
        if shift == 0 {
            return first;
        }
        let second = if column + 1 < self.stride_words {
            match column
                .checked_add(1)
                .and_then(|next| row_start.checked_add(next))
                .and_then(|index| self.words.get(index))
            {
                Some(word) => *word,
                None => 0,
            }
        } else {
            0
        };
        (first << shift) | (second >> (32 - shift))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            old(self).row_slots <= 3,
            old(self).words@.len()
                == old(self).stride_words as nat * old(self).row_slots as nat,
            old(self).width as nat <= old(self).stride_words as nat * 32,
        ensures
            final(self).width == old(self).width,
            final(self).stride_words == old(self).stride_words,
            final(self).row_slots == old(self).row_slots,
            final(self).charged_bytes == old(self).charged_bytes,
            out.is_ok() ==> final(self).words@.len() == old(self).words@.len(),
            out.is_ok() ==> out.unwrap() as nat + final(self).stride_words as nat
                <= final(self).words@.len(),
    ))]
    fn clear_row(&mut self, slot: usize) -> Result<usize, GenericRegionError> {
        let start = self.row_start(Some(slot)).ok_or_else(integer_overflow)?;
        let end = start
            .checked_add(self.stride_words)
            .ok_or_else(integer_overflow)?;
        if end > self.words.len() {
            return Err(integer_overflow());
        }
        #[cfg(verus_keep_ghost)]
        let word_length = self.words.len();
        let mut column = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                column <= self.stride_words,
                self.words@.len() == word_length,
            decreases self.stride_words - column,
        ))]
        while column < self.stride_words {
            let Some(index) = start.checked_add(column) else {
                return Err(integer_overflow());
            };
            let Some(word) = self.words.get_mut(index) else {
                return Err(integer_overflow());
            };
            *word = 0;
            column += 1;
        }
        Ok(start)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            old(self).row_slots <= 3,
            old(self).words@.len()
                == old(self).stride_words as nat * old(self).row_slots as nat,
            old(self).width as nat <= old(self).stride_words as nat * 32,
        ensures
            final(self).width == old(self).width,
            final(self).stride_words == old(self).stride_words,
            final(self).row_slots == old(self).row_slots,
            final(self).charged_bytes == old(self).charged_bytes,
            out.is_ok() ==> final(self).words@.len() == old(self).words@.len(),
    ))]
    fn copy_row(&mut self, source: usize, destination: usize) -> Result<(), GenericRegionError> {
        let source_start = self.row_start(Some(source)).ok_or_else(integer_overflow)?;
        let destination_start = self
            .row_start(Some(destination))
            .ok_or_else(integer_overflow)?;
        let source_end = source_start
            .checked_add(self.stride_words)
            .ok_or_else(integer_overflow)?;
        if source_end > self.words.len() {
            return Err(integer_overflow());
        }
        let destination_end = destination_start
            .checked_add(self.stride_words)
            .ok_or_else(integer_overflow)?;
        if destination_end > self.words.len() {
            return Err(integer_overflow());
        }
        #[cfg(verus_keep_ghost)]
        let word_length = self.words.len();
        let mut column = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                column <= self.stride_words,
                self.words@.len() == word_length,
            decreases self.stride_words - column,
        ))]
        while column < self.stride_words {
            let source_index = source_start
                .checked_add(column)
                .ok_or_else(integer_overflow)?;
            let destination_index = destination_start
                .checked_add(column)
                .ok_or_else(integer_overflow)?;
            let value = *self.words.get(source_index).ok_or_else(integer_overflow)?;
            let word = self
                .words
                .get_mut(destination_index)
                .ok_or_else(integer_overflow)?;
            *word = value;
            column += 1;
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires
            old(self).row_slots <= 3,
            old(self).words@.len()
                == old(self).stride_words as nat * old(self).row_slots as nat,
            old(self).width as nat <= old(self).stride_words as nat * 32,
            row_start as nat + old(self).stride_words as nat <= old(self).words@.len(),
            x < old(self).width,
        ensures
            final(self).width == old(self).width,
            final(self).stride_words == old(self).stride_words,
            final(self).row_slots == old(self).row_slots,
            final(self).charged_bytes == old(self).charged_bytes,
            final(self).words@.len() == old(self).words@.len(),
    ))]
    #[allow(clippy::cast_possible_truncation, clippy::inline_always)]
    #[inline(always)]
    fn set_black(&mut self, row_start: usize, x: u32) {
        let column = (x / 32) as usize;
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::div_mod::lemma_fundamental_div_mod(x as int, 32);
            vstd::arithmetic::div_mod::lemma_mod_bound(x as int, 32);
            assert((x as int) == 32 * ((x as int) / 32) + (x as int) % 32);
            assert(0 <= (x as int) % 32 < 32);
            assert((x as int) < (self.stride_words as int) * 32);
            assert((x as int) / 32 < self.stride_words as int);
            assert((x as nat) / 32 < self.stride_words as nat);
            assert((row_start as nat) + (column as nat) < self.words@.len());
            assert(self.words@.len() == self.words.len());
            assert(self.words@.len() <= usize::MAX);
            assert((row_start as nat) + (column as nat) <= usize::MAX);
        }
        self.words[row_start + column] |= 0x8000_0000_u32 >> (x % 32);
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            self.row_slots <= 3,
            self.words@.len()
                == self.stride_words as nat * self.row_slots as nat,
            self.width as nat <= self.stride_words as nat * 32,
            crate::spec::bitmap::storage_valid(old(bitmap)),
            old(bitmap).width == self.width,
            y < old(bitmap).height,
        ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        final(bitmap).stride == old(bitmap).stride,
    ))]
    fn flush_row(
        &self,
        slot: usize,
        bitmap: &mut Bitmap,
        y: u32,
    ) -> Result<(), GenericRegionError> {
        #[cfg(verus_keep_ghost)]
        let bitmap_width = bitmap.width;
        #[cfg(verus_keep_ghost)]
        let bitmap_height = bitmap.height;
        #[cfg(verus_keep_ghost)]
        let bitmap_stride = bitmap.stride;
        #[cfg(verus_keep_ghost)]
        let bitmap_length = bitmap.data.len();
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(bitmap);
            reveal(crate::spec::bitmap::storage_valid);
            assert(bitmap_length as nat
                == bitmap_stride as nat * bitmap_height as nat);
        }
        let word_start = self.row_start(Some(slot)).ok_or_else(integer_overflow)?;
        let row = usize::try_from(y).map_err(|_error| integer_overflow())?;
        let byte_start = row
            .checked_mul(bitmap.stride)
            .ok_or_else(integer_overflow)?;
        let full_words = bitmap.stride / 4;
        let mut column = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                column <= full_words,
                bitmap.width == bitmap_width,
                bitmap.height == bitmap_height,
                bitmap.stride == bitmap_stride,
                bitmap.data@.len() == bitmap_length,
                bitmap_length as nat
                    == bitmap_stride as nat * bitmap_height as nat,
            decreases full_words - column,
        ))]
        while column < full_words {
            let word_index = word_start
                .checked_add(column)
                .ok_or_else(integer_overflow)?;
            let word = *self.words.get(word_index).ok_or_else(integer_overflow)?;
            let byte_index = byte_start
                .checked_add(column.checked_mul(4).ok_or_else(integer_overflow)?)
                .ok_or_else(integer_overflow)?;
            let byte_end = byte_index.checked_add(4).ok_or_else(integer_overflow)?;
            let bytes = [
                crate::generic_region::word_byte(word, 0),
                crate::generic_region::word_byte(word, 1),
                crate::generic_region::word_byte(word, 2),
                crate::generic_region::word_byte(word, 3),
            ];
            bitmap
                .data
                .get_mut(byte_index..byte_end)
                .ok_or_else(integer_overflow)?
                .copy_from_slice(&bytes);
            column += 1;
        }
        let tail_start = full_words.checked_mul(4).ok_or_else(integer_overflow)?;
        let tail_length = bitmap.stride - tail_start;
        let mut tail = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                tail <= tail_length,
                tail_length <= 3,
                bitmap.width == bitmap_width,
                bitmap.height == bitmap_height,
                bitmap.stride == bitmap_stride,
                bitmap.data@.len() == bitmap_length,
                bitmap_length as nat
                    == bitmap_stride as nat * bitmap_height as nat,
            decreases tail_length - tail,
        ))]
        while tail < tail_length {
            let word_index = word_start
                .checked_add(full_words)
                .ok_or_else(integer_overflow)?;
            let word = *self.words.get(word_index).ok_or_else(integer_overflow)?;
            let byte_index = byte_start
                .checked_add(tail_start.checked_add(tail).ok_or_else(integer_overflow)?)
                .ok_or_else(integer_overflow)?;
            *bitmap
                .data
                .get_mut(byte_index)
                .ok_or_else(integer_overflow)? = crate::generic_region::word_byte(word, tail);
            tail += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(bitmap);
            reveal(crate::spec::bitmap::storage_valid);
        }
        Ok(())
    }
}

#[cold]
#[inline(never)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
fn storage_bytes(width: u32, rows: u32) -> Option<u64> {
    let stride_words = u64::from(width).checked_add(31)? / 32;
    stride_words
        .checked_mul(u64::from(rows.min(3)))?
        .checked_mul(4)
}

#[cold]
#[inline(never)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
fn fits_managed_memory(
    width: u32,
    rows: u32,
    context_bytes: u64,
    bitmap_bytes: u64,
    limits: &Limits,
) -> Result<bool, GenericRegionError> {
    let row_bytes = storage_bytes(width, rows).ok_or_else(integer_overflow)?;
    let managed = context_bytes
        .checked_add(bitmap_bytes)
        .and_then(|bytes| bytes.checked_add(row_bytes))
        .ok_or_else(integer_overflow)?;
    Ok(managed <= limits.max_decoder_managed_memory_bytes)
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        old(bitmap).width == width,
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        crate::generic_region::generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
        out.is_ok() ==> final(bitmap).width == old(bitmap).width,
        out.is_ok() ==> final(bitmap).height == old(bitmap).height,
        out.is_ok() ==> final(bitmap).stride == old(bitmap).stride,
        out.is_ok() ==>
            crate::generic_region::generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
pub(super) fn try_decode(
    bitmap: &mut Bitmap,
    width: u32,
    rows: u32,
    context_bytes: u64,
    bitmap_bytes: u64,
    limits: &Limits,
    typical_prediction: bool,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<bool, GenericRegionError> {
    if super::generic_packed::try_decode(
        bitmap,
        rows,
        GenericTemplate::Template0,
        super::GenericContextDescriptor::NominalTemplate0,
        typical_prediction,
        decoder,
        contexts,
    )? {
        return Ok(true);
    }
    if !fits_managed_memory(width, rows, context_bytes, bitmap_bytes, limits)? {
        return Ok(false);
    }
    let base_managed = context_bytes
        .checked_add(bitmap_bytes)
        .ok_or_else(integer_overflow)?;
    let available_bytes = limits
        .max_decoder_managed_memory_bytes
        .checked_sub(base_managed)
        .ok_or_else(integer_overflow)?;
    let Some(mut ring) = GenericRowRing::try_new(width, rows, available_bytes) else {
        return Ok(false);
    };
    let charged_context_bytes = context_bytes
        .checked_add(ring.charged_bytes)
        .ok_or_else(integer_overflow)?;
    enforce_managed_memory(charged_context_bytes, bitmap_bytes, limits)?;
    decode(
        &mut ring,
        bitmap,
        rows,
        typical_prediction,
        decoder,
        contexts,
    )?;
    Ok(true)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        old(ring).row_slots <= 3,
        old(ring).words@.len()
            == old(ring).stride_words as nat * old(ring).row_slots as nat,
        old(ring).width as nat <= old(ring).stride_words as nat * 32,
        crate::spec::bitmap::storage_valid(old(bitmap)),
        old(bitmap).width == old(ring).width,
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        crate::generic_region::generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        out.is_ok() ==> final(ring).row_slots <= 3,
        out.is_ok() ==> final(ring).words@.len()
            == final(ring).stride_words as nat * final(ring).row_slots as nat,
        out.is_ok() ==> final(ring).width as nat
            <= final(ring).stride_words as nat * 32,
        out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
        out.is_ok() ==> final(bitmap).width == old(bitmap).width,
        out.is_ok() ==> final(bitmap).height == old(bitmap).height,
        out.is_ok() ==> final(bitmap).stride == old(bitmap).stride,
        out.is_ok() ==>
            crate::generic_region::generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(clippy::too_many_lines)]
fn decode(
    ring: &mut GenericRowRing,
    bitmap: &mut Bitmap,
    rows: u32,
    typical_prediction: bool,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<(), GenericRegionError> {
    #[cfg(verus_keep_ghost)]
    let bitmap_width = bitmap.width;
    #[cfg(verus_keep_ghost)]
    let bitmap_height = bitmap.height;
    #[cfg(verus_keep_ghost)]
    let bitmap_stride = bitmap.stride;
    let mut ltp = false;
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= rows,
            rows <= bitmap.height,
            ring.row_slots <= 3,
            ring.words@.len()
                == ring.stride_words as nat * ring.row_slots as nat,
            ring.width as nat <= ring.stride_words as nat * 32,
            bitmap.width == ring.width,
            bitmap.width == bitmap_width,
            bitmap.height == bitmap_height,
            bitmap.stride == bitmap_stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            crate::generic_region::generic_region_spec::contexts_valid(contexts@),
        decreases rows - y,
    ))]
    while y < rows {
        let current_slot = ring.slot_for_row(y).ok_or_else(integer_overflow)?;
        let current_row = ring.clear_row(current_slot)?;
        if typical_prediction {
            let context = GenericTemplate::Template0.sltp_context();
            let sltp = decode_probability_signal(decoder, contexts, context);
            if sltp > 1 {
                return Err(probability_signal_error(
                    decoder,
                    context,
                    contexts.len(),
                    sltp,
                ));
            }
            ltp ^= sltp != 0;
        }
        if ltp {
            if y != 0 {
                let previous = ring.slot_for_row(y - 1).ok_or_else(integer_overflow)?;
                ring.copy_row(previous, current_slot)?;
            }
            ring.flush_row(current_slot, bitmap, y)?;
        } else {
            let top_slot = y.checked_sub(2).and_then(|row| ring.slot_for_row(row));
            let previous_slot = y.checked_sub(1).and_then(|row| ring.slot_for_row(row));
            let top_row = ring.row_start(top_slot);
            let previous_row = ring.row_start(previous_slot);
            let mut top = ring.row_bits(top_slot, -2, 5);
            let mut previous = ring.row_bits(previous_slot, -3, 7);
            let mut history = 0_usize;
            let mut x = 0_u32;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    x <= ring.width,
                    ring.row_slots <= 3,
                    ring.words@.len()
                        == ring.stride_words as nat * ring.row_slots as nat,
                    ring.width as nat <= ring.stride_words as nat * 32,
                    current_row as nat + ring.stride_words as nat <= ring.words@.len(),
                    crate::spec::bitmap::storage_valid(bitmap),
                    bitmap.width == ring.width,
                    rows <= bitmap.height,
                    bitmap.width == bitmap_width,
                    bitmap.height == bitmap_height,
                    bitmap.stride == bitmap_stride,
                    crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                    crate::generic_region::generic_region_spec::contexts_valid(contexts@),
                decreases ring.width - x,
            ))]
            while x < ring.width {
                let mut top_queue = ring.row_word(top_row, u64::from(x) + 3);
                let mut previous_queue = ring.row_word(previous_row, u64::from(x) + 4);
                let remaining = ring.width - x;
                let chunk_width = remaining.min(32);
                let chunk_end = x.checked_add(chunk_width).ok_or_else(integer_overflow)?;
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(chunk_width > 0);
                    assert(chunk_end <= ring.width);
                }
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        x <= chunk_end,
                        chunk_end <= ring.width,
                        ring.row_slots <= 3,
                        ring.words@.len()
                            == ring.stride_words as nat * ring.row_slots as nat,
                        ring.width as nat <= ring.stride_words as nat * 32,
                        current_row as nat + ring.stride_words as nat <= ring.words@.len(),
                        crate::spec::bitmap::storage_valid(bitmap),
                        bitmap.width == ring.width,
                        rows <= bitmap.height,
                        bitmap.width == bitmap_width,
                        bitmap.height == bitmap_height,
                        bitmap.stride == bitmap_stride,
                        crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                        crate::generic_region::generic_region_spec::contexts_valid(contexts@),
                    decreases chunk_end - x,
                ))]
                while x < chunk_end {
                    let context = (top << 11) | (previous << 4) | history & 0x0f;
                    top = ((top << 1) | usize::from(top_queue & 0x8000_0000 != 0)) & 0x1f;
                    previous =
                        ((previous << 1) | usize::from(previous_queue & 0x8000_0000 != 0)) & 0x7f;
                    top_queue <<= 1;
                    previous_queue <<= 1;
                    let signal = decode_probability_signal(decoder, contexts, context);
                    if signal > 1 {
                        return Err(probability_signal_error(
                            decoder,
                            context,
                            contexts.len(),
                            signal,
                        ));
                    }
                    history = ((history << 1) | usize::from(signal)) & 0x0f;
                    if signal != 0 {
                        ring.set_black(current_row, x);
                    }
                    x += 1;
                }
            }
            ring.flush_row(current_slot, bitmap, y)?;
        }
        y += 1;
    }
    Ok(())
}

#[cfg(test)]
#[path = "generic_row_ring/tests.rs"]
mod tests;
