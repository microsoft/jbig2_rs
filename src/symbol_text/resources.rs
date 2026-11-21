//! Resources implementation.

#[cfg(verus_keep_ghost)]
use super::*;
#[cfg(not(verus_keep_ghost))]
use super::{
    check_dimensions, enforce_limit, LimitExceeded, LimitKind, Limits, MalformedError,
    ResourceTracker, SymbolTextError,
};

impl ResourceTracker {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires symbol_text_spec::work_units(old(self)) <= limits.max_work_units,
        ensures out == symbol_text_spec::cycles::work_exhaustion_error(limits),
            symbol_text_spec::bitmap_bytes(final(self)) == symbol_text_spec::bitmap_bytes(old(self)),
            symbol_text_spec::work_units(final(self)) == if limits.max_work_units == u64::MAX {
                u64::MAX
            } else { (limits.max_work_units + 1) as u64 },
    ))]
    #[cold]
    pub(super) fn exhaust_work(&mut self, limits: &Limits) -> SymbolTextError {
        // A certified zero-progress orbit reaches this same existing boundary.
        self.work_units = limits.max_work_units;
        match self.work_units.checked_add(1) {
            Some(actual) => {
                self.work_units = actual;
                LimitExceeded {
                    kind: LimitKind::Work,
                    limit: limits.max_work_units,
                    actual,
                }
                .into()
            }
            None => MalformedError::IntegerOverflow.into(),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures symbol_text_spec::work_charge_effect(
            old(self), final(self), units, limits, out.is_ok(),
        ),
        match out {
            Ok(()) =>
                symbol_text_spec::work_units(final(self))
                    == symbol_text_spec::work_units(old(self)) + units
                && symbol_text_spec::work_units(final(self)) <= limits.max_work_units
                && symbol_text_spec::bitmap_bytes(final(self))
                    == symbol_text_spec::bitmap_bytes(old(self)),
            Err(_) => symbol_text_spec::bitmap_bytes(final(self))
                == symbol_text_spec::bitmap_bytes(old(self)),
        },
    ))]
    pub(crate) fn charge_work(
        &mut self,
        units: u64,
        limits: &Limits,
    ) -> Result<(), SymbolTextError> {
        #[cfg(verus_keep_ghost)]
        production_symbol_tracker_fields!(self);
        self.work_units = self
            .work_units
            .checked_add(units)
            .ok_or(MalformedError::IntegerOverflow)?;
        enforce_limit(LimitKind::Work, limits.max_work_units, self.work_units)?;
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures symbol_text_spec::bitmap_charge_effect(
            old(self), final(self), width, height, limits, out.is_ok(),
        ),
        match out {
            Ok(()) =>
                symbol_text_spec::work_units(final(self))
                    == symbol_text_spec::work_units(old(self))
                && symbol_text_spec::bitmap_bytes(final(self))
                    == symbol_text_spec::bitmap_bytes(old(self))
                        + ((width as u64 + 7) / 8) * height as u64
                && symbol_text_spec::bitmap_bytes(final(self))
                    <= limits.max_cumulative_decoded_bitmap_bytes,
            Err(_) => symbol_text_spec::work_units(final(self))
                == symbol_text_spec::work_units(old(self)),
        },
    ))]
    pub(crate) fn charge_bitmap(
        &mut self,
        width: u32,
        height: u32,
        limits: &Limits,
    ) -> Result<(), SymbolTextError> {
        check_dimensions(width, height, limits)?;
        let stride = u64::from(width)
            .checked_add(7)
            .ok_or(MalformedError::IntegerOverflow)?
            / 8;
        let bytes = stride
            .checked_mul(u64::from(height))
            .ok_or(MalformedError::IntegerOverflow)?;
        self.decoded_bitmap_bytes = self
            .decoded_bitmap_bytes
            .checked_add(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        enforce_limit(
            LimitKind::DecodedBitmapBytes,
            limits.max_cumulative_decoded_bitmap_bytes,
            self.decoded_bitmap_bytes,
        )?;
        Ok(())
    }
}
