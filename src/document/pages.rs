use super::{
    allocation_size, enforce_page_limit, keyed_get_mut, keyed_index, keyed_insert,
    validate_references, DecodeError, DocumentInfo, GlobalSegments, Limits, MalformedError,
    Organization, PageInfo, PageInformation, PageState, PageStates, ParsedData, ParsedSegment,
    SegmentHeader, SegmentType,
};

#[cfg(verus_keep_ghost)]
use super::document_spec;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(info) =>
            document_spec::document_organization(&info) == organization
            && document_spec::document_declared_page_count(&info) == declared_page_count
            && document_spec::document_segment_count(&info) as nat == segments@.len()
            && document_spec::document_pages(&info).len()
                == document_spec::page_information_count(segments@, segments@.len())
            && match declared_page_count {
                Some(count) => document_spec::document_pages(&info).len() == count as nat,
                None => true,
            },
        Err(_) => true,
    },
))]
pub(super) fn build_document_info(
    organization: Organization,
    declared_page_count: Option<u32>,
    segments: &[ParsedSegment<'_>],
    limits: &Limits,
    globals: Option<&GlobalSegments>,
    embedded: bool,
) -> Result<DocumentInfo, DecodeError> {
    validate_references(segments, globals, limits)?;
    validate_profiles(segments)?;
    let (page_states, page_order) = collect_page_states(segments, limits)?;
    let Ok(actual_page_count) = u32::try_from(page_states.len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    if let Some(declared) = declared_page_count {
        if declared != actual_page_count {
            return Err(MalformedError::PageCountMismatch {
                declared,
                actual: actual_page_count,
            }
            .into());
        }
    }
    let pages = finalize_pages(page_states, &page_order, segments, limits, embedded)?;

    let Ok(segment_count) = u32::try_from(segments.len()) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let info = DocumentInfo {
        organization,
        declared_page_count,
        segment_count,
        pages,
    };
    #[cfg(verus_keep_ghost)]
    production_document_fields!(&info);
    Ok(info)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) =>
            result.0@.len() == result.1@.len()
            && result.0@.len()
                == document_spec::page_information_count(segments@, segments@.len()),
        Err(_) => true,
    },
))]
pub(super) fn collect_page_states(
    segments: &[ParsedSegment<'_>],
    limits: &Limits,
) -> Result<(PageStates, Vec<u32>), DecodeError> {
    let mut page_states = Vec::new();
    let page_state_allocation =
        allocation_size::<PageState>(segments.len().min(limits.max_pages as usize))?;
    if page_states
        .try_reserve(segments.len().min(limits.max_pages as usize))
        .is_err()
    {
        return Err(DecodeError::Allocation {
            requested_bytes: page_state_allocation,
        });
    }
    let mut page_order = Vec::new();
    let page_order_capacity = segments.len().min(limits.max_pages as usize);
    let page_order_allocation = allocation_size::<u32>(page_order_capacity)?;
    if page_order.try_reserve(page_order_capacity).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: page_order_allocation,
        });
    }

    collect_page_states_into(segments, limits, &mut page_states, &mut page_order)?;
    Ok((page_states, page_order))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        old(page_states)@.len() == 0,
        old(page_order)@.len() == 0,
    ensures out.is_ok() ==>
        final(page_states)@.len() == final(page_order)@.len()
        && final(page_states)@.len()
            == document_spec::page_information_count(segments@, segments@.len()),
))]
pub(super) fn collect_page_states_into(
    segments: &[ParsedSegment<'_>],
    limits: &Limits,
    page_states: &mut PageStates,
    page_order: &mut Vec<u32>,
) -> Result<(), DecodeError> {
    let mut segment_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            segment_index <= segments@.len(),
            page_states@.len() == page_order@.len(),
            page_states@.len()
                == document_spec::page_information_count(segments@, segment_index as nat),
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        #[cfg(verus_keep_ghost)]
        production_page_information_count_step!(segments, segment_index);
        let segment = &segments[segment_index];
        validate_page_association(&segment.header)?;
        let page = segment.header.page_association;
        if page == 0 {
            segment_index += 1;
            continue;
        }

        if keyed_index(page_states, page).is_none() {
            if !matches!(segment.header.segment_type, SegmentType::PageInformation) {
                return Err(MalformedError::MissingPageInformation(page).into());
            }
            enforce_page_limit(page_states.len(), limits)?;
            let ParsedData::Page(info) = segment.data else {
                return Err(MalformedError::MissingPageInformation(page).into());
            };
            if !keyed_insert(page_states, page, PageState::new(info)) {
                return Err(MalformedError::DuplicatePageBoundary {
                    page,
                    segment_type: 48,
                }
                .into());
            }
            page_order.push(page);
            segment_index += 1;
            continue;
        }

        let state =
            keyed_get_mut(page_states, page).ok_or(MalformedError::MissingPageInformation(page))?;
        if state.ended {
            return Err(MalformedError::SegmentAfterEndOfPage {
                segment: segment.header.number,
                page,
            }
            .into());
        }
        match (&segment.header.segment_type, &segment.data) {
            (SegmentType::PageInformation, _) => {
                return Err(MalformedError::DuplicatePageBoundary {
                    page,
                    segment_type: 48,
                }
                .into());
            }
            (SegmentType::EndOfStripe, ParsedData::EndOfStripe(row)) => {
                state.add_stripe(*row)?;
            }
            (SegmentType::EndOfPage, _) => state.finish()?,
            _ => {}
        }
        segment_index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(pages) => pages@.len() == page_order@.len(),
        Err(_) => true,
    },
))]
pub(super) fn finalize_pages(
    mut page_states: PageStates,
    page_order: &[u32],
    segments: &[ParsedSegment<'_>],
    limits: &Limits,
    embedded: bool,
) -> Result<Vec<PageInfo>, DecodeError> {
    let mut pages: Vec<PageInfo> = Vec::new();
    let page_allocation = allocation_size::<PageInfo>(page_states.len())?;
    if pages.try_reserve(page_states.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: page_allocation,
        });
    }
    let mut order_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            order_index <= page_order@.len(),
            pages@.len() == order_index,
        decreases page_order@.len() - order_index,
    ))]
    while order_index < page_order.len() {
        let number = page_order[order_index];
        let state_index = keyed_index(&page_states, number)
            .ok_or(MalformedError::MissingPageInformation(number))?;
        let mut state = page_states.remove(state_index).1;
        if !state.ended {
            // Only the final page can use the enclosing embedded stream's end.
            if !embedded || order_index + 1 != page_order.len() {
                return Err(MalformedError::MissingEndOfPage(number).into());
            }
            if state.info.height.is_none() {
                validate_implicit_stripes(segments, number, state.info.width, limits)?;
            }
            state.finish()?;
        }
        let page = state.into_public(number)?;
        let mut position = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant position <= pages@.len(),
            decreases pages@.len() - position,
        ))]
        while position < pages.len() && pages[position].number < number {
            position += 1;
        }
        pages.insert(position, page);
        order_index += 1;
    }
    validate_public_page_numbers(&pages)?;
    Ok(pages)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn validate_implicit_stripes(
    segments: &[ParsedSegment<'_>],
    page: u32,
    width: u32,
    limits: &Limits,
) -> Result<(), DecodeError> {
    let mut region_since_stripe = false;
    let mut bottom = 0_u64;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= segments@.len(),
        decreases segments@.len() - index,
    ))]
    while index < segments.len() {
        let segment = &segments[index];
        if segment.header.page_association == page {
            match segment.header.segment_type {
                SegmentType::EndOfStripe => {
                    if let ParsedData::EndOfStripe(row) = segment.data {
                        if bottom > u64::from(row) + 1 {
                            return Err(MalformedError::InvalidPageState(
                                "region preceding stripe extends below its end row",
                            )
                            .into());
                        }
                        region_since_stripe = false;
                        bottom = 0;
                    }
                }
                SegmentType::ImmediateTextRegion
                | SegmentType::ImmediateLosslessTextRegion
                | SegmentType::ImmediateHalftoneRegion
                | SegmentType::ImmediateLosslessHalftoneRegion
                | SegmentType::ImmediateGenericRegion
                | SegmentType::ImmediateLosslessGenericRegion
                | SegmentType::ImmediateGenericRefinementRegion
                | SegmentType::ImmediateLosslessGenericRefinementRegion => {
                    region_since_stripe = true;
                    let region = crate::segment_data::parse_region_info(
                        &mut crate::reader::ByteReader::new(segment.payload),
                        limits,
                    )?;
                    if region.x < width && region.width != 0 && region.height != 0 {
                        bottom = bottom.max(u64::from(region.y) + u64::from(region.height));
                    }
                }
                SegmentType::IntermediateTextRegion
                | SegmentType::IntermediateHalftoneRegion
                | SegmentType::IntermediateGenericRegion
                | SegmentType::IntermediateGenericRefinementRegion => {
                    region_since_stripe = true;
                }
                _ => {}
            }
        }
        index += 1;
    }
    if region_since_stripe {
        return Err(MalformedError::InvalidPageState(
            "unknown-height page has an unfinished stripe",
        )
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> forall|i: int| 0 <= i < pages@.len() ==>
        pages@[i].number as int == i + 1,
))]
pub(super) fn validate_public_page_numbers(pages: &[PageInfo]) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= pages@.len(),
            forall|i: int| 0 <= i < index ==> pages@[i].number as int == i + 1,
        decreases pages@.len() - index,
    ))]
    while index < pages.len() {
        let Ok(index_u32) = u32::try_from(index) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        let Some(expected) = index_u32.checked_add(1) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        if pages[index].number != expected {
            return Err(MalformedError::MissingPageInformation(expected).into());
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> forall|i: int| 0 <= i < segments@.len()
        && segments@[i].header.segment_type == SegmentType::Profiles ==> (
            (i == 0 ==> segments@[i].header.page_association == 0)
            && (i > 0 ==> segments@[i].header.page_association != 0
                && segments@[0].header.segment_type == SegmentType::Profiles)
        ),
))]
pub(super) fn validate_profiles(segments: &[ParsedSegment<'_>]) -> Result<(), DecodeError> {
    let mut first_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            first_index <= segments@.len(),
            forall|i: int| 0 <= i < first_index ==>
                segments@[i].header.segment_type != SegmentType::Profiles,
        decreases segments@.len() - first_index,
    ))]
    while first_index < segments.len()
        && !matches!(
            segments[first_index].header.segment_type,
            SegmentType::Profiles
        )
    {
        first_index += 1;
    }
    if first_index == segments.len() {
        return Ok(());
    }
    let first_segment = &segments[first_index];
    if first_index != 0 || first_segment.header.page_association != 0 {
        return Err(MalformedError::InvalidProfilesSegment(first_segment.header.number).into());
    }
    let ParsedData::Profiles(global_profiles) = &first_segment.data else {
        return Err(MalformedError::InvalidProfilesSegment(first_segment.header.number).into());
    };
    let mut page_profiles = Vec::new();
    let profile_allocation = allocation_size::<(u32, u32)>(segments.len())?;
    if page_profiles.try_reserve(segments.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: profile_allocation,
        });
    }
    validate_page_profiles(
        segments,
        first_index + 1,
        global_profiles,
        &mut page_profiles,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires start <= segments@.len(),
    ensures out.is_ok() ==> forall|i: int| start <= i < segments@.len()
        && segments@[i].header.segment_type == SegmentType::Profiles ==>
            segments@[i].header.page_association != 0,
))]
pub(super) fn validate_page_profiles(
    segments: &[ParsedSegment<'_>],
    start: usize,
    global_profiles: &[u32],
    page_profiles: &mut Vec<(u32, u32)>,
) -> Result<(), DecodeError> {
    let mut segment_index = start;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            start <= segment_index <= segments@.len(),
            forall|i: int| start <= i < segment_index
                && segments@[i].header.segment_type == SegmentType::Profiles ==>
                    segments@[i].header.page_association != 0,
        decreases segments@.len() - segment_index,
    ))]
    while segment_index < segments.len() {
        let segment = &segments[segment_index];
        if !matches!(segment.header.segment_type, SegmentType::Profiles) {
            segment_index += 1;
            continue;
        }
        let page = segment.header.page_association;
        if page == 0 || !keyed_insert(page_profiles, page, segment.header.number) {
            return Err(MalformedError::InvalidProfilesSegment(segment.header.number).into());
        }
        let ParsedData::Profiles(profiles) = &segment.data else {
            return Err(MalformedError::InvalidProfilesSegment(segment.header.number).into());
        };
        let mut profile_index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant profile_index <= global_profiles@.len(),
            decreases global_profiles@.len() - profile_index,
        ))]
        while profile_index < global_profiles.len() {
            if !contains_profile(profiles, global_profiles[profile_index]) {
                return Err(MalformedError::InvalidProfilesSegment(segment.header.number).into());
            }
            profile_index += 1;
        }
        segment_index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == exists|i: int| 0 <= i < profiles@.len() && profiles@[i] == wanted,
))]
pub(super) fn contains_profile(profiles: &[u32], wanted: u32) -> bool {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= profiles@.len(),
            forall|i: int| 0 <= i < index ==> profiles@[i] != wanted,
        decreases profiles@.len() - index,
    ))]
    while index < profiles.len() {
        if profiles[index] == wanted {
            return true;
        }
        index += 1;
    }
    false
}

impl PageState {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.info == info, out.last_stripe == None, !out.ended,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) const fn new(info: PageInformation) -> Self {
        Self {
            info,
            last_stripe: None,
            ended: false,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out == document_spec::stripe_result(old(self).info, old(self).last_stripe, row),
            final(self).info == old(self).info,
            final(self).ended == old(self).ended,
            final(self).last_stripe == if out is Ok { Some(row) } else { old(self).last_stripe },
    ))]
    pub(super) fn add_stripe(&mut self, row: u32) -> Result<(), DecodeError> {
        if !self.info.striped {
            return Err(MalformedError::InvalidStripeEndRow(row).into());
        }
        if let Some(previous) = self.last_stripe {
            if row <= previous || row - previous > u32::from(self.info.max_stripe_size) {
                return Err(MalformedError::InvalidStripeEndRow(row).into());
            }
        } else if row > u32::from(self.info.max_stripe_size) {
            return Err(MalformedError::InvalidStripeEndRow(row).into());
        }
        if let Some(height) = self.info.height {
            if row >= height {
                return Err(MalformedError::InvalidStripeEndRow(row).into());
            }
        }
        self.last_stripe = Some(row);
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out == document_spec::page_finish_result(
                old(self).info.height, old(self).last_stripe, old(self).ended,
            ),
            final(self).info == old(self).info,
            final(self).last_stripe == old(self).last_stripe,
            final(self).ended == (old(self).ended || out is Ok),
    ))]
    pub(super) fn finish(&mut self) -> Result<(), DecodeError> {
        if self.ended {
            return Err(MalformedError::DuplicatePageBoundary {
                page: 0,
                segment_type: 49,
            }
            .into());
        }
        if self.info.height.is_none() && self.last_stripe.is_none() {
            return Err(MalformedError::InvalidStripeEndRow(0).into());
        }
        self.ended = true;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            match out {
                Ok(page) => document_spec::public_page_fields(self.info, number, page),
                Err(_) => true,
            },
            match out {
                Ok(page) => document_spec::public_page_height(self.info.height, self.last_stripe)
                    == Ok(page.height),
                Err(error) => document_spec::public_page_height(self.info.height, self.last_stripe)
                    == Err(error),
            },
    ))]
    pub(super) fn into_public(self, number: u32) -> Result<PageInfo, DecodeError> {
        let height = match (self.info.height, self.last_stripe) {
            (Some(height), _) => height,
            (None, Some(last)) => last.checked_add(1).ok_or(MalformedError::IntegerOverflow)?,
            (None, None) => return Err(MalformedError::InvalidStripeEndRow(0).into()),
        };
        Ok(PageInfo {
            number,
            width: self.info.width,
            height,
            x_resolution: self.info.x_resolution,
            y_resolution: self.info.y_resolution,
            striped: self.info.striped,
        })
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.is_ok() == document_spec::page_association_valid(header.segment_type, header.page_association),
        !document_spec::page_association_valid(header.segment_type, header.page_association) ==>
            out == Err(DecodeError::Malformed(MalformedError::InvalidPageAssociation {
                segment: header.number,
                page: header.page_association,
            })),
))]
pub(super) fn validate_page_association(header: &SegmentHeader) -> Result<(), DecodeError> {
    let must_have_page = matches!(
        header.segment_type,
        SegmentType::IntermediateTextRegion
            | SegmentType::ImmediateTextRegion
            | SegmentType::ImmediateLosslessTextRegion
            | SegmentType::IntermediateHalftoneRegion
            | SegmentType::ImmediateHalftoneRegion
            | SegmentType::ImmediateLosslessHalftoneRegion
            | SegmentType::IntermediateGenericRegion
            | SegmentType::ImmediateGenericRegion
            | SegmentType::ImmediateLosslessGenericRegion
            | SegmentType::IntermediateGenericRefinementRegion
            | SegmentType::ImmediateGenericRefinementRegion
            | SegmentType::ImmediateLosslessGenericRefinementRegion
            | SegmentType::PageInformation
            | SegmentType::EndOfPage
            | SegmentType::EndOfStripe
    );
    if (must_have_page && header.page_association == 0)
        || (matches!(header.segment_type, SegmentType::EndOfFile) && header.page_association != 0)
    {
        return Err(MalformedError::InvalidPageAssociation {
            segment: header.number,
            page: header.page_association,
        }
        .into());
    }
    Ok(())
}
