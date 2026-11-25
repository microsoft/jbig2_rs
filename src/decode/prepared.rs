//! Explicit, document-owned preparation; ordinary decoder entry points are unchanged.

#[cfg(verus_keep_ghost)]
use super::decode_spec;
use super::{
    decode_global_resources, decode_segments_collected, parse_embedded_document, structural_work,
    DecodeError, DecodedDocument, Decoder, GlobalSegments, InitialResources, Limits,
    MalformedError, MmrScratch, PageCollector,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

mod copying;
use copying::{admission, capacity_bytes, copy_resources, CopyFailure};

#[cfg(verus_keep_ghost)]
#[rustfmt::skip]
#[path = "../../specs/prepared.rs"]
mod prepared_spec;

/// Why an optional prepared owner uses uncached resources.
///
/// These are optimization refusals, not suppressed malformed-input errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum PreparedGlobalsBypass {
    /// There are no decoded pattern or symbol resources to reuse.
    NoResources,
    /// Custom Huffman tables are outside the conservative admission subset.
    CustomTables,
    /// Retained probability contexts require the original owned decoding path.
    RetainedContexts,
    /// Observed resident/copy capacities exceed the additional storage allowance.
    AdditionalStorage,
    /// Reserving an optional per-image resource copy failed.
    CopyAllocation,
}

/// Current state of an explicitly opted-in prepared owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub enum PreparedGlobalsState {
    /// No image has reached global preparation since creation or reset.
    Pending,
    /// Immutable resources are resident and independently copied for each image.
    Ready,
    /// The owner uses uncached resources until reset.
    Uncached(PreparedGlobalsBypass),
}

/// An allocation-free snapshot of a prepared owner's state and extra storage.
///
/// Byte counts are observed vector/bitmap payload capacities, not allocator
/// usable sizes, total process memory, or a replacement for per-image limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct PreparedGlobalsInfo {
    /// Lazy, ready, or explicitly bypassed preparation state.
    pub state: PreparedGlobalsState,
    /// Effective additional owner/copy allowance, separate from image limits.
    pub additional_storage_bytes: u64,
    /// Observed payload capacities currently retained by the immutable owner.
    pub resident_bytes: u64,
}

/// Opt-in immutable global resources owned by one document.
///
/// This owner borrows one exact decoder and global stream for its entire life.
/// Only strict embedded decoding is supported. It never accepts another global
/// identity, policy, or configuration. Each image consumes independent resource
/// copies; probability contexts and custom tables remain uncached.
///
/// The additional allowance is an explicit storage tradeoff, **not** a
/// same-total-memory-budget cache. Existing image quotas remain unchanged.
/// Successful resident/copy admission checks actual payload capacities. An
/// allocator can over-reserve before the post-allocation check rejects and
/// drops an optional copy; this is not an operating-system peak-memory bound.
/// Preparation is lazy so page-validation errors still precede global errors.
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub struct PreparedGlobals<'a> {
    decoder: &'a Decoder,
    globals: &'a GlobalSegments,
    additional_storage_bytes: u64,
    resident_bytes: u64,
    state: PreparedGlobalsState,
    resources: Option<InitialResources>,
}

impl Decoder {
    /// Creates a lazy, document-owned prepared-global decoder.
    ///
    /// No decoding or allocation occurs here. The effective extra allowance is
    /// the requested value capped by this decoder's memory and retained-state
    /// limits. Zero is permitted and takes the ordinary uncached resource path.
    /// Inspect [`PreparedGlobals::info`] for admission or bypass; actual input,
    /// quota, arithmetic and required-allocation errors remain typed errors.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures prepared_spec::fresh(self, globals, additional_storage_bytes, &out),
    ))]
    pub fn prepared_globals<'a>(
        &'a self,
        globals: &'a GlobalSegments,
        additional_storage_bytes: u64,
    ) -> PreparedGlobals<'a> {
        let limits = self.limits();
        let mut allowance = additional_storage_bytes;
        if allowance > limits.max_decoder_managed_memory_bytes {
            allowance = limits.max_decoder_managed_memory_bytes;
        }
        if allowance > limits.max_retained_state_bytes {
            allowance = limits.max_retained_state_bytes;
        }
        let out = PreparedGlobals {
            decoder: self,
            globals,
            additional_storage_bytes: allowance,
            resident_bytes: 0,
            state: PreparedGlobalsState::Pending,
            resources: None,
        };
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        out
    }
}

impl PreparedGlobals<'_> {
    /// Returns state and observed resident bytes without allocation or counters.
    #[must_use]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == prepared_spec::info(self),
    ))]
    pub fn info(&self) -> PreparedGlobalsInfo {
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        PreparedGlobalsInfo {
            state: self.state,
            additional_storage_bytes: self.additional_storage_bytes,
            resident_bytes: self.resident_bytes,
        }
    }

    /// Releases resident resources and permits lazy preparation on the next image.
    ///
    /// The borrowed decoder, globals and effective allowance never change.
    #[cfg_attr(verus_keep_ghost, verus_spec(
        requires prepared_spec::valid(old(self)),
        ensures prepared_spec::valid(final(self)),
            prepared_spec::same_owner(old(self), final(self)),
            prepared_spec::info(final(self)).state == PreparedGlobalsState::Pending,
            prepared_spec::info(final(self)).resident_bytes == 0,
    ))]
    pub fn reset(&mut self) {
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        self.resources = None;
        self.resident_bytes = 0;
        self.state = PreparedGlobalsState::Pending;
    }

    /// Decodes one strict embedded image with these exact immutable globals.
    ///
    /// Per-image parsing, work, decoded-byte and retained-state checks are the
    /// same as [`Decoder::decode_embedded_with_globals`]. Global preparation
    /// succeeds under this owner's exact immutable limits before it is reused.
    /// Optional copy budget/allocation refusals switch to owned uncached
    /// resources **before** page mutation. No image is retried after mutation.
    ///
    /// # Errors
    ///
    /// Returns the ordinary typed input, quota, allocation or decoding error.
    /// An error never returns partial pages or admits failed global preparation.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires prepared_spec::valid(old(self)),
        ensures prepared_spec::valid(final(self)),
            prepared_spec::same_owner(old(self), final(self)),
            match out {
                Ok(document) => crate::document::embedded_metadata(
                    super::decode_spec::document_info(&document)),
                Err(_) => true,
            },
    ))]
    pub fn decode_embedded(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        let limits = self.decoder.limits();
        let (info, segments) = parse_embedded_document(input, limits, Some(self.globals))?;
        let mut collector = PageCollector::new(limits);
        let global_bytes = u64::try_from(self.globals.bytes().len())
            .map_err(|_| MalformedError::IntegerOverflow)?;
        let input_work = structural_work(input.len(), &segments)?
            .checked_add(global_bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        let initial_resources = self.image_resources()?;
        let limits = self.decoder.limits();
        let mut scratch = MmrScratch::default();
        decode_segments_collected(
            &segments,
            &info,
            input_work,
            global_bytes,
            limits,
            &mut scratch,
            Some(initial_resources),
            &mut collector,
            true,
        )?;
        let out = DecodedDocument {
            info,
            pages: collector.pages,
        };
        #[cfg(verus_keep_ghost)]
        production_decode_document_fields!(&out);
        Ok(out)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires prepared_spec::valid(old(self)),
        ensures prepared_spec::valid(final(self)),
            prepared_spec::same_owner(old(self), final(self)),
            match out {
                Ok(resources) => super::decode_spec::initial_resources_valid(&resources),
                Err(_) => true,
            },
    ))]
    fn image_resources(&mut self) -> Result<InitialResources, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        if self.resources.is_none() {
            let resources = decode_global_resources(self.globals.bytes(), self.decoder.limits())?;
            if self.state != PreparedGlobalsState::Pending {
                return Ok(resources);
            }
            // First-image bypasses consume this preparation, not a second decode.
            if let Some(reason) = admission(&resources) {
                self.state = PreparedGlobalsState::Uncached(reason);
                return Ok(resources);
            }
            let resident = capacity_bytes(&resources)?;
            if resident > self.additional_storage_bytes {
                self.state =
                    PreparedGlobalsState::Uncached(PreparedGlobalsBypass::AdditionalStorage);
                return Ok(resources);
            }
            self.resident_bytes = resident;
            self.resources = Some(resources);
            self.state = PreparedGlobalsState::Ready;
        }
        let result = match self.resources.as_ref() {
            Some(resources) => copy_resources(
                resources,
                self.decoder.limits(),
                self.resident_bytes,
                self.additional_storage_bytes,
            ),
            None => {
                return Err(MalformedError::InvalidPageState("missing prepared resources").into());
            }
        };
        match result {
            Ok(resources) => Ok(resources),
            Err(CopyFailure::Decode(error)) => Err(error),
            // No page has been mutated: consume the resident original once.
            Err(CopyFailure::Optional(reason)) => self.transfer_uncached(reason),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires prepared_spec::valid(old(self)),
        ensures prepared_spec::valid(final(self)),
            prepared_spec::same_owner(old(self), final(self)),
            prepared_spec::info(final(self)).state == PreparedGlobalsState::Uncached(reason),
            prepared_spec::info(final(self)).resident_bytes == 0,
            match out {
                Ok(resources) => super::decode_spec::initial_resources_valid(&resources),
                Err(_) => true,
            },
            match old(self).resources {
                Some(resources) => out == Ok(resources),
                None => out == Err(DecodeError::Malformed(
                    MalformedError::InvalidPageState("missing prepared resources"))),
            },
    ))]
    fn transfer_uncached(
        &mut self,
        reason: PreparedGlobalsBypass,
    ) -> Result<InitialResources, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { prepared_spec::definitions(); }
        let resources = self.resources.take();
        self.resident_bytes = 0;
        self.state = PreparedGlobalsState::Uncached(reason);
        match resources {
            Some(resources) => Ok(resources),
            None => Err(MalformedError::InvalidPageState("missing prepared resources").into()),
        }
    }
}

#[cfg(test)]
mod tests;
