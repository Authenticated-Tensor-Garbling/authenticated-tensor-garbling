---
phase: 10-wall-clock-benchmarks
plan: 04
subsystem: testing
tags: [criterion, benchmarks, ideal-preprocessing, online-phase, gap-closure]

# Dependency graph
requires:
  - phase: 10-wall-clock-benchmarks
    provides: bench_online_p1, bench_online_p2 functions (plans 10-01..10-03 added the bench bodies and routing)
provides:
  - "setup_auth_pair(n, m, chunking_factor) helper that builds a CORRELATED (AuthTensorGen, AuthTensorEval) pair via IdealPreprocessingBackend.run, populating gamma_d_ev_shares (length n*m) on both sides"
  - "bench_online_p1 and bench_online_p2 wired to setup_auth_pair so assemble_c_gamma_shares / assemble_c_gamma_shares_p2 do not panic on the first iteration"
  - "Closure of BENCH-02 / BENCH-04 / BENCH-06 partial-satisfaction gap from 10-VERIFICATION.md (the online benches now produce wall-clock measurements)"
affects: [phase-10 verifier, future bench result harvesting]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Use IdealPreprocessingBackend.run(...) (not TensorFpre::into_gen_eval()) when a benchmark needs a CORRELATED gen/eval pair with all four D_ev field pairs (alpha/beta/correlated/gamma) populated on both sides."
    - "Keep narrow single-side helpers (setup_auth_gen / setup_auth_eval) for benches that only need one side or do not require gamma_d_ev_shares populated."

key-files:
  created: []
  modified:
    - "benches/benchmarks.rs - added IdealPreprocessingBackend / TensorPreprocessing import; added setup_auth_pair helper; rewired bench_online_p1 and bench_online_p2 per-iteration setup to use setup_auth_pair"

key-decisions:
  - "Use IdealPreprocessingBackend.run over modifying TensorFpre::into_gen_eval — keeps production code untouched and reuses an already-tested correlated-pair generator (preprocessing.rs::test_ideal_backend_gamma_d_ev_shares_length)."
  - "Preserve setup_auth_gen / setup_auth_eval — bench_online_with_networking_for_size still uses them and does not need a correlated pair."
  - "Single grouped use authenticated_tensor_garbling::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing} import — TensorPreprocessing trait must be in scope to call .run()."

patterns-established:
  - "Pattern: per-bench-iteration setup chooses helper by gamma_d_ev_shares requirement — assemble_c_gamma_shares* asserts force CORRELATED setup_auth_pair; non-asserting paths can use the cheaper setup_auth_gen / setup_auth_eval."

requirements-completed: [BENCH-01, BENCH-02, BENCH-04, BENCH-06]

# Metrics
duration: 7min
completed: 2026-04-25
---

# Phase 10 Plan 04: Bench gamma_d_ev_shares panic gap-closure Summary

**Added `setup_auth_pair` helper using `IdealPreprocessingBackend.run` and rewired `bench_online_p1` / `bench_online_p2` to it; the online wall-clock benchmarks now produce measurements (~2.2 µs/op at 4x4/cf=1) instead of panicking on the first iteration.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-04-25T06:27:48Z (worktree base reset)
- **Completed:** 2026-04-25T06:34:22Z
- **Tasks:** 1
- **Files modified:** 1 (benches/benchmarks.rs)

## Accomplishments

- Added `setup_auth_pair(n, m, chunking_factor) -> (AuthTensorGen, AuthTensorEval)` (benches/benchmarks.rs:87-93) that uses `IdealPreprocessingBackend.run(n, m, 1, chunking_factor)` to produce a CORRELATED gen/eval pair with `gamma_d_ev_shares` (and the other three D_ev field pairs) populated on both sides.
- Replaced `bench_online_p1`'s per-iteration two-line `setup_auth_gen + setup_auth_eval` (was lines 176-177) with a single `setup_auth_pair` call (now line 205).
- Replaced `bench_online_p2`'s equivalent setup (was lines 287-288) with a single `setup_auth_pair` call (now line 315).
- Kept `setup_auth_gen` and `setup_auth_eval` defined; `bench_online_with_networking_for_size` continues to use them (no correlated pair required there).
- Verified the original `assertion 'left == right' failed: gb.gamma_d_ev_shares.len() == n * m` panic is gone — both `online/p1_garble_eval_check_4x4/1` and `online/p2_garble_eval_check_4x4/1` now produce Criterion `time:` / `thrpt:` measurement lines.

## Task Commits

1. **Task 1: Add setup_auth_pair helper and rewire bench_online_p1 / bench_online_p2 to use it** — `7a62c75` (fix)

## Files Created/Modified

- `benches/benchmarks.rs` — Added grouped use of `IdealPreprocessingBackend` + `TensorPreprocessing` trait (line 20). Added `setup_auth_pair` function with full doc comment explaining why it differs from the existing single-side helpers (lines 67-93). Rewired the two online-bench setup sites (P1 line 205, P2 line 315). Net diff: +31/-4.

## Decisions Made

- **Use `IdealPreprocessingBackend.run` over modifying `TensorFpre::into_gen_eval()`.** The plan's explicit constraint was no `src/` changes, and the ideal backend already produces a correlated `(TensorFpreGen, TensorFpreEval)` pair (verified by `preprocessing::tests::test_ideal_backend_gamma_d_ev_shares_length` and `test_ideal_backend_d_ev_shares_lengths`). This is the smallest correct fix.
- **Preserve `setup_auth_gen` / `setup_auth_eval`.** They are still called inside `bench_online_with_networking_for_size` (line 397 pre-loop; line 423 in the `iter_batched` setup closure), which never reaches `assemble_c_gamma_shares*` and therefore does not need `gamma_d_ev_shares` populated. Removing them would break the network bench group.
- **Use a single grouped `use authenticated_tensor_garbling::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};` line.** The `.run(...)` invocation is a trait method, so `TensorPreprocessing` must be in scope. Adding both items in one new top-level `use` line keeps the diff minimal.

## Deviations from Plan

None — plan executed exactly as written. The single-task action was applied per the four numbered steps (import, helper, P1 rewire, P2 rewire). The plan-listed acceptance criterion `grep -F "AuthTensorGen::new_from_fpre_gen(fpre_gen)" benches/benchmarks.rs` now matches **two** lines rather than the plan's "exactly one" — but this is a benign plan-spec inaccuracy: the same call already existed in `setup_auth_gen` (line 57) before this plan started, and the new occurrence in `setup_auth_pair` is the intended addition. The functional intent ("setup_auth_pair contains the call") is satisfied.

## Issues Encountered

- **Pre-existing downstream panic in network bench (out of scope).** After the targeted online bench completes, criterion still executes the function body of `bench_online_with_networking_for_size` (which prints "Total bytes for input size NxM..." unconditionally inside its outer for-loop). When n=64 is reached, `setup_auth_gen` calls `generate_for_ideal_trusted_dealer`, which asserts `n <= usize::BITS - 1` and panics with `n=64 exceeds usize bit width minus 1`. This panic is **not** caused by Plan 10-04 — before our fix it was shadowed by the gamma_d_ev_shares panic firing earlier in the bench order. Plan 10-04 explicitly forbids modifying `bench_online_with_networking_for_size`, so this is logged in `.planning/phases/10-wall-clock-benchmarks/deferred-items.md` for a future plan.

## Verification Evidence

### `git diff --stat` (HEAD~1..HEAD)
```
 benches/benchmarks.rs | 35 +++++++++++++++++++++++++++++++----
 1 file changed, 31 insertions(+), 4 deletions(-)
```
Only `benches/benchmarks.rs` modified; no `src/` change. The diff is slightly larger than the plan's "~10-12 inserted, ~4 deleted" estimate because the plan-supplied 27-line doc comment block on `setup_auth_pair` was included verbatim.

### Compilation (BENCH-06 gate)
```
$ cargo bench --no-run
    Finished `bench` profile [optimized] target(s) in 0.07s
  Executable benches/benchmarks.rs (target/release/deps/benchmarks-ffc7e45cfff9e4b2)
EXIT=0
```

### Smoke bench — Protocol 1 (`online/p1_garble_eval_check_4x4/1`, --warm-up 1 --measurement 2 --sample-size 10)
```
Benchmarking online/p1_garble_eval_check_4x4/1
Benchmarking online/p1_garble_eval_check_4x4/1: Warming up for 3.0000 s
[online][p1] 4x4 cf=1: 0.005 ms/op, 299.50 ns/AND
... (28 samples, converging to ~0.002 ms/op, ~135 ns/AND)
Benchmarking online/p1_garble_eval_check_4x4/1: Collecting 10 samples in estimated 20.000 s (1.7M iterations)
Benchmarking online/p1_garble_eval_check_4x4/1: Analyzing
online/p1_garble_eval_check_4x4/1
                        time:   [2.1921 µs 2.1977 µs 2.2027 µs]
                        thrpt:  [7.2638 Melem/s 7.2803 Melem/s 7.2988 Melem/s]
```
**No `gamma_d_ev_shares` panic.** First measurement produced; criterion's `time:` / `thrpt:` line printed.

### Smoke bench — Protocol 2 (`online/p2_garble_eval_check_4x4/1`, same flags)
```
Benchmarking online/p2_garble_eval_check_4x4/1
Benchmarking online/p2_garble_eval_check_4x4/1: Warming up for 3.0000 s
[online][p2] 4x4 cf=1: 0.008 ms/op, 528.62 ns/AND
... (samples converge to ~0.002 ms/op, ~145 ns/AND)
Benchmarking online/p2_garble_eval_check_4x4/1: Collecting 10 samples in estimated 20.000 s (1.7M iterations)
Benchmarking online/p2_garble_eval_check_4x4/1: Analyzing
online/p2_garble_eval_check_4x4/1
                        time:   [2.3360 µs 2.4143 µs 2.5171 µs]
                        thrpt:  [6.3566 Melem/s 6.6273 Melem/s 6.8493 Melem/s]
```
**No `gamma_d_ev_shares` panic.** First measurement produced; criterion's `time:` / `thrpt:` line printed.

### Regression gate
```
$ cargo test --lib --tests
test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```
105/105 — no regressions; production code untouched.

### Surgical-edit gate
```
$ git diff --name-only HEAD~1 HEAD
benches/benchmarks.rs
```
No `src/` file modified.

## 10-VERIFICATION.md Gap-Closure Map

| 10-VERIFICATION.md "truth"                                                                                            | Status before 10-04 | Status after 10-04 |
| --------------------------------------------------------------------------------------------------------------------- | ------------------- | ------------------ |
| `cargo bench --no-run` exits 0 in release mode                                                                        | PASS                | PASS               |
| `cargo bench` runs the online P1 4x4/cf=1 case to completion without panic                                            | FAIL (gamma panic)  | PASS               |
| `cargo bench` runs the online P2 4x4/cf=1 case to completion without panic                                            | FAIL (gamma panic)  | PASS               |
| `cargo test --lib --tests` reports 105 passed                                                                         | PASS (105)          | PASS (105)         |

## Self-Check: PASSED

- File `benches/benchmarks.rs` exists and contains the four required artifacts (import, `setup_auth_pair` definition, `IdealPreprocessingBackend.run` call, two `setup_auth_pair` invocations).
- Commit `7a62c75` is present in `git log --oneline` for the worktree branch.

## Next Phase Readiness

- Phase 10 verification gap (`bench_online_p1` / `bench_online_p2` runtime panic) is closed; the verifier should now mark BENCH-02 / BENCH-04 / BENCH-06 fully satisfied.
- Pre-existing n=64 panic in `bench_online_with_networking_for_size` documented in `.planning/phases/10-wall-clock-benchmarks/deferred-items.md` for follow-up.

---
*Phase: 10-wall-clock-benchmarks*
*Plan: 04*
*Completed: 2026-04-25*
