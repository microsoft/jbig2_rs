#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

mod arithmetic_composition;
mod benchmarks;
mod codec_and_exports;
mod dictionary_dispatch;
mod flags_and_tables;
mod huffman_fields;
mod huffman_symbol_ids;
mod placement_render;
mod placement_state;
mod resource_accounting;
mod text_termination;

use super::bitmap::{bitmap_from_packed_bytes, clone_bitmap};
use super::codec::{decode_refined_bitmap, expand_symbol_code_lengths};
use super::dictionary::{assemble_exported_symbols, decode_export_runs};
use super::headers::parse_text_huffman_flags;
use super::placement::{flush_placement_batch, negative_magnitude, prepare_placement};
use super::tables::{
    associate_dictionary_tables, associate_text_tables, bind_tables, find_table_binding,
    materialize_huffman_table, same_table_parameter, select_dictionary_tables, select_text_tables,
    wrap_custom_tables,
};
use super::*;
use std::hint::black_box;
use std::time::{Duration, Instant};

fn arithmetic_codec_fixture(bytes: &[u8], iaid_count: usize, salt: usize) -> ArithmeticCodec<'_> {
    ArithmeticCodec {
        decoder: ArithmeticDecoder::new(bytes).expect("arithmetic constructor"),
        integers: IntegerContexts {
            slots: std::array::from_fn(|slot| {
                std::array::from_fn(|index| {
                    ArithmeticContext::from_state(
                        u8::try_from((index * 17 + slot + salt) % 47).unwrap(),
                        u8::try_from((index / 2 + slot + salt) % 2).unwrap(),
                    )
                    .unwrap()
                })
            }),
        },
        iaid: (0..iaid_count)
            .map(|index| {
                ArithmeticContext::from_state(
                    u8::try_from((index * 17 + salt) % 47).unwrap(),
                    u8::try_from((index / 2 + salt) % 2).unwrap(),
                )
                .unwrap()
            })
            .collect(),
        refinement: None,
        refinement_work: 73,
        prefix: None,
    }
}

fn assert_arithmetic_forwarding<T: std::fmt::Debug + PartialEq>(
    actual: Result<T, SymbolTextError>,
    expected: Result<T, SymbolTextError>,
) -> bool {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => {
            assert_eq!(actual, expected);
            true
        }
        (
            Err(SymbolTextError::Decode(DecodeError::Malformed(actual))),
            Err(SymbolTextError::Decode(DecodeError::Malformed(expected))),
        ) => {
            assert_eq!(actual, expected);
            false
        }
        (actual, expected) => {
            panic!("wrong arithmetic forwarding: {actual:?}, expected {expected:?}")
        }
    }
}

fn assert_arithmetic_cursor_and_continuation(
    actual: &mut ArithmeticDecoder<'_>,
    expected: &mut ArithmeticDecoder<'_>,
    success: bool,
) {
    assert_eq!(actual.byte_position(), expected.byte_position());
    assert_eq!(actual.marker(), expected.marker());
    if success {
        let mut actual_context = ArithmeticContext::new();
        let mut expected_context = ArithmeticContext::new();
        for _ in 0..64 {
            let signal = actual.decode_signal(&mut actual_context);
            assert_eq!(signal, expected.decode_signal(&mut expected_context));
            assert_eq!(actual_context, expected_context);
            assert_eq!(actual.byte_position(), expected.byte_position());
            assert_eq!(actual.marker(), expected.marker());
            if signal > 1 {
                break;
            }
        }
    }
}

fn scalar_compose_placement_oracle(
    destination: &mut Bitmap,
    source: &Bitmap,
    placement: Placement,
    operator: CombinationOperator,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<(), SymbolTextError> {
    let source_x = if placement.x < 0 {
        negative_magnitude(placement.x)?
    } else {
        0
    };
    let source_y = if placement.y < 0 {
        negative_magnitude(placement.y)?
    } else {
        0
    };
    let destination_x = placement.x.max(0);
    let destination_y = placement.y.max(0);
    let overlap_width = (i64::from(source.width()) - source_x)
        .min(i64::from(destination.width()) - destination_x)
        .max(0);
    let overlap_height = (i64::from(source.height()) - source_y)
        .min(i64::from(destination.height()) - destination_y)
        .max(0);
    resources.charge_work(
        u64::try_from(overlap_width)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(
                u64::try_from(overlap_height).map_err(|_| MalformedError::IntegerOverflow)?,
            )
            .ok_or(MalformedError::IntegerOverflow)?,
        limits,
    )?;
    for row in 0..overlap_height {
        for column in 0..overlap_width {
            let sx =
                u32::try_from(source_x + column).map_err(|_| MalformedError::IntegerOverflow)?;
            let sy = u32::try_from(source_y + row).map_err(|_| MalformedError::IntegerOverflow)?;
            let dx = u32::try_from(destination_x + column)
                .map_err(|_| MalformedError::IntegerOverflow)?;
            let dy =
                u32::try_from(destination_y + row).map_err(|_| MalformedError::IntegerOverflow)?;
            let incoming = source
                .pixel(sx, sy)
                .ok_or(MalformedError::IntegerOverflow)?;
            let current = destination
                .pixel(dx, dy)
                .ok_or(MalformedError::IntegerOverflow)?;
            destination.set_pixel(dx, dy, operator.apply(current, incoming))?;
        }
    }
    Ok(())
}

fn batched_compose_placement(
    destination: &mut Bitmap,
    placements: &[(&Bitmap, Placement)],
    operator: CombinationOperator,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<(), SymbolTextError> {
    let mut batch = PlacementBatch::new();
    batch.reserve(placements.len(), limits)?;
    for &(source, placement) in placements {
        if let Some(prepared) =
            prepare_placement(destination, source, placement, limits, resources)?
        {
            if !batch.compatible(prepared) {
                flush_placement_batch(destination, &mut batch, operator)?;
            }
            batch.push(prepared, destination)?;
        }
    }
    flush_placement_batch(destination, &mut batch, operator)
}

fn median_duration(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}

fn text_config(corner: ReferenceCorner, transposed: bool, strip_size: u8) -> TextRegionConfig {
    TextRegionConfig {
        huffman: false,
        refinement: false,
        strip_size,
        reference_corner: corner,
        transposed,
        operator: CombinationOperator::Or,
        default_pixel: false,
        s_offset: 0,
        refinement_template: 0,
        huffman_choices: None,
    }
}

fn assert_resource_guard_result(
    actual: Result<(), SymbolTextError>,
    expected: Result<(), Option<LimitExceeded>>,
) {
    match actual {
        Ok(()) => assert_eq!(expected, Ok(())),
        Err(SymbolTextError::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow))) => {
            assert_eq!(expected, Err(None));
        }
        Err(SymbolTextError::Decode(DecodeError::LimitExceeded(error))) => {
            assert_eq!(expected, Err(Some(error)));
        }
        other => panic!("unexpected resource guard result: {other:?}"),
    }
}

fn wide_placement_geometry(
    destination: &Bitmap,
    source: &Bitmap,
    placement: Placement,
) -> Option<(u32, u32, u32, u32, u32, u32)> {
    let x = i128::from(placement.x);
    let y = i128::from(placement.y);
    let left = x.max(0);
    let top = y.max(0);
    let right = (x + i128::from(source.width())).min(i128::from(destination.width()));
    let bottom = (y + i128::from(source.height())).min(i128::from(destination.height()));
    if right <= left || bottom <= top {
        None
    } else {
        Some((
            u32::try_from(left - x).unwrap(),
            u32::try_from(top - y).unwrap(),
            u32::try_from(left).unwrap(),
            u32::try_from(top).unwrap(),
            u32::try_from(right - left).unwrap(),
            u32::try_from(bottom - top).unwrap(),
        ))
    }
}

fn text_state_signature(
    state: &TextRegionState,
) -> (TextRegionConfig, u32, u32, i64, i64, Option<(i64, bool)>) {
    (
        state.config,
        state.declared_instances,
        state.decoded_instances,
        state.strip_t,
        state.first_s,
        state
            .strip
            .map(|strip| (strip.cursor_s, strip.has_instance)),
    )
}
