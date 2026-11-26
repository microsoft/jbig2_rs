#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/shared/bitmap.rs"]
pub(crate) mod bitmap_shared;

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../proofs/spec_properties/bitmap.rs"]
mod bitmap_properties;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum CombinationOperator {
    Or,
    And,
    Xor,
    Xnor,
    Replace,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct RowBlit<'a> {
    pub(crate) source: &'a Bitmap,
    pub(crate) source_x: u32,
    pub(crate) destination_x: u32,
    pub(crate) width: u32,
}

/// An owned row-major, one-bit-per-pixel bitmap.
///
/// Within each byte, the leftmost pixel is stored in the most significant bit.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct Bitmap {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: usize,
    pub(crate) data: Vec<u8>,
}

mod access;
mod chunks;
mod compose;
mod fast_compose;
mod operators;
mod region;
mod storage;

#[cfg(test)]
mod tests;
