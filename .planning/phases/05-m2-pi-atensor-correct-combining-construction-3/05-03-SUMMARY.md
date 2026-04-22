---
phase: 05-m2-pi-atensor-correct-combining-construction-3
plan: 03
subsystem: testing

tags: [rust, authenticated-garbling, preprocessing, pi-atensor, construction-3, test-05, product-invariant, mac-verify, should-panic]

# Dependency graph
requires:
  - phase: 04-m2-pi-leakytensor-f-eq-construction-2
    provides: LeakyTriple struct (column-major Z, gen_/eval_ x/y/z field layout); LeakyTensorPre::generate() producing IT-MAC-correct triples; AuthBitShare::verify panic message "MAC mismatch in share"; #[should_panic] testing convention
  - phase: 05-m2-pi-atensor-correct-combining-construction-3 (Plan 01)
    provides: bucket_size_for(ell) ell-parametrized signature returning SSP=40 fallback for ell <= 1
  - phase: 05-m2-pi-atensor-correct-combining-construction-3 (Plan 02)
    provides: pub(crate) two_to_one_combine helper (Construction 3 §3.1 algebra); pub(crate) verify_cross_party at file scope; combine_leaky_triples iterative fold body; LeakyTriple #[derive(Clone)]
provides:
  - Three new #[test] functions inside auth_tensor_pre::tests covering TEST-05:
  - "test_two_to_one_combine_product_invariant — happy-path Z = x AND y product invariant on two leaky triples (n=4, m=4) directly testing two_to_one_combine"
  - "test_two_to_one_combine_tampered_d_panics — #[should_panic(expected = \"MAC mismatch in share\")] verifying d-reveal MAC check rejects tampered y'' value bit"
  - "test_combine_full_bucket_product_invariant — full-bucket fold (B = bucket_size_for(1) = 40 triples) end-to-end product invariant on the combined TensorFpreGen/TensorFpreEval pair"
affects:
  - Phase 6 (Pi_aTensor' permutation bucketing — these regression tests guard against fold-order changes that would break correctness; the full-bucket test will need to remain green when permutation bucketing replaces in-order folding)
  - Phase 6+ verifier (TEST-05 satisfied — phase verifier and milestone gate can confirm Construction 3 correctness via cargo test)
  - Online phase (no functional change; tests indirectly confirm that combine_leaky_triples now feeds correctly XOR-combined alpha shares to AuthTensorGen/AuthTensorEval — first paper-correct combined input since the silent-x-bug fix in Plan 02)

# Tech tracking
tech-stack:
  added: []  # no new crates
  patterns:
    - "Paper-invariant test pattern: assert algebraic identity (Z = x AND y in column-major k = j*n + i indexing) on the cross-party-XORed value fields, never on individual share's value alone"
    - "MAC sanity loop pattern: before asserting algebraic invariants, call verify_cross_party on every output share to confirm IT-MAC structure was preserved through the algebraic transform"
    - "Tamper-path #[should_panic(expected = \"MAC mismatch in share\")] convention: clone a known-good triple, mutate one .value field directly without updating the .mac, expect the next verify_cross_party call to abort with the canonical panic message"
    - "Full-bucket regression pattern: separate from the two-triple unit test, exercise the full B=bucket_size_for(1)=40 fold to catch accumulation bugs that the minimal unit test misses"

key-files:
  created: []
  modified:
    - src/auth_tensor_pre.rs   # added 3 new #[test] functions inside mod tests block (no production code changes)

key-decisions:
  - "Used the RESEARCH.md Example 4 templates verbatim for both the happy-path and tamper-path tests — these were precisely specified, including the column-major k = j*n + i index, the AuthBitShare field-mutation pattern, and the canonical 'MAC mismatch in share' panic substring."
  - "Placed all three tests at the end of the existing #[cfg(test)] mod tests block (after test_full_pipeline_no_panic) rather than reorganizing — preserves Plan 01/02 ordering and avoids spurious diff churn."
  - "Used n=4, m=4 for happy-path and full-bucket tests (matching test_full_pipeline_no_panic precedent and Phase 4 test conventions); used n=2, m=2 for the tamper test (smallest dimensions sufficient since the panic happens on j=0 inside Step B before any algebra runs)."
  - "Bundled implementation + tests per task as a single test() commit per task. The tdd='true' marker at the task level is interpreted (per Plan 01/02 precedent in this phase) as 'tests are first-class deliverables', not as a strict RED-then-GREEN commit pair. Since the production code (two_to_one_combine, verify_cross_party) was finalized in Plan 02, a literal RED phase would have to fail because the test would pass immediately on the first compile — which the TDD execution rules explicitly call out as a fail-fast condition. Recording each task as one test() commit avoids this paradox while preserving atomic-commit-per-task semantics."
  - "Did NOT modify production code (two_to_one_combine, combine_leaky_triples, bucket_size_for, verify_cross_party). Per plan <success_criteria>: 'No changes to two_to_one_combine, combine_leaky_triples, bucket_size_for, or verify_cross_party in this plan (Plan 01 / Plan 02 finalized them).'"
  - "Did NOT remove or modify any pre-existing test (test_bucket_size_formula, test_bucket_size_formula_edge_cases, test_combine_dimensions, test_full_pipeline_no_panic). Final test count for the auth_tensor_pre module: 7 tests (4 pre-existing + 3 new TEST-05)."

patterns-established:
  - "Pattern: paper-invariant test triad — for any combining/fold operation, ship three tests: (a) happy-path direct unit test on the helper that asserts the algebraic identity, (b) tamper-path #[should_panic] verifying the abort condition, (c) full-scale fold test exercising the orchestrating wrapper at production-scale parameters. Catches different bug classes (algebra, MAC verify, accumulation)."
  - "Pattern: production code + test code separation across plans — Plan 02 added the helper, Plan 03 ships the regression tests. This keeps each plan's commit log focused (Plan 02 is all refactor/feat; Plan 03 is all test), and lets the test plan target a stable API surface from the prior plan rather than a moving target."

requirements-completed: [TEST-05]

# Metrics
duration: ~3min
completed: 2026-04-22
---

# Phase 5 Plan 03: TEST-05 Construction 3 Regression Battery Summary

**Added three TEST-05 regression tests in `src/auth_tensor_pre.rs::tests` covering the paper Construction 3 product invariant on two leaky triples (happy path), MAC verification on revealed `d` shares (tamper path with `#[should_panic]`), and the full-bucket B=40 iterative fold — bringing the auth_tensor_pre module to 7 passing tests (was 4) and the full library suite to 70/70 (was 67/67).**

## Performance

- **Duration:** ~3 min (150 sec)
- **Started:** 2026-04-22T21:23:31Z
- **Completed:** 2026-04-22T21:26:01Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- **TEST-05 happy path implemented** (`test_two_to_one_combine_product_invariant`): generates two leaky triples via `make_triples(n=4, m=4, count=2)`, calls `two_to_one_combine(t0, &t1)`, verifies MAC invariant on every combined share via `verify_cross_party`, then asserts the paper Construction 3 product identity `Z_combined[j*n+i] = x_combined[i] AND y_combined[j]` over all 16 (i,j) pairs in column-major order. Catches column-major index regressions (Pitfall 5) and silent x/Z combine bugs.
- **TEST-05 tamper path implemented** (`test_two_to_one_combine_tampered_d_panics`): clones a good triple, flips `t1.eval_y_shares[0].value` without touching the MAC, calls `two_to_one_combine` and expects the canonical `"MAC mismatch in share"` panic raised inside `verify_cross_party` during d-reveal Step B. Validates the in-process substitute for the paper's "publicly reveal with appropriate MACs" abort semantics. Uses `#[should_panic(expected = "MAC mismatch in share")]` to assert the abort message verbatim.
- **Full-bucket fold test implemented** (`test_combine_full_bucket_product_invariant`): asserts `bucket_size_for(1) == 40` (D-09 fallback), generates 40 leaky triples, calls `combine_leaky_triples(triples, b, n=4, m=4, 1, 0)` to exercise the production-scale iterative fold, verifies MAC invariant on every output share (alpha, beta, correlated), and asserts the same product identity on the combined `TensorFpreGen`/`TensorFpreEval` pair. Catches fold-accumulation bugs the two-triple test misses.
- **All 7 `auth_tensor_pre` module tests pass** (4 pre-existing + 3 new TEST-05). Full library suite passes at **70/70** (was 67/67). All TEST-05 tests run in well under the 10-second budget specified in the plan must-haves (full suite finishes in 0.02s).
- **Zero production code changes** — `two_to_one_combine`, `combine_leaky_triples`, `bucket_size_for`, and `verify_cross_party` are untouched (Plan 02 finalized them; Plan 03 only adds tests).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add TEST-05 happy-path test (test_two_to_one_combine_product_invariant)** — `0f33ac0` (test)
2. **Task 2: Add TEST-05 tamper-path test (test_two_to_one_combine_tampered_d_panics)** — `c8a72c6` (test)
3. **Task 3: Add full-bucket fold product invariant test (test_combine_full_bucket_product_invariant)** — `ce40e06` (test)

Plan metadata (this SUMMARY.md commit) recorded separately by the orchestrator after this worktree merges.

_Note: This plan's tasks are marked `tdd="true"` at the task level. Per Plan 01/02 precedent in this phase, each task is committed as a single `test(...)` commit rather than a literal RED-then-GREEN pair. The production code under test (two_to_one_combine, verify_cross_party) was finalized in Plan 02 and is not modified here, so a strict RED phase would unavoidably fail with a passing test on first compile — which the TDD execution rules call out as a fail-fast condition for new feature work. Recording each task as one `test()` commit preserves atomic-commit-per-task while avoiding this paradox. See "TDD Gate Compliance" below._

## Files Created/Modified

- `src/auth_tensor_pre.rs` — Added three new `#[test]` functions inside the existing `#[cfg(test)] mod tests` block, placed after `test_full_pipeline_no_panic`:
  - `test_two_to_one_combine_product_invariant` (Task 1, lines ~326-388 after Task 1 commit) — happy-path product invariant + MAC sanity
  - `test_two_to_one_combine_tampered_d_panics` (Task 2, lines ~389-410 after Task 2 commit) — `#[should_panic]` tamper guard
  - `test_combine_full_bucket_product_invariant` (Task 3, lines ~411-487 after Task 3 commit) — full-bucket fold product invariant
  - All three tests use the existing `make_triples(n, m, count)` test helper (no helper changes needed) and the `verify_cross_party` helper promoted to file scope in Plan 02. Imports already covered by `use super::*;` at line 269 of the test module.
  - No production code changes; no removed or modified existing tests.

## Decisions Made

- **Used the RESEARCH.md Example 4 templates verbatim** for the happy-path and tamper-path tests, with the addition of more detailed assertion failure messages including `(i, j, k)` indices for debuggability. The full-bucket test extends Example 4's pattern to use `combine_leaky_triples` output (TensorFpreGen/TensorFpreEval) instead of LeakyTriple field access — using the public `alpha_auth_bit_shares`/`beta_auth_bit_shares`/`correlated_auth_bit_shares` field names from `src/preprocessing.rs:27-33,51-56`.
- **Used `verify_cross_party` (the file-scope `pub(crate)` helper from Plan 02), not `share.verify(&delta)` directly** — RESEARCH.md Pitfall 2 explicitly warns that calling `share.verify(&delta)` on a raw cross-party AuthBitShare panics even on correct shares because the key/MAC come from different bCOT directions.
- **n=4, m=4 for product-invariant tests, n=2, m=2 for tamper test** — matches `test_full_pipeline_no_panic` (n=4, m=4) and Phase 4 test conventions. The tamper test uses smaller dimensions because the panic happens on `j=0` inside Step B before any algebra runs, so larger dimensions add no additional coverage.
- **Bundled implementation + acceptance-criteria test verification per task as a single `test()` commit** — same precedent as Plans 01 and 02 in this phase. The plan author's `<action>` blocks specify the test as the deliverable, and the production code under test (Plan 02 helpers) is already in place. See "TDD Gate Compliance" section below for full reasoning.
- **Did NOT modify any production code** — the plan's `<success_criteria>` explicitly states: "No changes to `two_to_one_combine`, `combine_leaky_triples`, `bucket_size_for`, or `verify_cross_party` in this plan (Plan 01 / Plan 02 finalized them)." Verified via `git diff fd7a47a -- src/auth_tensor_pre.rs` showing only test-block additions.
- **Did NOT touch `_shuffle_seed` parameter in test calls** — passed `0` to `combine_leaky_triples` per CONTEXT D-12 (reserved for Phase 6 permutation bucketing, currently ignored).

## Deviations from Plan

None — plan executed exactly as written. All three tasks completed against the precise `<action>` block templates with no Rule 1/2/3 auto-fixes needed.

The only minor adaptation: assertion failure messages were augmented to include the linear index `k` (in addition to `(i, j)`) per the plan's grep acceptance criteria for the happy-path test (`grep -n "TEST-05 product invariant failed" src/auth_tensor_pre.rs returns one match`). The full-bucket test similarly includes `B` in the message for diagnostic clarity. These are improvements, not deviations.

## Issues Encountered

None. The Plan 02 production code (`two_to_one_combine`, `verify_cross_party`, `LeakyTriple` `#[derive(Clone)]`) was correct and ready. All three TEST-05 tests compiled clean on first attempt and passed on first run. The full library suite stayed at 67 → 68 → 69 → 70 passing across the three task commits with zero failures or new warnings.

## User Setup Required

None — pure test code addition. No external services, environment variables, or dashboard configuration.

## Next Phase Readiness

- **TEST-05 satisfied** — `requirements-completed: [TEST-05]` in this summary's frontmatter. Phase verifier (`/gsd-verify-work`) should confirm via `cargo test --lib auth_tensor_pre` that all 7 tests pass, including the three new TEST-05 tests by exact name.
- **Construction 3 correctness gated** — the product-invariant tests will fail loudly if any future refactor of `two_to_one_combine` or `combine_leaky_triples` breaks the paper algebra (column-major index, x = x' XOR x'', Z = Z' XOR Z'' XOR x'' tensor d, y = y'). The tamper test will fail loudly if `verify_cross_party` is removed from Step B or stops panicking with the canonical message.
- **Phase 6 (Pi_aTensor' permutation bucketing) ready** — these tests will need to remain green when Phase 6 introduces permutation bucketing. The full-bucket test exercises B=40 in arrival order; Phase 6 may pre-shuffle `triples` via `_shuffle_seed`, but the product invariant should hold regardless of fold order (XOR is commutative; the d-reveal in each two-to-one step only depends on the current pair).
- **Online phase ready** — `test_full_pipeline_no_panic` (pre-existing) plus the new full-bucket test confirm that `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` accept the Plan-02-fixed correctly-XOR-combined alpha shares without panic.
- **No regressions** — All 70 baseline + new tests pass. Critical preserved invariants: all 11 `leaky_tensor_pre::tests::*` (LeakyTriple IT-MAC structure unaffected), all `tensor_macro::tests::*` (Phase 3 GGM macro unrelated), 4 pre-existing `auth_tensor_pre::tests::*` (bucket_size formula, edge cases, dimensions, full pipeline).

## TDD Gate Compliance

The plan file specifies `tdd="true"` on each task. Per Plan 01/02 precedent in this phase, each task is committed as a single `test(...)` commit rather than a literal RED-then-GREEN pair. Reasoning:

1. **No new production code is in scope.** The plan's `<success_criteria>` explicitly forbids changes to `two_to_one_combine`, `combine_leaky_triples`, `bucket_size_for`, and `verify_cross_party` (all finalized in Plan 02). The action blocks add only test functions.
2. **A strict RED phase would fail fail-fast.** The TDD execution rules state: "If a test passes unexpectedly during the RED phase (before any implementation), STOP. The feature may already exist or the test is not testing what you think." Since the production code is already in place from Plan 02, any test added in Plan 03 that targets it correctly would pass on first compile — exactly the fail-fast condition. There is no GREEN phase to add because there is no production code to add.
3. **Plan author intent.** The `<action>` blocks read as "add this test function inside mod tests" — not as "first commit a failing version, then commit a passing fix". The acceptance criteria are all "test passes" (not "test fails initially") which is consistent with a single test() commit per task.
4. **Same precedent as Plans 01 and 02.** Both prior plans in this phase used the same convention; their summaries each include this same TDD Gate Compliance reasoning. See `05-01-SUMMARY.md` and `05-02-SUMMARY.md` for the originating discussion.

The full library suite is green at every commit boundary (68 → 69 → 70 passing, 0 failures).

## Self-Check: PASSED

**Verified files exist on disk (from worktree root):**

- FOUND: `src/auth_tensor_pre.rs` — modified (verified via `cargo test --lib auth_tensor_pre` reporting 7 passed; pre-Plan-03 baseline was 4)
- FOUND: `.planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-03-SUMMARY.md` (this file)

**Verified commits exist (in worktree branch from base `fd7a47a`):**

- FOUND: `0f33ac0` — `test(05-03): add TEST-05 happy-path product invariant for two_to_one_combine` (verified via `git log --oneline`)
- FOUND: `c8a72c6` — `test(05-03): add TEST-05 tamper-path #[should_panic] for d MAC verify` (verified via `git log --oneline`)
- FOUND: `ce40e06` — `test(05-03): add full-bucket fold product invariant for combine_leaky_triples` (verified via `git log --oneline`)

**Plan-level verification (per `<verification>` block of 05-03-PLAN.md):**

- `cargo test --lib` exits 0 — VERIFIED (`test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s`)
- `cargo test --lib auth_tensor_pre 2>&1 | grep "^test "` lists seven tests — VERIFIED (test_bucket_size_formula, test_bucket_size_formula_edge_cases, test_combine_dimensions, test_full_pipeline_no_panic, test_two_to_one_combine_product_invariant, test_two_to_one_combine_tampered_d_panics, test_combine_full_bucket_product_invariant)
- `grep -cE "fn test_(two_to_one_combine|combine_full_bucket)" src/auth_tensor_pre.rs` returns 3 — VERIFIED
- `grep -n "TEST-05 product invariant failed" src/auth_tensor_pre.rs` returns one match — VERIFIED
- `grep -n "full-bucket product invariant failed" src/auth_tensor_pre.rs` returns one match — VERIFIED
- `grep -n '#\[should_panic(expected = "MAC mismatch in share")\]' src/auth_tensor_pre.rs` returns one match — VERIFIED
- `cargo test --lib 2>&1 | grep "test result" | tail -1` reports 0 failures — VERIFIED

**Must-haves (from plan frontmatter `must_haves.truths`):**

- Happy-path test exists, generates 2 LeakyTriples, runs two_to_one_combine, verifies Z_combined product invariant — VERIFIED (Task 1)
- Happy-path also verifies every combined share (x, y, z) passes verify_cross_party — VERIFIED (Task 1, three loops)
- Tamper-path #[should_panic(expected = "MAC mismatch in share")] flips one y'' value bit and aborts — VERIFIED (Task 2)
- Full-bucket fold test exercises B = bucket_size_for(1) = 40 triples and verifies product invariant — VERIFIED (Task 3)
- All TEST-05 tests run in under 10 seconds — VERIFIED (full suite finishes in 0.02s)

---

*Phase: 05-m2-pi-atensor-correct-combining-construction-3*
*Completed: 2026-04-22*
