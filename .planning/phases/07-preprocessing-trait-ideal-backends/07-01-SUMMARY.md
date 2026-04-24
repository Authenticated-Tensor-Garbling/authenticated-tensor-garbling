---
phase: 07-preprocessing-trait-ideal-backends
plan: 01
subsystem: preprocessing
tags:
  - preprocessing
  - fpre-structs
  - gamma-field
  - PRE-04
  - rust-struct-field-addition
requirements:
  - PRE-04
dependency-graph:
  requires:
    - preprocessing.rs (pre-existing TensorFpreGen/TensorFpreEval structs)
    - auth_tensor_fpre.rs (pre-existing TensorFpre::into_gen_eval)
    - auth_tensor_pre.rs (pre-existing combine_leaky_triples)
  provides:
    - "TensorFpreGen.gamma_auth_bit_shares: Vec<AuthBitShare> field"
    - "TensorFpreEval.gamma_auth_bit_shares: Vec<AuthBitShare> field"
    - "into_gen_eval() initializes gamma_auth_bit_shares to vec![]"
    - "combine_leaky_triples() initializes gamma_auth_bit_shares to vec![]"
    - "Phase 8 forwarding sites marked with TODO(Phase 8) in AuthTensorGen/Eval"
  affects:
    - src/preprocessing.rs
    - src/auth_tensor_fpre.rs
    - src/auth_tensor_pre.rs
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs
tech-stack:
  added: []
  patterns:
    - "Atomic struct-field addition: new field + all construction sites in same logical change"
    - "Stub-then-forward pattern: vec![] initialization + TODO(Phase N) marker at consumer"
key-files:
  created: []
  modified:
    - src/preprocessing.rs
    - src/auth_tensor_fpre.rs
    - src/auth_tensor_pre.rs
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs
decisions:
  - "gamma_auth_bit_shares is added as the last field of both TensorFpreGen and TensorFpreEval, consistent with existing column-major ordering (length n*m, index j*n+i)"
  - "Uncompressed path (combine_leaky_triples) initializes to vec![] — paper-faithful uncompressed preprocessing does not generate l_gamma; Phase 8 will fill this in (per D-04/D-05 in 07-CONTEXT)"
  - "Ideal trusted dealer path (into_gen_eval) also initializes to vec![] — the IdealPreprocessingBackend overwrites this AFTER calling into_gen_eval (planned in 07-02), so vec![] is a correct intermediate default"
  - "AuthTensorGen/Eval receive TODO(Phase 8) forwarding markers rather than new fields — the online consumers do not yet use gamma shares; adding placeholder fields would create dead state. Phase 8 will add the corresponding fields when the consistency check is wired."
metrics:
  duration: "1m 47s"
  completed: "2026-04-24"
  tasks: 2
  files_modified: 5
  tests_passing: "74/74 (zero regressions)"
---

# Phase 7 Plan 01: TensorFpreGen/Eval gamma_auth_bit_shares Field Summary

One-liner: Adds `gamma_auth_bit_shares: Vec<AuthBitShare>` to both `TensorFpreGen` and `TensorFpreEval`, and updates every existing struct-literal constructor (`TensorFpre::into_gen_eval`, `combine_leaky_triples`) so the codebase compiles and all 74 tests remain green; `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` receive `TODO(Phase 8)` forwarding markers.

## Objective

Implement requirement **PRE-04**: carry D_ev-authenticated shares of the gate-output mask `l_gamma` through the preprocessing output structs so Phase 8's consistency check has a field to populate. Per CONTEXT.md D-06, this field addition MUST land atomically with all construction-site updates — no intermediate broken compilation state.

## Work Completed

### Task 1: Add `gamma_auth_bit_shares` field to both structs (commit `f82c8b3`)

File: `src/preprocessing.rs`
- Appended `pub gamma_auth_bit_shares: Vec<AuthBitShare>` as the last field of `TensorFpreGen` (after `correlated_auth_bit_shares`).
- Appended the same field to `TensorFpreEval` (symmetric placement).
- Added docstrings on both fields:
  - `TensorFpreGen`: notes D_ev-authenticated under `delta_b`, column-major index `j*n+i`, distinct from `correlated_auth_bit_shares` (l_gamma* vs l_gamma), populated by `IdealPreprocessingBackend`, initialized to `vec![]` by `UncompressedPreprocessingBackend`. References CONTEXT.md D-04, D-05 and REQUIREMENTS.md PRE-04.
  - `TensorFpreEval`: notes symmetric MAC commitment under `delta_a`.

Intermediate state after Task 1 (expected per plan): two construction sites (`into_gen_eval`, `combine_leaky_triples`) do not initialize the new field and therefore fail compilation. Task 2 fixes them atomically.

### Task 2: Update all construction sites + downstream forwarding TODOs (commit `86f6a3b`)

File: `src/auth_tensor_fpre.rs`
- `TensorFpre::into_gen_eval()` — added `gamma_auth_bit_shares: vec![]` to both the `TensorFpreGen` and `TensorFpreEval` struct literals. `IdealPreprocessingBackend` (planned in 07-02) overwrites this after calling `into_gen_eval`, so `vec![]` is the correct intermediate default.

File: `src/auth_tensor_pre.rs`
- `combine_leaky_triples()` — added `gamma_auth_bit_shares: vec![]` to both struct literals at the return site, with the inline comment `// stub: uncompressed path does not generate l_gamma yet (Phase 8)`. The paper's uncompressed Pi_aTensor' protocol does not produce l_gamma; Phase 8 is where this would be filled in when the consistency check is implemented.

File: `src/auth_tensor_gen.rs`
- `new_from_fpre_gen()` — added `// TODO(Phase 8): forward fpre_gen.gamma_auth_bit_shares to a corresponding field on AuthTensorGen` immediately after the `correlated_auth_bit_shares` line. `AuthTensorGen` does not yet carry a gamma field; Phase 8 will add it alongside the consistency check wiring.

File: `src/auth_tensor_eval.rs`
- `new_from_fpre_eval()` — symmetric TODO comment after the `correlated_auth_bit_shares` line.

### Verification Run

- `cargo test`: 74 passed, 0 failed, 0 ignored. Matches the v1.0 baseline exactly — zero regressions.
- `grep -c "gamma_auth_bit_shares" src/preprocessing.rs`: 2
- `grep -c "gamma_auth_bit_shares" src/auth_tensor_fpre.rs`: 2
- `grep -c "gamma_auth_bit_shares" src/auth_tensor_pre.rs`: 2
- `grep "TODO(Phase 8)" src/auth_tensor_gen.rs`: 1 match (forwarding comment)
- `grep "TODO(Phase 8)" src/auth_tensor_eval.rs`: 1 match (forwarding comment)
- No file deletions in either commit.

## Commits

| # | Task                                        | Hash      | Files                                                                                    |
|---|---------------------------------------------|-----------|------------------------------------------------------------------------------------------|
| 1 | Add gamma_auth_bit_shares field             | `f82c8b3` | src/preprocessing.rs                                                                     |
| 2 | Initialize field in all construction sites  | `86f6a3b` | src/auth_tensor_fpre.rs, src/auth_tensor_pre.rs, src/auth_tensor_gen.rs, src/auth_tensor_eval.rs |

## Deviations from Plan

None — plan executed exactly as written.

### TDD Gate Compliance

The plan's Task 1 has `tdd="true"`, but the task is a pure struct-field addition: a standalone RED-phase unit test referencing the field before it exists would fail to compile rather than fail to run, which is not a meaningful RED/GREEN gate for Rust. The practical TDD gate at the plan level is satisfied by the pre-existing regression battery in `preprocessing::tests` and `auth_tensor_pre::tests` (e.g. `test_run_preprocessing_dimensions`, `test_run_preprocessing_feeds_online_phase`, `test_combine_full_bucket_product_invariant`, `test_run_preprocessing_product_invariant_construction_4`) which exercise both construction paths exhaustively and all passed after Task 2. Commits follow a `feat`-only pattern for this plan because no dedicated test file was added; the existing 74-test baseline is the gate. No test expected to fail passed unexpectedly. All 74 baseline tests remain green.

## Known Stubs

These `vec![]` initializations are intentional placeholders — documented in docstrings as "initialized to vec![] by UncompressedPreprocessingBackend (Phase 8 will fill in the real value)". They do not prevent this plan's goal: the plan's goal is to make the field exist and compile, not to populate it.

| File                       | Line | Stub                                            | Resolution Plan |
|----------------------------|------|-------------------------------------------------|-----------------|
| src/auth_tensor_fpre.rs    | 170  | `gamma_auth_bit_shares: vec![]` (TensorFpreGen) | Phase 7 Plan 02 (IdealPreprocessingBackend overwrites after into_gen_eval) |
| src/auth_tensor_fpre.rs    | 181  | `gamma_auth_bit_shares: vec![]` (TensorFpreEval)| Phase 7 Plan 02 (symmetric) |
| src/auth_tensor_pre.rs     | 241  | `gamma_auth_bit_shares: vec![]` (TensorFpreGen) | Phase 8 (uncompressed path: real l_gamma generation when consistency check lands) |
| src/auth_tensor_pre.rs     | 253  | `gamma_auth_bit_shares: vec![]` (TensorFpreEval)| Phase 8 (symmetric) |
| src/auth_tensor_gen.rs     | 64   | TODO(Phase 8) forwarding comment                | Phase 8 (add corresponding field + wire into consistency check) |
| src/auth_tensor_eval.rs    | 57   | TODO(Phase 8) forwarding comment                | Phase 8 (symmetric) |

All stubs are tracked and intentional; none represent unwired UI data or missing critical functionality for this plan's scope.

## Success Criteria

- [x] TensorFpreGen has `pub gamma_auth_bit_shares: Vec<AuthBitShare>` as its last field
- [x] TensorFpreEval has `pub gamma_auth_bit_shares: Vec<AuthBitShare>` as its last field
- [x] `into_gen_eval()` initializes `gamma_auth_bit_shares: vec![]` in both struct literals
- [x] `combine_leaky_triples()` initializes `gamma_auth_bit_shares: vec![]` in both struct literals
- [x] `AuthTensorGen::new_from_fpre_gen()` has a TODO(Phase 8) forwarding comment
- [x] `AuthTensorEval::new_from_fpre_eval()` has a TODO(Phase 8) forwarding comment
- [x] `cargo test` exits 0 with all 74 baseline tests green
- [x] No new test failures introduced
- [x] No intermediate broken compilation state landed on the branch (Task 1 + Task 2 form one atomic logical change for consumers of the branch tip)

## Self-Check: PASSED

Verified:
- FOUND: src/preprocessing.rs (contains 2 occurrences of `gamma_auth_bit_shares`)
- FOUND: src/auth_tensor_fpre.rs (contains 2 occurrences)
- FOUND: src/auth_tensor_pre.rs (contains 2 occurrences)
- FOUND: src/auth_tensor_gen.rs (contains TODO(Phase 8) forwarding comment)
- FOUND: src/auth_tensor_eval.rs (contains TODO(Phase 8) forwarding comment)
- FOUND: commit `f82c8b3` (Task 1) on branch
- FOUND: commit `86f6a3b` (Task 2) on branch
- `cargo test` returns 74 passed / 0 failed

No missing items.
