---
phase: 4
slug: m2-pi-leakytensor-f-eq-construction-2
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` harness (cargo test) |
| **Config file** | `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test --lib 2>&1 \| tail -30` |
| **Full suite command** | `cargo test 2>&1 \| tail -40` |
| **Estimated runtime** | ~5-10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -30`
- **After every plan wave:** Run `cargo test 2>&1 | tail -40`
- **Before `/gsd-verify-work`:** Full suite must be green (4 broken tests from Phase 3 baseline must be DELETED and replaced)
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 4-W0-01 | W0 | 0 | PROTO-04..09 | — | Δ_B LSB=0 so Δ_A⊕Δ_B has LSB=1 | unit | `cargo test --lib bcot::tests::test_delta_xor_lsb_is_one` | ❌ W0 | ⬜ pending |
| 4-W0-02 | W0 | 0 | PROTO-08, TEST-04 | — | F_eq module compiles; check() callable | unit | `cargo test --lib feq::` | ❌ W0 | ⬜ pending |
| 4-W0-03 | W0 | 0 | PROTO-09 | — | LeakyTriple compiles with renamed fields only | compile | `cargo build --lib` | ❌ W0 | ⬜ pending |
| 4-W0-04 | W0 | 0 | PROTO-04 | — | Correct dimensions on 5 bCOT batch pairs | unit | `cargo test --lib leaky_tensor_pre::tests::test_correlated_randomness_dimensions` | ❌ W0 | ⬜ pending |
| 4-01-01 | 01 | 1 | PROTO-05 | — | C_A⊕C_B == y(Δ_A⊕Δ_B) element-wise | unit | `cargo test --lib leaky_tensor_pre::tests::test_c_a_c_b_xor_invariant` | ❌ W0 | ⬜ pending |
| 4-01-02 | 01 | 1 | PROTO-06 | — | Macro outputs XOR invariant holds | integration | `cargo test --lib leaky_tensor_pre::tests::test_macro_outputs_xor_invariant` | ❌ W0 | ⬜ pending |
| 4-01-03 | 01 | 1 | PROTO-07 | — | D extraction and Z assembly correct | integration | `cargo test --lib leaky_tensor_pre::tests::test_d_extraction_and_z_assembly` | ❌ W0 | ⬜ pending |
| 4-01-04 | 01 | 1 | PROTO-08 | T-F_eq-mismatch | F_eq passes honest run; aborts on tamper | integration | `cargo test --lib feq::` | ❌ W0 | ⬜ pending |
| 4-02-01 | 02 | 2 | TEST-02 | T-cross-party-MAC | IT-MAC invariant holds on all x/y/z shares | integration | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_mac_invariants` | ❌ W0 | ⬜ pending |
| 4-02-02 | 02 | 2 | TEST-03 | — | Z(i,j) == x(i) AND y(j) for all (i,j) | integration | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_product_invariant` | ❌ W0 | ⬜ pending |
| 4-02-03 | 02 | 2 | TEST-04 | T-F_eq-softfail | F_eq panics on differing matrices | unit | `cargo test --lib feq::tests::test_check_differing_matrices_panics` | ❌ W0 | ⬜ pending |
| 4-02-04 | 02 | 2 | PROTO-09 | — | LeakyTriple shape: exact field set, no gamma/labels | compile+grep | `cargo build --lib && cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_shape` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] **Δ_B LSB fix** — modify `src/delta.rs` and `src/bcot.rs` so `delta_b.as_block().lsb() == 0`; add `bcot::tests::test_delta_xor_lsb_is_one`
- [ ] **`src/feq.rs` stub** — `pub fn check(l1: &BlockMatrix, l2: &BlockMatrix)` + 3 inline tests (equal-passes, differing-panics, dim-mismatch-panics); add `pub mod feq;` to `src/lib.rs`
- [ ] **New `LeakyTriple` struct** — fields per D-06/D-07/D-08/D-09; `LeakyTensorPre::generate(&mut self) -> LeakyTriple` signature with `unimplemented!()` body
- [ ] **Update `combine_leaky_triples`** — rename `gen_alpha_shares`→`gen_x_shares`, etc.; remove `*_labels` and `*_gamma_shares` references in `src/auth_tensor_pre.rs`
- [ ] **Update `preprocessing::run_preprocessing`** — change `ltp.generate(0, 0)` → `ltp.generate()` in `src/preprocessing.rs:99`
- [ ] **Delete 4 broken tests** — `test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `test_combine_mac_invariants`, `test_run_preprocessing_mac_invariants`
- [ ] **New test stubs** — PROTO-04 through TEST-04 as `#[test] fn test_xxx() { unimplemented!() }` placeholders (bodies filled in later waves)
- [ ] **Audit doc Q2** — cross-party AuthBitShare layout vs. paper notation; written before any C_A/C_B code
- [ ] **Audit doc Q1** — `itmac{D}{Δ}` local derivation convention with 2×2 worked example

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Q1: `itmac{D}{Δ}` derivation correctness | PROTO-07 | Paper does not give explicit field assignments; must whiteboard-verify before coding | Compute 2×2 example for all 4 (x,y) combinations; check IT-MAC invariant `mac = key XOR bit·Δ` on every resulting Z share |
| Q2: Cross-party AuthBitShare layout matches paper notation | PROTO-04..06 | doc-comment at `src/leaky_tensor_pre.rs:60-67` may need correction | Trace one bCOT call through `transfer_a_to_b` → verify `itmac{x_A}{Δ_B}` = (gen_share for party B under Δ_B) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
