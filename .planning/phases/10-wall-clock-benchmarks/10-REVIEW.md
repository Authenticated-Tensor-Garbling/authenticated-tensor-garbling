---
phase: 10-wall-clock-benchmarks
reviewed: 2026-04-24T00:00:00Z
depth: standard
files_reviewed: 2
files_reviewed_list:
  - src/lib.rs
  - benches/benchmarks.rs
findings:
  critical: 1
  warning: 2
  info: 2
  total: 5
status: issues_found
---

# Phase 10: Code Review Report

**Reviewed:** 2026-04-24
**Depth:** standard
**Files Reviewed:** 2
**Status:** issues_found

## Summary

Phase 10 promoted two simulation helpers (`assemble_c_gamma_shares` and `assemble_c_gamma_shares_p2`) to public crate-level functions and added Protocol 1 / Protocol 2 online-phase benchmarks to `benches/benchmarks.rs`. The `src/lib.rs` changes are structurally clean: the `pub fn` promotions, crate-level `use` imports, and test-module `use super::` wiring are all correct. The `SIMULATION ONLY` doc comment is present and accurate.

The critical issue is in `benches/benchmarks.rs`: the two new online benchmarks (`bench_online_p1`, `bench_online_p2`) construct the generator and evaluator from **two independent, uncorrelated** `TensorFpre` instances (seeds 0 and 1 respectively). Both `assemble_c_gamma_shares` and `assemble_c_gamma_shares_p2` assert that `gamma_d_ev_shares.len() == n*m`, but `TensorFpre::into_gen_eval()` always produces an empty `gamma_d_ev_shares` (`vec![]`). The benchmarks will **panic** at the first iteration for every `(n, m)` pair. A secondary consequence is that even if the assert were removed, the generator and evaluator would not share correlated IT-MAC secrets, so `check_zero` would return `false` on every call — meaning the benchmarks would measure a pipelines that silently aborts rather than one that represents an honest run.

The networking benchmark (`bench_online_with_networking_for_size`) uses the same split-seed helpers but does not call `assemble_c_gamma_shares`, so it is not affected by the panic. Two info-level items are noted.

---

## Critical Issues

### CR-01: `bench_online_p1` / `bench_online_p2` panic at runtime — mismatched setup helpers produce empty `gamma_d_ev_shares`

**File:** `benches/benchmarks.rs:52-64` (helpers), `benches/benchmarks.rs:176-177` (P1 use site), `benches/benchmarks.rs:287-288` (P2 use site)

**Issue:** `setup_auth_gen` and `setup_auth_eval` each call `TensorFpre::new` with a different seed (0 vs 1), run `generate_for_ideal_trusted_dealer` independently, and call `into_gen_eval()` — which always leaves `gamma_d_ev_shares` (and all other `_d_ev_shares` fields) as empty `Vec`s. The benchmarks then pass the resulting `&generator` / `&evaluator` to `assemble_c_gamma_shares` (P1, line 210-217) and `assemble_c_gamma_shares_p2` (P2, line 311-318), both of which immediately assert:

```rust
assert_eq!(gb.gamma_d_ev_shares.len(), n * m);  // src/lib.rs:109
assert_eq!(ev.gamma_d_ev_shares.len(), n * m);  // src/lib.rs:113
```

These asserts fire for every `(n, m)` pair on the first benchmark iteration, producing a panic rather than a measurement. Furthermore, even if the asserts were suppressed, the two `TensorFpre` instances were seeded independently so the generator and evaluator do not share correlated MACs — `check_zero` would return `false` on every call (silently measuring an aborting pipeline).

The correct setup for the online benchmarks is the same pattern used by the integration tests: a single `TensorFpre` instance (or `IdealPreprocessingBackend`) that produces a matching `(fpre_gen, fpre_eval)` pair.

**Fix:** Replace `setup_auth_gen` / `setup_auth_eval` with a single paired-setup helper in the online benchmarks, mirroring `IdealPreprocessingBackend.run()`:

```rust
fn setup_auth_pair(n: usize, m: usize, chunking_factor: usize)
    -> (AuthTensorGen, AuthTensorEval)
{
    use authenticated_tensor_garbling::preprocessing::{
        IdealPreprocessingBackend, TensorPreprocessing,
    };
    let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, chunking_factor);
    (
        AuthTensorGen::new_from_fpre_gen(fpre_gen),
        AuthTensorEval::new_from_fpre_eval(fpre_eval),
    )
}
```

Then in `bench_online_p1` and `bench_online_p2`, replace the two separate setup calls:

```rust
// Before (panics):
let mut generator = setup_auth_gen(n, m, chunking_factor);
let mut evaluator = setup_auth_eval(n, m, chunking_factor);

// After (correlated pair):
let (mut generator, mut evaluator) = setup_auth_pair(n, m, chunking_factor);
```

Note: `setup_auth_gen` / `setup_auth_eval` can remain for `bench_online_with_networking_for_size`, which only measures the garble/evaluate pipeline and never calls `assemble_c_gamma_shares`.

---

## Warnings

### WR-01: `iter_custom` accumulates setup time when `black_box` calls are placed before `total += start.elapsed()`

**File:** `benches/benchmarks.rs:194-229` (P1), `benches/benchmarks.rs:300-329` (P2)

**Issue:** In both online benchmarks the `black_box` calls on `c_gamma`, `check_ok`, `&generator`, and `&evaluator` are placed *after* `total += start.elapsed()`, which is correct for excluding them from timing. However, `start = Instant::now()` is placed *after* the setup calls (`setup_auth_gen`, `setup_auth_eval`) but the `l_alpha_pub` / `l_beta_pub` / `l_gamma_pub` `vec!` allocations happen between `start` and the garble calls (lines 191-193 in P1, lines 298-299 in P2). These zero-vector allocations are trivially fast, but they are inside the timed region. More importantly, the `setup_auth_gen` / `setup_auth_eval` calls (which are the expensive part) are correctly outside the timed region, so once CR-01 is fixed by replacing them with `setup_auth_pair` the pre-allocated sentinel vectors should move outside `start` as well for clean timing.

**Fix:** Move the sentinel-vector allocations outside the timed region (before `let start = Instant::now()`):

```rust
// Before start:
let l_alpha_pub: Vec<bool> = vec![false; n];
let l_beta_pub:  Vec<bool> = vec![false; m];
// (P2: l_gamma_pub: Vec<bool> = vec![false; n * m])

let start = Instant::now();
// ... garble/evaluate calls ...
```

### WR-02: `bench_online_with_networking_for_size` holds stale garble state across the outer `chunking_factor` loop

**File:** `benches/benchmarks.rs:370-417`

**Issue:** At lines 370-379 a `generator` is set up outside the timed loop solely to measure byte counts. The generator is mutated by `garble_first_half()` / `garble_second_half()` / `garble_final()` — consuming the `final_computed` flag (line 57 of `auth_tensor_gen.rs`). This "sizing run" generator is then dropped, which is fine. However, `total_bytes` is computed from ciphertext sizes that depend on `chunking_factor`-specific tree structure, and the byte-size precomputation is repeated for **each** chunking factor inside the outer loop — a new `generator` is created per iteration. There is no bug in the counting logic itself, but `garble_first_half()` / `garble_second_half()` return `Vec<Vec<Block>>` values that are dropped immediately after size measurement (they are not reused in the timed loop). This is correct but the code structure suggests intent to reuse them, which would be wrong. A comment clarifying that `first_levels` / `first_cts` are used only for size measurement (not fed to any evaluator) would prevent future misuse.

**Fix:** Add a comment to the precomputation block:

```rust
// Sizing run only — compute total ciphertext byte count for the network simulator.
// These garble outputs are NOT reused in the timed benchmark loop; a fresh pair
// of (generator, evaluator) is set up per iteration inside iter_batched.
let mut generator = setup_auth_gen(n, m, chunking_factor);
let (first_levels, first_cts) = generator.garble_first_half();
// ...
```

---

## Info

### IN-01: Crate-level `use` imports in `src/lib.rs` are private (`use`, not `pub use`) but `Block` was already imported before Phase 10

**File:** `src/lib.rs:28-32`

**Issue:** The `use crate::block::Block` import at line 28 predates Phase 10 (it was already needed for `MAC_ZERO` / `MAC_ONE`). Phase 10 added lines 29-32. All five `use` items at crate root are private (`use`, not `pub use`), which is correct — the two `pub fn` helpers use these types in their signatures, but the types are already part of the public API through their own `pub mod` declarations (`pub mod auth_tensor_gen`, etc.). No re-export is needed. This is correct, but worth confirming explicitly: callers of `assemble_c_gamma_shares` must import `AuthTensorGen` / `AuthTensorEval` / `AuthBitShare` from their respective modules, not from the crate root.

**Fix:** No code change required. Optionally add a short comment noting these are module-level convenience imports for the two `pub fn` helpers, not re-exports:

```rust
// Imports used by the crate-root pub fn helpers (assemble_c_gamma_shares*).
// These types are re-exported from their respective pub mod declarations.
use crate::auth_tensor_gen::AuthTensorGen;
```

### IN-02: `check_ok` result is consumed by `black_box` but never inspected in the online benchmarks

**File:** `benches/benchmarks.rs:218-224` (P1), `benches/benchmarks.rs:319-325` (P2)

**Issue:** The `check_ok: bool` result of `check_zero` is consumed by `black_box(check_ok)` to prevent dead-code elimination, which is correct practice for benchmarks. However, after CR-01 is fixed (correlated pairs), an unexpected `false` return would silently produce a benchmark that measures an aborting pipeline without any signal to the user. A debug-assertion or a panic on `!check_ok` would make protocol failures immediately visible during development without affecting release-mode benchmark timing.

**Fix:** Add a `debug_assert!` after `check_zero` and before `total += start.elapsed()`:

```rust
let check_ok = check_zero(&c_gamma, &generator.delta_a);
debug_assert!(check_ok, "honest P1 run must pass check_zero; \
    setup may be using uncorrelated gen/eval pair");
total += start.elapsed();
let _ = black_box(check_ok);
```

---

_Reviewed: 2026-04-24_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
