---
phase: 9
slug: protocol-2-garble-eval-check
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-24
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / cargo test |
| **Config file** | none (standard cargo) |
| **Quick run command** | `cargo test 2>&1` |
| **Full suite command** | `cargo test 2>&1` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test 2>&1`
- **After every plan wave:** Run `cargo test 2>&1`
- **Before `/gsd-verify-work`:** Full suite must be green (all 95 existing + new P2 tests)
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 9-01-01 | 01 | 1 | P2-01 | T-9-03 / rename-scope | gamma_auth_bit_shares grep returns zero matches after rename | unit | `grep -rn "gamma_auth_bit_shares" src/ \| wc -l` | ✅ | ⬜ pending |
| 9-01-02 | 01 | 1 | P2-01 | — | N/A | regression | `cargo test 2>&1` | ✅ | ⬜ pending |
| 9-02-01 | 02 | 1 | P2-01 | T-9-02 / tweak-collision | κ-half and ρ-half TCCR outputs are distinct (even/odd tweaks) | unit | `cargo test tensor_ops::tests 2>&1` | ❌ Wave 0 | ⬜ pending |
| 9-02-02 | 02 | 1 | P2-01 | — | IT-MAC invariant holds on wide output shares | unit | `cargo test tensor_ops::tests::test_wide_output_mac_invariant 2>&1` | ❌ Wave 0 | ⬜ pending |
| 9-03-01 | 03 | 2 | P2-02 | T-9-01 / garbler-privacy | garble_final_p2 return type contains no masked wire value | unit | `cargo test auth_tensor_gen::tests 2>&1` | ❌ Wave 0 | ⬜ pending |
| 9-03-02 | 03 | 2 | P2-03 | — | evaluate_final_p2 returns D_ev-authenticated output shares | unit | `cargo test auth_tensor_eval::tests 2>&1` | ❌ Wave 0 | ⬜ pending |
| 9-04-01 | 04 | 3 | P2-04 | T-9-01 / wrong-delta | P2 consistency check passes with delta_b (D_ev), not delta_a | integration | `cargo test tests::test_auth_tensor_product_full_protocol_2 2>&1` | ❌ Wave 0 | ⬜ pending |
| 9-04-02 | 04 | 3 | P2-04 | — | P1 tests remain green after P2 additions | regression | `cargo test tests::test_auth_tensor_product_full_protocol_1 2>&1` | ✅ | ⬜ pending |
| 9-04-03 | 04 | 3 | P2-05 | — | Garbler XOR evaluator output equals correct tensor product under _p2 path | integration | `cargo test tests::test_auth_tensor_product_full_protocol_2 2>&1` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for `gen_unary_outer_product_wide` in `src/tensor_ops.rs` (tests module) — covers P2-01 tweak independence and IT-MAC invariant
- [ ] Unit tests for `_p2` methods on `AuthTensorGen` (`src/auth_tensor_gen.rs`) — covers P2-02 garbler privacy
- [ ] Unit tests for `_p2` methods on `AuthTensorEval` (`src/auth_tensor_eval.rs`) — covers P2-03 D_ev authentication
- [ ] Integration test `test_auth_tensor_product_full_protocol_2` in `src/lib.rs` — covers P2-04 consistency check and P2-05 E2E correctness

*Existing infrastructure (`cargo test`) covers all scaffolding needs — no new test framework installation required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Garbler privacy — `garble_final_p2` does not send masked wire values | P2-02 | Type-level enforcement; verified by reading return type and call sites, not by test assertion | Inspect `garble_final_p2()` return type: must be `(Vec<Block>, Vec<Block>)` with no `L_gamma` field. Grep for `garble_final_p2` call sites and confirm no raw wire-value extraction. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
