//! Arithmetic implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    decode_generic_on_validated, decode_refinement_on, enforce_limit, ArithmeticCodec,
    ArithmeticContext, ArithmeticDecoder, ArithmeticInteger, Bitmap, DecodeError, EntropyInteger,
    GenericArithmeticCall, GenericContexts, IntegerContexts, IntegerField, LimitKind, Limits,
    MalformedError, NormalizedArithmetic, RefinementArithmeticCall, RefinementContexts,
    RefinementParameters, RefinementRequest, RefinementTemplate, SymbolIdRequest, SymbolTextError,
    TemplateOffset, BOUNDED_ARITHMETIC_TERMINATOR, INTEGER_FIELD_COUNT,
};

impl IntegerContexts {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures symbol_text_spec::integer_contexts_initial(&out),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(in crate::symbol_text) fn new() -> Self {
        Self {
            slots: [[ArithmeticContext::new(); 512]; INTEGER_FIELD_COUNT],
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(codec) =>
            symbol_text_spec::arithmetic_codec_valid(&codec)
            && bytes@.len() >= 2
            && symbol_text_spec::integer_contexts_initial(&codec.integers)
            && (iaid_code_length as u32) < usize::BITS
            && codec.iaid@.len() == (1usize << iaid_code_length)
            && (forall|index: int| 0 <= index < codec.iaid@.len() ==>
                #[trigger] codec.iaid@[index]
                    == ArithmeticContext { state_index: 0, mps: 0 })
            && codec.refinement_work == 0
            && codec.refinement.is_some() == refinement_template.is_some()
            && codec.decoder.bytes@ == bytes@
            && codec.decoder.interval == crate::arithmetic::MIN_INTERVAL
            && codec.decoder.byte_position
                == crate::arithmetic::arithmetic_spec::initialized_state(bytes).unwrap().0
            && codec.decoder.current_byte
                == crate::arithmetic::arithmetic_spec::initialized_state(bytes).unwrap().1
            && codec.decoder.code
                == crate::arithmetic::arithmetic_spec::initialized_state(bytes).unwrap().2
            && codec.decoder.bit_count
                == crate::arithmetic::arithmetic_spec::initialized_state(bytes).unwrap().3
            && codec.decoder.marker
                == crate::arithmetic::arithmetic_spec::initialized_state(bytes).unwrap().4,
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn new_arithmetic_codec<'a>(
    bytes: &'a [u8],
    iaid_code_length: u8,
    refinement_template: Option<RefinementTemplate>,
    limits: &Limits,
) -> Result<ArithmeticCodec<'a>, SymbolTextError> {
    if u32::from(iaid_code_length) >= usize::BITS {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let count = 1_usize << iaid_code_length;
    let requested_bytes = count
        .checked_mul(core::mem::size_of::<ArithmeticContext>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        requested_bytes,
    )?;
    let mut iaid = Vec::new();
    iaid.try_reserve(count)
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    iaid.resize(count, ArithmeticContext::new());
    Ok(ArithmeticCodec {
        decoder: ArithmeticDecoder::new(bytes)?,
        integers: IntegerContexts {
            slots: [[ArithmeticContext::new(); 512]; INTEGER_FIELD_COUNT],
        },
        iaid,
        refinement: match refinement_template {
            Some(template) => Some(RefinementContexts::new(template, limits)?),
            None => None,
        },
        refinement_work: 0,
    })
}

impl ArithmeticCodec<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::arithmetic::arithmetic_spec::decoder_valid(&old(self).decoder),
            symbol_text_spec::integer_contexts_valid(&old(self).integers),
        ensures
            crate::arithmetic::arithmetic_spec::decoder_state_valid(&final(self).decoder),
            symbol_text_spec::integer_contexts_valid(&final(self).integers),
            final(self).iaid@ == old(self).iaid@,
            final(self).refinement == old(self).refinement,
            final(self).refinement_work == old(self).refinement_work,
            out == symbol_text_spec::arithmetic_integer_result(
                crate::arithmetic::arithmetic_spec::integer_state(
                    old(self).decoder,
                    old(self).integers.slots[symbol_text_spec::integer_field_slot(field) as int]@,
                ).0,
            ),
            final(self).decoder == crate::arithmetic::arithmetic_spec::integer_state(
                old(self).decoder,
                old(self).integers.slots[symbol_text_spec::integer_field_slot(field) as int]@,
            ).1,
            final(self).integers.slots[symbol_text_spec::integer_field_slot(field) as int]@
                == crate::arithmetic::arithmetic_spec::integer_state(
                    old(self).decoder,
                    old(self).integers.slots[symbol_text_spec::integer_field_slot(field) as int]@,
                ).2,
            forall|slot: int| 0 <= slot < INTEGER_FIELD_COUNT
                && slot != symbol_text_spec::integer_field_slot(field) ==>
                    final(self).integers.slots[slot]@ == old(self).integers.slots[slot]@,
            match out {
                Ok(_) => crate::arithmetic::arithmetic_spec::decoder_valid(
                    &final(self).decoder
                ),
                Err(_) => true,
            },
    ))]
    pub(in crate::symbol_text) fn decode_integer(
        &mut self,
        field: IntegerField,
    ) -> Result<EntropyInteger, SymbolTextError> {
        let slot = field.slot();
        let decoded = match self.decoder.decode_integer(&mut self.integers.slots[slot]) {
            Ok(decoded) => decoded,
            Err(error) => return Err(SymbolTextError::Decode(error.into_decode())),
        };
        match decoded {
            ArithmeticInteger::Value(value) => Ok(EntropyInteger::Value(value)),
            ArithmeticInteger::OutOfBand => Ok(EntropyInteger::OutOfBand),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::arithmetic::arithmetic_spec::decoder_valid(&old(self).decoder),
            symbol_text_spec::arithmetic_contexts_valid(old(self).iaid@),
        ensures
            crate::arithmetic::arithmetic_spec::decoder_state_valid(&final(self).decoder),
            symbol_text_spec::arithmetic_contexts_valid(final(self).iaid@),
            final(self).integers.slots@ == old(self).integers.slots@,
            final(self).refinement == old(self).refinement,
            final(self).refinement_work == old(self).refinement_work,
            out == symbol_text_spec::arithmetic_symbol_id_result(
                crate::arithmetic::arithmetic_spec::iaid_state(
                    old(self).decoder, old(self).iaid@, request.code_length,
                ).0,
            ),
            final(self).decoder == crate::arithmetic::arithmetic_spec::iaid_state(
                old(self).decoder, old(self).iaid@, request.code_length,
            ).1,
            final(self).iaid@ == crate::arithmetic::arithmetic_spec::iaid_state(
                old(self).decoder, old(self).iaid@, request.code_length,
            ).2,
            match out {
                Ok(value) => request.code_length <= crate::arithmetic::MAX_IAID_CODE_LENGTH
                    && (value as u64) < (1u64 << request.code_length),
                Err(_) => true,
            },
            match out {
                Ok(_) => crate::arithmetic::arithmetic_spec::decoder_valid(
                    &final(self).decoder
                ),
                Err(_) => true,
            },
    ))]
    pub(in crate::symbol_text) fn decode_symbol_id(
        &mut self,
        request: SymbolIdRequest,
    ) -> Result<u32, SymbolTextError> {
        match self
            .decoder
            .decode_iaid(request.code_length, &mut self.iaid)
        {
            Ok(value) => Ok(value),
            Err(error) => Err(SymbolTextError::Decode(error.into_decode())),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::arithmetic::arithmetic_spec::decoder_valid(&old(self).decoder),
            crate::spec::bitmap::storage_valid(request.reference),
        ensures match out {
            Ok(bitmap) =>
                crate::spec::bitmap::storage_valid(&bitmap)
                    && crate::arithmetic::arithmetic_spec::decoder_valid(
                        &final(self).decoder
                    )
                    && final(self).integers.slots@ == old(self).integers.slots@
                    && final(self).iaid@ == old(self).iaid@,
            Err(_) => true,
        },
    ))]
    pub(in crate::symbol_text) fn decode_refinement(
        &mut self,
        request: RefinementRequest<'_>,
        limits: &Limits,
    ) -> Result<Bitmap, SymbolTextError> {
        let adaptive = [
            TemplateOffset::new(
                i32::from(request.adaptive_pixels[0].0),
                i32::from(request.adaptive_pixels[0].1),
            ),
            TemplateOffset::new(
                i32::from(request.adaptive_pixels[1].0),
                i32::from(request.adaptive_pixels[1].1),
            ),
        ];
        let call = RefinementArithmeticCall {
            width: request.geometry.width,
            height: request.geometry.height,
            adaptive,
            typical_prediction: false,
        };
        let parameters = RefinementParameters {
            reference_dx: request.geometry.reference_dx,
            reference_dy: request.geometry.reference_dy,
        };
        let contexts = self
            .refinement
            .as_mut()
            .ok_or(SymbolTextError::InvalidState(
                "missing arithmetic refinement contexts",
            ))?;
        Ok(decode_refinement_on(
            &mut self.decoder,
            contexts,
            call,
            request.reference,
            parameters,
            limits,
            &mut self.refinement_work,
        )?)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::arithmetic::arithmetic_spec::decoder_valid(&old(self).decoder),
            crate::generic_region::generic_region_spec::generic_contexts_valid(old(contexts)),
            match call.skip {
                Some(mask) => crate::spec::bitmap::storage_valid(mask),
                None => true,
            },
        ensures match out {
            Ok(bitmap) =>
                crate::spec::bitmap::storage_valid(&bitmap)
                    && crate::arithmetic::arithmetic_spec::decoder_valid(
                        &final(self).decoder
                    )
                    && final(self).integers.slots@ == old(self).integers.slots@
                    && final(self).iaid@ == old(self).iaid@
                    && crate::generic_region::generic_region_spec::generic_contexts_valid(final(contexts)),
            Err(_) => true,
        },
    ))]
    pub(in crate::symbol_text) fn decode_generic(
        &mut self,
        contexts: &mut GenericContexts,
        call: GenericArithmeticCall<'_>,
        height: u32,
        limits: &Limits,
        cumulative_work: &mut u64,
    ) -> Result<Bitmap, SymbolTextError> {
        Ok(decode_generic_on_validated(
            &mut self.decoder,
            contexts,
            call,
            height,
            limits,
            cumulative_work,
        )?)
    }
}

/// Validates that a bounded arithmetic slice ends in `FF AC` and contains no
/// earlier marker, mirroring `generic_region`'s equivalent (private) check
/// for the Huffman-declared-size refinement bitmaps embedded in symbol
/// dictionaries and text regions (T.88 6.5.8.2.2, 6.4.11).
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            data@.len() >= 2
            && data@[data@.len() - 2] == 0xff
            && data@[data@.len() - 1] == 0xac
            && forall|i: int| 0 <= i < data@.len() - 2 ==> !(
                #[trigger] data@[i] == 0xff && data@[i + 1] > 0x8f
            ),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn validate_bounded_arithmetic_terminator(
    data: &[u8],
) -> Result<(), SymbolTextError> {
    if data.len() < 2
        || data[data.len() - 2] != BOUNDED_ARITHMETIC_TERMINATOR[0]
        || data[data.len() - 1] != BOUNDED_ARITHMETIC_TERMINATOR[1]
    {
        return Err(SymbolTextError::InvalidState(
            "embedded arithmetic refinement data is missing its FF AC terminator",
        ));
    }

    let final_offset = data
        .len()
        .checked_sub(2)
        .ok_or(MalformedError::IntegerOverflow)?;
    let mut offset = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            final_offset == data@.len() - 2,
            offset <= data@.len() - 1,
            forall|i: int| 0 <= i < offset ==> (
                !(#[trigger] data@[i] == 0xff && data@[i + 1] > 0x8f)
                || i == final_offset
            ),
        decreases data@.len() - 1 - offset,
    ))]
    while offset < data.len() - 1 {
        if data[offset] == 0xff && data[offset + 1] > 0x8f && offset != final_offset {
            return Err(SymbolTextError::InvalidState(
                "premature arithmetic marker in embedded refinement data",
            ));
        }
        offset += 1;
    }
    Ok(())
}

impl NormalizedArithmetic<'_> {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(in crate::symbol_text) fn bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(in crate::symbol_text) fn len(&self) -> usize {
        self.bytes().len()
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) fn normalized_bounded_arithmetic<'a>(
    data: &'a [u8],
    limits: &Limits,
) -> Result<NormalizedArithmetic<'a>, SymbolTextError> {
    if data.ends_with(&BOUNDED_ARITHMETIC_TERMINATOR) {
        validate_bounded_arithmetic_terminator(data)?;
        return Ok(NormalizedArithmetic::Borrowed(data));
    }
    if data.last() != Some(&BOUNDED_ARITHMETIC_TERMINATOR[0]) {
        return Err(SymbolTextError::InvalidState(
            "bounded arithmetic data has no terminal marker",
        ));
    }
    let requested_bytes = u64::try_from(data.len())
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    enforce_limit(
        LimitKind::Memory,
        limits.max_decoder_managed_memory_bytes,
        requested_bytes,
    )?;
    let mut normalized = Vec::new();
    normalized
        .try_reserve(
            data.len()
                .checked_add(1)
                .ok_or(MalformedError::IntegerOverflow)?,
        )
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    normalized.extend_from_slice(data);
    normalized.push(BOUNDED_ARITHMETIC_TERMINATOR[1]);
    validate_bounded_arithmetic_terminator(&normalized)?;
    Ok(NormalizedArithmetic::Owned(normalized))
}

/// Validates that any marker the decoder actually observed is the declared
/// final `FF AC`, after a successful bounded decode.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) fn validate_bounded_arithmetic_marker(
    decoder: &ArithmeticDecoder<'_>,
    data_len: usize,
) -> Result<(), SymbolTextError> {
    if let Some(marker) = decoder.marker() {
        let expected_offset = data_len
            .checked_sub(2)
            .ok_or(MalformedError::IntegerOverflow)?;
        if marker.code != BOUNDED_ARITHMETIC_TERMINATOR[1] || marker.offset != expected_offset {
            return Err(SymbolTextError::InvalidState(
                "unexpected arithmetic marker in embedded refinement data",
            ));
        }
    }
    Ok(())
}
