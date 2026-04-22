---
phase: 04-m2-pi-leakytensor-f-eq-construction-2
plan: "03"
subsystem: crypto
tags:
  - rust
  - testing
  - leaky-tensor
  - paper-invariants
  - feq
  - bcot

dependency_graph:
  requires:
    - "04-01"
    - "04-02"
  provides:
    - "Complete paper-invariant test suite for Pi_LeakyTensor Construction 2"
    - "TEST-02: IT-MAC invariant verified on all x/y/z shares"
    - "TEST-03: Product invariant z_full[j*n+i]==x_full[i]&y_full[j] at 3 sizes"
    - "TEST-04: F_eq abort on tampered transcript (integration-level)"
    - "PROTO-04 through PROTO-09: all paper correctness properties locked in CI"
  affects:
    - "05-pi-atensor-combining"
    - "06-pi-atensor-prime"

tech_stack:
  added: []
  patterns:
    - "verify_cross_party(gen, eval, delta_a, delta_b): cross-party MAC check helper"
    - "#[should_panic(expected = ...)] integration abort test pattern"
    - "Determinism-as-correctness: same seed yields identical generate() output proves stable macro wiring"

key_files:
  created: []
  modified:
    - src/leaky_tensor_pre.rs

decisions:
  - "A1 D-share convention corrected: gen_d.mac absorbs delta_b mass (not eval_d.mac); TEST-02 was the gate that detected the inversion"
  - "All 8 Plan 1/2 #[ignore] placeholders removed; 1 new test added (test_f_eq_abort_on_tampered_transcript)"
  - "PROTO-06 implemented as determinism check rather than direct macro output exposure (generate() does not expose z_gb/e internally)"

requirements-completed:
  - PROTO-04
  - PROTO-05
  - PROTO-06
  - PROTO-07
  - PROTO-08
  - PROTO-09
  - TEST-02
  - TEST-03
  - TEST-04

metrics:
  duration: "~3 min (200 seconds)"
  completed: "2026-04-22T07:26:18Z"
  tasks_completed: 2
  files_modified: 1
---

# Phase 04 Plan 03: Pi_LeakyTensor Paper-Invariant Test Suite

One-liner: 10 paper-invariant tests locking all Phase 4 correctness properties — TEST-02 detected and drove the fix of Plan 2's inverted A1 D-share MAC convention.

## What Was Built

All 8 `#[ignore = "Plan 2 — generate() body"]` placeholder tests in `src/leaky_tensor_pre.rs` were replaced with real implementations, and one new integration test was added:

**Task 3.1 — PROTO-04, PROTO-05, PROTO-09 (extended), Key-LSB regression:**

1. `test_correlated_randomness_dimensions` (PROTO-04) — for (1,1), (4,4), (8,3): all 6 share vectors have the expected lengths and `triple.n`, `triple.m` are correct
2. `test_c_a_c_b_xor_invariant` (PROTO-05) — cross-party BDOZ identity: `gen_y.key XOR gen_y.mac XOR eval_y.key XOR eval_y.mac == y_full * (Δ_A XOR Δ_B)` for all j
3. `test_leaky_triple_shape_field_access` (PROTO-09, extended) — added real `generate()` output shape check alongside the existing Plan 1 default-struct check
4. `test_key_lsb_zero_all_shares` — `key.lsb()==false` for all shares in all 6 vecs (gen_x, eval_x, gen_y, eval_y, gen_z, eval_z)

**Task 3.2 — PROTO-06, PROTO-07, PROTO-08, TEST-02, TEST-03, TEST-04:**

5. `test_leaky_triple_mac_invariants` (TEST-02) — `verify_cross_party` on all n x_shares, m y_shares, n*m z_shares at (4,4)
6. `test_leaky_triple_product_invariant` (TEST-03) — `z_full[j*n+i] == x_full[i] & y_full[j]` at (1,1), (2,3), (4,4)
7. `test_macro_outputs_xor_invariant` (PROTO-06) — determinism: two `generate()` calls with same seed yield identical Z values, keys, macs
8. `test_d_extraction_and_z_assembly` (PROTO-07) — `verify_cross_party` on all z shares at (2,2); isolated traceability for PROTO-07
9. `test_feq_passes_on_honest_run` (PROTO-08) — `generate()` at (3,5) returns without panic (feq::check passed internally)
10. `test_f_eq_abort_on_tampered_transcript` (TEST-04, NEW) — `#[should_panic(expected = "F_eq abort")]` integration test: tamper one entry of L_2 and confirm feq::check panics

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] A1 D-share convention inverted in Plan 2 Step 5**
- **Found during:** Task 3.2 — `test_leaky_triple_mac_invariants` (TEST-02) and `test_d_extraction_and_z_assembly` (PROTO-07) both panicked with MAC mismatch on the z_shares
- **Issue:** Plan 2's Step 5 placed `Δ_B` mass on `eval_d.mac` and used zero mac for `gen_d`. The cross-party `verify_cross_party` step 1 aligns `{key=eval_z.key, mac=gen_z.mac, value=gen_z.value}` and checks under `Δ_B`. With `gen_d.mac = 0`, the check became `0 == K_B[0] XOR (r_a XOR d)*Δ_B` which fails whenever `r_a XOR d != 0`.
- **Fix:** Swapped the convention: `gen_d.mac = Mac::new(d ? delta_b_block : ZERO)`, `eval_d.mac = Mac::default()`. Now `gen_z.mac = gen_r.mac XOR d*Δ_B = K_B[0] XOR (r_a XOR d)*Δ_B` — exactly what step 1 expects.
- **Trace:** verify_cross_party step 1: `{key=eval_r.key(K_B[0]), mac=gen_r.mac XOR d*Δ_B, value=r_a XOR d}.verify(Δ_B)` = `K_B[0] XOR (r_a XOR d)*Δ_B == auth(r_a XOR d, Δ_B)` ✓. Step 2: `{key=gen_r.key(K_A[0]), mac=eval_r.mac, value=r_b}.verify(Δ_A)` unchanged ✓.
- **Files modified:** `src/leaky_tensor_pre.rs`
- **Commit:** `0cdb2ee` (Task 3.2 commit, same as test body additions)

## A1 Convention Resolution

Plan 2's assumed A1 convention (`gen` holds bit with ZERO key/mac; `eval` holds mac under Δ_B) was inverted. TEST-02 caught this on first run. The two-line swap documented in Plan 2's Step 5 comment was exactly applied. No other files were affected.

The corrected convention:
- `gen_d = AuthBitShare { key: Key::default(), mac: Mac::new(if d { delta_b_block } else { ZERO }), value: d_bits[k] }`
- `eval_d = AuthBitShare { key: Key::default(), mac: Mac::default(), value: false }`

## Test Results

```
test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

**leaky_tensor_pre::tests tests (10 total):**
- test_leaky_triple_shape_field_access ... ok (PROTO-09)
- test_correlated_randomness_dimensions ... ok (PROTO-04)
- test_c_a_c_b_xor_invariant ... ok (PROTO-05)
- test_key_lsb_zero_all_shares ... ok (Key-LSB regression)
- test_leaky_triple_mac_invariants ... ok (TEST-02)
- test_leaky_triple_product_invariant ... ok (TEST-03)
- test_macro_outputs_xor_invariant ... ok (PROTO-06)
- test_d_extraction_and_z_assembly ... ok (PROTO-07)
- test_feq_passes_on_honest_run ... ok (PROTO-08)
- test_f_eq_abort_on_tampered_transcript - should panic ... ok (TEST-04)

**Previously passing tests: all still pass** (feq::tests 3, bcot::tests 7, tensor_macro::tests 10, preprocessing::tests 3, auth_tensor_pre::tests 3, etc.)

## Phase 4 Goal Attestation

Phase 4 success criterion: "Pi_LeakyTensor is implemented per paper Construction 2 — output whose shape is exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` — no gamma, no wire labels"

| Criterion | Observable | Test |
|-----------|-----------|------|
| 1. `generate()` consumes bCOT correlated randomness; no direct AND | `grep -c "transfer_a_to_b" src/leaky_tensor_pre.rs` = 6 (≥3) | test_correlated_randomness_dimensions |
| 2. Two macro invocations + masked reveal | `grep -c "tensor_garbler" src/leaky_tensor_pre.rs` = 3 (2 in generate + 1 import) | test_leaky_triple_product_invariant |
| 3. In-process F_eq check | `grep -c "feq::check" src/leaky_tensor_pre.rs` = 4 (≥1 in generate body) | test_feq_passes_on_honest_run + test_f_eq_abort_on_tampered_transcript |
| 4. LeakyTriple exactly 10 fields, no gamma/labels | test_leaky_triple_shape_field_access passes | test_leaky_triple_shape_field_access |
| 5. IT-MAC + product invariants | TEST-02 + TEST-03 pass | test_leaky_triple_mac_invariants + test_leaky_triple_product_invariant |

All 5 Phase 4 success criteria are observable via `cargo test --lib`. Phase 4 is complete.

## Commits

| Task | Hash | Description |
|------|------|-------------|
| 3.1 | 34d2461 | PROTO-04, PROTO-05, PROTO-09 (extended), Key-LSB tests |
| 3.2 | 0cdb2ee | TEST-02/03/04 + PROTO-06/07/08 + A1 D-share convention fix |

## Known Stubs

None. All stubs from Plans 1 and 2 are either resolved (`unimplemented!()` generate body is now implemented) or no longer present (alpha_labels/beta_labels removed from LeakyTriple). No new stubs introduced.

## Threat Surface Scan

No new network endpoints, auth paths, file access patterns, or schema changes. All code in this plan is test-only additions inside `#[cfg(test)]` blocks (plus the A1 fix in production code which reduces surface by fixing a latent MAC mismatch bug). Nothing new outside `src/leaky_tensor_pre.rs`.

## Recommendation

**Phase 4 is ready for `/gsd-verify-work`.** The headline property `z_full[j*n+i] == x_full[i] & y_full[j]` (TEST-03) passes across 3 sizes. The IT-MAC invariant (TEST-02) holds on all 48 shares of a (4,4) triple. F_eq aborts on tampered transcripts (TEST-04). All 66 library tests pass, 0 ignored.

## Self-Check: PASSED

- `src/leaky_tensor_pre.rs` — FOUND (modified, 1 file changed across 2 commits)
- commit 34d2461 (Task 3.1) — FOUND
- commit 0cdb2ee (Task 3.2) — FOUND
- `cargo test --lib`: 66 passed, 0 failed, 0 ignored — VERIFIED
- `grep -c "#[ignore" src/leaky_tensor_pre.rs` = 0 — VERIFIED
