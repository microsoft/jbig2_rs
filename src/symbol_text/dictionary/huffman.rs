//! Huffman implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    assemble_exported_symbols, associate_dictionary_tables, bitmap_from_packed_bytes, byte_align,
    crop_bitmap_columns, decode_export_runs, decode_mmr, decode_one_new_symbol,
    materialize_huffman_table, new_huffman_codec, refinement_template_from_u8, required_integer,
    select_dictionary_tables, symbol_id_code_length, wrap_custom_tables, BitReader, Bitmap,
    DecodeError, DictionaryClassState, EntropyInteger, EofbPolicy, HuffmanChoiceSet,
    HuffmanFieldTables, HuffmanTable, IntegerField, Limits, MalformedError,
    RefinementContextSnapshot, ResourceTracker, StandardHuffmanTables, SymbolCodebook, SymbolCodec,
    SymbolDictionaryHeader, SymbolTextError,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::huffman::huffman_spec::reader_valid(old(reader)),
    ensures
        crate::huffman::huffman_spec::reader_valid(final(reader)),
        match out {
            Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
            Err(_) => true,
        },
))]
pub(in crate::symbol_text) fn decode_collective_bitmap(
    reader: &mut BitReader<'_>,
    bitmap_size: u32,
    width: u32,
    height: u32,
    limits: &Limits,
) -> Result<Bitmap, SymbolTextError> {
    byte_align(reader)?;
    let byte_offset = reader
        .bit_position
        .checked_div(8)
        .ok_or(MalformedError::IntegerOverflow)?;
    let length = if bitmap_size == 0 {
        (u64::from(width)
            .checked_add(7)
            .ok_or(MalformedError::IntegerOverflow)?
            / 8)
        .checked_mul(u64::from(height))
        .ok_or(MalformedError::IntegerOverflow)?
    } else {
        u64::from(bitmap_size)
    };
    let length_usize = usize::try_from(length).map_err(|_| MalformedError::IntegerOverflow)?;
    let end = byte_offset
        .checked_add(length_usize)
        .ok_or(MalformedError::IntegerOverflow)?;
    let remaining = if byte_offset <= reader.bytes.len() {
        reader.bytes.len() - byte_offset
    } else {
        0
    };
    let source = reader
        .bytes
        .get(byte_offset..end)
        .ok_or(MalformedError::UnexpectedEof {
            offset: u64::try_from(byte_offset).map_err(|_| MalformedError::IntegerOverflow)?,
            needed: length,
            remaining: u64::try_from(remaining).map_err(|_| MalformedError::IntegerOverflow)?,
        })?;
    let bitmap = if bitmap_size == 0 {
        bitmap_from_packed_bytes(width, height, source, limits)?
    } else {
        decode_mmr(source, width, height, limits, EofbPolicy::Optional)?
    };
    reader.bit_position = end.checked_mul(8).ok_or(MalformedError::IntegerOverflow)?;
    Ok(bitmap)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codebook_valid(imported),
        header.adaptive_count <= 4,
        total_symbols as nat
            == symbol_text_spec::codebook_len(imported) + header.new_symbols as nat,
    ensures match out {
        Ok((exported, _, consumed)) =>
            symbol_text_spec::bitmaps_valid(exported@)
                && consumed == body@.len(),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn decode_symbol_dictionary_huffman(
    header: &SymbolDictionaryHeader,
    imported: SymbolCodebook<'_>,
    body: &[u8],
    total_symbols: u32,
    refinement_adaptive: [(i8, i8); 2],
    custom_tables: &[&HuffmanTable],
    limits: &Limits,
) -> Result<(Vec<Bitmap>, Option<RefinementContextSnapshot>, usize), SymbolTextError> {
    let config = header.config;
    // T.88 6.5.8.2.3: implicit fixed-width IDs, constant for this dictionary.
    let code_length = symbol_id_code_length(total_symbols)?.max(1);
    let custom_wrapped = wrap_custom_tables(custom_tables)?;
    let bindings = associate_dictionary_tables(&config, &custom_wrapped)?;
    let choices = select_dictionary_tables(&config, &bindings)?;
    let tables = materialize_huffman_tables!(HuffmanChoiceSet::Dictionary(choices), limits)?;
    let refinement_template = config.refinement_aggregation.then(|| {
        #[cfg(verus_keep_ghost)]
        ensures(|out: crate::generic_region::RefinementTemplate| {
            equal(
                out,
                if config.refinement_template == 0 {
                    crate::generic_region::RefinementTemplate::Template0
                } else {
                    crate::generic_region::RefinementTemplate::Template1
                },
            )
        });
        refinement_template_from_u8(config.refinement_template)
    });
    let mut codec = SymbolCodec::Huffman(new_huffman_codec(
        body,
        tables,
        0,
        refinement_template,
        limits,
    )?);
    #[cfg(verus_keep_ghost)]
    production_huffman_segment_setup!(
        codec,
        body,
        0,
        config.refinement_aggregation,
        config.refinement_template
    );
    let mut resources = ResourceTracker {
        work_units: 0,
        decoded_bitmap_bytes: 0,
    };

    let new_symbol_capacity =
        usize::try_from(header.new_symbols).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut new_symbols: Vec<Bitmap> = Vec::new();
    new_symbols
        .try_reserve(new_symbol_capacity)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: u64::from(header.new_symbols),
        })?;

    let mut class_state = DictionaryClassState::new(header.new_symbols, limits)?;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            new_symbols@.len() <= header.new_symbols as nat,
            new_symbol_capacity == header.new_symbols as nat,
            class_state.decoded_symbols as nat == new_symbols@.len(),
            class_state.declared_new_symbols == header.new_symbols,
            !class_state.class_open,
            header.adaptive_count <= 4,
            symbol_text_spec::dictionary_state_valid(&class_state),
            symbol_text_spec::codebook_valid(imported),
            symbol_text_spec::bitmaps_valid(new_symbols@),
            symbol_text_spec::codec_valid(&codec),
            symbol_text_spec::work_units(&resources) <= limits.max_work_units,
        decreases limits.max_work_units - symbol_text_spec::work_units(&resources),
    ))]
    while u32::try_from(new_symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?
        < header.new_symbols
    {
        #[cfg_attr(not(verus_keep_ghost), allow(unused_variables))]
        let class_start_work = resources.work_units;
        let height_delta = required_integer(&mut codec, IntegerField::HeightDelta)?;
        let height = class_state.begin_height_class(height_delta, limits, &mut resources)?;

        if config.refinement_aggregation {
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    new_symbols@.len() <= header.new_symbols as nat,
                    new_symbol_capacity == header.new_symbols as nat,
                    class_state.decoded_symbols as nat == new_symbols@.len(),
                    class_state.declared_new_symbols == header.new_symbols,
                    class_state.class_open,
                    header.adaptive_count <= 4,
                    symbol_text_spec::dictionary_state_valid(&class_state),
                    symbol_text_spec::codebook_valid(imported),
                    symbol_text_spec::bitmaps_valid(new_symbols@),
                    symbol_text_spec::codec_valid(&codec),
                    symbol_text_spec::work_units(&resources)
                        <= limits.max_work_units,
                    resources.work_units > class_start_work,
                decreases limits.max_work_units
                    - symbol_text_spec::work_units(&resources),
            ))]
            while let EntropyInteger::Value(width_delta) =
                codec.decode_integer(IntegerField::WidthDelta)?
            {
                let (width, height) =
                    class_state.add_symbol(width_delta, limits, &mut resources)?;
                let instances = required_integer(&mut codec, IntegerField::AggregationInstances)?;
                let instances = u32::try_from(instances)
                    .map_err(|_| SymbolTextError::InvalidEntropyValue("aggregation instances"))?;
                let bitmap = decode_one_new_symbol(
                    header,
                    &mut codec,
                    imported,
                    &new_symbols,
                    None,
                    &mut 0,
                    Some(instances),
                    width,
                    height,
                    code_length,
                    refinement_adaptive,
                    limits,
                    &mut resources,
                )?;
                if new_symbols.len() >= new_symbol_capacity {
                    return Err(SymbolTextError::InvalidCount("too many new symbols"));
                }
                new_symbols.push(bitmap);
            }
        } else {
            // T.88 6.5.9: collect widths for one height class, then decode a
            // single collective bitmap and split it by recorded width.
            let mut widths: Vec<u32> = Vec::new();
            let decoded_count =
                u32::try_from(new_symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?;
            let remaining = header
                .new_symbols
                .checked_sub(decoded_count)
                .and_then(|count| usize::try_from(count).ok())
                .ok_or(MalformedError::IntegerOverflow)?;
            let requested_width_bytes = remaining
                .checked_mul(core::mem::size_of::<u32>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or(MalformedError::IntegerOverflow)?;
            widths
                .try_reserve(remaining)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: requested_width_bytes,
                })?;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    widths@.len() <= remaining,
                    new_symbols@.len() + widths@.len()
                        == class_state.decoded_symbols as nat,
                    class_state.declared_new_symbols == header.new_symbols,
                    class_state.decoded_symbols <= class_state.declared_new_symbols,
                    class_state.class_open,
                    symbol_text_spec::dictionary_state_valid(&class_state),
                    symbol_text_spec::work_units(&resources)
                        <= limits.max_work_units,
                    resources.work_units > class_start_work,
                    symbol_text_spec::codec_valid(&codec),
                decreases limits.max_work_units
                    - symbol_text_spec::work_units(&resources),
            ))]
            while let EntropyInteger::Value(width_delta) =
                codec.decode_integer(IntegerField::WidthDelta)?
            {
                let (width, _) = class_state.add_symbol(width_delta, limits, &mut resources)?;
                if widths.len() >= remaining {
                    return Err(SymbolTextError::InvalidCount("too many new symbols"));
                }
                widths.push(width);
            }
            let mut total_width = 0_u32;
            let mut width_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant width_index <= widths@.len(),
                decreases widths@.len() - width_index,
            ))]
            while width_index < widths.len() {
                total_width = total_width
                    .checked_add(widths[width_index])
                    .ok_or(MalformedError::IntegerOverflow)?;
                width_index += 1;
            }
            let bitmap_size = match codec.decode_integer(IntegerField::DictionaryBitmapSize)? {
                EntropyInteger::Value(value) => u32::try_from(value)
                    .map_err(|_| SymbolTextError::InvalidEntropyValue("bitmap size"))?,
                EntropyInteger::OutOfBand => {
                    return Err(SymbolTextError::UnexpectedOutOfBand(
                        "collective bitmap size",
                    ))
                }
            };
            let SymbolCodec::Huffman(huffman) = &mut codec else {
                return Err(SymbolTextError::InvalidState(
                    "Huffman dictionary has non-Huffman state",
                ));
            };
            let collective = decode_collective_bitmap(
                &mut huffman.reader,
                bitmap_size,
                total_width,
                height,
                limits,
            )?;
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(symbol_text_spec::codec_valid);
            }
            resources.charge_bitmap(total_width, height, limits)?;

            if widths.len() == 1 {
                if new_symbols.len() >= new_symbol_capacity {
                    return Err(SymbolTextError::InvalidCount("too many new symbols"));
                }
                new_symbols.push(collective);
            } else {
                let mut x_start = 0_u32;
                #[cfg_attr(not(verus_keep_ghost), allow(unused_variables))]
                let symbols_before = new_symbols.len();
                let mut crop_index = 0_usize;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        crop_index <= widths@.len(),
                        new_symbols@.len() == symbols_before + crop_index,
                        symbol_text_spec::bitmaps_valid(new_symbols@),
                        crate::spec::bitmap::storage_valid(&collective),
                    decreases widths@.len() - crop_index,
                ))]
                while crop_index < widths.len() {
                    let width = widths[crop_index];
                    let cropped = crop_bitmap_columns(&collective, x_start, width, limits)?;
                    if new_symbols.len() >= new_symbol_capacity {
                        return Err(SymbolTextError::InvalidCount("too many new symbols"));
                    }
                    new_symbols.push(cropped);
                    x_start = x_start
                        .checked_add(width)
                        .ok_or(MalformedError::IntegerOverflow)?;
                    crop_index += 1;
                }
            }
        }
        class_state.finish_height_class()?;
    }
    class_state.finish()?;

    let exported = decode_export_runs(
        &mut codec,
        total_symbols,
        header.exported_symbols,
        limits,
        &mut resources,
    )?;

    let consumed_within_body = codec
        .huffman_mut()?
        .reader
        .bit_position
        .checked_add(7)
        .ok_or(MalformedError::IntegerOverflow)?
        / 8;
    if consumed_within_body != body.len() {
        return Err(SymbolTextError::InvalidState(
            "Huffman symbol dictionary did not exactly consume its segment data",
        ));
    }

    let SymbolCodec::Huffman(huffman) = &codec else {
        return Err(SymbolTextError::InvalidState(
            "Huffman dictionary has non-Huffman state",
        ));
    };
    let retained_refinement = match huffman.refinement.as_ref() {
        Some(contexts) => Some(contexts.try_snapshot(limits)?),
        None => None,
    };
    let exported_symbols = assemble_exported_symbols(imported, &new_symbols, &exported, limits)?;
    Ok((exported_symbols, retained_refinement, consumed_within_body))
}
