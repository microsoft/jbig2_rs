//! Exact checkpoints for non-progressing arithmetic dictionary loops.

#[cfg(verus_keep_ghost)]
use super::super::symbol_text_spec::{self, cycles as cycle_spec};
use super::super::{ArithmeticCodec, ArithmeticContext, ArithmeticDecoder, IntegerField};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

// Optimization activation only: smaller budgets retain the ordinary loop.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) const EXPORT_CYCLE_MIN_WORK: u64 = 1 << 17;
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) const HEIGHT_CYCLE_MIN_WORK: u64 = 1 << 16;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) enum CycleMode {
    ExportRuns,
    EmptyHeightClasses,
}

#[cfg(test)]
mod tests;

impl CycleMode {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out[0] as usize == cycle_spec::fields(self).0,
            out[1] as usize == cycle_spec::fields(self).1,
    ))]
    fn fields(self) -> [IntegerField; 2] {
        match self {
            Self::ExportRuns => [IntegerField::ExportRun, IntegerField::ExportRun],
            Self::EmptyHeightClasses => [IntegerField::HeightDelta, IntegerField::WidthDelta],
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) struct Checkpoint<'a> {
    pub(in crate::symbol_text) decoder: ArithmeticDecoder<'a>,
    pub(in crate::symbol_text) banks: [[ArithmeticContext; 512]; 2],
    pub(in crate::symbol_text) cursor: (u32, u32, bool),
}

impl<'a> Checkpoint<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::arithmetic_codec_valid(codec),
        ensures cycle_spec::checkpoint_state(&out) == cycle_spec::state(codec, mode, cursor),
            cycle_spec::state_valid(cycle_spec::checkpoint_state(&out)),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[cold]
    fn capture(codec: &ArithmeticCodec<'a>, mode: CycleMode, cursor: (u32, u32, bool)) -> Self {
        let fields = mode.fields();
        Self {
            decoder: ArithmeticDecoder {
                bytes: codec.decoder.bytes,
                byte_position: codec.decoder.byte_position,
                current_byte: codec.decoder.current_byte,
                code: codec.decoder.code,
                interval: codec.decoder.interval,
                bit_count: codec.decoder.bit_count,
                marker: codec.decoder.marker,
            },
            banks: [
                codec.integers.slots[fields[0].slot()],
                codec.integers.slots[fields[1].slot()],
            ],
            cursor,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == cycle_spec::prefix_equal(self, codec, mode),
    ))]
    #[cold]
    fn prefix_matches(&self, codec: &ArithmeticCodec<'_>, mode: CycleMode) -> bool {
        let fields = mode.fields();
        let mut bank = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant bank <= 2,
                fields@[0] as usize == cycle_spec::fields(mode).0,
                fields@[1] as usize == cycle_spec::fields(mode).1,
                forall|done: int, key: int| 0 <= done < bank && 0 <= key < 4 ==>
                    self.banks[done][cycle_spec::prefix_keys(mode, done)[key]]
                        == codec.integers.slots[if done == 0 { cycle_spec::fields(mode).0 as int }
                            else { cycle_spec::fields(mode).1 as int }][cycle_spec::prefix_keys(mode, done)[key]],
            decreases 2 - bank,
        ))]
        while bank < 2 {
            let keys = if bank == 0 || matches!(mode, CycleMode::ExportRuns) {
                [1_usize, 2, 4, 8]
            } else {
                [1_usize, 3, 6, 12]
            };
            let current_slot = fields[bank].slot();
            let current = &codec.integers.slots[current_slot];
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(current_slot == if bank == 0 { cycle_spec::fields(mode).0 }
                    else { cycle_spec::fields(mode).1 });
            }
            let mut key = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant key <= 4, bank < 2,
                    current@.len() == 512,
                    current@ == codec.integers.slots[current_slot as int]@,
                    current_slot == if bank == 0 { cycle_spec::fields(mode).0 }
                        else { cycle_spec::fields(mode).1 },
                    forall|offset: int| 0 <= offset < 4 ==>
                        keys@[offset] as int == cycle_spec::prefix_keys(mode, bank as int)[offset]
                            && keys@[offset] < 512,
                    forall|done: int| 0 <= done < key ==>
                        self.banks[bank as int][keys@[done] as int] == current@[keys@[done] as int],
                decreases 4 - key,
            ))]
            while key < 4 {
                let index = keys[key];
                if self.banks[bank][index].state_index != current[index].state_index
                    || self.banks[bank][index].mps != current[index].mps
                {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(self.banks[bank as int][index as int] != current@[index as int]);
                        assert(!cycle_spec::prefix_equal(self, codec, mode));
                    }
                    return false;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(self.banks[bank as int][index as int] == current@[index as int]);
                }
                key += 1;
            }
            bank += 1;
        }
        true
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires cycle_spec::state_valid(cycle_spec::checkpoint_state(self)),
            symbol_text_spec::arithmetic_codec_valid(codec),
            self.decoder.bytes == codec.decoder.bytes,
        ensures out == (cycle_spec::checkpoint_state(self) == cycle_spec::state(codec, mode, cursor)),
    ))]
    #[cold]
    #[allow(clippy::too_many_lines)]
    fn matches(
        &self,
        codec: &ArithmeticCodec<'_>,
        mode: CycleMode,
        cursor: (u32, u32, bool),
    ) -> bool {
        let decoder = &codec.decoder;
        let same_marker = match (self.decoder.marker, decoder.marker) {
            (Some(left), Some(right)) => left.offset == right.offset && left.code == right.code,
            (None, None) => true,
            _ => false,
        };
        if self.cursor.0 != cursor.0
            || self.cursor.1 != cursor.1
            || self.cursor.2 != cursor.2
            || self.decoder.byte_position != decoder.byte_position
            || self.decoder.current_byte != decoder.current_byte
            || self.decoder.code != decoder.code
            || self.decoder.interval != decoder.interval
            || self.decoder.bit_count != decoder.bit_count
            || !same_marker
            || self.decoder.bytes.len() != decoder.bytes.len()
        {
            return false;
        }
        let fields = mode.fields();
        let mut bank = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant bank <= 2,
                fields@[0] as usize == cycle_spec::fields(mode).0,
                fields@[1] as usize == cycle_spec::fields(mode).1,
                forall|done: int| 0 <= done < bank ==>
                    self.banks[done]@ == codec.integers.slots[
                        if done == 0 { cycle_spec::fields(mode).0 as int }
                        else { cycle_spec::fields(mode).1 as int }
                    ]@,
            decreases 2 - bank,
        ))]
        while bank < 2 {
            let current_slot = fields[bank].slot();
            let current = &codec.integers.slots[current_slot];
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(current_slot == if bank == 0 { cycle_spec::fields(mode).0 }
                    else { cycle_spec::fields(mode).1 });
            }
            let mut index = 0_usize;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant index <= 512, bank < 2,
                    current@.len() == 512,
                    current_slot == if bank == 0 { cycle_spec::fields(mode).0 }
                        else { cycle_spec::fields(mode).1 },
                    current@ == codec.integers.slots[current_slot as int]@,
                    forall|done: int| 0 <= done < index ==>
                        self.banks[bank as int][done] == current@[done],
                decreases 512 - index,
            ))]
            while index < 512 {
                if self.banks[bank][index].state_index != current[index].state_index
                    || self.banks[bank][index].mps != current[index].mps
                {
                    #[cfg(verus_keep_ghost)]
                    proof! {
                        assert(self.banks[bank as int][index as int] != current@[index as int]);
                        assert(cycle_spec::checkpoint_state(self)
                            != cycle_spec::state(codec, mode, cursor));
                    }
                    return false;
                }
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(self.banks[bank as int][index as int] == current@[index as int]);
                }
                index += 1;
            }
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(self.banks[bank as int]@ == current@);
            }
            bank += 1;
        }
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant index <= decoder.bytes@.len(),
                decoder.bytes@ == codec.decoder.bytes@,
                self.decoder.bytes@.len() == decoder.bytes@.len(),
                forall|done: int| 0 <= done < index ==>
                    self.decoder.bytes@[done] == decoder.bytes@[done],
            decreases decoder.bytes@.len() - index,
        ))]
        while index < decoder.bytes.len() {
            if self.decoder.bytes[index] != decoder.bytes[index] {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(self.decoder.bytes@ != decoder.bytes@);
                    assert(cycle_spec::checkpoint_state(self).4
                        != cycle_spec::state(codec, mode, cursor).4);
                }
                return false;
            }
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.decoder.bytes@ == decoder.bytes@);
            assert(self.decoder.bytes == decoder.bytes);
            assert(self.decoder == codec.decoder);
        }
        true
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(in crate::symbol_text) struct IntegerCycle<'a> {
    pub(in crate::symbol_text) mode: CycleMode,
    pub(in crate::symbol_text) checkpoint: Option<Checkpoint<'a>>,
    #[cfg(verus_keep_ghost)]
    #[verifier::spec]
    pub(in crate::symbol_text) history: Ghost<Seq<cycle_spec::CycleState<'a>>>,
    #[cfg(verus_keep_ghost)]
    #[verifier::spec]
    pub(in crate::symbol_text) anchor: Ghost<int>,
}

impl<'a> IntegerCycle<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures cycle_spec::detector_valid(&out), out.mode == mode,
            out.checkpoint.is_none(), out.history@.len() == 0,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[inline]
    pub(in crate::symbol_text) fn new(mode: CycleMode) -> Self {
        Self {
            mode,
            checkpoint: None,
            #[cfg(verus_keep_ghost)]
            history: verus_exec_expr!(Ghost(Seq::empty())),
            #[cfg(verus_keep_ghost)]
            anchor: verus_exec_expr!(Ghost(0int)),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.checkpoint.is_some(),
    ))]
    #[inline]
    pub(in crate::symbol_text) fn active(&self) -> bool {
        self.checkpoint.is_some()
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(
        ensures cycle_spec::detector_valid(final(self)),
            final(self).mode == old(self).mode,
            final(self).checkpoint.is_none(), final(self).history@.len() == 0,
    ))]
    #[inline]
    pub(in crate::symbol_text) fn reset(&mut self) {
        self.checkpoint = None;
        #[cfg(verus_keep_ghost)]
        proof! {
            self.history = Ghost(Seq::empty());
            self.anchor = Ghost(0int);
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires cycle_spec::detector_valid(old(self)),
            symbol_text_spec::arithmetic_codec_valid(codec),
            old(self).history@.len() > 0,
            old(self).history@.last() == cycle_spec::state(codec, old(self).mode, cursor),
            old(self).checkpoint.is_some() ==>
                old(self).anchor@ < old(self).history@.len() - 1,
        ensures cycle_spec::detector_valid(final(self)),
            final(self).mode == old(self).mode,
            final(self).history@ == old(self).history@,
            out ==> cycle_spec::repetition(old(self).mode, old(self).history@),
    ))]
    #[cold]
    pub(in crate::symbol_text) fn observe(
        &mut self,
        codec: &ArithmeticCodec<'a>,
        cursor: (u32, u32, bool),
    ) -> bool {
        if let Some(checkpoint) = &self.checkpoint {
            if checkpoint.prefix_matches(codec, self.mode) {
                let matched = checkpoint.matches(codec, self.mode, cursor);
                #[cfg(verus_keep_ghost)]
                proof! {
                    if matched {
                        assert(self.history@[self.anchor@] == self.history@.last());
                    }
                }
                return matched;
            }
        }
        self.checkpoint = Some(Checkpoint::capture(codec, self.mode, cursor));
        #[cfg(verus_keep_ghost)]
        proof! {
            self.anchor = Ghost(self.history@.len() - 1);
        }
        false
    }
}
