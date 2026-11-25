//! Private experiment, deliberately absent from the library and Verus surface.

#![cfg(test)]

use super::*;

#[derive(Debug, PartialEq, Eq)]
enum ErrorView<'a> {
    Malformed(&'a MalformedError),
    Unsupported(&'a UnsupportedFeature),
    Limit(&'a crate::LimitExceeded),
    Allocation(u64),
    Io(crate::IoErrorKind),
}

fn error_view(error: &DecodeError) -> ErrorView<'_> {
    match error {
        DecodeError::Malformed(error) => ErrorView::Malformed(error),
        DecodeError::Unsupported(error) => ErrorView::Unsupported(error),
        DecodeError::LimitExceeded(error) => ErrorView::Limit(error),
        DecodeError::Allocation { requested_bytes } => ErrorView::Allocation(*requested_bytes),
        DecodeError::Io(kind) => ErrorView::Io(*kind),
    }
}

macro_rules! assert_outcome_eq {
    ($actual:expr, $expected:expr $(, $context:literal $(, $argument:expr)*)? $(,)?) => {
        assert_eq!(
            $actual.as_ref().map_err(error_view),
            $expected.as_ref().map_err(error_view)
            $(, $context $(, $argument)*)?
        )
    };
}

mod controls;
mod probe;

enum Preparation {
    Cold,
    Ready(InitialResources),
    Bypass,
}

/// One document's immutable globals and exact decoder/limits, never a keyed cache.
struct PreparedOwner<'a> {
    decoder: &'a Decoder,
    globals: &'a GlobalSegments,
    preparation: Preparation,
    extra_budget: u64,
    resident_bytes: u64,
    peak_copy_bytes: u64,
    hits: usize,
    preparations: usize,
    fallbacks: usize,
}

impl<'a> PreparedOwner<'a> {
    fn new(decoder: &'a Decoder, globals: &'a GlobalSegments, extra_budget: u64) -> Self {
        Self {
            decoder,
            globals,
            preparation: Preparation::Cold,
            extra_budget: extra_budget
                .min(decoder.limits().max_decoder_managed_memory_bytes)
                .min(decoder.limits().max_retained_state_bytes),
            resident_bytes: 0,
            peak_copy_bytes: 0,
            hits: 0,
            preparations: 0,
            fallbacks: 0,
        }
    }

    fn fallback(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        self.fallbacks += 1;
        self.decoder
            .decode_embedded_with_globals(input, self.globals)
    }

    fn decode(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        // Keep the production ordering: page validation precedes global decoding.
        // Even a ready owner must not bypass reference/page-association refusals.
        let limits = self.decoder.limits();
        let (info, segments) = parse_embedded_document(input, limits, Some(self.globals))?;
        let mut collector = PageCollector::new(limits);
        let global_bytes = u64::try_from(self.globals.bytes().len())
            .map_err(|_| MalformedError::IntegerOverflow)?;
        let input_work = structural_work(input.len(), &segments)?
            .checked_add(global_bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        if matches!(self.preparation, Preparation::Cold) {
            self.preparations += 1;
            // A successful original decode at these exact immutable limits is
            // the logical global-work/decoded-byte/retention admission witness.
            // Global and page work budgets are separate in production.
            let resources = decode_global_resources(self.globals.bytes(), limits)?;
            let resident = resource_bytes(&resources)?;
            if clonable(&resources) && resident <= self.extra_budget {
                self.resident_bytes = resident;
                self.preparation = Preparation::Ready(resources);
            } else {
                self.preparation = Preparation::Bypass;
            }
        }
        let Preparation::Ready(resources) = &self.preparation else {
            return self.fallback(input);
        };
        let mut copy_budget = CopyBudget {
            used: self.resident_bytes,
            limit: self.extra_budget,
        };
        let Ok(copied) = copy_resources(resources, limits, &mut copy_budget) else {
            // No page state has been mutated. Release the optional resident
            // allocation before taking the original refusal/allocation path.
            self.preparation = Preparation::Bypass;
            self.resident_bytes = 0;
            return self.fallback(input);
        };
        self.peak_copy_bytes = self.peak_copy_bytes.max(copy_budget.used);
        self.hits += 1;
        let mut scratch = MmrScratch::default();
        decode_segments_collected(
            &segments,
            &info,
            input_work,
            global_bytes,
            limits,
            &mut scratch,
            Some(copied),
            &mut collector,
            true,
        )?;
        Ok(DecodedDocument {
            info,
            pages: collector.pages,
        })
    }
}

fn clonable(resources: &InitialResources) -> bool {
    resources.tables.is_empty()
        && (!resources.symbols.is_empty() || !resources.patterns.is_empty())
        && resources
            .symbols
            .iter()
            .all(|(_, resource)| resource.contexts.is_none())
}

fn add_bytes(total: &mut u64, bytes: u64) -> Result<(), DecodeError> {
    *total = total
        .checked_add(bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    Ok(())
}

fn capacity_bytes<T>(values: &Vec<T>) -> Result<u64, DecodeError> {
    allocation_size::<T>(values.capacity())
}

fn resource_bytes(resources: &InitialResources) -> Result<u64, DecodeError> {
    let mut bytes = capacity_bytes(&resources.symbols)?;
    add_bytes(&mut bytes, capacity_bytes(&resources.patterns)?)?;
    add_bytes(&mut bytes, capacity_bytes(&resources.tables)?)?;
    for (_, resource) in &resources.symbols {
        // Context-bearing resources are not admitted; never pretend they cost 0.
        if resource.contexts.is_some() {
            return Ok(u64::MAX);
        }
        add_bytes(&mut bytes, capacity_bytes(&resource.exported)?)?;
        for bitmap in &resource.exported {
            add_bytes(&mut bytes, capacity_bytes(&bitmap.data)?)?;
        }
    }
    for (_, patterns) in &resources.patterns {
        add_bytes(&mut bytes, capacity_bytes(patterns)?)?;
        for bitmap in patterns {
            add_bytes(&mut bytes, capacity_bytes(&bitmap.data)?)?;
        }
    }
    // Nonempty tables have private heap allocations; reject before sizing them.
    if resources.tables.is_empty() {
        Ok(bytes)
    } else {
        Ok(u64::MAX)
    }
}

struct CopyBudget {
    used: u64,
    limit: u64,
}

impl CopyBudget {
    fn check(&self, bytes: u64) -> Result<(), DecodeError> {
        let actual = self
            .used
            .checked_add(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        if actual > self.limit {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::Memory,
                limit: self.limit,
                actual,
            }
            .into());
        }
        Ok(())
    }

    fn reserve<T>(&mut self, length: usize) -> Result<Vec<T>, DecodeError> {
        let requested_bytes = allocation_size::<T>(length)?;
        self.check(requested_bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| DecodeError::Allocation { requested_bytes })?;
        // The allocator may give a larger capacity than requested.
        let physical = capacity_bytes(&values)?;
        self.check(physical)?;
        add_bytes(&mut self.used, physical)?;
        Ok(values)
    }
}

fn copy_resources(
    source: &InitialResources,
    limits: &Limits,
    budget: &mut CopyBudget,
) -> Result<InitialResources, DecodeError> {
    assert!(clonable(source));
    let mut symbols = budget.reserve(source.symbols.len())?;
    for (number, resource) in &source.symbols {
        let exported = copy_bitmaps(&resource.exported, limits, budget)?;
        symbols.push((
            *number,
            SymbolResource {
                exported,
                contexts: None,
            },
        ));
    }
    let mut patterns = budget.reserve(source.patterns.len())?;
    for (number, bitmaps) in &source.patterns {
        patterns.push((*number, copy_bitmaps(bitmaps, limits, budget)?));
    }
    Ok(InitialResources {
        symbols,
        patterns,
        tables: Vec::new(),
    })
}

fn copy_bitmaps(
    source: &[Bitmap],
    limits: &Limits,
    budget: &mut CopyBudget,
) -> Result<Vec<Bitmap>, DecodeError> {
    let mut copied = budget.reserve(source.len())?;
    for bitmap in source {
        // Same packed, independent fallible copy as symbol_text::clone_bitmap.
        // Also charge actual capacities including the still-live owner.
        let requested = bitmap_byte_len(bitmap)?;
        if requested > limits.max_decoder_managed_memory_bytes {
            return Err(crate::LimitExceeded {
                kind: crate::LimitKind::Memory,
                limit: limits.max_decoder_managed_memory_bytes,
                actual: requested,
            }
            .into());
        }
        let mut data = budget.reserve(bitmap.data.len())?;
        data.extend_from_slice(&bitmap.data);
        copied.push(Bitmap {
            width: bitmap.width,
            height: bitmap.height,
            stride: bitmap.stride,
            data,
        });
    }
    Ok(copied)
}
