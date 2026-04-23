---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
plan: 01
subsystem: preprocessing

tags:
  - bucket-size
  - construction-4
  - formula
  - pi-atensor-prime

# Dependency graph
requires:
  - phase: 05-m2-pi-atensor-correct-combining-construction-3
    provides: "`combine_leaky_triples` iterative fold; `bucket_size_for(ell)` Construction 3 signature slated for replacement"
provides:
  - "`bucket_size_for(n, ell)` with Construction 4 formula `B = 1 + ceil(SSP / log2(n*ell))` (SSP=40, fallback for n*ell<=1)"
  - "`run_preprocessing` now identifies as Pi_aTensor' / Construction 4 in its doc header and calls `bucket_size_for(n, count)`"
  - "Unit tests pinned to Construction 4 worked values: (4,1)=21, (4,2)=15, (16,1)=11"
affects:
  - 06-02 (permutation bucketing inside combine_leaky_triples — consumes new bucket sizes)
  - 06-03 (benches — will run against new smaller buckets)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Integer-ceil idiom: `1 + (numerator + denominator - 1) / denominator`"
    - "Saturating arithmetic guard for benchmark-scale inputs: `n.saturating_mul(ell)` before `log2`"

key-files:
  created: []
  modified:
    - src/auth_tensor_pre.rs
    - src/preprocessing.rs

key-decisions:
  - "Single function, no parallel Construction 3 — Construction 3 formula fully deleted (D-01)"
  - "SSP fallback triggers on `n*ell <= 1` (product-based edge case) rather than `ell <= 1` (D-02)"
  - "Corrected plan arithmetic: `bucket_size_for(4, 2) = 15`, not 14 (1 + ceil(40/3) = 1 + 14 = 15)"

patterns-established:
  - "bucket_size_for(n, ell) — two-argument bucket sizer used by all call sites; no dead one-argument version remains"
  - "saturating_mul guard for bench-scale (n, ell) inputs — prevents panic on usize overflow at boundary test cases"

requirements-completed:
  - PROTO-15

# Metrics
duration: 4min
completed: 2026-04-23
---

# Phase 6 Plan 01: `bucket_size_for` Construction 4 Formula Summary

**Replaced `bucket_size_for(ell)` with `bucket_size_for(n, ell)` implementing Construction 4 formula `B = 1 + ceil(SSP / log2(n*ell))`, updated the `run_preprocessing` call site, and pinned unit tests to new worked values (4,1)=21, (4,2)=15, (16,1)=11.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-23T01:00:40Z
- **Completed:** 2026-04-23T01:04:40Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Construction 3 `bucket_size_for(ell)` is gone from the crate — single two-argument function implementing Construction 4 is the only bucket sizer.
- `n.saturating_mul(ell)` prevents integer overflow when benchmarks sweep large `(n, m)`.
- `run_preprocessing` identifies itself as Pi_aTensor' / Construction 4 and passes `n` to the bucket sizer; downstream `combine_leaky_triples` sees `B=21` instead of `B=40` for `n=4, count=1`.
- Six concrete test assertions pin the new formula: three worked values + two SSP-fallback edge cases, plus `test_combine_full_bucket_product_invariant` now exercises a B=21 bucket and the product invariant still holds.
- Full lib test suite: 70/70 passing; `cargo build --release` clean; no new warnings.

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace `bucket_size_for` body with the Construction 4 formula** — `f204900` (refactor)
2. **Task 2: Update `run_preprocessing` call site to pass `n` to `bucket_size_for`** — `fc830d1` (refactor)
3. **Task 3: Rewrite the two `bucket_size_for` unit tests for Construction 4 values** — `d18bdc9` (test)

## Files Created/Modified
- `src/auth_tensor_pre.rs` — `bucket_size_for` signature + body replaced; two `bucket_size_for` tests rewritten to Construction 4 values; two downstream tests (`test_full_pipeline_no_panic`, `test_combine_full_bucket_product_invariant`) updated to pass 2 arguments and assert the new `B=21` for `n=4, ell=1`.
- `src/preprocessing.rs` — `run_preprocessing` doc header updated from "Pi_aTensor, Construction 3" to "Pi_aTensor', Construction 4"; `bucket_size_for(count)` call site updated to `bucket_size_for(n, count)`; internal doc reference updated for consistency.

## Decisions Made
- **Corrected arithmetic bug in plan:** The plan's expected value `bucket_size_for(4, 2) = 14` was wrong (would fail the Construction 4 formula). Correct value is 15: `1 + ceil(40 / log2(8)) = 1 + ceil(40/3) = 1 + 14 = 15`. Both the doc-comment example and the test assertion were corrected. Rationale: the formula is the source of truth (it matches the paper); the plan's expected constant was a transcription error.
- **Doc-comment consistency:** Minor touch-up of an internal doc comment in `run_preprocessing` (`bucket_size_for(count)` → `bucket_size_for(n, count)`) to keep the documentation consistent with the implementation. Not strictly part of the plan's `<action>` block but within the spirit of "no stale Construction-3 references."

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan arithmetic error for `bucket_size_for(4, 2)` — expected 14, correct value is 15**
- **Found during:** Task 3 (unit test rewrite)
- **Issue:** Plan asserted `bucket_size_for(4, 2) == 14` in both the doc comment (Task 1 action) and the test (Task 3 action). The Construction 4 formula `1 + ceil(SSP / log2(n*ell))` with `n*ell = 8` yields `1 + ceil(40 / log2(8)) = 1 + ceil(40/3) = 1 + 14 = 15`. The `+1` in the plan's own comment "(1 + ceil(40/3))" was silently dropped in the asserted constant.
- **Fix:** Updated the test assertion to `== 15` and the doc-comment example to `= 15`, keeping the comment expression `(1 + ceil(40 / log2(8)) = 1 + ceil(40/3) = 1 + 14)` for clarity.
- **Files modified:** `src/auth_tensor_pre.rs` (doc comment on `bucket_size_for`, `test_bucket_size_formula` body)
- **Verification:** `cargo test --lib -- auth_tensor_pre::tests::test_bucket_size_formula` passes; all 7 module tests pass.
- **Committed in:** `d18bdc9` (Task 3 commit)

**2. [Rule 2 - Consistency] Updated stale `bucket_size_for(count)` reference in `run_preprocessing` doc comment**
- **Found during:** Task 2 acceptance-criteria verification
- **Issue:** Task 2 primary action updated the call site (line 93) but a doc-comment reference at line 63 (`"bucket_size_for(count) leaky triples per output triple"`) still named the old one-argument signature. The acceptance criterion `grep -c "bucket_size_for(count)"` required 0 matches.
- **Fix:** Updated the doc reference to `bucket_size_for(n, count)`.
- **Files modified:** `src/preprocessing.rs` (doc comment at line 63)
- **Verification:** `grep -c "bucket_size_for(count)" src/preprocessing.rs` outputs 0; `cargo check --lib` exit 0.
- **Committed in:** `fc830d1` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug in plan data, 1 doc consistency)
**Impact on plan:** The plan's arithmetic bug would have caused `test_bucket_size_formula` to fail and blocked the plan. Both auto-fixes are strictly within the plan's stated scope (Construction 4 values, signature consistency). No scope creep.

## Issues Encountered

- `cargo check --lib` after Task 1 (in isolation) fails with `E0061: this function takes 2 arguments but 1 argument was supplied` at `src/preprocessing.rs:93` — this is the inherent cost of a signature change across two files. Task 2 resolves it. The plan's own Task 1 `<acceptance_criteria>` note ("verify this after Task 3") acknowledged this sequencing. No deviation — expected flow.
- Auth test battery passed as-is after Task 3 because the implementation was installed in Task 1 and the tests were simply pinned to match. No RED-fail phase (the refactor-driven TDD pattern: implementation drives the contract; tests pin the numeric values).

## User Setup Required

None — no external service configuration required. Pure local arithmetic refactor.

## Next Phase Readiness

- Plan 06-02 can begin: `combine_leaky_triples` already has the `_shuffle_seed: u64` parameter stubbed (per Phase 5 D-05/D-06) and the new bucket sizes (e.g., `B=21` for `n=4, ell=1`) are what the permutation step will operate on.
- Plan 06-03 bench comment update (Construction 3 → 4) is independent and will pick up the new formula automatically via `run_preprocessing`.
- No blockers.

## Threat Surface Scan

No new threat surface introduced. The plan's threat model mitigation T-06-01-01 (integer overflow in `n * ell`) is implemented via `saturating_mul` at `src/auth_tensor_pre.rs:138`. T-06-01-02 (off-by-one in integer-ceiling idiom) is mitigated by the pinned unit tests.

## Self-Check: PASSED

- FOUND: src/auth_tensor_pre.rs
- FOUND: src/preprocessing.rs
- FOUND: .planning/phases/06-m2-pi-atensor-permutation-bucketing-construction-4-benches/06-01-SUMMARY.md
- FOUND commit: f204900 (Task 1)
- FOUND commit: fc830d1 (Task 2)
- FOUND commit: d18bdc9 (Task 3)

---
*Phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches*
*Completed: 2026-04-23*
