//! Validation implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    size_of, BitReader, DecodeError, HuffmanError, HuffmanLine, HuffmanLines, HuffmanTable,
    HuffmanTableError, LimitExceeded, LimitKind, Limits, LineKind, MalformedError,
    PREFIX_LOOKUP_SIZE,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        huffman_spec::reader_valid(old(reader)),
        count <= 8,
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        huffman_spec::reader_valid(final(reader)),
        match out {
            Ok(value) =>
                final(reader).bit_position == old(reader).bit_position + count as usize
                && value as nat == crate::spec::readers::bits_value(
                    old(reader).bytes@,
                    old(reader).bit_position as nat,
                    count as nat,
                ),
            Err(_) => final(reader).bit_position == old(reader).bit_position,
        },
))]
#[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
pub(super) fn read_u8_bits(reader: &mut BitReader<'_>, count: u8) -> Result<u8, HuffmanError> {
    #[cfg(verus_keep_ghost)]
    let start = reader.bit_position;
    let value = match reader.read_bits(count) {
        Ok(value) => value,
        Err(error) => {
            return Err(HuffmanError::Decode(DecodeError::Malformed(error)));
        }
    };
    #[cfg(verus_keep_ghost)]
    production_huffman_u8_bits!(reader, start, count, value);
    if value > u8::MAX as u32 {
        Err(HuffmanError::InvalidTable(
            HuffmanTableError::InvalidRangePartition,
        ))
    } else {
        Ok(value as u8)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(remaining) =>
            payload_bytes <= usize::MAX / 8
            && consumed_bits <= payload_bytes * 8
            && remaining == payload_bytes * 8 - consumed_bits,
        None =>
            payload_bytes > usize::MAX / 8
            || consumed_bits > payload_bytes * 8,
    },
))]
pub(super) fn padding_bit_count(payload_bytes: usize, consumed_bits: usize) -> Option<usize> {
    match payload_bytes.checked_mul(8) {
        Some(bits) => bits.checked_sub(consumed_bits),
        None => None,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.0 == (flags & 1 != 0),
        out.1 == ((((flags >> 1) & 7) as u16 + 1) as u8),
        out.2 == ((((flags >> 4) & 7) as u16 + 1) as u8),
))]
#[allow(clippy::cast_lossless, clippy::cast_possible_truncation)]
pub(super) fn encoded_table_fields(flags: u8) -> (bool, u8, u8) {
    let prefix_bits = (((flags >> 1) & 7) as u16 + 1) as u8;
    let range_bits = (((flags >> 4) & 7) as u16 + 1) as u8;
    (flags & 1 != 0, prefix_bits, range_bits)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> final(lines)@ == old(lines)@.push(line),
))]
pub(super) fn push_parsed_line(
    lines: &mut Vec<HuffmanLine>,
    line: HuffmanLine,
    limits: &Limits,
) -> Result<(), HuffmanError> {
    let new_len = lines
        .len()
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    check_table_memory(new_len, false, limits)?;
    let requested_bytes = allocation_size::<HuffmanLine>(new_len)?;
    if lines.try_reserve(1).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    lines.push(line);
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures final(values)@ == old(values)@,
))]
pub(super) fn try_reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), HuffmanError> {
    let requested_bytes = allocation_size::<T>(count)?;
    if values.try_reserve(count).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) =>
            bytes as nat == count as nat * layout::size_of::<T>(),
        Err(_) =>
            count as nat * layout::size_of::<T>() > usize::MAX
            || count as nat * layout::size_of::<T>() > u64::MAX,
    },
))]
pub(super) fn allocation_size<T>(count: usize) -> Result<u64, MalformedError> {
    let Some(bytes) = count.checked_mul(size_of::<T>()) else {
        return Err(MalformedError::IntegerOverflow);
    };
    let Ok(bytes) = u64::try_from(bytes) else {
        return Err(MalformedError::IntegerOverflow);
    };
    Ok(bytes)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn table_memory(line_count: usize, prefix_lookup: bool) -> Result<u64, MalformedError> {
    let Ok(fixed) = u64::try_from(size_of::<HuffmanTable>()) else {
        return Err(MalformedError::IntegerOverflow);
    };
    let per_line = size_of::<HuffmanLine>()
        .checked_add(size_of::<u32>())
        .ok_or(MalformedError::IntegerOverflow)?;
    let variable_count = line_count
        .checked_mul(per_line)
        .ok_or(MalformedError::IntegerOverflow)?;
    let fixed = if prefix_lookup {
        fixed
            .checked_add(allocation_size::<u16>(PREFIX_LOOKUP_SIZE)?)
            .ok_or(MalformedError::IntegerOverflow)?
    } else {
        fixed
    };
    fixed
        .checked_add(allocation_size::<u8>(variable_count)?)
        .ok_or(MalformedError::IntegerOverflow)
}

#[allow(clippy::cast_lossless)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out is Ok ==> line_count <= u32::MAX,
))]
pub(super) fn check_table_memory(
    line_count: usize,
    prefix_lookup: bool,
    limits: &Limits,
) -> Result<(), HuffmanError> {
    let line_count_u64 = line_count as u64;
    if line_count_exceeds_symbol_limit(line_count, limits.max_symbols) {
        return Err(LimitExceeded {
            kind: LimitKind::Symbols,
            limit: limits.max_symbols as u64,
            actual: line_count_u64,
        }
        .into());
    }
    let actual = table_memory(line_count, prefix_lookup)?;
    if actual > limits.max_decoder_managed_memory_bytes {
        return Err(LimitExceeded {
            kind: LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual,
        }
        .into());
    }
    if !line_count_fits_u32(line_count) {
        return Err(HuffmanError::InvalidTable(
            HuffmanTableError::Oversubscribed,
        ));
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (line_count as nat > maximum as nat),
))]
#[allow(clippy::cast_lossless)]
pub(super) fn line_count_exceeds_symbol_limit(line_count: usize, maximum: u32) -> bool {
    line_count as u64 > maximum as u64
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (line_count as nat <= u32::MAX as nat),
))]
#[allow(clippy::cast_lossless, clippy::checked_conversions)]
pub(super) fn line_count_fits_u32(line_count: usize) -> bool {
    line_count as u64 <= u32::MAX as u64
}

#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
#[allow(clippy::cast_lossless, clippy::manual_let_else)]
pub(super) fn validate_lines(lines: HuffmanLines<'_>) -> Result<(), HuffmanError> {
    let line_count = lines.len();
    if line_count == 0 {
        return Err(HuffmanError::InvalidTable(HuffmanTableError::NoCodes));
    }

    let mut previous_normal_end: Option<i64> = None;
    let mut last_normal_start: Option<i64> = None;
    let mut first_normal_start: Option<i64> = None;
    let mut lower: Option<i64> = None;
    let mut upper: Option<i64> = None;
    let mut oob_seen = false;
    let mut special_seen = false;
    let mut active_lower_seen = false;

    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= line_count,
        decreases line_count - index,
    ))]
    while index < line_count {
        let line = lines.line(index).ok_or(HuffmanError::InvalidTable(
            HuffmanTableError::InvalidLineOrder,
        ))?;
        if line.range_len > 32 {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidRangeLength(line.range_len),
            ));
        }
        match line.kind {
            LineKind::Normal => {
                if special_seen {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidLineOrder,
                    ));
                }
                let width_minus_one = if line.range_len == 32 {
                    u32::MAX as i64
                } else {
                    let width = 1_i64 << line.range_len;
                    match width.checked_sub(1) {
                        Some(value) => value,
                        None => {
                            return Err(HuffmanError::Decode(DecodeError::Malformed(
                                MalformedError::IntegerOverflow,
                            )));
                        }
                    }
                };
                let end = match line.range_low.checked_add(width_minus_one) {
                    Some(end) => end,
                    None => {
                        return Err(HuffmanError::Decode(DecodeError::Malformed(
                            MalformedError::IntegerOverflow,
                        )));
                    }
                };
                if line.range_low < i32::MIN as i64
                    || line.range_low > i32::MAX as i64
                    || end < i32::MIN as i64
                    || end > i32::MAX as i64
                {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidRangePartition,
                    ));
                }
                if let Some(previous) = previous_normal_end {
                    if previous.checked_add(1) != Some(line.range_low) {
                        return Err(HuffmanError::InvalidTable(
                            HuffmanTableError::InvalidRangePartition,
                        ));
                    }
                } else {
                    first_normal_start = Some(line.range_low);
                }
                previous_normal_end = Some(end);
                last_normal_start = Some(line.range_low);
            }
            LineKind::Lower => {
                special_seen = true;
                if lower.is_some() {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::DuplicateSpecialLine,
                    ));
                }
                lower = Some(line.range_low);
                if upper.is_some() || oob_seen || line.range_len != 32 {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidLineOrder,
                    ));
                }
                active_lower_seen = line.prefix_len != 0;
            }
            LineKind::Upper => {
                special_seen = true;
                if upper.is_some() {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::DuplicateSpecialLine,
                    ));
                }
                upper = Some(line.range_low);
                if oob_seen || line.range_len != 32 {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidLineOrder,
                    ));
                }
            }
            LineKind::OutOfBand => {
                special_seen = true;
                if oob_seen {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::DuplicateSpecialLine,
                    ));
                }
                if line.range_len != 0 {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::InvalidSpecialLine,
                    ));
                }
                oob_seen = true;
            }
        }
        index += 1;
    }

    let first = match first_normal_start {
        Some(first) => first,
        None => {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidRangePartition,
            ));
        }
    };
    let last = match previous_normal_end {
        Some(last) => last,
        None => {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidRangePartition,
            ));
        }
    };
    if let Some(base) = lower {
        if first.checked_sub(1) != Some(base) || (base < i32::MIN as i64 && active_lower_seen) {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidSpecialLine,
            ));
        }
    }
    if let Some(base) = upper {
        let valid = match last_normal_start {
            Some(start) => upper_base_within_final_range(start, last, base),
            None => false,
        };
        if !valid {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::InvalidSpecialLine,
            ));
        }
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (
        start < base
        && base as int <= last as int + 1
        && i32::MIN <= base <= i32::MAX
    ),
))]
pub(super) fn upper_base_within_final_range(start: i64, last: i64, base: i64) -> bool {
    start < base
        && i128::from(base) <= i128::from(last) + 1
        && i64::from(i32::MIN) <= base
        && base <= i64::from(i32::MAX)
}
