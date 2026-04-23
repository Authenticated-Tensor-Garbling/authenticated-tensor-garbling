---
status: complete
phase: 05-m2-pi-atensor-correct-combining-construction-3
source: [05-01-SUMMARY.md, 05-02-SUMMARY.md, 05-03-SUMMARY.md]
started: 2026-04-22T00:00:00Z
updated: 2026-04-22T00:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Bucket Size Formula Correctness
expected: Run `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula`. The test asserts Theorem 1 values: bucket_size_for(2)=41, bucket_size_for(16)=11, bucket_size_for(128)=6, bucket_size_for(1024)=5. Test passes with 0 failures.
result: pass

### 2. Edge-Case Guard (ell=0 and ell=1)
expected: Run `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula_edge_cases`. Both bucket_size_for(0) and bucket_size_for(1) return SSP=40 (no underflow/panic). Test passes.
result: pass

### 3. Construction 3 Product Invariant (Happy Path)
expected: Run `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_product_invariant`. Two leaky triples are generated (n=4, m=4), combined via two_to_one_combine, all 16 output shares satisfy Z_combined[j*4+i] == x_combined[i] AND y_combined[j], and all shares pass verify_cross_party. Test passes.
result: pass

### 4. MAC Tamper Detection (Tamper Path)
expected: Run `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_tampered_d_panics`. A triple with a flipped y'' value bit (MAC not updated) causes two_to_one_combine to panic with "MAC mismatch in share". Test passes (should_panic assertion holds).
result: pass

### 5. Full-Bucket Fold Product Invariant (B=40)
expected: Run `cargo test --lib auth_tensor_pre::tests::test_combine_full_bucket_product_invariant`. bucket_size_for(1)==40 is confirmed, 40 triples are combined via combine_leaky_triples, the resulting TensorFpreGen/TensorFpreEval pair satisfies the product invariant, all output shares pass verify_cross_party. Test passes.
result: pass

### 6. Full Suite Green (No Regressions)
expected: Run `cargo test --lib`. All 70 tests pass (0 failures). This includes the 67 baseline tests from phases 1-4 plus the 3 new TEST-05 tests.
result: pass

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
