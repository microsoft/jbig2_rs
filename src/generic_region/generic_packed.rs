#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::support::copy_bitmap_row;
use super::{
    decode_probability_signal, probability_signal_error, GenericContextDescriptor,
    GenericRegionError, GenericRowContext, GenericTemplate,
};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::{Bitmap, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        final(bitmap).stride == old(bitmap).stride,
        final(bitmap).data@.len() == old(bitmap).data@.len(),
        match out {
            Ok(()) => index < old(bitmap).data@.len()
                && final(bitmap).data@ == old(bitmap).data@.update(
                    index as int, old(bitmap).data@[index as int] | bits,
                ),
            Err(_) => final(bitmap).data@ == old(bitmap).data@,
        },
))]
fn store_bits(bitmap: &mut Bitmap, index: usize, bits: u8) -> Result<(), GenericRegionError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        reveal(crate::spec::bitmap::storage_valid);
    }
    if index >= bitmap.data.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    bitmap.data[index] |= bits;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines, clippy::ptr_arg)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        final(bitmap).width == old(bitmap).width,
        final(bitmap).height == old(bitmap).height,
        final(bitmap).stride == old(bitmap).stride,
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
pub(super) fn try_decode(
    bitmap: &mut Bitmap,
    rows: u32,
    template: GenericTemplate,
    descriptor: GenericContextDescriptor,
    typical_prediction: bool,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<bool, GenericRegionError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    if !matches!(
        descriptor,
        GenericContextDescriptor::NominalTemplate0
            | GenericContextDescriptor::NominalTemplate1
            | GenericContextDescriptor::NominalTemplate2
            | GenericContextDescriptor::NominalTemplate3
    ) {
        return Ok(false);
    }
    let width = bitmap.width;
    #[cfg(verus_keep_ghost)]
    let height = bitmap.height;
    #[cfg(verus_keep_ghost)]
    let stride = bitmap.stride;
    let mut ltp = false;
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= rows <= bitmap.height,
            bitmap.width == width,
            bitmap.height == height,
            bitmap.stride == stride,
            width == old(bitmap).width,
            height == old(bitmap).height,
            stride == old(bitmap).stride,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
        decreases rows - y,
    ))]
    while y < rows {
        if typical_prediction {
            let context = template.sltp_context();
            let signal = decode_probability_signal(decoder, contexts, context);
            if signal > 1 {
                return Err(probability_signal_error(
                    decoder,
                    context,
                    contexts.len(),
                    signal,
                ));
            }
            ltp ^= signal != 0;
        }
        if ltp {
            if y != 0 {
                copy_bitmap_row(bitmap, y - 1, y)?;
            }
            y += 1;
            continue;
        }
        let row_start = usize::try_from(y)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(bitmap.stride)
            .ok_or(MalformedError::IntegerOverflow)?;
        let mut history = 0_usize;
        let mut row_context = GenericRowContext::new(descriptor, bitmap, y);
        let mut x = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                x <= width,
                y < rows <= bitmap.height,
                bitmap.width == width,
                bitmap.height == height,
                bitmap.stride == stride,
                width == old(bitmap).width,
                height == old(bitmap).height,
                stride == old(bitmap).stride,
                crate::spec::bitmap::storage_valid(bitmap),
                crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                generic_region_spec::contexts_valid(contexts@),
            decreases width - x,
        ))]
        while x < width {
            let count = (width - x).min(8);
            let index = row_start
                .checked_add(usize::try_from(x / 8).map_err(|_| MalformedError::IntegerOverflow)?)
                .ok_or(MalformedError::IntegerOverflow)?;
            let mut bits = 0_u8;
            let mut offset = 0_u32;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    offset <= count <= 8,
                    x <= width,
                    count <= width - x,
                    y < rows <= bitmap.height,
                    bitmap.width == width,
                    bitmap.height == height,
                    bitmap.stride == stride,
                    width == old(bitmap).width,
                    height == old(bitmap).height,
                    stride == old(bitmap).stride,
                    crate::spec::bitmap::storage_valid(bitmap),
                    crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                    generic_region_spec::contexts_valid(contexts@),
                decreases count - offset,
            ))]
            while offset < count {
                let context = row_context.context_and_advance(bitmap, x + offset, y, history);
                let signal = decode_probability_signal(decoder, contexts, context);
                if signal > 1 {
                    if bits != 0 {
                        store_bits(bitmap, index, bits)?;
                    }
                    return Err(probability_signal_error(
                        decoder,
                        context,
                        contexts.len(),
                        signal,
                    ));
                }
                bits |= signal << (7 - offset);
                history = ((history << 1) | usize::from(signal)) & 15;
                offset += 1;
            }
            if bits != 0 {
                store_bits(bitmap, index, bits)?;
            }
            x += count;
        }
        y += 1;
    }
    Ok(true)
}
