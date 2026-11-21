//! Project compatibility policy, not normative conformance of malformed input.
//! T.88 (08/2018): 7.2, 7.4.1.5, 7.4.9–11, 8.2, D.1 and D.4.3.

use vstd::prelude::*;

verus! {

pub(crate) open spec fn eop_recovery_allowed(
    associated: u32, open: Seq<u32>, closed: Seq<u32>,
) -> bool {
    open.len() == 1 && open[0] > 0
        && (associated == 0 || (
            0 < associated < open[0]
            && exists|index: int| 0 <= index < closed.len() && closed[index] == associated
        ))
}

/// Every repair has one observed open owner; nonzero sources require history.
pub(crate) open spec fn eop_ownership(
    associated: u32, open: Seq<u32>, closed: Seq<u32>,
    owner: u32, kind: crate::document::standalone::RecoveryKind,
) -> bool {
    &&& eop_recovery_allowed(associated, open, closed)
    &&& owner == open[0]
    &&& forall|index: int| 0 <= index < open.len() ==> open[index] == owner
    &&& match kind {
        crate::document::standalone::RecoveryKind::ZeroAssociatedEndOfPage => associated == 0,
        crate::document::standalone::RecoveryKind::ClosedPageEndOfPage => associated > 0,
        _ => false,
    }
}

/// Prefix state: page encountered, next page boundary encountered, valid.
pub(crate) open spec fn boundary_state(
    segments: Seq<crate::document::ParsedSegment>,
    page: u32,
    count: nat,
) -> (bool, bool, bool)
    recommends count <= segments.len(),
    decreases count,
{
    if count == 0 {
        (false, false, true)
    } else {
        let prior = boundary_state(segments, page, (count - 1) as nat);
        let header = segments[count - 1].header;
        let closed = prior.1 || (prior.0
            && header.segment_type == crate::segment_header::SegmentType::PageInformation);
        let own = header.page_association == page;
        (prior.0 || own, closed, prior.2 && !(closed && own))
    }
}

/// No source page segment is skipped across the proposed next-page boundary.
pub(crate) open spec fn no_page_revisit(
    segments: Seq<crate::document::ParsedSegment>, page: u32, count: nat,
) -> bool {
    forall|first: int, boundary: int, later: int|
        0 <= first < boundary <= later < count ==>
        !(segments[first].header.page_association == page
            && segments[boundary].header.segment_type
                == crate::segment_header::SegmentType::PageInformation
            && segments[later].header.page_association == page)
}

pub(crate) proof fn boundary_witnesses(
    segments: Seq<crate::document::ParsedSegment>, page: u32, count: nat,
)
    requires count <= segments.len(),
    ensures
        forall|first: int| 0 <= first < count
            && segments[first].header.page_association == page
            ==> boundary_state(segments, page, count).0,
        forall|first: int, boundary: int| 0 <= first < boundary < count
            && segments[first].header.page_association == page
            && segments[boundary].header.segment_type
                == crate::segment_header::SegmentType::PageInformation
            ==> boundary_state(segments, page, count).1,
        boundary_state(segments, page, count).2 ==> no_page_revisit(segments, page, count),
    decreases count,
{
    if count > 0 {
        boundary_witnesses(segments, page, (count - 1) as nat);
        reveal_with_fuel(boundary_state, 1);
        assert forall|first: int, boundary: int, later: int|
            0 <= first < boundary <= later < count
            && boundary_state(segments, page, count).2
            implies !(segments[first].header.page_association == page
                && segments[boundary].header.segment_type
                    == crate::segment_header::SegmentType::PageInformation
                && segments[later].header.page_association == page) by {
            if later < count - 1 {
                assert(no_page_revisit(segments, page, (count - 1) as nat));
            } else if boundary < count - 1 {
                assert(boundary_state(segments, page, (count - 1) as nat).1
                    || segments[first].header.page_association != page
                    || segments[boundary].header.segment_type
                        != crate::segment_header::SegmentType::PageInformation);
            } else {
                assert(boundary_state(segments, page, (count - 1) as nat).0
                    || segments[first].header.page_association != page);
            }
        }
    }
}

/// The colour flag and all composition bits survive a bit-4 waiver unchanged.
pub(crate) open spec fn reserved_mask_result(flag: u8, normalized: u8) -> bool {
    &&& flag & 0xf0u8 == 0x10u8
    &&& normalized == flag & 0xefu8
    &&& normalized & 0x0fu8 == flag & 0x0fu8
    &&& normalized & 0xf0u8 == 0u8
}

pub(crate) open spec fn page_byte(page: u32, width: usize, index: int) -> u8 {
    if width == 1 { page as u8 }
    else if index == 0 { (page >> 24u32) as u8 }
    else if index == 1 { ((page >> 16u32) & 255u32) as u8 }
    else if index == 2 { ((page >> 8u32) & 255u32) as u8 }
    else { (page & 255u32) as u8 }
}

/// The ownership repair changes no bytes outside its existing association field.
pub(crate) open spec fn page_patch(
    before: Seq<u8>, after: Seq<u8>, start: int, width: usize, page: u32,
) -> bool {
    &&& width == 1 || width == 4
    &&& 0 <= start
    &&& width == 1 ==> page <= 255
    &&& start + width <= before.len()
    &&& after.len() == before.len()
    &&& forall|index: int| 0 <= index < after.len() ==>
        after[index] == if start <= index < start + width {
            page_byte(page, width, index - start)
        } else { before[index] }
}

}
