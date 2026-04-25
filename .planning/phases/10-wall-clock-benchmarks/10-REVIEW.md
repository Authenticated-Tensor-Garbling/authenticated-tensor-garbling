---
phase: 10-wall-clock-benchmarks
reviewed: 2026-04-24T00:00:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - benches/benchmarks.rs
  - src/lib.rs
  - src/auth_tensor_eval.rs
  - src/auth_tensor_gen.rs
  - src/tensor_ops.rs
findings:
  critical: 0
  warning: 1
  info: 4
  total: 5
status: issues_found
---

# Phase 10: Code Review Report

**Reviewed:** 2026-04-24
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Phase 10 added Protocol 1 / Protocol 2 online-phase wall-clock benchmarks (`bench_online_p1`, `bench_online_p2`) and the `setup_auth_pair` correlated-pair helper to `benches/benchmarks.rs`, and promoted `assemble_c_gamma_shares` / `assemble_c_gamma_shares_p2` to public crate-level functions in `src/lib.rs`. The three source-side files (`src/auth_tensor_eval.rs`, `src/auth_tensor_gen.rs`, `src/tensor_ops.rs`) required by the new benchmarks were reviewed at standard depth.

The online benchmarks are structurally correct: `setup_auth_pair` is present (lines 87-93) and is used in both `bench_online_p1` (line 205) and `bench_online_p2` (line 315), producing correlated `(AuthTensorGen, AuthTensorEval)` pairs. The sentinel-vector allocations (`l_alpha_pub`, `l_beta_pub`, `l_gamma_pub`) are placed before `let start = Instant::now()` in both benchmarks, so they are correctly excluded from timing. The P1 `check_zero` call uses `generator.delta_a` and the P2 call uses `evaluator.delta_b`, matching the respective MAC structures.

The one warning concerns the pre-existing networking benchmark (`bench_online_with_networking_for_size`): it constructs generator and evaluator from independent `TensorFpre` instances with different random seeds, meaning their delta values are uncorrelated. The evaluator decodes the garbler's ciphertexts using MACs authenticated under a different delta, silently producing garbage wire labels. The computation runs to completion (no panic, same instruction count), but the evaluation does not represent a correct protocol execution. For a pure latency benchmark this may be acceptable, but it should be documented.

The `src/auth_tensor_eval.rs`, `src/auth_tensor_gen.rs`, and `src/tensor_ops.rs` files are structurally clean. Four info-level items are noted: a `check_ok` black-box without a debug assertion, a doc inaccuracy on `final_computed`, a documentation gap on the networking benchmark's uncorrelated setup, and a missing guard against `chunking_factor = 0` in the loop-bound division.

---

## Warnings

### WR-01: Networking benchmark uses uncorrelated generator/evaluator pairs — evaluation produces garbage wire labels

**File:** `benches/benchmarks.rs:397-444`

**Issue:** `bench_online_with_networking_for_size` calls `setup_auth_gen(n, m, chunking_factor)` (lines 397 and 420) and `setup_auth_eval(n, m, chunking_factor)` (line 421) independently. Each call creates a `TensorFpre` instance with a different RNG seed (`seed=0` vs `seed=1`), so the two instances generate independent `delta_a` values. When `evaluator.evaluate_first_half` processes the generator's `chunk_levels` (built under `delta_a_seed0`), `eval_populate_seeds_mem_optimized` uses `evaluator.x_labels` — MACs authenticated under `delta_a_seed1`. The GGM tree reconstruction runs without panicking (same code paths, same instruction count) but produces incorrect leaf seeds. `evaluate_final` then XOR-combines with `correlated_auth_bit_shares.mac` from the seed-1 instance, accumulating further garbage. The timing numbers for garble + network delay are representative of real compute load (same operations are executed), but the evaluate side does not represent a valid protocol participant.

**Fix:** Document the intentional mismatch at the call site, or replace the setup with `setup_auth_pair` for fully representative timing. If the intent is network-latency-only measurement (not correctness), add a comment:

```rust
// NOTE: generator and evaluator are intentionally uncorrelated — each is
// constructed from an independent TensorFpre instance. The evaluator's
// wire-label decoding produces garbage because the MACs were authenticated
// under a different delta. This benchmark measures garble-time + network-
// transfer latency only; correctness of the evaluate output is not tested.
let mut generator = setup_auth_gen(n, m, chunking_factor);
// ... sizing run ...
```

And in `iter_batched`:

```rust
|| {
    // Uncorrelated setup: timing-only benchmark, not a correctness check.
    (
        setup_auth_gen(n, m, chunking_factor),
        setup_auth_eval(n, m, chunking_factor),
        SimpleNetworkSimulator::new(100.0, 0),
    )
},
```

---

## Info

### IN-01: `check_ok` in online benchmarks is consumed by `black_box` without a debug assertion

**File:** `benches/benchmarks.rs:246-252` (P1), `benches/benchmarks.rs:346-352` (P2)

**Issue:** The `check_ok: bool` result of `check_zero` is consumed by `black_box(check_ok)` to prevent dead-code elimination, which is correct benchmark practice. However, if the correlated setup in `setup_auth_pair` were ever broken or replaced with an uncorrelated helper, `check_zero` would silently return `false` on every iteration and the benchmark would measure a pipeline that always aborts — without any diagnostic output. A `debug_assert!` would catch this class of regression immediately in debug/test builds without adding overhead to release-mode benchmark runs.

**Fix:** Add a `debug_assert!` after `check_zero` and before `total += start.elapsed()`:

```rust
let check_ok = check_zero(&c_gamma, &generator.delta_a);
debug_assert!(check_ok, "honest P1 run must pass check_zero; \
    check that setup_auth_pair produces a correlated gen/eval pair");
total += start.elapsed();
let _ = black_box(check_ok);
```

Apply the same pattern in `bench_online_p2` using `&evaluator.delta_b`.

### IN-02: `final_computed` doc comment is incomplete — `garble_final_p2` / `evaluate_final_p2` also set it

**File:** `src/auth_tensor_gen.rs:54-56`, `src/auth_tensor_eval.rs:46-48`

**Issue:** The doc comment on `final_computed` states "Set to `true` by `garble_final()`" (gen) and "Set to `true` by `evaluate_final()`" (eval). Both `garble_final_p2` (line 434 in gen) and `evaluate_final_p2` (line 406 in eval) also set this flag to `true`. As a result, `compute_lambda_gamma` will not panic if called after `garble_final_p2` / `evaluate_final_p2`. In practice this is harmless because `garble_final_p2` writes `first_half_out` via the same D_gb path as `garble_final`, so the underlying data is correct. However, the doc comment misleads readers about which methods advance the flag.

**Fix:** Update the doc comment to enumerate all methods that advance the flag:

```rust
/// Set to `true` by `garble_final()` or `garble_final_p2()`. Guards
/// `compute_lambda_gamma()` against being called before the D_gb output
/// is fully accumulated in `first_half_out`.
final_computed: bool,
```

Apply the same update to `AuthTensorEval`:

```rust
/// Set to `true` by `evaluate_final()` or `evaluate_final_p2()`. Guards
/// `compute_lambda_gamma()` against being called before the D_gb output
/// is fully accumulated in `first_half_out`.
final_computed: bool,
```

### IN-03: `gen_populate_seeds_mem_optimized` and `eval_populate_seeds_mem_optimized` have no guard against empty input slice

**File:** `src/tensor_ops.rs:29`, `src/tensor_ops.rs:155`

**Issue:** Both `gen_populate_seeds_mem_optimized` (line 29: `x[n-1]`) and `eval_populate_seeds_mem_optimized` (line 155: `a_bits[n-1]`, `x[n-1]`) index `x[n-1]` unconditionally, where `n = x.len()`. If `n = 0` (an empty slice is passed), the access panics with an index-out-of-bounds error. In the current codebase this cannot be triggered through normal use: `chunking_factor >= 1` is enforced by all benchmark and test entry points, and `x.rows() > 0` is guaranteed by `n, m >= 1`. However, there is no compile-time or runtime assertion enforcing `n > 0` in either function, so a future caller passing an empty slice would get an opaque panic rather than a clear error message.

**Fix:** Add a `debug_assert!` at the entry of each function:

```rust
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &[Block],
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
    let n: usize = x.len();
    debug_assert!(n > 0, "gen_populate_seeds_mem_optimized: x must be non-empty");
    // ...
}
```

And in `eval_populate_seeds_mem_optimized`:

```rust
debug_assert!(n > 0, "eval_populate_seeds_mem_optimized: x must be non-empty");
debug_assert_eq!(a_bits.len(), n, "a_bits must have same length as MAC blocks");
```

### IN-04: Networking benchmark sizing run lacks a comment clarifying that garble outputs are not reused

**File:** `benches/benchmarks.rs:397-411`

**Issue:** Inside the outer `chunking_factor` loop, `bench_online_with_networking_for_size` creates a `generator`, runs `garble_first_half()` / `garble_second_half()` / `garble_final()`, and extracts ciphertext byte counts. The resulting `first_levels`, `first_cts`, `second_levels`, `second_cts` variables are used only for their `.len()` values and then dropped. A reader familiar with the P1 integration tests might expect these outputs to be fed to the evaluator in the timed loop — which would be wrong (the timed loop creates fresh pairs via `setup_auth_gen` / `setup_auth_eval` in `iter_batched`). A brief comment prevents this misreading.

**Fix:** Add a comment on the sizing-run block:

```rust
// Sizing run only — garble once to compute total ciphertext byte count for the
// network simulator. These outputs are NOT fed to the evaluator; `iter_batched`
// sets up a fresh (generator, evaluator) pair for each timed iteration below.
let mut generator = setup_auth_gen(n, m, chunking_factor);
let (first_levels, first_cts) = generator.garble_first_half();
let (second_levels, second_cts) = generator.garble_second_half();
generator.garble_final();
```

---

_Reviewed: 2026-04-24_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
