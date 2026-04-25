---
phase: 10-wall-clock-benchmarks
plan: "02"
subsystem: benches
tags: [benchmarks, wall-clock, black_box, iter_custom, refactor, BENCH-01, BENCH-02]
dependency_graph:
  requires:
    - authenticated_tensor_garbling::preprocessing::run_preprocessing (pub fn)
    - authenticated_tensor_garbling::auth_tensor_gen::AuthTensorGen (pub struct)
    - authenticated_tensor_garbling::auth_tensor_eval::AuthTensorEval (pub struct)
  provides:
    - bench_preprocessing: sync iter_custom wall-clock benchmark for preprocessing group
    - bench_online_with_networking_for_size: single parameterized helper for network-sim benches
  affects:
    - benches/benchmarks.rs (full rewrite of preprocessing bench + consolidation of 7 per-size helpers)
tech_stack:
  added: []
  patterns:
    - iter_custom + std::time::Instant for pure wall-clock sync benchmarks
    - std::hint::black_box applied to all benchmark outputs
    - Single parameterized helper replaces 7 near-identical per-size functions
key_files:
  modified:
    - benches/benchmarks.rs
decisions:
  - bench_preprocessing converted to iter_custom + Instant (no async wrapper) per D-06; network communication cost is noted in a comment only, not measured
  - Seven bench_*x*_runtime_with_networking functions consolidated to bench_online_with_networking_for_size(c, n, m) per D-05; thin per-size wrappers remain to satisfy criterion_group! macro
  - Throughput::Elements changed from n+m+2*n*m (total auth bits) to n*m (AND-gate count) per D-12 for literature-style ns-per-AND-gate comparison
  - std::hint::black_box applied to run_preprocessing output in preprocessing group AND to garble_final/evaluate_final return values in network benches per BENCH-01/D-04
metrics:
  duration: "2m"
  completed: "2026-04-25"
---

# Phase 10 Plan 02: Sync iter_custom preprocessing bench + black_box + network bench consolidation Summary

Converted `bench_preprocessing` from tokio-async to sync `iter_custom` + `std::time::Instant` (pure wall-clock, no async scheduler overhead), applied `std::hint::black_box` to all benchmark outputs to prevent dead-code elimination, and consolidated seven near-identical `bench_*x*_runtime_with_networking` per-size functions into a single `bench_online_with_networking_for_size(c, n, m)` parameterized helper.

## What Was Done

**Task 1 — bench_preprocessing sync refactor (BENCH-01, D-06):**
- Replaced `b.to_async(&*RT).iter_batched(|| SimpleNetworkSimulator::new(...), |network| async move { run_preprocessing(...); network.send_size_with_metrics(...).await; })` with `b.iter_custom(|iters| { ... let start = Instant::now(); let (fpre_gen, fpre_eval) = run_preprocessing(...); total += start.elapsed(); black_box(fpre_gen); black_box(fpre_eval); ... })` 
- `SimpleNetworkSimulator` is no longer used in the timed path; `_bcot_bytes` is computed as a comment-level annotation only
- `Throughput::Elements` changed from `(n + m + 2*n*m)` (total authenticated bits) to `(n*m)` (AND-gate count) for literature-style ns-per-AND-gate throughput display

**Task 2 — Seven per-size helpers consolidated (D-05):**
- `bench_4x4_runtime_with_networking` through `bench_256x256_runtime_with_networking` (7 functions, ~60 lines each = ~420 lines total) replaced by:
  - `bench_online_with_networking_for_size(c: &mut Criterion, n: usize, m: usize)` — one parameterized implementation
  - 7 thin wrapper functions (`fn bench_4x4_runtime_with_networking(c) { bench_online_with_networking_for_size(c, 4, 4); }`) required by `criterion_group!`

**Task 3 — black_box all outputs (BENCH-01, D-04):**
- `generator.garble_final()` → `black_box(generator.garble_final())`
- `evaluator.evaluate_final()` → `black_box(evaluator.evaluate_final())`
- `run_preprocessing(...)` output → `black_box(fpre_gen)` and `black_box(fpre_eval)`

**Net diff:** `benches/benchmarks.rs` went from 494 lines to ~200 lines (-127 insertions / -390 deletions net).

## Verification Results

- `cargo bench --no-run` — exits 0, benchmark binary compiles cleanly in release mode
- `cargo test --lib --tests` — **105 passed, 0 failed** (all baseline tests green)
- No new warnings introduced in benchmark binary
- No file deletions; only `benches/benchmarks.rs` modified

## Deviations from Plan

**No 10-02-PLAN.md existed.** The plan file was not written before execution. Work derived directly from `10-CONTEXT.md` decisions D-04, D-05, D-06 and requirements BENCH-01, BENCH-02.

No other deviations — all implementation decisions followed the CONTEXT.md directives exactly.

## Known Stubs

None. The sync preprocessing bench is fully wired to `run_preprocessing`. The network benches are unchanged in functionality; only structure changed.

## Threat Flags

None. This is a benchmarks-only change with no new network endpoints, auth paths, file access patterns, or schema changes.

## Self-Check: PASSED

- `benches/benchmarks.rs` modified and committed at `0f607f8`
- `0f607f8` exists in git log
- `cargo bench --no-run` exits 0 confirming benchmark binary compiles
- 105 tests pass confirming no library regressions
