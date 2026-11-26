//! Compile-time precomputed T.88 Annex B standard Huffman tables A–O.
//!
//! Zero heap allocation, zero runtime construction, and 100% Verus-verifiable.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    HuffmanTable, StandardHuffmanTable, TableSlice, TABLE_A, TABLE_B, TABLE_C, TABLE_D, TABLE_E,
    TABLE_F, TABLE_G, TABLE_H, TABLE_I, TABLE_J, TABLE_K, TABLE_L, TABLE_M, TABLE_N, TABLE_O,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static EMPTY_PREFIX_LOOKUP: [u16; 0] = [];

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_A: [u32; 4] = [0, 1, 2, 3];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_A: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_A),
    ordered_lines: TableSlice::Static(&ORDERED_A),
    short_length_count: [
        0, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 3,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_B: [u32; 7] = [0, 1, 2, 3, 4, 5, 6];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_B: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_B),
    ordered_lines: TableSlice::Static(&ORDERED_B),
    short_length_count: [
        0, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 14, 30, 62, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 6,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_C: [u32; 9] = [1, 2, 3, 4, 5, 8, 7, 0, 6];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_C: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_C),
    ordered_lines: TableSlice::Static(&ORDERED_C),
    short_length_count: [
        0, 1, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 14, 30, 62, 126, 254, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 8,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_D: [u32; 6] = [0, 1, 2, 3, 4, 5];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_D: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_D),
    ordered_lines: TableSlice::Static(&ORDERED_D),
    short_length_count: [
        0, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 14, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 5,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_E: [u32; 8] = [1, 2, 3, 4, 5, 7, 0, 6];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_E: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_E),
    ordered_lines: TableSlice::Static(&ORDERED_E),
    short_length_count: [
        0, 1, 1, 1, 1, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 14, 30, 62, 126, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 7,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_F: [u32; 14] = [7, 8, 9, 1, 2, 3, 6, 10, 11, 0, 4, 5, 12, 13];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_F: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_F),
    ordered_lines: TableSlice::Static(&ORDERED_F),
    short_length_count: [
        0, 0, 1, 2, 6, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 0, 2, 8, 28, 62, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_line: [
        0, 0, 0, 1, 3, 9, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 6,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_G: [u32; 15] = [1, 10, 11, 12, 0, 2, 5, 6, 9, 3, 4, 7, 8, 13, 14];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_G: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_G),
    ordered_lines: TableSlice::Static(&ORDERED_G),
    short_length_count: [
        0, 0, 0, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 0, 0, 8, 26, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_line: [
        0, 0, 0, 0, 4, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 5,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_H: [u32; 21] = [
    6, 20, 9, 5, 11, 12, 7, 13, 14, 8, 10, 15, 17, 4, 16, 0, 2, 1, 3, 18, 19,
];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_H: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_H),
    ordered_lines: TableSlice::Static(&ORDERED_H),
    short_length_count: [
        0, 0, 2, 1, 3, 3, 4, 2, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 0, 4, 10, 26, 58, 124, 252, 508, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 0, 2, 3, 6, 9, 13, 15, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 9,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_I: [u32; 22] = [
    21, 6, 7, 10, 5, 12, 13, 8, 14, 15, 9, 11, 16, 18, 4, 17, 0, 2, 1, 3, 19, 20,
];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_I: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_I),
    ordered_lines: TableSlice::Static(&ORDERED_I),
    short_length_count: [
        0, 0, 1, 3, 3, 3, 4, 2, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 0, 2, 10, 26, 58, 124, 252, 508, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 0, 1, 4, 7, 10, 14, 16, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 9,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_J: [u32; 21] = [
    4, 9, 20, 3, 5, 10, 6, 11, 12, 13, 14, 15, 16, 0, 2, 7, 17, 1, 8, 18, 19,
];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_J: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_J),
    ordered_lines: TableSlice::Static(&ORDERED_J),
    short_length_count: [
        0, 0, 3, 0, 0, 3, 7, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 0, 6, 12, 24, 54, 122, 252, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 0, 3, 3, 3, 6, 13, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 8,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_K: [u32; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_K: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_K),
    ordered_lines: TableSlice::Static(&ORDERED_K),
    short_length_count: [
        0, 1, 1, 0, 2, 2, 1, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 12, 28, 60, 122, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 2, 4, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 7,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_L: [u32; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_L: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_L),
    ordered_lines: TableSlice::Static(&ORDERED_L),
    short_length_count: [
        0, 1, 1, 1, 0, 2, 1, 5, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 6, 14, 28, 60, 122, 254, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 2, 3, 3, 5, 6, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 8,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_M: [u32; 13] = [0, 1, 5, 2, 4, 3, 6, 7, 8, 9, 10, 11, 12];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_M: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_M),
    ordered_lines: TableSlice::Static(&ORDERED_M),
    short_length_count: [
        0, 1, 0, 2, 2, 1, 5, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 4, 12, 28, 58, 126, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 1, 3, 5, 6, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 7,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_N: [u32; 5] = [2, 0, 1, 3, 4];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_N: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_N),
    ordered_lines: TableSlice::Static(&ORDERED_N),
    short_length_count: [
        0, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 3,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
static ORDERED_O: [u32; 13] = [5, 4, 6, 3, 7, 2, 8, 1, 9, 0, 10, 11, 12];
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) static STANDARD_TABLE_O: HuffmanTable = HuffmanTable {
    lines: TableSlice::Static(&TABLE_O),
    ordered_lines: TableSlice::Static(&ORDERED_O),
    short_length_count: [
        0, 1, 0, 2, 2, 2, 2, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    short_first_code: [
        0, 0, 2, 4, 12, 28, 60, 124, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ],
    short_first_line: [
        0, 0, 1, 1, 3, 5, 7, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0,
    ],
    long_tables: None,
    prefix_lookup: TableSlice::Static(&EMPTY_PREFIX_LOOKUP),
    prefer_prefix_lookup: false,
    max_prefix_len: 7,
};

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn static_standard_table(table: StandardHuffmanTable) -> &'static HuffmanTable {
    match table {
        StandardHuffmanTable::A => &STANDARD_TABLE_A,
        StandardHuffmanTable::B => &STANDARD_TABLE_B,
        StandardHuffmanTable::C => &STANDARD_TABLE_C,
        StandardHuffmanTable::D => &STANDARD_TABLE_D,
        StandardHuffmanTable::E => &STANDARD_TABLE_E,
        StandardHuffmanTable::F => &STANDARD_TABLE_F,
        StandardHuffmanTable::G => &STANDARD_TABLE_G,
        StandardHuffmanTable::H => &STANDARD_TABLE_H,
        StandardHuffmanTable::I => &STANDARD_TABLE_I,
        StandardHuffmanTable::J => &STANDARD_TABLE_J,
        StandardHuffmanTable::K => &STANDARD_TABLE_K,
        StandardHuffmanTable::L => &STANDARD_TABLE_L,
        StandardHuffmanTable::M => &STANDARD_TABLE_M,
        StandardHuffmanTable::N => &STANDARD_TABLE_N,
        StandardHuffmanTable::O => &STANDARD_TABLE_O,
    }
}
