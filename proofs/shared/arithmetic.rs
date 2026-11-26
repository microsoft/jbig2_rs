use vstd::prelude::*;

verus! {

pub(super) proof fn qe_transition_valid(index: int)
    requires
        0 <= index < 47,
    ensures
        super::QE_STATES@[index].next_mps < 47,
        super::QE_STATES@[index].next_lps < 47,
        super::QE_STATES@[index].qe > 0,
        super::QE_STATES@[index].qe <= 0x5601,
{
    assert(super::QE_STATES@.len() == 47);
    if index == 0 {} else if index == 1 {} else if index == 2 {}
    else if index == 3 {} else if index == 4 {} else if index == 5 {}
    else if index == 6 {} else if index == 7 {} else if index == 8 {}
    else if index == 9 {} else if index == 10 {} else if index == 11 {}
    else if index == 12 {} else if index == 13 {} else if index == 14 {}
    else if index == 15 {} else if index == 16 {} else if index == 17 {}
    else if index == 18 {} else if index == 19 {} else if index == 20 {}
    else if index == 21 {} else if index == 22 {} else if index == 23 {}
    else if index == 24 {} else if index == 25 {} else if index == 26 {}
    else if index == 27 {} else if index == 28 {} else if index == 29 {}
    else if index == 30 {} else if index == 31 {} else if index == 32 {}
    else if index == 33 {} else if index == 34 {} else if index == 35 {}
    else if index == 36 {} else if index == 37 {} else if index == 38 {}
    else if index == 39 {} else if index == 40 {} else if index == 41 {}
    else if index == 42 {} else if index == 43 {} else if index == 44 {}
    else if index == 45 {} else {}
}

pub(crate) proof fn packed_context_bounds(context: super::ArithmeticContext)
    requires
        super::arithmetic_spec::context_valid(context),
    ensures
        super::arithmetic_spec::context_index(context) < 47,
        super::arithmetic_spec::context_mps(context) <= 1,
        context.packed == ((super::arithmetic_spec::context_index(context) * 2)
            + super::arithmetic_spec::context_mps(context)),
{
}

pub(super) proof fn packed_tables_match(packed: u8)
    requires
        packed < 94,
    ensures
        super::QE_PACKED_VALS@[packed as int] == super::QE_STATES@[(packed / 2) as int].qe,
        super::NEXT_MPS_PACKED@[packed as int] < 94,
        (super::NEXT_MPS_PACKED@[packed as int] / 2)
            == super::QE_STATES@[(packed / 2) as int].next_mps,
        (super::NEXT_MPS_PACKED@[packed as int] % 2) == (packed % 2),
        super::QE_STATES@[(packed / 2) as int].next_mps < 47,
        super::QE_STATES@[(packed / 2) as int].next_lps < 47,
        super::QE_STATES@[(packed / 2) as int].qe > 0,
        super::QE_STATES@[(packed / 2) as int].qe <= 0x5601,
{
    let idx: int = (packed / 2) as int;
    qe_transition_valid(idx);
    assert(super::QE_PACKED_VALS@.len() == 128);
    assert(super::NEXT_MPS_PACKED@.len() == 128);
    if packed == 0 {} else if packed == 1 {} else if packed == 2 {} else if packed == 3 {}
    else if packed == 4 {} else if packed == 5 {} else if packed == 6 {} else if packed == 7 {}
    else if packed == 8 {} else if packed == 9 {} else if packed == 10 {} else if packed == 11 {}
    else if packed == 12 {} else if packed == 13 {} else if packed == 14 {} else if packed == 15 {}
    else if packed == 16 {} else if packed == 17 {} else if packed == 18 {} else if packed == 19 {}
    else if packed == 20 {} else if packed == 21 {} else if packed == 22 {} else if packed == 23 {}
    else if packed == 24 {} else if packed == 25 {} else if packed == 26 {} else if packed == 27 {}
    else if packed == 28 {} else if packed == 29 {} else if packed == 30 {} else if packed == 31 {}
    else if packed == 32 {} else if packed == 33 {} else if packed == 34 {} else if packed == 35 {}
    else if packed == 36 {} else if packed == 37 {} else if packed == 38 {} else if packed == 39 {}
    else if packed == 40 {} else if packed == 41 {} else if packed == 42 {} else if packed == 43 {}
    else if packed == 44 {} else if packed == 45 {} else if packed == 46 {} else if packed == 47 {}
    else if packed == 48 {} else if packed == 49 {} else if packed == 50 {} else if packed == 51 {}
    else if packed == 52 {} else if packed == 53 {} else if packed == 54 {} else if packed == 55 {}
    else if packed == 56 {} else if packed == 57 {} else if packed == 58 {} else if packed == 59 {}
    else if packed == 60 {} else if packed == 61 {} else if packed == 62 {} else if packed == 63 {}
    else if packed == 64 {} else if packed == 65 {} else if packed == 66 {} else if packed == 67 {}
    else if packed == 68 {} else if packed == 69 {} else if packed == 70 {} else if packed == 71 {}
    else if packed == 72 {} else if packed == 73 {} else if packed == 74 {} else if packed == 75 {}
    else if packed == 76 {} else if packed == 77 {} else if packed == 78 {} else if packed == 79 {}
    else if packed == 80 {} else if packed == 81 {} else if packed == 82 {} else if packed == 83 {}
    else if packed == 84 {} else if packed == 85 {} else if packed == 86 {} else if packed == 87 {}
    else if packed == 88 {} else if packed == 89 {} else if packed == 90 {} else if packed == 91 {}
    else if packed == 92 {} else {}
}

proof fn single_shot_renormalize_step_induction(
    decoder: &super::ArithmeticDecoder,
    shift: u32,
)
    requires
        super::arithmetic_spec::decoder_state_valid(decoder),
        0 < decoder.interval < super::MIN_INTERVAL,
        1 <= shift <= decoder.bit_count,
        shift <= 8,
        ((decoder.interval << (shift - 1) as u32) as u32) < super::MIN_INTERVAL,
        super::MIN_INTERVAL <= ((decoder.interval << shift) as u32) <= 0xffffu32,
        (decoder.code >> (32u32 - shift)) == 0,
    ensures
        super::arithmetic_spec::normalized_state(decoder) == (
            0u8,
            (decoder.interval << shift) as u32,
            decoder.byte_position,
            decoder.current_byte,
            (decoder.code << shift) as u32,
            (decoder.bit_count - shift as u8) as u8,
            decoder.marker,
        ),
    decreases shift,
{
    reveal(super::arithmetic_spec::decoder_state_valid);
    reveal(super::arithmetic_spec::decoder_valid);
    let interval = decoder.interval;
    let code = decoder.code;
    assert(
        ((interval << 1u32) as nat) == (interval as nat) * 2
        && ((code >> 31u32) == 0)
        && ((code << 1u32) as nat) == (code as nat) * 2
        && (code as nat) * 2 <= u32::MAX as nat
    ) by (bit_vector)
        requires
            0 < interval < 0x8000u32,
            1 <= shift <= 8,
            (code >> sub(32u32, shift)) == 0;
    let step1 = super::ArithmeticDecoder {
        bytes: decoder.bytes,
        byte_position: decoder.byte_position,
        current_byte: decoder.current_byte,
        code: code << 1u32,
        interval: interval << 1u32,
        bit_count: (decoder.bit_count - 1) as u8,
        marker: decoder.marker,
    };
    reveal_with_fuel(super::arithmetic_spec::normalized_state, 2);
    assert(super::arithmetic_spec::normalized_state(decoder)
        == super::arithmetic_spec::normalized_state(&step1));
    if shift == 1 {
        assert((interval << 1u32) >= 0x8000u32);
    } else {
        let rem = (shift - 1) as u32;
        assert(
            ((interval << 1u32) < 0x8000u32)
            && ((interval << 1u32) > 0u32)
            && (((interval << 1u32) << sub(rem, 1u32)) == (interval << rem))
            && (((interval << 1u32) << rem) == (interval << shift))
            && (((code << 1u32) << rem) == (code << shift))
            && (((code << 1u32) >> sub(32u32, rem)) == 0u32)
        ) by (bit_vector)
            requires
                0 < interval < 0x8000u32,
                2 <= shift <= 8,
                rem == sub(shift, 1u32),
                (interval << rem) < 0x8000u32,
                0x8000u32 <= (interval << shift) <= 0xffffu32,
                (code >> sub(32u32, shift)) == 0;
        single_shot_renormalize_step_induction(&step1, rem);
    }
}

pub(super) proof fn single_shot_renormalize_matches(
    decoder: &super::ArithmeticDecoder,
    shift: u32,
)
    requires
        super::arithmetic_spec::decoder_state_valid(decoder),
        decoder.interval < super::MIN_INTERVAL,
        shift == vstd::std_specs::bits::u32_leading_zeros(decoder.interval).wrapping_sub(16u32),
        shift <= decoder.bit_count as u32,
        (decoder.code >> (32u32 - shift)) == 0,
    ensures
        1 <= shift <= 8,
        decoder.interval > 0,
        super::MIN_INTERVAL <= ((decoder.interval << shift) as u32) <= 0xffffu32,
        super::arithmetic_spec::normalized_state(decoder) == (
            0u8,
            (decoder.interval << shift) as u32,
            decoder.byte_position,
            decoder.current_byte,
            (decoder.code << shift) as u32,
            (decoder.bit_count - shift as u8) as u8,
            decoder.marker,
        ),
{
    reveal(super::arithmetic_spec::decoder_state_valid);
    let interval = decoder.interval;
    vstd::std_specs::bits::axiom_u32_leading_zeros(interval);
    let lz = vstd::std_specs::bits::u32_leading_zeros(interval);
    assert(
        interval > 0
        && 17 <= lz <= 31
        && shift == sub(lz, 16u32)
        && 1 <= shift <= 8
        && ((interval << sub(shift, 1u32)) < 0x8000u32)
        && (0x8000u32 <= (interval << shift) <= 0xffffu32)
    ) by (bit_vector)
        requires
            interval < 0x8000u32,
            0 <= lz <= 32,
            (interval == 0) <==> (lz == 32),
            (0 <= lz < 32) ==> (((interval >> sub(31u32, lz)) & 1u32) != 0u32),
            (interval >> sub(32u32, lz)) == 0u32,
            shift == sub(lz, 16u32),
            shift <= 8u32;
    single_shot_renormalize_step_induction(decoder, shift);
}

pub(super) proof fn inline_mps_renorm_matches(
    decoder: &super::ArithmeticDecoder,
    context: super::ArithmeticContext,
    qe: u32,
    next_a: u32,
    next_c: u32,
)
    requires
        super::arithmetic_spec::decoder_valid(decoder),
        super::arithmetic_spec::context_valid(context),
        qe == super::QE_STATES@[super::arithmetic_spec::context_index(context) as int].qe as u32,
        decoder.code >= (qe << 16u32),
        next_a == (decoder.interval - qe) as u32,
        next_c == (decoder.code - (qe << 16u32)) as u32,
        0x4000u32 <= next_a < super::MIN_INTERVAL,
        next_a >= qe,
        decoder.bit_count != 0,
        (next_c & 0x8000_0000u32) == 0,
    ensures
        super::MIN_INTERVAL <= ((next_a << 1u32) as u32) <= 0xffffu32,
        super::arithmetic_spec::entry_decision(
            decoder.interval,
            decoder.code,
            super::arithmetic_spec::context_index(context),
            super::arithmetic_spec::context_mps(context),
        ) == super::arithmetic_spec::context_mps(context),
        super::arithmetic_spec::entry_index(
            decoder.interval,
            decoder.code,
            super::arithmetic_spec::context_index(context),
        ) == super::QE_STATES@[super::arithmetic_spec::context_index(context) as int].next_mps,
        super::arithmetic_spec::entry_mps(
            decoder.interval,
            decoder.code,
            super::arithmetic_spec::context_index(context),
            super::arithmetic_spec::context_mps(context),
        ) == super::arithmetic_spec::context_mps(context),
        super::arithmetic_spec::decision_state(decoder, context) == (
            super::arithmetic_spec::context_mps(context),
            (next_a << 1u32) as u32,
            decoder.byte_position,
            decoder.current_byte,
            (next_c << 1u32) as u32,
            (decoder.bit_count - 1) as u8,
            decoder.marker,
        ),
{
    reveal(super::arithmetic_spec::decoder_state_valid);
    reveal(super::arithmetic_spec::decoder_valid);
    super::arithmetic_spec::decision_model_equations(decoder, context);
    super::arithmetic_spec::entry_model_equations(
        decoder.interval,
        decoder.code,
        super::arithmetic_spec::context_index(context),
        super::arithmetic_spec::context_mps(context),
    );
    reveal(super::arithmetic_spec::exchange_is_mps);
    reveal(super::arithmetic_spec::exchange_decision);
    reveal(super::arithmetic_spec::exchange_index);
    reveal(super::arithmetic_spec::exchange_mps);
    let idx = super::arithmetic_spec::context_index(context);
    qe_transition_valid(idx as int);
    let c = decoder.code;
    assert(
        (c >> 16u32) >= qe
        && (next_a << 1u32) == mul(next_a, 2u32)
        && 0x8000u32 <= (next_a << 1u32) <= 0xffffu32
        && (next_c >> 31u32) == 0u32
        && ((next_c << 1u32) as nat) == (next_c as nat) * 2
        && (next_c as nat) * 2 <= u32::MAX as nat
    ) by (bit_vector)
        requires
            0 < qe <= 0x5601u32,
            c >= (qe << 16u32),
            0x4000u32 <= next_a < 0x8000u32,
            (next_c & 0x8000_0000u32) == 0u32;
    let frame = super::ArithmeticDecoder {
        bytes: decoder.bytes,
        byte_position: decoder.byte_position,
        current_byte: decoder.current_byte,
        code: next_c,
        interval: next_a,
        bit_count: decoder.bit_count,
        marker: decoder.marker,
    };
    reveal_with_fuel(super::arithmetic_spec::normalized_state, 2);
    assert(super::arithmetic_spec::normalized_state(&frame) == (
        0u8,
        (next_a << 1u32) as u32,
        decoder.byte_position,
        decoder.current_byte,
        (next_c << 1u32) as u32,
        (decoder.bit_count - 1) as u8,
        decoder.marker,
    ));
}

}
