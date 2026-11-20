use crate::bitmap::CombinationOperator;
use crate::reader::ByteReader;
use crate::{DecodeError, LimitExceeded, LimitKind, Limits, MalformedError, UnsupportedFeature};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/segment_data.rs"]
mod segment_data_properties;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/segment_data.rs"]
mod segment_data_shared;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/segment_data.rs"]
mod segment_data_proofs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RegionInfo {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) operator: CombinationOperator,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).position <= old(reader).bytes@.len(),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        match out {
        Ok(info) =>
            crate::spec::segments::dimensions_within_limits(info.width, info.height, *limits)
                && final(reader).position == old(reader).position + 17
                && final(reader).position <= final(reader).bytes@.len()
                && crate::spec::segments::region_flags_match(
                    info,
                    old(reader).bytes@[old(reader).position as int + 16],
                ),
        Err(_) => true,
    },
))]
pub(crate) fn parse_region_info(
    reader: &mut ByteReader<'_>,
    limits: &Limits,
) -> Result<RegionInfo, DecodeError> {
    let width = reader.read_u32()?;
    let height = reader.read_u32()?;
    enforce_dimension_limits(width, height, limits)?;
    let x = reader.read_u32()?;
    let y = reader.read_u32()?;
    let flags = reader.read_u8()?;
    if flags & 0xF0 != 0 {
        return Err(MalformedError::ReservedRegionFlags(flags & 0xF0).into());
    }
    if flags & 0x08 != 0 {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let operator = parse_combination_operator(flags & 0x07, true)?;
    Ok(RegionInfo {
        width,
        height,
        x,
        y,
        operator,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct PageInformation {
    pub(crate) width: u32,
    pub(crate) height: Option<u32>,
    pub(crate) x_resolution: u32,
    pub(crate) y_resolution: u32,
    pub(crate) eventually_lossless: bool,
    pub(crate) might_contain_refinements: bool,
    pub(crate) default_pixel: bool,
    pub(crate) default_operator: CombinationOperator,
    pub(crate) requires_auxiliary_buffers: bool,
    pub(crate) combination_operator_override: bool,
    pub(crate) striped: bool,
    pub(crate) max_stripe_size: u16,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(info) =>
            payload@.len() == 19
                && match info.height {
                    Some(height) =>
                        crate::spec::segments::dimensions_within_limits(
                            info.width,
                            height,
                            *limits,
                        ),
                    None => info.width <= limits.max_width && info.striped,
                }
                && info.default_operator != CombinationOperator::Replace
                && crate::spec::segments::page_flags_match(
                    info,
                    payload@[16],
                    ((payload@[17] as u16) << 8) | payload@[18] as u16,
                )
                && crate::spec::segments::page_information_fields(payload@, info),
        Err(_) => true,
    },
))]
pub(crate) fn parse_page_information(
    payload: &[u8],
    limits: &Limits,
) -> Result<PageInformation, DecodeError> {
    require_length(48, payload, 19)?;
    let mut reader = ByteReader::new(payload);
    let width = reader.read_u32()?;
    let encoded_height = reader.read_u32()?;
    let height = if encoded_height == u32::MAX {
        None
    } else {
        Some(encoded_height)
    };
    if let Some(height) = height {
        enforce_dimension_limits(width, height, limits)?;
    } else if width > limits.max_width {
        return Err(LimitExceeded {
            kind: LimitKind::Width,
            limit: u64::from(limits.max_width),
            actual: u64::from(width),
        }
        .into());
    }
    let x_resolution = reader.read_u32()?;
    let y_resolution = reader.read_u32()?;
    let flags = reader.read_u8()?;
    if flags & 0x80 != 0 {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let striping = reader.read_u16()?;
    let striped = striping & 0x8000 != 0;
    if height.is_none() && !striped {
        return Err(MalformedError::UnknownHeightWithoutStriping.into());
    }

    Ok(PageInformation {
        width,
        height,
        x_resolution,
        y_resolution,
        eventually_lossless: flags & 0x01 != 0,
        might_contain_refinements: flags & 0x02 != 0,
        default_pixel: flags & 0x04 != 0,
        default_operator: parse_combination_operator((flags >> 3) & 0x03, false)?,
        requires_auxiliary_buffers: flags & 0x20 != 0,
        combination_operator_override: flags & 0x40 != 0,
        striped,
        max_stripe_size: striping & 0x7FFF,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(value) =>
            payload@.len() == 4
                && value == crate::spec::segments::be_u32_at(payload@, 0),
        Err(_) => true,
    },
))]
pub(crate) fn parse_end_of_stripe(payload: &[u8]) -> Result<u32, DecodeError> {
    require_length(50, payload, 4)?;
    Ok(ByteReader::new(payload).read_u32()?)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => payload@.len() == 0,
        Err(_) => payload@.len() != 0,
    },
))]
pub(crate) fn require_empty(segment_type: u8, payload: &[u8]) -> Result<(), DecodeError> {
    require_length(segment_type, payload, 0)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(profiles) => crate::spec::segments::profiles_payload_fields(payload@, profiles@),
        Err(_) => true,
    },
))]
pub(crate) fn parse_profiles(payload: &[u8]) -> Result<Vec<u32>, DecodeError> {
    if payload.len() < 4 {
        return Err(MalformedError::InvalidSegmentDataLength {
            segment_type: 52,
            expected: 4,
            actual: payload.len() as u64,
        }
        .into());
    }
    let mut reader = ByteReader::new(payload);
    let count = reader.read_u32()?;
    validate_profiles_length(payload, count)?;
    let Ok(count_usize) = usize::try_from(count) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let mut profiles = Vec::new();
    let allocation_bytes = u64::from(count)
        .checked_mul(4)
        .ok_or(MalformedError::IntegerOverflow)?;
    if profiles.try_reserve(count_usize).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: allocation_bytes,
        });
    }
    read_profiles_into(&mut reader, count, &mut profiles)?;
    #[cfg(verus_keep_ghost)]
    production_profiles_fields!(payload, profiles, count);
    Ok(profiles)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).position <= old(reader).bytes@.len(),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).position <= final(reader).position <= final(reader).bytes@.len(),
        match out {
            Ok(()) =>
                final(reader).position == old(reader).position as int + count as int * 4
                && final(profiles)@.len() == old(profiles)@.len() + count
                && (forall|index: int| 0 <= index < old(profiles)@.len() ==>
                    #[trigger] final(profiles)@[index] == old(profiles)@[index])
                && (forall|index: int| 0 <= index < count ==>
                    0 <= old(reader).position as int + index * 4
                    && old(reader).position as int + index * 4 + 4 <= old(reader).bytes@.len()
                    && #[trigger] final(profiles)@[old(profiles)@.len() + index]
                        == crate::spec::segments::be_u32_at(
                            old(reader).bytes@, old(reader).position as int + index * 4,
                        )),
            Err(_) => true,
        },
))]
fn read_profiles_into(
    reader: &mut ByteReader<'_>,
    count: u32,
    profiles: &mut Vec<u32>,
) -> Result<(), DecodeError> {
    let mut index = 0_u32;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost start = reader.position as int; }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost original_profiles = profiles@; }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= count,
            reader.position <= reader.bytes@.len(),
            start == old(reader).position as int,
            original_profiles == old(profiles)@,
            reader.bytes@ == old(reader).bytes@,
            reader.position == start + index as int * 4,
            profiles@.len() == original_profiles.len() + index,
            forall|position: int| 0 <= position < original_profiles.len() ==>
                #[trigger] profiles@[position] == original_profiles[position],
            forall|position: int| 0 <= position < index ==>
                #[trigger] profiles@[original_profiles.len() + position]
                    == crate::spec::segments::be_u32_at(
                        reader.bytes@, start + position * 4,
                    ),
        decreases count - index,
    ))]
    while index < count {
        profiles.push(reader.read_u32()?);
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        payload@.len() >= 4,
        count == crate::spec::segments::be_u32_at(payload@, 0),
    ensures match out {
        Ok(()) => crate::spec::segments::profile_length_matches(payload@, count),
        Err(_) => !crate::spec::segments::profile_length_matches(payload@, count),
    },
))]
fn validate_profiles_length(payload: &[u8], count: u32) -> Result<(), DecodeError> {
    let expected = (u64::from(count) + 1) * 4;
    if payload.len() as u64 != expected {
        return Err(MalformedError::InvalidSegmentDataLength {
            segment_type: 52,
            expected,
            actual: payload.len() as u64,
        }
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => crate::spec::segments::extension_payload_flags_valid(payload@),
        Err(_) => true,
    },
))]
pub(crate) fn parse_extension(payload: &[u8]) -> Result<(), DecodeError> {
    if payload.len() < 4 {
        return Err(MalformedError::InvalidSegmentDataLength {
            segment_type: 62,
            expected: 4,
            actual: payload.len() as u64,
        }
        .into());
    }
    let mut reader = ByteReader::new(payload);
    let extension_type = reader.read_u32()?;
    match classify_extension(extension_type)? {
        1 => validate_comment(reader.remaining_bytes(), 1),
        2 => validate_comment(reader.remaining_bytes(), 2),
        _ => Ok(()),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(kind) => crate::spec::segments::extension_kind(extension_type) == Some(kind),
        Err(_) => crate::spec::segments::extension_kind(extension_type).is_none(),
    },
))]
fn classify_extension(extension_type: u32) -> Result<u8, DecodeError> {
    validate_extension_flags(extension_type)?;
    if extension_type & 0x8000_0000 != 0 {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Extension(
            extension_type,
        )));
    }
    match extension_type {
        0x2000_0000 => Ok(1),
        0x2000_0002 => Ok(2),
        _ => Ok(0),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => crate::spec::segments::extension_flags_valid(extension_type),
        Err(_) => !crate::spec::segments::extension_flags_valid(extension_type),
    },
))]
fn validate_extension_flags(extension_type: u32) -> Result<(), DecodeError> {
    let reserved = extension_type & 0x2000_0000 != 0;
    let necessary = extension_type & 0x8000_0000 != 0;
    if necessary && !reserved {
        return Err(MalformedError::InvalidExtensionType(extension_type).into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires code_unit_size == 1 || code_unit_size == 2,
    ensures match out {
        Ok(()) =>
            bytes@.len() % (code_unit_size as nat) == 0
                && crate::spec::segments::final_code_unit_zero(bytes@, code_unit_size),
        Err(_) => true,
    },
))]
fn validate_comment(bytes: &[u8], code_unit_size: usize) -> Result<(), DecodeError> {
    if bytes.len() % code_unit_size != 0 || bytes.len() < code_unit_size {
        return Err(MalformedError::InvalidCommentExtension.into());
    }
    let mut cursor = 0;
    let mut elements = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            code_unit_size == 1 || code_unit_size == 2,
            bytes@.len() % (code_unit_size as nat) == 0,
            cursor <= bytes@.len(),
            cursor % code_unit_size == 0,
            elements <= cursor,
        decreases bytes@.len() - cursor,
    ))]
    while cursor < bytes.len() {
        let start = cursor;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                code_unit_size == 1 || code_unit_size == 2,
                bytes@.len() % (code_unit_size as nat) == 0,
                start <= cursor,
                cursor <= bytes@.len(),
                cursor % code_unit_size == 0,
                elements <= start,
            decreases bytes@.len() - cursor,
        ))]
        while cursor < bytes.len() && !code_unit_is_zero(bytes, cursor, code_unit_size) {
            cursor = advance_code_unit(cursor, bytes.len(), code_unit_size)?;
        }
        if cursor >= bytes.len() {
            return Err(MalformedError::InvalidCommentExtension.into());
        }
        if cursor == start {
            if comment_terminates_at_end(bytes, cursor, code_unit_size) && elements % 2 == 0 {
                return Ok(());
            }
            return Err(MalformedError::InvalidCommentExtension.into());
        }
        elements = elements
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        cursor = advance_code_unit(cursor, bytes.len(), code_unit_size)?;
    }
    Err(MalformedError::InvalidCommentExtension.into())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        code_unit_size == 1 || code_unit_size == 2,
        cursor < bytes@.len(),
        cursor % code_unit_size == 0,
        bytes@.len() % (code_unit_size as nat) == 0,
    ensures out == forall|i: int|
        cursor <= i < cursor + code_unit_size ==> bytes@[i] == 0,
))]
fn code_unit_is_zero(bytes: &[u8], cursor: usize, code_unit_size: usize) -> bool {
    if code_unit_size == 1 {
        bytes[cursor] == 0
    } else {
        bytes[cursor] == 0 && bytes[cursor + 1] == 0
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            code_unit_size == 1 || code_unit_size == 2,
            cursor < bytes@.len(),
            cursor % code_unit_size == 0,
            bytes@.len() % (code_unit_size as nat) == 0,
        ensures
            out == (
                cursor + code_unit_size == bytes@.len()
                    && forall|i: int|
                        cursor <= i < cursor + code_unit_size ==> bytes@[i] == 0
            ),
            out ==> crate::spec::segments::final_code_unit_zero(bytes@, code_unit_size),
))]
fn comment_terminates_at_end(bytes: &[u8], cursor: usize, code_unit_size: usize) -> bool {
    match advance_code_unit(cursor, bytes.len(), code_unit_size) {
        Ok(next) => next == bytes.len() && code_unit_is_zero(bytes, cursor, code_unit_size),
        Err(_) => false,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        code_unit_size == 1 || code_unit_size == 2,
        cursor < length,
        cursor % code_unit_size == 0,
        length % code_unit_size == 0,
    ensures match out {
        Ok(next) =>
            next == cursor + code_unit_size
                && cursor < next
                && next <= length
                && next % code_unit_size == 0,
        Err(_) => false,
    },
))]
fn advance_code_unit(
    cursor: usize,
    length: usize,
    code_unit_size: usize,
) -> Result<usize, MalformedError> {
    if cursor >= length {
        return Err(MalformedError::IntegerOverflow);
    }
    cursor
        .checked_add(code_unit_size)
        .ok_or(MalformedError::IntegerOverflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(operator) =>
            crate::spec::segments::combination_operator(encoded, replace_allowed)
                == Some(operator),
        Err(_) =>
            crate::spec::segments::combination_operator(encoded, replace_allowed).is_none(),
    },
))]
fn parse_combination_operator(
    encoded: u8,
    replace_allowed: bool,
) -> Result<CombinationOperator, MalformedError> {
    let operator = match encoded {
        0 => CombinationOperator::Or,
        1 => CombinationOperator::And,
        2 => CombinationOperator::Xor,
        3 => CombinationOperator::Xnor,
        4 if replace_allowed => CombinationOperator::Replace,
        _ => return Err(MalformedError::InvalidCombinationOperator(encoded)),
    };
    Ok(operator)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => crate::spec::segments::dimensions_within_limits(width, height, *limits),
        Err(_) => !crate::spec::segments::dimensions_within_limits(width, height, *limits),
    },
))]
fn enforce_dimension_limits(width: u32, height: u32, limits: &Limits) -> Result<(), DecodeError> {
    if width > limits.max_width {
        return Err(LimitExceeded {
            kind: LimitKind::Width,
            limit: u64::from(limits.max_width),
            actual: u64::from(width),
        }
        .into());
    }
    if height > limits.max_height {
        return Err(LimitExceeded {
            kind: LimitKind::Height,
            limit: u64::from(limits.max_height),
            actual: u64::from(height),
        }
        .into());
    }
    #[cfg(verus_keep_ghost)]
    production_dimension_product!(width, height);
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(MalformedError::IntegerOverflow)?;
    if pixels > limits.max_pixels_per_page {
        return Err(LimitExceeded {
            kind: LimitKind::PagePixels,
            limit: limits.max_pixels_per_page,
            actual: pixels,
        }
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => payload@.len() == expected,
        Err(_) => payload@.len() != expected,
    },
))]
fn require_length(segment_type: u8, payload: &[u8], expected: usize) -> Result<(), DecodeError> {
    if payload.len() != expected {
        return Err(MalformedError::InvalidSegmentDataLength {
            segment_type,
            expected: expected as u64,
            actual: payload.len() as u64,
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
