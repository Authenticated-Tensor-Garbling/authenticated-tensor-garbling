---
phase: 04-m2-pi-leakytensor-f-eq-construction-2
verified: 2026-04-21T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) Verification Report

**Phase Goal:** `Pi_LeakyTensor` is implemented per paper Construction 2: consume correlated randomness from `IdealBCot`, run two tensor-macro calls (A and B as garblers under their own Δ), XOR results, execute masked reveal, verify consistency via in-process `F_eq`, and output a leaky triple whose shape is exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` — no gamma, no wire labels.
**Verified:** 2026-04-21
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                                                   | Status     | Evidence                                                                                                                    |
|----|-------------------------------------------------------------------------------------------------------------------------|------------|-----------------------------------------------------------------------------------------------------------------------------|
| 1  | `Pi_LeakyTensor::generate` consumes itmac{x_A}{Δ_B}, itmac{x_B}{Δ_A}, itmac{y_A}{Δ_B}, itmac{y_B}{Δ_A}, itmac{R}{Δ} from IdealBCot; no direct AND | ✓ VERIFIED | 6 bCOT batch calls in generate() (lines 98-103); `transfer_a_to_b` × 3, `transfer_b_to_a` × 3; no bitwise `&` for correlated randomness |
| 2  | Two tensor-macro invocations (A and B as garblers) with C_A/C_B correlations; masked reveal yields public D; itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ} | ✓ VERIFIED | `tensor_garbler` called at lines 204-208 (under delta_a) and 220-224 (under delta_b); S_1/S_2 + D computation at lines 249-261; Z assembly at lines 297-314; `test_leaky_triple_product_invariant` passes |
| 3  | In-process F_eq receives L_1 = S_1 ⊕ D·Δ_A and L_2 = S_2 ⊕ D·Δ_B; matching inputs pass, mismatched abort — verified by test | ✓ VERIFIED | `feq::check(&l_1, &l_2)` at line 281 of `leaky_tensor_pre.rs`; `test_feq_passes_on_honest_run` passes; `test_f_eq_abort_on_tampered_transcript` and `feq::tests::test_check_differing_matrices_panics` both exercise the abort path |
| 4  | `LeakyTriple` struct contains exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})`; gamma bits and wire labels removed | ✓ VERIFIED | 10 fields present: n, m, gen_x_shares, gen_y_shares, gen_z_shares, eval_x_shares, eval_y_shares, eval_z_shares, delta_a, delta_b; grep for old field names (gen_alpha_shares, gen_correlated_shares, gen_gamma_shares, *_labels) returns 0; `test_leaky_triple_shape_field_access` passes |
| 5  | Tests verify IT-MAC equation `mac = key XOR bit·Δ` on every share, and XOR(gen, eval) of Z equals tensor product of XOR(gen, eval) of x and y | ✓ VERIFIED | `test_leaky_triple_mac_invariants` (TEST-02) calls `verify_cross_party` on all n+m+n*m shares at (4,4); `test_leaky_triple_product_invariant` (TEST-03) asserts `z_full[j*n+i] == x_full[i] & y_full[j]` at sizes (1,1), (2,3), (4,4) — all pass |

**Score:** 5/5 truths verified

### Deferred Items

None.

### Required Artifacts

| Artifact                     | Expected                                                          | Status     | Details                                                                         |
|------------------------------|-------------------------------------------------------------------|------------|---------------------------------------------------------------------------------|
| `src/delta.rs`               | Delta::new_with_lsb and Delta::random_b constructors             | ✓ VERIFIED | Both functions present at lines 41-53; `random_b` always sets LSB=0            |
| `src/bcot.rs`                | IdealBCot::new uses Delta::random_b for delta_b; regression test | ✓ VERIFIED | Line 52: `Delta::random_b(&mut rng_b)`; `test_delta_xor_lsb_is_one` passes     |
| `src/feq.rs`                 | Ideal F_eq module with `pub fn check` panic-on-mismatch          | ✓ VERIFIED | File exists; `check` at line 19 panics with "F_eq abort: ..." or dimension mismatch message |
| `src/lib.rs`                 | `pub mod feq;` registered                                        | ✓ VERIFIED | Line 22: `pub mod feq;` between bcot and leaky_tensor_pre                      |
| `src/leaky_tensor_pre.rs`    | LeakyTriple 10-field struct; full generate() implementation; paper-invariant tests | ✓ VERIFIED | Struct has exactly 10 fields; generate() runs all 5 Construction 2 steps; 10 tests all pass, 0 ignored |
| `src/auth_tensor_pre.rs`     | combine_leaky_triples references gen_x_shares/gen_y_shares/gen_z_shares | ✓ VERIFIED | `t0.gen_x_shares` and `t0.gen_y_shares` found; labels stubbed to Vec::new() with explicit Phase 5 comments |
| `src/preprocessing.rs`       | run_preprocessing calls ltp.generate() (no args)                 | ✓ VERIFIED | Line 100: `triples.push(ltp.generate());`                                       |

### Key Link Verification

| From                                              | To                                                  | Via                                                                 | Status     | Details                                                                       |
|---------------------------------------------------|-----------------------------------------------------|---------------------------------------------------------------------|------------|-------------------------------------------------------------------------------|
| `src/bcot.rs::IdealBCot::new`                    | `src/delta.rs::Delta::random_b`                     | `delta_b = Delta::random_b(&mut rng_b)` (replaces Delta::random)   | ✓ WIRED    | Line 52 in bcot.rs; `Delta::random(&mut rng_b)` not present                  |
| `src/lib.rs`                                     | `src/feq.rs`                                        | `pub mod feq;`                                                      | ✓ WIRED    | Line 22 in lib.rs                                                             |
| `src/preprocessing.rs::run_preprocessing`        | `src/leaky_tensor_pre.rs::LeakyTensorPre::generate` | `triples.push(ltp.generate())`                                      | ✓ WIRED    | Line 100; no-arg form confirmed                                               |
| `src/auth_tensor_pre.rs::combine_leaky_triples`  | `src/leaky_tensor_pre.rs::LeakyTriple`              | field access via gen_x_shares, gen_y_shares, gen_z_shares           | ✓ WIRED    | grep for t0.gen_x_shares and t0.gen_y_shares returns 1 each                  |
| `src/leaky_tensor_pre.rs::generate`              | `src/bcot.rs::IdealBCot::{transfer_a_to_b, b_to_a}` | 6 calls (x/y/R × 2 directions)                                     | ✓ WIRED    | Lines 98-103; 6 bCOT calls total                                              |
| `src/leaky_tensor_pre.rs::generate`              | `src/tensor_macro.rs::{tensor_garbler, tensor_evaluator}` | 2 garbler + 2 evaluator calls                                  | ✓ WIRED    | Lines 204-230; `tensor_macro::` imported at line 15                           |
| `src/leaky_tensor_pre.rs::generate`              | `src/feq.rs::check`                                 | `feq::check(&l_1, &l_2)` after L_1/L_2 construction                | ✓ WIRED    | Line 281; `feq` imported at line 16                                           |

### Data-Flow Trace (Level 4)

| Artifact                  | Data Variable      | Source                                  | Produces Real Data | Status      |
|---------------------------|--------------------|-----------------------------------------|--------------------|-------------|
| `src/leaky_tensor_pre.rs` | gen_z_shares       | bCOT + tensor_garbler/evaluator + D XOR | Yes — bCOT output from IdealBCot, macro from tensor_ops GGM tree | ✓ FLOWING |
| `src/preprocessing.rs`   | triples Vec        | ltp.generate() loop × bucket_size      | Yes — real LeakyTriple with IT-MAC verified shares | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior                                 | Command                                                                                  | Result                     | Status  |
|------------------------------------------|------------------------------------------------------------------------------------------|----------------------------|---------|
| cargo build succeeds                     | `cargo build --lib`                                                                      | exit 0                     | ✓ PASS  |
| Full test suite: 0 failures, 0 ignored  | `cargo test --lib`                                                                       | 66 passed; 0 failed; 0 ignored | ✓ PASS  |
| bcot:: 7 tests pass (incl. new LSB test) | `cargo test --lib bcot::`                                                               | 7 passed                   | ✓ PASS  |
| feq:: 3 tests pass                      | `cargo test --lib feq::`                                                                 | 3 passed                   | ✓ PASS  |
| leaky_tensor_pre:: 10 tests pass        | `cargo test --lib leaky_tensor_pre::`                                                    | 10 passed; 0 ignored       | ✓ PASS  |
| Product invariant z = x ⊗ y            | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_product_invariant`          | ok                         | ✓ PASS  |
| IT-MAC invariant on all shares          | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_mac_invariants`             | ok                         | ✓ PASS  |
| F_eq aborts on tampered transcript      | `cargo test --lib leaky_tensor_pre::tests::test_f_eq_abort_on_tampered_transcript`      | ok (should_panic)          | ✓ PASS  |

### Requirements Coverage

| Requirement | Source Plans       | Description                                                                                            | Status      | Evidence                                                                                        |
|-------------|--------------------|---------------------------------------------------------------------------------------------------------|-------------|-------------------------------------------------------------------------------------------------|
| PROTO-04    | 04-01, 04-02, 04-03 | Obtain correlated randomness from F_bCOT: itmac{x_A}{Δ_B}, itmac{x_B}{Δ_A}, itmac{y_A/B}{Δ}, itmac{R}{Δ} | ✓ SATISFIED | 6 bCOT batch calls in generate(); `test_correlated_randomness_dimensions` verifies lengths at 3 sizes |
| PROTO-05    | 04-02, 04-03       | Compute C_A and C_B (XOR combinations under Δ_A ⊕ Δ_B)                                                | ✓ SATISFIED | C_A/C_B computation at lines 154-186; `test_c_a_c_b_xor_invariant` verifies BDOZ identity     |
| PROTO-06    | 04-02, 04-03       | Execute two tensor macro calls (A garbler Δ_A, B garbler Δ_B) and XOR results                         | ✓ SATISFIED | tensor_garbler × 2 + tensor_evaluator × 2 in generate(); `test_macro_outputs_xor_invariant` (determinism regression) |
| PROTO-07    | 04-02, 04-03       | Masked tensor reveal: D = lsb(S1) ⊕ lsb(S2), then itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}          | ✓ SATISFIED | D computation at lines 256-261; Z assembly at 297-314; `test_d_extraction_and_z_assembly` verifies z shares |
| PROTO-08    | 04-01, 04-02, 04-03 | F_eq consistency check: L_1 = S_1 ⊕ D·Δ_A, L_2 = S_2 ⊕ D·Δ_B; abort if check fails              | ✓ SATISFIED | feq::check called at line 281; `test_feq_passes_on_honest_run` and `test_f_eq_abort_on_tampered_transcript` both pass |
| PROTO-09    | 04-01, 04-03       | LeakyTriple output is (itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ}) only — no gamma bits or wire labels    | ✓ SATISFIED | 10-field struct confirmed; old field names absent; `test_leaky_triple_shape_field_access` passes with real generate() output |
| TEST-02     | 04-03              | Leaky triple IT-MAC invariant: mac = key XOR bit · delta under verifier's delta for each share         | ✓ SATISFIED | `test_leaky_triple_mac_invariants` calls verify_cross_party on all n+m+n*m shares at (4,4)     |
| TEST-03     | 04-03              | Leaky triple product invariant: Z_full = x_full ⊗ y_full                                              | ✓ SATISFIED | `test_leaky_triple_product_invariant` passes at (1,1), (2,3), (4,4)                            |
| TEST-04     | 04-01, 04-03       | F_eq: correct L values pass; malformed L values cause abort                                            | ✓ SATISFIED | Unit: `feq::tests::test_check_differing_matrices_panics`; Integration: `test_f_eq_abort_on_tampered_transcript` (#[should_panic]) |

All 9 phase-declared requirements (PROTO-04 through PROTO-09, TEST-02 through TEST-04) are satisfied.

### Anti-Patterns Found

| File                       | Line | Pattern                                                        | Severity | Impact                                                                    |
|----------------------------|------|----------------------------------------------------------------|----------|---------------------------------------------------------------------------|
| `src/feq.rs`               | 8    | `TODO: Replace with real equality-check protocol`              | ℹ Info   | Documents intentional scope boundary (ideal vs. real); does not block Phase 4 goal |
| `src/bcot.rs`              | 20   | `TODO: Replace with real OT protocol`                          | ℹ Info   | Pre-existing from Phase 1; ideal functionality by design for this milestone |
| `src/auth_tensor_pre.rs`   | 90-102 | `alpha_labels: Vec::new()`, `beta_labels: Vec::new()`        | ℹ Info   | Intentional Phase 5 stub; explicitly commented; all downstream tests pass; Phase 5 PROTO-10/11 will rewrite combine semantics |

No blocker or warning anti-patterns. The Vec::new() stubs are explicitly documented as Phase 5 stubs and do not flow to any rendering or user-visible output in the tested paths — `test_run_preprocessing_feeds_online_phase` and `test_full_pipeline_no_panic` pass despite empty label vecs because AuthTensorGen/Eval handle them without panic.

### Human Verification Required

None. All phase goal truths are programmatically verifiable via cargo tests. The protocol is an in-process ideal functionality (no network, no UI) and all correctness properties are expressed as Rust assertions.

### Gaps Summary

No gaps. All 5 roadmap success criteria for Phase 4 are met, all 9 requirements are satisfied, and the full `cargo test --lib` run shows 66 passed, 0 failed, 0 ignored.

---

_Verified: 2026-04-21_
_Verifier: Claude (gsd-verifier)_
