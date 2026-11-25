# Understanding and auditing the JBIG2 models

JBIG2 describes how to turn a compressed stream into a black-and-white page.
The models here make selected parts of that process precise: reading bits,
decoding values, placing symbols and assembling pages. You can review one small
decision without reading the whole decoder or learning the project's tracking
system first.

## Reading path and reference status

- **New to the code?** Start with the [component overview](#component-overview).
- **Ready to check one equation?** Follow the [BYTEIN example](#a-first-audit-the-bytein-refill-step).
- **Looking for notation or proof commands?** Use the [auditor index](../docs/verus-auditor-index.md)
  or [Cargo-Verus instructions](../README.md#run-verus-through-cargo).

**Scope:** a proof that Rust follows a model does not by itself establish
complete standard-to-model correspondence. The models cover selected
properties; the requirement list and functional-proof coverage are partial.

### Free official specification downloads

**Why T.6?** JBIG2 can use MMR (Group 4) compression for bitmap data.
T.6 defines that coding method; T.88 defines how it is used within JBIG2.
We need both to audit the MMR path, not to implement a fax application.

| Reference | Free download from ITU | What to open |
|---|---|---|
| T.88 (08/2018), edition 2.0: JBIG2 | [Download the ZIP containing the specification PDF](https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.88-201808-I!!SOFT-ZST-E&type=items) | `ITU-T_T_88__08_2018.pdf` inside the ZIP |
| T.6 (11/1988): the MMR coding reference | [Download the specification PDF](https://www.itu.int/rec/dologin_pub.asp?lang=e&id=T-REC-T.6-198811-I!!PDF-E&type=items) | The download is the PDF itself |

T.88's download is labelled as a software package on the ITU site, but it
includes the specification PDF. The recommendation pages for
[T.88](https://www.itu.int/rec/T-REC-T.88-201808-I/en) and
[T.6](https://www.itu.int/rec/T-REC-T.6-198811-I/en) provide the surrounding metadata.
Use these editions and check the document hashes in
[source provenance](../validation/source-provenance.json).

The standard documents are external references, not part of this repository
or the published crate. Their use and reproduction terms are separate.
Printed page
numbers may differ from the PDF viewer's page index.

## Component overview

Here is how the parts fit together. Each link is a starting point, not a claim
that the corresponding model covers the component's entire behavior.

| Component | Its job in the decoder | Start reading |
|---|---|---|
| Bits and pixels | Read values from the stream and store or combine bitmap pixels. | [Readers](readers.rs), [bitmaps](bitmap.rs) |
| File structure | Identify segments, their fields and the earlier segments they reference. | [File headers](file_header.rs), [segments](segments.rs) |
| Pages | Track how segments belong to pages, including pages delivered in stripes. | [Documents and pages](document.rs) |
| Arithmetic coding | Recover binary decisions, integers and symbol IDs using adaptive probability contexts. | [Arithmetic models](arithmetic.rs) |
| Huffman coding | Associate bit prefixes with values using standard or custom code tables. | [Huffman models](huffman.rs) |
| MMR | Reconstruct a row using runs of black/white pixels and changes in the preceding row. | [MMR models](mmr.rs) |
| Generic and refinement regions | Decode pixels directly or with help from a reference bitmap. | [Region models](generic_region.rs) |
| Symbols and text | Reuse small bitmap shapes, position them and combine them into an image. | [Symbols and text](symbol_text.rs), [decoding and composition](decode.rs) |
| Halftones | Place patterns on a grid to represent an image. | [Halftone models](halftone.rs) |

For Huffman integer fields, the [adapter models](symbol_text/huffman_codec.rs)
bind each supplied field to its own custom table or A-O cache slot, including
missing bindings. The concrete and enum adapters preserve exact signed values,
OOB, converted errors, consumed cursor and untouched metadata. Literal fields
retain their separate exact bit contracts. These are project-result models:
they do not independently approve Annex B entries, derive upstream wire
selectors, or establish complete dictionary/text raster semantics. The
[readiness assessment](../tools/readiness_assessment.py) remains a gap report,
not an external-validation approval.

## A first audit: the BYTEIN refill step

Try a concrete question: **when a refill recognizes a marker, does it consume
the next byte?**

1. Open the [arithmetic models](arithmetic.rs) and find `refill_step`.
   It describes the next signal, input position, current byte, code register,
   bit count and marker. For this question, concentrate on position and marker.
2. Compare that branch with the recorded reference, **T.88 E.3.4, Figure E.19
   (BYTEIN)**. The [notation guide](../docs/verus-auditor-index.md#arithmetic-notation-and-obligations)
   connects the standard's register names to the code's fields. Read the
   standard first; do not use the Rust implementation as the normative answer.
3. Now open the [Rust arithmetic refill implementation](../src/arithmetic/refill.rs) and find
   `byte_in_signal`. Read its `ensures` conditions: these are the promises
   the implementation proof makes. Check whether its preconditions rule out
   inputs the decoder should support.
4. Open the separate [decoder-register tests](../src/arithmetic/tests/decoder_registers.rs) and find
   `marker_is_strictly_greater_than_8f_and_is_not_consumed`.
   A concrete test helps explain the behavior; it does not replace checking
   the model against the standard.
5. To see the recorded assumptions and remaining gaps, look up
   `T88-ARITHMETIC-BYTEIN-BRANCH` in the
   [requirements ledger](../validation/traceability/requirements.json).
   Caller timing remains a separate question, even if this branch is correct.

A useful review note says **what you compared, what you found, and what you did
not check**. One checked branch is not verification of the entire arithmetic coder.

## What a model does, and does not, promise

Not every definition describes decoded output:

- A **result model** describes an output or state change.
- A **representation invariant** says a state is well formed, not which pixel
  or value should come next.
- A **project policy** describes choices such as resource limits and Rust errors,
  rather than an ITU rule.
- **Proof support** helps Verus reason about the code.

Use `Model navigation` comments for purpose and `Audit` for ledger identity.
`REQUIREMENT CONTEXT` is a reading aid, not a certified translation.
Arithmetic and MMR models refer to implementation tables: agreeing with those
arrays does not establish that the tables agree with the standard.

## When you need more detail

- [Auditor index](../docs/verus-auditor-index.md): notation, entry points and component-specific questions.
- [Traceability guide](../docs/spec-traceability.md): what each evidence label actually establishes.
- [Verifier limitations](../docs/verifier-limitations.md): trusted boundaries and remaining proof gaps.
- [Review procedure](../docs/specification-review.md): property checks, exact snapshots and inventory maintenance.
