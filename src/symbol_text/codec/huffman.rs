//! Huffman implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    bit_reader, decode_refinement_on, normalized_bounded_arithmetic,
    validate_bounded_arithmetic_marker, ArithmeticDecoder, BitReader, Bitmap, EntropyInteger,
    HuffmanCodec, HuffmanFieldTables, HuffmanValue, IntegerField, Limits, MalformedError,
    RefinementArithmeticCall, RefinementContexts, RefinementParameters, RefinementRequest,
    RefinementTemplate, SymbolIdRequest, SymbolTextError, TemplateOffset,
};

/// Rounds a bit reader forward to the next byte boundary.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).bit_position <= old(reader).bytes@.len() * 8,
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        match out {
            Ok(()) =>
                final(reader).bit_position >= old(reader).bit_position
                && final(reader).bit_position <= final(reader).bytes@.len() * 8
                && final(reader).bit_position % 8 == 0
                && final(reader).bit_position < old(reader).bit_position + 8,
            Err(_) => final(reader).bit_position == old(reader).bit_position,
        },
))]
pub(in crate::symbol_text) fn byte_align(
    reader: &mut BitReader<'_>,
) -> Result<(), SymbolTextError> {
    let total_bits = reader
        .bytes
        .len()
        .checked_mul(8)
        .ok_or(MalformedError::IntegerOverflow)?;
    let aligned = reader
        .bit_position
        .checked_add(7)
        .ok_or(MalformedError::IntegerOverflow)?
        / 8
        * 8;
    if aligned > total_bits {
        let needed = aligned
            .checked_sub(reader.bit_position)
            .and_then(|bits| u64::try_from(bits).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        let remaining = total_bits
            .checked_sub(reader.bit_position)
            .and_then(|bits| u64::try_from(bits).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        return Err(MalformedError::UnexpectedEof {
            offset: u64::try_from(reader.bit_position)
                .map_err(|_error| MalformedError::IntegerOverflow)?,
            needed,
            remaining,
        }
        .into());
    }
    reader.bit_position = aligned;
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(codec) => crate::huffman::huffman_spec::reader_valid(&codec.reader)
            && codec.tables == tables
            && symbol_text_spec::huffman_codec_initial(
                &codec, bytes@, log_strip_size, refinement_template,
            ),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn new_huffman_codec<'a>(
    bytes: &'a [u8],
    tables: HuffmanFieldTables<'a>,
    log_strip_size: u8,
    refinement_template: Option<RefinementTemplate>,
    limits: &Limits,
) -> Result<HuffmanCodec<'a>, SymbolTextError> {
    if bytes.len() > usize::MAX / 8 {
        return Err(MalformedError::IntegerOverflow.into());
    }
    Ok(HuffmanCodec {
        reader: bit_reader(bytes),
        tables,
        log_strip_size,
        symbol_id_table: None,
        refinement: match refinement_template {
            Some(template) => Some(RefinementContexts::new(template, limits)?),
            None => None,
        },
        refinement_work: 0,
    })
}

impl HuffmanCodec<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::huffman::huffman_spec::reader_valid(&old(self).reader),
        ensures
            crate::huffman::huffman_spec::reader_valid(&final(self).reader),
            final(self).reader.bytes@ == old(self).reader.bytes@,
            final(self).tables == old(self).tables,
            final(self).log_strip_size == old(self).log_strip_size,
            final(self).symbol_id_table == old(self).symbol_id_table,
            final(self).refinement == old(self).refinement,
            final(self).refinement_work == old(self).refinement_work,
            match out {
                Ok(value) =>
                    count <= 32
                        && value as nat == crate::spec::readers::bits_value(
                            old(self).reader.bytes@, old(self).reader.bit_position as nat,
                            count as nat,
                        ),
                Err(_) => true,
            },
            out == match crate::spec::readers::bit_read_error(
                old(self).reader.bytes@, old(self).reader.bit_position, count,
            ) {
                Some(error) => Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                None => Ok(crate::spec::readers::bits_value(
                    old(self).reader.bytes@, old(self).reader.bit_position as nat, count as nat,
                ) as u32),
            },
            final(self).reader.bit_position == old(self).reader.bit_position
                + if crate::spec::readers::bit_read_error(
                    old(self).reader.bytes@, old(self).reader.bit_position, count,
                ).is_none() { count as nat } else { 0nat },
    ))]
    pub(in crate::symbol_text) fn read_raw_bits(
        &mut self,
        count: u8,
    ) -> Result<u32, SymbolTextError> {
        Ok(self.reader.read_bits(count)?)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::huffman::huffman_spec::reader_valid(&old(self).reader),
        ensures
            crate::huffman::huffman_spec::reader_valid(&final(self).reader),
            final(self).log_strip_size == old(self).log_strip_size,
            (field == IntegerField::WithinStripT || field == IntegerField::RefinementIndicator)
                ==> {
                    let count = if field == IntegerField::WithinStripT {
                        old(self).log_strip_size
                    } else { 1u8 };
                    &&& final(self).reader.bytes@ == old(self).reader.bytes@
                    &&& out == match crate::spec::readers::bit_read_error(
                        old(self).reader.bytes@, old(self).reader.bit_position, count,
                    ) {
                        Some(error) => Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                        None => Ok(EntropyInteger::Value(crate::spec::readers::bits_value(
                            old(self).reader.bytes@, old(self).reader.bit_position as nat, count as nat,
                        ) as i64)),
                    }
                    &&& final(self).reader.bit_position == old(self).reader.bit_position
                        + if crate::spec::readers::bit_read_error(
                            old(self).reader.bytes@, old(self).reader.bit_position, count,
                        ).is_none() { count as nat } else { 0nat }
                },
    ))]
    pub(in crate::symbol_text) fn decode_integer(
        &mut self,
        field: IntegerField,
    ) -> Result<EntropyInteger, SymbolTextError> {
        match field {
            IntegerField::WithinStripT => Ok(EntropyInteger::Value(i64::from(
                self.read_raw_bits(self.log_strip_size)?,
            ))),
            IntegerField::RefinementIndicator => {
                Ok(EntropyInteger::Value(i64::from(self.read_raw_bits(1)?)))
            }
            _ => {
                let table = self.tables.table_for(field).ok_or(
                    SymbolTextError::InvalidTableAssociation("no Huffman table bound for field"),
                )?;
                match table.decode(&mut self.reader)? {
                    HuffmanValue::Integer(value) => Ok(EntropyInteger::Value(i64::from(value))),
                    HuffmanValue::OutOfBand => Ok(EntropyInteger::OutOfBand),
                }
            }
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires crate::huffman::huffman_spec::reader_valid(&old(self).reader),
        ensures
            crate::huffman::huffman_spec::reader_valid(&final(self).reader),
            out == symbol_text_spec::huffman_symbol_id_state(old(self), request).0,
            final(self).reader.bit_position == symbol_text_spec::huffman_symbol_id_state(old(self), request).1,
            final(self).reader.bytes@ == old(self).reader.bytes@,
            final(self).tables == old(self).tables,
            final(self).log_strip_size == old(self).log_strip_size,
            final(self).symbol_id_table == old(self).symbol_id_table,
            final(self).refinement == old(self).refinement,
            final(self).refinement_work == old(self).refinement_work,
            old(self).symbol_id_table.is_none() ==> {
                &&& final(self).reader.bytes@ == old(self).reader.bytes@
                &&& out == match crate::spec::readers::bit_read_error(
                    old(self).reader.bytes@, old(self).reader.bit_position, request.code_length,
                ) {
                    Some(error) => Err(SymbolTextError::Decode(DecodeError::Malformed(error))),
                    None => Ok(crate::spec::readers::bits_value(
                        old(self).reader.bytes@, old(self).reader.bit_position as nat,
                        request.code_length as nat,
                    ) as u32),
                }
                &&& final(self).reader.bit_position == old(self).reader.bit_position
                    + if crate::spec::readers::bit_read_error(
                        old(self).reader.bytes@, old(self).reader.bit_position, request.code_length,
                    ).is_none() { request.code_length as nat } else { 0nat }
            },
    ))]
    pub(in crate::symbol_text) fn decode_symbol_id(
        &mut self,
        request: SymbolIdRequest,
    ) -> Result<u32, SymbolTextError> {
        // When a symbol-ID Huffman table has been prepared (T.88 7.4.3.1.7,
        // normal text-region decoding), decode through it. Otherwise this is
        // a symbol dictionary's refinement/aggregate ID (T.88 6.5.8.2.3):
        // an implicit index code of `code_length` bits, for both single-
        // and multiple-instance symbols.
        if let Some(table) = self.symbol_id_table.as_ref() {
            match table.decode(&mut self.reader)? {
                HuffmanValue::Integer(value) => u32::try_from(value).map_err(|_| {
                    #[cfg(verus_keep_ghost)]
                    ensures(|out: SymbolTextError| {
                        equal(out, SymbolTextError::InvalidEntropyValue("symbol ID"))
                    });
                    SymbolTextError::InvalidEntropyValue("symbol ID")
                }),
                HuffmanValue::OutOfBand => Err(SymbolTextError::UnexpectedOutOfBand("symbol ID")),
            }
        } else {
            self.read_raw_bits(request.code_length)
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::huffman::huffman_spec::reader_valid(&old(self).reader),
            crate::spec::bitmap::storage_valid(request.reference),
        ensures
            crate::huffman::huffman_spec::reader_valid(&final(self).reader),
            match out {
                Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap),
                Err(_) => true,
            },
    ))]
    pub(in crate::symbol_text) fn decode_refinement(
        &mut self,
        request: RefinementRequest<'_>,
        limits: &Limits,
    ) -> Result<Bitmap, SymbolTextError> {
        let declared_size = request
            .declared_data_size
            .ok_or(SymbolTextError::InvalidState(
                "Huffman refinement bitmap is missing its declared byte size",
            ))?;
        byte_align(&mut self.reader)?;
        let byte_offset = self
            .reader
            .bit_position
            .checked_div(8)
            .ok_or(MalformedError::IntegerOverflow)?;
        let size = usize::try_from(declared_size).map_err(|_| MalformedError::IntegerOverflow)?;
        let end = byte_offset
            .checked_add(size)
            .ok_or(MalformedError::IntegerOverflow)?;
        let remaining = if byte_offset <= self.reader.bytes.len() {
            self.reader.bytes.len() - byte_offset
        } else {
            0
        };
        let slice =
            self.reader
                .bytes
                .get(byte_offset..end)
                .ok_or(MalformedError::UnexpectedEof {
                    offset: u64::try_from(byte_offset)
                        .map_err(|_| MalformedError::IntegerOverflow)?,
                    needed: declared_size.into(),
                    remaining: u64::try_from(remaining)
                        .map_err(|_| MalformedError::IntegerOverflow)?,
                })?;
        let normalized = normalized_bounded_arithmetic(slice, limits)?;

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
                "missing Huffman refinement contexts",
            ))?;
        let mut decoder = ArithmeticDecoder::new(normalized.bytes())?;
        let bitmap = decode_refinement_on(
            &mut decoder,
            contexts,
            call,
            request.reference,
            parameters,
            limits,
            &mut self.refinement_work,
        )?;
        validate_bounded_arithmetic_marker(&decoder, normalized.len())?;

        let advanced_bits = end.checked_mul(8).ok_or(MalformedError::IntegerOverflow)?;
        self.reader.bit_position = advanced_bits;
        Ok(bitmap)
    }
}
