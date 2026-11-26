// Included in huffman_spec; the closed model is not made open for callers.
verus! {

pub(super) proof fn decode_state_definition(
    table: &super::HuffmanTable, bytes: Seq<u8>, position: usize,
)
    requires bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
    ensures decode_state(table, bytes, position) == (
        if table.prefer_prefix_lookup && position + super::PREFIX_LOOKUP_BITS as nat
            <= bytes.len() * 8 {
            let index = position / 8;
            let offset = position % 8;
            let prefix = if offset == 0 { bytes[index as int] } else {
                let first = bytes[index as int] as u16;
                let second = bytes[(index + 1) as int] as u16;
                ((((first << 8) | second) >> ((8 - offset) as usize)) & 0xff) as u8
            };
            let entry = if (prefix as nat) < slice_view(&table.prefix_lookup).len() {
                slice_view(&table.prefix_lookup)[prefix as int]
            } else { 0u16 };
            if entry == 0 {
                fallback_run(table, bytes, position, 1, super::WideCode { limbs: [0u64, 0, 0, 0] })
            } else {
                let index = entry as int - 1;
                if index >= slice_view(&table.lines).len() {
                    (Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode)), position)
                } else {
                    let line = slice_view(&table.lines)[index];
                    if line.prefix_len > super::PREFIX_LOOKUP_BITS {
                        (Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode)), position)
                    } else { line_state(line, bytes, (position + line.prefix_len as nat) as usize) }
                }
            }
        } else {
            fallback_run(table, bytes, position, 1, super::WideCode { limbs: [0u64, 0, 0, 0] })
        }),
{
    reveal(decode_state);
}

pub(super) proof fn fallback_run_definition(
    table: &super::HuffmanTable, bytes: Seq<u8>, position: usize,
    length: usize, code: super::WideCode,
)
    requires
        bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
        1 <= length <= 256,
    ensures fallback_run(table, bytes, position, length, code) == (
        if length > table.max_prefix_len as usize {
            (Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode)), position)
        } else {
            match crate::spec::readers::bit_read_error(bytes, position, 1) {
                Some(error) => (
                    Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(error))), position,
                ),
                None => {
                    let next = (position + 1) as usize;
                    let bit = crate::spec::readers::bits_value(bytes, position as nat, 1) as u32;
                    match append_bit(code, bit) {
                        None => (
                            Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(
                                crate::MalformedError::IntegerOverflow,
                            ))), next,
                        ),
                        Some(prefix) => match small_distance_from(
                            prefix, table_first_codes(table)[length as int],
                            table_length_counts(table)[length as int],
                        ) {
                            None => fallback_run(table, bytes, next, (length + 1) as usize, prefix),
                            Some(distance) => {
                                let ordered =
                                    table_first_lines(table)[length as int] as nat + distance as nat;
                                if ordered > u32::MAX {
                                    (
                                        Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(
                                            crate::MalformedError::IntegerOverflow,
                                        ))), next,
                                    )
                                } else if ordered >= slice_view(&table.ordered_lines).len() {
                                    (
                                        Err(super::HuffmanError::InvalidTable(
                                            super::HuffmanTableError::InvalidCode,
                                        )), next,
                                    )
                                } else {
                                    let index = slice_view(&table.ordered_lines)[ordered as int] as nat;
                                    if index >= slice_view(&table.lines).len() {
                                        (
                                            Err(super::HuffmanError::InvalidTable(
                                                super::HuffmanTableError::InvalidCode,
                                            )), next,
                                        )
                                    } else { line_state(slice_view(&table.lines)[index as int], bytes, next) }
                                }
                            },
                        },
                    }
                },
            }
        }),
{
    reveal_with_fuel(fallback_run, 1);
}

pub(super) proof fn cached_decode_outcome_definition()
    ensures forall|
        table: &super::HuffmanTable,
        before: &crate::reader::BitReader,
        after: &crate::reader::BitReader,
        out: Result<super::HuffmanValue, super::HuffmanError>,
    | reader_valid(before) ==> #[trigger] cached_decode_outcome(table, before, after, out) == (
        (table.prefer_prefix_lookup && before.bit_position + super::PREFIX_LOOKUP_BITS as nat
            <= before.bytes@.len() * 8) ==> {
            let index = before.bit_position / 8;
            let offset = before.bit_position % 8;
            let prefix = if offset == 0 {
                before.bytes@[index as int]
            } else {
                let first = before.bytes@[index as int] as u16;
                let second = before.bytes@[(index + 1) as int] as u16;
                ((((first << 8) | second) >> ((8 - offset) as usize)) & 0xff) as u8
            };
            let entry = if (prefix as nat) < slice_view(&table.prefix_lookup).len() {
                slice_view(&table.prefix_lookup)[prefix as int]
            } else { 0u16 };
            entry != 0 ==> {
                let line_index = entry as int - 1;
                &&& after.bytes@ == before.bytes@
                &&& if line_index >= slice_view(&table.lines).len() {
                    out == Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode))
                        && after.bit_position == before.bit_position
                } else {
                    let line = slice_view(&table.lines)[line_index];
                    if line.prefix_len > super::PREFIX_LOOKUP_BITS {
                        out == Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode))
                            && after.bit_position == before.bit_position
                    } else {
                        let suffix_start = (before.bit_position + line.prefix_len as nat) as nat;
                        if line.kind == super::LineKind::OutOfBand {
                            out == Ok(super::HuffmanValue::OutOfBand)
                                && after.bit_position == suffix_start
                        } else {
                            match crate::spec::readers::bit_read_error(
                                before.bytes@, suffix_start as usize, line.range_len,
                            ) {
                                Some(error) =>
                                    out == Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(error)))
                                        && after.bit_position == suffix_start,
                                None => {
                                    let range = crate::spec::readers::bits_value(
                                        before.bytes@, suffix_start, line.range_len as nat,
                                    ) as int;
                                    let value = if line.kind == super::LineKind::Lower {
                                        line.range_low as int - range
                                    } else {
                                        line.range_low as int + range
                                    };
                                    &&& after.bit_position == suffix_start + line.range_len as nat
                                    &&& out == if i32::MIN as int <= value <= i32::MAX as int {
                                        Ok(super::HuffmanValue::Integer(value as i32))
                                    } else {
                                        Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(
                                            crate::MalformedError::IntegerOverflow,
                                        )))
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }),
{
    reveal(cached_decode_outcome);
}

pub(super) proof fn full_prefix(lines: Seq<super::HuffmanLine>)
    ensures lines.take(lines.len() as int) == lines,
{
    vstd::assert_seqs_equal!(lines.take(lines.len() as int), lines);
}

pub(super) proof fn canonical_assignment_definition()
    ensures
        forall|lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable|
            #[trigger] assignment_contents(lines, table) == (
                slice_view(&table.lines) == lines
                && histogram(lines, table_length_counts(table), lines.len())
                && 1 <= table.max_prefix_len <= 255
                && table_length_counts(table)[table.max_prefix_len as int] > 0
                && (forall|length: int| table.max_prefix_len < length < 256 ==>
                    table_length_counts(table)[length] == 0)
                && slice_view(&table.ordered_lines).len()
                    == ordered_before(lines, table.max_prefix_len as nat + 1).len()),
        forall|lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable|
            #[trigger] assignment_codes(lines, table) == (
                table_first_codes(table)[0].limbs@ == seq![0u64, 0u64, 0u64, 0u64]
                && forall|length: int| 1 <= length <= table.max_prefix_len ==>
                    #[trigger] first_prefix(lines, length as nat) == Some(table_first_codes(table)[length])
                    && (table_length_counts(table)[length] > 0 ==>
                        canonical_range_fits(table_first_codes(table)[length], table_length_counts(table)[length], length as usize))),
        forall|lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable|
            #[trigger] assignment_starts(lines, table) == (
                forall|length: int| 1 <= length <= table.max_prefix_len
                    && table_length_counts(table)[length] > 0 ==>
                        table_first_lines(table)[length] == ordered_before(lines, length as nat).len()),
        forall|lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable|
            #[trigger] assignment_order(lines, table) == (
                slice_view(&table.ordered_lines) == ordered_before(lines, table.max_prefix_len as nat + 1)),
{
    reveal(assignment_contents);
    reveal(assignment_codes);
    reveal(assignment_starts);
    reveal(assignment_order);
}

pub(super) proof fn assignment_complete(lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable)
    requires
        slice_view(&table.lines) == lines,
        histogram(lines, table_length_counts(table), lines.len()),
        1 <= table.max_prefix_len <= 255,
        table_length_counts(table)[table.max_prefix_len as int] > 0,
        forall|length: int| table.max_prefix_len < length < 256 ==>
            table_length_counts(table)[length] == 0,
        assignment_prefix(
            lines, table_first_codes(table), table_first_lines(table), slice_view(&table.ordered_lines),
            table.max_prefix_len as nat + 1,
        ),
    ensures canonical_assignment(lines, table),
{
    canonical_assignment_definition();
    assert forall|length: int| 1 <= length <= table.max_prefix_len implies
        #[trigger] first_prefix(lines, length as nat) == Some(table_first_codes(table)[length])
        && (table_length_counts(table)[length] > 0 ==>
            canonical_range_fits(table_first_codes(table)[length], table_length_counts(table)[length], length as usize)) by {
        assert(table_length_counts(table)[length] == bucket(lines, length as nat, lines.len()).len());
    }
    assert forall|length: int| 1 <= length <= table.max_prefix_len
        && table_length_counts(table)[length] > 0 implies
            table_first_lines(table)[length] == ordered_before(lines, length as nat).len() by {
        assert(first_prefix(lines, length as nat) == Some(table_first_codes(table)[length]));
        assert(table_length_counts(table)[length] == bucket(lines, length as nat, lines.len()).len());
    }
}

pub(super) proof fn histogram_empty(lines: Seq<super::HuffmanLine>, counts: Seq<u32>)
    requires counts.len() == 256, forall|i: int| 0 <= i < 256 ==> counts[i] == 0,
    ensures histogram(lines, counts, 0),
{
    assert forall|length: int| 0 <= length < 256 implies
        counts[length] == bucket(lines, length as nat, 0).len() by {
    }
}

pub(super) proof fn histogram_step(
    lines: Seq<super::HuffmanLine>, before: Seq<u32>, after: Seq<u32>, scanned: nat,
)
    requires
        histogram(lines, before, scanned),
        scanned < lines.len(),
        after.len() == 256,
        forall|length: int| 0 <= length < 256 ==> after[length] as int ==
            before[length] as int
                + if length != 0 && lines[scanned as int].prefix_len == length { 1int } else { 0int },
    ensures histogram(lines, after, scanned + 1),
{
    assert forall|length: int| 0 <= length < 256 implies
        after[length] == bucket(lines, length as nat, scanned + 1).len() by {
        reveal(bucket);
    }
}

pub(super) proof fn empty_bucket(
    lines: Seq<super::HuffmanLine>, length: nat, scanned: nat, prefix: Seq<u32>,
)
    requires scanned <= lines.len(), bucket(lines, length, scanned).len() == 0,
    ensures bucket(lines, length, scanned) == Seq::<u32>::empty(),
        prefix + bucket(lines, length, scanned) == prefix,
{
    vstd::assert_seqs_equal!(bucket(lines, length, scanned), Seq::<u32>::empty());
    vstd::assert_seqs_equal!(prefix + bucket(lines, length, scanned), prefix);
}

pub(super) proof fn bucket_step(
    lines: Seq<super::HuffmanLine>, length: nat, scanned: nat,
    prefix: Seq<u32>, before: Seq<u32>, after: Seq<u32>,
)
    requires
        scanned < lines.len(),
        before == prefix + bucket(lines, length, scanned),
        after == if length != 0 && lines[scanned as int].prefix_len == length {
            before.push(scanned as u32)
        } else { before },
    ensures after == prefix + bucket(lines, length, scanned + 1),
{
    reveal(bucket);
    vstd::assert_seqs_equal!(after, prefix + bucket(lines, length, scanned + 1));
}

pub(super) proof fn assignment_step(
    lines: Seq<super::HuffmanLine>, counts: Seq<u32>,
    old_codes: Seq<super::WideCode>, old_starts: Seq<u32>, old_order: Seq<u32>,
    codes: Seq<super::WideCode>, starts: Seq<u32>, order: Seq<u32>, length: nat,
)
    requires
        histogram(lines, counts, lines.len()),
        assignment_prefix(lines, old_codes, old_starts, old_order, length),
        length < 256,
        codes.len() == 256,
        codes == old_codes.update(length as int, codes[length as int]),
        canonical_first_code(old_codes[length - 1], counts[length - 1]) == Some(codes[length as int]),
        starts.len() == 256,
        forall|i: int| 0 <= i < 256 && i != length ==> starts[i] == old_starts[i],
        counts[length as int] > 0 ==> starts[length as int] == old_order.len()
            && canonical_range_fits(codes[length as int], counts[length as int], length as usize),
        order == old_order + bucket(lines, length, lines.len()),
    ensures assignment_prefix(lines, codes, starts, order, length + 1),
{
    if length == 1 {
        assert(ext_equal(old_codes[0].limbs, [0u64, 0u64, 0u64, 0u64]));
        assert(first_prefix(lines, 0) == Some(old_codes[0]));
    } else {
        assert(1 <= length - 1 < length);
        assert(first_prefix(lines, (length - 1) as nat) == Some(old_codes[length - 1]));
    }
    assert(counts[length - 1] == bucket(lines, (length - 1) as nat, lines.len()).len());
    assert(first_prefix(lines, length) == Some(codes[length as int]));
    assert(order == ordered_before(lines, length + 1));
    assert forall|i: int| 1 <= i < length + 1 implies
        first_prefix(lines, i as nat) == Some(codes[i])
        && (bucket(lines, i as nat, lines.len()).len() > 0 ==>
            starts[i] == ordered_before(lines, i as nat).len()
            && canonical_range_fits(codes[i], bucket(lines, i as nat, lines.len()).len() as u32, i as usize)) by {
        if i != length {
            assert(codes[i] == old_codes[i]);
        }
    }
}

pub(super) proof fn table_has_oob_definition(table: &super::HuffmanTable)
    ensures table_has_oob(table) == exists|index: int|
        0 <= index < slice_view(&table.lines).len()
        && slice_view(&table.lines)[index].kind == super::LineKind::OutOfBand,
{
    reveal(table_has_oob);
}

pub(super) proof fn short_canonical_step(prev: u32, count: u32, next_u64: u64)
    requires
        next_u64 == (((prev as u64 + count as u64) as u64) << 1u64),
        next_u64 <= u32::MAX as u64,
    ensures
        canonical_first_code(super::WideCode { limbs: [prev as u64, 0, 0, 0] }, count)
            == Some(super::WideCode { limbs: [next_u64, 0, 0, 0] }),
{
    reveal(canonical_first_code);
    reveal(checked_add_small);
    reveal(checked_shift_left_one);
    reveal(add_limb);
    let sum: u64 = (prev as u64 + count as u64) as u64;
    let w = super::WideCode { limbs: [sum, 0u64, 0u64, 0u64] };
    assert(
        (sum << 1) == next_u64
        && (sum >> 63) == 0u64
        && ((0u64 << 1) | (sum >> 63)) == 0u64
        && ((0u64 << 1) | (0u64 >> 63)) == 0u64
        && (0u64 >> 63) == 0u64
    ) by (bit_vector)
        requires
            sum <= 0x1_ffff_fffeu64,
            next_u64 == (sum << 1u64);
    assert(ext_equal(
        checked_add_small(super::WideCode { limbs: [prev as u64, 0, 0, 0] }, count).unwrap().limbs,
        w.limbs,
    ));
    assert(ext_equal(
        checked_shift_left_one(w).unwrap().limbs,
        [next_u64, 0u64, 0u64, 0u64],
    ));
}

pub(super) proof fn short_range_fits(first: u32, count: u32, length: usize)
    requires
        1 <= length <= 32,
        count > 0,
        (first as u64) + ((count - 1) as u64) < (1u64 << (length as u64)),
    ensures
        canonical_range_fits(super::WideCode { limbs: [first as u64, 0, 0, 0] }, count, length),
{
    reveal(canonical_range_fits);
    reveal(checked_add_small);
    reveal(add_limb);
    reveal(fits_in_bits);
    let last: u64 = ((first as u64) + ((count - 1) as u64)) as u64;
    assert(
        ((1u64 << (length as u64)) as nat) == pow2(length as nat)
        && last < (1u64 << (length as u64))
    ) by {
        pow2_eq_u64_shift(length as nat);
    };
    assert(last <= u32::MAX as u64) by (bit_vector)
        requires 1usize <= length <= 32usize, last < (1u64 << (length as u64));
    assert(ext_equal(
        checked_add_small(super::WideCode { limbs: [first as u64, 0, 0, 0] }, (count - 1) as u32)
            .unwrap()
            .limbs,
        [last, 0, 0, 0],
    ));
    let w = super::WideCode { limbs: [last, 0, 0, 0] };
    assert(wide_value(w) == last as nat);
    assert(wide_fits(w, length));
    if length < 32 {
        assert((last >> (length as u64)) == 0u64) by (bit_vector)
            requires 1usize <= length < 32usize, last < (1u64 << (length as u64));
    } else {
        assert((last >> 32u64) == 0u64) by (bit_vector)
            requires last < (1u64 << 32u64);
    }
}

proof fn pow2_eq_u64_shift(k: nat)
    requires k <= 32,
    ensures ((1u64 << (k as u64)) as nat) == pow2(k),
    decreases k,
{
    if k == 0 {
        assert((1u64 << 0u64) == 1u64) by (bit_vector);
    } else {
        pow2_eq_u64_shift((k - 1) as nat);
        let ku = k as u64;
        let prev = 1u64 << sub(ku, 1u64);
        assert((1u64 << ku) == mul(prev, 2u64) && (prev as nat) * 2 <= u64::MAX as nat)
            by (bit_vector)
            requires 1u64 <= ku <= 32u64, prev == (1u64 << sub(ku, 1u64));
    }
}

pub(super) proof fn linear_buckets_match_ordered_before(
    lines: Seq<super::HuffmanLine>,
    starts: Seq<u32>,
    counts: Seq<u32>,
    order: Seq<u32>,
    upto: nat,
)
    requires
        1 <= upto <= 33,
        starts.len() == 33,
        counts.len() == 33,
        forall|l: int| 1 <= l < upto ==> counts[l] == bucket(lines, l as nat, lines.len()).len(),
        forall|l: int| 1 <= l < upto ==> starts[l] == ordered_before(lines, l as nat).len(),
        order.len() >= ordered_before(lines, upto).len(),
        forall|l: int| 1 <= l < upto ==>
            order.subrange(starts[l] as int, (starts[l] + counts[l]) as int)
                == bucket(lines, l as nat, lines.len()),
    ensures
        order.subrange(0, ordered_before(lines, upto).len() as int) == ordered_before(lines, upto),
    decreases upto,
{
    if upto == 1 {
        vstd::assert_seqs_equal!(order.subrange(0, 0), ordered_before(lines, 1));
    } else {
        let prev = (upto - 1) as nat;
        linear_buckets_match_ordered_before(lines, starts, counts, order, prev);
        let s = starts[prev as int] as int;
        let c = counts[prev as int] as int;
        let left = order.subrange(0, s);
        let right = order.subrange(s, s + c);
        assert(left == ordered_before(lines, prev));
        assert(right == bucket(lines, prev, lines.len()));
        vstd::assert_seqs_equal!(order.subrange(0, s + c), left + right);
    }
}

pub(super) proof fn short_append_bit_matches(code: u32, bit: u32)
    requires
        code <= 0x7fff_ffffu32,
        bit <= 1u32,
    ensures
        append_bit(super::WideCode { limbs: [code as u64, 0, 0, 0] }, bit)
            == Some(super::WideCode { limbs: [(((code << 1u32) | bit) as u64), 0, 0, 0] }),
{
    reveal(append_bit);
    reveal(checked_shift_left_one);
    reveal(checked_add_small);
    reveal(add_limb);
    let next = (code << 1u32) | bit;
    let c64 = code as u64;
    assert(
        ((c64 << 1) == ((code << 1u32) as u64))
        && ((c64 >> 63) == 0u64)
        && ((0u64 << 1) | (c64 >> 63)) == 0u64
        && ((0u64 << 1) | (0u64 >> 63)) == 0u64
        && (0u64 >> 63) == 0u64
        && (((code << 1u32) as u64) + (bit as u64) == (next as u64))
    ) by (bit_vector)
        requires code <= 0x7fff_ffffu32, bit <= 1u32, c64 == code as u64, next == ((code << 1u32) | bit);
    let shifted = checked_shift_left_one(super::WideCode { limbs: [c64, 0u64, 0u64, 0u64] }).unwrap();
    assert(ext_equal(shifted.limbs, [(code << 1u32) as u64, 0u64, 0u64, 0u64]));
    let added = checked_add_small(shifted, bit).unwrap();
    assert(ext_equal(added.limbs, [next as u64, 0u64, 0u64, 0u64]));
}

pub(super) proof fn short_distance_matches(code: u32, first: u32, count: u32)
    ensures
        small_distance_from(
            super::WideCode { limbs: [code as u64, 0, 0, 0] },
            super::WideCode { limbs: [first as u64, 0, 0, 0] },
            count,
        ) == if count != 0 && code >= first && (code - first) < count {
            Some((code - first) as u32)
        } else {
            None
        },
{
    reveal(small_distance_from);
    reveal(subtract_limb);
}

pub(super) proof fn bucket_absent_above_max(
    lines: Seq<super::HuffmanLine>,
    length: nat,
    scanned: nat,
    max_len: u8,
)
    requires
        scanned <= lines.len(),
        length > max_len as nat,
        forall|i: int| 0 <= i < lines.len() ==> lines[i].prefix_len <= max_len,
    ensures
        bucket(lines, length, scanned).len() == 0,
    decreases scanned,
{
    if scanned > 0 {
        bucket_absent_above_max(lines, length, (scanned - 1) as nat, max_len);
    }
}

pub(super) proof fn bucket_nonempty_witness(
    lines: Seq<super::HuffmanLine>,
    length: nat,
    scanned: nat,
    witness: int,
)
    requires
        0 <= witness < scanned <= lines.len(),
        length != 0,
        lines[witness].prefix_len == length,
    ensures
        bucket(lines, length, scanned).len() > 0,
    decreases scanned,
{
    if witness < (scanned - 1) as int {
        bucket_nonempty_witness(lines, length, (scanned - 1) as nat, witness);
    }
}

pub(super) proof fn bucket_prefix_bound(
    lines: Seq<super::HuffmanLine>,
    length: nat,
    place: nat,
    total: nat,
)
    requires
        place < total <= lines.len(),
        length != 0,
        lines[place as int].prefix_len == length,
    ensures
        bucket(lines, length, place).len() < bucket(lines, length, total).len(),
    decreases total - place,
{
    if place + 1 < total {
        bucket_prefix_bound(lines, length, place, (total - 1) as nat);
    }
}

pub(super) proof fn bucket_monotone(
    lines: Seq<super::HuffmanLine>,
    length: nat,
    place: nat,
    total: nat,
)
    requires
        place <= total <= lines.len(),
    ensures
        bucket(lines, length, place).len() <= bucket(lines, length, total).len(),
    decreases total - place,
{
    if place < total {
        bucket_monotone(lines, length, place, (total - 1) as nat);
    }
}

pub(super) proof fn ordered_before_disjoint(
    lines: Seq<super::HuffmanLine>,
    a: nat,
    b: nat,
)
    requires
        1 <= a,
        1 <= b,
        a != b,
    ensures
        a < b ==> ordered_before(lines, a).len() + bucket(lines, a, lines.len()).len()
            <= ordered_before(lines, b).len(),
        b < a ==> ordered_before(lines, b).len() + bucket(lines, b, lines.len()).len()
            <= ordered_before(lines, a).len(),
    decreases if a < b { b } else { a },
{
    if a < b {
        if a + 1 < b {
            ordered_before_disjoint(lines, a, (b - 1) as nat);
        }
    } else if b + 1 < a {
        ordered_before_disjoint(lines, (a - 1) as nat, b);
    }
}

}
