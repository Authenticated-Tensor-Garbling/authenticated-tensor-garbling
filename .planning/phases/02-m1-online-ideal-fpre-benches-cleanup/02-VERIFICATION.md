---
phase: 02-m1-online-ideal-fpre-benches-cleanup
verified: 2026-04-21T00:00:00Z
status: passed
score: 14/14 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 10/14
  gaps_closed:
    - "src/auth_tensor_gen.rs has NO `// awful return type` comment on gen_chunked_half_outer_product"
    - "src/auth_tensor_gen.rs garble_final has a `///` doc comment explaining its role"
    - "src/auth_tensor_eval.rs evaluate_final has a `///` doc comment explaining its role"
    - "src/auth_tensor_eval.rs eval_populate_seeds_mem_optimized has a GGM tree domain-separation comment above the two tccr calls"
  gaps_remaining: []
  regressions: []
---

# Phase 02: M1 Online Ideal Fpre Benches Cleanup — Verification Report

**Phase Goal:** M1 online phase cleanup — ideal Fpre refactor, benches dedup, documentation
**Verified:** 2026-04-21
**Status:** passed
**Re-verification:** Yes — after CLEAN-10 documentation gap closure

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | `TensorFpre::generate_for_ideal_trusted_dealer` exists; no `generate_with_input_values` in `src/` or `benches/` | ✓ VERIFIED | `grep -c` = 1. Old name count = 0 (macOS dup `* 2.` files excluded). |
| 2  | `TensorFpreGen` and `TensorFpreEval` live in `src/preprocessing.rs` | ✓ VERIFIED | `pub struct TensorFpreGen` count = 1; `pub struct TensorFpreEval` count = 1. |
| 3  | `run_preprocessing` lives in `src/preprocessing.rs` and is imported by all callers via `crate::preprocessing` | ✓ VERIFIED | `pub fn run_preprocessing` count = 1. All four callers import from `crate::preprocessing`. |
| 4  | `auth_tensor_fpre.rs` contains ONLY the ideal TensorFpre trusted dealer | ✓ VERIFIED | No `TensorFpreGen`, `TensorFpreEval`, or `run_preprocessing` definitions in the file. Imports from `crate::preprocessing`. |
| 5  | No `gamma_*` field or gamma generation code in TensorFpre, TensorFpreGen, TensorFpreEval, AuthTensorGen, AuthTensorEval | ✓ VERIFIED | `grep -rn "gamma_auth_bit" src/*.rs` = 0 matches in canonical files. |
| 6  | `src/leaky_tensor_pre.rs` gamma fields are UNCHANGED (cascade boundary respected) | ✓ VERIFIED | File untouched; `gen_gamma_shares` / `eval_gamma_shares` on LeakyTriple remain. |
| 7  | `cargo build --lib --tests` exits 0; test failure set matches before.txt exactly | ✓ VERIFIED | Build exits 0 (warnings only). 4 FAILED tests: `preprocessing::tests::test_run_preprocessing_mac_invariants`, `auth_tensor_pre::tests::test_combine_mac_invariants`, `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`, `leaky_tensor_pre::tests::test_correlated_mac_invariants` — identical to before.txt modulo expected path rename. |
| 8  | `src/auth_tensor_gen.rs` has NO `// awful return type` comment | ✓ VERIFIED | `grep -c "awful return type" src/auth_tensor_gen.rs` = **0**. Fixed. |
| 9  | `garble_final` has a `///` doc comment (`/// Combines`) | ✓ VERIFIED | `grep -c "/// Combines" src/auth_tensor_gen.rs` = **1**. |
| 10 | `evaluate_final` has a `///` doc comment (`/// Combines`) | ✓ VERIFIED | `grep -c "/// Combines" src/auth_tensor_eval.rs` = **1**. |
| 11 | GGM tree domain-separation comment in `eval_populate_seeds_mem_optimized` | ✓ VERIFIED | `grep -c "GGM tree" src/auth_tensor_eval.rs` = **1**. |
| 12 | `src/auth_gen.rs` and `src/auth_eval.rs` do not exist (CLEAN-11) | ✓ VERIFIED | Both files confirmed absent. |
| 13 | `benches/benchmarks.rs` has 2 `for cf in` loops; `preprocessing::run_preprocessing` import count = 1 | ✓ VERIFIED | `for cf in` count = 2; `preprocessing::run_preprocessing` count = 1. |
| 14 | Every `fn bench_*` definition has a paper-protocol header comment | ✓ VERIFIED | 10 bench functions, 10 header comments (confirmed in initial verification). |

**Score:** 14/14 truths verified

### Deferred Items

None.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/preprocessing.rs` | TensorFpreGen, TensorFpreEval, run_preprocessing, per-field docs | ✓ VERIFIED | Both structs present. `grep -c "    /// "` = 23 (≥18 required). `run_preprocessing` at line 79. |
| `src/auth_tensor_fpre.rs` | Ideal TensorFpre only; `generate_for_ideal_trusted_dealer` with trusted-dealer doc | ✓ VERIFIED | Method present. No real-protocol structs. Imports from `crate::preprocessing`. |
| `src/auth_tensor_pre.rs` | `combine_leaky_triples` with gamma removed; import from preprocessing | ✓ VERIFIED | No gamma fields. Imports `preprocessing::{TensorFpreGen, TensorFpreEval}`. |
| `src/auth_tensor_gen.rs` | No gamma field; import from preprocessing; no awful comment; `garble_final` `///` doc | ✓ VERIFIED | All four criteria met. Awful comment removed. `/// Combines` present. |
| `src/auth_tensor_eval.rs` | No gamma field; import from preprocessing; `evaluate_final` `///` doc; GGM comment | ✓ VERIFIED | All four criteria met. `/// Combines` present. `GGM tree` comment present. |
| `src/lib.rs` | `pub mod preprocessing;`; integration test uses renamed method | ✓ VERIFIED | `pub mod preprocessing;` present. `generate_for_ideal_trusted_dealer` count = 1. |
| `benches/benchmarks.rs` | for-loops over [1,2,4,6,8]; preprocessing import; renamed method; headers | ✓ VERIFIED | All criteria confirmed. |
| `before.txt` | 4 pre-existing failing test names | ✓ VERIFIED | 4 lines. Actual failures match exactly (path rename expected and matched). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/auth_tensor_fpre.rs` | `src/preprocessing.rs` | `use crate::preprocessing::{TensorFpreGen, TensorFpreEval}` | ✓ WIRED | `into_gen_eval` constructs and returns preprocessing structs. |
| `src/auth_tensor_gen.rs` | `src/preprocessing.rs` | `preprocessing::TensorFpreGen` (nested use block) | ✓ WIRED | Import confirmed. |
| `src/auth_tensor_eval.rs` | `src/preprocessing.rs` | `use crate::preprocessing::TensorFpreEval` | ✓ WIRED | Import confirmed. |
| `src/auth_tensor_pre.rs` | `src/preprocessing.rs` | `preprocessing::{TensorFpreGen, TensorFpreEval}` | ✓ WIRED | Import confirmed. |
| `benches/benchmarks.rs` | `src/preprocessing.rs` | `preprocessing::run_preprocessing` | ✓ WIRED | Import and call site confirmed. |
| `benches/benchmarks.rs` | `src/auth_tensor_fpre.rs` | `generate_for_ideal_trusted_dealer` calls | ✓ WIRED | Two call sites in setup helpers. |

### Data-Flow Trace (Level 4)

Not applicable. This phase produces no UI/rendering components. All artifacts are cryptographic protocol modules and benchmarks. The `run_preprocessing` data flow was confirmed in initial verification: it calls `combine_leaky_triples` and returns populated `(TensorFpreGen, TensorFpreEval)` structs (not stubs).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `cargo build --lib --tests` exits 0 | `cargo build --lib --tests` | Exit 0, warnings only | ✓ PASS |
| Test failure set matches before.txt | `cargo test --lib --no-fail-fast \| grep FAILED \| sort` | 4 failures, same 4 tests as before.txt | ✓ PASS |
| No `generate_with_input_values` in src/ or benches/ | `grep -rn "generate_with_input_values" src/ benches/` | 0 matches | ✓ PASS |
| No `// awful return type` in auth_tensor_gen.rs | `grep -c "awful return type" src/auth_tensor_gen.rs` | 0 | ✓ PASS |
| `/// Combines` doc in auth_tensor_gen.rs | `grep -c "/// Combines" src/auth_tensor_gen.rs` | 1 | ✓ PASS |
| `/// Combines` doc in auth_tensor_eval.rs | `grep -c "/// Combines" src/auth_tensor_eval.rs` | 1 | ✓ PASS |
| GGM tree comment in auth_tensor_eval.rs | `grep -c "GGM tree" src/auth_tensor_eval.rs` | 1 | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CLEAN-07 | Plan 02 | Rename `generate_with_input_values` to `generate_for_ideal_trusted_dealer` with trusted-dealer doc | ✓ SATISFIED | Method at auth_tensor_fpre.rs with 3-line `///` doc. Old name absent from all canonical src/ files. |
| CLEAN-08 | Plan 01 + Plan 02 | Separate `TensorFpre` from `TensorFpreGen`/`TensorFpreEval`; real structs in `preprocessing` module | ✓ SATISFIED | Both structs in preprocessing.rs. All 4 consumers import from `crate::preprocessing`. `lib.rs` declares `pub mod preprocessing;`. |
| CLEAN-09 | Plan 02 | Per-field `///` docs on TensorFpreGen and TensorFpreEval | ✓ SATISFIED | 23 field doc lines in preprocessing.rs (≥18 required). |
| CLEAN-10 | Plan 02 + Plan 04 | Audit auth_tensor_gen/eval for dead code, unexplained constants; doc non-obvious steps | ✓ SATISFIED | All 4 sub-items now met: awful comment removed; `garble_final` has `/// Combines` doc; `evaluate_final` has `/// Combines` doc; GGM tree comment present in `eval_populate_seeds_mem_optimized`. |
| CLEAN-11 | Plan 01 | Remove/isolate auth_gen.rs, auth_eval.rs if unused legacy | ✓ SATISFIED | Both files confirmed absent. |
| CLEAN-12 | Plan 03 | Deduplicate benchmark setup, add paper-protocol comments | ✓ SATISFIED | Two `for cf in` loops. `preprocessing::run_preprocessing` import count = 1. 10/10 bench functions have header comments. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `benches/benchmarks.rs` | ~569 | `2 * n * m` throughput comment references gamma bits (`n + m + 2*n*m`) which were removed — stale formula comment | ℹ️ Info | Does not block goal. Not a Phase 2 requirement. Carry forward to Phase 3 cleanup if desired. |

No blockers. The single remaining item is an informational stale comment with no correctness impact.

### Human Verification Required

None. All CLEAN-10 items are mechanically verifiable and confirmed passing.

### Gaps Summary

No gaps. All 14 must-haves verified. The 4 CLEAN-10 documentation gaps from the initial verification have been resolved:

- `// awful return type` comment removed from `auth_tensor_gen.rs`
- `/// Combines` doc added above `garble_final` in `auth_tensor_gen.rs`
- `/// Combines` doc added above `evaluate_final` in `auth_tensor_eval.rs`
- GGM tree domain-separation comment added above the two `cipher.tccr` calls in `eval_populate_seeds_mem_optimized`

Build is clean (exit 0) and the test failure set is identical to `before.txt` — no regressions introduced.

---

_Verified: 2026-04-21_
_Verifier: Claude (gsd-verifier)_
