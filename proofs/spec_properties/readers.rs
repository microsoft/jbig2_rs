// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: ../../specs/README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::bits_limit [proof-support]
// Purpose: Doubling bound used to justify reader accumulation.
// Audit: reader-bits-limit
// REQUIREMENT CONTEXT: T88-BITS-MSB-FIRST | ITU-T T.88 (08/2018) | 5.4.1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bits_limit(count: nat) -> nat
    decreases count,
{
    if count == 0 { 1 } else { 2 * bits_limit((count - 1) as nat) }
}

pub(crate) proof fn bits_limit_monotone(first: nat, last: nat)
    requires first <= last,
    ensures bits_limit(first) <= bits_limit(last),
    decreases last,
{
    if first < last {
        bits_limit_monotone(first, (last - 1) as nat);
    }
}

pub(crate) proof fn bits_value_bound(bytes: Seq<u8>, start: nat, count: nat)
    requires start + count <= bytes.len() * 8,
    ensures crate::spec::readers::bits_value(bytes, start, count) < bits_limit(count),
    decreases count,
{
    if count > 0 {
        bits_value_bound(bytes, start, (count - 1) as nat);
        let position = start + count - 1;
        let byte = bytes[(position / 8) as int];
        let shift = (7 - position % 8) as u8;
        assert(((byte >> shift) & 1) <= 1) by (bit_vector);
    }
}

pub(crate) proof fn bits_value_append_one(bytes: Seq<u8>, start: nat, count: nat)
    requires start + count < bytes.len() * 8,
    ensures
        crate::spec::readers::bits_value(bytes, start, count + 1)
            == crate::spec::readers::bits_value(bytes, start, count) * 2
                + crate::spec::readers::bits_value(bytes, start + count, 1),
{
    reveal_with_fuel(crate::spec::readers::bits_value, 2);
}

pub(crate) proof fn single_byte_bits_value(
    bytes: Seq<u8>,
    byte_idx: nat,
    bit_off: u32,
    count: u32,
)
    requires
        byte_idx < bytes.len(),
        1 <= count <= 8,
        bit_off + count <= 8,
    ensures
        crate::spec::readers::bits_value(bytes, byte_idx * 8 + bit_off as nat, count as nat)
            == (((bytes[byte_idx as int] as u32) >> ((8u32 - bit_off - count) as u32))
                & (0xffu32 >> ((8u32 - count) as u32))) as nat,
    decreases count,
{
    let b = bytes[byte_idx as int];
    let pos = byte_idx * 8 + bit_off as nat + (count - 1) as nat;
    assert(pos / 8 == byte_idx);
    assert(pos % 8 == bit_off as nat + (count - 1) as nat);
    let bit_shift = (8u32 - bit_off - count) as u32;
    let mask = 0xffu32 >> ((8u32 - count) as u32);
    if count == 1 {
        reveal_with_fuel(crate::spec::readers::bits_value, 2);
        assert(
            ((((b as u32) >> bit_shift) & mask) as nat)
                == (((b >> (bit_shift as u8)) & 1u8) as nat)
        ) by (bit_vector)
            requires
                bit_shift < 8u32,
                mask == (0xffu32 >> 7u32);
    } else {
        single_byte_bits_value(bytes, byte_idx, bit_off, (count - 1) as u32);
        let prev_shift = (8u32 - bit_off - (count - 1)) as u32;
        let prev_mask = 0xffu32 >> ((8u32 - (count - 1)) as u32);
        let prev_val = ((b as u32) >> prev_shift) & prev_mask;
        let bit_val = ((b >> (bit_shift as u8)) & 1u8) as u32;
        assert(
            (((b as u32) >> bit_shift) & mask) == add(mul(prev_val, 2u32), bit_val)
            && ((prev_val as nat) * 2 + (bit_val as nat) <= u32::MAX as nat)
        ) by (bit_vector)
            requires
                2u32 <= count <= 8u32,
                bit_off + count <= 8u32,
                bit_shift == sub(sub(8u32, bit_off), count),
                prev_shift == add(bit_shift, 1u32),
                mask == (0xffu32 >> sub(8u32, count)),
                prev_mask == (0xffu32 >> sub(9u32, count)),
                prev_val == (((b as u32) >> prev_shift) & prev_mask),
                bit_val == (((b >> (bit_shift as u8)) & 1u8) as u32);
    }
}

proof fn u64_word_bit_at(
    bytes: Seq<u8>,
    byte_idx: nat,
    k: u32,
    word: u64,
)
    requires
        byte_idx + 8 <= bytes.len(),
        k < 40,
        word == ((((bytes[byte_idx as int] as u64) << 56u64)
            | ((bytes[(byte_idx + 1) as int] as u64) << 48u64)
            | ((bytes[(byte_idx + 2) as int] as u64) << 40u64)
            | ((bytes[(byte_idx + 3) as int] as u64) << 32u64)
            | ((bytes[(byte_idx + 4) as int] as u64) << 24u64)
            | ((bytes[(byte_idx + 5) as int] as u64) << 16u64)
            | ((bytes[(byte_idx + 6) as int] as u64) << 8u64)
            | (bytes[(byte_idx + 7) as int] as u64))),
    ensures
        crate::spec::readers::bit_at(bytes, byte_idx * 8 + k as nat)
            == ((word >> ((63u32 - k) as u64)) & 1u64) as nat,
{
    let b0 = bytes[byte_idx as int];
    let b1 = bytes[(byte_idx + 1) as int];
    let b2 = bytes[(byte_idx + 2) as int];
    let b3 = bytes[(byte_idx + 3) as int];
    let b4 = bytes[(byte_idx + 4) as int];
    let b5 = bytes[(byte_idx + 5) as int];
    let b6 = bytes[(byte_idx + 6) as int];
    let b7 = bytes[(byte_idx + 7) as int];
    let lane = k / 8;
    let rem = k % 8;
    assert((byte_idx * 8 + k as nat) / 8 == byte_idx + lane as nat);
    assert((byte_idx * 8 + k as nat) % 8 == rem as nat);
    if lane == 0 {
        assert(((word >> sub(63u64, k as u64)) & 1u64) == (((b0 >> sub(7u8, rem as u8)) & 1u8) as u64))
            by (bit_vector)
            requires
                k < 8u32,
                rem == k,
                word == (((b0 as u64) << 56u64) | ((b1 as u64) << 48u64) | ((b2 as u64) << 40u64)
                    | ((b3 as u64) << 32u64) | ((b4 as u64) << 24u64) | ((b5 as u64) << 16u64)
                    | ((b6 as u64) << 8u64) | (b7 as u64));
    } else if lane == 1 {
        assert(((word >> sub(63u64, k as u64)) & 1u64) == (((b1 >> sub(7u8, rem as u8)) & 1u8) as u64))
            by (bit_vector)
            requires
                8u32 <= k < 16u32,
                rem == sub(k, 8u32),
                word == (((b0 as u64) << 56u64) | ((b1 as u64) << 48u64) | ((b2 as u64) << 40u64)
                    | ((b3 as u64) << 32u64) | ((b4 as u64) << 24u64) | ((b5 as u64) << 16u64)
                    | ((b6 as u64) << 8u64) | (b7 as u64));
    } else if lane == 2 {
        assert(((word >> sub(63u64, k as u64)) & 1u64) == (((b2 >> sub(7u8, rem as u8)) & 1u8) as u64))
            by (bit_vector)
            requires
                16u32 <= k < 24u32,
                rem == sub(k, 16u32),
                word == (((b0 as u64) << 56u64) | ((b1 as u64) << 48u64) | ((b2 as u64) << 40u64)
                    | ((b3 as u64) << 32u64) | ((b4 as u64) << 24u64) | ((b5 as u64) << 16u64)
                    | ((b6 as u64) << 8u64) | (b7 as u64));
    } else if lane == 3 {
        assert(((word >> sub(63u64, k as u64)) & 1u64) == (((b3 >> sub(7u8, rem as u8)) & 1u8) as u64))
            by (bit_vector)
            requires
                24u32 <= k < 32u32,
                rem == sub(k, 24u32),
                word == (((b0 as u64) << 56u64) | ((b1 as u64) << 48u64) | ((b2 as u64) << 40u64)
                    | ((b3 as u64) << 32u64) | ((b4 as u64) << 24u64) | ((b5 as u64) << 16u64)
                    | ((b6 as u64) << 8u64) | (b7 as u64));
    } else {
        assert(((word >> sub(63u64, k as u64)) & 1u64) == (((b4 >> sub(7u8, rem as u8)) & 1u8) as u64))
            by (bit_vector)
            requires
                32u32 <= k < 40u32,
                rem == sub(k, 32u32),
                word == (((b0 as u64) << 56u64) | ((b1 as u64) << 48u64) | ((b2 as u64) << 40u64)
                    | ((b3 as u64) << 32u64) | ((b4 as u64) << 24u64) | ((b5 as u64) << 16u64)
                    | ((b6 as u64) << 8u64) | (b7 as u64));
    }
}

pub(crate) proof fn u64_be_bits_value(
    bytes: Seq<u8>,
    byte_idx: nat,
    bit_off: u32,
    count: u32,
    word: u64,
)
    requires
        byte_idx + 8 <= bytes.len(),
        bit_off < 8,
        1 <= count <= 32,
        word == ((((bytes[byte_idx as int] as u64) << 56u64)
            | ((bytes[(byte_idx + 1) as int] as u64) << 48u64)
            | ((bytes[(byte_idx + 2) as int] as u64) << 40u64)
            | ((bytes[(byte_idx + 3) as int] as u64) << 32u64)
            | ((bytes[(byte_idx + 4) as int] as u64) << 24u64)
            | ((bytes[(byte_idx + 5) as int] as u64) << 16u64)
            | ((bytes[(byte_idx + 6) as int] as u64) << 8u64)
            | (bytes[(byte_idx + 7) as int] as u64))),
    ensures
        crate::spec::readers::bits_value(bytes, byte_idx * 8 + bit_off as nat, count as nat)
            == ((word >> ((64u32 - bit_off - count) as u64))
                & (0xffff_ffffu64 >> ((32u32 - count) as u64))) as nat,
    decreases count,
{
    let k = (bit_off + count - 1) as u32;
    u64_word_bit_at(bytes, byte_idx, k, word);
    let bit_shift = (64u32 - bit_off - count) as u64;
    let mask = 0xffff_ffffu64 >> ((32u32 - count) as u64);
    if count == 1 {
        reveal_with_fuel(crate::spec::readers::bits_value, 2);
        assert(((word >> bit_shift) & mask) == ((word >> bit_shift) & 1u64)) by (bit_vector)
            requires
                bit_shift < 64u64,
                mask == (0xffff_ffffu64 >> 31u64);
    } else {
        u64_be_bits_value(bytes, byte_idx, bit_off, (count - 1) as u32, word);
        let prev_shift = (64u32 - bit_off - (count - 1)) as u64;
        let prev_mask = 0xffff_ffffu64 >> ((32u32 - (count - 1)) as u64);
        let prev_val = (word >> prev_shift) & prev_mask;
        let bit_val = (word >> bit_shift) & 1u64;
        assert(
            ((word >> bit_shift) & mask) == add(mul(prev_val, 2u64), bit_val)
            && ((prev_val as nat) * 2 + (bit_val as nat) <= u64::MAX as nat)
        ) by (bit_vector)
            requires
                2u32 <= count <= 32u32,
                bit_off < 8u32,
                bit_shift == sub(sub(64u64, bit_off as u64), count as u64),
                prev_shift == add(bit_shift, 1u64),
                mask == (0xffff_ffffu64 >> sub(32u64, count as u64)),
                prev_mask == (0xffff_ffffu64 >> sub(33u64, count as u64)),
                prev_val == ((word >> prev_shift) & prev_mask),
                bit_val == ((word >> bit_shift) & 1u64);
    }
}

}
