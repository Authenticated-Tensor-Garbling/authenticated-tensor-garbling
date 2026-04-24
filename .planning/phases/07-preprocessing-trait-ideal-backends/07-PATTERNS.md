# Phase 7: Preprocessing Trait + Ideal Backends - Pattern Map

**Mapped:** 2026-04-23
**Files analyzed:** 4 files modified (no new files)
**Analogs found:** 4 / 4

---

## File Classification

| Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------|------|-----------|----------------|---------------|
| `src/preprocessing.rs` | trait-def + service + model | batch / transform | `src/auth_tensor_fpre.rs` (struct + impl pattern) | role-match |
| `src/auth_tensor_fpre.rs` | service (trusted dealer) | transform | itself — `into_gen_eval()` must be extended | exact |
| `src/auth_tensor_pre.rs` | service (combiner) | transform | itself — `combine_leaky_triples` return site | exact |
| `src/auth_tensor_gen.rs` + `src/auth_tensor_eval.rs` | model (consumer) | request-response | themselves — `new_from_fpre_gen/eval` field-forward pattern | exact |

---

## Pattern Assignments

### `src/preprocessing.rs` — TensorPreprocessing trait + two backend structs

**Changes:** Add `TensorPreprocessing` trait (PRE-01). Add `UncompressedPreprocessingBackend` (PRE-03). Add `IdealPreprocessingBackend` (PRE-02). Add `gamma_auth_bit_shares` field to `TensorFpreGen` and `TensorFpreEval` (PRE-04).

**Analog for zero-field struct + trait impl pattern:** `src/bcot.rs` (IdealBCot) for overall struct shape; `src/auth_tensor_fpre.rs` for delegation style.

**Imports pattern** (`src/preprocessing.rs` lines 7-10, extended for Phase 7):
```rust
// EXISTING (lines 7-10):
use crate::{block::Block, delta::Delta, sharing::AuthBitShare};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};

// ADD for IdealPreprocessingBackend:
use crate::auth_tensor_fpre::TensorFpre;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
// Note: rand::Rng is already in scope via auth_tensor_fpre.rs pattern
```

**PRE-04 field addition — TensorFpreGen** (`src/preprocessing.rs` lines 12-34, new field appended):
```rust
// EXISTING struct body; ADD one field at the end (same position on both structs):
pub struct TensorFpreGen {
    // ... all existing fields unchanged (lines 13-33) ...
    /// Garbler's `AuthBitShare` for each gate-output mask l_gamma per (i,j) pair;
    /// length n*m, column-major index j*n + i. MAC committed under delta_b.
    /// Distinct from `correlated_auth_bit_shares` (which encodes l_gamma*).
    /// Populated by IdealPreprocessingBackend; initialized to vec![] by
    /// UncompressedPreprocessingBackend (Phase 8 will fill in the real value).
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}
```

**PRE-04 field addition — TensorFpreEval** (`src/preprocessing.rs` lines 36-58, new field appended):
```rust
pub struct TensorFpreEval {
    // ... all existing fields unchanged (lines 37-57) ...
    /// Evaluator's `AuthBitShare` for each gate-output mask l_gamma (column-major, length n*m,
    /// index j*n + i). MAC committed under delta_a. Symmetric to TensorFpreGen.
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}
```

**PRE-01 trait definition** (new, after the struct definitions):
```rust
/// Abstraction over preprocessing backends.
///
/// Zero-field struct implementors (unit structs with no state) implement this
/// trait. Using `&self` even though no data is read — this makes the trait
/// object-safe (usable as `dyn TensorPreprocessing`) and consistent with the
/// codebase's `&self` / `&mut self` method convention.
pub trait TensorPreprocessing {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval);
}
```

**PRE-03 UncompressedPreprocessingBackend** (new, after the trait):
```rust
/// Backend that wraps the real two-party uncompressed preprocessing protocol.
///
/// Callers should use `UncompressedPreprocessingBackend.run(n, m, 1, cf)` instead
/// of calling `run_preprocessing` directly.
pub struct UncompressedPreprocessingBackend;

impl TensorPreprocessing for UncompressedPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        run_preprocessing(n, m, count, chunking_factor)
    }
}
```

**PRE-02 IdealPreprocessingBackend** (new, after UncompressedPreprocessingBackend):

The ordering constraint from RESEARCH.md Pattern 3 — `into_gen_eval(self)` consumes `fpre` by value — means all `gen_auth_bit()` calls must happen BEFORE `into_gen_eval()`.

```rust
/// Backend that uses an ideal trusted-dealer oracle (in-process, not secure).
///
/// Fixed seed 0 — matches the IdealBCot pattern (src/bcot.rs IdealBCot::new(0, 1)).
/// Use for tests and benchmarks only.
pub struct IdealPreprocessingBackend;

impl TensorPreprocessing for IdealPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        let _ = count; // IdealPreprocessingBackend always returns one triple
        let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
        fpre.generate_for_ideal_trusted_dealer(0, 0);

        // Generate n*m independent authenticated bits for l_gamma BEFORE consuming fpre.
        // CRITICAL: into_gen_eval(self) takes fpre by value; gen_auth_bit() calls must
        // precede it. See RESEARCH.md Pitfall 2.
        let mut rng = ChaCha12Rng::seed_from_u64(42);
        let mut gamma_auth_bits = Vec::with_capacity(n * m);
        for _ in 0..(n * m) {
            let l_gamma: bool = rng.random_bool(0.5);
            gamma_auth_bits.push(fpre.gen_auth_bit(l_gamma));
        }

        let (mut gen, mut eval) = fpre.into_gen_eval();

        // Distribute gen_share / eval_share for gamma_auth_bit_shares.
        gen.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.gen_share).collect();
        eval.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.eval_share).collect();

        (gen, eval)
    }
}
```

**Tests pattern** (`src/preprocessing.rs` lines 119-146 — existing tests; new tests appended):

Follow the existing test co-location style. New tests live in the same `#[cfg(test)] mod tests` block at the bottom of `src/preprocessing.rs`. Structural precedent from `src/auth_tensor_pre.rs` lines 336-728.

```rust
// Pattern: use verify_cross_party from auth_tensor_pre — import it in the test mod.
// Pattern for gamma_auth_bit_shares test:
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_pre::verify_cross_party; // re-use existing cross-party verifier

    #[test]
    fn test_ideal_backend_gamma_auth_bit_shares_mac_invariant() {
        let backend = IdealPreprocessingBackend;
        let (gen, eval) = backend.run(4, 4, 1, 1);
        assert_eq!(gen.gamma_auth_bit_shares.len(), 4 * 4);
        assert_eq!(eval.gamma_auth_bit_shares.len(), 4 * 4);
        for k in 0..(4 * 4) {
            verify_cross_party(
                &gen.gamma_auth_bit_shares[k],
                &eval.gamma_auth_bit_shares[k],
                &gen.delta_a,
                &eval.delta_b,
            );
        }
    }
    // ... additional tests for trait dispatch, UncompressedPreprocessingBackend, etc.
}
```

---

### `src/auth_tensor_fpre.rs` — TensorFpre::into_gen_eval() constructor update

**Change:** Both struct literals inside `into_gen_eval()` must initialize `gamma_auth_bit_shares`.

**Analog:** The existing field initialization pattern for `correlated_auth_bit_shares` (lines 169, 179) is the exact pattern to replicate.

**Core constructor pattern** (`src/auth_tensor_fpre.rs` lines 158-181, must add one field to each struct literal):
```rust
// EXISTING — lines 158-181 (both struct literals must get one new field):
pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {
    (TensorFpreGen {
        n: self.n,
        m: self.m,
        chunking_factor: self.chunking_factor,
        delta_a: self.delta_a,
        alpha_labels: self.x_labels.iter().map(|share| share.gen_share).collect(),
        beta_labels: self.y_labels.iter().map(|share| share.gen_share).collect(),
        alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.gen_share).collect(),
        beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.gen_share).collect(),
        correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.gen_share).collect(),
        // ADD THIS LINE (same pattern as the line above):
        gamma_auth_bit_shares: vec![],
        // NOTE: vec![] is correct here — the ideal dealer (IdealPreprocessingBackend)
        // generates gamma bits externally before calling into_gen_eval, then overwrites this.
        // The uncompressed path also starts with vec![].
    }, TensorFpreEval {
        n: self.n,
        m: self.m,
        chunking_factor: self.chunking_factor,
        delta_b: self.delta_b,
        alpha_labels: self.x_labels.iter().map(|share| share.eval_share).collect(),
        beta_labels: self.y_labels.iter().map(|share| share.eval_share).collect(),
        alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.eval_share).collect(),
        beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.eval_share).collect(),
        correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.eval_share).collect(),
        // ADD THIS LINE (symmetric):
        gamma_auth_bit_shares: vec![],
    })
}
```

**gen_auth_bit() reference** (`src/auth_tensor_fpre.rs` lines 66-86, used as-is by IdealPreprocessingBackend):
```rust
pub fn gen_auth_bit(&mut self, x: bool) -> AuthBit {
    let a = self.rng.random_bool(0.5);
    let b = x ^ a;
    let a_share = build_share(&mut self.rng, a, &self.delta_b);
    let b_share = build_share(&mut self.rng, b, &self.delta_a);
    AuthBit {
        gen_share: AuthBitShare { key: b_share.key, mac: a_share.mac, value: a },
        eval_share: AuthBitShare { key: a_share.key, mac: b_share.mac, value: b },
    }
}
```

Do NOT call this method after `into_gen_eval()` — it consumes `self`. Always collect `gen_auth_bit()` results first, then call `into_gen_eval()`.

---

### `src/auth_tensor_pre.rs` — combine_leaky_triples() return site

**Change:** Both `TensorFpreGen` and `TensorFpreEval` struct literals at lines 231-253 must include `gamma_auth_bit_shares: vec![]`.

**Analog:** The comment at line 229 ("Labels stubbed to Vec::new() per Phase 4 D-07") is the exact pattern — same stub strategy for `gamma_auth_bit_shares` in Phase 7.

**Struct literal site** (`src/auth_tensor_pre.rs` lines 231-253):
```rust
// EXISTING (both struct literals gain one field):
(
    TensorFpreGen {
        n,
        m,
        chunking_factor,
        delta_a,
        alpha_labels: Vec::new(),      // stub per Phase 4 D-07
        beta_labels: Vec::new(),       // stub per Phase 4 D-07
        alpha_auth_bit_shares: acc.gen_x_shares,
        beta_auth_bit_shares: acc.gen_y_shares,
        correlated_auth_bit_shares: acc.gen_z_shares,
        // ADD: stub matching the pattern for alpha_labels / beta_labels above
        gamma_auth_bit_shares: vec![], // stub: uncompressed path does not generate l_gamma yet
    },
    TensorFpreEval {
        n,
        m,
        chunking_factor,
        delta_b,
        alpha_labels: Vec::new(),
        beta_labels: Vec::new(),
        alpha_auth_bit_shares: acc.eval_x_shares,
        beta_auth_bit_shares: acc.eval_y_shares,
        correlated_auth_bit_shares: acc.eval_z_shares,
        // ADD (symmetric):
        gamma_auth_bit_shares: vec![],
    },
)
```

---

### `src/auth_tensor_gen.rs` + `src/auth_tensor_eval.rs` — downstream consumer check

**Change:** `new_from_fpre_gen()` and `new_from_fpre_eval()` do NOT require modification for compilation (Rust does not require exhaustive field access on a struct being moved field-by-field). However, `gamma_auth_bit_shares` will be silently dropped unless forwarded.

**Decision per RESEARCH.md open question 2 (planner must resolve):** Either (a) add `gamma_auth_bit_shares` to `AuthTensorGen`/`AuthTensorEval` now and forward it, or (b) add a `// TODO(Phase 8): forward gamma_auth_bit_shares` comment at the drop site.

**Pattern to copy if forwarding (option a) — use the existing field-forward pattern:**

From `src/auth_tensor_gen.rs` lines 52-67:
```rust
pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
    Self {
        cipher: &(*FIXED_KEY_AES),
        n: fpre_gen.n,
        m: fpre_gen.m,
        chunking_factor: fpre_gen.chunking_factor,
        delta_a: fpre_gen.delta_a,
        x_labels: fpre_gen.alpha_labels,
        y_labels: fpre_gen.beta_labels,
        alpha_auth_bit_shares: fpre_gen.alpha_auth_bit_shares,
        beta_auth_bit_shares: fpre_gen.beta_auth_bit_shares,
        correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
        // ADD if forwarding: gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
        // (also add the field to AuthTensorGen struct definition above)
        first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
        second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
    }
}
```

From `src/auth_tensor_eval.rs` lines 45-60 (symmetric pattern):
```rust
pub fn new_from_fpre_eval(fpre_eval: TensorFpreEval) -> Self {
    Self {
        // ... all existing fields ...
        correlated_auth_bit_shares: fpre_eval.correlated_auth_bit_shares,
        // ADD if forwarding: gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,
        first_half_out: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
        second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
    }
}
```

**If not forwarding (option b):** The field is silently dropped by Rust's move semantics (no compile error). Add the TODO comment immediately after `correlated_auth_bit_shares` in each constructor.

---

## Shared Patterns

### Zero-Field Struct Backend Pattern
**Source:** `src/bcot.rs` — `IdealBCot::new(seed_a, seed_b)` pattern
**Apply to:** `UncompressedPreprocessingBackend` and `IdealPreprocessingBackend`
```rust
// From src/bcot.rs lines 21-55:
// IdealBCot has fields, but the STRUCTURAL lesson is:
//   - Seeded deterministic RNG: ChaCha12Rng::seed_from_u64(seed)
//   - Fixed seeds baked into the impl, not passed by callers
// For zero-field structs, go further — no struct fields at all:
pub struct UncompressedPreprocessingBackend;
pub struct IdealPreprocessingBackend;
// Fixed seed 0 used internally in IdealPreprocessingBackend::run()
// (matches IdealBCot::new(0, 1) precedent)
```

### Cross-Party MAC Verification
**Source:** `src/auth_tensor_pre.rs` lines 316-334 (`verify_cross_party` function)
**Apply to:** All tests that verify `gamma_auth_bit_shares` entries
```rust
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare {
        key: eval_share.key,
        mac: gen_share.mac,
        value: gen_share.value,
    }
    .verify(delta_b);
    AuthBitShare {
        key: gen_share.key,
        mac: eval_share.mac,
        value: eval_share.value,
    }
    .verify(delta_a);
}
```
WARNING: Do NOT call `share.verify(delta)` directly on a raw cross-party `AuthBitShare` — it panics on correctly-formed shares. Always use `verify_cross_party`.

### Deterministic RNG Seeding
**Source:** `src/bcot.rs` line 49, `src/auth_tensor_fpre.rs` line 26, `src/auth_tensor_pre.rs` lines 209
**Apply to:** All RNG initialization in `IdealPreprocessingBackend::run()` and any test helpers
```rust
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
// Pattern: always seed_from_u64 with a fixed literal, never rand::rng()
let mut rng = ChaCha12Rng::seed_from_u64(42);
```

### Vec Stub Pattern for Unimplemented Fields
**Source:** `src/auth_tensor_pre.rs` lines 235-236 (`alpha_labels: Vec::new()`)
**Apply to:** `gamma_auth_bit_shares: vec![]` in `combine_leaky_triples` and `into_gen_eval`
```rust
// Phase 4 precedent for stubbing not-yet-populated fields:
alpha_labels: Vec::new(),
beta_labels: Vec::new(),
// Phase 7 follows same pattern:
gamma_auth_bit_shares: vec![],
```

### AuthBit gen_share / eval_share Distribution
**Source:** `src/auth_tensor_fpre.rs` lines 158-181 (`into_gen_eval` — lines 166-169, 176-179)
**Apply to:** Distributing `gamma_auth_bits` in `IdealPreprocessingBackend::run()`
```rust
// Existing pattern for correlated_auth_bits (line 169):
correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.gen_share).collect(),
// Identical pattern for gamma_auth_bits:
gen.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.gen_share).collect();
eval.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.eval_share).collect();
```

### Co-Located Test Module
**Source:** `src/preprocessing.rs` lines 119-146, `src/auth_tensor_pre.rs` lines 336-728
**Apply to:** All new tests for Phase 7 — go in `#[cfg(test)] mod tests { ... }` at the bottom of `src/preprocessing.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    // Add for Phase 7:
    use crate::auth_tensor_pre::verify_cross_party;

    // Existing tests (lines 124-146) remain unchanged.
    // Append new tests below.
}
```

---

## No Analog Found

No files in this phase are truly greenfield — all changes are surgical extensions to existing files.

---

## Critical Sequencing Notes (for planner)

1. **Atomic field addition** (D-06): `gamma_auth_bit_shares` field must be added to `TensorFpreGen`/`TensorFpreEval` in the same commit that updates BOTH construction sites:
   - `src/auth_tensor_fpre.rs` — `into_gen_eval()` (lines 158-181)
   - `src/auth_tensor_pre.rs` — `combine_leaky_triples()` (lines 231-253)
   Missing either site causes a compile error: "missing field `gamma_auth_bit_shares` in initializer of `TensorFpreGen`".

2. **Ownership ordering** (RESEARCH.md Pitfall 2): `TensorFpre::into_gen_eval(self)` takes `self` by value (line 158). All `fpre.gen_auth_bit()` calls in `IdealPreprocessingBackend::run()` must complete before `fpre.into_gen_eval()` is called.

3. **`auth_tensor_gen.rs` / `auth_tensor_eval.rs`**: These files do NOT need changes for compilation (Rust does not require exhaustive field access on a moved struct). The planner must explicitly decide whether to forward `gamma_auth_bit_shares` now (for Phase 8 readiness) or defer with a TODO comment.

---

## Metadata

**Analog search scope:** `src/` (all 20 `.rs` files enumerated)
**Files scanned:** `src/preprocessing.rs`, `src/auth_tensor_fpre.rs`, `src/bcot.rs`, `src/auth_tensor_pre.rs`, `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`, `src/sharing.rs`, `src/lib.rs`
**Pattern extraction date:** 2026-04-23
