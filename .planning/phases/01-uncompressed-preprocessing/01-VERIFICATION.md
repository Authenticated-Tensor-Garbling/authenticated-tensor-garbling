---
phase: 01-uncompressed-preprocessing
verified: 2026-04-21T22:00:00Z
status: passed
score: 14/14 must-haves verified
overrides_applied: 0
---

# Phase 1: Uncompressed Preprocessing Verification Report

**Phase Goal:** Stable pre-April-10 primitives (block, delta, keys, macs, aes, sharing, matrix, tensor_ops) enforce invariants at construction time, are correctly named, and have documentation calling out non-obvious behavior — with zero algorithmic changes.
**Verified:** 2026-04-21T22:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | `Key::new(block)` exists and enforces `lsb()==0` at construction (CLEAN-01) | VERIFIED | `src/keys.rs:24`: `pub fn new(mut block: Block) -> Self { block.set_lsb(false); Self(block) }` |
| 2  | `Key::random()` produces keys with `lsb()==0` (CLEAN-01) | VERIFIED | `src/keys.rs:101-103`: `pub fn random<R: Rng>(rng: &mut R) -> Self { Self::new(Block::random(rng)) }` |
| 3  | `Key::from(block)` retained unchanged as zero-cost cast (CLEAN-01 escape hatch) | VERIFIED | `src/keys.rs:113-118`: `impl From<Block> for Key` unwraps directly to `Key(block)` with no LSB manipulation |
| 4  | `build_share()` uses `Key::new` not the old `Key::from` pattern (CLEAN-04) | VERIFIED | `src/sharing.rs:125`: `let key: Key = Key::new(Block::random(rng));`; old `Key::from(rng.random::<[u8;16]>())` pattern absent |
| 5  | `AuthBitShare` has doc comment stating it is one party's view with IT-MAC invariant (CLEAN-02) | VERIFIED | `src/sharing.rs:30`: `/// One party's view of an authenticated bit.` with full invariant formula `mac == key.auth(value, verifier_delta)` |
| 6  | `AuthBit` has doc comment stating it holds both parties' views (CLEAN-02) | VERIFIED | `src/sharing.rs:130`: `/// Both parties' views of an authenticated bit, paired together.` |
| 7  | `InputSharing::shares_differ()` replaces `InputSharing::bit()` — old method deleted (CLEAN-03) | VERIFIED | `src/sharing.rs:25`: `pub fn shares_differ(&self) -> bool { self.gen_share != self.eval_share }`. The only `fn bit(&self)` remaining is on `AuthBitShare` (line 55), not `InputSharing` |
| 8  | All 4 call sites updated in `auth_tensor_fpre.rs` and `tensor_pre.rs` (CLEAN-03) | VERIFIED | `auth_tensor_fpre.rs:235,239`: both `x_labels[i].shares_differ()` and `y_labels[j].shares_differ()`; `tensor_pre.rs:109,119`: all 4 occurrences use `shares_differ()`. No `.bit() as usize` pattern anywhere in `src/` |
| 9  | `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` in `tensor_ops.rs` are `pub(crate)` (CLEAN-05) | VERIFIED | `src/tensor_ops.rs:9`: `pub(crate) fn gen_populate_seeds_mem_optimized`; line 88: `pub(crate) fn gen_unary_outer_product` |
| 10 | `MatrixViewRef` and `MatrixViewMut` in `matrix.rs` are `pub(crate)` (CLEAN-05) | VERIFIED | `src/matrix.rs:39`: `pub(crate) struct MatrixViewRef`; line 50: `pub(crate) struct MatrixViewMut` |
| 11 | `TypedMatrix` has column-major doc comment (CLEAN-06) | VERIFIED | `src/matrix.rs:19-26`: 8-line `///` doc starting `/// **Storage is column-major**`, formula `j * rows + i` also in `flat_index` doc at line 63 |
| 12 | `FIXED_KEY_AES` has doc comment explaining `once_cell` and protocol-constant nature (CLEAN-06) | VERIFIED | `src/aes.rs:14-36`: 22-line `///` doc covers `once_cell::sync::Lazy` rationale, `Send + Sync` thread-safety, and `FIXED_KEY` is a **protocol constant**, not a secret |
| 13 | `cargo build --lib` succeeds | VERIFIED | `Finished dev profile [unoptimized + debuginfo] target(s) in 0.71s` — 8 unreachable-pub warnings on `pub(crate)` struct methods (expected, pre-existing pattern); 0 errors |
| 14 | `bcot.rs` uses `Key::new(Block::random(rng))` in both transfer functions (CLEAN-01 follow-through) | VERIFIED | `src/bcot.rs:68` and `92`: `let k0 = Key::new(Block::random(&mut self.rng));`. Old `k0_block.set_lsb(false); Key::from(k0_block)` two-step pattern absent |

**Score:** 14/14 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/keys.rs` | `Key::new()` safe constructor, `Key::random()` using it | VERIFIED | `Key::new` at line 24, `Key::random` at line 101, `From<Block>` unchanged at line 113 |
| `src/sharing.rs` | `shares_differ()`, `AuthBitShare`/`AuthBit`/`build_share` docs, `Key::new` in `build_share` | VERIFIED | All items present and substantive |
| `src/auth_tensor_fpre.rs` | `get_clear_values` using `shares_differ()` | VERIFIED | Lines 235, 239 use `shares_differ()` |
| `src/tensor_pre.rs` | `mask_inputs` using `shares_differ()` | VERIFIED | Lines 109, 119 use `shares_differ()` (4 occurrences total) |
| `src/bcot.rs` | `transfer_a_to_b` and `transfer_b_to_a` using `Key::new` | VERIFIED | Lines 68, 92 each use `Key::new(Block::random(&mut self.rng))` |
| `src/matrix.rs` | `pub(crate) MatrixViewRef/Mut`, column-major doc on `TypedMatrix` | VERIFIED | Visibility and docs confirmed |
| `src/tensor_ops.rs` | `pub(crate) gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` | VERIFIED | Both `pub(crate)` at lines 9 and 88 |
| `src/aes.rs` | `FIXED_KEY_AES` with `once_cell`/protocol-constant doc | VERIFIED | 22-line doc present at line 14 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `sharing.rs (build_share)` | `keys.rs (Key::new)` | `Key::new(Block::random(rng))` | WIRED | `src/sharing.rs:125` matches pattern `Key::new\(Block::random` |
| `auth_tensor_fpre.rs (get_clear_values)` | `sharing.rs (InputSharing::shares_differ)` | method call | WIRED | `src/auth_tensor_fpre.rs:235,239` — both `x_labels[i].shares_differ()` and `y_labels[j].shares_differ()` |
| `tensor_pre.rs (mask_inputs)` | `sharing.rs (InputSharing::shares_differ)` | method call | WIRED | `src/tensor_pre.rs:109,119` — 4 `shares_differ()` calls |
| `bcot.rs (transfer_a_to_b, transfer_b_to_a)` | `keys.rs (Key::new)` | `Key::new(Block::random(&mut self.rng))` | WIRED | `src/bcot.rs:68,92` — two occurrences |
| `auth_tensor_gen.rs, tensor_gen.rs` | `tensor_ops.rs (pub(crate) fns)` | in-crate `use crate::tensor_ops::{...}` | WIRED | `cargo build --lib` succeeds with 0 errors, confirming in-crate callers compile against `pub(crate)` items |
| `auth_tensor_eval.rs, tensor_eval.rs, auth_tensor_gen.rs, tensor_gen.rs, tensor_ops.rs` | `matrix.rs (MatrixViewRef, MatrixViewMut)` | in-crate `use crate::matrix::{...}` | WIRED | `cargo build --lib` succeeds with 0 errors |

### Data-Flow Trace (Level 4)

Not applicable — phase makes no behavioral/data-flow changes. All modifications are construction idiom consolidation, documentation, and visibility narrowing. No new dynamic data flows introduced.

### Behavioral Spot-Checks

| Behavior | Result | Status |
|----------|--------|--------|
| `cargo build --lib` exits 0 | `Finished dev profile` — 0 errors, 8 pre-existing unreachable-pub warnings | PASS |
| No `.bit() as usize` anywhere in `src/` | `grep -rn "\.bit\(\) as usize" src/` returns 0 matches | PASS |
| `Key::new(Block::random` appears exactly 3 times across codebase (keys.rs via random, sharing.rs build_share, bcot.rs x2) | `keys.rs:102`, `sharing.rs:125`, `bcot.rs:68`, `bcot.rs:92` — 4 occurrences (keys.rs internal delegation counts separately) | PASS |
| Old `k0_block.set_lsb(false)` pattern absent from `bcot.rs` | 0 matches | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CLEAN-01 | 01-PLAN-keys-sharing, 01-PLAN-bcot-migration | Enforce `Key.lsb()==0` invariant at type level (construction-time guarantee) | SATISFIED | `Key::new` clears LSB; `Key::random` delegates to `Key::new`; `build_share` uses `Key::new`; `bcot.rs` both transfer methods use `Key::new` |
| CLEAN-02 | 01-PLAN-keys-sharing | Clarify `AuthBitShare` vs `AuthBit` with doc comments | SATISFIED | `AuthBitShare` doc: "One party's view"; `AuthBit` doc: "Both parties' views paired together" |
| CLEAN-03 | 01-PLAN-keys-sharing | Rename/document confusing `InputSharing.bit()` | SATISFIED | Hard-renamed to `shares_differ()`; old method deleted; all 4 call sites migrated |
| CLEAN-04 | 01-PLAN-keys-sharing | Fix `build_share` ignoring Key LSB=0 invariant | SATISFIED | `build_share` now uses `Key::new(Block::random(rng))` |
| CLEAN-05 | 01-PLAN-matrix-ops-aes | Audit matrix/tensor_ops visibility; document column-major indexing | SATISFIED | `MatrixViewRef`, `MatrixViewMut`, `gen_populate_seeds_mem_optimized`, `gen_unary_outer_product` all `pub(crate)`; column-major doc on `TypedMatrix` and `flat_index` |
| CLEAN-06 | 01-PLAN-matrix-ops-aes | Document `aes.rs` `once_cell::Lazy` singleton and thread-safety | SATISFIED | 22-line doc on `FIXED_KEY_AES` covers `Lazy` rationale, `Send + Sync`, protocol-constant status |

**Orphaned requirements check:** REQUIREMENTS.md maps CLEAN-01 through CLEAN-06 to Phase 1 — all 6 are claimed by the three plans and all 6 are satisfied. No orphaned requirements.

**Out-of-scope (Phase 2+):** CLEAN-07 through CLEAN-12 are mapped to Phase 2. PROTO-* and TEST-* are Phase 3+. Not verified here.

### Anti-Patterns Found

No blockers or warnings found in modified files. Notes:

| File | Pattern | Severity | Assessment |
|------|---------|----------|------------|
| `src/bcot.rs:20` | `// TODO: Replace with a real OT protocol` | INFO | Pre-existing acknowledged TODO, out of scope (v2 requirement per REQUIREMENTS.md). Does not affect Phase 1 goal. |
| `src/matrix.rs` (tests) | `Key::from(Block::new(...))` in test helpers | INFO | In test code only; intentionally uses zero-cost cast as test data setup. Not a stub — keys are used for index/XOR tests, not protocol mac operations. |

### Human Verification Required

None — all Phase 1 success criteria are structural (code pattern, visibility, documentation) and verifiable programmatically. Build success and grep checks confirm all criteria.

### Gaps Summary

No gaps. All 14 success criteria pass. All 6 Phase 1 requirement IDs (CLEAN-01 through CLEAN-06) are fully satisfied with direct code evidence. The `cargo build --lib` exits 0. The 4 pre-existing test failures in `leaky_tensor_pre` and `auth_tensor_pre` are confirmed known-broken from before Phase 1 (base commit `db6bd2c`) and are explicitly out of scope (Phase 3-6 work per REQUIREMENTS.md).

---

_Verified: 2026-04-21T22:00:00Z_
_Verifier: Claude (gsd-verifier)_
