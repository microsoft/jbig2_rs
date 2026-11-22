//! Complete bounded text-region payload decoding.

#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::{
    associate_text_tables, byte_align, decode_symbol_id_code_lengths, decode_text_region_body,
    decode_text_region_body_with_end, log2_strip_size, materialize_huffman_table,
    new_arithmetic_codec, new_huffman_codec, normalized_bounded_arithmetic,
    parse_text_region_header, read_symbol_id_runcode_lines, refinement_template_from_u8,
    select_text_tables, symbol_id_code_length, validate_bounded_arithmetic_marker,
    wrap_custom_tables, HuffmanChoiceSet, HuffmanFieldTables, HuffmanTable, MalformedError,
    ResourceTracker, StandardHuffmanTables, SymbolCodec, SymbolTextError, TextRegionEnd,
    TextRegionInput, TextRegionOutput,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Decodes one complete text-region segment payload (T.88 6.4, 7.4.3).
#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::codebook_valid(input.symbols),
    ensures match out {
        Ok(output) =>
            crate::spec::bitmap::storage_valid(&output.bitmap)
                && output.consumed_bytes <= input.payload@.len()
                && (output.missing_final_huffman_oob ==> input.allow_missing_final_huffman_oob),
        Err(_) => true,
    },
))]
pub(crate) fn decode_text_region(
    input: &TextRegionInput<'_>,
) -> Result<TextRegionOutput, SymbolTextError> {
    let limits = input.limits;
    let (header, header_len) = parse_text_region_header(input.payload, limits)?;
    let config = header.config;
    let body = input
        .payload
        .get(header_len..)
        .ok_or(MalformedError::IntegerOverflow)?;

    let symbol_count =
        u32::try_from(input.symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?;
    let code_length = symbol_id_code_length(symbol_count)?;
    #[cfg(verus_keep_ghost)]
    production_segment_symbol_width!(
        symbol_count,
        code_length,
        symbol_text_spec::codebook_len(input.symbols)
    );

    let refinement_adaptive: [(i8, i8); 2] = [
        (
            i8::try_from(header.refinement_adaptive[0].x)
                .map_err(|_| MalformedError::IntegerOverflow)?,
            i8::try_from(header.refinement_adaptive[0].y)
                .map_err(|_| MalformedError::IntegerOverflow)?,
        ),
        (
            i8::try_from(header.refinement_adaptive[1].x)
                .map_err(|_| MalformedError::IntegerOverflow)?,
            i8::try_from(header.refinement_adaptive[1].y)
                .map_err(|_| MalformedError::IntegerOverflow)?,
        ),
    ];
    let mut resources = ResourceTracker {
        work_units: 0,
        decoded_bitmap_bytes: 0,
    };
    let refinement_template = refinement_template_from_u8(config.refinement_template);

    let mut end = if input.allow_missing_final_huffman_oob {
        TextRegionEnd::BoundedHuffman
    } else {
        TextRegionEnd::Strict
    };
    let (bitmap, consumed_within_body) = if config.huffman {
        let custom_wrapped = wrap_custom_tables(input.custom_tables)?;
        let bindings = associate_text_tables(&config, &custom_wrapped)?;
        let choices = select_text_tables(&bindings, config.refinement)?;
        let tables = materialize_huffman_tables!(HuffmanChoiceSet::TextRegion(choices), limits)?;
        let log_strip_size = log2_strip_size(config.strip_size);
        let mut codec = SymbolCodec::Huffman(new_huffman_codec(
            body,
            tables,
            log_strip_size,
            if config.refinement {
                Some(refinement_template)
            } else {
                None
            },
            limits,
        )?);
        #[cfg(verus_keep_ghost)]
        production_huffman_segment_setup!(
            codec,
            body,
            log_strip_size,
            config.refinement,
            config.refinement_template
        );
        let SymbolCodec::Huffman(huffman) = &mut codec else {
            return Err(SymbolTextError::InvalidState(
                "Huffman text region has non-Huffman state",
            ));
        };
        prepare_symbol_ids!(huffman, symbol_count, limits)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(symbol_text_spec::codec_valid);
        }

        let bitmap = decode_text_region_body_with_end(
            &mut codec,
            input.symbols,
            code_length,
            header.region.width,
            header.region.height,
            header.number_of_instances,
            &config,
            refinement_adaptive,
            limits,
            &mut resources,
            &mut end,
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
                "Huffman text region did not exactly consume its segment data",
            ));
        }
        (bitmap, consumed_within_body)
    } else {
        if !input.custom_tables.is_empty() {
            return Err(SymbolTextError::InvalidTableAssociation(
                "custom tables on an arithmetic text region",
            ));
        }
        let normalized = normalized_bounded_arithmetic(body, limits)?;
        let mut codec = SymbolCodec::Arithmetic(new_arithmetic_codec(
            normalized.bytes(),
            code_length,
            if config.refinement {
                Some(refinement_template)
            } else {
                None
            },
            limits,
        )?);

        let bitmap = decode_text_region_body(
            &mut codec,
            input.symbols,
            code_length,
            header.region.width,
            header.region.height,
            header.number_of_instances,
            &config,
            refinement_adaptive,
            limits,
            &mut resources,
        )?;

        let SymbolCodec::Arithmetic(arithmetic) = &codec else {
            return Err(SymbolTextError::InvalidState(
                "arithmetic text region has non-arithmetic state",
            ));
        };
        validate_bounded_arithmetic_marker(&arithmetic.decoder, normalized.len())?;
        (bitmap, body.len())
    };

    let consumed_bytes = header_len
        .checked_add(consumed_within_body)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(TextRegionOutput {
        region: header.region,
        bitmap,
        consumed_bytes,
        missing_final_huffman_oob: matches!(end, TextRegionEnd::RecoveredHuffman { .. }),
    })
}
