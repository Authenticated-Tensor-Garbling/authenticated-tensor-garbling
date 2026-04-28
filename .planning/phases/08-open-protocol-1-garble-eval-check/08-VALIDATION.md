---
phase: 8
slug: open-protocol-1-garble-eval-check
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-23
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` harness |
| **Config file** | None — pure `cargo test` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** `cargo test --lib <module>` for the modified module (e.g., `cargo test --lib online`, `cargo test --lib auth_tensor_gen`)
- **After every plan wave:** `cargo test --lib`
- **Before `/gsd-verify-work`:** `cargo test` (full suite) must be green

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | P1-01 | — | `gamma_auth_bit_shares` forwarded to AuthTensorGen/Eval | unit | `cargo test --lib auth_tensor_gen::tests::test_compute_lambda_gamma_dimensions` | ❌ W0 | ⬜ pending |
| 08-01-02 | 01 | 1 | P1-01 | — | `compute_lambda_gamma` returns correct Vec<bool> length n*m | unit | `cargo test --lib auth_tensor_gen::tests` | ❌ W0 | ⬜ pending |
| 08-01-03 | 01 | 1 | P1-02 | — | Evaluator `compute_lambda_gamma(lambda_gb)` reconstructs L_gamma | unit | `cargo test --lib auth_tensor_eval::tests::test_compute_lambda_gamma_reconstruction` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 1 | P1-03 | — | `check_zero` returns true on honest c_gamma=0 with valid MAC | unit | `cargo test --lib online::tests::test_check_zero_passes_on_zero_bit_with_valid_mac` | ❌ W0 | ⬜ pending |
| 08-02-02 | 02 | 1 | P1-03 | — | `check_zero` returns false on bit=1 | unit | `cargo test --lib online::tests::test_check_zero_fails_on_nonzero_bit` | ❌ W0 | ⬜ pending |
| 08-02-03 | 02 | 1 | P1-03 | — | `check_zero` returns false on tampered MAC | unit | `cargo test --lib online::tests::test_check_zero_fails_on_invalid_mac` | ❌ W0 | ⬜ pending |
| 08-03-01 | 03 | 2 | P1-04 | — | End-to-end: L_gamma_ev == (input_x ⊗ input_y) XOR l_gamma | integration | `cargo test --lib tests::test_auth_tensor_product_full_protocol_1` | ❌ W0 | ⬜ pending |
| 08-03-02 | 03 | 2 | P1-05 | — | Tampered L_gamma_gb causes check_zero to return false | integration | `cargo test --lib tests::test_protocol_1_check_zero_aborts_on_tampered_lambda` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/online.rs` — new file with `check_zero()` + 3+ unit tests covering pass/fail-bit/fail-MAC paths
- [ ] `src/auth_tensor_eval.rs` — add `#[cfg(test)] mod tests {}` block (currently has no test module — TESTING.md coverage gap)
- [ ] `src/auth_tensor_gen.rs` — extend existing `mod tests` with `compute_lambda_gamma` dimension + correctness test stubs
- [ ] `src/lib.rs` — add `test_auth_tensor_product_full_protocol_1` and `test_protocol_1_check_zero_aborts_on_tampered_lambda` stubs

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s (cargo test --lib runs in ~5s)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
