---
phase: 02-m1-online-ideal-fpre-benches-cleanup
plan: 01
subsystem: infra
tags: [rust, module-refactor, cargo, baseline-snapshot]

# Dependency graph
requires:
  - phase: 01-uncompressed-preprocessing
    provides: existing src/lib.rs module graph (16 modules) and the TensorFpreGen/Eval layout in src/auth_tensor_fpre.rs that Plan 02 will migrate
provides:
  - Baseline test failure snapshot at .planning/phases/02-m1-online-ideal-fpre-benches-cleanup/before.txt (4 pre-existing red tests captured)
  - Empty src/preprocessing.rs module (doc-comment-only skeleton, compiles clean)
  - pub mod preprocessing; declaration in src/lib.rs grouped with preprocessing-pipeline modules
  - Documented confirmation that CLEAN-11 is trivially satisfied (src/auth_gen.rs and src/auth_eval.rs do not exist)
affects:
  - 02-02 (module migration + gamma cascade — unblocks moving TensorFpreGen/Eval + run_preprocessing into preprocessing.rs)
  - 02-03 (gamma removal — depends on struct layout settled in preprocessing.rs)
  - 02-04 (bench dedup — independent but shares the overall no-new-failures baseline)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Baseline-diff regression gate (cargo test failure set persisted to before.txt; later plans must produce identical set)
    - Empty-module skeleton (doc-comment-only *.rs file as a reserved namespace for later population)

key-files:
  created:
    - .planning/phases/02-m1-online-ideal-fpre-benches-cleanup/before.txt
    - src/preprocessing.rs
  modified:
    - src/lib.rs

key-decisions:
  - "Treat the 4 pre-existing test failures as known-red baseline via before.txt snapshot (per Research Pitfall 1 option A); no attempt to repair them in Phase 2"
  - "Place pub mod preprocessing; grouped with the preprocessing-pipeline modules (after pub mod auth_tensor_pre;) per D-05 and the pattern-map recommendation"
  - "preprocessing.rs contains only //! doc comments; no re-exports, no use statements, no stubs — minimal valid skeleton"
  - "CLEAN-11 discharged by presence-check, not source edit, since the legacy files do not exist"

patterns-established:
  - "Baseline regression gate: cargo test --lib --no-fail-fast | grep -E '^test [^ ]+ \\.\\.\\. FAILED$' | sort > before.txt — downstream plans diff against this to prove no new failures"
  - "Wave-0 module skeleton: introduce a doc-only Rust file + one lib.rs line before downstream plans migrate content, so imports resolve during incremental refactor"

requirements-completed: [CLEAN-08, CLEAN-11]

# Metrics
duration: 2min
completed: 2026-04-21
---

# Phase 02 Plan 01: Wave 0 Prerequisites Summary

**Baseline test-failure snapshot captured (4 pre-existing red), empty src/preprocessing.rs module created and wired into lib.rs, and CLEAN-11 confirmed trivially satisfied — unblocking Plan 02 to migrate TensorFpreGen/Eval + run_preprocessing into the new namespace.**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-04-21T23:33:06Z
- **Completed:** 2026-04-21T23:34:47Z
- **Tasks:** 3 (2 code/docs tasks + 1 pre-verified no-op)
- **Files modified:** 3 (1 created doc artefact, 1 created Rust module, 1 edited)

## Accomplishments

- Captured `before.txt` with the 4 known-red tests (`test_run_preprocessing_mac_invariants`, `test_combine_mac_invariants`, `test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`) so downstream Phase 2 plans can prove "no NEW failures introduced" via diff.
- Created `src/preprocessing.rs` as a compilable doc-only skeleton (no code, no imports) and added `pub mod preprocessing;` to `src/lib.rs` — Plan 02 can now `use crate::preprocessing::...` without module-resolution errors.
- Confirmed CLEAN-11 is trivially satisfied: both `src/auth_gen.rs` and `src/auth_eval.rs` are absent from the tree; no source edits were required.
- Verified `cargo build --lib`, `cargo check --lib --tests --benches`, and the test failure set all match the baseline — the skeleton introduction caused zero regressions.

## Task Commits

Each task was committed atomically:

1. **Task 1: Capture baseline test failure snapshot** - `bdd6001` (docs)
2. **Task 2: Create empty preprocessing module and wire it into lib.rs** - `1b979b2` (feat)
3. **Task 3: Document that CLEAN-11 is trivially satisfied** - no commit (no-op: legacy files already absent, per task description)

## Files Created/Modified

- `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/before.txt` — baseline list of 4 pre-existing failing tests (sorted, line-by-line)
- `src/preprocessing.rs` — new doc-comment-only module; future home for `TensorFpreGen`, `TensorFpreEval`, `run_preprocessing`
- `src/lib.rs` — added single line `pub mod preprocessing;` grouped with preprocessing-pipeline modules (after `pub mod auth_tensor_pre;`)

## Decisions Made

- **Baseline grep pattern tightened.** The plan's suggested command `grep "^test .* FAILED"` also matches libtest's per-file summary line (`test result: FAILED. 48 passed; 4 failed; ...`). Used the more precise regex `^test [^ ]+ \.\.\. FAILED$` so that `before.txt` contains exactly the 4 expected failing-test name lines and nothing else. Acceptance criteria (sorted, contains the 4 test names, non-empty) all satisfied. This is a faithful refinement of the intent, not a deviation from plan goals.
- **No code in `src/preprocessing.rs`.** Populated only with the plan-specified 7-line `//!` docstring block. Rust accepts a file containing only outer doc comments as an empty module — no `pub fn`, no `pub struct`, no `use` statements, no re-exports (D-04 forbids re-exports from `auth_tensor_fpre`; this plan avoids creating them in `preprocessing.rs` too).
- **Task 3 had no commit.** The task description explicitly states "No file modifications are needed in this task" and the acceptance criteria are pure presence/absence checks. With nothing to commit, a separate commit would be empty. CLEAN-11 discharge is documented here in the SUMMARY frontmatter (`requirements-completed`) and above.

## Deviations from Plan

None - plan executed exactly as written. The grep-pattern refinement in Task 1 is a tightening that preserves the plan's intent (the expected file contents and all acceptance criteria match exactly); it is not a scope/behavior deviation.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Plan 02 (module migration + gamma cascade) is unblocked.** It can now `use crate::preprocessing::{TensorFpreGen, TensorFpreEval, run_preprocessing};` in callers once Plan 02 moves those items into `preprocessing.rs`, and `into_gen_eval` in `auth_tensor_fpre.rs` can return types defined in `preprocessing.rs` (cross-module return types are idiomatic Rust).
- **Baseline regression gate is armed.** Downstream plans must verify: `cargo test --lib --no-fail-fast 2>&1 | grep -E "^test [^ ]+ \.\.\. FAILED$" | sort` produces output identical to `before.txt`. Any additional FAILED lines signal a Phase-2-introduced regression to be investigated before plan completion.
- **Known red carried forward:** The 4 pre-existing MAC-invariant failures (in `leaky_tensor_pre`, `auth_tensor_pre`, `auth_tensor_fpre::run_preprocessing`) remain intentionally unfixed — per Research Open Question Q1 resolution, they are out of Phase 2 scope and slated for a later phase that rebuilds the real-protocol pipeline.

## Self-Check: PASSED

Verified at SUMMARY write time:

- `before.txt` exists, 4 lines, contains all 4 expected test name substrings, sorted
- `src/preprocessing.rs` exists, only `//!` doc lines, zero `pub fn`/`pub struct`/`use` lines
- `src/lib.rs` contains exactly one `pub mod preprocessing;` line
- `src/auth_gen.rs` and `src/auth_eval.rs` both absent
- `cargo check --lib --tests --benches` exits 0
- `cargo build --lib` exits 0 (only pre-existing warnings)
- `cargo test --lib --no-fail-fast` failure set is byte-identical to `before.txt` (`diff` returns 0)
- Task 1 commit `bdd6001` present in `git log`
- Task 2 commit `1b979b2` present in `git log`

---
*Phase: 02-m1-online-ideal-fpre-benches-cleanup*
*Completed: 2026-04-21*
