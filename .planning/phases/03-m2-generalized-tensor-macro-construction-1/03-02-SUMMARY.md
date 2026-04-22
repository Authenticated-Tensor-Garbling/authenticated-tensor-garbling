---
phase: 03-m2-generalized-tensor-macro-construction-1
plan: "02"
subsystem: tensor-macro-implementation
tags: [ggm-tree, tensor-macro, garbler, evaluator, construction-1, proto-01, proto-02]

# Dependency graph
requires:
  - phase: 03-01
    provides: [generalized-gen-kernel, hoisted-eval-kernels, tensor_macro-skeleton]

provides:
  - tensor_garbler: full GGM-tree garbler body composing gen_populate_seeds_mem_optimized + gen_unary_outer_product
  - tensor_evaluator: full GGM-tree evaluator body composing eval_populate_seeds_mem_optimized + eval_unary_outer_product
  - precondition assertions on both functions (5 total)

affects: [leaky_tensor_pre, phase-04, phase-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "FIXED_KEY_AES singleton reuse — no per-call FixedKeyAes::new"
    - "Key::as_blocks / Mac::as_blocks zero-cost slice conversion for block-level kernel dispatch"
    - "g.level_cts.clone() — kernel takes Vec by value per existing signature"
    - "_recovered_missing_cts discard pattern — eval_unary_outer_product return value not needed at this layer"

key-files:
  created: []
  modified:
    - src/tensor_macro.rs

key-decisions:
  - "Implemented both tensor_garbler and tensor_evaluator in a single file edit — both modify the same file and the import block is shared; committed as one logical unit"
  - "Used g.level_cts.clone() to satisfy eval_populate_seeds_mem_optimized's by-value Vec parameter — matches existing call pattern in tensor_eval.rs"
  - "Discarded eval_unary_outer_product return value with _recovered_missing_cts — Plan 03 tests will validate the Z output, not the recovered ciphertexts"

patterns-established:
  - "tensor macro functions use FIXED_KEY_AES (static singleton) — consistent with TensorProductGen pattern"
  - "Preconditions via assert_eq! with exact string messages — matches plan-specified messages for test-verifiable panic behavior"

requirements-completed: [PROTO-01, PROTO-02]

# Metrics
duration: 2min
completed: "2026-04-22"
---

# Phase 3 Plan 02: Implement tensor_garbler and tensor_evaluator Bodies

**GGM-tree tensor macro implemented: tensor_garbler builds level+leaf ciphertexts via gen_populate_seeds_mem_optimized + gen_unary_outer_product; tensor_evaluator reconstructs subtree via eval_populate_seeds_mem_optimized + eval_unary_outer_product, returning Z shares that XOR to a ⊗ T.**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-22T04:20:45Z
- **Completed:** 2026-04-22T04:22:51Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- `tensor_garbler` fully implemented: asserts 3 preconditions, calls gen GGM kernel + outer product kernel, returns (Z_gen, TensorMacroCiphertexts)
- `tensor_evaluator` fully implemented: asserts 5 preconditions, calls eval GGM kernel + outer product kernel, returns Z_eval
- Both functions use FIXED_KEY_AES singleton and Key::as_blocks / Mac::as_blocks for zero-cost block slice conversion
- No `unimplemented!()` remains in tensor_macro.rs
- Baseline test counts preserved: 48 passed, 4 failed

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement tensor_garbler body** - `e0ec6d3` (feat) — also includes Task 2 evaluator since both share the same file and import block
2. **Task 2: Implement tensor_evaluator body** - included in `e0ec6d3` (same file edit)

**Plan metadata:** committed with SUMMARY below

_Note: Tasks 1 and 2 both modify src/tensor_macro.rs with a shared use block — a single atomic commit covers both implementations._

## Files Created/Modified
- `src/tensor_macro.rs` - Replaced both `unimplemented!()` stubs with full garbler and evaluator bodies; updated use block to import FIXED_KEY_AES, Key, Mac, and all four tensor_ops kernels

## Decisions Made
- Implemented both tasks in one file write since the use block is shared and the implementations are interdependent at the import level — committed as a single logical unit
- Used `g.level_cts.clone()` to satisfy `eval_populate_seeds_mem_optimized`'s by-value `Vec<(Block,Block)>` parameter, matching the existing pattern in `tensor_eval.rs::eval_chunked_half_outer_product`
- Discarded `eval_unary_outer_product` return value via `_recovered_missing_cts` prefix — the recovered ciphertexts are not needed by callers at this layer; Plan 03 tests validate Z correctness

## Deviations from Plan

None - plan executed exactly as written. Both function bodies match the exact code from the plan's `<action>` sections verbatim.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `tensor_garbler` and `tensor_evaluator` are complete pub(crate) free functions ready for Plan 03 paper-invariant test battery (TEST-01, TEST-04)
- Plan 03 tests will verify the correctness invariant: `Z_gen XOR Z_eval == a ⊗ T` using IdealBCot as the test oracle
- No blockers

## Known Stubs

None — both function bodies are fully implemented.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes introduced. T-03-05 (tensor_garbler input validation) and T-03-06 (tensor_evaluator input validation) mitigations are fully implemented via the five `assert_eq!` precondition checks. No new trust boundary surfaces beyond what the plan's threat model documented.

---
*Phase: 03-m2-generalized-tensor-macro-construction-1*
*Completed: 2026-04-22*
