// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::segment_type [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: segment-type
// REQUIREMENT CONTEXT: T88-SEGMENT-TYPE-FIELD | ITU-T T.88 (08/2018) | 7.2.3 bits 0-5; 7.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn segment_type(
    number: u8,
) -> Option<crate::segment_header::SegmentType> {
    match number {
        0 => Some(crate::segment_header::SegmentType::SymbolDictionary),
        4 => Some(crate::segment_header::SegmentType::IntermediateTextRegion),
        6 => Some(crate::segment_header::SegmentType::ImmediateTextRegion),
        7 => Some(crate::segment_header::SegmentType::ImmediateLosslessTextRegion),
        16 => Some(crate::segment_header::SegmentType::PatternDictionary),
        20 => Some(crate::segment_header::SegmentType::IntermediateHalftoneRegion),
        22 => Some(crate::segment_header::SegmentType::ImmediateHalftoneRegion),
        23 => Some(crate::segment_header::SegmentType::ImmediateLosslessHalftoneRegion),
        36 => Some(crate::segment_header::SegmentType::IntermediateGenericRegion),
        38 => Some(crate::segment_header::SegmentType::ImmediateGenericRegion),
        39 => Some(crate::segment_header::SegmentType::ImmediateLosslessGenericRegion),
        40 => Some(crate::segment_header::SegmentType::IntermediateGenericRefinementRegion),
        42 => Some(crate::segment_header::SegmentType::ImmediateGenericRefinementRegion),
        43 => Some(crate::segment_header::SegmentType::ImmediateLosslessGenericRefinementRegion),
        48 => Some(crate::segment_header::SegmentType::PageInformation),
        49 => Some(crate::segment_header::SegmentType::EndOfPage),
        50 => Some(crate::segment_header::SegmentType::EndOfStripe),
        51 => Some(crate::segment_header::SegmentType::EndOfFile),
        52 => Some(crate::segment_header::SegmentType::Profiles),
        53 => Some(crate::segment_header::SegmentType::Tables),
        54 => Some(crate::segment_header::SegmentType::ColourPalette),
        62 => Some(crate::segment_header::SegmentType::Extension),
        _ => None,
    }
}

// Model navigation: spec::reference_width [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Audit: reference-width
// REQUIREMENT CONTEXT: T88-SEGMENT-REFERENCE-WIDTH | ITU-T T.88 (08/2018) | 7.2.5
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn reference_width(segment_number: u32) -> u8 {
    if segment_number <= 256 {
        1
    } else if segment_number <= 65_536 {
        2
    } else {
        4
    }
}

// Model navigation: spec::short_count_valid [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Audit: short-count-valid
// REQUIREMENT CONTEXT: T88-SEGMENT-RETENTION-UNUSED-BITS | ITU-T T.88 (08/2018) | 7.2.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn short_count_valid(first: u8) -> bool {
    match first >> 5 {
        0 => first & 0x1e == 0,
        1 => first & 0x1c == 0,
        2 => first & 0x18 == 0,
        3 => first & 0x10 == 0,
        4 => true,
        _ => false,
    }
}

// Model navigation: spec::retention_bit [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Audit: retention-wire-bit
// REQUIREMENT CONTEXT: T88-SEGMENT-RETENTION-WIRE-ORDER | ITU-T T.88 (08/2018) | 7.2.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn retention_bit(bytes: Seq<u8>, index: nat) -> u8
    recommends index / 8 < bytes.len(),
{
    let byte = bytes[(index / 8) as int];
    if index % 8 == 0 { byte & 1 }
    else if index % 8 == 1 { (byte >> 1) & 1 }
    else if index % 8 == 2 { (byte >> 2) & 1 }
    else if index % 8 == 3 { (byte >> 3) & 1 }
    else if index % 8 == 4 { (byte >> 4) & 1 }
    else if index % 8 == 5 { (byte >> 5) & 1 }
    else if index % 8 == 6 { (byte >> 6) & 1 }
    else { (byte >> 7) & 1 }
}

// Model navigation: spec::retention_values_match [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: retention-values-match
// REQUIREMENT CONTEXT: T88-SEGMENT-RETENTION-WIRE-ORDER | ITU-T T.88 (08/2018) | 7.2.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn retention_values_match(values: Seq<u8>, bytes: Seq<u8>) -> bool {
    forall|index: int| 0 <= index < values.len() ==>
        index / 8 < bytes.len()
        && values[index] == retention_bit(bytes, index as nat)
}

// Model navigation: spec::unused_retention_bits_zero [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Audit: retention-unused-zero
// REQUIREMENT CONTEXT: T88-SEGMENT-RETENTION-UNUSED-BITS | ITU-T T.88 (08/2018) | 7.2.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn unused_retention_bits_zero(
    bit_count: u32,
    bytes: Seq<u8>,
) -> bool {
    let used = bit_count % 8;
    let mask: u16 = match used {
        1 => 0x0001u16,
        2 => 0x0003u16,
        3 => 0x0007u16,
        4 => 0x000fu16,
        5 => 0x001fu16,
        6 => 0x003fu16,
        7 => 0x007fu16,
        _ => 0xffffu16,
    };
    used == 0
        || (bytes.len() > 0
            && ((bytes[bytes.len() - 1] as u16) & !mask) == 0u16)
}

// Model navigation: spec::combination_operator [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: segment-combination-operator
// REQUIREMENT CONTEXT: T88-COMBINATION-WIRE-CODE | ITU-T T.88 (08/2018) | 7.4.1.5 bits 0-2; 7.4.8.5 bits 3-4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn combination_operator(
    encoded: u8,
    replace_allowed: bool,
) -> Option<crate::bitmap::CombinationOperator> {
    match encoded {
        0 => Some(crate::bitmap::CombinationOperator::Or),
        1 => Some(crate::bitmap::CombinationOperator::And),
        2 => Some(crate::bitmap::CombinationOperator::Xor),
        3 => Some(crate::bitmap::CombinationOperator::Xnor),
        4 if replace_allowed => Some(crate::bitmap::CombinationOperator::Replace),
        _ => None,
    }
}

// Model navigation: spec::dimensions_within_limits [project-policy]
// Purpose: Caller Limits policy for dimensions and pixel count.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn dimensions_within_limits(
    width: u32,
    height: u32,
    limits: crate::Limits,
) -> bool {
    width <= limits.max_width
        && height <= limits.max_height
        && width as nat * height as nat <= limits.max_pixels_per_page as nat
}

// Model navigation: spec::extension_flags_valid [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn extension_flags_valid(extension_type: u32) -> bool {
    extension_type & 0x8000_0000 == 0
        || extension_type & 0x2000_0000 != 0
}

// Model navigation: spec::be_u32_at [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Audit: segment-be-u32
// REQUIREMENT CONTEXT: T88-STRIPE-END-ROW-FIELD | ITU-T T.88 (08/2018) | 7.4.10
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn be_u32_at(bytes: Seq<u8>, start: int) -> u32
    recommends 0 <= start, start + 4 <= bytes.len(),
{
    ((bytes[start] as u32) << 24)
        | ((bytes[start + 1] as u32) << 16)
        | ((bytes[start + 2] as u32) << 8)
        | (bytes[start + 3] as u32)
}

// Model navigation: spec::reference_value_at [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn reference_value_at(bytes: Seq<u8>, start: int, width: u8) -> u32
    recommends
        width == 1 || width == 2 || width == 4,
        0 <= start,
        start + width <= bytes.len(),
{
    if width == 1 {
        bytes[start] as u32
    } else if width == 2 {
        (((bytes[start] as u16) << 8) | bytes[start + 1] as u16) as u32
    } else {
        be_u32_at(bytes, start)
    }
}

// Model navigation: spec::header_reference_count [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn header_reference_count(bytes: Seq<u8>, start: int) -> u32
    recommends
        0 <= start < bytes.len(),
        bytes[start] >> 5 <= 4 || start + 4 <= bytes.len(),
{
    if bytes[start] >> 5 <= 4 {
        (bytes[start] >> 5) as u32
    } else {
        be_u32_at(bytes, start) & 0x1fff_ffff
    }
}

// Model navigation: spec::header_retention_start [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn header_retention_start(bytes: Seq<u8>, start: int) -> int
    recommends 0 <= start < bytes.len(),
{
    if bytes[start] >> 5 == 7 { start + 4 } else { start }
}

// Model navigation: spec::header_references_start [proof-support]
// Purpose: Big-endian/reference/retention projections, encoded widths and field offsets.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn header_references_start(bytes: Seq<u8>, start: int) -> int
    recommends
        0 <= start < bytes.len(),
        bytes[start] >> 5 <= 4 || start + 4 <= bytes.len(),
{
    if bytes[start] >> 5 == 7 {
        start + 4 + (header_reference_count(bytes, start) as int + 8) / 8
    } else {
        start + 1
    }
}

// Model navigation: spec::referred_header_fields [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: segment-referred-header-fields
// REQUIREMENT CONTEXT: T88-SEGMENT-RETENTION-WIRE-ORDER | ITU-T T.88 (08/2018) | 7.2.4
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn referred_header_fields(
    bytes: Seq<u8>,
    start: int,
    number: u32,
    references: Seq<u32>,
    retention: Seq<u8>,
    end: int,
) -> bool {
    &&& 0 <= start < bytes.len()
    &&& bytes[start] >> 5 <= 4 || bytes[start] >> 5 == 7
    &&& bytes[start] >> 5 <= 4 || start + 4 <= bytes.len()
    &&& {
        let count = header_reference_count(bytes, start);
        let flags_start = header_retention_start(bytes, start);
        let references_start = header_references_start(bytes, start);
        let width = reference_width(number);
        &&& count <= 0x1fff_ffff
        &&& if bytes[start] >> 5 == 7 {
            count > 4
        } else {
            short_count_valid(bytes[start])
        }
        &&& flags_start <= references_start <= end <= bytes.len()
        &&& references.len() == count
        &&& retention.len() == count as nat + 1
        &&& retention_values_match(retention, bytes.subrange(flags_start, references_start))
        &&& (bytes[start] >> 5 == 7 ==> unused_retention_bits_zero(
            (count + 1) as u32, bytes.subrange(flags_start, references_start),
        ))
        &&& end == references_start + references.len() * width
        &&& forall|index: int| 0 <= index < references.len() ==>
            #[trigger] references[index] < number
                && references[index] == reference_value_at(
                    bytes, references_start + index * width, width,
                )
    }
}

#[verifier::opaque]
// Model navigation: spec::segment_header_fields [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: segment-header-fields
// REQUIREMENT CONTEXT: T88-SEGMENT-TYPE-FIELD | ITU-T T.88 (08/2018) | 7.2.3 bits 0-5; 7.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn segment_header_fields(
    header: &crate::segment_header::SegmentHeader,
    bytes: Seq<u8>,
    start: int,
    end: int,
) -> bool {
    &&& 0 <= start
    &&& start + 6 <= bytes.len()
    &&& bytes[start + 5] >> 5 <= 4 || start + 9 <= bytes.len()
    &&& header.number == be_u32_at(bytes, start)
    &&& segment_type(bytes[start + 4] & 0x3f) == Some(header.segment_type)
    &&& header.deferred_non_retain == (bytes[start + 4] & 0x80 != 0)
    &&& {
        let references_start = header_references_start(bytes, start + 5);
        let page_start = references_start + header.referred_segments@.len()
            * reference_width(header.number);
        let long_page = bytes[start + 4] & 0x40 != 0;
        let length_start = page_start + if long_page { 4int } else { 1int };
        &&& referred_header_fields(
            bytes, start + 5, header.number, header.referred_segments@,
            header.retention_flags.values@, page_start,
        )
        &&& end == length_start + 4
        &&& end <= bytes.len()
        &&& header.page_association == if long_page {
            be_u32_at(bytes, page_start)
        } else {
            bytes[page_start] as u32
        }
        &&& match header.data_length {
            Some(length) => length == be_u32_at(bytes, length_start)
                && length != 0xffff_ffff,
            None => be_u32_at(bytes, length_start) == 0xffff_ffff
                && header.segment_type == crate::segment_header::SegmentType::ImmediateGenericRegion,
        }
        &&& header.byte_length == end - start
    }
}

// Model navigation: spec::profiles_payload_valid [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn profiles_payload_valid(bytes: Seq<u8>) -> bool {
    bytes.len() >= 4
        && bytes.len() as nat
            == 4 * (be_u32_at(bytes, 0) as nat + 1)
}

// Model navigation: spec::profile_length_matches [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn profile_length_matches(bytes: Seq<u8>, count: u32) -> bool {
    bytes.len() == 4 * (count as nat + 1)
}

// Model navigation: spec::extension_payload_flags_valid [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn extension_payload_flags_valid(bytes: Seq<u8>) -> bool {
    bytes.len() >= 4
        && extension_flags_valid(be_u32_at(bytes, 0))
}

// Model navigation: spec::extension_kind [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn extension_kind(extension_type: u32) -> Option<u8> {
    if !extension_flags_valid(extension_type)
        || extension_type & 0x8000_0000 != 0
    {
        None
    } else if extension_type == 0x2000_0000 {
        Some(1)
    } else if extension_type == 0x2000_0002 {
        Some(2)
    } else {
        Some(0)
    }
}

// Model navigation: spec::final_code_unit_zero [candidate-standard-facing-model]
// Purpose: Reserved bits, extension/profile extents and final-code-unit candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn final_code_unit_zero(
    bytes: Seq<u8>,
    code_unit_size: usize,
) -> bool {
    code_unit_size > 0
        && bytes.len() >= code_unit_size
        && forall|i: int|
            bytes.len() - code_unit_size <= i < bytes.len()
                ==> bytes[i] == 0
}

// Model navigation: spec::region_flags_match [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn region_flags_match(
    info: crate::segment_data::RegionInfo,
    flags: u8,
) -> bool {
    flags & 0xf0 == 0
        && flags & 0x08 == 0
        && combination_operator(flags & 0x07, true) == Some(info.operator)
}

// Model navigation: spec::page_flags_match [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: page-flags-match
// REQUIREMENT CONTEXT: T88-PAGE-DEFAULT-PIXEL | ITU-T T.88 (08/2018) | 7.4.8.5 bit 2; 8.2 step 3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn page_flags_match(
    info: crate::segment_data::PageInformation,
    flags: u8,
    striping: u16,
) -> bool {
    &&& flags & 0x80 == 0
    &&& info.eventually_lossless == (flags & 0x01 != 0)
    &&& info.might_contain_refinements == (flags & 0x02 != 0)
    &&& info.default_pixel == (flags & 0x04 != 0)
    &&& combination_operator((flags >> 3) & 0x03, false)
        == Some(info.default_operator)
    &&& info.requires_auxiliary_buffers == (flags & 0x20 != 0)
    &&& info.combination_operator_override == (flags & 0x40 != 0)
    &&& info.striped == (striping & 0x8000 != 0)
    &&& info.max_stripe_size == striping & 0x7fff
}

// Model navigation: spec::page_information_fields [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: page-information-fields
// REQUIREMENT CONTEXT: T88-PAGE-KNOWN-DIMENSIONS | ITU-T T.88 (08/2018) | 7.4.8.1-7.4.8.2; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn page_information_fields(
    bytes: Seq<u8>,
    info: crate::segment_data::PageInformation,
) -> bool {
    &&& bytes.len() == 19
    &&& info.width == be_u32_at(bytes, 0)
    &&& info.height == if be_u32_at(bytes, 4) == 0xffff_ffff {
        None
    } else {
        Some(be_u32_at(bytes, 4))
    }
    &&& info.x_resolution == be_u32_at(bytes, 8)
    &&& info.y_resolution == be_u32_at(bytes, 12)
    &&& page_flags_match(info, bytes[16], ((bytes[17] as u16) << 8) | bytes[18] as u16)
}

// Model navigation: spec::colour_page_information_fields [project-result-model]
// Purpose: Exact page fields with the colour declaration handled separately.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn colour_page_information_fields(
    bytes: Seq<u8>, info: crate::segment_data::PageInformation,
) -> bool {
    &&& bytes.len() == 19
    &&& info.width == be_u32_at(bytes, 0)
    &&& info.height == if be_u32_at(bytes, 4) == 0xffff_ffff { None }
        else { Some(be_u32_at(bytes, 4)) }
    &&& info.x_resolution == be_u32_at(bytes, 8)
    &&& info.y_resolution == be_u32_at(bytes, 12)
    &&& page_flags_match(info, bytes[16] & 0x7f, ((bytes[17] as u16) << 8) | bytes[18] as u16)
}

// Model navigation: spec::profiles_payload_fields [project-result-model]
// Purpose: Encoded enums, ordered retention/references, metadata fields and header extents.
// Audit: profiles-payload-fields
// REQUIREMENT CONTEXT: T88-SEQUENTIAL-SEGMENT-LAYOUT | ITU-T T.88 (08/2018) | D.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn profiles_payload_fields(bytes: Seq<u8>, profiles: Seq<u32>) -> bool {
    &&& profiles_payload_valid(bytes)
    &&& profiles.len() == be_u32_at(bytes, 0)
    &&& forall|index: int| 0 <= index < profiles.len() ==>
        #[trigger] profiles[index] == be_u32_at(bytes, 4 + index * 4)
}

}
