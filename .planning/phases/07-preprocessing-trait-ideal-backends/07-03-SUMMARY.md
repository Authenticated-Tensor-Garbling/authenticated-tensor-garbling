---
phase: 07-preprocessing-trait-ideal-backends
plan: 03
subsystem: preprocessing
tags:
  - preprocessing
  - tests
  - trait-dispatch
  - ideal-backend
  - uncompressed-backend
  - gamma-auth-bits
  - it-mac-invariant
  - PRE-01
  - PRE-02
  - PRE-03
  - PRE-04
requirements:
  - PRE-01
  - PRE-02
  - PRE-03
  - PRE-04
dependency-graph:
  requires:
    - src/preprocessing.rs (post Plan 02 — TensorPreprocessing trait + Uncompressed/Ideal backends exist; IdealPreprocessingBackend populates gamma_auth_bit_shares)
    - src/auth_tensor_pre.rs (verify_cross_party is pub(crate) — re-imported into preprocessing::tests)
  provides:
    - "Phase-gate verification test suite (8 new #[test] functions in src/preprocessing.rs)"
    - "Cross-module import of verify_cross_party into preprocessing::tests"
    - "test_ideal_backend_gamma_auth_bit_shares_mac_invariant as the canonical IT-MAC phase gate"
  affects:
    - src/preprocessing.rs
tech-stack:
  added: []
  patterns:
    - "Cross-module test re-import of pub(crate) helper (verify_cross_party)"
    - "dyn TensorPreprocessing object-safety verification via &dyn reference construction"
    - "Iterative IT-MAC verification across all n*m pairs (no-panic-means-invariant-holds idiom)"
key-files:
  created:
    - .planning/phases/07-preprocessing-trait-ideal-backends/07-03-SUMMARY.md
  modified:
    - src/preprocessing.rs
decisions:
  - "Reused gen_out / eval_out binding names (matching Plan 02) — `gen` remains a reserved keyword in Rust 2024 edition and would break the test module otherwise; documented as an inline test-module comment"
  - "Imported verify_cross_party (pub(crate)) rather than re-implementing the IT-MAC check inline — single source of truth; also silences the cross-party panic pitfall (RESEARCH.md Pitfall 3)"
  - "Used `4 * 4` literal rather than `n * m` constants in test bodies — matches the plan's spec verbatim and keeps each assertion self-describing at the call site"
  - "Accepted the 'distinct-from-correlated' test as structural rather than probabilistic — the plan's own comments acknowledge the deterministic seed makes this a structural check, not a statistical one"
metrics:
  duration: "~3m"
  completed: "2026-04-24"
  tasks: 2
  files_modified: 1
  tests_passing: "82/82 (74 baseline + 4 Task 1 + 4 Task 2)"
---

# Phase 7 Plan 03: Phase-Gate Test Suite for Preprocessing Trait + Backends Summary

One-liner: Adds 8 new `#[test]` functions to `src/preprocessing.rs` that serve as the Phase 7 affirmative gate — verifying trait object dispatch (PRE-01), `IdealPreprocessingBackend` dimensional and `gamma_auth_bit_shares` correctness (PRE-02 + PRE-04), `UncompressedPreprocessingBackend` delegation semantics and stub `gamma_auth_bit_shares` emptiness (PRE-03), and the IT-MAC invariant `mac = key XOR bit * delta` across all 16 gamma share pairs via `verify_cross_party`.

## Objective

The 74 tests inherited from v1.0 only cover pre-existing behavior. They verify no regressions but say nothing affirmative about the new Phase 7 surface: the `TensorPreprocessing` trait, the two backends, and the `gamma_auth_bit_shares` field. This plan plants that affirmative flag — if any of these 8 tests regress in a future plan, the Phase 7 contract is broken, and the failure is immediate and diagnostic.

## Work Completed

### Task 1: Tests for PRE-01 (trait dispatch) + PRE-03 (uncompressed delegation) (commit `3c7666f`)

File: `src/preprocessing.rs`

- Added `use super::{TensorPreprocessing, UncompressedPreprocessingBackend, IdealPreprocessingBackend};` to the `#[cfg(test)] mod tests` import block.
- Appended four new `#[test]` functions:
  1. `test_trait_dispatch_ideal` — constructs `&dyn TensorPreprocessing = &IdealPreprocessingBackend`, calls `backend.run(4, 4, 1, 1)`, asserts `n == 4`, `m == 4` on both returned structs. Confirms the trait is object-safe.
  2. `test_trait_dispatch_uncompressed` — same pattern for `UncompressedPreprocessingBackend`. Confirms both backends share the trait surface.
  3. `test_uncompressed_backend_delegates_to_run_preprocessing` — calls `UncompressedPreprocessingBackend.run(4, 4, 1, 1)` and asserts `correlated_auth_bit_shares.len() == 16`, matching the direct `run_preprocessing` contract.
  4. `test_uncompressed_backend_gamma_field_is_empty` — asserts `gen.gamma_auth_bit_shares.len() == 0` and `eval.gamma_auth_bit_shares.len() == 0` (stub behavior documented until Phase 8).

Per-task verification: `cargo test preprocessing::tests` → 7 passed / 0 failed (3 baseline + 4 new). Full suite: 78 passed / 0 failed.

### Task 2: Tests for PRE-02 + PRE-04 (ideal backend dimensions, gamma length, IT-MAC invariant) (commit `11bb31f`)

File: `src/preprocessing.rs`

- Added `use crate::auth_tensor_pre::verify_cross_party;` to the test import block. `verify_cross_party` is `pub(crate)` in `src/auth_tensor_pre.rs:318` and importable from any module in the crate.
- Appended four new `#[test]` functions:
  5. `test_ideal_backend_dimensions` — asserts `alpha_auth_bit_shares.len() == 4`, `beta_auth_bit_shares.len() == 4`, `correlated_auth_bit_shares.len() == 16`, plus `eval.n == 4`, `eval.m == 4`, `eval.correlated_auth_bit_shares.len() == 16`.
  6. `test_ideal_backend_gamma_auth_bit_shares_length` — asserts `gen.gamma_auth_bit_shares.len() == 16` and `eval.gamma_auth_bit_shares.len() == 16` (both sides).
  7. **`test_ideal_backend_gamma_auth_bit_shares_mac_invariant`** (the phase gate) — iterates `k ∈ 0..16` and calls `verify_cross_party(&gen.gamma_auth_bit_shares[k], &eval.gamma_auth_bit_shares[k], &gen.delta_a, &eval.delta_b)`. No panic on all 16 iterations ⇒ the IT-MAC invariant `mac = key XOR bit * delta` holds for every gamma share pair.
  8. `test_ideal_backend_gamma_distinct_from_correlated` — structural access check that both `gamma_auth_bit_shares` and `correlated_auth_bit_shares` fields exist and index correctly; documents that a deterministic seed makes this a structural (not probabilistic) check.

Per-task verification:
- `cargo test` → **82 passed / 0 failed / 0 ignored** (74 baseline + 4 Task 1 + 4 Task 2).
- `grep -c "#\[test\]" src/preprocessing.rs` → **11** (plan required ≥ 8).
- `grep -c "verify_cross_party" src/preprocessing.rs` → **3** (plan required ≥ 1; 1 import + 1 call + 1 comment reference).
- `grep -c "test_ideal_backend_gamma_auth_bit_shares_mac_invariant" src/preprocessing.rs` → **1** (exact-match requirement met).

## Commits

| # | Task                                                                      | Hash      | Files                 |
|---|---------------------------------------------------------------------------|-----------|-----------------------|
| 1 | Tests for PRE-01 (trait dispatch) + PRE-03 (uncompressed delegation)      | `3c7666f` | src/preprocessing.rs  |
| 2 | Tests for PRE-02 (ideal dimensions) + PRE-04 (gamma length + IT-MAC gate) | `11bb31f` | src/preprocessing.rs  |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `gen` is a reserved keyword in Rust 2024 edition (test module)**

- **Found during:** Task 1, first `cargo test preprocessing::tests` run after appending the four plan-provided test functions.
- **Issue:** The plan's action snippets use `let (gen, eval) = backend.run(4, 4, 1, 1);` and `assert_eq!(gen.n, 4);`. Rust 2024 reserves `gen` as a keyword (generator blocks), so the compiler rejected both the destructuring let-binding and subsequent field access expressions. Errors included:
  ```
  error: expected identifier, found reserved keyword `gen`
  error: expected expression, found reserved keyword `gen`
  ```
  This is the identical pattern Plan 02 encountered in `IdealPreprocessingBackend::run`, which is why its implementation uses `gen_out` / `eval_out`.
- **Fix:** Renamed all `gen` / `eval` bindings to `gen_out` / `eval_out` throughout every new test function (`test_trait_dispatch_ideal`, `test_trait_dispatch_uncompressed`, `test_uncompressed_backend_delegates_to_run_preprocessing`, `test_uncompressed_backend_gamma_field_is_empty`, `test_ideal_backend_dimensions`, `test_ideal_backend_gamma_auth_bit_shares_length`, `test_ideal_backend_gamma_auth_bit_shares_mac_invariant`, `test_ideal_backend_gamma_distinct_from_correlated`). Added inline comments documenting the rename reason.
- **Files modified:** `src/preprocessing.rs` (test module only).
- **Commits:** `3c7666f` (Task 1 tests) and `11bb31f` (Task 2 tests). The rename was applied before committing each task, so both task commits already include the fix.

**2. [Meta — Test-first gate collapse]** This plan was typed `tdd="true"` on both tasks, but the features under test (trait, backends, gamma population) were already implemented in Plan 02. The `<done>` criteria in the plan itself state the tests must pass, not fail. Accordingly the RED phase was collapsed — the tests were written and run green on first successful compile. This is not a deviation from the plan's own intent (the plan explicitly frames these as verification tests); it is a deviation from the generic TDD template's fail-first rule. No new feature was built here — this is the phase-gate verification suite.

No architectural deviations. No user-permission-required changes. No Rule 4 checkpoints triggered.

## TDD Gate Compliance

`git log --oneline 3c7666f 11bb31f` shows two `test(07-03):` commits and zero `feat(07-03):` or `refactor(07-03):` commits. This plan is a pure test-addition plan; the `feat` commits for the features under test are in Plan 01 (`f82c8b3`, `86f6a3b`) and Plan 02 (`4f0bc9d`, `d2f8534`). The RED → GREEN ordering is therefore satisfied across the phase but not within this plan's commit log — this is intentional and described in the Plan 03 `<objective>`.

## Threat Flags

None — this plan only adds test code that imports and exercises existing crate-internal functions. No new network endpoints, no new auth paths, no new file access patterns, no schema changes. The `T-07-05` and `T-07-06` threats from the plan's `<threat_model>` are both mitigated/accepted as planned:

- T-07-05 (direct `share.verify(delta)` in test code → false-negative risk): **mitigated** — all gamma-share verification flows through `verify_cross_party`, never through a direct `share.verify()` call. Test comments explicitly call out the pitfall.
- T-07-06 (fixed-seed determinism in `test_ideal_backend_gamma_distinct_from_correlated`): **accepted** — seed 0 is documented in Plan 02 as the IdealBCot precedent, seed 42 for the gamma RNG is documented in Plan 02 SUMMARY. The test body acknowledges the deterministic-not-probabilistic framing.

## Known Stubs

None introduced by this plan. (Note: the existing `UncompressedPreprocessingBackend` stub behavior — `gamma_auth_bit_shares: vec![]` — is not introduced here; it is verified to still hold by `test_uncompressed_backend_gamma_field_is_empty`, which is the intended behavior until Phase 8.)

## Success Criteria

- [x] All five plan-specified test areas covered with 8 `#[test]` functions (trait dispatch ×2, uncompressed delegation, uncompressed gamma empty, ideal dimensions, ideal gamma length, ideal gamma MAC invariant, ideal gamma distinct from correlated).
- [x] `test_ideal_backend_gamma_auth_bit_shares_mac_invariant` is fully automated and does not panic — confirming `mac = key XOR bit * delta` holds for all 16 gamma pairs.
- [x] `cargo test` exits 0 with all 74 baseline tests green plus all 8 new tests green (82/82 total).
- [x] `grep "#\[test\]" src/preprocessing.rs` shows 11 entries (≥ 8 required).
- [x] `grep "verify_cross_party" src/preprocessing.rs` shows 3 entries (≥ 1 required).
- [x] `grep "test_ideal_backend_gamma_auth_bit_shares_mac_invariant" src/preprocessing.rs` shows exactly 1 entry.
- [x] Phase 7 success criteria from ROADMAP.md are now verified end-to-end by the test suite:
  1. ✓ `TensorPreprocessing` trait exists and is object-safe — verified by `test_trait_dispatch_*`.
  2. ✓ Both `TensorFpreGen` and `TensorFpreEval` compile with `gamma_auth_bit_shares` — implicit in every new test; no compile errors.
  3. ✓ `IdealPreprocessingBackend::run()` returns a `(TensorFpreGen, TensorFpreEval)` pair satisfying the IT-MAC invariant — directly verified by `test_ideal_backend_gamma_auth_bit_shares_mac_invariant`.
  4. ✓ `cargo test` passes with zero regressions — 82/82 green.

## Self-Check: PASSED

Verified:
- FOUND: `src/preprocessing.rs` contains `test_trait_dispatch_ideal`
- FOUND: `src/preprocessing.rs` contains `test_trait_dispatch_uncompressed`
- FOUND: `src/preprocessing.rs` contains `test_uncompressed_backend_delegates_to_run_preprocessing`
- FOUND: `src/preprocessing.rs` contains `test_uncompressed_backend_gamma_field_is_empty`
- FOUND: `src/preprocessing.rs` contains `test_ideal_backend_dimensions`
- FOUND: `src/preprocessing.rs` contains `test_ideal_backend_gamma_auth_bit_shares_length`
- FOUND: `src/preprocessing.rs` contains `test_ideal_backend_gamma_auth_bit_shares_mac_invariant`
- FOUND: `src/preprocessing.rs` contains `test_ideal_backend_gamma_distinct_from_correlated`
- FOUND: `src/preprocessing.rs` contains `use crate::auth_tensor_pre::verify_cross_party;`
- FOUND: commit `3c7666f` on branch `worktree-agent-ae4f075d13a278204` (git log confirmed)
- FOUND: commit `11bb31f` on branch `worktree-agent-ae4f075d13a278204` (git log confirmed)
- `cargo test` returns 82 passed / 0 failed / 0 ignored

No missing items.
