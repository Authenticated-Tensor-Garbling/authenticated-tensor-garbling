---
phase: 04-m2-pi-leakytensor-f-eq-construction-2
plan: "02"
subsystem: preprocessing
tags:
  - rust
  - crypto
  - leaky-tensor
  - tensor-macro
  - feq
  - bcot
dependency_graph:
  requires:
    - "04-01"
  provides:
    - "LeakyTensorPre::generate() full Construction 2 body"
    - "Working run_preprocessing() pipeline"
    - "Same-delta BCot convention"
    - "Explicit a_bits parameter in eval_populate_seeds_mem_optimized"
  affects:
    - "04-03 (Plan 3 paper-invariant tests)"
    - "tensor_eval.rs / auth_tensor_eval.rs (a_bits propagation)"
tech_stack:
  added: []
  patterns:
    - "Same-delta BCot: sender uses own delta (transfer_a_to_b uses delta_a, transfer_b_to_a uses delta_b)"
    - "Explicit choice bits in GGM tree navigation decoupled from MAC LSB"
    - "Cross-party AuthBitShare assembly with matching COT batch per macro call"
key_files:
  created: []
  modified:
    - src/leaky_tensor_pre.rs
    - src/tensor_ops.rs
    - src/tensor_macro.rs
    - src/bcot.rs
    - src/tensor_eval.rs
    - src/auth_tensor_eval.rs
    - src/preprocessing.rs
    - src/auth_tensor_pre.rs
decisions:
  - "Same-delta BCot convention (transfer_a_to_b uses delta_a, transfer_b_to_a uses delta_b) — required for C_A/C_B cancellation property: C_A XOR C_B = y*(delta_a XOR delta_b)"
  - "Explicit a_bits parameter in eval_populate_seeds_mem_optimized — decouples tree navigation from mac.lsb(), required when Δ_B.lsb()=0"
  - "Macro 1: A garbles under Δ_A using cot_x_a_to_b (keys + receiver_macs + x_b_bits explicit); Macro 2: B garbles under Δ_B using cot_x_b_to_a (keys + receiver_macs + x_a_bits explicit)"
  - "A1 convention for D share: gen_d has value=d_bit with zero key/mac; eval_d has mac=d_bit*delta_b with value=false (Plan 3 TEST-02 will validate)"
metrics:
  duration: "~3 hours (including context restoration from summary)"
  completed: "2026-04-22T07:19:37Z"
  tasks_completed: 2
  files_modified: 8
---

# Phase 04 Plan 02: Full Pi_LeakyTensor Construction 2 Implementation Summary

One-liner: Full 5-step Pi_LeakyTensor Construction 2 `generate()` body with same-delta BCot convention fix and explicit GGM tree navigation bits.

## What Was Built

`LeakyTensorPre::generate()` now executes the complete paper Construction 2 protocol:

1. **Step 1** — 6 bCOT batch calls (x, y, R × {a_to_b, b_to_a}); cross-party AuthBitShare assembly into 6 Vec<AuthBitShare>
2. **Step 2** — C_A, C_B, C_A^(R), C_B^(R) inline computation using correct field selections from cross-party shares
3. **Step 3** — Two tensor_macro calls: Macro 1 (A garbles under Δ_A, B evaluates with x_b_bits explicit); Macro 2 (B garbles under Δ_B, A evaluates with x_a_bits explicit)
4. **Step 4** — S_1/S_2 masked reveal; D = lsb(S_1) XOR lsb(S_2) extraction (column-major)
5. **Step 5** — L_1/L_2 construction; `feq::check(&l_1, &l_2)`; final gen_z_shares/eval_z_shares via R XOR D trivial shares

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] BCot cross-delta convention broken with delta_b.lsb()=0**
- **Found during:** Task 2.2 — all tensor_macro tests failing after initial implementation
- **Issue:** `transfer_a_to_b` was using `delta_b` as correlation key (cross-delta convention). With `delta_b.lsb()=0`, C_A/C_B produced `K_A[0] XOR K_B[0]` for both (both identical, T=0), but more critically Macro 2 evaluator's MACs had `lsb=0` always, making GGM tree navigation always take the `missing=0` branch regardless of choice bits
- **Fix:** Changed to same-delta convention: `transfer_a_to_b` uses `delta_a`, `transfer_b_to_a` uses `delta_b`. This gives `C_A XOR C_B = y*(delta_a XOR delta_b)` as required by paper. Updated all 6 BCot test assertions to match new convention.
- **Files modified:** `src/bcot.rs`
- **Commit:** 92c9d6d

**2. [Rule 1 - Bug] eval_populate_seeds_mem_optimized deduced choice bits from mac.lsb()**
- **Found during:** Task 2.2 — architectural issue blocking Macro 2 correctness
- **Issue:** The function used `x[n-1].lsb()` and `x[n-i-1].lsb()` to determine the missing path in the GGM tree. When the garbler's delta has `lsb=0` (Macro 2 under Δ_B), all MACs under Δ_B have `lsb=0`, so the missing path was always 0 regardless of actual choice bits.
- **Fix:** Added `a_bits: &[bool]` explicit parameter to `eval_populate_seeds_mem_optimized`. Updated `tensor_evaluator` to also accept and forward `a_bits`. Updated all callers: `tensor_macro.rs` (passes explicit choices), `tensor_eval.rs` (derives from slice LSBs), `auth_tensor_eval.rs` (derives from slice LSBs).
- **Files modified:** `src/tensor_ops.rs`, `src/tensor_macro.rs`, `src/tensor_eval.rs`, `src/auth_tensor_eval.rs`
- **Commit:** 92c9d6d

**3. [Rule 1 - Bug] Macro calls in generate() used inconsistent COT batches**
- **Found during:** Task 2.2 — existing stub had Macro 2 garbling with cot_x_a_to_b.sender_keys but evaluating with cot_x_b_to_a.receiver_macs (different batches)
- **Fix:** Corrected to: Macro 1 uses `cot_x_a_to_b.sender_keys` + `cot_x_a_to_b.receiver_macs` + `x_b_bits`; Macro 2 uses `cot_x_b_to_a.sender_keys` + `cot_x_b_to_a.receiver_macs` + `x_a_bits`
- **Files modified:** `src/leaky_tensor_pre.rs`
- **Commit:** 92c9d6d

**4. [Rule 1 - Bug] tensor_macro test used transfer_b_to_a but expected mac.lsb()=choice**
- **Found during:** Fixing tensor_macro tests post-BCot convention change
- **Issue:** `run_one_case` called `transfer_b_to_a` (now uses delta_b with lsb=0) and then derived `a_bits` from `mac.lsb()`, which always gives false for choice=true entries
- **Fix:** Changed test to use `transfer_a_to_b` (delta_a, lsb=1) and pass explicit `choices` as `a_bits` to `tensor_evaluator`
- **Files modified:** `src/tensor_macro.rs`
- **Commit:** 92c9d6d

## Key Correctness Observations

**C_A/C_B cancellation (with same-delta convention):**
- `gen_y_shares[j].mac = K_B^y[0] XOR y_A * delta_b` (transfer_b_to_a now uses delta_b)
- `C_A = y_A*delta_a XOR K_A^y[0] XOR K_B^y[0] XOR y_A*delta_b = K_A^y[0] XOR K_B^y[0] XOR y_A*(delta_a XOR delta_b)` ✓
- `C_B = y_B*delta_b XOR K_A^y[0] XOR y_B*delta_a XOR K_B^y[0] = K_A^y[0] XOR K_B^y[0] XOR y_B*(delta_a XOR delta_b)` ✓
- `C_A XOR C_B = y*(delta_a XOR delta_b)` ✓ (where y = y_A XOR y_B is the combined bit)

**Macro 1 alignment (A garbles under Δ_A):**
- Garbler keys: `cot_x_a_to_b.sender_keys` (A's keys, lsb=0) ✓
- Evaluator MACs: `cot_x_a_to_b.receiver_macs = K[0] XOR x_B * delta_a` (lsb=x_B since delta_a.lsb()=1) ✓
- Explicit bits: `x_b_bits` — consistent with MAC LSB in this case ✓

**Macro 2 alignment (B garbles under Δ_B):**
- Garbler keys: `cot_x_b_to_a.sender_keys` (B's keys, lsb=0) ✓
- Evaluator MACs: `cot_x_b_to_a.receiver_macs = K[0] XOR x_A * delta_b` (lsb unreliable, delta_b.lsb()=0) ✓
- Explicit bits: `x_a_bits` — mandatory since MAC LSBs don't encode the bits ✓

## Test Results

- **57 passed, 0 failed, 8 ignored** (8 remaining ignores are Plan 3 placeholders in leaky_tensor_pre.rs)
- `preprocessing::tests::test_run_preprocessing_dimensions` — PASSED (un-ignored)
- `preprocessing::tests::test_run_preprocessing_delta_lsb` — PASSED (un-ignored)
- `preprocessing::tests::test_run_preprocessing_feeds_online_phase` — PASSED (un-ignored; AuthTensorGen handles empty labels without panic)
- `auth_tensor_pre::tests::test_combine_dimensions` — PASSED (un-ignored)
- `auth_tensor_pre::tests::test_full_pipeline_no_panic` — PASSED (un-ignored; no Phase 5 re-ignore needed)
- All tensor_macro correctness tests (9 cases) — PASSED
- All BCot tests — PASSED

## A1 Convention for D Share

Plan's assumed A1 convention implemented as-is:
- `gen_d = AuthBitShare { key: Key::default(), mac: Mac::default(), value: d_bits[k] }` (gen owns bit value, zero key/mac)
- `eval_d = AuthBitShare { key: Key::default(), mac: Mac::new(d_bit ? delta_b_block : ZERO), value: false }` (eval owns mac under Δ_B)

This is UNVERIFIED by cross-party invariant tests — Plan 3's TEST-02 (`test_leaky_triple_mac_invariants`) is the acceptance gate.

## Commits

| Task | Hash | Description |
|------|------|-------------|
| 2.1 (prior session) | c6f4502 | Steps 1+2: correlated randomness + C_A/C_B assembly |
| 2.2 | 92c9d6d | Steps 3-5: tensor_macro calls, masked reveal, F_eq, Z assembly + BCot + a_bits fixes |

## Self-Check: PASSED

- src/leaky_tensor_pre.rs — FOUND
- src/tensor_ops.rs — FOUND
- src/bcot.rs — FOUND
- commit c6f4502 (Task 2.1) — FOUND
- commit 92c9d6d (Task 2.2) — FOUND
- cargo test --lib: 57 passed, 0 failed — VERIFIED
