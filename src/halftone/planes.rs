#[cfg(verus_keep_ghost)]
use super::halftone_spec;
use super::render::pixel_offset;
use super::resources::{
    bitmap_bytes, checked_memory_sum, enforce_bitmap_dimensions, enforce_limit,
    require_bitmap_dimensions,
};
use super::{
    GenericRegionCoding, GenericRegionRequest, GenericRegionState, GrayIndexes, HalftoneError,
    PatternDictionaryHeader, WorkBudget,
};
use crate::bitmap::CombinationOperator;
use crate::{Bitmap, DecodeError, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(width) =>
            width as nat == pattern_count as nat * pattern_width as nat
            && pattern_count as nat * pattern_width as nat <= u32::MAX as nat,
        None =>
            pattern_count as nat * pattern_width as nat > u32::MAX as nat,
    },
))]
pub(super) fn checked_collective_width(pattern_count: u64, pattern_width: u8) -> Option<u32> {
    let width = pattern_count.checked_mul(u64::from(pattern_width))?;
    u32::try_from(width).ok()
}

/// Decodes and slices a pattern dictionary's collective bitmap.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        halftone_spec::decoder_state_valid(old(decoder)),
        halftone_spec::work_valid(old(work)),
    ensures match out {
        Ok(patterns) =>
            halftone_spec::decoder_state_valid(final(decoder))
                && halftone_spec::work_valid(final(work))
                && halftone_spec::bitmaps_valid(patterns@),
        Err(_) => true,
    },
))]
pub(crate) fn decode_pattern_dictionary(
    header: PatternDictionaryHeader,
    decoder: &mut GenericRegionState<'_>,
    limits: &Limits,
    work: &mut WorkBudget,
) -> Result<Vec<Bitmap>, HalftoneError> {
    validate_pattern_header(header)?;
    let pattern_count = header.pattern_count();
    enforce_limit(
        LimitKind::Symbols,
        u64::from(limits.max_symbols),
        pattern_count,
    )?;
    let collective_width = checked_collective_width(pattern_count, header.pattern_width)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_bitmap_dimensions(collective_width, u32::from(header.pattern_height), limits)?;
    let pixels = u64::from(collective_width)
        .checked_mul(u64::from(header.pattern_height))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(pixels)?;

    let coding = if header.mmr {
        GenericRegionCoding::Mmr {
            require_eofb: false,
        }
    } else {
        GenericRegionCoding::Arithmetic {
            template: header.template,
            adaptive_pixels: pattern_adaptive_pixels(header.pattern_width),
        }
    };
    let collective = decoder.decode_generic_region(GenericRegionRequest {
        width: collective_width,
        height: u32::from(header.pattern_height),
        coding,
        typical_prediction: false,
        extended_template: false,
        skip: None,
    })?;
    require_bitmap_dimensions(
        &collective,
        collective_width,
        u32::from(header.pattern_height),
    )?;
    slice_collective_bitmap(&collective, header, limits, work)
}

/// Slices a checked collective bitmap into owned fixed-size patterns.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(collective),
        halftone_spec::work_valid(old(work)),
    ensures match out {
        Ok(patterns) =>
            halftone_spec::bitmaps_valid(patterns@)
                && halftone_spec::work_valid(final(work)),
        Err(_) => true,
    },
))]
#[allow(clippy::cast_lossless, clippy::too_many_lines)]
pub(crate) fn slice_collective_bitmap(
    collective: &Bitmap,
    header: PatternDictionaryHeader,
    limits: &Limits,
    work: &mut WorkBudget,
) -> Result<Vec<Bitmap>, HalftoneError> {
    validate_pattern_header(header)?;
    let raw_pattern_width = header.pattern_width;
    let raw_pattern_height = header.pattern_height;
    let pattern_width = raw_pattern_width as u32;
    let pattern_height = raw_pattern_height as u32;
    let pattern_count = header.pattern_count();
    enforce_limit(
        LimitKind::Symbols,
        u64::from(limits.max_symbols),
        pattern_count,
    )?;
    let expected_width = checked_collective_width(pattern_count, header.pattern_width)
        .ok_or(MalformedError::IntegerOverflow)?;
    require_bitmap_dimensions(collective, expected_width, u32::from(header.pattern_height))?;

    let pattern_bytes = bitmap_bytes(pattern_width, pattern_height)?;
    let retained_pattern_bytes = pattern_bytes
        .checked_mul(pattern_count)
        .ok_or(MalformedError::IntegerOverflow)?;
    let vector_bytes = pattern_count
        .checked_mul(match u64::try_from(core::mem::size_of::<Bitmap>()) {
            Ok(bytes) => bytes,
            Err(_) => return Err(MalformedError::IntegerOverflow.into()),
        })
        .ok_or(MalformedError::IntegerOverflow)?;
    let Ok(collective_bytes) = u64::try_from(collective.bytes().len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let total_memory =
        checked_memory_sum(collective_bytes, retained_pattern_bytes, vector_bytes, 0)
            .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        total_memory,
    )?;

    let total_pixels = pattern_count
        .checked_mul(u64::from(pattern_width))
        .ok_or(MalformedError::IntegerOverflow)?
        .checked_mul(u64::from(pattern_height))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(total_pixels)?;

    let Ok(count) = usize::try_from(pattern_count) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let mut patterns = Vec::new();
    if patterns.try_reserve(count).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: vector_bytes,
        }
        .into());
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            crate::spec::bitmap::storage_valid(collective),
            pattern_index <= pattern_count,
            patterns@.len() == pattern_index,
            halftone_spec::bitmaps_valid(patterns@),
            raw_pattern_width == header.pattern_width,
            raw_pattern_height == header.pattern_height,
            raw_pattern_width > 0,
            raw_pattern_height > 0,
            pattern_width > 0,
            pattern_height > 0,
            pattern_width as nat == raw_pattern_width as nat,
            pattern_height as nat == raw_pattern_height as nat,
            expected_width as nat
                == pattern_count as nat * header.pattern_width as nat,
            pattern_count as nat * header.pattern_width as nat
                <= u32::MAX as nat,
        decreases pattern_count - pattern_index,
    ))]
    for pattern_index in 0..pattern_count {
        let mut pattern = Bitmap::blank(pattern_width, pattern_height, limits)?;
        let source_x = pattern_index
            .checked_mul(u64::from(pattern_width))
            .ok_or(MalformedError::IntegerOverflow)?;
        let Ok(source_x) = u32::try_from(source_x) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(source_x as nat
                == pattern_index as nat * header.pattern_width as nat);
        }
        #[cfg(verus_keep_ghost)]
        {
            let last_pattern_x = pattern_width - 1;
            production_slice_coordinate!(
                pattern_index,
                pattern_count,
                last_pattern_x,
                header,
                source_x,
                expected_width
            );
        }
        pattern.compose_region(
            collective,
            source_x,
            0,
            0,
            0,
            pattern_width,
            pattern_height,
            CombinationOperator::Replace,
        )?;
        patterns.push(pattern);
    }
    Ok(patterns)
}

/// Returns the number of gray planes needed for a pattern count.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires 1 <= pattern_count <= 4_294_967_296,
    ensures out == halftone_spec::gray_depth_expected(pattern_count),
))]
pub(super) fn gray_bit_depth_value(pattern_count: u64) -> u8 {
    if pattern_count <= 1 {
        0
    } else if pattern_count <= 2 {
        1
    } else if pattern_count <= 4 {
        2
    } else if pattern_count <= 8 {
        3
    } else if pattern_count <= 16 {
        4
    } else if pattern_count <= 32 {
        5
    } else if pattern_count <= 64 {
        6
    } else if pattern_count <= 128 {
        7
    } else if pattern_count <= 256 {
        8
    } else if pattern_count <= 512 {
        9
    } else if pattern_count <= 1_024 {
        10
    } else if pattern_count <= 2_048 {
        11
    } else if pattern_count <= 4_096 {
        12
    } else if pattern_count <= 8_192 {
        13
    } else if pattern_count <= 16_384 {
        14
    } else if pattern_count <= 32_768 {
        15
    } else if pattern_count <= 65_536 {
        16
    } else if pattern_count <= 131_072 {
        17
    } else if pattern_count <= 262_144 {
        18
    } else if pattern_count <= 524_288 {
        19
    } else if pattern_count <= 1_048_576 {
        20
    } else if pattern_count <= 2_097_152 {
        21
    } else if pattern_count <= 4_194_304 {
        22
    } else if pattern_count <= 8_388_608 {
        23
    } else if pattern_count <= 16_777_216 {
        24
    } else if pattern_count <= 33_554_432 {
        25
    } else if pattern_count <= 67_108_864 {
        26
    } else if pattern_count <= 134_217_728 {
        27
    } else if pattern_count <= 268_435_456 {
        28
    } else if pattern_count <= 536_870_912 {
        29
    } else if pattern_count <= 1_073_741_824 {
        30
    } else if pattern_count <= 2_147_483_648 {
        31
    } else {
        32
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(depth) =>
            1 <= pattern_count <= 4_294_967_296
            && depth == halftone_spec::gray_depth_expected(pattern_count),
        Err(_) => pattern_count == 0 || pattern_count > 4_294_967_296,
    },
))]
pub(crate) fn gray_bit_depth(pattern_count: u64) -> Result<u8, HalftoneError> {
    if pattern_count == 0 || pattern_count > (u64::from(u32::MAX) + 1) {
        return Err(MalformedError::IntegerOverflow.into());
    }
    if pattern_count == 1 {
        #[cfg(verus_keep_ghost)]
        production_one_pattern_zero_plane!();
    }
    Ok(gray_bit_depth_value(pattern_count))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == halftone_spec::gray_binary_bit(coded, previous),
))]
pub(super) const fn gray_to_binary_bit(coded: bool, previous: Option<bool>) -> bool {
    match previous {
        Some(bit) => coded != bit,
        None => coded,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(current)),
        crate::spec::bitmap::storage_valid(previous),
    ensures match out {
        Ok(()) => crate::spec::bitmap::storage_valid(final(current)),
        Err(_) => true,
    },
))]
pub(super) fn xor_gray_plane_bytes(
    current: &mut Bitmap,
    previous: &Bitmap,
) -> Result<(), HalftoneError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(current);
    }
    if current.width != previous.width
        || current.height != previous.height
        || current.stride != previous.stride
        || current.data.len() != previous.data.len()
    {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= current.data@.len(),
            current.width == old(current).width,
            current.height == old(current).height,
            current.stride == old(current).stride,
            current.data@.len() == old(current).data@.len(),
            current.data@.len() == previous.data@.len(),
        decreases current.data@.len() - index,
    ))]
    while index < current.data.len() {
        current.data[index] ^= previous.data[index];
        index += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(current);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires plane_index < 32,
    ensures
        final(values)@.len() == old(values)@.len(),
        match out {
            Ok(()) => offset as nat + 8 <= old(values)@.len(),
            Err(_) => true,
        },
))]
pub(super) fn accumulate_gray_byte(
    values: &mut [u32],
    offset: usize,
    byte: u8,
    plane_index: u8,
) -> Result<(), HalftoneError> {
    let end = offset
        .checked_add(8)
        .ok_or(MalformedError::IntegerOverflow)?;
    let chunk = values
        .get_mut(offset..end)
        .ok_or(MalformedError::IntegerOverflow)?;
    chunk[0] |= u32::from((byte >> 7) & 1) << plane_index;
    chunk[1] |= u32::from((byte >> 6) & 1) << plane_index;
    chunk[2] |= u32::from((byte >> 5) & 1) << plane_index;
    chunk[3] |= u32::from((byte >> 4) & 1) << plane_index;
    chunk[4] |= u32::from((byte >> 3) & 1) << plane_index;
    chunk[5] |= u32::from((byte >> 2) & 1) << plane_index;
    chunk[6] |= u32::from((byte >> 1) & 1) << plane_index;
    chunk[7] |= u32::from(byte & 1) << plane_index;
    Ok(())
}

/// Decodes Gray-coded planes and reconstructs unsigned pattern indexes.
#[allow(
    clippy::cast_lossless,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        halftone_spec::decoder_state_valid(old(decoder)),
        halftone_spec::work_valid(old(work)),
        match skip {
            Some(bitmap) => crate::spec::bitmap::storage_valid(bitmap),
            None => true,
        },
    ensures match out {
        Ok(indexes) =>
            halftone_spec::decoder_state_valid(final(decoder))
                && halftone_spec::work_valid(final(work))
                && (bit_depth == 0 ==> (
                    *final(decoder) == *old(decoder)
                    && *final(work) == *old(work)
                    && halftone_spec::index_values(&indexes).len() == width as nat * height as nat
                    && forall|index: int|
                        0 <= index < halftone_spec::index_values(&indexes).len() ==>
                            halftone_spec::index_values(&indexes)[index] == 0
                )),
        Err(_) => true,
    },
))]
pub(crate) fn decode_gray_planes(
    width: u32,
    height: u32,
    bit_depth: u8,
    mmr: bool,
    template: u8,
    skip: Option<&Bitmap>,
    decoder: &mut GenericRegionState<'_>,
    limits: &Limits,
    work: &mut WorkBudget,
) -> Result<GrayIndexes, HalftoneError> {
    if bit_depth > 32 {
        return Err(HalftoneError::InvalidGrayBitDepth(bit_depth));
    }
    if template > 3 || (mmr && (template != 0 || skip.is_some())) {
        return Err(HalftoneError::InvalidHalftoneConfiguration);
    }
    enforce_bitmap_dimensions(width, height, limits)?;
    if let Some(skip_bitmap) = skip {
        require_bitmap_dimensions(skip_bitmap, width, height)?;
    }

    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(MalformedError::IntegerOverflow)?;
    let value_bytes = pixels
        .checked_mul(if bit_depth <= 8 { 1 } else { 4 })
        .ok_or(MalformedError::IntegerOverflow)?;
    let plane_bytes = bitmap_bytes(width, height)?;
    let concurrent_plane_count = if bit_depth == 0 {
        0
    } else if bit_depth == 1 {
        1
    } else {
        2
    };
    let skip_bytes = if skip.is_some() { plane_bytes } else { 0 };
    let concurrent_plane_bytes = plane_bytes
        .checked_mul(concurrent_plane_count)
        .ok_or(MalformedError::IntegerOverflow)?;
    let managed_bytes = checked_memory_sum(value_bytes, concurrent_plane_bytes, skip_bytes, 0)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        managed_bytes,
    )?;

    let pixel_count = usize::try_from(pixels).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut values = GrayIndexes::zeroed(pixel_count, bit_depth)?;
    if bit_depth == 0 {
        return Ok(values);
    }

    let plane_work = pixels
        .checked_mul(u64::from(bit_depth))
        .and_then(|value| value.checked_mul(2))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(plane_work)?;

    let coding = if mmr {
        GenericRegionCoding::Mmr { require_eofb: true }
    } else {
        GenericRegionCoding::Arithmetic {
            template,
            adaptive_pixels: gray_adaptive_pixels(template),
        }
    };
    let mut more_significant: Option<Bitmap> = None;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            halftone_spec::decoder_state_valid(decoder),
            halftone_spec::work_valid(work),
            halftone_spec::index_values(&values).len() == pixel_count,
            match skip {
                Some(bitmap) => crate::spec::bitmap::storage_valid(bitmap),
                None => true,
            },
            match more_significant {
                Some(ref bitmap) => crate::spec::bitmap::storage_valid(bitmap),
                None => true,
            },
    ))]
    for plane_index in (0..bit_depth).rev() {
        let mut plane = decoder.decode_generic_region(GenericRegionRequest {
            width,
            height,
            coding,
            typical_prediction: false,
            extended_template: false,
            skip,
        })?;
        require_bitmap_dimensions(&plane, width, height)?;
        if let Some(previous) = more_significant.as_ref() {
            xor_gray_plane_bytes(&mut plane, previous)?;
        }
        if plane_index >= 32 {
            return Err(HalftoneError::InvalidGrayBitDepth(bit_depth));
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(plane_index < 32);
        }
        let width_usize =
            usize::try_from(width).map_err(|_error| MalformedError::IntegerOverflow)?;
        let full_bytes = width_usize / 8;
        let tail_start = width
            .checked_sub(width % 8)
            .ok_or(MalformedError::IntegerOverflow)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::div_mod::lemma_mod_bound(width as int, 8);
            assert(tail_start <= width);
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                y <= height,
                plane_index < 32,
                tail_start <= width,
                crate::spec::bitmap::storage_valid(&plane),
                halftone_spec::index_values(&values).len() == pixel_count,
                match more_significant {
                    Some(ref bitmap) => crate::spec::bitmap::storage_valid(bitmap),
                    None => true,
                },
            decreases height - y,
        ))]
        for y in 0..height {
            let y_usize = usize::try_from(y).map_err(|_error| MalformedError::IntegerOverflow)?;
            let row_start = y_usize
                .checked_mul(plane.stride)
                .ok_or(MalformedError::IntegerOverflow)?;
            let value_row = y_usize
                .checked_mul(width_usize)
                .ok_or(MalformedError::IntegerOverflow)?;
            let mut byte_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    byte_index <= full_bytes,
                    y < height,
                    plane_index < 32,
                    crate::spec::bitmap::storage_valid(&plane),
                    halftone_spec::index_values(&values).len() == pixel_count,
                    match more_significant {
                        Some(ref bitmap) => crate::spec::bitmap::storage_valid(bitmap),
                        None => true,
                    },
                decreases full_bytes - byte_index,
            ))]
            while byte_index < full_bytes {
                let source_index = row_start
                    .checked_add(byte_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let byte = *plane
                    .data
                    .get(source_index)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let value_offset = value_row
                    .checked_add(
                        byte_index
                            .checked_mul(8)
                            .ok_or(MalformedError::IntegerOverflow)?,
                    )
                    .ok_or(MalformedError::IntegerOverflow)?;
                values.accumulate_byte(value_offset, byte, plane_index)?;
                byte_index += 1;
            }
            let mut x = tail_start;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    x <= width,
                    y < height,
                    plane_index < 32,
                    crate::spec::bitmap::storage_valid(&plane),
                    halftone_spec::index_values(&values).len() == pixel_count,
                decreases width - x,
            ))]
            while x < width {
                let true_value = plane.pixel(x, y).ok_or(MalformedError::IntegerOverflow)?;
                if true_value {
                    let offset = pixel_offset(width, x, y)?;
                    values.insert_bit(offset, plane_index)?;
                }
                x += 1;
            }
        }
        more_significant = Some(plane);
    }
    Ok(values)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == [
        (-pattern_width as i32, 0i32),
        (-3i32, -1i32),
        (2i32, -2i32),
        (-2i32, -2i32),
    ],
))]
#[allow(clippy::cast_lossless)]
pub(super) fn pattern_adaptive_pixels(pattern_width: u8) -> [(i32, i32); 4] {
    [(-(pattern_width as i32), 0), (-3, -1), (2, -2), (-2, -2)]
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == [
        (if template <= 1 { 3i32 } else { 2i32 }, -1i32),
        (-3i32, -1i32),
        (2i32, -2i32),
        (-2i32, -2i32),
    ],
))]
pub(super) const fn gray_adaptive_pixels(template: u8) -> [(i32, i32); 4] {
    [
        (if template <= 1 { 3 } else { 2 }, -1),
        (-3, -1),
        (2, -2),
        (-2, -2),
    ]
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            header.pattern_width > 0
            && header.pattern_height > 0
            && header.template <= 3
            && (!header.mmr || header.template == 0),
        Err(_) => true,
    },
))]
#[allow(clippy::bool_to_int_with_if)]
pub(super) fn validate_pattern_header(
    header: PatternDictionaryHeader,
) -> Result<(), HalftoneError> {
    if header.pattern_width == 0 || header.pattern_height == 0 {
        return Err(HalftoneError::InvalidPatternDimensions {
            width: header.pattern_width,
            height: header.pattern_height,
        });
    }
    if header.template > 3 || (header.mmr && header.template != 0) {
        let encoded = (if header.mmr { 1 } else { 0 }) | (header.template << 1);
        return Err(HalftoneError::InvalidPatternDictionaryFlags(encoded));
    }
    Ok(())
}
