# Phase 7: Preprocessing Trait + Ideal Backends - Research

**Researched:** 2026-04-23
**Domain:** Rust trait design, authenticated garbling preprocessing abstraction
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** `TensorPreprocessing` trait lives in `src/preprocessing.rs`, alongside the existing `TensorFpreGen`, `TensorFpreEval`, and `run_preprocessing`. No new file.
- **D-02:** The existing `run_preprocessing` function is wrapped in a zero-field struct named `UncompressedPreprocessingBackend` that implements `TensorPreprocessing`.
- **D-03:** `IdealPreprocessingBackend` is a zero-field struct (unit struct). Fixed seed `0` is used internally — matches `IdealBCot` pattern; no caller configuration needed.
- **D-04:** `TensorFpreGen` and `TensorFpreEval` both get ONE new field: `gamma_auth_bit_shares: Vec<AuthBitShare>`. Length `n*m` per triple, column-major (same indexing as `correlated_auth_bit_shares`). Symmetric layout — same field name on both structs.
- **D-05:** `gamma_auth_bit_shares` holds D_ev-authenticated shares of **l_gamma** (the gate output mask), NOT l_gamma*. `correlated_auth_bit_shares` already encodes l_gamma*. REQUIREMENTS.md PRE-04 text contains an error: it says "l_gamma\*" where it should say "l_gamma".
- **D-06:** Every existing constructor of `TensorFpreGen` / `TensorFpreEval` must initialize `gamma_auth_bit_shares` in the same commit that adds the field — no intermediate broken state.
- **D-07:** `IdealPreprocessingBackend::run()` delegates to `TensorFpre` internally: creates a `TensorFpre`, calls `generate_for_ideal_trusted_dealer()`, then `into_gen_eval()` to produce the base struct pair. It then generates the `gamma_auth_bit_shares` field on top.
- **D-08:** `gamma_auth_bit_shares` in the ideal backend is a separate random authenticated bit per (i,j) pair — `l_gamma` is an independent random wire mask. The ideal dealer calls `TensorFpre::gen_auth_bit()` once per (i,j) with a freshly sampled random bit.
- **D-09:** IT-MAC invariant (`mac = key XOR bit * delta`) must hold for `gamma_auth_bit_shares` entries on both structs. Satisfied automatically by delegating to `gen_auth_bit()`.
- **D-10:** `IdealCompressedPreprocessingBackend` and PRE-05 are **deferred to v3**. Do not add any compressed-preprocessing scaffolding.

### Claude's Discretion

- Trait method signature: `fn run(n: usize, m: usize, count: usize, chunking_factor: usize) -> (TensorFpreGen, TensorFpreEval)` — use whatever Rust trait form (associated function vs `&self`) is most natural given the zero-field struct design. Can use `&self` for object-safety even though the struct holds no state.
- Count > 1 handling in `UncompressedPreprocessingBackend`: may retain the existing `assert_eq!(count, 1)` panic until a batch variant is implemented.

### Deferred Ideas (OUT OF SCOPE)

- PRE-05 / `IdealCompressedPreprocessingBackend` — deferred to v3. Do not add any compressed-preprocessing scaffolding.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PRE-01 | `TensorPreprocessing` trait defined with `run(n, m, count, chunking_factor) -> (TensorFpreGen, TensorFpreEval)` | Trait design patterns documented; zero-field struct impls analyzed |
| PRE-02 | `IdealPreprocessingBackend` implements `TensorPreprocessing` as trusted-dealer oracle | `TensorFpre` delegation pattern verified in `src/auth_tensor_fpre.rs` |
| PRE-03 | All existing preprocessing implementations satisfy `TensorPreprocessing` | `UncompressedPreprocessingBackend` wrapping `run_preprocessing` — wrapper pattern confirmed |
| PRE-04 | `TensorFpreGen` and `TensorFpreEval` extended with `gamma_auth_bit_shares` | Field layout, MAC invariant, constructor update scope — all analyzed |
</phase_requirements>

---

## Summary

Phase 7 is a pure Rust refactoring and extension phase — no new algorithms, no new cryptographic primitives, no new external dependencies. All work is within a single source module (`src/preprocessing.rs`) and two support modules (`src/auth_tensor_fpre.rs` for `TensorFpre::gen_auth_bit()`, `src/bcot.rs` for structural reference).

The three implementation tasks are tightly coupled: (1) Adding `gamma_auth_bit_shares` to `TensorFpreGen`/`TensorFpreEval` (PRE-04) must happen atomically with updating every constructor that builds those structs. (2) The `TensorPreprocessing` trait (PRE-01) wraps the now-extended return type. (3) Both backends (PRE-02, PRE-03) implement the trait against the extended structs. Tasks must execute in this order or the codebase will not compile at intermediate states.

The primary risk is the atomic constructor update: `TensorFpre::into_gen_eval()` in `src/auth_tensor_fpre.rs` must be updated simultaneously with the field addition, and the `run_preprocessing` path via `combine_leaky_triples` in `src/auth_tensor_pre.rs` must also initialize `gamma_auth_bit_shares`. Missing any constructor site is a compile error.

**Primary recommendation:** Implement in dependency order — PRE-04 field additions and all constructor updates first (Wave 1), then trait definition + both backend impls (Wave 2), then tests (Wave 3). Never leave the repo in a non-compiling state.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `TensorPreprocessing` trait definition | Preprocessing layer (`src/preprocessing.rs`) | — | Trait lives with its return types; D-01 locks this |
| `UncompressedPreprocessingBackend` impl | Preprocessing layer (`src/preprocessing.rs`) | — | Thin wrapper over `run_preprocessing` in same file |
| `IdealPreprocessingBackend` impl | Preprocessing layer (`src/preprocessing.rs`) | Fpre interface layer (`src/auth_tensor_fpre.rs`) | Impl in preprocessing.rs; delegates to `TensorFpre` from fpre module |
| `gamma_auth_bit_shares` field | Fpre interface layer (`src/auth_tensor_fpre.rs` + `src/preprocessing.rs`) | Online phase constructors | Field on structs in both files; online phase constructors (`AuthTensorGen`, `AuthTensorEval`) must not reject the new field |
| `gen_auth_bit()` for gamma generation | Fpre interface layer (`src/auth_tensor_fpre.rs`) | — | Already exists; used as-is by `IdealPreprocessingBackend` |

---

## Standard Stack

### Core (no new dependencies)

This phase introduces no new dependencies. [VERIFIED: Cargo.toml inspection]

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rand_chacha` | 0.9 | `ChaCha12Rng` for seeded RNG in `IdealPreprocessingBackend` | Project-standard PRNG; all ideal backends use it |
| Rust trait system | stable 1.90 | `TensorPreprocessing` trait definition | No external crate needed |

**Installation:** No new packages needed. `cargo test` continues to use existing `Cargo.toml`.

---

## Architecture Patterns

### System Architecture Diagram

```
Callers (online phase, tests, benchmarks)
          |
          | call TensorPreprocessing::run(n, m, count, chunking_factor)
          v
  ┌─────────────────────────────────┐
  │   TensorPreprocessing trait     │
  │   (src/preprocessing.rs)        │
  └──────┬──────────────────┬───────┘
         │                  │
         v                  v
UncompressedPreprocessingBackend    IdealPreprocessingBackend
(wraps run_preprocessing)           (delegates to TensorFpre)
         │                                    │
         v                                    v
combine_leaky_triples()              TensorFpre::new(seed=0, n, m, cf)
(src/auth_tensor_pre.rs)             TensorFpre::generate_for_ideal_trusted_dealer()
         │                           TensorFpre::into_gen_eval()
         │                           + loop: fpre.gen_auth_bit(random_bit) per (i,j)
         │                                    │
         └───────────────┬───────────────────┘
                         v
         (TensorFpreGen, TensorFpreEval)
         [now includes gamma_auth_bit_shares: Vec<AuthBitShare>]
                         │
                         v
        AuthTensorGen::new_from_fpre_gen()
        AuthTensorEval::new_from_fpre_eval()
        (online phase — must compile after PRE-04)
```

### Recommended Project Structure

No new files. All changes are in:
```
src/
├── preprocessing.rs     # ADD: TensorPreprocessing trait, UncompressedPreprocessingBackend,
│                        #      IdealPreprocessingBackend (new struct + impl)
│                        # MODIFY: TensorFpreGen, TensorFpreEval (add gamma_auth_bit_shares field)
└── auth_tensor_fpre.rs  # MODIFY: TensorFpre::into_gen_eval() to initialize gamma_auth_bit_shares
                         # ALSO MODIFY: Any TensorFpre constructor or test that builds
                         #              TensorFpreGen/TensorFpreEval struct literals
```

Additional files requiring constructor updates (no structural changes, just field initialization):
```
src/auth_tensor_pre.rs   # MODIFY: combine_leaky_triples() return — add gamma_auth_bit_shares: vec![]
                         #         (the uncompressed path does not populate it yet; vec![] is correct
                         #          until the real protocol adds it in a future phase)
src/auth_tensor_gen.rs   # CHECK: new_from_fpre_gen() — field is dropped (not stored); no change needed
                         #        UNLESS Rust requires exhaustive field access (it doesn't for move)
src/auth_tensor_eval.rs  # CHECK: new_from_fpre_eval() — same analysis
```

### Pattern 1: Zero-Field Struct Backend (IdealBCot structural precedent)

**What:** A unit struct with no fields implements a trait. All state is derived from deterministic seeds baked into the impl. This is the `IdealBCot` pattern.

**When to use:** When the "backend" is a pure function with fixed parameters (here: fixed seed=0 for the ideal dealer, fixed delta generation from that seed).

**Example from existing codebase:**
```rust
// Source: src/bcot.rs (structural precedent)
pub struct IdealBCot {
    pub delta_a: Delta,
    pub delta_b: Delta,
    rng: ChaCha12Rng,
}
impl IdealBCot {
    pub fn new(seed_a: u64, seed_b: u64) -> Self { ... }
}
```

**Phase 7 analog:**
```rust
// Source: CONTEXT.md D-02, D-03
pub struct UncompressedPreprocessingBackend;
pub struct IdealPreprocessingBackend;

impl TensorPreprocessing for UncompressedPreprocessingBackend {
    fn run(&self, n: usize, m: usize, count: usize, chunking_factor: usize)
        -> (TensorFpreGen, TensorFpreEval)
    {
        run_preprocessing(n, m, count, chunking_factor)
    }
}

impl TensorPreprocessing for IdealPreprocessingBackend {
    fn run(&self, n: usize, m: usize, count: usize, chunking_factor: usize)
        -> (TensorFpreGen, TensorFpreEval)
    {
        // CONTEXT.md D-07, D-08
        let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
        fpre.generate_for_ideal_trusted_dealer(0, 0); // masks are random; inputs unused
        let (mut gen, mut eval) = fpre.into_gen_eval();
        // Append gamma_auth_bit_shares: n*m independent random auth bits
        for _ in 0..(n * m) {
            let bit = fpre.rng.random_bool(0.5); // or use internal rng via gen_auth_bit
            let auth_bit = fpre.gen_auth_bit(bit);
            gen.gamma_auth_bit_shares.push(auth_bit.gen_share);
            eval.gamma_auth_bit_shares.push(auth_bit.eval_share);
        }
        (gen, eval)
    }
}
```

Note: `TensorFpre::rng` is private. The correct approach is to call `fpre.gen_auth_bit(rng.random_bool(0.5))` using a local RNG or expose a helper. See Pattern 3 for the precise call sequence.

### Pattern 2: Trait Method Signature — `&self` vs Associated Function

**What:** Rust traits can have methods that take `&self` (object-safe) or associated functions (no receiver). Zero-field structs do not need `&self` for data, but `&self` enables trait objects (`dyn TensorPreprocessing`) and is the idiomatic choice when the trait may eventually be used as a trait object.

**When to use `&self`:** Always for this trait — even though no data is read, it makes the trait object-safe and consistent with the existing method convention in this codebase (all protocol steps take `&mut self` or `&self`).

**Example:**
```rust
// Source: [ASSUMED] — Rust language, confirmed by CONTEXT.md Claude's Discretion
pub trait TensorPreprocessing {
    fn run(&self, n: usize, m: usize, count: usize, chunking_factor: usize)
        -> (TensorFpreGen, TensorFpreEval);
}
```

### Pattern 3: gamma_auth_bit_shares Generation in IdealPreprocessingBackend

**What:** After `into_gen_eval()` produces the base `(TensorFpreGen, TensorFpreEval)`, the caller must append `n*m` independent authenticated bits to `gamma_auth_bit_shares` on both structs.

**Precise call sequence** (from CONTEXT.md D-07, D-08, D-09):

```rust
// Inside IdealPreprocessingBackend::run()
let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
fpre.generate_for_ideal_trusted_dealer(0, 0);
let (mut gen, mut eval) = fpre.into_gen_eval();

// gen and eval now have gamma_auth_bit_shares: vec![] (initialized empty by into_gen_eval)
// Generate n*m independent random authenticated bits for l_gamma
// Use a separate RNG seeded deterministically from the fixed seed
let mut rng = ChaCha12Rng::seed_from_u64(42); // or any fixed secondary seed
for _ in 0..(n * m) {
    let l_gamma = rng.random_bool(0.5);
    // TensorFpre::gen_auth_bit() is pub — call it on fpre BEFORE into_gen_eval
    // Problem: fpre is consumed by into_gen_eval. Must generate gamma bits BEFORE calling into_gen_eval.
}
```

**CRITICAL ORDERING CONSTRAINT:** `TensorFpre::into_gen_eval(self)` consumes `fpre` by value. Therefore `gen_auth_bit()` calls for `gamma_auth_bit_shares` must happen **before** `into_gen_eval()`. The correct sequence:

```rust
let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
fpre.generate_for_ideal_trusted_dealer(0, 0);
// Generate n*m gamma bits on fpre before consuming it
let mut gamma_auth_bits = Vec::with_capacity(n * m);
let mut rng = ChaCha12Rng::seed_from_u64(42);
for _ in 0..(n * m) {
    let l_gamma = rng.random_bool(0.5);
    gamma_auth_bits.push(fpre.gen_auth_bit(l_gamma));
}
// Now consume fpre
let (mut gen, mut eval) = fpre.into_gen_eval();
// Distribute gen_share / eval_share
gen.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.gen_share).collect();
eval.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.eval_share).collect();
(gen, eval)
```

**Alternative (simpler):** Extend `TensorFpre::into_gen_eval()` to generate gamma bits internally as part of the conversion. This keeps all generation inside `TensorFpre` and avoids the ordering problem. This is the approach taken by the existing `correlated_auth_bits` generation pattern.

**Recommendation:** Extend `into_gen_eval()` to generate gamma bits from a deterministic sub-RNG. This way `IdealPreprocessingBackend::run()` is simply:

```rust
let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
fpre.generate_for_ideal_trusted_dealer(0, 0);
fpre.into_gen_eval() // returns (TensorFpreGen, TensorFpreEval) with gamma_auth_bit_shares populated
```

The planner should choose one approach and document it clearly. Both are valid. The extension approach is cleaner but requires modifying `auth_tensor_fpre.rs` more.

### Pattern 4: gamma_auth_bit_shares Initialization in UncompressedPreprocessingBackend

**What:** The `UncompressedPreprocessingBackend` delegates to `run_preprocessing()`. After PRE-04, `run_preprocessing()` returns structs with `gamma_auth_bit_shares`. The uncompressed protocol (`combine_leaky_triples`) does not yet generate l_gamma — that field should be initialized to `vec![]` (empty) or a zero-length vec for the `count=1` case until a future phase fills it in.

**Correctness note:** Phase 8 (consistency check) requires `gamma_auth_bit_shares` to be populated. If the uncompressed backend leaves it empty, the online phase will fail at runtime when it accesses the field. The planner must decide:
- Option A: Initialize as `vec![]` and document that `UncompressedPreprocessingBackend` does not yet support consistency check.
- Option B: Generate gamma bits inside `combine_leaky_triples` using the existing shared `IdealBCot` (same delta pair).

Option B is more complete and aligns with PRE-03 ("all existing implementations satisfy `TensorPreprocessing`"). Option A leaves a known gap. The CONTEXT.md is silent on this — it is Claude's Discretion for the planner to resolve.

### Anti-Patterns to Avoid

- **Splitting the field addition from constructor updates:** Adding `gamma_auth_bit_shares` to `TensorFpreGen`/`TensorFpreEval` without simultaneously updating all struct literal constructions is a compile error. Rust requires all fields to be initialized in struct literal syntax.
- **Calling `gen_auth_bit()` after `into_gen_eval()`:** `into_gen_eval()` consumes `TensorFpre` by value. Any `gen_auth_bit()` call must precede it.
- **Using the wrong delta for gamma shares:** `gen_auth_bit()` on `TensorFpre` uses `self.delta_a` and `self.delta_b` correctly. Do not reconstruct `AuthBitShare` values manually — always use `gen_auth_bit()`.
- **Verifying gen/eval gamma shares with direct `share.verify(delta)`:** The cross-party IT-MAC layout means `gen.gamma_auth_bit_shares[k].verify(delta_b)` PANICS (same as all other cross-party shares). Use the `verify_cross_party` pattern from tests.
- **PRE-05 scaffolding:** Do not add any `IdealCompressedPreprocessingBackend` skeleton, even as a stub. D-10 is an explicit prohibition.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Authenticated bit generation | Custom `AuthBitShare` construction | `TensorFpre::gen_auth_bit(bit: bool) -> AuthBit` | Existing function handles key LSB invariant, delta assignment, and IT-MAC construction correctly; hand-rolling risks violating `key.lsb() == 0` or using wrong delta |
| IT-MAC verification in tests | Inline MAC checks | `AuthBitShare::verify(delta)` / `verify_cross_party` pattern | Already validated by 74-test suite; the cross-party layout requires the swap that `verify_cross_party` implements |
| RNG seeding | `rand::rng()` (non-deterministic) | `ChaCha12Rng::seed_from_u64(seed)` | Project invariant: all ideal backends use explicit deterministic seeds for reproducibility |

**Key insight:** `TensorFpre::gen_auth_bit()` is the correct and complete abstraction for generating one IT-MAC authenticated bit. Everything in Phase 7 that needs a new `AuthBitShare` should use it.

---

## Common Pitfalls

### Pitfall 1: Struct Literal Exhaustiveness

**What goes wrong:** Adding `gamma_auth_bit_shares` to `TensorFpreGen`/`TensorFpreEval` breaks every location that constructs these structs using struct literal syntax. This is a compile error in Rust, not a runtime bug.

**Why it happens:** Rust struct literals require all fields to be present unless `..default` is used (and these structs don't derive `Default`).

**Locations that construct TensorFpreGen/TensorFpreEval struct literals:** [VERIFIED: grep of src/]
1. `src/auth_tensor_fpre.rs` — `TensorFpre::into_gen_eval()` (the primary constructor)
2. `src/auth_tensor_pre.rs` — `combine_leaky_triples()` (builds TensorFpreGen/TensorFpreEval directly from LeakyTriple fields)

These two are the **only** struct literal construction sites. `src/preprocessing.rs::run_preprocessing` delegates to `combine_leaky_triples`, so it is covered transitively.

**How to avoid:** Update both struct literal sites atomically in the same commit that adds the field (D-06 requirement).

**Warning signs:** `cargo build` fails with "missing field `gamma_auth_bit_shares`" error.

### Pitfall 2: `into_gen_eval()` Consumes `TensorFpre`

**What goes wrong:** `IdealPreprocessingBackend::run()` calls `fpre.into_gen_eval()` which takes `self` by value. Any `fpre.gen_auth_bit()` call placed AFTER `into_gen_eval()` is a compile error ("use of moved value").

**Why it happens:** Rust's ownership system enforces move semantics; the method signature `fn into_gen_eval(self)` is explicit.

**How to avoid:** Collect all `gen_auth_bit()` results into a `Vec<AuthBit>` before calling `into_gen_eval()`. See Pattern 3 above.

**Warning signs:** Compiler error "use of moved value: `fpre`".

### Pitfall 3: Wrong Delta in gamma_auth_bit_shares Tests

**What goes wrong:** Cross-party shares — `gen.gamma_auth_bit_shares[k]` and `eval.gamma_auth_bit_shares[k]` — cannot be verified with direct `share.verify(delta)`. Calling `gen.gamma_auth_bit_shares[k].verify(&gen.delta_a)` will panic.

**Why it happens:** In the cross-party layout, gen_share.mac is authenticated under delta_b (the evaluator's delta), not delta_a. The `AuthBitShare::verify()` uses `self.key` and `self.mac` from the same struct, which come from different parties.

**How to avoid:** Use the `verify_cross_party(pa_share, pb_share, delta_a, delta_b)` helper pattern established in `src/leaky_tensor_pre.rs` and `src/auth_tensor_pre.rs`:
```rust
fn verify_cross_party(
    pa_share: &AuthBitShare, pb_share: &AuthBitShare,
    delta_a: &Delta, delta_b: &Delta,
) {
    AuthBitShare { key: pb_share.key, mac: pa_share.mac, value: pa_share.value }.verify(delta_b);
    AuthBitShare { key: pa_share.key, mac: pb_share.mac, value: pb_share.value }.verify(delta_a);
}
```

**Warning signs:** Test panics with "MAC mismatch in share" when verifying gamma_auth_bit_shares.

### Pitfall 4: `new_from_fpre_gen` / `new_from_fpre_eval` Not Reading `gamma_auth_bit_shares`

**What goes wrong:** `AuthTensorGen::new_from_fpre_gen()` and `AuthTensorEval::new_from_fpre_eval()` move fields out of `TensorFpreGen`/`TensorFpreEval`. After PRE-04, the new `gamma_auth_bit_shares` field is present in the struct but NOT mapped to a corresponding field in `AuthTensorGen`/`AuthTensorEval`. In Rust, moving a struct out of an argument using field-by-field destructuring does not require exhaustive field access — so this will NOT be a compile error. The field will be silently dropped.

**Why it matters:** Phase 8 (consistency check) will need `gamma_auth_bit_shares` available in `AuthTensorGen`/`AuthTensorEval`. If it is dropped here, Phase 8 will need to retrofit these constructors.

**How to avoid:** The planner should decide: either (a) add `gamma_auth_bit_shares` to `AuthTensorGen`/`AuthTensorEval` now and forward it in the constructors, or (b) leave it as a known Phase 8 task. If (b), add a `// TODO(Phase 8): forward gamma_auth_bit_shares` comment at the drop site.

**Warning signs:** Phase 8 tries to access `gamma_auth_bit_shares` on `AuthTensorGen` and finds the field doesn't exist.

---

## Code Examples

Verified patterns from the codebase:

### Existing `into_gen_eval()` (must be extended)
```rust
// Source: src/auth_tensor_fpre.rs lines 158-181
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
        // MUST ADD: gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(...).collect(),
        // OR:       gamma_auth_bit_shares: vec![],  (if generated externally)
    }, TensorFpreEval {
        // ... (symmetric)
        // MUST ADD: gamma_auth_bit_shares field
    })
}
```

### Existing `gen_auth_bit()` (use as-is)
```rust
// Source: src/auth_tensor_fpre.rs lines 66-86
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

### `combine_leaky_triples` return site (must add field)
```rust
// Source: src/auth_tensor_pre.rs (combine_leaky_triples return)
// The function builds TensorFpreGen and TensorFpreEval struct literals.
// After PRE-04, both struct literals must include:
//   gamma_auth_bit_shares: vec![],
// (or populate from LeakyTriple gamma_shares if the plan adds that logic)
```

### Cross-party verification pattern (use in tests)
```rust
// Source: src/leaky_tensor_pre.rs tests, src/auth_tensor_pre.rs tests
fn verify_cross_party(
    pa_share: &AuthBitShare,
    pb_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare { key: pb_share.key, mac: pa_share.mac, value: pa_share.value }
        .verify(delta_b);
    AuthBitShare { key: pa_share.key, mac: pb_share.mac, value: pb_share.value }
        .verify(delta_a);
}
```

---

## Runtime State Inventory

Step 2.5 SKIPPED — this is a greenfield feature addition (trait + backend structs), not a rename/refactor/migration phase. No runtime state is renamed.

---

## Environment Availability

Step 2.6: No external dependencies identified beyond the existing Rust toolchain.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust stable | All compilation | ✓ | 1.90.0 | — |
| Cargo | Build/test | ✓ | 1.90.0 | — |

**All dependencies available.** No blocking gaps.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness |
| Config file | none (standard `cargo test`) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PRE-01 | Trait is defined; both backends implement it | unit (compile check + instantiation) | `cargo test --lib` | ❌ Wave 0 |
| PRE-02 | `IdealPreprocessingBackend::run()` returns valid (Gen, Eval) pair with correct IT-MAC on all fields | unit | `cargo test --lib preprocessing::tests` | ❌ Wave 0 |
| PRE-03 | `UncompressedPreprocessingBackend::run()` delegates to `run_preprocessing` correctly; existing preprocessing tests still pass | regression | `cargo test --lib preprocessing::tests` | ❌ Wave 0 (new tests); existing tests cover run_preprocessing |
| PRE-04 | `TensorFpreGen`/`TensorFpreEval` compile with `gamma_auth_bit_shares`; all constructors initialize it; IT-MAC invariant holds for gamma shares | unit | `cargo test --lib` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green (`cargo test`) before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] Tests for `TensorPreprocessing` trait implementation — covers PRE-01 (compile check is implicit; add explicit instantiation test)
- [ ] Tests for `IdealPreprocessingBackend::run()` — covers PRE-02: dimensions correct, IT-MAC invariant holds for `gamma_auth_bit_shares` using `verify_cross_party`
- [ ] Tests for `UncompressedPreprocessingBackend::run()` — covers PRE-03: equivalent to existing `test_run_preprocessing_*` tests but called through the trait
- [ ] Tests for `gamma_auth_bit_shares` field on both structs — covers PRE-04: length check (== n*m), MAC invariant check, value distinctness from `correlated_auth_bit_shares`

All new tests go in `#[cfg(test)] mod tests { ... }` at the bottom of `src/preprocessing.rs`, following the project convention of co-located tests.

---

## Security Domain

This phase does not introduce new cryptographic operations. The IT-MAC invariant (`mac = key XOR bit * delta`) is an existing invariant already enforced by `TensorFpre::gen_auth_bit()`. No new threat patterns are introduced.

ASVS categories are not applicable to a pure research/library crate with no network surface, authentication layer, or user input.

---

## Open Questions

1. **Should `gamma_auth_bit_shares` be populated by `UncompressedPreprocessingBackend` in Phase 7, or deferred to Phase 8?**
   - What we know: `combine_leaky_triples` already has access to `gamma_shares` from `LeakyTriple` (the noise bits from Pi_LeakyTensor). These are the correct per-step gamma bits for the protocol.
   - What's unclear: Whether these `gamma_shares` from `LeakyTriple` are semantically equivalent to "l_gamma" (the gate output mask) or are a different quantity. The CONTEXT.md says l_gamma is "the gate output mask" and is distinct from correlated_auth_bit_shares (l_gamma*). The LeakyTriple gamma_shares are noise bits used in the bucketing combiner — their relationship to l_gamma requires paper consultation.
   - Recommendation: Initialize `gamma_auth_bit_shares` to `vec![]` in `combine_leaky_triples` for Phase 7 (safe default), and task Phase 8 planning to fill in the correct value. This avoids a paper-misreading bug.

2. **Should `AuthTensorGen` and `AuthTensorEval` be extended with `gamma_auth_bit_shares` in Phase 7 or Phase 8?**
   - What we know: Phase 8 needs these fields for the consistency check. Adding them in Phase 7 avoids a breaking change in Phase 8.
   - What's unclear: Whether adding unused fields in Phase 7 creates linting noise or confusion.
   - Recommendation: Add the fields in Phase 7 (forward them from the fpre constructors) but leave them unused with `#[allow(dead_code)]`. This makes Phase 8 a pure feature addition, not a breaking struct change.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `combine_leaky_triples` in `src/auth_tensor_pre.rs` is the only non-fpre construction site for `TensorFpreGen`/`TensorFpreEval` struct literals | Pitfall 1 / Architecture Patterns | If another construction site exists, the struct field addition will miss it and break compilation |
| A2 | `AuthTensorGen::new_from_fpre_gen()` and `AuthTensorEval::new_from_fpre_eval()` do NOT use struct literal destructuring that requires exhaustive field coverage — they access fields by name individually | Pitfall 4 | If they use struct literal `..rest` destructuring, behavior may differ; but Rust field access by name is the pattern observed in source |

Note on A1: [VERIFIED by grep of src/ — no other files construct `TensorFpreGen` or `TensorFpreEval` struct literals]

Note on A2: [VERIFIED by reading `src/auth_tensor_gen.rs` lines 52-66 and `src/auth_tensor_eval.rs` lines 45-60 — both use named field access, not struct literal exhaustion]

Both assumptions are therefore verified. Assumptions log is effectively empty of unverified claims.

---

## Sources

### Primary (HIGH confidence)

- `src/preprocessing.rs` (read in full) — current `TensorFpreGen`, `TensorFpreEval`, `run_preprocessing` implementation; zero existing trait or backend types
- `src/auth_tensor_fpre.rs` (read in full) — `TensorFpre`, `gen_auth_bit()`, `into_gen_eval()` — the delegation target for `IdealPreprocessingBackend`
- `src/bcot.rs` (read in full) — `IdealBCot` structural precedent for zero-field backend pattern
- `src/auth_tensor_gen.rs` (read in full) — downstream consumer of `TensorFpreGen`; confirms no exhaustive field destructuring
- `src/auth_tensor_eval.rs` (read in full) — downstream consumer of `TensorFpreEval`; confirms no exhaustive field destructuring
- `src/sharing.rs` (read in full) — `AuthBitShare`, `AuthBit`, `build_share` — types used in new field
- `.planning/codebase/CONVENTIONS.md` — naming, invariants, cross-party MAC layout
- `.planning/codebase/ARCHITECTURE.md` — module boundaries, data flow
- `.planning/codebase/TESTING.md` — test patterns, `verify_cross_party` helper
- `.planning/phases/07-preprocessing-trait-ideal-backends/07-CONTEXT.md` — all decisions D-01 through D-10

### Secondary (MEDIUM confidence)

- `cargo test` output: 74/74 tests passing at research time — confirmed baseline
- `rustc --version` + `cargo --version`: 1.90.0 stable — confirmed Rust edition 2024 support

### Tertiary (LOW confidence)

- None. All findings are directly verified from source code or CONTEXT.md.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; existing stack verified
- Architecture: HIGH — all construction sites verified by grep; constructor signatures read directly
- Pitfalls: HIGH — all pitfalls derived from direct code reading, not inference
- Test patterns: HIGH — patterns verified from existing test modules

**Research date:** 2026-04-23
**Valid until:** 2026-05-23 (stable codebase; only changes from Phase 7 implementation itself could invalidate)
