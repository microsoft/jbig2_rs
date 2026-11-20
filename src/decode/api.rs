use super::{
    decode_global_resources, decode_segments, decode_segments_collected, parse_embedded_document,
    parse_standalone_document, read_bounded_input, structural_work, DecodeError, DecodedDocument,
    Decoder, DecoderContext, DocumentInfo, GlobalSegments, MalformedError, MmrScratch,
    PageCollector, PageSink, Read,
};

#[cfg(verus_keep_ghost)]
use super::decode_spec;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl Decoder {
    /// Decodes a standalone file using an explicit validation policy.
    ///
    /// Compatibility permits uniquely owned zero-associated end-of-page,
    /// complete pages missing end-of-page at exact boundaries, observed page
    /// counts, one CR LF after a validated EOF, reserved region bit 4, and
    /// unretained, unreferenced fixed-length type 1. Random-access files remain
    /// strict. Colour, necessary extensions, entropy errors and truncation are
    /// never waived. Diagnostics accompany only complete successful output.
    /// Preparation, copies and diagnostics consume the existing budgets.
    ///
    /// # Errors
    ///
    /// Returns ordinary typed errors without retry or partial output.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(output) => crate::document::declared_count_matches_pages(
                decode_spec::document_info(&output.value)),
            Err(_) => true,
        },
    ))]
    pub fn decode_with_policy(
        &self,
        input: &[u8],
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        self.context().decode_with_policy(input, policy)
    }

    /// Bounded reader equivalent of [`Decoder::decode_with_policy`].
    ///
    /// The reader's owned input copy is included in memory, retained-state and
    /// work budgets in addition to the compatibility preparation.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors, including input and I/O limits.
    pub fn decode_reader_with_policy<R: Read>(
        &self,
        reader: &mut R,
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        self.context().decode_reader_with_policy(reader, policy)
    }

    /// Decodes embedded PDF/JBIG2 data with an explicit recovery policy.
    ///
    /// See [`Decoder::inspect_embedded_with_policy`] for the allowlist and budget
    /// accounting. Inspect `recoveries` on the returned result to distinguish
    /// normalized metadata from a strict success. No partial image is returned.
    ///
    /// # Errors
    ///
    /// Returns the normal typed errors; there is no implicit retry or fallback.
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(output) => crate::document::embedded_metadata(decode_spec::document_info(&output.value)),
            Err(_) => true,
        },
    ))]
    pub fn decode_embedded_with_policy(
        &self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        policy: crate::EmbeddedPolicy,
    ) -> Result<crate::EmbeddedResult<DecodedDocument>, DecodeError> {
        self.context()
            .decode_embedded_with_policy(input, globals, policy)
    }

    /// Decodes a standalone document into owned pages.
    ///
    /// # Errors
    ///
    /// Returns a typed malformed, unsupported, resource, allocation, I/O, or
    /// sink error.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(document) => crate::document::standalone_metadata(
                input@, decode_spec::document_info(&document),
            ),
            Err(_) => true,
        },
    ))]
    pub fn decode(&self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        self.context().decode(input)
    }

    /// Reads a bounded standalone document and decodes it into owned pages.
    ///
    /// # Errors
    ///
    /// Returns a typed malformed, unsupported, resource, allocation, or I/O
    /// error.
    pub fn decode_reader<R: Read>(&self, reader: &mut R) -> Result<DecodedDocument, DecodeError> {
        let bytes = read_bounded_input(reader, self.limits())?;
        self.context().decode(&bytes)
    }

    /// Decodes a contiguous embedded page stream without external globals.
    ///
    /// For PDF integration, including [PDFium](https://pdfium.googlesource.com/pdfium/),
    /// a complete input may omit the final end-of-page segment. An initially
    /// unknown page height must still be established by a completed final stripe.
    /// This does not permit truncated segment or compressed bitmap data, or
    /// substitute PDF image dimensions.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, unresolved, unsupported, or
    /// over-limit input.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        },
    ))]
    pub fn decode_embedded(&self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        self.context().decode_embedded(input)
    }

    /// Decodes a contiguous embedded page stream with validated globals.
    ///
    /// Final-page completion follows [`Decoder::decode_embedded`].
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed, unresolved, unsupported, or
    /// over-limit input.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        },
    ))]
    pub fn decode_embedded_with_globals(
        &self,
        input: &[u8],
        globals: &GlobalSegments,
    ) -> Result<DecodedDocument, DecodeError> {
        self.context().decode_embedded_with_globals(input, globals)
    }

    /// Decodes a standalone document and emits each completed page.
    ///
    /// # Errors
    ///
    /// Returns a typed malformed, unsupported, resource, allocation, I/O, or
    /// sink error.
    pub fn decode_with_sink<S: PageSink>(
        &self,
        input: &[u8],
        sink: &mut S,
    ) -> Result<DocumentInfo, DecodeError> {
        let (info, segments) = parse_standalone_document(input, self.limits())?;
        let initial_work = structural_work(input.len(), &segments)?;
        let mut scratch = MmrScratch::default();
        decode_segments(
            &segments,
            &info,
            initial_work,
            0,
            self.limits(),
            &mut scratch,
            None,
            sink,
        )?;
        Ok(info)
    }

    /// Reads a bounded standalone document and emits each completed page.
    ///
    /// # Errors
    ///
    /// Returns a typed malformed, unsupported, resource, allocation, I/O, or
    /// sink error.
    pub fn decode_reader_with_sink<R: Read, S: PageSink>(
        &self,
        reader: &mut R,
        sink: &mut S,
    ) -> Result<DocumentInfo, DecodeError> {
        let bytes = read_bounded_input(reader, self.limits())?;
        self.decode_with_sink(&bytes, sink)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        },
    ))]
    pub(super) fn decode_embedded_internal(
        &self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        mmr_scratch: &mut MmrScratch,
    ) -> Result<DecodedDocument, DecodeError> {
        let (info, segments) = parse_embedded_document(input, self.limits(), globals)?;
        let mut collector = PageCollector::new(self.limits());
        let global_bytes = match globals {
            Some(state) => {
                u64::try_from(state.bytes().len()).map_err(|_| MalformedError::IntegerOverflow)?
            }
            None => 0,
        };
        let input_work = structural_work(input.len(), &segments)?
            .checked_add(global_bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        let global_resources = match globals {
            Some(state) => Some(decode_global_resources(state.bytes(), self.limits())?),
            None => None,
        };
        decode_segments_collected(
            &segments,
            &info,
            input_work,
            global_bytes,
            self.limits(),
            mmr_scratch,
            global_resources,
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
}

impl DecoderContext<'_> {
    /// Context equivalent of [`Decoder::decode_with_policy`].
    ///
    /// # Errors
    ///
    /// Returns typed errors and performs normal cleanup on every result.
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_finished(old(self), final(self)),
            match out {
                Ok(output) => crate::document::declared_count_matches_pages(
                    decode_spec::document_info(&output.value)),
                Err(_) => true,
            },
    ))]
    pub fn decode_with_policy(
        &mut self,
        input: &[u8],
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        let result = self.decode_policy_internal(input, policy);
        self.finish_call();
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_state(final(self)).0
            == crate::document::decoder_context_state(old(self)).0,
            match out {
                Ok(output) => crate::document::declared_count_matches_pages(
                    decode_spec::document_info(&output.value)),
                Err(_) => true,
            },
    ))]
    fn decode_policy_internal(
        &mut self,
        input: &[u8],
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        if policy == crate::DecodePolicy::Strict {
            return Ok(crate::DecodeResult {
                value: self.decode_internal(input)?,
                recoveries: Vec::new(),
            });
        }
        self.release_scratch();
        let mut prepared = crate::document::standalone::prepare(input, self.limits())?;
        if prepared.bytes.is_empty() {
            return Ok(crate::DecodeResult {
                value: Decoder::new(prepared.limits).decode(input)?,
                recoveries: Vec::new(),
            });
        }
        let (info, segments) = crate::document::standalone::parse(
            &prepared.bytes,
            &mut prepared.recoveries,
            &mut prepared.limits,
        )?;
        let work = structural_work(prepared.bytes.len(), &segments)?;
        let mut collector = PageCollector::new(&prepared.limits);
        super::orchestration::decode_segments_compatible_collected(
            &segments,
            &info,
            work,
            &prepared.limits,
            &mut self.mmr,
            &mut collector,
            &prepared.recoveries,
        )?;
        let value = DecodedDocument {
            info,
            pages: collector.pages,
        };
        #[cfg(verus_keep_ghost)]
        production_decode_document_fields!(&value);
        Ok(crate::DecodeResult {
            value,
            recoveries: prepared.recoveries,
        })
    }

    /// Context reader equivalent of [`Decoder::decode_reader_with_policy`].
    ///
    /// # Errors
    ///
    /// Returns typed errors and performs normal cleanup on every result.
    pub fn decode_reader_with_policy<R: Read>(
        &mut self,
        reader: &mut R,
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        self.release_scratch();
        let result = self.decode_reader_policy_internal(reader, policy);
        self.finish_call();
        result
    }

    fn decode_reader_policy_internal<R: Read>(
        &self,
        reader: &mut R,
        policy: crate::DecodePolicy,
    ) -> Result<crate::DecodeResult<DecodedDocument>, DecodeError> {
        let (bytes, limits) = crate::document::standalone::read_input(reader, self.limits())?;
        Decoder::new(limits).decode_with_policy(&bytes, policy)
    }

    /// Decodes with the explicit policy of [`Decoder::decode_embedded_with_policy`].
    ///
    /// # Errors
    ///
    /// Returns the same typed errors and always performs normal context cleanup.
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            crate::document::decoder_context_finished(old(self), final(self)),
            match out {
                Ok(output) => crate::document::embedded_metadata(decode_spec::document_info(&output.value)),
                Err(_) => true,
            },
    ))]
    pub fn decode_embedded_with_policy(
        &mut self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        policy: crate::EmbeddedPolicy,
    ) -> Result<crate::EmbeddedResult<DecodedDocument>, DecodeError> {
        let result = self.decode_embedded_policy_internal(input, globals, policy);
        self.finish_call();
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures
            crate::document::decoder_context_state(final(self)).0
                == crate::document::decoder_context_state(old(self)).0,
            match out {
                Ok(output) => crate::document::embedded_metadata(decode_spec::document_info(&output.value)),
                Err(_) => true,
            },
    ))]
    fn decode_embedded_policy_internal(
        &mut self,
        input: &[u8],
        globals: Option<&GlobalSegments>,
        policy: crate::EmbeddedPolicy,
    ) -> Result<crate::EmbeddedResult<DecodedDocument>, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        let prepared = crate::document::embedded::prepare(input, globals, policy, self.limits())?;
        let result = Decoder::new(prepared.limits()).decode_embedded_internal(
            prepared.input(input),
            globals,
            &mut self.mmr,
        );
        prepared.finish(result)
    }

    /// Decodes one standalone document using retained scratch capacity.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::decode`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_finished(old(self), final(self)),
        match out {
            Ok(document) => crate::document::standalone_metadata(
                input@, decode_spec::document_info(&document),
            ),
            Err(_) => true,
        },
    ))]
    pub fn decode(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        let result = self.decode_internal(input);
        self.finish_call();
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_state(final(self)).0
            == crate::document::decoder_context_state(old(self)).0,
        match out {
            Ok(document) => crate::document::standalone_metadata(
                input@, decode_spec::document_info(&document),
            ),
            Err(_) => true,
        },
    ))]
    pub(super) fn decode_internal(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        let (info, segments) = parse_standalone_document(input, self.decoder.limits())?;
        let mut collector = PageCollector::new(self.decoder.limits());
        let initial_work = structural_work(input.len(), &segments)?;
        decode_segments_collected(
            &segments,
            &info,
            initial_work,
            0,
            self.decoder.limits(),
            &mut self.mmr,
            None,
            &mut collector,
            false,
        )?;
        let out = DecodedDocument {
            info,
            pages: collector.pages,
        };
        #[cfg(verus_keep_ghost)]
        production_decode_document_fields!(&out);
        Ok(out)
    }

    /// Decodes one embedded page stream using retained scratch capacity.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::decode_embedded`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_finished(old(self), final(self)),
        match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        },
    ))]
    pub fn decode_embedded(&mut self, input: &[u8]) -> Result<DecodedDocument, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        let result = self
            .decoder
            .decode_embedded_internal(input, None, &mut self.mmr);
        self.finish_call();
        result
    }

    /// Decodes one embedded page stream with validated globals.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`Decoder::decode_embedded_with_globals`].
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures crate::document::decoder_context_finished(old(self), final(self)),
        match out {
            Ok(document) => crate::document::embedded_metadata(decode_spec::document_info(&document)),
            Err(_) => true,
        },
    ))]
    pub fn decode_embedded_with_globals(
        &mut self,
        input: &[u8],
        globals: &GlobalSegments,
    ) -> Result<DecodedDocument, DecodeError> {
        #[cfg(verus_keep_ghost)]
        proof! { crate::document::decoder_context_fields(); }
        let result = self
            .decoder
            .decode_embedded_internal(input, Some(globals), &mut self.mmr);
        self.finish_call();
        result
    }
}
