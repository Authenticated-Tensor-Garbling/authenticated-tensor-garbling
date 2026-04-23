---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
reviewed: 2026-04-22T00:00:00Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - src/auth_tensor_pre.rs
  - src/preprocessing.rs
  - benches/benchmarks.rs
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 06: Code Review Report

**Reviewed:** 2026-04-22
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

This phase introduces the Construction 4 permutation-bucketing pipeline (`apply_permutation_to_triple`, `bucket_size_for`, `combine_leaky_triples`), the `run_preprocessing` entry point in `preprocessing.rs`, and an extended benchmark suite in `benches/benchmarks.rs`.

The cryptographic core is sound and the MAC invariant is correctly propagated through the bucketing fold. Three warnings and three info items were found. The most impactful is a seed-collision risk in `combine_leaky_triples` when `shuffle_seed == 0` and the number of triples exceeds 1 (seeds `0^0=0` and `0^(bucket_size)=bucket_size` are distinct, but `shuffle_seed ^ j` gives `j` directly when `shuffle_seed=0`, which is weak diversity — more importantly, `0 ^ 0 = 0` means triple 0 always gets the identity-biased seed regardless of caller intent). The two benchmarking functions (`bench_full_protocol_garbling`, `bench_full_protocol_with_networking`) are defined but never registered in `criterion_group!`, making them permanently dead code. There is also a subtle logic issue in `run_preprocessing` where the `total_leaky` count calculation uses `count` (always 1 per the assert) rather than the actual bucket demand, meaning it happens to be correct today but will silently produce the wrong bucket count if the `count != 1` restriction is ever lifted without updating this formula.

---

## Warnings

### WR-01: Seed `shuffle_seed ^ j` collapses to `j` when `shuffle_seed == 0` — triple 0 always gets seed 0

**File:** `src/auth_tensor_pre.rs:206`

**Issue:** The per-triple RNG is seeded with `shuffle_seed ^ j as u64`. When the caller passes `shuffle_seed = 0` (which `run_preprocessing` does at line 109 of `preprocessing.rs`), this reduces to `seed = j`. Triple 0 therefore always receives seed 0, regardless of the caller's intent to randomise. More importantly, the design intent of `shuffle_seed` is to allow the caller to vary the permutation family across protocol runs; with seed 0, different runs of `run_preprocessing` always apply the same permutation to triple 0, eliminating the per-run freshness that Construction 4 requires for security amplification.

The correct fix is to use a keyed derivation (e.g., hash `shuffle_seed || j` or use `ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j as u64))`) so that `shuffle_seed = 0` does not make triple 0's permutation predictable.

**Fix:**
```rust
// Replace line 206 in combine_leaky_triples:
// Before:
let mut rng = ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64);

// After (wrapping_add gives distinct seeds for all j even when shuffle_seed=0):
let mut rng = ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j as u64));
```

Separately, `run_preprocessing` should pass a randomly-generated seed rather than the hardcoded `42`, or document explicitly that a fixed seed is intentional for reproducibility:
```rust
// preprocessing.rs line 109:
// If reproducibility is the goal, document it:
//   combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42 /* fixed for reproducibility */)
// If security freshness is required per run, derive from a random seed:
//   combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, rand::random())
```

---

### WR-02: `total_leaky` formula is accidentally correct but breaks if `count != 1` restriction is lifted

**File:** `src/preprocessing.rs:91-94`

**Issue:** The function asserts `count == 1` on line 91 and then computes:
```rust
let bucket_size = bucket_size_for(n, count);  // = bucket_size_for(n, 1)
let total_leaky = bucket_size * count;         // = bucket_size * 1 = bucket_size
```

The `bucket_size_for` formula is `1 + ceil(SSP / log2(n * ell))` where `ell` is the number of OUTPUT triples. When `count = 1`, `ell = count = 1` is the intended argument. However, the formula is intended to size a bucket for `ell` output triples — meaning the total number of leaky triples needed is `bucket_size_for(n, ell) * ell`, not `bucket_size_for(n, count) * count`. The current code happens to work for `count = 1` (`1 * 1 = 1`), but the `bucket_size_for` call passes `count` (the number of outputs) both as the ell argument and then multiplies by `count`. When `count = 2` (if the assertion is ever removed), `total_leaky = bucket_size_for(n, 2) * 2` — this is actually correct, but only by accident since the same `count` is used as `ell`. The real risk is that `bucket_size_for` is also being called with `count` as `ell` inside the function to compute the size, and the multiplication is by the same `count`. This happens to be mathematically correct but is confusing: if a caller later passes `count > 1` after removing the assert, the function will silently produce `count` output triples from `bucket_size * count` leaky triples — which is the right quantity — but the combine call on line 109 only returns one pair, so the extra `(bucket_size * (count - 1))` generated triples are silently discarded (only the first bucket is combined and returned).

The assert prevents this for now, but the relationship between the assertion and the silently-discarded triples should be documented explicitly.

**Fix:**
```rust
// preprocessing.rs — add a comment clarifying the assertion's load-bearing role:
assert_eq!(count, 1, "Phase 1: only count=1 is supported; \
    batch output requires a Vec-returning variant. \
    Note: total_leaky = bucket_size * count generates enough triples \
    for 'count' outputs but combine_leaky_triples below only consumes \
    bucket_size of them — remove this assert only after adding a loop \
    that calls combine_leaky_triples once per output triple.");
```

---

### WR-03: `bench_full_protocol_garbling` and `bench_full_protocol_with_networking` are defined but never registered — permanently dead code

**File:** `benches/benchmarks.rs:87-177`

**Issue:** Two benchmark functions — `bench_full_protocol_garbling` (lines 87-115) and `bench_full_protocol_with_networking` (lines 118-177) — are defined but absent from the `criterion_group!(benches, ...)` macro at line 603. Criterion never calls them. The `_setup_semihonest_gen` and `_setup_semihonest_eval` helper functions (lines 47-68) exist solely to support these dead benchmarks and are themselves dead (prefixed with `_` as an acknowledgement). The registered group contains only the per-dimension networking benchmarks and `bench_preprocessing`.

This is not merely cosmetic: dead benchmark functions prevent the compiler from flagging regressions (e.g., if `garble_first_half` signature changes, the broken dead code will not be caught until the benchmark is re-enabled), and readers may assume these benchmarks run as part of the suite.

**Fix:** Either register the two functions in `criterion_group!` if they are intended to run:
```rust
criterion_group!(
    benches,
    bench_full_protocol_garbling,        // add
    bench_full_protocol_with_networking, // add
    bench_4x4_runtime_with_networking,
    // ... rest unchanged
);
```
Or delete the two functions and their helper setup functions (`_setup_semihonest_gen`, `_setup_semihonest_eval`) if they are superseded by the per-dimension networking benchmarks.

---

## Info

### IN-01: `triples[0].clone()` is unnecessary — `triples.into_iter().next()` avoids a clone

**File:** `src/auth_tensor_pre.rs:215-217`

**Issue:** After the permutation loop borrows `triples` mutably, `triples[0].clone()` is used to seed the fold accumulator while leaving `triples[0]` alive so the subsequent `triples.iter().skip(1)` can still borrow the full vec. This is correct but wastes one clone of a `LeakyTriple` (which contains six `Vec<AuthBitShare>` fields, each of length proportional to `n` or `n*m`). An iterator-based fold would consume ownership of all triples without any clone.

**Fix:**
```rust
// Replace lines 215-218:
// Before:
let mut acc: LeakyTriple = triples[0].clone();
for next in triples.iter().skip(1) {
    acc = two_to_one_combine(acc, next);
}

// After (no clone, ownership transferred):
let mut iter = triples.iter();
let first = iter.next().expect("bucket_size >= 1 asserted above").clone();
// Actually to fully avoid clone, consume the vec:
let mut triples_iter = triples.into_iter();
let mut acc = triples_iter.next().expect("bucket_size >= 1 asserted above");
// But two_to_one_combine takes &LeakyTriple for dprime, so we need collected refs:
// Simplest fix that avoids the full-vec clone:
let mut triples_iter = triples.into_iter();
let mut acc = triples_iter.next().unwrap(); // safe: bucket_size >= 1 asserted
let remaining: Vec<LeakyTriple> = triples_iter.collect();
for next in &remaining {
    acc = two_to_one_combine(acc, next);
}
```

Note: This is a minor allocation trade-off (one `Vec` vs one `LeakyTriple` clone). The current approach is acceptable; this is purely a style/efficiency note.

---

### IN-02: Magic number `42` hardcoded as `shuffle_seed` in `run_preprocessing`

**File:** `src/preprocessing.rs:109`

**Issue:** `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)` uses the literal `42` as the shuffle seed with no explanation. Given that WR-01 already flags the security implications of a fixed seed, this should at minimum be a named constant (e.g., `SHUFFLE_SEED: u64 = 42`) with a doc comment explaining whether the fixed value is intentional for reproducibility or is a placeholder.

**Fix:**
```rust
// preprocessing.rs — before run_preprocessing:
/// Fixed shuffle seed used for the per-triple row permutation in Construction 4.
/// This seed is intentionally fixed for reproducibility in benchmarks and tests.
/// Replace with `rand::random::<u64>()` if per-run freshness is required.
const SHUFFLE_SEED: u64 = 42;

// In run_preprocessing body:
combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, SHUFFLE_SEED)
```

---

### IN-03: `bench_full_protocol_with_networking` reuses pre-computed `total_bytes` inside the async closure — bytes are captured from setup, not from each iteration's fresh garble output

**File:** `benches/benchmarks.rs:141-176`

**Issue:** `total_bytes` (line 145) is computed once during setup from a single call to `garble_first_half` / `garble_second_half` on a shared generator, then captured by value into every async closure in `iter_batched` (line 165). Each iteration creates a fresh `AuthTensorGen` (via `setup_auth_gen`), but the simulated bandwidth (line 165: `network.send_size_with_metrics(total_bytes).await`) still uses the pre-captured constant. This is safe and intentional — the garbled output size is deterministic for fixed `(n, m, chunking_factor)` — but it means the benchmark silently stops reflecting the real size if the garbler output structure changes. The same pattern appears in `bench_4x4_runtime_with_networking` through `bench_256x256_runtime_with_networking`. A comment clarifying the determinism assumption would prevent future confusion.

**Fix:** Add a comment at the `total_bytes` capture site:
```rust
// total_bytes is deterministic for fixed (n, m, chunking_factor) — garble output
// sizes depend only on circuit structure, not on randomised keys. Captured once
// and reused across iterations for efficiency. Update if garbler output shape changes.
let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
```

---

_Reviewed: 2026-04-22_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
