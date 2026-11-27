// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::work_limit [proof-support]
// Purpose: Powers, row-major ordering and private work-budget projections.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn work_limit(work: &super::WorkBudget) -> u64 {
    work.limit
}

// Model navigation: spec::work_used [proof-support]
// Purpose: Powers, row-major ordering and private work-budget projections.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn work_used(work: &super::WorkBudget) -> u64 {
    work.used
}

// Model navigation: spec::work_valid [representation-invariant]
// Purpose: Pattern storage, gray-index, work-budget and decoder-session invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn work_valid(work: &super::WorkBudget) -> bool {
    work_used(work) <= work_limit(work)
}

// Model navigation: spec::bitmaps_valid [representation-invariant]
// Purpose: Pattern storage, gray-index, work-budget and decoder-session invariants.
// Audit: pattern-bitmaps-valid
// REQUIREMENT CONTEXT: T88-PATTERN-EXTRACTION | ITU-T T.88 (08/2018) | 6.7.5 steps 3-4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bitmaps_valid(bitmaps: Seq<crate::bitmap::Bitmap>) -> bool {
    forall|index: int| 0 <= index < bitmaps.len()
        ==> crate::spec::bitmap::storage_valid(#[trigger] &bitmaps[index])
}

// Model navigation: spec::decoder_state_valid [representation-invariant]
// Purpose: Pattern storage, gray-index, work-budget and decoder-session invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn decoder_state_valid(
    state: &super::GenericRegionState,
) -> bool {
    match state {
        super::GenericRegionState::ZeroPlanes => true,
        super::GenericRegionState::Arithmetic { session, .. } =>
            crate::generic_region::generic_region_spec::generic_session_valid(session),
        super::GenericRegionState::PatternMmr { .. } => true,
        super::GenericRegionState::GrayMmr { coded, offset, .. } =>
            *offset <= coded@.len(),
    }
}

// Model navigation: spec::pattern_flags_valid [candidate-standard-facing-model]
// Purpose: Pattern flag, template/skip and operator-domain candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pattern_flags_valid(flags: u8) -> bool {
    flags & 0xf8 == 0
        && (flags & 1 == 0 || (flags / 2) % 4 == 0)
}

// Model navigation: spec::halftone_flags_valid [candidate-standard-facing-model]
// Purpose: Pattern flag, template/skip and operator-domain candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn halftone_flags_valid(flags: u8) -> bool {
    flags & 1 == 0 || ((flags / 2) % 4 == 0 && flags & 8 == 0)
}

// Model navigation: spec::operator_code_valid [candidate-standard-facing-model]
// Purpose: Pattern flag, template/skip and operator-domain candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn operator_code_valid(flags: u8) -> bool {
    ((flags >> 4) & 7) <= 4
}

// Model navigation: spec::operator_for_code [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn operator_for_code(
    code: u8,
) -> Option<crate::bitmap::CombinationOperator> {
    match code {
        0 => Some(crate::bitmap::CombinationOperator::Or),
        1 => Some(crate::bitmap::CombinationOperator::And),
        2 => Some(crate::bitmap::CombinationOperator::Xor),
        3 => Some(crate::bitmap::CombinationOperator::Xnor),
        4 => Some(crate::bitmap::CombinationOperator::Replace),
        _ => None,
    }
}

// Model navigation: spec::signed_u32 [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn signed_u32(value: u32) -> int {
    if value <= i32::MAX as u32 {
        value as int
    } else {
        value as int - 4_294_967_296
    }
}

// Model navigation: spec::pattern_count [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Audit: pattern-count
// REQUIREMENT CONTEXT: T88-PATTERN-COUNT | ITU-T T.88 (08/2018) | 6.7.3; 7.4.4.1.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pattern_count(
    header: super::PatternDictionaryHeader,
) -> nat {
    header.gray_max as nat + 1
}

// Model navigation: spec::pattern_header_accepted [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Audit: pattern-header-accepted
// REQUIREMENT CONTEXT: T88-PATTERN-HEADER-MMR | ITU-T T.88 (08/2018) | 7.4.4.1.1 bit 0; Figure 42
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pattern_header_accepted(bytes: Seq<u8>, start: int) -> bool {
    0 <= start && start + 7 <= bytes.len()
        && pattern_flags_valid(bytes[start])
        && bytes[start + 1] > 0 && bytes[start + 2] > 0
}

// Model navigation: spec::halftone_header_accepted [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Audit: halftone-header-accepted
// REQUIREMENT CONTEXT: T88-HALFTONE-HEADER-MMR | ITU-T T.88 (08/2018) | 7.4.5.1.1 bit 0; Figure 44
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn halftone_header_accepted(bytes: Seq<u8>, start: int) -> bool {
    0 <= start && start + 21 <= bytes.len()
        && halftone_flags_valid(bytes[start])
        && operator_for_code((bytes[start] >> 4) & 7).is_some()
}

// Model navigation: spec::pattern_header_fields [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Audit: pattern-header-fields
// REQUIREMENT CONTEXT: T88-PATTERN-HEADER-MMR | ITU-T T.88 (08/2018) | 7.4.4.1.1 bit 0; Figure 42
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pattern_header_fields(
    header: super::PatternDictionaryHeader, bytes: Seq<u8>, start: int,
) -> bool {
    0 <= start && start + 7 <= bytes.len()
        && pattern_flags_valid(bytes[start])
        && header.mmr == (bytes[start] & 1 != 0)
        && header.template == (bytes[start] / 2) % 4
        && header.pattern_width == bytes[start + 1]
        && header.pattern_height == bytes[start + 2]
        && header.gray_max == crate::spec::segments::be_u32_at(bytes, start + 3)
}

// Model navigation: spec::halftone_header_fields [project-result-model]
// Purpose: Accepted header bytes, grid fields, signed origins and pattern-count results.
// Audit: halftone-header-fields
// REQUIREMENT CONTEXT: T88-HALFTONE-HEADER-MMR | ITU-T T.88 (08/2018) | 7.4.5.1.1 bit 0; Figure 44
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn halftone_header_fields(
    header: super::HalftoneHeader, bytes: Seq<u8>, start: int,
) -> bool {
    0 <= start && start + 21 <= bytes.len()
        && halftone_flags_valid(bytes[start])
        && header.mmr == (bytes[start] & 1 != 0)
        && header.template == (bytes[start] / 2) % 4
        && header.enable_skip == (bytes[start] & 8 != 0)
        && match operator_for_code((bytes[start] >> 4) & 7) {
            Some(operator) => header.operator == operator,
            None => false,
        }
        && header.default_pixel == (bytes[start] & 0x80 != 0)
        && header.grid_width == crate::spec::segments::be_u32_at(bytes, start + 1)
        && header.grid_height == crate::spec::segments::be_u32_at(bytes, start + 5)
        && header.grid_x as int == signed_u32(crate::spec::segments::be_u32_at(bytes, start + 9))
        && header.grid_y as int == signed_u32(crate::spec::segments::be_u32_at(bytes, start + 13))
        && header.grid_vector_x == ((bytes[start + 17] as u16) << 8) | bytes[start + 18] as u16
        && header.grid_vector_y == ((bytes[start + 19] as u16) << 8) | bytes[start + 20] as u16
}

// Model navigation: spec::pow2 [proof-support]
// Purpose: Powers, row-major ordering and private work-budget projections.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn pow2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * pow2((exponent - 1) as nat)
    }
}

// Model navigation: spec::gray_depth_valid [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn gray_depth_valid(count: u64, depth: u8) -> bool {
    1 <= count
        && count as nat <= pow2(32)
        && if count == 1 {
            depth == 0
        } else {
            1 <= depth <= 32
                && pow2((depth - 1) as nat) < count as nat
                && count as nat <= pow2(depth as nat)
        }
}

// Model navigation: spec::gray_depth_expected [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Audit: halftone-gray-depth
// REQUIREMENT CONTEXT: T88-HALFTONE-GRAY-DEPTH | ITU-T T.88 (08/2018) | 6.6.5 step 3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn gray_depth_expected(count: u64) -> u8 {
    if count <= 1 { 0 }
    else if count <= 2 { 1 }
    else if count <= 4 { 2 }
    else if count <= 8 { 3 }
    else if count <= 16 { 4 }
    else if count <= 32 { 5 }
    else if count <= 64 { 6 }
    else if count <= 128 { 7 }
    else if count <= 256 { 8 }
    else if count <= 512 { 9 }
    else if count <= 1_024 { 10 }
    else if count <= 2_048 { 11 }
    else if count <= 4_096 { 12 }
    else if count <= 8_192 { 13 }
    else if count <= 16_384 { 14 }
    else if count <= 32_768 { 15 }
    else if count <= 65_536 { 16 }
    else if count <= 131_072 { 17 }
    else if count <= 262_144 { 18 }
    else if count <= 524_288 { 19 }
    else if count <= 1_048_576 { 20 }
    else if count <= 2_097_152 { 21 }
    else if count <= 4_194_304 { 22 }
    else if count <= 8_388_608 { 23 }
    else if count <= 16_777_216 { 24 }
    else if count <= 33_554_432 { 25 }
    else if count <= 67_108_864 { 26 }
    else if count <= 134_217_728 { 27 }
    else if count <= 268_435_456 { 28 }
    else if count <= 536_870_912 { 29 }
    else if count <= 1_073_741_824 { 30 }
    else if count <= 2_147_483_648 { 31 }
    else { 32 }
}

// Model navigation: spec::gray_binary_bit [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn gray_binary_bit(coded: bool, previous: Option<bool>) -> bool {
    match previous {
        Some(bit) => coded != bit,
        None => coded,
    }
}

// Model navigation: spec::index_bit_inserted [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn index_bit_inserted(
    old_value: u32,
    plane: u8,
    bit: bool,
    new_value: u32,
) -> bool {
    plane < 32
        && new_value
            == if bit {
                old_value | (1u32 << plane)
            } else {
                old_value
            }
}

// Model navigation: spec::grid_x_fixed [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Audit: halftone-grid-x
// REQUIREMENT CONTEXT: T88-HALFTONE-GRID-COORDINATES | ITU-T T.88 (08/2018) | 6.6.5.1 step 1a i; 6.6.5.2 step 1a i
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn grid_x_fixed(
    header: &super::HalftoneHeader,
    grid_x: u32,
    grid_y: u32,
) -> int {
    header.grid_x as int
        + grid_y as int * header.grid_vector_y as int
        + grid_x as int * header.grid_vector_x as int
}

// Model navigation: spec::grid_y_fixed [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Audit: halftone-grid-y
// REQUIREMENT CONTEXT: T88-HALFTONE-GRID-COORDINATES | ITU-T T.88 (08/2018) | 6.6.5.1 step 1a i; 6.6.5.2 step 1a i
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn grid_y_fixed(
    header: &super::HalftoneHeader,
    grid_x: u32,
    grid_y: u32,
) -> int {
    header.grid_y as int
        + grid_y as int * header.grid_vector_x as int
        - grid_x as int * header.grid_vector_y as int
}

// Model navigation: spec::fixed_pixel [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Audit: halftone-fixed-pixel
// REQUIREMENT CONTEXT: T88-HALFTONE-GRID-COORDINATES | ITU-T T.88 (08/2018) | 6.6.5.1 step 1a i; 6.6.5.2 step 1a i
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn fixed_pixel(value: i64) -> i64 {
    value >> 8
}

// Model navigation: spec::wholly_outside [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Audit: halftone-outside
// REQUIREMENT CONTEXT: T88-HALFTONE-SKIP-BOUNDARY | ITU-T T.88 (08/2018) | 6.6.5.1 step 1a ii
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn wholly_outside(
    x: i64,
    y: i64,
    pattern_width: u32,
    pattern_height: u32,
    region_width: u32,
    region_height: u32,
) -> bool {
    x as int + pattern_width as int <= 0
        || x as int >= region_width as int
        || y as int + pattern_height as int <= 0
        || y as int >= region_height as int
}

// Model navigation: spec::indexes_valid [representation-invariant]
// Purpose: Pattern storage, gray-index, work-budget and decoder-session invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn indexes_valid(indexes: Seq<u32>, count: u64) -> bool {
    forall|i: int| 0 <= i < indexes.len() ==> (indexes[i] as u64) < count
}

// Model navigation: spec::clipped_axis_valid [project-result-model]
// Purpose: Bit depth, Gray XOR, index insertion and signed affine/clipped grid results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn clipped_axis_valid(
    origin: i64,
    far_edge: i64,
    bound: u32,
    start: i64,
    end: i64,
) -> bool {
    start == if origin < 0 { 0 } else { origin }
        && end == if far_edge < bound as i64 { far_edge } else { bound as i64 }
        && 0 <= start
        && end <= bound
        && (start < end ==> (
            origin <= start
                && end <= far_edge
        ))
}

// Model navigation: spec::row_major_offset [proof-support]
// Purpose: Powers, row-major ordering and private work-budget projections.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn row_major_offset(
    width: u32,
    x: u32,
    y: u32,
) -> nat {
    y as nat * width as nat + x as nat
}

// Model navigation: spec::render_precedes [proof-support]
// Purpose: Powers, row-major ordering and private work-budget projections.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn render_precedes(
    first_x: u32,
    first_y: u32,
    second_x: u32,
    second_y: u32,
) -> bool {
    first_y < second_y || (first_y == second_y && first_x < second_x)
}

// Model navigation: spec::sliced_patterns_geometry [project-result-model]
// Purpose: Extracted pattern count and per-pattern bitmap dimensions for collective pattern slicing.
// Audit: halftone-sliced-patterns
// REQUIREMENT CONTEXT: T88-PATTERN-EXTRACTION | ITU-T T.88 (08/2018) | 6.7.5 steps 3-4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn sliced_patterns_geometry(
    patterns: Seq<crate::Bitmap>,
    header: super::PatternDictionaryHeader,
) -> bool {
    patterns.len() == header.gray_max as nat + 1
        && forall|i: int| 0 <= i < patterns.len() ==> (
            crate::views::bitmap_width(&#[trigger] patterns[i]) == header.pattern_width as u32
                && crate::views::bitmap_height(&patterns[i]) == header.pattern_height as u32
        )
}

// Model navigation: spec::gray_plane_xored [project-result-model]
// Purpose: Exact bytewise Gray-code XOR reconstruction across halftone gray planes.
// Audit: halftone-gray-plane-xor
// REQUIREMENT CONTEXT: T88-HALFTONE-GRAY-RECONSTRUCTION | ITU-T T.88 (08/2018) | 6.6.5 step 4; C.5 steps 1-4
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn gray_plane_xored(
    before: &crate::Bitmap,
    previous: &crate::Bitmap,
    after: &crate::Bitmap,
) -> bool {
    after.width == before.width
        && after.height == before.height
        && after.stride == before.stride
        && before.width == previous.width
        && before.height == previous.height
        && before.stride == previous.stride
        && after.data@.len() == before.data@.len()
        && before.data@.len() == previous.data@.len()
        && forall|i: int| 0 <= i < after.data@.len() ==>
            #[trigger] after.data@[i] == (before.data@[i] ^ previous.data@[i])
}

// Model navigation: spec::gray_indexes_geometry [project-result-model]
// Purpose: Reconstructed halftone gray-index buffer length and zero-depth values.
// Audit: halftone-gray-indexes-geometry
// REQUIREMENT CONTEXT: T88-HALFTONE-GRAY-RECONSTRUCTION | ITU-T T.88 (08/2018) | 6.6.5 step 4; C.5 steps 1-4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn gray_indexes_geometry(
    indexes: Seq<u32>,
    width: u32,
    height: u32,
    bit_depth: u8,
) -> bool {
    indexes.len() == width as nat * height as nat
        && (bit_depth == 0 ==> forall|index: int| 0 <= index < indexes.len() ==> indexes[index] == 0)
}

// Model navigation: spec::clipped_pattern_frame [project-result-model]
// Purpose: Clipped halftone pattern placement geometry preservation and wholly-outside no-op frame.
// Audit: halftone-clipped-pattern-frame
// REQUIREMENT CONTEXT: T88-HALFTONE-RENDER-ORDER | ITU-T T.88 (08/2018) | 6.6.5 steps 5-6; 6.6.5.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn clipped_pattern_frame(
    before: &crate::Bitmap,
    pattern: &crate::Bitmap,
    x: i64,
    y: i64,
    after: &crate::Bitmap,
) -> bool {
    crate::spec::bitmap::storage_valid(after)
        && after.width == before.width
        && after.height == before.height
        && after.stride == before.stride
        && after.data@.len() == before.data@.len()
        && (wholly_outside(x, y, pattern.width, pattern.height, before.width, before.height)
            ==> after.data@ == before.data@)
}

}

// Preserve the logical owner and private-field access of the model bridge.
include!("../proofs/model_support/halftone.rs");
