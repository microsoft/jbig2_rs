//! Probability implementation.

#[cfg(not(verus_keep_ghost))]
use super::QeState;
#[cfg(verus_keep_ghost)]
use super::*;

// T.88, Table E.1. Keep the order identical to the probability-state index.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const QE_STATES: [QeState; 47] = [
    QeState {
        qe: 0x5601,
        next_mps: 1,
        next_lps: 1,
        switch_mps: true,
    },
    QeState {
        qe: 0x3401,
        next_mps: 2,
        next_lps: 6,
        switch_mps: false,
    },
    QeState {
        qe: 0x1801,
        next_mps: 3,
        next_lps: 9,
        switch_mps: false,
    },
    QeState {
        qe: 0x0ac1,
        next_mps: 4,
        next_lps: 12,
        switch_mps: false,
    },
    QeState {
        qe: 0x0521,
        next_mps: 5,
        next_lps: 29,
        switch_mps: false,
    },
    QeState {
        qe: 0x0221,
        next_mps: 38,
        next_lps: 33,
        switch_mps: false,
    },
    QeState {
        qe: 0x5601,
        next_mps: 7,
        next_lps: 6,
        switch_mps: true,
    },
    QeState {
        qe: 0x5401,
        next_mps: 8,
        next_lps: 14,
        switch_mps: false,
    },
    QeState {
        qe: 0x4801,
        next_mps: 9,
        next_lps: 14,
        switch_mps: false,
    },
    QeState {
        qe: 0x3801,
        next_mps: 10,
        next_lps: 14,
        switch_mps: false,
    },
    QeState {
        qe: 0x3001,
        next_mps: 11,
        next_lps: 17,
        switch_mps: false,
    },
    QeState {
        qe: 0x2401,
        next_mps: 12,
        next_lps: 18,
        switch_mps: false,
    },
    QeState {
        qe: 0x1c01,
        next_mps: 13,
        next_lps: 20,
        switch_mps: false,
    },
    QeState {
        qe: 0x1601,
        next_mps: 29,
        next_lps: 21,
        switch_mps: false,
    },
    QeState {
        qe: 0x5601,
        next_mps: 15,
        next_lps: 14,
        switch_mps: true,
    },
    QeState {
        qe: 0x5401,
        next_mps: 16,
        next_lps: 14,
        switch_mps: false,
    },
    QeState {
        qe: 0x5101,
        next_mps: 17,
        next_lps: 15,
        switch_mps: false,
    },
    QeState {
        qe: 0x4801,
        next_mps: 18,
        next_lps: 16,
        switch_mps: false,
    },
    QeState {
        qe: 0x3801,
        next_mps: 19,
        next_lps: 17,
        switch_mps: false,
    },
    QeState {
        qe: 0x3401,
        next_mps: 20,
        next_lps: 18,
        switch_mps: false,
    },
    QeState {
        qe: 0x3001,
        next_mps: 21,
        next_lps: 19,
        switch_mps: false,
    },
    QeState {
        qe: 0x2801,
        next_mps: 22,
        next_lps: 19,
        switch_mps: false,
    },
    QeState {
        qe: 0x2401,
        next_mps: 23,
        next_lps: 20,
        switch_mps: false,
    },
    QeState {
        qe: 0x2201,
        next_mps: 24,
        next_lps: 21,
        switch_mps: false,
    },
    QeState {
        qe: 0x1c01,
        next_mps: 25,
        next_lps: 22,
        switch_mps: false,
    },
    QeState {
        qe: 0x1801,
        next_mps: 26,
        next_lps: 23,
        switch_mps: false,
    },
    QeState {
        qe: 0x1601,
        next_mps: 27,
        next_lps: 24,
        switch_mps: false,
    },
    QeState {
        qe: 0x1401,
        next_mps: 28,
        next_lps: 25,
        switch_mps: false,
    },
    QeState {
        qe: 0x1201,
        next_mps: 29,
        next_lps: 26,
        switch_mps: false,
    },
    QeState {
        qe: 0x1101,
        next_mps: 30,
        next_lps: 27,
        switch_mps: false,
    },
    QeState {
        qe: 0x0ac1,
        next_mps: 31,
        next_lps: 28,
        switch_mps: false,
    },
    QeState {
        qe: 0x09c1,
        next_mps: 32,
        next_lps: 29,
        switch_mps: false,
    },
    QeState {
        qe: 0x08a1,
        next_mps: 33,
        next_lps: 30,
        switch_mps: false,
    },
    QeState {
        qe: 0x0521,
        next_mps: 34,
        next_lps: 31,
        switch_mps: false,
    },
    QeState {
        qe: 0x0441,
        next_mps: 35,
        next_lps: 32,
        switch_mps: false,
    },
    QeState {
        qe: 0x02a1,
        next_mps: 36,
        next_lps: 33,
        switch_mps: false,
    },
    QeState {
        qe: 0x0221,
        next_mps: 37,
        next_lps: 34,
        switch_mps: false,
    },
    QeState {
        qe: 0x0141,
        next_mps: 38,
        next_lps: 35,
        switch_mps: false,
    },
    QeState {
        qe: 0x0111,
        next_mps: 39,
        next_lps: 36,
        switch_mps: false,
    },
    QeState {
        qe: 0x0085,
        next_mps: 40,
        next_lps: 37,
        switch_mps: false,
    },
    QeState {
        qe: 0x0049,
        next_mps: 41,
        next_lps: 38,
        switch_mps: false,
    },
    QeState {
        qe: 0x0025,
        next_mps: 42,
        next_lps: 39,
        switch_mps: false,
    },
    QeState {
        qe: 0x0015,
        next_mps: 43,
        next_lps: 40,
        switch_mps: false,
    },
    QeState {
        qe: 0x0009,
        next_mps: 44,
        next_lps: 41,
        switch_mps: false,
    },
    QeState {
        qe: 0x0005,
        next_mps: 45,
        next_lps: 42,
        switch_mps: false,
    },
    QeState {
        qe: 0x0001,
        next_mps: 45,
        next_lps: 43,
        switch_mps: false,
    },
    QeState {
        qe: 0x5601,
        next_mps: 46,
        next_lps: 46,
        switch_mps: false,
    },
];

/// 128-entry packed Qe lookup indexed by `usize::from(context.packed()) & 0x7f`.
/// Valid entries `0..94` equal `QE_STATES[p >> 1].qe`; entries `94..128` are zero padding.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const QE_PACKED_VALS: [u16; 128] = [
    0x5601, 0x5601, 0x3401, 0x3401, 0x1801, 0x1801, 0x0ac1, 0x0ac1, 0x0521, 0x0521, 0x0221, 0x0221,
    0x5601, 0x5601, 0x5401, 0x5401, 0x4801, 0x4801, 0x3801, 0x3801, 0x3001, 0x3001, 0x2401, 0x2401,
    0x1c01, 0x1c01, 0x1601, 0x1601, 0x5601, 0x5601, 0x5401, 0x5401, 0x5101, 0x5101, 0x4801, 0x4801,
    0x3801, 0x3801, 0x3401, 0x3401, 0x3001, 0x3001, 0x2801, 0x2801, 0x2401, 0x2401, 0x2201, 0x2201,
    0x1c01, 0x1c01, 0x1801, 0x1801, 0x1601, 0x1601, 0x1401, 0x1401, 0x1201, 0x1201, 0x1101, 0x1101,
    0x0ac1, 0x0ac1, 0x09c1, 0x09c1, 0x08a1, 0x08a1, 0x0521, 0x0521, 0x0441, 0x0441, 0x02a1, 0x02a1,
    0x0221, 0x0221, 0x0141, 0x0141, 0x0111, 0x0111, 0x0085, 0x0085, 0x0049, 0x0049, 0x0025, 0x0025,
    0x0015, 0x0015, 0x0009, 0x0009, 0x0005, 0x0005, 0x0001, 0x0001, 0x5601, 0x5601, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
];

/// 128-entry packed next-MPS transition indexed by `usize::from(context.packed()) & 0x7f`.
/// Valid entries `0..94` equal `(QE_STATES[p >> 1].next_mps << 1) | (p & 1)`.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const NEXT_MPS_PACKED: [u8; 128] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 76, 77, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    58, 59, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51,
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75,
    76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 90, 91, 92, 93, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
