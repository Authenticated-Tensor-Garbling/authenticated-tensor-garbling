---
phase: "01"
plan: "cot"
subsystem: "bcot"
tags: ["ideal-functionality", "correlated-ot", "preprocessing", "authenticated-shares"]
dependency_graph:
  requires: []
  provides: ["IdealBCot", "BcotOutput", "output_to_auth_bit_shares_a_holds_key"]
  affects: ["src/bcot.rs", "src/lib.rs"]
tech_stack:
  added: ["src/bcot.rs"]
  patterns: ["ideal-functionality-trusted-dealer", "no-network-in-process", "lsb-invariant-enforcement"]
key_files:
  created: ["src/bcot.rs"]
  modified: ["src/lib.rs"]
decisions:
  - "Use in-process ideal functionality (no networking) matching TensorFpre pattern"
  - "Key LSB cleared to 0 via set_lsb(false) immediately after random generation"
  - "output_to_auth_bit_shares_b_holds_key intentionally omitted — casting receiver_macs to Key violates Key LSB=0 invariant"
  - "Separate transfer_b_to_a call used when B needs to hold the key (B's sender_keys have LSB=0 by construction)"
metrics:
  duration: "709s"
  completed_date: "2026-04-20"
  tasks_completed: 2
  files_created: 1
  files_modified: 1
---

# Phase 01 Plan cot: IdealBCot (Boolean Correlated OT) Summary

**One-liner:** In-process ideal bCOT with two directional transfer functions (A-to-B and B-to-A) enforcing Key LSB=0 and mac==key.auth(bit,delta) for all outputs.

## Files Created / Modified

| File | Action | Description |
|------|--------|-------------|
| `src/bcot.rs` | Created | IdealBCot struct, BcotOutput, transfer_a_to_b, transfer_b_to_a, output_to_auth_bit_shares_a_holds_key, 6 tests |
| `src/lib.rs` | Modified | Added `pub mod bcot;` after auth_tensor_eval declaration |

## Key Design Decisions

### 1. In-process ideal functionality (no networking)

`IdealBCot` follows the same trusted-dealer pattern as `TensorFpre` — both parties' views are computed in a single process with no channels or async. This is explicitly intended for uncompressed preprocessing benchmarks and is documented with a TODO for replacement with a real OT protocol (Ferret/IKNP) for production.

### 2. Key LSB=0 invariant enforced at generation

For every `transfer_a_to_b` and `transfer_b_to_a` call, each sender key is generated as:
```rust
let mut k0_block = Block::random(&mut self.rng);
k0_block.set_lsb(false);
let k0 = Key::from(k0_block);
```
This matches the `Key::adjust()` pattern in `src/keys.rs` (line 38). All 6 tests include LSB checks.

### 3. Why `output_to_auth_bit_shares_b_holds_key` was omitted (I-05 fix)

`receiver_macs` in a `BcotOutput` are `Mac` values derived from `K[0] XOR b*delta`. When `b=true`, the mac block has `delta`'s LSB=1 XORed in, so its LSB may be 1. Casting a `Mac` to `Key` directly would produce a `Key` with LSB=1, violating the global invariant `Key.lsb() == 0` (enforced in `src/keys.rs::Key::adjust()`).

The correct approach — when the caller (e.g., `leaky_tensor_pre.rs`) needs B to hold the key — is to run a separate `transfer_b_to_a` call where B is the sender. B's `sender_keys` are always generated with `set_lsb(false)` and therefore have LSB=0 by construction.

### 4. Delta derivation

`delta_a` and `delta_b` are derived from separate seeded RNGs (`seed_a` and `seed_b`), while key generation uses a combined RNG seeded from `seed_a ^ seed_b`. This gives deterministic, reproducible behavior for benchmarks.

## Tests (6 tests, all pass)

| Test | Description |
|------|-------------|
| `test_transfer_a_to_b_all_false` | With b=false, receiver_mac block == sender_key block |
| `test_transfer_a_to_b_all_true` | With b=true, receiver_mac == sender_key XOR delta_b |
| `test_transfer_b_to_a_mixed` | Mixed choices, mac == key XOR bit*delta_a |
| `test_auth_bit_shares_verify` | AuthBitShares pass verify() with correct delta |
| `test_key_lsb_is_zero` | All sender_keys have LSB=0 for both transfer directions |
| `test_stress_256_pairs` | 256 COT pairs generated without panic, spot-checked |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unused `Rng` import**
- **Found during:** Task 1 verification (compiler warning)
- **Issue:** `use rand::{Rng, SeedableRng}` — `Rng` trait was imported but `Block::random` dispatch does not require explicit trait import at the call site since `ChaCha12Rng` implements it transitively
- **Fix:** Changed to `use rand::SeedableRng;`
- **Files modified:** `src/bcot.rs`
- **Commit:** 08d686f (edit applied before final commit)

### Execution Notes

- Task 2 (lib.rs registration) was implemented before running Task 1 tests, since the Rust module system requires `pub mod bcot;` to be present for `cargo test bcot::` to compile and discover the tests. The incremental build cache had a stale lock from a parallel build attempt; it cleared automatically on the next clean invocation.

## Self-Check

- [x] `src/bcot.rs` exists
- [x] `src/lib.rs` contains `pub mod bcot;`
- [x] `output_to_auth_bit_shares_b_holds_key` NOT in `src/bcot.rs`
- [x] 6 tests pass: `cargo test bcot:: -- all ok`
- [x] Key LSB is 0 for all generated keys
- [x] Commits 08d686f and 1fc0f90 exist

## Self-Check: PASSED
