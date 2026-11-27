//! Clipped symbol placement and compatible row-blit batching.

#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::{
    enforce_limit, Bitmap, CombinationOperator, DecodeError, LimitKind, Limits, MalformedError,
    Placement, PlacementBatch, PreparedPlacement, ResourceTracker, RowBlit, SymbolTextError,
    MAX_PLACEMENT_BATCH,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

/// Combines a symbol bitmap at a signed top-left location with normative
/// clipping. Nonnegative origins use the existing checked bitmap composition
/// API; negative origins use an explicitly checked cropped loop.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires value < 0,
    ensures match out {
        Ok(magnitude) =>
            value < 0
                && magnitude >= 0
                && magnitude as int == -(value as int),
        Err(_) => value == i64::MIN,
    },
))]
pub(super) fn negative_magnitude(value: i64) -> Result<i64, SymbolTextError> {
    if value == i64::MIN {
        return Err(MalformedError::IntegerOverflow.into());
    }
    Ok(-value)
}

impl<'a> PlacementBatch<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.blits@.len() == 0,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn new() -> Self {
        Self {
            blits: Vec::new(),
            source_y: 0,
            destination_y: 0,
            height: 0,
            source_shift: 0,
            destination_shift: 0,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures final(self).blits@ == old(self).blits@,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn reserve(&mut self, count: usize, limits: &Limits) -> Result<(), SymbolTextError> {
        let capacity = count.min(MAX_PLACEMENT_BATCH);
        let requested_bytes = capacity
            .checked_mul(core::mem::size_of::<RowBlit<'_>>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(MalformedError::IntegerOverflow)?;
        enforce_limit(
            LimitKind::Memory,
            limits.max_decoder_managed_memory_bytes,
            requested_bytes,
        )?;
        self.blits
            .try_reserve(capacity)
            .map_err(|_| DecodeError::Allocation { requested_bytes }.into())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out ==> (
            self.blits@.len() < MAX_PLACEMENT_BATCH
            && (
                self.blits@.len() == 0
                || (
                    self.source_y == prepared.source_y
                    && self.destination_y == prepared.destination_y
                    && self.height == prepared.height
                    && self.source_shift == prepared.blit.source_x % 8
                    && self.destination_shift == prepared.blit.destination_x % 8
                )
            )
        ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn compatible(&self, prepared: PreparedPlacement<'a>) -> bool {
        self.blits.is_empty()
            || (self.blits.len() < MAX_PLACEMENT_BATCH
                && self.source_y == prepared.source_y
                && self.destination_y == prepared.destination_y
                && self.height == prepared.height
                && self.source_shift == prepared.blit.source_x % 8
                && self.destination_shift == prepared.blit.destination_x % 8)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            symbol_text_spec::placement_batch_valid(old(self), _destination),
            symbol_text_spec::prepared_placement_valid(prepared, _destination),
        ensures match out {
            Ok(()) => symbol_text_spec::placement_batch_valid(final(self), _destination),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn push(
        &mut self,
        prepared: PreparedPlacement<'a>,
        _destination: &Bitmap,
    ) -> Result<(), SymbolTextError> {
        if !self.compatible(prepared) {
            return Err(SymbolTextError::InvalidState(
                "incompatible text placement batch",
            ));
        }
        if self.blits.is_empty() {
            self.source_y = prepared.source_y;
            self.destination_y = prepared.destination_y;
            self.height = prepared.height;
            self.source_shift = prepared.blit.source_x % 8;
            self.destination_shift = prepared.blit.destination_x % 8;
        }
        self.blits.push(prepared.blit);
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(symbol_text_spec::placement_batch_valid);
            reveal(symbol_text_spec::prepared_placement_valid);
        }
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(destination)),
        symbol_text_spec::placement_batch_valid(old(batch), old(destination)),
    ensures match out {
        Ok(()) =>
            crate::spec::bitmap::storage_valid(final(destination))
            && final(destination).width == old(destination).width
            && final(destination).height == old(destination).height
            && crate::views::bitmap_width(final(destination))
                == crate::views::bitmap_width(old(destination))
            && crate::views::bitmap_height(final(destination))
                == crate::views::bitmap_height(old(destination))
            && crate::views::bitmap_stride(final(destination))
                == crate::views::bitmap_stride(old(destination))
            && crate::views::bitmap_data(final(destination)).len()
                == crate::views::bitmap_data(old(destination)).len()
            && final(batch).blits@.len() == 0
            && symbol_text_spec::placement_batch_valid(final(batch), final(destination)),
        Err(_) => true,
    },
        (old(batch).blits@.len() == 0 || old(batch).height == 0) ==> out.is_ok(),
        (old(batch).blits@.len() == 0 || old(batch).height == 0) ==>
            crate::views::bitmap_data(final(destination))
                == crate::views::bitmap_data(old(destination)),
))]
pub(super) fn flush_placement_batch(
    destination: &mut Bitmap,
    batch: &mut PlacementBatch<'_>,
    operator: CombinationOperator,
) -> Result<(), SymbolTextError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        crate::views::bitmap_fields(old(destination));
    }
    if batch.blits.is_empty() {
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(symbol_text_spec::placement_batch_valid);
            crate::views::bitmap_fields(destination);
            crate::views::bitmap_fields(old(destination));
        }
        return Ok(());
    }
    if batch.blits.len() == 1 {
        let blit = batch.blits[0];
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(symbol_text_spec::placement_batch_valid);
            crate::views::bitmap_fields(blit.source);
        }
        destination.compose_region(
            blit.source,
            blit.source_x,
            batch.source_y,
            blit.destination_x,
            batch.destination_y,
            blit.width,
            batch.height,
            operator,
        )?;
        batch.blits.clear();
        #[cfg(verus_keep_ghost)]
        proof! {
            reveal(symbol_text_spec::placement_batch_valid);
            crate::views::bitmap_fields(destination);
            crate::views::bitmap_fields(old(destination));
        }
        return Ok(());
    }
    destination.compose_row_batch_chunked(
        &batch.blits,
        batch.source_y,
        batch.destination_y,
        batch.height,
        operator,
    )?;
    batch.blits.clear();
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::placement_batch_valid);
        crate::views::bitmap_fields(destination);
        crate::views::bitmap_fields(old(destination));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(destination),
        crate::spec::bitmap::storage_valid(source),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
    ensures symbol_text_spec::placement_preflight_result(
        destination, source, placement, old(resources), final(resources), limits, out,
    ),
    match out {
        Ok(Some(prepared)) =>
            symbol_text_spec::work_units(final(resources))
                <= limits.max_work_units
            && symbol_text_spec::bitmap_bytes(final(resources))
                == symbol_text_spec::bitmap_bytes(old(resources))
            && symbol_text_spec::prepared_placement_valid(prepared, destination)
            && prepared.blit.source_x as nat + prepared.blit.width as nat
                <= source.width as nat
            && prepared.source_y as nat + prepared.height as nat
                <= source.height as nat
            && prepared.blit.destination_x as nat + prepared.blit.width as nat
                <= destination.width as nat
            && prepared.destination_y as nat + prepared.height as nat
                <= destination.height as nat,
        Ok(None) =>
            symbol_text_spec::work_units(final(resources))
                <= limits.max_work_units
            && symbol_text_spec::bitmap_bytes(final(resources))
                == symbol_text_spec::bitmap_bytes(old(resources)),
        Err(_) => true,
    },
))]
pub(super) fn prepare_placement<'a>(
    destination: &Bitmap,
    source: &'a Bitmap,
    placement: Placement,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<Option<PreparedPlacement<'a>>, SymbolTextError> {
    let source_width = i64::from(source.width());
    let source_height = i64::from(source.height());
    let destination_width = i64::from(destination.width());
    let destination_height = i64::from(destination.height());
    let source_x = if placement.x < 0 {
        negative_magnitude(placement.x)?
    } else {
        0
    };
    let source_y = if placement.y < 0 {
        negative_magnitude(placement.y)?
    } else {
        0
    };
    let destination_x = placement.x.max(0);
    let destination_y = placement.y.max(0);
    let raw_overlap_width = source_width
        .checked_sub(source_x)
        .ok_or(MalformedError::IntegerOverflow)?
        .min(
            destination_width
                .checked_sub(destination_x)
                .ok_or(MalformedError::IntegerOverflow)?,
        );
    let overlap_width = if raw_overlap_width < 0 {
        0
    } else {
        raw_overlap_width
    };
    let raw_overlap_height = source_height
        .checked_sub(source_y)
        .ok_or(MalformedError::IntegerOverflow)?
        .min(
            destination_height
                .checked_sub(destination_y)
                .ok_or(MalformedError::IntegerOverflow)?,
        );
    let overlap_height = if raw_overlap_height < 0 {
        0
    } else {
        raw_overlap_height
    };
    #[cfg(verus_keep_ghost)]
    production_symbol_clipped_geometry!(
        destination,
        source,
        placement,
        overlap_width,
        overlap_height
    );
    let work = u64::try_from(overlap_width)
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_mul(u64::try_from(overlap_height).map_err(|_| MalformedError::IntegerOverflow)?)
        .ok_or(MalformedError::IntegerOverflow)?;
    resources.charge_work(work, limits)?;
    if overlap_width == 0 || overlap_height == 0 {
        return Ok(None);
    }
    let prepared = PreparedPlacement {
        blit: RowBlit {
            source,
            source_x: u32::try_from(source_x).map_err(|_| MalformedError::IntegerOverflow)?,
            destination_x: u32::try_from(destination_x)
                .map_err(|_| MalformedError::IntegerOverflow)?,
            width: u32::try_from(overlap_width).map_err(|_| MalformedError::IntegerOverflow)?,
        },
        source_y: u32::try_from(source_y).map_err(|_| MalformedError::IntegerOverflow)?,
        destination_y: u32::try_from(destination_y).map_err(|_| MalformedError::IntegerOverflow)?,
        height: u32::try_from(overlap_height).map_err(|_| MalformedError::IntegerOverflow)?,
    };
    #[cfg(verus_keep_ghost)]
    proof! {
        reveal(symbol_text_spec::prepared_placement_valid);
        crate::views::bitmap_fields(source);
        crate::views::bitmap_fields(destination);
    }
    Ok(Some(prepared))
}

#[allow(clippy::too_many_lines)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(destination)),
        crate::spec::bitmap::storage_valid(source),
        symbol_text_spec::work_units(old(resources)) <= limits.max_work_units,
    ensures
        symbol_text_spec::dictionary::clipped_composition_frame(
            old(destination), source, placement, old(resources),
            final(destination), final(resources), limits, out,
        ),
        match out {
            Ok(()) =>
                crate::spec::bitmap::storage_valid(final(destination))
                    && crate::views::bitmap_width(final(destination))
                        == crate::views::bitmap_width(old(destination))
                    && crate::views::bitmap_height(final(destination))
                        == crate::views::bitmap_height(old(destination))
                    && symbol_text_spec::work_units(final(resources))
                        <= limits.max_work_units
                    && symbol_text_spec::bitmap_bytes(final(resources))
                        == symbol_text_spec::bitmap_bytes(old(resources)),
            Err(_) => true,
        },
))]
pub(crate) fn compose_placement(
    destination: &mut Bitmap,
    source: &Bitmap,
    placement: Placement,
    operator: CombinationOperator,
    limits: &Limits,
    resources: &mut ResourceTracker,
) -> Result<(), SymbolTextError> {
    let Some(prepared) = prepare_placement(destination, source, placement, limits, resources)?
    else {
        return Ok(());
    };
    destination.compose_region(
        prepared.blit.source,
        prepared.blit.source_x,
        prepared.source_y,
        prepared.blit.destination_x,
        prepared.destination_y,
        prepared.blit.width,
        prepared.height,
        operator,
    )?;
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        crate::views::bitmap_fields(old(destination));
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}
