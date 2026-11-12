//! Macros implementation.

macro_rules! materialize_huffman_tables {
    ($choice_set:expr, $limits:expr) => {{
        let choice_set = $choice_set;
        let limits = $limits;
        let mut standard_tables = StandardHuffmanTables::empty();
        macro_rules! resolve {
            ($choice:expr) => {
                materialize_huffman_table($choice, &mut standard_tables, limits)?
            };
        }

        let tables = match choice_set {
            HuffmanChoiceSet::Dictionary(choices) => {
                let height_delta = resolve!(choices.height_delta);
                let width_delta = resolve!(choices.width_delta);
                let export_run = resolve!(choices.export_run);
                let dictionary_bitmap_size = resolve!(choices.dictionary_bitmap_size);
                let aggregation_instances = resolve!(choices.aggregation_instances);
                let strip_delta = resolve!(choices.strip_delta);
                let first_s = resolve!(choices.first_s);
                let subsequent_s = resolve!(choices.subsequent_s);
                let refinement_width = resolve!(choices.refinement_width);
                let refinement_height = resolve!(choices.refinement_height);
                let refinement_x = resolve!(choices.refinement_x);
                let refinement_y = resolve!(choices.refinement_y);
                let refinement_size = resolve!(choices.refinement_size);
                HuffmanFieldTables {
                    standard_tables,
                    aggregation_instances,
                    height_delta,
                    width_delta,
                    export_run,
                    strip_delta,
                    first_s,
                    subsequent_s,
                    refinement_width,
                    refinement_height,
                    refinement_x,
                    refinement_y,
                    refinement_size,
                    dictionary_bitmap_size,
                }
            }
            HuffmanChoiceSet::TextRegion(choices) => {
                let first_s = resolve!(choices.first_s);
                let subsequent_s = resolve!(choices.subsequent_s);
                let strip_delta = resolve!(choices.strip_delta);
                let refinement_width = resolve!(choices.refinement_width);
                let refinement_height = resolve!(choices.refinement_height);
                let refinement_x = resolve!(choices.refinement_x);
                let refinement_y = resolve!(choices.refinement_y);
                let refinement_size = resolve!(choices.refinement_size);
                HuffmanFieldTables {
                    standard_tables,
                    aggregation_instances: None,
                    height_delta: None,
                    width_delta: None,
                    export_run: None,
                    strip_delta,
                    first_s,
                    subsequent_s,
                    refinement_width,
                    refinement_height,
                    refinement_x,
                    refinement_y,
                    refinement_size,
                    dictionary_bitmap_size: None,
                }
            }
        };
        Ok::<HuffmanFieldTables<'_>, SymbolTextError>(tables)
    }};
}

macro_rules! prepare_symbol_ids {
    ($codec:expr, $symbol_count:expr, $limits:expr) => {{
        let codec = $codec;
        let symbol_count = $symbol_count;
        let limits = $limits;
        let runcode_lines = read_symbol_id_runcode_lines(&mut codec.reader)?;
        let runcode_table = HuffmanTable::new(&runcode_lines, limits)?;
        let lengths =
            decode_symbol_id_code_lengths(&mut codec.reader, &runcode_table, symbol_count, limits)?;
        codec.symbol_id_table = Some(HuffmanTable::new_symbol_ids(&lengths, limits)?);
        byte_align(&mut codec.reader)?;
        Ok::<(), SymbolTextError>(())
    }};
}
