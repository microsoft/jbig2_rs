// Exact input to both owned-standard construction and the codec descriptor.
verus! {

pub(super) open spec fn standard_source_tag(table: super::StandardHuffmanTable) -> super::StandardLines {
    match table {
        super::StandardHuffmanTable::A => super::StandardLines::A,
        super::StandardHuffmanTable::B => super::StandardLines::B,
        super::StandardHuffmanTable::C => super::StandardLines::C,
        super::StandardHuffmanTable::D => super::StandardLines::D,
        super::StandardHuffmanTable::E => super::StandardLines::E,
        super::StandardHuffmanTable::F => super::StandardLines::F,
        super::StandardHuffmanTable::G => super::StandardLines::G,
        super::StandardHuffmanTable::H => super::StandardLines::H,
        super::StandardHuffmanTable::I => super::StandardLines::I,
        super::StandardHuffmanTable::J => super::StandardLines::J,
        super::StandardHuffmanTable::K => super::StandardLines::K,
        super::StandardHuffmanTable::L => super::StandardLines::L,
        super::StandardHuffmanTable::M => super::StandardLines::M,
        super::StandardHuffmanTable::N => super::StandardLines::N,
        super::StandardHuffmanTable::O => super::StandardLines::O,
    }
}

pub(super) open spec fn standard_source_lines(table: super::StandardLines) -> Seq<super::HuffmanLine> {
    match table {
        super::StandardLines::A => super::TABLE_A@,
        super::StandardLines::B => super::TABLE_B@,
        super::StandardLines::C => super::TABLE_C@,
        super::StandardLines::D => super::TABLE_D@,
        super::StandardLines::E => super::TABLE_E@,
        super::StandardLines::F => super::TABLE_F@,
        super::StandardLines::G => super::TABLE_G@,
        super::StandardLines::H => super::TABLE_H@,
        super::StandardLines::I => super::TABLE_I@,
        super::StandardLines::J => super::TABLE_J@,
        super::StandardLines::K => super::TABLE_K@,
        super::StandardLines::L => super::TABLE_L@,
        super::StandardLines::M => super::TABLE_M@,
        super::StandardLines::N => super::TABLE_N@,
        super::StandardLines::O => super::TABLE_O@,
    }
}

pub(crate) closed spec fn standard_table_lines(table: super::StandardHuffmanTable) -> Seq<super::HuffmanLine> {
    standard_source_lines(standard_source_tag(table))
}

pub(super) proof fn standard_table_lines_definition(table: super::StandardHuffmanTable)
    ensures standard_table_lines(table) == standard_source_lines(standard_source_tag(table)),
{
    reveal(standard_table_lines);
}

}
