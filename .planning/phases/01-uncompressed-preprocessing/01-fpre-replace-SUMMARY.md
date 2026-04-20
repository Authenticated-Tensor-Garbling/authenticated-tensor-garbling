---
phase: "01"
plan: "fpre-replace"
subsystem: "preprocessing"
tags: [rust, preprocessing, authenticated-tensor, bcot, leaky-triple]
dependency_graph:
  requires: [01-PLAN-leaky-tensor]
  provides: [run_preprocessing entry point]
  affects: [src/auth_tensor_fpre.rs]
tech_stack:
  added: []
  patterns: [shared-IdealBCot-borrow, Pi_aTensor bucketing via combine_leaky_triples]
key_files:
  created: []
  modified:
    - src/auth_tensor_fpre.rs
decisions:
  - "gen renamed to gen_out in new tests — gen is a reserved keyword in Rust 2024 edition (same fix applied in prior plans)"
  - "count=1 asserted in run_preprocessing — Phase 1 only needs single-triple output; Vec return requires separate design"
metrics:
  duration: "~5 minutes"
  completed: "2026-04-20T08:55:17Z"
---

# Phase 01 Plan fpre-replace: run_preprocessing Entry Point Summary

**One-liner:** Added `run_preprocessing(n, m, count, chunking_factor)` to `auth_tensor_fpre.rs` — one shared `IdealBCot` before the loop ensures all leaky triples share `delta_a`/`delta_b` for valid Pi_aTensor XOR-combination.

## What Was Added

### `run_preprocessing` signature

```rust
pub fn run_preprocessing(
    n: usize,
    m: usize,
    count: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval)
```

Located in `src/auth_tensor_fpre.rs` after the `impl TensorFpre` block.

### Single shared IdealBCot

One `IdealBCot::new(0, 1)` is created before the generation loop. All `bucket_size * count` `LeakyTensorPre` instances borrow `&mut bcot`, ensuring they all use the same `delta_a` and `delta_b`. This satisfies the invariant required by `combine_leaky_triples` (which asserts delta equality at runtime).

### Added use statements

```rust
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
```

### 4 new tests (inside existing `#[cfg(test)] mod tests` block)

| Test | What it checks |
|------|---------------|
| `test_run_preprocessing_dimensions` | n=4, m=4 gives `correlated_auth_bit_shares.len() == 16` for both gen and eval |
| `test_run_preprocessing_delta_lsb` | `gen_out.delta_a.as_block().lsb() == true` (Delta LSB=1 invariant) |
| `test_run_preprocessing_mac_invariants` | Cross-party verify on all alpha/beta/correlated/gamma shares using `AuthBitShare { key: e.key, mac: g.mac, value: g.value }.verify(&delta_b)` pattern |
| `test_run_preprocessing_feeds_online_phase` | `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` accept output without panic |

## Confirmation: No Structs Modified

`TensorFpre`, `TensorFpreGen`, and `TensorFpreEval` struct definitions are byte-for-byte identical to the original. Only new `use` statements, the `run_preprocessing` function, and new tests were added.

## Confirmation: Single Shared IdealBCot

`grep -c "IdealBCot::new" src/auth_tensor_fpre.rs` returns `1` — the single instance before the generation loop.

## Test Results

```
test auth_tensor_fpre::tests::test_tensor_fpre_auth_bits ... ok
test auth_tensor_fpre::tests::test_tensor_fpre_input_sharings ... ok
test auth_tensor_fpre::tests::test_run_preprocessing_dimensions ... ok
test auth_tensor_fpre::tests::test_run_preprocessing_feeds_online_phase ... ok
test auth_tensor_fpre::tests::test_run_preprocessing_mac_invariants ... ok
test auth_tensor_fpre::tests::test_run_preprocessing_delta_lsb ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

Full suite: 45 tests, 0 failures.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `gen` is a reserved keyword in Rust 2024 edition**
- **Found during:** Task 1 (first compile attempt)
- **Issue:** Plan used `gen` as a variable name in new tests; Rust 2024 edition reserves `gen` as a keyword, causing compile errors
- **Fix:** Renamed `gen` to `gen_out` and `eval` to `eval_out` in the three affected tests (`test_run_preprocessing_dimensions`, `test_run_preprocessing_delta_lsb`, `test_run_preprocessing_mac_invariants`)
- **Files modified:** `src/auth_tensor_fpre.rs`
- **Commit:** 4d05872

This same deviation was encountered and documented in the prior plan (STATE.md decision entry), confirming the pattern.

## Known Stubs

None — `run_preprocessing` is fully wired to real `LeakyTensorPre::generate` calls and `combine_leaky_triples`.

## Threat Flags

None — no new network endpoints, auth paths, or trust boundary crossings introduced.

## Self-Check: PASSED

- `src/auth_tensor_fpre.rs` — FOUND, contains `pub fn run_preprocessing`
- Commit `4d05872` — FOUND in git log
- `grep -c "IdealBCot::new" src/auth_tensor_fpre.rs` — returns 1
- All 6 `auth_tensor_fpre::` tests pass
- All 45 project tests pass
