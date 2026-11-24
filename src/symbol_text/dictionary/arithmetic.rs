//! Arithmetic implementation.

#[cfg(verus_keep_ghost)]
use super::super::symbol_text_spec::cycles as cycle_spec;
use super::cycles::{CycleMode, IntegerCycle, HEIGHT_CYCLE_MIN_WORK};
#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    assemble_owned_exported_symbols, check_dimensions, decode_checked_symbol_id,
    decode_export_runs, decode_text_region_body, generic_template_from_u8, new_arithmetic_codec,
    normalized_bounded_arithmetic, refinement_template_from_u8, required_integer, symbol_by_id,
    validate_bounded_arithmetic_marker, ArithmeticDictionaryResult, Bitmap, CombinationOperator,
    DecodeError, DictionaryClassState, DictionaryContexts, EntropyInteger, GenericArithmeticCall,
    GenericContexts, IntegerField, Limits, MalformedError, ReferenceCorner, RefinementGeometry,
    RefinementRequest, ResourceTracker, SymbolCodebook, SymbolCodec, SymbolDictionaryHeader,
    SymbolIdRequest, SymbolTextError, TemplateOffset, TextRegionConfig,
};

/// Decodes one T.88 6.5.8.2.2 single-instance refinement/aggregate symbol
/// bitmap, shared by the arithmetic and Huffman symbol-dictionary decoders.
///
/// Unlike the text-region per-instance refinement procedure (handled by the
/// existing [`decode_refined_bitmap`] helper), this procedure never decodes a
/// width or height delta: the symbol's dimensions were already fixed by the
/// enclosing height-class loop, so only the symbol ID and reference offset
/// are decoded before invoking the shared generic-refinement callback.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codec_valid(old(codec)),
        symbol_text_spec::codebook_valid(imported),
        symbol_text_spec::bitmaps_valid(new_symbols@),
    ensures match out {
        Ok(bitmap) =>
            symbol_text_spec::codec_valid(final(codec))
                && crate::spec::bitmap::storage_valid(&bitmap)
                && crate::views::bitmap_width(&bitmap) == width
                && crate::views::bitmap_height(&bitmap) == height
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn decode_dictionary_single_instance_refinement(
    codec: &mut SymbolCodec<'_>,
    imported: SymbolCodebook<'_>,
    new_symbols: &[Bitmap],
    code_length: u8,
    width: u32,
    height: u32,
    refinement_template: u8,
    refinement_adaptive: [(i8, i8); 2],
    huffman: bool,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<Bitmap, SymbolTextError> {
    let available = imported
        .len()
        .checked_add(new_symbols.len())
        .and_then(|count| {
            #[cfg(verus_keep_ghost)]
            ensures(|out: Option<u32>| match out {
                Some(value) => equal(value as u128, count as u128),
                None => true,
            });
            u32::try_from(count).ok()
        })
        .ok_or(MalformedError::IntegerOverflow)?;
    let id_request = SymbolIdRequest {
        available_symbols: available,
        code_length,
        huffman,
    };
    #[cfg(verus_keep_ghost)]
    production_symbol_id_request!(
        id_request,
        symbol_text_spec::codebook_len(imported) + new_symbols.len(),
        code_length,
        huffman
    );
    let id = decode_checked_symbol_id(codec, id_request)?;
    let reference_x = required_integer(codec, IntegerField::RefinementX)?;
    let reference_y = required_integer(codec, IntegerField::RefinementY)?;
    let declared_size = if huffman {
        let size = required_integer(codec, IntegerField::RefinementSize)?;
        Some(
            u32::try_from(size)
                .map_err(|_| SymbolTextError::InvalidEntropyValue("refinement size"))?,
        )
    } else {
        None
    };
    let reference = symbol_by_id(imported, new_symbols, id)?;
    check_dimensions(width, height, limits)?;
    resources.charge_bitmap(width, height, limits)?;
    let geometry = RefinementGeometry {
        width,
        height,
        reference_dx: i32::try_from(reference_x).map_err(|_| MalformedError::IntegerOverflow)?,
        reference_dy: i32::try_from(reference_y).map_err(|_| MalformedError::IntegerOverflow)?,
    };
    let request = RefinementRequest {
        reference,
        geometry,
        template: refinement_template,
        adaptive_pixels: refinement_adaptive,
        declared_data_size: declared_size,
    };
    let bitmap = codec.decode_refinement(request, limits)?;
    if bitmap.width() != width || bitmap.height() != height {
        return Err(SymbolTextError::InvalidDimensions);
    }
    Ok(bitmap)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codec_valid(old(codec)),
        symbol_text_spec::codebook_valid(imported),
        symbol_text_spec::bitmaps_valid(new_symbols@),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
        header.adaptive_count <= 4,
        match *old(generic_contexts) {
            Some(ref contexts) => crate::generic_region::generic_region_spec::generic_contexts_valid(contexts),
            None => true,
        },
    ensures match out {
        Ok(bitmap) =>
            symbol_text_spec::codec_valid(final(codec))
                && crate::spec::bitmap::storage_valid(&bitmap)
                && crate::views::bitmap_width(&bitmap) == width
                && crate::views::bitmap_height(&bitmap) == height
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && symbol_text_spec::work_units(final(resources))
                    >= symbol_text_spec::work_units(old(resources))
                && match *final(generic_contexts) {
                    Some(ref contexts) => crate::generic_region::generic_region_spec::generic_contexts_valid(contexts),
                    None => true,
                },
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn decode_one_new_symbol(
    header: &SymbolDictionaryHeader,
    codec: &mut SymbolCodec<'_>,
    imported: SymbolCodebook<'_>,
    new_symbols: &[Bitmap],
    generic_contexts: &mut Option<GenericContexts>,
    generic_work: &mut u64,
    aggregation_instances: Option<u32>,
    width: u32,
    height: u32,
    code_length: u8,
    refinement_adaptive: [(i8, i8); 2],
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<Bitmap, SymbolTextError> {
    let config = header.config;
    if config.refinement_aggregation {
        let instances = aggregation_instances.ok_or(SymbolTextError::InvalidState(
            "missing aggregation-instance count",
        ))?;
        if instances == 0 {
            return Err(SymbolTextError::InvalidCount(
                "zero-instance aggregate symbol",
            ));
        }
        if instances > 1 {
            let capacity = imported
                .len()
                .checked_add(new_symbols.len())
                .ok_or(MalformedError::IntegerOverflow)?;
            let requested_bytes = capacity
                .checked_mul(core::mem::size_of::<&Bitmap>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or(MalformedError::IntegerOverflow)?;
            let mut symbol_refs: Vec<&Bitmap> = Vec::new();
            symbol_refs
                .try_reserve(capacity)
                .map_err(|_| DecodeError::Allocation { requested_bytes })?;
            let mut imported_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    imported_index <= symbol_text_spec::codebook_len(imported),
                    symbol_refs@.len() == imported_index,
                    symbol_text_spec::codebook_valid(imported),
                    symbol_text_spec::bitmap_refs_valid(symbol_refs@),
                decreases symbol_text_spec::codebook_len(imported) - imported_index,
            ))]
            while imported_index < imported.len() {
                symbol_refs.push(imported.get(imported_index).ok_or(
                    SymbolTextError::InvalidState("missing imported aggregate symbol"),
                )?);
                imported_index += 1;
            }
            let mut new_index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    new_index <= new_symbols@.len(),
                    symbol_refs@.len()
                        == symbol_text_spec::codebook_len(imported) + new_index,
                    symbol_text_spec::bitmaps_valid(new_symbols@),
                    symbol_text_spec::bitmap_refs_valid(symbol_refs@),
                decreases new_symbols@.len() - new_index,
            ))]
            while new_index < new_symbols.len() {
                symbol_refs.push(&new_symbols[new_index]);
                new_index += 1;
            }
            let nested_config = TextRegionConfig {
                huffman: config.huffman,
                refinement: true,
                strip_size: 1,
                reference_corner: ReferenceCorner::TopLeft,
                transposed: false,
                operator: CombinationOperator::Or,
                default_pixel: false,
                s_offset: 0,
                refinement_template: config.refinement_template,
                huffman_choices: None,
            };
            let work_before = resources.work_units;
            let bitmap = decode_text_region_body(
                codec,
                SymbolCodebook::Borrowed(&symbol_refs),
                code_length,
                width,
                height,
                instances,
                &nested_config,
                refinement_adaptive,
                limits,
                resources,
            )?;
            if resources.work_units < work_before {
                return Err(SymbolTextError::InvalidState(
                    "resource work counter decreased",
                ));
            }
            Ok(bitmap)
        } else {
            decode_dictionary_single_instance_refinement(
                codec,
                imported,
                new_symbols,
                code_length,
                width,
                height,
                config.refinement_template,
                refinement_adaptive,
                config.huffman,
                limits,
                resources,
            )
        }
    } else {
        if aggregation_instances.is_some() {
            return Err(SymbolTextError::InvalidState(
                "aggregation count on direct-coded symbol",
            ));
        }
        resources.charge_bitmap(width, height, limits)?;
        let contexts = generic_contexts
            .as_mut()
            .ok_or(SymbolTextError::InvalidState(
                "missing generic contexts for a direct-coded dictionary",
            ))?;
        let mut full_adaptive = [TemplateOffset::new(0, 0); 12];
        full_adaptive[..usize::from(header.adaptive_count)]
            .copy_from_slice(&header.adaptive[..usize::from(header.adaptive_count)]);
        let call = GenericArithmeticCall {
            width,
            height,
            adaptive: full_adaptive,
            typical_prediction: false,
            skip: None,
        };
        let bitmap = match codec {
            SymbolCodec::Arithmetic(codec) => {
                codec.decode_generic(contexts, call, height, limits, generic_work)
            }
            SymbolCodec::Huffman(_) => Err(SymbolTextError::InvalidState(
                "direct dictionary bitmap requires arithmetic state",
            )),
            #[cfg(test)]
            SymbolCodec::Scripted(_) => Err(SymbolTextError::InvalidState(
                "direct dictionary bitmap requires arithmetic state",
            )),
        }?;
        if bitmap.width() != width || bitmap.height() != height {
            return Err(SymbolTextError::InvalidDimensions);
        }
        Ok(bitmap)
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codebook_valid(imported),
        header.adaptive_count <= 4,
        total_symbols as nat
            == symbol_text_spec::codebook_len(imported) + header.new_symbols as nat,
    ensures match out {
        Ok((exported, _, _, consumed)) =>
            symbol_text_spec::bitmaps_valid(exported@)
                && consumed == body@.len(),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn decode_symbol_dictionary_arithmetic(
    header: &SymbolDictionaryHeader,
    imported: SymbolCodebook<'_>,
    body: &[u8],
    code_length: u8,
    total_symbols: u32,
    refinement_adaptive: [(i8, i8); 2],
    used_contexts: Option<&DictionaryContexts>,
    limits: &Limits,
) -> ArithmeticDictionaryResult {
    let config = header.config;
    let normalized = normalized_bounded_arithmetic(body, limits)?;
    let refinement_template = config
        .refinement_aggregation
        .then(|| refinement_template_from_u8(config.refinement_template));
    let mut codec = SymbolCodec::Arithmetic(new_arithmetic_codec(
        normalized.bytes(),
        code_length,
        refinement_template,
        limits,
    )?);
    let mut resources = ResourceTracker {
        work_units: 0,
        decoded_bitmap_bytes: 0,
    };

    let mut generic_contexts = if config.refinement_aggregation {
        None
    } else {
        let mut contexts =
            GenericContexts::new(generic_template_from_u8(config.template), false, limits)?;
        if let Some(used) = used_contexts.and_then(|used| used.generic.as_ref()) {
            contexts.restore(used, limits)?;
        }
        Some(contexts)
    };
    let mut generic_work = 0_u64;

    if config.refinement_aggregation {
        if let Some(used) = used_contexts.and_then(|used| used.refinement.as_ref()) {
            let SymbolCodec::Arithmetic(arithmetic) = &mut codec else {
                return Err(SymbolTextError::InvalidState(
                    "arithmetic dictionary has non-arithmetic state",
                ));
            };
            let contexts = arithmetic
                .refinement
                .as_mut()
                .ok_or(SymbolTextError::InvalidState(
                    "missing arithmetic refinement contexts",
                ))?;
            contexts.restore(used, limits)?;
        }
    }

    let new_symbol_capacity =
        usize::try_from(header.new_symbols).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut new_symbols: Vec<Bitmap> = Vec::new();
    new_symbols
        .try_reserve(new_symbol_capacity)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: u64::from(header.new_symbols),
        })?;

    let mut class_state = DictionaryClassState::new(header.new_symbols, limits)?;
    let mut cycle = IntegerCycle::new(CycleMode::EmptyHeightClasses);
    let mut empty_class = false;
    let detect_cycles = limits.max_work_units > HEIGHT_CYCLE_MIN_WORK;
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
            match generic_contexts {
                Some(ref contexts) => crate::generic_region::generic_region_spec::generic_contexts_valid(contexts),
                None => true,
            },
            symbol_text_spec::codec_valid(&codec),
            cycle_spec::detector_valid(&cycle),
            cycle.mode == CycleMode::EmptyHeightClasses,
            cycle.checkpoint.is_some() ==> (
                detect_cycles && empty_class
                    && cycle_spec::source_state(&codec, CycleMode::EmptyHeightClasses,
                        (class_state.decoded_symbols, class_state.height, false))
                        == Some(cycle.history@.last())
            ),
            symbol_text_spec::work_units(&resources) <= limits.max_work_units,
        decreases limits.max_work_units - symbol_text_spec::work_units(&resources),
    ))]
    while u32::try_from(new_symbols.len()).map_err(|_| MalformedError::IntegerOverflow)?
        < header.new_symbols
    {
        let class_start_symbols = new_symbols.len();
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost cycle_before = codec;
            let ghost cycle_cursor = (class_state.decoded_symbols, class_state.height, false);
        }
        #[cfg_attr(not(verus_keep_ghost), allow(unused_variables))]
        let class_start_work = resources.work_units;
        let height_delta = required_integer(&mut codec, IntegerField::HeightDelta)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost cycle_after_height = codec;
        }
        class_state.begin_height_class(height_delta, limits, &mut resources)?;
        let mut width_integer = codec.decode_integer(IntegerField::WidthDelta)?;
        let empty_width = matches!(width_integer, EntropyInteger::OutOfBand);
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost cycle_after_first_width = codec;
            let ghost first_width_integer = width_integer;
            let ghost class_at_open = class_state;
        }
        if !empty_width {
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
                class_start_symbols <= new_symbols@.len(),
                new_symbols@.len() == class_start_symbols ==> (
                    codec == cycle_after_first_width
                        && width_integer == first_width_integer
                        && class_state == class_at_open
                ),
                match generic_contexts {
                    Some(ref contexts) => crate::generic_region::generic_region_spec::generic_contexts_valid(contexts),
                    None => true,
                },
                symbol_text_spec::codec_valid(&codec),
                symbol_text_spec::work_units(&resources)
                    <= limits.max_work_units,
                resources.work_units > class_start_work,
            decreases limits.max_work_units
                - symbol_text_spec::work_units(&resources),
        ))]
            while let EntropyInteger::Value(width_delta) = width_integer {
                let (width, height) =
                    class_state.add_individual_symbol(width_delta, limits, &mut resources)?;
                let aggregation_instances = if config.refinement_aggregation {
                    let instances =
                        required_integer(&mut codec, IntegerField::AggregationInstances)?;
                    Some(u32::try_from(instances).map_err(|_| {
                        SymbolTextError::InvalidEntropyValue("aggregation instances")
                    })?)
                } else {
                    None
                };
                let bitmap = decode_one_new_symbol(
                    header,
                    &mut codec,
                    imported,
                    &new_symbols,
                    &mut generic_contexts,
                    &mut generic_work,
                    aggregation_instances,
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
                width_integer = codec.decode_integer(IntegerField::WidthDelta)?;
            }
        }
        class_state.finish_height_class()?;
        if detect_cycles {
            if height_delta == 0 && empty_width && new_symbols.len() == class_start_symbols {
                if empty_class {
                    if let SymbolCodec::Arithmetic(arithmetic) = &codec {
                        if cycle.active() || arithmetic.decoder.marker.is_some() {
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                assert(first_width_integer == EntropyInteger::OutOfBand);
                                assert(codec == cycle_after_first_width);
                                assert((class_state.decoded_symbols, class_state.height, false)
                                    == cycle_cursor);
                                let before = cycle_spec::source_state(
                                    &cycle_before, CycleMode::EmptyHeightClasses, cycle_cursor,
                                );
                                let after = cycle_spec::state(
                                    arithmetic, CycleMode::EmptyHeightClasses, cycle_cursor,
                                );
                                if cycle.checkpoint.is_some() {
                                    cycle_spec::height_zero_step(
                                        &cycle_before, &cycle_after_height,
                                        &cycle_after_first_width, cycle_cursor,
                                    );
                                }
                                let recorded = cycle_spec::record(&cycle, before, after);
                                cycle.history = Ghost(recorded.0);
                                cycle.anchor = Ghost(recorded.1);
                            }
                            let repeated = cycle.observe(
                                arithmetic,
                                (class_state.decoded_symbols, class_state.height, false),
                            );
                            if repeated {
                                #[cfg(verus_keep_ghost)]
                                proof! {
                                    cycle_spec::repetition_never_progresses(
                                        CycleMode::EmptyHeightClasses, cycle.history@,
                                    );
                                }
                                return Err(resources.exhaust_work(limits));
                            }
                        }
                    }
                }
                empty_class = true;
            } else {
                cycle.reset();
                empty_class = false;
            }
        }
    }
    class_state.finish()?;

    let exported = decode_export_runs(
        &mut codec,
        total_symbols,
        header.exported_symbols,
        limits,
        &mut resources,
    )?;

    let SymbolCodec::Arithmetic(arithmetic) = &codec else {
        return Err(SymbolTextError::InvalidState(
            "arithmetic dictionary has non-arithmetic state",
        ));
    };
    validate_bounded_arithmetic_marker(&arithmetic.decoder, normalized.len())?;

    let retained_generic = match generic_contexts.as_ref() {
        Some(contexts) if config.bitmap_context_retained => Some(contexts.try_snapshot(limits)?),
        _ => None,
    };
    let retained_refinement = match arithmetic.refinement.as_ref() {
        Some(contexts) => Some(contexts.try_snapshot(limits)?),
        None => None,
    };

    let exported_symbols =
        assemble_owned_exported_symbols(imported, new_symbols, &exported, limits)?;
    Ok((
        exported_symbols,
        retained_generic,
        retained_refinement,
        body.len(),
    ))
}
