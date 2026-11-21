//! Validation implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    widen_u32_i64, EntropyInteger, IntegerField, LimitExceeded, LimitKind, Limits, MalformedError,
    SymbolCodec, SymbolTextError,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(value) =>
            value as int == current as int + delta as int
            && value <= maximum,
        Err(_) => true,
    },
))]
pub(super) fn checked_unsigned_delta(
    current: u32,
    delta: i64,
    maximum: u32,
) -> Result<u32, SymbolTextError> {
    let result = widen_u32_i64(current)
        .checked_add(delta)
        .ok_or(MalformedError::IntegerOverflow)?;
    let result = u32::try_from(result).map_err(|_error| SymbolTextError::InvalidDimensions)?;
    if result > maximum {
        return Err(SymbolTextError::InvalidDimensions);
    }
    Ok(result)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::codec_valid(old(codec)),
    ensures match out {
        Ok(_) => symbol_text_spec::codec_valid(final(codec)),
        Err(_) => true,
    },
))]
pub(super) fn required_integer(
    codec: &mut SymbolCodec<'_>,
    field: IntegerField,
) -> Result<i64, SymbolTextError> {
    match codec.decode_integer(field)? {
        EntropyInteger::Value(value) => Ok(value),
        EntropyInteger::OutOfBand => Err(SymbolTextError::UnexpectedOutOfBand("required integer")),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() == (
        width <= limits.max_width
        && height <= limits.max_height
        && width as nat * height as nat <= limits.max_pixels_per_page
    ),
    match out {
        Ok(()) =>
            width <= limits.max_width
            && height <= limits.max_height
            && width as nat * height as nat <= limits.max_pixels_per_page,
        Err(_) => true,
    },
))]
pub(super) fn check_dimensions(
    width: u32,
    height: u32,
    limits: &Limits,
) -> Result<(), SymbolTextError> {
    enforce_limit(
        LimitKind::Width,
        u64::from(limits.max_width),
        u64::from(width),
    )?;
    enforce_limit(
        LimitKind::Height,
        u64::from(limits.max_height),
        u64::from(height),
    )?;
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(LimitKind::PagePixels, limits.max_pixels_per_page, pixels)?;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => actual <= limit,
        Err(error) => actual > limit
            && error == SymbolTextError::Decode(DecodeError::LimitExceeded(
                LimitExceeded { kind, limit, actual },
            )),
    },
))]
pub(super) fn enforce_limit(
    kind: LimitKind,
    limit: u64,
    actual: u64,
) -> Result<(), SymbolTextError> {
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
