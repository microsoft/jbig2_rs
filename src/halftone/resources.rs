use super::{HalftoneError, HalftoneHeader};
use crate::{Bitmap, DecodeError, LimitExceeded, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            crate::views::bitmap_width(bitmap) == expected_width
            && crate::views::bitmap_height(bitmap) == expected_height,
        Err(_) => true,
    },
))]
pub(super) fn require_bitmap_dimensions(
    bitmap: &Bitmap,
    expected_width: u32,
    expected_height: u32,
) -> Result<(), HalftoneError> {
    if bitmap.width() != expected_width || bitmap.height() != expected_height {
        return Err(HalftoneError::InvalidBitmapDimensions {
            expected_width,
            expected_height,
            actual_width: bitmap.width(),
            actual_height: bitmap.height(),
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(total) =>
            total as nat
                == first as nat + second as nat + third as nat + fourth as nat
            && first <= total
            && second <= total
            && third <= total
            && fourth <= total,
        None =>
            first as nat + second as nat + third as nat + fourth as nat
                > u64::MAX as nat,
    },
))]
pub(super) fn checked_memory_sum(first: u64, second: u64, third: u64, fourth: u64) -> Option<u64> {
    first
        .checked_add(second)?
        .checked_add(third)?
        .checked_add(fourth)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==>
        header.grid_width as nat * header.grid_height as nat * 4
            <= limits.max_decoder_managed_memory_bytes as nat,
))]
pub(super) fn enforce_halftone_memory(
    region_width: u32,
    region_height: u32,
    header: &HalftoneHeader,
    patterns: &[Bitmap],
    bit_depth: u8,
    limits: &Limits,
) -> Result<(), HalftoneError> {
    let mut pattern_bytes = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= patterns@.len(),
        decreases patterns@.len() - index,
    ))]
    while index < patterns.len() {
        let Ok(bytes) = u64::try_from(patterns[index].bytes().len()) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        let Some(next) = pattern_bytes.checked_add(bytes) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        pattern_bytes = next;
        index += 1;
    }
    let Some(cells) = u64::from(header.grid_width).checked_mul(u64::from(header.grid_height))
    else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let Some(values_bytes) = cells.checked_mul(4) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let plane_bytes = bitmap_bytes(header.grid_width, header.grid_height)?;
    let skip_bytes = if header.enable_skip { plane_bytes } else { 0 };
    let active_planes = if bit_depth == 0 {
        0
    } else if bit_depth == 1 {
        1
    } else {
        2
    };
    let Some(active_plane_bytes) = plane_bytes.checked_mul(active_planes) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let Some(gray_peak) =
        checked_memory_sum(pattern_bytes, values_bytes, skip_bytes, active_plane_bytes)
    else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let output_bytes = bitmap_bytes(region_width, region_height)?;
    let Some(render_peak) =
        checked_memory_sum(pattern_bytes, values_bytes, skip_bytes, output_bytes)
    else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let peak = if gray_peak >= render_peak {
        gray_peak
    } else {
        render_peak
    };
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        peak,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            width <= limits.max_width
            && height <= limits.max_height
            && width as nat * height as nat <= limits.max_pixels_per_page,
        Err(_) => true,
    },
))]
#[allow(clippy::cast_lossless)]
pub(super) fn enforce_bitmap_dimensions(
    width: u32,
    height: u32,
    limits: &Limits,
) -> Result<(), HalftoneError> {
    enforce_limit(LimitKind::Width, limits.max_width as u64, width as u64)?;
    enforce_limit(LimitKind::Height, limits.max_height as u64, height as u64)?;
    let Some(pixels) = (width as u64).checked_mul(height as u64) else {
        return Err(HalftoneError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow,
        )));
    };
    enforce_limit(LimitKind::PagePixels, limits.max_pixels_per_page, pixels)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => actual <= limit,
        Err(_) => actual > limit,
    },
))]
pub(super) fn enforce_limit(kind: LimitKind, limit: u64, actual: u64) -> Result<(), HalftoneError> {
    if actual > limit {
        return Err(LimitExceeded {
            kind,
            limit,
            actual,
        }
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) =>
            bytes as nat
                == ((width as nat + 7) / 8) * height as nat,
        Err(_) => true,
    },
))]
#[allow(clippy::cast_lossless)]
pub(super) fn bitmap_bytes(width: u32, height: u32) -> Result<u64, HalftoneError> {
    let Some(stride) = (width as u64).checked_add(7) else {
        return Err(HalftoneError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow,
        )));
    };
    let Some(bytes) = (stride / 8).checked_mul(height as u64) else {
        return Err(HalftoneError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow,
        )));
    };
    Ok(bytes)
}
