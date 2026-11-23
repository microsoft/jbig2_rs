// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Model navigation: spec::zero_padding_before_prefix [project-policy]
// Purpose: Sub-byte zero padding too short to contain any Huffman table prefix.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn zero_padding_before_prefix(
    table: &super::HuffmanTable, reader: &crate::reader::BitReader,
) -> bool {
    let total = reader.bytes@.len() * 8;
    let remaining = total - reader.bit_position;
    &&& reader_valid(reader)
    &&& remaining < 8
    &&& forall|length: int| 1 <= length <= remaining ==> prefix_count(table, length) == 0
    &&& crate::spec::readers::bits_value(
        reader.bytes@, reader.bit_position as nat, remaining as nat,
    ) == 0
}

// Model navigation: spec::prefix_count [proof-support]
// Purpose: Exact cached count of canonical prefixes of a given length.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn prefix_count(table: &super::HuffmanTable, length: int) -> u32 {
    table.length_count@[length]
}

pub(super) proof fn prefix_count_definition(table: &super::HuffmanTable)
    ensures forall|length: int| 0 <= length < 256 ==>
        prefix_count(table, length) == table.length_count@[length],
{
    reveal(prefix_count);
}

// Model navigation: spec::huffman_error_result [project-result-model]
// Purpose: Reuse the exact existing wrapped-error or invalid-table conversion policy.
// Audit: huffman-error-result
// REQUIREMENT CONTEXT: T88-PROFILE-SYMBOL-ID-PRESERVATION | ITU-T T.88 (08/2018) | 0.2 (informative context); 6.4.10 (index decoding context)
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn huffman_error_result(error: &super::HuffmanError) -> crate::DecodeError {
    match *error {
        super::HuffmanError::Decode(error) => error,
        super::HuffmanError::InvalidTable(kind) =>
            crate::DecodeError::Malformed(crate::MalformedError::InvalidHuffman(kind)),
    }
}

pub(crate) proof fn huffman_error_result_definition(error: &super::HuffmanError)
    ensures huffman_error_result(error) == match *error {
        super::HuffmanError::Decode(error) => error,
        super::HuffmanError::InvalidTable(kind) =>
            crate::DecodeError::Malformed(crate::MalformedError::InvalidHuffman(kind)),
    },
{
    reveal(huffman_error_result);
}

// Model navigation: spec::pow2 [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn pow2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * pow2((exponent - 1) as nat)
    }
}

// Model navigation: spec::wide_value [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn wide_value(code: super::WideCode) -> nat {
    code.limbs@[0] as nat
        + code.limbs@[1] as nat * pow2(64)
        + code.limbs@[2] as nat * pow2(128)
        + code.limbs@[3] as nat * pow2(192)
}

// Model navigation: spec::wide_fits [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn wide_fits(
    code: super::WideCode,
    bit_count: usize,
) -> bool {
    bit_count >= 256 || wide_value(code) < pow2(bit_count as nat)
}

// Model navigation: spec::add_limb [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn add_limb(left: u64, right: u64) -> (u64, bool) {
    if right <= u64::MAX - left {
        ((left + right) as u64, false)
    } else {
        ((right - (u64::MAX - left) - 1) as u64, true)
    }
}

// Model navigation: spec::checked_add_small [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn checked_add_small(
    code: super::WideCode,
    value: u32,
) -> Option<super::WideCode> {
    let first = add_limb(code.limbs@[0], value as u64);
    if !first.1 {
        Some(super::WideCode {
            limbs: [first.0, code.limbs@[1], code.limbs@[2], code.limbs@[3]],
        })
    } else if code.limbs@[1] != u64::MAX {
        Some(super::WideCode {
            limbs: [first.0, (code.limbs@[1] + 1) as u64, code.limbs@[2], code.limbs@[3]],
        })
    } else if code.limbs@[2] != u64::MAX {
        Some(super::WideCode {
            limbs: [first.0, 0, (code.limbs@[2] + 1) as u64, code.limbs@[3]],
        })
    } else if code.limbs@[3] != u64::MAX {
        Some(super::WideCode {
            limbs: [first.0, 0, 0, (code.limbs@[3] + 1) as u64],
        })
    } else {
        None
    }
}

// Model navigation: spec::checked_shift_left_one [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn checked_shift_left_one(
    code: super::WideCode,
) -> Option<super::WideCode> {
    if code.limbs@[3] >> 63 != 0 {
        None
    } else {
        Some(super::WideCode {
            limbs: [
                code.limbs@[0] << 1,
                (code.limbs@[1] << 1) | (code.limbs@[0] >> 63),
                (code.limbs@[2] << 1) | (code.limbs@[1] >> 63),
                (code.limbs@[3] << 1) | (code.limbs@[2] >> 63),
            ],
        })
    }
}

// Model navigation: spec::append_bit [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn append_bit(
    code: super::WideCode,
    bit: u32,
) -> Option<super::WideCode> {
    match checked_shift_left_one(code) {
        Some(shifted) => checked_add_small(shifted, bit),
        None => None,
    }
}

// Model navigation: spec::fits_in_bits [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn fits_in_bits(
    code: super::WideCode,
    bit_count: usize,
) -> bool {
    if bit_count >= 256 {
        true
    } else {
        let limb = bit_count / 64;
        let bit = bit_count % 64;
        !(limb < 3 && code.limbs@[3] != 0)
            && !(limb < 2 && code.limbs@[2] != 0)
            && !(limb < 1 && code.limbs@[1] != 0)
            && code.limbs@[limb as int] >> bit == 0
    }
}

// Model navigation: spec::subtract_limb [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn subtract_limb(
    left: u64,
    right: u64,
    borrow: bool,
) -> (u64, bool) {
    let borrow_value = if borrow { 1u64 } else { 0u64 };
    let first = if left >= right {
        ((left - right) as u64, false)
    } else {
        ((u64::MAX - (right - left) + 1) as u64, true)
    };
    let second = if first.0 >= borrow_value {
        ((first.0 - borrow_value) as u64, false)
    } else {
        (u64::MAX, true)
    };
    (second.0, first.1 || second.1)
}

// Model navigation: spec::small_distance_from [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn small_distance_from(
    left: super::WideCode,
    right: super::WideCode,
    upper_bound: u32,
) -> Option<u32> {
    let d0 = subtract_limb(left.limbs@[0], right.limbs@[0], false);
    let d1 = subtract_limb(left.limbs@[1], right.limbs@[1], d0.1);
    let d2 = subtract_limb(left.limbs@[2], right.limbs@[2], d1.1);
    let d3 = subtract_limb(left.limbs@[3], right.limbs@[3], d2.1);
    if d3.1 || d1.0 != 0 || d2.0 != 0 || d3.0 != 0 || d0.0 > u32::MAX {
        None
    } else if d0.0 < upper_bound {
        Some(d0.0 as u32)
    } else {
        None
    }
}

// Model navigation: spec::reader_valid [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Audit: huffman-reader-valid
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn reader_valid(reader: &crate::reader::BitReader) -> bool {
    reader.bytes@.len() <= usize::MAX / 8
        && reader.bit_position <= reader.bytes@.len() * 8
}

// Model navigation: spec::line_decodable [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn line_decodable(line: super::HuffmanLine) -> bool {
    line.range_len <= 32
        && match line.kind {
            super::LineKind::Normal | super::LineKind::Upper =>
                i32::MIN <= line.range_low
                && line.range_low
                    + (pow2(line.range_len as nat) - 1) as int <= i32::MAX,
            super::LineKind::Lower =>
                i32::MIN <= line.range_low
                    - (pow2(line.range_len as nat) - 1) as int
                && line.range_low <= i32::MAX,
            super::LineKind::OutOfBand => line.range_len == 0,
        }
}

// Model navigation: spec::decoded_line_value [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Audit: huffman-decoded-value
// REQUIREMENT CONTEXT: T88-HUFFMAN-NORMAL-VALUE | ITU-T T.88 (08/2018) | B.4 steps 2 and 5
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn decoded_line_value(
    line: super::HuffmanLine,
    offset: nat,
    value: i32,
) -> bool {
    match line.kind {
        super::LineKind::Lower =>
            value as int == line.range_low - offset as int,
        super::LineKind::Normal | super::LineKind::Upper =>
            value as int == line.range_low + offset as int,
        super::LineKind::OutOfBand => false,
    }
}

// Model navigation: spec::canonical_first_code [project-result-model]
// Purpose: Checked wide-code arithmetic, first-prefix recurrence and signed value results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn canonical_first_code(
    previous: super::WideCode,
    count: u32,
) -> Option<super::WideCode> {
    match checked_add_small(previous, count) {
        Some(sum) => checked_shift_left_one(sum),
        None => None,
    }
}

// Model navigation: spec::canonical_range_fits [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn canonical_range_fits(
    first: super::WideCode,
    count: u32,
    length: usize,
) -> bool {
    match checked_add_small(first, (count - 1) as u32) {
        Some(last) => fits_in_bits(last, length),
        None => false,
    }
}

// Model navigation: spec::line_lengths_valid [representation-invariant]
// Purpose: Reader, line-width, wide-code and assigned-prefix representation invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn line_lengths_valid(lines: Seq<super::HuffmanLine>) -> bool {
    forall|index: int| 0 <= index < lines.len() ==>
        lines[index].range_len <= 32
}

// Model navigation: spec::is_raw [proof-support]
// Purpose: Assignment-source projection for borrowed lines and the four standard-A lines.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn is_raw(source: super::HuffmanLines) -> bool {
    match source {
        super::HuffmanLines::Borrowed(_) => true,
        super::HuffmanLines::Standard(super::StandardLines::A) => true,
        _ => false,
    }
}

// Model navigation: spec::raw_lines [proof-support]
// Purpose: Exact lines projected from borrowed sources or the standard-A source.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn raw_lines(source: super::HuffmanLines) -> Seq<super::HuffmanLine> {
    match source {
        super::HuffmanLines::Borrowed(lines) => lines@,
        super::HuffmanLines::Standard(super::StandardLines::A) => super::TABLE_A@,
        _ => Seq::empty(),
    }
}

pub(crate) closed spec fn standard_a_lines() -> Seq<super::HuffmanLine> {
    super::TABLE_A@
}

pub(super) proof fn standard_a_lines_definition()
    ensures standard_a_lines() == super::TABLE_A@,
{
    reveal(standard_a_lines);
}

// Select original indices, not the constructor's histogram or sorted storage.
// Model navigation: spec::bucket [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bucket(
    lines: Seq<super::HuffmanLine>, length: nat, scanned: nat,
) -> Seq<u32>
    recommends scanned <= lines.len(),
    decreases scanned,
{
    if scanned == 0 {
        Seq::empty()
    } else {
        let earlier = bucket(lines, length, (scanned - 1) as nat);
        if length != 0 && lines[scanned - 1].prefix_len == length {
            earlier.push((scanned - 1) as u32)
        } else {
            earlier
        }
    }
}

// Model navigation: spec::ordered_before [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn ordered_before(
    lines: Seq<super::HuffmanLine>, length: nat,
) -> Seq<u32>
    decreases length,
{
    if length <= 1 {
        Seq::empty()
    } else {
        ordered_before(lines, (length - 1) as nat)
            + bucket(lines, (length - 1) as nat, lines.len())
    }
}

// Model navigation: spec::first_prefix [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn first_prefix(
    lines: Seq<super::HuffmanLine>, length: nat,
) -> Option<super::WideCode>
    decreases length,
{
    if length == 0 {
        Some(super::WideCode { limbs: [0, 0, 0, 0] })
    } else {
        match first_prefix(lines, (length - 1) as nat) {
            Some(previous) => {
                let count = bucket(lines, (length - 1) as nat, lines.len()).len();
                if count <= u32::MAX {
                    canonical_first_code(previous, count as u32)
                } else {
                    None
                }
            },
            None => None,
        }
    }
}

// Model navigation: spec::histogram [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn histogram(
    lines: Seq<super::HuffmanLine>, counts: Seq<u32>, scanned: nat,
) -> bool {
    scanned <= lines.len() && counts.len() == 256
        && forall|length: int| 0 <= length < 256 ==>
            counts[length] == bucket(lines, length as nat, scanned).len()
}

// Model navigation: spec::assignment_prefix [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn assignment_prefix(
    lines: Seq<super::HuffmanLine>,
    codes: Seq<super::WideCode>, starts: Seq<u32>, order: Seq<u32>, next: nat,
) -> bool {
    codes.len() == 256 && starts.len() == 256 && 1 <= next <= 256
        && codes[0].limbs@ == seq![0u64, 0u64, 0u64, 0u64]
        && order == ordered_before(lines, next)
        && forall|length: int| 1 <= length < next ==>
            #[trigger] first_prefix(lines, length as nat) == Some(codes[length])
            && (bucket(lines, length as nat, lines.len()).len() > 0 ==>
                starts[length] == ordered_before(lines, length as nat).len()
                && canonical_range_fits(
                    codes[length],
                    bucket(lines, length as nat, lines.len()).len() as u32,
                    length as usize,
                ))
}

// Model navigation: spec::assignment_contents [project-result-model]
// Purpose: Canonical assignment from raw-line histogram, prefix recurrence and stable order.
// Audit: huffman-assignment-contents
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn assignment_contents(
    lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable,
) -> bool {
    table.lines@ == lines
        && histogram(lines, table.length_count@, lines.len())
        && 1 <= table.max_prefix_len <= 255
        && table.length_count@[table.max_prefix_len as int] > 0
        && (forall|length: int| table.max_prefix_len < length < 256 ==>
            table.length_count@[length] == 0)
        && table.ordered_lines@.len()
            == ordered_before(lines, table.max_prefix_len as nat + 1).len()
}

// Model navigation: spec::assignment_codes [project-result-model]
// Purpose: Canonical assignment from raw-line histogram, prefix recurrence and stable order.
// Audit: huffman-assignment-codes
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn assignment_codes(
    lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable,
) -> bool {
    table.first_code@[0].limbs@ == seq![0u64, 0u64, 0u64, 0u64]
        && forall|length: int| 1 <= length <= table.max_prefix_len ==>
            #[trigger] first_prefix(lines, length as nat) == Some(table.first_code@[length])
            && (table.length_count@[length] > 0 ==>
                canonical_range_fits(table.first_code@[length], table.length_count@[length], length as usize))
}

// Model navigation: spec::assignment_starts [project-result-model]
// Purpose: Canonical assignment from raw-line histogram, prefix recurrence and stable order.
// Audit: huffman-assignment-starts
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn assignment_starts(
    lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable,
) -> bool {
    forall|length: int| 1 <= length <= table.max_prefix_len
        && table.length_count@[length] > 0 ==>
            table.first_line@[length] == ordered_before(lines, length as nat).len()
}

// Model navigation: spec::assignment_order [project-result-model]
// Purpose: Canonical assignment from raw-line histogram, prefix recurrence and stable order.
// Audit: huffman-assignment-order
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn assignment_order(
    lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable,
) -> bool {
    table.ordered_lines@ == ordered_before(lines, table.max_prefix_len as nat + 1)
}

// Model navigation: spec::canonical_assignment [project-result-model]
// Purpose: Canonical assignment from raw-line histogram, prefix recurrence and stable order.
// Audit: huffman-canonical-assignment
// REQUIREMENT CONTEXT: T88-HUFFMAN-CANONICAL-ASSIGNMENT | ITU-T T.88 (08/2018) | B.2 step 11; B.3 steps 1-3
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn canonical_assignment(
    lines: Seq<super::HuffmanLine>, table: &super::HuffmanTable,
) -> bool {
    assignment_contents(lines, table) && assignment_codes(lines, table)
        && assignment_starts(lines, table) && assignment_order(lines, table)
}

// Model navigation: spec::line_state [project-result-model]
// Purpose: Exact selected-line result and cursor after value, OOB or typed failure.
// Audit: huffman-line-state
// REQUIREMENT CONTEXT: T88-HUFFMAN-NORMAL-VALUE | ITU-T T.88 (08/2018) | B.4 steps 2 and 5
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn line_state(
    line: super::HuffmanLine, bytes: Seq<u8>, position: usize,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
{
    if line.kind == super::LineKind::OutOfBand {
        (Ok(super::HuffmanValue::OutOfBand), position)
    } else {
        match crate::spec::readers::bit_read_error(bytes, position, line.range_len) {
            Some(error) => (
                Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(error))), position,
            ),
            None => {
                let offset = crate::spec::readers::bits_value(
                    bytes, position as nat, line.range_len as nat,
                ) as int;
                let value = if line.kind == super::LineKind::Lower {
                    line.range_low as int - offset
                } else { line.range_low as int + offset };
                (
                    if i32::MIN as int <= value <= i32::MAX as int {
                        Ok(super::HuffmanValue::Integer(value as i32))
                    } else {
                        Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(
                            crate::MalformedError::IntegerOverflow,
                        )))
                    },
                    (position + line.range_len as nat) as usize,
                )
            },
        }
    }
}

// Model navigation: spec::fallback_run [project-result-model]
// Purpose: Input-bit, checked-wide-prefix, bucket and stored-line fallback state.
// Audit: huffman-fallback-run
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(super) closed spec fn fallback_run(
    table: &super::HuffmanTable, bytes: Seq<u8>, position: usize,
    length: usize, code: super::WideCode,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends
        bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
        1 <= length <= 256,
    decreases if length <= 256 {
        (257 - length) as nat
    } else { 0nat },
{
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
                        prefix, table.first_code@[length as int], table.length_count@[length as int],
                    ) {
                        None => fallback_run(table, bytes, next, (length + 1) as usize, prefix),
                        Some(distance) => {
                            let ordered = table.first_line@[length as int] as nat + distance as nat;
                            if ordered > u32::MAX {
                                (
                                    Err(super::HuffmanError::Decode(crate::DecodeError::Malformed(
                                        crate::MalformedError::IntegerOverflow,
                                    ))), next,
                                )
                            } else if ordered >= table.ordered_lines@.len() {
                                (
                                    Err(super::HuffmanError::InvalidTable(
                                        super::HuffmanTableError::InvalidCode,
                                    )), next,
                                )
                            } else {
                                let index = table.ordered_lines@[ordered as int] as nat;
                                if index >= table.lines@.len() {
                                    (
                                        Err(super::HuffmanError::InvalidTable(
                                            super::HuffmanTableError::InvalidCode,
                                        )), next,
                                    )
                                } else { line_state(table.lines@[index as int], bytes, next) }
                            }
                        },
                    },
                }
            },
        }
    }
}

// Model navigation: spec::decode_state [project-result-model]
// Purpose: Complete cached-or-fallback input-table result and cursor dispatch.
// Audit: huffman-decode-state
// REQUIREMENT CONTEXT: T88-HUFFMAN-PREFIX-MATCH | ITU-T T.88 (08/2018) | B.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn decode_state(
    table: &super::HuffmanTable, bytes: Seq<u8>, position: usize,
) -> (Result<super::HuffmanValue, super::HuffmanError>, usize)
    recommends bytes.len() <= usize::MAX / 8, position <= bytes.len() * 8,
{
    if table.prefer_prefix_lookup && position + super::PREFIX_LOOKUP_BITS as nat
        <= bytes.len() * 8 {
        let index = position / 8;
        let offset = position % 8;
        let prefix = if offset == 0 { bytes[index as int] } else {
            let first = bytes[index as int] as u16;
            let second = bytes[(index + 1) as int] as u16;
            ((((first << 8) | second) >> ((8 - offset) as usize)) & 0xff) as u8
        };
        let entry = if (prefix as nat) < table.prefix_lookup@.len() {
            table.prefix_lookup@[prefix as int]
        } else { 0u16 };
        if entry == 0 {
            fallback_run(table, bytes, position, 1, super::WideCode { limbs: [0u64, 0, 0, 0] })
        } else {
            let index = entry as int - 1;
            if index >= table.lines@.len() {
                (Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode)), position)
            } else {
                let line = table.lines@[index];
                if line.prefix_len > super::PREFIX_LOOKUP_BITS {
                    (Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode)), position)
                } else { line_state(line, bytes, (position + line.prefix_len as nat) as usize) }
            }
        }
    } else {
        fallback_run(table, bytes, position, 1, super::WideCode { limbs: [0u64, 0, 0, 0] })
    }
}

// Model navigation: spec::cached_decode_outcome [project-result-model]
// Purpose: Input-indexed cached-line selection and exact value/error/cursor composition.
// Audit: huffman-cached-decode-outcome
// REQUIREMENT CONTEXT: T88-HUFFMAN-NORMAL-VALUE | ITU-T T.88 (08/2018) | B.4 steps 2 and 5
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn cached_decode_outcome(
    table: &super::HuffmanTable,
    before: &crate::reader::BitReader,
    after: &crate::reader::BitReader,
    out: Result<super::HuffmanValue, super::HuffmanError>,
) -> bool
    recommends reader_valid(before),
{
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
        let entry = if (prefix as nat) < table.prefix_lookup@.len() {
            table.prefix_lookup@[prefix as int]
        } else { 0u16 };
        entry != 0 ==> {
            let line_index = entry as int - 1;
            &&& after.bytes@ == before.bytes@
            &&& if line_index >= table.lines@.len() {
                out == Err(super::HuffmanError::InvalidTable(super::HuffmanTableError::InvalidCode))
                    && after.bit_position == before.bit_position
            } else {
                let line = table.lines@[line_index];
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
    }
}

// Model navigation: spec::table_has_oob [proof-support]
// Purpose: Borrowed-source projections, powers, bucket indices and assignment accumulators.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn table_has_oob(table: &super::HuffmanTable) -> bool {
    exists|index: int| 0 <= index < table.lines@.len()
        && table.lines@[index].kind == super::LineKind::OutOfBand
}

}

// Keep the closed-model reveal in its owning module.
include!("../proofs/model_support/huffman.rs");
