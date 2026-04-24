---
phase: 08-open-protocol-1-garble-eval-check
fixed_at: 2026-04-24T00:00:00Z
review_path: .planning/phases/08-open-protocol-1-garble-eval-check/08-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 08: Code Review Fix Report

**Fixed at:** 2026-04-24
**Source review:** .planning/phases/08-open-protocol-1-garble-eval-check/08-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5 (WR-01, WR-02, WR-03, IN-01, IN-02 — all scope)
- Fixed: 5
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

### IN-01: Debug `println!` left in `test_auth_tensor_product`

**Files modified:** `src/lib.rs`
**Commit:** 0a181fb
**Applied fix:** Removed all five diagnostic `println!` calls from the
`test_auth_tensor_product` test (the four annotated calls reporting chunk-level
and ciphertext counts, plus the bare `println!()` newline at the end of the
inner loop). The `print!` used for the expected-value grid is not a diagnostic
and was left in place. All 95 tests continue to pass.

### IN-02: `check_zero` parameter named `delta_ev` but called with `delta_a` (garbler's delta)

**Files modified:** `src/online.rs`
**Commit:** b0fc15c
**Applied fix:** Renamed the `delta_ev` parameter to `delta_mac` throughout
`check_zero` — the function signature, all doc-comment references, and the
single internal call to `share.key.auth(share.value, delta_mac)`. Added a
clarifying paragraph to the doc comment explaining that `delta_mac` is the
global correlation key under which the `c_gamma_shares` IT-MACs are
authenticated (the verifying party's delta), and noting that `delta_a` from
`AuthTensorGen` is that delta in this codebase. Eliminates the ambiguity that
arose from `delta_ev` implying it must be the evaluator's own delta.

---

_Fixed: 2026-04-24_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
