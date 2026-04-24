---
phase: 7
slug: preprocessing-trait-ideal-backends
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-23
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` harness |
| **Config file** | none — standard `cargo test` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 7-01-01 | 01 | 1 | PRE-04 | — | N/A | compile | `cargo build` | ❌ W0 | ⬜ pending |
| 7-01-02 | 01 | 1 | PRE-04 | — | N/A | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| 7-02-01 | 02 | 2 | PRE-01 | — | N/A | unit | `cargo test --lib preprocessing::tests` | ❌ W0 | ⬜ pending |
| 7-02-02 | 02 | 2 | PRE-03 | — | N/A | unit | `cargo test --lib preprocessing::tests` | ❌ W0 | ⬜ pending |
| 7-03-01 | 03 | 2 | PRE-02 | — | N/A | unit | `cargo test --lib preprocessing::tests` | ❌ W0 | ⬜ pending |
| 7-03-02 | 03 | 2 | PRE-02 | — | IT-MAC: mac = key XOR bit * delta for all gamma_auth_bit_shares entries | unit | `cargo test --lib preprocessing::tests` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/preprocessing.rs` test module — stubs for PRE-01, PRE-02, PRE-03, PRE-04
  - `test_tensor_preprocessing_trait_uncompressed` — instantiates `UncompressedPreprocessingBackend`, calls `.run(2, 2, 1, 1)`, verifies return type
  - `test_tensor_preprocessing_trait_ideal` — instantiates `IdealPreprocessingBackend`, calls `.run(2, 2, 1, 1)`, verifies return type
  - `test_ideal_backend_gamma_auth_bit_shares_length` — verifies `gen.gamma_auth_bit_shares.len() == n*m`
  - `test_ideal_backend_gamma_mac_invariant` — verifies IT-MAC invariant using `verify_cross_party` pattern
  - `test_uncompressed_backend_regression` — calls through trait, asserts same result as direct `run_preprocessing`

*All new tests in `#[cfg(test)] mod tests { ... }` at bottom of `src/preprocessing.rs`.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| PRE-04 field semantically distinct from `correlated_auth_bit_shares` | PRE-04 / D-05 | Semantic paper correctness — automated test verifies length + IT-MAC but not semantic identity | Read CONTEXT.md D-05, confirm field docstring documents l_gamma vs l_gamma* distinction |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
