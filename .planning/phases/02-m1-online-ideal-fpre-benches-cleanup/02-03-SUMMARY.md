---
phase: 02-m1-online-ideal-fpre-benches-cleanup
plan: 03
subsystem: benches
tags: [rust, benchmarks, criterion, dedup, import-fix, doc-comments]

# Dependency graph
requires:
  - plan: 02-02
    provides: preprocessing module populated + auth_tensor_fpre trimmed + generate method renamed; benches/benchmarks.rs left intentionally broken for Plan 03 to fix
provides:
  - benches/benchmarks.rs restored to green (`cargo bench --no-run` exit 0, `cargo build --benches` exit 0)
  - CLEAN-12 complete: bench_full_protocol_garbling and bench_full_protocol_with_networking each collapsed to a single `for cf in [1usize, 2, 4, 6, 8]` loop
  - Paper-protocol `//` header comments on all 10 `fn bench_*` definitions (D-18)
  - Import path `use authenticated_tensor_garbling::preprocessing::run_preprocessing`
  - Setup helpers (`setup_auth_gen` / `setup_auth_eval`) now call renamed `generate_for_ideal_trusted_dealer`
affects:
  - 02-04 (comment/doc audit — does not touch benches/; fully unblocked and independent of this plan)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Criterion Loop-Over-Parameters (Pattern 2 from RESEARCH): explicit `[1usize, 2, 4, 6, 8]` literal preserves BenchmarkId strings when `cf.to_string()` replaces hard-coded `"1"`/`"2"`/`"4"`/`"6"`/`"8"` literals in `BenchmarkId::new(...)`, keeping any Criterion baselines at `target/criterion/<group>/<factor>/` valid
    - Paper-protocol header comment per bench fn (D-18): single-line `//` (not `///` doc) identifying which paper construction and module path the benchmark measures

key-files:
  created: []
  modified:
    - benches/benchmarks.rs

key-decisions:
  - "Used Edit-based application for every task; no file-level Write. Each of the four edits was well-isolated (block-scoped substitutions) and grep-verifiable after the fact"
  - "Applied Task 3 dedup (bench_full_protocol_with_networking) rather than skipping: inspection of the five original blocks showed they differed ONLY in the chunking_factor literal value and minor whitespace (block 1 had a multi-line `|| (...)` layout, blocks 2-5 used single-line) — the semantic body (setup_auth_gen, total_bytes computation, async iter_batched closure) was verbatim-identical modulo the factor. RESEARCH Q4 recommendation therefore applies and was executed"
  - "Wrote Task 4 headers as single-line `//` comments (not `///` doc comments). `fn bench_*` definitions are free functions inside a bench crate — not public library API items — so `///` would be misleading; `//` matches D-18's example wording"
  - "Did NOT register bench_full_protocol_garbling or bench_full_protocol_with_networking in `criterion_group!`. The pre-refactor state already excluded them from the macro (only bench_Nx_N_runtime_with_networking and bench_preprocessing were registered). Re-registering is out of scope for CLEAN-12 — the dedup brief was scaffolding reduction, not behavior change"

requirements-completed: [CLEAN-12]

# Metrics
duration: ~15min
completed: 2026-04-21
---

# Phase 02 Plan 03: Bench dedup + import/rename fixes + paper-protocol header comments

**Restored `benches/benchmarks.rs` to green after Plan 02's API changes; collapsed ten near-identical chunking-factor blocks across two functions into two `for cf in [1usize, 2, 4, 6, 8]` loops preserving BenchmarkId byte-identity; added paper-protocol header comments to all 10 bench functions. Net diff: -274 / +86 lines.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-22T00:00:00Z (worktree session start)
- **Completed:** 2026-04-22T00:15:00Z
- **Tasks:** 4 (all `type="auto"`; all verifiable by grep + `cargo bench --no-run`)
- **Files modified:** 1 (`benches/benchmarks.rs`)

## Accomplishments

- **Bench crate restored to green.** Before Task 1, `cargo bench --no-run` produced three errors (`E0432 unresolved import auth_tensor_fpre::run_preprocessing`; `E0599 no method named generate_with_input_values` x 2). After Task 1, all three errors resolved; build is green at every commit boundary through Task 4.
- **CLEAN-12 complete.** `bench_full_protocol_garbling` (was 5 near-identical blocks at chunking_factors 1/2/4/6/8) and `bench_full_protocol_with_networking` (same, 5 blocks, plus wrapping async setup) each now contain exactly one inner loop over `[1usize, 2, 4, 6, 8]`. Net reduction: ten redundant blocks → two loops; -188 source lines net across the plan (insertions 86, deletions 274).
- **BenchmarkId byte-identity preserved.** Pre-refactor literals `BenchmarkId::new("1", ...)` / `BenchmarkId::new("2", ...)` / ... / `BenchmarkId::new("8", ...)` are now produced by `BenchmarkId::new(cf.to_string(), ...)` where `cf: usize` iterates over `[1, 2, 4, 6, 8]`. `cf.to_string()` yields byte-identical outputs `"1"`, `"2"`, `"4"`, `"6"`, `"8"`, so any pre-existing Criterion baselines stored under `target/criterion/full_protocol_garbling/{1,2,4,6,8}/` and `target/criterion/full_protocol_with_networking/{1,2,4,6,8}/` remain comparable. (See caveat in "Verification Evidence" about `criterion_group!` registration — the two functions are defined but not currently registered in the bench harness; this is the pre-existing state and is unchanged by this plan.)
- **Paper-protocol header comments added to all 10 bench functions (D-18).** Single-line `//` comments above each `fn bench_*` definition identify: which paper construction or code path is measured, which authenticated vs. preprocessing module is exercised, and the parameter sweep in effect. Preserved as non-doc `//` (not `///`) comments since bench functions are not public library items. No helper functions (`setup_auth_gen`, `setup_auth_eval`, `_setup_semihonest_*`) received headers — D-18 scope was explicitly bench functions only.
- **Import path modernized.** `use authenticated_tensor_garbling::auth_tensor_fpre::{TensorFpre, run_preprocessing}` → `auth_tensor_fpre::TensorFpre` + `preprocessing::run_preprocessing` (matches Plan 02-02's module migration).
- **Setup helpers call the renamed method.** `fpre.generate_with_input_values(...)` → `fpre.generate_for_ideal_trusted_dealer(...)` at both sites in `setup_auth_gen` and `setup_auth_eval`.
- **Baseline regression gate green.** `cargo test --lib --no-fail-fast` produces 4 FAILED lines — same four tests as `before.txt`, modulo the test-relocation carryover from Plan 02-02 (`test_run_preprocessing_mac_invariants` now under `preprocessing::tests` instead of `auth_tensor_fpre::tests`). Zero new regressions introduced by this plan.

## Task Commits

Each task committed atomically with `--no-verify` (parallel-executor convention):

1. **Task 1: Redirect imports and rename setup helper call sites** — `85d5e94` (refactor)
2. **Task 2: Dedup bench_full_protocol_garbling with [1,2,4,6,8] loop** — `288ad3c` (refactor)
3. **Task 3: Dedup bench_full_protocol_with_networking with [1,2,4,6,8] loop** — `b34e875` (refactor)
4. **Task 4: Add paper-protocol header comments to every fn bench_\*** — `cb18101` (docs)

## Files Modified

- `benches/benchmarks.rs` (+86 / -274 net vs. base `f231d20`)
  - Import block: swapped `auth_tensor_fpre::{TensorFpre, run_preprocessing}` for two separate imports (`auth_tensor_fpre::TensorFpre`, `preprocessing::run_preprocessing`).
  - `setup_auth_gen`, `setup_auth_eval`: two call sites, `generate_with_input_values` → `generate_for_ideal_trusted_dealer`.
  - `bench_full_protocol_garbling`: body shrunk from 5 hand-unrolled blocks to one `for cf in [1usize, 2, 4, 6, 8]` loop inside the existing `for &(n, m) in BENCHMARK_PARAMS` loop. `group.throughput` and `group.finish` ordering preserved. No Criterion tuning (warm_up_time, measurement_time, sample_size) was present in the original function and none was added.
  - `bench_full_protocol_with_networking`: body shrunk from 5 hand-unrolled blocks to one loop. The original `warm_up_time(10s)` and `measurement_time(30s)` at the function top remain intact. The multi-line `|| ( setup_auth_gen(...), setup_auth_eval(...), SimpleNetworkSimulator::new(100.0, 0) )` layout from block 1 was chosen as the canonical layout (over the compact single-line layout blocks 2-5 used) since it is more readable inside the tighter loop scope.
  - 10 single-line `//` header comments, one per `fn bench_*`.

## Decisions Made

- **Dedup applied to bench_full_protocol_with_networking (Task 3), not skipped.** The plan offered an escape hatch: "If analysis of the function body reveals that the five blocks are NOT identical modulo chunking factor ... DO NOT deduplicate." I inspected all five blocks (pre-edit lines 173-370) and confirmed they differ ONLY in (a) the hard-coded `chunking_factor` integer (1, 2, 4, 6, 8) and (b) inconsequential whitespace. All five blocks: re-run `setup_auth_gen` → compute `total_bytes` from four sub-sums → call `group.throughput(Throughput::Bytes(...))` → call `group.bench_with_input(BenchmarkId::new("<cf>", ...), ...)` with an async `iter_batched` closure over `SimpleNetworkSimulator::new(100.0, 0)` and the same `garble_first/second/final` + `evaluate_first/second/final` sequence. The dedup is semantically safe. RESEARCH Q4 recommended extending D-17 here; this plan executed that recommendation.
- **Canonical async-closure layout chosen from block 1.** Blocks 1 and 2-5 had two different whitespace layouts for the `iter_batched` argument list. I preserved the block-1 multi-line layout (one argument per line) in the unified loop for readability — functionally identical to the compact form.
- **Pre-existing `criterion_group!` registration NOT modified.** Before this plan, `bench_full_protocol_garbling` and `bench_full_protocol_with_networking` were already absent from the `criterion_group!` macro list (only the seven `bench_Nx_N_runtime_with_networking` functions and `bench_preprocessing` are registered). This plan did not add them. Doing so would change behavior (would cause `cargo bench` to execute them, producing new results under `target/criterion/full_protocol_garbling/` and `target/criterion/full_protocol_with_networking/`) — outside CLEAN-12's scaffolding-reduction scope. Flagged here for owner's awareness.

## Deviations from Plan

None requiring a rule flag. All four tasks executed as specified. One discovery (criterion_group registration gap) is a pre-existing condition, not caused by this plan and not in scope to fix.

## Idiosyncrasies Discovered

- **`bench_full_protocol_garbling` and `bench_full_protocol_with_networking` are defined but not registered in `criterion_group!`.** At both the pre-plan state (commit `f231d20`) and post-plan state (`cb18101`), the `criterion_group!` macro at the bottom of the file lists only: `bench_4x4_runtime_with_networking`, `bench_8x8_runtime_with_networking`, `bench_16x16_runtime_with_networking`, `bench_32x32_runtime_with_networking`, `bench_64x64_runtime_with_networking`, `bench_128x128_runtime_with_networking`, `bench_256x256_runtime_with_networking`, `bench_preprocessing`. Implication: running `cargo bench` does not execute the two now-deduplicated `bench_full_protocol_*` functions, and Criterion never writes baselines to `target/criterion/full_protocol_garbling/*` or `target/criterion/full_protocol_with_networking/*`. The BenchmarkId preservation guarantee in the plan is therefore vacuously true for these two functions at the baseline-comparison level, but structurally still intact (the strings `"1"`/`"2"`/`"4"`/`"6"`/`"8"` ARE still generated when the functions are invoked). If the user later re-registers these functions in `criterion_group!`, any legacy baselines on disk would remain comparable.
- **`Duration` import remains used (by `warm_up_time` / `measurement_time` in `bench_full_protocol_with_networking` and `bench_preprocessing`).** After Task 3 dedup, `Duration` is still needed; no unused-import warning was introduced.
- **Dead functions `_setup_semihonest_gen` / `_setup_semihonest_eval` still exist.** Underscored (leading `_`) so the compiler does not warn. Not in this plan's scope; ignored.

## Issues Encountered

None blocking. Pre-commit hooks bypassed per parallel-executor protocol (`--no-verify`). Read-before-edit reminders fired on each Edit call since the executor runtime tracks Read calls per agent session; the file had been read at session start, so every Edit was valid and the operations completed successfully — verified by grep and build after each one.

## User Setup Required

None.

## Verification Evidence

Plan-level verification checklist (from `<verification>` section):

| # | Check | Expected | Actual | Result |
|---|-------|----------|--------|--------|
| 1 | `cargo build --benches` | exit 0 | exit 0 (2 pre-existing bench warnings: 1 unused import in lib.rs test mod; 1 unused mod reference) | green |
| 2 | `cargo bench --no-run` | exit 0 | exit 0 | green |
| 3 | `grep -c "generate_with_input_values" benches/benchmarks.rs` | 0 | 0 | green |
| 4 | `grep -c "preprocessing::run_preprocessing" benches/benchmarks.rs` | 1 | 1 | green |
| 5 | `grep -c 'for cf in [1usize, 2, 4, 6, 8]' benches/benchmarks.rs` | 1 or 2 | 2 | green (Task 3 applied) |
| 6 | Every `^fn bench_*` has a preceding `//` header | matches | benches=10, headers=10 | green |
| 7 | `cargo test --lib --no-fail-fast` failure set matches `before.txt` | 4 pre-existing failures | 4 failures, same tests (modulo test-relocation from Plan 02-02) | green |

Task-level acceptance greps (representative):

```
# Task 1
$ grep -c "generate_with_input_values" benches/benchmarks.rs    # 0
$ grep -c "generate_for_ideal_trusted_dealer" benches/benchmarks.rs  # 2
$ grep -c "preprocessing::run_preprocessing" benches/benchmarks.rs   # 1
$ grep -c "auth_tensor_fpre::run_preprocessing\|auth_tensor_fpre::{TensorFpre, run_preprocessing}" benches/benchmarks.rs  # 0

# Task 2
$ grep -c "for cf in \[1usize, 2, 4, 6, 8\]" benches/benchmarks.rs   # 1 (after Task 2), 2 (after Task 3)
$ grep -cE 'BenchmarkId::new\("[1-8]",' benches/benchmarks.rs       # 10 -> 5 (after Task 2) -> 0 (after Task 3)
$ grep -c "sampling_mode\|warm_up_time\|measurement_time\|sample_size" benches/benchmarks.rs   # 9 (unchanged throughout)
$ grep -c "BENCHMARK_PARAMS" benches/benchmarks.rs                  # 4 (unchanged throughout)

# Task 4
$ BENCH_COUNT=$(grep -c "^fn bench_" benches/benchmarks.rs);
$ HEADER_COUNT=$(grep -B1 "^fn bench_" benches/benchmarks.rs | grep -cE "^// Benchmark")
$ echo $BENCH_COUNT $HEADER_COUNT   # 10 10
```

Header comment list (all 10):

```
bench_full_protocol_garbling:           // Benchmarks online garbling for the authenticated tensor gate (Pi_Garble, §4 / auth_tensor_gen): first half, second half, and final combine across BENCHMARK_PARAMS dimensions and chunking factors [1, 2, 4, 6, 8].
bench_full_protocol_with_networking:    // Benchmarks online garbling + evaluation for the authenticated tensor gate (auth_tensor_gen + auth_tensor_eval) with simulated network I/O between parties at 100 Mbps; sweeps chunking factors [1, 2, 4, 6, 8] across BENCHMARK_PARAMS.
bench_4x4_runtime_with_networking:      // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 4x4, sweeping chunking factors 1..=8 (auth_tensor_gen + auth_tensor_eval + SimpleNetworkSimulator).
bench_8x8_runtime_with_networking:      // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 8x8, sweeping chunking factors 1..=8.
bench_16x16_runtime_with_networking:    // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 16x16, sweeping chunking factors 1..=8.
bench_32x32_runtime_with_networking:    // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 32x32, sweeping chunking factors 1..=8.
bench_64x64_runtime_with_networking:    // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 64x64, sweeping chunking factors 1..=8.
bench_128x128_runtime_with_networking:  // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 128x128, sweeping chunking factors 1..=8.
bench_256x256_runtime_with_networking:  // Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 256x256, sweeping chunking factors 1..=8.
bench_preprocessing:                    // Benchmarks the uncompressed preprocessing pipeline (Pi_aTensor / Construction 3, Appendix F): ideal F_bCOT + leaky_tensor_pre + auth_tensor_pre producing TensorFpreGen / TensorFpreEval output, plus simulated bCOT network bandwidth accounting.
```

## Summary of Dedup Math

| Function | Blocks Before | Loops After | Lines Before (fn body) | Lines After (fn body) | Approx. Reduction |
|----------|---------------|-------------|------------------------|------------------------|--------------------|
| bench_full_protocol_garbling | 5 | 1 | ~76 | ~30 | -46 |
| bench_full_protocol_with_networking | 5 | 1 | ~210 | ~60 | -150 |
| **Total net diff vs base** | — | — | — | — | **+86 / -274 (net -188)** |

The +86 side includes: header comments (10 lines), loop scaffolding (outer `for cf in ...` + inner `let chunking_factor = cf;`), and import-block restructure.

## Next Plan Readiness

- **Plan 04 (comment/doc audit)** unblocked. It targets `src/auth_tensor_gen.rs` and `src/auth_tensor_eval.rs` — no overlap with `benches/benchmarks.rs`. Fully independent of this plan.
- **CLEAN-12 marked complete.** The plan's success criteria are met:
  - bench_full_protocol_garbling has a single for-loop
  - bench_full_protocol_with_networking has a single for-loop
  - Every bench function has a paper-protocol header comment
  - cargo bench --no-run green
  - Import paths match Plan 02's API
  - Setup helpers use the renamed method
  - Criterion BenchmarkIds "1"/"2"/"4"/"6"/"8" preserved

## Self-Check: PASSED

Verified at SUMMARY write time:

- All four task commits present in `git log --oneline`: `85d5e94`, `288ad3c`, `b34e875`, `cb18101`.
- `cargo build --benches` exits 0.
- `cargo bench --no-run` exits 0.
- `grep -c "generate_with_input_values" benches/benchmarks.rs` returns 0.
- `grep -c "preprocessing::run_preprocessing" benches/benchmarks.rs` returns 1.
- `grep -c "for cf in \[1usize, 2, 4, 6, 8\]" benches/benchmarks.rs` returns 2.
- `grep -c "^fn bench_" benches/benchmarks.rs` returns 10; all 10 have a preceding `// Benchmark...` header.
- `cargo test --lib --no-fail-fast` failure count = 4 (same as `before.txt`, modulo test-relocation from Plan 02-02).
- `git status --short` clean after Task 4 commit.

---
*Phase: 02-m1-online-ideal-fpre-benches-cleanup*
*Completed: 2026-04-21*
