---
status: complete
phase: 01-uncompressed-preprocessing
source: [01-cot-SUMMARY.md, 01-leaky-tensor-SUMMARY.md, 01-fpre-replace-SUMMARY.md, 01-benchmarks-SUMMARY.md, 01-keys-sharing-SUMMARY.md, 01-bcot-migration-SUMMARY.md, 01-matrix-ops-aes-SUMMARY.md]
started: 2026-04-22T00:00:00Z
updated: 2026-04-22T12:00:00Z
---

## Current Test
<!-- OVERWRITE each test - shows where we are -->

[testing complete]

## Tests

### 1. Full test suite — bcot, keys, sharing, leaky_tensor_pre, auth_tensor_pre, auth_tensor_fpre
expected: Run `cargo test` from the project root. All tests in the modules added by phase 01 pass: bcot (6 tests), keys (4 tests), sharing (3 tests), leaky_tensor_pre (8 tests), auth_tensor_pre (4 tests), auth_tensor_fpre (6 tests). No regressions in pre-existing tests.
result: pass

### 2. Benchmark compiles clean
expected: Run `cargo check --bench benchmarks`. Exits with 0 errors. Pre-existing warnings (dead_code, unreachable_pub) may appear but no new errors from phase 01 changes.
result: pass

### 3. BENCHMARK_PARAMS has 10 entries including (256,256)
expected: Open `benches/benchmarks.rs` and find the BENCHMARK_PARAMS array (around line 39). It lists exactly 10 pairs: (4,4), (8,8), (16,16), (24,24), (32,32), (48,48), (64,64), (96,96), (128,128), (256,256).
result: pass

### 4. bench_preprocessing is registered in criterion_group!
expected: In `benches/benchmarks.rs`, `bench_preprocessing` appears in the `criterion_group!` macro alongside the existing per-size benchmark functions.
result: pass

### 5. run_preprocessing feeds the online phase
expected: In `src/auth_tensor_fpre.rs`, `pub fn run_preprocessing(n, m, count, chunking_factor)` exists. The test `test_run_preprocessing_feeds_online_phase` passes — it verifies that run_preprocessing output is accepted by `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` without panicking.
result: pass

### 6. Key::new enforces LSB=0 at construction
expected: In `src/keys.rs`, `Key::new()` exists and calls `set_lsb(false)` before wrapping. The test `test_key_new_clears_lsb_when_set` passes — verifies that a block with LSB=1 passed to Key::new() comes out with LSB=0.
result: pass

### 7. InputSharing::bit() is gone — shares_differ() is the only method
expected: Searching the codebase for `InputSharing` usage shows no calls to `.bit()` on `InputSharing` values — all call sites use `.shares_differ()`. Running `cargo build --lib` compiles with no "method not found" errors for shares_differ.
result: pass

## Summary

total: 7
passed: 7
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none yet]
