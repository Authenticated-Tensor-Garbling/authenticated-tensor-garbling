---
phase: 2
slug: m1-online-ideal-fpre-benches-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` harness + `criterion = "0.7"` for benchmarks |
| **Config file** | None (uses standard `cargo test`) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --lib && cargo bench --no-run` |
| **Estimated runtime** | ~10–15 seconds (tests ~5s + bench compile ~5–10s) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --lib && cargo build --benches`
- **Before `/gsd-verify-work`:** `cargo test --lib && cargo bench --no-run` + baseline diff against `before.txt` from Wave 0
- **Max feedback latency:** ~15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 2-??-01 | module-split | 1 | CLEAN-08 | — | Module boundaries preserved; no pub re-exports from auth_tensor_fpre | integration | `cargo build --lib --tests --benches` | ⚠️ W0 creates preprocessing.rs | ⬜ pending |
| 2-??-02 | rename+doc | 1 | CLEAN-07 | — | generate_for_ideal_trusted_dealer callable at all call sites | unit + smoke | `cargo test --lib auth_tensor_fpre::` + `cargo bench --no-run` | ✅ | ⬜ pending |
| 2-??-03 | gamma-removal | 2 | CLEAN-10 | — | No _gamma_share dead code; no awful comment; GGM comment present | grep | `grep -c "_gamma_share\|awful return type" src/auth_tensor_gen.rs` == 0 | ✅ | ⬜ pending |
| 2-??-04 | field-docs | 2 | CLEAN-09 | — | Every TensorFpreGen/Eval field has /// doc comment | manual/doc | `grep -B1 "pub .*:" src/preprocessing.rs \| grep "///"` | ⚠️ W0 | ⬜ pending |
| 2-??-05 | auth-gen-eval-audit | 2 | CLEAN-10 | — | garble_final/evaluate_final have doc comments; GGM tweak direction comment present | grep | `grep -c "/// " src/auth_tensor_gen.rs` >= 1 | ✅ | ⬜ pending |
| 2-??-06 | bench-dedup | 1 | CLEAN-12 | — | bench_full_protocol_garbling uses single loop; header comments present | grep + compile | `cargo bench --no-run` | ✅ | ⬜ pending |
| 2-??-07 | auth-gen-auth-eval-absent | 0 | CLEAN-11 | — | src/auth_gen.rs and src/auth_eval.rs do not exist | pre-verified | `test ! -f src/auth_gen.rs && test ! -f src/auth_eval.rs` | ✅ (trivially) | ⬜ pending |
| 2-??-08 | regression guard | any | Regression | — | No new test failures vs before.txt baseline | diff | `cargo test --lib 2>&1 \| grep 'FAILED' \| sort > after.txt && diff before.txt after.txt` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] **Baseline snapshot** — `cargo test --lib 2>&1 | grep 'FAILED' | sort > .planning/phases/02-m1-online-ideal-fpre-benches-cleanup/before.txt` (captures the 4 pre-existing failures before any code changes)
- [ ] **Module skeleton** — create `src/preprocessing.rs` with minimal `pub use` stubs + add `pub mod preprocessing;` to `src/lib.rs`

*Existing test infrastructure (rustc + cargo) covers all phase requirements. No new test framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TensorFpreGen/Eval field doc quality | CLEAN-09 | Doc comment content is semantic; grep only verifies presence | Run `cargo doc --no-deps --lib`; open `target/doc/authenticated_tensor_garbling/preprocessing/` and confirm every field has a meaningful description |
| D-15 GGM tweak direction accuracy | CLEAN-10 | D-15 wording may be inverted vs code; requires paper cross-check | Compare `seeds[j*2+1] = tccr(Block::from(0),...)` / `seeds[j*2] = tccr(Block::from(1),...)` against KRRW paper §GGM tree construction; use direction-neutral comment if ambiguous |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
