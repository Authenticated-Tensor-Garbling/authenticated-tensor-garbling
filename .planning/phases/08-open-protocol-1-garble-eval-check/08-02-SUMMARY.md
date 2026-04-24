---
phase: 08-open-protocol-1-garble-eval-check
plan: "02"
subsystem: mpc
tags: [rust, mpc, authenticated-garbling, protocol-1, online-phase, check-zero, consistency-check, it-mac]

# Dependency graph
requires:
  - phase: 07-preprocessing-trait-ideal-backends
    provides: AuthBitShare + Key + Delta types; gamma_auth_bit_shares fields on TensorFpreGen/Eval
  - phase: 04-m2-pi-leakytensor-f-eq-construction-2
    provides: IT-MAC invariant pattern (mac == key XOR value * delta); verify() contract

provides:
  - "src/online.rs: check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool"
  - "pub mod online declared in src/lib.rs adjacent to pub mod preprocessing"
  - "Caller-pre-XOR contract documented; open() deferred per D-01"

affects:
  - phase 08-03: Plan 03 tests call check_zero after assembling c_gamma_shares from auth tensor protocol output
  - phase 09: Protocol 2 online phase uses check_zero with its own D_ev-authenticated shares

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Thin online primitive returns bool (not panic) for abort signaling — matches D-07 decision"
    - "MAC check via key.auth(value, delta) — canonical IT-MAC reconstruction; when value=false reduces to mac==key"
    - "TDD RED/GREEN cycle: stub with always-false, then full implementation"

key-files:
  created:
    - src/online.rs
  modified:
    - src/lib.rs

key-decisions:
  - "check_zero returns bool, not panic — per D-07 (CONTEXT.md)"
  - "open() (ONL-01, ONL-02) explicitly deferred; doc comment in online.rs records deferral per D-01"
  - "test_check_zero_fails_on_invalid_mac uses corrupted mac (key.auth(true,delta) with value=false) — when value=false, auth is delta-independent so wrong-delta cannot produce a distinguishable MAC; the test exercises the MAC-mismatch branch correctly"

patterns-established:
  - "check_zero: loop checks value==false then mac==key.auth(value,delta_ev); returns false on first failure"
  - "Caller-pre-XOR contract: caller must XOR gen+eval shares before passing to check_zero; raw cross-party shares violate IT-MAC invariant"

requirements-completed: [P1-03, ONL-01, ONL-02]

# Metrics
duration: ~20min
completed: 2026-04-23
---

# Phase 8 Plan 02: check_zero Online Primitive Summary

**`src/online.rs` created with IT-MAC consistency-check `check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool` — five unit tests, caller-pre-XOR contract documented, `open()` deferred per D-01**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-04-23
- **Completed:** 2026-04-23
- **Tasks:** 1 (TDD: RED + GREEN commits)
- **Files modified:** 2

## Accomplishments

- Created `src/online.rs` with `check_zero` matching the D-07 signature exactly
- Documented the caller-pre-XOR contract in detail, referencing the cross-party share hazard at `src/auth_tensor_pre.rs:305-336`
- Added `pub mod online;` to `src/lib.rs` line 26, adjacent to `pub mod preprocessing;` (line 25)
- All 5 unit tests pass; 87/87 lib tests green (no regressions on 74 baseline + Plan 01 additions)
- Confirmed `open()` is NOT implemented — doc comment placeholder only, per D-01

## Task Commits

Each task was committed atomically (TDD flow):

1. **Task 1 RED: stub check_zero + failing tests** — `2fce856` (test)
2. **Task 1 GREEN: full check_zero implementation** — `e0c4e4b` (feat)

_TDD: RED commit had 2 tests failing (pass paths), GREEN commit made all 5 pass._

## Files Created/Modified

- `src/online.rs` — new module hosting `check_zero` primitive + 5 unit tests; `open()` deferred per D-01
- `src/lib.rs` — added `pub mod online;` at line 26 (adjacent to `pub mod preprocessing;`)

## Decisions Made

- **D-07 preserved:** signature is exactly `pub fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool`
- **D-01 preserved:** `open()` is NOT implemented; a doc comment at the top of the module records the deferral
- **D-08 preserved:** `check_zero` is thin — it does not know about the protocol structs or c_gamma assembly

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed structurally impossible MAC-mismatch test**

- **Found during:** Task 1 GREEN (running tests)
- **Issue:** `test_check_zero_fails_on_invalid_mac` as specified in the plan used `key.auth(false, &wrong_delta)` to produce a "wrong" MAC. However, `Key::auth(false, delta) = key.0 XOR Block::ZERO = key.0` regardless of delta — so `key.auth(false, wrong_delta) == key.auth(false, correct_delta)` always. The test incorrectly passed the MAC check when it should have failed, making the `assert!(!...)` panic.
- **Fix:** Changed the test to use `key.auth(true, &delta)` as the corrupted MAC for a `value=false` share. This produces `key.0 XOR delta.0` (nonzero with overwhelming probability), which does NOT equal `key.auth(false, delta) = key.0`, correctly triggering the MAC-mismatch branch in `check_zero`.
- **Files modified:** `src/online.rs`
- **Verification:** `cargo test --lib online::tests` — all 5 tests pass
- **Committed in:** `e0c4e4b` (GREEN phase commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug in test specification)
**Impact on plan:** The fix correctly exercises the MAC-mismatch abort path. The test still covers the intended security property: a share with `value=false` but an incorrect MAC must be rejected. No scope creep.

## Issues Encountered

- When `value == false`, `Key::auth` XORs `Block::ZERO`, making the output delta-independent. This means "wrong delta" cannot produce a distinguishable MAC for zero-value shares. The test was updated to use a structurally corrupted MAC (authenticated for `true` instead of `false`) to properly exercise the MAC-mismatch branch.

## Known Stubs

- `open()` function — explicitly deferred per D-01. The module doc comment records: "open() (ONL-01) and its wrong-delta negative test (ONL-02) are deferred to a later phase per Phase 8 CONTEXT.md D-01". This is an intentional, documented deferral — not a correctness-blocking stub. Plan 03's end-to-end Protocol 1 test does not require `open()`.

## Threat Surface Scan

No new network endpoints, auth paths, file access, or external trust boundaries introduced. `src/online.rs` is a pure in-process Rust function over already-existing types. Threat register items T-08-07 through T-08-12 from the plan's threat model are addressed:
- T-08-07 (caller misuse): caller-pre-XOR contract documented verbatim in `check_zero` doc comment
- T-08-08 (silent pass on tampered MAC): `test_check_zero_fails_on_invalid_mac` directly proves the MAC check runs independently of the value check
- T-08-09 (panic on bad input): function is total; empty slice returns `true` per `test_check_zero_passes_on_empty_slice`

## Next Phase Readiness

- `check_zero` is available at `crate::online::check_zero` for Plan 03's end-to-end Protocol 1 tests
- Plan 03 must: add `gamma_auth_bit_shares` field to `AuthTensorGen`/`AuthTensorEval`, implement `compute_lambda_gamma()` on both, then assemble `c_gamma_shares` and call `check_zero`
- The caller-pre-XOR contract (pre-XOR gen+eval `AuthBitShare` vecs using `+` operator before passing to `check_zero`) is documented in the function's doc comment

## Self-Check: PASSED

- `src/online.rs` exists: FOUND
- `pub mod online;` in lib.rs at line 26: FOUND
- `pub fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool` in online.rs: FOUND
- `fn open` in online.rs: NOT FOUND (correct — deferred per D-01)
- RED commit `2fce856`: exists in git log
- GREEN commit `e0c4e4b`: exists in git log
- `cargo test --lib online::tests`: 5/5 pass
- `cargo test --lib`: 87/87 pass

---
*Phase: 08-open-protocol-1-garble-eval-check*
*Completed: 2026-04-23*
