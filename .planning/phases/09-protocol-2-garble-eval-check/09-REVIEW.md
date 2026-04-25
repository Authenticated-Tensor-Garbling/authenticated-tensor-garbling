---
phase: 09-protocol-2-garble-eval-check
reviewed: 2026-04-25T02:43:57Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - src/auth_tensor_eval.rs
  - src/auth_tensor_fpre.rs
  - src/auth_tensor_gen.rs
  - src/auth_tensor_pre.rs
  - src/lib.rs
  - src/preprocessing.rs
  - src/tensor_ops.rs
findings:
  critical: 0
  warning: 2
  info: 4
  total: 6
status: issues_found
---

# Phase 9: Code Review Report

**Reviewed:** 2026-04-25T02:43:57Z
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Phase 9 adds the Protocol 2 garble/eval/check path: wide GGM leaf expansion
(`gen_unary_outer_product_wide` / `eval_unary_outer_product_wide` in
`tensor_ops.rs`), three new D_ev preprocessing fields plus the
`gamma_d_ev_shares` rename in `preprocessing.rs`, `_p2`-suffixed methods on
`AuthTensorGen` and `AuthTensorEval`, and an end-to-end Protocol 2 test in
`lib.rs`.

The implementation is correct for the honest-party use cases exercised by the
test suite. Two warnings are raised:

1. **Double-call silent corruption**: `evaluate_final` and `evaluate_final_p2`
   (and their garbler-side counterparts) both mutate `first_half_out` with the
   same D_gb XOR loop, but no guard prevents calling both in sequence. Doing so
   silently cancels the D_gb accumulation (XOR self-inverse), producing
   incorrect output without any panic.

2. **Narrow vs wide tweak aliasing**: the narrow leaf-expansion tweak
   (`seeds.len() * j + i`) and the wide kappa-half tweak
   (`(seeds.len() * j + i) << 1`) share the same codomain for even indices,
   meaning the narrow function and the wide kappa path can produce identical
   TCCR inputs for some `(j, i)` pair if the same seeds are fed to both. This
   does not affect current tests (P1 and P2 are never mixed on the same
   `AuthTensor{Gen,Eval}` instance) but the functions lack any type-level or
   runtime barrier preventing such misuse.

No security vulnerabilities were found. The cryptographic primitives (TCCR,
IT-MAC, `check_zero`) are used as designed. The IT-MAC invariant is correctly
maintained through all four D_ev field pairs. The Protocol 2 consistency check
correctly passes `delta_b` (not `delta_a`) to `check_zero`.

---

## Warnings

### WR-01: No guard against calling both `evaluate_final` and `evaluate_final_p2` on the same instance

**File:** `src/auth_tensor_eval.rs:291-300` and `src/auth_tensor_eval.rs:362-370`

**Issue:** `evaluate_final` (Protocol 1) and `evaluate_final_p2` (Protocol 2)
each contain an identical D_gb combination loop that XORs the correlated MAC
into `first_half_out[(i, j)]`. If a caller invokes `evaluate_final()` and then
`evaluate_final_p2()` (or vice versa) on the same `AuthTensorEval` instance,
`first_half_out` is XOR'd with the same value twice, cancelling the
accumulation entirely and leaving `first_half_out` in the state it was in
before either call. `final_computed` is set to `true` by both methods, so the
downstream `compute_lambda_gamma` assertion provides no protection — it will
silently return incorrect results.

The same pattern exists on the garbler side: `garble_final` (line 292) and
`garble_final_p2` (line 385) in `src/auth_tensor_gen.rs` have the same
structural issue.

**Fix:** Add a guard at the top of each `*_final*` method that asserts
`!final_computed`. This makes double-call an immediate explicit panic rather
than a silent data corruption:

```rust
// In evaluate_final(), evaluate_final_p2(), garble_final(), garble_final_p2():
assert!(
    !self.final_computed,
    "evaluate_final called twice on the same instance — \
     first_half_out would be double-XOR'd; create a new instance per gate"
);
```

Alternatively, if calling both on the same instance is genuinely never
intended, document that invariant with a type-state pattern (separate
`Garbled` / `Evaluated` marker types). For the current codebase, the
`assert!(!final_computed)` is the simplest fix.

---

### WR-02: Narrow and wide leaf-expansion tweaks alias in the same u128 codomain

**File:** `src/tensor_ops.rs:97` and `src/tensor_ops.rs:300-304`

**Issue:** The narrow `gen_unary_outer_product` uses tweak
`= (seeds.len() * j + i) as u128` (values 0, 1, 2, ...). The wide
`gen_unary_outer_product_wide` uses `base = (seeds.len() * j + i) as u128;
kappa = cipher.tccr(Block::from(base << 1), seeds[i])` (values 0, 2, 4, ...).
The narrow function's tweaks and the wide function's kappa-half tweaks share
even values in the same domain. For any `(j, i)` pair where the narrow tweak
equals a wide kappa tweak for some other `(j', i')`, and the same leaf seed
appears at both positions, the TCCR outputs would be identical — breaking PRF
output independence between the two function families.

This is not exploited by the current test suite (P1 and P2 paths are never
invoked simultaneously on the same `AuthTensor*` instance), but the functions
lack any domain-separation identifier (such as a protocol-version prefix bit in
the tweak) that would prevent future misuse.

**Fix:** Add a domain-separation constant to the narrow tweak so narrow tweaks
are disjoint from wide tweaks. One clean approach: namespace wide tweaks with
a high bit that is never set by the narrow convention:

```rust
// In gen_unary_outer_product (narrow): keep as-is (tweaks are 0..max)
let tweak = (seeds.len() * j + i) as u128;

// In gen_unary_outer_product_wide: prepend a domain tag in bits 127..64
// so that no wide tweak can equal any narrow tweak:
const WIDE_DOMAIN: u128 = 1u128 << 64;
let s_gb = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1)),     seeds[i]);
let s_ev = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i]);
```

This must also be mirrored in `eval_unary_outer_product_wide` and in the
corresponding evaluator-side tweak computations to keep gen/eval consistent.
Note: if P1 and P2 invocations are guaranteed never to share a seed vector
(which is currently true by construction), this is defense-in-depth rather than
a fix for an active bug.

---

## Info

### IN-01: `debug_assert` for dimension checks in `eval_populate_seeds_mem_optimized` and wide variants — elided in release builds

**File:** `src/tensor_ops.rs:143`, `src/tensor_ops.rs:228-233`,
`src/tensor_ops.rs:290-292`, `src/tensor_ops.rs:344-348`

**Issue:** The dimension consistency checks in the inner `tensor_ops`
functions use `debug_assert_eq!` / `debug_assert!`, which are compiled out in
release (`--release`) builds. If a caller passes mismatched `seeds.len()` vs
`gen_cts.len()` (for example), the release build will silently read out of
bounds or produce garbage output rather than panicking.

**Fix:** Upgrade the critical preconditions (especially `seeds[missing] ==
Block::default()` and `gen_cts.len() == m`) to `assert!` so they fire in
release builds too:

```rust
// In eval_unary_outer_product_wide (line 344):
assert_eq!(seeds[missing], Block::default(),
    "seeds[missing] must be Block::default() sentinel");
// In eval_unary_outer_product (line 228):
assert_eq!(seeds[missing], Block::default(),
    "seeds[missing] must be Block::default() sentinel");
```

The length/shape checks (`a_bits.len() == n`, `gen_cts.len() == m`) are lower
priority since they panic on OOB access anyway, but upgrading them to
`assert_eq!` improves diagnostics.

---

### IN-02: `IdealPreprocessingBackend` uses fixed seed 0 for `TensorFpre` but generates D_ev bits from separate seeded RNGs — seed documentation gap

**File:** `src/preprocessing.rs:153-195`

**Issue:** `IdealPreprocessingBackend::run` creates `TensorFpre::new(0, ...)` (fixed seed) and then generates D_ev bits using four separate `ChaCha12Rng` instances seeded with constants 43, 44, 45, 42. The choice of these exact constants is not documented (e.g., why 42 for gamma and not 45; why the ordering 43/44/45/42). A future maintainer adding a fifth D_ev field or reordering the generation might accidentally reuse a seed, making two fields correlated.

**Fix:** Document the seed constants explicitly and/or use a named enum or array:

```rust
// Clear documentation of seed allocation:
// Seed 42: gamma_d_ev_shares (l_gamma, gate output mask) — established in Phase 7
// Seed 43: alpha_d_ev_shares (l_alpha, row input mask)   — Phase 9 D-06
// Seed 44: beta_d_ev_shares  (l_beta, column input mask) — Phase 9 D-06
// Seed 45: correlated_d_ev_shares (l_gamma*, corr bit)   — Phase 9 D-06
// Next available seed: 46
```

This is documentation only; no functional change required.

---

### IN-03: TODO comment in `auth_tensor_fpre.rs` line 1

**File:** `src/auth_tensor_fpre.rs:1`

**Issue:** `// TODO refactor authbit from fpre to a common module, or redefine with new name.`

This is a leftover TODO that has not been addressed across multiple phases. It
indicates a known architectural debt (the `AuthBit` type living in `fpre`
rather than a dedicated `sharing` module).

**Fix:** Either address the refactor or file it as a tracked issue and remove
the in-code TODO. If it is genuinely deferred, a `// FUTURE:` or `// DEBT:`
prefix is more informative than `// TODO`.

---

### IN-04: `generate_for_ideal_trusted_dealer` asserts `n <= usize::BITS - 1` but `gen_populate_seeds_mem_optimized` has no corresponding guard

**File:** `src/auth_tensor_fpre.rs:92-100` vs `src/tensor_ops.rs:22`

**Issue:** `generate_for_ideal_trusted_dealer` asserts `n <= usize::BITS - 1`
to ensure bit-manipulation safety on the input values `x` and `y`, but
`gen_populate_seeds_mem_optimized` (and its eval counterpart) allocate
`vec![Block::default(); 1 << n]` with no guard on `n`. For `n >= 64` on a
64-bit platform, `1 << n` overflows `usize`, causing either a panic or an
allocation of 0 bytes (silent wrap) depending on the allocator. Similarly,
the chunking loops in `auth_tensor_gen.rs` and `auth_tensor_eval.rs` process
slices of size up to `chunking_factor` where `chunking_factor` can equal up to
`n`.

**Fix:** Add a check in `gen_populate_seeds_mem_optimized` (or the calling
`gen_chunked_half_outer_product`) that `x.len() <= 63` (for 64-bit) before
the allocation:

```rust
assert!(
    x.len() <= 63,
    "gen_populate_seeds_mem_optimized: n={} exceeds max supported 63 \
     (1<<n would overflow usize on a 64-bit platform)",
    x.len()
);
```

In practice the chunking factor is 1–8 per the bench documentation, so this
is a latent rather than active bug, but the allocation can panic in unexpected
ways on untrusted input.

---

_Reviewed: 2026-04-25T02:43:57Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
