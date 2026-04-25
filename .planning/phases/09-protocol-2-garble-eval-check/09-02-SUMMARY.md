---
phase: 09-protocol-2-garble-eval-check
plan: 02
subsystem: tensor-ops
tags: [rust, tensor-ops, ggm-tree, wide-leaf-expansion, kappa-rho, p2-01]
requirements: [P2-01]
dependencies:
  requires:
    - 09-01 (D_ev field plumbing in TensorFpreGen/TensorFpreEval)
  provides:
    - gen_unary_outer_product_wide (Vec<Block> -> Vec<(Block, Block)> ciphertexts)
    - eval_unary_outer_product_wide (Vec<Block> -> Vec<(Block, Block)> ciphertexts)
  affects:
    - src/tensor_ops.rs (additions only — no existing function changed)
tech-stack:
  added: []
  patterns:
    - Wide-leaf GGM expansion via even/odd tweak split (base<<1 / base<<1|1)
    - Wide ciphertext as (Block, Block) tuple — kappa-half then rho-half
key-files:
  created: []
  modified:
    - src/tensor_ops.rs
decisions:
  - "Wide ciphertexts use Vec<(Block, Block)> per CONTEXT.md D-01 — no new types"
  - "Same TCCR convention as gen_populate_seeds_mem_optimized: base<<1 / base<<1|1"
  - "New tests live in a new #[cfg(test)] mod tests block at the end of tensor_ops.rs"
metrics:
  duration_sec: 130
  duration_min: ~2
  completed_date: 2026-04-25
  tasks_completed: 1
  commits: 2
  tests_added: 4
  tests_passing: 101
---

# Phase 9 Plan 2: Wide-Leaf gen/eval (P2-01) Summary

**One-liner:** Added `gen_unary_outer_product_wide` and `eval_unary_outer_product_wide` to `src/tensor_ops.rs` — the (κ+ρ)-bit wide-leaf-expansion GGM tree variants that drive Protocol 2 by accumulating a κ-half (D_gb) and a ρ-half (D_ev) in a single tree pass, producing `Vec<(Block, Block)>` wide ciphertexts.

## Scope

P2-01 only: cryptographic primitives in `src/tensor_ops.rs`. No call-site integration in `auth_tensor_gen.rs` / `auth_tensor_eval.rs` (deferred to Plan 03).

## Implementation

### Functions added

```rust
pub(crate) fn gen_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<(Block, Block)>;

pub(crate) fn eval_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[(Block, Block)],
) -> Vec<(Block, Block)>;
```

Both signatures match exactly the `<interfaces>` block in 09-02-PLAN.md.

### Tweak convention (D-03)

For each (column j, leaf i) pair:

```rust
let base = (seeds.len() * j + i) as u128;
let s_gb = cipher.tccr(Block::from(base << 1),     seeds[i]); // kappa-half
let s_ev = cipher.tccr(Block::from(base << 1 | 1), seeds[i]); // rho-half
```

Confirmed by `grep -n "base << 1 | 1" src/tensor_ops.rs` returning 4 occurrences (2 in implementations, 2 in tests for kappa/rho row-equation derivation).

The convention matches `gen_populate_seeds_mem_optimized` lines 55–56 (the existing GGM-tree level tweak scheme) so domain separation between κ-half and ρ-half is identical to what the rest of the codebase relies on.

### Eval reconstruction (T-9-06 mitigation)

Both halves are recovered independently:

```rust
eval_ct_gb ^= gen_cts[j].0 ^ y_d_gb[j];
eval_ct_ev ^= gen_cts[j].1 ^ y_d_ev[j];
```

then both halves are XOR-distributed into `out_gb`/`out_ev` for every row `k` where `(missing >> k) & 1 == 1`.

## Tests Added

All four added to a new `#[cfg(test)] mod tests { use super::*; ... }` block at the end of `src/tensor_ops.rs`.

| # | Name | Asserts |
|---|------|---------|
| 1 | `test_gen_unary_outer_product_wide_tweak_independence` | For deterministic seeds (n=2, m=2): every `gen_cts[j].0 != gen_cts[j].1`, and `out_gb` differs from `out_ev` at ≥1 entry. Mitigates T-9-02 (TCCR tweak reuse). |
| 2 | `test_eval_unary_outer_product_wide_round_trip_kappa` | `gen_cts[0].0 == XOR_i tccr(Block::from(base<<1), seeds[i]) ^ y_gb[0]` for n=2, m=1. Verifies the kappa-half row equation. |
| 3 | `test_eval_unary_outer_product_wide_round_trip_rho` | `gen_cts[0].1 == XOR_i tccr(Block::from(base<<1|1), seeds[i]) ^ y_ev[0]` for n=2, m=1. Verifies the rho-half row equation under the odd tweak. |
| 4 | `test_wide_signature_shapes` | For seeds.len()=4, m=3: `gen_cts.len() == 3`, both `out_gb` and `out_ev` have ≥1 non-default entry. |

## Verification

| Acceptance criterion | Result |
|----------------------|--------|
| `grep -n "pub(crate) fn gen_unary_outer_product_wide" src/tensor_ops.rs \| wc -l` | 1 |
| `grep -n "pub(crate) fn eval_unary_outer_product_wide" src/tensor_ops.rs \| wc -l` | 1 |
| `grep -n "base << 1 \| 1" src/tensor_ops.rs \| wc -l` | 4 (>= 2) |
| `grep -n "test_gen_unary_outer_product_wide_tweak_independence" src/tensor_ops.rs \| wc -l` | 1 |
| `grep -n "test_eval_unary_outer_product_wide_round_trip_kappa" src/tensor_ops.rs \| wc -l` | 1 |
| `grep -n "test_eval_unary_outer_product_wide_round_trip_rho" src/tensor_ops.rs \| wc -l` | 1 |
| `grep -n "test_wide_signature_shapes" src/tensor_ops.rs \| wc -l` | 1 |
| `cargo test tensor_ops::tests::*` | 4 passed, 0 failed |
| `cargo test` (full suite) | 101 passed, 0 failed |

## TDD Gate Compliance

| Gate | Commit | Status |
|------|--------|--------|
| RED | `79266d2` `test(09-02): add failing tests for wide-leaf gen/eval (P2-01)` | Tests compiled → unresolved-name errors against the missing wide functions (E0425). Confirmed via `cargo test --no-run`. |
| GREEN | `44e0565` `feat(09-02): add gen/eval_unary_outer_product_wide for P2-01` | All 4 wide tests pass; full suite 101/101. |
| REFACTOR | (none) | Implementation directly mirrors the narrow gen/eval; no clean-up commit needed. |

## Commits

| Hash | Type | Message |
|------|------|---------|
| `79266d2` | test | `test(09-02): add failing tests for wide-leaf gen/eval (P2-01)` |
| `44e0565` | feat | `feat(09-02): add gen/eval_unary_outer_product_wide for P2-01` |

## Deviations from Plan

None — plan executed exactly as written. Test file structure (no pre-existing `mod tests` in `tensor_ops.rs`) was anticipated by the plan's NOTE; new module was created as instructed.

## Threat Model Compliance

| Threat ID | Disposition | Status |
|-----------|-------------|--------|
| T-9-02 (Tampering — TCCR tweak reuse across κ/ρ) | mitigate | Even/odd tweak split applied (`base << 1` vs `base << 1 \| 1`); `test_gen_unary_outer_product_wide_tweak_independence` asserts κ-half ≠ ρ-half outputs. |
| T-9-06 (Tampering — eval reconstructs only one half) | mitigate | Both `eval_ct_gb` and `eval_ct_ev` are computed and distributed; round-trip tests verify both halves' row equations. |
| T-9-07 (Information Disclosure — wide ciphertext type) | accept | `Vec<(Block, Block)>` carries no secret values — public garbled-circuit data. |

No new threat surface introduced beyond what the plan registered.

## Self-Check: PASSED

- File `src/tensor_ops.rs`: FOUND
- Plan path `.planning/phases/09-protocol-2-garble-eval-check/09-02-PLAN.md`: FOUND
- Commit `79266d2` (RED): FOUND in `git log --oneline`
- Commit `44e0565` (GREEN): FOUND in `git log --oneline`
- Function `gen_unary_outer_product_wide`: defined `pub(crate) fn gen_unary_outer_product_wide`
- Function `eval_unary_outer_product_wide`: defined `pub(crate) fn eval_unary_outer_product_wide`
- All four named tests: present and passing
- Full test suite: 101 passed, 0 failed
