//! Packed bitmap construction, copying and template selection.

use super::{
    check_dimensions, enforce_limit, Bitmap, CombinationOperator, DecodeError, GenericTemplate,
    LimitKind, Limits, MalformedError, RefinementTemplate, SymbolTextError,
};
#[cfg(verus_keep_ghost)]
use super::{symbol_text_proofs, symbol_text_spec};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
// ---------------------------------------------------------------------------
// Small bitmap and template-mapping helpers shared by the dictionary and
// text-region decoders.
// ---------------------------------------------------------------------------

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures symbol_text_spec::generic_template_matches(value, out),
))]
pub(super) fn generic_template_from_u8(value: u8) -> GenericTemplate {
    match value {
        0 => GenericTemplate::Template0,
        1 => GenericTemplate::Template1,
        2 => GenericTemplate::Template2,
        _ => GenericTemplate::Template3,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures symbol_text_spec::refinement_template_matches(value, out),
))]
pub(super) fn refinement_template_from_u8(value: u8) -> RefinementTemplate {
    if value == 0 {
        RefinementTemplate::Template0
    } else {
        RefinementTemplate::Template1
    }
}

/// Constructs a bitmap directly from exactly `stride * height` packed bytes,
/// as used by T.88 6.5.9's uncompressed collective bitmap.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && bitmap.width == width
            && bitmap.height == height,
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_lines)]
pub(super) fn bitmap_from_packed_bytes(
    width: u32,
    height: u32,
    bytes: &[u8],
    limits: &Limits,
) -> Result<Bitmap, SymbolTextError> {
    check_dimensions(width, height, limits)?;
    let stride = u64::from(width)
        .checked_add(7)
        .ok_or(MalformedError::IntegerOverflow)?
        / 8;
    let length = stride
        .checked_mul(u64::from(height))
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        length,
    )?;
    let Ok(length_usize) = usize::try_from(length) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    if bytes.len() < length_usize {
        let Ok(remaining) = u64::try_from(bytes.len()) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        return Err(MalformedError::UnexpectedEof {
            offset: 0,
            needed: length,
            remaining,
        }
        .into());
    }
    if width % 8 != 0 {
        let unused = 8 - (width % 8);
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(1 <= unused <= 7);
        }
        let padding_mask = match unused {
            1 => 0x01,
            2 => 0x03,
            3 => 0x07,
            4 => 0x0f,
            5 => 0x1f,
            6 => 0x3f,
            7 => 0x7f,
            _ => return Err(MalformedError::IntegerOverflow.into()),
        };
        let Ok(stride_usize) = usize::try_from(stride) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                row <= height,
                length_usize <= bytes@.len(),
                length_usize as nat == stride_usize as nat * height as nat,
                stride_usize > 0,
            decreases height - row,
        ))]
        for row in 0..height {
            let Ok(row) = usize::try_from(row) else {
                return Err(MalformedError::IntegerOverflow.into());
            };
            let row_offset = row
                .checked_mul(stride_usize)
                .ok_or(MalformedError::IntegerOverflow)?;
            let stride_minus_one = stride_usize
                .checked_sub(1)
                .ok_or(MalformedError::IntegerOverflow)?;
            let final_byte = row_offset
                .checked_add(stride_minus_one)
                .ok_or(MalformedError::IntegerOverflow)?;
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(row < height as usize);
                assert(row_offset as nat == row as nat * stride_usize as nat);
                assert(stride_minus_one as nat + 1 == stride_usize as nat);
                assert(final_byte as nat
                    == row_offset as nat + stride_minus_one as nat);
                symbol_text_proofs::packed_row_end_in_bounds(
                    row as nat, height as nat, stride_usize as nat, final_byte as nat,
                );
                assert(final_byte < bytes@.len());
            }
            if bytes[final_byte] & padding_mask != 0 {
                return Err(SymbolTextError::InvalidState(
                    "raw collective bitmap has nonzero padding",
                ));
            }
        }
    }
    let mut data = Vec::new();
    if data.try_reserve(length_usize).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: length,
        }
        .into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= length_usize,
            length_usize <= bytes@.len(),
            data@ == bytes@.subrange(0, index as int),
        decreases length_usize - index,
    ))]
    while index < length_usize {
        data.push(bytes[index]);
        index += 1;
    }
    let Ok(stride) = usize::try_from(stride) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let bitmap = Bitmap {
        width,
        height,
        stride,
        data,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&bitmap);
        reveal(crate::spec::bitmap::storage_valid);
        reveal(crate::spec::bitmap::stride_for_width);
    }
    Ok(bitmap)
}

/// Crops a fixed-height column range out of a wider collective bitmap, used
/// to split T.88 6.5.9 height-class collective bitmaps into their individual
/// symbols.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(source),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::views::bitmap_width(&bitmap) == width
            && crate::views::bitmap_height(&bitmap)
                == crate::views::bitmap_height(source),
        Err(_) => true,
    },
))]
pub(super) fn crop_bitmap_columns(
    source: &Bitmap,
    x_start: u32,
    width: u32,
    limits: &Limits,
) -> Result<Bitmap, SymbolTextError> {
    let height = source.height();
    let mut cropped = Bitmap::blank(width, height, limits)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&cropped);
        assert(cropped.width == width);
        assert(cropped.height == height);
    }
    let source_end = x_start
        .checked_add(width)
        .ok_or(MalformedError::IntegerOverflow)?;
    if source_end > source.width() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    cropped.compose_region(
        source,
        x_start,
        0,
        0,
        0,
        width,
        height,
        CombinationOperator::Replace,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&cropped);
        reveal(crate::spec::bitmap::storage_valid);
        assert(crate::spec::bitmap::storage_valid(&cropped));
        assert(crate::views::bitmap_width(&cropped) == width);
        assert(crate::views::bitmap_height(&cropped) == height);
    }
    Ok(cropped)
}

/// Deep-copies a bitmap's packed bytes under fallible allocation.
///
/// [`Bitmap`] intentionally has no infallible [`Clone`]; exported symbols
/// must still become independently owned bitmaps, so this performs the copy
/// explicitly through its crate-visible fields.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(source),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && bitmap.width == source.width
            && bitmap.height == source.height
            && bitmap.stride == source.stride
            && bitmap.data@ == source.data@,
        Err(_) => true,
    },
))]
pub(crate) fn clone_bitmap(source: &Bitmap, limits: &Limits) -> Result<Bitmap, SymbolTextError> {
    let Ok(requested_bytes) = u64::try_from(source.data.len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        requested_bytes,
    )?;
    let mut data = Vec::new();
    if data.try_reserve(source.data.len()).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source.data@.len(),
            data@ == source.data@.subrange(0, index as int),
        decreases source.data@.len() - index,
    ))]
    while index < source.data.len() {
        data.push(source.data[index]);
        index += 1;
    }
    let bitmap = Bitmap {
        width: source.width,
        height: source.height,
        stride: source.stride,
        data,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&bitmap);
        crate::views::bitmap_fields(source);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(bitmap)
}
