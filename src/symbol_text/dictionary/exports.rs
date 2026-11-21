//! Exports implementation.

#[cfg(verus_keep_ghost)]
use super::super::symbol_text_spec::cycles as cycle_spec;
use super::cycles::{CycleMode, IntegerCycle, EXPORT_CYCLE_MIN_WORK};
#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    check_dimensions, checked_unsigned_delta, clone_bitmap, enforce_limit, symbol_by_id, Bitmap,
    DecodeError, DictionaryClassState, EntropyInteger, IntegerField, LimitExceeded, LimitKind,
    Limits, MalformedError, ResourceTracker, SymbolCodebook, SymbolCodec, SymbolTextError,
};

impl DictionaryClassState {
    /// Advances a class whose symbols are decoded individually, not joined into
    /// the collective bitmap used only by Huffman non-aggregation dictionaries.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::dictionary_state_valid(old(self)),
        ensures match out {
            Ok((width, height)) =>
                symbol_text_spec::dictionary_state_valid(final(self))
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::work_units(final(resources)) <= limits.max_work_units
                && final(self).class_open
                && final(self).declared_new_symbols == old(self).declared_new_symbols
                && final(self).decoded_symbols == old(self).decoded_symbols + 1
                && width == final(self).width
                && height == final(self).height
                && final(self).total_width == width
                && width <= limits.max_width,
            Err(_) => true,
        },
        (old(self).class_open
            && old(self).decoded_symbols >= old(self).declared_new_symbols) ==>
            out == Err(SymbolTextError::InvalidCount("too many new symbols"))
                && final(resources).work_units == old(resources).work_units
                && *final(self) == *old(self),
    ))]
    pub(crate) fn add_individual_symbol(
        &mut self,
        width_delta: i64,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<(u32, u32), SymbolTextError> {
        if !self.class_open {
            return Err(SymbolTextError::InvalidState("no open height class"));
        }
        if self.decoded_symbols >= self.declared_new_symbols {
            return Err(SymbolTextError::InvalidCount("too many new symbols"));
        }
        resources.charge_work(1, limits)?;
        self.width = checked_unsigned_delta(self.width, width_delta, limits.max_width)?;
        check_dimensions(self.width, self.height, limits)?;
        self.total_width = self.width;
        self.decoded_symbols = self
            .decoded_symbols
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        Ok((self.width, self.height))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(state) =>
                symbol_text_spec::dictionary_state_valid(&state)
                && state.declared_new_symbols == declared_new_symbols
                && state.decoded_symbols == 0
                && !state.class_open,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(declared_new_symbols: u32, limits: &Limits) -> Result<Self, SymbolTextError> {
        enforce_limit(
            LimitKind::Symbols,
            u64::from(limits.max_symbols),
            u64::from(declared_new_symbols),
        )?;
        Ok(Self {
            declared_new_symbols,
            decoded_symbols: 0,
            height: 0,
            width: 0,
            total_width: 0,
            class_open: false,
        })
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::dictionary_state_valid(old(self)),
        ensures match out {
            Ok(height) =>
                symbol_text_spec::dictionary_state_valid(final(self))
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && final(self).class_open
                && final(self).declared_new_symbols == old(self).declared_new_symbols
                && final(self).height == height
                && height as int == old(self).height as int + delta as int
                && height <= limits.max_height
                && final(self).decoded_symbols == old(self).decoded_symbols
                && final(self).width == 0
                && final(self).total_width == 0,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn begin_height_class(
        &mut self,
        delta: i64,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<u32, SymbolTextError> {
        if self.class_open {
            return Err(SymbolTextError::InvalidState("height class already open"));
        }
        if self.decoded_symbols == self.declared_new_symbols {
            return Err(SymbolTextError::InvalidState(
                "height class after final symbol",
            ));
        }
        resources.charge_work(1, limits)?;
        self.height = checked_unsigned_delta(self.height, delta, limits.max_height)?;
        self.width = 0;
        self.total_width = 0;
        self.class_open = true;
        Ok(self.height)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::dictionary_state_valid(old(self)),
        ensures match out {
            Ok((width, height)) =>
                symbol_text_spec::dictionary_state_valid(final(self))
                && symbol_text_spec::work_units(final(resources))
                    == symbol_text_spec::work_units(old(resources)) + 1
                && symbol_text_spec::work_units(final(resources))
                    <= limits.max_work_units
                && final(self).class_open
                && final(self).declared_new_symbols == old(self).declared_new_symbols
                && final(self).decoded_symbols == old(self).decoded_symbols + 1
                && final(self).decoded_symbols <= final(self).declared_new_symbols
                && width == final(self).width
                && height == final(self).height
                && final(self).total_width <= limits.max_width,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn add_symbol(
        &mut self,
        width_delta: i64,
        limits: &Limits,
        resources: &mut ResourceTracker,
    ) -> Result<(u32, u32), SymbolTextError> {
        if !self.class_open {
            return Err(SymbolTextError::InvalidState("no open height class"));
        }
        if self.decoded_symbols >= self.declared_new_symbols {
            return Err(SymbolTextError::InvalidCount("too many new symbols"));
        }
        resources.charge_work(1, limits)?;
        self.width = checked_unsigned_delta(self.width, width_delta, limits.max_width)?;
        self.total_width = self
            .total_width
            .checked_add(self.width)
            .ok_or(MalformedError::IntegerOverflow)?;
        if self.total_width > limits.max_width {
            return Err(LimitExceeded {
                kind: LimitKind::Width,
                limit: u64::from(limits.max_width),
                actual: u64::from(self.total_width),
            }
            .into());
        }
        check_dimensions(self.total_width, self.height, limits)?;
        self.decoded_symbols = self
            .decoded_symbols
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        Ok((self.width, self.height))
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::dictionary_state_valid(old(self)),
        ensures match out {
            Ok(total_width) =>
                symbol_text_spec::dictionary_state_valid(final(self))
                && !final(self).class_open
                && final(self).declared_new_symbols == old(self).declared_new_symbols
                && final(self).decoded_symbols == old(self).decoded_symbols
                && final(self).height == old(self).height
                && final(self).width == old(self).width
                && final(self).total_width == old(self).total_width
                && total_width == final(self).total_width,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn finish_height_class(&mut self) -> Result<u32, SymbolTextError> {
        if !self.class_open {
            return Err(SymbolTextError::InvalidState("no open height class"));
        }
        self.class_open = false;
        Ok(self.total_width)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::dictionary_state_valid(&self),
        ensures match out {
            Ok(()) =>
                !self.class_open
                && self.decoded_symbols == self.declared_new_symbols,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn finish(self) -> Result<(), SymbolTextError> {
        if self.class_open {
            return Err(SymbolTextError::InvalidState("height class still open"));
        }
        if self.decoded_symbols != self.declared_new_symbols {
            return Err(SymbolTextError::InvalidCount("new symbol count mismatch"));
        }
        Ok(())
    }
}

/// Decodes alternating export-flag runs, beginning with a zero run.
#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codec_valid(old(codec)),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
    ensures match out {
        Ok(exported) =>
            symbol_text_spec::codec_valid(final(codec))
                && exported@.len() == expected_exports
                && symbol_text_spec::export_indices_valid(
                    exported@, total_candidates
                )
                && (forall|left: int, right: int|
                    0 <= left < right < exported@.len() ==>
                        #[trigger] exported@[left] < #[trigger] exported@[right])
                && symbol_text_spec::bitmap_bytes(final(resources))
                    == symbol_text_spec::bitmap_bytes(old(resources)),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn decode_export_runs(
    codec: &mut SymbolCodec<'_>,
    total_candidates: u32,
    expected_exports: u32,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<Vec<u32>, SymbolTextError> {
    if expected_exports > total_candidates {
        return Err(SymbolTextError::InvalidCount(
            "export count exceeds candidate count",
        ));
    }
    let capacity =
        usize::try_from(expected_exports).map_err(|_| MalformedError::IntegerOverflow)?;
    let requested_bytes = u64::from(expected_exports)
        .checked_mul(4)
        .ok_or(MalformedError::IntegerOverflow)?;
    let mut exported: Vec<u32> = Vec::new();
    exported
        .try_reserve(capacity)
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    let mut index = 0_u32;
    let mut export_flag = false;
    let mut cycle = IntegerCycle::new(CycleMode::ExportRuns);
    let mut zero_run = false;
    let detect_cycles = limits.max_work_units > EXPORT_CYCLE_MIN_WORK;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            symbol_text_spec::codec_valid(codec),
            cycle_spec::detector_valid(&cycle),
            cycle.mode == CycleMode::ExportRuns,
            cycle.checkpoint.is_some() ==> (
                detect_cycles && zero_run
                    && cycle_spec::source_state(codec, CycleMode::ExportRuns, (index, 0, export_flag))
                        == Some(cycle.history@.last())
            ),
            symbol_text_spec::work_units(resources) <= limits.max_work_units,
            symbol_text_spec::work_units(resources)
                >= symbol_text_spec::work_units(old(resources)),
            symbol_text_spec::bitmap_bytes(resources)
                == symbol_text_spec::bitmap_bytes(old(resources)),
            index <= total_candidates,
            capacity == expected_exports,
            exported@.len() <= capacity,
            forall|offset: int| 0 <= offset < exported@.len()
                ==> #[trigger] exported@[offset] < index,
            forall|left: int, right: int|
                0 <= left < right < exported@.len() ==>
                    #[trigger] exported@[left] < #[trigger] exported@[right],
        decreases limits.max_work_units - symbol_text_spec::work_units(resources),
    ))]
    while index < total_candidates {
        #[cfg(verus_keep_ghost)]
        proof_decl! {
            let ghost cycle_before = *codec;
        }
        resources.charge_work(1, limits)?;
        let decoded_run = codec.decode_integer(IntegerField::ExportRun)?;
        let run = match decoded_run {
            EntropyInteger::Value(value) => u32::try_from(value)
                .map_err(|_| SymbolTextError::InvalidEntropyValue("negative export run"))?,
            EntropyInteger::OutOfBand => {
                return Err(SymbolTextError::UnexpectedOutOfBand("export run"))
            }
        };
        let end = index
            .checked_add(run)
            .ok_or(MalformedError::IntegerOverflow)?;
        if end > total_candidates {
            return Err(SymbolTextError::InvalidCount(
                "export run exceeds candidate count",
            ));
        }
        if detect_cycles {
            if run == 0 {
                if zero_run {
                    if let SymbolCodec::Arithmetic(arithmetic) = &*codec {
                        if cycle.active() || arithmetic.decoder.marker.is_some() {
                            #[cfg(verus_keep_ghost)]
                            proof! {
                                assert(decoded_run == EntropyInteger::Value(0));
                                let before = cycle_spec::source_state(
                                    &cycle_before, CycleMode::ExportRuns, (index, 0, export_flag),
                                );
                                let after = cycle_spec::state(
                                    arithmetic, CycleMode::ExportRuns, (index, 0, !export_flag),
                                );
                                if cycle.checkpoint.is_some() {
                                    cycle_spec::export_zero_step(
                                        &cycle_before, &*codec, (index, 0, export_flag),
                                    );
                                }
                                let recorded = cycle_spec::record(&cycle, before, after);
                                cycle.history = Ghost(recorded.0);
                                cycle.anchor = Ghost(recorded.1);
                            }
                            let repeated = cycle.observe(arithmetic, (index, 0, !export_flag));
                            if repeated {
                                #[cfg(verus_keep_ghost)]
                                proof! {
                                    cycle_spec::repetition_never_progresses(
                                        CycleMode::ExportRuns, cycle.history@,
                                    );
                                }
                                return Err(resources.exhaust_work(limits));
                            }
                        }
                    }
                }
                zero_run = true;
            } else {
                cycle.reset();
                zero_run = false;
            }
        }
        if export_flag {
            let mut candidate = index;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    index <= candidate <= end,
                    end <= total_candidates,
                    exported@.len() <= capacity,
                    forall|offset: int| 0 <= offset < exported@.len()
                        ==> #[trigger] exported@[offset] < candidate,
                    forall|left: int, right: int|
                        0 <= left < right < exported@.len() ==>
                            #[trigger] exported@[left] < #[trigger] exported@[right],
                decreases end - candidate,
            ))]
            while candidate < end {
                if exported.len() >= capacity {
                    return Err(SymbolTextError::InvalidCount("too many exported symbols"));
                }
                exported.push(candidate);
                candidate = candidate
                    .checked_add(1)
                    .ok_or(MalformedError::IntegerOverflow)?;
            }
        }
        index = end;
        export_flag = !export_flag;
    }
    if exported.len() != capacity {
        return Err(SymbolTextError::InvalidCount(
            "exported-symbol popcount mismatch",
        ));
    }
    Ok(exported)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        symbol_text_spec::codebook_valid(imported),
        symbol_text_spec::bitmaps_valid(new_symbols@),
        forall|index: int| 0 <= index < exported_indices@.len()
            ==> (#[trigger] exported_indices@[index] as nat)
                < symbol_text_spec::codebook_len(imported) + (new_symbols@).len(),
    ensures match out {
        Ok(exported) =>
            exported@.len() == exported_indices@.len()
                && symbol_text_spec::bitmaps_valid(exported@)
                && symbol_text_spec::exported_contents_match(
                    imported, new_symbols@, exported_indices@, exported@,
                ),
        Err(_) => true,
    },
))]
pub(in crate::symbol_text) fn assemble_exported_symbols(
    imported: SymbolCodebook<'_>,
    new_symbols: &[Bitmap],
    exported_indices: &[u32],
    limits: &Limits,
) -> Result<Vec<Bitmap>, SymbolTextError> {
    let mut exported_symbols: Vec<Bitmap> = Vec::new();
    let requested_bytes = exported_indices
        .len()
        .checked_mul(core::mem::size_of::<Bitmap>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(MalformedError::IntegerOverflow)?;
    exported_symbols
        .try_reserve(exported_indices.len())
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= exported_indices@.len(),
            exported_symbols@.len() == index,
            symbol_text_spec::codebook_valid(imported),
            symbol_text_spec::bitmaps_valid(new_symbols@),
            symbol_text_spec::bitmaps_valid(exported_symbols@),
            forall|offset: int| 0 <= offset < exported_indices@.len()
                ==> (#[trigger] exported_indices@[offset] as nat)
                    < symbol_text_spec::codebook_len(imported) + (new_symbols@).len(),
            forall|offset: int| 0 <= offset < index ==>
                #[trigger] symbol_text_spec::symbol_id_selects(
                    imported, new_symbols@, exported_indices@[offset], &exported_symbols@[offset],
                ),
        decreases exported_indices@.len() - index,
    ))]
    while index < exported_indices.len() {
        let symbol_id = exported_indices[index];
        let source = symbol_by_id(imported, new_symbols, symbol_id)?;
        exported_symbols.push(clone_bitmap(source, limits)?);
        index += 1;
    }
    Ok(exported_symbols)
}
