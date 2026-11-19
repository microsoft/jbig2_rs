//! Codes implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    BitReader, MmrError, Mode, RunCode, BLACK_MAKEUP, BLACK_TERMINATING, COMMON_MAKEUP,
    MAX_MODE_CODE_BITS, MAX_RUN_CODE_BITS, MODE_CODES, WHITE_MAKEUP, WHITE_TERMINATING,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == if prefix as nat * 2 + bit as nat <= u16::MAX {
        Ok((prefix as nat * 2 + bit as nat) as u16)
    } else {
        Err(MmrError::IntegerOverflow)
    },
))]
pub(super) fn append_code_bit(prefix: u16, bit: u16) -> Result<u16, MmrError> {
    let doubled = prefix.checked_mul(2).ok_or(MmrError::IntegerOverflow)?;
    doubled.checked_add(bit).ok_or(MmrError::IntegerOverflow)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        final(reader).bit_position as nat
            <= old(reader).bit_position as nat + MAX_MODE_CODE_BITS as nat,
        match out {
            Ok(mode) => exists|code: ModeCode|
                mmr_spec::mode_code_in_implementation_table(code)
                && code.mode == mode
                && final(reader).bit_position
                    == old(reader).bit_position + code.length as usize
                && code.bits as nat == crate::spec::readers::bits_value(
                    old(reader).bytes@, old(reader).bit_position as nat, code.length as nat,
                ),
            Err(_) => true,
        },
))]
pub(super) fn decode_mode(reader: &mut BitReader<'_>) -> Result<Mode, MmrError> {
    let start = reader.bit_position;
    let mut prefix = 0_u16;
    let mut length = 1_u8;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            mmr_spec::reader_valid(reader),
            reader.bytes@ == old(reader).bytes@,
            1 <= length <= MAX_MODE_CODE_BITS + 1,
            start == old(reader).bit_position,
            reader.bit_position
                == start + (length - 1) as usize,
            prefix as nat == crate::spec::readers::bits_value(
                reader.bytes@, start as nat, (length - 1) as nat,
            ),
        decreases MAX_MODE_CODE_BITS + 1 - length,
    ))]
    while length <= MAX_MODE_CODE_BITS {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost previous_prefix = prefix;
        }
        let bit =
            u16::try_from(reader.read_bits(1)?).map_err(|_error| MmrError::IntegerOverflow)?;
        prefix = append_code_bit(prefix, bit)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::reader_properties::bits_value_append_one(
                reader.bytes@, start as nat, (length - 1) as nat,
            );
            assert(prefix as nat == previous_prefix as nat * 2 + bit as nat);
            assert(prefix as nat == crate::spec::readers::bits_value(
                reader.bytes@, start as nat, length as nat,
            ));
        }
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                index <= MODE_CODES@.len(),
                mmr_spec::reader_valid(reader),
                reader.bytes@ == old(reader).bytes@,
                start == old(reader).bit_position,
                reader.bit_position == start + length as usize,
                prefix as nat == crate::spec::readers::bits_value(
                    reader.bytes@, start as nat, length as nat,
                ),
            decreases MODE_CODES@.len() - index,
        ))]
        while index < MODE_CODES.len() {
            let code = MODE_CODES[index];
            if code.length == length && code.bits == prefix {
                #[cfg(verus_keep_ghost)]
                production_mmr_mode_entry!(code, index, prefix, length);
                return Ok(code.mode);
            }
            index += 1;
        }
        length = length.checked_add(1).ok_or(MmrError::IntegerOverflow)?;
    }
    Err(MmrError::InvalidMode { bit_offset: start })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        match out {
            Ok((after_first, end)) =>
                x <= after_first <= end <= width
                && x < end
                && mmr_spec::horizontal_pair(
                    black, old(reader).bytes@, old(reader).bit_position as nat,
                    final(reader).bit_position as nat, x, width, after_first, end,
                ),
            Err(_) => true,
        },
))]
pub(super) fn decode_horizontal_runs(
    reader: &mut BitReader<'_>,
    black: bool,
    x: u32,
    width: u32,
) -> Result<(u32, u32), MmrError> {
    let remaining = width.checked_sub(x).ok_or(MmrError::InvalidTransition)?;
    let first = decode_run(reader, black, remaining)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost split = reader.bit_position as nat;
        let ghost first_codes = choose|codes: Seq<RunCode>|
            #[trigger] codes.len() > 0
            && mmr_spec::complete_run(
                black, old(reader).bytes@, old(reader).bit_position as nat,
                split, first, codes,
            );
    }
    let after_first = x.checked_add(first).ok_or(MmrError::IntegerOverflow)?;
    let second_remaining = width
        .checked_sub(after_first)
        .ok_or(MmrError::RunExceedsRow)?;
    let second = decode_run(reader, !black, second_remaining)?;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost second_codes = choose|codes: Seq<RunCode>|
            #[trigger] codes.len() > 0
            && mmr_spec::complete_run(
                !black, old(reader).bytes@, split, reader.bit_position as nat,
                second, codes,
            );
    }
    let end = after_first
        .checked_add(second)
        .ok_or(MmrError::IntegerOverflow)?;
    if end > width {
        return Err(MmrError::RunExceedsRow);
    }
    if end == x {
        return Err(MmrError::NonProgress);
    }
    #[cfg(verus_keep_ghost)]
    production_mmr_horizontal_progress!(x, width, first, second);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(after_first - x == first);
        assert(end - after_first == second);
        assert(mmr_spec::complete_run(
            black, old(reader).bytes@, old(reader).bit_position as nat,
            old(reader).bit_position as nat + mmr_spec::run_bit_length(first_codes),
            first, first_codes,
        ));
        assert(reader.bit_position as nat == old(reader).bit_position as nat
            + mmr_spec::run_bit_length(first_codes)
            + mmr_spec::run_bit_length(second_codes));
        assert(mmr_spec::complete_run(
            !black, old(reader).bytes@,
            old(reader).bit_position as nat + mmr_spec::run_bit_length(first_codes),
            reader.bit_position as nat, second, second_codes,
        ));
        assert(mmr_spec::complete_run(
            black, old(reader).bytes@, old(reader).bit_position as nat,
            old(reader).bit_position as nat + mmr_spec::run_bit_length(first_codes),
            (after_first - x) as u32, first_codes,
        ));
        assert(mmr_spec::complete_run(
            !black, old(reader).bytes@,
            old(reader).bit_position as nat + mmr_spec::run_bit_length(first_codes),
            reader.bit_position as nat, (end - after_first) as u32, second_codes,
        ));
        assert(mmr_spec::horizontal_pair(
            black, old(reader).bytes@, old(reader).bit_position as nat,
            reader.bit_position as nat, x, width, after_first, end,
        ));
    }
    Ok((after_first, end))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        match out {
            Ok(total) => total <= remaining
                && exists|codes: Seq<RunCode>| mmr_spec::complete_run(
                    black, old(reader).bytes@, old(reader).bit_position as nat,
                    final(reader).bit_position as nat, total, codes,
                ),
            Err(_) => true,
        },
))]
pub(super) fn decode_run(
    reader: &mut BitReader<'_>,
    black: bool,
    remaining: u32,
) -> Result<u32, MmrError> {
    let mut total = 0_u32;
    let mut makeup_count = 0_u32;
    #[cfg(verus_keep_ghost)]
    proof_decl! {
        let ghost mut codes = Seq::<RunCode>::empty();
    }
    let max_makeups = remaining
        .checked_div(64)
        .and_then(|value| value.checked_add(1))
        .ok_or(MmrError::IntegerOverflow)?;

    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            mmr_spec::reader_valid(reader),
            reader.bytes@ == old(reader).bytes@,
            old(reader).bit_position <= reader.bit_position,
            total <= remaining,
            makeup_count <= max_makeups,
            total as nat == mmr_spec::run_value_sum(codes),
            reader.bit_position as nat
                == old(reader).bit_position as nat + mmr_spec::run_bit_length(codes),
            mmr_spec::run_prefix_matches(
                black, old(reader).bytes@, old(reader).bit_position as nat, codes,
            ),
            mmr_spec::run_all_makeups(codes),
        decreases max_makeups + 1 - makeup_count,
    ))]
    loop {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost before_codes = codes;
            let ghost before_position = reader.bit_position;
        }
        let code = decode_run_code(reader, black)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            codes = codes.push(code);
            assert(codes.drop_last() == before_codes);
            assert(codes.last() == code);
            assert(mmr_spec::run_prefix_matches(
                black, old(reader).bytes@, old(reader).bit_position as nat, codes,
            ));
        }
        total = total
            .checked_add(u32::from(code.value))
            .ok_or(MmrError::IntegerOverflow)?;
        if total > remaining {
            return Err(MmrError::RunExceedsRow);
        }
        if code.terminal {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(mmr_spec::complete_run(
                    black, old(reader).bytes@, old(reader).bit_position as nat,
                    reader.bit_position as nat, total, codes,
                ));
            }
            return Ok(total);
        }
        #[cfg(verus_keep_ghost)]
        production_mmr_makeup_bounds!(black, code);
        makeup_count = makeup_count
            .checked_add(1)
            .ok_or(MmrError::IntegerOverflow)?;
        if makeup_count > max_makeups {
            return Err(MmrError::RunExceedsRow);
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires mmr_spec::reader_valid(old(reader)),
    ensures
        mmr_spec::reader_valid(final(reader)),
        final(reader).bytes@ == old(reader).bytes@,
        old(reader).bit_position <= final(reader).bit_position,
        final(reader).bit_position as nat
            <= old(reader).bit_position as nat + MAX_RUN_CODE_BITS as nat,
        match out {
            Ok(code) =>
                mmr_spec::run_code_in_implementation_tables(black, code)
                && final(reader).bit_position
                    == old(reader).bit_position + code.length as usize
                && code.bits as nat == crate::spec::readers::bits_value(
                    old(reader).bytes@, old(reader).bit_position as nat, code.length as nat,
                ),
            Err(_) => true,
        },
))]
pub(super) fn decode_run_code(
    reader: &mut BitReader<'_>,
    black: bool,
) -> Result<RunCode, MmrError> {
    let start = reader.bit_position;
    let mut prefix = 0_u16;
    let mut length = 1_u8;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            mmr_spec::reader_valid(reader),
            reader.bytes@ == old(reader).bytes@,
            1 <= length <= MAX_RUN_CODE_BITS + 1,
            start == old(reader).bit_position,
            reader.bit_position
                == start + (length - 1) as usize,
            prefix as nat == crate::spec::readers::bits_value(
                reader.bytes@, start as nat, (length - 1) as nat,
            ),
        decreases MAX_RUN_CODE_BITS + 1 - length,
    ))]
    while length <= MAX_RUN_CODE_BITS {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost previous_prefix = prefix;
        }
        let bit =
            u16::try_from(reader.read_bits(1)?).map_err(|_error| MmrError::IntegerOverflow)?;
        prefix = append_code_bit(prefix, bit)?;
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::reader_properties::bits_value_append_one(
                reader.bytes@, start as nat, (length - 1) as nat,
            );
            assert(prefix as nat == previous_prefix as nat * 2 + bit as nat);
            assert(prefix as nat == crate::spec::readers::bits_value(
                reader.bytes@, start as nat, length as nat,
            ));
        }
        if let Some(code) = find_run_code(black, prefix, length) {
            #[cfg(verus_keep_ghost)]
            production_mmr_run_code_bounds!(black, code);
            return Ok(code);
        }
        length = length.checked_add(1).ok_or(MmrError::IntegerOverflow)?;
    }
    Err(MmrError::InvalidRunCode {
        bit_offset: start,
        black,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(code) =>
            mmr_spec::run_code_in_implementation_tables(black, code)
            && mmr_spec::run_key_matches(code, bits, length),
        None => true,
    },
))]
pub(super) fn find_run_code(black: bool, bits: u16, length: u8) -> Option<RunCode> {
    let mut index = 0_usize;
    if black {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                black,
                index <= BLACK_TERMINATING@.len(),
            decreases BLACK_TERMINATING@.len() - index,
        ))]
        while index < BLACK_TERMINATING.len() {
            let code = BLACK_TERMINATING[index];
            if code.length == length && code.bits == bits {
                #[cfg(verus_keep_ghost)]
                production_mmr_run_entry!(
                    BLACK_TERMINATING,
                    code,
                    index,
                    black,
                    true,
                    bits,
                    length
                );
                return Some(code);
            }
            index += 1;
        }
        index = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                black,
                index <= BLACK_MAKEUP@.len(),
            decreases BLACK_MAKEUP@.len() - index,
        ))]
        while index < BLACK_MAKEUP.len() {
            let code = BLACK_MAKEUP[index];
            if code.length == length && code.bits == bits {
                #[cfg(verus_keep_ghost)]
                production_mmr_run_entry!(BLACK_MAKEUP, code, index, black, true, bits, length);
                return Some(code);
            }
            index += 1;
        }
    } else {
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                !black,
                index <= WHITE_TERMINATING@.len(),
            decreases WHITE_TERMINATING@.len() - index,
        ))]
        while index < WHITE_TERMINATING.len() {
            let code = WHITE_TERMINATING[index];
            if code.length == length && code.bits == bits {
                #[cfg(verus_keep_ghost)]
                production_mmr_run_entry!(
                    WHITE_TERMINATING,
                    code,
                    index,
                    black,
                    false,
                    bits,
                    length
                );
                return Some(code);
            }
            index += 1;
        }
        index = 0;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                !black,
                index <= WHITE_MAKEUP@.len(),
            decreases WHITE_MAKEUP@.len() - index,
        ))]
        while index < WHITE_MAKEUP.len() {
            let code = WHITE_MAKEUP[index];
            if code.length == length && code.bits == bits {
                #[cfg(verus_keep_ghost)]
                production_mmr_run_entry!(WHITE_MAKEUP, code, index, black, false, bits, length);
                return Some(code);
            }
            index += 1;
        }
    }
    index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= COMMON_MAKEUP@.len(),
        decreases COMMON_MAKEUP@.len() - index,
    ))]
    while index < COMMON_MAKEUP.len() {
        let code = COMMON_MAKEUP[index];
        if code.length == length && code.bits == bits {
            #[cfg(verus_keep_ghost)]
            production_mmr_run_entry!(COMMON_MAKEUP, code, index, black, black, bits, length);
            return Some(code);
        }
        index += 1;
    }
    None
}
