---
phase: 01-uncompressed-preprocessing
plan: 01-PLAN-keys-sharing
subsystem: primitives/sharing
tags:
  - refactor
  - primitives
  - invariants
  - key-type
  - sharing
dependency_graph:
  requires: []
  provides:
    - Key::new() safe constructor with lsb==0 invariant
    - Key::random() invariant-preserving constructor
    - InputSharing::shares_differ() replaces bit()
    - AuthBitShare doc: one party's view, mac == key.auth(value, verifier_delta)
    - AuthBit doc: both parties' views paired
    - build_share doc: delta is verifying party's global correlation key
  affects:
    - src/auth_tensor_fpre.rs (call sites updated)
    - src/tensor_pre.rs (call sites updated)
tech_stack:
  added: []
  patterns:
    - Construction-time invariant enforcement via Key::new (follows Delta::new pattern)
key_files:
  created: []
  modified:
    - src/keys.rs
    - src/sharing.rs
    - src/auth_tensor_fpre.rs
    - src/tensor_pre.rs
decisions:
  - "Key::from(block) retained as zero-cost cast per D-02; callers that already cleared LSB use it directly"
  - "Hard rename of InputSharing::bit() to shares_differ() (no alias) — compile-time enforcement of all call sites"
  - "Key::random delegates to Key::new rather than bare Self(Block::random(rng))"
metrics:
  duration: ~8 minutes
  completed: 2026-04-21T21:20:55Z
  tasks_completed: 3
  tasks_total: 3
---

# Phase 1 Plan 01: Keys & Sharing Cleanup Summary

**One-liner:** Construction-time lsb==0 invariant via Key::new(), build_share bug fixed, InputSharing::bit() hard-renamed to shares_differ(), and BDOZ doc comments added to AuthBitShare/AuthBit/build_share.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add Key::new() and enforce lsb==0 in Key::random() | caeb111 | src/keys.rs |
| 2 | Fix build_share, add docs, rename bit() to shares_differ() | a1a1faf | src/sharing.rs |
| 3 | Update 4 InputSharing.bit() call sites to shares_differ() | 90107a9 | src/auth_tensor_fpre.rs, src/tensor_pre.rs |

## What Was Built

### Task 1 — Key::new() and Key::random() (CLEAN-01)

Added `Key::new(mut block: Block) -> Self` that clears `block.set_lsb(false)` before wrapping, following the `Delta::new` pattern established in `src/delta.rs`. Updated `Key::random` to delegate to `Key::new(Block::random(rng))` instead of the bare `Self(Block::random(rng))`.

`Key::from(block)` and `Key::from([u8;16])` are retained unchanged as zero-cost casts — callers that have already enforced the invariant (e.g., `bcot.rs` which manually calls `set_lsb(false)` before `Key::from`) continue to work without behavioral change.

Four tests added to `src/keys.rs`:
- `test_key_new_clears_lsb_when_set`
- `test_key_new_idempotent_when_already_cleared`
- `test_key_random_lsb_is_zero` (seeded ChaCha12Rng, 64 iterations)
- `test_key_from_block_preserves_lsb_for_backward_compat`

### Task 2 — sharing.rs cleanup (CLEAN-02, CLEAN-03, CLEAN-04)

**Bug fix (T-01-02 mitigation):** `build_share` was using `Key::from(rng.random::<[u8;16]>())` which does NOT clear the LSB. Replaced with `Key::new(Block::random(rng))` so all `build_share` outputs satisfy `key.lsb() == 0`. The old pattern no longer appears anywhere in `src/sharing.rs`.

**Hard rename:** `InputSharing::bit()` removed; `InputSharing::shares_differ()` added in its place. The new method returns `self.gen_share != self.eval_share` — same value, unambiguous name. No alias was added per D-09.

**Doc comments added:**
- `AuthBitShare`: "One party's view of an authenticated bit" with invariant `mac == key.auth(value, verifier_delta)`
- `AuthBit`: "Both parties' views of an authenticated bit, paired together"
- `build_share`: explains `delta` is the verifying party's global correlation key

Unused `use rand::Rng` import removed.

Three tests added: `test_build_share_key_lsb_is_zero`, `test_build_share_mac_invariant_holds`, `test_input_sharing_shares_differ`.

### Task 3 — Call site migration (CLEAN-03 follow-through)

Four `InputSharing::bit()` call sites replaced with `shares_differ()`:
- `src/auth_tensor_fpre.rs:235` — `x_labels[i].bit()` → `shares_differ()`
- `src/auth_tensor_fpre.rs:239` — `y_labels[j].bit()` → `shares_differ()`
- `src/tensor_pre.rs:109` — `x_labels[i].bit()` and `alpha_labels[i].bit()` → `shares_differ()`
- `src/tensor_pre.rs:119` — `y_labels[j].bit()` and `beta_labels[j].bit()` → `shares_differ()`

`AuthBitShare::bit()` calls in other files (auth_tensor_gen.rs, auth_tensor_eval.rs, lib.rs, leaky_tensor_pre.rs) were not touched — those are a different method on a different type.

## Verification Results

```
cargo build --lib     → Finished (0 errors, 1 pre-existing unused-import warning cleared)
cargo build --tests --benches → Finished (2 pre-existing dead_code warnings only)
cargo test --lib      → 48 passed; 4 failed (all 4 pre-existing, see below)
```

New tests all pass:
- `keys::tests` — 4/4
- `sharing::tests` — 3/3

## Pre-existing Test Failures (not caused by this plan)

The following 4 tests were already failing at commit `db6bd2c` (the base of this plan) before any changes:

| Test | Failure |
|------|---------|
| `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants` | MAC mismatch in share |
| `leaky_tensor_pre::tests::test_correlated_mac_invariants` | MAC mismatch in share |
| `auth_tensor_fpre::tests::test_run_preprocessing_mac_invariants` | MAC mismatch in share |
| `auth_tensor_pre::tests::test_combine_mac_invariants` | MAC mismatch in share |

These failures are in the leaky tensor preprocessing protocol which is a known-broken subsystem (see PROJECT.md: "Bug 3 — Wrong Pi_aTensor Combining Algorithm"). They are not related to the Key/sharing refactor and will be addressed in later phases.

Confirmed pre-existing by temporarily restoring `db6bd2c:src/keys.rs` and running the tests — same failures occurred with the original code.

## Deviations from Plan

None — plan executed exactly as written. All acceptance criteria satisfied.

## Known Stubs

None — this plan makes no data-flow changes and introduces no stub values.

## Threat Flags

No new security-relevant surface introduced. The changes strengthen the Key LSB invariant (T-01-02 mitigated) and make T-01-03 compile-time enforced. No new network endpoints, auth paths, file access patterns, or schema changes.

## Self-Check: PASSED

- src/keys.rs: FOUND
- src/sharing.rs: FOUND
- src/auth_tensor_fpre.rs: FOUND
- src/tensor_pre.rs: FOUND
- SUMMARY.md: FOUND
- Commit caeb111: FOUND
- Commit a1a1faf: FOUND
- Commit 90107a9: FOUND
- No file deletions in any commit: OK
