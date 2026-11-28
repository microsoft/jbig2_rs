#[cfg(verus_keep_ghost)]
use super::halftone_spec;
use super::planes::{decode_gray_planes, gray_bit_depth};
use super::resources::{enforce_bitmap_dimensions, enforce_halftone_memory, enforce_limit};
use super::{GenericRegionState, HalftoneError, HalftoneHeader, WorkBudget};
use crate::bitmap::CombinationOperator;
use crate::{Bitmap, DecodeError, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Computes one signed pixel position from the 24.8 halftone grid.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.0 as int == halftone_spec::grid_x_fixed(header, grid_x, grid_y),
        out.1 as int == halftone_spec::grid_y_fixed(header, grid_x, grid_y),
))]
#[allow(clippy::cast_lossless)]
pub(super) fn grid_fixed_position(header: &HalftoneHeader, grid_x: u32, grid_y: u32) -> (i64, i64) {
    let ng = grid_x as i64;
    let mg = grid_y as i64;
    let vector_x = header.grid_vector_x as i64;
    let vector_y = header.grid_vector_y as i64;
    let origin_x = header.grid_x as i64;
    let origin_y = header.grid_y as i64;
    #[cfg(verus_keep_ghost)]
    production_grid_position_bounds!(ng, mg, vector_x, vector_y);
    (
        origin_x + mg * vector_y + ng * vector_x,
        origin_y + mg * vector_x - ng * vector_y,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == halftone_spec::fixed_pixel(value),
))]
pub(super) fn fixed_to_pixel(value: i64) -> i64 {
    value >> 8
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((x, y)) =>
            x == halftone_spec::fixed_pixel(
                halftone_spec::grid_x_fixed(header, grid_x, grid_y) as i64,
            )
            && y == halftone_spec::fixed_pixel(
                halftone_spec::grid_y_fixed(header, grid_x, grid_y) as i64,
            ),
        Err(_) => false,
    },
))]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn grid_position(
    header: &HalftoneHeader,
    grid_x: u32,
    grid_y: u32,
) -> Result<(i64, i64), HalftoneError> {
    let (x_fixed, y_fixed) = grid_fixed_position(header, grid_x, grid_y);
    Ok((fixed_to_pixel(x_fixed), fixed_to_pixel(y_fixed)))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        right as int == x as int + pattern_width as int,
        bottom as int == y as int + pattern_height as int,
    ensures out == halftone_spec::wholly_outside(
        x,
        y,
        pattern_width,
        pattern_height,
        region_width,
        region_height,
    ),
))]
#[allow(clippy::cast_lossless, clippy::too_many_arguments)]
pub(super) fn half_open_outside(
    x: i64,
    y: i64,
    right: i64,
    bottom: i64,
    pattern_width: u32,
    pattern_height: u32,
    region_width: u32,
    region_height: u32,
) -> bool {
    let _ = (pattern_width, pattern_height);
    right <= 0 || x >= region_width as i64 || bottom <= 0 || y >= region_height as i64
}

/// Builds the normative skip mask for wholly out-of-bounds patterns.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires halftone_spec::work_valid(old(work)),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::views::bitmap_width(&bitmap) == header.grid_width
            && crate::views::bitmap_height(&bitmap) == header.grid_height
            && halftone_spec::work_valid(final(work)),
        Err(_) => true,
    },
))]
#[allow(clippy::cast_lossless, clippy::too_many_arguments)]
pub(crate) fn generate_skip_mask(
    header: &HalftoneHeader,
    region_width: u32,
    region_height: u32,
    pattern_width: u32,
    pattern_height: u32,
    limits: &Limits,
    work: &mut WorkBudget,
) -> Result<Bitmap, HalftoneError> {
    let cells = u64::from(header.grid_width)
        .checked_mul(u64::from(header.grid_height))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(cells)?;
    let mut skip = Bitmap::blank(header.grid_width, header.grid_height, limits)?;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            crate::spec::bitmap::storage_valid(&skip),
            crate::views::bitmap_width(&skip) == header.grid_width,
            crate::views::bitmap_height(&skip) == header.grid_height,
            mg <= header.grid_height,
        decreases header.grid_height - mg,
    ))]
    for mg in 0..header.grid_height {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                crate::spec::bitmap::storage_valid(&skip),
                crate::views::bitmap_width(&skip) == header.grid_width,
                crate::views::bitmap_height(&skip) == header.grid_height,
                ng <= header.grid_width,
                mg < header.grid_height,
            decreases header.grid_width - ng,
        ))]
        for ng in 0..header.grid_width {
            let (x, y) = grid_position(header, ng, mg)?;
            let right = x
                .checked_add(pattern_width as i64)
                .ok_or(MalformedError::IntegerOverflow)?;
            let bottom = y
                .checked_add(pattern_height as i64)
                .ok_or(MalformedError::IntegerOverflow)?;
            #[cfg(verus_keep_ghost)]
            production_skip_boundary!(
                x,
                y,
                pattern_width,
                pattern_height,
                region_width,
                region_height
            );
            let value = half_open_outside(
                x,
                y,
                right,
                bottom,
                pattern_width,
                pattern_height,
                region_width,
                region_height,
            );
            skip.set_pixel(ng, mg, value)?;
        }
    }
    Ok(skip)
}

/// Validates every decoded gray value before it can index the dictionary.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => halftone_spec::indexes_valid(indexes@, pattern_count),
        Err(_) => !halftone_spec::indexes_valid(indexes@, pattern_count),
    },
))]
pub(crate) fn validate_pattern_indices(
    indexes: &[u32],
    pattern_count: u64,
) -> Result<(), HalftoneError> {
    let mut position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            position <= indexes@.len(),
            forall|i: int| 0 <= i < position
                ==> (indexes@[i] as u64) < pattern_count,
        decreases indexes.len() - position,
    ))]
    while position < indexes.len() {
        let index = indexes[position];
        if u64::from(index) >= pattern_count {
            return Err(HalftoneError::InvalidPatternIndex {
                index,
                pattern_count,
            });
        }
        position += 1;
    }
    Ok(())
}

/// Decodes and renders a complete halftone region.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        halftone_spec::decoder_state_valid(old(decoder)),
        halftone_spec::work_valid(old(work)),
        halftone_spec::bitmaps_valid(patterns@),
    ensures match out {
        Ok(bitmap) =>
            halftone_spec::decoder_state_valid(final(decoder))
                && halftone_spec::work_valid(final(work))
                && crate::spec::bitmap::storage_valid(&bitmap)
                && (patterns@.len() == 1 ==> *final(decoder) == *old(decoder)),
        Err(_) => true,
    },
))]
pub(crate) fn decode_halftone_region(
    header: &HalftoneHeader,
    region_width: u32,
    region_height: u32,
    patterns: &[Bitmap],
    decoder: &mut GenericRegionState<'_>,
    limits: &Limits,
    work: &mut WorkBudget,
) -> Result<Bitmap, HalftoneError> {
    if header.template > 3 || (header.mmr && (header.template != 0 || header.enable_skip)) {
        return Err(HalftoneError::InvalidHalftoneConfiguration);
    }
    enforce_bitmap_dimensions(region_width, region_height, limits)?;
    let first = patterns
        .first()
        .ok_or(HalftoneError::EmptyPatternDictionary)?;
    let pattern_width = first.width();
    let pattern_height = first.height();
    if pattern_width == 0 || pattern_height == 0 {
        return Err(HalftoneError::InconsistentPatternDimensions);
    }
    let mut pattern_position = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            pattern_position <= patterns@.len(),
            halftone_spec::bitmaps_valid(patterns@),
        decreases patterns@.len() - pattern_position,
    ))]
    while pattern_position < patterns.len() {
        let pattern = &patterns[pattern_position];
        if pattern.width() != pattern_width || pattern.height() != pattern_height {
            return Err(HalftoneError::InconsistentPatternDimensions);
        }
        pattern_position += 1;
    }
    let pattern_count =
        u64::try_from(patterns.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Symbols,
        u64::from(limits.max_symbols),
        pattern_count,
    )?;
    let bit_depth = gray_bit_depth(pattern_count)?;
    enforce_halftone_memory(
        region_width,
        region_height,
        header,
        patterns,
        bit_depth,
        limits,
    )?;

    let grid_cells = u64::from(header.grid_width)
        .checked_mul(u64::from(header.grid_height))
        .ok_or(MalformedError::IntegerOverflow)?;
    let pattern_pixels = u64::from(pattern_width)
        .checked_mul(u64::from(pattern_height))
        .ok_or(MalformedError::IntegerOverflow)?;
    let render_work = grid_cells
        .checked_mul(
            pattern_pixels
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?,
        )
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(render_work)?;

    let skip = if header.enable_skip {
        Some(generate_skip_mask(
            header,
            region_width,
            region_height,
            pattern_width,
            pattern_height,
            limits,
            work,
        )?)
    } else {
        None
    };
    let indexes = decode_gray_planes(
        header.grid_width,
        header.grid_height,
        bit_depth,
        header.mmr,
        header.template,
        skip.as_ref(),
        decoder,
        limits,
        work,
    )?;
    validate_pattern_indices(&indexes, pattern_count)?;

    let mut output = Bitmap::filled(region_width, region_height, header.default_pixel, limits)?;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            mg <= header.grid_height,
            crate::spec::bitmap::storage_valid(&output),
            halftone_spec::bitmaps_valid(patterns@),
            halftone_spec::indexes_valid(indexes@, pattern_count),
        decreases header.grid_height - mg,
    ))]
    for mg in 0..header.grid_height {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                ng <= header.grid_width,
                mg < header.grid_height,
                crate::spec::bitmap::storage_valid(&output),
                halftone_spec::bitmaps_valid(patterns@),
                halftone_spec::indexes_valid(indexes@, pattern_count),
            decreases header.grid_width - ng,
        ))]
        for ng in 0..header.grid_width {
            let offset = pixel_offset(header.grid_width, ng, mg)?;
            let pattern_index = *indexes.get(offset).ok_or(MalformedError::IntegerOverflow)?;
            let pattern = patterns
                .get(usize::try_from(pattern_index).map_err(|_| MalformedError::IntegerOverflow)?)
                .ok_or(HalftoneError::InvalidPatternIndex {
                    index: pattern_index,
                    pattern_count,
                })?;
            let (x, y) = grid_position(header, ng, mg)?;
            render_clipped_pattern(&mut output, pattern, x, y, header.operator)?;
        }
    }
    Ok(output)
}

#[allow(clippy::cast_lossless, clippy::similar_names, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        crate::spec::bitmap::storage_valid(old(destination)),
        crate::spec::bitmap::storage_valid(pattern),
    ensures
        crate::spec::bitmap::storage_valid(final(destination)),
        halftone_spec::clipped_pattern_frame(old(destination), pattern, x, y, final(destination)),
))]
pub(super) fn render_clipped_pattern(
    destination: &mut Bitmap,
    pattern: &Bitmap,
    x: i64,
    y: i64,
    operator: CombinationOperator,
) -> Result<(), HalftoneError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(halftone_spec::clipped_pattern_frame);
    }
    let pattern_width = pattern.width();
    let pattern_height = pattern.height();
    let destination_width = destination.width();
    let destination_height = destination.height();
    let pattern_width_i64 = pattern_width as i64;
    let pattern_height_i64 = pattern_height as i64;
    let right = x
        .checked_add(pattern_width_i64)
        .ok_or(MalformedError::IntegerOverflow)?;
    let bottom = y
        .checked_add(pattern_height_i64)
        .ok_or(MalformedError::IntegerOverflow)?;
    let (destination_x_start, destination_x_end) = clipped_axis(x, right, destination_width);
    let (destination_y_start, destination_y_end) = clipped_axis(y, bottom, destination_height);
    if destination_x_start >= destination_x_end || destination_y_start >= destination_y_end {
        return Ok(());
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        crate::views::bitmap_fields(pattern);
        reveal(halftone_spec::clipped_axis_valid);
        assert(0 <= destination_x_start < destination_x_end);
        assert(destination_x_end <= destination_width);
        assert(x <= destination_x_start);
        assert(destination_x_end <= right);
        assert(0 <= destination_y_start < destination_y_end);
        assert(destination_y_end <= destination_height);
        assert(y <= destination_y_start);
        assert(destination_y_end <= bottom);
        assert(right == x + pattern_width_i64);
        assert(bottom == y + pattern_height_i64);
        assert(!halftone_spec::wholly_outside(
            x, y, pattern.width, pattern.height, destination.width, destination.height,
        ));
    }

    let source_x = u32::try_from(
        destination_x_start
            .checked_sub(x)
            .ok_or(MalformedError::IntegerOverflow)?,
    )
    .map_err(|_| MalformedError::IntegerOverflow)?;
    let source_y = u32::try_from(
        destination_y_start
            .checked_sub(y)
            .ok_or(MalformedError::IntegerOverflow)?,
    )
    .map_err(|_| MalformedError::IntegerOverflow)?;
    let destination_x =
        u32::try_from(destination_x_start).map_err(|_| MalformedError::IntegerOverflow)?;
    let destination_y =
        u32::try_from(destination_y_start).map_err(|_| MalformedError::IntegerOverflow)?;
    let width = u32::try_from(destination_x_end - destination_x_start)
        .map_err(|_| MalformedError::IntegerOverflow)?;
    let height = u32::try_from(destination_y_end - destination_y_start)
        .map_err(|_| MalformedError::IntegerOverflow)?;
    destination.compose_region(
        pattern,
        source_x,
        source_y,
        destination_x,
        destination_y,
        width,
        height,
        operator,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures halftone_spec::clipped_axis_valid(
        origin,
        far_edge,
        bound,
        out.0,
        out.1,
    ),
))]
#[allow(clippy::cast_lossless)]
pub(super) fn clipped_axis(origin: i64, far_edge: i64, bound: u32) -> (i64, i64) {
    (origin.max(0), far_edge.min(bound as i64))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out as nat == halftone_spec::row_major_offset(width, x, y),
))]
pub(super) fn row_major_offset(width: u32, x: u32, y: u32) -> u64 {
    #[cfg(verus_keep_ghost)]
    production_row_major_bounds!(y, width);
    u64::from(y) * u64::from(width) + u64::from(x)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(offset) =>
            offset as nat == halftone_spec::row_major_offset(width, x, y),
        Err(_) =>
            halftone_spec::row_major_offset(width, x, y) > usize::MAX,
    },
))]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn pixel_offset(width: u32, x: u32, y: u32) -> Result<usize, HalftoneError> {
    let offset = row_major_offset(width, x, y);
    if offset > usize::MAX as u64 {
        return Err(HalftoneError::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow,
        )));
    }
    Ok(offset as usize)
}
