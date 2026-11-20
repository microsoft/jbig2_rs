//! Symbol dictionary and text region flags, headers and count validation.

#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::{
    enforce_limit, parse_region_info, widen_u8_i32, ByteReader, CombinationOperator, DecodeError,
    LimitKind, Limits, MalformedError, ReferenceCorner, StandardHuffmanTable,
    SymbolDictionaryConfig, SymbolDictionaryHeader, SymbolTextError, TableChoice, TemplateOffset,
    TextHuffmanChoices, TextRegionConfig, TextRegionHeader, UnsupportedFeature,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(config) => symbol_text_spec::dictionary_config_safe(flags, config),
        Err(_) => true,
    },
))]
pub(crate) fn parse_symbol_dictionary_flags(
    flags: u16,
) -> Result<SymbolDictionaryConfig, SymbolTextError> {
    if flags & 0xe000 != 0 {
        return Err(SymbolTextError::InvalidFlags(
            "reserved symbol-dictionary flags",
        ));
    }
    let huffman = flags & 1 != 0;
    let refinement_aggregation = flags & 2 != 0;
    let height_selector = (flags / 4) % 4;
    let width_selector = (flags / 16) % 4;
    let dh = u8::try_from(height_selector).map_err(|_error| MalformedError::IntegerOverflow)?;
    let dw = u8::try_from(width_selector).map_err(|_error| MalformedError::IntegerOverflow)?;
    let bitmap_size_custom = flags & 0x0040 != 0;
    let aggregation_custom = flags & 0x0080 != 0;
    let bitmap_context_used = flags & 0x0100 != 0;
    let bitmap_context_retained = flags & 0x0200 != 0;
    let template_raw = (flags / 1_024) % 4;
    let refinement_template_raw = (flags / 4_096) % 2;
    let template = u8::try_from(template_raw).map_err(|_error| MalformedError::IntegerOverflow)?;
    let refinement_template =
        u8::try_from(refinement_template_raw).map_err(|_error| MalformedError::IntegerOverflow)?;

    if !huffman && (dh != 0 || dw != 0 || bitmap_size_custom || aggregation_custom) {
        return Err(SymbolTextError::InvalidFlags(
            "Huffman selections in arithmetic dictionary",
        ));
    }
    if huffman && template != 0 {
        return Err(SymbolTextError::InvalidFlags(
            "direct bitmap template in Huffman dictionary",
        ));
    }
    if !refinement_aggregation && (aggregation_custom || refinement_template != 0) {
        return Err(SymbolTextError::InvalidFlags(
            "aggregation fields without refinement aggregation",
        ));
    }
    if huffman && !refinement_aggregation && (bitmap_context_used || bitmap_context_retained) {
        return Err(SymbolTextError::InvalidFlags(
            "bitmap contexts in direct Huffman dictionary",
        ));
    }

    let config = SymbolDictionaryConfig {
        huffman,
        refinement_aggregation,
        height_delta: two_bit_choice(
            dh,
            StandardHuffmanTable::D,
            StandardHuffmanTable::E,
            "invalid height-delta table selection",
        )?,
        width_delta: two_bit_choice(
            dw,
            StandardHuffmanTable::B,
            StandardHuffmanTable::C,
            "invalid width-delta table selection",
        )?,
        bitmap_size: if bitmap_size_custom {
            TableChoice::Custom
        } else {
            TableChoice::Standard(StandardHuffmanTable::A)
        },
        aggregation_instances: if aggregation_custom {
            TableChoice::Custom
        } else {
            TableChoice::Standard(StandardHuffmanTable::A)
        },
        bitmap_context_used,
        bitmap_context_retained,
        template,
        refinement_template,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::dictionary_config_safe);
        assert(config.template == template);
        assert(config.refinement_template == refinement_template);
        assert(symbol_text_spec::dictionary_config_safe(flags, config));
    }
    Ok(config)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(choice) => symbol_text_spec::two_bit_choice_result(
            encoded, first, second, choice
        ),
        Err(_) => encoded == 2 || encoded > 3,
    },
))]
pub(super) fn two_bit_choice(
    encoded: u8,
    first: StandardHuffmanTable,
    second: StandardHuffmanTable,
    error: &'static str,
) -> Result<TableChoice, SymbolTextError> {
    match encoded {
        0 => Ok(TableChoice::Standard(first)),
        1 => Ok(TableChoice::Standard(second)),
        3 => Ok(TableChoice::Custom),
        _ => Err(SymbolTextError::InvalidFlags(error)),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(config) => symbol_text_spec::text_config_safe(
            flags, huffman_flags, colour_extension, config
        ),
        Err(_) => true,
    },
))]
pub(crate) fn parse_text_region_flags(
    flags: u16,
    huffman_flags: Option<u16>,
    colour_extension: bool,
) -> Result<TextRegionConfig, SymbolTextError> {
    if colour_extension {
        return Err(SymbolTextError::Decode(DecodeError::Unsupported(
            UnsupportedFeature::Colour,
        )));
    }
    let huffman = flags & 1 != 0;
    let refinement = flags & 2 != 0;
    let log_strips_raw = (flags / 4) % 4;
    let log_strips =
        u8::try_from(log_strips_raw).map_err(|_error| MalformedError::IntegerOverflow)?;
    let strip_size = match log_strips {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let corner = match (flags >> 4) & 3 {
        0 => ReferenceCorner::BottomLeft,
        1 => ReferenceCorner::TopLeft,
        2 => ReferenceCorner::BottomRight,
        3 => ReferenceCorner::TopRight,
        _ => return Err(SymbolTextError::InvalidFlags("invalid reference corner")),
    };
    let operator = match (flags >> 7) & 3 {
        0 => CombinationOperator::Or,
        1 => CombinationOperator::And,
        2 => CombinationOperator::Xor,
        3 => CombinationOperator::Xnor,
        _ => {
            return Err(SymbolTextError::InvalidFlags(
                "invalid text combination operator",
            ))
        }
    };
    let refinement_template_raw = (flags / 32_768) % 2;
    let refinement_template =
        u8::try_from(refinement_template_raw).map_err(|_error| MalformedError::IntegerOverflow)?;
    if !refinement && refinement_template != 0 {
        return Err(SymbolTextError::InvalidFlags(
            "refinement template without refinement",
        ));
    }
    let raw_offset_bits = (flags / 1_024) % 32;
    let raw_offset =
        i8::try_from(raw_offset_bits).map_err(|_error| MalformedError::IntegerOverflow)?;
    let s_offset = if raw_offset & 0x10 != 0 {
        raw_offset - 0x20
    } else {
        raw_offset
    };
    let choices = match (huffman, huffman_flags) {
        (true, Some(value)) => Some(parse_text_huffman_flags(value, refinement)?),
        (true, None) => return Err(SymbolTextError::InvalidFlags("missing text Huffman flags")),
        (false, Some(_)) => {
            return Err(SymbolTextError::InvalidFlags(
                "text Huffman flags in arithmetic region",
            ))
        }
        (false, None) => None,
    };

    let config = TextRegionConfig {
        huffman,
        refinement,
        strip_size,
        reference_corner: corner,
        transposed: flags & 0x0040 != 0,
        operator,
        default_pixel: flags & 0x0200 != 0,
        s_offset,
        refinement_template,
        huffman_choices: choices,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::text_config_safe);
        assert(symbol_text_spec::text_config_safe(
            flags, huffman_flags, colour_extension, config
        ));
    }
    Ok(config)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(choices) => symbol_text_spec::text_huffman_choices_safe(
            flags, refinement, choices
        ),
        Err(_) => true,
    },
))]
pub(super) fn parse_text_huffman_flags(
    flags: u16,
    refinement: bool,
) -> Result<TextHuffmanChoices, SymbolTextError> {
    if flags & 0x8000 != 0 {
        return Err(SymbolTextError::InvalidFlags("reserved text Huffman flag"));
    }
    let first_s = two_bit_choice(
        text_huffman_selector(flags, 0),
        StandardHuffmanTable::F,
        StandardHuffmanTable::G,
        "invalid first-S table selection",
    )?;
    let subsequent_s = four_way_choice(
        text_huffman_selector(flags, 2),
        StandardHuffmanTable::H,
        StandardHuffmanTable::I,
        StandardHuffmanTable::J,
    );
    let strip_delta = four_way_choice(
        text_huffman_selector(flags, 4),
        StandardHuffmanTable::K,
        StandardHuffmanTable::L,
        StandardHuffmanTable::M,
    );
    let refinement_width = refinement_choice(text_huffman_selector(flags, 6))?;
    let refinement_height = refinement_choice(text_huffman_selector(flags, 8))?;
    let refinement_x = refinement_choice(text_huffman_selector(flags, 10))?;
    let refinement_y = refinement_choice(text_huffman_selector(flags, 12))?;
    let refinement_size = if flags & 0x4000 != 0 {
        TableChoice::Custom
    } else {
        TableChoice::Standard(StandardHuffmanTable::A)
    };
    if !refinement && flags & 0x7fc0 != 0 {
        return Err(SymbolTextError::InvalidFlags(
            "refinement Huffman fields without refinement",
        ));
    }
    Ok(TextHuffmanChoices {
        first_s,
        subsequent_s,
        strip_delta,
        refinement_width,
        refinement_height,
        refinement_x,
        refinement_y,
        refinement_size,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires shift <= 12,
    ensures out <= u8::MAX,
))]
pub(super) fn text_huffman_selector(flags: u16, shift: u32) -> u8 {
    let selected = (flags >> shift) & 3;
    selected as u8
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures symbol_text_spec::four_way_choice_result(
        encoded, first, second, third, out
    ),
))]
pub(super) fn four_way_choice(
    encoded: u8,
    first: StandardHuffmanTable,
    second: StandardHuffmanTable,
    third: StandardHuffmanTable,
) -> TableChoice {
    match encoded {
        0 => TableChoice::Standard(first),
        1 => TableChoice::Standard(second),
        2 => TableChoice::Standard(third),
        _ => TableChoice::Custom,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(choice) => symbol_text_spec::refinement_choice_result(encoded, choice),
        Err(_) => encoded == 2 || encoded > 3,
    },
))]
pub(super) fn refinement_choice(encoded: u8) -> Result<TableChoice, SymbolTextError> {
    two_bit_choice(
        encoded,
        StandardHuffmanTable::N,
        StandardHuffmanTable::O,
        "invalid refinement table selection",
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(total) =>
            total as nat == imported as nat + new_symbols as nat
            && total <= limits.max_symbols
            && exported <= total,
        Err(_) => true,
    },
    out == symbol_text_spec::symbol_count_result(imported, new_symbols, exported, limits),
))]
pub(crate) fn check_symbol_counts(
    imported: u32,
    new_symbols: u32,
    exported: u32,
    limits: &Limits,
) -> Result<u32, SymbolTextError> {
    let total = imported
        .checked_add(new_symbols)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Symbols,
        u64::from(limits.max_symbols),
        u64::from(total),
    )?;
    if exported > total {
        return Err(SymbolTextError::InvalidCount(
            "exported symbols exceed candidates",
        ));
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Wire header parsing (T.88 7.4.2 symbol dictionary, 7.4.3 text region).
// ---------------------------------------------------------------------------

/// Reads one signed adaptive-pixel pair, as used by generic and refinement
/// AT flags.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        -128 <= out,
        out <= 127,
        out == symbol_text_spec::signed_byte(value),
))]
pub(super) fn signed_byte(value: u8) -> i32 {
    if value <= 127 {
        widen_u8_i32(value)
    } else {
        widen_u8_i32(value) - 256
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).position <= old(reader).bytes@.len(),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        final(reader).position <= final(reader).bytes@.len(),
        match out {
            Ok(offset) =>
                final(reader).position == old(reader).position + 2
                && -128 <= offset.x <= 127
                && -128 <= offset.y <= 127,
            Err(_) => final(reader).position <= old(reader).position + 1,
        },
))]
pub(super) fn read_at_pixel(
    reader: &mut ByteReader<'_>,
) -> Result<TemplateOffset, SymbolTextError> {
    let x = signed_byte(reader.read_u8()?);
    let y = signed_byte(reader.read_u8()?);
    Ok(TemplateOffset::new(x, y))
}

/// Parses a symbol-dictionary segment data header and returns its byte length.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((header, consumed)) =>
            consumed <= payload@.len()
            && header.adaptive_count <= 4
            && header.new_symbols <= limits.max_symbols
            && consumed as nat
                == 10 + 2 * header.adaptive_count as nat
                    + if header.config.refinement_aggregation
                        && header.config.refinement_template == 0
                    { 4nat } else { 0nat },
        Err(_) => true,
    },
))]
pub(crate) fn parse_symbol_dictionary_header(
    payload: &[u8],
    limits: &Limits,
) -> Result<(SymbolDictionaryHeader, usize), SymbolTextError> {
    let mut reader = ByteReader::new(payload);
    let flags = reader.read_u16()?;
    let config = parse_symbol_dictionary_flags(flags)?;

    let mut adaptive = [TemplateOffset::new(0, 0); 4];
    let adaptive_count: u8 = if config.huffman {
        0
    } else if config.template == 0 {
        4
    } else {
        1
    };
    let mut adaptive_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            adaptive_count <= 4,
            adaptive_index <= adaptive_count as usize,
            reader.position <= payload@.len(),
            reader.bytes@ == payload@,
            reader.position == 2 + 2 * adaptive_index,
        decreases adaptive_count as usize - adaptive_index,
    ))]
    while adaptive_index < adaptive_count as usize {
        adaptive[adaptive_index] = read_at_pixel(&mut reader)?;
        adaptive_index += 1;
    }

    let mut refinement_adaptive = [TemplateOffset::new(0, 0); 2];
    if config.refinement_aggregation && config.refinement_template == 0 {
        refinement_adaptive[0] = read_at_pixel(&mut reader)?;
        refinement_adaptive[1] = read_at_pixel(&mut reader)?;
    }

    let exported_symbols = reader.read_u32()?;
    let new_symbols = reader.read_u32()?;
    enforce_limit(
        LimitKind::Symbols,
        u64::from(limits.max_symbols),
        u64::from(new_symbols),
    )?;

    Ok((
        SymbolDictionaryHeader {
            config,
            adaptive,
            adaptive_count,
            refinement_adaptive,
            exported_symbols,
            new_symbols,
        },
        reader.position(),
    ))
}

/// Parses a text-region segment data header and returns its byte length.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((header, _)) =>
            symbol_text_spec::valid_strip_size(header.config.strip_size),
        Err(_) => true,
    },
))]
pub(crate) fn parse_text_region_header(
    payload: &[u8],
    limits: &Limits,
) -> Result<(TextRegionHeader, usize), SymbolTextError> {
    let mut reader = ByteReader::new(payload);
    let region = parse_region_info(&mut reader, limits)?;
    let flags = reader.read_u16()?;
    let huffman = flags & 1 != 0;
    let huffman_flags = if huffman {
        Some(reader.read_u16()?)
    } else {
        None
    };
    let config = parse_text_region_flags(flags, huffman_flags, false)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::text_config_safe);
    }

    let mut refinement_adaptive = [TemplateOffset::new(0, 0); 2];
    if config.refinement && config.refinement_template == 0 {
        refinement_adaptive[0] = read_at_pixel(&mut reader)?;
        refinement_adaptive[1] = read_at_pixel(&mut reader)?;
    }

    let number_of_instances = reader.read_u32()?;
    enforce_limit(
        LimitKind::Work,
        limits.max_work_units,
        u64::from(number_of_instances),
    )?;

    Ok((
        TextRegionHeader {
            region,
            config,
            refinement_adaptive,
            number_of_instances,
        },
        reader.position(),
    ))
}
