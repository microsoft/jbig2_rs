#![cfg(test)]
#![allow(clippy::duplicated_attributes)]

mod context_state;
mod decoder_registers;
mod error_projection;
mod integer_procedures;
mod prefix_certainty;

use super::*;

const ANNEX_H_DECISIONS: [u8; 32] = [
    0x00, 0x02, 0x00, 0x51, 0x00, 0x00, 0x00, 0xc0, 0x03, 0x52, 0x87, 0x2a, 0xaa, 0xaa, 0xaa, 0xaa,
    0x82, 0xc0, 0x20, 0x00, 0xfc, 0xd7, 0x9e, 0xf6, 0xbf, 0x7f, 0xed, 0x90, 0x4f, 0x46, 0xa3, 0xbf,
];

const ANNEX_H_ENCODED: [u8; 30] = [
    0x84, 0xc7, 0x3b, 0xfc, 0xe1, 0xa1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00, 0x41, 0x0d, 0xbb, 0x86,
    0xf4, 0x31, 0x7f, 0xff, 0x88, 0xff, 0x37, 0x47, 0x1a, 0xdb, 0x6a, 0xdf, 0xff, 0xac,
];

fn next_random(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 32) as u32
}

fn assert_decoder_state_eq(left: &ArithmeticDecoder<'_>, right: &ArithmeticDecoder<'_>) {
    assert_eq!(left.byte_position, right.byte_position);
    assert_eq!(left.current_byte, right.current_byte);
    assert_eq!(left.code, right.code);
    assert_eq!(left.interval, right.interval);
    assert_eq!(left.bit_count, right.bit_count);
    assert_eq!(left.marker, right.marker);
}

fn refill_fixture(
    bytes: &[u8],
    position: usize,
    code: u32,
    bit_count: u8,
    marker: Option<ArithmeticMarker>,
) -> ArithmeticDecoder<'_> {
    ArithmeticDecoder {
        bytes,
        byte_position: position,
        current_byte: bytes[position],
        code,
        interval: MIN_INTERVAL,
        bit_count,
        marker,
    }
}

fn assert_refill_matches_numeric_oracle(
    bytes: &[u8],
    position: usize,
    code: u32,
    bit_count: u8,
    marker: Option<ArithmeticMarker>,
) {
    let mut expected = refill_fixture(bytes, position, code, bit_count, marker);
    let signal = if let Some(&next) = bytes.get(position + 1) {
        let marker_byte = bytes[position] == 0xff && next >= 0x90;
        let stuffed = bytes[position] == 0xff && !marker_byte;
        let increment = if marker_byte {
            65_280
        } else {
            u64::from(next) * if stuffed { 512 } else { 256 }
        };
        if !marker_byte {
            expected.byte_position += 1;
            expected.current_byte = next;
        }
        if let Ok(sum) = u32::try_from(u64::from(code) + increment) {
            expected.code = sum;
            expected.bit_count = if stuffed { 7 } else { 8 };
            if marker_byte {
                expected.marker = Some(ArithmeticMarker {
                    offset: position,
                    code: next,
                });
            }
            0
        } else {
            RASTER_SIGNAL_OVERFLOW
        }
    } else {
        RASTER_SIGNAL_EOF
    };
    let mut actual = refill_fixture(bytes, position, code, bit_count, marker);
    assert_eq!(actual.byte_in_signal(), signal);
    assert_decoder_state_eq(&actual, &expected);
    assert_eq!(actual.bytes, bytes);

    let mut checked = refill_fixture(bytes, position, code, bit_count, marker);
    match (signal, checked.byte_in()) {
        (0, Ok(()))
        | (
            RASTER_SIGNAL_OVERFLOW,
            Err(ArithmeticError::Malformed(MalformedError::IntegerOverflow)),
        ) => {}
        (
            RASTER_SIGNAL_EOF,
            Err(ArithmeticError::Malformed(MalformedError::UnexpectedEof {
                offset,
                needed,
                remaining,
            })),
        ) => {
            assert_eq!(offset, u64::try_from(position + 1).unwrap());
            assert_eq!(needed, 1);
            assert_eq!(remaining, 0);
        }
        (_, result) => panic!("wrong checked refill result for signal {signal}: {result:?}"),
    }
    assert_decoder_state_eq(&checked, &expected);
    assert_eq!(checked.bytes, bytes);
}

fn decision_fixture(
    bytes: &[u8],
    interval: u32,
    code: u32,
    bit_count: u8,
) -> ArithmeticDecoder<'_> {
    ArithmeticDecoder {
        bytes,
        byte_position: 0,
        current_byte: bytes[0],
        code,
        interval,
        bit_count,
        marker: None,
    }
}

fn decode_integer_with<F>(mut decode: F) -> Result<ArithmeticInteger, ArithmeticError>
where
    F: FnMut(usize) -> Result<u8, ArithmeticError>,
{
    let mut previous = 1_u16;
    let sign = integer_bit(&mut previous, &mut decode)?;

    let (bit_count, offset) = if integer_bit(&mut previous, &mut decode)? == 0 {
        (2, 0_u64)
    } else if integer_bit(&mut previous, &mut decode)? == 0 {
        (4, 4)
    } else if integer_bit(&mut previous, &mut decode)? == 0 {
        (6, 20)
    } else if integer_bit(&mut previous, &mut decode)? == 0 {
        (8, 84)
    } else if integer_bit(&mut previous, &mut decode)? == 0 {
        (12, 340)
    } else {
        (32, 4436)
    };

    let mut magnitude_bits = 0_u32;
    for _ in 0..bit_count {
        let bit = integer_bit(&mut previous, &mut decode)?;
        magnitude_bits = magnitude_bits
            .checked_mul(2)
            .and_then(|partial| partial.checked_add(u32::from(bit)))
            .ok_or(MalformedError::IntegerOverflow)?;
    }
    let magnitude = offset
        .checked_add(u64::from(magnitude_bits))
        .ok_or(MalformedError::IntegerOverflow)?;
    if sign == 1 && magnitude == 0 {
        return Ok(ArithmeticInteger::OutOfBand);
    }

    let magnitude = i64::try_from(magnitude).map_err(|_| MalformedError::IntegerOverflow)?;
    if sign == 0 {
        Ok(ArithmeticInteger::Value(magnitude))
    } else {
        Ok(ArithmeticInteger::Value(
            magnitude
                .checked_neg()
                .ok_or(MalformedError::IntegerOverflow)?,
        ))
    }
}

fn integer_bit<F>(previous: &mut u16, decode: &mut F) -> Result<u8, ArithmeticError>
where
    F: FnMut(usize) -> Result<u8, ArithmeticError>,
{
    let context_index = usize::from(*previous & 0x01ff);
    let decision = decode(context_index)?;
    let shifted = (*previous << 1) | u16::from(decision);
    *previous = if *previous < 256 {
        shifted
    } else {
        (shifted & 511) | 256
    };
    Ok(decision)
}

fn decode_iaid_with<F>(
    code_length: u8,
    context_count: usize,
    mut decode: F,
) -> Result<u32, ArithmeticError>
where
    F: FnMut(usize) -> Result<u8, ArithmeticError>,
{
    if code_length > MAX_IAID_CODE_LENGTH {
        return Err(ArithmeticError::InvalidIaidCodeLength(code_length));
    }
    let required = 1_usize
        .checked_shl(u32::from(code_length))
        .ok_or(ArithmeticError::InvalidIaidCodeLength(code_length))?;
    if context_count < required {
        return Err(ArithmeticError::InsufficientContexts {
            required,
            actual: context_count,
        });
    }

    let mut previous = 1_usize;
    for _ in 0..code_length {
        let decision = decode(previous)?;
        previous = previous
            .checked_mul(2)
            .and_then(|partial| partial.checked_add(usize::from(decision)))
            .ok_or(MalformedError::IntegerOverflow)?;
    }
    let value = previous
        .checked_sub(required)
        .ok_or(MalformedError::IntegerOverflow)?;
    u32::try_from(value).map_err(|_| ArithmeticError::Malformed(MalformedError::IntegerOverflow))
}

#[test]
fn qe_table_matches_annex_e_table_e_1() {
    let expected = [
        (0x5601, 1, 1, true),
        (0x3401, 2, 6, false),
        (0x1801, 3, 9, false),
        (0x0ac1, 4, 12, false),
        (0x0521, 5, 29, false),
        (0x0221, 38, 33, false),
        (0x5601, 7, 6, true),
        (0x5401, 8, 14, false),
        (0x4801, 9, 14, false),
        (0x3801, 10, 14, false),
        (0x3001, 11, 17, false),
        (0x2401, 12, 18, false),
        (0x1c01, 13, 20, false),
        (0x1601, 29, 21, false),
        (0x5601, 15, 14, true),
        (0x5401, 16, 14, false),
        (0x5101, 17, 15, false),
        (0x4801, 18, 16, false),
        (0x3801, 19, 17, false),
        (0x3401, 20, 18, false),
        (0x3001, 21, 19, false),
        (0x2801, 22, 19, false),
        (0x2401, 23, 20, false),
        (0x2201, 24, 21, false),
        (0x1c01, 25, 22, false),
        (0x1801, 26, 23, false),
        (0x1601, 27, 24, false),
        (0x1401, 28, 25, false),
        (0x1201, 29, 26, false),
        (0x1101, 30, 27, false),
        (0x0ac1, 31, 28, false),
        (0x09c1, 32, 29, false),
        (0x08a1, 33, 30, false),
        (0x0521, 34, 31, false),
        (0x0441, 35, 32, false),
        (0x02a1, 36, 33, false),
        (0x0221, 37, 34, false),
        (0x0141, 38, 35, false),
        (0x0111, 39, 36, false),
        (0x0085, 40, 37, false),
        (0x0049, 41, 38, false),
        (0x0025, 42, 39, false),
        (0x0015, 43, 40, false),
        (0x0009, 44, 41, false),
        (0x0005, 45, 42, false),
        (0x0001, 45, 43, false),
        (0x5601, 46, 46, false),
    ];
    let actual =
        QE_STATES.map(|state| (state.qe, state.next_mps, state.next_lps, state.switch_mps));
    assert_eq!(actual, expected);
}

fn normalization_numeric_oracle<'a>(
    initial: &ArithmeticDecoder<'a>,
) -> (u8, ArithmeticDecoder<'a>) {
    let mut expected = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    expected.interval = initial.interval;
    let shifts = initial.interval.leading_zeros().saturating_sub(16);
    for shift in 0..shifts {
        if expected.bit_count == 0 {
            let Some(&next) = expected.bytes.get(expected.byte_position + 1) else {
                return (RASTER_SIGNAL_EOF, expected);
            };
            let marker = expected.current_byte == 255 && next >= 144;
            let stuffed = expected.current_byte == 255 && !marker;
            let increment = if marker {
                65_280
            } else {
                u64::from(next) * if stuffed { 512 } else { 256 }
            };
            if !marker {
                expected.byte_position += 1;
                expected.current_byte = next;
            }
            let sum = u64::from(expected.code) + increment;
            if sum > u64::from(u32::MAX) {
                return (RASTER_SIGNAL_OVERFLOW, expected);
            }
            expected.code = u32::try_from(sum).unwrap();
            expected.bit_count = if stuffed { 7 } else { 8 };
            if marker {
                expected.marker = Some(ArithmeticMarker {
                    offset: expected.byte_position,
                    code: next,
                });
            }
        }
        expected.interval = initial.interval << (shift + 1);
        let product = u64::from(expected.code) * 2;
        if product > u64::from(u32::MAX) {
            return (RASTER_SIGNAL_OVERFLOW, expected);
        }
        expected.code = u32::try_from(product).unwrap();
        expected.bit_count -= 1;
    }
    (0, expected)
}

fn decision_numeric_oracle<'a>(
    initial: &ArithmeticDecoder<'a>,
    context: ArithmeticContext,
) -> (u8, ArithmeticDecoder<'a>, ArithmeticContext) {
    let row = QE_STATES[usize::from(context.state_index())];
    let qe = u32::from(row.qe);
    let reduced = initial.interval - qe;
    let lower = u64::from(initial.code) / 65_536 < u64::from(qe);
    let fast = !lower && reduced >= MIN_INTERVAL;
    let mps_route = (lower && reduced < qe) || (!lower && reduced >= qe);
    let decision = if fast || mps_route {
        context.mps()
    } else {
        1 - context.mps()
    };
    let next_index = if fast {
        context.state_index()
    } else if mps_route {
        row.next_mps
    } else {
        row.next_lps
    };
    let next_mps = if !fast && !mps_route && row.switch_mps {
        decision
    } else {
        context.mps()
    };
    let next_context = ArithmeticContext::from_state(next_index, next_mps).unwrap();
    let code = u64::from(initial.code) - if lower { 0 } else { u64::from(qe) * 65_536 };
    let mut next = refill_fixture(
        initial.bytes,
        initial.byte_position,
        u32::try_from(code).unwrap(),
        initial.bit_count,
        initial.marker,
    );
    next.interval = if lower { qe } else { reduced };
    if fast {
        (decision, next, next_context)
    } else {
        let (signal, normalized) = normalization_numeric_oracle(&next);
        (
            if signal == 0 { decision } else { signal },
            normalized,
            next_context,
        )
    }
}

fn assert_decision_matches_numeric_oracle(
    mut initial: ArithmeticDecoder<'_>,
    context: ArithmeticContext,
) -> u8 {
    let bytes = initial.bytes;
    let (signal, expected, expected_context) = decision_numeric_oracle(&initial, context);
    let mut actual = refill_fixture(
        bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    actual.interval = initial.interval;
    let mut actual_context = context;
    assert_eq!(actual.decode_signal(&mut actual_context), signal);
    assert_eq!(actual_context, expected_context);
    assert_decoder_state_eq(&actual, &expected);
    assert_eq!(actual.bytes, bytes);
    assert_eq!(actual.bytes.as_ptr(), bytes.as_ptr());

    let mut checked_context = context;
    let expected_result = if signal <= 1 {
        Ok(signal)
    } else {
        Err(ArithmeticError::Malformed(if signal == RASTER_SIGNAL_EOF {
            MalformedError::UnexpectedEof {
                offset: u64::try_from(expected.byte_position).unwrap() + 1,
                needed: 1,
                remaining: 0,
            }
        } else {
            MalformedError::IntegerOverflow
        }))
    };
    assert_eq!(initial.decode(&mut checked_context), expected_result);
    assert_eq!(checked_context, expected_context);
    assert_decoder_state_eq(&initial, &expected);
    assert_eq!(initial.bytes.as_ptr(), bytes.as_ptr());
    signal
}

fn patterned_integer_contexts() -> [ArithmeticContext; INTEGER_CONTEXT_COUNT] {
    let mut contexts = [ArithmeticContext::new(); INTEGER_CONTEXT_COUNT];
    for (index, context) in contexts.iter_mut().enumerate() {
        *context = ArithmeticContext::from_state(
            u8::try_from(index % 47).unwrap(),
            u8::try_from(index % 2).unwrap(),
        )
        .unwrap();
    }
    contexts
}

fn assert_integer_bit_matches_numeric_oracle(
    mut decoder: ArithmeticDecoder<'_>,
    prior: u16,
    mut contexts: [ArithmeticContext; INTEGER_CONTEXT_COUNT],
) -> u8 {
    let index = usize::from(prior);
    let before = contexts;
    let (signal, expected_decoder, expected_context) =
        decision_numeric_oracle(&decoder, before[index]);
    let mut expected_contexts = before;
    expected_contexts[index] = expected_context;
    let mut previous = prior;
    let result = decoder.decode_integer_bit(&mut previous, &mut contexts);
    if signal <= 1 {
        assert_eq!(result, Ok(signal));
        let wide = u32::from(prior) * 2 + u32::from(signal);
        let expected = if prior < 256 {
            u16::try_from(wide).unwrap()
        } else {
            256 + u16::try_from(wide % 256).unwrap()
        };
        assert_eq!(previous, expected);
    } else {
        let expected = if signal == RASTER_SIGNAL_EOF {
            eof_numeric_error(expected_decoder.byte_position + 1, 1, 0)
        } else {
            assert_eq!(signal, RASTER_SIGNAL_OVERFLOW);
            ArithmeticError::Malformed(MalformedError::IntegerOverflow)
        };
        assert_eq!(result, Err(expected));
        assert_eq!(previous, prior);
    }
    assert_eq!(contexts, expected_contexts);
    assert_decoder_state_eq(&decoder, &expected_decoder);
    assert_eq!(decoder.bytes.as_ptr(), expected_decoder.bytes.as_ptr());
    signal
}

fn integer_numeric_bit(
    decoder: &mut ArithmeticDecoder<'_>,
    contexts: &mut [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    previous: &mut u16,
) -> Result<u8, ArithmeticError> {
    let index = usize::from(*previous);
    let (signal, next, context) = decision_numeric_oracle(decoder, contexts[index]);
    *decoder = next;
    contexts[index] = context;
    if signal > 1 {
        return Err(if signal == RASTER_SIGNAL_EOF {
            eof_numeric_error(decoder.byte_position + 1, 1, 0)
        } else {
            assert_eq!(signal, RASTER_SIGNAL_OVERFLOW);
            ArithmeticError::Malformed(MalformedError::IntegerOverflow)
        });
    }
    let next = *previous * 2 + u16::from(signal);
    *previous = if *previous < 256 {
        next
    } else {
        256 + next % 256
    };
    Ok(signal)
}

fn integer_numeric_oracle<'a>(
    initial: &ArithmeticDecoder<'a>,
    contexts: &[ArithmeticContext; INTEGER_CONTEXT_COUNT],
) -> (
    Result<ArithmeticInteger, ArithmeticError>,
    ArithmeticDecoder<'a>,
    [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    Option<usize>,
) {
    let mut decoder = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    decoder.interval = initial.interval;
    let mut bank = *contexts;
    let mut previous = 1;
    let mut selected_leaf = None;
    let result = (|| {
        let sign = integer_numeric_bit(&mut decoder, &mut bank, &mut previous)?;
        let mut ones = 0;
        while ones < 5 {
            if integer_numeric_bit(&mut decoder, &mut bank, &mut previous)? == 0 {
                break;
            }
            ones += 1;
        }
        selected_leaf = Some(ones);
        let (count, offset) = [(2, 0), (4, 4), (6, 20), (8, 84), (12, 340), (32, 4436)][ones];
        let mut value = 0_u64;
        for position in (0..count).rev() {
            let bit = integer_numeric_bit(&mut decoder, &mut bank, &mut previous)?;
            value |= u64::from(bit) << position;
        }
        let magnitude = i64::try_from(value + offset).unwrap();
        Ok(if sign == 1 && magnitude == 0 {
            ArithmeticInteger::OutOfBand
        } else {
            ArithmeticInteger::Value(if sign == 0 { magnitude } else { -magnitude })
        })
    })();
    (result, decoder, bank, selected_leaf)
}

fn integer_payload_numeric<'a>(
    initial: &ArithmeticDecoder<'a>,
    contexts: &[ArithmeticContext; INTEGER_CONTEXT_COUNT],
    mut previous: u16,
    bit_count: u8,
) -> (
    Result<u32, ArithmeticError>,
    ArithmeticDecoder<'a>,
    [ArithmeticContext; INTEGER_CONTEXT_COUNT],
    u16,
) {
    let mut decoder = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    decoder.interval = initial.interval;
    let mut bank = *contexts;
    let result = (|| {
        let mut value = 0_u64;
        for position in (0..bit_count).rev() {
            let bit = integer_numeric_bit(&mut decoder, &mut bank, &mut previous)?;
            value |= u64::from(bit) << position;
        }
        Ok(u32::try_from(value).unwrap())
    })();
    (result, decoder, bank, previous)
}

fn iaid_numeric_oracle<'a>(
    initial: &ArithmeticDecoder<'a>,
    contexts: &[ArithmeticContext],
    code_length: u8,
) -> (
    Result<u32, ArithmeticError>,
    ArithmeticDecoder<'a>,
    Vec<ArithmeticContext>,
) {
    let mut decoder = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    decoder.interval = initial.interval;
    let mut bank = contexts.to_vec();
    let mut value = 0_u64;
    for used in 0..code_length {
        let index = usize::try_from((1_u64 << used) | value).unwrap();
        assert!(index < bank.len());
        let (signal, next, context) = decision_numeric_oracle(&decoder, bank[index]);
        decoder = next;
        bank[index] = context;
        if signal > 1 {
            let error = if signal == RASTER_SIGNAL_EOF {
                eof_numeric_error(decoder.byte_position + 1, 1, 0)
            } else {
                assert_eq!(signal, RASTER_SIGNAL_OVERFLOW);
                ArithmeticError::Malformed(MalformedError::IntegerOverflow)
            };
            return (Err(error), decoder, bank);
        }
        value = value * 2 + u64::from(signal);
    }
    (Ok(u32::try_from(value).unwrap()), decoder, bank)
}

fn assert_iaid_matches_numeric_and_prior_procedure(
    mut initial: ArithmeticDecoder<'_>,
    code_length: u8,
    contexts: &[ArithmeticContext],
) -> Result<u32, ArithmeticError> {
    let (expected, decoder, bank) = iaid_numeric_oracle(&initial, contexts, code_length);
    let mut reference = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    reference.interval = initial.interval;
    let mut reference_bank = contexts.to_vec();
    let prior = decode_iaid_with(code_length, reference_bank.len(), |index| {
        reference.decode(&mut reference_bank[index])
    });
    assert_eq!(prior, expected);
    assert_decoder_state_eq(&reference, &decoder);
    assert_eq!(reference_bank, bank);
    let mut actual_bank = contexts.to_vec();
    let actual = initial.decode_iaid(code_length, &mut actual_bank);
    assert_eq!(actual, expected);
    assert_decoder_state_eq(&initial, &decoder);
    assert_eq!(initial.bytes.as_ptr(), decoder.bytes.as_ptr());
    assert_eq!(actual_bank, bank);
    actual
}

fn eof_numeric_error(offset: usize, needed: usize, remaining: usize) -> ArithmeticError {
    if [offset, needed, remaining]
        .into_iter()
        .any(|value| value as u128 > u128::from(u64::MAX))
    {
        ArithmeticError::Malformed(MalformedError::IntegerOverflow)
    } else {
        ArithmeticError::Malformed(MalformedError::UnexpectedEof {
            offset: offset as u64,
            needed: needed as u64,
            remaining: remaining as u64,
        })
    }
}

fn assert_normalization_matches_numeric_oracle(initial: ArithmeticDecoder<'_>) {
    let (signal, expected) = normalization_numeric_oracle(&initial);
    let mut actual = refill_fixture(
        initial.bytes,
        initial.byte_position,
        initial.code,
        initial.bit_count,
        initial.marker,
    );
    actual.interval = initial.interval;
    assert_eq!(actual.renormalize_signal(), signal);
    assert_decoder_state_eq(&actual, &expected);
    assert!(std::ptr::eq(actual.bytes.as_ptr(), initial.bytes.as_ptr()));

    let mut checked = initial;
    match (signal, checked.renormalize()) {
        (0, Ok(()))
        | (
            RASTER_SIGNAL_OVERFLOW,
            Err(ArithmeticError::Malformed(MalformedError::IntegerOverflow)),
        ) => {}
        (
            RASTER_SIGNAL_EOF,
            Err(ArithmeticError::Malformed(MalformedError::UnexpectedEof {
                offset,
                needed,
                remaining,
            })),
        ) => {
            assert_eq!(offset, u64::try_from(expected.byte_position + 1).unwrap());
            assert_eq!((needed, remaining), (1, 0));
        }
        (_, result) => panic!("wrong normalization result for signal {signal}: {result:?}"),
    }
    assert_decoder_state_eq(&checked, &expected);
    assert!(std::ptr::eq(
        checked.bytes.as_ptr(),
        expected.bytes.as_ptr()
    ));
}

fn scripted_integer(bits: &[u8]) -> ArithmeticInteger {
    let mut offset = 0;
    let result = decode_integer_with(|_| {
        let bit = bits
            .get(offset)
            .copied()
            .ok_or_else(|| unexpected_eof(offset, 1, 0))?;
        offset += 1;
        Ok(bit)
    })
    .expect("complete scripted integer");
    assert_eq!(offset, bits.len());
    result
}
