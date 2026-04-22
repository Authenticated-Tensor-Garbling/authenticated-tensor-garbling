---
phase: 05-m2-pi-atensor-correct-combining-construction-3
plan: 02
subsystem: crypto-protocol

tags: [rust, authenticated-garbling, preprocessing, pi-atensor, construction-3, two-to-one-combine, mac-verify]

# Dependency graph
requires:
  - phase: 04-m2-pi-leakytensor-f-eq-construction-2
    provides: LeakyTriple struct (column-major Z, gen_/eval_ x/y/z field layout); AuthBitShare with Default + Add overloads + verify; Delta::as_block; IdealBCot single-shared-delta convention
  - phase: 05-m2-pi-atensor-correct-combining-construction-3 (Plan 01)
    provides: bucket_size_for(ell) ell-parametrized signature with edge-case guard; cleared the path for combine_leaky_triples body rewrite without signature drift
provides:
  - pub(crate) fn two_to_one_combine(prime, &dprime) -> LeakyTriple — paper Construction 3 §3.1 lines 415-444 algebra in one helper
  - pub(crate) fn verify_cross_party at file scope (promoted from #[cfg(test)] mod tests) for in-process MAC verification of revealed d shares
  - rewritten combine_leaky_triples body as a thin iterative fold: acc = triples[0].clone(); for next in triples.iter().skip(1) { acc = two_to_one_combine(acc, next) }
  - silent x-bug fix — output (TensorFpreGen, TensorFpreEval) now takes alpha_auth_bit_shares from the XOR-combined acc.gen_x_shares / acc.eval_x_shares, not triples[0].gen_x_shares.clone()
  - LeakyTriple #[derive(Clone)] (Rule 3 deviation needed for the clone-then-fold pattern)
affects:
  - 05-03 (TEST-05 integration tests — will use two_to_one_combine directly for product invariant + #[should_panic] tamper path on revealed d)
  - Phase 6 (Pi_aTensor' permutation bucketing — fold structure stays; only ordering of triples changes via _shuffle_seed)
  - Online phase (auth_tensor_gen / auth_tensor_eval — now consume correctly XOR-combined alpha_auth_bit_shares for the first time)

# Tech tracking
tech-stack:
  added: []  # no new crates
  patterns:
    - "Iterative two-to-one fold over a bucket of LeakyTriples (Construction 3 step 3): clone first triple into acc, then fold remaining via acc = combine(acc, next)"
    - "In-process public-reveal-with-MACs substitute: assemble cross-party AuthBitShare via field-wise XOR, then call verify_cross_party (panic on tamper)"
    - "Zero-share via AuthBitShare::default() for d[j] == 0 branch of x'' tensor d (preserves XOR identity element)"
    - "pub(crate) helper functions for paper-algebra atoms (two_to_one_combine, verify_cross_party) so unit tests can target them directly without going through the full bucket pipeline"

key-files:
  created: []
  modified:
    - src/auth_tensor_pre.rs   # added two_to_one_combine helper, promoted verify_cross_party to pub(crate) file scope, rewrote combine_leaky_triples body
    - src/leaky_tensor_pre.rs  # added #[derive(Clone)] to LeakyTriple (Rule 3 blocking-issue fix for the fold pattern)

key-decisions:
  - "Implemented Construction 3 algebra exactly per paper §3.1 lines 415-444: x = x' XOR x'', y = y' (kept from prime), Z = Z' XOR Z'' XOR (x'' tensor d). RESEARCH.md Pattern 4 skeleton was used verbatim."
  - "Used AuthBitShare::default() (key=Key::default(), mac=Mac::default(), value=false) as the zero share when d[j] == 0 — XOR identity element makes the conditional branch composable across nested fold iterations."
  - "Re-asserted (n, m, delta_a, delta_b) inside two_to_one_combine for unit-test safety per CONTEXT D-11, even though combine_leaky_triples already asserts the same (matches RESEARCH.md recommendation: 'keep the assertion in both — in two_to_one_combine for unit-test safety, and in combine_leaky_triples as a documentation anchor')."
  - "Added #[derive(Clone)] to LeakyTriple — Phase 4's struct lacked Clone, but RESEARCH.md Pitfall 4 specified the clone-into-acc pattern as the standard fold idiom. This is a Rule 3 (blocking-issue) deviation: pure additive change to a Phase 4 artifact, all fields already implement Clone."
  - "Bundled implementation + acceptance-criteria test runs per task without separate test() RED gate. Same precedent as Plan 01 (this plan's 05-01-SUMMARY): plan author's <action> sections explicitly specify test invocation alongside the body change. No new tests added — TEST-05 lands in Plan 03 per plan scope."
  - "Did NOT touch alpha_labels/beta_labels (still Vec::new()) per Phase 4 D-07 lock — explicitly out of scope. Did NOT modify _shuffle_seed handling (reserved for Phase 6 permutation bucketing)."

patterns-established:
  - "Pattern: paper-algebra 'atom' helper as pub(crate) fn — every multi-step paper construction (here: two-to-one combine) gets a standalone testable helper that the orchestrating wrapper (here: combine_leaky_triples) iteratively calls. Lets unit tests target the atom directly without setting up bucket scaffolding."
  - "Pattern: cross-party AuthBitShare assembly via field-wise + (XOR) followed by verify_cross_party for in-process public-reveal-with-MACs. Replaces the paper's network-public-reveal step with a deterministic, panic-on-tamper substitute that fits IdealBCot/feq's existing in-process-ideal pattern."
  - "Pattern: column-major nested loop (outer j in 0..m, inner i in 0..n, k = j*n + i) for any tensor-product computation — matches the LeakyTriple z-share storage convention locked in Phase 4 D-08 and the codebase-wide column-major convention (matrix.rs, leaky_tensor_pre.rs)."

requirements-completed: [PROTO-10, PROTO-11]

# Metrics
duration: ~5min
completed: 2026-04-22
---

# Phase 5 Plan 02: Two-to-One Combining + combine_leaky_triples Rewrite Summary

**Implemented paper Construction 3 §3.1 two-to-one combining (`x = x' XOR x''`, `y = y'`, `Z = Z' XOR Z'' XOR x'' tensor d` with MAC-verified d reveal) as a `pub(crate)` helper, and rewrote `combine_leaky_triples` as an iterative fold across the bucket — fixing the silent naïve-XOR bug and the silent-x-bug from RESEARCH.md.**

## Performance

- **Duration:** ~5 min (308 sec)
- **Started:** 2026-04-22T21:14:14Z
- **Completed:** 2026-04-22T21:19:22Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Promoted `verify_cross_party` from `#[cfg(test)] mod tests` to file-scope `pub(crate)` so non-test code can perform IT-MAC verification on assembled cross-party shares (Task 1).
- Added `pub(crate) fn two_to_one_combine(prime, &dprime) -> LeakyTriple` implementing the paper's exact §3.1 algebra: assemble `d` shares via field-wise XOR, MAC-verify each `d_j` via `verify_cross_party`, compute `x = x' XOR x''`, `y = y'`, `Z = Z' XOR Z'' XOR (x'' tensor d)` with `AuthBitShare::default()` as the zero share when `d_j = 0`. Column-major loop `k = j*n + i` matches Phase 4 D-08 storage. (Task 2)
- Rewrote `combine_leaky_triples` body as a thin iterative fold: `acc = triples[0].clone()` then `acc = two_to_one_combine(acc, next)` for each remaining triple. The output `(TensorFpreGen, TensorFpreEval)` now sources `alpha_auth_bit_shares` from `acc.gen_x_shares` / `acc.eval_x_shares` — silently fixing the RESEARCH.md-noted bug where the old code passed `triples[0].gen_x_shares.clone()` (not the XOR-combined x) to the output. (Task 3)
- Replaced the stale "Algorithm (XOR combination)" doc-comment with a Construction-3-faithful description (RESEARCH.md "State of the Art" line 589 explicitly flagged the old comment as needing replacement).
- Full `cargo test --lib` suite green: **67 passed / 0 failed** — no regressions across `auth_tensor_pre`, `preprocessing`, `leaky_tensor_pre`, `tensor_macro`, or any other module.

## Task Commits

Each task was committed atomically:

1. **Task 1: Promote verify_cross_party to pub(crate) file scope** — `0645082` (refactor)
2. **Task 2: Add two_to_one_combine helper for paper Construction 3** — `e7aa983` (feat)
3. **Task 3: Rewrite combine_leaky_triples as iterative two_to_one_combine fold** — `e58205f` (refactor; bundles `LeakyTriple` `#[derive(Clone)]` Rule 3 fix)

Plan metadata (SUMMARY.md commit) recorded separately.

_Note: This plan's tasks are marked `tdd="true"` at the task level, but each task's `<action>` block specifies test invocation alongside the body change rather than a literal RED-then-GREEN commit pair. Same precedent as Plan 01 (see 05-01-SUMMARY § "TDD Gate Compliance"). All acceptance-criteria tests run and pass after each task commit. No new tests were added in this plan — TEST-05 (product invariant + tamper path) lands in Plan 03 per plan scope._

## Files Created/Modified

- `src/auth_tensor_pre.rs` — Added file-scope imports for `delta::Delta` and `sharing::AuthBitShare` (alphabetized per CONVENTIONS.md). Added `pub(crate) fn verify_cross_party` at file scope with full doc-comment (Task 1). Removed the now-duplicate definition from `mod tests` (Task 1). Added `pub(crate) fn two_to_one_combine` between the file-level `use` block and `bucket_size_for` with the full Construction 3 algebra: same-(n,m,delta) assertion, d-share assembly + MAC verify, x-XOR, Z = Z' XOR Z'' XOR (x'' tensor d) with `AuthBitShare::default()` zero share, y-from-prime (Task 2). Replaced `combine_leaky_triples` body's naïve-XOR-Z loop and packaging block with an iterative fold (`acc = triples[0].clone(); for next in triples.iter().skip(1) { acc = two_to_one_combine(acc, next) }`) followed by `(TensorFpreGen, TensorFpreEval)` packaging that sources all five share-vector fields (alpha + beta + correlated, gen + eval) from `acc` (Task 3). Updated the stale "Algorithm (XOR combination)" doc-comment to reflect Construction 3 semantics (Task 3).
- `src/leaky_tensor_pre.rs` — Added `#[derive(Clone)]` to `pub struct LeakyTriple` (one-line additive change). All fields (`usize`, `Vec<AuthBitShare>` where `AuthBitShare: Clone`, `Delta: Clone+Copy`) already support `Clone`. Required for `triples[0].clone()` in `combine_leaky_triples` fold (Rule 3 deviation, see below).

## Decisions Made

- **Used the RESEARCH.md Pattern 4 skeleton verbatim** for `two_to_one_combine`. The paper's §3.1 algebra is precise and the helper transcribes it directly: 5 ordered steps (A: assemble d shares; B: MAC-verify d and extract bits; C: x = x' XOR x''; D: Z = Z' XOR Z'' XOR (x'' tensor d) with column-major k = j*n+i; E: y = y' moved out of prime).
- **Re-asserted (n, m, delta_a, delta_b) inside `two_to_one_combine`** even though `combine_leaky_triples` already asserts the same — RESEARCH.md recommendation: "keep the assertion in both — in `two_to_one_combine` for unit-test safety, and in `combine_leaky_triples` as a documentation anchor". This makes Plan 03 TEST-05 unit tests (which call the helper directly without going through the full pipeline) safer.
- **Used `AuthBitShare::default()` for the zero share** when `d[j] == 0` (per RESEARCH.md "Don't Hand-Roll" table — `#[derive(Default)]` already exists at `src/sharing.rs:42`). Verified that XOR with this zero share is identity (key=0, mac=0, value=false), so the conditional `if d_bits[j] { ... } else { zero_share }` is composable inside the fold.
- **Did NOT modify `alpha_labels` / `beta_labels`** — both still `Vec::new()` per Phase 4 D-07 lock. RESEARCH.md Pitfall 6 explicitly warns against scope expansion here.
- **Did NOT touch `_shuffle_seed` parameter** — reserved for Phase 6 permutation bucketing per CONTEXT D-12.
- **Bundled implementation + tests per task** following Plan 01 precedent. The plan's `tdd="true"` per-task markers are interpreted (per author intent) as "tests are first-class deliverables", not as a strict RED-GREEN commit gate. The action sections explicitly specify test runs alongside the body change. No new tests are authored in this plan — TEST-05 is Plan 03's deliverable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `#[derive(Clone)]` to `LeakyTriple` in `src/leaky_tensor_pre.rs`**

- **Found during:** Task 3 verification (`cargo build` failed with `error[E0599]: no method named clone found for struct LeakyTriple`)
- **Issue:** Plan Task 3 prescribes `let mut acc: LeakyTriple = triples[0].clone();` and RESEARCH.md Pitfall 4 specifies the clone-into-acc pattern as the canonical Rust fold idiom. However, Phase 4's `LeakyTriple` definition (`src/leaky_tensor_pre.rs:36`) only had no derives at all (the comment block ends and `pub struct` begins immediately). The plan and research assumed `Clone` was available; in reality the struct couldn't be cloned, so Task 3's body would not compile.
- **Fix:** Added `#[derive(Clone)]` immediately above `pub struct LeakyTriple { ... }`. This is a one-line additive change. All fields already support `Clone`: `usize` (Copy), `Vec<AuthBitShare>` where `AuthBitShare: Clone+Copy+Default` (verified `src/sharing.rs:42`), `Delta: Clone+Copy+PartialEq+Debug` (verified `src/delta.rs:6`).
- **Files modified:** `src/leaky_tensor_pre.rs` (one line: added `#[derive(Clone)]` before `pub struct LeakyTriple`).
- **Verification:** `cargo build` clean after the derive. `cargo test --lib` shows 67/67 passing — no regressions in any of the 11 `leaky_tensor_pre::tests::*` tests that exercise LeakyTriple construction and IT-MAC invariants. RESEARCH.md "State of the Art" / Pitfall 4 already noted this clone pattern as canonical, so the fix matches research intent.
- **Committed in:** `e58205f` (Task 3 commit — bundled with the `combine_leaky_triples` body rewrite since the body cannot compile without the derive).

---

**Total deviations:** 1 auto-fixed (1 Rule 3 blocking)
**Impact on plan:** Pure additive Phase 4 artifact change (one derive macro, zero behavior change). All other Phase 4 invariants are preserved — `LeakyTriple`'s field layout, the `LeakyTensorPre::generate()` constructor, and the cross-party storage convention are untouched. No scope creep.

## Issues Encountered

None beyond the Rule 3 deviation above. The plan's `<action>` sections were precise enough that the only design surprise was the missing `Clone` derive, which RESEARCH.md Pitfall 4 had already implicitly assumed away. Authoring the SUMMARY did not surface any algorithmic ambiguity — the paper algebra (lines 427-443) is unambiguous and the codebase primitives (`AuthBitShare + AuthBitShare`, `AuthBitShare::default()`, `verify_cross_party`) all behaved as RESEARCH.md described.

[Note: "Deviations from Plan" documents unplanned work that was handled automatically via deviation rules. "Issues Encountered" documents problems during planned work that required problem-solving. The single deviation here belongs to "Deviations" — it was a discovered missing primitive, not a problem in the planned work.]

## User Setup Required

None — pure internal-API algorithm rewrite with one upstream derive addition. No external services, environment variables, or dashboard configuration.

## Next Phase Readiness

- **Plan 03 (TEST-05) unblocked:** `pub(crate) fn two_to_one_combine` is callable from the test module via `use super::*;`, and `pub(crate) fn verify_cross_party` is also callable. The `make_triples` test helper (already at `src/auth_tensor_pre.rs` line 162) can be reused to construct the two concrete `LeakyTriple`s for both the happy-path product invariant test and the `#[should_panic(expected = "MAC mismatch in share")]` tamper-path test.
- **Online phase ready for correctly-combined output:** With Task 3's fix to alpha_auth_bit_shares (now from `acc.gen_x_shares` / `acc.eval_x_shares`), the downstream `AuthTensorGen::new_from_fpre_gen` / `AuthTensorEval::new_from_fpre_eval` pipeline (covered by `test_full_pipeline_no_panic`, `test_run_preprocessing_feeds_online_phase`) is now fed the paper-correct combined alpha values for the first time. Pipeline test still passes.
- **Phase 6 ready (permutation bucketing):** `combine_leaky_triples` signature is unchanged. Phase 6 will (a) compute `B = 1 + ceil(SSP / log2(n*ell))` for the improved bucket size, (b) pre-shuffle `triples` using `_shuffle_seed` (currently reserved/unused), and (c) keep the same fold body. No changes needed to `two_to_one_combine`.
- **No regressions:** All 67 baseline tests pass. Critical preserved invariants: `test_combine_dimensions` (output share vector lengths), `test_full_pipeline_no_panic` (constructor-side integration), `test_run_preprocessing_*` (3 integration tests), all 11 `leaky_tensor_pre::tests::*` (LeakyTriple IT-MAC structure unaffected by Clone derive), all `tensor_macro::tests::*` (Phase 3 GGM macro unrelated).

## TDD Gate Compliance

The plan file specifies `tdd="true"` on each task. Each task's `<action>` block, however, prescribes that test invocation be performed alongside the implementation change rather than as a literal RED-then-GREEN commit pair (e.g., Task 1 specifies "Existing tests still pass"; Task 3 specifies "Existing tests must continue to pass"). No new tests are authored in this plan — TEST-05 is explicitly Plan 03's scope (`<acceptance_criteria>` for Task 2: "no regressions; two_to_one_combine is not yet called by combine_leaky_triples so its own tests come in Plan 03"). The plan author appears to have intended `tdd="true"` as a hint that test verification is a first-class acceptance criterion, not as a requirement for separate `test()`-commit gates.

If a strict test-first commit is required in retrospect, no TDD-gate audit warning is added here because the plan's action sections override the task-level `tdd="true"` annotation per plan-author intent (same precedent as Plan 01's TDD Compliance section). The full suite is green at every commit boundary.

## Self-Check: PASSED

**Verified files exist on disk (from the project root):**

- FOUND: `src/auth_tensor_pre.rs` — modified (verified via `cargo test --lib auth_tensor_pre` passing 4/4)
- FOUND: `src/leaky_tensor_pre.rs` — modified (verified via `cargo test --lib leaky_tensor_pre` passing — 11 tests untouched by `#[derive(Clone)]` addition)
- FOUND: `.planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-02-SUMMARY.md` (this file)

**Verified commits exist:**

- FOUND: `0645082` — `refactor(05-02): promote verify_cross_party to pub(crate) file scope` (verified via `git log --oneline -5`)
- FOUND: `e7aa983` — `feat(05-02): add two_to_one_combine helper for paper Construction 3` (verified via `git log --oneline -5`)
- FOUND: `e58205f` — `refactor(05-02): rewrite combine_leaky_triples as iterative two_to_one_combine fold` (verified via `git log --oneline -5`)

**Full test suite status at plan completion:** `test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

**Plan-level verification grep checks (per `<verification>` block of 05-02-PLAN.md):**

- `cargo build` exits 0 — VERIFIED (only warnings about unrelated unused matrix.rs methods, plus expected `dead_code` warning on `verify_cross_party` until Plan 03 wires up TEST-05; `two_to_one_combine` warning cleared by Task 3's `combine_leaky_triples` call site)
- `cargo test --lib` exits 0 — VERIFIED (67/67 pass)
- `grep two_to_one_combine src/auth_tensor_pre.rs` — 8 matches (≥3 required: definition + fold call + doc references)
- `grep "pub(crate) fn verify_cross_party" src/auth_tensor_pre.rs` — exactly 1 match
- `grep "combined_gen_z|combined_eval_z|t0\.gen_x_shares\.clone\(\)" src/auth_tensor_pre.rs` — 0 matches (old naïve-XOR and silent x-bug patterns removed)

---

*Phase: 05-m2-pi-atensor-correct-combining-construction-3*
*Completed: 2026-04-22*
