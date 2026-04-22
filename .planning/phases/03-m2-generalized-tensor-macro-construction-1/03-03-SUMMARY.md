---
phase: 03-m2-generalized-tensor-macro-construction-1
plan: "03"
subsystem: testing
tags: [tdd, paper-invariant, tensor-macro, ggm-tree, bcot, correctness]

# Dependency graph
requires:
  - phase: 03-02
    provides: [tensor_garbler, tensor_evaluator, TensorMacroCiphertexts]
  - phase: 03-01
    provides: [elements_slice-accessor, generalized-gen-kernel, tensor_macro-skeleton]

provides:
  - paper-invariant-test-battery: 10 #[test] functions in tensor_macro::tests verifying Z_gen XOR Z_eval == a ⊗ T
  - run_one_case helper: IdealBCot oracle + ChaCha12Rng seeded setup for reproducible (n,m,seed) tuples
  - deterministic-regression: test_deterministic_seed_42 at (n=4,m=4,seed=42) as regression gate

affects: [phase-04, phase-05, phase-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "IdealBCot::new(seed, seed ^ 0xDEAD_BEEF) — consistent bCOT oracle seeding for test isolation"
    - "run_one_case(n, m, seed) helper pattern — parameterized test oracle; #[test] fns are thin wrappers"
    - "ChaCha12Rng::seed_from_u64(seed) + rng.random_bool(0.5) — fully deterministic random choice bits"
    - "BlockMatrix::new(m, 1) + matrix[k] linear index for column vector T shares"

key-files:
  created: []
  modified:
    - src/tensor_macro.rs

key-decisions:
  - "Followed plan exactly: test code uses a_macs[i].as_block().lsb() for bit extraction (IT-MAC invariant: mac.lsb() == choice)"
  - "t_gen/t_eval are BlockMatrix::new(m,1) column vectors; t_gen[k] linear indexing matches kernel convention"
  - "g.level_cts.len() checked with n.saturating_sub(1) to handle n=1 edge case without underflow"

patterns-established:
  - "Paper-invariant test: oracle builds (a_keys, a_macs) from IdealBCot; checks Z_gen XOR Z_eval == a ⊗ T entry-wise"
  - "10 test functions (9 parameterized + 1 deterministic regression) is the standard battery size for (n,m) coverage"

requirements-completed: [PROTO-03, TEST-01]

# Metrics
duration: 2min
completed: "2026-04-22"
---

# Phase 3 Plan 03: Paper-Invariant Test Battery Summary

**Inline #[cfg(test)] mod tests with 10 reproducible tests verifying Z_garbler XOR Z_evaluator == a ⊗ T across (n,m) ∈ {(1,1),(1,4),(2,1),(2,3),(4,4),(4,8),(4,64),(8,1),(8,16)} plus a fixed-seed regression at (4,4,42)**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-22T04:25:10Z
- **Completed:** 2026-04-22T04:26:50Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Replaced empty `#[cfg(test)] mod tests { }` placeholder with a full 10-test paper-invariant battery
- `run_one_case(n, m, seed)` helper: uses `IdealBCot::transfer_a_to_b` to produce matched `(a_keys, a_macs)`, samples random T shares via `ChaCha12Rng`, runs both macro sides, asserts `Z_gen XOR Z_eval == a ⊗ T` entry-wise
- All 10 tests pass; baseline 4 failures unchanged: total `58 passed; 4 failed`
- PROTO-03 (correctness invariant) and TEST-01 (battery with edge cases) requirements satisfied

## Task Commits

1. **Task 1: Paper-invariant test battery (Z_gen XOR Z_eval == a ⊗ T)** - `c62d04f` (test)

## Files Created/Modified
- `src/tensor_macro.rs` - Replaced 2-line empty mod tests with 113-line test module containing `run_one_case` helper and 10 `#[test]` entry points

## Decisions Made
- Used `n.saturating_sub(1)` in the `g.level_cts.len()` assertion to avoid usize underflow when n=1
- `t_gen[k]` / `t_eval[k]` linear indexing on `BlockMatrix::new(m, 1)` — matches the existing column-vector access pattern established in Plan 01

## Deviations from Plan

None - plan executed exactly as written. The test code matches the plan's `<action>` block verbatim with one minor addition: `n.saturating_sub(1)` instead of `n - 1` in the sanity assertion (avoids usize underflow for the n=1 case; behavior is equivalent for n >= 1).

## Issues Encountered
None. The Plan 02 implementation was correct — all 10 tests passed on first run without any diagnosis or fixes required.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 3 (M2 Generalized Tensor Macro Construction 1) is complete: Plans 01, 02, and 03 all done
- `tensor_garbler` and `tensor_evaluator` are proven correct by the paper-invariant battery
- Phase 4 can proceed to implement Pi_LeakyTensor (Construction 2) using these primitives
- No blockers

## Known Stubs

None — the test module is fully implemented with real assertions.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes. All code is `#[cfg(test)]` only. T-03-10 (test oracle endianness), T-03-11 (reproducibility via seeded RNG), and T-03-12 (debug output of Blocks) from the plan's threat model are fully mitigated.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/tensor_macro.rs contains mod tests | FOUND |
| run_one_case function | FOUND (1 occurrence) |
| IdealBCot::new(seed, seed ^ 0xDEAD_BEEF) | FOUND (1 occurrence) |
| 9 test_n*_m*() functions | FOUND (9 occurrences) |
| test_deterministic_seed_42 | FOUND (1 occurrence) |
| commit c62d04f | FOUND |
| cargo test tensor_macro::tests: 10 passed; 0 failed | VERIFIED |
| cargo test --lib: 58 passed; 4 failed | VERIFIED |
| 4 baseline failures unchanged | VERIFIED |

---
*Phase: 03-m2-generalized-tensor-macro-construction-1*
*Completed: 2026-04-22*
