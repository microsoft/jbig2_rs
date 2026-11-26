#![allow(clippy::duplicated_attributes)]
#![cfg(test)]

mod budgets_and_pages;
mod container_routes;
mod direct_and_reuse;
mod dispatch_and_resources;
mod error_adapters;
mod symbol_resources;

use super::dispatch::{
    borrowed_symbol_memory, collect_referred_symbols, collect_referred_tables,
    decode_generic_segment_buffered, direct_generic_page_index, first_referred_segment,
    generic_template,
};
use super::rendering::{
    checked_page_write_state, checked_seal_stripe_state, same_combination_operator,
    seal_stripe_on_page,
};
use super::resources::structural_work_totals;
use super::*;

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn decode_generic_segment_oracle(
    segment: &ParsedSegment<'_>,
    pages: &mut RuntimePages,
    buffers: &mut RegionBuffers,
    retained: &mut RetainedBudget,
    work: &mut DecodeWork,
    limits: &Limits,
    mmr_scratch: &mut MmrScratch,
) -> Result<(), DecodeError> {
    let mut generic_scratch = Vec::new();
    decode_generic_segment_buffered(
        segment,
        pages,
        buffers,
        retained,
        work,
        limits,
        mmr_scratch,
        &mut generic_scratch,
    )
}

#[cfg(test)]
fn clipped_coordinate(origin: u32, offset: u32, bound: u32) -> Option<u32> {
    let coordinate = origin.checked_add(offset)?;
    if coordinate < bound {
        Some(coordinate)
    } else {
        None
    }
}

const SIGNATURE: [u8; 8] = [0x97, 0x4A, 0x42, 0x32, 0x0D, 0x0A, 0x1A, 0x0A];

fn segment_header(number: u32, segment_type: u8, page: u8, length: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&number.to_be_bytes());
    bytes.push(segment_type);
    bytes.push(0);
    bytes.push(page);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes
}

fn referenced_segment_header(
    number: u32,
    segment_type: u8,
    page: u8,
    referred: u8,
    length: u32,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&number.to_be_bytes());
    bytes.push(segment_type);
    bytes.push(0x20);
    bytes.push(referred);
    bytes.push(page);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes
}

fn page_data() -> [u8; 19] {
    let mut data = [0_u8; 19];
    data[0..4].copy_from_slice(&8_u32.to_be_bytes());
    data[4..8].copy_from_slice(&1_u32.to_be_bytes());
    data[16] = 0x40;
    data
}

fn symbol_reference_segment(references: &[u32]) -> ParsedSegment<'static> {
    assert!(references.len() <= 4);
    let mut bytes = segment_header(100, 6, 1, 0);
    let count = u8::try_from(references.len()).unwrap();
    bytes[5] = (count << 5) | ((1 << (count + 1)) - 1);
    for (index, number) in references.iter().enumerate() {
        bytes.insert(6 + index, u8::try_from(*number).unwrap());
    }
    let mut reader = ByteReader::new(&bytes);
    let header =
        crate::segment_header::parse_segment_header(&mut reader, &Limits::default()).unwrap();
    assert_eq!(reader.position(), bytes.len());
    assert_eq!(header.referred_segments, references);
    ParsedSegment {
        header,
        payload: &[],
        data: ParsedData::Other,
    }
}

fn generic_data(coded: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&8_u32.to_be_bytes());
    data.extend_from_slice(&1_u32.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.extend_from_slice(&0_u32.to_be_bytes());
    data.push(0);
    data.push(1);
    data.extend_from_slice(coded);
    data
}

fn document_with_generic(coded: &[u8]) -> Vec<u8> {
    let generic = generic_data(coded);
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page_data());
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("test payload fits"),
    ));
    bytes.extend_from_slice(&generic);
    bytes.extend_from_slice(&segment_header(2, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(3, 51, 0, 0));
    bytes
}

fn arithmetic_generic_document(width: u32, height: u32, region_flags: u8, coded: &[u8]) -> Vec<u8> {
    let mut page = [0_u8; 19];
    page[0..4].copy_from_slice(&width.to_be_bytes());
    page[4..8].copy_from_slice(&height.to_be_bytes());
    let mut generic = Vec::new();
    generic.extend_from_slice(&width.to_be_bytes());
    generic.extend_from_slice(&height.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.push(region_flags);
    generic.push(0);
    generic.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    generic.extend_from_slice(coded);
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&generic);
    bytes.extend_from_slice(&segment_header(2, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(3, 51, 0, 0));
    bytes
}

fn arithmetic_test_bytes(state: &mut u64) -> Vec<u8> {
    let mut coded = Vec::with_capacity(1_026);
    for _ in 0..1_024 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        coded.push(state.to_be_bytes()[3] & 0x7f);
    }
    coded.extend_from_slice(&[0xff, 0xac]);
    coded
}

fn all_white_mmr_document(width: u32, height: u32) -> Vec<u8> {
    let mut page = [0_u8; 19];
    page[0..4].copy_from_slice(&width.to_be_bytes());
    page[4..8].copy_from_slice(&height.to_be_bytes());
    page[16] = 0x40;
    let mut generic = Vec::new();
    generic.extend_from_slice(&width.to_be_bytes());
    generic.extend_from_slice(&height.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.extend_from_slice(&0_u32.to_be_bytes());
    generic.push(0);
    generic.push(1);
    let coded_len = usize::try_from(height.div_ceil(8)).expect("test height fits");
    generic.extend(std::iter::repeat_n(0xFF, coded_len));
    if height % 8 != 0 {
        *generic.last_mut().expect("coded byte") = 0xFF << (8 - height % 8);
    }
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&segment_header(0, 48, 1, 19));
    bytes.extend_from_slice(&page);
    bytes.extend_from_slice(&segment_header(
        1,
        38,
        1,
        u32::try_from(generic.len()).expect("payload fits"),
    ));
    bytes.extend_from_slice(&generic);
    bytes.extend_from_slice(&segment_header(2, 49, 1, 0));
    bytes.extend_from_slice(&segment_header(3, 51, 0, 0));
    bytes
}

fn runtime_test_page(
    width: u32,
    height: u32,
    default_pixel: bool,
    pristine: bool,
    operator: CombinationOperator,
    limits: &Limits,
) -> RuntimePage {
    RuntimePage {
        default_pixel,
        pristine,
        default_operator: operator,
        combination_override: false,
        refinements_allowed: false,
        auxiliary_allowed: false,
        last_stripe_end: None,
        open_stripe_max_row: None,
        bitmap: Bitmap::filled(width, height, default_pixel, limits).expect("test page bitmap"),
    }
}

fn assert_direct_page_matches_oracle(
    document: &[u8],
    default_pixel: bool,
    pristine: bool,
    page_operator: CombinationOperator,
    limits: &Limits,
) {
    let (_info, segments) =
        parse_standalone_document(document, &Limits::default()).expect("test document");
    let segment = &segments[1];
    let (header, _) =
        parse_generic_region_header(segment.payload, &Limits::default()).expect("header");
    let mut direct_pages = vec![(
        1,
        runtime_test_page(
            header.region.width,
            header.region.height,
            default_pixel,
            pristine,
            page_operator,
            &Limits::default(),
        ),
    )];
    let mut oracle_pages = vec![(
        1,
        runtime_test_page(
            header.region.width,
            header.region.height,
            default_pixel,
            pristine,
            page_operator,
            &Limits::default(),
        ),
    )];
    let mut direct_buffers = Vec::new();
    let mut oracle_buffers = Vec::new();
    let mut direct_retained = RetainedBudget::new();
    let mut oracle_retained = RetainedBudget::new();
    let mut direct_work = DecodeWork::new(0, limits).expect("direct work");
    let mut oracle_work = DecodeWork::new(0, limits).expect("oracle work");
    let mut direct_scratch = MmrScratch::default();
    let mut oracle_scratch = MmrScratch::default();
    let mut direct_generic_scratch = Vec::new();
    let direct = decode_generic_segment(
        segment,
        &mut direct_pages,
        &mut direct_buffers,
        &mut direct_retained,
        &mut direct_work,
        limits,
        &mut direct_scratch,
        &mut direct_generic_scratch,
    );
    let oracle = decode_generic_segment_oracle(
        segment,
        &mut oracle_pages,
        &mut oracle_buffers,
        &mut oracle_retained,
        &mut oracle_work,
        limits,
        &mut oracle_scratch,
    );
    assert_eq!(
        direct.as_ref().err().map(ToString::to_string),
        oracle.as_ref().err().map(ToString::to_string)
    );
    if direct.is_ok() {
        let direct_page = &direct_pages[0].1;
        let oracle_page = &oracle_pages[0].1;
        assert_eq!(direct_page.bitmap, oracle_page.bitmap);
        assert_eq!(direct_page.pristine, oracle_page.pristine);
        assert_eq!(
            direct_page.open_stripe_max_row,
            oracle_page.open_stripe_max_row
        );
        assert_eq!(direct_work.used, oracle_work.used);
        assert_eq!(direct_retained.bytes, oracle_retained.bytes);
    }
}

fn assert_wide_budget_result(
    result: Result<(), DecodeError>,
    total: u128,
    limit: u64,
    kind: crate::LimitKind,
) {
    if total > u128::from(u64::MAX) {
        assert!(matches!(
            result,
            Err(DecodeError::Malformed(MalformedError::IntegerOverflow))
        ));
    } else if total > u128::from(limit) {
        assert!(matches!(
            result,
            Err(DecodeError::LimitExceeded(error))
                if error.kind == kind
                    && error.limit == limit
                    && u128::from(error.actual) == total
        ));
    } else {
        result.expect("representable total within the inclusive limit");
    }
}
