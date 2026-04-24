---
phase: 08-open-protocol-1-garble-eval-check
plan: 01
subsystem: mpc
tags: [rust, mpc, authenticated-garbling, protocol-1, online-phase, tensor-gates, it-mac, compute_lambda_gamma]

# Dependency graph
requires:
  - phase: 07-preprocessing-trait-ideal-backends
    provides: gamma_auth_bit_shares field on TensorFpreGen/TensorFpreEval (length n*m, column-major, D_ev-authenticated l_gamma shares)
provides:
  - AuthTensorGen.gamma_auth_bit_shares field populated from TensorFpreGen via new_from_fpre_gen
  - AuthTensorEval.gamma_auth_bit_shares field populated from TensorFpreEval via new_from_fpre_eval
  - AuthTensorGen::compute_lambda_gamma() -> Vec<bool> implementing D-04 formula
  - AuthTensorEval::compute_lambda_gamma(&[bool]) -> Vec<bool> implementing D-05 formula
  - First #[cfg(test)] mod tests block in auth_tensor_eval.rs (closes TESTING.md coverage gap)
  - Phase 7 TODO(Phase 8) comments removed from both files
affects:
  - 08-02 (check_zero in online.rs — consumes gamma_auth_bit_shares via combined c_gamma shares)
  - 08-03 (end-to-end Protocol 1 test — calls compute_lambda_gamma on both sides)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TDD RED/GREEN cycle: write failing tests first, then implement to pass"
    - "Column-major j*n+i indexing for n*m output vecs — consistent across garbler and evaluator"
    - "Panic assertion guarding compute_lambda_gamma against empty gamma_auth_bit_shares (UncompressedPreprocessingBackend stub)"
    - "AuthBitShare::bit() is delta-independent — D_ev-authenticated shares yield correct extbit despite paper D_gb notation"

key-files:
  created: []
  modified:
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs

key-decisions:
  - "gamma_auth_bit_shares forwarded by move (not clone) from TensorFpre{Gen,Eval} — consistent with all other field forwarding in new_from_fpre_{gen,eval}"
  - "compute_lambda_gamma uses j-outer i-inner loop so output Vec is built in lockstep with j*n+i — avoids separate index computation"
  - "Panic message names UncompressedPreprocessingBackend explicitly as the empty-vec offender — aids debuggability"
  - "D_ev vs D_gb delta note preserved verbatim in both doc comments per Pitfall 1 in 08-RESEARCH.md"

patterns-established:
  - "Protocol step method naming: compute_lambda_gamma mirrors garble_final/evaluate_final naming convention"
  - "MUST-call-after doc comment pattern: both methods document the post-garble_final/evaluate_final ordering constraint"

requirements-completed: [P1-01, P1-02]

# Metrics
duration: 25min
completed: 2026-04-23
---

# Phase 8 Plan 01: gamma_auth_bit_shares field forwarding + compute_lambda_gamma on AuthTensorGen and AuthTensorEval

**Forwarded Phase 7 gamma_auth_bit_shares into both online protocol structs and implemented the D-04/D-05 masked-output formulas as compute_lambda_gamma methods, closing the Protocol 1 garble/eval loop and opening the TESTING.md coverage gap in auth_tensor_eval.rs.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-23T00:00:00Z
- **Completed:** 2026-04-23T00:25:00Z
- **Tasks:** 2 (both TDD)
- **Files modified:** 2

## Accomplishments

- `AuthTensorGen` gains `pub gamma_auth_bit_shares: Vec<AuthBitShare>` field populated from `TensorFpreGen` in `new_from_fpre_gen`, plus `compute_lambda_gamma() -> Vec<bool>` implementing D-04: `[L_gamma]^gb[j*n+i] = first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()`
- `AuthTensorEval` gains the symmetric field from `TensorFpreEval` in `new_from_fpre_eval`, plus `compute_lambda_gamma(&[bool]) -> Vec<bool>` implementing D-05: `L_gamma[j*n+i] = lambda_gb[j*n+i] XOR first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()`
- Both `TODO(Phase 8)` comments in `auth_tensor_gen.rs:64` and `auth_tensor_eval.rs:57` are removed
- First `#[cfg(test)] mod tests` block created in `auth_tensor_eval.rs` — closes the TESTING.md coverage gap flagged in the plan
- 6 new tests added (3 per file); full suite grows from 82 to 88, all passing

## Task Commits

Each task was committed with TDD RED then GREEN commits:

1. **Task 1 RED: test(08-01) — failing tests for AuthTensorGen compute_lambda_gamma** - `f4fc2f1`
2. **Task 1 GREEN: feat(08-01) — gamma_auth_bit_shares + compute_lambda_gamma on AuthTensorGen** - `15897ff`
3. **Task 2 RED: test(08-01) — failing tests for AuthTensorEval compute_lambda_gamma** - `7e088f0`
4. **Task 2 GREEN: feat(08-01) — gamma_auth_bit_shares + compute_lambda_gamma on AuthTensorEval** - `44758ce`

_TDD tasks have test commit (RED) followed by feat commit (GREEN)._

## TDD Gate Compliance

- RED gate: `test(08-01)` commits exist for both tasks — tests failed to compile before implementation (field/method not found errors confirmed)
- GREEN gate: `feat(08-01)` commits exist after RED commits — all tests pass after implementation
- REFACTOR gate: not needed — implementation was clean on first pass

## Files Created/Modified

- `src/auth_tensor_gen.rs` — Added `pub gamma_auth_bit_shares: Vec<AuthBitShare>` field, `Vec::new()` init in `new()`, `fpre_gen.gamma_auth_bit_shares` forwarding in `new_from_fpre_gen`, `compute_lambda_gamma()` method, and 3 new tests (`test_compute_lambda_gamma_dimensions`, `test_compute_lambda_gamma_uses_column_major`, `test_compute_lambda_gamma_full_consistency`)
- `src/auth_tensor_eval.rs` — Same structural changes mirrored for eval side: field, inits, forwarding, `compute_lambda_gamma(&[bool])` method, and first-ever `#[cfg(test)] mod tests` block with 3 tests (`test_compute_lambda_gamma_reconstruction`, `test_compute_lambda_gamma_xors_three_inputs`, `test_compute_lambda_gamma_panics_on_wrong_lambda_length`)

## Decisions Made

- Forwarded `gamma_auth_bit_shares` by move (not clone) — consistent with all other field forwarding in the constructors; `TensorFpre{Gen,Eval}` is consumed at this callsite anyway
- Used `j`-outer `i`-inner loop ordering in `compute_lambda_gamma` so the output `Vec` is built sequentially in `j*n+i` order — avoids a separate index variable and matches the column-major convention of `garble_final`/`evaluate_final`
- Preserved the D_ev vs D_gb delta note verbatim in both doc comments per 08-RESEARCH.md Pitfall 1 — `AuthBitShare::bit()` is delta-independent (`self.value`), so D_ev-authenticated shares yield correct `extbit` despite paper writing D_gb

## Deviations from Plan

None — plan executed exactly as written. All 5 edits per task matched the plan's `<action>` specifications. The three tests per task matched the plan's `<behavior>` specifications verbatim.

## Issues Encountered

None. Compilation errors on RED phase were as expected (field/method not found). GREEN phase compiled and passed on first attempt for both tasks.

## Known Stubs

None. `gamma_auth_bit_shares` is fully populated via `IdealPreprocessingBackend` in all new tests. The `UncompressedPreprocessingBackend` legitimately leaves this vec empty (Phase 7 stub) — the panic assertion with explicit message is the intended guard, not a silent placeholder.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. All changes are pure in-memory Rust struct field additions and method implementations. T-08-01 through T-08-03 mitigations from the plan's threat model are implemented:
- T-08-01: `assert_eq!(gamma_auth_bit_shares.len(), n*m)` panic with explicit message on both methods
- T-08-02: Delta-independence of `AuthBitShare::bit()` documented in both doc comments
- T-08-03: Full-consistency test (`test_compute_lambda_gamma_full_consistency`) verifies all j*n+i index entries; probe test (`test_compute_lambda_gamma_uses_column_major`) verifies at non-trivial (i=2, j=1)

## Next Phase Readiness

- Plan 02 (check_zero in src/online.rs) can proceed — `gamma_auth_bit_shares` is now available on both protocol structs, providing the D_ev-authenticated l_gamma shares that the c_gamma combiner in check_zero needs
- Plan 03 (end-to-end Protocol 1 test) can proceed — both `compute_lambda_gamma` methods are available and tested; the round-trip test can combine garbler and evaluator outputs directly
- No blockers

---
*Phase: 08-open-protocol-1-garble-eval-check*
*Completed: 2026-04-23*
