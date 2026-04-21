---
phase: 02-m1-online-ideal-fpre-benches-cleanup
plan: 02
subsystem: preprocessing
tags: [rust, module-refactor, gamma-cascade-removal, rename, per-field-docs]

# Dependency graph
requires:
  - plan: 02-01
    provides: empty src/preprocessing.rs skeleton + pub mod preprocessing; in lib.rs + baseline before.txt
provides:
  - src/preprocessing.rs populated with TensorFpreGen, TensorFpreEval (both gamma-less with per-field /// docs), run_preprocessing, and the four test_run_preprocessing_* tests
  - src/auth_tensor_fpre.rs trimmed to ideal-dealer only (TensorFpre + impl + renamed generate_for_ideal_trusted_dealer)
  - Gamma cascade discharged across TensorFpre, TensorFpreGen, TensorFpreEval, AuthTensorGen, AuthTensorEval, and combine_leaky_triples
  - CLEAN-07 (rename) and CLEAN-08 (module migration) complete; CLEAN-09 (per-field /// docs) complete; CLEAN-10 partial (_gamma_share dead code removed; comment/doc audit deferred to Plan 04)
affects:
  - 02-03 (bench dedup + bench import update — now unblocked; cargo build --benches currently fails by design because benches still reference auth_tensor_fpre::run_preprocessing and the old generate method name)
  - 02-04 (comment/doc audit — now unblocked; touches the same files but different lines)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Cross-module return type (Pattern S1): TensorFpre::into_gen_eval returns (preprocessing::TensorFpreGen, preprocessing::TensorFpreEval) while TensorFpre lives in auth_tensor_fpre — idiomatic Rust, no re-exports needed
    - Per-field `///` doc comments on public struct fields (Pattern S2), applied to both TensorFpreGen and TensorFpreEval
    - Column-major (j*n+i) indexing on correlated shares — annotated explicitly in the `///` doc for `correlated_auth_bit_shares`

key-files:
  created: []
  modified:
    - src/preprocessing.rs
    - src/auth_tensor_fpre.rs
    - src/auth_tensor_pre.rs
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs
    - src/lib.rs

key-decisions:
  - "Executed the file-level rewrite of src/auth_tensor_fpre.rs via a single Write (not 7 scattered Edits) because the changes span 1-443 with multiple interacting substitutions; the resulting diff is a small, reviewable delta (-157/+15) and matches the plan's EDIT 1-7 intent byte-for-byte"
  - "Moved four test_run_preprocessing_* tests to preprocessing.rs (plan spec). The gamma loop inside test_run_preprocessing_mac_invariants was deleted per D-11; the other three tests had no gamma references to remove"
  - "Replaced `assert_eq!(eval_out.gamma_auth_bit_shares.len(), n * m);` in auth_tensor_pre::tests::test_combine_dimensions with the equivalent `assert_eq!(eval_out.correlated_auth_bit_shares.len(), n * m);` to preserve the test's dimension-check intent after gamma removal, rather than silently dropping the assertion"
  - "Left bench file untouched — the plan explicitly states Plan 03 owns bench import updates; cargo build --benches fails by design"

requirements-completed: [CLEAN-07, CLEAN-08, CLEAN-09]

# Metrics
duration: ~5min
completed: 2026-04-21
---

# Phase 02 Plan 02: Module migration + gamma cascade removal + ideal-dealer rename

**TensorFpreGen/Eval/run_preprocessing physically migrated from auth_tensor_fpre.rs into src/preprocessing.rs; TensorFpre::generate_with_input_values renamed to generate_for_ideal_trusted_dealer; gamma_* field/generation/propagation removed end-to-end across 5 files and 6 struct-literal/function-body sites; per-field `///` documentation added; baseline test-failure set preserved (4 pre-existing failures, zero new regressions).**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-04-21T23:38:35Z
- **Completed:** 2026-04-21T23:43:17Z
- **Tasks:** 5 (all `type="auto"`, all verifiable by grep + build)
- **Files modified:** 6

## Accomplishments

- **CLEAN-07 (rename) complete.** `TensorFpre::generate_for_ideal_trusted_dealer` now carries the D-06 three-line doc comment; the old name is gone from `src/` (benches handled by Plan 03).
- **CLEAN-08 (module migration) complete.** `TensorFpreGen`, `TensorFpreEval`, and `run_preprocessing` live in `src/preprocessing.rs`; all four in-crate consumers import them from `crate::preprocessing`.
- **CLEAN-09 (per-field `///` docs) complete.** Each of the 9 fields in `TensorFpreGen` and 9 in `TensorFpreEval` has a `///` comment specifying party ownership (garbler vs. evaluator), semantic meaning, and (for column-major fields) indexing layout. Total of 23 matched lines satisfies the plan's ≥ 18 bar.
- **CLEAN-10 partial.** `_gamma_share` dead-code let-binding removed from `AuthTensorGen::garble_final`. Remainder of CLEAN-10 (`// awful return type` comment, `garble_final`/`evaluate_final` docstrings, GGM tweak comment) deferred to Plan 04 as specified.
- **Gamma cascade discharged end-to-end.** Removed `gamma_auth_bits` (TensorFpre), `gamma_auth_bit_shares` (TensorFpreGen, TensorFpreEval, AuthTensorGen, AuthTensorEval), gamma generation loop in `generate_for_ideal_trusted_dealer`, `combined_gen_gamma`/`combined_eval_gamma` locals and their XOR loop in `combine_leaky_triples`, and the corresponding struct-literal initialisers in `into_gen_eval` and `combine_leaky_triples`. `LeakyTriple.gen_gamma_shares`/`eval_gamma_shares` in `src/leaky_tensor_pre.rs` remain untouched per the cascade boundary (confirmed by `git diff --stat` showing no changes to that file).
- **Baseline regression gate green.** `cargo test --lib --no-fail-fast` produces exactly 4 FAILED lines — same count and same tests as `before.txt`. The only textual difference is that `test_run_preprocessing_mac_invariants` is reported under its new module path `preprocessing::tests` (it moved to preprocessing.rs in Task 1) instead of the old `auth_tensor_fpre::tests`. Zero new regressions.
- **Integration test `tests::test_auth_tensor_product` passes** on the renamed call site.

## Task Commits

Each task was committed atomically with `--no-verify` (parallel-executor convention):

1. **Task 1: Populate src/preprocessing.rs with structs, run_preprocessing, and tests** — `ecd51a1` (feat)
2. **Task 2: Trim auth_tensor_fpre.rs to ideal-dealer only; rename generate method; remove gamma** — `7ec1f0d` (refactor)
3. **Task 3: Rewire auth_tensor_pre.rs (forced gamma cascade + import redirect)** — `42c319b` (refactor)
4. **Task 4: Remove gamma_auth_bit_shares from AuthTensorGen and redirect TensorFpreGen import** — `7e718ad` (refactor)
5. **Task 5: Remove gamma from AuthTensorEval; redirect import; update lib.rs integration test** — `2caf2c9` (refactor)

## Files Created/Modified

- `src/preprocessing.rs` — populated with two public structs (9 `///`-documented fields each), `run_preprocessing` (verbatim body + preserved docblock), and a `#[cfg(test)] mod tests` with four test functions. No re-exports, no `pub use`.
- `src/auth_tensor_fpre.rs` — shrunk to `TensorFpre` + `impl` (gamma field gone; renamed method; struct defs and `run_preprocessing` removed). Imports now pull `TensorFpreGen`/`TensorFpreEval` from `crate::preprocessing` for the `into_gen_eval` return type.
- `src/auth_tensor_pre.rs` — `TensorFpreGen`/`TensorFpreEval` import redirected to `crate::preprocessing`; `combined_gen_gamma`/`combined_eval_gamma` and their XOR loop removed; gamma initialisers dropped from both struct literals; gamma mention removed from docstring; test dimension assertion re-targeted to `correlated_auth_bit_shares`.
- `src/auth_tensor_gen.rs` — import redirected; `gamma_auth_bit_shares` field + both constructor initialisers removed; dead `_gamma_share` let-binding removed from `garble_final`; test call site renamed and gamma assertions removed.
- `src/auth_tensor_eval.rs` — import redirected; `gamma_auth_bit_shares` field + both constructor initialisers removed.
- `src/lib.rs` — integration test `tests::test_auth_tensor_product` call site renamed to `generate_for_ideal_trusted_dealer`.

## Decisions Made

- **File-level rewrite for auth_tensor_fpre.rs (Task 2).** The plan lists 7 edits spanning lines 1-443 with overlapping deletions (struct removals, import swap, method rename, gamma loop removal inside the renamed method, struct-literal field removal, test block deletion, call-site renames). The cleanest atomic way to apply these without brittle intermediate parse states was a single `Write` producing the final target. The resulting git diff shows -157/+15 lines, which is exactly what the plan would produce if all 7 edits were applied sequentially. All Task 2 acceptance grep checks pass.
- **Edit-based application for the other four tasks.** Tasks 3, 4, and 5 involve smaller, well-isolated changes (single-line swaps, single struct-field removals, single let-binding deletions) and were applied via `Edit` calls, each verified by grep immediately after.
- **Gamma dimension assertion in auth_tensor_pre tests.** The line `assert_eq!(eval_out.gamma_auth_bit_shares.len(), n * m);` in `test_combine_dimensions` was part of the test's dimensional coverage. Rather than silently drop the assertion, I re-pointed it to `correlated_auth_bit_shares` (the non-gamma equivalent already being asserted on `gen_out`) so the test continues to verify eval-side dimensional integrity. This is a test-preservation refinement consistent with the plan's intent (the plan says "DELETE" that assertion, but preserving dimensional coverage is clearly a better outcome; the eval correlated assertion was already absent from the original test). Classifiable as Rule 2 (auto-add missing critical testing coverage) — see Deviations.

## Deviations from Plan

### Auto-added coverage

**1. [Rule 2 — Missing test coverage] Retargeted gamma length assertion in `test_combine_dimensions`**
- **Found during:** Task 3
- **Issue:** The plan says `DELETE` the line `assert_eq!(eval_out.gamma_auth_bit_shares.len(), n * m);`. Simply deleting would leave `test_combine_dimensions` with no eval-side length check (the other two existing assertions cover `gen_out.alpha_auth_bit_shares.len()` and `gen_out.correlated_auth_bit_shares.len()`, both gen-side).
- **Fix:** Replaced the line with the equivalent non-gamma assertion `assert_eq!(eval_out.correlated_auth_bit_shares.len(), n * m);` to preserve eval-side dimensional coverage.
- **Files modified:** `src/auth_tensor_pre.rs`
- **Commit:** `42c319b` (Task 3)
- **Impact:** Test continues to pass; coverage of eval-side output dimensions is maintained after gamma removal.

## Issues Encountered

None blocking. Pre-commit hooks were bypassed per parallel-executor protocol (`--no-verify`) to avoid worktree contention. `PreToolUse:Edit` read-before-edit reminders fired on several files; since the files had already been read earlier in the same session, the Edits proceeded and completed successfully — verified by grep after each step.

## User Setup Required

None.

## Verification Evidence

Plan-level verification checklist (from `<verification>` section):

| # | Check | Result |
|---|-------|--------|
| 1 | `cargo build --lib --tests` exit 0 | ✓ green (pre-existing warnings only) |
| 2 | `cargo test --lib` failure set matches `before.txt` (modulo test-relocation) | ✓ 4 failures, same tests; only difference is `test_run_preprocessing_mac_invariants` now reported under its new module path `preprocessing::tests` (moved by Task 1) |
| 3 | `gamma_auth_bit*` absent outside `leaky_tensor_pre.rs` | ✓ `grep -rn "gamma_auth_bit\|gamma_auth_bits" src/ --include="*.rs"` returns zero matches |
| 4 | `generate_with_input_values` absent in `src/` and `benches/` | ⚠ absent from `src/` (0 matches); still present at `benches/benchmarks.rs:73,80` — **expected per plan**, Plan 03 handles |
| 5 | `pub mod preprocessing;` in `src/lib.rs` | ✓ count = 1 |
| 6 | Two struct defs in `src/preprocessing.rs` | ✓ count = 2 |
| 7 | No struct/run_preprocessing in `src/auth_tensor_fpre.rs` | ✓ count = 0 |
| 8 | `src/leaky_tensor_pre.rs` untouched | ✓ `git diff --stat` shows no changes |

Baseline regression gate (authoritative per-plan green gate): **PASSED.** 4 pre-existing failures preserved, 0 new failures introduced.

Post-task bench build (Plan 03 concern): `cargo build --benches` fails with `no method named generate_with_input_values found for struct TensorFpre` and `unresolved import auth_tensor_fpre::run_preprocessing` — **this is the expected state** per the plan's verification notes: "Plan 03 (bench dedup + import update) fixes these."

## Next Plan Readiness

- **Plan 03 (bench dedup + import update)** is unblocked. It must update `benches/benchmarks.rs`:
  - Swap `use ...::auth_tensor_fpre::run_preprocessing;` → `use ...::preprocessing::run_preprocessing;`
  - Rename both `fpre.generate_with_input_values(...)` call sites to `generate_for_ideal_trusted_dealer(...)`.
  Once those two edits land, `cargo build --benches` goes green.
- **Plan 04 (comment/doc audit)** is unblocked. Deferred items from this plan that Plan 04 owns:
  - Remove `// awful return type` comment from `AuthTensorGen::gen_chunked_half_outer_product` (D-13)
  - Add `///` docstring to `AuthTensorGen::garble_final` (D-14)
  - Add `///` docstring to `AuthTensorEval::evaluate_final` (D-14)
  - Add GGM tweak-direction comment to `AuthTensorEval::eval_populate_seeds_mem_optimized` (D-15)
- **Pitfall 3 (gamma field-use collision) confirmed mitigated.** All five gamma-touching files were edited in one atomic wave-2 plan, so no intermediate broken-build state leaked into a merged commit. `cargo build --lib --tests` is green at every commit boundary from Task 2 onward except for the expected transient duplicate-name state between Tasks 1 and 2 (both were committed in order without an intermediate compile gate, consistent with the plan's "full compilation is deferred to Task 2" note).
- **Pattern S1 (cross-module return type) demonstrated.** `TensorFpre::into_gen_eval` in `auth_tensor_fpre.rs` constructs and returns `(preprocessing::TensorFpreGen, preprocessing::TensorFpreEval)` via direct import — no re-export, no wrapper, idiomatic Rust.

## Self-Check: PASSED

Verified at SUMMARY write time:

- All five task commits present in `git log --oneline`: `ecd51a1`, `7ec1f0d`, `42c319b`, `7e718ad`, `2caf2c9`.
- Plan-wide grep sweep: `gamma_auth_bit` matches = 0 in `src/` (outside `leaky_tensor_pre.rs`).
- `cargo build --lib --tests` exits 0.
- `cargo test --lib --no-fail-fast` failure count = 4 (same as `before.txt`).
- `src/preprocessing.rs` contains both structs, `run_preprocessing`, and `test_run_preprocessing_mac_invariants` (grep checks pass).
- `src/auth_tensor_fpre.rs` contains `generate_for_ideal_trusted_dealer` (1), contains no `gamma` references (0), contains no old struct defs or `run_preprocessing` (0 each).
- `git diff --stat` on `src/leaky_tensor_pre.rs` shows no changes.
- `src/lib.rs` integration test uses the renamed method (`generate_for_ideal_trusted_dealer` count = 1, old name count = 0).

---
*Phase: 02-m1-online-ideal-fpre-benches-cleanup*
*Completed: 2026-04-21*
