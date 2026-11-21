//! Project-generated envelope cases with literal metadata, pixels and typed errors.

#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

#[path = "public_envelope/embedded_policy.rs"]
mod embedded_policy;
#[path = "public_envelope/metadata_extensions.rs"]
mod metadata_extensions;
#[path = "public_envelope/organization_and_limits.rs"]
mod organization_and_limits;
#[path = "public_envelope/page_lifecycle.rs"]
mod page_lifecycle;
#[path = "public_envelope/references.rs"]
mod references;
#[path = "public_envelope/standalone_policy.rs"]
mod standalone_policy;

use std::io::Cursor;

use jbig2_decoder::{
    DecodeError, Decoder, DocumentInfo, HuffmanErrorKind, LimitExceeded, LimitKind, Limits,
    MalformedError, Organization, Page, PageSink, UnsupportedFeature,
};

#[allow(dead_code)]
#[path = "../tools/distribution_consumer.rs"]
mod fixture;

#[derive(Clone)]
struct Segment {
    number: u32,
    kind: u8,
    page: u32,
    retention: u8,
    references: Vec<u8>,
    payload: Vec<u8>,
    unknown_length: bool,
    deferred: bool,
}

impl Segment {
    fn new(number: u32, kind: u8, page: u32, payload: Vec<u8>) -> Self {
        Self {
            number,
            kind,
            page,
            retention: 1,
            references: Vec::new(),
            payload,
            unknown_length: false,
            deferred: false,
        }
    }

    fn references(mut self, numbers: &[u8], retention: u8) -> Self {
        self.references = numbers.to_vec();
        self.retention = retention;
        self
    }

    fn retaining_references(self, numbers: &[u8]) -> Self {
        let retention = if numbers.len() <= 4 {
            u8::try_from((1_u16 << (numbers.len() + 1)) - 1).unwrap()
        } else {
            u8::MAX
        };
        self.references(numbers, retention)
    }

    fn header(&self) -> Vec<u8> {
        assert!(self.references.len() <= 256);
        assert!(self.references.is_empty() || self.number <= 256);
        let mut bytes = self.number.to_be_bytes().to_vec();
        let long_page = self.page > 255;
        bytes.push(self.kind | (u8::from(long_page) << 6) | (u8::from(self.deferred) << 7));
        if self.references.len() <= 4 {
            bytes.push((u8::try_from(self.references.len()).unwrap() << 5) | self.retention);
        } else {
            assert_eq!(self.retention, u8::MAX);
            bytes.extend_from_slice(
                &(0xe000_0000 | u32::try_from(self.references.len()).unwrap()).to_be_bytes(),
            );
            let flag_count = self.references.len() + 1;
            for first in (0..flag_count).step_by(8) {
                let count = (flag_count - first).min(8);
                bytes.push(u8::try_from((1_u16 << count) - 1).unwrap());
            }
        }
        bytes.extend_from_slice(&self.references);
        if long_page {
            bytes.extend_from_slice(&self.page.to_be_bytes());
        } else {
            bytes.push(u8::try_from(self.page).unwrap());
        }
        let length = if self.unknown_length {
            u32::MAX
        } else {
            u32::try_from(self.payload.len()).unwrap()
        };
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes
    }
}

fn embedded(segments: &[Segment]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for segment in segments {
        bytes.extend(segment.header());
        bytes.extend_from_slice(&segment.payload);
    }
    bytes
}

fn standalone(sequential: bool, pages: Option<u32>, segments: &[Segment]) -> Vec<u8> {
    let mut bytes = b"\x97JB2\r\n\x1a\n".to_vec();
    bytes.push(u8::from(sequential) | (u8::from(pages.is_none()) << 1));
    if let Some(count) = pages {
        bytes.extend_from_slice(&count.to_be_bytes());
    }
    if sequential {
        bytes.extend(embedded(segments));
    } else {
        for segment in segments {
            bytes.extend(segment.header());
        }
        for segment in segments {
            bytes.extend_from_slice(&segment.payload);
        }
    }
    bytes
}

fn page(number: u32, association: u32, width: u32, height: u32) -> Segment {
    let mut payload = width.to_be_bytes().to_vec();
    payload.extend_from_slice(&height.to_be_bytes());
    payload.extend_from_slice(&300_u32.to_be_bytes());
    payload.extend_from_slice(&600_u32.to_be_bytes());
    payload.extend_from_slice(&[0, 0, 0]);
    Segment::new(number, 48, association, payload)
}

fn striped_page(number: u32, height: u32, maximum: u16) -> Segment {
    let mut segment = page(number, 1, 9, height);
    segment.payload[17..19].copy_from_slice(&(0x8000 | maximum).to_be_bytes());
    segment
}

fn end(number: u32, association: u32) -> Segment {
    Segment::new(number, 49, association, Vec::new())
}

fn eof(number: u32) -> Segment {
    Segment::new(number, 51, 0, Vec::new())
}

fn reference_payload(kind: u8) -> Vec<u8> {
    match kind {
        48 => page(0, 1, 8, 1).payload,
        50 | 52 | 62 => vec![0; 4],
        53 => vec![0x03, 0, 0, 0, 0, 0, 0, 0, 1, 0b0100_0101, 0],
        _ => Vec::new(),
    }
}

fn assert_reference_metadata(segments: &[Segment]) {
    let decoder = Decoder::default();
    let count = u32::try_from(segments.len()).unwrap();
    for sequential in [true, false] {
        let bytes = standalone(sequential, Some(1), segments);
        let info = decoder.inspect(&bytes).unwrap();
        assert_info(
            &info,
            if sequential {
                Organization::Sequential
            } else {
                Organization::RandomAccess
            },
            Some(1),
            count,
            &[(1, 8, 1, 300, 600, false)],
        );
        assert_eq!(
            decoder.inspect_reader(&mut Cursor::new(&bytes)).unwrap(),
            info
        );
    }
    let info = decoder.inspect_embedded(&embedded(segments)).unwrap();
    assert_info(
        &info,
        Organization::Sequential,
        None,
        count,
        &[(1, 8, 1, 300, 600, false)],
    );
}

fn stripe(number: u32, row: u32) -> Segment {
    Segment::new(number, 50, 1, row.to_be_bytes().to_vec())
}

fn profiles(number: u32, association: u32, values: &[u32]) -> Segment {
    let mut payload = u32::try_from(values.len()).unwrap().to_be_bytes().to_vec();
    for value in values {
        payload.extend_from_slice(&value.to_be_bytes());
    }
    Segment::new(number, 52, association, payload)
}

fn extension(number: u32, association: u32, kind: u32, data: &[u8]) -> Segment {
    let mut payload = kind.to_be_bytes().to_vec();
    payload.extend_from_slice(data);
    Segment::new(number, 62, association, payload)
}

type Metadata = (u32, u32, u32, u32, u32, bool);

fn assert_info(
    info: &DocumentInfo,
    organization: Organization,
    declared: Option<u32>,
    count: u32,
    expected: &[Metadata],
) {
    assert_eq!(info.organization(), organization);
    assert_eq!(info.declared_page_count(), declared);
    assert_eq!(info.segment_count(), count);
    let actual: Vec<_> = info
        .pages()
        .iter()
        .map(|page| {
            (
                page.number(),
                page.width(),
                page.height(),
                page.x_resolution(),
                page.y_resolution(),
                page.is_striped(),
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

fn assert_pixels(page: &Page, number: u32, width: u32, height: u32, bytes: &[u8]) {
    assert_eq!(page.number(), number);
    assert_eq!(page.bitmap().width(), width);
    assert_eq!(page.bitmap().height(), height);
    assert_eq!(page.bitmap().stride(), width.div_ceil(8) as usize);
    assert_eq!(page.bitmap().bytes(), bytes);
}

fn assert_malformed(error: DecodeError, expected: &MalformedError) {
    match error {
        DecodeError::Malformed(actual) => assert_eq!(&actual, expected),
        other => panic!("expected {expected:?}, got {other:?}"),
    }
}

fn assert_limit(error: DecodeError, kind: LimitKind, limit: u64, actual: u64) {
    match error {
        DecodeError::LimitExceeded(detail) => {
            assert_eq!(
                detail,
                LimitExceeded {
                    kind,
                    limit,
                    actual
                }
            );
        }
        other => panic!("expected {kind:?} limit {limit}, actual {actual}; got {other:?}"),
    }
}

fn reject(segments: &[Segment], expected: &MalformedError) {
    let decoder = Decoder::default();
    for sequential in [true, false] {
        let bytes = standalone(sequential, None, segments);
        assert_malformed(decoder.inspect(&bytes).unwrap_err(), expected);
        assert_malformed(decoder.decode(&bytes).unwrap_err(), expected);
        let mut calls = 0;
        let error = decoder
            .decode_with_sink(&bytes, &mut |_| {
                calls += 1;
                Ok(())
            })
            .unwrap_err();
        assert_malformed(error, expected);
        assert_eq!(calls, 0, "envelope rejection precedes page delivery");
    }
    let bytes = embedded(segments);
    assert_malformed(decoder.inspect_embedded(&bytes).unwrap_err(), expected);
    assert_malformed(decoder.decode_embedded(&bytes).unwrap_err(), expected);
    let (valid, _) = fixture::generated_page();
    let recovered = decoder.decode(&valid).unwrap();
    assert_pixels(&recovered.pages()[0], 1, 8, 1, &[0]);
}
