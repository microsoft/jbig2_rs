//! Finite canonical metadata, checked against the existing Annex B line constants.

#[cfg(verus_keep_ghost)]
use super::standard_spec;
use super::{StandardDescriptor, StandardHuffmanTable};
use crate::huffman::StandardLines;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

macro_rules! descriptor {
    ($name:ident, $max:expr, $codes:expr, $counts:expr, $starts:expr, $order:expr) => {
        #[cfg_attr(verus_keep_ghost, verus_verify)]
        pub(super) const $name: StandardDescriptor = StandardDescriptor {
            lines: StandardLines::$name,
            first_code: $codes,
            length_count: $counts,
            first_line: $starts,
            ordered_lines: $order,
            max_prefix_len: $max,
        };
    };
}

descriptor!(
    A,
    3,
    [0, 0, 2, 6, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 2, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 2, 0, 0, 0, 0, 0, 0],
    [0, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    B,
    6,
    [0, 0, 2, 6, 14, 30, 62, 0, 0, 0],
    [0, 1, 1, 1, 1, 1, 2, 0, 0, 0],
    [0, 0, 1, 2, 3, 4, 5, 0, 0, 0],
    [0, 1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    C,
    8,
    [0, 0, 2, 6, 14, 30, 62, 126, 254, 0],
    [0, 1, 1, 1, 1, 1, 1, 1, 2, 0],
    [0, 0, 1, 2, 3, 4, 5, 6, 7, 0],
    [1, 2, 3, 4, 5, 8, 7, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    D,
    5,
    [0, 0, 2, 6, 14, 30, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 2, 0, 0, 0, 0],
    [0, 0, 1, 2, 3, 4, 0, 0, 0, 0],
    [0, 1, 2, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    E,
    7,
    [0, 0, 2, 6, 14, 30, 62, 126, 0, 0],
    [0, 1, 1, 1, 1, 1, 1, 2, 0, 0],
    [0, 0, 1, 2, 3, 4, 5, 6, 0, 0],
    [1, 2, 3, 4, 5, 7, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    F,
    6,
    [0, 0, 0, 2, 8, 28, 62, 0, 0, 0],
    [0, 0, 1, 2, 6, 3, 2, 0, 0, 0],
    [0, 0, 0, 1, 3, 9, 12, 0, 0, 0],
    [7, 8, 9, 1, 2, 3, 6, 10, 11, 0, 4, 5, 12, 13, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    G,
    5,
    [0, 0, 0, 0, 8, 26, 0, 0, 0, 0],
    [0, 0, 0, 4, 5, 6, 0, 0, 0, 0],
    [0, 0, 0, 0, 4, 9, 0, 0, 0, 0],
    [1, 10, 11, 12, 0, 2, 5, 6, 9, 3, 4, 7, 8, 13, 14, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    H,
    9,
    [0, 0, 0, 4, 10, 26, 58, 124, 252, 508],
    [0, 0, 2, 1, 3, 3, 4, 2, 2, 4],
    [0, 0, 0, 2, 3, 6, 9, 13, 15, 17],
    [6, 20, 9, 5, 11, 12, 7, 13, 14, 8, 10, 15, 17, 4, 16, 0, 2, 1, 3, 18, 19, 0]
);
descriptor!(
    I,
    9,
    [0, 0, 0, 2, 10, 26, 58, 124, 252, 508],
    [0, 0, 1, 3, 3, 3, 4, 2, 2, 4],
    [0, 0, 0, 1, 4, 7, 10, 14, 16, 18],
    [21, 6, 7, 10, 5, 12, 13, 8, 14, 15, 9, 11, 16, 18, 4, 17, 0, 2, 1, 3, 19, 20]
);
descriptor!(
    J,
    8,
    [0, 0, 0, 6, 12, 24, 54, 122, 252, 0],
    [0, 0, 3, 0, 0, 3, 7, 4, 4, 0],
    [0, 0, 0, 0, 0, 3, 6, 13, 17, 0],
    [4, 9, 20, 3, 5, 10, 6, 11, 12, 13, 14, 15, 16, 0, 2, 7, 17, 1, 8, 18, 19, 0]
);
descriptor!(
    K,
    7,
    [0, 0, 2, 6, 12, 28, 60, 122, 0, 0],
    [0, 1, 1, 0, 2, 2, 1, 6, 0, 0],
    [0, 0, 1, 0, 2, 4, 6, 7, 0, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    L,
    8,
    [0, 0, 2, 6, 14, 28, 60, 122, 254, 0],
    [0, 1, 1, 1, 0, 2, 1, 5, 2, 0],
    [0, 0, 1, 2, 0, 3, 5, 6, 11, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    M,
    7,
    [0, 0, 2, 4, 12, 28, 58, 126, 0, 0],
    [0, 1, 0, 2, 2, 1, 5, 2, 0, 0],
    [0, 0, 0, 1, 3, 5, 6, 11, 0, 0],
    [0, 1, 5, 2, 4, 3, 6, 7, 8, 9, 10, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    N,
    3,
    [0, 0, 2, 4, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, 4, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
    [2, 0, 1, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);
descriptor!(
    O,
    7,
    [0, 0, 2, 4, 12, 28, 60, 124, 0, 0],
    [0, 1, 0, 2, 2, 2, 2, 4, 0, 0],
    [0, 0, 0, 1, 3, 5, 7, 9, 0, 0],
    [5, 4, 6, 3, 7, 2, 8, 1, 9, 0, 10, 11, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0]
);

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures *out == *standard_spec::selected(table), standard_spec::canonical(out),
))]
pub(super) fn select(table: StandardHuffmanTable) -> &'static StandardDescriptor {
    #[cfg(verus_keep_ghost)]
    proof! {
        standard_spec::all_canonical();
        standard_spec::selected_definition(table);
    }
    match table {
        StandardHuffmanTable::A => &A,
        StandardHuffmanTable::B => &B,
        StandardHuffmanTable::C => &C,
        StandardHuffmanTable::D => &D,
        StandardHuffmanTable::E => &E,
        StandardHuffmanTable::F => &F,
        StandardHuffmanTable::G => &G,
        StandardHuffmanTable::H => &H,
        StandardHuffmanTable::I => &I,
        StandardHuffmanTable::J => &J,
        StandardHuffmanTable::K => &K,
        StandardHuffmanTable::L => &L,
        StandardHuffmanTable::M => &M,
        StandardHuffmanTable::N => &N,
        StandardHuffmanTable::O => &O,
    }
}
