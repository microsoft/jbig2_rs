#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::*;

#[test]
fn filled_bitmap_enforces_observed_capacity() {
    let mut probe = Vec::<u8>::new();
    probe.try_reserve(1).unwrap();
    let observed = u64::try_from(probe.capacity()).unwrap();
    for value in [false, true] {
        for limit in [0, 1, observed.saturating_sub(1), observed, observed + 1] {
            let limits = Limits {
                max_decoder_managed_memory_bytes: limit,
                ..Limits::default()
            };
            match Bitmap::filled(1, 1, value, &limits) {
                Ok(bitmap) => {
                    assert!(u64::try_from(bitmap.data.capacity()).unwrap() <= limit);
                    assert_eq!(bitmap.bytes(), &[if value { 0x80 } else { 0 }]);
                }
                Err(DecodeError::LimitExceeded(error)) => {
                    assert_eq!(error.kind, LimitKind::Memory);
                    assert_eq!(error.limit, limit);
                    assert_eq!(error.actual, if limit < 1 { 1 } else { observed });
                    assert!(error.actual > limit);
                }
                Err(error) => panic!("unexpected bitmap quota error: {error:?}"),
            }
        }
    }
}

#[test]
fn blank_bitmap_has_expected_layout() {
    let bitmap = Bitmap::blank(9, 2, &Limits::default()).expect("bitmap should fit");
    assert_eq!(bitmap.width(), 9);
    assert_eq!(bitmap.height(), 2);
    assert_eq!(bitmap.stride(), 2);
    assert_eq!(bitmap.bytes(), &[0, 0, 0, 0]);
    assert_eq!(bitmap.pixel(8, 1), Some(false));
    assert_eq!(bitmap.pixel(9, 1), None);
}

#[test]
fn bitmap_enforces_pixel_limit_before_allocation() {
    let limits = Limits {
        max_pixels_per_page: 8,
        ..Limits::default()
    };
    let error = Bitmap::blank(9, 1, &limits).expect_err("bitmap must exceed the limit");
    assert!(matches!(
        error,
        DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::PagePixels,
            limit: 8,
            actual: 9
        })
    ));
}

#[test]
fn filled_bitmap_clears_padding_bits() {
    let bitmap = Bitmap::filled(9, 2, true, &Limits::default()).expect("bitmap should fit");
    assert_eq!(bitmap.bytes(), &[0xFF, 0x80, 0xFF, 0x80]);
    assert_eq!(bitmap.pixel(8, 1), Some(true));
}

#[test]
fn filled_layout_and_padding_are_deterministic() {
    let limits = Limits::default();
    let mut cases = 0;
    for width in 0..=128 {
        for height in 0..=4 {
            for value in [false, true] {
                let bitmap = Bitmap::filled(width, height, value, &limits).expect("bitmap fits");
                let expected_stride = usize::try_from(width.div_ceil(8)).expect("test width fits");
                assert_eq!(bitmap.stride(), expected_stride);
                assert_eq!(
                    bitmap.bytes().len(),
                    expected_stride * usize::try_from(height).unwrap()
                );
                for y in 0..height {
                    for x in 0..width {
                        assert_eq!(bitmap.pixel(x, y), Some(value));
                    }
                    for column in 0..expected_stride {
                        let mut expected = 0_u8;
                        for bit in 0..8 {
                            expected = expected * 2
                                + u8::from(
                                    value && column * 8 + bit < usize::try_from(width).unwrap(),
                                );
                        }
                        assert_eq!(
                            bitmap.bytes()[usize::try_from(y).unwrap() * expected_stride + column],
                            expected,
                            "{width}x{height}, row{y} byte{column}, value{value}",
                        );
                    }
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 1290);
}

#[test]
fn set_pixel_is_local_and_rejects_out_of_bounds() {
    let limits = Limits::default();
    let mut bitmap = Bitmap::blank(9, 2, &limits).expect("bitmap fits");
    bitmap.set_pixel(7, 0, true).expect("pixel is in bounds");
    assert_eq!(bitmap.bytes(), &[0x01, 0, 0, 0]);
    assert_eq!(bitmap.pixel(7, 0), Some(true));
    assert_eq!(bitmap.pixel(6, 0), Some(false));
    assert_eq!(bitmap.pixel(8, 0), Some(false));

    let before = bitmap.bytes().to_vec();
    assert!(matches!(
        bitmap.set_pixel(9, 0, true),
        Err(MalformedError::IntegerOverflow)
    ));
    assert_eq!(bitmap.bytes(), before);
}

#[test]
fn shifted_reads_accept_exactly_available_valid_bit_spans() {
    for length in 0..=18 {
        let bytes: Vec<u8> = (0..length)
            .map(|index| u8::try_from((index * 37 + 19) % 256).unwrap())
            .collect();
        let scalar = |start: usize, shift: u32, count: u32| -> Option<u64> {
            if shift >= 8 {
                return None;
            }
            let first = start
                .checked_mul(8)?
                .checked_add(usize::try_from(shift).unwrap())?;
            let mut result = 0;
            for bit in 0..count {
                let position = first.checked_add(usize::try_from(bit).unwrap())?;
                let byte = bytes.get(position / 8)?;
                result = (result << 1) | u64::from((byte >> (7 - position % 8)) & 1);
            }
            Some(result)
        };
        for start in (0..=length).chain([usize::MAX]) {
            for shift in 0..=8 {
                assert_eq!(
                    read_shifted_byte(&bytes, start, shift).map(u64::from),
                    scalar(start, shift, 8)
                );
                assert_eq!(
                    read_shifted_chunk(&bytes, start, shift),
                    scalar(start, shift, 64)
                );
                for count in 1..=8 {
                    assert_eq!(
                        read_shifted_tail(&bytes, start, shift, count)
                            .map(|value| u64::from(value >> (8 - count))),
                        scalar(start, shift, count),
                    );
                }
            }
        }
    }
}

#[test]
fn checked_layout_enforces_each_bitmap_limit() {
    let width_limits = Limits {
        max_width: 8,
        ..Limits::default()
    };
    assert!(matches!(
        checked_bitmap_layout(9, 1, &width_limits),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Width,
            limit: 8,
            actual: 9,
        }))
    ));

    let memory_limits = Limits {
        max_decoder_managed_memory_bytes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        checked_bitmap_layout(9, 1, &memory_limits),
        Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Memory,
            limit: 1,
            actual: 2,
        }))
    ));
}

#[test]
fn prefix_copy_preserves_suffix_source_and_error_state() {
    for (source_height, destination_height) in [
        (0, 0),
        (0, 9),
        (9, 0),
        (3, 7),
        (7, 3),
        (66, 129),
        (129, 66),
        (257, 1024),
    ] {
        let source = patterned_bitmap(8, source_height, 17);
        let original = patterned_bitmap(8, destination_height, 29);
        let immutable = source.clone();
        let limit = source.data.len().min(original.data.len());
        for count in 0..=limit {
            let mut expected = original.clone();
            expected.data[..count].copy_from_slice(&source.data[..count]);
            let mut actual = original.clone();
            actual.copy_prefix_from(&source, count).unwrap();
            assert_eq!(
                actual, expected,
                "source {source_height}, destination {destination_height}, count {count}"
            );
            assert_eq!(source, immutable);
        }
        for count in [
            limit + 1,
            source.data.len().max(original.data.len()) + 1,
            usize::MAX,
        ] {
            let mut actual = original.clone();
            assert!(matches!(
                actual.copy_prefix_from(&source, count),
                Err(MalformedError::IntegerOverflow)
            ));
            assert_eq!(actual, original);
            assert_eq!(source, immutable);
        }
    }
}

#[test]
fn blank_storage_is_zero_for_empty_and_partial_rows() {
    for width in [0, 1, 7, 8, 9, 63, 64, 65] {
        for height in [0, 1, 3] {
            let bitmap = Bitmap::blank(width, height, &Limits::default()).unwrap();
            assert_eq!((bitmap.width(), bitmap.height()), (width, height));
            assert!(bitmap.bytes().iter().all(|byte| *byte == 0));
            for y in 0..height {
                for x in 0..width {
                    assert_eq!(bitmap.pixel(x, y), Some(false));
                }
            }
        }
    }
}

#[test]
fn chunk_values_match_independent_big_endian_oracles() {
    let mut values = vec![0, u64::MAX, 0x0123_4567_89ab_cdef];
    for offset in 0..8 {
        for byte in 0..=u8::MAX {
            values.push(u64::from(byte) << (offset * 8));
        }
    }
    for value in values {
        let encoded = value.to_be_bytes();
        for start in 0..=9 {
            let mut bytes = vec![0xa5; 25];
            let mut expected = bytes.clone();
            expected[start..start + 8].copy_from_slice(&encoded);
            assert!(write_chunk(&mut bytes, start, value));
            assert_eq!(bytes, expected);
            assert_eq!(read_chunk(&bytes, start), Some(value));
            assert_eq!(
                read_chunk(&expected, start),
                Some(u64::from_be_bytes(encoded)),
            );
        }
    }
}

#[test]
fn chunk_access_accepts_exactly_complete_ranges_and_preserves_failed_writes() {
    for length in 0..=25 {
        let original: Vec<u8> = (0..length)
            .map(|index| u8::try_from((index * 37 + 19) % 256).expect("byte"))
            .collect();
        for start in (0..=length).chain([usize::MAX - 7, usize::MAX]) {
            let range = start
                .checked_add(8)
                .filter(|&end| end <= length)
                .map(|end| start..end);
            let expected_read = range.as_ref().map(|range| {
                u64::from_be_bytes(
                    original[range.clone()]
                        .try_into()
                        .expect("exactly eight bytes"),
                )
            });
            assert_eq!(read_chunk(&original, start), expected_read);
            for value in [0, u64::MAX, 0x0123_4567_89ab_cdef] {
                let mut bytes = original.clone();
                let mut expected = original.clone();
                if let Some(range) = &range {
                    expected[range.clone()].copy_from_slice(&value.to_be_bytes());
                }
                assert_eq!(write_chunk(&mut bytes, start, value), range.is_some());
                assert_eq!(bytes, expected);
            }
        }
    }
}

#[test]
fn row_copy_bytes_and_chunk_access_have_exact_boundaries() {
    for length in [0, 1, 2, 8, 64, 65, 66, 127, 128, 129, 257] {
        let source: Vec<u8> = (0..length)
            .map(|index| u8::try_from(index % 251).unwrap())
            .collect();
        let mut destination = vec![0xa5; length];
        copy_row_bytes(&mut destination, &source);
        assert_eq!(destination, source);
    }
    let mut bytes = vec![0x81; 8];
    let original = bytes.clone();
    for start in [1, usize::MAX - 7, usize::MAX] {
        assert_eq!(read_chunk(&bytes, start), None);
        assert!(!write_chunk(&mut bytes, start, u64::MAX));
        assert_eq!(bytes, original);
    }
    assert_eq!(read_shifted_byte(&bytes, 0, 8), None);
    assert_eq!(read_shifted_tail(&bytes, 0, 1, 0), None);
    assert_eq!(read_shifted_tail(&bytes, 0, 1, 9), None);
    assert_eq!(read_shifted_chunk(&bytes, 0, 8), None);
}

#[test]
fn row_copy_rejects_invalid_indices_and_storage_without_mutation() {
    for (source, destination) in [(2, 0), (0, 2), (2, 2)] {
        let mut bitmap = Bitmap::blank(8, 2, &Limits::default()).unwrap();
        let original = bitmap.clone();
        assert!(matches!(
            bitmap.copy_row(source, destination),
            Err(MalformedError::IntegerOverflow)
        ));
        assert_eq!(bitmap, original);
    }
    for (stride, source, destination) in [
        (usize::MAX, 2, 0),
        (usize::MAX, 0, 2),
        (usize::MAX, 1, 0),
        (usize::MAX, 0, 1),
        (1, 1, 0),
        (1, 0, 1),
    ] {
        let mut bitmap = Bitmap {
            width: 8,
            height: 3,
            stride,
            data: vec![0xa5],
        };
        let original = bitmap.clone();
        assert!(matches!(
            bitmap.copy_row(source, destination),
            Err(MalformedError::IntegerOverflow)
        ));
        assert_eq!(bitmap, original);
    }
}

#[test]
fn black_spans_preserve_arbitrary_padding_and_bounds_errors() {
    for width in [1_u32, 7, 9, 65, 73, 129, 137] {
        for seed in [0_u8, 0xFF, 0xA5, 0x5A] {
            let mut original = patterned_bitmap(width, 3, u64::from(width));
            original.data.fill(seed);
            for row in 0..3 {
                for start in 0..=width {
                    for end in start..=width {
                        let mut actual = original.clone();
                        let mut expected = original.clone();
                        actual.fill_black_span(row, start, end).unwrap();
                        scalar_fill_black_span(&mut expected, row, start, end).unwrap();
                        assert_eq!(actual, expected, "{width}: {row}, {start}..{end}, {seed}");
                    }
                }
            }
            for (row, start, end) in [(3, 0, 0), (0, 1, 0), (0, 0, width + 1)] {
                let mut actual = original.clone();
                assert_eq!(
                    actual.fill_black_span(row, start, end),
                    Err(MalformedError::IntegerOverflow),
                );
                assert_eq!(actual, original);
            }
        }
    }
}

#[test]
fn chunk_span_fill_and_row_copy_match_scalar_oracles() {
    for width in 0_u32..=130 {
        let original = patterned_bitmap(width, 4, u64::from(width));
        for row in 0_u32..4 {
            for start in 0..=width {
                for end in start..=width {
                    let mut expected = original.clone();
                    let mut actual = original.clone();
                    scalar_fill_black_span(&mut expected, row, start, end)
                        .expect("scalar fill succeeds");
                    actual
                        .fill_black_span(row, start, end)
                        .expect("chunk fill succeeds");
                    assert_eq!(actual, expected, "width {width}, row {row}, {start}..{end}");
                }
            }
            for destination in 0_u32..4 {
                let mut expected = original.clone();
                let mut actual = original.clone();
                scalar_copy_row(&mut expected, row, destination).expect("scalar copy succeeds");
                actual
                    .copy_row(row, destination)
                    .expect("chunk copy succeeds");
                assert_eq!(
                    actual, expected,
                    "width {width}, source {row}, destination {destination}"
                );
            }
            let source = patterned_bitmap(width, 4, !u64::from(width));
            for byte_count in 0..=original.data.len() {
                let mut expected = original.clone();
                let mut actual = original.clone();
                expected.data[..byte_count].copy_from_slice(&source.data[..byte_count]);
                actual
                    .copy_prefix_from(&source, byte_count)
                    .expect("chunk prefix copy succeeds");
                assert_eq!(actual, expected, "width {width}, byte count {byte_count}");
            }
        }
    }
}
#[test]
fn zero_storage_check_rejects_nonzero_bytes_including_padding() {
    for width in [0, 1, 7, 8, 9, 65] {
        let mut bitmap = Bitmap::blank(width, 3, &Limits::default()).unwrap();
        assert!(bitmap.is_zeroed_storage());
        for index in 0..bitmap.data.len() {
            bitmap.data[index] = 1;
            assert!(!bitmap.is_zeroed_storage());
            bitmap.data[index] = 0;
        }
        assert!(bitmap.is_zeroed_storage());
    }
}
