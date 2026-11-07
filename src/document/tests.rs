#![allow(clippy::duplicated_attributes)]
#![cfg(test)]

mod container_inspection;
mod payload_windows;
mod quotas_and_references;
mod reference_types;

use super::pages::validate_page_profiles;
use super::parsing::{
    find_unknown_generic_length, parse_known_segment_data, parse_random_access, read_payload,
};
use super::references::{
    count_total_references, known_reference_type, validate_reference_depth,
    validate_reference_depth_ordered, validate_reference_links, validate_reference_types,
    validate_retention_flags,
};
use super::*;

fn raw_page_state_info(height: Option<u32>, striped: bool, maximum: u16) -> PageInformation {
    PageInformation {
        width: 0x1234_5678,
        height,
        x_resolution: 0x0102_0304,
        y_resolution: 0x89ab_cdef,
        eventually_lossless: true,
        might_contain_refinements: true,
        default_pixel: true,
        default_operator: crate::bitmap::CombinationOperator::Xor,
        requires_auxiliary_buffers: true,
        combination_operator_override: true,
        striped,
        max_stripe_size: maximum,
    }
}

fn raw_page_state_result<T>(result: Result<T, DecodeError>) -> Result<T, MalformedError> {
    result.map_err(|error| match error {
        DecodeError::Malformed(error) => error,
        other => panic!("unexpected PageState error: {other:?}"),
    })
}

#[test]
fn raw_page_state_new_preserves_all_information_and_initial_boundaries() {
    for height in [None, Some(0), Some(1), Some(u32::MAX)] {
        for striped in [false, true] {
            for maximum in [0, 1, 0x7fff, u16::MAX] {
                let info = raw_page_state_info(height, striped, maximum);
                let state = PageState::new(info);
                assert_eq!(state.info, info);
                assert_eq!(state.last_stripe, None);
                assert!(!state.ended);
            }
        }
    }
}

#[test]
fn raw_page_state_stripes_match_wide_boundaries_and_complete_failure_frames() {
    for height in [
        None,
        Some(0),
        Some(1),
        Some(2),
        Some(0x1_0000),
        Some(u32::MAX),
    ] {
        for striped in [false, true] {
            for maximum in [0, 1, 2, 0x7fff, u16::MAX] {
                for previous in [
                    None,
                    Some(0),
                    Some(1),
                    Some(0xffff),
                    Some(u32::MAX - 1),
                    Some(u32::MAX),
                ] {
                    let mut rows = vec![0, 1, 2, 0xffff, 0x1_0000, u32::MAX - 1, u32::MAX];
                    let origin = u64::from(previous.unwrap_or(0));
                    let boundary = origin + u64::from(maximum);
                    for candidate in [
                        origin.saturating_sub(1),
                        origin,
                        origin + 1,
                        boundary.saturating_sub(1),
                        boundary,
                        boundary + 1,
                    ] {
                        if let Ok(row) = u32::try_from(candidate) {
                            rows.push(row);
                        }
                    }
                    if let Some(value) = height {
                        rows.extend([value.saturating_sub(1), value, value.saturating_add(1)]);
                    }
                    for row in rows {
                        for ended in [false, true] {
                            let info = raw_page_state_info(height, striped, maximum);
                            let mut state = PageState {
                                info,
                                last_stripe: previous,
                                ended,
                            };
                            let distance = i64::from(row) - i64::from(previous.unwrap_or(0));
                            let accepted = striped
                                && (previous.is_none() || distance > 0)
                                && distance >= 0
                                && distance <= i64::from(maximum)
                                && height.is_none_or(|value| row < value);
                            let expected = if accepted {
                                Ok(())
                            } else {
                                Err(MalformedError::InvalidStripeEndRow(row))
                            };
                            assert_eq!(
                                raw_page_state_result(state.add_stripe(row)),
                                expected,
                                "{height:?} {striped} {maximum} {previous:?} {row} {ended}"
                            );
                            assert_eq!(state.info, info);
                            assert_eq!(state.ended, ended);
                            assert_eq!(
                                state.last_stripe,
                                if accepted { Some(row) } else { previous }
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn raw_page_state_finish_preserves_frames_and_duplicate_error_priority() {
    for height in [None, Some(0), Some(1), Some(u32::MAX)] {
        for last_stripe in [None, Some(0), Some(1), Some(u32::MAX)] {
            for ended in [false, true] {
                for striped in [false, true] {
                    let info = raw_page_state_info(height, striped, 0);
                    let mut state = PageState {
                        info,
                        last_stripe,
                        ended,
                    };
                    let expected = if ended {
                        Err(MalformedError::DuplicatePageBoundary {
                            page: 0,
                            segment_type: 49,
                        })
                    } else if height.is_none() && last_stripe.is_none() {
                        Err(MalformedError::InvalidStripeEndRow(0))
                    } else {
                        Ok(())
                    };
                    assert_eq!(raw_page_state_result(state.finish()), expected);
                    assert_eq!(state.info, info);
                    assert_eq!(state.last_stripe, last_stripe);
                    assert_eq!(state.ended, ended || expected.is_ok());
                    if expected.is_ok() {
                        assert_eq!(
                            raw_page_state_result(state.finish()),
                            Err(MalformedError::DuplicatePageBoundary {
                                page: 0,
                                segment_type: 49
                            },)
                        );
                        assert_eq!(state.info, info);
                        assert_eq!(state.last_stripe, last_stripe);
                        assert!(state.ended);
                    }
                }
            }
        }
    }
}

#[test]
fn raw_page_state_public_metadata_matches_known_and_wide_inferred_heights() {
    for height in [None, Some(0), Some(1), Some(0x1234_5678), Some(u32::MAX)] {
        for last_stripe in [None, Some(0), Some(1), Some(u32::MAX - 1), Some(u32::MAX)] {
            for ended in [false, true] {
                for striped in [false, true] {
                    for number in [0, 1, 0x0102_0304, u32::MAX] {
                        let info = raw_page_state_info(height, striped, u16::MAX);
                        let state = PageState {
                            info,
                            last_stripe,
                            ended,
                        };
                        let expected_height = match (height, last_stripe) {
                            (Some(value), _) => Ok(value),
                            (None, Some(row)) => u32::try_from(u64::from(row) + 1)
                                .map_err(|_| MalformedError::IntegerOverflow),
                            (None, None) => Err(MalformedError::InvalidStripeEndRow(0)),
                        };
                        assert_eq!(
                            raw_page_state_result(state.into_public(number)),
                            expected_height.map(|value| PageInfo {
                                number,
                                width: info.width,
                                height: value,
                                x_resolution: info.x_resolution,
                                y_resolution: info.y_resolution,
                                striped,
                            })
                        );
                    }
                }
            }
        }
    }
}

type LiteralPayloadWindow = (u32, Vec<u8>, usize, usize);

fn raw_extension_windows(numbers: &[u32], eof: bool) -> (Vec<u8>, Vec<LiteralPayloadWindow>) {
    let mut input = Vec::new();
    let mut expected = Vec::new();
    for (index, number) in numbers.iter().enumerate() {
        let mut payload = 0x100_u32.to_be_bytes().to_vec();
        payload.extend(std::iter::repeat_n(
            u8::try_from(index + 1).unwrap(),
            index + 1,
        ));
        input.extend(header(
            *number,
            62,
            0,
            u32::try_from(payload.len()).unwrap(),
        ));
        let start = input.len();
        input.extend_from_slice(&payload);
        expected.push((*number, payload, start, input.len()));
    }
    if eof {
        input.extend(header(43, 51, 0, 0));
        expected.push((43, Vec::new(), input.len(), input.len()));
    }
    (input, expected)
}

fn raw_random_extension_windows(numbers: &[u32]) -> (Vec<u8>, Vec<LiteralPayloadWindow>) {
    let (_, mut expected) = raw_extension_windows(numbers, false);
    let eof_number = numbers
        .last()
        .map_or(0, |number| number.checked_add(1).unwrap());
    expected.push((eof_number, Vec::new(), 0, 0));
    let mut input = Vec::new();
    for (index, (number, payload, _, _)) in expected.iter().enumerate() {
        input.extend(header(
            *number,
            if index < numbers.len() { 62 } else { 51 },
            0,
            u32::try_from(payload.len()).unwrap(),
        ));
    }
    for (_, payload, start, end) in &mut expected {
        *start = input.len();
        input.extend_from_slice(payload);
        *end = input.len();
    }
    (input, expected)
}

fn quota_view(result: Result<(), DecodeError>) -> Result<Option<LimitExceeded>, MalformedError> {
    match result {
        Ok(()) => Ok(None),
        Err(DecodeError::LimitExceeded(limit)) => Ok(Some(limit)),
        Err(DecodeError::Malformed(error)) => Err(error),
        Err(error) => panic!("unexpected preflight error: {error:?}"),
    }
}

fn wide_quota(
    actual: u128,
    maximum: u128,
    limit: u64,
    kind: LimitKind,
) -> Result<Option<LimitExceeded>, MalformedError> {
    if actual > maximum {
        Err(MalformedError::IntegerOverflow)
    } else if actual > u128::from(limit) {
        Ok(Some(LimitExceeded {
            kind,
            limit,
            actual: u64::try_from(actual).unwrap(),
        }))
    } else {
        Ok(None)
    }
}

const SIGNATURE: [u8; 8] = [0x97, 0x4A, 0x42, 0x32, 0x0D, 0x0A, 0x1A, 0x0A];

fn header(number: u32, segment_type: u8, page: u8, data_length: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&number.to_be_bytes());
    bytes.push(segment_type);
    bytes.push(0);
    bytes.push(page);
    bytes.extend_from_slice(&data_length.to_be_bytes());
    bytes
}

fn header_with_reference(number: u32, segment_type: u8, referred: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&number.to_be_bytes());
    bytes.push(segment_type);
    bytes.push(0x21);
    bytes.push(referred);
    bytes.push(0);
    bytes.extend_from_slice(&0_u32.to_be_bytes());
    bytes
}

fn reference_validation_segment(
    number: u32,
    references: &[u32],
    retention: u8,
) -> ParsedSegment<'static> {
    assert!(references.len() <= 2);
    let mut bytes = header(number, 0, 0, 0);
    bytes[5] = (u8::try_from(references.len()).unwrap() << 5) | retention;
    for (index, referred) in references.iter().enumerate() {
        bytes.insert(6 + index, u8::try_from(*referred).unwrap());
    }
    let mut reader = crate::reader::ByteReader::new(&bytes);
    let header =
        crate::segment_header::parse_segment_header(&mut reader, &Limits::default()).unwrap();
    assert_eq!(reader.position(), bytes.len());
    ParsedSegment {
        header,
        payload: &[],
        data: ParsedData::Other,
    }
}

fn page_data(height: u32) -> [u8; 19] {
    let mut data = [0_u8; 19];
    data[0..4].copy_from_slice(&8_u32.to_be_bytes());
    data[4..8].copy_from_slice(&height.to_be_bytes());
    data
}

fn direct_page_info(height: Option<u32>, striped: bool, maximum: u16) -> PageInformation {
    let mut info = parse_page_information(&page_data(4), &Limits::default()).unwrap();
    info.width = 13;
    info.height = height;
    info.x_resolution = 123;
    info.y_resolution = 456;
    info.striped = striped;
    info.max_stripe_size = maximum;
    info
}

fn valid_sequential_document() -> Vec<u8> {
    let mut bytes = SIGNATURE.to_vec();
    bytes.push(0x01);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&header(0, 48, 1, 19));
    bytes.extend_from_slice(&page_data(8));
    bytes.extend_from_slice(&header(1, 49, 1, 0));
    bytes.extend_from_slice(&header(2, 51, 0, 0));
    bytes
}
