use super::support::{enforce_managed_memory, integer_overflow};
#[cfg(verus_keep_ghost)]
use super::{generic_region_shared, generic_region_spec};
use super::{
    GenericContextSnapshot, GenericContexts, GenericRegionError, GenericTemplate,
    RefinementContextSnapshot, RefinementContexts, RefinementTemplate, MAX_GENERIC_CONTEXTS,
    MAX_REFINEMENT_CONTEXTS,
};
use crate::arithmetic::{reset_contexts, ArithmeticContext};
use crate::{DecodeError, LimitExceeded, LimitKind, Limits, MalformedError};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl GenericTemplate {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures generic_region_spec::generic_template_code(out)
            == ((flags >> 1) & 0x03) as nat,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn from_flags(flags: u8) -> Self {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(((flags >> 1) & 0x03) <= 3) by (bit_vector);
        }

        match (flags >> 1) & 0x03 {
            0 => Self::Template0,
            1 => Self::Template1,
            2 => Self::Template2,
            _ => Self::Template3,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out as nat == generic_region_spec::generic_context_bits(self),
            10 <= out <= 16,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) const fn context_bits(self) -> u8 {
        #[cfg(verus_keep_ghost)]
        production_generic_template_facts!(self);
        match self {
            Self::Template0 => 16,
            Self::Template1 => 13,
            Self::Template2 | Self::Template3 => 10,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out as nat == generic_region_spec::generic_sltp_context(self),
            (out as nat) < generic_region_spec::pow2(
                generic_region_spec::generic_context_bits(self),
            ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn sltp_context(self) -> usize {
        #[cfg(verus_keep_ghost)]
        production_generic_template_facts!(self);
        match self {
            Self::Template0 => 0x9b25,
            Self::Template1 => 0x0795,
            Self::Template2 => 0x00e5,
            Self::Template3 => 0x0195,
        }
    }
}

impl GenericContexts {
    /// Allocate a default statistics set compatible with `template`.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(contexts) =>
                contexts.template == template
                && contexts.extended_template == extended_template
                && generic_region_spec::generic_contexts_valid(&contexts)
                && contexts.storage_bytes as nat == contexts.contexts@.len() * 2
                && (forall|index: int| 0 <= index < contexts.contexts@.len() ==>
                    crate::arithmetic::arithmetic_spec::context_index(contexts.contexts@[index]) == 0
                        && crate::arithmetic::arithmetic_spec::context_mps(contexts.contexts@[index]) == 0),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(
        template: GenericTemplate,
        extended_template: bool,
        limits: &Limits,
    ) -> Result<Self, GenericRegionError> {
        let count = generic_context_count(template);
        let storage_bytes = context_storage_bytes(count)?;
        let contexts = allocate_contexts(count, 0, limits)?;
        Ok(Self {
            template,
            extended_template,
            contexts,
            storage_bytes,
        })
    }

    /// Reset all contexts to `I = 0, MPS = 0`.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).template == old(self).template,
            final(self).extended_template == old(self).extended_template,
            final(self).storage_bytes == old(self).storage_bytes,
            final(self).contexts@.len() == old(self).contexts@.len(),
            match out {
                Ok(()) => forall|index: int| 0 <= index < final(self).contexts@.len() ==>
                    crate::arithmetic::arithmetic_spec::context_index(final(self).contexts@[index]) == 0
                        && crate::arithmetic::arithmetic_spec::context_mps(final(self).contexts@[index]) == 0,
                Err(_) => final(self).contexts@ == old(self).contexts@,
            },
    ))]
    pub(crate) fn reset(&mut self) -> Result<(), GenericRegionError> {
        validate_generic_context_layout(self.template, self.extended_template, &self.contexts)?;
        reset_contexts(&mut self.contexts);
        Ok(())
    }

    /// Fallibly retain the current statistics for later dictionary use.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(snapshot) =>
                generic_region_spec::generic_snapshot_view(&snapshot).0 == self.template
                && generic_region_spec::generic_snapshot_view(&snapshot).1 == self.extended_template
                && generic_region_spec::generic_snapshot_view(&snapshot).3 == self.storage_bytes
                && generic_region_spec::generic_snapshot_view(&snapshot).2 == self.contexts@
                && generic_region_spec::contexts_valid(
                    generic_region_spec::generic_snapshot_view(&snapshot).2,
                ),
            Err(_) => true,
        },
    ))]
    pub(crate) fn try_snapshot(
        &self,
        limits: &Limits,
    ) -> Result<GenericContextSnapshot, GenericRegionError> {
        #[cfg(verus_keep_ghost)]
        proof! { generic_region_spec::generic_snapshot_view_definition(); }
        validate_generic_context_layout(self.template, self.extended_template, &self.contexts)?;
        let contexts = try_copy_contexts(&self.contexts, self.storage_bytes, limits)?;
        Ok(GenericContextSnapshot {
            template: self.template,
            extended_template: self.extended_template,
            contexts,
            storage_bytes: self.storage_bytes,
        })
    }

    /// Restore retained statistics, rejecting a snapshot for another template.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).template == old(self).template,
            final(self).extended_template == old(self).extended_template,
            final(self).storage_bytes == old(self).storage_bytes,
            final(self).contexts@.len() == old(self).contexts@.len(),
            match out {
                Ok(()) => final(self).contexts@ == generic_region_spec::generic_snapshot_view(snapshot).2
                    && generic_region_spec::contexts_valid(final(self).contexts@),
                Err(_) => final(self).contexts@ == old(self).contexts@,
            },
    ))]
    pub(crate) fn restore(
        &mut self,
        snapshot: &GenericContextSnapshot,
        limits: &Limits,
    ) -> Result<(), GenericRegionError> {
        #[cfg(verus_keep_ghost)]
        proof! { generic_region_spec::generic_snapshot_view_definition(); }
        if self.template != snapshot.template
            || self.extended_template != snapshot.extended_template
        {
            return Err(GenericRegionError::InvalidFlags(generic_template_code(
                snapshot.template,
                snapshot.extended_template,
            )));
        }
        validate_generic_context_layout(
            snapshot.template,
            snapshot.extended_template,
            &snapshot.contexts,
        )?;
        validate_snapshot_limits(snapshot.storage_bytes, limits)?;
        if self.contexts.len() != snapshot.contexts.len() {
            return Err(GenericRegionError::ContextIndex {
                index: snapshot.contexts.len(),
                count: self.contexts.len(),
            });
        }
        validate_probability_states(&snapshot.contexts)?;
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.contexts@.len() == snapshot.contexts@.len(),
                index <= self.contexts@.len(),
                forall|completed: int| 0 <= completed < index ==>
                    self.contexts@[completed] == snapshot.contexts@[completed],
                generic_region_spec::contexts_valid(snapshot.contexts@),
            decreases self.contexts@.len() - index,
        ))]
        while index < self.contexts.len() {
            self.contexts[index] = snapshot.contexts[index];
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.contexts@ == snapshot.contexts@);
            generic_region_spec::generic_snapshot_view_definition();
        }
        Ok(())
    }
}

impl RefinementTemplate {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == if flags & 0x01 == 0 {
            RefinementTemplate::Template0
        } else {
            RefinementTemplate::Template1
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn from_flags(flags: u8) -> Self {
        if flags & 0x01 == 0 {
            Self::Template0
        } else {
            Self::Template1
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out as nat == generic_region_spec::refinement_context_bits(self),
            10 <= out <= 13,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) const fn context_bits(self) -> u8 {
        #[cfg(verus_keep_ghost)]
        production_refinement_template_facts!(self);
        match self {
            Self::Template0 => 13,
            Self::Template1 => 10,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            out as nat == generic_region_spec::refinement_sltp_context(self),
            (out as nat) < generic_region_spec::pow2(
                generic_region_spec::refinement_context_bits(self),
            ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) const fn sltp_context(self) -> usize {
        #[cfg(verus_keep_ghost)]
        production_refinement_template_facts!(self);
        match self {
            Self::Template0 => 0x0010,
            Self::Template1 => 0x0008,
        }
    }
}

impl RefinementContexts {
    /// Allocate a default statistics set compatible with `template`.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(contexts) =>
                contexts.template == template
                && generic_region_spec::refinement_contexts_valid(&contexts)
                && contexts.storage_bytes as nat == contexts.contexts@.len() * 2
                && (forall|index: int| 0 <= index < contexts.contexts@.len() ==>
                    crate::arithmetic::arithmetic_spec::context_index(contexts.contexts@[index]) == 0
                        && crate::arithmetic::arithmetic_spec::context_mps(contexts.contexts@[index]) == 0),
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn new(
        template: RefinementTemplate,
        limits: &Limits,
    ) -> Result<Self, GenericRegionError> {
        let count = refinement_context_count(template);
        let storage_bytes = context_storage_bytes(count)?;
        let contexts = allocate_contexts(count, 0, limits)?;
        Ok(Self {
            template,
            contexts,
            storage_bytes,
        })
    }

    /// Reset all contexts to `I = 0, MPS = 0`.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).template == old(self).template,
            final(self).storage_bytes == old(self).storage_bytes,
            final(self).contexts@.len() == old(self).contexts@.len(),
            match out {
                Ok(()) => forall|index: int| 0 <= index < final(self).contexts@.len() ==>
                    crate::arithmetic::arithmetic_spec::context_index(final(self).contexts@[index]) == 0
                        && crate::arithmetic::arithmetic_spec::context_mps(final(self).contexts@[index]) == 0,
                Err(_) => final(self).contexts@ == old(self).contexts@,
            },
    ))]
    pub(crate) fn reset(&mut self) -> Result<(), GenericRegionError> {
        validate_refinement_context_layout(self.template, &self.contexts)?;
        reset_contexts(&mut self.contexts);
        Ok(())
    }

    /// Fallibly retain the current statistics for later dictionary use.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(snapshot) =>
                generic_region_spec::refinement_snapshot_view(&snapshot).0 == self.template
                && generic_region_spec::refinement_snapshot_view(&snapshot).2 == self.storage_bytes
                && generic_region_spec::refinement_snapshot_view(&snapshot).1 == self.contexts@
                && generic_region_spec::contexts_valid(
                    generic_region_spec::refinement_snapshot_view(&snapshot).1,
                ),
            Err(_) => true,
        },
    ))]
    pub(crate) fn try_snapshot(
        &self,
        limits: &Limits,
    ) -> Result<RefinementContextSnapshot, GenericRegionError> {
        #[cfg(verus_keep_ghost)]
        proof! { generic_region_spec::refinement_snapshot_view_definition(); }
        validate_refinement_context_layout(self.template, &self.contexts)?;
        let contexts = try_copy_contexts(&self.contexts, self.storage_bytes, limits)?;
        Ok(RefinementContextSnapshot {
            template: self.template,
            contexts,
            storage_bytes: self.storage_bytes,
        })
    }

    /// Restore retained statistics, rejecting a snapshot for another template.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            final(self).template == old(self).template,
            final(self).storage_bytes == old(self).storage_bytes,
            final(self).contexts@.len() == old(self).contexts@.len(),
            match out {
                Ok(()) => final(self).contexts@ == generic_region_spec::refinement_snapshot_view(snapshot).1
                    && generic_region_spec::contexts_valid(final(self).contexts@),
                Err(_) => final(self).contexts@ == old(self).contexts@,
            },
    ))]
    pub(crate) fn restore(
        &mut self,
        snapshot: &RefinementContextSnapshot,
        limits: &Limits,
    ) -> Result<(), GenericRegionError> {
        #[cfg(verus_keep_ghost)]
        proof! { generic_region_spec::refinement_snapshot_view_definition(); }
        if self.template != snapshot.template {
            return Err(GenericRegionError::InvalidFlags(refinement_template_code(
                snapshot.template,
            )));
        }
        validate_refinement_context_layout(snapshot.template, &snapshot.contexts)?;
        validate_snapshot_limits(snapshot.storage_bytes, limits)?;
        if self.contexts.len() != snapshot.contexts.len() {
            return Err(GenericRegionError::ContextIndex {
                index: snapshot.contexts.len(),
                count: self.contexts.len(),
            });
        }
        validate_probability_states(&snapshot.contexts)?;
        let mut index = 0_usize;
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                self.contexts@.len() == snapshot.contexts@.len(),
                index <= self.contexts@.len(),
                forall|completed: int| 0 <= completed < index ==>
                    self.contexts@[completed] == snapshot.contexts@[completed],
                generic_region_spec::contexts_valid(snapshot.contexts@),
            decreases self.contexts@.len() - index,
        ))]
        while index < self.contexts.len() {
            self.contexts[index] = snapshot.contexts[index];
            index += 1;
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(self.contexts@ == snapshot.contexts@);
            generic_region_spec::refinement_snapshot_view_definition();
        }
        Ok(())
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == match template {
        GenericTemplate::Template0 => 65_536usize,
        GenericTemplate::Template1 => 8_192usize,
        GenericTemplate::Template2 | GenericTemplate::Template3 => 1_024usize,
    },
))]
pub(super) fn generic_context_count(template: GenericTemplate) -> usize {
    match template {
        GenericTemplate::Template0 => 65_536,
        GenericTemplate::Template1 => 8_192,
        GenericTemplate::Template2 | GenericTemplate::Template3 => 1_024,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == match template {
        RefinementTemplate::Template0 => 8_192usize,
        RefinementTemplate::Template1 => 1_024usize,
    },
))]
pub(super) fn refinement_context_count(template: RefinementTemplate) -> usize {
    match template {
        RefinementTemplate::Template0 => 8_192,
        RefinementTemplate::Template1 => 1_024,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => contexts@.len() == match template {
            GenericTemplate::Template0 => 65_536usize,
            GenericTemplate::Template1 => 8_192usize,
            GenericTemplate::Template2 | GenericTemplate::Template3 => 1_024usize,
        },
        Err(_) => true,
    },
))]
pub(super) fn validate_generic_context_layout(
    template: GenericTemplate,
    _extended_template: bool,
    contexts: &[ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let expected = generic_context_count(template);
    if contexts.len() != expected {
        return Err(GenericRegionError::ContextIndex {
            index: expected,
            count: contexts.len(),
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => contexts@.len() == match template {
            RefinementTemplate::Template0 => 8_192usize,
            RefinementTemplate::Template1 => 1_024usize,
        },
        Err(_) => true,
    },
))]
pub(super) fn validate_refinement_context_layout(
    template: RefinementTemplate,
    contexts: &[ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let expected = refinement_context_count(template);
    if contexts.len() != expected {
        return Err(GenericRegionError::ContextIndex {
            index: expected,
            count: contexts.len(),
        });
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) => forall|index: int| 0 <= index < contexts@.len() ==>
            crate::arithmetic::arithmetic_spec::context_valid(contexts@[index]),
        Err(_) => true,
    },
    generic_region_spec::probability_validation_result(contexts@, out),
))]
pub(super) fn validate_probability_states(
    contexts: &[ArithmeticContext],
) -> Result<(), GenericRegionError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= contexts@.len(),
            forall|completed: int| 0 <= completed < index ==>
                crate::arithmetic::arithmetic_spec::context_valid(contexts@[completed]),
        decreases contexts@.len() - index,
    ))]
    while index < contexts.len() {
        let context = contexts[index];
        if context.packed() >= 94 {
            let state_index = context.state_index();
            let mps = context.mps();
            let error = DecodeError::Malformed(MalformedError::InvalidArithmeticContextState {
                state_index,
                mps,
            });
            return Err(GenericRegionError::Decode(error));
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out
        == ((generic_region_spec::generic_template_code(template) as u8) << 1)
            | if extended { 0x10 } else { 0 },
))]
pub(super) fn generic_template_code(template: GenericTemplate, extended: bool) -> u8 {
    let template_bits = match template {
        GenericTemplate::Template0 => 0,
        GenericTemplate::Template1 => 1,
        GenericTemplate::Template2 => 2,
        GenericTemplate::Template3 => 3,
    };
    (template_bits << 1) | if extended { 0x10 } else { 0 }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == match template {
        RefinementTemplate::Template0 => 0u8,
        RefinementTemplate::Template1 => 1u8,
    },
))]
pub(super) fn refinement_template_code(template: RefinementTemplate) -> u8 {
    match template {
        RefinementTemplate::Template0 => 0,
        RefinementTemplate::Template1 => 1,
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(()) =>
            storage_bytes <= limits.max_retained_state_bytes
            && storage_bytes as nat * 2
                <= limits.max_decoder_managed_memory_bytes,
        Err(_) => true,
    },
))]
pub(super) fn validate_snapshot_limits(
    storage_bytes: u64,
    limits: &Limits,
) -> Result<(), GenericRegionError> {
    if storage_bytes > limits.max_retained_state_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::RetainedState,
            limit: limits.max_retained_state_bytes,
            actual: storage_bytes,
        })
        .into());
    }
    let Some(combined) = storage_bytes.checked_mul(2) else {
        return Err(integer_overflow());
    };
    if combined > limits.max_decoder_managed_memory_bytes {
        return Err(DecodeError::LimitExceeded(LimitExceeded {
            kind: LimitKind::Memory,
            limit: limits.max_decoder_managed_memory_bytes,
            actual: combined,
        })
        .into());
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(copied) => copied@ == source@ && generic_region_spec::contexts_valid(copied@),
        Err(_) => true,
    },
))]
pub(super) fn try_copy_contexts(
    source: &[ArithmeticContext],
    storage_bytes: u64,
    limits: &Limits,
) -> Result<Vec<ArithmeticContext>, GenericRegionError> {
    validate_probability_states(source)?;
    validate_snapshot_limits(storage_bytes, limits)?;
    let mut copied = Vec::new();
    if copied.try_reserve(source.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: storage_bytes,
        }
        .into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source@.len(),
            copied@ == source@.subrange(0, index as int),
        decreases source@.len() - index,
    ))]
    while index < source.len() {
        copied.push(source[index]);
        index += 1;
    }
    Ok(copied)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(contexts) =>
            contexts@.len() == count
            && forall|index: int| 0 <= index < contexts@.len() ==>
                crate::arithmetic::arithmetic_spec::context_valid(contexts@[index])
                && crate::arithmetic::arithmetic_spec::context_index(contexts@[index]) == 0
                && crate::arithmetic::arithmetic_spec::context_mps(contexts@[index]) == 0,
        Err(_) => true,
    },
))]
pub(super) fn allocate_contexts(
    count: usize,
    bitmap_bytes: u64,
    limits: &Limits,
) -> Result<Vec<ArithmeticContext>, GenericRegionError> {
    if count > MAX_GENERIC_CONTEXTS.max(MAX_REFINEMENT_CONTEXTS) {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let context_bytes = context_storage_bytes(count)?;
    enforce_managed_memory(context_bytes, bitmap_bytes, limits)?;
    let mut contexts = Vec::new();
    if contexts.try_reserve(count).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: context_bytes,
        }
        .into());
    }
    contexts.resize(count, ArithmeticContext::new());
    Ok(contexts)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bytes) => bytes as nat == count as nat * 2,
        Err(_) => count as nat * 2 > u64::MAX,
    },
))]
pub(super) fn context_storage_bytes(count: usize) -> Result<u64, GenericRegionError> {
    let Ok(count) = u64::try_from(count) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    count
        .checked_mul(2)
        .ok_or_else(|| MalformedError::IntegerOverflow.into())
}
