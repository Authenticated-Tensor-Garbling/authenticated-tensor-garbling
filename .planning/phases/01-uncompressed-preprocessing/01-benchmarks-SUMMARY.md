---
phase: "01-uncompressed-preprocessing"
plan: "01-benchmarks"
subsystem: "benchmarks"
tags: [benchmark, preprocessing, criterion, throughput]
dependency_graph:
  requires: [01-fpre-replace]
  provides: [bench_preprocessing criterion group]
  affects: [benches/benchmarks.rs]
tech_stack:
  added: []
  patterns: [iter_batched + to_async + SimpleNetworkSimulator, Throughput::Elements]
key_files:
  modified:
    - benches/benchmarks.rs
decisions:
  - "Throughput reported as Throughput::Elements(n + m + 2*n*m) — counts all authenticated bits produced (alpha + beta + correlated + gamma)"
  - "Communication annotated as bcot_bytes = 2*(n+m+2*n*m)*size_of::<Block>() — simulated via SimpleNetworkSimulator"
  - "sample_size(10) applied when n*m > 4096 to prevent criterion timeout on 128x128 and 256x256"
  - "run_preprocessing placed inside measurement closure (not setup) to measure its wall-clock cost"
  - "TensorFpreGen and TensorFpreEval not imported — only run_preprocessing needed"
metrics:
  duration: "~4 minutes (dominated by initial cargo build)"
  completed: "2026-04-20T09:06:26Z"
  tasks_completed: 1
  tasks_total: 2
  files_modified: 1
---

# Phase 01 Plan benchmarks: Add bench_preprocessing to benchmarks Summary

bench_preprocessing criterion group added to benches/benchmarks.rs measuring run_preprocessing (Pi_aTensor) for all 10 BENCHMARK_PARAMS pairs with Throughput::Elements(n+m+2*n*m) auth bits and simulated bCOT communication.

## What Was Built

- `bench_preprocessing` function added to `benches/benchmarks.rs` (lines 746-789)
- Follows the exact `iter_batched + to_async + SimpleNetworkSimulator` pattern of existing benchmarks
- Iterates over all 10 `BENCHMARK_PARAMS` pairs: (4,4) through (256,256)
- `run_preprocessing(n, m, 1, chunking_factor=1)` is the measured operation in the async closure
- `SimpleNetworkSimulator::new(100.0, 0)` created in setup closure; `send_size_with_metrics(bcot_bytes)` called in measurement closure for bandwidth annotation
- `bench_preprocessing` registered in `criterion_group!`

## BENCHMARK_PARAMS Change

Added `(256, 256)` as the 10th entry:

```
(4,4), (8,8), (16,16), (24,24), (32,32), (48,48), (64,64), (96,96), (128,128), (256,256)
```

## Throughput Formula

```
n_auth_bits = n + m + 2*n*m
  = n  (alpha bits)
  + m  (beta bits)
  + n*m (correlated bits)
  + n*m (gamma bits)

Throughput::Elements(n_auth_bits as u64)
```

## Communication Formula

```
bcot_bytes = 2 * (n + m + 2*n*m) * size_of::<Block>()
           = 2 * n_auth_bits * 16  (Block = 16 bytes)
```

## Sample Size Adjustment

For large sizes (n*m > 4096, i.e. 128x128 and 256x256), `group.sample_size(10)` is applied before `bench_with_input` to prevent criterion from timing out during the measurement phase.

## Throughput Numbers

Throughput numbers are pending human verification (Task 2 checkpoint). Expected values to be filled in after running:

```
cargo bench -- preprocessing
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unused TensorFpreGen and TensorFpreEval imports**
- **Found during:** Compile verification
- **Issue:** Plan specified importing `TensorFpreGen` and `TensorFpreEval` but neither is referenced in bench_preprocessing — only `run_preprocessing` is used. Compiler emitted unused_imports warnings.
- **Fix:** Removed the two unused types from the import line, keeping only `TensorFpre` and `run_preprocessing`.
- **Files modified:** benches/benchmarks.rs (line 16)
- **Commit:** 5340f56

## Task Status

| Task | Name | Status | Commit |
|------|------|--------|--------|
| 1 | Add bench_preprocessing and (256,256) | Complete | 5340f56 |
| 2 | Human verify cargo bench -- preprocessing | Awaiting checkpoint | — |

## Existing Benchmarks

All pre-existing benchmark functions remain intact and registered in `criterion_group!`:
- bench_4x4_runtime_with_networking
- bench_8x8_runtime_with_networking
- bench_16x16_runtime_with_networking
- bench_32x32_runtime_with_networking
- bench_64x64_runtime_with_networking
- bench_128x128_runtime_with_networking
- bench_256x256_runtime_with_networking

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| benches/benchmarks.rs exists | FOUND |
| 01-benchmarks-SUMMARY.md exists | FOUND |
| commit 5340f56 exists | FOUND |
| fn bench_preprocessing at line 746 | FOUND |
| bench_preprocessing in criterion_group! at line 800 | FOUND |
| (256, 256) in BENCHMARK_PARAMS at line 39 | FOUND |
| cargo check --bench benchmarks | PASSED (0 errors, 2 pre-existing warnings) |
