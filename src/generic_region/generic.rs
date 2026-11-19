use super::contexts::{validate_generic_context_layout, validate_probability_states};
use super::generic_context::packed_pixel;
#[cfg(verus_keep_ghost)]
use super::generic_region_spec;
use super::headers::validate_generic_adaptive_values;
use super::segments::{validate_arithmetic_terminator, validate_observed_marker};
use super::support::{
    bitmap_storage_bytes, copy_bitmap_row, enforce_generic_work, enforce_input_limit,
    enforce_managed_memory, enforce_work, generic_work,
};
use super::{
    generic_row_ring, GenericArithmeticCall, GenericArithmeticSession, GenericContextDescriptor,
    GenericContexts, GenericRegionError, GenericRegionHeader, GenericRowContext, GenericTemplate,
    TemplateOffset,
};
use crate::arithmetic::{ArithmeticContext, ArithmeticDecoder};
use crate::{Bitmap, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl<'a> GenericArithmeticSession<'a> {
    /// Start a reusable session at the beginning of one bounded arithmetic
    /// stream.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(session) => generic_region_spec::generic_session_valid(&session)
                && session.decoder.bytes@ == coded@
                && session.coded_length == coded@.len()
                && session.work_used == 0
                && session.contexts.template == template
                && session.contexts.extended_template == extended_template
                && session.limits == *limits
                && generic_region_spec::bounded_arithmetic_tail(coded@),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(
        coded: &'a [u8],
        template: GenericTemplate,
        extended_template: bool,
        limits: &Limits,
    ) -> Result<Self, GenericRegionError> {
        enforce_input_limit(coded, limits)?;
        validate_arithmetic_terminator(coded)?;
        let contexts = GenericContexts::new(template, extended_template, limits)?;
        let decoder = match ArithmeticDecoder::new(coded) {
            Ok(decoder) => decoder,
            Err(error) => return Err(GenericRegionError::Decode(error.into_decode())),
        };
        Ok(Self {
            decoder,
            contexts,
            coded_length: coded.len(),
            work_used: 0,
            limits: *limits,
        })
    }

    /// Decode another bitmap while preserving this session's decoder position
    /// and adaptive probability statistics.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            generic_region_spec::generic_session_valid(old(self)),
            match call.skip {
                Some(mask) => crate::spec::bitmap::storage_valid(mask),
                None => true,
            },
        ensures
            out.is_ok() ==> generic_region_spec::generic_session_valid(final(self)),
            match out {
                Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
                Err(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn decode_bitmap(
        &mut self,
        call: GenericArithmeticCall<'_>,
    ) -> Result<Bitmap, GenericRegionError> {
        self.decode_bitmap_rows(call, call.height)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            generic_region_spec::generic_session_valid(old(self)),
            match call.skip {
                Some(mask) => crate::spec::bitmap::storage_valid(mask),
                None => true,
            },
        ensures
            out.is_ok() ==> generic_region_spec::generic_session_valid(final(self)),
            match out {
                Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
                Err(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    fn decode_bitmap_rows(
        &mut self,
        call: GenericArithmeticCall<'_>,
        rows: u32,
    ) -> Result<Bitmap, GenericRegionError> {
        decode_generic_on_validated(
            &mut self.decoder,
            &mut self.contexts,
            call,
            rows,
            &self.limits,
            &mut self.work_used,
        )
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            generic_region_spec::generic_session_valid(old(self)),
            crate::spec::bitmap::storage_valid(old(bitmap)),
            bitmap.width == call.width,
            bitmap.height == call.height,
        ensures
            out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
            out.is_ok() ==> generic_region_spec::generic_session_valid(final(self)),
    ))]
    fn decode_bitmap_rows_into(
        &mut self,
        call: GenericArithmeticCall<'_>,
        rows: u32,
        bitmap: &mut Bitmap,
    ) -> Result<(), GenericRegionError> {
        decode_generic_on_into(
            &mut self.decoder,
            &mut self.contexts,
            call,
            rows,
            &self.limits,
            &mut self.work_used,
            bitmap,
        )
    }

    /// Current compressed-byte position. A recognized marker is not consumed,
    /// so this equals the marker prefix offset after marker recognition.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.decoder.byte_position,
    ))]
    pub(crate) const fn consumed_byte_offset(&self) -> usize {
        self.decoder.byte_position()
    }

    /// Offset of a marker already observed by the arithmetic decoder.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == match self.decoder.marker {
            Some(marker) => Some(marker.offset),
            None => None,
        },
    ))]
    pub(crate) fn observed_marker_offset(&self) -> Option<usize> {
        self.decoder.marker().map(|marker| {
            #[cfg(verus_keep_ghost)]
            ensures(|out: usize| equal(out, marker.offset));
            marker.offset
        })
    }

    /// Validate that any observed marker is the bounded final `FF AC` marker
    /// and return that marker's offset.
    ///
    /// The arithmetic decoder need not read the marker to complete the
    /// requested decisions. The complete input block was already checked by
    /// [`Self::new`], so the returned offset is still authoritative when
    /// [`Self::observed_marker_offset`] is `None`.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == generic_region_spec::final_marker_result(
            self.decoder.marker, self.coded_length,
        ),
    ))]
    pub(crate) fn validate_final_marker(&self) -> Result<usize, GenericRegionError> {
        validate_observed_marker(&self.decoder, self.coded_length)?;
        self.coded_length
            .checked_sub(2)
            .ok_or(GenericRegionError::InvalidArithmeticTerminator)
    }
}

/// Decode arithmetic generic-region data, optionally with a skip bitmap.
///
/// `rows` may be less than the declared height only for unknown-length
/// immediate generic regions.  Undecoded nominal rows remain zero.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires match skip {
        Some(mask) => crate::spec::bitmap::storage_valid(mask),
        None => true,
    },
    ensures match out {
        Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
        Err(_) => true,
    },
))]
pub(crate) fn decode_arithmetic_generic(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    skip: Option<&Bitmap>,
    limits: &Limits,
) -> Result<Bitmap, GenericRegionError> {
    if header.mmr || rows > header.region.height {
        return Err(GenericRegionError::InvalidFlags(0x01));
    }
    validate_arithmetic_terminator(coded)?;
    validate_generic_adaptive_values(header.template, header.extended_template, &header.adaptive)?;
    enforce_generic_work(header.region.width, rows, header.typical_prediction, limits)?;
    if let Some(skip_bitmap) = skip {
        if skip_bitmap.width() != header.region.width
            || skip_bitmap.height() != header.region.height
        {
            return Err(GenericRegionError::InvalidSkipDimensions);
        }
    }

    let mut session =
        GenericArithmeticSession::new(coded, header.template, header.extended_template, limits)?;
    let bitmap = session.decode_bitmap_rows(
        GenericArithmeticCall {
            width: header.region.width,
            height: header.region.height,
            adaptive: header.adaptive,
            typical_prediction: header.typical_prediction,
            skip,
        },
        rows,
    )?;
    session.validate_final_marker()?;
    Ok(bitmap)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(old(bitmap)),
    ensures out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
))]
pub(super) fn decode_arithmetic_generic_into(
    coded: &[u8],
    header: &GenericRegionHeader,
    rows: u32,
    limits: &Limits,
    bitmap: &mut Bitmap,
) -> Result<(), GenericRegionError> {
    if header.mmr
        || rows > header.region.height
        || bitmap.width() != header.region.width
        || bitmap.height() != header.region.height
    {
        return Err(GenericRegionError::InvalidFlags(0x01));
    }
    validate_arithmetic_terminator(coded)?;
    validate_generic_adaptive_values(header.template, header.extended_template, &header.adaptive)?;
    enforce_generic_work(header.region.width, rows, header.typical_prediction, limits)?;
    let mut session =
        GenericArithmeticSession::new(coded, header.template, header.extended_template, limits)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    session.decode_bitmap_rows_into(
        GenericArithmeticCall {
            width: header.region.width,
            height: header.region.height,
            adaptive: header.adaptive,
            typical_prediction: header.typical_prediction,
            skip: None,
        },
        rows,
        bitmap,
    )?;
    session.validate_final_marker()?;
    Ok(())
}

/// Decode one generic bitmap on an existing arithmetic stream and statistics
/// set.
///
/// This kernel neither initializes nor resets `decoder` or `contexts`, and it
/// does not require or validate an end marker. `cumulative_work` is charged
/// before the first arithmetic decision and is preserved across interleaved
/// decoding procedures.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        match call.skip {
            Some(mask) => crate::spec::bitmap::storage_valid(mask),
            None => true,
        },
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder))
            && generic_region_spec::generic_contexts_valid(final(contexts)),
        Err(_) => true,
    },
))]
#[allow(clippy::collapsible_if, clippy::too_many_lines)]
pub(crate) fn decode_generic_on(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut GenericContexts,
    call: GenericArithmeticCall<'_>,
    rows: u32,
    limits: &Limits,
    cumulative_work: &mut u64,
) -> Result<Bitmap, GenericRegionError> {
    if rows > call.height {
        return Err(MalformedError::GenericRegionRowCount {
            declared: call.height,
            actual: rows,
        }
        .into());
    }
    validate_generic_context_layout(
        contexts.template,
        contexts.extended_template,
        &contexts.contexts,
    )?;
    validate_probability_states(&contexts.contexts)?;
    decode_generic_on_validated(decoder, contexts, call, rows, limits, cumulative_work)
}

/// Reuse a bank whose validity was established at creation or checked restore.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::generic_contexts_valid(old(contexts)),
        match call.skip {
            Some(mask) => crate::spec::bitmap::storage_valid(mask),
            None => true,
        },
    ensures match out {
        Ok(bitmap) =>
            crate::spec::bitmap::storage_valid(&bitmap)
            && crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder))
            && generic_region_spec::generic_contexts_valid(final(contexts)),
        Err(_) => true,
    },
))]
#[allow(clippy::collapsible_if, clippy::too_many_lines)]
pub(crate) fn decode_generic_on_validated(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut GenericContexts,
    call: GenericArithmeticCall<'_>,
    rows: u32,
    limits: &Limits,
    cumulative_work: &mut u64,
) -> Result<Bitmap, GenericRegionError> {
    if rows > call.height {
        return Err(MalformedError::GenericRegionRowCount {
            declared: call.height,
            actual: rows,
        }
        .into());
    }
    validate_generic_context_layout(
        contexts.template,
        contexts.extended_template,
        &contexts.contexts,
    )?;
    validate_generic_adaptive_values(
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
    )?;
    if let Some(skip) = call.skip {
        if skip.width() != call.width || skip.height() != call.height {
            return Err(GenericRegionError::InvalidSkipDimensions);
        }
    }

    let call_work = generic_work(call.width, rows, call.typical_prediction)?;
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
        reveal(generic_region_spec::generic_contexts_valid);
    }
    *cumulative_work = next_work;

    let descriptor = GenericContextDescriptor::new(
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
    );
    if matches!(descriptor, GenericContextDescriptor::NominalTemplate0) && call.skip.is_none() {
        if generic_row_ring::try_decode(
            &mut bitmap,
            call.width,
            rows,
            contexts.storage_bytes,
            bitmap_bytes,
            limits,
            call.typical_prediction,
            decoder,
            &mut contexts.contexts,
        )? {
            #[cfg(verus_keep_ghost)]
            proof! {
                reveal(generic_region_spec::generic_contexts_valid);
            }
            return Ok(bitmap);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(generic_region_spec::generic_contexts_valid);
        }
    }

    decode_generic_with(
        &mut bitmap,
        rows,
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
        call.typical_prediction,
        call.skip,
        decoder,
        &mut contexts.contexts,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(generic_region_spec::generic_contexts_valid);
    }
    Ok(bitmap)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        crate::spec::bitmap::storage_valid(old(bitmap)),
        bitmap.width == call.width,
        bitmap.height == call.height,
    ensures
        out.is_ok() ==> crate::spec::bitmap::storage_valid(final(bitmap)),
        out.is_ok() ==>
            crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder))
            && generic_region_spec::generic_contexts_valid(final(contexts)),
))]
#[allow(
    clippy::collapsible_if,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
pub(super) fn decode_generic_on_into(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut GenericContexts,
    call: GenericArithmeticCall<'_>,
    rows: u32,
    limits: &Limits,
    cumulative_work: &mut u64,
    bitmap: &mut Bitmap,
) -> Result<(), GenericRegionError> {
    if rows > call.height || bitmap.width() != call.width || bitmap.height() != call.height {
        return Err(MalformedError::GenericRegionRowCount {
            declared: call.height,
            actual: rows,
        }
        .into());
    }
    validate_generic_context_layout(
        contexts.template,
        contexts.extended_template,
        &contexts.contexts,
    )?;
    validate_probability_states(&contexts.contexts)?;
    validate_generic_adaptive_values(
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
    )?;
    let call_work = generic_work(call.width, rows, call.typical_prediction)?;
    let next_work = cumulative_work
        .checked_add(call_work)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_work(next_work, limits)?;
    let bitmap_bytes = bitmap_storage_bytes(call.width, call.height)?;
    enforce_managed_memory(contexts.storage_bytes, bitmap_bytes, limits)?;
    *cumulative_work = next_work;

    let descriptor = GenericContextDescriptor::new(
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
    );
    if matches!(descriptor, GenericContextDescriptor::NominalTemplate0) {
        if generic_row_ring::try_decode(
            bitmap,
            call.width,
            rows,
            contexts.storage_bytes,
            bitmap_bytes,
            limits,
            call.typical_prediction,
            decoder,
            &mut contexts.contexts,
        )? {
            return Ok(());
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(generic_region_spec::generic_contexts_valid);
        }
    }
    decode_generic_with(
        bitmap,
        rows,
        contexts.template,
        contexts.extended_template,
        &call.adaptive,
        call.typical_prediction,
        None,
        decoder,
        &mut contexts.contexts,
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        generic_region_spec::contexts_valid(final(contexts)@),
        match out {
            Ok(bit) =>
                bit <= 1
                && crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
            Err(_) => true,
        },
))]
#[allow(clippy::ptr_arg)]
pub(super) fn decode_probability(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
    index: usize,
) -> Result<u8, GenericRegionError> {
    if index >= contexts.len() {
        return Err(GenericRegionError::ContextIndex {
            index,
            count: contexts.len(),
        });
    }
    let result = decoder.decode(&mut contexts[index]);
    match result {
        Ok(bit) => Ok(bit),
        Err(error) => Err(GenericRegionError::Decode(error.into_decode())),
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) const RASTER_SIGNAL_CONTEXT: u8 = 4;

#[allow(clippy::ptr_arg)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
    ensures
        out <= RASTER_SIGNAL_CONTEXT,
        generic_region_spec::contexts_valid(final(contexts)@),
        out <= 1 ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
pub(crate) fn decode_probability_signal(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
    index: usize,
) -> u8 {
    let Some(context) = contexts.get_mut(index) else {
        return RASTER_SIGNAL_CONTEXT;
    };
    decoder.decode_signal(context)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == generic_region_spec::probability_error_for_signal(
        decoder, index, context_count, signal,
    ),
))]
#[cold]
pub(crate) fn probability_signal_error(
    decoder: &ArithmeticDecoder<'_>,
    index: usize,
    context_count: usize,
    signal: u8,
) -> GenericRegionError {
    if signal == RASTER_SIGNAL_CONTEXT {
        GenericRegionError::ContextIndex {
            index,
            count: context_count,
        }
    } else {
        GenericRegionError::Decode(decoder.signal_error(signal).into_decode())
    }
}

#[allow(clippy::ptr_arg)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(bitmap)),
        rows <= old(bitmap).height,
        crate::arithmetic::arithmetic_spec::decoder_valid(old(decoder)),
        generic_region_spec::contexts_valid(old(contexts)@),
        match skip {
            Some(mask) => crate::spec::bitmap::storage_valid(mask),
            None => true,
        },
    ensures
        crate::spec::bitmap::storage_valid(final(bitmap)),
        generic_region_spec::contexts_valid(final(contexts)@),
        out.is_ok() ==> crate::arithmetic::arithmetic_spec::decoder_valid(final(decoder)),
))]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn decode_generic_with(
    bitmap: &mut Bitmap,
    rows: u32,
    template: GenericTemplate,
    extended: bool,
    adaptive: &[TemplateOffset; 12],
    typical_prediction: bool,
    skip: Option<&Bitmap>,
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut Vec<ArithmeticContext>,
) -> Result<(), GenericRegionError> {
    let bitmap_width = bitmap.width;
    let bitmap_height = bitmap.height;
    if rows > bitmap_height {
        return Err(GenericRegionError::RowCountMismatch {
            expected: bitmap_height,
            actual: rows,
        });
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(bitmap);
    }
    let descriptor = GenericContextDescriptor::new(template, extended, adaptive);
    let mut ltp = false;
    let mut y = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            y <= rows,
            rows <= bitmap_height,
            crate::views::bitmap_width(bitmap) == bitmap_width,
            crate::views::bitmap_height(bitmap) == bitmap_height,
            crate::spec::bitmap::storage_valid(bitmap),
            crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
            generic_region_spec::contexts_valid(contexts@),
            match skip {
                Some(mask) => crate::spec::bitmap::storage_valid(mask),
                None => true,
            },
        decreases rows - y,
    ))]
    while y < rows {
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
        if ltp {
            if y != 0 {
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(bitmap);
                    assert(bitmap.height == bitmap_height);
                    assert(y - 1 < bitmap_height);
                    assert(y < bitmap_height);
                }
                copy_bitmap_row(bitmap, y - 1, y)?;
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::views::bitmap_fields(bitmap);
                    assert(bitmap.width == bitmap_width);
                    assert(bitmap.height == bitmap_height);
                }
            }
            y += 1;
            continue;
        }

        let mut history = 0_usize;
        let mut row_context = GenericRowContext::new(descriptor, bitmap, y);
        let mut x = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                x <= bitmap_width,
                y < rows,
                rows <= bitmap_height,
                crate::views::bitmap_width(bitmap) == bitmap_width,
                crate::views::bitmap_height(bitmap) == bitmap_height,
                crate::spec::bitmap::storage_valid(bitmap),
                crate::arithmetic::arithmetic_spec::decoder_valid(decoder),
                generic_region_spec::contexts_valid(contexts@),
                match skip {
                    Some(mask) => crate::spec::bitmap::storage_valid(mask),
                    None => true,
                },
            decreases bitmap_width - x,
        ))]
        while x < bitmap_width {
            let skipped = match skip {
                Some(mask) => packed_pixel(mask, i64::from(x), i64::from(y)),
                None => false,
            };
            if skipped {
                history = (history << 1) & 0x0f;
                row_context.advance(bitmap, x);
                x += 1;
                continue;
            }
            let context = row_context.context_and_advance(bitmap, x, y, history);
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
            history = ((history << 1) | usize::from(value)) & 0x0f;
            if value {
                bitmap.set_pixel(x, y, true)?;
            }
            x += 1;
        }
        y += 1;
    }
    Ok(())
}
