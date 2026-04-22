---
phase: 03-m2-generalized-tensor-macro-construction-1
fixed_at: 2026-04-21T00:00:00Z
review_path: .planning/phases/03-m2-generalized-tensor-macro-construction-1/03-REVIEW.md
iteration: 1
findings_in_scope: 8
fixed: 7
skipped: 1
status: partial
---

# Phase 03: Code Review Fix Report

**Fixed at:** 2026-04-21
**Source review:** `.planning/phases/03-m2-generalized-tensor-macro-construction-1/03-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 8 (CR-01, CR-02, CR-03, WR-01, WR-02, WR-03, WR-04, WR-05)
- Fixed: 7
- Skipped: 1

## Fixed Issues

### CR-01: Integer underflow panic / wrong assert when `n = 0` in `tensor_evaluator`

**Files modified:** `src/tensor_macro.rs`
**Commit:** 70b212c
**Applied fix:** Added `assert!(n > 0, ...)` as the first statement in both `tensor_garbler` and `tensor_evaluator`, before the `n - 1` usize subtraction. This converts the debug-only panic / release-mode wraparound into a deterministic panic on both sides with a clear message.

---

### CR-02: GGM tree ciphertext tweak collision — no level index in tweaks

**Files modified:** `src/tensor_ops.rs`
**Commit:** cece794
**Applied fix:** Replaced the constant tweaks `0` and `1` with level-indexed tweaks `(i << 1)` (even child) and `(i << 1 | 1)` (odd child) in both `gen_populate_seeds_mem_optimized` and `eval_populate_seeds_mem_optimized`. The key-contribution lines in the garbler and the sibling-reconstruction tweak selection in the evaluator were updated to use the same level-indexed values. All 10 paper-invariant tests pass, confirming garbler/evaluator remain mutually consistent.

---

### CR-03: `matrix.rs` `BitXorAssign` silently corrupts data on dimension mismatch

**Files modified:** `src/matrix.rs`
**Commit:** 45f47a6
**Applied fix:** Added `assert_eq!((self.rows, self.cols), (rhs.rows, rhs.cols), ...)` at the top of `BitXorAssign::bitxor_assign`. This promotes the silent zip-truncation into a hard panic in both debug and release builds, consistent with the existing `debug_assert!` in the owned `BitXor` implementation.

---

### WR-01: `tensor_eval.rs` `evaluate_final_outer_product` XORs a zero matrix

**Files modified:** `src/tensor_eval.rs`
**Commit:** d7c746d
**Applied fix:** Removed the dead `eval_alpha_beta = BlockMatrix::constant(n, m, Block::default())` allocation and the no-op XOR against it. Added a comment documenting that the semi-honest evaluator's correlated share is intentionally zero in this path, and that the authenticated path (`auth_tensor_eval.rs`) handles the non-zero case via `correlated_auth_bit_shares[j*n+i].mac`.

---

### WR-03: `auth_tensor_gen.rs` `gen_chunked_half_outer_product` is `pub` while its eval counterpart is private

**Files modified:** `src/auth_tensor_gen.rs`
**Commit:** 7795945
**Applied fix:** Changed `pub fn gen_chunked_half_outer_product` to `pub(crate) fn gen_chunked_half_outer_product`, restricting access to within the crate and matching the visibility of the evaluator's private counterpart.

---

### WR-04: GGM `tree` accumulator grows to `O(2^(n+1))` before leaves are extracted

**Files modified:** `src/tensor_ops.rs`
**Commit:** 59fea05
**Applied fix:** Removed the `tree: Vec<Block>` accumulator entirely from both `gen_populate_seeds_mem_optimized` and `eval_populate_seeds_mem_optimized`. The `seeds` buffer already holds the correct 2^n leaves after all level-expansion iterations; the final `tree[tree.len()-(1<<n)..].to_vec()` slice was redundant. Returning `seeds` directly eliminates up to ~64 MB of peak allocation for n=20.

---

### WR-05: `tensor_macro.rs` `tensor_evaluator` clones `g.level_cts` unnecessarily

**Files modified:** `src/tensor_ops.rs`, `src/tensor_macro.rs`, `src/tensor_eval.rs`, `src/auth_tensor_eval.rs`
**Commit:** 0ced78f
**Applied fix:** Changed `eval_populate_seeds_mem_optimized`'s `levels` parameter from `Vec<(Block, Block)>` to `&[(Block, Block)>`. Updated all three call sites (`tensor_macro.rs:159`, `tensor_eval.rs:93`, `auth_tensor_eval.rs:95`) to pass `&chunk_levels[s]` / `&g.level_cts` instead of `.clone()`.

---

## Skipped Issues

### WR-02: `matrix.rs` `with_subrows` row-offset arithmetic only correct for column vectors

**File:** `src/matrix.rs:401` (`MatrixViewRef`) and `src/matrix.rs:458` (`MatrixViewMut`)
**Reason:** The proposed `debug_assert!(self.view_cols == 1, ...)` guard was applied and caused 7 test failures immediately. Investigation revealed that `with_subrows` is actively called on multi-column views (`first_half_out` is n×m, `second_half_out` is m×n) in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `tensor_gen.rs`, and `tensor_eval.rs`. The arithmetic is actually correct for multi-column views because the index computation uses `view_start + col * total_rows + row` — the `+offset` to `view_start` shifts the row base uniformly for all columns, which is the correct semantics. The reviewer's concern about `view_start + j*total_rows + offset` was already handled by the column-major index formula. The guard was rolled back via `git checkout -- src/matrix.rs` before committing.

---

_Fixed: 2026-04-21_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
