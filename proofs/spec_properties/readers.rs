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

pub(crate) proof fn bits_value_append_eight(bytes: Seq<u8>, start: nat, count: nat)
    requires start + count + 8 <= bytes.len() * 8,
    ensures crate::spec::readers::bits_value(bytes, start, count + 8)
        == crate::spec::readers::bits_value(bytes, start, count) * 256
            + crate::spec::readers::bits_value(bytes, start + count, 8),
{
    reveal_with_fuel(crate::spec::readers::bits_value, 9);
}

pub(crate) proof fn unaligned_byte_bit(bytes: Seq<u8>, p: nat, byte: u8, i: nat)
    requires
        i < 8,
        p + 8 <= bytes.len() * 8,
        byte == if p % 8 == 0 { bytes[(p / 8) as int] } else {
            (bytes[(p / 8) as int] << ((p % 8) as u32))
                | (bytes[((p + 7) / 8) as int] >> ((8 - p % 8) as u32))
        },
    ensures crate::spec::readers::bit_at(bytes, p + i)
        == ((byte as u32 >> ((7 - i) as u32)) & 1) as nat,
{
    let first = bytes[(p / 8) as int];
    let last = bytes[((p + 7) / 8) as int];
    let r = p % 8;
    assert(r < 8);
    assert(p == p / 8 * 8 + r);
        let source = if r + i < 8 {
            assert((p + i) / 8 == p / 8);
            first
        } else {
            assert((p + i) / 8 == (p + 7) / 8);
            last
        };
        assert((p + i) % 8 == (r + i) % 8);
        let source_shift = (7 - (r + i) % 8) as u32;
        let target_shift = (7 - i) as u32;
        let offset = r as u32;
        let ordinal = i as u32;
        assert(
            ((source as u32 >> source_shift) & 1)
                == ((byte as u32 >> target_shift) & 1)
        ) by (bit_vector)
            requires offset < 8, ordinal < 8,
                source == if offset + ordinal < 8 { first } else { last },
                source_shift == 7 - (offset + ordinal) % 8,
                target_shift == 7 - ordinal,
                byte == if offset == 0 { first } else {
                    (first << offset) | (last >> ((8 - offset) as u32))
                };
        assert(((source >> (source_shift as u8)) & 1) as u32
            == ((source as u32 >> source_shift) & 1)) by (bit_vector)
            requires source_shift < 8;
}

pub(crate) proof fn unaligned_byte_value(bytes: Seq<u8>, p: nat, byte: u8)
    requires
        p + 8 <= bytes.len() * 8,
        byte == if p % 8 == 0 { bytes[(p / 8) as int] } else {
            (bytes[(p / 8) as int] << ((p % 8) as u32))
                | (bytes[((p + 7) / 8) as int] >> ((8 - p % 8) as u32))
        },
    ensures crate::spec::readers::bits_value(bytes, p, 8) == byte as nat,
{
    unaligned_byte_bit(bytes, p, byte, 0);
    unaligned_byte_bit(bytes, p, byte, 1);
    unaligned_byte_bit(bytes, p, byte, 2);
    unaligned_byte_bit(bytes, p, byte, 3);
    unaligned_byte_bit(bytes, p, byte, 4);
    unaligned_byte_bit(bytes, p, byte, 5);
    unaligned_byte_bit(bytes, p, byte, 6);
    unaligned_byte_bit(bytes, p, byte, 7);
    assert((byte as u32 >> 0) == byte as u32) by (bit_vector);
    assert((byte as u32)
        == (((byte as u32 >> 7) & 1) * 128
            + ((byte as u32 >> 6) & 1) * 64
            + ((byte as u32 >> 5) & 1) * 32
            + ((byte as u32 >> 4) & 1) * 16
            + ((byte as u32 >> 3) & 1) * 8
            + ((byte as u32 >> 2) & 1) * 4
            + ((byte as u32 >> 1) & 1) * 2
            + (byte as u32 & 1))) by (bit_vector);
    reveal_with_fuel(crate::spec::readers::bits_value, 9);
    assert(crate::spec::readers::bit_at(bytes, p) == ((byte as u32 >> 7) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 1) == ((byte as u32 >> 6) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 2) == ((byte as u32 >> 5) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 3) == ((byte as u32 >> 4) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 4) == ((byte as u32 >> 3) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 5) == ((byte as u32 >> 2) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 6) == ((byte as u32 >> 1) & 1) as nat);
    assert(crate::spec::readers::bit_at(bytes, p + 7) == (byte as u32 & 1) as nat);
}

pub(crate) proof fn unaligned_byte_append(
    bytes: Seq<u8>, start: nat, count: nat, previous: u32, byte: u8,
)
    requires
        start + count + 8 <= bytes.len() * 8,
        count <= 24,
        byte == if (start + count) % 8 == 0 {
            bytes[((start + count) / 8) as int]
        } else {
            (bytes[((start + count) / 8) as int] << (((start + count) % 8) as u32))
                | (bytes[((start + count + 7) / 8) as int]
                    >> ((8 - (start + count) % 8) as u32))
        },
        previous as nat == crate::spec::readers::bits_value(bytes, start, count),
    ensures
        ((previous << 8) | byte as u32) as nat
            == crate::spec::readers::bits_value(bytes, start, count + 8),
{
    bits_value_bound(bytes, start, count);
    bits_limit_monotone(count, 24);
    assert(bits_limit(24) == 0x100_0000) by (compute);
    assert(previous <= 0x00ff_ffff);
    assert(((previous << 8) | byte as u32) == previous * 256 + byte as u32)
        by (bit_vector) requires previous <= 0x00ff_ffff;
    bits_value_append_eight(bytes, start, count);
    unaligned_byte_value(bytes, start + count, byte);
}

}
