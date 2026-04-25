---
phase: 10-wall-clock-benchmarks
verified: 2026-04-24T12:00:00Z
status: passed
score: 8/8 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 5/8
  gaps_closed:
    - "bench_online_p1 and bench_online_p2 each use iter_custom + std::time::Instant (no tokio, no async) and sweep BENCHMARK_PARAMS x chunking_factor 1..=8"
    - "Each measured iteration includes the full garble/eval pipeline AND the c_gamma assembly + check_zero call inside the timed region"
    - "cargo bench --no-run exits 0 in release mode (runtime execution also confirmed)"
  gaps_remaining: []
  regressions: []
deferred:
  - truth: "BENCH-05 (distributed half-gates / DTG) is not implemented"
    addressed_in: "v2"
    evidence: "ROADMAP.md Phase 10 requirements: 'BENCH-05 (deferred to v2)'; CONTEXT.md D-01 '4_distributed_garbling.tex is marked TODO, scrap'; intentionally absent from online_benches group"
---

# Phase 10: Wall-Clock Benchmarks Verification Report

**Phase Goal:** All garbling benchmarks correctly measure wall-clock time (no dead-code elimination, no async overhead), preprocessing and online phases are isolated into separate criterion groups, and Protocol 2 garble/evaluate/check is benchmarked alongside Protocol 1 with dual-unit throughput reporting (ms-per-tensor-op + ns-per-AND-gate). BENCH-05 (distributed half-gates) is deferred to v2 per Phase 10 CONTEXT D-01.
**Verified:** 2026-04-24T12:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure (Plan 04 closed the 3 gaps from initial verification)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | assemble_c_gamma_shares is callable from benches/benchmarks.rs (no #[cfg(test)] gate) | VERIFIED | `pub fn assemble_c_gamma_shares` at src/lib.rs:94, before `#[cfg(test)]` block at line 277; imported at benchmarks.rs:17 |
| 2 | assemble_c_gamma_shares_p2 is callable from benches/benchmarks.rs (no #[cfg(test)] gate) | VERIFIED | `pub fn assemble_c_gamma_shares_p2` at src/lib.rs:206, before `#[cfg(test)]` block; imported at benchmarks.rs:18 |
| 3 | All 105 existing tests still pass after the changes (zero regressions) | VERIFIED | `cargo test --lib --tests`: 105 passed, 0 failed |
| 4 | bench_preprocessing uses iter_custom + std::time::Instant (no tokio, no SimpleNetworkSimulator) and is in the 'preprocessing' criterion group | VERIFIED | benchmarks.rs:113 uses `benchmark_group("preprocessing")`; body uses `b.iter_custom` at line 139 and `Instant::now()` at line 142; no `b.to_async` or `SimpleNetworkSimulator::new` in function body |
| 5 | Every measured output (preprocessing output + every online bench's outputs) is wrapped in std::hint::black_box | VERIFIED | 18 total `black_box` calls in benchmarks.rs; preprocessing: 2 (fpre_gen, fpre_eval); bench_online_p1: 4 (c_gamma, check_ok, &generator, &evaluator); bench_online_p2: 6 (c_gamma, check_ok, gb_d_ev_out, ev_d_ev_out, &generator, &evaluator); network bench: 2 (garble_final, evaluate_final) |
| 6 | bench_online_p1 and bench_online_p2 each use iter_custom + std::time::Instant (no tokio, no async) and sweep BENCHMARK_PARAMS x chunking_factor 1..=8 | VERIFIED | Both functions use `b.iter_custom` (lines 201, 312), `Instant::now()` (lines 222, 327), sweep `for chunking_factor in 1usize..=8` (lines 193, 304); setup via `setup_auth_pair` (Plan 04 fix); no `b.to_async` or tokio in either body |
| 7 | Each measured iteration includes the full garble/eval pipeline AND the c_gamma assembly + check_zero call inside the timed region | VERIFIED | P1: garble_first_half through evaluate_final, lambda reconstruction, assemble_c_gamma_shares, check_zero all between `Instant::now()` and `total += start.elapsed()` (lines 222-248); P2: same structure with _p2 methods (lines 327-348); smoke bench confirmed: p1_garble_eval_check_4x4/1 produces `time: [2.1380 µs 2.1450 µs 2.1508 µs]` with no panic |
| 8 | criterion_main! exposes three groups: preprocessing_benches, online_benches, network_benches | VERIFIED | benchmarks.rs:484: `criterion_main!(preprocessing_benches, online_benches, network_benches)`; three `criterion_group!` invocations at lines 472-483 |

**Score:** 8/8 truths verified

### Deferred Items

Items not yet met but explicitly addressed in later milestone phases.

| # | Item | Addressed In | Evidence |
|---|------|-------------|----------|
| 1 | BENCH-05 (distributed half-gates / DTG) not implemented | v2 | ROADMAP.md Phase 10: "BENCH-05 (deferred to v2)"; CONTEXT.md D-01: "4_distributed_garbling.tex is marked TODO, scrap"; intentionally absent from online_benches per 10-03-SUMMARY.md |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/lib.rs` | pub fn assemble_c_gamma_shares + pub fn assemble_c_gamma_shares_p2 outside #[cfg(test)] | VERIFIED | Both pub fns at crate root (lines 94, 206); #[cfg(test)] block starts at line 277; test module imports via `use super::{assemble_c_gamma_shares, assemble_c_gamma_shares_p2}` at line 486 |
| `benches/benchmarks.rs` | sync iter_custom preprocessing bench + parameterized network helper + black_box on all outputs + bench_online_p1 + bench_online_p2 using setup_auth_pair | VERIFIED | All elements present; setup_auth_pair (lines 87-93) uses IdealPreprocessingBackend.run; both online benches wired to setup_auth_pair; 18 black_box calls; criterion_main with three groups |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| benches/benchmarks.rs::bench_online_p1 | assemble_c_gamma_shares + online::check_zero | iter_custom timed pipeline via setup_auth_pair | VERIFIED | setup_auth_pair (line 205) uses IdealPreprocessingBackend.run populating gamma_d_ev_shares; assemble_c_gamma_shares called at line 238; check_zero at line 246; smoke bench executed without panic |
| benches/benchmarks.rs::bench_online_p2 | assemble_c_gamma_shares_p2 + online::check_zero | iter_custom timed pipeline via setup_auth_pair | VERIFIED | setup_auth_pair at line 315; assemble_c_gamma_shares_p2 at line 338; check_zero at line 346 with &evaluator.delta_b; smoke bench executed without panic |
| benches/benchmarks.rs::bench_preprocessing | preprocessing::run_preprocessing | iter_custom closure measured by Instant::now/start.elapsed | VERIFIED | Confirmed: Instant::now() at line 142; run_preprocessing at line 143; total += start.elapsed() at line 144; black_box on both halves at lines 146-147 |
| benches/benchmarks.rs::setup_auth_pair | IdealPreprocessingBackend::run | TensorPreprocessing trait method | VERIFIED | Import at line 20; IdealPreprocessingBackend.run(n, m, 1, chunking_factor) at line 88; AuthTensorGen::new_from_fpre_gen at line 90; AuthTensorEval::new_from_fpre_eval at line 91 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| bench_online_p1 | generator.gamma_d_ev_shares | setup_auth_pair -> IdealPreprocessingBackend.run | Yes — IdealPreprocessingBackend.run populates gamma_d_ev_shares to length n*m on both gen and eval | FLOWING |
| bench_online_p2 | evaluator.gamma_d_ev_shares | setup_auth_pair -> IdealPreprocessingBackend.run | Yes — same as P1; correlated pair produced | FLOWING |
| bench_preprocessing | (fpre_gen, fpre_eval) | run_preprocessing() | Yes — real preprocessing pipeline producing TensorFpreGen/TensorFpreEval | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cargo bench --no-run exits 0 | cargo bench --no-run | Exits 0; 3 benchmark executables listed | PASS |
| 105 tests pass | cargo test --lib --tests | 105 passed, 0 failed | PASS |
| bench_online_p1 executes without panic | cargo bench -- 'online/p1_garble_eval_check_4x4/1' --warm-up-time 1 --measurement-time 2 --sample-size 10 | time: [2.1380 µs 2.1450 µs 2.1508 µs]; no gamma_d_ev_shares panic | PASS |
| bench_online_p2 executes without panic | cargo bench -- 'online/p2_garble_eval_check_4x4/1' --warm-up-time 1 --measurement-time 2 --sample-size 10 | time: [2.3420 µs 2.3493 µs 2.3536 µs]; no gamma_d_ev_shares panic | PASS |

Note: The network bench group panics at n=64 (`setup_auth_gen` calls `generate_for_ideal_trusted_dealer` which asserts `n <= usize::BITS - 1`). This is a pre-existing issue documented in `10-04-SUMMARY.md` "Issues Encountered" and is out of scope for Phase 10. The online and preprocessing benchmark groups function correctly.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| BENCH-01 | 10-02, 10-03 | All garbling benchmark outputs wrapped in black_box | SATISFIED | 18 black_box calls in benchmarks.rs; preprocessing tuple both halves; online bench c_gamma + check_ok + structs + P2 D_ev outputs; network bench garble_final/evaluate_final |
| BENCH-02 | 10-02, 10-03, 10-04 | Wall-clock benchmarks use iter_custom + std::time::Instant (no async) | SATISFIED | bench_preprocessing: iter_custom + Instant confirmed; bench_online_p1/p2: iter_custom + Instant confirmed; smoke benches execute and produce timing output |
| BENCH-04 | 10-01, 10-02, 10-03 | Preprocessing vs online phase comparison in separate criterion groups | SATISFIED | "preprocessing" criterion group (preprocessing_benches) and "online" criterion group (online_benches) both exist and execute; criterion_main with three groups |
| BENCH-05 | 10-03 | Distributed half gates benchmarked | DEFERRED to v2 | Per CONTEXT.md D-01: 4_distributed_garbling.tex marked "TODO, scrap"; intentionally absent from online_benches |
| BENCH-06 | 10-03 | Benchmark output reports wall-clock time per gate in ns alongside throughput | SATISFIED | Dual-unit reporting: ms_per_op and ns_per_and printed via println! (lines 266-271 for P1, 361-366 for P2); Throughput::Elements((n*m) as u64) for Criterion AND-gates/s output; confirmed via smoke bench output |

### Anti-Patterns Found

No blocking anti-patterns. The pre-existing n=64 panic in `bench_online_with_networking_for_size` (documented in 10-04-SUMMARY.md) is out of scope for Phase 10 and does not affect the online or preprocessing benchmark groups.

### Human Verification Required

None — all must-haves are programmatically verifiable and confirmed.

### Re-verification Summary

**Gaps closed by Plan 04:**

Plan 04 added `setup_auth_pair(n, m, chunking_factor) -> (AuthTensorGen, AuthTensorEval)` using `IdealPreprocessingBackend.run` which populates `gamma_d_ev_shares` (length n*m) on both sides via IT-MAC correlation. Both `bench_online_p1` (line 205) and `bench_online_p2` (line 315) now call `setup_auth_pair` instead of the separate `setup_auth_gen`/`setup_auth_eval` helpers that left `gamma_d_ev_shares` as `vec![]`. The `assemble_c_gamma_shares` assert at `src/lib.rs:109` and `assemble_c_gamma_shares_p2` assert at `src/lib.rs:218-219` now pass, allowing the full measured pipeline to execute.

Smoke bench evidence:
- `online/p1_garble_eval_check_4x4/1`: `time: [2.1380 µs 2.1450 µs 2.1508 µs]` — no panic
- `online/p2_garble_eval_check_4x4/1`: `time: [2.3420 µs 2.3493 µs 2.3536 µs]` — no panic
- 105 tests pass with zero regressions
- Only `benches/benchmarks.rs` was modified (no src/ changes)

---

_Verified: 2026-04-24T12:00:00Z_
_Verifier: Claude (gsd-verifier)_
