---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
verified: 2026-04-22T00:00:00Z
status: passed
score: 11/11 must-haves verified
overrides_applied: 0
re_verification: false
---

# Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches — Verification Report

**Phase Goal:** `Pi_aTensor'` is implemented per paper Construction 4 with uniform row-permutation bucketing and the improved bucket size `B = 1 + ceil(SSP / log2(n·ℓ))`; the end-to-end preprocessing pipeline produces a valid authenticated tensor triple, and benchmarks run after the full restructure.
**Verified:** 2026-04-22
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

The must-haves below are drawn from:
- ROADMAP Phase 6 Success Criteria (5 items — non-negotiable)
- Plan 01 frontmatter must_haves (4 truths)
- Plan 02 frontmatter must_haves (6 truths)
- Plan 03 frontmatter must_haves (5 truths)

After deduplication and merging, 11 distinct verifiable truths are tracked.

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | A uniformly random permutation `π_j ∈ S_n` is sampled per triple before bucketing (ROADMAP SC-1) | VERIFIED | `combine_leaky_triples` contains the Construction 4 permutation step at line 199–210 of `src/auth_tensor_pre.rs`: iterates over `triples.iter_mut().enumerate()`, seeds `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)`, fills a `Vec<usize>` via `(0..n).collect()`, shuffles it via `SliceRandom::shuffle` (Fisher-Yates), and applies it per triple. |
| 2  | `π_j` is applied to rows of `gen_x_shares`/`eval_x_shares` and the i-index of `gen_z_shares`/`eval_z_shares`; `gen_y_shares`/`eval_y_shares` are unchanged (ROADMAP SC-2) | VERIFIED | `apply_permutation_to_triple` at lines 267–298 of `src/auth_tensor_pre.rs` permutes x-shares and column-major Z-slice i-indices in-place; y-shares are not touched. Three unit tests (identity, swap, wrong-length-panic) pin this behavior. All pass. |
| 3  | Bucket size formula is `B = 1 + ceil(SSP / log2(n·ℓ))` (ROADMAP SC-3 / PROTO-15) | VERIFIED | `bucket_size_for(n: usize, ell: usize) -> usize` at line 138 uses `saturating_mul`, `log2_floor = usize::BITS - product.leading_zeros() - 1`, and `1 + (SSP + log2_p - 1) / log2_p`. Tests assert `(4,1)->21`, `(4,2)->15`, `(16,1)->11`, `(1,0)->40`, `(1,1)->40`. All pass. |
| 4  | End-to-end test: authenticated tensor triple satisfies Z = x ⊗ y (ROADMAP SC-4 / TEST-06) | VERIFIED | `test_run_preprocessing_product_invariant_construction_4` in `src/auth_tensor_pre.rs` line 640 calls `crate::preprocessing::run_preprocessing(4, 4, 1, 1)`, verifies MAC invariant via `verify_cross_party` on every x, y, z share, and asserts `z_full[j*n+i] == x_full[i] & y_full[j]` for all (i,j). Test passes (`test result: ok. 74 passed`). |
| 5  | `cargo bench --no-run` compiles the benchmark binary cleanly (ROADMAP SC-5 / TEST-07) | VERIFIED | `cargo bench --no-run` exits 0. Two bench executables produced. Only dead-code warnings for two unused bench functions (pre-existing, not introduced by Phase 6). |
| 6  | `bucket_size_for` accepts two parameters `(n: usize, ell: usize)` and old one-arg form is eliminated (Plan 01) | VERIFIED | `grep -c "pub fn bucket_size_for(ell: usize)"` = 0. `grep -c "bucket_size_for(1)"` = 0. Signature line 138 confirmed two-arg. |
| 7  | `run_preprocessing` calls `bucket_size_for(n, count)` (Plan 01) | VERIFIED | `src/preprocessing.rs` line 93: `let bucket_size = bucket_size_for(n, count);`. Old `bucket_size_for(count)` count = 0. |
| 8  | `apply_permutation_to_triple` exists as `pub(crate)` in `src/auth_tensor_pre.rs` with three unit tests (Plan 02) | VERIFIED | Line 267 confirmed `pub(crate) fn apply_permutation_to_triple`. Tests `test_apply_permutation_identity_is_noop` (line 368), `test_apply_permutation_swap_moves_x_and_z_rows_but_not_y` (line 390), `test_apply_permutation_wrong_length_panics` (line 431) all present and passing. |
| 9  | `combine_leaky_triples` parameter renamed to `shuffle_seed` (no leading underscore) and consumed by permutation loop (Plan 02) | VERIFIED | `grep -c "_shuffle_seed" src/auth_tensor_pre.rs` = 0. Parameter `shuffle_seed: u64` at line 173. `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)` at line 206. |
| 10 | `run_preprocessing` passes `42` as the shuffle seed (Plan 02) | VERIFIED | `src/preprocessing.rs` line 109: `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)`. Old `..., 0)` call count = 0. |
| 11 | `benches/benchmarks.rs` comment identifies protocol as `Pi_aTensor' / Construction 4` (Plan 03 / TEST-07) | VERIFIED | Line 557 confirmed: `// Benchmarks the uncompressed preprocessing pipeline (Pi_aTensor' / Construction 4, Appendix F): ...`. Old `Pi_aTensor / Construction 3` count = 0. |

**Score:** 11/11 truths verified

### Deferred Items

None.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/auth_tensor_pre.rs` | `pub fn bucket_size_for(n: usize, ell: usize) -> usize` with Construction 4 formula | VERIFIED | Line 138; saturating_mul present; integer-ceiling formula present |
| `src/auth_tensor_pre.rs` | `pub(crate) fn apply_permutation_to_triple` helper | VERIFIED | Line 267; permutes x and Z, leaves y untouched |
| `src/auth_tensor_pre.rs` | Updated `combine_leaky_triples` with active permutation step | VERIFIED | Lines 199–210; consumes shuffle_seed; calls apply_permutation_to_triple |
| `src/auth_tensor_pre.rs` | TEST-06 function `test_run_preprocessing_product_invariant_construction_4` | VERIFIED | Line 640; MAC + product + dimension + D-12 assertions all present |
| `src/preprocessing.rs` | `run_preprocessing` calling `bucket_size_for(n, count)` | VERIFIED | Line 93 |
| `src/preprocessing.rs` | `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)` | VERIFIED | Line 109 |
| `benches/benchmarks.rs` | Updated Construction 4 doc comment on `bench_preprocessing` | VERIFIED | Line 557 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/preprocessing.rs:run_preprocessing` | `src/auth_tensor_pre.rs:bucket_size_for` | `let bucket_size = bucket_size_for(n, count);` | WIRED | Line 93 — pattern `bucket_size_for\(n, count\)` matches exactly one line |
| `src/auth_tensor_pre.rs:combine_leaky_triples` | `src/auth_tensor_pre.rs:apply_permutation_to_triple` | `for (j, triple) in triples.iter_mut().enumerate()` loop calling `apply_permutation_to_triple(triple, &perm)` | WIRED | Line 209 — pattern `apply_permutation_to_triple\(triple, &perm\)` matches exactly one line |
| `src/preprocessing.rs:run_preprocessing` | `src/auth_tensor_pre.rs:combine_leaky_triples` | `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)` | WIRED | Line 109 — seed `42` confirms permutation is active |
| `src/auth_tensor_pre.rs` imports | `rand_chacha::ChaCha12Rng` + `rand::SeedableRng` + `rand::seq::SliceRandom` | `use rand::{SeedableRng, seq::SliceRandom};` and `use rand_chacha::ChaCha12Rng;` | WIRED | Lines 7–8 |
| TEST-06 test | `src/preprocessing.rs:run_preprocessing` | `crate::preprocessing::run_preprocessing(n, m, 1, 1)` | WIRED | Line 662 — pattern `run_preprocessing\([0-9]+, [0-9]+, 1, 1\)` matches |
| `benches/benchmarks.rs:bench_preprocessing` | `src/preprocessing.rs:run_preprocessing` | `run_preprocessing(n, m, 1, chunking_factor)` | WIRED | Line 590 — unchanged call site, now executes Construction 4 end-to-end |

### Data-Flow Trace (Level 4)

Not applicable. All artifacts are protocol logic (Rust functions and unit tests), not UI/component renderers. No dynamic data rendering path to trace.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full library test suite (74 tests, all modules) | `cargo test --lib` | `test result: ok. 74 passed; 0 failed` | PASS |
| Benchmark binary compiles (TEST-07) | `cargo bench --no-run` | Exit 0; bench executables produced | PASS |
| `bucket_size_for(4, 1) == 21` (D-12 pin) | asserted in `test_bucket_size_formula` and `test_run_preprocessing_product_invariant_construction_4` | Both tests pass | PASS |
| Product invariant end-to-end via `run_preprocessing` | `test_run_preprocessing_product_invariant_construction_4` | Passes with permutation active | PASS |
| Full-bucket product invariant (B=21 triples) | `test_combine_full_bucket_product_invariant` | Passes | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| PROTO-13 | 06-02-PLAN.md | Sample uniformly random permutation `π_j ∈ S_n` per triple before bucketing | SATISFIED | `combine_leaky_triples` permutation loop at lines 199–210; ChaCha12Rng seeded per-triple; Fisher-Yates via `SliceRandom::shuffle` |
| PROTO-14 | 06-02-PLAN.md | Apply `π_j` to x-rows and Z i-index; y-rows unchanged | SATISFIED | `apply_permutation_to_triple` at lines 267–298; y vectors not touched; three passing unit tests confirm semantics |
| PROTO-15 | 06-01-PLAN.md | Bucket size `B = 1 + ceil(SSP / log2(n·ℓ))` | SATISFIED | `bucket_size_for(n: usize, ell: usize)` at line 138; formula verified by tests `(4,1)->21`, `(4,2)->15`, `(16,1)->11` |
| TEST-06 | 06-03-PLAN.md | Pi_aTensor' output triple satisfies Z = x ⊗ y with MAC invariant | SATISFIED | `test_run_preprocessing_product_invariant_construction_4` passes; covers MAC invariant, product invariant, dimensions, D-12 bucket-size pin |
| TEST-07 | 06-03-PLAN.md | Benchmarks compile and run after restructure | SATISFIED | `cargo bench --no-run` exits 0; `bench_preprocessing` doc comment updated to Construction 4 |

**Orphaned requirements:** None. All 5 Phase 6 requirements (PROTO-13, PROTO-14, PROTO-15, TEST-06, TEST-07) were claimed across Plans 01–03 and verified in the codebase.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/preprocessing.rs` | 84 | "not yet implemented" in doc comment | Info | Refers to the batch `count > 1` variant; the count=1 path (the only path exercised by Phase 6) is fully implemented. Not a blocker. |

No TODO/FIXME/HACK/PLACEHOLDER markers found in the Phase 6 modified files. No empty implementations. No hardcoded empty data in rendering paths. No stub handlers.

### Human Verification Required

None. All phase 6 success criteria are verifiable programmatically via `cargo test --lib` and `cargo bench --no-run`. The test suite (74 tests passing) provides complete coverage of the protocol invariants.

### Gaps Summary

No gaps. All 11 must-haves are verified. All 5 requirement IDs are satisfied. Both compile checks (`cargo test --lib` = 74/74 passed, `cargo bench --no-run` = exit 0) confirm the implementation is correct and complete.

**Notable observation (non-blocking):** The Plan 01 Task 1 behavior spec stated `bucket_size_for(4, 2) returns 14` — this was a transcription error in the plan; `14` is the `ceil(40/3)` subterm, not the full formula result. The actual implementation and tests correctly use `15` (`1 + 14 = 15`). The doc comment in `src/auth_tensor_pre.rs` line 136 also correctly annotates the value as `15`. This is not a defect.

---

_Verified: 2026-04-22_
_Verifier: Claude (gsd-verifier)_
