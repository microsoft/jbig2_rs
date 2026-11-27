// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: README.md
// All requirement links and inherited locators: ../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

use vstd::prelude::*;

verus! {

// Audit: mmr-mode-table-entry
// T.6 (1988), 2.2.3 / Table 1: https://www.itu.int/rec/T-REC-T.6-198811-I/en
// Membership in implementation data is not an independent normative-table proof.
// Model navigation: spec::mode_code_in_implementation_table [proof-support]
// Purpose: Membership/key equality against implementation mode/run arrays, not table fidelity.
// REQUIREMENT CONTEXT: T6-MODE-TABLE-DISPATCH | ITU-T T.6 (1988) | 2.2.3; 2.2.4; Table 1/T.6
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn mode_code_in_implementation_table(code: super::ModeCode) -> bool {
    super::MODE_CODES@.contains(code)
}

// Model navigation: spec::mode_matches [proof-support]
// Purpose: Membership/key equality against implementation mode/run arrays, not table fidelity.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn mode_matches(
    code: super::ModeCode,
    bits: u16,
    length: u8,
) -> bool {
    code.bits == bits && code.length == length
}

// Audit: mmr-run-table-entry
// T.6 (1988), 2.2.4 / Tables 2-3: https://www.itu.int/rec/T-REC-T.6-198811-I/en
// Existing locator only; the arrays still need independent table-fidelity review.
// Model navigation: spec::run_code_in_implementation_tables [proof-support]
// Purpose: Membership/key equality against implementation mode/run arrays, not table fidelity.
// REQUIREMENT CONTEXT: T6-WHITE-RUN-LOOKUP | ITU-T T.6 (1988) | 2.2.4 step 2iii; Tables 2/T.6 and 3/T.6
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_code_in_implementation_tables(
    black: bool,
    code: super::RunCode,
) -> bool {
    if black {
        super::BLACK_TERMINATING@.contains(code)
            || super::BLACK_MAKEUP@.contains(code)
            || super::COMMON_MAKEUP@.contains(code)
    } else {
        super::WHITE_TERMINATING@.contains(code)
            || super::WHITE_MAKEUP@.contains(code)
            || super::COMMON_MAKEUP@.contains(code)
    }
}

// Model navigation: spec::run_key_matches [proof-support]
// Purpose: Membership/key equality against implementation mode/run arrays, not table fidelity.
// Audit: mmr-run-key
// REQUIREMENT CONTEXT: T6-WHITE-RUN-LOOKUP | ITU-T T.6 (1988) | 2.2.4 step 2iii; Tables 2/T.6 and 3/T.6
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_key_matches(
    code: super::RunCode,
    bits: u16,
    length: u8,
) -> bool {
    code.bits == bits && code.length == length
}

// Model navigation: spec::reader_valid [representation-invariant]
// Purpose: Packed bitmap, bit cursor and ordered transition/sentinel invariants.
// Audit: mmr-reader-valid
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn reader_valid(reader: &crate::reader::BitReader) -> bool {
    reader.bytes@.len() <= usize::MAX / 8
        && reader.bit_position <= reader.bytes@.len() * 8
}

// Model navigation: spec::run_bit_length [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_bit_length(codes: Seq<super::RunCode>) -> nat
    decreases codes.len(),
{
    if codes.len() == 0 { 0 } else {
        run_bit_length(codes.drop_last()) + codes.last().length as nat
    }
}

// Model navigation: spec::run_value_sum [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_value_sum(codes: Seq<super::RunCode>) -> nat
    decreases codes.len(),
{
    if codes.len() == 0 { 0 } else {
        run_value_sum(codes.drop_last()) + codes.last().value as nat
    }
}

// Model navigation: spec::run_prefix_matches [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_prefix_matches(
    black: bool, bytes: Seq<u8>, start: nat, codes: Seq<super::RunCode>,
) -> bool
    decreases codes.len(),
{
    if codes.len() == 0 {
        start <= bytes.len() * 8
    } else {
        let prefix = codes.drop_last();
        let code = codes.last();
        run_prefix_matches(black, bytes, start, prefix)
            && run_code_in_implementation_tables(black, code)
            && start + run_bit_length(codes) <= bytes.len() * 8
            && code.bits as nat == crate::spec::readers::bits_value(
                bytes, start + run_bit_length(prefix), code.length as nat,
            )
    }
}

// Model navigation: spec::run_all_makeups [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_all_makeups(codes: Seq<super::RunCode>) -> bool
    decreases codes.len(),
{
    codes.len() == 0
        || (run_all_makeups(codes.drop_last()) && !codes.last().terminal)
}

// Model navigation: spec::complete_run [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-complete-run
// REQUIREMENT CONTEXT: T6-RUN-MAKEUP-SUM | ITU-T T.6 (1988) | 2.2.4 step 2iii
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn complete_run(
    black: bool, bytes: Seq<u8>, start: nat, end: nat, total: u32,
    codes: Seq<super::RunCode>,
) -> bool {
    codes.len() > 0
        && run_prefix_matches(black, bytes, start, codes)
        && run_all_makeups(codes.drop_last())
        && codes.last().terminal
        && end == start + run_bit_length(codes)
        && total as nat == run_value_sum(codes)
}

// Model navigation: spec::horizontal_pair [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-horizontal-pair
// REQUIREMENT CONTEXT: T6-HORIZONTAL-PAIR | ITU-T T.6 (1988) | 2.2.3.3; 2.2.4 step 2iii
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn horizontal_pair(
    black: bool, bytes: Seq<u8>, start: nat, finish: nat,
    x: u32, width: u32, after_first: u32, end: u32,
) -> bool {
    x <= after_first <= end <= width
        && x < end
        && exists|first: Seq<super::RunCode>, second: Seq<super::RunCode>|
            #[trigger] complete_run(
                black, bytes, start, start + run_bit_length(first),
                (after_first - x) as u32, first,
            )
            && finish == start + run_bit_length(first) + #[trigger] run_bit_length(second)
            && complete_run(
                !black, bytes, start + run_bit_length(first), finish,
                (end - after_first) as u32, second,
            )
}

// Model navigation: spec::transitions_ordered [representation-invariant]
// Purpose: Packed bitmap, bit cursor and ordered transition/sentinel invariants.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn transitions_ordered(
    transitions: Seq<u32>,
    width: u32,
) -> bool {
    forall|index: int| 0 <= index < transitions.len() ==>
        transitions[index] <= width
    && forall|left: int, right: int|
        0 <= left < right < transitions.len() ==>
            transitions[left] <= transitions[right]
}

// Model navigation: spec::reference_line_valid [representation-invariant]
// Purpose: Packed bitmap, bit cursor and ordered transition/sentinel invariants.
// Audit: mmr-reference-line-valid
// REQUIREMENT CONTEXT: T6-REFERENCE-CHANGING-ELEMENTS | ITU-T T.6 (1988) | 2.2.2; 2.2.5.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn reference_line_valid(
    transitions: Seq<u32>,
    width: u32,
) -> bool {
    transitions.len() >= 2
        && transitions[transitions.len() - 2] == width
        && transitions[transitions.len() - 1] == width
        && transitions_ordered(transitions, width)
}

// Model navigation: spec::reference_position_eligible [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-reference-position-eligible
// REQUIREMENT CONTEXT: T6-REFERENCE-CHANGING-ELEMENTS | ITU-T T.6 (1988) | 2.2.2; 2.2.5.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn reference_position_eligible(
    position: u32, x: u32, initial: bool,
) -> bool {
    position > x || (initial && position == x)
}

// Model navigation: spec::first_reference_candidate [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-first-reference-candidate
// REQUIREMENT CONTEXT: T6-REFERENCE-CHANGING-ELEMENTS | ITU-T T.6 (1988) | 2.2.2; 2.2.5.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn first_reference_candidate(
    transitions: Seq<u32>, x: u32, black: bool, initial: bool,
    start: int, index: int,
) -> bool {
    0 <= start <= index < transitions.len() - 1
        && reference_position_eligible(transitions[index], x, initial)
        && (index % 2 == 0) != black
        && forall|previous: int| start <= previous < index ==>
            !reference_position_eligible(#[trigger] transitions[previous], x, initial)
                || (previous % 2 == 0) == black
}

// Model navigation: spec::reference_pair_matches [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-reference-pair
// REQUIREMENT CONTEXT: T6-REFERENCE-CHANGING-ELEMENTS | ITU-T T.6 (1988) | 2.2.2; 2.2.5.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn reference_pair_matches(
    transitions: Seq<u32>, x: u32, black: bool, width: u32, initial: bool,
    start: int, b1: u32, b2: u32,
) -> bool {
    (exists|index: int|
        #[trigger] first_reference_candidate(transitions, x, black, initial, start, index)
            && b1 == transitions[index] && b2 == transitions[index + 1])
        || (!(exists|index: int|
            #[trigger] first_reference_candidate(transitions, x, black, initial, start, index))
            && b1 == width && b2 == width)
}

// Model navigation: spec::span_byte [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn span_byte(
    bitmap: &crate::bitmap::Bitmap,
    row: u32,
    x: u32,
) -> nat {
    row as nat * bitmap.stride as nat + x as nat / 8
}

// Model navigation: spec::bitmap_storage_valid [representation-invariant]
// Purpose: Packed bitmap, bit cursor and ordered transition/sentinel invariants.
// Audit: mmr-storage-valid
// REQUIREMENT CONTEXT: T88-MMR-BINARY-POLARITY | ITU-T T.88 (08/2018) | 6.2.6 second exception
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bitmap_storage_valid(bitmap: &crate::bitmap::Bitmap) -> bool {
    bitmap.width as nat <= bitmap.stride as nat * 8
        && bitmap.data@.len()
            == bitmap.stride as nat * bitmap.height as nat
}

// Model navigation: spec::span_result [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-span-result
// REQUIREMENT CONTEXT: T88-MMR-BINARY-POLARITY | ITU-T T.88 (08/2018) | 6.2.6 second exception
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn span_result(
    before: &crate::bitmap::Bitmap,
    after: &crate::bitmap::Bitmap,
    row: u32,
    start: u32,
    end: u32,
    black: bool,
) -> bool {
    if black {
        crate::spec::bitmap::black_span_result(before, after, row, start, end)
    } else {
        after.data@ == before.data@
    }
}

// Model navigation: spec::transition_count_at [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn transition_count_at(transitions: Seq<u32>, x: u32) -> nat
    decreases transitions.len(),
{
    if transitions.len() == 0 {
        0
    } else {
        transition_count_at(transitions.subrange(0, transitions.len() - 1), x)
            + if transitions[transitions.len() - 1] <= x { 1nat } else { 0nat }
    }
}

// Model navigation: spec::transition_black_at [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Audit: mmr-cache-colour
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn transition_black_at(transitions: Seq<u32>, x: u32) -> bool {
    transition_count_at(transitions, x) % 2 == 1
}

// Model navigation: spec::row_cache_matches [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-row-cache-pixels
// REQUIREMENT CONTEXT: T6-REFERENCE-NEXT-LINE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn row_cache_matches(
    bitmap: &crate::Bitmap, row: u32, transitions: Seq<u32>,
) -> bool {
    row < bitmap.height
        && forall|x: u32| x < bitmap.width ==>
            #[trigger] crate::spec::bitmap::stored_pixel(bitmap, x, row)
                == transition_black_at(transitions, x)
}

// Model navigation: spec::white_reference_cache [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-white-reference-cache
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn white_reference_cache(transitions: Seq<u32>, width: u32) -> bool {
    forall|x: u32| x < width ==> !#[trigger] transition_black_at(transitions, x)
}

// Model navigation: spec::span_pixels [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn span_pixels(
    before: &crate::Bitmap, after: &crate::Bitmap,
    row: u32, start: u32, end: u32, black: bool,
) -> bool {
    forall|x: u32, y: u32| x < before.width && y < before.height ==>
        #[trigger] crate::spec::bitmap::stored_pixel(after, x, y)
            == ((black && y == row && start <= x < end)
                || crate::spec::bitmap::stored_pixel(before, x, y))
}

pub(super) struct RasterState {
    pub(super) ghost position: nat,
    pub(super) ghost x: u32,
    pub(super) ghost black: bool,
    pub(super) ghost transitions: Seq<u32>,
    pub(super) ghost emitted_black: bool,
}

pub(super) struct RasterStep {
    pub(super) ghost before: RasterState,
    pub(super) ghost after: RasterState,
    pub(super) ghost mode: super::Mode,
    pub(super) ghost mode_end: nat,
    pub(super) ghost b1: u32,
    pub(super) ghost b2: u32,
    pub(super) ghost split: u32,
}

pub(super) struct RasterRow {
    pub(super) ghost start: nat,
    pub(super) ghost steps: Seq<RasterStep>,
    pub(super) ghost state: RasterState,
}

// Model navigation: spec::raster_state [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn raster_state(
    position: nat, x: u32, black: bool, transitions: Seq<u32>, emitted_black: bool,
) -> RasterState {
    RasterState { position, x, black, transitions, emitted_black }
}

// Model navigation: spec::transition_span_state [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-transition-span-state
// REQUIREMENT CONTEXT: T6-PASS-TRANSITION | ITU-T T.6 (1988) | 2.2.3.1; 2.2.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn transition_span_state(
    before: Seq<u32>, emitted: bool, start: u32, end: u32, black: bool,
) -> (Seq<u32>, bool) {
    (
        if start < end && emitted != black { before.push(start) } else { before },
        if start < end { black } else { emitted },
    )
}

// Model navigation: spec::mode_prefix_matches [project-result-model]
// Purpose: Consumed run/mode bits, run sums, horizontal pairs and reference selection results.
// Audit: mmr-mode-input-prefix
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn mode_prefix_matches(
    bytes: Seq<u8>, start: nat, end: nat, mode: super::Mode,
) -> bool {
    start <= end <= bytes.len() * 8
        && exists|code: super::ModeCode|
            #[trigger] mode_code_in_implementation_table(code)
                && code.mode == mode
                && end == start + code.length as nat
                && code.bits as nat
                    == crate::spec::readers::bits_value(bytes, start, code.length as nat)
}

// Model navigation: spec::raster_step [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-raster-step
// REQUIREMENT CONTEXT: T6-PASS-TRANSITION | ITU-T T.6 (1988) | 2.2.3.1; 2.2.4 step 1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn raster_step(
    bytes: Seq<u8>, reference: Seq<u32>, width: u32, initial: bool,
    step: RasterStep,
) -> bool {
    let before = step.before;
    let after = step.after;
    &&& before.x < width
    &&& before.x <= after.x <= width
    &&& step.mode_end <= after.position
    &&& mode_prefix_matches(bytes, before.position, step.mode_end, step.mode)
    &&& reference_pair_matches(
        reference, before.x, before.black, width, initial, 0, step.b1, step.b2,
    )
    &&& match step.mode {
        super::Mode::Extension => false,
        super::Mode::Pass =>
            before.x < step.b2
                && after.x == step.b2
                && after.black == before.black
                && after.position == step.mode_end
                && transition_span_result(
                    before.transitions, after.transitions,
                    before.emitted_black, after.emitted_black,
                    before.x, after.x, before.black,
                ),
        super::Mode::Vertical(displacement) =>
            -3 <= displacement <= 3
                && after.x as int == step.b1 as int + displacement as int
                && after.black == !before.black
                && after.position == step.mode_end
                && transition_span_result(
                    before.transitions, after.transitions,
                    before.emitted_black, after.emitted_black,
                    before.x, after.x, before.black,
                ),
        super::Mode::Horizontal => {
            let middle = transition_span_state(
                before.transitions, before.emitted_black,
                before.x, step.split, before.black,
            );
            after.black == before.black
                && horizontal_pair(
                    before.black, bytes, step.mode_end, after.position,
                    before.x, width, step.split, after.x,
                )
                && transition_span_result(
                    middle.0, after.transitions, middle.1, after.emitted_black,
                    step.split, after.x, !before.black,
                )
        },
    }
}

// Model navigation: spec::raster_steps [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-raster-steps
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn raster_steps(
    bytes: Seq<u8>, reference: Seq<u32>, width: u32, start: nat,
    steps: Seq<RasterStep>, state: RasterState,
) -> bool
    decreases steps.len(),
{
    if steps.len() == 0 {
        state == raster_state(start, 0, false, Seq::empty(), false)
    } else {
        let step = steps.last();
        raster_steps(bytes, reference, width, start, steps.drop_last(), step.before)
            && raster_step(bytes, reference, width, steps.len() == 1, step)
            && state == step.after
    }
}

// Model navigation: spec::rows_position [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn rows_position(rows: Seq<RasterRow>) -> nat {
    if rows.len() == 0 { 0 } else { rows.last().state.position }
}

// Model navigation: spec::rows_reference [proof-support]
// Purpose: Raster projections, run folds, packed addresses and transition-parity vocabulary.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn rows_reference(rows: Seq<RasterRow>, width: u32) -> Seq<u32> {
    if rows.len() == 0 {
        Seq::empty().push(width).push(width)
    } else {
        rows.last().state.transitions.push(width).push(width)
    }
}

// Model navigation: spec::raster_rows [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-raster-rows
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn raster_rows(bytes: Seq<u8>, width: u32, rows: Seq<RasterRow>) -> bool
    decreases rows.len(),
{
    if rows.len() == 0 {
        true
    } else {
        let prefix = rows.drop_last();
        let row = rows.last();
        raster_rows(bytes, width, prefix)
            && row.start == rows_position(prefix)
            && raster_steps(
                bytes, rows_reference(prefix, width), width, row.start,
                row.steps, row.state,
            )
            && row.state.x == width
    }
}

// Model navigation: spec::rows_cache_pixels [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn rows_cache_pixels(
    bitmap: &crate::Bitmap, rows: Seq<RasterRow>,
) -> bool {
    crate::spec::bitmap::storage_valid(bitmap)
        && rows.len() <= bitmap.height
        && forall|row: int| 0 <= row < rows.len() ==>
            row_cache_matches(
                bitmap, row as u32,
                #[trigger] rows[row].state.transitions.push(bitmap.width).push(bitmap.width),
            )
}

// Model navigation: spec::bitmap_decodes [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-input-bitmap
// REQUIREMENT CONTEXT: T6-REFERENCE-FIRST-WHITE | ITU-T T.6 (1988) | 2.2.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bitmap_decodes(
    bytes: Seq<u8>, width: u32, height: u32, bitmap: &crate::Bitmap, end: nat,
) -> bool {
    bitmap.width == width && bitmap.height == height
        && crate::spec::bitmap::storage_valid(bitmap)
        && exists|rows: Seq<RasterRow>|
            rows.len() == height
                && #[trigger] raster_rows(bytes, width, rows)
                && rows_cache_pixels(bitmap, rows)
                && end == rows_position(rows)
}

// Model navigation: spec::required_eofb_prefix [project-result-model]
// Purpose: Immediate implementation EOFB marker and bounded zero alignment result.
// Audit: mmr-required-eofb-prefix
// REQUIREMENT CONTEXT: T6-EOFB-PADDING | ITU-T T.6 (1988) | 2.4.1.2
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn required_eofb_prefix(
    bytes: Seq<u8>, start: nat, end: nat,
) -> bool {
    start + 24 <= end <= start + 31
        && end <= bytes.len() * 8
        && end % 8 == 0
        && exact_eofb(bytes, start)
        && crate::spec::readers::bits_value(
            bytes, start + 24, (end - start - 24) as nat,
        ) == 0
}

// Model navigation: spec::bounded_bitmap_decodes [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-bounded-bitmap
// REQUIREMENT CONTEXT: T88-MMR-BINARY-POLARITY | ITU-T T.88 (08/2018) | 6.2.6 second exception
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn bounded_bitmap_decodes(
    bytes: Seq<u8>, width: u32, height: u32, bitmap: &crate::Bitmap,
    policy: super::EofbPolicy,
) -> bool {
    exists|end: nat| #[trigger] bitmap_decodes(bytes, width, height, bitmap, end)
        && match policy {
            super::EofbPolicy::Optional => true,
            super::EofbPolicy::Required => required_eofb_prefix(bytes, end, bytes.len() * 8),
        }
}

// Model navigation: spec::prefix_bitmap_decodes [project-result-model]
// Purpose: Transition-span effects, parity pixels and finite chained raster output histories.
// Audit: mmr-prefix-bitmap
// REQUIREMENT CONTEXT: T6-EOFB-PADDING | ITU-T T.6 (1988) | 2.4.1.2
// Fidelity: unresolved; no independent normative approval.
pub(crate) closed spec fn prefix_bitmap_decodes(
    bytes: Seq<u8>, width: u32, height: u32, bitmap: &crate::Bitmap, consumed: usize,
) -> bool {
    exists|end: nat| #[trigger] bitmap_decodes(bytes, width, height, bitmap, end)
        && required_eofb_prefix(bytes, end, consumed as nat * 8)
}

// Model navigation: spec::exact_eofb [project-result-model]
// Purpose: Immediate implementation EOFB marker and bounded zero alignment result.
// Audit: mmr-exact-eofb
// REQUIREMENT CONTEXT: T6-EOFB-MARKER | ITU-T T.6 (1988) | 2.4.1.1
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn exact_eofb(
    bytes: Seq<u8>,
    start: nat,
) -> bool {
    crate::spec::readers::bits_value(bytes, start, 24) == super::EOFB
}

// Model navigation: spec::run_code_eof_result [project-policy]
// Purpose: Truncated horizontal run-code EOF and typed error boundary for T.6 decoding.
// Audit: mmr-truncated-run-code
// REQUIREMENT CONTEXT: T6-POLICY-TRUNCATED-HORIZONTAL | ITU-T T.6 (1988) | 2.2.3.3; 2.2.4 (syntax context; typed error is project API policy)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn run_code_eof_result(
    black: bool,
    bytes: Seq<u8>,
    start: nat,
    end: nat,
    result: Result<super::RunCode, super::MmrError>,
) -> bool {
    (start == bytes.len() * 8 ==> (
        result == Err(super::MmrError::Reader(crate::MalformedError::UnexpectedEof {
            offset: start as u64,
            needed: 1,
            remaining: 0,
        }))
        && end == start
    ))
        && match result {
            Ok(_) => true,
            Err(error) => match error {
                super::MmrError::Reader(crate::MalformedError::UnexpectedEof { .. }) => true,
                super::MmrError::InvalidRunCode { bit_offset, black: err_black } =>
                    bit_offset as nat == start && err_black == black,
                super::MmrError::IntegerOverflow => true,
                _ => false,
            },
        }
}

// Model navigation: spec::bounded_run_result [project-policy]
// Purpose: Row-remainder bounding and overshoot rejection for decoded T.6 runs.
// Audit: mmr-bounded-run-result
// REQUIREMENT CONTEXT: T6-POLICY-RUN-OVERSHOOT | ITU-T T.6 (1988) | 2.2.5.2 (line-extent context; typed error is project API policy)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn bounded_run_result(
    black: bool,
    bytes: Seq<u8>,
    start: nat,
    end: nat,
    remaining: u32,
    result: Result<u32, super::MmrError>,
) -> bool {
    match result {
        Ok(total) => total <= remaining,
        Err(error) => match error {
            super::MmrError::RunExceedsRow
            | super::MmrError::Reader(crate::MalformedError::UnexpectedEof { .. })
            | super::MmrError::InvalidRunCode { .. }
            | super::MmrError::IntegerOverflow => true,
            _ => false,
        },
    }
}

// Model navigation: spec::finish_block_eofb_policy [project-policy]
// Purpose: Complete EOFB marker, truncation, trailing-data and zero-padding classification for bounded blocks.
// Audit: mmr-finish-block-eofb-policy
// REQUIREMENT CONTEXT: T6-POLICY-TRUNCATED-EOFB | ITU-T T.6 (1988) | 2.4.1.1 (marker context; typed error is project API policy)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn finish_block_eofb_policy(
    bytes: Seq<u8>,
    start: nat,
    end: nat,
    policy: super::EofbPolicy,
    result: Result<(), super::MmrError>,
) -> bool {
    match policy {
        super::EofbPolicy::Optional => result == Ok(()) && end == start,
        super::EofbPolicy::Required => {
            let remaining = (bytes.len() * 8 - start) as int;
            if remaining < 24 {
                result == Err(super::MmrError::MissingEofb) && end == start
            } else if !exact_eofb(bytes, start) {
                result == Err(super::MmrError::InvalidEofb) && end == start + 24
            } else if remaining - 24 > 7 {
                result == Err(super::MmrError::TrailingData) && end == start + 24
            } else if remaining - 24 > 0
                && crate::spec::readers::bits_value(bytes, start + 24, (remaining - 24) as nat) != 0
            {
                result == Err(super::MmrError::NonZeroEofbPadding) && end == bytes.len() * 8
            } else {
                result == Ok(())
                    && end == bytes.len() * 8
                    && exact_eofb(bytes, start)
                    && required_eofb_prefix(bytes, start, end)
            }
        }
    }
}

// Model navigation: spec::finish_prefix_eofb_policy [project-policy]
// Purpose: Complete EOFB marker, truncation and byte-alignment padding classification for required prefixes.
// Audit: mmr-finish-prefix-eofb-policy
// REQUIREMENT CONTEXT: T6-POLICY-TRUNCATED-EOFB | ITU-T T.6 (1988) | 2.4.1.1 (marker context; typed error is project API policy)
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn finish_prefix_eofb_policy(
    bytes: Seq<u8>,
    start: nat,
    end: nat,
    result: Result<(), super::MmrError>,
) -> bool {
    let remaining = (bytes.len() * 8 - start) as int;
    if remaining < 24 {
        result == Err(super::MmrError::MissingEofb) && end == start
    } else if !exact_eofb(bytes, start) {
        result == Err(super::MmrError::InvalidEofb) && end == start + 24
    } else {
        let after_marker = start + 24;
        let padding: nat = if after_marker % 8 == 0 { 0 } else { (8 - (after_marker % 8)) as nat };
        if bytes.len() * 8 < after_marker + padding {
            result == Err(super::MmrError::MissingEofb) && end == after_marker
        } else {
            end == after_marker + padding
                && match result {
                    Ok(()) => true,
                    Err(error) => error == super::MmrError::NonZeroEofbPadding,
                }
        }
    }
}

}

// Private helper preconditions still use mmr_spec::transition_prefix_valid.
include!("../proofs/model_support/mmr.rs");
