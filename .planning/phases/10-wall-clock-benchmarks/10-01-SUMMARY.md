---
phase: 10-wall-clock-benchmarks
plan: "01"
subsystem: lib
tags: [visibility, pub-fn, benchmark-unblock, consistency-check]
dependency_graph:
  requires: []
  provides:
    - authenticated_tensor_garbling::assemble_c_gamma_shares (pub fn at crate root)
    - authenticated_tensor_garbling::assemble_c_gamma_shares_p2 (pub fn at crate root)
  affects:
    - benches/benchmarks.rs (Plan 03 can now import these via use re-export)
tech_stack:
  added: []
  patterns:
    - Promoted test-only helpers to crate-root pub fns; test module uses `use super::` to re-import
key_files:
  modified:
    - src/lib.rs
decisions:
  - Chose explicit named `use super::{assemble_c_gamma_shares, assemble_c_gamma_shares_p2};` over `use super::*` — more precise, avoids pulling any future crate-root items into test scope unintentionally
  - Kept inline `use crate::keys::Key; use crate::block::Block;` removal from P1 function body — those types are now imported at crate root, so the inline imports were redundant and would generate unused-import warnings
  - No field visibility changes were required — all `AuthTensorGen` and `AuthTensorEval` fields accessed by the helpers were already `pub`
metrics:
  duration: "4m"
  completed: "2026-04-25"
---

# Phase 10 Plan 01: Promote assemble_c_gamma_shares{,_p2} to pub fn at crate root Summary

Promoted `assemble_c_gamma_shares` (P1) and `assemble_c_gamma_shares_p2` (P2) from private `#[cfg(test)]` helpers in `src/lib.rs` to `pub fn`s at the crate root, unblocking Plan 03's benchmark binary from calling them inside the timed `iter_custom` pipeline.

## What Was Done

Both helper functions were moved verbatim from inside `#[cfg(test)] mod tests` to the crate root in `src/lib.rs`:

- **Final visibility:** `pub fn assemble_c_gamma_shares` and `pub fn assemble_c_gamma_shares_p2` at crate root (lines 94 and 206 respectively), both before the `#[cfg(test)]` block.
- **Test-side import style:** Explicit named import — `use super::{assemble_c_gamma_shares, assemble_c_gamma_shares_p2};` — added inside the test module alongside the existing `use crate::...` imports. Existing bare-name call sites in the three test functions resolve unchanged.
- **Inline use removal:** The two inline `use crate::keys::Key; use crate::block::Block;` statements inside the P1 function body were removed; both types are now imported at crate root via the new `use crate::keys::Key;` and the pre-existing `use crate::block::Block;`.
- **Doc comments:** Preserved verbatim from original locations, including the `// SIMULATION ONLY:` annotation.

## Verification Results

- `grep -c 'pub fn assemble_c_gamma_shares' src/lib.rs` → **2** (one per helper, no duplicates)
- Both `pub fn` line numbers (94, 206) are below the `#[cfg(test)]` block start
- `use super::{assemble_c_gamma_shares, assemble_c_gamma_shares_p2};` present at line 486
- `use crate::keys::Key` at crate root (line 32)
- `use crate::block::Block` at crate root (line 28)
- `cargo build --lib --release` — exits 0, no unused-import warnings for the new top-level imports
- `cargo test --lib --tests` — **105 passed, 0 failed** (all 74 baseline + all later-phase tests)
- `cargo bench --no-run` — exits 0, benchmark binary compiles cleanly

## Field Visibility Check

No field visibility changes were required. All `AuthTensorGen` and `AuthTensorEval` fields accessed by the function bodies (`delta_a`, `delta_b`, `alpha_auth_bit_shares`, `beta_auth_bit_shares`, `correlated_auth_bit_shares`, `gamma_d_ev_shares`) were already `pub` from prior phases.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None. This is a pure visibility/relocation change with no new network endpoints, auth paths, file access patterns, or schema changes. The `SIMULATION ONLY` annotation is preserved in doc comments. External callers cannot misuse these functions because constructing `&AuthTensorGen` + `&AuthTensorEval` simultaneously requires full in-process two-party state.

## Self-Check: PASSED

- `src/lib.rs` modified and committed at `88021c1`
- `88021c1` exists in git log
- Both pub fns present at crate root, test module import updated, all 105 tests pass
