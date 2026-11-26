use super::contexts::{validate_probability_states, validate_refinement_context_layout};
use super::generic::{decode_probability_signal, probability_signal_error};
use super::generic_context::{packed_pixel, packed_row_bits, sample_cached};
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::headers::validate_target_adaptive;
use super::segments::{validate_arithmetic_terminator, validate_observed_marker};
use super::support::{
    bitmap_storage_bytes, enforce_managed_memory, enforce_refinement_work, enforce_work,
    refinement_work, sample_bitmap, sample_signed,
};
use super::{
    GenericRegionError, RefinementArithmeticCall, RefinementContextDescriptor, RefinementContexts,
    RefinementParameters, RefinementPlane, RefinementRegionHeader, RefinementSlot,
    RefinementTemplate, TemplateOffset,
};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::{Bitmap, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Decode one bounded arithmetic generic-refinement bitmap.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(reference),
    ensures match out {
        Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
        Err(_) => true,
    },
))]
pub(crate) fn decode_arithmetic_refinement(
    coded: &[u8],
    header: &RefinementRegionHeader,
    parameters: RefinementParameters,
    reference: &Bitmap,
    limits: &Limits,
) -> Result<Bitmap, GenericRegionError> {
    validate_arithmetic_terminator(coded)?;
    if header.template == RefinementTemplate::Template0 {
        validate_target_adaptive(1, header.adaptive[0])?;
    }
    enforce_refinement_work(
        header.region.width,
        header.region.height,
        header.typical_prediction,
        limits,
    )?;

    let mut contexts = RefinementContexts::new(header.template, limits)?;
    let mut decoder = match ArithmeticDecoder::new(coded) {
        Ok(decoder) => decoder,
        Err(error) => return Err(GenericRegionError::Decode(error.into_decode())),
    };
    let mut cumulative_work = 0;
    let bitmap = decode_refinement_on_validated(
        &mut decoder,
        &mut contexts,
        RefinementArithmeticCall {
            width: header.region.width,
            height: header.region.height,
            adaptive: header.adaptive,
            typical_prediction: header.typical_prediction,
        },
        reference,
        parameters,
        limits,
        &mut cumulative_work,
    )?;
    validate_observed_marker(&decoder, coded.len())?;
    Ok(bitmap)
}

/// Decode one refinement bitmap on an existing arithmetic stream and
/// statistics set.
///
/// This kernel does not create or reset the decoder, does not reset `GR`
/// statistics, and does not validate an end marker. It is therefore safe to
/// interleave with arithmetic integers and generic-region decisions.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        crate::spec::bitmap::storage_valid(reference),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder))
            && generic_region_spec::refinement_contexts_valid(final(contexts)),
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_refinement_on(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut RefinementContexts,
    call: RefinementArithmeticCall,
    reference: &Bitmap,
    parameters: RefinementParameters,
    limits: &Limits,
    cumulative_work: &mut u64,
) -> Result<Bitmap, GenericRegionError> {
    validate_refinement_context_layout(contexts.template, &contexts.contexts)?;
    validate_probability_states(&contexts.contexts)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(generic_region_spec::refinement_contexts_valid);
    }
    decode_refinement_on_validated(
        decoder,
        contexts,
        call,
        reference,
        parameters,
        limits,
        cumulative_work,
    )
}

/// Reuse a refinement context bank whose validity was established at creation
/// or checked restore.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::refinement_contexts_valid(old(contexts)),
        crate::spec::bitmap::storage_valid(reference),
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder))
            && generic_region_spec::refinement_contexts_valid(final(contexts)),
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_refinement_on_validated(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut RefinementContexts,
    call: RefinementArithmeticCall,
    reference: &Bitmap,
    parameters: RefinementParameters,
    limits: &Limits,
    cumulative_work: &mut u64,
) -> Result<Bitmap, GenericRegionError> {
    validate_refinement_context_layout(contexts.template, &contexts.contexts)?;
    if contexts.template == RefinementTemplate::Template0 {
        validate_target_adaptive(1, call.adaptive[0])?;
    }

    let call_work = refinement_work(call.width, call.height, call.typical_prediction)?;
    let next_work = cumulative_work
        .checked_add(call_work)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_work(next_work, limits)?;
    let bitmap_bytes = bitmap_storage_bytes(call.width, call.height)?;
    enforce_managed_memory(contexts.storage_bytes, bitmap_bytes, limits)?;
    let mut bitmap = Bitmap::blank(call.width, call.height, limits)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(&bitmap);
        reveal(generic_region_spec::refinement_contexts_valid);
    }
    *cumulative_work = next_work;

    super::fast_refinement::decode_refinement_fast(
        &mut bitmap,
        reference,
        contexts.template,
        call.adaptive,
        call.typical_prediction,
        parameters,
        decoder,
        &mut contexts.contexts,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(generic_region_spec::refinement_contexts_valid);
    }
    Ok(bitmap)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        crate::spec::bitmap::storage_valid(reference),
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(clippy::ptr_arg, clippy::too_many_arguments)]
pub(super) fn decode_refinement_with(
    bitmap: &mut Bitmap,
    reference: &Bitmap,
    template: RefinementTemplate,
    adaptive: [TemplateOffset; 2],
    typical_prediction: bool,
    parameters: RefinementParameters,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<(), GenericRegionError> {
    let descriptor = RefinementContextDescriptor::new(template, adaptive);
    let bitmap_width = bitmap.width;
    let bitmap_height = bitmap.height;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    let mut ltp = false;
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= bitmap_height,
            crate::views::bitmap_width(bitmap) == bitmap_width,
            crate::views::bitmap_height(bitmap) == bitmap_height,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::spec::bitmap::storage_valid(reference),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
        decreases bitmap_height - y,
    ))]
    while y < bitmap_height {
        if typical_prediction {
            let context = template.sltp_context();
            let sltp = decode_probability_signal(decoder, contexts, context);
            if sltp > 1 {
                return Err(probability_signal_error(
                    decoder,
                    context,
                    contexts.len(),
                    sltp,
                ));
            }
            ltp ^= sltp != 0;
        }
        let mut history = 0_usize;
        let mut x = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                x <= bitmap_width,
                y < bitmap_height,
                crate::views::bitmap_width(bitmap) == bitmap_width,
                crate::views::bitmap_height(bitmap) == bitmap_height,
                crate::spec::bitmap::storage_valid(bitmap),
                crate::spec::bitmap::storage_valid(reference),
                crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                generic_region_spec::contexts_valid(contexts@),
            decreases bitmap_width - x,
        ))]
        while x < bitmap_width {
            if ltp {
                if let Some(prediction) = refinement_prediction_cached(reference, x, y, parameters)
                {
                    history = ((history << 1) | usize::from(prediction)) & 1;
                    if prediction {
                        bitmap.set_pixel(x, y, true)?;
                    }
                    x += 1;
                    continue;
                }
            }
            let context = descriptor.context(bitmap, reference, x, y, history, parameters);
            let signal = decode_probability_signal(decoder, contexts, context);
            if signal > 1 {
                return Err(probability_signal_error(
                    decoder,
                    context,
                    contexts.len(),
                    signal,
                ));
            }
            let value = signal != 0;
            history = ((history << 1) | usize::from(value)) & 1;
            if value {
                bitmap.set_pixel(x, y, true)?;
            }
            x += 1;
        }
        y += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.plane == RefinementPlane::Target,
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive.is_none(),
))]
pub(super) const fn target_slot(x: i32, y: i32) -> RefinementSlot {
    RefinementSlot {
        plane: RefinementPlane::Target,
        nominal: TemplateOffset::new(x, y),
        adaptive: None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.plane == RefinementPlane::Target,
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive == Some(number),
))]
pub(super) const fn target_at(x: i32, y: i32, number: u8) -> RefinementSlot {
    RefinementSlot {
        plane: RefinementPlane::Target,
        nominal: TemplateOffset::new(x, y),
        adaptive: Some(number),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.plane == RefinementPlane::Reference,
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive.is_none(),
))]
pub(super) const fn reference_slot(x: i32, y: i32) -> RefinementSlot {
    RefinementSlot {
        plane: RefinementPlane::Reference,
        nominal: TemplateOffset::new(x, y),
        adaptive: None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.plane == RefinementPlane::Reference,
        out.nominal.x == x,
        out.nominal.y == y,
        out.adaptive == Some(number),
))]
pub(super) const fn reference_at(x: i32, y: i32, number: u8) -> RefinementSlot {
    RefinementSlot {
        plane: RefinementPlane::Reference,
        nominal: TemplateOffset::new(x, y),
        adaptive: Some(number),
    }
}

macro_rules! target_slot_const {
    ($x:expr, $y:expr) => {
        RefinementSlot {
            plane: RefinementPlane::Target,
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: None,
        }
    };
}

macro_rules! target_at_const {
    ($x:expr, $y:expr, $number:expr) => {
        RefinementSlot {
            plane: RefinementPlane::Target,
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: Some($number),
        }
    };
}

macro_rules! reference_slot_const {
    ($x:expr, $y:expr) => {
        RefinementSlot {
            plane: RefinementPlane::Reference,
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: None,
        }
    };
}

macro_rules! reference_at_const {
    ($x:expr, $y:expr, $number:expr) => {
        RefinementSlot {
            plane: RefinementPlane::Reference,
            nominal: TemplateOffset { x: $x, y: $y },
            adaptive: Some($number),
        }
    };
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const REFINEMENT_TEMPLATE_0: [RefinementSlot; 13] = [
    target_at_const!(-1, -1, 1),
    target_slot_const!(0, -1),
    target_slot_const!(1, -1),
    target_slot_const!(-1, 0),
    reference_at_const!(-1, -1, 2),
    reference_slot_const!(0, -1),
    reference_slot_const!(1, -1),
    reference_slot_const!(-1, 0),
    reference_slot_const!(0, 0),
    reference_slot_const!(1, 0),
    reference_slot_const!(-1, 1),
    reference_slot_const!(0, 1),
    reference_slot_const!(1, 1),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) const REFINEMENT_TEMPLATE_1: [RefinementSlot; 10] = [
    target_slot_const!(-1, -1),
    target_slot_const!(0, -1),
    target_slot_const!(1, -1),
    target_slot_const!(-1, 0),
    reference_slot_const!(0, -1),
    reference_slot_const!(-1, 0),
    reference_slot_const!(0, 0),
    reference_slot_const!(1, 0),
    reference_slot_const!(0, 1),
    reference_slot_const!(1, 1),
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn refinement_adaptive_offset(
    adaptive: [TemplateOffset; 2],
    number: u8,
    nominal: TemplateOffset,
) -> TemplateOffset {
    match number {
        1 => adaptive[0],
        2 => adaptive[1],
        _ => nominal,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires
        crate::spec::bitmap::storage_valid(target),
        crate::spec::bitmap::storage_valid(reference),
))]
#[allow(clippy::bool_to_int_with_if)]
pub(super) fn refinement_context(
    target: &Bitmap,
    reference: &Bitmap,
    x: u32,
    y: u32,
    template: RefinementTemplate,
    adaptive: [TemplateOffset; 2],
    parameters: RefinementParameters,
) -> usize {
    let count = match template {
        RefinementTemplate::Template0 => 13usize,
        RefinementTemplate::Template1 => 10usize,
    };
    let mut context = 0_usize;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= count,
            count <= 13,
            count == match template {
                RefinementTemplate::Template0 => 13usize,
                RefinementTemplate::Template1 => 10usize,
            },
            crate::spec::bitmap::storage_valid(target),
            crate::spec::bitmap::storage_valid(reference),
        decreases count - index,
    ))]
    while index < count {
        let slot = match template {
            RefinementTemplate::Template0 => REFINEMENT_TEMPLATE_0[index],
            RefinementTemplate::Template1 => REFINEMENT_TEMPLATE_1[index],
        };
        let offset = match slot.adaptive {
            Some(number) => refinement_adaptive_offset(adaptive, number, slot.nominal),
            None => slot.nominal,
        };
        let value = match slot.plane {
            RefinementPlane::Target => sample_bitmap(target, x, y, offset),
            RefinementPlane::Reference => sample_reference(reference, x, y, offset, parameters),
        };
        context = (context << 1) | if value { 1 } else { 0 };
        index += 1;
    }
    context
}

impl RefinementContextDescriptor {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) const fn new(template: RefinementTemplate, adaptive: [TemplateOffset; 2]) -> Self {
        match template {
            RefinementTemplate::Template0 => {
                if adaptive[0].x == -1
                    && adaptive[0].y == -1
                    && adaptive[1].x == -1
                    && adaptive[1].y == -1
                {
                    Self::NominalTemplate0
                } else {
                    Self::Template0 {
                        target: adaptive[0],
                        reference: adaptive[1],
                    }
                }
            }
            RefinementTemplate::Template1 => Self::Template1,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn context(
        self,
        target: &Bitmap,
        reference: &Bitmap,
        x: u32,
        y: u32,
        history: usize,
        parameters: RefinementParameters,
    ) -> usize {
        let reference_x = i64::from(x) - i64::from(parameters.reference_dx);
        let reference_y = i64::from(y) - i64::from(parameters.reference_dy);
        match self {
            Self::NominalTemplate0 => {
                let target_previous =
                    packed_row_bits(target, i64::from(x), i64::from(y) - 1, -1, 3);
                let reference_previous =
                    packed_row_bits(reference, reference_x, reference_y - 1, -1, 3);
                let reference_current = packed_row_bits(reference, reference_x, reference_y, -1, 3);
                let reference_next =
                    packed_row_bits(reference, reference_x, reference_y + 1, -1, 3);
                let mut context = (target_previous << 1) | history & 1;
                context = (context << 3) | reference_previous;
                context = (context << 3) | reference_current;
                (context << 3) | reference_next
            }
            Self::Template0 {
                target: target_adaptive,
                reference: reference_adaptive,
            } => {
                let target_a = usize::from(sample_cached(target, x, y, target_adaptive));
                let target_previous = packed_row_bits(target, i64::from(x), i64::from(y) - 1, 0, 2);
                let reference_a = usize::from(packed_pixel(
                    reference,
                    reference_x + i64::from(reference_adaptive.x),
                    reference_y + i64::from(reference_adaptive.y),
                ));
                let reference_previous =
                    packed_row_bits(reference, reference_x, reference_y - 1, 0, 2);
                let reference_current = packed_row_bits(reference, reference_x, reference_y, -1, 3);
                let reference_next =
                    packed_row_bits(reference, reference_x, reference_y + 1, -1, 3);
                let mut context = (target_a << 2) | target_previous;
                context = (context << 1) | history & 1;
                context = (context << 1) | reference_a;
                context = (context << 2) | reference_previous;
                context = (context << 3) | reference_current;
                (context << 3) | reference_next
            }
            Self::Template1 => {
                let target_previous =
                    packed_row_bits(target, i64::from(x), i64::from(y) - 1, -1, 3);
                let reference_previous =
                    usize::from(packed_pixel(reference, reference_x, reference_y - 1));
                let reference_current = packed_row_bits(reference, reference_x, reference_y, -1, 3);
                let reference_next = packed_row_bits(reference, reference_x, reference_y + 1, 0, 2);
                let mut context = (target_previous << 1) | history & 1;
                context = (context << 1) | reference_previous;
                context = (context << 3) | reference_current;
                (context << 2) | reference_next
            }
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn refinement_prediction_cached(
    reference: &Bitmap,
    x: u32,
    y: u32,
    parameters: RefinementParameters,
) -> Option<bool> {
    let reference_x = i64::from(x) - i64::from(parameters.reference_dx);
    let reference_y = i64::from(y) - i64::from(parameters.reference_dy);
    let neighbourhood = (packed_row_bits(reference, reference_x, reference_y - 1, -1, 3) << 6)
        | (packed_row_bits(reference, reference_x, reference_y, -1, 3) << 3)
        | packed_row_bits(reference, reference_x, reference_y + 1, -1, 3);
    match neighbourhood {
        0 => Some(false),
        0x01ff => Some(true),
        _ => None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires crate::spec::bitmap::storage_valid(reference),
))]
pub(super) fn refinement_prediction(
    reference: &Bitmap,
    x: u32,
    y: u32,
    parameters: RefinementParameters,
) -> Option<bool> {
    let common = sample_reference(reference, x, y, TemplateOffset::new(-1, -1), parameters);
    if sample_reference(reference, x, y, TemplateOffset::new(0, -1), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(1, -1), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(-1, 0), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(0, 0), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(1, 0), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(-1, 1), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(0, 1), parameters) != common
        || sample_reference(reference, x, y, TemplateOffset::new(1, 1), parameters) != common
    {
        return None;
    }
    Some(common)
}

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires crate::spec::bitmap::storage_valid(bitmap),
))]
#[allow(clippy::cast_lossless)]
pub(super) fn sample_reference(
    bitmap: &Bitmap,
    x: u32,
    y: u32,
    offset: TemplateOffset,
    parameters: RefinementParameters,
) -> bool {
    let reference_x = (x as i64)
        .checked_sub(parameters.reference_dx as i64)
        .and_then(|value| value.checked_add(offset.x as i64));
    let reference_y = (y as i64)
        .checked_sub(parameters.reference_dy as i64)
        .and_then(|value| value.checked_add(offset.y as i64));
    sample_signed(bitmap, reference_x, reference_y)
}
