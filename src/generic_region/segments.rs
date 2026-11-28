#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::headers::{parse_generic_region_header, validate_generic_adaptive_values};
use super::support::{enforce_generic_work, enforce_input_limit, unexpected_eof};
use super::{
    DecodedGenericRegion, DecodedGenericRegionDirect, GenericArithmeticCall,
    GenericArithmeticSession, GenericRegionError, GenericRegionHeader, UnknownLengthData,
    ARITHMETIC_TERMINATOR, MMR_UNKNOWN_LENGTH_SEPARATOR,
};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::mmr::{decode_mmr_reused, EofbPolicy, MmrScratch};
use crate::{Bitmap, DecodeError, LimitExceeded, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Decode a complete generic-region data part.
///
/// If `unknown_length` is true, `payload` may include following input.  The
/// returned `consumed_bytes` stops immediately after the terminal row count.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(decoded) => crate::spec::bitmap::storage_valid(&decoded.bitmap),
        Err(_) => true,
    },
))]
pub(crate) fn decode_generic_region_segment(
    payload: &[u8],
    unknown_length: bool,
    limits: &Limits,
) -> Result<DecodedGenericRegion, GenericRegionError> {
    let mut scratch = MmrScratch::default();
    decode_generic_region_segment_reused(payload, unknown_length, limits, &mut scratch)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(decoded) => crate::spec::bitmap::storage_valid(&decoded.bitmap),
        Err(_) => true,
    },
))]
pub(crate) fn decode_generic_region_segment_reused(
    payload: &[u8],
    unknown_length: bool,
    limits: &Limits,
    scratch: &mut MmrScratch,
) -> Result<DecodedGenericRegion, GenericRegionError> {
    let mut generic_scratch = Vec::new();
    decode_generic_region_segment_reused_with_scratch(
        payload,
        unknown_length,
        limits,
        scratch,
        &mut generic_scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(decoded) => crate::spec::bitmap::storage_valid(&decoded.bitmap),
        Err(_) => true,
    },
))]
pub(crate) fn decode_generic_region_segment_reused_with_scratch(
    payload: &[u8],
    unknown_length: bool,
    limits: &Limits,
    scratch: &mut MmrScratch,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<DecodedGenericRegion, GenericRegionError> {
    let (header, header_length) = parse_generic_region_header(payload, limits)?;
    if header_length > payload.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let coded_and_following = &payload[header_length..];

    let (coded, rows, coded_consumed, eofb_policy) = if unknown_length {
        let terminal = split_unknown_length_data(
            coded_and_following,
            header.mmr,
            header.region.height,
            limits,
        )?;
        (
            terminal.coded,
            terminal.rows,
            terminal.consumed,
            EofbPolicy::Required,
        )
    } else {
        (
            coded_and_following,
            header.region.height,
            coded_and_following.len(),
            EofbPolicy::Optional,
        )
    };

    let bitmap = if !unknown_length {
        decode_known_bitmap(coded, &header, limits, scratch, generic_scratch)?
    } else if header.mmr {
        let decoded = decode_mmr_reused(
            coded,
            header.region.width,
            rows,
            limits,
            eofb_policy,
            scratch,
        )?;
        pad_bitmap_height(decoded, header.region.height, limits)?
    } else {
        decode_arithmetic_generic_with_scratch(coded, &header, rows, None, limits, generic_scratch)?
    };

    let consumed_bytes = header_length
        .checked_add(coded_consumed)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(DecodedGenericRegion {
        header,
        bitmap,
        consumed_bytes,
        decoded_rows: rows,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(&out.unwrap()),
))]
pub(super) fn decode_known_bitmap(
    coded: &[u8],
    header: &GenericRegionHeader,
    limits: &Limits,
    scratch: &mut MmrScratch,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<Bitmap, GenericRegionError> {
    if header.mmr {
        let bitmap = decode_mmr_reused(
            coded,
            header.region.width,
            header.region.height,
            limits,
            EofbPolicy::Optional,
            scratch,
        )?;
        pad_bitmap_height(bitmap, header.region.height, limits)
    } else {
        decode_known_length_arithmetic_generic_with_scratch(
            coded,
            header,
            header.region.height,
            limits,
            generic_scratch,
        )
    }
}

/// Decode an arithmetic generic region directly into a matching, caller-owned
/// blank bitmap. Unsupported encodings return `Ok(None)` before modification.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
#[allow(dead_code)]
pub(crate) fn decode_generic_region_segment_into(
    payload: &[u8],
    unknown_length: bool,
    limits: &Limits,
    bitmap: &mut Bitmap,
) -> Result<Option<DecodedGenericRegionDirect>, GenericRegionError> {
    let mut generic_scratch = Vec::new();
    decode_generic_region_segment_into_with_scratch(
        payload,
        unknown_length,
        limits,
        bitmap,
        &mut generic_scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
pub(crate) fn decode_generic_region_segment_into_with_scratch(
    payload: &[u8],
    unknown_length: bool,
    limits: &Limits,
    bitmap: &mut Bitmap,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<Option<DecodedGenericRegionDirect>, GenericRegionError> {
    let (header, header_length) = parse_generic_region_header(payload, limits)?;
    if header.mmr
        || bitmap.width() != header.region.width
        || bitmap.height() != header.region.height
    {
        return Ok(None);
    }
    let coded_and_following = payload
        .get(header_length..)
        .ok_or(MalformedError::IntegerOverflow)?;
    let (rows, coded_consumed) = if unknown_length {
        let terminal =
            split_unknown_length_data(coded_and_following, false, header.region.height, limits)?;
        decode_arithmetic_generic_into_with_scratch(
            terminal.coded,
            &header,
            terminal.rows,
            limits,
            bitmap,
            generic_scratch,
        )?;
        (terminal.rows, terminal.consumed)
    } else {
        decode_known_length_arithmetic_generic_into_with_scratch(
            coded_and_following,
            &header,
            header.region.height,
            limits,
            bitmap,
            generic_scratch,
        )?;
        (header.region.height, coded_and_following.len())
    };
    let consumed_bytes = header_length
        .checked_add(coded_consumed)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(Some(DecodedGenericRegionDirect {
        header,
        consumed_bytes,
        decoded_rows: rows,
    }))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires match skip {
        Some(mask) => crate::spec::bitmap::storage_valid(mask),
        None => true,
    },
    ensures match out {
        Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
        Err(_) => true,
    },
))]
pub(super) fn decode_arithmetic_generic_with_scratch(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    skip: Option<&Bitmap>,
    limits: &Limits,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<Bitmap, GenericRegionError> {
    if header.mmr || rows > header.region.height {
        return Err(GenericRegionError::InvalidFlags(0x01));
    }
    validate_arithmetic_terminator(coded)?;
    validate_generic_adaptive_values(header.template, header.extended_template, &header.adaptive)?;
    enforce_generic_work(header.region.width, rows, header.typical_prediction, limits)?;
    if let Some(skip_bitmap) = skip {
        if skip_bitmap.width() != header.region.width
            || skip_bitmap.height() != header.region.height
        {
            return Err(GenericRegionError::InvalidSkipDimensions);
        }
    }

    let mut session = GenericArithmeticSession::new_from_scratch(
        coded,
        header.template,
        header.extended_template,
        limits,
        generic_scratch,
    )?;
    let mut result = session.decode_bitmap_rows(
        GenericArithmeticCall {
            width: header.region.width,
            height: header.region.height,
            adaptive: header.adaptive,
            typical_prediction: header.typical_prediction,
            skip,
        },
        rows,
    );
    if result.is_ok() {
        if let Err(error) = session.validate_final_marker() {
            result = Err(error);
        }
    }
    session.into_scratch(generic_scratch);
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
pub(super) fn decode_arithmetic_generic_into_with_scratch(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
    bitmap: &mut Bitmap,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<(), GenericRegionError> {
    if header.mmr
        || rows > header.region.height
        || bitmap.width() != header.region.width
        || bitmap.height() != header.region.height
    {
        return Err(GenericRegionError::InvalidFlags(0x01));
    }
    validate_arithmetic_terminator(coded)?;
    validate_generic_adaptive_values(header.template, header.extended_template, &header.adaptive)?;
    enforce_generic_work(header.region.width, rows, header.typical_prediction, limits)?;
    let mut session = GenericArithmeticSession::new_from_scratch(
        coded,
        header.template,
        header.extended_template,
        limits,
        generic_scratch,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    let mut result = session.decode_bitmap_rows_into(
        GenericArithmeticCall {
            width: header.region.width,
            height: header.region.height,
            adaptive: header.adaptive,
            typical_prediction: header.typical_prediction,
            skip: None,
        },
        rows,
        bitmap,
    );
    if result.is_ok() {
        if let Err(error) = session.validate_final_marker() {
            result = Err(error);
        }
    }
    session.into_scratch(generic_scratch);
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
        Err(_) => true,
    },
))]
#[cfg(test)]
pub(super) fn decode_known_length_arithmetic_generic(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
) -> Result<Bitmap, GenericRegionError> {
    let mut generic_scratch = Vec::new();
    decode_known_length_arithmetic_generic_with_scratch(
        coded,
        header,
        rows,
        limits,
        &mut generic_scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
        Err(_) => true,
    },
))]
pub(super) fn decode_known_length_arithmetic_generic_with_scratch(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<Bitmap, GenericRegionError> {
    let has_terminator = coded.len() >= 2
        && coded[coded.len() - 2] == ARITHMETIC_TERMINATOR[0]
        && coded[coded.len() - 1] == ARITHMETIC_TERMINATOR[1];
    if has_terminator {
        return decode_arithmetic_generic_with_scratch(
            coded,
            header,
            rows,
            None,
            limits,
            generic_scratch,
        );
    }
    if coded.last() != Some(&0xff) {
        return Err(GenericRegionError::InvalidArithmeticTerminator);
    }
    let Ok(requested_bytes) = u64::try_from(coded.len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let requested_bytes = requested_bytes
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    if requested_bytes > limits.max_decoder_managed_memory_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual: requested_bytes,
        })
        .into());
    }
    let mut normalized = Vec::new();
    let capacity = coded
        .len()
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    if normalized.try_reserve(capacity).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= coded@.len(),
            normalized@ == coded@.subrange(0, index as int),
        decreases coded@.len() - index,
    ))]
    while index < coded.len() {
        normalized.push(coded[index]);
        index += 1;
    }
    normalized.push(ARITHMETIC_TERMINATOR[1]);
    decode_arithmetic_generic_with_scratch(&normalized, header, rows, None, limits, generic_scratch)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
#[cfg(test)]
pub(super) fn decode_known_length_arithmetic_generic_into(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
    bitmap: &mut Bitmap,
) -> Result<(), GenericRegionError> {
    let mut generic_scratch = Vec::new();
    decode_known_length_arithmetic_generic_into_with_scratch(
        coded,
        header,
        rows,
        limits,
        bitmap,
        &mut generic_scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
pub(super) fn decode_known_length_arithmetic_generic_into_with_scratch(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
    bitmap: &mut Bitmap,
    generic_scratch: &mut Vec<ArithmeticContext>,
) -> Result<(), GenericRegionError> {
    let has_terminator = coded.len() >= 2
        && coded[coded.len() - 2] == ARITHMETIC_TERMINATOR[0]
        && coded[coded.len() - 1] == ARITHMETIC_TERMINATOR[1];
    if has_terminator {
        return decode_arithmetic_generic_into_with_scratch(
            coded,
            header,
            rows,
            limits,
            bitmap,
            generic_scratch,
        );
    }
    if coded.last() != Some(&0xff) {
        return Err(GenericRegionError::InvalidArithmeticTerminator);
    }
    let requested_bytes = u64::try_from(coded.len())
        .map_err(|_error| MalformedError::IntegerOverflow)?
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    if requested_bytes > limits.max_decoder_managed_memory_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual: requested_bytes,
        })
        .into());
    }
    let capacity = coded
        .len()
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    let mut normalized = Vec::new();
    if normalized.try_reserve(capacity).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= coded@.len(),
            normalized@ == coded@.subrange(0, index as int),
        decreases coded@.len() - index,
    ))]
    while index < coded.len() {
        normalized.push(coded[index]);
        index += 1;
    }
    normalized.push(ARITHMETIC_TERMINATOR[1]);
    decode_arithmetic_generic_into_with_scratch(
        &normalized,
        header,
        rows,
        limits,
        bitmap,
        generic_scratch,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(split) =>
            split.rows <= declared_height
            && split.consumed <= data@.len()
            && split.coded@.len() <= data@.len()
            && (!mmr ==> (
                split.coded@.len() >= 2
                && split.coded@[split.coded@.len() - 2] == 0xff
                && split.coded@[split.coded@.len() - 1] == 0xac
            )),
        Err(_) => true,
    },
))]
pub(super) fn split_unknown_length_data<'a>(
    data: &'a [u8],
    mmr: bool,
    declared_height: u32,
    limits: &Limits,
) -> Result<UnknownLengthData<'a>, GenericRegionError> {
    enforce_input_limit(data, limits)?;
    let marker = if mmr {
        MMR_UNKNOWN_LENGTH_SEPARATOR
    } else {
        ARITHMETIC_TERMINATOR
    };

    let mut marker_offset = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant marker_offset <= data@.len(),
        decreases data@.len() - marker_offset,
    ))]
    while marker_offset < data.len() {
        let Some(next) = marker_offset.checked_add(1) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        if next >= data.len() {
            return Err(MalformedError::MissingGenericRegionTerminator.into());
        }
        if data[marker_offset] == marker[0] && data[next] == marker[1] {
            break;
        }
        marker_offset = next;
    }
    if marker_offset == data.len() {
        return Err(MalformedError::MissingGenericRegionTerminator.into());
    }
    let count_start = marker_offset
        .checked_add(2)
        .ok_or(MalformedError::IntegerOverflow)?;
    let count_end = count_start
        .checked_add(4)
        .ok_or(MalformedError::IntegerOverflow)?;
    let remaining = if count_start <= data.len() {
        data.len() - count_start
    } else {
        0
    };
    if count_end > data.len() {
        return Err(unexpected_eof(count_start, 4, remaining));
    }
    let rows = (u32::from(data[count_start]) << 24)
        | (u32::from(data[count_start + 1]) << 16)
        | (u32::from(data[count_start + 2]) << 8)
        | u32::from(data[count_start + 3]);
    if rows > declared_height {
        return Err(MalformedError::GenericRegionRowCount {
            declared: declared_height,
            actual: rows,
        }
        .into());
    }

    let coded_end = if mmr { marker_offset } else { count_start };
    if coded_end > data.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let coded = &data[..coded_end];
    if !mmr {
        validate_arithmetic_terminator(coded)?;
    }
    Ok(UnknownLengthData {
        coded,
        rows,
        consumed: count_end,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures generic_region_spec::arithmetic_terminator_validation(data@, out),
))]
pub(super) fn validate_arithmetic_terminator(data: &[u8]) -> Result<(), GenericRegionError> {
    if data.len() < 2
        || data[data.len() - 2] != ARITHMETIC_TERMINATOR[0]
        || data[data.len() - 1] != ARITHMETIC_TERMINATOR[1]
    {
        return Err(GenericRegionError::InvalidArithmeticTerminator);
    }
    let final_offset = data
        .len()
        .checked_sub(2)
        .ok_or(GenericRegionError::InvalidArithmeticTerminator)?;
    let mut offset = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            data@.len() >= 2,
            final_offset == data@.len() - 2,
            offset <= data@.len() - 1,
            generic_region_spec::bounded_arithmetic_tail(data@),
            forall|before: int| 0 <= before < offset ==>
                !generic_region_spec::early_arithmetic_marker(data@, before),
        decreases data@.len() - 1 - offset,
    ))]
    while offset < data.len() - 1 {
        if data[offset] == 0xff && data[offset + 1] > 0x8f && offset != final_offset {
            return Err(GenericRegionError::UnexpectedArithmeticMarker {
                offset,
                code: data[offset + 1],
            });
        }
        offset += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == generic_region_spec::observed_marker_result(decoder.marker, coded_length),
))]
pub(super) fn validate_observed_marker(
    decoder: &ArithmeticDecoder<'_>,
    coded_length: usize,
) -> Result<(), GenericRegionError> {
    if let Some(marker) = decoder.marker() {
        let expected_offset = coded_length
            .checked_sub(2)
            .ok_or(GenericRegionError::InvalidArithmeticTerminator)?;
        if marker.code != ARITHMETIC_TERMINATOR[1] || marker.offset != expected_offset {
            return Err(GenericRegionError::UnexpectedArithmeticMarker {
                offset: marker.offset,
                code: marker.code,
            });
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(&decoded),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && bitmap.width == decoded.width
            && bitmap.height == nominal_height
            && forall|index: int| 0 <= index < crate::views::bitmap_data(&bitmap).len() ==>
                crate::views::bitmap_data(&bitmap)[index]
                    == if index < crate::views::bitmap_data(&decoded).len() {
                        crate::views::bitmap_data(&decoded)[index]
                    } else { 0u8 },
        Err(_) => true,
    },
))]
pub(super) fn pad_bitmap_height(
    decoded: Bitmap,
    nominal_height: u32,
    limits: &Limits,
) -> Result<Bitmap, GenericRegionError> {
    if decoded.height == nominal_height {
        return Ok(decoded);
    }
    if decoded.height > nominal_height {
        return Err(GenericRegionError::RowCountMismatch {
            expected: nominal_height,
            actual: decoded.height,
        });
    }
    let mut padded = Bitmap::blank(decoded.width, nominal_height, limits)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&padded);
        reveal(crate::spec::bitmap::storage_valid);
        reveal(crate::spec::bitmap::zeroed_storage);
    }
    let byte_count = decoded.data.len();
    if byte_count > padded.data.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    padded.copy_prefix_from(&decoded, byte_count)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&padded);
        crate::views::bitmap_fields(&decoded);
        reveal(crate::spec::bitmap::storage_valid);
        reveal(crate::spec::bitmap::prefix_copy_result);
    }
    Ok(padded)
}
