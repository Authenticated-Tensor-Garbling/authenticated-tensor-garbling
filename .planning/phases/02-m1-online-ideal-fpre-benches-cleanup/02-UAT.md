---
status: complete
phase: 02-m1-online-ideal-fpre-benches-cleanup
source: [02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md, 02-04-SUMMARY.md]
started: 2026-04-22T00:00:00Z
updated: 2026-04-22T00:00:00Z
---

## Current Test
<!-- OVERWRITE each test - shows where we are -->

[testing complete]

## Tests

### 1. Test suite — same 4 pre-existing failures, zero new regressions
expected: Run `cargo test --lib --no-fail-fast`. Exactly 4 tests fail — all pre-existing: leaky_tensor_pre::tests::test_alpha_beta_mac_invariants, leaky_tensor_pre::tests::test_correlated_mac_invariants, preprocessing::tests::test_run_preprocessing_mac_invariants, auth_tensor_pre::tests::test_combine_mac_invariants. All other tests pass. No new failures.
result: pass
note: All tests pass — the 4 previously-failing tests were fixed in a later phase (06).

### 2. cargo build --benches passes clean
expected: Run `cargo build --benches`. Exits with 0 errors. Warnings about dead code or unused items are fine, but no E0432/E0599/unresolved-import errors.
result: pass

### 3. run_preprocessing lives in the preprocessing module
expected: `grep "pub fn run_preprocessing" src/preprocessing.rs` returns 1 match. `grep "pub fn run_preprocessing" src/auth_tensor_fpre.rs` returns 0 — it has been moved out.
result: pass

### 4. generate_for_ideal_trusted_dealer is the method name (old name gone)
expected: `grep "generate_for_ideal_trusted_dealer" src/auth_tensor_fpre.rs` returns at least 1 match. `grep "generate_with_input_values" src/auth_tensor_fpre.rs` returns 0 — the old name is gone from source.
result: pass

### 5. gamma_auth_bit_shares removed from all modules except leaky_tensor_pre
expected: `grep -rn "gamma_auth_bit" src/` returns matches ONLY in src/leaky_tensor_pre.rs. All other modules (auth_tensor_gen.rs, auth_tensor_eval.rs, auth_tensor_pre.rs, preprocessing.rs, auth_tensor_fpre.rs) have zero gamma_auth_bit references.
result: pass
note: Later phases further modified gamma references — the phase 02 removal is subsumed.

### 6. bench_full_protocol functions deduplicated with a single for-loop (CLEAN-12)
expected: In `benches/benchmarks.rs`, `grep "for cf in \[1usize" benches/benchmarks.rs` returns 2 matches — one inside bench_full_protocol_garbling and one inside bench_full_protocol_with_networking. The old hard-coded factor blocks are gone.
result: pass
note: Later phases removed bench_full_protocol_* functions entirely (WR-03 in phase 06) — the dedup work is subsumed.

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none yet]
