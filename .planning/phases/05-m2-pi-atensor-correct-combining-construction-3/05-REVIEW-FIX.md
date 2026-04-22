---
phase: 05-m2-pi-atensor-correct-combining-construction-3
fixed_at: 2026-04-22T21:45:00Z
review_path: .planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-04-22T21:45:00Z
**Source review:** `.planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: `TensorFpreEval.delta_b` doc comment claims wrong LSB invariant

**Files modified:** `src/preprocessing.rs`
**Commit:** `0b3cb43`
**Applied fix:** Changed the `TensorFpreEval.delta_b` field doc comment from the incorrect
`` `as_block().lsb() == 1` invariant `` (a mirror-copy of the garbler's `delta_a` comment) to
the correct two-line comment: `` `as_block().lsb() == 0` invariant (required so that
`lsb(delta_a XOR delta_b) == 1` per Pi_LeakyTensor §F) ``. This matches the actual
behaviour of `Delta::random_b` (LSB=0) and the `test_delta_xor_lsb_is_one` regression.

---

### WR-02: `run_preprocessing` API accepts `count` but panics on any value other than 1

**Files modified:** `src/preprocessing.rs`
**Commit:** `b0ef52e`
**Applied fix:** Added a `# Panics` rustdoc section immediately above the function signature
(Option B from the review, preferred over Option A because every call site — tests and the
benchmark — already passes `count=1` hardcoded, so removing the parameter would require
touching `benches/benchmarks.rs` and three test call sites with no correctness gain). The
new section reads: *"Panics if `count != 1`. Batch output (count > 1) requires a
Vec-returning variant that is not yet implemented."* This makes the runtime restriction
visible to any user of the public API via `cargo doc`.

---

### WR-03: Dead computation in `eval_z_shares` construction

**Files modified:** `src/leaky_tensor_pre.rs`
**Commit:** `643d2b0`
**Applied fix:** Replaced the `(0..n*m).map(|k| { let eval_d = AuthBitShare { key: default,
mac: default, value: false }; eval_r_shares[k] + eval_d }).collect()` no-op map with a
direct binding `let eval_z_shares: Vec<AuthBitShare> = eval_r_shares;` and moved the
already-accurate explanation from the block comment above down to the assignment line as an
inline comment: *"eval side: evaluator holds no D contribution (D is public; eval_z =
eval_r only)."* This eliminates the n*m-element allocation and XOR pass and removes the
contradiction between the correct comment and the misleading code. `cargo check` confirmed
no compilation errors after the change.

---

_Fixed: 2026-04-22T21:45:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
