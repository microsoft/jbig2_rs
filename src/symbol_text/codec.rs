//! Arithmetic and Huffman entropy adapters, symbol IDs and refinement geometry.

#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::{
    bit_reader, check_dimensions, checked_unsigned_delta, decode_generic_on_validated,
    decode_refinement_on_validated, enforce_limit, required_integer, widen_bool_i64,
    ArithmeticCodec, ArithmeticContext, ArithmeticDecoder, ArithmeticInteger, BitReader, Bitmap,
    DecodeError, EntropyInteger, GenericArithmeticCall, GenericContexts, HuffmanCodec,
    HuffmanFieldTables, HuffmanLine, HuffmanTable, HuffmanValue, IntegerContexts, IntegerField,
    LimitKind, Limits, MalformedError, NormalizedArithmetic, RefinementArithmeticCall,
    RefinementContexts, RefinementGeometry, RefinementParameters, RefinementRequest,
    RefinementTemplate, ResourceTracker, SymbolCodec, SymbolIdRequest, SymbolLengthRun,
    SymbolTextError, TemplateOffset, BOUNDED_ARITHMETIC_TERMINATOR, INTEGER_FIELD_COUNT,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl<'a> SymbolCodec<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::codec_valid(old(self)),
        ensures
            symbol_text_spec::arithmetic_integer_transition(old(self), final(self), field, out),
            symbol_text_spec::huffman_integer_transition(old(self), final(self), field, out),
            match out {
                Ok(_) => symbol_text_spec::codec_valid(final(self)),
                Err(_) => true,
            },
            match *old(self) {
                Self::Huffman(before) => match *final(self) {
                    Self::Huffman(after) =>
                        (field == IntegerField::WithinStripT
                            || field == IntegerField::RefinementIndicator) ==> {
                            let count = if field == IntegerField::WithinStripT {
                                before.log_strip_size
                            } else { 1u8 };
                            &&& after.reader.bytes@ == before.reader.bytes@
                            &&& out == match crate::spec::readers::bit_read_error(
                                before.reader.bytes@, before.reader.bit_position, count,
                            ) {
                                Some(error) => Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                                None => Ok(EntropyInteger::Value(crate::spec::readers::bits_value(
                                    before.reader.bytes@, before.reader.bit_position as nat, count as nat,
                                ) as i64)),
                            }
                            &&& after.reader.bit_position == before.reader.bit_position
                                + if crate::spec::readers::bit_read_error(
                                    before.reader.bytes@, before.reader.bit_position, count,
                                ).is_none() { count as nat } else { 0nat }
                        },
                    _ => false,
                },
                _ => true,
            },
    ))]
    pub(super) fn decode_integer(
        &mut self,
        field: IntegerField,
    ) -> Result<EntropyInteger, SymbolTextError> {
        match self {
            Self::Arithmetic(codec) => codec.decode_integer(field),
            Self::Huffman(codec) => codec.decode_integer(field),
            #[cfg(test)]
            Self::Scripted(codec) => codec
                .integers
                .pop_front()
                .ok_or(SymbolTextError::InvalidState("missing scripted integer")),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::codec_valid(old(self)),
        ensures
            symbol_text_spec::arithmetic_symbol_id_transition(old(self), final(self), request, out),
            symbol_text_spec::huffman_symbol_id_transition(old(self), final(self), request, out),
            match out {
                Ok(_) => symbol_text_spec::codec_valid(final(self)),
                Err(_) => true,
            },
            match *old(self) {
                Self::Huffman(before) => match *final(self) {
                    Self::Huffman(after) => before.symbol_id_table.is_none() ==> {
                        &&& after.reader.bytes@ == before.reader.bytes@
                        &&& out == match crate::spec::readers::bit_read_error(
                            before.reader.bytes@, before.reader.bit_position, request.code_length,
                        ) {
                            Some(error) => Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                            None => Ok(crate::spec::readers::bits_value(
                                before.reader.bytes@, before.reader.bit_position as nat,
                                request.code_length as nat,
                            ) as u32),
                        }
                        &&& after.reader.bit_position == before.reader.bit_position
                            + if crate::spec::readers::bit_read_error(
                                before.reader.bytes@, before.reader.bit_position, request.code_length,
                            ).is_none() { request.code_length as nat } else { 0nat }
                    },
                    _ => false,
                },
                _ => true,
            },
    ))]
    pub(super) fn decode_symbol_id(
        &mut self,
        request: SymbolIdRequest,
    ) -> Result<u32, SymbolTextError> {
        match self {
            Self::Arithmetic(codec) => codec.decode_symbol_id(request),
            Self::Huffman(codec) => codec.decode_symbol_id(request),
            #[cfg(test)]
            Self::Scripted(codec) => Ok(codec.symbol_id),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            symbol_text_spec::codec_valid(old(self)),
            crate::spec::bitmap::storage_valid(request.reference),
        ensures match out {
            Ok(bitmap) =>
                symbol_text_spec::codec_valid(final(self))
                    && crate::spec::bitmap::storage_valid(&bitmap),
            Err(_) => true,
        },
    ))]
    pub(super) fn decode_refinement(
        &mut self,
        request: RefinementRequest<'_>,
        limits: &Limits,
    ) -> Result<Bitmap, SymbolTextError> {
        match self {
            Self::Arithmetic(codec) => codec.decode_refinement(request, limits),
            Self::Huffman(codec) => codec.decode_refinement(request, limits),
            #[cfg(test)]
            Self::Scripted(_) => Ok(Bitmap::blank(
                request.geometry.width,
                request.geometry.height,
                limits,
            )?),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::codec_valid(old(self)),
        ensures match out {
            Ok(codec) =>
                crate::huffman::huffman_spec::reader_valid(&codec.reader),
            Err(_) => true,
        },
    ))]
    pub(super) fn huffman_mut(&mut self) -> Result<&mut HuffmanCodec<'a>, SymbolTextError> {
        match self {
            Self::Huffman(codec) => Ok(codec),
            Self::Arithmetic(_) => Err(SymbolTextError::InvalidState(
                "Huffman cursor requested from arithmetic state",
            )),
            #[cfg(test)]
            Self::Scripted(_) => Err(SymbolTextError::InvalidState(
                "Huffman cursor requested from scripted state",
            )),
        }
    }
}

mod arithmetic;
mod huffman;
mod refinement;
mod symbols;

#[allow(unused_imports)]
pub(super) use arithmetic::{
    new_arithmetic_codec, normalized_bounded_arithmetic, validate_bounded_arithmetic_marker,
    validate_bounded_arithmetic_terminator,
};
pub(super) use huffman::{byte_align, new_huffman_codec};
pub(crate) use refinement::refinement_geometry;
#[allow(unused_imports)]
pub(super) use refinement::{decode_refined_bitmap, floor_divide_by_two};
pub(crate) use symbols::{checked_symbol_id, symbol_id_code_length};
#[allow(unused_imports)]
pub(super) use symbols::{
    decode_checked_symbol_id, decode_symbol_id_code_lengths, expand_symbol_code_lengths,
    extend_lengths, read_symbol_id_runcode_lines,
};
