---
phase: 01-uncompressed-preprocessing
plan: 01-PLAN-matrix-ops-aes
subsystem: matrix, tensor_ops, aes
tags:
  - refactor
  - docs
  - visibility
  - CLEAN-05
  - CLEAN-06
dependency_graph:
  requires: []
  provides:
    - pub(crate) MatrixViewRef and MatrixViewMut (matrix.rs)
    - pub(crate) gen_populate_seeds_mem_optimized and gen_unary_outer_product (tensor_ops.rs)
    - FIXED_KEY_AES doc comment (aes.rs)
  affects:
    - src/auth_tensor_gen.rs (still compiles via pub(crate))
    - src/tensor_gen.rs (still compiles via pub(crate))
    - src/auth_tensor_eval.rs (still compiles via pub(crate))
    - src/tensor_eval.rs (still compiles via pub(crate))
tech_stack:
  added: []
  patterns:
    - pub(crate) visibility narrowing
    - once_cell::sync::Lazy singleton documentation pattern
key_files:
  created: []
  modified:
    - src/tensor_ops.rs
    - src/matrix.rs
    - src/aes.rs
decisions:
  - "Narrowed MatrixViewRef/MatrixViewMut and gen_populate_seeds_mem_optimized/gen_unary_outer_product to pub(crate) — no external consumers exist (verified by grep of benches/, tests/, examples/)"
  - "Added both struct-level and method-level column-major doc comments to TypedMatrix per D-11 (both acceptable)"
  - "FIXED_KEY_AES doc explicitly states fixed key is protocol constant not a secret, and warns against per-session key replacement"
metrics:
  duration: ~5 minutes
  completed: "2026-04-21T21:16:57Z"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 3
requirements_satisfied:
  - CLEAN-05
  - CLEAN-06
---

# Phase 1 Plan matrix-ops-aes: Matrix/TensorOps/AES Visibility and Docs Summary

**One-liner:** Narrowed four internal items to `pub(crate)` (MatrixViewRef, MatrixViewMut, gen_populate_seeds_mem_optimized, gen_unary_outer_product) and added column-major and Lazy/protocol-constant doc comments — zero algorithmic changes, all callers still compile.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Narrow tensor_ops and matrix visibility; add column-major docs | 852c2f8 | src/tensor_ops.rs, src/matrix.rs |
| 2 | Document FIXED_KEY_AES Lazy singleton | 413cf5f | src/aes.rs |

## Verification Results

```
cargo build --lib          → Finished (8 unreachable-pub warnings on pub(crate) struct methods — expected)
cargo build --tests --benches → Finished
cargo test --lib matrix::tests → 12/12 passed
cargo test --lib aes_test  → 1/1 passed
```

**Pre-existing failures (not caused by this plan):**
- `auth_tensor_fpre::tests::test_run_preprocessing_mac_invariants` — pre-existing Bug 1/3 (known algorithmic bugs, documented in PROJECT.md)
- `auth_tensor_pre::tests::test_combine_mac_invariants` — same
- `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants` — same
- `leaky_tensor_pre::tests::test_correlated_mac_invariants` — same

All 4 fail identically on the base commit (db6bd2c) before this plan's changes.

## Spot Check Results

| Check | Expected | Actual |
|-------|----------|--------|
| `pub(crate)` fn count in tensor_ops.rs | 2 | 2 |
| `pub(crate)` struct count in matrix.rs | 2 | 2 |
| Public API items in matrix.rs (TypedMatrix, KeyMatrix, BlockMatrix) | 3 | 3 |
| Column-major doc lines in matrix.rs | >=2 | 2 |
| `once_cell::sync::Lazy` or `protocol constant` in aes.rs | >=2 | 3 |

## Changes Made

### src/tensor_ops.rs
- `pub fn gen_populate_seeds_mem_optimized` → `pub(crate) fn gen_populate_seeds_mem_optimized`
- `pub fn gen_unary_outer_product` → `pub(crate) fn gen_unary_outer_product`

### src/matrix.rs
- Added 8-line `///` doc comment to `TypedMatrix` struct explaining column-major storage, formula `j * rows + i`, and auditing note
- Added 3-line `///` doc comment to `flat_index` method: `index = j * rows + i` where `i` = row, `j` = column
- `pub struct MatrixViewRef` → `pub(crate) struct MatrixViewRef`
- `pub struct MatrixViewMut` → `pub(crate) struct MatrixViewMut`

### src/aes.rs
- Replaced 1-line `/// Fixed-key AES cipher` doc on `FIXED_KEY_AES` with 22-line doc covering:
  - Why `once_cell::sync::Lazy` is used (deferred key expansion, lifetime caching)
  - Thread-safety: `Send + Sync` via once-lock serialization of first-initialization race
  - Fixed key is a **protocol constant** (not a secret); replacing it breaks interoperability

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — this plan makes no behavioral changes and introduces no placeholder values.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes introduced. Visibility narrowing strictly reduces crate API surface.

## Self-Check: PASSED

- `src/tensor_ops.rs` exists and contains `pub(crate) fn gen_populate_seeds_mem_optimized`: confirmed
- `src/matrix.rs` exists and contains `pub(crate) struct MatrixViewRef`: confirmed
- `src/aes.rs` exists and contains `once_cell::sync::Lazy` doc: confirmed
- Commit 852c2f8 exists: confirmed
- Commit 413cf5f exists: confirmed
