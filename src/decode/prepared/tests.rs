#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

use super::copying::{copy_bitmap, copy_bitmaps, copy_failure, CopyBudget, CopyFailure};
use super::*;
use crate::Bitmap;

fn header(number: u32, kind: u8, page: u8, length: u32) -> Vec<u8> {
    let mut bytes = number.to_be_bytes().to_vec();
    bytes.extend([kind, 0, page]);
    bytes.extend(length.to_be_bytes());
    bytes
}

fn global_dictionary(retain_contexts: bool) -> Vec<u8> {
    let mut payload = vec![if retain_contexts { 2 } else { 0 }, 0];
    payload.extend([3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    payload.extend([0; 8]);
    payload.extend([0xff, 0xac]);
    let mut bytes = header(0, 0, 0, u32::try_from(payload.len()).unwrap());
    bytes.extend(payload);
    bytes
}

fn page() -> Vec<u8> {
    let mut bytes = header(1, 48, 1, 19);
    let mut payload = [0; 19];
    payload[..4].copy_from_slice(&8_u32.to_be_bytes());
    payload[4..8].copy_from_slice(&1_u32.to_be_bytes());
    bytes.extend(payload);
    bytes.extend(header(3, 49, 1, 0));
    bytes
}

fn assert_same(
    actual: Result<DecodedDocument, DecodeError>,
    expected: Result<DecodedDocument, DecodeError>,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (Err(DecodeError::Malformed(a)), Err(DecodeError::Malformed(b))) => assert_eq!(a, b),
        (Err(DecodeError::Unsupported(a)), Err(DecodeError::Unsupported(b))) => assert_eq!(a, b),
        (Err(DecodeError::LimitExceeded(a)), Err(DecodeError::LimitExceeded(b))) => {
            assert_eq!(a, b);
        }
        (
            Err(DecodeError::Allocation { requested_bytes: a }),
            Err(DecodeError::Allocation { requested_bytes: b }),
        ) => assert_eq!(a, b),
        (Err(DecodeError::Io(a)), Err(DecodeError::Io(b))) => assert_eq!(a, b),
        (a, b) => panic!("different outcomes: {a:?}, {b:?}"),
    }
}

#[test]
fn public_prepared_owner_is_lazy_bound_and_resettable() {
    let limits = Limits {
        max_width: 321,
        max_height: 234,
        max_decoder_managed_memory_bytes: 900_000,
        max_retained_state_bytes: 800_000,
        ..Limits::default()
    };
    let decoder = Decoder::new(limits);
    let globals = decoder.parse_globals(&global_dictionary(false)).unwrap();
    let mut owner = decoder.prepared_globals(&globals, u64::MAX);
    assert_eq!(
        owner.info(),
        PreparedGlobalsInfo {
            state: PreparedGlobalsState::Pending,
            additional_storage_bytes: 800_000,
            resident_bytes: 0,
        }
    );
    for _ in 0..3 {
        assert_same(
            owner.decode_embedded(&page()),
            decoder.decode_embedded_with_globals(&page(), &globals),
        );
        assert_eq!(owner.info().state, PreparedGlobalsState::Ready);
        assert!(owner.info().resident_bytes > 0);
        assert!(owner.info().resident_bytes <= 800_000);
    }
    let ready = owner.info();
    assert_same(
        owner.decode_embedded(&[0]),
        decoder.decode_embedded_with_globals(&[0], &globals),
    );
    assert_eq!(owner.info(), ready);
    owner.reset();
    assert_eq!(owner.info().state, PreparedGlobalsState::Pending);
    assert_eq!(owner.info().resident_bytes, 0);
    assert_eq!(owner.info().additional_storage_bytes, 800_000);
    assert_same(
        owner.decode_embedded(&page()),
        decoder.decode_embedded_with_globals(&page(), &globals),
    );
    assert_eq!(owner.info().state, PreparedGlobalsState::Ready);
}

#[test]
fn public_prepared_owner_reports_supported_optimization_refusals() {
    let decoder = Decoder::default();
    for contexts in [false, true] {
        let globals = decoder.parse_globals(&global_dictionary(contexts)).unwrap();
        for allowance in [0, 65_536] {
            let mut owner = decoder.prepared_globals(&globals, allowance);
            for _ in 0..2 {
                assert_same(
                    owner.decode_embedded(&page()),
                    decoder.decode_embedded_with_globals(&page(), &globals),
                );
            }
            let expected = if contexts {
                PreparedGlobalsState::Uncached(PreparedGlobalsBypass::RetainedContexts)
            } else if allowance == 0 {
                PreparedGlobalsState::Uncached(PreparedGlobalsBypass::AdditionalStorage)
            } else {
                PreparedGlobalsState::Ready
            };
            assert_eq!(owner.info().state, expected);
        }
    }
    let empty = decoder.parse_globals(&[]).unwrap();
    let mut owner = decoder.prepared_globals(&empty, 65_536);
    owner.decode_embedded(&page()).unwrap();
    assert_eq!(
        owner.info().state,
        PreparedGlobalsState::Uncached(PreparedGlobalsBypass::NoResources)
    );
}

#[test]
fn public_prepared_owner_preserves_global_refusal_and_page_error_priority() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&header(2, 49, 0, 0)).unwrap();
    let mut owner = decoder.prepared_globals(&globals, 65_536);
    for input in [&[0][..], &page(), &page()] {
        assert_same(
            owner.decode_embedded(input),
            decoder.decode_embedded_with_globals(input, &globals),
        );
        assert_eq!(owner.info().state, PreparedGlobalsState::Pending);
        assert_eq!(owner.info().resident_bytes, 0);
    }
    assert!(matches!(
        owner.decode_embedded(&page()),
        Err(DecodeError::Malformed(
            MalformedError::InvalidPageAssociation {
                segment: 2,
                page: 0
            }
        ))
    ));
}

#[test]
fn production_prepared_copies_exact_packed_frames_and_ids() {
    let limits = Limits::default();
    let mut bitmap = Bitmap::blank(9, 2, &limits).unwrap();
    bitmap.data.copy_from_slice(&[0xaa, 0x80, 0x55, 0]);
    let mut budget = CopyBudget {
        used: 123,
        limit: 65_536,
    };
    let mut copied = copy_bitmap(&bitmap, &limits, &mut budget).unwrap();
    assert_eq!(bitmap, copied);
    assert!(budget.used >= 127);
    copied.data[0] = 0;
    assert_eq!(bitmap.bytes(), &[0xaa, 0x80, 0x55, 0]);

    let mut pattern = Bitmap::blank(8, 1, &limits).unwrap();
    pattern.data[0] = 0x42;
    let source = InitialResources {
        symbols: vec![(
            7,
            super::super::SymbolResource {
                exported: vec![bitmap],
                contexts: None,
            },
        )],
        patterns: vec![(8, vec![pattern])],
        tables: Vec::new(),
    };
    let resident = capacity_bytes(&source).unwrap();
    let expected_capacity = source.symbols.capacity()
        * core::mem::size_of::<(u32, super::super::SymbolResource)>()
        + source.patterns.capacity() * core::mem::size_of::<(u32, Vec<Bitmap>)>()
        + source.tables.capacity() * core::mem::size_of::<(u32, crate::huffman::HuffmanTable)>()
        + source.symbols[0].1.exported.capacity() * core::mem::size_of::<Bitmap>()
        + source.symbols[0].1.exported[0].data.capacity()
        + source.patterns[0].1.capacity() * core::mem::size_of::<Bitmap>()
        + source.patterns[0].1[0].data.capacity();
    assert_eq!(resident, u64::try_from(expected_capacity).unwrap());
    let mut copied = copy_resources(&source, &limits, resident, 65_536).unwrap();
    assert_eq!(copied.symbols[0].0, 7);
    assert_eq!(copied.symbols[0].1.exported, source.symbols[0].1.exported);
    assert!(copied.symbols[0].1.contexts.is_none());
    assert_eq!(copied.patterns, source.patterns);
    copied.symbols.clear();
    copied.patterns[0].1[0].data[0] = 0;
    assert_eq!(
        source.symbols[0].1.exported[0].bytes(),
        &[0xaa, 0x80, 0x55, 0]
    );
    assert_eq!(source.patterns[0].1[0].bytes(), &[0x42]);
    assert!(matches!(
        copy_resources(&source, &limits, resident, resident),
        Err(CopyFailure::Optional(
            PreparedGlobalsBypass::AdditionalStorage
        ))
    ));
    assert_eq!(source.patterns[0].1[0].bytes(), &[0x42]);
}

#[test]
fn prepared_partial_copy_refusal_leaves_source_and_budget_valid() {
    let limits = Limits::default();
    let mut first = Bitmap::blank(8, 1, &limits).unwrap();
    first.data[0] = 0xa5;
    let mut second = Bitmap::blank(8, 1, &limits).unwrap();
    second.data[0] = 0x5a;
    let source = [first, second];
    let mut metadata: Vec<Bitmap> = Vec::new();
    metadata.try_reserve(2).unwrap();
    let mut data: Vec<u8> = Vec::new();
    data.try_reserve(1).unwrap();
    let allowance =
        u64::try_from(metadata.capacity() * core::mem::size_of::<Bitmap>() + data.capacity())
            .unwrap();
    let mut budget = CopyBudget {
        used: 0,
        limit: allowance,
    };
    assert!(matches!(
        copy_bitmaps(&source, &limits, &mut budget),
        Err(CopyFailure::Optional(
            PreparedGlobalsBypass::AdditionalStorage
        ))
    ));
    assert_eq!(source[0].bytes(), &[0xa5]);
    assert_eq!(source[1].bytes(), &[0x5a]);
    assert!(budget.used > 0);
    assert!(budget.used <= allowance);
}

#[test]
fn prepared_budget_is_inclusive_and_never_suppresses_internal_errors() {
    let budget = CopyBudget {
        used: 13,
        limit: 21,
    };
    assert!(budget.check(8).is_ok());
    assert!(matches!(
        budget.check(9),
        Err(CopyFailure::Optional(
            PreparedGlobalsBypass::AdditionalStorage
        ))
    ));
    let overflow = CopyBudget {
        used: u64::MAX,
        limit: u64::MAX,
    };
    assert!(matches!(
        overflow.check(1),
        Err(CopyFailure::Decode(DecodeError::Malformed(
            MalformedError::IntegerOverflow
        )))
    ));
    assert!(matches!(
        copy_failure(DecodeError::Allocation {
            requested_bytes: 17
        }),
        CopyFailure::Optional(PreparedGlobalsBypass::CopyAllocation)
    ));
    assert!(matches!(
        copy_failure(DecodeError::Malformed(MalformedError::IntegerOverflow)),
        CopyFailure::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow))
    ));
}

#[test]
fn prepared_copy_refusal_transfers_the_original_cache_before_page_mutation() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&global_dictionary(false)).unwrap();
    let mut owner = decoder.prepared_globals(&globals, 65_536);
    drop(owner.image_resources().unwrap());
    assert_eq!(owner.info().state, PreparedGlobalsState::Ready);
    let original = owner.resources.as_ref().unwrap().symbols.as_ptr();
    let original_number = owner.resources.as_ref().unwrap().symbols[0].0;
    // A valid internal equality-boundary state forces the optional copy refusal.
    owner.additional_storage_bytes = owner.resident_bytes;
    let resources = owner.image_resources().unwrap();
    assert_eq!(resources.symbols.as_ptr(), original);
    assert_eq!(resources.symbols[0].0, original_number);
    assert_eq!(
        owner.info().state,
        PreparedGlobalsState::Uncached(PreparedGlobalsBypass::AdditionalStorage)
    );
    assert_eq!(owner.info().resident_bytes, 0);
    assert!(owner.resources.is_none());
    drop(resources);
    assert_same(
        owner.decode_embedded(&page()),
        decoder.decode_embedded_with_globals(&page(), &globals),
    );
    assert_eq!(
        owner.info().state,
        PreparedGlobalsState::Uncached(PreparedGlobalsBypass::AdditionalStorage)
    );
}

#[test]
fn prepared_admission_checks_contexts_tables_and_real_empty_resources() {
    use crate::huffman::{HuffmanTable, StandardHuffmanTable};
    let decoder = Decoder::default();
    let mut resources =
        decode_global_resources(&global_dictionary(true), decoder.limits()).unwrap();
    assert_eq!(
        admission(&resources),
        Some(PreparedGlobalsBypass::RetainedContexts)
    );
    resources.tables.push((
        19,
        HuffmanTable::standard(StandardHuffmanTable::A, decoder.limits()).unwrap(),
    ));
    assert_eq!(
        admission(&resources),
        Some(PreparedGlobalsBypass::CustomTables)
    );
    assert_eq!(
        admission(&InitialResources::default()),
        Some(PreparedGlobalsBypass::NoResources)
    );
}
