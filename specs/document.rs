// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;
use crate::file_header::file_header_spec;

#[path = "document/policy.rs"]
mod policy;
pub use policy::*;

#[path = "document/context.rs"]
mod context;
pub use context::{decoder_context_finished, decoder_context_state};
pub(crate) use context::decoder_context_fields;

#[path = "document/results.rs"]
mod results;
pub use results::global_parse_result;

#[path = "document/catalogs.rs"]
mod catalogs;
pub(crate) use catalogs::{reference_total, reference_total_monotone};

#[path = "document/reference_types.rs"]
mod reference_types;
pub(crate) use reference_types::*;

verus! {

// Project metadata transitions; caller sequencing and raster semantics are separate.
// Model navigation: spec::stripe_accepted [project-result-model]
// Purpose: Accepted stripe/finish metadata, row ordering and typed project errors.
// Audit: page-stripe-accepted
// REQUIREMENT CONTEXT: T88-STRIPE-END-ROW-ORDER | ITU-T T.88 (08/2018) | 7.4.8.6; 7.4.10
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn stripe_accepted(
    info: crate::segment_data::PageInformation,
    previous: Option<u32>,
    row: u32,
) -> bool {
    &&& info.striped
    &&& match previous {
        Some(last) => last < row && row as int - last as int <= info.max_stripe_size,
        None => row <= info.max_stripe_size,
    }
    &&& match info.height {
        Some(height) => row < height,
        None => true,
    }
}

// Model navigation: spec::stripe_result [project-result-model]
// Purpose: Accepted stripe/finish metadata, row ordering and typed project errors.
// Audit: page-stripe-result
// REQUIREMENT CONTEXT: T88-STRIPE-END-ROW-ORDER | ITU-T T.88 (08/2018) | 7.4.8.6; 7.4.10
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn stripe_result(
    info: crate::segment_data::PageInformation,
    previous: Option<u32>,
    row: u32,
) -> Result<(), crate::DecodeError> {
    if stripe_accepted(info, previous, row) {
        Ok(())
    } else {
        Err(crate::DecodeError::Malformed(crate::MalformedError::InvalidStripeEndRow(row)))
    }
}

// Model navigation: spec::page_finish_result [project-result-model]
// Purpose: Accepted stripe/finish metadata, row ordering and typed project errors.
// Audit: page-finish-result
// REQUIREMENT CONTEXT: T88-STRIPE-FINAL-PAGE-HEIGHT | ITU-T T.88 (08/2018) | 7.4.9; 7.4.10; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn page_finish_result(
    height: Option<u32>,
    last: Option<u32>,
    ended: bool,
) -> Result<(), crate::DecodeError> {
    if ended {
        Err(crate::DecodeError::Malformed(crate::MalformedError::DuplicatePageBoundary {
            page: 0, segment_type: 49,
        }))
    } else if height is None && last is None {
        Err(crate::DecodeError::Malformed(crate::MalformedError::InvalidStripeEndRow(0)))
    } else {
        Ok(())
    }
}

// Model navigation: spec::public_page_height [project-result-model]
// Purpose: Accepted stripe/finish metadata, row ordering and typed project errors.
// Audit: public-page-height
// REQUIREMENT CONTEXT: T88-STRIPE-FINAL-PAGE-HEIGHT | ITU-T T.88 (08/2018) | 7.4.9; 7.4.10; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn public_page_height(
    height: Option<u32>,
    last: Option<u32>,
) -> Result<u32, crate::DecodeError> {
    match height {
        Some(value) => Ok(value),
        None => match last {
            Some(row) => if row < u32::MAX {
                Ok((row + 1) as u32)
            } else {
                Err(crate::DecodeError::Malformed(crate::MalformedError::IntegerOverflow))
            },
            None => Err(crate::DecodeError::Malformed(
                crate::MalformedError::InvalidStripeEndRow(0),
            )),
        },
    }
}

// Model navigation: spec::public_page_fields [project-result-model]
// Purpose: Accepted stripe/finish metadata, row ordering and typed project errors.
// Audit: public-page-fields
// REQUIREMENT CONTEXT: T88-STRIPE-FINAL-PAGE-HEIGHT | ITU-T T.88 (08/2018) | 7.4.9; 7.4.10; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn public_page_fields(
    info: crate::segment_data::PageInformation,
    number: u32,
    page: crate::document::PageInfo,
) -> bool {
    &&& page.number == number
    &&& page.width == info.width
    &&& page.x_resolution == info.x_resolution
    &&& page.y_resolution == info.y_resolution
    &&& page.striped == info.striped
}

// Project quota/error policy, not a normative JBIG2 decoding equation.
// Model navigation: spec::quota_result [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn quota_result(
    actual: nat,
    limit: u64,
    kind: crate::LimitKind,
) -> Result<(), crate::DecodeError> {
    if actual > u64::MAX {
        Err(crate::DecodeError::Malformed(crate::MalformedError::IntegerOverflow))
    } else if actual > limit {
        Err(crate::DecodeError::LimitExceeded(crate::LimitExceeded {
            kind, limit, actual: actual as u64,
        }))
    } else {
        Ok(())
    }
}

// Model navigation: spec::next_count_quota_result [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn next_count_quota_result(
    count: usize,
    limit: u32,
    kind: crate::LimitKind,
) -> Result<(), crate::DecodeError> {
    if count == usize::MAX {
        Err(crate::DecodeError::Malformed(crate::MalformedError::IntegerOverflow))
    } else {
        quota_result(count as nat + 1, limit as u64, kind)
    }
}

// Model navigation: spec::decoder_limits [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn decoder_limits(
    decoder: &crate::document::Decoder,
) -> crate::Limits {
    decoder.limits
}

// Model navigation: spec::default_limits [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn default_limits(limits: crate::Limits) -> bool {
    &&& limits.max_width == 1_000_000
    &&& limits.max_height == 1_000_000
    &&& limits.max_pixels_per_page == 1_000_000_000
    &&& limits.max_decoder_managed_memory_bytes == 1_073_741_824
    &&& limits.max_stream_bytes == 1_073_741_824
    &&& limits.max_pages == 10_000
    &&& limits.max_segments == 100_000
    &&& limits.max_symbols == 1_000_000
    &&& limits.max_references_per_segment == 4_096
    &&& limits.max_total_references == 1_000_000
    &&& limits.max_reference_graph_depth == 128
    &&& limits.max_nesting_depth == 128
    &&& limits.max_work_units == 2_000_000_000
    &&& limits.max_cumulative_decoded_bitmap_bytes == 4_294_967_296
    &&& limits.max_retained_state_bytes == 536_870_912
}

// Model navigation: spec::document_organization [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_organization(
    document: &crate::document::DocumentInfo,
) -> crate::file_header::Organization {
    document.organization
}

// Model navigation: spec::document_declared_page_count [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-count
// REQUIREMENT CONTEXT: T88-DOCUMENT-COUNT-AGREEMENT | ITU-T T.88 (08/2018) | D.4.3
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_declared_page_count(
    document: &crate::document::DocumentInfo,
) -> Option<u32> {
    document.declared_page_count
}

// Model navigation: spec::document_segment_count [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_segment_count(
    document: &crate::document::DocumentInfo,
) -> u32 {
    document.segment_count
}

// Model navigation: spec::document_pages [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-pages
// REQUIREMENT CONTEXT: T88-DOCUMENT-COUNT-AGREEMENT | ITU-T T.88 (08/2018) | D.4.3
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn document_pages(
    document: &crate::document::DocumentInfo,
) -> Seq<crate::document::PageInfo> {
    document.pages@
}

// Model navigation: spec::declared_count_matches_pages [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: declared-count-matches-pages
// REQUIREMENT CONTEXT: T88-DOCUMENT-COUNT-AGREEMENT | ITU-T T.88 (08/2018) | D.4.3
// Fidelity: unresolved; no independent normative approval.
pub open spec fn declared_count_matches_pages(
    document: &crate::document::DocumentInfo,
) -> bool {
    match document_declared_page_count(document) {
        Some(count) => document_pages(document).len() == count as nat,
        None => true,
    }
}

// Model navigation: spec::standalone_metadata [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: standalone-metadata
// REQUIREMENT CONTEXT: T88-DOCUMENT-COUNT-AGREEMENT | ITU-T T.88 (08/2018) | D.4.3
// Fidelity: unresolved; no independent normative approval.
pub open spec fn standalone_metadata(
    input: Seq<u8>,
    document: &crate::document::DocumentInfo,
) -> bool {
    &&& file_header_spec::valid(input)
    &&& !file_header_spec::colour_regions(input)
    &&& document_organization(document) == file_header_spec::organization(input)
    &&& document_declared_page_count(document) == file_header_spec::page_count(input)
    &&& declared_count_matches_pages(document)
}

// Model navigation: spec::embedded_metadata [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: embedded-metadata
// REQUIREMENT CONTEXT: T88-PROFILE-EMBEDDED-GLOBAL-METADATA | ITU-T T.88 (08/2018) | D.3 (embedded context; metadata API shape is project policy)
// Fidelity: unresolved; no independent normative approval.
pub open spec fn embedded_metadata(
    document: &crate::document::DocumentInfo,
) -> bool {
    &&& document_organization(document) == crate::file_header::Organization::Sequential
    &&& document_declared_page_count(document) == None
}

// Model navigation: spec::payload_window [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: document-payload-window
// REQUIREMENT CONTEXT: T88-SEQUENTIAL-SEGMENT-LAYOUT | ITU-T T.88 (08/2018) | D.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn payload_window(
    header: &crate::segment_header::SegmentHeader,
    bytes: Seq<u8>,
    start: int,
    end: int,
    payload: Seq<u8>,
) -> bool {
    &&& 0 <= start <= end <= bytes.len()
    &&& payload == bytes.subrange(start, end)
    &&& match header.data_length {
        Some(length) => end - start == length,
        None => true,
    }
}

#[verifier::opaque]
// Model navigation: spec::known_segment_data_fields [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: known-segment-data-fields
// REQUIREMENT CONTEXT: T88-PAGE-KNOWN-DIMENSIONS | ITU-T T.88 (08/2018) | 7.4.8.1-7.4.8.2; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn known_segment_data_fields(
    segment_type: crate::segment_header::SegmentType,
    payload: Seq<u8>,
    data: crate::document::ParsedData,
) -> bool {
    match segment_type {
        crate::segment_header::SegmentType::PageInformation => match data {
            crate::document::ParsedData::Page(info) =>
                crate::spec::segments::page_information_fields(payload, info),
            _ => false,
        },
        crate::segment_header::SegmentType::EndOfStripe => match data {
            crate::document::ParsedData::EndOfStripe(row) =>
                payload.len() == 4 && row == crate::spec::segments::be_u32_at(payload, 0),
            _ => false,
        },
        crate::segment_header::SegmentType::Profiles => match data {
            crate::document::ParsedData::Profiles(profiles) =>
                crate::spec::segments::profiles_payload_fields(payload, profiles@),
            _ => false,
        },
        crate::segment_header::SegmentType::EndOfPage
        | crate::segment_header::SegmentType::EndOfFile =>
            payload.len() == 0 && data is Other,
        crate::segment_header::SegmentType::Extension =>
            crate::spec::segments::extension_payload_flags_valid(payload) && data is Other,
        crate::segment_header::SegmentType::ColourPalette => false,
        _ => data is Other,
    }
}

// Model navigation: spec::segment_metadata [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: container-segment-metadata
// REQUIREMENT CONTEXT: T88-PAGE-KNOWN-DIMENSIONS | ITU-T T.88 (08/2018) | 7.4.8.1-7.4.8.2; 8.2 step 2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn segment_metadata(
    segments: Seq<crate::document::ParsedSegment<'_>>,
) -> bool {
    forall|index: int| 0 <= index < segments.len() ==>
        #[trigger] known_segment_data_fields(
            segments[index].header.segment_type, segments[index].payload@, segments[index].data,
        )
}

// Model navigation: spec::sequential_offset [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-sequential-offset
// REQUIREMENT CONTEXT: T88-SEQUENTIAL-SEGMENT-LAYOUT | ITU-T T.88 (08/2018) | D.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn sequential_offset(
    segments: Seq<crate::document::ParsedSegment<'_>>,
    prefix: nat,
) -> nat
    recommends prefix <= segments.len(),
    decreases prefix,
{
    if prefix == 0 {
        0
    } else {
        sequential_offset(segments, (prefix - 1) as nat)
            + segments[(prefix - 1) as int].header.byte_length as nat
            + segments[(prefix - 1) as int].payload@.len()
    }
}

#[verifier::opaque]
// Model navigation: spec::sequential_container_fields [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: document-sequential-fields
// REQUIREMENT CONTEXT: T88-SEQUENTIAL-SEGMENT-LAYOUT | ITU-T T.88 (08/2018) | D.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn sequential_container_fields(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    ordered: bool,
) -> bool {
    &&& segment_metadata(segments)
    &&& sequential_offset(segments, segments.len()) == bytes.len()
    &&& forall|index: int| 0 <= index < segments.len() ==> {
        let segment = #[trigger] segments[index];
        let start = sequential_offset(segments, index as nat) as int;
        let payload_start = start + segment.header.byte_length;
        let end = sequential_offset(segments, (index + 1) as nat) as int;
        &&& 0 <= start < payload_start <= end <= bytes.len()
        &&& crate::spec::segments::segment_header_fields(
            &segment.header, bytes, start, payload_start,
        )
        &&& payload_window(&segment.header, bytes, payload_start, end, segment.payload@)
        &&& (ordered && index > 0 ==> segments[index - 1].header.number < segment.header.number)
    }
}

// Model navigation: spec::segment_headers [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-header-sequence
// REQUIREMENT CONTEXT: T88-RANDOM-HEADER-PAYLOAD-LAYOUT | ITU-T T.88 (08/2018) | D.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn segment_headers(
    segments: Seq<crate::document::ParsedSegment<'_>>,
) -> Seq<crate::segment_header::SegmentHeader> {
    Seq::new(segments.len(), |index: int| segments[index].header)
}

// Model navigation: spec::header_byte_offset [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-header-byte-offset
// REQUIREMENT CONTEXT: T88-RANDOM-HEADER-PAYLOAD-LAYOUT | ITU-T T.88 (08/2018) | D.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn header_byte_offset(
    headers: Seq<crate::segment_header::SegmentHeader>,
    prefix: nat,
) -> nat
    recommends prefix <= headers.len(),
    decreases prefix,
{
    if prefix == 0 {
        0
    } else {
        header_byte_offset(headers, (prefix - 1) as nat)
            + headers[(prefix - 1) as int].byte_length as nat
    }
}

// Model navigation: spec::payload_byte_offset [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: document-payload-byte-offset
// REQUIREMENT CONTEXT: T88-RANDOM-HEADER-PAYLOAD-LAYOUT | ITU-T T.88 (08/2018) | D.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn payload_byte_offset(
    segments: Seq<crate::document::ParsedSegment<'_>>,
    prefix: nat,
) -> nat
    recommends prefix <= segments.len(),
    decreases prefix,
{
    if prefix == 0 {
        0
    } else {
        payload_byte_offset(segments, (prefix - 1) as nat)
            + segments[(prefix - 1) as int].payload@.len()
    }
}

#[verifier::opaque]
// Model navigation: spec::random_access_segment_fields [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: document-random-access-segment-fields
// REQUIREMENT CONTEXT: T88-RANDOM-HEADER-PAYLOAD-LAYOUT | ITU-T T.88 (08/2018) | D.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn random_access_segment_fields(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    index: int,
) -> bool
    recommends 0 <= index < segments.len(),
{
    let headers = segment_headers(segments);
    let header_end = header_byte_offset(headers, headers.len());
    let segment = segments[index];
    let start = header_byte_offset(headers, index as nat) as int;
    let end = header_byte_offset(headers, (index + 1) as nat) as int;
    let payload_start = header_end + payload_byte_offset(segments, index as nat);
    let payload_end = header_end + payload_byte_offset(segments, (index + 1) as nat);
    &&& 0 <= start < end <= header_end <= payload_start <= payload_end <= bytes.len()
    &&& crate::spec::segments::segment_header_fields(
        &segment.header, bytes, start, end,
    )
    &&& payload_window(
        &segment.header, bytes, payload_start as int, payload_end as int, segment.payload@,
    )
    &&& (index > 0 ==> headers[index - 1].number < segment.header.number)
    &&& (index + 1 < segments.len() ==>
        segment.header.segment_type != crate::segment_header::SegmentType::EndOfFile)
}

#[verifier::opaque]
// Model navigation: spec::random_access_container_fields [project-result-model]
// Purpose: Accepted metadata and exact sequential/random header/payload window results.
// Audit: document-random-access-fields
// REQUIREMENT CONTEXT: T88-RANDOM-HEADER-PAYLOAD-LAYOUT | ITU-T T.88 (08/2018) | D.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn random_access_container_fields(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
) -> bool {
    let headers = segment_headers(segments);
    let header_end = header_byte_offset(headers, headers.len());
    &&& segment_metadata(segments)
    &&& segments.len() > 0
    &&& headers.last().segment_type == crate::segment_header::SegmentType::EndOfFile
    &&& header_end + payload_byte_offset(segments, segments.len()) == bytes.len()
    &&& forall|index: int| 0 <= index < segments.len() ==>
        #[trigger] random_access_segment_fields(bytes, segments, index)
}

// Model navigation: spec::page_information_count [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Audit: page-information-count
// REQUIREMENT CONTEXT: T88-DOCUMENT-COUNT-AGREEMENT | ITU-T T.88 (08/2018) | D.4.3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn page_information_count(
    segments: Seq<crate::document::ParsedSegment<'_>>,
    prefix: nat,
) -> nat
    recommends prefix <= segments.len(),
    decreases prefix,
{
    if prefix == 0 {
        0
    } else {
        let header = segments[(prefix - 1) as int].header;
        page_information_count(segments, (prefix - 1) as nat)
            + if header.page_association != 0
                && header.segment_type == crate::segment_header::SegmentType::PageInformation {
                1nat
            } else {
                0nat
            }
    }
}

// Model navigation: spec::global_segment_count [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn global_segment_count(
    globals: &crate::document::GlobalSegments,
) -> nat {
    globals.types@.len()
}

// Model navigation: spec::global_depth_count [proof-support]
// Purpose: Private retained reference-depth catalog cardinality.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn global_depth_count(
    globals: &crate::document::GlobalSegments,
) -> nat {
    globals.depths@.len()
}

// Model navigation: spec::global_types [proof-support]
// Purpose: Private global type catalog used by concrete staged reference lookup.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn global_types(globals: &crate::document::GlobalSegments)
    -> Seq<(u32, crate::segment_header::SegmentType)> {
    globals.types@
}

// Model navigation: spec::global_bytes [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn global_bytes(
    globals: &crate::document::GlobalSegments,
) -> Seq<u8> {
    globals.bytes@
}

// Model navigation: spec::production_is_intermediate_region [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn production_is_intermediate_region(
    segment_type: crate::segment_header::SegmentType,
) -> bool {
    segment_type == crate::segment_header::SegmentType::IntermediateTextRegion
        || segment_type == crate::segment_header::SegmentType::IntermediateHalftoneRegion
        || segment_type == crate::segment_header::SegmentType::IntermediateGenericRegion
        || segment_type
            == crate::segment_header::SegmentType::IntermediateGenericRefinementRegion
}

pub enum Envelope {
    Sequential,
    RandomAccess,
    Embedded,
}

pub enum SegmentClass {
    SymbolDictionary,
    IntermediateTextRegion,
    ImmediateTextRegion,
    PatternDictionary,
    IntermediateHalftoneRegion,
    ImmediateHalftoneRegion,
    IntermediateGenericRegion,
    ImmediateGenericRegion,
    IntermediateRefinementRegion,
    ImmediateRefinementRegion,
    PageInformation,
    EndOfPage,
    EndOfStripe,
    EndOfFile,
    Profiles,
    Tables,
    ColourPalette,
    Extension,
}

pub struct Reference {
    pub number: nat,
    pub class: SegmentClass,
}

pub struct Segment {
    pub number: nat,
    pub class: SegmentClass,
    pub page: nat,
    pub references: Seq<Reference>,
}

// Model navigation: spec::sum [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn sum(values: Seq<nat>) -> nat
    decreases values.len(),
{
    if values.len() == 0 {
        0
    } else {
        sum(values.drop_last()) + values.last()
    }
}

// Model navigation: spec::reference_count [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn reference_count(segments: Seq<Segment>) -> nat
    decreases segments.len(),
{
    if segments.len() == 0 {
        0
    } else {
        reference_count(segments.drop_last()) + segments.last().references.len()
    }
}

// Model navigation: spec::work [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn work(input_bytes: nat, segments: Seq<Segment>) -> nat {
    input_bytes + segments.len() + reference_count(segments)
}

// Model navigation: spec::envelope_bytes [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn envelope_bytes(
    envelope: Envelope,
    file_header_bytes: nat,
    segment_header_bytes: Seq<nat>,
    payload_bytes: Seq<nat>,
) -> nat {
    match envelope {
        Envelope::Sequential =>
            file_header_bytes + sum(segment_header_bytes) + sum(payload_bytes),
        Envelope::RandomAccess =>
            file_header_bytes + sum(segment_header_bytes) + sum(payload_bytes),
        Envelope::Embedded => sum(segment_header_bytes) + sum(payload_bytes),
    }
}

// Model navigation: spec::unknown_marker_fits [candidate-standard-facing-model]
// Purpose: Abstract page-event/final-height and marker-extent candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn unknown_marker_fits(input_len: nat, marker_offset: nat) -> bool {
    18 <= marker_offset && marker_offset + 6 <= input_len
}

// Model navigation: spec::unknown_marker_valid [candidate-standard-facing-model]
// Purpose: Abstract page-event/final-height and marker-extent candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn unknown_marker_valid(
    input_len: nat,
    marker_offset: nat,
    declared_height: nat,
    row_count: nat,
) -> bool {
    unknown_marker_fits(input_len, marker_offset) && row_count <= declared_height
}

// Model navigation: spec::bounded_reads [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn bounded_reads(chunks: Seq<nat>, cap: nat) -> bool
    decreases chunks.len(),
{
    if chunks.len() == 0 {
        true
    } else {
        bounded_reads(chunks.drop_last(), cap) && sum(chunks) <= cap
    }
}

// Model navigation: spec::count_limits [project-policy]
// Purpose: Project read/count/work limits, defaults and typed error priority.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn count_limits(
    segments: nat,
    pages: nat,
    max_segments: nat,
    max_pages: nat,
) -> bool {
    segments <= max_segments && pages <= max_pages
}

// Model navigation: spec::unique_numbers [candidate-standard-facing-model]
// Purpose: Abstract document order, reference ownership, retention and profile candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn unique_numbers(segments: Seq<Segment>) -> bool {
    forall|i: int, j: int|
        0 <= i < segments.len() && 0 <= j < segments.len() && i != j
            ==> segments[i].number != segments[j].number
}

// Model navigation: spec::contains_number [proof-support]
// Purpose: Private document projections, counts, membership and extent folds.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn contains_number(segments: Seq<Segment>, number: nat) -> bool {
    exists|i: int| 0 <= i < segments.len() && segments[i].number == number
}

// Model navigation: spec::references_resolve_lower [candidate-standard-facing-model]
// Purpose: Abstract document order, reference ownership, retention and profile candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn references_resolve_lower(
    segments: Seq<Segment>,
    global_numbers: Set<nat>,
) -> bool {
    forall|i: int, r: int|
        0 <= i < segments.len() && 0 <= r < segments[i].references.len()
            ==> {
                let reference = #[trigger] segments[i].references[r];
                &&& reference.number < segments[i].number
                &&& (contains_number(segments, reference.number)
                    || global_numbers.contains(reference.number))
            }
}

// Model navigation: spec::page_association_allowed [candidate-standard-facing-model]
// Purpose: Abstract document order, reference ownership, retention and profile candidates.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn page_association_allowed(class: SegmentClass, page: nat) -> bool {
    match class {
        SegmentClass::IntermediateTextRegion
        | SegmentClass::ImmediateTextRegion
        | SegmentClass::IntermediateHalftoneRegion
        | SegmentClass::ImmediateHalftoneRegion
        | SegmentClass::IntermediateGenericRegion
        | SegmentClass::ImmediateGenericRegion
        | SegmentClass::IntermediateRefinementRegion
        | SegmentClass::ImmediateRefinementRegion
        | SegmentClass::PageInformation
        | SegmentClass::EndOfPage
        | SegmentClass::EndOfStripe => page != 0,
        SegmentClass::EndOfFile => page == 0,
        _ => true,
    }
}


}

// Preserve the logical owner and private-field access of the model bridges.
include!("../proofs/model_support/document.rs");
