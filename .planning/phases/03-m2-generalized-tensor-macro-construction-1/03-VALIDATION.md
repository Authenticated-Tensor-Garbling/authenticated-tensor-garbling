---
phase: 3
slug: m2-generalized-tensor-macro-construction-1
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | `Cargo.toml` — workspace root |
| **Quick run command** | `cargo test --lib 2>&1 \| tail -20` |
| **Full suite command** | `cargo test 2>&1 \| tail -30` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test 2>&1 | tail -30`
- **Before `/gsd-verify-work`:** Full suite must be green (or baseline-accepted failures documented)
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 3-W0-01 | W0 | 0 | PROTO-01 | — | N/A | baseline | `cargo test --lib 2>&1 \| tail -20` | ❌ W0 | ⬜ pending |
| 3-01-01 | garbler | 1 | PROTO-01 | — | N/A | unit | `cargo test tensor_macro::tests::test_tensor_garbler` | ❌ W0 | ⬜ pending |
| 3-01-02 | garbler | 1 | PROTO-02 | — | N/A | unit | `cargo test tensor_macro::tests::test_tensor_evaluator` | ❌ W0 | ⬜ pending |
| 3-02-01 | invariant | 2 | TEST-01 | — | N/A | integration | `cargo test tensor_macro::tests::test_xor_invariant` | ❌ W0 | ⬜ pending |
| 3-02-02 | invariant | 2 | PROTO-03 | — | N/A | integration | `cargo test tensor_macro::tests::test_xor_invariant_edge_cases` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/tensor_macro.rs` — module stub with `pub(crate) fn tensor_garbler(...)` and `pub(crate) fn tensor_evaluator(...)` signatures
- [ ] `#[cfg(test)] mod tests` inside `tensor_macro.rs` — test stubs for PROTO-01, PROTO-02, PROTO-03, TEST-01
- [ ] Baseline-accept or fix the 4 pre-existing failures in `leaky_tensor_pre` / `auth_tensor_pre` / `preprocessing` (Q1 decision required)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Z_garbler XOR Z_evaluator == a ⊗ T paper equivalence | PROTO-03 | Mathematical correctness beyond unit test vectors | Cross-reference with paper Construction 1 steps 1–6; spot-check n=4, m=128 output against manual derivation |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
