#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::{GenericRegionError, TemplateOffset};
use crate::{Bitmap, DecodeError, LimitExceeded, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == match index {
        0 => (word >> 24) as u8,
        1 => (word >> 16) as u8,
        2 => (word >> 8) as u8,
        _ => word as u8,
    },
))]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn word_byte(word: u32, index: usize) -> u8 {
    match index {
        0 => (word >> 24) as u8,
        1 => (word >> 16) as u8,
        2 => (word >> 8) as u8,
        _ => word as u8,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(bitmap),
    ensures out == generic_region_spec::bitmap_pixel_at(
        bitmap, x as int + offset.x as int, y as int + offset.y as int,
    ),
))]
#[allow(clippy::cast_lossless)]
pub(super) fn sample_bitmap(bitmap: &Bitmap, x: u32, y: u32, offset: TemplateOffset) -> bool {
    let sample_x = (x as i64).checked_add(offset.x as i64);
    let sample_y = (y as i64).checked_add(offset.y as i64);
    sample_signed(bitmap, sample_x, sample_y)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(bitmap),
    ensures out == match (x, y) {
        (Some(xv), Some(yv)) =>
            super::generic_region_spec::bitmap_pixel_at(bitmap, xv as int, yv as int),
        _ => false,
    },
))]
#[allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub(super) fn sample_signed(bitmap: &Bitmap, x: Option<i64>, y: Option<i64>) -> bool {
    let (Some(x), Some(y)) = (x, y) else {
        return false;
    };
    if x < 0 || x > u32::MAX as i64 || y < 0 || y > u32::MAX as i64 {
        return false;
    }
    bitmap.pixel(x as u32, y as u32).unwrap_or(false)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            bitmap_bytes as nat + context_bytes as nat
                <= limits.max_decoder_managed_memory_bytes,
        Err(_) => true,
    },
))]
pub(crate) fn enforce_managed_memory(
    context_bytes: u64,
    bitmap_bytes: u64,
    limits: &Limits,
) -> Result<(), GenericRegionError> {
    let Some(managed) = bitmap_bytes.checked_add(context_bytes) else {
        return Err(integer_overflow());
    };
    if managed > limits.max_decoder_managed_memory_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual: managed,
        })
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) => bytes as nat
            == generic_region_spec::bitmap_storage_spec(width as nat, height as nat),
        Err(_) => true,
    },
))]
pub(super) fn bitmap_storage_bytes(width: u32, height: u32) -> Result<u64, GenericRegionError> {
    let Some(stride) = u64::from(width).checked_add(7) else {
        return Err(integer_overflow());
    };
    let Some(bytes) = (stride / 8).checked_mul(u64::from(height)) else {
        return Err(integer_overflow());
    };
    Ok(bytes)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => generic_region_spec::generic_work_spec(
            width as nat,
            rows as nat,
            typical_prediction,
        ) <= limits.max_work_units,
        Err(_) => true,
    },
))]
pub(super) fn enforce_generic_work(
    width: u32,
    rows: u32,
    typical_prediction: bool,
    limits: &Limits,
) -> Result<(), GenericRegionError> {
    enforce_work(generic_work(width, rows, typical_prediction)?, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(work) => work as nat == generic_region_spec::generic_work_spec(
            width as nat,
            rows as nat,
            typical_prediction,
        ),
        Err(_) => generic_region_spec::generic_work_spec(
            width as nat,
            rows as nat,
            typical_prediction,
        ) > u64::MAX,
    },
))]
pub(super) fn generic_work(
    width: u32,
    rows: u32,
    typical_prediction: bool,
) -> Result<u64, GenericRegionError> {
    let Some(pixels) = u64::from(width).checked_mul(u64::from(rows)) else {
        return Err(integer_overflow());
    };
    let Some(mut work) = pixels.checked_mul(17) else {
        return Err(integer_overflow());
    };
    if typical_prediction {
        let Some(next_work) = work.checked_add(u64::from(rows)) else {
            return Err(integer_overflow());
        };
        work = next_work;
    }
    Ok(work)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => generic_region_spec::refinement_work_spec(
            width as nat,
            height as nat,
            typical_prediction,
        ) <= limits.max_work_units,
        Err(_) => true,
    },
))]
pub(super) fn enforce_refinement_work(
    width: u32,
    height: u32,
    typical_prediction: bool,
    limits: &Limits,
) -> Result<(), GenericRegionError> {
    enforce_work(refinement_work(width, height, typical_prediction)?, limits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(work) => work as nat
            == generic_region_spec::refinement_work_spec(
                width as nat,
                height as nat,
                typical_prediction,
            ),
        Err(_) => true,
    },
))]
pub(super) fn refinement_work(
    width: u32,
    height: u32,
    typical_prediction: bool,
) -> Result<u64, GenericRegionError> {
    let Some(pixels) = u64::from(width).checked_mul(u64::from(height)) else {
        return Err(integer_overflow());
    };
    let multiplier = if typical_prediction { 23 } else { 14 };
    let Some(partial) = pixels.checked_mul(multiplier) else {
        return Err(integer_overflow());
    };
    let Some(work) = partial.checked_add(u64::from(height)) else {
        return Err(integer_overflow());
    };
    Ok(work)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => work <= limits.max_work_units,
        Err(_) => work > limits.max_work_units,
    },
))]
pub(super) fn enforce_work(work: u64, limits: &Limits) -> Result<(), GenericRegionError> {
    if work > limits.max_work_units {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Work,
            limit: limits.max_work_units,
            actual: work,
        })
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => data@.len() <= limits.max_stream_bytes,
        Err(_) => true,
    },
))]
pub(super) fn enforce_input_limit(data: &[u8], limits: &Limits) -> Result<(), GenericRegionError> {
    let actual = u64::try_from(data.len()).map_err(|_error| MalformedError::IntegerOverflow)?;
    if actual > limits.max_stream_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::StreamBytes,
            limit: limits.max_stream_bytes,
            actual,
        })
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == GenericRegionError::Decode(
        DecodeError::Malformed(MalformedError::IntegerOverflow),
    ),
))]
pub(crate) fn integer_overflow() -> GenericRegionError {
    GenericRegionError::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == GenericRegionError::Decode(
        crate::arithmetic::arithmetic_spec::decode_error(
            crate::arithmetic::arithmetic_spec::eof_error(offset, needed, remaining),
        ),
    ),
))]
pub(super) fn unexpected_eof(offset: usize, needed: usize, remaining: usize) -> GenericRegionError {
    let Ok(offset) = u64::try_from(offset) else {
        return GenericRegionError::Decode(MalformedError::IntegerOverflow.into());
    };
    let Ok(needed) = u64::try_from(needed) else {
        return GenericRegionError::Decode(MalformedError::IntegerOverflow.into());
    };
    let Ok(remaining) = u64::try_from(remaining) else {
        return GenericRegionError::Decode(MalformedError::IntegerOverflow.into());
    };
    GenericRegionError::Decode(
        MalformedError::UnexpectedEof {
            offset,
            needed,
            remaining,
        }
        .into(),
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        source < old(bitmap).height,
        destination < old(bitmap).height,
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        final(bitmap).stride == old(bitmap).stride,
        final(bitmap).data@.len() == old(bitmap).data@.len(),
        crate::spec::bitmap::row_copy_result(old(bitmap), final(bitmap), source, destination),
        forall|x: u32| x < old(bitmap).width ==>
            crate::spec::bitmap::stored_pixel(final(bitmap), x, destination)
                == crate::spec::bitmap::stored_pixel(old(bitmap), x, source),
        out.is_ok(),
))]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn copy_bitmap_row(
    bitmap: &mut Bitmap,
    source: u32,
    destination: u32,
) -> Result<(), GenericRegionError> {
    bitmap.copy_row(source, destination)?;
    #[cfg(verus_keep_ghost)]
    production_generic_copied_row_pixels!(bitmap, source, destination);
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}
