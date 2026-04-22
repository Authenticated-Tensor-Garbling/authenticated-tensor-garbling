---
phase: 05-m2-pi-atensor-correct-combining-construction-3
verified: 2026-04-22T21:34:40Z
status: passed
score: 7/7 must-haves verified
overrides_applied: 0
---

# Phase 5: Pi_aTensor Correct Combining (Construction 3) Verification Report

**Phase Goal:** Implement the correct combining step for Pi_aTensor (Construction 3): fix bucket_size_for formula per Theorem 1, implement two_to_one_combine helper with paper's Z=Z'⊕Z''⊕(x''⊗d) algebra, rewire combine_leaky_triples as iterative fold, and add TEST-05 regression battery.
**Verified:** 2026-04-22T21:34:40Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Two-to-one combine implements Z = Z'⊕Z''⊕x''⊗d with d MAC-verified; replaces naive XOR | ✓ VERIFIED | `two_to_one_combine` at line 27 implements Steps A–E; Step B calls `verify_cross_party(&gen_d[j], &eval_d[j], &delta_a, &delta_b)` per line 63; Z computed via `prime.gen_z_shares[k] + dprime.gen_z_shares[k] + dx_gen` at line 95 |
| 2 | Bucket size uses B = floor(SSP/log2(ell))+1 with ell = number of output triples (not n·m) | ✓ VERIFIED | `bucket_size_for(ell: usize)` at line 134; edge-case guard `if ell <= 1 { return SSP; }` at line 136; old `bucket_size_for(n, m)` signature has zero matches in src/ |
| 3 | Iterative combining folds B leaky triples one at a time into a single authenticated triple | ✓ VERIFIED | `let mut acc: LeakyTriple = triples[0].clone()` at line 196; `for next in triples.iter().skip(1) { acc = two_to_one_combine(acc, next); }` at lines 197–199 |
| 4 | Test verifies Z_combined = Z'⊕Z''⊕x''⊗d on two triples and MAC on d rejects tampered values | ✓ VERIFIED | `test_two_to_one_combine_product_invariant` at line 327 asserts product invariant; `test_two_to_one_combine_tampered_d_panics` at line 391 with `#[should_panic(expected = "MAC mismatch in share")]`; both pass |
| 5 | bucket_size_for(1) = 40, bucket_size_for(0) = 40 (SSP fallback) | ✓ VERIFIED | `test_bucket_size_formula_edge_cases` asserts these at lines 298–299; test passes |
| 6 | bucket_size_for(ell) for ell >= 2 returns floor(SSP/log2(ell)) + 1 | ✓ VERIFIED | `test_bucket_size_formula` asserts {2:41, 16:11, 128:6, 1024:5}; test passes |
| 7 | Full library test suite green (70 passed, 0 failed) | ✓ VERIFIED | `cargo test --lib` outputs: `test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s` |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/auth_tensor_pre.rs` | `pub fn bucket_size_for(ell: usize)` with ell<=1 guard + formula | ✓ VERIFIED | Line 134: `pub fn bucket_size_for(ell: usize) -> usize`; guard at line 136; formula at lines 139–140 |
| `src/auth_tensor_pre.rs` | `pub(crate) fn two_to_one_combine` implementing paper §3.1 algebra | ✓ VERIFIED | Line 27: `pub(crate) fn two_to_one_combine(prime: LeakyTriple, dprime: &LeakyTriple) -> LeakyTriple`; full 5-step implementation |
| `src/auth_tensor_pre.rs` | `pub(crate) fn verify_cross_party` at file scope (not test-only) | ✓ VERIFIED | Line 247: `pub(crate) fn verify_cross_party`; exactly 1 definition; no duplicate in mod tests |
| `src/auth_tensor_pre.rs` | `combine_leaky_triples` as thin iterative fold | ✓ VERIFIED | Lines 196–199: clone + fold body; old `combined_gen_z`/`combined_eval_z` patterns absent (0 matches) |
| `src/auth_tensor_pre.rs` | Three new TEST-05 test functions | ✓ VERIFIED | `test_two_to_one_combine_product_invariant` (line 327), `test_two_to_one_combine_tampered_d_panics` (line 391), `test_combine_full_bucket_product_invariant` (line 412) |
| `src/preprocessing.rs` | `bucket_size_for(count)` call site | ✓ VERIFIED | Line 87: `let bucket_size = bucket_size_for(count);`; no `bucket_size_for(n, m)` call in file |
| `src/leaky_tensor_pre.rs` | `#[derive(Clone)]` on `LeakyTriple` | ✓ VERIFIED | Line 36: `#[derive(Clone)]` present; required for `triples[0].clone()` in fold |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `combine_leaky_triples` | `two_to_one_combine` | `acc = two_to_one_combine(acc, next)` | ✓ WIRED | Line 198 confirmed; 14 total references to `two_to_one_combine` in file |
| `two_to_one_combine` | `verify_cross_party` | `verify_cross_party(&gen_d[j], &eval_d[j], &delta_a, &delta_b)` | ✓ WIRED | Line 63 confirmed |
| `two_to_one_combine` Step D | `AuthBitShare::default()` | zero share when d[j] == 0 | ✓ WIRED | Line 78: `let zero_share = AuthBitShare::default()`; used in conditional at lines 85–94 |
| `src/preprocessing.rs` | `bucket_size_for` | `bucket_size_for(count)` | ✓ WIRED | Line 87 confirmed |
| `test_two_to_one_combine_product_invariant` | `two_to_one_combine` | `let combined = two_to_one_combine(t0, t1_ref)` | ✓ WIRED | Line 337 confirmed |
| `test_two_to_one_combine_tampered_d_panics` | `verify_cross_party` (via `two_to_one_combine` Step B) | `#[should_panic(expected = "MAC mismatch in share")]` | ✓ WIRED | Line 390 confirmed; test passes as should_panic |
| `test_combine_full_bucket_product_invariant` | `combine_leaky_triples` | `combine_leaky_triples(triples, b, n, m, 1, 0)` | ✓ WIRED | Line 423 confirmed |

### Data-Flow Trace (Level 4)

Phase 5 produces library functions, not a UI/dashboard component. Data flows are verified through test execution rather than rendered display. The three TEST-05 tests exercise the full data path from `make_triples` → `two_to_one_combine`/`combine_leaky_triples` → product invariant assertion. All pass, confirming real data flows through the wiring.

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `two_to_one_combine` | `gen_z`, `eval_z` | Paper algebra on `prime`/`dprime` shares | Yes — combinatorially derived from IT-MAC shares; verified by product-invariant test | ✓ FLOWING |
| `combine_leaky_triples` | `acc` (combined triple) | Iterative `two_to_one_combine` fold | Yes — output sourced from `acc.gen_x_shares` etc. (not `triples[0]`); confirmed by line 216 | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `bucket_size_for(1) = 40` | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula_edge_cases -- --exact` | 1 passed | ✓ PASS |
| `bucket_size_for({2,16,128,1024})` formula | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula -- --exact` | 1 passed | ✓ PASS |
| Happy-path product invariant on 2 triples | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_product_invariant -- --exact` | 1 passed | ✓ PASS |
| Tamper-path MAC mismatch panic | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_tampered_d_panics -- --exact` | 1 passed (should panic) | ✓ PASS |
| Full-bucket fold (B=40) product invariant | `cargo test --lib auth_tensor_pre::tests::test_combine_full_bucket_product_invariant -- --exact` | 1 passed | ✓ PASS |
| Full library suite green | `cargo test --lib` | 70 passed, 0 failed, 0.02s | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| PROTO-10 | 05-02-PLAN | Implement correct combining: x = x', y = y', reveal d = y'⊕y'' (MAC-verified), Z = Z'⊕Z''⊕x''⊗d | ✓ SATISFIED | `two_to_one_combine` implements all steps; Steps A–E match paper §3.1 lines 427–443 exactly; functional through `combine_leaky_triples` fold |
| PROTO-11 | 05-02-PLAN | Iterative combining: fold B leaky triples one at a time using two-to-one combine | ✓ SATISFIED | `combine_leaky_triples` at line 161: `acc = triples[0].clone()` + `for next in triples.iter().skip(1) { acc = two_to_one_combine(acc, next); }` |
| PROTO-12 | 05-01-PLAN | Fix bucket size formula: B = floor(SSP/log2(ell))+1 where ell = number of output triples (not n·m) | ✓ SATISFIED | `bucket_size_for(ell: usize)` with `ell <= 1` guard; `run_preprocessing` calls `bucket_size_for(count)` |
| TEST-05 | 05-03-PLAN | Pi_aTensor combining: Z_combined = Z'⊕Z''⊕x''⊗d for two test triples; MAC on d rejects tampered values | ✓ SATISFIED | Three tests: happy-path product invariant, tampered-d should_panic, full-bucket fold invariant — all pass |

All four requirement IDs declared across plans are satisfied. No orphaned requirements detected (REQUIREMENTS.md maps PROTO-10, PROTO-11, PROTO-12, TEST-05 to Phase 5; all four are covered by plans).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | — |

No TODO/FIXME/HACK/PLACEHOLDER comments found in `src/auth_tensor_pre.rs`. No stub patterns (`return null`, empty handlers, hardcoded-empty returns) found. No stale `bucket_size_for(n, m)` calls in production files. The `alpha_labels: Vec::new()` and `beta_labels: Vec::new()` are intentional Phase 4 stubs (D-07 design decision preserved per plan), not bugs — Phase 6 populates them. The `_shuffle_seed` unused parameter is reserved for Phase 6 permutation bucketing per plan scope.

### Human Verification Required

None. All must-haves are verifiable programmatically via `cargo test`. The paper-algebraic correctness of Construction 3 is covered by:

- `test_two_to_one_combine_product_invariant`: asserts Z_combined = x_combined AND y_combined over all (i,j) pairs using concrete seeded LeakyTriples
- `test_two_to_one_combine_tampered_d_panics`: confirms MAC verification catches tampered y'' bit
- `test_combine_full_bucket_product_invariant`: exercises the production fold at B=40 and confirms the product invariant holds end-to-end

These three tests constitute a complete automated regression battery for Construction 3.

### Gaps Summary

No gaps found. All seven observable truths are verified, all four required artifacts pass the three-level check (exists, substantive, wired), all four requirement IDs are satisfied, and the full library suite is green at 70/70 tests. The phase goal is fully achieved.

---

_Verified: 2026-04-22T21:34:40Z_
_Verifier: Claude (gsd-verifier)_
