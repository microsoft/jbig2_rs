# JBIG2 compatibility decisions for 0.3

These changes target main/0.3 only. They are not backported to release/0.2.
Comparison baseline: `e7f2fffcd0c3f8e844c6740be3e4ac0a63fa3650`.

## Evidence and interpretation

The 28 reports (#191 through #218) identify 32 distinct standalone files.
The commit linked in the reports,
`681f6a378dadd84b83ae35849b7071953e724dac`, does not resolve through GitHub's
commit or tree API. Reproduction instead pins the available public fixture
revision `e7ada1354dff4041cdeed3d8ba56fe01b6186132`, checks each Git blob and
SHA-256, and compares successful complete pages with the pinned reference PNGs.
External streams and standards documents are not redistributed here.

The applicable normative reference is T.88 (08/2018), edition 2, with the
earlier amendments consolidated; the locally checked document identity is in
[source provenance](../validation/source-provenance.json). Clause references
below explain engineering decisions, not new independent normative approval.

Hayro 0.4.0 is a black-box comparison, not a validity oracle. Reproduction
explicitly selects its strict and lenient settings and checks every page,
rather than accepting first-page success as complete-document parity:

| Published Hayro 0.4.0 policy | Decoded files | Errors | Four-second timeouts |
|---|---:|---:|---:|
| Strict | 6 | 23 | 3 |
| Lenient | 26 | 3 | 3 |

Several reports therefore do not describe an input accepted by both decoders'
strict implementations. Some describe missing content, unsupported features or
resource-test inputs. Matching a permissive output is not grounds to invent
symbols, clamp pattern indices, fill missing rows, ignore a necessary extension
or return a partial image as a complete bitmap.

Of the 32 reported files, five now decode strictly and thirteen decode with explicit
Compatibility selected. The remaining 19 retain an intentional malformed,
unsupported or work-limit outcome. The pinned replay checks 160 configurations
and 224 decoder calls, including reuse of the same context; every successful
page must match its complete expected pixels. The cycle-performance follow-up
adds ten default-budget nonprogress configurations, for 170 configurations
and 238 actual calls; those exact Work errors must complete within three seconds.

## Strict decoding corrections

| Reports | Decision and relevant requirement |
|---|---|
| #191 | Parse and validate the entire generic header, including template-specific adaptive coordinates, before scanning an unknown-length coding body. Marker-like coordinate bytes are not delimiters. T.88 7.2.7 and 7.4.6. |
| #192, two files | Symbol imports do not imply probability-context reuse. A clear context-used flag initializes fresh banks; a set flag selects the prescribed last referenced dictionary and validates the retained model. T.88 7.4.2.2. |
| #193 | One pattern needs zero gray planes. Produce zero indices and render pattern zero without initializing arithmetic/MMR entropy state, retaining geometry/work/memory checks. T.88 6.6.5 and Annex C. |
| #194 | Follow B.2 step 5(d)'s stop-at-or-above-high procedure. Preserve the final normal range and the declared upper-range base, even where distinct prefixes encode overlapping values. Do not clamp, move HTHIGH or wrap signed values. Canonical prefix validation and checked 32-bit values remain. |

All five reported strict-success outputs match the full reference images:
163 by 109, including every packed bitmap byte. Generated tests also cover
false header markers, truncation, incompatible/missing retained models,
last-reference selection, zero/nonzero planes, range boundaries and overflow.

## Explicit standalone compatibility

Existing byte/reader/context APIs remain strict by default, with no implicit
retry. The caller-selected standalone byte entry points return the complete
document together with explicit recovery diagnostics; see
[standalone compatibility](standalone-compatibility.md).
New compatibility-reader wrappers are deliberately excluded: they would need
an additional opaque I/O proof interface. Applications use bounded input
acquisition and then select the byte API; existing reader APIs stay strict.

| Reports | Original-file disposition |
|---|---|
| #196 | Strict rejection remains. Compatibility requires exactly one open page and either association zero or an explicit association naming an already-closed, smaller-numbered page. Both ownership witnesses are checked and reported; live, unseen, future and ambiguous targets remain fatal. All three original pages match their reference images. |
| #197 | Compatibility may close a page only at an independently validated complete boundary, without inventing image data. Strict still requires the marker. |
| #198 | Compatibility may reconcile the file count with complete validated pages. Every page remains subject to the original limits and is returned in order. Strict count mismatch remains fatal. |
| #199 | Ignore exactly one CR LF after an independently framed EOF, or after a final empty, reference-free EOP with a positive known page count matching distinct closed pages and no open page. The original sequential fixture now matches the complete honeycomb bitmap, with `TrailingCrLf` at byte 228. D.1 does not require an EOF segment in sequential organization; D.2 requires one for random-access header-table delimitation, and 7.4.11 constrains it when present. Strict still rejects the suffix; arbitrary short headers remain errors. |
| #200, two files | Both remain strict errors. Standalone byte Compatibility recovers the fixed-length unrefined Huffman file using table-aware zero-padding certainty, and the arithmetic file using a verified initialized actual-trace interval certificate independent of numeric marker-prefix/fill contributions. Every declared instance must first be fully decoded and placed. Normal OOB wins without recovery. The arithmetic snapshot precedes final DS and additionally requires the real bounded final `FF AC`; the unsafe count-plus-marker shortcut is not used. Both complete honeycombs match. The four-byte deletion, incomplete prefixes, earlier failures, refinement and nested/shared codecs remain rejected. |
| #201 | The individually documented reserved-bit recovery is explicit compatibility only, not a strict-mode mask or colour-extension waiver. |
| #202 | The individually documented reserved-type recovery requires complete fixed-length framing and no live reference to the skipped segment. It does not ignore arbitrary unknown necessary semantics. |
| #203 | Unknown-length intermediate generic segments remain rejected. The immediate-generic sentinel rule is not extended by guessing a delimiter. T.88 7.2.7. |

Positive and negative generated controls distinguish each recovery capability
from acceptance of a particular original fixture. A diagnostic is not evidence
that unrelated malformed data was validated. There is no partial-rendering
policy in these APIs.

## Rejections that protect output meaning

| Reports | Disposition and controls |
|---|---|
| #195 | The pinned segment is type 22, **immediate**, with two patterns and no required plane data, not an unused valid zero-plane intermediate. It remains malformed. Generated unused/referenced intermediate controls retain decoding and payload validation; T.88 8.2 step 5(b), 6.6 and 7.4.5 do not justify bypassing it. Valid nonzero-plane controls retain their memory-limit errors. |
| #204 | Missing referenced resources remain fatal; no symbol/reference synthesis. T.88 7.2, 7.4 and 8. |
| #205, #206 | Retain refinement reference-type/lifetime rules. Do not substitute the current page or reuse an immediate region as an intermediate resource. T.88 7.4.7 and 8. |
| #207, two files | Out-of-range symbol IDs remain fatal, with no clamping or skipped text region. T.88 6.4. |
| #208 | Out-of-range gray indices remain fatal; selecting another pattern changes pixels. T.88 6.6 and Annex C. |
| #209 | Missing MMR rows remain fatal; no blank padding or cross-segment reads. T.88 6.2 and MMR framing. |
| #210 | Declared segment payloads must be fully present. Compatibility does not convert 430 available bytes into the declared 479. T.88 7.2.7. |
| #211 | Unrepresentable Huffman values remain rejected by checked arithmetic/conversion, not wrapped or clamped. T.88 B.2 limits the integer domain. |
| #212 | Colour requires actual extension semantics and a colour output contract. It remains typed `Unsupported`, not a lenient bi-level success. The original additionally clears file-header colour bit 3 while setting a region's colour bit, contrary to D.4.2; fixing that flag alone cannot implement channel/palette semantics. The supported bi-level profile is unchanged. |
| #213 | An unknown necessary extension cannot yield a complete successful decode. Optional and necessary extension cases remain distinct. T.88 7.4 extension rules. |
| #214 | Preserve the original framing rejection and separately test correctly framed matching/incompatible retained models; the filename does not establish which check was reached. T.88 7.4.2.2 and Annex E. |
| #215, #216 | Preserve legal zero runs/empty classes and the original budgets. A complete decoder/input/bank/cursor recurrence within an actually executed zero-only trace proves that the loop cannot finish, allowing the same eventual `Work` boundary to be reported without billions of repeated iterations. The 100,000-unit-cap errors remain exact; default-budget errors now complete within the three-second regression deadline. No arbitrary repetition limit is introduced. T.88 6.5 and Annex A. |
| #217 | Missing bounded arithmetic termination remains fatal. No cross-section read or invented marker. T.88 6.5, 7.4.2 and Annex E. |
| #218, two files | Both violate the declared new-symbol count, rather than merely the default configured resource ceiling. Keep the typed count failure and independent valid-count/limit controls; do not raise limits to match another decoder. T.88 6.5 and 7.4.2. |

## Verification scope

The same executing sources are checked by the pinned Verus toolchain.
Changed paths cover complete payload windows, checked scan/row limits, fresh
versus retained model selection, zero-plane initialization and bounded recovery
preparation. Exact-pixel regressions, negative controls and work/memory tests
supplement those component contracts.

Local validation including the final-Huffman recovery has 1,424 verified obligations
with zero errors, exact accounting of 710 directly verified production functions
and the unchanged 20
reviewed verifier limitations. There are no incomplete functions, new native
assumptions or runtime/proof substitutions. The 623 native/doctest and 822
audit-tool tests passed for the initial compatibility changes; the cycle
follow-up adds focused full-state/default-budget/overflow regressions and
retains all existing validation requirements. Coverage floors, Rust 1.88
isolated distribution checks and image companion tests remain mandatory.
The ten original strict
conformance outcomes retain identical pixels and typed diagnostics.

Existing proof limitations remain explicit. Neither finite fixture parity nor
component verification establishes a complete decoded-output proof, full
JBIG2 conformance, universal Hayro parity or full PDFium integration results.
All existing native, audit, coverage, compiler, distribution, semantic/contrast
and timing gates remain required; no release or publication is part of this work.
