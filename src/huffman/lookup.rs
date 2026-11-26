//! Prefix lookahead table builders for compact and wide Huffman tables.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    try_reserve, HuffmanError, HuffmanTableError, MalformedError, WideCode, MAX_PREFIX_LEN,
    PREFIX_LOOKUP_BITS, PREFIX_LOOKUP_SIZE, SHORT_PREFIX_MAX,
};

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(lookup) =>
            lookup@.len() == 256
                && forall|prefix: int| 0 <= prefix < 256 ==>
                    #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                        WideCode { limbs: [short_first_code@[8] as u64, 0, 0, 0] },
                        short_length_count@[8],
                        short_first_line@[8],
                        ordered_lines@,
                        prefix as u8,
                        short_length_count@[8],
                    ),
        Err(_) => true,
    },
))]
#[allow(clippy::cast_possible_truncation)]
pub(super) fn build_prefix_lookup_short(
    short_length_count: &[u32; SHORT_PREFIX_MAX + 1],
    short_first_code: &[u32; SHORT_PREFIX_MAX + 1],
    short_first_line: &[u32; SHORT_PREFIX_MAX + 1],
    ordered_lines: &[u32],
) -> Result<Vec<u16>, HuffmanError> {
    let mut lookup = Vec::new();
    try_reserve::<u16>(&mut lookup, PREFIX_LOOKUP_SIZE)?;
    lookup.resize(PREFIX_LOOKUP_SIZE, 0);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(PREFIX_LOOKUP_SIZE == 256) by (bit_vector);
        assert(lookup@.len() == 256);
    }
    let count = short_length_count[8];
    let base = short_first_code[8];
    let first_line = short_first_line[8];
    let mut distance = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            lookup@.len() == 256,
            distance <= count,
            count == short_length_count@[8],
            base == short_first_code@[8],
            first_line == short_first_line@[8],
            forall|prefix: int| 0 <= prefix < 256 ==>
                #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                    WideCode { limbs: [base as u64, 0, 0, 0] },
                    count,
                    first_line,
                    ordered_lines@,
                    prefix as u8,
                    distance,
                ),
        decreases count - distance,
    ))]
    while distance < count {
        let code = base
            .checked_add(distance)
            .ok_or(HuffmanError::InvalidTable(
                HuffmanTableError::Oversubscribed,
            ))?;
        if code > u32::from(u8::MAX) || distance > u32::from(u8::MAX) {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::Oversubscribed,
            ));
        }
        let ordered_index = first_line
            .checked_add(distance)
            .ok_or(MalformedError::IntegerOverflow)?;
        let line_index = *ordered_lines
            .get(ordered_index as usize)
            .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
        if line_index >= u32::from(u16::MAX) {
            #[cfg(verus_keep_ghost)]
            proof! {
                huffman_lookup_spec::advance_slot(
                    WideCode { limbs: [base as u64, 0, 0, 0] },
                    count,
                    first_line,
                    ordered_lines@,
                    distance,
                    code as u8,
                    0u16,
                );
            }
            distance += 1;
            continue;
        }
        let encoded_line = (line_index + 1) as u16;
        let slot_index = code as usize;
        if lookup[slot_index] != 0 {
            return Err(HuffmanError::InvalidTable(
                HuffmanTableError::Oversubscribed,
            ));
        }
        lookup[slot_index] = encoded_line;
        #[cfg(verus_keep_ghost)]
        proof! {
            huffman_lookup_spec::advance_slot(
                WideCode { limbs: [base as u64, 0, 0, 0] },
                count,
                first_line,
                ordered_lines@,
                distance,
                code as u8,
                encoded_line,
            );
        }
        distance += 1;
    }
    Ok(lookup)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(lookup) =>
            lookup@.len() == 256
                && (huffman_lookup_spec::fast_shape(length_count@) ==>
                    forall|prefix: int| 0 <= prefix < 256 ==>
                        #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                            first_code@[8], length_count@[8], first_line@[8], ordered_lines@,
                            prefix as u8, if maximum_length >= 8 { length_count@[8] } else { 0u32 },
                        )),
        Err(_) => true,
    },
))]
// Loop contracts and ghost proofs account for the additional source lines.
#[allow(
    clippy::cast_possible_truncation,
    clippy::redundant_closure,
    clippy::too_many_lines
)]
pub(super) fn build_prefix_lookup(
    length_count: &[u32; MAX_PREFIX_LEN + 1],
    first_code: &[WideCode; MAX_PREFIX_LEN + 1],
    first_line: &[u32; MAX_PREFIX_LEN + 1],
    ordered_lines: &[u32],
    maximum_length: usize,
) -> Result<Vec<u16>, HuffmanError> {
    let mut lookup = Vec::new();
    try_reserve::<u16>(&mut lookup, PREFIX_LOOKUP_SIZE)?;
    lookup.resize(PREFIX_LOOKUP_SIZE, 0);
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(PREFIX_LOOKUP_SIZE == 256) by (bit_vector);
        assert(lookup@.len() == 256);
        assert forall|prefix: int| 0 <= prefix < 256 implies lookup@[prefix] == 0 by {
        }
    }
    let lookup_length = maximum_length.min(usize::from(PREFIX_LOOKUP_BITS));
    let mut length = 1_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            lookup@.len() == PREFIX_LOOKUP_SIZE,
            lookup@.len() == 256,
            lookup_length <= PREFIX_LOOKUP_BITS as usize,
            1 <= length,
            length <= lookup_length + 1,
            huffman_lookup_spec::fast_shape(length_count@) ==>
                forall|prefix: int| 0 <= prefix < 256 ==>
                    #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                        first_code@[8], length_count@[8], first_line@[8], ordered_lines@,
                        prefix as u8, if length <= 8 { 0u32 } else { length_count@[8] },
                    ),
        decreases lookup_length + 1 - length,
    ))]
    while length <= lookup_length {
        let count = length_count[length];
        let mut distance = 0_u32;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                lookup@.len() == PREFIX_LOOKUP_SIZE,
                lookup@.len() == 256,
                lookup_length <= PREFIX_LOOKUP_BITS as usize,
                1 <= length,
                length <= lookup_length,
                distance <= count,
                count == length_count@[length as int],
                huffman_lookup_spec::fast_shape(length_count@) ==>
                    (length == 8 || count == 0),
                huffman_lookup_spec::fast_shape(length_count@) ==>
                    forall|prefix: int| 0 <= prefix < 256 ==>
                        #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                            first_code@[8], length_count@[8], first_line@[8], ordered_lines@,
                            prefix as u8, distance,
                        ),
            decreases count - distance,
        ))]
        while distance < count {
            let code = first_code[length]
                .checked_add_small(distance)
                .ok_or(HuffmanTableError::Oversubscribed)
                .map_err(|error| HuffmanError::InvalidTable(error))?;
            if code.limbs[1] != 0
                || code.limbs[2] != 0
                || code.limbs[3] != 0
                || code.limbs[0] > u64::from(u8::MAX)
                || distance > u32::from(u8::MAX)
            {
                return Err(HuffmanError::InvalidTable(
                    HuffmanTableError::Oversubscribed,
                ));
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                if huffman_lookup_spec::fast_shape(length_count@) {
                    huffman_lookup_spec::small_code_value(
                        first_code@[8], distance, code,
                    );
                }
            }
            let suffix_bits = usize::from(PREFIX_LOOKUP_BITS) - length;
            let repeats = 1_usize << suffix_bits;
            let code = usize::try_from(code.limbs[0])
                .map_err(|_| HuffmanError::InvalidTable(HuffmanTableError::Oversubscribed))?;
            let first_prefix = code << suffix_bits;
            let ordered_index = first_line[length]
                .checked_add(distance)
                .ok_or(MalformedError::IntegerOverflow)?;
            let line_index = *ordered_lines
                .get(ordered_index as usize)
                .ok_or(HuffmanError::InvalidTable(HuffmanTableError::InvalidCode))?;
            if line_index >= u32::from(u16::MAX) {
                #[cfg(verus_keep_ghost)]
                proof! {
                    if huffman_lookup_spec::fast_shape(length_count@) {
                        assert(line_index as int + 1 > u16::MAX);
                        huffman_lookup_spec::advance_slot(
                            first_code@[8], count, first_line@[8], ordered_lines@,
                            distance, code as u8, 0u16,
                        );
                    }
                }
                distance += 1;
                continue;
            }
            let encoded_line = (line_index + 1) as u16;
            #[cfg(verus_keep_ghost)]
            proof! {
                if huffman_lookup_spec::fast_shape(length_count@) {
                    assert(length == 8);
                    assert(suffix_bits == 0);
                    assert((1usize << 0usize) == 1usize) by (bit_vector);
                    assert((code << 0usize) == code) by (bit_vector);
                    assert(repeats == 1);
                    assert(first_prefix == code);
                }
            }
            let mut suffix = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    lookup@.len() == PREFIX_LOOKUP_SIZE,
                    lookup_length <= PREFIX_LOOKUP_BITS as usize,
                    1 <= length,
                    length <= lookup_length,
                    suffix <= repeats,
                    lookup@.len() == 256,
                    distance < count,
                    count == length_count@[length as int],
                    code <= 255,
                    ordered_index as int == first_line@[length as int] as int + distance as int,
                    ordered_index < ordered_lines@.len(),
                    line_index == ordered_lines@[ordered_index as int],
                    encoded_line as int == line_index as int + 1,
                    huffman_lookup_spec::fast_shape(length_count@) ==>
                        (length == 8 && repeats == 1 && first_prefix == code),
                    huffman_lookup_spec::fast_shape(length_count@) ==> (
                        first_code@[8].limbs@[1] == 0
                        && first_code@[8].limbs@[2] == 0
                        && first_code@[8].limbs@[3] == 0
                        && first_code@[8].limbs@[0] as int + distance as int == code as int
                    ),
                    huffman_lookup_spec::fast_shape(length_count@) ==>
                        forall|prefix: int| 0 <= prefix < 256 ==>
                            #[trigger] lookup@[prefix] == huffman_lookup_spec::fast_slot(
                                first_code@[8], length_count@[8], first_line@[8], ordered_lines@,
                                prefix as u8,
                                if suffix == 0 { distance } else { (distance + 1) as u32 },
                            ),
                decreases repeats - suffix,
            ))]
            while suffix < repeats {
                let index = first_prefix
                    .checked_add(suffix)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let slot = lookup.get_mut(index).ok_or(HuffmanError::InvalidTable(
                    HuffmanTableError::Oversubscribed,
                ))?;
                if *slot != 0 {
                    return Err(HuffmanError::InvalidTable(
                        HuffmanTableError::Oversubscribed,
                    ));
                }
                *slot = encoded_line;
                #[cfg(verus_keep_ghost)]
                proof! {
                    if huffman_lookup_spec::fast_shape(length_count@) {
                        huffman_lookup_spec::advance_slot(
                            first_code@[8], count, first_line@[8], ordered_lines@,
                            distance, code as u8, encoded_line,
                        );
                    }
                }
                suffix += 1;
            }
            distance += 1;
        }
        length += 1;
    }
    Ok(lookup)
}
