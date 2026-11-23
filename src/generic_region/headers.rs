#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::support::enforce_input_limit;
use super::{
    GenericRegionError, GenericRegionHeader, GenericTemplate, RefinementRegionHeader,
    RefinementTemplate, TemplateOffset,
};
use crate::reader::ByteReader;
use crate::segment_data::{parse_region_info, parse_region_info_with_colour};
use crate::{Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (flags & 0x01 != 0),
))]
pub(super) fn generic_mmr_flag(flags: u8) -> bool {
    flags & 0x01 != 0
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (flags & 0x08 != 0),
))]
pub(super) fn generic_typical_prediction_flag(flags: u8) -> bool {
    flags & 0x08 != 0
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (flags & 0x10 != 0),
))]
pub(super) fn generic_extended_template_flag(flags: u8) -> bool {
    flags & 0x10 != 0
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == flags & 0xe0,
))]
pub(super) fn generic_reserved_flags(flags: u8) -> u8 {
    flags & 0xe0
}

/// Parse a generic-region header and return its byte length.
///
/// For extended template 0 this deliberately consumes 24 bytes: twelve signed
/// X/Y pairs.  T.88's isolated phrase "32-byte field" conflicts with both the
/// enumerated byte range 0..=23 and the twelve defined adaptive pixels.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((header, length)) =>
            payload@.len() >= 18
                && length == generic_region_spec::generic_header_length(payload@[17])
                && length <= payload@.len()
                && header.mmr == (payload@[17] & 1 != 0),
        Err(_) => true,
    },
))]
pub(crate) fn parse_generic_region_header(
    payload: &[u8],
    limits: &Limits,
) -> Result<(GenericRegionHeader, usize), GenericRegionError> {
    let (header, length, _) = parse_generic_region_header_with_colour(payload, limits, false)?;
    Ok((header, length))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((header, length, colour)) =>
            payload@.len() >= 18
                && length == generic_region_spec::generic_header_length(payload@[17])
                && length <= payload@.len()
                && header.mmr == (payload@[17] & 1 != 0)
                && crate::spec::segments::dimensions_within_limits(
                    header.region.width, header.region.height, *limits,
                )
                && colour == (payload@[16] & 8 != 0)
                && (colour ==> allow_colour
                    && header.region.operator == crate::bitmap::CombinationOperator::Replace),
        Err(_) => true,
    },
))]
pub(super) fn parse_generic_region_header_with_colour(
    payload: &[u8],
    limits: &Limits,
    allow_colour: bool,
) -> Result<(GenericRegionHeader, usize, bool), GenericRegionError> {
    enforce_input_limit(payload, limits)?;
    let mut reader = ByteReader::new(payload);
    let (region, colour) = parse_region_info_with_colour(&mut reader, limits, allow_colour)?;
    let flags = reader.read_u8()?;
    let reserved = generic_reserved_flags(flags);
    if reserved != 0 {
        return Err(GenericRegionError::ReservedFlags(reserved));
    }

    let mmr = generic_mmr_flag(flags);
    let template = GenericTemplate::from_flags(flags);
    let typical_prediction = generic_typical_prediction_flag(flags);
    let extended_template = generic_extended_template_flag(flags);
    if mmr && template != GenericTemplate::Template0 {
        return Err(GenericRegionError::InvalidFlags(flags));
    }

    let mut adaptive = [TemplateOffset { x: 0, y: 0 }; 12];
    if !mmr {
        let count = match template {
            GenericTemplate::Template0 => {
                if extended_template {
                    12
                } else {
                    4
                }
            }
            _ => 1,
        };
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                index <= count,
                count <= 12,
                reader.bytes@ == payload@,
                reader.position == 18 + 2 * index,
                18 + 2 * count == generic_region_spec::generic_header_length(flags),
                flags == payload@[17],
                mmr == (flags & 1 != 0),
                reader.position <= reader.bytes@.len(),
            decreases count - index,
        ))]
        while index < count {
            adaptive[index] = read_offset(&mut reader)?;
            let Ok(number) = u8::try_from(index + 1) else {
                return Err(MalformedError::IntegerOverflow.into());
            };
            validate_target_adaptive(number, adaptive[index])?;
            index += 1;
        }
    }

    Ok((
        GenericRegionHeader {
            region,
            mmr,
            template,
            typical_prediction,
            extended_template,
            adaptive,
        },
        reader.position(),
        colour,
    ))
}

/// Parse a generic-refinement region data header and return its byte length.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) fn parse_refinement_region_header(
    payload: &[u8],
    limits: &Limits,
) -> Result<(RefinementRegionHeader, usize), GenericRegionError> {
    enforce_input_limit(payload, limits)?;
    let mut reader = ByteReader::new(payload);
    let region = parse_region_info(&mut reader, limits)?;
    let flags = reader.read_u8()?;
    if flags & 0xfc != 0 {
        return Err(GenericRegionError::ReservedFlags(flags & 0xfc));
    }
    let template = RefinementTemplate::from_flags(flags);
    let typical_prediction = flags & 0x02 != 0;
    let mut adaptive = [TemplateOffset { x: 0, y: 0 }; 2];
    if template == RefinementTemplate::Template0 {
        adaptive[0] = read_offset(&mut reader)?;
        adaptive[1] = read_offset(&mut reader)?;
        validate_target_adaptive(1, adaptive[0])?;
    }
    Ok((
        RefinementRegionHeader {
            region,
            template,
            typical_prediction,
            adaptive,
        },
        reader.position(),
    ))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).position <= old(reader).bytes@.len(),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        final(reader).position <= final(reader).bytes@.len(),
        out.is_ok() ==> final(reader).position == old(reader).position + 2,
))]
pub(super) fn read_offset(
    reader: &mut ByteReader<'_>,
) -> Result<TemplateOffset, GenericRegionError> {
    let x = signed_offset_byte(reader.read_u8()?);
    let y = signed_offset_byte(reader.read_u8()?);
    Ok(TemplateOffset::new(x, y))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out as int == if value <= i8::MAX as u8 {
        value as int
    } else {
        value as int - 256
    },
))]
#[allow(clippy::cast_lossless, clippy::checked_conversions)]
pub(super) fn signed_offset_byte(value: u8) -> i32 {
    if value <= i8::MAX as u8 {
        value as i32
    } else {
        value as i32 - 256
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => generic_region_spec::target_adaptive_valid(offset),
        Err(_) => !generic_region_spec::target_adaptive_valid(offset),
    },
))]
pub(super) fn validate_target_adaptive(
    number: u8,
    offset: TemplateOffset,
) -> Result<(), GenericRegionError> {
    if offset.y < 0 || (offset.y == 0 && offset.x < 0) {
        return Ok(());
    }
    Err(GenericRegionError::InvalidAdaptivePixel {
        number,
        x: offset.x,
        y: offset.y,
    })
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn validate_generic_adaptive_values(
    template: GenericTemplate,
    extended_template: bool,
    adaptive: &[TemplateOffset; 12],
) -> Result<(), GenericRegionError> {
    let count = match template {
        GenericTemplate::Template0 if extended_template => 12,
        GenericTemplate::Template0 => 4,
        _ => 1,
    };
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            count <= adaptive@.len(),
            index <= count,
        decreases count - index,
    ))]
    while index < count {
        let number = (index + 1) as u8;
        validate_target_adaptive(number, adaptive[index])?;
        index += 1;
    }
    Ok(())
}
