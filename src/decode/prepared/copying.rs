#[cfg(verus_keep_ghost)]
use super::prepared_spec;
use super::{DecodeError, InitialResources, Limits, MalformedError};
use crate::decode::{allocation_size, symbol_text_error_into_decode, Bitmap, SymbolResource};
use crate::PreparedGlobalsBypass;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) enum CopyFailure {
    Optional(PreparedGlobalsBypass),
    Decode(DecodeError),
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == CopyFailure::Decode(error),
))]
fn required_copy_failure(error: DecodeError) -> CopyFailure {
    CopyFailure::Decode(error)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures prepared_spec::copy_failure(&error, &out),
))]
pub(super) fn copy_failure(error: DecodeError) -> CopyFailure {
    match error {
        DecodeError::Allocation { .. } => {
            CopyFailure::Optional(PreparedGlobalsBypass::CopyAllocation)
        }
        error => CopyFailure::Decode(error),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == prepared_spec::admission(resources),
))]
pub(super) fn admission(resources: &InitialResources) -> Option<PreparedGlobalsBypass> {
    if !resources.tables.is_empty() {
        return Some(PreparedGlobalsBypass::CustomTables);
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= resources.symbols@.len(),
            resources.tables@.len() == 0,
            forall|done: int| 0 <= done < index ==>
                resources.symbols@[done].1.contexts.is_none(),
        decreases resources.symbols@.len() - index,
    ))]
    while index < resources.symbols.len() {
        if resources.symbols[index].1.contexts.is_some() {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(exists|witness: int| 0 <= witness < resources.symbols@.len()
                    && resources.symbols@[witness].1.contexts.is_some()) by {
                    assert(resources.symbols@[index as int].1.contexts.is_some());
                }
            }
            return Some(PreparedGlobalsBypass::RetainedContexts);
        }
        index += 1;
    }
    if resources.patterns.is_empty() && resources.symbols.is_empty() {
        Some(PreparedGlobalsBypass::NoResources)
    } else {
        None
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == if first as nat + second as nat > u64::MAX {
        Err(DecodeError::Malformed(MalformedError::IntegerOverflow))
    } else {
        Ok((first as nat + second as nat) as u64)
    },
))]
fn add_bytes(first: u64, second: u64) -> Result<u64, DecodeError> {
    let Some(total) = first.checked_add(second) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    Ok(total)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn vector_bytes<T>(values: &Vec<T>) -> Result<u64, DecodeError> {
    allocation_size::<T>(values.capacity())
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
fn bitmap_vector_bytes(bitmaps: &Vec<Bitmap>) -> Result<u64, DecodeError> {
    let mut bytes = vector_bytes(bitmaps)?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= bitmaps@.len(),
        decreases bitmaps@.len() - index,
    ))]
    while index < bitmaps.len() {
        bytes = add_bytes(bytes, vector_bytes(&bitmaps[index].data)?)?;
        index += 1;
    }
    Ok(bytes)
}

/// Only admitted resources are sized: no opaque context/table heap is omitted.
#[cfg_attr(verus_keep_ghost, verus_spec(
    requires prepared_spec::admission(resources).is_none(),
))]
pub(super) fn capacity_bytes(resources: &InitialResources) -> Result<u64, DecodeError> {
    let mut bytes = vector_bytes(&resources.patterns)?;
    bytes = add_bytes(bytes, vector_bytes(&resources.symbols)?)?;
    bytes = add_bytes(bytes, vector_bytes(&resources.tables)?)?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= resources.symbols@.len(),
        decreases resources.symbols@.len() - index,
    ))]
    while index < resources.symbols.len() {
        bytes = add_bytes(
            bytes,
            bitmap_vector_bytes(&resources.symbols[index].1.exported)?,
        )?;
        index += 1;
    }
    index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= resources.patterns@.len(),
        decreases resources.patterns@.len() - index,
    ))]
    while index < resources.patterns.len() {
        bytes = add_bytes(bytes, bitmap_vector_bytes(&resources.patterns[index].1)?)?;
        index += 1;
    }
    Ok(bytes)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) struct CopyBudget {
    pub(super) used: u64,
    pub(super) limit: u64,
}

impl CopyBudget {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures prepared_spec::budget_check(self.used, bytes, self.limit, &out),
    ))]
    pub(super) fn check(&self, bytes: u64) -> Result<(), CopyFailure> {
        let actual = add_bytes(self.used, bytes).map_err(required_copy_failure)?;
        if actual > self.limit {
            return Err(CopyFailure::Optional(
                PreparedGlobalsBypass::AdditionalStorage,
            ));
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).used <= old(self).limit,
        ensures final(self).limit == old(self).limit,
            final(self).used <= final(self).limit,
            match out {
                Ok(()) => final(self).used as nat == old(self).used as nat + bytes as nat,
                Err(_) => final(self).used == old(self).used,
            },
    ))]
    fn charge(&mut self, bytes: u64) -> Result<(), CopyFailure> {
        self.check(bytes)?;
        self.used = add_bytes(self.used, bytes).map_err(required_copy_failure)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).used <= old(self).limit,
        ensures prepared_spec::budget_frame(old(self), final(self)),
            match out {
                Ok(values) => values@.len() == 0,
                Err(_) => true,
            },
    ))]
    fn reserve<T>(&mut self, length: usize) -> Result<Vec<T>, CopyFailure> {
        let requested = allocation_size::<T>(length).map_err(required_copy_failure)?;
        self.check(requested)?;
        let mut values = Vec::new();
        if values.try_reserve(length).is_err() {
            return Err(CopyFailure::Optional(PreparedGlobalsBypass::CopyAllocation));
        }
        self.charge(vector_bytes(&values).map_err(required_copy_failure)?)?;
        Ok(values)
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::spec::bitmap::storage_valid(source),
        old(budget).used <= old(budget).limit,
    ensures prepared_spec::budget_frame(old(budget), final(budget)),
        match out {
            Ok(bitmap) => crate::spec::bitmap::storage_valid(&bitmap)
                && prepared_spec::same_bitmap(source, &bitmap),
            Err(_) => true,
        },
))]
pub(super) fn copy_bitmap(
    source: &Bitmap,
    limits: &Limits,
    budget: &mut CopyBudget,
) -> Result<Bitmap, CopyFailure> {
    let requested = u64::try_from(source.data.len()).map_err(|_| {
        CopyFailure::Decode(DecodeError::Malformed(MalformedError::IntegerOverflow))
    })?;
    budget.check(requested)?;
    let copied = crate::symbol_text::clone_bitmap(source, limits)
        .map_err(|error| copy_failure(symbol_text_error_into_decode(error)))?;
    budget.charge(vector_bytes(&copied.data).map_err(required_copy_failure)?)?;
    Ok(copied)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires crate::halftone::halftone_spec::bitmaps_valid(source@),
        old(budget).used <= old(budget).limit,
    ensures prepared_spec::budget_frame(old(budget), final(budget)),
        match out {
            Ok(bitmaps) => crate::halftone::halftone_spec::bitmaps_valid(bitmaps@)
                && prepared_spec::same_bitmaps(source@, bitmaps@),
            Err(_) => true,
        },
))]
pub(super) fn copy_bitmaps(
    source: &[Bitmap],
    limits: &Limits,
    budget: &mut CopyBudget,
) -> Result<Vec<Bitmap>, CopyFailure> {
    let mut copied = budget.reserve(source.len())?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source@.len(),
            crate::halftone::halftone_spec::bitmaps_valid(source@),
            crate::halftone::halftone_spec::bitmaps_valid(copied@),
            copied@.len() == index,
            prepared_spec::bitmap_prefix(source@, copied@),
            budget.limit == old(budget).limit,
            old(budget).used <= budget.used <= budget.limit,
        decreases source@.len() - index,
    ))]
    while index < source.len() {
        let bitmap = copy_bitmap(&source[index], limits, budget)?;
        copied.push(bitmap);
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::bitmap_push(source@, copied@, index as int); }
        index += 1;
    }
    Ok(copied)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires super::super::decode_spec::initial_resources_valid(source),
        prepared_spec::admission(source).is_none(),
        resident_bytes <= additional_storage_bytes,
    ensures match out {
        Ok(resources) => super::super::decode_spec::initial_resources_valid(&resources)
            && prepared_spec::same_resources(source, &resources)
            && prepared_spec::admission(&resources).is_none(),
        Err(_) => true,
    },
))]
pub(super) fn copy_resources(
    source: &InitialResources,
    limits: &Limits,
    resident_bytes: u64,
    additional_storage_bytes: u64,
) -> Result<InitialResources, CopyFailure> {
    let mut budget = CopyBudget {
        used: resident_bytes,
        limit: additional_storage_bytes,
    };
    let mut symbols = budget.reserve(source.symbols.len())?;
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source.symbols@.len(),
            super::super::decode_spec::initial_resources_valid(source),
            prepared_spec::admission(source).is_none(),
            super::super::decode_spec::symbol_store_valid(&symbols),
            prepared_spec::symbol_prefix(source.symbols@, symbols@),
            symbols@.len() == index,
            budget.used <= budget.limit,
            budget.limit == additional_storage_bytes,
        decreases source.symbols@.len() - index,
    ))]
    while index < source.symbols.len() {
        let (number, resource) = &source.symbols[index];
        let exported = copy_bitmaps(&resource.exported, limits, &mut budget)?;
        symbols.push((
            *number,
            SymbolResource {
                exported,
                contexts: None,
            },
        ));
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::symbol_push(source.symbols@, symbols@, index as int); }
        index += 1;
    }
    let mut patterns = budget.reserve(source.patterns.len())?;
    index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source.patterns@.len(),
            super::super::decode_spec::initial_resources_valid(source),
            prepared_spec::admission(source).is_none(),
            super::super::decode_spec::symbol_store_valid(&symbols),
            prepared_spec::symbol_prefix(source.symbols@, symbols@),
            symbols@.len() == source.symbols@.len(),
            super::super::decode_spec::pattern_store_valid(&patterns),
            prepared_spec::pattern_prefix(source.patterns@, patterns@),
            patterns@.len() == index,
            budget.used <= budget.limit,
            budget.limit == additional_storage_bytes,
        decreases source.patterns@.len() - index,
    ))]
    while index < source.patterns.len() {
        let (number, bitmaps) = &source.patterns[index];
        patterns.push((*number, copy_bitmaps(bitmaps, limits, &mut budget)?));
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::pattern_push(source.patterns@, patterns@, index as int); }
        index += 1;
    }
    let copied = InitialResources {
        patterns,
        symbols,
        tables: Vec::new(),
    };
    #[cfg(verus_keep_ghost)]
    proof! { prepared_spec::finish_resources(source, &copied); }
    // Reobserve the completed stores as well as each staged allocation.
    // No claim about allocator usable sizes or process peaks is inferred.
    let observed = capacity_bytes(&copied).map_err(required_copy_failure)?;
    CopyBudget {
        used: resident_bytes,
        limit: additional_storage_bytes,
    }
    .check(observed)?;
    Ok(copied)
}
