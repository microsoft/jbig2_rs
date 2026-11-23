//! Generic colour extensions retain the original mask and foreground identifier.

use super::headers::parse_generic_region_header_with_colour;
use super::segments::decode_known_bitmap;
use super::{DecodedGenericRegion, GenericRegionError};
use crate::mmr::MmrScratch;
use crate::reader::ByteReader;
use crate::{Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((decoded, foreground)) =>
            crate::spec::bitmap::storage_valid(&decoded.bitmap)
            && decoded.consumed_bytes == payload@.len()
            && (foreground.is_some() ==> payload@.len() >= 4
                && foreground.unwrap() == crate::spec::segments::be_u32_at(
                    payload@, payload@.len() - 4,
                )),
        Err(_) => true,
    },
))]
pub(crate) fn decode_generic_with_colour(
    payload: &[u8],
    limits: &Limits,
    scratch: &mut MmrScratch,
) -> Result<(DecodedGenericRegion, Option<u32>), GenericRegionError> {
    let (header, header_length, colour) =
        parse_generic_region_header_with_colour(payload, limits, true)?;
    let coded_end = if colour {
        payload
            .len()
            .checked_sub(4)
            .ok_or(MalformedError::IntegerOverflow)?
    } else {
        payload.len()
    };
    if header_length > coded_end {
        return Err(MalformedError::InvalidPageState("incomplete coloured generic payload").into());
    }
    let foreground = if colour {
        Some(ByteReader::new(&payload[coded_end..]).read_u32()?)
    } else {
        None
    };
    let bitmap = decode_known_bitmap(&payload[header_length..coded_end], &header, limits, scratch)?;
    let rows = header.region.height;
    Ok((
        DecodedGenericRegion {
            header,
            bitmap,
            consumed_bytes: payload.len(),
            decoded_rows: rows,
        },
        foreground,
    ))
}
