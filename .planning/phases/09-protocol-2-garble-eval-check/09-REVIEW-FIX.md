---
phase: 09-protocol-2-garble-eval-check
fixed_at: 2026-04-24T00:00:00Z
review_path: .planning/phases/09-protocol-2-garble-eval-check/09-REVIEW.md
iteration: 1
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 9: Code Review Fix Report

**Fixed at:** 2026-04-24T00:00:00Z
**Source review:** .planning/phases/09-protocol-2-garble-eval-check/09-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 2
- Fixed: 2
- Skipped: 0

## Fixed Issues

### WR-01: No guard against calling both `evaluate_final` and `evaluate_final_p2` on the same instance

**Files modified:** `src/auth_tensor_eval.rs`, `src/auth_tensor_gen.rs`
**Commit:** c5f639e
**Applied fix:** Added `assert!(!self.final_computed, …)` at the top of all four
finalisation methods: `evaluate_final`, `evaluate_final_p2` (in
`auth_tensor_eval.rs`) and `garble_final`, `garble_final_p2` (in
`auth_tensor_gen.rs`). Without the guard a second call silently XOR-cancels
the D_gb accumulation (XOR self-inverse) and sets `final_computed = true`,
causing `compute_lambda_gamma` to return incorrect results without panicking.
The assert converts this silent data corruption into an immediate explicit panic.

---

### WR-02: Narrow and wide leaf-expansion tweaks alias in the same u128 codomain

**Files modified:** `src/tensor_ops.rs`
**Commit:** 518914a
**Applied fix:** Introduced `const WIDE_DOMAIN: u128 = 1u128 << 64` and OR'd it
into every TCCR call inside `gen_unary_outer_product_wide` and
`eval_unary_outer_product_wide` (four call sites total). The narrow
`gen_unary_outer_product` uses tweaks 0, 1, 2, … that fit entirely in the low
64 bits; setting bit 64 in all wide tweaks makes the two codomain ranges
disjoint, ensuring PRF output independence between the two function families
even if the same leaf seeds were ever supplied to both. Updated the two inline
test assertions in `tensor_ops::tests` that hard-coded the old tweak formula.
All 105 tests pass.

---

_Fixed: 2026-04-24T00:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
