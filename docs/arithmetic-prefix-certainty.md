# Arithmetic final-strip prefix certainty

This is a project compatibility certificate, not a proof that a missing OOB
conforms to T.88 or a proof of complete JBIG2 output correctness.

## Numeric model and provenance

The actual Annex E decoder's code `C` is the upper endpoint of `[C-W, C]`,
where `W >= 0`. Original input slices and BYTEIN control frames are immutable:
cursor, current byte, stuffing/marker classification and CT are those of the
genuinely executed decoder. Genuine coded-byte contributions are fixed.
Numeric contributions originating at or beyond `entropy_end = len-2`, including
the *first* `FF` prefix, range independently over `[0, actual contribution]`.
Recognized marker-fill contributions also range over that interval.
The uncertainty is numeric; it does not turn a marker into a coded byte.

Initialization is included: first-byte contribution `first << 16`, BYTEIN,
then multiplication by 128 and CT subtraction by 7. With only `FF AC`, both
initial contributions are unknown, so `W=C`; there is no invented coded prefix.
If initialization already consumed a terminal prefix or recognized a marker,
its contribution is included. `initial_enclosure` proves the bounds for every
admissible initial assignment, using the existing initialization model.

## Decision and normalization induction

This decoder subtracts QE from the interval and compares `C >> 16 < QE`.
Set `T = QE << 16`. An active observation requires `W <= C` and excludes
`C >= T && C-W < T`.

* If `C < T`, every alternative code is below T. The decoder selects its
  lower path and interval QE.
* Otherwise every alternative code is at least T. All select the higher path,
  subtract T, and retain the old interval minus QE.

The chosen bit, next state index and next MPS depend only on this common
branch, the interval and the prior context. `decision_independent` proves all
three exact equalities against the existing entry models.

Normalization has an identical interval/CT/cursor schedule for these executions.
For a fixed refill contribution A, endpoints and alternative codes add A,
leaving W unchanged. For an unknown contribution X in `[0,A]`, the actual code
adds A and W adds A. Thus the alternative remains in `[C+A-(W+A), C+A]`.
Doubling code doubles W. These updates occur before the corresponding shift.
`normalization_enclosure` proves the recursive numeric enclosure, the successful
actual normalization signal and representable final endpoints. Alternative
values cannot underflow or overflow because they stay between zero and the
successful actual upper endpoint. Certifier arithmetic failures disable the
observer permanently, without changing the real entropy result/error.

## Genuinely executed trace

`PrefixCertainty::decode` calls the existing actual binary decoder exactly once.
Only then does it observe that decision's pre-frame and extend its ghost history.
Each `Step` records both actual frames, the selected context, returned bit,
actual updated context and before/after widths. `step_valid` ties them to the
existing exact decision model. `observer_valid` ties the connected history to
one actual initialization and one immutable input slice. Nothing in that
history is produced by a permissive second bit decoder.

`alternative_codes_exist` constructs an alternative numeric execution from
every admissible source assignment. Its premises impose no agreement or bounds
on alternative code values. `prefix_enclosure` establishes their bounds by
induction. `arbitrary_fill_sound` then proves equality of every genuinely
executed bit and both probability-context updates. Integer and IAID histories
select the same contexts because earlier bits and bank updates agree.
The certified integer/IAID drivers additionally prove exactly the ordinary
integer/IAID result and complete decoder/context state, including errors.

The proofs use no `assume_specification`, trusted algorithm bodies, new native
interfaces or runtime/proof substitutions.

## Recovery boundary and scope

After the last declared instance's ID, coordinates and placement succeed, the
active prefix is captured **before** decoding final DS. The ghost snapshot is
preserved through that real DS attempt. `saved_arbitrary_fill_sound` remains
valid independently of the later DS outcome, but runtime recovery never
suppresses a DS error: every error propagates unchanged.
A real final OOB is always ordinary success, including when certainty is false.
Only a successfully decoded DS integer may recover, with the saved certified
count and an actually observed
`FF AC` at the original fixed-length body's final two bytes.
Every successful body result retains a valid arithmetic/entropy codec.
No symbol ID or refinement is read after the declared count.
Batches flush and exact-count finish still run.

Only explicit standalone byte Compatibility creates an observer, for top-level
fixed-length unrefined arithmetic text with a literal final `FF AC`.
Strict/default, strict reusable context, reader, sink, embedded, refinement and
nested dictionary calls do not enable it. Constructors leave shared dictionary
observers absent; their exact result/state cycle contracts are preserved.
Ordinary integer/IAID bit loops remain untouched. Diagnostics reserve/debit
capacity before entropy decoding, as for Huffman.

The pinned original has 90 complete instances and packed SHA-256
`e316193144fc23ceccb0c946c47379e5e1555cb876ce045b99f3df3a99a438f7`.
Removing four coded bytes while retaining `FF AC` is not certified and remains
rejected. Cuts 2 and 10 are strict-accepted alternative encodings; they retain
ordinary no-recovery success. These finite contrasts supplement, not replace,
the universal numeric-prefix theorem.
