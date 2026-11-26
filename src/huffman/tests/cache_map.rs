#![cfg(test)]

use super::*;

#[test]
fn compact_cache_indices_match_wide_sentinel_math_at_every_boundary() {
    let mut counts = [0_u32; MAX_PREFIX_LEN + 1];
    counts[8] = 1;
    let mut codes = [WideCode::zero(); MAX_PREFIX_LEN + 1];
    let starts = [0_u32; MAX_PREFIX_LEN + 1];
    let mut checked = 0;
    for index in (0..=65_536_u32).chain([u32::MAX]) {
        let prefix = usize::try_from(index & 0xff).unwrap();
        codes[8].limbs[0] = u64::try_from(prefix).unwrap();
        let cache = build_prefix_lookup(&counts, &codes, &starts, &[index], 8).unwrap();
        let wide = u64::from(index) + 1;
        let expected = u16::try_from(wide).unwrap_or(0);
        assert_eq!(cache.len(), 256);
        for (slot, &value) in cache.iter().enumerate() {
            assert_eq!(value, if slot == prefix { expected } else { 0 });
        }
        checked += 1;
    }
    assert_eq!(checked, 65_538);
}

#[test]
fn production_cache_maps_sparse_codes_and_rejects_invalid_metadata() {
    let mut counts = [0_u32; MAX_PREFIX_LEN + 1];
    let mut codes = [WideCode::zero(); MAX_PREFIX_LEN + 1];
    let starts = [0_u32; MAX_PREFIX_LEN + 1];
    let mut checked = 0;
    for first in [0_u64, 1, 17, 254, 255] {
        codes[8].limbs[0] = first;
        for count in 0..=256_u32 {
            counts[8] = count;
            let order: Vec<_> = (0..count).map(|index| index * 3 + 5).collect();
            let result = build_prefix_lookup(&counts, &codes, &starts, &order, 8);
            if count != 0 && first + u64::from(count) - 1 > 255 {
                assert!(matches!(
                    result,
                    Err(HuffmanError::InvalidTable(
                        HuffmanTableError::Oversubscribed
                    ))
                ));
            } else {
                let cache = result.unwrap();
                assert_eq!(cache.len(), 256);
                for prefix in 0..=u8::MAX {
                    let distance = i64::from(prefix) - i64::try_from(first).unwrap();
                    let expected = if distance >= 0 && distance < i64::from(count) {
                        u16::try_from(distance * 3 + 6).unwrap()
                    } else {
                        0
                    };
                    assert_eq!(cache[usize::from(prefix)], expected);
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 1_285);
    counts[8] = 1;
    codes[8].limbs = [0, 1, 0, 0];
    assert!(matches!(
        build_prefix_lookup(&counts, &codes, &starts, &[0], 8),
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::Oversubscribed
        ))
    ));
    codes[8] = WideCode::zero();
    assert!(matches!(
        build_prefix_lookup(&counts, &codes, &starts, &[], 8),
        Err(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))
    ));
}

fn assert_flat_cache(table: &HuffmanTable, length: u8, count: usize) {
    assert!(table.prefer_prefix_lookup);
    assert_eq!(table.prefix_lookup.len(), 256);
    for (prefix, &entry) in table.prefix_lookup.as_slice().iter().enumerate() {
        let expected = if length == 8 && prefix < count {
            u16::try_from(prefix + 1).unwrap()
        } else {
            0
        };
        assert_eq!(entry, expected);
    }
}

#[test]
fn raw_symbol_id_and_standard_constructors_keep_their_production_cache_map() {
    let limits = Limits::default();
    for length in [8, 9, 16, 32, 255] {
        for count in [1_usize, 2, 17, 128, 256] {
            let lines: Vec<_> = (0..count)
                .map(|value| HuffmanLine::normal(i32::try_from(value).unwrap(), length, 0))
                .collect();
            let raw_table = HuffmanTable::new(&lines, &limits).unwrap();
            assert_flat_cache(&raw_table, length, count);
            let lengths = vec![length; count];
            let symbols = HuffmanTable::new_symbol_ids(&lengths, &limits).unwrap();
            assert_flat_cache(&symbols, length, count);
        }
    }
    for standard in ALL_STANDARD_TABLES {
        let table = HuffmanTable::standard(standard, &limits).unwrap();
        let minimum = table
            .lines
            .as_slice()
            .iter()
            .map(|line| line.prefix_len)
            .filter(|&length| length != 0)
            .min()
            .unwrap();
        if minimum < 8 {
            assert!(!table.prefer_prefix_lookup);
            assert!(table.prefix_lookup.is_empty());
        } else {
            assert!(table.prefer_prefix_lookup);
            assert_eq!(table.prefix_lookup.len(), 256);
        }
    }
}
