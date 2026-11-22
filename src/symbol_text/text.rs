//! Text strip state, instance placement and region payload decoding.

use super::bitmap::refinement_template_from_u8;
use super::codec::{
    byte_align, decode_checked_symbol_id, decode_refined_bitmap, decode_symbol_id_code_lengths,
    new_arithmetic_codec, new_huffman_codec, normalized_bounded_arithmetic,
    read_symbol_id_runcode_lines, validate_bounded_arithmetic_marker,
};
use super::placement::{flush_placement_batch, prepare_placement};
#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::tables::{
    associate_text_tables, materialize_huffman_table, select_text_tables, wrap_custom_tables,
};
use super::{
    compose_placement, enforce_limit, parse_text_region_header, required_integer,
    symbol_id_code_length, widen_u32_i64, widen_u8_i64, Bitmap, EntropyInteger, HuffmanChoiceSet,
    HuffmanFieldTables, HuffmanTable, HuffmanValue, IntegerField, LimitKind, Limits,
    MalformedError, Placement, PlacementBatch, ReferenceCorner, ResourceTracker,
    StandardHuffmanTables, StripState, SymbolCodebook, SymbolCodec, SymbolIdRequest,
    SymbolTextError, TextRegionConfig, TextRegionEnd, TextRegionInput, TextRegionOutput,
    TextRegionState,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl TextRegionState {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.is_ok() == symbol_text_spec::text_initialization_allowed(
            config, declared_instances, initial_strip_value, limits,
        ),
        match out {
            Ok(state) =>
                symbol_text_spec::text_state_valid(&state)
                && state.config == config
                && state.declared_instances == declared_instances
                && state.decoded_instances == 0
                && state.strip_t as int
                    == -(initial_strip_value as int * config.strip_size as int)
                && state.first_s == 0
                && state.strip.is_none(),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(
        config: TextRegionConfig,
        declared_instances: u32,
        initial_strip_value: i64,
        limits: &Limits,
    ) -> Result<Self, SymbolTextError> {
        enforce_limit(
            LimitKind::Work,
            limits.max_work_units,
            u64::from(declared_instances),
        )?;
        let scaled = initial_strip_value
            .checked_mul(widen_u8_i64(config.strip_size))
            .ok_or(MalformedError::IntegerOverflow)?;
        let strip_t = 0_i64
            .checked_sub(scaled)
            .ok_or(MalformedError::IntegerOverflow)?;
        Ok(Self {
            config,
            declared_instances,
            decoded_instances: 0,
            strip_t,
            first_s: 0,
            strip: None,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(old(self)),
        ensures match out {
            Ok(()) =>
                symbol_text_spec::text_state_valid(final(self))
                && final(self).strip.is_some()
                && final(self).decoded_instances == old(self).decoded_instances,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn begin_strip(
        &mut self,
        strip_delta: i64,
        first_s_delta: i64,
    ) -> Result<(), SymbolTextError> {
        if self.strip.is_some() {
            return Err(SymbolTextError::InvalidState("strip already open"));
        }
        if self.decoded_instances >= self.declared_instances {
            return Err(SymbolTextError::InvalidState("strip after final instance"));
        }
        let scaled = strip_delta
            .checked_mul(widen_u8_i64(self.config.strip_size))
            .ok_or(MalformedError::IntegerOverflow)?;
        self.strip_t = self
            .strip_t
            .checked_add(scaled)
            .ok_or(MalformedError::IntegerOverflow)?;
        self.first_s = self
            .first_s
            .checked_add(first_s_delta)
            .ok_or(MalformedError::IntegerOverflow)?;
        self.strip = Some(StripState {
            cursor_s: self.first_s,
            has_instance: false,
        });
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(old(self)),
        ensures match out {
            Ok(placement) =>
                symbol_text_spec::text_state_valid(final(self))
                && final(self).decoded_instances == old(self).decoded_instances + 1
                && final(self).strip.is_some()
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::placement_within_extents(placement, width, height),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn place_first(
        &mut self,
        within_strip_t: u8,
        width: u32,
        height: u32,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<Placement, SymbolTextError> {
        let strip = self
            .strip
            .as_ref()
            .ok_or(SymbolTextError::InvalidState("no open strip"))?;
        if strip.has_instance {
            return Err(SymbolTextError::InvalidState(
                "first instance already placed",
            ));
        }
        self.place_current(within_strip_t, width, height, limits, resources)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(old(self)),
        ensures match out {
            Ok(placement) =>
                symbol_text_spec::text_state_valid(final(self))
                && final(self).decoded_instances == old(self).decoded_instances + 1
                && final(self).strip.is_some()
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::placement_within_extents(placement, width, height),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn place_subsequent(
        &mut self,
        delta_s: i64,
        within_strip_t: u8,
        width: u32,
        height: u32,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<Placement, SymbolTextError> {
        let strip = self
            .strip
            .as_mut()
            .ok_or(SymbolTextError::InvalidState("no open strip"))?;
        if !strip.has_instance {
            return Err(SymbolTextError::InvalidState(
                "subsequent instance before first",
            ));
        }
        strip.cursor_s = strip
            .cursor_s
            .checked_add(delta_s)
            .and_then(|value| value.checked_add(i64::from(self.config.s_offset)))
            .ok_or(MalformedError::IntegerOverflow)?;
        self.place_current(within_strip_t, width, height, limits, resources)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(old(self)),
        ensures match out {
            Ok(placement) =>
                symbol_text_spec::text_state_valid(final(self))
                && final(self).decoded_instances == old(self).decoded_instances + 1
                && final(self).strip.is_some()
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::placement_within_extents(placement, width, height)
                && symbol_text_spec::placement_for_current(
                    old(self), within_strip_t, width, height, placement
                ),
            Err(_) => true,
        },
        out.is_ok() == symbol_text_spec::text_placement_allowed(
            old(self), within_strip_t, width, height, old(resources), limits,
        ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn place_current(
        &mut self,
        within_strip_t: u8,
        width: u32,
        height: u32,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<Placement, SymbolTextError> {
        if width == 0 || height == 0 {
            return Err(SymbolTextError::InvalidDimensions);
        }
        if within_strip_t >= self.config.strip_size {
            return Err(SymbolTextError::InvalidEntropyValue(
                "within-strip T coordinate",
            ));
        }
        if self.decoded_instances >= self.declared_instances {
            return Err(SymbolTextError::InvalidCount("too many text instances"));
        }
        resources.charge_work(1, limits)?;
        let strip = self
            .strip
            .as_mut()
            .ok_or(SymbolTextError::InvalidState("no open strip"))?;
        let width_extent = widen_u32_i64(width) - 1;
        let height_extent = widen_u32_i64(height) - 1;
        if (!self.config.transposed
            && matches!(
                self.config.reference_corner,
                ReferenceCorner::TopRight | ReferenceCorner::BottomRight
            ))
            || (self.config.transposed
                && matches!(
                    self.config.reference_corner,
                    ReferenceCorner::BottomLeft | ReferenceCorner::BottomRight
                ))
        {
            let extent = if self.config.transposed {
                height_extent
            } else {
                width_extent
            };
            strip.cursor_s = strip
                .cursor_s
                .checked_add(extent)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        let symbol_s = strip.cursor_s;
        let symbol_t = self
            .strip_t
            .checked_add(widen_u8_i64(within_strip_t))
            .ok_or(MalformedError::IntegerOverflow)?;
        let (reference_x, reference_y) = if self.config.transposed {
            (symbol_t, symbol_s)
        } else {
            (symbol_s, symbol_t)
        };
        let x = match self.config.reference_corner {
            ReferenceCorner::TopLeft | ReferenceCorner::BottomLeft => reference_x,
            ReferenceCorner::TopRight | ReferenceCorner::BottomRight => reference_x
                .checked_sub(width_extent)
                .ok_or(MalformedError::IntegerOverflow)?,
        };
        let y = match self.config.reference_corner {
            ReferenceCorner::TopLeft | ReferenceCorner::TopRight => reference_y,
            ReferenceCorner::BottomLeft | ReferenceCorner::BottomRight => reference_y
                .checked_sub(height_extent)
                .ok_or(MalformedError::IntegerOverflow)?,
        };
        if (!self.config.transposed
            && matches!(
                self.config.reference_corner,
                ReferenceCorner::TopLeft | ReferenceCorner::BottomLeft
            ))
            || (self.config.transposed
                && matches!(
                    self.config.reference_corner,
                    ReferenceCorner::TopLeft | ReferenceCorner::TopRight
                ))
        {
            let extent = if self.config.transposed {
                height_extent
            } else {
                width_extent
            };
            strip.cursor_s = strip
                .cursor_s
                .checked_add(extent)
                .ok_or(MalformedError::IntegerOverflow)?;
        }
        strip.has_instance = true;
        self.decoded_instances = self
            .decoded_instances
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        #[cfg(verus_keep_ghost)]
        production_symbol_current_placement_witness!(self, within_strip_t, width, height, x, y);
        Ok(Placement { x, y })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(old(self)),
        ensures match out {
            Ok(()) =>
                symbol_text_spec::text_state_valid(final(self))
                && final(self).strip.is_none()
                && final(self).decoded_instances == old(self).decoded_instances,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn end_strip(&mut self) -> Result<(), SymbolTextError> {
        let strip = self
            .strip
            .take()
            .ok_or(SymbolTextError::InvalidState("no open strip"))?;
        if !strip.has_instance {
            return Err(SymbolTextError::InvalidState("empty text strip"));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::text_state_valid(&self),
        ensures match out {
            Ok(()) =>
                self.strip.is_none()
                && self.decoded_instances == self.declared_instances,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn finish(self) -> Result<(), SymbolTextError> {
        if self.strip.is_some() {
            return Err(SymbolTextError::InvalidState("text strip still open"));
        }
        if self.decoded_instances != self.declared_instances {
            return Err(SymbolTextError::InvalidCount(
                "text instance count mismatch",
            ));
        }
        Ok(())
    }
}

/// The base-two logarithm of a strip size that is always a power of two
/// (T.88 7.4.3.1.1's two-bit `LOGSBSTRIPS` field).
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::valid_strip_size(strip_size),
    ensures symbol_text_spec::strip_log_valid(strip_size, out),
))]
pub(super) fn log2_strip_size(strip_size: u8) -> u8 {
    if strip_size <= 1 {
        0
    } else if strip_size <= 3 {
        1
    } else if strip_size <= 7 {
        2
    } else if strip_size <= 15 {
        3
    } else if strip_size <= 31 {
        4
    } else if strip_size <= 63 {
        5
    } else if strip_size <= 127 {
        6
    } else {
        7
    }
}

/// Decodes one complete text-region bitmap (T.88 6.4), shared by top-level
/// text-region segments and the nested multiple-instance aggregate
/// procedure invoked from a symbol dictionary (T.88 6.5.8.2.1).
///
/// `code_length` is the fixed IAID bit width for the whole enclosing segment;
/// `available_symbols` is the (possibly
/// smaller, for a nested aggregate call still in progress) slice used both
/// to validate decoded IDs and to look up referenced bitmaps.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codec_valid(old(codec)),
        symbol_text_spec::codebook_valid(available_symbols),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
    ensures match out {
        Ok(bitmap) =>
            symbol_text_spec::codec_valid(final(codec))
                && crate::spec::bitmap::storage_valid(&bitmap)
                && crate::views::bitmap_width(&bitmap) == width
                && crate::views::bitmap_height(&bitmap) == height
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units,
        Err(_) => true,
    },
))]
pub(super) fn decode_text_region_body(
    codec: &mut SymbolCodec<'_>,
    available_symbols: SymbolCodebook<'_>,
    code_length: u8,
    width: u32,
    height: u32,
    number_of_instances: u32,
    config: &TextRegionConfig,
    refinement_adaptive: [(i8, i8); 2],
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<Bitmap, SymbolTextError> {
    decode_text_region_body_with_end(
        codec,
        available_symbols,
        code_length,
        width,
        height,
        number_of_instances,
        config,
        refinement_adaptive,
        limits,
        resources,
        &mut TextRegionEnd::Strict,
    )
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codec_valid(old(codec)),
        symbol_text_spec::codebook_valid(available_symbols),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
        *old(end) == TextRegionEnd::Strict || *old(end) == TextRegionEnd::BoundedHuffman,
    ensures match out {
        Ok(bitmap) =>
            symbol_text_spec::codec_valid(final(codec))
                && crate::spec::bitmap::storage_valid(&bitmap)
                && crate::views::bitmap_width(&bitmap) == width
                && crate::views::bitmap_height(&bitmap) == height
                && symbol_text_spec::work_units(final(resources)) <= limits.max_work_units
                && match *final(end) {
                    TextRegionEnd::RecoveredHuffman { instances, padding_start } =>
                        *old(end) == TextRegionEnd::BoundedHuffman
                        && instances == number_of_instances && instances > 0
                        && !config.refinement
                        && match *final(codec) {
                            SymbolCodec::Huffman(h) =>
                                padding_start <= h.reader.bit_position
                                && h.reader.bit_position - padding_start < 8
                                && h.reader.bit_position == h.reader.bytes@.len() * 8,
                            _ => false,
                        },
                    _ => *final(end) == *old(end),
                },
        Err(_) => true,
    },
))]
pub(super) fn decode_text_region_body_with_end(
    codec: &mut SymbolCodec<'_>,
    available_symbols: SymbolCodebook<'_>,
    code_length: u8,
    width: u32,
    height: u32,
    number_of_instances: u32,
    config: &TextRegionConfig,
    refinement_adaptive: [(i8, i8); 2],
    limits: &Limits,
    resources: &mut ResourceTracker,
    end: &mut TextRegionEnd,
) -> Result<Bitmap, SymbolTextError> {
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost initial_end = *end; }
    resources.charge_bitmap(width, height, limits)?;
    let mut bitmap = Bitmap::filled(width, height, config.default_pixel, limits)?;
    let mut placement_batch = PlacementBatch::new();
    if !config.refinement {
        placement_batch.reserve(
            usize::try_from(number_of_instances).map_err(|_| MalformedError::IntegerOverflow)?,
            limits,
        )?;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::placement_batch_valid);
    }

    let initial_strip = required_integer(codec, IntegerField::StripDelta)?;
    let mut state = TextRegionState::new(*config, number_of_instances, initial_strip, limits)?;

    let available_count =
        u32::try_from(available_symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?;

    let mut decoded = 0_u32;
    let mut recovered_padding = None;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            decoded <= number_of_instances,
            state.decoded_instances == decoded,
            state.strip.is_none(),
            symbol_text_spec::text_state_valid(&state),
            symbol_text_spec::codec_valid(codec),
            symbol_text_spec::codebook_valid(available_symbols),
            available_count as nat == symbol_text_spec::codebook_len(available_symbols),
            crate::spec::bitmap::storage_valid(&bitmap),
            crate::views::bitmap_width(&bitmap) == width,
            crate::views::bitmap_height(&bitmap) == height,
            symbol_text_spec::placement_batch_valid(&placement_batch, &bitmap),
            symbol_text_spec::work_units(resources) <= limits.max_work_units,
            initial_end == *old(end),
            *end == initial_end,
            recovered_padding.is_some() ==> (
                initial_end == TextRegionEnd::BoundedHuffman
                && decoded == number_of_instances
                && symbol_text_spec::text_end_matches_count(
                    TextRegionEnd::RecoveredHuffman { instances: decoded, padding_start: recovered_padding.unwrap() },
                    codec, decoded, config.refinement,
                )
            ),
        decreases number_of_instances - decoded,
    ))]
    while decoded < number_of_instances {
        let strip_start = decoded;
        let strip_delta = required_integer(codec, IntegerField::StripDelta)?;
        let first_s_delta = required_integer(codec, IntegerField::FirstS)?;
        state.begin_strip(strip_delta, first_s_delta)?;
        let mut first = true;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                decoded <= number_of_instances,
                strip_start <= decoded,
                strip_start < number_of_instances,
                first ==> decoded == strip_start,
                !first ==> decoded > strip_start,
                state.decoded_instances == decoded,
                state.strip.is_some(),
                symbol_text_spec::text_state_valid(&state),
                symbol_text_spec::codec_valid(codec),
                symbol_text_spec::codebook_valid(available_symbols),
                available_count as nat == symbol_text_spec::codebook_len(available_symbols),
                crate::spec::bitmap::storage_valid(&bitmap),
                crate::views::bitmap_width(&bitmap) == width,
                crate::views::bitmap_height(&bitmap) == height,
                symbol_text_spec::placement_batch_valid(&placement_batch, &bitmap),
                symbol_text_spec::work_units(resources) <= limits.max_work_units,
                *end == initial_end,
            invariant_except_break recovered_padding.is_none(),
            ensures recovered_padding.is_none() || (
                    initial_end == TextRegionEnd::BoundedHuffman
                    && decoded == number_of_instances
                    && symbol_text_spec::text_end_matches_count(
                        TextRegionEnd::RecoveredHuffman { instances: decoded, padding_start: recovered_padding.unwrap() },
                        codec, decoded, config.refinement,
                    )
                ),
            decreases number_of_instances - decoded,
        ))]
        loop {
            if decoded == number_of_instances
                && matches!(end, TextRegionEnd::BoundedHuffman)
                && !config.refinement
            {
                if let SymbolCodec::Huffman(huffman) = codec {
                    let table = huffman.tables.table_for(IntegerField::SubsequentS).ok_or(
                        SymbolTextError::InvalidTableAssociation(
                            "no Huffman table bound for field",
                        ),
                    )?;
                    let padding_start = huffman.reader.bit_position;
                    match table.decode_final_strip_value(&mut huffman.reader)? {
                        Some(HuffmanValue::OutOfBand) => break,
                        Some(HuffmanValue::Integer(_)) => {
                            return Err(SymbolTextError::InvalidCount("too many text instances"));
                        }
                        None => {
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                assert(decoded > 0);
                                assert(padding_start <= huffman.reader.bit_position);
                                assert(huffman.reader.bit_position - padding_start < 8);
                            }
                            recovered_padding = Some(padding_start);
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                assert(initial_end == TextRegionEnd::BoundedHuffman);
                                assert(decoded == number_of_instances);
                                assert(symbol_text_spec::text_end_matches_count(
                                    TextRegionEnd::RecoveredHuffman { instances: decoded, padding_start },
                                    codec, decoded, config.refinement,
                                ));
                            }
                            break;
                        }
                    }
                }
            }
            let delta_s = if first {
                None
            } else {
                match codec.decode_integer(IntegerField::SubsequentS)? {
                    EntropyInteger::Value(value) => Some(value),
                    EntropyInteger::OutOfBand => {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            assert(decoded > strip_start);
                        }
                        break;
                    }
                }
            };
            if decoded >= number_of_instances {
                return Err(SymbolTextError::InvalidCount("too many text instances"));
            }
            let within_strip_t = if config.strip_size > 1 {
                match codec.decode_integer(IntegerField::WithinStripT)? {
                    EntropyInteger::Value(value) => u8::try_from(value).map_err(|_| {
                        SymbolTextError::InvalidEntropyValue("within-strip T coordinate")
                    })?,
                    EntropyInteger::OutOfBand => {
                        return Err(SymbolTextError::UnexpectedOutOfBand(
                            "within-strip T coordinate",
                        ))
                    }
                }
            } else {
                0
            };
            let id_request = SymbolIdRequest {
                available_symbols: available_count,
                code_length,
                huffman: config.huffman,
            };
            #[cfg(verus_keep_ghost)]
            production_symbol_id_request!(
                id_request,
                symbol_text_spec::codebook_len(available_symbols),
                code_length,
                config.huffman
            );
            let symbol_id = decode_checked_symbol_id(codec, id_request)?;
            let base_symbol = available_symbols
                .get(usize::try_from(symbol_id).map_err(|_| MalformedError::IntegerOverflow)?)
                .ok_or(SymbolTextError::SymbolIdOutOfRange {
                    id: symbol_id,
                    available: available_count,
                })?;

            let refined_storage;
            let placed_bitmap: &Bitmap = if config.refinement {
                let indicator = match codec.decode_integer(IntegerField::RefinementIndicator)? {
                    EntropyInteger::Value(value) => value,
                    EntropyInteger::OutOfBand => {
                        return Err(SymbolTextError::UnexpectedOutOfBand("refinement indicator"))
                    }
                };
                if indicator != 0 {
                    refined_storage = Some(decode_refined_bitmap(
                        codec,
                        base_symbol,
                        config.refinement_template,
                        refinement_adaptive,
                        config.huffman,
                        limits,
                        resources,
                    )?);
                    refined_storage.as_ref().unwrap_or(base_symbol)
                } else {
                    base_symbol
                }
            } else {
                base_symbol
            };

            let symbol_width = placed_bitmap.width();
            let symbol_height = placed_bitmap.height();
            let placement = match delta_s {
                None => state.place_first(
                    within_strip_t,
                    symbol_width,
                    symbol_height,
                    limits,
                    resources,
                )?,
                Some(delta) => state.place_subsequent(
                    delta,
                    within_strip_t,
                    symbol_width,
                    symbol_height,
                    limits,
                    resources,
                )?,
            };
            if config.refinement {
                flush_placement_batch(&mut bitmap, &mut placement_batch, config.operator)?;
                compose_placement(
                    &mut bitmap,
                    placed_bitmap,
                    placement,
                    config.operator,
                    limits,
                    resources,
                )?;
            } else if let Some(prepared) =
                prepare_placement(&bitmap, base_symbol, placement, limits, resources)?
            {
                if !placement_batch.compatible(prepared) {
                    flush_placement_batch(&mut bitmap, &mut placement_batch, config.operator)?;
                }
                placement_batch.push(prepared, &bitmap)?;
            }

            decoded = decoded
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?;
            first = false;
        }
        if decoded == strip_start {
            return Err(SymbolTextError::InvalidState("empty text strip"));
        }
        flush_placement_batch(&mut bitmap, &mut placement_batch, config.operator)?;
        state.end_strip()?;
    }
    if let Some(padding_start) = recovered_padding {
        *end = TextRegionEnd::RecoveredHuffman {
            instances: decoded,
            padding_start,
        };
    }
    flush_placement_batch(&mut bitmap, &mut placement_batch, config.operator)?;
    state.finish()?;
    Ok(bitmap)
}

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
