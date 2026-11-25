#![cfg(test)]

use super::*;

fn globals_bytes(retain_contexts: bool) -> Vec<u8> {
    let mut payload = vec![if retain_contexts { 2 } else { 0 }, 0];
    payload.extend_from_slice(&[3, 0xff, 0xfd, 0xff, 2, 0xfe, 0xfe, 0xfe]);
    payload.extend_from_slice(&[0; 8]);
    payload.extend_from_slice(&[0xff, 0xac]);
    let mut bytes = segment_header(0, 0, 0, u32::try_from(payload.len()).unwrap());
    bytes.extend(payload);
    bytes
}

fn embedded_page() -> Vec<u8> {
    let mut bytes = segment_header(1, 48, 1, 19);
    bytes.extend_from_slice(&page_data());
    bytes.extend(segment_header(3, 49, 1, 0));
    bytes
}

#[test]
fn prepared_owner_reuses_exact_resources_and_recovers_after_page_refusal() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&globals_bytes(false)).unwrap();
    let page = embedded_page();
    let expected = decoder.decode_embedded_with_globals(&page, &globals);
    let mut owner = PreparedOwner::new(&decoder, &globals, 65_536);
    for _ in 0..3 {
        assert_outcome_eq!(owner.decode(&page), expected);
    }
    assert_eq!(owner.preparations, 1);
    assert_eq!(owner.hits, 3);
    assert_eq!(owner.fallbacks, 0);
    assert!(owner.resident_bytes > 0);
    assert!(owner.peak_copy_bytes > owner.resident_bytes);
    assert_outcome_eq!(
        owner.decode(&[0]),
        decoder.decode_embedded_with_globals(&[0], &globals)
    );
    assert_outcome_eq!(owner.decode(&page), expected);
    assert_eq!(owner.preparations, 1);
}

#[test]
fn prepared_owner_uses_original_path_for_contexts_or_extra_memory_refusal() {
    for contexts in [false, true] {
        let decoder = Decoder::default();
        let globals = decoder.parse_globals(&globals_bytes(contexts)).unwrap();
        let page = embedded_page();
        let expected = decoder.decode_embedded_with_globals(&page, &globals);
        for budget in [0, 65_536] {
            let mut owner = PreparedOwner::new(&decoder, &globals, budget);
            assert_outcome_eq!(owner.decode(&page), expected);
            assert_outcome_eq!(owner.decode(&page), expected);
            if contexts || budget == 0 {
                assert_eq!(owner.hits, 0);
                assert_eq!(owner.fallbacks, 2);
            }
        }
    }
}

#[test]
fn prepared_owner_drops_resident_state_before_copy_budget_fallback() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&globals_bytes(false)).unwrap();
    let resources = decode_global_resources(globals.bytes(), decoder.limits()).unwrap();
    let resident = resource_bytes(&resources).unwrap();
    drop(resources);
    let page = embedded_page();
    let expected = decoder.decode_embedded_with_globals(&page, &globals);
    let mut owner = PreparedOwner::new(&decoder, &globals, resident);
    for _ in 0..2 {
        assert_outcome_eq!(owner.decode(&page), expected);
    }
    assert_eq!(owner.resident_bytes, 0);
    assert_eq!(owner.hits, 0);
    assert_eq!(owner.fallbacks, 2);
    assert!(matches!(owner.preparation, Preparation::Bypass));
}

#[test]
fn prepared_copy_is_independent_and_counts_resident_and_all_capacities() {
    let limits = Limits::default();
    let mut bitmap = Bitmap::blank(9, 2, &limits).unwrap();
    bitmap.data.copy_from_slice(&[0xaa, 0x80, 0x55, 0]);
    let mut pattern = Bitmap::blank(8, 1, &limits).unwrap();
    pattern.data[0] = 0x42;
    let source = InitialResources {
        symbols: vec![(
            7,
            SymbolResource {
                exported: vec![bitmap],
                contexts: None,
            },
        )],
        patterns: vec![(8, vec![pattern])],
        tables: Vec::new(),
    };
    let resident = resource_bytes(&source).unwrap();
    let mut budget = CopyBudget {
        used: resident,
        limit: 65_536,
    };
    let mut copied = copy_resources(&source, &limits, &mut budget).unwrap();
    assert_eq!(budget.used, resident + resource_bytes(&copied).unwrap());
    assert_eq!(copied.symbols[0].0, 7);
    assert_eq!(source.symbols[0].1.exported, copied.symbols[0].1.exported);
    assert_eq!(source.patterns, copied.patterns);
    copied.symbols[0].1.exported[0].data[0] ^= 0xff;
    copied.symbols.remove(0);
    copied.patterns[0].1[0].data[0] = 0;
    copied.patterns.clear();
    assert_eq!(
        source.symbols[0].1.exported[0].bytes(),
        &[0xaa, 0x80, 0x55, 0]
    );
    assert_eq!(source.patterns[0].1[0].bytes(), &[0x42]);
    let mut too_small = CopyBudget {
        used: resident,
        limit: resident,
    };
    assert!(copy_resources(&source, &limits, &mut too_small).is_err());
}

#[test]
fn prepared_owner_does_not_admit_global_eop_or_reorder_page_errors() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&segment_header(2, 49, 0, 0)).unwrap();
    let mut owner = PreparedOwner::new(&decoder, &globals, 65_536);
    for input in [&[0][..], &embedded_page(), &embedded_page()] {
        let expected = decoder.decode_embedded_with_globals(input, &globals);
        assert!(expected.is_err());
        assert_outcome_eq!(owner.decode(input), expected);
    }
    assert_eq!(owner.hits, 0);
    assert!(matches!(owner.preparation, Preparation::Cold));
    assert_outcome_eq!(
        owner.decode(&embedded_page()),
        Err(DecodeError::Malformed(
            MalformedError::InvalidPageAssociation {
                segment: 2,
                page: 0,
            }
        ))
    );
}

#[test]
fn prepared_owner_never_caches_failed_global_dictionary_validation() {
    let decoder = Decoder::default();
    let mut bytes = globals_bytes(false);
    bytes[11..13].copy_from_slice(&0x8000_u16.to_be_bytes());
    let globals = decoder.parse_globals(&bytes).unwrap();
    let mut owner = PreparedOwner::new(&decoder, &globals, 65_536);
    let page = embedded_page();
    let expected = decoder.decode_embedded_with_globals(&page, &globals);
    assert!(expected.is_err());
    for _ in 0..2 {
        assert_outcome_eq!(owner.decode(&page), expected);
    }
    assert!(matches!(owner.preparation, Preparation::Cold));
    assert_eq!(owner.hits, 0);
}

#[test]
fn prepared_owner_rechecks_each_images_logical_limits() {
    let decoder = Decoder::default();
    let globals = decoder.parse_globals(&globals_bytes(false)).unwrap();
    let page = embedded_page();
    for limits in [
        Limits {
            max_work_units: 1,
            ..Limits::default()
        },
        Limits {
            max_retained_state_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_cumulative_decoded_bitmap_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_decoder_managed_memory_bytes: 0,
            ..Limits::default()
        },
    ] {
        let limited = Decoder::new(limits);
        let expected = limited.decode_embedded_with_globals(&page, &globals);
        assert!(expected.is_err());
        let mut owner = PreparedOwner::new(&limited, &globals, 65_536);
        for _ in 0..2 {
            assert_outcome_eq!(owner.decode(&page), expected);
        }
    }
}
