# Standalone compatibility (0.3 development)

`Decoder::decode_with_policy(bytes, DecodePolicy::Compatibility)` is an explicit
policy selection, not a retry after strict failure. The same operation
is available on `DecoderContext`; both also provide `decode_reader_with_policy`.
Existing standalone APIs, inspection, and `DecodePolicy::Strict` remain strict.
`EmbeddedPolicy` is independent and unchanged. Random-access files remain strict.
No release-0.2 behavior or API is changed.

Success returns `DecodeResult { value, recoveries }`. Every page and all entropy
data must decode successfully before either output or diagnostics is returned.
An empty diagnostic list means no exception was used. Recovered output is **not**
a claim that the original file conforms to T.88. Source bytes are immutable.
Diagnostic byte offsets refer to the original input; inferred page closures have
`byte_offset: None` and identify their page instead. Page-count recovery makes
`DocumentInfo::declared_page_count()` report the observed count, with an
`ObservedPageCount` diagnostic distinguishing it from source metadata. Segment
counts exclude deliberately skipped type-1 segments; all other metadata is
validated normally.

## Allowlist and dispositions

The local normative source is T.88 (08/2018), also labelled ISO/IEC 14492:2019,
including the earlier amendments. The following are **project compatibility
exceptions**, not interpretations that make malformed syntax normative:

| Issue | Rule and normative context |
|---|---|
| #196 | Reassociate an empty, reference-free EOP only when exactly one positive page is currently open and its number fits the existing association field. The source association must be zero, or explicitly name an already-closed page with a smaller page number. Live, unseen, future-page and ambiguous associations are never overridden. Closed-earlier-page repairs report `ClosedPageEndOfPage`; zero repairs retain their existing kind. T.88 7.2.6, 7.4.9, 8. |
| #197 | Close a missing-EOP page at the next complete page-information segment, or at the exact fully framed stream/EOF boundary. Reject overlapping open pages at a next-page boundary when inferring closures, and any later segment associated with the recovered page. Known dimensions are required; unknown heights need a complete validated final stripe. Striped recovery additionally validates the final stripe and rejects regions following it. No segment or bitmap content is synthesized. 7.4.8–10, 8.2, D.1. |
| #198 | Use the observed page count only after validating framing, references, page ordering, lifecycle, dimensions, limits, and decoding **every** page. D.4.3. |
| #199 | Ignore exactly one CR LF pair only after an independently parsed empty, unassociated, reference-free EOF. Short headers without EOF remain errors; other suffixes remain errors. 7.2, 7.4.11, D.1. |
| #200 | **Deferred, both arithmetic and Huffman remain rejected.** Exact instance count alone is insufficient to certify entropy termination; see below. 6.4.5 symbol-instance loop, 6.4.8, Annexes A/B. |
| #201 | Clear bit `0x10` only when bits 4–7 equal exactly `0x10`. The 2018 text 7.4.1.5 explicitly makes bit 3 colour and bits 4–7 reserved. The colour and composition bits are preserved; other reserved bits still reject. |
| #202 | Skip only type 1 with the short association form, no deferred retention, no references, self-retention clear, and a checked known-length payload wholly inside the source. Its page must already be open, or be zero. All source segment numbers and segment limits are checked even for skipped segments. Normal reference validation then rejects **any** surviving reference to the omitted number, including retention/dependency uses. 7.2, 7.3. Other reserved types remain rejected. |
| #203 | **Rejected.** No new boundary contract is established for unknown-length intermediate generic regions. A marker scan is not a sufficient replacement. 7.2.7, 7.4.6. |

### Why #200 is not an EOF catch

Arithmetic and Huffman need separate termination certificates. In
`symbol_text/text.rs`, the strip loop decodes `SubsequentS` before checking the
completed instance count. An arithmetic value after the final instance therefore
raises `InvalidCount("too many text instances")`; a Huffman prefix can instead
raise `UnexpectedEnd`. Neither error certifies the absence of a partially decoded
integer, symbol, refinement, earlier strip terminator, or truncated entropy.

The shared entropy-codec interface does not expose a proof that the final complete
symbol/refinement ended at the independently known segment boundary, excluding
arithmetic fill/lookahead and Huffman partial-code/padding cases. Implementing
this requires coding-mode-specific boundary state and stronger codec/strip
contracts, not returning success when `decoded == declared`, catching arbitrary
EOF, padding data, or altering dictionary/Huffman behavior. This change does none
of those things and preserves the original typed errors in both cases.

### Pinned reproductions

The test-only upstream revision is
`e7ada1354dff4041cdeed3d8ba56fe01b6186132`; the issue-linked revision is not
resolvable. Generated controls do not depend on upstream implementation behavior.
An optional ignored regression compares every page of the five accepted pinned
reproductions to packed pixels converted from their reference PNGs.

The pinned #196 reproduction has EOP associations **0, 1, 2** for pages 1, 2, 3.
Each marker has exactly one observed open owner; the last two explicitly name
already-closed earlier pages. It reports one zero repair and two
`ClosedPageEndOfPage` repairs, and all three pages match the reference PNGs.
The pinned #199 reproduction has
EOP + CR LF but **no EOF**, so it remains rejected. The pinned #197, #198,
#201 and #202 reproductions are eligible; #200 (both modes) and #203 are not.

## Resource and proof contracts

Preparation checks stream, work, retained-state and memory budgets before scanning.
Its conservative work reservation covers copying, header/reference scanning,
ownership searches, page-boundary scans and diagnostic searches. A checked
`64 * source_bytes` memory reservation includes scanner header/reference and
ownership scratch; the observed output-copy allocation remains debited while
rendering. Diagnostics debit their observed vector-capacity growth. The reader's
owned input allocation is separately debited. Normal segment, page, reference,
dimension, entropy, bitmap, cumulative-output and decode budgets still apply;
errors may report the remaining effective budget. This deliberately can reject a
file whose strict decode fits a smaller budget.
Compatibility calls release old context scratch before preparation; the new
context reader methods do likewise before buffering input, so an earlier
document's retained allocations cannot escape these budgets.
The closed-page witness list is included in the existing scanner scratch
reservation; no limit or work reservation is increased. Each historical lookup
occurs only when the open list has one entry, preserving the existing quadratic
preparation bound.

The policy preparation, editing, page validation, byte wrappers and collected
rendering functions are verified, without new assumptions or trusted bodies.
Contracts cover exact append slices, association-only byte patches, preservation
of colour/composition under masking, monotone budget debits, reconciled page
counts, context cleanup, and a prefix boundary-state model. Its inductive
no-revisit theorem proves that an accepted next-page closure does not skip a
later source segment associated with the recovered page. Ordinary validated
preparation and full decoding still gate successful public results; these are not
proofs of normative validity for repaired input or of compatibility with arbitrary
malformed files.
The EOP ownership contract proves a unique positive open owner, with a closed
smaller-page witness for every nonzero reassociation; successful closure removes
exactly that open entry and appends exactly it to the observed closed history.
Reader I/O uses the existing ordinary Rust `Read` boundary, with runtime checked
length/capacity accounting; it is not a proof of arbitrary reader implementations.
