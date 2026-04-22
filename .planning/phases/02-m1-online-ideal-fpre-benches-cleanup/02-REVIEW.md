---
phase: 02-m1-online-ideal-fpre-benches-cleanup
reviewed: 2026-04-21T00:00:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - src/auth_tensor_eval.rs
  - src/auth_tensor_fpre.rs
  - src/auth_tensor_gen.rs
  - src/auth_tensor_pre.rs
  - src/lib.rs
  - src/preprocessing.rs
  - benches/benchmarks.rs
findings:
  critical: 0
  warning: 4
  info: 4
  total: 8
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-04-21
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

This review covers the Phase 02 refactoring: moving `TensorFpreGen`/`TensorFpreEval`/`run_preprocessing` to `preprocessing.rs`, removing gamma fields from all output structs and online logic, renaming `generate_with_input_values` to `generate_for_ideal_trusted_dealer`, deduplicating benchmark chunking-factor loops, and adding documentation comments.

**Gamma removal is structurally complete** across all reviewed files (`auth_tensor_fpre.rs`, `auth_tensor_pre.rs`, `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `preprocessing.rs`). `combine_leaky_triples` correctly ignores `gen_gamma_shares`/`eval_gamma_shares` on the incoming `LeakyTriple` without referencing them. `TensorFpreGen` and `TensorFpreEval` in `preprocessing.rs` carry no gamma field.

The main issues found are: (1) a stale comment in `bench_preprocessing` still counts gamma bits in the throughput formula — a factual error that misleads performance analysis; (2) `bench_full_protocol_garbling` and `bench_full_protocol_with_networking` are defined but absent from `criterion_group!`, meaning they are dead code that never runs; (3) a `_clear_value` parameter in `eval_populate_seeds_mem_optimized` is silently unused; (4) several minor quality items.

---

## Warnings

### WR-01: bench_preprocessing throughput formula still accounts for gamma bits

**File:** `benches/benchmarks.rs:568-574`
**Issue:** The comment says `n + m + 2*n*m` = alpha_bits + beta_bits + correlated_bits + gamma_bits. But gamma was removed from `TensorFpreGen`/`TensorFpreEval` in this phase. The actual output is `n + m + n*m` authenticated bits (alpha + beta + correlated). The `n_auth_bits` value and `bcot_bytes` estimate are both inflated by an extra `n*m` — the value passed to `Throughput::Elements` is wrong by a factor approaching 2 for large `n*m`, directly corrupting throughput numbers reported by Criterion.

**Fix:**
```rust
// n alpha_bits + m beta_bits + n*m correlated_bits = n + m + n*m
let n_auth_bits = n + m + n * m;
// bCOT estimate: 2 rounds * (n + m + n*m) authenticated bits * 16 bytes
let bcot_bytes = 2 * (n + m + n * m) * block_sz;
```

---

### WR-02: bench_full_protocol_garbling and bench_full_protocol_with_networking are dead code — never registered in criterion_group!

**File:** `benches/benchmarks.rs:603-613` (criterion_group), `benches/benchmarks.rs:87` and `118` (definitions)
**Issue:** Both `bench_full_protocol_garbling` and `bench_full_protocol_with_networking` are defined but do not appear in the `criterion_group!` macro at line 603. These benchmarks never execute. `bench_full_protocol_garbling` in particular was added for the Phase 02 deduplication work (it uses the new loop over chunking factors), so its absence from the group means the garbling-only benchmark is invisible to Criterion.

This is a logic error: if the intent was to retire these benchmarks, they should be removed or `#[allow(dead_code)]`-annotated; if they are meant to run they must be added to `criterion_group!`.

**Fix (if benchmarks should run):**
```rust
criterion_group!(
    benches,
    bench_full_protocol_garbling,
    bench_full_protocol_with_networking,
    bench_4x4_runtime_with_networking,
    bench_8x8_runtime_with_networking,
    bench_16x16_runtime_with_networking,
    bench_32x32_runtime_with_networking,
    bench_64x64_runtime_with_networking,
    bench_128x128_runtime_with_networking,
    bench_256x256_runtime_with_networking,
    bench_preprocessing,
);
```
**Fix (if benchmarks are intentionally retired):** remove the two function definitions.

---

### WR-03: _clear_value parameter is unused in eval_populate_seeds_mem_optimized

**File:** `src/auth_tensor_eval.rs:66`
**Issue:** The parameter `_clear_value: &usize` is underscore-prefixed to suppress the unused warning, but it is never consumed anywhere inside the function body. `get_clear_value()` is called on the slice at the call site (line 196) and the result is passed in, but the function ignores it entirely. If the evaluator is not supposed to use the clear value (which is correct — the evaluator must not learn it), the parameter should be removed from the signature rather than silently suppressed. Leaving it in creates an API that implies the clear value is used, which is misleading and a potential security footgun.

**Fix:** Remove the parameter from the function signature and the corresponding argument at line 207:
```rust
fn eval_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    levels: Vec<(Block, Block)>,
    cipher: &FixedKeyAes,
) -> Vec<Block> {
```
And at the call site (line 207):
```rust
let eval_seeds = Self::eval_populate_seeds_mem_optimized(
    &slice.as_view(), chunk_levels[s].clone(), cipher
);
```

---

### WR-04: bucket_size_for panics on n*m == 0 or n*m == 1 due to division-by-zero / subtraction underflow

**File:** `src/auth_tensor_pre.rs:15-21`
**Issue:** `bucket_size_for` computes `log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize`. For `ell = n*m = 0`, `ell.leading_zeros()` returns `usize::BITS`, and the subtraction `usize::BITS - usize::BITS - 1` wraps (debug: panics; release: silently wraps to `usize::MAX`). For `ell = 1`, `leading_zeros() = usize::BITS - 1`, so `log2_ell = 0`, then `SSP / 0` panics with division-by-zero. These edge cases can be triggered by a caller passing `n=1, m=1` or `n=0`.

**Fix:** Add a guard at the top of the function:
```rust
pub fn bucket_size_for(n: usize, m: usize) -> usize {
    const SSP: usize = 40;
    let ell = n * m;
    assert!(ell >= 2, "bucket_size_for requires n*m >= 2, got n={} m={}", n, m);
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

---

## Info

### IN-01: Stale top-level TODO in auth_tensor_fpre.rs is untracked

**File:** `src/auth_tensor_fpre.rs:1`
**Issue:** `// TODO refactor authbit from fpre to a common module, or redefine with new name.` This has been present since before Phase 02 and is now more visible since this file is the sole home of the ideal-Fpre functionality. If there is no backlog item tracking this, it may be forgotten.
**Fix:** Either convert to a tracked issue or remove if not planned.

---

### IN-02: Debug println! statements left in integration test

**File:** `src/lib.rs:324-327, 365, 376`
**Issue:** `test_auth_tensor_product` emits `println!` and `print!` diagnostic output during `cargo test`. This is test-only code so it does not affect production, but it pollutes `cargo test --nocapture` output and is typically cleaned up before shipping a milestone.
**Fix:** Remove the six `println!`/`print!` statements at lines 324–327, 365, and 376.

---

### IN-03: benchmark setup functions for semi-honest protocol are dead code

**File:** `benches/benchmarks.rs:47-68`
**Issue:** `_setup_semihonest_gen` and `_setup_semihonest_eval` are underscore-prefixed (suppressing warnings) and are called by no benchmark in the file. These appear to be leftovers from an earlier iteration.
**Fix:** Remove `_setup_semihonest_gen` and `_setup_semihonest_eval`, and the corresponding `use` imports for `TensorProductGen`, `TensorProductEval`, and `SemiHonestTensorPre` (lines 11-13), to keep the bench file clean.

---

### IN-04: Documentation comment on generate_for_ideal_trusted_dealer does not document the return value

**File:** `src/auth_tensor_fpre.rs:88-91`
**Issue:** The doc comment says what the function does but does not document what `(usize, usize)` returns. The actual return is `(alpha, beta)` — the accumulated mask integers. Callers who read only the doc comment cannot determine this without reading the body.
**Fix:** Add a returns clause:
```rust
/// Returns `(alpha, beta)` — the accumulated mask bits in little-endian order
/// (alpha_i = bit i of alpha, beta_j = bit j of beta).
```

---

_Reviewed: 2026-04-21_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
