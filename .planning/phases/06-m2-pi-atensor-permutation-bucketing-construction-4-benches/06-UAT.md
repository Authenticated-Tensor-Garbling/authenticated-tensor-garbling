---
status: complete
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
source: 06-01-SUMMARY.md, 06-02-SUMMARY.md, 06-03-SUMMARY.md
started: 2026-04-22T00:00:00Z
updated: 2026-04-22T00:05:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Full test suite passes (74 tests)
expected: Run `cargo test --lib`. All 74 tests pass with 0 failures and 0 errors.
result: pass

### 2. Construction 4 bucket size formula pinned
expected: Run `cargo test --lib -- auth_tensor_pre::tests::test_bucket_size_formula`. Test passes and asserts `bucket_size_for(4, 1) == 21`, `bucket_size_for(4, 2) == 15`, `bucket_size_for(16, 1) == 11`. No Construction 3 formula remains.
result: pass

### 3. Permutation step is active and `_shuffle_seed` is gone
expected: Run `grep -c "_shuffle_seed" src/auth_tensor_pre.rs` → outputs `0`. Run `grep -n "combine_leaky_triples" src/preprocessing.rs` → shows call site passing `42` as the last argument (shuffle_seed). The permutation loop is present in `combine_leaky_triples` (verify with `grep -n "apply_permutation_to_triple" src/auth_tensor_pre.rs`).
result: pass

### 4. End-to-end product invariant regression test (TEST-06)
expected: Run `cargo test --lib -- auth_tensor_pre::tests::test_run_preprocessing_product_invariant_construction_4`. Test passes, asserting: MAC invariant holds, product invariant (`z == x[i] & y[j]`) holds for all 16 (i,j) pairs, output dimensions correct, and `bucket_size_for(4, 1) == 21 < 40` (D-12 improvement pinned).
result: pass

### 5. Benchmark identifies Construction 4 and compiles
expected: Run `cargo bench --no-run`. Exits 0 (clean compile). Run `grep "Construction 4" benches/benchmarks.rs` → matches the `bench_preprocessing` doc comment at line 557 reading `Pi_aTensor' / Construction 4, Appendix F`. No `Construction 3` reference remains in that line.
result: pass

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]
