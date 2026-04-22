---
phase: 03-m2-generalized-tensor-macro-construction-1
plan: "01"
subsystem: tensor-macro-prerequisites
tags: [refactor, generalization, kernel-hoist, module-skeleton]
depends_on: []
provides: [elements_slice-accessor, generalized-gen-kernel, hoisted-eval-kernels, tensor_macro-skeleton]
affects: [tensor_ops, tensor_gen, tensor_eval, auth_tensor_eval, auth_tensor_gen, matrix, lib]
tech_stack:
  added: []
  patterns: [pub(crate)-free-function, crate-path-delegation, elements_slice-column-vector-pattern]
key_files:
  created:
    - src/tensor_macro.rs
    - .planning/phases/03-m2-generalized-tensor-macro-construction-1/before.txt
  modified:
    - src/matrix.rs
    - src/tensor_ops.rs
    - src/tensor_gen.rs
    - src/auth_tensor_gen.rs
    - src/tensor_eval.rs
    - src/auth_tensor_eval.rs
    - src/lib.rs
decisions:
  - "elements_slice() placed on TypedMatrix<T> as pub(crate) — avoids exposing Vec<T> but gives kernels flat &[T] access to column vectors"
  - "gen_populate_seeds_mem_optimized changed from &MatrixViewRef<Block> to &[Block] — unifies call sites across tensor_gen, auth_tensor_gen, and future tensor_macro"
  - "eval_populate_seeds_mem_optimized return type changed to (Vec<Block>, usize) — hoisted kernel derives missing internally; callers still pass slice_clear to eval_unary_outer_product for behavioral parity"
  - "Unused MatrixViewMut imports pruned from tensor_eval.rs and auth_tensor_eval.rs after private method removal"
metrics:
  duration_minutes: 4
  completed_date: "2026-04-21"
  tasks_completed: 3
  files_changed: 9
---

# Phase 3 Plan 01: Wave-0 Prerequisites — Generalize GGM Kernels + tensor_macro Skeleton

**One-liner:** Generalized gen kernel to `&[Block]`, hoisted duplicate eval kernels from two files into `tensor_ops.rs` as `pub(crate)` free functions, and created the `tensor_macro.rs` module skeleton with `TensorMacroCiphertexts` and stub signatures.

## Tasks Completed

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Baseline snapshot + elements_slice + generalize gen kernel | 87d1527 | before.txt, matrix.rs, tensor_ops.rs, tensor_gen.rs, auth_tensor_gen.rs |
| 2 | Hoist eval kernel to tensor_ops.rs; delegate from eval files | 55e61bc | tensor_ops.rs, tensor_eval.rs, auth_tensor_eval.rs |
| 3 | Create tensor_macro.rs skeleton and register in lib.rs | 792940b | tensor_macro.rs, lib.rs |

## Verification Results

- `cargo build --lib` exits 0
- `cargo test --lib` prints `test result: FAILED. 48 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out` (unchanged from before.txt baseline)
- `tests::test_semihonest_tensor_product` — PASSED (regression gate for gen signature change)
- `tests::test_auth_tensor_product` — PASSED (regression gate for eval hoist)
- `grep -c "^pub mod tensor_macro;" src/lib.rs` returns 1

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Second call site in auth_tensor_gen.rs also needed updating**
- **Found during:** Task 1 Step 4 — cargo build revealed a second call site not mentioned in the plan
- **Issue:** `src/auth_tensor_gen.rs` line 101 also called `gen_populate_seeds_mem_optimized(&slice.as_view(), ...)` which broke after the signature change to `&[Block]`
- **Fix:** Updated the call to `gen_populate_seeds_mem_optimized(slice.elements_slice(), cipher, delta)` — same pattern as tensor_gen.rs
- **Files modified:** src/auth_tensor_gen.rs
- **Commit:** 87d1527

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `tensor_garbler` body | src/tensor_macro.rs:66 | Intentional — body delivered in Plan 02 per phase design |
| `tensor_evaluator` body | src/tensor_macro.rs:82 | Intentional — body delivered in Plan 02 per phase design |

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. All changes are internal `pub(crate)` refactors within an in-process library. T-03-01 (gen signature change) and T-03-02 (eval hoist behavioral equivalence) mitigations confirmed by passing regression tests.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/tensor_macro.rs | FOUND |
| before.txt | FOUND |
| 03-01-SUMMARY.md | FOUND |
| commit 87d1527 | FOUND |
| commit 55e61bc | FOUND |
| commit 792940b | FOUND |
