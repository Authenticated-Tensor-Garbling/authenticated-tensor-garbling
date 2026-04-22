---
phase: 5
slug: m2-pi-atensor-correct-combining-construction-3
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` harness (cargo test) |
| **Config file** | `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test --lib 2>&1 \| tail -30` |
| **Full suite command** | `cargo test 2>&1 \| tail -40` |
| **Estimated runtime** | ~5-10 seconds (current baseline: ~0.03s for 66 tests) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -30`
- **After every plan wave:** Run `cargo test 2>&1 | tail -40`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 0 | PROTO-12 | T-5-05 | `bucket_size_for(ell≤1)` returns SSP=40 | unit | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula` | ✅ update | ⬜ pending |
| 5-01-02 | 01 | 0 | PROTO-12 | T-5-05 | `bucket_size_for(ell≤1)` edge case returns SSP | unit | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula_edge_cases` | ❌ W0 | ⬜ pending |
| 5-01-03 | 01 | 0 | PROTO-11 | — | Call site `preprocessing.rs:87` uses `bucket_size_for(count)` | unit | `cargo test --lib` | ✅ call site change | ⬜ pending |
| 5-02-01 | 02 | 1 | PROTO-10 | T-5-01 | `two_to_one_combine` produces `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d` | unit | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_product_invariant` | ❌ W0 stub | ⬜ pending |
| 5-02-02 | 02 | 1 | PROTO-10 | T-5-02 | MAC verify on d rejects tampered `y''` (panics) | integration | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_tampered_d_panics` | ❌ W0 stub | ⬜ pending |
| 5-03-01 | 03 | 2 | PROTO-11 | — | Iterative fold of B triples produces valid combined triple | integration | `cargo test --lib auth_tensor_pre::tests::test_full_pipeline_no_panic` | ✅ re-enable | ⬜ pending |
| 5-03-02 | 03 | 2 | PROTO-11 | — | Full bucket product invariant `Z = x ⊗ y` holds | integration | `cargo test --lib auth_tensor_pre::tests::test_combine_full_bucket_product_invariant` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/auth_tensor_pre.rs` — update `bucket_size_for` signature `(ell: usize)` and body (new formula)
- [ ] `src/preprocessing.rs` — update `bucket_size_for(n, m)` call site to `bucket_size_for(count)` at line 87
- [ ] `src/auth_tensor_pre.rs` — `test_bucket_size_formula` assertions updated for new formula values
- [ ] `src/auth_tensor_pre.rs` — new test `test_bucket_size_formula_edge_cases` for `ell ≤ 1` → SSP=40
- [ ] `src/auth_tensor_pre.rs` — new `pub(crate) fn two_to_one_combine` skeleton (may start as `unimplemented!()`)
- [ ] `src/auth_tensor_pre.rs` — new test stub `test_two_to_one_combine_product_invariant`
- [ ] `src/auth_tensor_pre.rs` — new test stub `test_two_to_one_combine_tampered_d_panics` (`#[should_panic]`)
- [ ] `src/auth_tensor_pre.rs` — new test `test_combine_full_bucket_product_invariant`

Framework install: not needed — `cargo test` already runs.

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
