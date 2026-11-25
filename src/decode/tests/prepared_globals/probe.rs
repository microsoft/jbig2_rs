#![cfg(test)]

use super::*;
use std::path::PathBuf;

struct Fixtures {
    globals: GlobalSegments,
    pages: [Vec<u8>; 3],
    original: Vec<u8>,
    refused_globals: GlobalSegments,
    refused_pages: [Vec<u8>; 3],
}

impl Fixtures {
    fn read(decoder: &Decoder) -> Self {
        let root = PathBuf::from(
            std::env::var_os("JBIG2_PREPARED_FIXTURE_DIR").expect("set fixture directory"),
        );
        let read = |name: &str| std::fs::read(root.join(name)).expect("read pinned fixture");
        Self {
            globals: decoder.parse_globals(&read("file_pages.globals")).unwrap(),
            pages: std::array::from_fn(|index| read(&format!("file_pages-page{}.jb2", index + 1))),
            original: read("file_pages.jbig2"),
            refused_globals: decoder
                .parse_globals(&read("lenient_end_of_page_of_previous_page.globals"))
                .unwrap(),
            refused_pages: std::array::from_fn(|index| {
                read(&format!(
                    "lenient_end_of_page_of_previous_page-page{}.jb2",
                    index + 1
                ))
            }),
        }
    }

    fn expected(&self, decoder: &Decoder) -> [DecodedDocument; 3] {
        let original = decoder.decode(&self.original).unwrap();
        assert_eq!(original.pages().len(), 3);
        std::array::from_fn(|index| {
            let embedded = decoder
                .decode_embedded_with_globals(&self.pages[index], &self.globals)
                .unwrap();
            assert_eq!(embedded.pages().len(), 1);
            assert_eq!(
                embedded.pages()[0].bitmap(),
                original.pages()[index].bitmap()
            );
            embedded
        })
    }
}

// Each field is exercised at its baseline success frontier, including both
// inclusive-boundary neighbours, without changing production accounting.
fn set_limit(limits: &mut Limits, index: usize, value: u64) {
    let narrow = u32::try_from(value).unwrap_or(u32::MAX);
    match index {
        0 => limits.max_width = narrow,
        1 => limits.max_height = narrow,
        2 => limits.max_pixels_per_page = value,
        3 => limits.max_decoder_managed_memory_bytes = value,
        4 => limits.max_stream_bytes = value,
        5 => limits.max_pages = narrow,
        6 => limits.max_segments = narrow,
        7 => limits.max_symbols = narrow,
        8 => limits.max_references_per_segment = narrow,
        9 => limits.max_total_references = value,
        10 => limits.max_reference_graph_depth = narrow,
        11 => limits.max_nesting_depth = narrow,
        12 => limits.max_work_units = value,
        13 => limits.max_cumulative_decoded_bitmap_bytes = value,
        14 => limits.max_retained_state_bytes = value,
        _ => unreachable!(),
    }
}

#[test]
#[ignore = "external pinned fixtures; correctness only, no timing"]
fn prepared_globals_fixture_controls() {
    let decoder = Decoder::default();
    let fixtures = Fixtures::read(&decoder);
    let expected = fixtures.expected(&decoder);
    let mut owner = PreparedOwner::new(&decoder, &fixtures.globals, 65_536);
    let mut public = decoder.prepared_globals(&fixtures.globals, 65_536);
    for _ in 0..2 {
        for (page, expected) in fixtures.pages.iter().zip(&expected) {
            assert_eq!(owner.decode(page).unwrap(), *expected);
            assert_eq!(public.decode_embedded(page).unwrap(), *expected);
        }
    }
    assert_eq!(owner.preparations, 1);
    assert_eq!(owner.hits, 6);
    assert_eq!(owner.fallbacks, 0);
    assert_eq!(public.info().state, crate::PreparedGlobalsState::Ready);
    for page in &fixtures.pages {
        for cut in [0, 1, page.len() / 2, page.len() - 1] {
            assert_outcome_eq!(
                owner.decode(&page[..cut]),
                decoder.decode_embedded_with_globals(&page[..cut], &fixtures.globals)
            );
            assert_outcome_eq!(
                public.decode_embedded(&page[..cut]),
                decoder.decode_embedded_with_globals(&page[..cut], &fixtures.globals)
            );
        }
    }
    let mut refused = PreparedOwner::new(&decoder, &fixtures.refused_globals, 65_536);
    let mut public_refused = decoder.prepared_globals(&fixtures.refused_globals, 65_536);
    for _ in 0..2 {
        for page in &fixtures.refused_pages {
            let expected = decoder.decode_embedded_with_globals(page, &fixtures.refused_globals);
            assert_outcome_eq!(
                expected,
                Err(DecodeError::Malformed(
                    MalformedError::InvalidPageAssociation {
                        segment: 2,
                        page: 0,
                    }
                ))
            );
            assert_outcome_eq!(refused.decode(page), expected);
            assert_outcome_eq!(public_refused.decode_embedded(page), expected);
        }
    }
    assert_eq!(refused.hits, 0);
    for index in 0..15 {
        let succeeds = |value| {
            let mut limits = Limits::default();
            set_limit(&mut limits, index, value);
            let limited = Decoder::new(limits);
            fixtures.pages.iter().all(|page| {
                limited
                    .decode_embedded_with_globals(page, &fixtures.globals)
                    .is_ok()
            })
        };
        let mut high = 1_u64;
        while !succeeds(high) {
            high = high.checked_mul(2).unwrap();
        }
        let mut low = 0;
        while low < high {
            let middle = low + (high - low) / 2;
            if succeeds(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        for value in [low.saturating_sub(1), low, low + 1] {
            let mut limits = Limits::default();
            set_limit(&mut limits, index, value);
            let limited = Decoder::new(limits);
            let mut owner = PreparedOwner::new(&limited, &fixtures.globals, 65_536);
            let mut public = limited.prepared_globals(&fixtures.globals, 65_536);
            for _ in 0..2 {
                for page in &fixtures.pages {
                    assert_outcome_eq!(
                        owner.decode(page),
                        limited.decode_embedded_with_globals(page, &fixtures.globals),
                        "limit field {index}, value {value}"
                    );
                    assert_outcome_eq!(
                        public.decode_embedded(page),
                        limited.decode_embedded_with_globals(page, &fixtures.globals),
                        "public limit field {index}, value {value}"
                    );
                }
            }
        }
        println!("prepared-limit field={index} minimum={low}");
    }
    println!(
        "prepared-controls exact=true refused=true resident_bytes={} peak_owner_plus_copy_bytes={}",
        owner.resident_bytes, owner.peak_copy_bytes
    );
}

fn run_sequence(
    decoder: &Decoder,
    fixtures: &Fixtures,
    mode: &str,
    warm: &mut PreparedOwner<'_>,
) -> [usize; 3] {
    let mut cold = PreparedOwner::new(decoder, &fixtures.globals, 65_536);
    for page in &fixtures.pages {
        let output = match mode {
            "baseline" => decoder
                .decode_embedded_with_globals(page, &fixtures.globals)
                .unwrap(),
            "cold" => cold.decode(page).unwrap(),
            "warm" => warm.decode(page).unwrap(),
            _ => unreachable!(),
        };
        std::hint::black_box(output);
    }
    let counters = [cold.preparations, cold.hits, cold.fallbacks];
    drop(cold);
    counters
}

fn check_sequence_totals(
    mode: &str,
    iterations: usize,
    cold_totals: [usize; 3],
    warm: &PreparedOwner<'_>,
) {
    let expected = if mode == "cold" {
        [iterations, iterations * 3, 0]
    } else {
        [0; 3]
    };
    assert_eq!(cold_totals, expected);
    if mode == "warm" {
        assert_eq!(warm.preparations, 1);
        assert_eq!(warm.hits, (iterations + 1) * 3);
        assert_eq!(warm.fallbacks, 0);
    }
}

#[test]
#[ignore = "external pinned fixtures; exercises all probe modes without a timer"]
fn prepared_globals_probe_functional_controls() {
    let decoder = Decoder::default();
    let fixtures = Fixtures::read(&decoder);
    let expected = fixtures.expected(&decoder);
    for mode in ["baseline", "cold", "warm"] {
        let mut warm = PreparedOwner::new(&decoder, &fixtures.globals, 65_536);
        if mode == "warm" {
            for (page, expected) in fixtures.pages.iter().zip(&expected) {
                assert_eq!(warm.decode(page).unwrap(), *expected);
            }
        }
        let mut cold_totals = [0; 3];
        for _ in 0..2 {
            let counters = run_sequence(&decoder, &fixtures, mode, &mut warm);
            for (total, count) in cold_totals.iter_mut().zip(counters) {
                *total += count;
            }
        }
        check_sequence_totals(mode, 2, cold_totals, &warm);
        for (page, expected) in fixtures.pages.iter().zip(&expected) {
            assert_eq!(warm.decode(page).unwrap(), *expected);
        }
    }
}

#[test]
#[ignore = "parent-owned timing probe; never run as part of correctness validation"]
fn prepared_globals_sequence_probe() {
    let decoder = Decoder::default();
    let fixtures = Fixtures::read(&decoder);
    let expected = fixtures.expected(&decoder);
    let mode = std::env::var("JBIG2_PREPARED_MODE").expect("baseline, cold or warm");
    let iterations: usize = std::env::var("JBIG2_PREPARED_ITERATIONS")
        .unwrap_or_else(|_| "1001".into())
        .parse()
        .unwrap();
    assert!(iterations > 0);
    assert!(["baseline", "cold", "warm"].contains(&mode.as_str()));
    let mut warm = PreparedOwner::new(&decoder, &fixtures.globals, 65_536);
    if mode == "warm" {
        for (page, expected) in fixtures.pages.iter().zip(&expected) {
            assert_eq!(warm.decode(page).unwrap(), *expected);
        }
    }
    let mut samples_ns = Vec::new();
    samples_ns
        .try_reserve_exact(iterations)
        .expect("preallocate sequence samples");
    let mut cold_totals = [0; 3];
    for _ in 0..iterations {
        let start = std::time::Instant::now();
        let counters = run_sequence(&decoder, &fixtures, &mode, &mut warm);
        let duration_ns = start.elapsed().as_nanos();
        samples_ns.push(duration_ns);
        for (total, count) in cold_totals.iter_mut().zip(counters) {
            *total += count;
        }
    }
    let elapsed: u128 = samples_ns.iter().sum();
    check_sequence_totals(&mode, iterations, cold_totals, &warm);
    // Validate the same candidate outside the timed interval, not a skip-decode
    // surrogate. Baseline/candidate output equality is a separate hard control.
    let mut checked = PreparedOwner::new(&decoder, &fixtures.globals, 65_536);
    for (page, expected) in fixtures.pages.iter().zip(&expected) {
        assert_eq!(checked.decode(page).unwrap(), *expected);
    }
    assert_eq!(checked.hits, 3);
    println!(
        "prepared-sequence mode={mode} iterations={iterations} pages={} elapsed_ns={elapsed} exact=true resident_bytes={} peak_owner_plus_copy_bytes={}",
        iterations * 3, checked.resident_bytes, checked.peak_copy_bytes
    );
    println!("samples_ns={samples_ns:?}");
}
