---
phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
fixed_at: 2026-04-22T00:00:00Z
review_path: .planning/phases/06-m2-pi-atensor-permutation-bucketing-construction-4-benches/06-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-04-22
**Source review:** .planning/phases/06-m2-pi-atensor-permutation-bucketing-construction-4-benches/06-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### WR-01: Seed `shuffle_seed ^ j` collapses to `j` when `shuffle_seed == 0`

**Files modified:** `src/auth_tensor_pre.rs`
**Commit:** 49d73fd
**Applied fix:** Replaced `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)` with
`ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j as u64))` on line 209 of
`combine_leaky_triples`. Also updated the doc comment on the `shuffle_seed` parameter
and the inline comment in the permutation loop to explain that `wrapping_add` is used
instead of XOR to avoid seed collapse when `shuffle_seed = 0`.

### WR-02: `total_leaky` formula is accidentally correct but breaks if `count != 1` restriction is lifted

**Files modified:** `src/preprocessing.rs`
**Commit:** a27806f
**Applied fix:** Expanded the `assert_eq!(count, 1, ...)` message in `run_preprocessing`
to explicitly document that the assertion is load-bearing: `total_leaky = bucket_size * count`
generates enough leaky triples for `count` output authenticated triples, but
`combine_leaky_triples` below only consumes `bucket_size` of them and returns one pair.
The message now warns that removing this assert requires adding a loop over outputs,
not just relaxing the guard.

### WR-03: `bench_full_protocol_garbling` and `bench_full_protocol_with_networking` are defined but never registered

**Files modified:** `benches/benchmarks.rs`
**Commit:** ea5687e
**Applied fix:** Deleted both dead benchmark functions (`bench_full_protocol_garbling`,
`bench_full_protocol_with_networking`) and their exclusive helper functions
(`_setup_semihonest_gen`, `_setup_semihonest_eval`). Removed the three now-unused
imports (`TensorProductGen`, `TensorProductEval`, `SemiHonestTensorPre`). The active
per-dimension networking benchmarks and `bench_preprocessing` in `criterion_group!`
are untouched.

---

_Fixed: 2026-04-22_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
