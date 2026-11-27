// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use super::target_adaptive_valid;
use vstd::prelude::*;

verus! {

// Model navigation: spec::signed_offset_value [proof-support]
// Purpose: Signed two's-complement byte decoding for adaptive template offsets.
// Audit: generic-signed-offset-value
// REQUIREMENT CONTEXT: T88-REFINEMENT-ADAPTIVE-PIXELS | ITU-T T.88 (08/2018) | 6.3.5.3-6.3.5.4; 7.4.7.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn signed_offset_value(value: u8) -> i32 {
    if value <= 127 {
        value as i32
    } else {
        (value as int - 256) as i32
    }
}

// Model navigation: spec::refinement_adaptive_fields [project-result-model]
// Purpose: Parsed refinement adaptive-pixel coordinates, omitted Template 1 fields and target bounds.
// Audit: refinement-adaptive-fields
// REQUIREMENT CONTEXT: T88-REFINEMENT-ADAPTIVE-PIXELS | ITU-T T.88 (08/2018) | 6.3.5.3-6.3.5.4; 7.4.7.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_adaptive_fields(
    payload: Seq<u8>,
    header: &crate::generic_region::RefinementRegionHeader,
    header_len: usize,
) -> bool {
    match header.template {
        crate::generic_region::RefinementTemplate::Template0 =>
            header_len == 22
            && payload.len() >= 22
            && header.adaptive@[0] == crate::generic_region::TemplateOffset {
                x: signed_offset_value(payload[18]),
                y: signed_offset_value(payload[19]),
            }
            && header.adaptive@[1] == crate::generic_region::TemplateOffset {
                x: signed_offset_value(payload[20]),
                y: signed_offset_value(payload[21]),
            }
            && target_adaptive_valid(header.adaptive@[0]),
        crate::generic_region::RefinementTemplate::Template1 =>
            header_len == 18
            && payload.len() >= 18
            && header.adaptive@[0] == crate::generic_region::TemplateOffset { x: 0, y: 0 }
            && header.adaptive@[1] == crate::generic_region::TemplateOffset { x: 0, y: 0 },
    }
}

// Model navigation: spec::refinement_adaptive_selected [project-result-model]
// Purpose: Adaptive-slot coordinate selection for refinement context gathering.
// Audit: refinement-adaptive-selected
// REQUIREMENT CONTEXT: T88-REFINEMENT-ADAPTIVE-PIXELS | ITU-T T.88 (08/2018) | 6.3.5.3-6.3.5.4; 7.4.7.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_adaptive_selected(
    adaptive: Seq<crate::generic_region::TemplateOffset>,
    number: u8,
    nominal: crate::generic_region::TemplateOffset,
) -> crate::generic_region::TemplateOffset {
    if number == 1 {
        adaptive[0]
    } else if number == 2 {
        adaptive[1]
    } else {
        nominal
    }
}

// Model navigation: spec::bitmap_pixel_at [proof-support]
// Purpose: Signed-coordinate bitmap sampling with white out-of-bounds default.
// Audit: refinement-bitmap-pixel-at
// REQUIREMENT CONTEXT: T88-REFINEMENT-TYPICAL-PREDICTION | ITU-T T.88 (08/2018) | 6.3.5.5; 6.3.5.6 step 3d
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmap_pixel_at(
    bitmap: &crate::Bitmap,
    x: int,
    y: int,
) -> bool {
    if 0 <= x <= u32::MAX as int && 0 <= y <= u32::MAX as int {
        match crate::spec::bitmap::pixel_result(bitmap, x as u32, y as u32) {
            Some(value) => value,
            None => false,
        }
    } else {
        false
    }
}

// Model navigation: spec::refinement_reference_pixel [proof-support]
// Purpose: Aligned reference-bitmap pixel sampling for refinement prediction.
// Audit: refinement-reference-pixel
// REQUIREMENT CONTEXT: T88-REFINEMENT-TYPICAL-PREDICTION | ITU-T T.88 (08/2018) | 6.3.5.5; 6.3.5.6 step 3d
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_reference_pixel(
    reference: &crate::Bitmap,
    x: u32,
    y: u32,
    dx: int,
    dy: int,
    parameters: crate::generic_region::RefinementParameters,
) -> bool {
    bitmap_pixel_at(
        reference,
        x as int - parameters.reference_dx as int + dx,
        y as int - parameters.reference_dy as int + dy,
    )
}

// Model navigation: spec::refinement_predicted_pixel [project-result-model]
// Purpose: 3x3 uniform-neighbourhood typical-prediction detector on aligned reference bitmap.
// Audit: refinement-predicted-pixel
// REQUIREMENT CONTEXT: T88-REFINEMENT-TYPICAL-PREDICTION | ITU-T T.88 (08/2018) | 6.3.5.5; 6.3.5.6 step 3d
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn refinement_predicted_pixel(
    reference: &crate::Bitmap,
    x: u32,
    y: u32,
    parameters: crate::generic_region::RefinementParameters,
) -> Option<bool> {
    let common = refinement_reference_pixel(reference, x, y, -1, -1, parameters);
    if refinement_reference_pixel(reference, x, y, 0, -1, parameters) == common
        && refinement_reference_pixel(reference, x, y, 1, -1, parameters) == common
        && refinement_reference_pixel(reference, x, y, -1, 0, parameters) == common
        && refinement_reference_pixel(reference, x, y, 0, 0, parameters) == common
        && refinement_reference_pixel(reference, x, y, 1, 0, parameters) == common
        && refinement_reference_pixel(reference, x, y, -1, 1, parameters) == common
        && refinement_reference_pixel(reference, x, y, 0, 1, parameters) == common
        && refinement_reference_pixel(reference, x, y, 1, 1, parameters) == common
    {
        Some(common)
    } else {
        None
    }
}

}
