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
const SHORT_PREFIX_MAX: usize = 32;
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
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out.prefix_len == prefix_len,
            out.range_len == range_len,
            out.range_low == range_low as i64,
            out.kind == LineKind::Normal,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn normal(range_low: i32, prefix_len: u8, range_len: u8) -> Self {
        Self {
            prefix_len,
            range_len,
            range_low: range_low as i64,
            kind: LineKind::Normal,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out.prefix_len == prefix_len,
            out.range_len == 32,
            out.range_low == range_low,
            out.kind == LineKind::Lower,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn lower(range_low: i64, prefix_len: u8) -> Self {
        Self {
            prefix_len,
            range_len: 32,
            range_low,
            kind: LineKind::Lower,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out.prefix_len == prefix_len,
            out.range_len == 32,
            out.range_low == range_low as i64,
            out.kind == LineKind::Upper,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn upper(range_low: i32, prefix_len: u8) -> Self {
        Self {
            prefix_len,
            range_len: 32,
            range_low: range_low as i64,
            kind: LineKind::Upper,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out.prefix_len == prefix_len,
            out.range_len == 0,
            out.range_low == 0,
            out.kind == LineKind::OutOfBand,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
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
pub(crate) struct WideCode {
    // Little-endian limbs. Annex B permits prefix lengths through 255 bits.
    pub(crate) limbs: [u64; 4],
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum TableSlice<T: 'static> {
    Owned(Vec<T>),
    Static(&'static [T]),
}

impl<T: 'static> TableSlice<T> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out@ == huffman_spec::slice_view(self),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn as_slice(&self) -> &[T] {
        match self {
            Self::Owned(v) => v.as_slice(),
            Self::Static(s) => s,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == huffman_spec::slice_view(self).len(),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == if index < huffman_spec::slice_view(self).len() {
            Some(&huffman_spec::slice_view(self)[index as int])
        } else {
            None
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct LongPrefixTables {
    pub(crate) length_count: [u32; MAX_PREFIX_LEN + 1],
    pub(crate) first_code: [WideCode; MAX_PREFIX_LEN + 1],
    pub(crate) first_line: [u32; MAX_PREFIX_LEN + 1],
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
    pub(crate) lines: TableSlice<HuffmanLine>,
    pub(crate) ordered_lines: TableSlice<u32>,
    pub(crate) short_length_count: [u32; SHORT_PREFIX_MAX + 1],
    pub(crate) short_first_code: [u32; SHORT_PREFIX_MAX + 1],
    pub(crate) short_first_line: [u32; SHORT_PREFIX_MAX + 1],
    pub(crate) long_tables: Option<Box<LongPrefixTables>>,
    pub(crate) prefix_lookup: TableSlice<u16>,
    pub(crate) prefer_prefix_lookup: bool,
    pub(crate) max_prefix_len: u8,
}

impl HuffmanTable {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires length <= 255usize,
        ensures out == huffman_spec::table_length_counts(self)[length as int],
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn length_count_at(&self, length: usize) -> u32 {
        if length <= SHORT_PREFIX_MAX {
            self.short_length_count[length]
        } else {
            match &self.long_tables {
                Some(long) => long.length_count[length],
                None => 0,
            }
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires length <= 255usize,
        ensures out == huffman_spec::table_first_codes(self)[length as int],
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn first_code_at(&self, length: usize) -> WideCode {
        if length <= SHORT_PREFIX_MAX {
            WideCode {
                limbs: [u64::from(self.short_first_code[length]), 0, 0, 0],
            }
        } else {
            match &self.long_tables {
                Some(long) => long.first_code[length],
                None => WideCode {
                    limbs: [0, 0, 0, 0],
                },
            }
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires length <= 255usize,
        ensures out == huffman_spec::table_first_lines(self)[length as int],
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(super) fn first_line_at(&self, length: usize) -> u32 {
        if length <= SHORT_PREFIX_MAX {
            self.short_first_line[length]
        } else {
            match &self.long_tables {
                Some(long) => long.first_line[length],
                None => 0,
            }
        }
    }
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
        ensures 4 <= out <= 22,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
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
        ensures out.is_some() == match self {
            Self::A => index < 4,
            Self::B => index < 7,
            Self::C => index < 9,
            Self::D => index < 6,
            Self::E => index < 8,
            Self::F => index < 14,
            Self::G => index < 15,
            Self::H | Self::J => index < 21,
            Self::I => index < 22,
            Self::K | Self::L | Self::M | Self::O => index < 13,
            Self::N => index < 5,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
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
    ensures match table {
        StandardHuffmanTable::A => out == StandardLines::A,
        StandardHuffmanTable::B => out == StandardLines::B,
        StandardHuffmanTable::C => out == StandardLines::C,
        StandardHuffmanTable::D => out == StandardLines::D,
        StandardHuffmanTable::E => out == StandardLines::E,
        StandardHuffmanTable::F => out == StandardLines::F,
        StandardHuffmanTable::G => out == StandardLines::G,
        StandardHuffmanTable::H => out == StandardLines::H,
        StandardHuffmanTable::I => out == StandardLines::I,
        StandardHuffmanTable::J => out == StandardLines::J,
        StandardHuffmanTable::K => out == StandardLines::K,
        StandardHuffmanTable::L => out == StandardLines::L,
        StandardHuffmanTable::M => out == StandardLines::M,
        StandardHuffmanTable::N => out == StandardLines::N,
        StandardHuffmanTable::O => out == StandardLines::O,
    },
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

mod construction;
mod construction_long;
mod decoding;
mod lookup;
mod standard_static;
mod tables;
mod validation;
mod wide;

// Private aliases also serve cfg-gated proof and test callers.
#[allow(unused_imports)]
use self::{
    decoding::{
        canonical_first_code, canonical_range_fits, decode_line, next_prefix_length, peek_prefix,
    },
    lookup::{build_prefix_lookup, build_prefix_lookup_short},
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
