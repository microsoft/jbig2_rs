//! Termination implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{BitReader, EofbPolicy, MmrError, EOFB};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        mmr_spec::finish_block_eofb_policy(
            old(reader).bytes@, old(reader).bit_position as nat,
            final(reader).bit_position as nat, policy, out,
        ),
        match out {
            Ok(()) => match policy {
                EofbPolicy::Optional =>
                    final(reader).bit_position == old(reader).bit_position,
                EofbPolicy::Required =>
                    final(reader).bit_position == old(reader).bytes@.len() * 8
                    && mmr_spec::exact_eofb(
                        old(reader).bytes@,
                        old(reader).bit_position as nat,
                    )
                    && mmr_spec::required_eofb_prefix(
                        old(reader).bytes@, old(reader).bit_position as nat,
                        final(reader).bit_position as nat,
                    ),
            },
            Err(_) => true,
        },
))]
pub(super) fn finish_block(reader: &mut BitReader<'_>, policy: EofbPolicy) -> Result<(), MmrError> {
    if let EofbPolicy::Optional = policy {
        return Ok(());
    }
    let remaining = remaining_bits(reader)?;

    if remaining < 24 {
        return Err(MmrError::MissingEofb);
    }
    if reader.read_bits(24)? != EOFB {
        return Err(MmrError::InvalidEofb);
    }

    let padding = remaining_bits(reader)?;
    if padding > 7 {
        return Err(MmrError::TrailingData);
    }
    if padding != 0 {
        let padding_count = u8::try_from(padding).map_err(|_error| MmrError::IntegerOverflow)?;
        let value = reader.read_bits(padding_count)?;
        if value != 0 {
            return Err(MmrError::NonZeroEofbPadding);
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        mmr_spec::finish_prefix_eofb_policy(
            old(reader).bytes@, old(reader).bit_position as nat,
            final(reader).bit_position as nat, out,
        ),
        match out {
            Ok(()) =>
                old(reader).bit_position as nat + 24
                    <= final(reader).bit_position as nat
                && final(reader).bit_position as nat
                    <= old(reader).bit_position as nat + 31
                && final(reader).bit_position % 8 == 0
                && mmr_spec::exact_eofb(
                    old(reader).bytes@,
                    old(reader).bit_position as nat,
                )
                && mmr_spec::required_eofb_prefix(
                    old(reader).bytes@, old(reader).bit_position as nat,
                    final(reader).bit_position as nat,
                ),
            Err(_) => true,
        },
))]
pub(super) fn finish_required_prefix(reader: &mut BitReader<'_>) -> Result<(), MmrError> {
    if remaining_bits(reader)? < 24 {
        return Err(MmrError::MissingEofb);
    }
    if reader.read_bits(24)? != EOFB {
        return Err(MmrError::InvalidEofb);
    }

    let bit_remainder = reader.bit_position % 8;
    let padding = if bit_remainder == 0 {
        0
    } else {
        8_usize
            .checked_sub(bit_remainder)
            .ok_or(MmrError::IntegerOverflow)?
    };
    if remaining_bits(reader)? < padding {
        return Err(MmrError::MissingEofb);
    }
    if padding != 0 {
        let padding_count = u8::try_from(padding).map_err(|_error| MmrError::IntegerOverflow)?;
        if reader.read_bits(padding_count)? != 0 {
            return Err(MmrError::NonZeroEofbPadding);
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(reader),
    ensures
        out == Ok((reader.bytes@.len() * 8 - reader.bit_position) as usize),
        match out {
            Ok(remaining) =>
                remaining == reader.bytes@.len() * 8 - reader.bit_position,
            Err(_) => true,
        },
))]
pub(super) fn remaining_bits(reader: &BitReader<'_>) -> Result<usize, MmrError> {
    let total = reader
        .bytes
        .len()
        .checked_mul(8)
        .ok_or(MmrError::IntegerOverflow)?;
    total
        .checked_sub(reader.bit_position)
        .ok_or(MmrError::IntegerOverflow)
}
