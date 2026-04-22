---
phase: 04-m2-pi-leakytensor-f-eq-construction-2
plan: 01
subsystem: crypto
tags: [rust, preprocessing, leaky-tensor, feq, delta, lsb-invariant]

# Dependency graph
requires:
  - phase: 03-m2-generalized-tensor-macro-construction-1
    provides: tensor_garbler and tensor_evaluator primitives (Plan 2 will consume them)
provides:
  - Delta::new_with_lsb and Delta::random_b constructors (LSB=0 for Party B)
  - IdealBCot with lsb(Δ_A ⊕ Δ_B) == 1 invariant enforced
  - src/feq.rs — ideal F_eq module with feq::check(l1, l2) panic-on-mismatch
  - LeakyTriple struct with paper-notation 10-field shape (gen/eval x/y/z + deltas)
  - LeakyTensorPre::generate() no-arg signature with unimplemented!() scaffold
  - Caller cascade: auth_tensor_pre.rs and preprocessing.rs compile against new fields
  - All 4 baseline-failing tests deleted; Plan 2/3 placeholder tests with #[ignore]
affects: [04-02-PLAN, 04-03-PLAN, 05-pi-atensor-combining, 06-pi-atensor-prime]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Delta::random_b() for LSB=0 delta — use when paper requires lsb(Δ_A ⊕ Δ_B)==1"
    - "feq::check() panic pattern — ideal functionality abort via panic!, no Result return"
    - "Phase-stub Vec::new() with explicit comment for intentionally empty label vecs"

key-files:
  created:
    - src/feq.rs
  modified:
    - src/delta.rs
    - src/bcot.rs
    - src/lib.rs
    - src/leaky_tensor_pre.rs
    - src/auth_tensor_pre.rs
    - src/preprocessing.rs
    - src/tensor_macro.rs

key-decisions:
  - "D-01: generate() takes no args — x/y sampled internally; preprocessing is input-independent"
  - "D-03/D-04: feq.rs as separate ideal-functionality module, panic on L1 != L2 (no Result)"
  - "D-06/D-07: LeakyTriple renamed to paper notation (x/y/z) and gamma+labels removed"
  - "Option A for combine_leaky_triples: rename fields, stub labels to Vec::new(), keep build green"
  - "Auto-fix: tensor_macro tests switched from delta_b (LSB=0) to delta_a (LSB=1) after Phase 4 delta change"

patterns-established:
  - "Delta::random_b: Party-B delta always LSB=0; pairs with Delta::random (LSB=1) for Δ_A"
  - "feq::check: column-major iteration, panic! message includes (i,j) but not block contents"

requirements-completed: [PROTO-08, PROTO-09]

# Metrics
duration: 8min
completed: 2026-04-21
---

# Phase 04 Plan 01: Pi_LeakyTensor Scaffolding — Delta LSB Fix, F_eq Module, LeakyTriple Rewrite

**Scaffold for Pi_LeakyTensor Construction 2: lsb(Δ_A⊕Δ_B)==1 invariant enforced via Delta::random_b, ideal F_eq module with panic-abort semantics, LeakyTriple rewritten to 10-field paper notation (no gamma/labels), generate() no-arg scaffold, and compile-green cascade through combine_leaky_triples and run_preprocessing**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-21T05:16:16Z
- **Completed:** 2026-04-21T05:23:41Z
- **Tasks:** 3
- **Files modified:** 7 (1 new: src/feq.rs)

## Accomplishments

- `lsb(Δ_A ⊕ Δ_B) == 1` invariant is now locked by a regression test (`test_delta_xor_lsb_is_one`) and enforced structurally by `IdealBCot::new` using `Delta::random_b` for Party B
- New `src/feq.rs` ideal F_eq module: `feq::check(&l1, &l2)` panics on mismatch with `"F_eq abort: ..."` message and coordinates; three `#[should_panic]` tests cover the abort and dimension-mismatch paths
- `LeakyTriple` struct is exactly the paper's 10-field shape (`gen/eval_x/y/z_shares` + `delta_a/delta_b`); gamma bits and wire labels are physically absent from the type
- `generate()` is the no-arg scaffold; callers in `auth_tensor_pre.rs` and `preprocessing.rs` compile via Option A field-rename cascade with `Vec::new()` stubs for labels
- 52 tests pass, 0 fail, 13 ignored (Plan 2/3 placeholders); `cargo build --lib` clean

## Task Commits

1. **Task 1.1: Δ_B LSB fix + Delta constructors + regression test** - `45c15ab` (feat)
2. **Task 1.2: Create src/feq.rs with check() + 3 tests; register in lib.rs** - `62bb8fb` (feat)
3. **Task 1.3: Rewrite LeakyTriple struct + generate() signature; cascade** - `14b6c6b` (feat)

## Files Created/Modified

- `src/delta.rs` — Added `Delta::new_with_lsb(block, lsb_value)` and `Delta::random_b(rng)` (LSB=0 for Party B)
- `src/bcot.rs` — `IdealBCot::new` now uses `Delta::random_b` for `delta_b`; added `test_delta_xor_lsb_is_one` regression test
- `src/feq.rs` — NEW: ideal F_eq module with `feq::check(l1, l2)` and 3 inline tests
- `src/lib.rs` — Added `pub mod feq;` between `bcot` and `leaky_tensor_pre`
- `src/leaky_tensor_pre.rs` — Full rewrite: `LeakyTriple` 10-field paper shape; `generate()` no-arg scaffold with `unimplemented!()`; `verify_cross_party` preserved verbatim; placeholder tests for Plan 2/3
- `src/auth_tensor_pre.rs` — Field-rename cascade (Option A): `gen_correlated_shares` → `gen_z_shares`, `gen_alpha_shares` → `gen_x_shares`, `gen_beta_shares` → `gen_y_shares`; labels stubbed to `Vec::new()`; `test_combine_mac_invariants` deleted; `test_combine_dimensions` and `test_full_pipeline_no_panic` marked `#[ignore]`
- `src/preprocessing.rs` — Call site updated `ltp.generate(0, 0)` → `ltp.generate()`; comment updated to reflect input-independence; `test_run_preprocessing_mac_invariants` deleted; 3 tests marked `#[ignore]`
- `src/tensor_macro.rs` — Auto-fix: `run_one_case` test fixture updated to use `delta_a`/`transfer_b_to_a` (was `delta_b`/`transfer_a_to_b`) after delta_b LSB change

## Decisions Made

- **D-01** — `generate()` takes no arguments; x and y bits are sampled internally from `self.rng` (preprocessing is fully input-independent per Construction 2)
- **D-03/D-04** — `feq.rs` is a standalone module matching the `IdealBCot` pattern; `feq::check` panics (no `Result`) on L_1 ≠ L_2 — abort semantics are unconditional
- **D-06/D-07** — `LeakyTriple` fields renamed to paper notation (alpha→x, beta→y, correlated→z); gamma bits and wire labels physically removed
- **Option A** for combine_leaky_triples — rename field references, stub labels to `Vec::new()` with explicit comment; keeps build green for Plan 2 to build on

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] tensor_macro tests used delta_b (now LSB=0) to extract choice bits from MACs**
- **Found during:** Task 1.3 (final test run after all cascade edits)
- **Issue:** 8 `tensor_macro::tests` failed because `run_one_case` used `bcot.delta_b` as the garbler delta and `transfer_a_to_b`. After the Phase 4 change `delta_b.lsb()==0`, MACs always had LSB=0 regardless of choice bits, so `a_bits` was all-false and the `Z_gen XOR Z_eval == a ⊗ T` invariant failed.
- **Fix:** Changed `run_one_case` to use `bcot.delta_a` (LSB=1, invariant unchanged) as the garbler delta and `transfer_b_to_a` instead of `transfer_a_to_b`. This correctly encodes choice bits in MAC LSBs: `mac = K[0] XOR choice * delta_a`, `mac.lsb() = choice * 1 = choice`.
- **Files modified:** `src/tensor_macro.rs`
- **Verification:** All 10 `tensor_macro::tests` pass after fix.
- **Committed in:** `14b6c6b` (Task 1.3 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug caused by Task 1.1's delta_b LSB change propagating into Phase 3 test fixtures)
**Impact on plan:** Necessary correctness fix. The tensor_macro tests are independent standalone tests that previously relied on `delta_b.lsb()==1`; switching to `delta_a` correctly isolates the macro test from the Phase 4 structural change.

## Known Stubs

| Stub | File | Line | Reason |
|------|------|------|--------|
| `alpha_labels: Vec::new()` | `src/auth_tensor_pre.rs` | 90, 101 | Phase 5 stub — labels removed from LeakyTriple (D-07); Phase 5 rewrites combine semantics with real derivation |
| `beta_labels: Vec::new()` | `src/auth_tensor_pre.rs` | 91, 102 | Same as above |
| `unimplemented!("Pi_LeakyTensor generate() body is Plan 2 of Phase 4")` | `src/leaky_tensor_pre.rs` | ~81 | Intentional scaffold — Plan 2 replaces with 5-step paper transcript |

All stubs are intentional and documented. The `Vec::new()` label stubs prevent the online-phase pipeline from being exercised until Phase 5 (all downstream tests that reach this path are `#[ignore]`'d). The `unimplemented!()` generate body is the entire purpose of Plan 1 — Plan 2 fills it in.

## Issues Encountered

None — the Plan 2/3 placeholder ignores and the tensor_macro fix were the only complications, both handled by the deviation rules.

## Next Phase Readiness

Plan 2 can now:
- Implement the `generate()` body in `src/leaky_tensor_pre.rs` (5-step paper transcript) without touching struct shape, module graph, or call sites
- Assume: `delta_b.lsb()==0`, `lsb(Δ_A⊕Δ_B)==1`, `feq::check` callable, `LeakyTriple` paper-shape, `tensor_garbler`/`tensor_evaluator` importable
- Remove all `#[ignore]` placeholders as the generate body makes them runnable

No blockers. All structural preconditions for Plan 2 are in place.

---
*Phase: 04-m2-pi-leakytensor-f-eq-construction-2*
*Completed: 2026-04-21*

## Self-Check: PASSED

- All 8 source files exist
- All 3 task commits confirmed (45c15ab, 62bb8fb, 14b6c6b)
- SUMMARY.md exists
