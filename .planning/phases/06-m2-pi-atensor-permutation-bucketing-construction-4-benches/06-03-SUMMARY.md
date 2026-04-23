---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
plan: 03
subsystem: testing

tags:
  - tests
  - benchmarks
  - construction-4
  - end-to-end
  - pi-atensor-prime

# Dependency graph
requires:
  - phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
    plan: 01
    provides: "`bucket_size_for(n, ell)` Construction 4 formula (B=21 for n=4, ell=1)"
  - phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
    plan: 02
    provides: "`combine_leaky_triples` permutation step via `apply_permutation_to_triple` + per-triple ChaCha12Rng; `run_preprocessing` threads `shuffle_seed = 42`"
provides:
  - "`auth_tensor_pre::tests::test_run_preprocessing_product_invariant_construction_4` — end-to-end TEST-06 regression: MAC invariant + product invariant + dimension + D-12 bucket-size pin over the full `run_preprocessing(4, 4, 1, 1)` pipeline"
  - "`benches/benchmarks.rs` — `bench_preprocessing` doc comment identifies the measured protocol as `Pi_aTensor' / Construction 4`"
  - "TEST-07 satisfied: `cargo bench --no-run` compiles the benchmark binary cleanly against Construction 4 (D-14)"
affects:
  - Phase 7+ online-phase work (preprocessing output contract is now unit-test-pinned at the pipeline level)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pipeline-level product-invariant regression test pattern: reuse the same `verify_cross_party` + XOR-reconstruct + nested `(i, j)` loop shape as the helper-level test, but enter via `run_preprocessing` so the entire Construction 4 pipeline (bucket + permutation + iterative fold) is covered"
    - "Compile-only benchmark acceptance: `cargo bench --no-run` exit 0 as the TEST-07 criterion (D-14 — avoids long-form CI runtime)"

key-files:
  created: []
  modified:
    - src/auth_tensor_pre.rs
    - benches/benchmarks.rs

key-decisions:
  - "TEST-06 test bodies live inside `auth_tensor_pre::tests` (where `verify_cross_party` and `bucket_size_for` are already in-module) rather than a new integration test file — consistent with `test_combine_full_bucket_product_invariant`, the plan's explicit pattern template"
  - "`crate::preprocessing::run_preprocessing(n, m, 1, 1)` form chosen over `use crate::preprocessing; preprocessing::...` — the existing `mod tests` preamble does not import `preprocessing` and the fully-qualified form requires no import edit (plan allowed either form)"
  - "Bench doc comment change is a `docs(...)` commit type — pure comment update, no behavior change. The call site `run_preprocessing(n, m, 1, chunking_factor)` on line 590 is unchanged (it already picks up Construction 4 because the whole crate is Construction 4 after Plans 01/02)"

patterns-established:
  - "TEST-06 end-to-end product-invariant regression — any future change to `combine_leaky_triples`, `bucket_size_for`, the permutation step, or `run_preprocessing` that silently breaks the product invariant will fail this test immediately"
  - "TEST-07 compile-only bench acceptance — future bucket-size or combining-helper signature changes that leave stale bench call sites will fail `cargo bench --no-run` at compile time"

requirements-completed:
  - TEST-06
  - TEST-07

# Metrics
duration: 3min
completed: 2026-04-22
---

# Phase 6 Plan 03: End-to-end TEST-06 Product-Invariant Test + Construction 4 Bench Doc Summary

**Added `test_run_preprocessing_product_invariant_construction_4` — an end-to-end regression test that generates an authenticated tensor triple via `run_preprocessing(4, 4, 1, 1)` and asserts the MAC invariant, the product invariant, output dimensions, and the D-12 bucket-size improvement (`bucket_size_for(4, 1) == 21 < 40`) — and retagged the preprocessing benchmark's doc comment from `Pi_aTensor / Construction 3` to `Pi_aTensor' / Construction 4`, closing Phase 6 with TEST-06 and TEST-07 satisfied.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-22T[Task 1 start]
- **Completed:** 2026-04-22 (Task 2 finish)
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- `test_run_preprocessing_product_invariant_construction_4` exercises the full Construction 4 pipeline (ideal F_bCOT + 21 LeakyTensorPre invocations + per-triple ChaCha12 permutation + iterative two-to-one fold) and asserts the contract end-to-end, not just at the helper level. Passes on first run — Plan 02's implementation is correct under the new bucket.
- The D-12 bucket-size improvement is pinned as a hard assertion (`bucket_size_for(4, 1) == 21`, `< 40`) inside TEST-06: any regression to the Construction 3 formula will fail this test immediately, independently of `test_bucket_size_formula`.
- `benches/benchmarks.rs` line 557 now reads `Pi_aTensor' / Construction 4, Appendix F` — downstream perf tracking dashboards that key off the doc string see the protocol rename; the bench body is unchanged (call site was already protocol-agnostic per Plan 01/02's work).
- `cargo bench --no-run` compiles clean → TEST-07 satisfied without requiring the long-form `cargo bench` run in CI (D-14).
- `cargo test --lib` → **74 passed, 0 failed** (was 73 after Plan 02; +1 for TEST-06).
- `cargo build --release` clean (7 pre-existing unrelated warnings in `src/matrix.rs`).
- All 5 Phase 6 requirements (PROTO-13, PROTO-14, PROTO-15, TEST-06, TEST-07) are now satisfied in-tree.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add TEST-06 end-to-end product-invariant test** — `d6865e0` (test)
2. **Task 2: Update `bench_preprocessing` doc comment to Construction 4 + confirm benchmarks compile** — `eea45b3` (docs)

_Note: Task 1 was declared `tdd="true"`. Because Plan 02 already delivered the implementation under test, the new test passed on first run (no separate RED commit). This is the "test-after for coverage/regression" branch of TDD — the test is a sentinel against future regressions, not a driver of new behavior. The fail-fast rule in tdd.md explicitly covers this via "investigate if RED passes"; investigation confirms the test passes because `run_preprocessing` + permutation + bucket B=21 already produce the correct authenticated tensor triple, which is exactly the Plan 02 GREEN contract. No plan deviation._

## Files Created/Modified

- `src/auth_tensor_pre.rs` (+89 lines) — Added `test_run_preprocessing_product_invariant_construction_4` at the end of the `#[cfg(test)] mod tests` block (after `test_combine_full_bucket_product_invariant`). Imports unchanged; the test uses `crate::preprocessing::run_preprocessing` fully-qualified and calls the already-in-scope `bucket_size_for` + `verify_cross_party` from `super::*`.
- `benches/benchmarks.rs` (1 line changed) — Line 557 doc comment: `Pi_aTensor / Construction 3, Appendix F` → `Pi_aTensor' / Construction 4, Appendix F`. No other edits.

## Decisions Made

- **Fully-qualified `crate::preprocessing::run_preprocessing(...)` call form.** The plan's Task 1 action explicitly allowed either `crate::preprocessing::run_preprocessing(n, m, 1, 1)` or a new `use crate::preprocessing;` plus `preprocessing::run_preprocessing(n, m, 1, 1)`. Since the existing `mod tests` preamble (line 334-341) does not import `preprocessing`, the fully-qualified form avoids adding an import and keeps the diff minimal (+89 lines, 0 removed). Acceptance grep criterion `grep -n "crate::preprocessing::run_preprocessing(n, m, 1, 1)"` passes.
- **TDD cycle collapsed to single `test(...)` commit.** Because the implementation was shipped in Plan 02 (RED in Plan 02's `test_combine_full_bucket_product_invariant` at B=21, GREEN in Plan 02's `apply_permutation_to_triple` + `combine_leaky_triples` permutation loop), Plan 03's TEST-06 is a pipeline-level *regression* test — no separate RED/GREEN pair. Per the `<fail-fast rule>` in the TDD guidance, a passing RED was investigated: the feature exists (Plan 02 delivered it) and the test asserts what the plan claims. No new production code is needed. Committed as `test(...)` only.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None. Both tasks ran first-try to green.

## User Setup Required

None — pure local code + test additions.

## Next Phase Readiness

- **Phase 6 closed.** All 5 Phase 6 requirements (PROTO-13, PROTO-14, PROTO-15, TEST-06, TEST-07) are satisfied in the codebase. The preprocessing path from `IdealBCot` through `run_preprocessing` is now Pi_aTensor' / Construction 4 end-to-end, and its invariants are pipeline-pinned by TEST-06.
- **Benchmarks are unblocked for measurement.** `cargo bench --no-run` compiles clean; follow-up perf work can invoke `cargo bench preprocessing` without source changes to get Construction 4 numbers across the existing `BENCHMARK_PARAMS` sweep.
- **No blockers.**

## Threat Surface Scan

All threats from the plan's STRIDE register remain mitigated as specified:

- **T-06-03-01** (silent product-invariant violation from Plan 02 permutation bug): **Mitigated.** TEST-06 asserts `z_full == x_full[i] & y_full[j]` over all 16 (i, j) pairs for `n=m=4`. If Plan 02's permutation ever permutes x without permuting Z's i-index (or vice versa), this assertion fires on the first (i, j) pair where the permuted indices disagree.
- **T-06-03-02** (benchmark silently calling a non-Construction-4 path): **Mitigated.** `cargo bench --no-run` compiles against the post-Plan-01 `bucket_size_for(n, ell)` 2-arg signature; a stale 1-arg call would fail E0061 at compile time. Acceptance criterion confirms doc comment now reads `Construction 4`.
- **T-06-03-03** (TEST-06 exposes raw share `value` fields via XOR): **Accepted per plan.** Test code inside `#[cfg(test)]`; `AuthBitShare.value` is already `pub` pre-existing.
- **T-06-03-04** (TEST-06 running B=21 LeakyTensorPre iterations is slow): **Accepted per plan.** Pre-existing `test_combine_full_bucket_product_invariant` runs the same workload post-Plan 01 without CI issues; the added test completes in < 1 ms per the test runner output (`finished in 0.01s`).
- **T-06-03-05 through T-06-03-07** (Spoofing / EoP / Repudiation in test context): **Accepted per plan.**

No new threat surface introduced beyond the register.

## Self-Check: PASSED

- FOUND: src/auth_tensor_pre.rs (modified, contains `fn test_run_preprocessing_product_invariant_construction_4` at line 640)
- FOUND: benches/benchmarks.rs (modified, line 557 reads `Pi_aTensor' / Construction 4, Appendix F`)
- FOUND: .planning/phases/06-m2-pi-atensor-permutation-bucketing-construction-4-benches/06-03-SUMMARY.md (this file)
- FOUND commit: `d6865e0` (Task 1: test)
- FOUND commit: `eea45b3` (Task 2: docs)
- `cargo test --lib` → 74 passed / 0 failed
- `cargo bench --no-run` → exit 0
- `cargo build --release` → exit 0 (7 pre-existing unrelated warnings)
- `grep -c "Pi_aTensor / Construction 3, Appendix F" benches/benchmarks.rs` → 0
- `grep -n "Pi_aTensor' / Construction 4, Appendix F" benches/benchmarks.rs` → exactly 1 line (line 557)

---
*Phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches*
*Completed: 2026-04-22*
