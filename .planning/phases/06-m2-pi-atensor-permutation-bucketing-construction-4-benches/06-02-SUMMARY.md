---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
plan: 02
subsystem: preprocessing

tags:
  - permutation
  - bucketing
  - construction-4
  - chacha
  - pi-atensor-prime

# Dependency graph
requires:
  - phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
    plan: 01
    provides: "`bucket_size_for(n, ell)` Construction 4 formula (B=21 for n=4, ell=1)"
  - phase: 05-m2-pi-atensor-correct-combining-construction-3
    provides: "`combine_leaky_triples` iterative fold; `_shuffle_seed: u64` parameter stub"
provides:
  - "`apply_permutation_to_triple(&mut LeakyTriple, &[usize])` pub(crate) helper in `src/auth_tensor_pre.rs` — permutes x-rows and the i-index of Z-rows in lockstep, leaves y-rows untouched"
  - "`combine_leaky_triples` consumes its `shuffle_seed: u64` parameter via per-triple `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j)` + Fisher-Yates (SliceRandom::shuffle) → uniform π_j ∈ S_n"
  - "`run_preprocessing` threads a stable `shuffle_seed = 42` through to `combine_leaky_triples`"
  - "Product invariant under permutation: `test_combine_full_bucket_product_invariant` passes with B=21 and the permutation step active"
affects:
  - 06-03 (benches — will measure Construction 4's permutation-bucketed preprocessing against Construction 3's naive bucketing)

# Tech tracking
tech-stack:
  added:
    - "rand::seq::SliceRandom trait (rand 0.9) — imported into `src/auth_tensor_pre.rs`"
    - "rand::SeedableRng trait (already in deps, first use in this module)"
    - "rand_chacha::ChaCha12Rng (already in deps, first use in this module)"
  patterns:
    - "Per-triple CSPRNG seeding idiom: `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)` — lifts a single master seed into B independent sub-streams"
    - "In-place permutation from a snapshot clone: clone original vecs, then assign by perm index; avoids borrow-checker struggles on nested mut references"
    - "Column-major permutation of a flat Vec: outer j (column) × inner i (row), index `j*n + perm[i]`"
    - "Field-by-field equality helper `shares_eq` for types that don't derive `PartialEq` (AuthBitShare)"

key-files:
  created: []
  modified:
    - src/auth_tensor_pre.rs
    - src/preprocessing.rs

key-decisions:
  - "Rebind `triples` as `mut` inside `combine_leaky_triples` (local shadow) rather than making the parameter `mut triples: Vec<LeakyTriple>` — keeps the public signature clean and localizes mutability to the permutation step"
  - "Use `SliceRandom::shuffle` (Fisher-Yates under the hood) rather than the fallback manual Fisher-Yates — rand 0.9's `rand::seq::SliceRandom` is present and idiomatic; no fallback needed"
  - "Added field-by-field equality helpers `shares_eq` / `slices_eq` to the test module rather than deriving `PartialEq` on `AuthBitShare` — structural changes to `sharing.rs` are out of scope for Plan 02 (and a derive would need `Key` + `Mac` + `bool` all to be `PartialEq`, which they are, but the plan explicitly says 'This fallback is allowed per Claude's Discretion in 06-CONTEXT')"

patterns-established:
  - "apply_permutation_to_triple — the only row-permutation helper for LeakyTriple; all bucketing-amplification code paths must route through it"
  - "Construction 4 permutation-then-fold ordering: permutation runs BEFORE the iterative two_to_one_combine fold (not interleaved), making the product-invariant regression test a reliable sentinel"

requirements-completed:
  - PROTO-13
  - PROTO-14

# Metrics
duration: 3min
completed: 2026-04-23
---

# Phase 6 Plan 02: `combine_leaky_triples` Permutation Step Summary

**Activated the Construction 4 per-triple row-permutation inside `combine_leaky_triples` by adding a `pub(crate) fn apply_permutation_to_triple` helper and a per-triple `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j)` Fisher-Yates shuffle; `run_preprocessing` now passes `shuffle_seed = 42` so the full pipeline is deterministic and the B=21 product-invariant regression test still holds under the active permutation step.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-23T01:08:51Z
- **Completed:** 2026-04-23T01:12:38Z
- **Tasks:** 3 (Task 1 = 2 commits via TDD cycle)
- **Files modified:** 2

## Accomplishments

- `apply_permutation_to_triple(&mut LeakyTriple, &[usize])` is a `pub(crate)` helper with three unit tests (identity no-op, concrete swap of rows 0↔1, wrong-length panic) — 3/3 passing.
- `combine_leaky_triples` now owns the permutation phase: before the iterative fold, for each triple `j`, a fresh `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)` feeds `perm.shuffle(&mut rng)` to draw a uniform π_j ∈ S_n; `apply_permutation_to_triple(triple, &perm)` applies it in place.
- `_shuffle_seed` is fully eliminated from the codebase (`grep -c "_shuffle_seed"` → 0).
- `run_preprocessing` passes `42` through the pipeline; all three `preprocessing::tests` continue to pass.
- `test_combine_full_bucket_product_invariant` (B = `bucket_size_for(4, 1) = 21`) still asserts `z_full == x_full[i] & y_full[j]` with the permutation step active — direct evidence that permuting x-rows and the i-index of Z-rows in lockstep is invariant to the tensor-product relation (because y-rows are untouched).
- Doc header for `combine_leaky_triples` now reads "Pi_aTensor', Construction 4"; the inline `shuffle_seed` doc line was updated from "reserved for future Phase 6" to its active semantics; the iterative-fold comment flipped from "Construction 3" to "Construction 4" for consistency.
- `cargo build --release` clean; `cargo test --lib` → 73/73 passing.

## Task Commits

Each task was committed atomically. Task 1 followed the TDD RED → GREEN cycle:

1. **Task 1 RED: Add failing tests for `apply_permutation_to_triple`** — `634660c` (test)
2. **Task 1 GREEN: Implement `apply_permutation_to_triple`** — `3d91353` (feat)
3. **Task 2: Activate the permutation step inside `combine_leaky_triples`** — `823c786` (refactor)
4. **Task 3: Thread `shuffle_seed = 42` through `run_preprocessing`** — `d2ee910` (refactor)

## Files Created/Modified

- `src/auth_tensor_pre.rs`
  - Added imports `use rand::{SeedableRng, seq::SliceRandom};` and `use rand_chacha::ChaCha12Rng;`.
  - Added `pub(crate) fn apply_permutation_to_triple(triple: &mut LeakyTriple, perm: &[usize])` just before `verify_cross_party`.
  - Renamed `combine_leaky_triples` parameter `_shuffle_seed: u64` → `shuffle_seed: u64`.
  - Updated doc comment (first line + `shuffle_seed` line + iterative-fold comment) from Construction 3 to Construction 4.
  - Inserted the permutation loop (rebind `let mut triples = triples; for (j, triple) in triples.iter_mut().enumerate() { … }`) between the delta-consistency check and the iterative fold.
  - Added three unit tests (`test_apply_permutation_identity_is_noop`, `test_apply_permutation_swap_moves_x_and_z_rows_but_not_y`, `test_apply_permutation_wrong_length_panics`) and two field-by-field equality helpers (`shares_eq`, `slices_eq`) to the existing `#[cfg(test)] mod tests` block.

- `src/preprocessing.rs`
  - One-character change: `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 0)` → `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)`.

## Decisions Made

- **Local `mut` rebinding over mutable parameter.** The plan's Task 2 action explicitly shows `let mut triples = triples;` inside the function body. Chosen over changing the public signature to `mut triples: Vec<LeakyTriple>`. Rationale: signature stability (PROTO-13/14 only mandate the *behavior*); the shadowed local keeps mutability scoped to the single block that needs it.
- **`SliceRandom::shuffle` instead of manual Fisher-Yates.** The plan's `<interfaces>` block provides a fallback path if `rand::seq::SliceRandom` has moved under rand 0.9. It has not moved: the import `use rand::seq::SliceRandom;` compiles cleanly under rand 0.9, and `perm.shuffle(&mut rng)` produces a uniform permutation. No fallback needed.
- **Field-by-field equality helpers instead of `#[derive(PartialEq)]` on `AuthBitShare`.** The plan explicitly notes (Task 1 action, "NOTES" block): "if `AuthBitShare` does NOT derive `PartialEq`, rewrite the test assertions to compare field-by-field … This fallback is allowed per Claude's Discretion in 06-CONTEXT." Confirmed by reading `src/sharing.rs:42` that `AuthBitShare` derives only `Debug, Clone, Default, Copy`. Added `shares_eq` / `slices_eq` helpers in the test module rather than modifying `sharing.rs` — keeps the plan's two-file scope (`src/auth_tensor_pre.rs` + `src/preprocessing.rs`) intact.

## Deviations from Plan

None. The plan executed exactly as written. The PartialEq situation was pre-authorized by the plan's NOTES block (not a deviation — an anticipated branch in the plan that fired).

## Issues Encountered

- After Task 1 GREEN, two unused-import warnings appeared for `SeedableRng` / `seq::SliceRandom` and `ChaCha12Rng`. These are expected: Task 1's action block explicitly adds the imports at module scope (because the helper uses `apply_permutation_to_triple` is needed in the test module too, but the rand + ChaCha imports will actually be consumed only in Task 2's loop body). Task 2 resolved the warnings as scheduled. No deviation.
- `cargo build` has 7 pre-existing warnings in `src/matrix.rs` ("method is never used") unrelated to this plan. Per scope-boundary rule, logged here but not fixed.

## User Setup Required

None — pure local code refactor.

## Next Phase Readiness

- **Plan 06-03 (benches) is unblocked.** The benchmark harness calls `run_preprocessing` unchanged; it will now exercise the Construction 4 permutation step automatically via the `shuffle_seed = 42` threaded through `run_preprocessing`. No bench API changes needed.
- **TEST-06 (Plan 03) reproducibility is guaranteed.** The hard-coded `shuffle_seed = 42` gives deterministic permutations across machines; any snapshot-style assertion Plan 03 writes can be pinned to the B=21 bucket with seed=42.
- **Pipeline end-to-end sanity.** `test_run_preprocessing_feeds_online_phase` passes: the `TensorFpreGen`/`TensorFpreEval` pair produced by the fully-active Construction 4 pipeline feeds `AuthTensorGen::new_from_fpre_gen` / `AuthTensorEval::new_from_fpre_eval` without panic.

## Threat Surface Scan

All threats from the plan's STRIDE register remain mitigated as specified:

- **T-06-02-01** (non-bijective `perm`): Mitigated. Only call site is `perm.shuffle(&mut rng)` after `let mut perm: Vec<usize> = (0..n).collect();` — Fisher-Yates preserves the bijection; Task 1's `test_apply_permutation_wrong_length_panics` pins the length-mismatch panic.
- **T-06-02-02** (out-of-range index in permutation loop): Mitigated by Rust's bounds-check default — no `unsafe` blocks added.
- **T-06-02-03** (observer learns π_j from hard-coded seed 42): Accepted per D-09; preprocessing output is not secret from the holder.
- **T-06-02-04** (large-n DoS via `(0..n).collect()`): Accepted — memory envelope unchanged from existing `LeakyTriple` storage.

No new threat surface introduced beyond the register.

## Self-Check: PASSED

- FOUND: src/auth_tensor_pre.rs (modified)
- FOUND: src/preprocessing.rs (modified)
- FOUND: .planning/phases/06-m2-pi-atensor-permutation-bucketing-construction-4-benches/06-02-SUMMARY.md
- FOUND commit: 634660c (Task 1 RED)
- FOUND commit: 3d91353 (Task 1 GREEN)
- FOUND commit: 823c786 (Task 2)
- FOUND commit: d2ee910 (Task 3)
- `cargo test --lib` → 73 passed / 0 failed
- `cargo build --release` → clean (7 pre-existing unrelated warnings)
- `grep -c "_shuffle_seed" src/auth_tensor_pre.rs` → 0

---
*Phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches*
*Completed: 2026-04-23*
