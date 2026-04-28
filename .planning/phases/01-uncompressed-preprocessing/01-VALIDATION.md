---
phase: 1
slug: uncompressed-preprocessing
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-19
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `criterion` for benchmarks |
| **Config file** | None (standard `cargo test`) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test && cargo bench -- preprocessing 2>/dev/null` |
| **Estimated runtime** | ~15 seconds (tests), ~120 seconds (full bench) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test && cargo build`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------------|-----------|-------------------|--------|
| 1-01-01 | 01-PLAN-cot | 1 | PREPROC-COT | IdealBCot produces correct correlation: receiver_mac[i] = sender_key[i] XOR choice[i]*delta | unit | `cargo test bcot::tests` | ⬜ pending |
| 1-02-01 | 01-PLAN-leaky-tensor | 2 | PREPROC-LEAKY | Pi_LeakyTensor produces valid cross-party AuthBitShares (verify_cross_party passes) | unit | `cargo test leaky_tensor_pre::tests` | ⬜ pending |
| 1-02-02 | 01-PLAN-leaky-tensor | 2 | PREPROC-BUCKET | Pi_aTensor bucketing reduces B leaky triples to 1 with combined MACs; bucket_size_for correct | unit | `cargo test auth_tensor_pre::tests` | ⬜ pending |
| 1-03-01 | 01-PLAN-fpre-replace | 3 | PREPROC-FPRE | run_preprocessing output feeds AuthTensorGen/Eval without panic; existing tests pass | integration | `cargo test auth_tensor_fpre::` | ⬜ pending |
| 1-04-01 | 01-PLAN-benchmarks | 4 | PREPROC-BENCH | bench_preprocessing runs for all 10 BENCHMARK_PARAMS without panic | smoke | `cargo bench -- preprocessing 2>/dev/null` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No new test files need to be created before execution — each plan creates its own `#[cfg(test)] mod tests` inline.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Benchmark throughput matches expected preprocessing cost | PREPROC-BENCH | No automated threshold — requires human judgment | Run `cargo bench -- preprocessing` and verify output includes throughput numbers for all (n,m) pairs |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: every task has an automated verify command
- [x] No MISSING test file references (all tests are inline in each module)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-04-19
