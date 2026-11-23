//! JBIG2 Huffman table construction and decoding (T.88, Annex B).

use core::mem::size_of;

use crate::reader::{bit_reader, BitReader, ByteReader};
use crate::{
    DecodeError, HuffmanErrorKind as HuffmanTableError, LimitExceeded, LimitKind, Limits,
    MalformedError,
};
#[cfg(verus_keep_ghost)]
use vstd::layout;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::convert::FromSpecImpl;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/huffman.rs"]
pub(crate) mod huffman_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../specs/huffman_lookup.rs"]
pub(crate) mod huffman_lookup_spec;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/huffman.rs"]
mod huffman_shared;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/huffman.rs"]
mod huffman_properties;

#[cfg(verus_keep_ghost)]
#[macro_use]
#[rustfmt::skip]
#[path = "../proofs/production_impl/huffman.rs"]
mod huffman_proofs;

#[cfg_attr(verus_keep_ghost, verus_verify)]
const MAX_PREFIX_LEN: usize = 255;
#[cfg_attr(verus_keep_ghost, verus_verify)]
const PREFIX_LOOKUP_BITS: u8 = 8;
#[cfg_attr(verus_keep_ghost, verus_verify)]
const PREFIX_LOOKUP_SIZE: usize = 1 << PREFIX_LOOKUP_BITS;
#[cfg_attr(verus_keep_ghost, verus_verify)]
const MIN_FAST_PREFIX_LEN: usize = 8;

/// Error from Huffman table parsing, construction, or value decoding.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum HuffmanError {
    Decode(DecodeError),
    InvalidTable(HuffmanTableError),
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<DecodeError> for HuffmanError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<MalformedError> for HuffmanError {
    fn from(error: MalformedError) -> Self {
        Self::Decode(error.into())
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<LimitExceeded> for HuffmanError {
    fn from(error: LimitExceeded) -> Self {
        Self::Decode(error.into())
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
impl From<HuffmanError> for DecodeError {
    fn from(error: HuffmanError) -> Self {
        #[cfg(verus_keep_ghost)]
        ensures(|out: Self| equal(out, huffman_spec::huffman_error_result(&error)));
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_spec::huffman_error_result_definition(&error);
        }
        match error {
            HuffmanError::Decode(error) => error,
            HuffmanError::InvalidTable(kind) => {
                Self::Malformed(MalformedError::InvalidHuffman(kind))
            }
        }
    }
}

#[cfg(verus_keep_ghost)]
verus! {

impl FromSpecImpl<DecodeError> for HuffmanError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: DecodeError) -> HuffmanError {
        HuffmanError::Decode(error)
    }
}

impl FromSpecImpl<MalformedError> for HuffmanError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: MalformedError) -> HuffmanError {
        HuffmanError::Decode(DecodeError::Malformed(error))
    }
}

impl FromSpecImpl<LimitExceeded> for HuffmanError {
    open spec fn obeys_from_spec() -> bool { true }

    closed spec fn from_spec(error: LimitExceeded) -> HuffmanError {
        HuffmanError::Decode(DecodeError::LimitExceeded(error))
    }
}

// Project error policy, discharged by the executing From body through the pinned trait contract.
impl FromSpecImpl<HuffmanError> for DecodeError {
    open spec fn obeys_from_spec() -> bool {
        true
    }

    closed spec fn from_spec(error: HuffmanError) -> DecodeError {
        huffman_spec::huffman_error_result(&error)
    }
}

}

/// The result of decoding one Huffman-coded value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum HuffmanValue {
    Integer(i32),
    OutOfBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum LineKind {
    Normal,
    Lower,
    Upper,
    OutOfBand,
}

/// One line of a JBIG2 Huffman table before canonical codes are assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct HuffmanLine {
    pub(crate) prefix_len: u8,
    pub(crate) range_len: u8,
    pub(crate) range_low: i64,
    pub(crate) kind: LineKind,
}

impl HuffmanLine {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) const fn normal(range_low: i32, prefix_len: u8, range_len: u8) -> Self {
        Self {
            prefix_len,
            range_len,
            range_low: range_low as i64,
            kind: LineKind::Normal,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) const fn lower(range_low: i64, prefix_len: u8) -> Self {
        Self {
            prefix_len,
            range_len: 32,
            range_low,
            kind: LineKind::Lower,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) const fn upper(range_low: i32, prefix_len: u8) -> Self {
        Self {
            prefix_len,
            range_len: 32,
            range_low: range_low as i64,
            kind: LineKind::Upper,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(crate) const fn out_of_band(prefix_len: u8) -> Self {
        Self {
            prefix_len,
            range_len: 0,
            range_low: 0,
            kind: LineKind::OutOfBand,
        }
    }
}

/// One of the fifteen standard Huffman tables in T.88 Annex B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum StandardHuffmanTable {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
struct WideCode {
    // Little-endian limbs. Annex B permits prefix lengths through 255 bits.
    limbs: [u64; 4],
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum HuffmanLines<'a> {
    Borrowed(&'a [HuffmanLine]),
    SymbolLengths(&'a [u8]),
    Standard(StandardLines),
}

impl HuffmanLines<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures huffman_spec::is_raw(self) ==> out == huffman_spec::raw_lines(self).len(),
    ))]
    fn len(self) -> usize {
        match self {
            Self::Borrowed(lines) => lines.len(),
            Self::SymbolLengths(lengths) => lengths.len(),
            Self::Standard(lines) => lines.len(),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures huffman_spec::is_raw(self) ==> out ==
            if index < huffman_spec::raw_lines(self).len() {
                Some(huffman_spec::raw_lines(self)[index as int])
            } else { None },
    ))]
    fn line(self, index: usize) -> Option<HuffmanLine> {
        match self {
            Self::Borrowed(lines) => {
                if index < lines.len() {
                    Some(lines[index])
                } else {
                    None
                }
            }
            Self::SymbolLengths(lengths) => {
                if index < lengths.len() {
                    i32::try_from(index)
                        .ok()
                        .map(|value| HuffmanLine::normal(value, lengths[index], 0))
                } else {
                    None
                }
            }
            Self::Standard(lines) => lines.line(index),
        }
    }
}

/// A validated table with canonical prefix codes assigned as in T.88 B.3.
#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct HuffmanTable {
    lines: Vec<HuffmanLine>,
    ordered_lines: Vec<u32>,
    length_count: [u32; MAX_PREFIX_LEN + 1],
    first_code: [WideCode; MAX_PREFIX_LEN + 1],
    first_line: [u32; MAX_PREFIX_LEN + 1],
    prefix_lookup: Vec<u16>,
    prefer_prefix_lookup: bool,
    max_prefix_len: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
enum StandardLines {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
}

impl StandardLines {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures self == StandardLines::A ==> out == 4,
    ))]
    const fn len(self) -> usize {
        match self {
            Self::A => 4,
            Self::B => 7,
            Self::C => 9,
            Self::D => 6,
            Self::E => 8,
            Self::F => 14,
            Self::G => 15,
            Self::H | Self::J => 21,
            Self::I => 22,
            Self::K | Self::L | Self::M | Self::O => 13,
            Self::N => 5,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures self == StandardLines::A ==> out ==
            if index < 4 { Some(TABLE_A@[index as int]) } else { None },
    ))]
    fn line(self, index: usize) -> Option<HuffmanLine> {
        macro_rules! get {
            ($table:expr) => {
                if index < $table.len() {
                    Some($table[index])
                } else {
                    None
                }
            };
        }
        match self {
            Self::A => get!(TABLE_A),
            Self::B => get!(TABLE_B),
            Self::C => get!(TABLE_C),
            Self::D => get!(TABLE_D),
            Self::E => get!(TABLE_E),
            Self::F => get!(TABLE_F),
            Self::G => get!(TABLE_G),
            Self::H => get!(TABLE_H),
            Self::I => get!(TABLE_I),
            Self::J => get!(TABLE_J),
            Self::K => get!(TABLE_K),
            Self::L => get!(TABLE_L),
            Self::M => get!(TABLE_M),
            Self::N => get!(TABLE_N),
            Self::O => get!(TABLE_O),
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures table == StandardHuffmanTable::A ==> out == StandardLines::A,
))]
fn standard_lines(table: StandardHuffmanTable) -> StandardLines {
    match table {
        StandardHuffmanTable::A => StandardLines::A,
        StandardHuffmanTable::B => StandardLines::B,
        StandardHuffmanTable::C => StandardLines::C,
        StandardHuffmanTable::D => StandardLines::D,
        StandardHuffmanTable::E => StandardLines::E,
        StandardHuffmanTable::F => StandardLines::F,
        StandardHuffmanTable::G => StandardLines::G,
        StandardHuffmanTable::H => StandardLines::H,
        StandardHuffmanTable::I => StandardLines::I,
        StandardHuffmanTable::J => StandardLines::J,
        StandardHuffmanTable::K => StandardLines::K,
        StandardHuffmanTable::L => StandardLines::L,
        StandardHuffmanTable::M => StandardLines::M,
        StandardHuffmanTable::N => StandardLines::N,
        StandardHuffmanTable::O => StandardLines::O,
    }
}

mod standard_a;
pub(crate) use standard_a::{CodecHuffmanTable, StandardADescriptor};

mod construction;
mod decoding;
mod tables;
mod validation;
mod wide;

// Private aliases also serve cfg-gated proof and test callers.
#[allow(unused_imports)]
use self::{
    decoding::{
        build_prefix_lookup, canonical_first_code, canonical_range_fits, decode_line,
        next_prefix_length, peek_prefix,
    },
    tables::{
        TABLE_A, TABLE_B, TABLE_C, TABLE_D, TABLE_E, TABLE_F, TABLE_G, TABLE_H, TABLE_I, TABLE_J,
        TABLE_K, TABLE_L, TABLE_M, TABLE_N, TABLE_O,
    },
    validation::{
        allocation_size, check_table_memory, encoded_table_fields, line_count_exceeds_symbol_limit,
        line_count_fits_u32, padding_bit_count, push_parsed_line, read_u8_bits, table_memory,
        try_reserve, validate_lines,
    },
    wide::subtract_limb,
};

#[cfg(test)]
mod tests;
