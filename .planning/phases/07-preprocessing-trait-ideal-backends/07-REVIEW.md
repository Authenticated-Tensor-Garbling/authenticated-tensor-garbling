---
phase: 07-preprocessing-trait-ideal-backends
reviewed: 2026-04-24T01:16:29Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - src/auth_tensor_eval.rs
  - src/auth_tensor_fpre.rs
  - src/auth_tensor_gen.rs
  - src/auth_tensor_pre.rs
  - src/preprocessing.rs
findings:
  critical: 0
  warning: 3
  info: 3
  total: 6
status: issues_found
---

# Phase 7: Code Review Report

**Reviewed:** 2026-04-24T01:16:29Z
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Phase 7 delivers the `TensorPreprocessing` trait, `UncompressedPreprocessingBackend`, `IdealPreprocessingBackend`, and the new `gamma_auth_bit_shares` field across `TensorFpreGen`/`TensorFpreEval`. The implementation is structurally sound and correct on the main protocol paths. No security vulnerabilities or data-loss bugs were found.

Three warnings were identified: a vacuous test assertion that will never fail regardless of implementation correctness; unenforced `count` semantics in `IdealPreprocessingBackend` that silently under-delivers when `count > 1`; and unchecked integer shift-overflow paths in `generate_for_ideal_trusted_dealer` for `n`/`m` >= 64. Three informational items cover code style and dead public API.

## Warnings

### WR-01: Vacuous assertion in `test_ideal_backend_gamma_distinct_from_correlated` never fails

**File:** `src/preprocessing.rs:353-356`
**Issue:** The `assert_ne!` checks that `gen_out.gamma_auth_bit_shares[0].value as u8 + gen_out.correlated_auth_bit_shares[0].value as u8 != 255u8`. Because `bool as u8` is always 0 or 1, the sum is always 0, 1, or 2 — never 255. The assertion is unconditionally true and tests nothing about the distinctness or correctness of the `gamma_auth_bit_shares` values. A regression that sets all gamma bits to the same constant as the correlated bits would silently pass this test.
**Fix:** Replace the vacuous assertion with one that actually verifies the independence between `l_gamma` and `l_gamma*` at a fixed seed. Since the seed is deterministic (ChaCha12 seed 42 for gamma, seed 0 for the fpre rng), the exact values are stable across runs:

```rust
// Replace the assert_ne!(sum, 255u8) block with:
// Verify that gamma and correlated shares are not byte-for-byte identical
// (independent random samples from different RNG seeds must differ).
let gamma_bits: Vec<bool> = gen_out.gamma_auth_bit_shares.iter()
    .map(|s| s.value)
    .collect();
let correlated_bits: Vec<bool> = gen_out.correlated_auth_bit_shares.iter()
    .map(|s| s.value)
    .collect();
assert_ne!(
    gamma_bits,
    correlated_bits,
    "gamma and correlated auth bit shares must be independently sampled (different RNG seeds)"
);
```

---

### WR-02: `IdealPreprocessingBackend::run` silently ignores `count > 1`, returning only 1 triple

**File:** `src/preprocessing.rs:126-156`
**Issue:** The `TensorPreprocessing` trait's `run` signature accepts `count: usize`, semantically meaning "generate `count` triples." `IdealPreprocessingBackend::run` silently discards `count` (line 129: `let _ = count;`) and always returns one pair. A caller passing `count = 5` receives data for 1 triple, not 5. Unlike `UncompressedPreprocessingBackend` (which panics on `count != 1` with a clear message), the ideal backend gives no signal. This breaks the principle of failing loudly.
**Fix:** Add an explicit panic matching the uncompressed backend's pattern, or document the constraint in the trait:

```rust
// At the top of IdealPreprocessingBackend::run:
assert_eq!(
    count, 1,
    "IdealPreprocessingBackend::run: count > 1 is not yet supported; \
     the ideal backend returns exactly one (TensorFpreGen, TensorFpreEval) pair. \
     Use a loop calling run(n, m, 1, cf) for batch use."
);
let _ = count;
```

---

### WR-03: Integer shift overflow in `generate_for_ideal_trusted_dealer` when `n` or `m` >= 64

**File:** `src/auth_tensor_fpre.rs:101,109,128,136`
**Issue:** The function accumulates bits with `alpha |= (alpha_bit as usize) << i` and extracts input bits with `((1<<i & x) != 0)`. On a 64-bit platform, `usize` is 64 bits wide. If `self.n` or `self.m` is 64 or larger, the shift `1usize << 64` (when `i == 64`) causes a panic in debug builds and is undefined behavior in release builds. The function signature takes `x: usize`, which already limits meaningful input to 64 bits, but there is no guard preventing callers from passing `n = 64` or larger.
**Fix:** Add a precondition check at the top of `generate_for_ideal_trusted_dealer`:

```rust
assert!(
    self.n <= usize::BITS as usize - 1,
    "generate_for_ideal_trusted_dealer: n={} exceeds usize bit width minus 1; \
     x must be representable as usize", self.n
);
assert!(
    self.m <= usize::BITS as usize - 1,
    "generate_for_ideal_trusted_dealer: m={} exceeds usize bit width minus 1; \
     y must be representable as usize", self.m
);
```

Alternatively, replace the shift-based bit extraction with a method that does not overflow, e.g., `x.wrapping_shr(i as u32) & 1 != 0`, which is safe for all values of `i < usize::BITS`.

## Info

### IN-01: `gamma_auth_bit_shares` silently dropped in `new_from_fpre_gen` and `new_from_fpre_eval`

**File:** `src/auth_tensor_gen.rs:64`, `src/auth_tensor_eval.rs:57`
**Issue:** Both constructors consume a `TensorFpreGen`/`TensorFpreEval` by value and forward all fields except `gamma_auth_bit_shares`, which is silently dropped. The TODO comments (`// TODO(Phase 8): forward fpre_*.gamma_auth_bit_shares`) correctly flag the intent. Noted here for tracking: if the online phase begins consuming `gamma_auth_bit_shares` before Phase 8 lands, the data will be quietly discarded with no compiler error.
**Fix:** No change required now. Ensure the Phase 8 plan adds `gamma_auth_bit_shares` to `AuthTensorGen`/`AuthTensorEval` struct definitions and forwards the field in both constructors before any online-phase code references it.

---

### IN-02: `generate_for_ideal_trusted_dealer` return value silently discarded at two test call sites

**File:** `src/auth_tensor_fpre.rs:221`, `src/auth_tensor_fpre.rs:259`
**Issue:** Both test calls discard the returned `(alpha, beta)` tuple without a binding. In `test_tensor_fpre_auth_bits` (line 221) and `test_tensor_fpre_input_sharings` (line 259), the returned masking values are not checked or used. This is not a correctness bug (the tests verify other properties), but it creates the misleading impression that the return value is unimportant and makes the function signature harder to understand. The same pattern recurs in `preprocessing.rs` line 132, where discarding is intentional (inputs are zero).
**Fix:** Either annotate the function with `#[must_use]` to generate a compiler warning when the return is dropped, or use an explicit discard where intentional:

```rust
// In tests where the return is not needed:
let _masks = fpre.generate_for_ideal_trusted_dealer(0b101, 0b110);

// On the function itself, to make discarding visible to callers:
#[must_use = "returns (alpha, beta) masking values used for label correctness checks"]
pub fn generate_for_ideal_trusted_dealer(&mut self, x: usize, y: usize) -> (usize, usize) {
```

---

### IN-03: `AuthTensorGen::new` and `AuthTensorEval::new` are public but produce unusable state

**File:** `src/auth_tensor_gen.rs:35-49`, `src/auth_tensor_eval.rs:28-43`
**Issue:** Both `pub fn new(...)` constructors initialize `x_labels`, `y_labels`, `alpha_auth_bit_shares`, `beta_auth_bit_shares`, and `correlated_auth_bit_shares` as empty `Vec::new()`. Calling any of `garble_first_half`, `garble_second_half`, `garble_final`, `evaluate_first_half`, `evaluate_second_half`, or `evaluate_final` on a struct built with `new()` will index-panic immediately. These constructors are never called within the project (all internal uses go through `new_from_fpre_gen` / `new_from_fpre_eval`). Making them public exposes a footgun API to external consumers.
**Fix:** Either make the constructors `pub(crate)` to limit their exposure, or document the precondition that all `Vec` fields must be populated before the garble/evaluate methods are called:

```rust
// Option A: restrict visibility
pub(crate) fn new(n: usize, m: usize, chunking_factor: usize) -> Self { ... }

// Option B: add a doc comment warning
/// Creates an `AuthTensorGen` with empty label and share vectors.
///
/// # Panics
///
/// All garble/evaluate methods will panic if called on an instance
/// created via `new()` — labels and share vectors must be populated first.
/// Prefer `AuthTensorGen::new_from_fpre_gen(fpre_gen)` instead.
pub fn new(n: usize, m: usize, chunking_factor: usize) -> Self { ... }
```

---

_Reviewed: 2026-04-24T01:16:29Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
