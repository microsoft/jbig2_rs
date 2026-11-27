// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use super::global_types;
use vstd::prelude::*;

verus! {

// Model navigation: spec::page_association_valid [project-result-model]
// Purpose: Page-zero and nonzero page-association rules by segment type.
// Audit: document-page-association-valid
// REQUIREMENT CONTEXT: T88-GLOBAL-PAGE-ASSOCIATION | ITU-T T.88 (08/2018) | 7.2.6; 7.3.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn page_association_valid(
    segment_type: crate::segment_header::SegmentType,
    page_association: u32,
) -> bool {
    let must_have_page = matches!(
        segment_type,
        crate::segment_header::SegmentType::IntermediateTextRegion
            | crate::segment_header::SegmentType::ImmediateTextRegion
            | crate::segment_header::SegmentType::ImmediateLosslessTextRegion
            | crate::segment_header::SegmentType::IntermediateHalftoneRegion
            | crate::segment_header::SegmentType::ImmediateHalftoneRegion
            | crate::segment_header::SegmentType::ImmediateLosslessHalftoneRegion
            | crate::segment_header::SegmentType::IntermediateGenericRegion
            | crate::segment_header::SegmentType::ImmediateGenericRegion
            | crate::segment_header::SegmentType::ImmediateLosslessGenericRegion
            | crate::segment_header::SegmentType::IntermediateGenericRefinementRegion
            | crate::segment_header::SegmentType::ImmediateGenericRefinementRegion
            | crate::segment_header::SegmentType::ImmediateLosslessGenericRefinementRegion
            | crate::segment_header::SegmentType::PageInformation
            | crate::segment_header::SegmentType::EndOfPage
            | crate::segment_header::SegmentType::EndOfStripe
    );
    !(must_have_page && page_association == 0)
        && !(matches!(segment_type, crate::segment_header::SegmentType::EndOfFile)
            && page_association != 0)
}

#[verifier::opaque]
// Model navigation: spec::reference_link_in_scope [project-result-model]
// Purpose: Page-zero, same-page, cross-page and global reference-scope rules.
// Audit: document-reference-link-in-scope
// REQUIREMENT CONTEXT: T88-GLOBAL-REFERENCE-SCOPE | ITU-T T.88 (08/2018) | 7.3.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn reference_link_in_scope(
    segments: Seq<crate::document::ParsedSegment<'_>>,
    indexes: Seq<(u32, usize)>,
    globals: Option<&crate::document::GlobalSegments>,
    segment_index: int,
    referred: u32,
) -> bool {
    if exists|e: int| 0 <= e < indexes.len() && indexes[e].0 == referred {
        exists|e: int| 0 <= e < indexes.len()
            && indexes[e].0 == referred
            && (forall|prev: int| 0 <= prev < e ==> indexes[prev].0 != referred)
            && indexes[e].1 < segments.len()
            && (segments[indexes[e].1 as int].header.page_association == 0
                || segments[indexes[e].1 as int].header.page_association
                    == segments[segment_index].header.page_association)
    } else {
        match globals {
            Some(state) => exists|g: int| 0 <= g < global_types(state).len()
                && global_types(state)[g].0 == referred,
            None => false,
        }
    }
}

// Model navigation: spec::palette_segment_rejected [project-policy]
// Purpose: Explicit bi-level profile rejection of colour palette segments.
// Audit: document-palette-segment-rejected
// REQUIREMENT CONTEXT: T88-PROFILE-PALETTE-REJECTION | ITU-T T.88 (08/2018) | 7.3; 7.4.16 (syntax context; rejection is project policy)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn palette_segment_rejected(
    segment_type: crate::segment_header::SegmentType,
    result: Result<crate::document::ParsedData, crate::DecodeError>,
) -> bool {
    segment_type == crate::segment_header::SegmentType::ColourPalette ==>
        result == Err(crate::DecodeError::Unsupported(crate::UnsupportedFeature::Colour))
}

}
