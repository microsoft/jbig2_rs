# Standalone compatibility (0.3 development)

`Decoder::decode_with_policy(bytes, DecodePolicy::Compatibility)` is an explicit
policy selection, not a retry after strict failure. The same operation
is available on `DecoderContext`. These two policy entry points accept byte
slices only; there is no standalone compatibility reader API.
Existing standalone APIs, inspection, and `DecodePolicy::Strict` remain strict.
`EmbeddedPolicy` is independent and unchanged. Random-access files remain strict.
No release-0.2 behavior or API is changed.

The existing ordinary strict reader APIs are unchanged. Applications opting into
compatibility supply already-buffered bytes to `decode_with_policy`; buffering
and bounding their I/O remain the caller's responsibility. The decoder still
applies its stream and preparation limits to the supplied slice.

Success returns `DecodeResult { value, recoveries }`. Every page's image data
must decode completely before either output or diagnostics is returned; only
the explicitly certified final-strip termination exceptions below are allowed.
An empty diagnostic list means no exception was used. Recovered output is **not**
a claim that the original file conforms to T.88. Source bytes are immutable.
Diagnostic byte offsets refer to the original input; inferred page closures and
final entropy boundaries have `byte_offset: None` and identify their page. Page-count recovery makes
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
| #199 | Ignore exactly one CR LF pair after an independently parsed empty, unassociated, reference-free EOF, or a final empty, reference-free EOP on a positive page. The EOP case additionally requires a positive known declared count equal to the observed count, distinct positive closed-page IDs and no open page. Ordinary framing, references, page lifecycle and complete entropy decoding still validate the prefix. Other suffixes remain errors. Sequential organization does not require EOF (D.1); random-access header-table delimitation does (D.2), and an EOF when present must be last (7.4.11). Random-access policy remains strict. |
| #200 | Recover a missing final strip OOB only in a fixed-length, unrefined top-level text region in standalone byte Compatibility. All declared instances must already be fully decoded and placed; normal final OOB is tried first. Huffman requires unambiguous sub-byte zero padding and reports `MissingFinalHuffmanStripEnd`. Arithmetic requires an initialized, genuinely executed prefix certificate independent of both numeric marker-prefix and marker-fill contributions, plus an actually observed final bounded `FF AC`, and reports `MissingFinalArithmeticStripEnd`. No partial symbol, earlier failure, refinement, nested/shared dictionary codec, reader, sink or embedded decode is recovered. Strict and other entry points remain unchanged. 6.4.5, Annexes A/B/E. |
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

Huffman recovery uses a separate table-aware certificate. It runs only after the
last instance's ID and placement succeeded, before any additional symbol can be
read. The verified canonical table counts exclude every prefix length that
could fit the remaining bits. Those bits must also be zero and strictly fewer
than eight. Therefore an EOF while reading the next prefix cannot hide a
matched prefix followed by a truncated value. A decoded real OOB remains a
normal success; any decoded integer remains an overfull-strip error.
Placement batches are flushed and the ordinary exact-count finish still runs.
No bits or compressed marker are synthesized.

The follow-up experiment checked actual terminal state, not just literal suffix
bytes. The arithmetic original observes its final `FF AC` after instance 87 of
90, before the last three complete instance decodes. A discarded prototype
stopping at the declared count plus that observed marker produced the reference
bitmap for the original, but also accepted strict-rejected bodies with four or
more coding bytes
removed while preserving the marker and adjusting the segment length, producing
corrupted complete-size bitmaps. Thus marker observation is not the missing
completion certificate. The Huffman original has one zero bit remaining at the
complete count (bit 1071 of 1072). Its table has no prefix that fits that final
zero bit, satisfying the new stronger certificate. The earlier count/marker or
padding-length-only prototype was not retained.

Arithmetic now uses a separate [prefix-certainty certificate](arithmetic-prefix-certainty.md).
It observes the real integer/IAID bit decoder only in eligible Compatibility
text, without changing entropy results or errors. Its interval includes the
numeric contribution of the first marker `FF`, even before marker recognition,
and every subsequent fill. A threshold crossing or overflow disables it
permanently. A ghost-only actual trace proves that every admissible assignment
of these contributions preserves all prefix bits and probability updates.
The complete placement prefix is snapshotted before final DS; DS must be tried,
but need not itself remain certain. All DS errors propagate unchanged; only a
successfully decoded integer may recover. Every successful body retains a valid
codec. Recovery additionally requires the original
body's literal, bounded final `FF AC`. Placement batches are flushed and the
ordinary exact-count finish runs. The original complete page matches; the
four-byte deletion remains rejected. Shortened alternative encodings that have
a real final OOB remain ordinary successes without diagnostics.

### Pinned reproductions

The test-only upstream revision is
`e7ada1354dff4041cdeed3d8ba56fe01b6186132`; the issue-linked revision is not
resolvable. Generated controls do not depend on upstream implementation behavior.
An optional ignored regression compares every page of the seven accepted pinned
reproductions to packed pixels converted from their reference PNGs.

The pinned #196 reproduction has EOP associations **0, 1, 2** for pages 1, 2, 3.
Each marker has exactly one observed open owner; the last two explicitly name
already-closed earlier pages. It reports one zero repair and two
`ClosedPageEndOfPage` repairs, and all three pages match the reference PNGs.
The pinned #199 reproduction has EOP + CR LF but **no EOF**. Its complete
sequential prefix is valid without EOF; Compatibility reports `TrailingCrLf` at
original byte offset 228 and returns the complete reference honeycomb page.
Strict continues to reject the suffix. The pinned #197, #198,
#201, #202 and both #200 reproductions are eligible. #203 remains rejected.

## Resource and proof contracts

Preparation checks stream, work, retained-state and memory budgets before scanning.
Its conservative work reservation covers copying, header/reference scanning,
ownership searches, page-boundary scans and diagnostic searches. A checked
`64 * source_bytes` memory reservation includes scanner header/reference and
ownership scratch; the observed output-copy allocation remains debited while
rendering. Diagnostics debit their observed vector-capacity growth. Normal
segment, page, reference,
dimension, entropy, bitmap, cumulative-output and decode budgets still apply;
errors may report the remaining effective budget. This deliberately can reject a
file whose strict decode fits a smaller budget.
Compatibility calls release old context scratch before preparation, so an
earlier document's retained allocations cannot escape these budgets.
Potential unrefined arithmetic and Huffman text diagnostics reserve checked vector capacity
before constructing the page collector or decoding any entropy. Observed
capacity growth is debited from memory and retained-state limits; recording a
recovery uses only reserved slots. No additional decoder work cap is introduced.
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
The final-EOP suffix predicate is verified equivalent to the exact
known-count/empty-open/positive-distinct-closed-page model. It runs only for the
exact CR LF candidate and uses the existing quadratic preparation work
reservation and scanner scratch; no additional input or synthetic EOF is added.
The existing strict reader APIs retain their explicitly reviewed I/O limitations;
they do not implement this compatibility policy. No new I/O assumption or trusted
boundary is introduced by the byte-only policy.
