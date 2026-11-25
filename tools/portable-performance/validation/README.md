# Local setup/correctness evidence — no latency

These are receipts for the prepared infrastructure, not hosted-run timing or
an adoption decision. No `measure`, `once`, `profile`, latency benchmark,
workflow dispatch or push was executed.

* BASE: `14aeac451f6491aa3e413fcbc60aafb11fa75b7c`.
* Candidate used for setup validation:
  `d66db50bca76c8650b29346592151d81427f8323`.
* All129 encoded/global inputs and independently frozen packed hashes/shapes
  passed. The separate BASE-only materializer produced every golden.
* Both revisions passed all492 API configurations, each exercised twice.
  Complete baseline/candidate result arrays are included and compare equal.
* Eight Python fail-closed infrastructure tests and both original Rust
  wrong-pixel/page-count output-gate tests passed.
* Strict correctness-helper Clippy, workflow policy and runtime/source-budget
  audits passed. Existing performance job/steps/permissions and production,
  proof and shared-benchmark sources were checked unchanged.

The108 selected immutable public source containers were individually downloaded,
hash-verified and extracted successfully in prior complete local setup passes.
A later repeated fresh acquisition encountered Google HTTP429; that failure
log is retained. The final replay reused only those previously downloaded
containers whose lengths/hashes matched the independent frozen source records.
`cache-recovery.json` records every recovered path/hash. Acquisition now paces
requests and has bounded, logged `Retry-After` handling, covered by a mocked
HTTP429 test. This is not a claim that the final replay was a cold network run
or that the full timing matrix fits the hosted20-minute budget.

`materialization.json`, `golden-verification.json`, the build records/logs,
correctness arrays, `configuration.json` and `setup.json` are actual completed
setup outputs. `local-validation.json` binds their hashes and source identities.
No PDF, raw public encoded stream, packed bitmap, executable or toolchain is
included. Historical validation files are excluded from measurement inputs;
each manual dispatch must perform its own complete setup.
