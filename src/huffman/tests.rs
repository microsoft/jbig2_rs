#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

mod benchmarks;
mod cache_map;
mod canonical_decoding;
mod fallback_state;
mod table_validation;
mod wide_arithmetic;

use super::*;

const ALL_STANDARD_TABLES: [StandardHuffmanTable; 15] = [
    StandardHuffmanTable::A,
    StandardHuffmanTable::B,
    StandardHuffmanTable::C,
    StandardHuffmanTable::D,
    StandardHuffmanTable::E,
    StandardHuffmanTable::F,
    StandardHuffmanTable::G,
    StandardHuffmanTable::H,
    StandardHuffmanTable::I,
    StandardHuffmanTable::J,
    StandardHuffmanTable::K,
    StandardHuffmanTable::L,
    StandardHuffmanTable::M,
    StandardHuffmanTable::N,
    StandardHuffmanTable::O,
];

fn bits(bit_string: &str) -> Vec<u8> {
    let mut bytes = vec![0_u8; bit_string.len().div_ceil(8)];
    for (position, bit) in bit_string.bytes().enumerate() {
        if bit == b'1' {
            bytes[position / 8] |= 1 << (7 - position % 8);
        }
    }
    bytes
}

fn decode_bits(table: &HuffmanTable, bit_string: &str) -> HuffmanValue {
    let bytes = bits(bit_string);
    let mut reader = bit_reader(&bytes);
    table.decode(&mut reader).expect("valid encoded value")
}

fn assert_decoders_equal(table: &HuffmanTable, bytes: &[u8], bit_position: usize) {
    let mut optimized = bit_reader(bytes);
    optimized.bit_position = bit_position;
    let mut canonical = optimized;
    let optimized_result = table.decode(&mut optimized);
    let canonical_result = table.decode_canonical(&mut canonical);
    assert_eq!(
        format!("{optimized_result:?}"),
        format!("{canonical_result:?}"),
        "bytes={bytes:02x?}, bit_position={bit_position}"
    );
    assert_eq!(
        optimized.bit_position, canonical.bit_position,
        "bytes={bytes:02x?}, bit_position={bit_position}"
    );
}

fn force_prefix_lookup(table: &mut HuffmanTable) {
    let mut lc = [0_u32; MAX_PREFIX_LEN + 1];
    let mut fc = [WideCode::zero(); MAX_PREFIX_LEN + 1];
    let mut fl = [0_u32; MAX_PREFIX_LEN + 1];
    for len in 0..=MAX_PREFIX_LEN {
        lc[len] = table.length_count_at(len);
        fc[len] = table.first_code_at(len);
        fl[len] = table.first_line_at(len);
    }
    table.prefix_lookup = TableSlice::Owned(
        build_prefix_lookup(
            &lc,
            &fc,
            &fl,
            table.ordered_lines.as_slice(),
            usize::from(table.max_prefix_len),
        )
        .expect("prefix lookup"),
    );
    table.prefer_prefix_lookup = true;
}

impl HuffmanTable {
    fn test_empty(lines: Vec<HuffmanLine>, ordered_lines: Vec<u32>, max_prefix_len: u8) -> Self {
        Self {
            lines: TableSlice::Owned(lines),
            ordered_lines: TableSlice::Owned(ordered_lines),
            short_length_count: [0; SHORT_PREFIX_MAX + 1],
            short_first_code: [0; SHORT_PREFIX_MAX + 1],
            short_first_line: [0; SHORT_PREFIX_MAX + 1],
            long_tables: Some(Box::new(LongPrefixTables {
                length_count: [0; MAX_PREFIX_LEN + 1],
                first_code: [WideCode::zero(); MAX_PREFIX_LEN + 1],
                first_line: [0; MAX_PREFIX_LEN + 1],
            })),
            prefix_lookup: TableSlice::Owned(Vec::new()),
            prefer_prefix_lookup: false,
            max_prefix_len,
        }
    }

    fn set_test_count(&mut self, len: usize, val: u32) {
        if len <= SHORT_PREFIX_MAX {
            self.short_length_count[len] = val;
        }
        if let Some(long) = &mut self.long_tables {
            long.length_count[len] = val;
        }
    }

    fn set_test_first_code(&mut self, len: usize, val: WideCode) {
        if len <= SHORT_PREFIX_MAX {
            #[allow(clippy::cast_possible_truncation)]
            {
                self.short_first_code[len] = val.limbs[0] as u32;
            }
        }
        if let Some(long) = &mut self.long_tables {
            long.first_code[len] = val;
        }
    }

    fn set_test_first_line(&mut self, len: usize, val: u32) {
        if len <= SHORT_PREFIX_MAX {
            self.short_first_line[len] = val;
        }
        if let Some(long) = &mut self.long_tables {
            long.first_line[len] = val;
        }
    }
}

#[test]
fn static_standard_tables_match_fresh_construction_and_struct_is_compact() {
    assert!(
        size_of::<HuffmanTable>() <= 600,
        "HuffmanTable regressed in size: {} bytes",
        size_of::<HuffmanTable>()
    );
    let limits = Limits::default();
    let sources: [(StandardHuffmanTable, &[HuffmanLine]); 15] = [
        (StandardHuffmanTable::A, &TABLE_A),
        (StandardHuffmanTable::B, &TABLE_B),
        (StandardHuffmanTable::C, &TABLE_C),
        (StandardHuffmanTable::D, &TABLE_D),
        (StandardHuffmanTable::E, &TABLE_E),
        (StandardHuffmanTable::F, &TABLE_F),
        (StandardHuffmanTable::G, &TABLE_G),
        (StandardHuffmanTable::H, &TABLE_H),
        (StandardHuffmanTable::I, &TABLE_I),
        (StandardHuffmanTable::J, &TABLE_J),
        (StandardHuffmanTable::K, &TABLE_K),
        (StandardHuffmanTable::L, &TABLE_L),
        (StandardHuffmanTable::M, &TABLE_M),
        (StandardHuffmanTable::N, &TABLE_N),
        (StandardHuffmanTable::O, &TABLE_O),
    ];
    for (tag, raw_lines) in sources {
        let static_tbl = HuffmanTable::standard_static(tag, &limits).expect("static");
        let built = HuffmanTable::new(raw_lines, &limits).expect("built");
        assert_eq!(static_tbl.lines.as_slice(), built.lines.as_slice());
        assert_eq!(
            static_tbl.ordered_lines.as_slice(),
            built.ordered_lines.as_slice()
        );
        assert_eq!(static_tbl.short_length_count, built.short_length_count);
        assert_eq!(static_tbl.short_first_code, built.short_first_code);
        assert_eq!(static_tbl.short_first_line, built.short_first_line);
        assert_eq!(static_tbl.prefer_prefix_lookup, built.prefer_prefix_lookup);
        assert_eq!(static_tbl.max_prefix_len, built.max_prefix_len);
        assert!(static_tbl.long_tables.is_none());
    }
}

fn append_code(output: &mut String, code: WideCode, length: u8) {
    for position in (0..usize::from(length)).rev() {
        let limb = position / 64;
        let bit = position % 64;
        output.push(if code.limbs[limb] >> bit & 1 == 0 {
            '0'
        } else {
            '1'
        });
    }
}

fn encoded_line(table: &HuffmanTable, index: usize, offset: u32) -> String {
    let (code, length) = table.code_for_line(index).expect("active line");
    let line = table.lines.as_slice()[index];
    let mut encoded = String::new();
    append_code(&mut encoded, code, length);
    for position in (0..line.range_len).rev() {
        encoded.push(if offset >> position & 1 == 0 {
            '0'
        } else {
            '1'
        });
    }
    encoded
}

fn code_string(table: &HuffmanTable, index: usize) -> String {
    let (code, length) = table.code_for_line(index).expect("active line");
    let mut result = String::new();
    append_code(&mut result, code, length);
    result
}
