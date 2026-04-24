---
phase: 08-open-protocol-1-garble-eval-check
fixed_at: 2026-04-23T00:00:00Z
review_path: .planning/phases/08-open-protocol-1-garble-eval-check/08-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 08: Code Review Fix Report

**Fixed at:** 2026-04-23
**Source review:** .planning/phases/08-open-protocol-1-garble-eval-check/08-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 3 (WR-01, WR-02, WR-03 — Info findings excluded per fix_scope)
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: `check_zero` docstring describes the `Add`-operator combination pattern but `assemble_c_gamma_shares` does not follow it

**Files modified:** `src/online.rs`
**Commit:** f30a611
**Applied fix:** Replaced the misleading caller contract (which told callers to
pre-XOR shares via `AuthBitShare::add` so `mac = gen.mac XOR ev.mac`) with an
accurate description of the freshly-computed-MAC pattern that
`assemble_c_gamma_shares` actually uses. The updated contract specifies that
callers must accumulate `key = XOR(gen-side B-keys)` and then compute
`mac = key.auth(value, delta_ev)` directly. An explicit warning was added
explaining that the `+` operator must NOT be used to combine cross-party shares
because the two parties' MACs are authenticated under different deltas.
A pointer to `assemble_c_gamma_shares` as the reference implementation was also
added.

### WR-02: `compute_lambda_gamma` silently returns garbage when called before `garble_final` / `evaluate_final`

**Files modified:** `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`
**Commit:** 2cb3694
**Applied fix:** Added `final_computed: bool` field to both `AuthTensorGen` and
`AuthTensorEval` structs. The field is initialised to `false` in `new()` and
`new_from_fpre_gen()` / `new_from_fpre_eval()`. It is set to `true` at the end
of `garble_final()` and `evaluate_final()` respectively. Each
`compute_lambda_gamma` implementation now opens with an `assert!(self.final_computed, ...)`
that panics with a descriptive message if the method is called before the
corresponding `*_final()` step, preventing silent garbage output.

### WR-03: `assemble_c_gamma_shares` accesses both parties' private fields — not usable in a real protocol

**Files modified:** `src/lib.rs`
**Commit:** 7134d9d
**Applied fix:** Added a four-line `// SIMULATION ONLY` block comment immediately
above the `#[allow(clippy::too_many_arguments)]` attribute and `fn` signature of
`assemble_c_gamma_shares`. The comment states that the function requires both
parties' private state, is only valid inside `#[cfg(test)]`, and describes what
a real protocol implementation would do instead (each party assembles its own
half independently and then runs `check_zero` over the network).

---

_Fixed: 2026-04-23_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
