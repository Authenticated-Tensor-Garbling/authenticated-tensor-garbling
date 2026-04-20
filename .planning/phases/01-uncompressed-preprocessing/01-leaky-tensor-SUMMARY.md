---
phase: 01
plan: leaky-tensor
subsystem: preprocessing
tags: [leaky-triple, bcot, bucketing, authenticated-bits, rust]
dependency_graph:
  requires: [01-PLAN-cot]
  provides: [leaky_tensor_pre, auth_tensor_pre]
  affects: [01-PLAN-fpre-replace]
tech_stack:
  added: []
  patterns:
    - "Two-COT cross-party BDOZ layout: transfer_a_to_b gives eval_share.key, transfer_b_to_a gives gen_share.key"
    - "Column-major indexing for n*m correlated bits: index = j*n+i"
    - "XOR-combination bucketing: B AuthBitShares with shared delta XOR under Add impl"
key_files:
  created:
    - src/leaky_tensor_pre.rs
    - src/auth_tensor_pre.rs
  modified:
    - src/lib.rs
decisions:
  - "LeakyTensorPre borrows &mut IdealBCot (does not own it) to guarantee all leaky triples share the same delta_a/delta_b — required for XOR-combination MAC invariant in Pi_aTensor"
  - "gen is a reserved keyword in Rust 2024 edition — renamed verify_cross_party parameter from gen to gen_share"
  - "corr_bits computed with nested loop instead of flat_map closure to avoid Rust borrow checker issues with Vec<bool> captures"
  - "Delta and AuthBitShare imports placed in #[cfg(test)] mod tests only (not in outer module) to eliminate unused import warnings"
metrics:
  duration: "570s (9m 30s)"
  completed: "2026-04-20"
  tasks: 3
  files: 3
---

# Phase 01 Plan leaky-tensor: Leaky Tensor Preprocessing Summary

JWT-style one-liner: COT-based Pi_LeakyTensor producing cross-party BDOZ authenticated bit shares with Pi_aTensor XOR-bucketing combiner feeding directly into AuthTensorGen/AuthTensorEval.

## What Was Built

### src/leaky_tensor_pre.rs

Implements `LeakyTensorPre` (Construction 2 — Pi_LeakyTensor) and `LeakyTriple`.

`LeakyTensorPre::generate(x_clear, y_clear)` produces:
- **Alpha/beta auth shares** (n + m bits): two COT calls each — `transfer_a_to_b` gives eval_share.key (A's key, LSB=0); `transfer_b_to_a` gives gen_share.key (B's key, LSB=0).
- **Alpha/beta labels**: standard GC label sharing — gen_label = random Block with LSB=0; eval_label = gen_label XOR masked_bit*delta_a.
- **Correlated auth shares** (n*m bits, column-major j*n+i): corr_bits[j*n+i] = alpha_bits[i] AND beta_bits[j]; same two-COT pattern.
- **Gamma auth shares** (n*m bits): uniform random bits, same two-COT pattern.

**Key layout invariant** (matches `gen_auth_bit` canonical layout from auth_tensor_fpre.rs):
```
gen_share.key  = cot_b_to_a.sender_keys[i]        (A holds B's key, LSB=0)
gen_share.mac  = cot_a_to_b.receiver_macs[i]       (A's MAC under delta_b)
eval_share.key = cot_a_to_b.sender_keys[i]         (B holds A's key, LSB=0)
eval_share.mac = cot_b_to_a.receiver_macs[i]       (B's MAC under delta_a)
```

**Cross-party verify pattern**: Direct `gen_share.verify(&delta_b)` panics because gen_share.key is B's key but gen_share.mac was produced under A's key. Tests use `verify_cross_party` helper:
```rust
AuthBitShare { key: eval_share.key, mac: gen_share.mac, value: gen_share.value }.verify(delta_b);
AuthBitShare { key: gen_share.key, mac: eval_share.mac, value: eval_share.value }.verify(delta_a);
```

### src/auth_tensor_pre.rs

Implements `bucket_size_for` and `combine_leaky_triples` (Construction 3 — Pi_aTensor).

`bucket_size_for(n, m)`: `B = floor(40 / floor(log2(n*m))) + 1`
- bucket_size_for(4, 4) = 11, bucket_size_for(16, 16) = 6, bucket_size_for(128, 128) = 3

`combine_leaky_triples`: XOR-combines B leaky triples into a `(TensorFpreGen, TensorFpreEval)` pair:
- Keeps first triple's alpha/beta shares and labels unchanged.
- Accumulates correlated and gamma shares via `AuthBitShare::Add` (XOR under the hood).
- W-04 assertion: all input triples must share the same delta_a and delta_b (guaranteed by shared IdealBCot reference from run_preprocessing).

### src/lib.rs

Added `pub mod leaky_tensor_pre` and `pub mod auth_tensor_pre` (bcot was already registered in Wave 1).

## Test Results

```
leaky_tensor_pre::tests::test_alpha_beta_dimensions       ok
leaky_tensor_pre::tests::test_alpha_beta_mac_invariants   ok
leaky_tensor_pre::tests::test_alpha_label_sharing         ok
leaky_tensor_pre::tests::test_key_lsb_zero                ok
leaky_tensor_pre::tests::test_correlated_bit_correctness  ok
leaky_tensor_pre::tests::test_correlated_mac_invariants   ok
leaky_tensor_pre::tests::test_generate_dimensions_full    ok
leaky_tensor_pre::tests::test_large_n_m                   ok

auth_tensor_pre::tests::test_bucket_size_formula          ok
auth_tensor_pre::tests::test_combine_dimensions           ok
auth_tensor_pre::tests::test_combine_mac_invariants       ok
auth_tensor_pre::tests::test_full_pipeline_no_panic       ok
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Closure capture error in corr_bits flat_map**
- **Found during:** Task 1b compilation
- **Issue:** `flat_map(|j| (0..self.n).map(move |i| alpha_bits[i] && beta_bits[j]))` failed — inner `move` closure consumed `beta_bits` on first iteration of outer closure (which is FnMut, so must not consume captured variables).
- **Fix:** Replaced with a simple nested `for j / for i` loop pushing to a pre-allocated Vec.
- **Files modified:** src/leaky_tensor_pre.rs
- **Commit:** da7f8c4

**2. [Rule 1 - Bug] `gen` is a reserved keyword in Rust 2024 edition**
- **Found during:** Task 1a compilation
- **Issue:** Parameter named `gen: &AuthBitShare` in `verify_cross_party` caused parse error `expected expression, found reserved keyword 'gen'`.
- **Fix:** Renamed parameter to `gen_share` throughout `verify_cross_party`.
- **Files modified:** src/leaky_tensor_pre.rs
- **Commit:** da7f8c4

**3. [Rule 2 - Import cleanup] Moved Delta/AuthBitShare imports to test module only**
- **Found during:** Task 2 compilation
- **Issue:** `delta::Delta` and `sharing::AuthBitShare` in outer module imports triggered unused import warnings (they're only used in #[cfg(test)] code).
- **Fix:** Removed from outer `use` block; added `use crate::delta::Delta` and `use crate::sharing::AuthBitShare` inside `mod tests`.
- **Files modified:** src/auth_tensor_pre.rs
- **Commit:** 42b90fd

## Known Stubs

None — all fields are fully wired with real COT-derived values.

## Threat Flags

None — no new network endpoints, auth paths, or trust boundary crossings. All operations are in-process ideal functionality.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| src/leaky_tensor_pre.rs | FOUND |
| src/auth_tensor_pre.rs | FOUND |
| commit da7f8c4 (Tasks 1a+1b) | FOUND |
| commit 42b90fd (Task 2) | FOUND |
