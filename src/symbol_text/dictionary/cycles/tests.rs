use super::*;
use crate::symbol_text::IntegerContexts;

fn codec(bytes: &[u8]) -> ArithmeticCodec<'_> {
    ArithmeticCodec {
        decoder: ArithmeticDecoder::new(bytes).unwrap(),
        integers: IntegerContexts {
            slots: [[ArithmeticContext::new(); 512]; crate::symbol_text::INTEGER_FIELD_COUNT],
        },
        iaid: vec![ArithmeticContext::new()],
        refinement: None,
        refinement_work: 0,
    }
}

#[test]
fn checkpoints_compare_entire_banks_not_only_refresh_keys() {
    let bytes = [0, 0, 0, 0];
    for mode in [CycleMode::ExportRuns, CycleMode::EmptyHeightClasses] {
        let mut current = codec(&bytes);
        let checkpoint = Checkpoint::capture(&current, mode, (3, 7, false));
        assert!(checkpoint.matches(&current, mode, (3, 7, false)));
        for field in mode.fields() {
            for index in [0, 1, 255, 511] {
                let slot = field.slot();
                current.integers.slots[slot][index].state_index = 1;
                assert!(!checkpoint.matches(&current, mode, (3, 7, false)));
                current.integers.slots[slot][index].state_index = 0;
                current.integers.slots[slot][index].mps = 1;
                assert!(!checkpoint.matches(&current, mode, (3, 7, false)));
                current.integers.slots[slot][index].mps = 0;
            }
        }
        assert!(!checkpoint.matches(&current, mode, (4, 7, false)));
        assert!(!checkpoint.matches(&current, mode, (3, 8, false)));
        assert!(!checkpoint.matches(&current, mode, (3, 7, true)));
        assert!(checkpoint.matches(&current, mode, (3, 7, false)));
    }
}

#[test]
fn checkpoints_reject_changed_registers_and_same_sized_input() {
    let bytes = [0, 0, 0, 0];
    let changed = [0, 0, 0, 1];
    let mut current = codec(&bytes);
    let checkpoint = Checkpoint::capture(&current, CycleMode::ExportRuns, (0, 0, false));
    current.decoder.code += 1;
    assert!(!checkpoint.matches(&current, CycleMode::ExportRuns, (0, 0, false)));
    current.decoder.code -= 1;
    current.decoder.interval += 1;
    assert!(!checkpoint.matches(&current, CycleMode::ExportRuns, (0, 0, false)));
    current.decoder.interval -= 1;
    current.decoder.bit_count += 1;
    assert!(!checkpoint.matches(&current, CycleMode::ExportRuns, (0, 0, false)));
    current.decoder.bit_count -= 1;
    current.decoder.byte_position = 0;
    assert!(!checkpoint.matches(&current, CycleMode::ExportRuns, (0, 0, false)));
    current.decoder.byte_position = 1;
    current.decoder.bytes = &changed;
    assert!(!checkpoint.matches(&current, CycleMode::ExportRuns, (0, 0, false)));
}
