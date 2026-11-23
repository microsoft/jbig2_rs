use super::huffman_fields::codec;
use super::*;

#[test]
fn immutable_a_fields_keep_owned_values_errors_and_cursor_state() {
    let owned = HuffmanTable::standard(StandardHuffmanTable::A, &Limits::default()).unwrap();
    let limits = Limits {
        max_symbols: 4,
        max_decoder_managed_memory_bytes: 0,
        ..Limits::default()
    };
    let samples: [&[u8]; 5] = [
        &[],
        &[0],
        &[0xff],
        &[0xa5, 0x32, 0xdc, 0x4f, 0x80],
        &[0xff; 8],
    ];
    let a = Some(MaterializedHuffmanTable::Standard(StandardHuffmanTable::A));
    for bytes in samples {
        for start in 0..=bytes.len() * 8 {
            for field in [
                IntegerField::AggregationInstances,
                IntegerField::HeightDelta,
                IntegerField::WidthDelta,
                IntegerField::ExportRun,
                IntegerField::StripDelta,
                IntegerField::FirstS,
                IntegerField::SubsequentS,
                IntegerField::RefinementWidth,
                IntegerField::RefinementHeight,
                IntegerField::RefinementX,
                IntegerField::RefinementY,
                IntegerField::RefinementSize,
                IntegerField::DictionaryBitmapSize,
            ] {
                let mut actual = codec(bytes, start, 0);
                actual
                    .tables
                    .standard_tables
                    .materialize(StandardHuffmanTable::A, &limits)
                    .unwrap();
                actual.tables.aggregation_instances = a;
                actual.tables.height_delta = a;
                actual.tables.width_delta = a;
                actual.tables.export_run = a;
                actual.tables.strip_delta = a;
                actual.tables.first_s = a;
                actual.tables.subsequent_s = a;
                actual.tables.refinement_width = a;
                actual.tables.refinement_height = a;
                actual.tables.refinement_x = a;
                actual.tables.refinement_y = a;
                actual.tables.refinement_size = a;
                actual.tables.dictionary_bitmap_size = a;
                let mut reference = BitReader {
                    bytes,
                    bit_position: start,
                };
                let expected = owned
                    .decode(&mut reference)
                    .map(|value| match value {
                        HuffmanValue::Integer(value) => EntropyInteger::Value(i64::from(value)),
                        HuffmanValue::OutOfBand => EntropyInteger::OutOfBand,
                    })
                    .map_err(SymbolTextError::from);
                let decoded = actual.decode_integer(field);
                assert_eq!(format!("{decoded:?}"), format!("{expected:?}"));
                assert_eq!(actual.reader.bit_position, reference.bit_position);
                assert_eq!(actual.reader.bytes, bytes);
            }
        }
    }
    eprintln!(
        "standard codec cache bytes={}",
        core::mem::size_of::<StandardHuffmanTables>(),
    );
}
