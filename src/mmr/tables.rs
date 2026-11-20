//! Tables implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{Mode, ModeCode, RunCode};

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const RUN_INDEX_SIZE: usize = 14 * 256;

#[cfg_attr(verus_keep_ghost, verus_spec(ensures true))]
pub(super) const WHITE_RUN_INDEX: [u8; RUN_INDEX_SIZE] = build_run_index(false);
#[cfg_attr(verus_keep_ghost, verus_spec(ensures true))]
pub(super) const BLACK_RUN_INDEX: [u8; RUN_INDEX_SIZE] = build_run_index(true);

#[cfg_attr(verus_keep_ghost, verus_verify)]
#[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
const fn index_run_code(
    mut out: [u8; RUN_INDEX_SIZE],
    code: RunCode,
    ordinal: usize,
) -> [u8; RUN_INDEX_SIZE] {
    if code.length <= 13 && code.bits <= u8::MAX as u16 && ordinal < u8::MAX as usize {
        let base = code.length as usize * 256;
        if let Some(key) = base.checked_add(code.bits as usize) {
            if key < RUN_INDEX_SIZE && out[key] == 0 {
                out[key] = (ordinal + 1) as u8;
            }
        }
    }
    out
}

// Length and significant bits form the key. Entries are canonical-table
// ordinals plus one; zero means absent, and dispatch validates the original key.
// Unroll table ordinals so CTFE never executes ghost loop/decreases markers.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const fn build_run_index(black: bool) -> [u8; RUN_INDEX_SIZE] {
    let terminating = if black {
        &BLACK_TERMINATING
    } else {
        &WHITE_TERMINATING
    };
    let makeup = if black { &BLACK_MAKEUP } else { &WHITE_MAKEUP };
    let mut out = [0_u8; RUN_INDEX_SIZE];
    macro_rules! index_entries {
        ($table:expr, $base:expr; $($index:expr),+ $(,)?) => {
            $(out = index_run_code(out, $table[$index], $base + $index);)+
        };
    }
    index_entries!(terminating, 0;
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
        32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
    );
    index_entries!(makeup, terminating.len();
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    );
    index_entries!(COMMON_MAKEUP, terminating.len() + makeup.len();
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    );
    out
}

macro_rules! mode_code {
    ($bits:expr, $length:expr, $mode:expr) => {
        ModeCode {
            bits: $bits,
            length: $length,
            mode: $mode,
        }
    };
}

// ITU-T T.6, Table 1. Extension represents the fixed 0000001 prefix;
// the table's following xxx bits do not alter T.88's requirement to reject it.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const MODE_CODES: [ModeCode; 10] = [
    mode_code!(0b1, 1, Mode::Vertical(0)),
    mode_code!(0b011, 3, Mode::Vertical(1)),
    mode_code!(0b010, 3, Mode::Vertical(-1)),
    mode_code!(0b001, 3, Mode::Horizontal),
    mode_code!(0b0001, 4, Mode::Pass),
    mode_code!(0b000011, 6, Mode::Vertical(2)),
    mode_code!(0b000010, 6, Mode::Vertical(-2)),
    mode_code!(0b0000011, 7, Mode::Vertical(3)),
    mode_code!(0b0000010, 7, Mode::Vertical(-3)),
    mode_code!(0b0000001, 7, Mode::Extension),
];

macro_rules! terminal_code {
    ($value:expr, $bits:expr, $length:expr) => {
        RunCode {
            bits: $bits,
            length: $length,
            value: $value,
            terminal: true,
        }
    };
}

macro_rules! makeup_code {
    ($value:expr, $bits:expr, $length:expr) => {
        RunCode {
            bits: $bits,
            length: $length,
            value: $value,
            terminal: false,
        }
    };
}

// ITU-T T.6, Table 2: white terminating codes for run lengths 0 through 63.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const WHITE_TERMINATING: [RunCode; 64] = [
    terminal_code!(0, 0b00110101, 8),
    terminal_code!(1, 0b000111, 6),
    terminal_code!(2, 0b0111, 4),
    terminal_code!(3, 0b1000, 4),
    terminal_code!(4, 0b1011, 4),
    terminal_code!(5, 0b1100, 4),
    terminal_code!(6, 0b1110, 4),
    terminal_code!(7, 0b1111, 4),
    terminal_code!(8, 0b10011, 5),
    terminal_code!(9, 0b10100, 5),
    terminal_code!(10, 0b00111, 5),
    terminal_code!(11, 0b01000, 5),
    terminal_code!(12, 0b001000, 6),
    terminal_code!(13, 0b000011, 6),
    terminal_code!(14, 0b110100, 6),
    terminal_code!(15, 0b110101, 6),
    terminal_code!(16, 0b101010, 6),
    terminal_code!(17, 0b101011, 6),
    terminal_code!(18, 0b0100111, 7),
    terminal_code!(19, 0b0001100, 7),
    terminal_code!(20, 0b0001000, 7),
    terminal_code!(21, 0b0010111, 7),
    terminal_code!(22, 0b0000011, 7),
    terminal_code!(23, 0b0000100, 7),
    terminal_code!(24, 0b0101000, 7),
    terminal_code!(25, 0b0101011, 7),
    terminal_code!(26, 0b0010011, 7),
    terminal_code!(27, 0b0100100, 7),
    terminal_code!(28, 0b0011000, 7),
    terminal_code!(29, 0b00000010, 8),
    terminal_code!(30, 0b00000011, 8),
    terminal_code!(31, 0b00011010, 8),
    terminal_code!(32, 0b00011011, 8),
    terminal_code!(33, 0b00010010, 8),
    terminal_code!(34, 0b00010011, 8),
    terminal_code!(35, 0b00010100, 8),
    terminal_code!(36, 0b00010101, 8),
    terminal_code!(37, 0b00010110, 8),
    terminal_code!(38, 0b00010111, 8),
    terminal_code!(39, 0b00101000, 8),
    terminal_code!(40, 0b00101001, 8),
    terminal_code!(41, 0b00101010, 8),
    terminal_code!(42, 0b00101011, 8),
    terminal_code!(43, 0b00101100, 8),
    terminal_code!(44, 0b00101101, 8),
    terminal_code!(45, 0b00000100, 8),
    terminal_code!(46, 0b00000101, 8),
    terminal_code!(47, 0b00001010, 8),
    terminal_code!(48, 0b00001011, 8),
    terminal_code!(49, 0b01010010, 8),
    terminal_code!(50, 0b01010011, 8),
    terminal_code!(51, 0b01010100, 8),
    terminal_code!(52, 0b01010101, 8),
    terminal_code!(53, 0b00100100, 8),
    terminal_code!(54, 0b00100101, 8),
    terminal_code!(55, 0b01011000, 8),
    terminal_code!(56, 0b01011001, 8),
    terminal_code!(57, 0b01011010, 8),
    terminal_code!(58, 0b01011011, 8),
    terminal_code!(59, 0b01001010, 8),
    terminal_code!(60, 0b01001011, 8),
    terminal_code!(61, 0b00110010, 8),
    terminal_code!(62, 0b00110011, 8),
    terminal_code!(63, 0b00110100, 8),
];

// ITU-T T.6, Table 2: black terminating codes for run lengths 0 through 63.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const BLACK_TERMINATING: [RunCode; 64] = [
    terminal_code!(0, 0b0000110111, 10),
    terminal_code!(1, 0b010, 3),
    terminal_code!(2, 0b11, 2),
    terminal_code!(3, 0b10, 2),
    terminal_code!(4, 0b011, 3),
    terminal_code!(5, 0b0011, 4),
    terminal_code!(6, 0b0010, 4),
    terminal_code!(7, 0b00011, 5),
    terminal_code!(8, 0b000101, 6),
    terminal_code!(9, 0b000100, 6),
    terminal_code!(10, 0b0000100, 7),
    terminal_code!(11, 0b0000101, 7),
    terminal_code!(12, 0b0000111, 7),
    terminal_code!(13, 0b00000100, 8),
    terminal_code!(14, 0b00000111, 8),
    terminal_code!(15, 0b000011000, 9),
    terminal_code!(16, 0b0000010111, 10),
    terminal_code!(17, 0b0000011000, 10),
    terminal_code!(18, 0b0000001000, 10),
    terminal_code!(19, 0b00001100111, 11),
    terminal_code!(20, 0b00001101000, 11),
    terminal_code!(21, 0b00001101100, 11),
    terminal_code!(22, 0b00000110111, 11),
    terminal_code!(23, 0b00000101000, 11),
    terminal_code!(24, 0b00000010111, 11),
    terminal_code!(25, 0b00000011000, 11),
    terminal_code!(26, 0b000011001010, 12),
    terminal_code!(27, 0b000011001011, 12),
    terminal_code!(28, 0b000011001100, 12),
    terminal_code!(29, 0b000011001101, 12),
    terminal_code!(30, 0b000001101000, 12),
    terminal_code!(31, 0b000001101001, 12),
    terminal_code!(32, 0b000001101010, 12),
    terminal_code!(33, 0b000001101011, 12),
    terminal_code!(34, 0b000011010010, 12),
    terminal_code!(35, 0b000011010011, 12),
    terminal_code!(36, 0b000011010100, 12),
    terminal_code!(37, 0b000011010101, 12),
    terminal_code!(38, 0b000011010110, 12),
    terminal_code!(39, 0b000011010111, 12),
    terminal_code!(40, 0b000001101100, 12),
    terminal_code!(41, 0b000001101101, 12),
    terminal_code!(42, 0b000011011010, 12),
    terminal_code!(43, 0b000011011011, 12),
    terminal_code!(44, 0b000001010100, 12),
    terminal_code!(45, 0b000001010101, 12),
    terminal_code!(46, 0b000001010110, 12),
    terminal_code!(47, 0b000001010111, 12),
    terminal_code!(48, 0b000001100100, 12),
    terminal_code!(49, 0b000001100101, 12),
    terminal_code!(50, 0b000001010010, 12),
    terminal_code!(51, 0b000001010011, 12),
    terminal_code!(52, 0b000000100100, 12),
    terminal_code!(53, 0b000000110111, 12),
    terminal_code!(54, 0b000000111000, 12),
    terminal_code!(55, 0b000000100111, 12),
    terminal_code!(56, 0b000000101000, 12),
    terminal_code!(57, 0b000001011000, 12),
    terminal_code!(58, 0b000001011001, 12),
    terminal_code!(59, 0b000000101011, 12),
    terminal_code!(60, 0b000000101100, 12),
    terminal_code!(61, 0b000001011010, 12),
    terminal_code!(62, 0b000001100110, 12),
    terminal_code!(63, 0b000001100111, 12),
];

// ITU-T T.6, Table 3: colour-specific make-up codes, 64 through 1728.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const WHITE_MAKEUP: [RunCode; 27] = [
    makeup_code!(64, 0b11011, 5),
    makeup_code!(128, 0b10010, 5),
    makeup_code!(192, 0b010111, 6),
    makeup_code!(256, 0b0110111, 7),
    makeup_code!(320, 0b00110110, 8),
    makeup_code!(384, 0b00110111, 8),
    makeup_code!(448, 0b01100100, 8),
    makeup_code!(512, 0b01100101, 8),
    makeup_code!(576, 0b01101000, 8),
    makeup_code!(640, 0b01100111, 8),
    makeup_code!(704, 0b011001100, 9),
    makeup_code!(768, 0b011001101, 9),
    makeup_code!(832, 0b011010010, 9),
    makeup_code!(896, 0b011010011, 9),
    makeup_code!(960, 0b011010100, 9),
    makeup_code!(1024, 0b011010101, 9),
    makeup_code!(1088, 0b011010110, 9),
    makeup_code!(1152, 0b011010111, 9),
    makeup_code!(1216, 0b011011000, 9),
    makeup_code!(1280, 0b011011001, 9),
    makeup_code!(1344, 0b011011010, 9),
    makeup_code!(1408, 0b011011011, 9),
    makeup_code!(1472, 0b010011000, 9),
    makeup_code!(1536, 0b010011001, 9),
    makeup_code!(1600, 0b010011010, 9),
    makeup_code!(1664, 0b011000, 6),
    makeup_code!(1728, 0b010011011, 9),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const BLACK_MAKEUP: [RunCode; 27] = [
    makeup_code!(64, 0b0000001111, 10),
    makeup_code!(128, 0b000011001000, 12),
    makeup_code!(192, 0b000011001001, 12),
    makeup_code!(256, 0b000001011011, 12),
    makeup_code!(320, 0b000000110011, 12),
    makeup_code!(384, 0b000000110100, 12),
    makeup_code!(448, 0b000000110101, 12),
    makeup_code!(512, 0b0000001101100, 13),
    makeup_code!(576, 0b0000001101101, 13),
    makeup_code!(640, 0b0000001001010, 13),
    makeup_code!(704, 0b0000001001011, 13),
    makeup_code!(768, 0b0000001001100, 13),
    makeup_code!(832, 0b0000001001101, 13),
    makeup_code!(896, 0b0000001110010, 13),
    makeup_code!(960, 0b0000001110011, 13),
    makeup_code!(1024, 0b0000001110100, 13),
    makeup_code!(1088, 0b0000001110101, 13),
    makeup_code!(1152, 0b0000001110110, 13),
    makeup_code!(1216, 0b0000001110111, 13),
    makeup_code!(1280, 0b0000001010010, 13),
    makeup_code!(1344, 0b0000001010011, 13),
    makeup_code!(1408, 0b0000001010100, 13),
    makeup_code!(1472, 0b0000001010101, 13),
    makeup_code!(1536, 0b0000001011010, 13),
    makeup_code!(1600, 0b0000001011011, 13),
    makeup_code!(1664, 0b0000001100100, 13),
    makeup_code!(1728, 0b0000001100101, 13),
];

// ITU-T T.6, Table 3: make-up codes shared by white and black.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const COMMON_MAKEUP: [RunCode; 13] = [
    makeup_code!(1792, 0b00000001000, 11),
    makeup_code!(1856, 0b00000001100, 11),
    makeup_code!(1920, 0b00000001101, 11),
    makeup_code!(1984, 0b000000010010, 12),
    makeup_code!(2048, 0b000000010011, 12),
    makeup_code!(2112, 0b000000010100, 12),
    makeup_code!(2176, 0b000000010101, 12),
    makeup_code!(2240, 0b000000010110, 12),
    makeup_code!(2304, 0b000000010111, 12),
    makeup_code!(2368, 0b000000011100, 12),
    makeup_code!(2432, 0b000000011101, 12),
    makeup_code!(2496, 0b000000011110, 12),
    makeup_code!(2560, 0b000000011111, 12),
];
