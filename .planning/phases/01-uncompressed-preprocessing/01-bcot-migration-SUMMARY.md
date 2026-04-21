---
phase: 01-uncompressed-preprocessing
plan: 01-bcot-migration
subsystem: primitives
tags: [rust, keys, bcot, refactor, invariants]

# Dependency graph
requires:
  - phase: 01-uncompressed-preprocessing
    provides: Key::new constructor (Wave 1, 01-PLAN-keys-sharing)
provides:
  - bcot.rs transfer_a_to_b and transfer_b_to_a using Key::new constructor
  - Last manual-invariant pattern eliminated from the codebase
affects: [01-uncompressed-preprocessing, any future plan that reads bcot.rs construction idioms]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Key::new(Block::random(rng)) — canonical pattern for constructing a fresh Key with lsb()==0"

key-files:
  created: []
  modified:
    - src/bcot.rs

key-decisions:
  - "Replace two-step set_lsb(false)+Key::from with Key::new; no algorithmic change, purely a construction idiom consolidation"
  - "Pre-existing test failures in auth_tensor_fpre, auth_tensor_pre, leaky_tensor_pre are out of scope and pre-date this plan (confirmed by stash check)"

patterns-established:
  - "Key construction from random block: always use Key::new(Block::random(rng)), never two-step"

requirements-completed: [CLEAN-01]

# Metrics
duration: 5min
completed: 2026-04-21
---

# Phase 01 Plan bcot-migration: bcot.rs migrated to Key::new for LSB-zero invariant enforcement

**Replaced the two-step `k0_block.set_lsb(false); Key::from(k0_block)` pattern in both transfer_a_to_b and transfer_b_to_a with the single-call `Key::new(Block::random(rng))`, consolidating the Key.lsb()==0 invariant into one place.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-04-21T00:00:00Z
- **Completed:** 2026-04-21T00:05:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Eliminated both occurrences of the manual `set_lsb(false) + Key::from` two-step pattern in `src/bcot.rs`
- Key.lsb()==0 invariant is now enforced exclusively by `Key::new` — no caller-side discipline required
- All 6 bcot tests pass unchanged; no behavioral change to the produced (key, mac) pairs

## Task Commits

Each task was committed atomically:

1. **Task 1: Collapse set_lsb(false) + Key::from to Key::new in both transfer_* methods** - `a400017` (refactor)

**Plan metadata:** committed with SUMMARY below

## Files Created/Modified
- `src/bcot.rs` - Replaced three-line two-step pattern with one-line Key::new call in both transfer_a_to_b and transfer_b_to_a

## Decisions Made
- None - followed plan as specified. The replacement is byte-identical in behavior: Key::new internally calls set_lsb(false) before wrapping, exactly matching the old two-step.

## Deviations from Plan

None - plan executed exactly as written.

Pre-existing test failures in `auth_tensor_fpre::tests`, `auth_tensor_pre::tests`, and `leaky_tensor_pre::tests` (4 tests) were confirmed to pre-date this plan via `git stash` baseline check. They are out of scope and tracked for a future plan.

## Issues Encountered
- None.

## Known Stubs
None — no stubs or placeholders introduced.

## Threat Surface Scan
No new network endpoints, auth paths, file access patterns, or schema changes introduced. This is a pure refactor of a construction idiom within an in-process ideal functionality used only for benchmarking.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `src/bcot.rs` is fully migrated to `Key::new`
- The `Key.lsb()==0` invariant is now enforced in exactly one place across the codebase
- Ready for Wave 3 plans or any subsequent plan that builds on bcot primitives

---
*Phase: 01-uncompressed-preprocessing*
*Completed: 2026-04-21*
