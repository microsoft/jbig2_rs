use super::{
    get_runtime_page, runtime_page_index, Bitmap, CombinationOperator, DecodeError, DecodeWork,
    DocumentInfo, Limits, MalformedError, PageInfo, ParsedSegment, RuntimePage, RuntimePages,
};

#[cfg(verus_keep_ghost)]
use super::decode_spec;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (left == right),
))]
pub(super) fn same_combination_operator(
    left: crate::bitmap::CombinationOperator,
    right: crate::bitmap::CombinationOperator,
) -> bool {
    match left {
        crate::bitmap::CombinationOperator::Or => {
            matches!(right, crate::bitmap::CombinationOperator::Or)
        }
        crate::bitmap::CombinationOperator::And => {
            matches!(right, crate::bitmap::CombinationOperator::And)
        }
        crate::bitmap::CombinationOperator::Xor => {
            matches!(right, crate::bitmap::CombinationOperator::Xor)
        }
        crate::bitmap::CombinationOperator::Xnor => {
            matches!(right, crate::bitmap::CombinationOperator::Xnor)
        }
        crate::bitmap::CombinationOperator::Replace => {
            matches!(right, crate::bitmap::CombinationOperator::Replace)
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            combination_override
                || region_operator == page_operator
                || refinement,
        Err(_) =>
            !combination_override
                && region_operator != page_operator
                && !refinement,
    },
))]
pub(crate) fn check_combination_operator(
    combination_override: bool,
    region_operator: crate::bitmap::CombinationOperator,
    page_operator: crate::bitmap::CombinationOperator,
    refinement: bool,
) -> Result<(), DecodeError> {
    let same_operator = same_combination_operator(region_operator, page_operator);
    if !combination_override && !same_operator && !refinement {
        return Err(MalformedError::InvalidPageState(
            "region overrides the page combination operator",
        )
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => equal,
        Err(_) => !equal,
    },
))]
pub(super) fn require_matching_geometry(equal: bool) -> Result<(), DecodeError> {
    if !equal {
        return Err(MalformedError::InvalidPageState(
            "refinement geometry differs from its intermediate reference",
        )
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        decode_spec::runtime_pages_valid(old(pages)),
        crate::spec::bitmap::storage_valid(bitmap),
    ensures decode_spec::runtime_pages_valid(final(pages)),
))]
pub(super) fn compose_immediate(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    region: &crate::segment_data::RegionInfo,
    bitmap: &Bitmap,
    refinement: bool,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let index = runtime_page_index(pages, segment.header.page_association).ok_or(
        MalformedError::MissingPageInformation(segment.header.page_association),
    )?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost original_pages = pages@;
    }
    let result = compose_immediate_on_page(
        &mut pages[index].1,
        region,
        bitmap,
        refinement,
        work,
        limits,
    );
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(decode_spec::runtime_pages_valid);
        assert forall|entry: int| 0 <= entry < pages@.len()
            implies crate::spec::bitmap::storage_valid(
                #[trigger] &pages@[entry].1.bitmap
            ) by {
            if entry == index {
            } else {
                assert(pages@[entry] == original_pages[entry]);
            }
        }
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(&old(page).bitmap),
        crate::spec::bitmap::storage_valid(bitmap),
    ensures crate::spec::bitmap::storage_valid(&final(page).bitmap),
))]
pub(super) fn compose_immediate_on_page(
    page: &mut RuntimePage,
    region: &crate::segment_data::RegionInfo,
    bitmap: &Bitmap,
    refinement: bool,
    work: &mut DecodeWork,
    limits: &Limits,
) -> Result<(), DecodeError> {
    check_combination_operator(
        page.combination_override,
        region.operator,
        page.default_operator,
        refinement,
    )?;
    note_page_write(page, region)?;
    let composition_work = u64::from(bitmap.width())
        .checked_mul(u64::from(bitmap.height()))
        .ok_or(MalformedError::IntegerOverflow)?;
    work.charge(composition_work, limits)?;
    page.bitmap
        .compose(bitmap, region.x, region.y, region.operator)?;
    page.pristine = false;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next_open) =>
            if decode_spec::region_intersects_page(
                region_width,
                region_height,
                region_x,
                region_y,
                page_width,
                page_height,
            ) {
                next_open.is_some()
                    && match last_stripe_end {
                        Some(sealed) => region_y > sealed,
                        None => true,
                    }
                    && match open_stripe_max_row {
                        Some(previous) => next_open.unwrap() >= previous,
                        None => true,
                    }
            } else {
                next_open == open_stripe_max_row
            },
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_arguments)]
pub(super) fn checked_page_write_state(
    last_stripe_end: Option<u32>,
    open_stripe_max_row: Option<u32>,
    region_width: u32,
    region_height: u32,
    region_x: u32,
    region_y: u32,
    page_width: u32,
    page_height: u32,
) -> Result<Option<u32>, DecodeError> {
    if region_width == 0 || region_height == 0 || region_x >= page_width || region_y >= page_height
    {
        return Ok(open_stripe_max_row);
    }
    if let Some(sealed) = last_stripe_end {
        if region_y <= sealed {
            return Err(MalformedError::InvalidPageState("region modifies a sealed stripe").into());
        }
    }
    let bottom = region_y
        .checked_add(region_height)
        .ok_or(MalformedError::IntegerOverflow)?
        .min(page_height)
        .checked_sub(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    let next = match open_stripe_max_row {
        Some(current) => current.max(bottom),
        None => bottom,
    };
    Ok(Some(next))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(&old(page).bitmap),
    ensures
        crate::spec::bitmap::storage_valid(&final(page).bitmap),
        final(page).bitmap.width == old(page).bitmap.width,
        final(page).bitmap.height == old(page).bitmap.height,
        final(page).bitmap.stride == old(page).bitmap.stride,
        final(page).bitmap.data@ == old(page).bitmap.data@,
))]
pub(super) fn note_page_write(
    page: &mut RuntimePage,
    region: &crate::segment_data::RegionInfo,
) -> Result<(), DecodeError> {
    page.open_stripe_max_row = checked_page_write_state(
        page.last_stripe_end,
        page.open_stripe_max_row,
        region.width,
        region.height,
        region.x,
        region.y,
        page.bitmap.width(),
        page.bitmap.height(),
    )?;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(next_open) =>
            next_open.is_none()
                && match open_stripe_max_row {
                    Some(maximum) => maximum <= row,
                    None => true,
                },
        Err(_) =>
            match open_stripe_max_row {
                Some(maximum) => maximum > row,
                None => false,
            },
    },
))]
pub(super) fn checked_seal_stripe_state(
    open_stripe_max_row: Option<u32>,
    row: u32,
) -> Result<Option<u32>, DecodeError> {
    if let Some(maximum) = open_stripe_max_row {
        if maximum > row {
            return Err(MalformedError::InvalidPageState(
                "region preceding stripe extends below its end row",
            )
            .into());
        }
    }
    Ok(None)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::runtime_pages_valid(old(pages)),
    ensures decode_spec::runtime_pages_valid(final(pages)),
))]
pub(super) fn seal_stripe(
    pages: &mut RuntimePages,
    page_number: u32,
    row: u32,
) -> Result<(), DecodeError> {
    let index = runtime_page_index(pages, page_number)
        .ok_or(MalformedError::MissingPageInformation(page_number))?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost original_pages = pages@;
    }
    let result = seal_stripe_on_page(&mut pages[index].1, row);
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(decode_spec::runtime_pages_valid);
        assert forall|entry: int| 0 <= entry < pages@.len()
            implies crate::spec::bitmap::storage_valid(
                #[trigger] &pages@[entry].1.bitmap
            ) by {
            if entry == index {
            } else {
                assert(pages@[entry] == original_pages[entry]);
            }
        }
    }
    result
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(&old(page).bitmap),
    ensures crate::spec::bitmap::storage_valid(&final(page).bitmap),
))]
pub(super) fn seal_stripe_on_page(page: &mut RuntimePage, row: u32) -> Result<(), DecodeError> {
    let next_open = checked_seal_stripe_state(page.open_stripe_max_row, row)?;
    page.last_stripe_end = Some(row);
    page.open_stripe_max_row = next_open;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires decode_spec::runtime_pages_valid(pages),
))]
pub(super) fn require_auxiliary_page(
    segment: &ParsedSegment<'_>,
    pages: &RuntimePages,
) -> Result<(), DecodeError> {
    let page = get_runtime_page(pages, segment.header.page_association).ok_or(
        MalformedError::MissingPageInformation(segment.header.page_association),
    )?;
    require_auxiliary_page_state(page)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures decode_spec::auxiliary_page_result(page.auxiliary_allowed, out),
))]
pub(super) fn require_auxiliary_page_state(page: &RuntimePage) -> Result<(), DecodeError> {
    if !page.auxiliary_allowed {
        return Err(MalformedError::InvalidPageState(
            "intermediate region on page that forbids auxiliary buffers",
        )
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(page),
    ensures match out {
        Ok(snapshot) =>
            crate::spec::bitmap::storage_valid(&snapshot)
            && crate::views::bitmap_width(&snapshot) == region.width
            && crate::views::bitmap_height(&snapshot) == region.height,
        Err(_) => true,
    },
))]
pub(super) fn snapshot_region(
    page: &Bitmap,
    region: &crate::segment_data::RegionInfo,
    limits: &Limits,
) -> Result<Bitmap, DecodeError> {
    let mut snapshot = Bitmap::blank(region.width, region.height, limits)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&snapshot);
        assert(snapshot.width == region.width);
        assert(snapshot.height == region.height);
    }
    let width = region.width.min(page.width().saturating_sub(region.x));
    let height = region.height.min(page.height().saturating_sub(region.y));
    if width != 0 && height != 0 {
        snapshot.compose_region(
            page,
            region.x,
            region.y,
            0,
            0,
            width,
            height,
            CombinationOperator::Replace,
        )?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&snapshot);
        reveal(crate::spec::bitmap::storage_valid);
        assert(crate::spec::bitmap::storage_valid(&snapshot));
        assert(crate::views::bitmap_width(&snapshot) == region.width);
        assert(crate::views::bitmap_height(&snapshot) == region.height);
    }
    Ok(snapshot)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(page) => crate::views::page_number(page) == number,
        Err(_) => true,
    },
))]
pub(super) fn find_page_info(info: &DocumentInfo, number: u32) -> Result<&PageInfo, DecodeError> {
    let pages = info.pages();
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= pages@.len(),
        decreases pages@.len() - index,
    ))]
    while index < pages.len() {
        if pages[index].number() == number {
            return Ok(&pages[index]);
        }
        index += 1;
    }
    Err(MalformedError::MissingPageInformation(number).into())
}
