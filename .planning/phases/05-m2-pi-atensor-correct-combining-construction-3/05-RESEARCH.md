# Phase 5: M2 Pi_aTensor Correct Combining (Construction 3) - Research

**Researched:** 2026-04-22
**Domain:** Paper-faithful two-to-one combining of leaky tensor triples into an authenticated tensor triple; iterative bucket combining; corrected bucket-size formula under `ell` parametrization (Pi_aTensor, Appendix F Construction 3).
**Confidence:** HIGH — every factual claim is verified against the paper appendix (`references/appendix_krrw_pre.tex`, lines 415–546) or the existing codebase (`src/auth_tensor_pre.rs`, `src/leaky_tensor_pre.rs`, `src/sharing.rs`, `src/preprocessing.rs`). No novel library or framework research is required; everything composes from Phase 4 primitives.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Two-to-One Combining: x output (PROTO-10):**
- **D-01:** The combined `itmac{x}{Δ}` is `itmac{x'}{Δ} ⊕ itmac{x''}{Δ}` — XOR of both triples' x shares. The ROADMAP description "keep x = x'" was a shorthand error; the paper (appendix_krrw_pre.tex line 427) is unambiguous. Concretely: `gen_x_shares` and `eval_x_shares` are XOR-combined across all B triples in the bucket (same loop structure as z combining).
- **D-02:** `itmac{y}{Δ} := itmac{y'}{Δ}` — keep the first triple's y shares unchanged (no combining needed for y). This is correct per the paper.

**Two-to-One Combining: Z output (PROTO-10):**
- **D-03:** `Z = Z' ⊕ Z'' ⊕ (itmac{x''}{Δ} ⊗ d)` where d is the publicly revealed bit vector of length m. The tensor product `itmac{x''}{Δ} ⊗ d` is computed locally by both parties: for each (i, j) pair, the IT-MAC share at column-major index `j*n+i` is `x''_shares[i]` if `d[j] == 1`, else zero share (key=0, mac=0, value=false). No GGM macro call needed — d is public so the computation requires no interaction.
- **D-04:** Z storage convention is unchanged from Phase 4: `Vec<AuthBitShare>` in column-major order (index `j*n+i`), length `n*m`.

**d Reveal and MAC Verification (PROTO-10):**
- **D-05:** `d[j] = y'_j ⊕ y''_j` (bit XOR of the value fields of the two y shares at index j). Each party assembles their IT-MAC share of `d_j` by XORing the key/mac/value fields of their y' and y'' AuthBitShares.
- **D-06:** Before using d to compute Z, call `AuthBitShare::verify(delta)` on each assembled d share (both gen-side and eval-side). This is the in-process substitute for "publicly reveal with appropriate MACs" from the paper. Verification failure panics (same convention as F_eq).
- **D-07:** d is a `Vec<bool>` of length m extracted from the assembled d shares after verification. The tensor product computation (D-03) uses these bool values directly.

**Bucket Size Formula Fix (PROTO-12):**
- **D-08:** `bucket_size_for(ell: usize) -> usize` replaces `bucket_size_for(n: usize, m: usize) -> usize`. The parameter `ell` is the number of OUTPUT authenticated tensor triples (not the tensor dimensions n·m).
- **D-09:** Formula: `B = floor(SSP / log2(ell)) + 1` for `ell ≥ 2`. When `ell ≤ 1`, return `SSP` (= 40). This matches the naive combining approach from the paper's §3.1: without bucketing amortization, you need SSP triples to reach 2^−ρ security.
- **D-10:** Call site in `run_preprocessing`: change `bucket_size_for(n, m)` to `bucket_size_for(count)`. With `count = 1` (current use), `B = SSP = 40`.

**Iterative Combining Structure (PROTO-11):**
- **D-11:** Implement a `pub(crate) fn two_to_one_combine(prime: LeakyTriple, dprime: &LeakyTriple) -> LeakyTriple` helper that performs one combining step. This makes TEST-05 directly testable on two `LeakyTriple`s without going through the full bucket pipeline, and keeps `combine_leaky_triples` as a thin iterative wrapper.
- **D-12:** `combine_leaky_triples` folds B triples one at a time: start with `triples[0]`, then iteratively call `two_to_one_combine(acc, &triples[i])` for `i in 1..B`. The final accumulated `LeakyTriple` is converted to `(TensorFpreGen, TensorFpreEval)` at the end — same return type as today.

### Claude's Discretion

- Exact zero-share representation for `AuthBitShare` when `d[j] == 0` in the tensor product — use `AuthBitShare::default()` (key=0, mac=0, value=false) which is the same as `AuthBitShare { key: Key::default(), mac: Mac::default(), value: false }`. `AuthBitShare::default()` is already derived (`src/sharing.rs:42`). [VERIFIED: src/sharing.rs line 42 `#[derive(Debug, Clone, Default, Copy)]`]
- Whether the `delta_a`/`delta_b` same-delta assertion in `combine_leaky_triples` is moved into `two_to_one_combine` or kept in the outer wrapper. Recommended: keep the assertion in both — in `two_to_one_combine` for unit-test safety, and in `combine_leaky_triples` as a documentation anchor.
- Loop ordering (iterate over `j` then `i`, or flat index `k`) for the `d ⊗ x''` computation. Recommended: outer `j` / inner `i` to match the established column-major pattern (`src/leaky_tensor_pre.rs:240-247`, `src/matrix.rs:252-258`).

### Deferred Ideas (OUT OF SCOPE)

None — Phase 5 context-gathering stayed within scope. Explicitly deferred to Phase 6:
- Permutation bucketing (PROTO-13, PROTO-14) — `Pi_aTensor'` improvement.
- Improved bucket size formula `B = 1 + ceil(SSP/log2(n·ell))` (PROTO-15).
- Random partitioning (Pi_aTensor Construction 3, step 2 in the paper) — the current `combine_leaky_triples` signature receives `triples: Vec<LeakyTriple>` already assembled; partitioning is the caller's job in Phase 6.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **PROTO-10** | Implement correct combining procedure: keep `y = y'`, reveal `d = y' ⊕ y''` (with MAC verification), compute `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d`. | Paper lines 427–444 give the exact algebra. `AuthBitShare + AuthBitShare` XOR (`src/sharing.rs:66-77`) combines key/mac/value field-wise. `AuthBitShare::verify(&delta)` (`src/sharing.rs:59-63`) is the in-process MAC check — but ONLY after reassembling the cross-party pair via `verify_cross_party` helper pattern (`src/leaky_tensor_pre.rs:344-362`). See **Architecture Patterns → Pattern 1**, **Pattern 2**. |
| **PROTO-11** | Iterative combining: fold B leaky triples one at a time using the two-to-one combine. | The existing `combine_leaky_triples` loop at `src/auth_tensor_pre.rs:71-80` already folds B triples one at a time for z (naïve XOR). Phase 5 replaces the naïve body with a proper `two_to_one_combine(acc, next)` call inside the loop. See **Architecture Patterns → Pattern 3**. |
| **PROTO-12** | Fix bucket size formula: `B = floor(SSP / log2(ell)) + 1` where `ell` is the number of OUTPUT authenticated tensor triples (not `n·m`). | Paper line 484: `B = ⌊ssp / log ℓ⌋ + 1` where `ℓ` is the count of output triples (Construction 3, Theorem 1). Existing code at `src/auth_tensor_pre.rs:15-21` uses `ell = n * m` — a direct bug per CONTEXT.md. Signature change is breaking; Wave 0 updates both definition and call site. |
| **TEST-05** | `Z_combined = Z' ⊕ Z'' ⊕ x'' ⊗ d` verified on two concrete leaky triples AND IT-MAC on d rejects tampered values. | Two test shapes: (a) happy path — construct two `LeakyTriple`s via `LeakyTensorPre::generate()`, call `two_to_one_combine`, assert product invariant `z_full == x_full ⊗ y_full` on the combined triple; (b) tamper path — modify one `y''` value field before combining and assert `#[should_panic]` from the MAC-verify step on d. See **Architecture Patterns → Pattern 4**, **Code Examples → Example 4**. |
</phase_requirements>

---

## Summary

Phase 5 is a **pure algorithm rewrite** of two functions in `src/auth_tensor_pre.rs` — `combine_leaky_triples` (correct two-to-one combining) and `bucket_size_for` (correct `ell`-parametrized formula) — plus a new `two_to_one_combine` helper and one new TEST-05 test pair. No new modules, no new external dependencies, no schema changes beyond renaming `bucket_size_for`'s signature. The paper specification is precise (Appendix F §3.1 lines 415–444 for combining, lines 449–535 for the bucket-size theorem) and the existing Phase 4 primitives (`LeakyTriple`, `AuthBitShare`, `AuthBitShare::verify`, `IdealBCot`) already cover everything needed.

The only non-obvious element is the interaction between the paper's "publicly reveal `d` with appropriate MACs" language and the in-process ideal substitute: per D-06 the parties each assemble their cross-party IT-MAC share of `d = y' ⊕ y''` (trivial given that `AuthBitShare + AuthBitShare` is XOR across all three fields) and run `verify_cross_party(gen_d, eval_d, Δ_A, Δ_B)` — the same helper already in the test suite — to detect tampering before `d` is used in the Z computation. If verification passes, the bool value of `d_j` is `gen_d_j.value ^ eval_d_j.value`. If it fails, the protocol panics (matching `feq::check` and Phase 4 abort semantics).

The iterative combining is the straightforward fold described in Construction 3 step 3: start with `triples[0]`, repeatedly two-to-one combine with the next triple. Since two-to-one is associative-like (the combined x is sum of all x's, the combined y is y', and Z accumulates correctly), the order is well-defined. The full call tree becomes `run_preprocessing` → `combine_leaky_triples(triples, bucket_size_for(count), ...)` → iterative `two_to_one_combine` → `(TensorFpreGen, TensorFpreEval)`.

**Primary recommendation:** Implement `two_to_one_combine` as a pure function returning `LeakyTriple`, verify it on two hand-checked triples (TEST-05), then rewrite `combine_leaky_triples` as a 3-line wrapper that iterates the helper. Fix `bucket_size_for` and `run_preprocessing` call site together in the same wave so the build stays green. The `correlated_auth_bit_shares` alignment bug in the existing code (it hands `t0.gen_x_shares.clone()` to `alpha_auth_bit_shares` instead of the XOR-combined x) gets fixed automatically as a consequence of D-01 — no separate task.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Bucket size formula (`B = ⌊SSP / log₂ ℓ⌋ + 1`) | `auth_tensor_pre::bucket_size_for` (`src/auth_tensor_pre.rs`) | — | Pure arithmetic utility; same file as the caller. Signature change from `(n, m)` to `(ell)` is a breaking rename captured by Wave 0. |
| Two-to-one combining step (one bucket's worth of two triples → one triple) | NEW `auth_tensor_pre::two_to_one_combine` helper | — | Per D-11: standalone `pub(crate)` helper so TEST-05 can target it directly without the bucket wrapper. |
| Iterative bucket combining (`B` triples → one authenticated triple) | `auth_tensor_pre::combine_leaky_triples` | `two_to_one_combine` | Per D-12: thin `fold` wrapper. Same signature as today; only the body changes. |
| `d` reveal + MAC verification on `d` shares | `two_to_one_combine` (inline) | `AuthBitShare::verify` (`src/sharing.rs:59-63`) | In-process substitute for the paper's "publicly reveal with appropriate MACs"; per D-06 both parties run `verify_cross_party` on each `d_j` share pair. |
| `itmac{x''}{Δ} ⊗ d` tensor product (local, no interaction) | `two_to_one_combine` (inline) | `AuthBitShare + AuthBitShare` (`src/sharing.rs:66-117`) | Per D-03: for each `(i,j)`, either the `x''_i` share or a zero share, depending on `d[j]`. The tensor product is a local XOR fold with the running Z. |
| Cross-party MAC verification helper (test-only) | `verify_cross_party` (already in `src/auth_tensor_pre.rs:134-152` and `src/leaky_tensor_pre.rs:344-362`) | — | Reuse verbatim. Do NOT call `share.verify(&delta)` directly on a single cross-party share — it will panic (doc-comment at `src/leaky_tensor_pre.rs:29-35`, codebase-wide convention). |
| Construction 3 random partitioning (paper lines 469–471, step 2) | Out of Phase 5 scope (deferred to Phase 6) | — | The `_shuffle_seed` parameter on `combine_leaky_triples` is already reserved for this. Phase 5 consumes `triples` in arrival order and relies on the caller (`run_preprocessing`) to supply them pre-ordered. |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust (edition 2024) | 1.90.0 | Host language | `[VERIFIED: rustc --version output 2026-04-22]` — project pins edition 2024 (`Cargo.toml`, prior RESEARCH.md) |
| cargo test | bundled with 1.90.0 | Test runner for `#[cfg(test)] mod tests` | `[VERIFIED: cargo test --lib passes 66 tests]` — codebase convention (every module has inline tests) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `rand_chacha::ChaCha12Rng` | pinned in Cargo.toml | Deterministic seeded RNG (inherited from Phase 4) | `make_triples` test helper already uses it via `LeakyTensorPre::new(seed, ...)` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-process `AuthBitShare::verify` as d-reveal MAC check | A real commit-and-open equality protocol | v2 deferred item; `IdealBCot` and `feq` already follow the in-process-ideal pattern. In-process is correct for benchmarking and proof-modeling. |
| Bucket partitioning inside `combine_leaky_triples` | Move partitioning to `run_preprocessing` | Per CONTEXT.md deferred ideas — Phase 6 will add shuffled partitioning in a dedicated step; Phase 5 keeps `combine_leaky_triples` order-preserving. |

**Installation:**

No new dependencies. Existing `Cargo.toml` is sufficient — every symbol Phase 5 needs (`AuthBitShare`, `LeakyTriple`, `Delta`, `Block`, `Key::default()`, `Mac::default()`, `ChaCha12Rng`) is already in-crate.

**Version verification:** No external crate versions under review. `rustc 1.90.0 (1159e78c4 2025-09-14)` and `cargo 1.90.0 (840b83a10 2025-07-30)` both verified.

---

## Architecture Patterns

### System Architecture Diagram

```
                                      ┌────────────────────────────────────────┐
                                      │ preprocessing::run_preprocessing(n, m, │
                                      │   count, chunking_factor)              │
                                      └─────────────────┬──────────────────────┘
                                                        │
                                 ┌──────────────────────┴───────────────────┐
                                 │                                          │
                                 ▼                                          ▼
                 ┌──────────────────────────────┐      ┌──────────────────────────────┐
                 │  bucket_size_for(count)      │      │  Loop (B * count) times:     │
                 │  → B = ⌊SSP/log₂ count⌋ + 1  │      │    LeakyTensorPre::generate()│
                 │  (or SSP if count ≤ 1)       │      │    (Phase 4 artifact)        │
                 └──────────────┬───────────────┘      └──────────────┬───────────────┘
                                │ B                                   │ B triples
                                └──────────────────┬──────────────────┘
                                                   ▼
                            ┌─────────────────────────────────────────────────┐
                            │  combine_leaky_triples(triples, B, n, m, cf, _) │
                            │  ───────────────────────────────────────────    │
                            │  1. Assert all triples share Δ_A and Δ_B        │
                            │  2. acc ← triples[0]                            │
                            │  3. for i in 1..B:                              │
                            │       acc ← two_to_one_combine(acc, triples[i]) │
                            │  4. Pack acc into (TensorFpreGen, TensorFpreEval│
                            └──────────────────┬──────────────────────────────┘
                                               │
                                               ▼
                          ┌─────────────────────────────────────────────┐
                          │  two_to_one_combine(prime, &dprime)         │
                          │  ─────────────────────────────────────      │
                          │  Step A: assemble d shares                  │
                          │    gen_d[j]  = gen_y'[j] + gen_y''[j]       │
                          │    eval_d[j] = eval_y'[j] + eval_y''[j]     │
                          │                                             │
                          │  Step B: MAC-verify d (PROTO-10)            │
                          │    for j in 0..m:                           │
                          │      verify_cross_party(gen_d[j], eval_d[j],│
                          │                         &Δ_A, &Δ_B)         │
                          │      d[j] = gen_d[j].value ^ eval_d[j].value│
                          │                                             │
                          │  Step C: combine x = x' ⊕ x''               │
                          │    for i in 0..n:                           │
                          │      gen_x[i]  = gen_x'[i]  + gen_x''[i]    │
                          │      eval_x[i] = eval_x'[i] + eval_x''[i]   │
                          │                                             │
                          │  Step D: combine Z = Z' ⊕ Z'' ⊕ x'' ⊗ d     │
                          │    for j in 0..m:                           │
                          │      for i in 0..n:                         │
                          │        k = j*n + i                          │
                          │        dx_gen  = if d[j] {gen_x''[i]}       │
                          │                  else {AuthBitShare::zero}  │
                          │        dx_eval = if d[j] {eval_x''[i]}      │
                          │                  else {AuthBitShare::zero}  │
                          │        gen_z[k]  = gen_z'[k]  +             │
                          │                    gen_z''[k]  + dx_gen     │
                          │        eval_z[k] = eval_z'[k] +             │
                          │                    eval_z''[k] + dx_eval    │
                          │                                             │
                          │  Step E: y = y' (copy y' shares)            │
                          │                                             │
                          │  → LeakyTriple { x, y, z, Δ_A, Δ_B }        │
                          └─────────────────────────────────────────────┘

    Paper reference: appendix_krrw_pre.tex §3.1 lines 415–444 (combining),
                     lines 449–535 (Construction 3 + Theorem 1 bucket size).
```

**Reader trace:** `run_preprocessing` counts how many output triples are needed (`count`), asks `bucket_size_for(count)` for the bucket amplification factor `B`, generates `B·count` leaky triples via the Phase 4 `LeakyTensorPre::generate()`, and hands them to `combine_leaky_triples` which folds them `B` at a time using `two_to_one_combine`. The helper internally reveals and MAC-verifies `d`, then applies the paper's `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d` formula.

### Recommended Project Structure

No file additions. Phase 5 modifies two files and adds tests only.

```
src/
├── auth_tensor_pre.rs   # MODIFIED — bucket_size_for signature, combine_leaky_triples body rewritten,
│                        #            new pub(crate) fn two_to_one_combine, new TEST-05 test pair
├── preprocessing.rs     # MODIFIED — one-line call-site change: bucket_size_for(n, m) → bucket_size_for(count)
├── leaky_tensor_pre.rs  # UNCHANGED — LeakyTriple is read-only here (Phase 4 artifact)
├── sharing.rs           # UNCHANGED — AuthBitShare::verify, Add impls, ::default() all usable as-is
├── feq.rs               # UNCHANGED — not needed in Phase 5 (the d-reveal MAC check uses share.verify, not F_eq)
└── bcot.rs, delta.rs, matrix.rs, keys.rs, macs.rs, block.rs  # UNCHANGED
```

### Pattern 1: Cross-party `AuthBitShare` XOR (d assembly, x combining, Z combining)

**What:** Every additive combination in this phase is XOR of two `AuthBitShare`s field-wise (`key XOR key`, `mac XOR mac`, `value XOR value`).

**When to use:** `d_j = y'_j ⊕ y''_j`; `x = x' ⊕ x''`; `Z' ⊕ Z''`; adding the `x'' ⊗ d` correction into the combined Z.

**Example:**
```rust
// Source: src/sharing.rs:66-77 (verified); used verbatim in src/auth_tensor_pre.rs:77-78 (existing z loop)
// combining two shares
let gen_combined: AuthBitShare = gen_prime_share + gen_dprime_share;

// verify_cross_party after d assembly (Pattern 2) — cf. src/auth_tensor_pre.rs:134-152
verify_cross_party(&gen_d, &eval_d, &delta_a, &delta_b);

// Extract the bit once verified
let d_j: bool = gen_d.value ^ eval_d.value;
```

### Pattern 2: `verify_cross_party` — MAC check for in-process public reveal

**What:** The helper reassembles the properly-aligned IT-MAC pair from cross-party `AuthBitShare`s and calls `share.verify(&delta)` on each side under its verifier's delta. Copy verbatim from either of the two existing definitions; do not call `share.verify(&delta)` on a cross-party share directly (panics).

**When to use:** Inside `two_to_one_combine` after assembling each `d_j` pair. Also in TEST-05 to verify the final combined `LeakyTriple`.

**Example:**
```rust
// Source: src/auth_tensor_pre.rs:134-152 (verified) — and src/leaky_tensor_pre.rs:344-362.
fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare { key: eval_share.key, mac: gen_share.mac, value: gen_share.value }
        .verify(delta_b);
    AuthBitShare { key: gen_share.key, mac: eval_share.mac, value: eval_share.value }
        .verify(delta_a);
}
```

Both `.verify()` calls use `assert_eq!` under the hood (`src/sharing.rs:59-63` — `assert_eq!(self.mac, want, "MAC mismatch in share")`). On failure the test `#[should_panic(expected = "MAC mismatch in share")]` attribute captures the abort.

### Pattern 3: Iterative fold over a bucket of triples

**What:** The classic "start with first element, fold the rest" Rust pattern.

**When to use:** `combine_leaky_triples` after Phase 5 rewrite.

**Example:**
```rust
// Pattern is already in use at src/auth_tensor_pre.rs:71-80 (naïve XOR version); rewritten:
pub fn combine_leaky_triples(
    triples: Vec<LeakyTriple>,
    bucket_size: usize,
    n: usize,
    m: usize,
    chunking_factor: usize,
    _shuffle_seed: u64,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(triples.len(), bucket_size, "triples.len() must equal bucket_size");
    assert!(bucket_size >= 1);

    // Same-delta assertion — copy verbatim from src/auth_tensor_pre.rs:53-68
    let delta_a = triples[0].delta_a;
    let delta_b = triples[0].delta_b;
    for (idx, t) in triples.iter().enumerate() {
        assert_eq!(t.delta_a.as_block(), delta_a.as_block(), "triple[{}] delta_a mismatch", idx);
        assert_eq!(t.delta_b.as_block(), delta_b.as_block(), "triple[{}] delta_b mismatch", idx);
    }

    // Iterative fold (D-12)
    let mut acc: LeakyTriple = triples[0].clone();  // see Pitfall 4 on clone
    for next in triples.iter().skip(1) {
        acc = two_to_one_combine(acc, next);
    }

    // Pack into TensorFpreGen / TensorFpreEval (existing pattern at src/auth_tensor_pre.rs:83-107)
    (
        TensorFpreGen {
            n, m, chunking_factor, delta_a,
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: acc.gen_x_shares,
            beta_auth_bit_shares:  acc.gen_y_shares,
            correlated_auth_bit_shares: acc.gen_z_shares,
        },
        TensorFpreEval {
            n, m, chunking_factor, delta_b,
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: acc.eval_x_shares,
            beta_auth_bit_shares:  acc.eval_y_shares,
            correlated_auth_bit_shares: acc.eval_z_shares,
        },
    )
}
```

### Pattern 4: `two_to_one_combine` helper skeleton

**What:** A single-step combine; the atom of Construction 3.

**When to use:** Unit-tested by TEST-05 directly; also used by `combine_leaky_triples`.

**Example:**
```rust
// New in src/auth_tensor_pre.rs. Sources cited inline.
pub(crate) fn two_to_one_combine(
    prime: LeakyTriple,
    dprime: &LeakyTriple,
) -> LeakyTriple {
    // PRECONDITION: both triples share (n, m, delta_a, delta_b). The outer
    // combine_leaky_triples already asserts this, but re-assert for unit-test safety.
    assert_eq!(prime.n, dprime.n, "two_to_one_combine: n mismatch");
    assert_eq!(prime.m, dprime.m, "two_to_one_combine: m mismatch");
    assert_eq!(prime.delta_a.as_block(), dprime.delta_a.as_block(), "delta_a mismatch");
    assert_eq!(prime.delta_b.as_block(), dprime.delta_b.as_block(), "delta_b mismatch");
    let n = prime.n;
    let m = prime.m;
    let delta_a = prime.delta_a;
    let delta_b = prime.delta_b;

    // ---- Step A: assemble d shares (paper line 428: d := y' XOR y'') ----
    // AuthBitShare + AuthBitShare is XOR field-wise per src/sharing.rs:66-77.
    let gen_d:  Vec<AuthBitShare> = (0..m).map(|j|  prime.gen_y_shares[j]  + dprime.gen_y_shares[j]).collect();
    let eval_d: Vec<AuthBitShare> = (0..m).map(|j|  prime.eval_y_shares[j] + dprime.eval_y_shares[j]).collect();

    // ---- Step B: MAC-verify d and extract d bits (in-process substitute for
    //             "publicly reveal with appropriate MACs", paper line 428) ----
    let mut d_bits: Vec<bool> = Vec::with_capacity(m);
    for j in 0..m {
        verify_cross_party(&gen_d[j], &eval_d[j], &delta_a, &delta_b);
        d_bits.push(gen_d[j].value ^ eval_d[j].value);
    }

    // ---- Step C: x = x' XOR x'' (paper line 427) ----
    let gen_x: Vec<AuthBitShare>  = (0..n).map(|i|  prime.gen_x_shares[i]  + dprime.gen_x_shares[i]).collect();
    let eval_x: Vec<AuthBitShare> = (0..n).map(|i|  prime.eval_x_shares[i] + dprime.eval_x_shares[i]).collect();

    // ---- Step D: Z = Z' XOR Z'' XOR (x'' tensor d)  (paper lines 430-443) ----
    // Column-major: k = j*n + i. ZERO-share when d[j] == 0.
    let zero_share = AuthBitShare::default();  // key=0, mac=0, value=false (src/sharing.rs:42)
    let mut gen_z:  Vec<AuthBitShare> = Vec::with_capacity(n * m);
    let mut eval_z: Vec<AuthBitShare> = Vec::with_capacity(n * m);
    for j in 0..m {
        for i in 0..n {
            let k = j * n + i;
            // Rightmost term: x''_i if d[j] else ZERO
            let dx_gen  = if d_bits[j] { dprime.gen_x_shares[i]  } else { zero_share };
            let dx_eval = if d_bits[j] { dprime.eval_x_shares[i] } else { zero_share };
            gen_z.push( prime.gen_z_shares[k]  + dprime.gen_z_shares[k]  + dx_gen);
            eval_z.push(prime.eval_z_shares[k] + dprime.eval_z_shares[k] + dx_eval);
        }
    }

    // ---- Step E: y = y' (paper line 427) ----
    let gen_y  = prime.gen_y_shares;   // move, not clone — prime is owned
    let eval_y = prime.eval_y_shares;

    LeakyTriple {
        n, m, delta_a, delta_b,
        gen_x_shares: gen_x,
        gen_y_shares: gen_y,
        gen_z_shares: gen_z,
        eval_x_shares: eval_x,
        eval_y_shares: eval_y,
        eval_z_shares: eval_z,
    }
}
```

### Anti-Patterns to Avoid

- **Naïve XOR of all Z shares (the current bug).** `src/auth_tensor_pre.rs:71-80` XOR-combines Z across all B triples — this is wrong per the paper. It silently passes dimension tests but fails TEST-03/TEST-05 product invariants. The Phase 5 rewrite replaces this with iterative `two_to_one_combine`.
- **Calling `share.verify(&delta)` on a raw cross-party `AuthBitShare`.** It panics with `"MAC mismatch in share"` because `gen.key` comes from A's sender direction while `gen.mac` comes from B's sender direction — the MAC commits under `Δ_B`, not a single delta pair. Always use `verify_cross_party`.
- **`n*m` in the bucket formula.** The existing `bucket_size_for(n, m)` uses `ell = n * m` — a direct bug. Replace with `bucket_size_for(ell: usize)` per D-08.
- **Moving `y` combining into the loop.** The paper is precise: `y := y'` (NOT `y' ⊕ y''`). Combining y would break the algebra because the correctness proof (paper lines 430-436) relies on `y = y' = y''` after `d = y' ⊕ y''` is added in.
- **Forgetting the `d[j] == 0` case yielding the zero share.** Always use `AuthBitShare::default()` (not `prime.gen_x_shares[i]`) when `d[j] == 0`. If you skip the XOR entirely (omit the `dx_gen` term), you lose composability when later Z terms depend on bucket-level XOR accumulators.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| XOR two `AuthBitShare`s field-wise | Custom `fn xor_shares(a, b)` | `a + b` via `Add` impl (`src/sharing.rs:66-117`) | Four `Add` overloads for owned/borrowed combinations already exist. |
| Construct a zero `AuthBitShare` | Manual `AuthBitShare { key: Key::default(), mac: Mac::default(), value: false }` | `AuthBitShare::default()` | `#[derive(Default)]` at `src/sharing.rs:42`. |
| Verify IT-MAC on a d share | Rebuild `mac == key XOR bit·delta` from scratch | `verify_cross_party` helper (verbatim copy) | Already in `src/auth_tensor_pre.rs:134-152`. |
| Generate two concrete `LeakyTriple`s for TEST-05 | Manual struct construction with crafted bits | `LeakyTensorPre::new(seed, n, m, &mut bcot).generate()` | The existing `make_triples` helper at `src/auth_tensor_pre.rs:120-129` already does this. Just extend count. |
| Integer log₂ | Custom log function with edge-case handling | `(usize::BITS - ell.leading_zeros() - 1) as usize` | Already used at `src/auth_tensor_pre.rs:19` (keep, just apply to `ell` not `n*m`). Guard with `ell ≥ 2` check per D-09. |
| Panic on protocol violation | `Result<Error>` | `assert!` / `panic!` | Codebase convention (`.planning/codebase/CONVENTIONS.md:105-111`): never use `Result`, always panic. |

**Key insight:** Everything Phase 5 needs is already in-crate. The only new primitive is the paper's algebra, which is three XORs and one conditional — there is nothing to build from scratch.

---

## Runtime State Inventory

Phase 5 is a pure in-process algorithm rewrite of two Rust functions plus tests. There is no persisted state, no external service, no OS-level registration, no secrets, no build-artifact caching affected by the rename of `bucket_size_for`. The change propagates at compile time: any caller of the old `bucket_size_for(n, m)` signature that is not updated will fail to build, so there is no silent drift risk.

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | None — verified by Grep. No database, no persistence layer, no cache. | None. |
| Live service config | None — verified. Protocol is in-process only; `IdealBCot`/`feq` are both ideal functionalities with no external service. | None. |
| OS-registered state | None — verified. No OS tasks, no daemons, no launched processes. | None. |
| Secrets/env vars | None — verified. No env vars read in `src/auth_tensor_pre.rs` or `src/preprocessing.rs`. | None. |
| Build artifacts | Only `target/` — regenerated on `cargo build`. | None. |

**Cross-codebase `bucket_size_for` call sites** [VERIFIED: grep over src/]:

1. `src/preprocessing.rs:87` — `let bucket_size = bucket_size_for(n, m);` — update to `bucket_size_for(count)` per D-10.
2. `src/auth_tensor_pre.rs:157` — test `test_bucket_size_formula` — update assertions to the new signature values (B for given `ell`).
3. `src/auth_tensor_pre.rs:177` — test `test_full_pipeline_no_panic` — calls `bucket_size_for(n, m)`; update to `bucket_size_for(1)`.

No other callers.

---

## Common Pitfalls

### Pitfall 1: Silent Z corruption from naïve XOR
**What goes wrong:** Current code XORs all B triples' Z shares together, producing a Z that does not satisfy `z = x ⊗ y` for the combined x, y.
**Why it happens:** Easy to read the paper's "combine" language as "XOR" without tracking the `x'' ⊗ d` correction.
**How to avoid:** Implement the full Step D in `two_to_one_combine` (paper line 443): `Z := Z' ⊕ Z'' ⊕ itmac{x''}{Δ} ⊗ d`. TEST-05 catches this regression end-to-end.
**Warning signs:** `test_leaky_triple_product_invariant` passes for `bucket_size = 1` (no combining) but fails for `bucket_size > 1`.

### Pitfall 2: `AuthBitShare::verify` panics on cross-party shares
**What goes wrong:** Calling `gen_d_share.verify(&delta_b)` directly will panic with `"MAC mismatch in share"` even on a correctly-formed cross-party share.
**Why it happens:** `gen_d_share.key` is A's sender key for y from `cot_y_a_to_b`; `gen_d_share.mac` is A's MAC for y from `cot_y_b_to_a` — they are committed under different deltas.
**How to avoid:** Always use `verify_cross_party(&gen_d, &eval_d, &Δ_A, &Δ_B)` which reassembles the two properly-aligned pairs internally.
**Warning signs:** MAC mismatch panics in tests that *should* pass.

### Pitfall 3: `log2(ell)` undefined / underflow for `ell ≤ 1`
**What goes wrong:** `(usize::BITS - ell.leading_zeros() - 1) as usize` for `ell = 0` or `ell = 1` produces a wrong or panicking result.
**Why it happens:** `0u64.leading_zeros() == 64` so `BITS - 64 - 1` underflows; `1u64.leading_zeros() == 63` so `log2 = 0` → division by zero.
**How to avoid:** Branch on `ell <= 1` per D-09: return `SSP` directly. Test this boundary.
**Warning signs:** Integer underflow panic in debug builds, wrong `B` silently in release builds.

### Pitfall 4: Ownership / borrow of `LeakyTriple` in the fold
**What goes wrong:** `two_to_one_combine(prime: LeakyTriple, dprime: &LeakyTriple)` consumes `prime` and borrows `dprime`. If you write `let acc = triples[0]; for i in 1..B { acc = two_to_one_combine(acc, triples[i]) }` without cloning, the loop fails to compile because `triples[0]` is not `Copy`.
**Why it happens:** `LeakyTriple` contains `Vec<AuthBitShare>`, which is `Clone` but not `Copy`.
**How to avoid:** Clone `triples[0]` into `acc` before the loop, then iterate with `triples.iter().skip(1)` to get `&LeakyTriple` for `dprime`.
**Warning signs:** `cannot move out of index of ...` compile error.

### Pitfall 5: Column-major index confusion in `x'' ⊗ d`
**What goes wrong:** Using `i*m + j` instead of `j*n + i` in the Z loop, producing a transposed product.
**Why it happens:** Some matrix libraries use row-major; this codebase consistently uses column-major (see `src/matrix.rs:22-27`, `src/leaky_tensor_pre.rs:42`).
**How to avoid:** Copy the nested-loop shape from `src/leaky_tensor_pre.rs:240-247` (outer `j in 0..m`, inner `i in 0..n`, `k = j*n + i`).
**Warning signs:** TEST-05 product invariant fails with a consistent index-swap pattern (z_ij appears at position (j, i)).

### Pitfall 6: Forgetting to exclude the `alpha_labels` / `beta_labels` stubs from semantic verification
**What goes wrong:** `TensorFpreGen.alpha_labels` and `beta_labels` are `Vec::new()` stubs (per Phase 4 D-07); subsequent online-phase code may try to use them.
**Why it happens:** The `AuthTensorGen::new_from_fpre_gen(fpre)` still accepts the struct but may index into empty label vectors.
**How to avoid:** The existing `test_full_pipeline_no_panic` test at `src/auth_tensor_pre.rs:174-183` only checks construction, not use. Phase 5 does NOT fix this deeper issue — it is a scope-boundary decision explicitly locked in Phase 4 (CONTEXT.md for Phase 4, D-07). Do not expand scope.
**Warning signs:** Integration tests that use the online AuthTensor pipeline panic; those tests are out of scope for Phase 5.

---

## Code Examples

### Example 1: Correct `bucket_size_for(ell)` (PROTO-12)

```rust
// Source: NEW — replacing src/auth_tensor_pre.rs:15-21
// Paper: appendix_krrw_pre.tex line 484 — B = ⌊ssp / log ℓ⌋ + 1 for ℓ ≥ 2.
//        For ℓ ≤ 1, naïve (non-bucketed) combining needs SSP triples per the §3.1
//        opening paragraph ("combining ρ such triples into one").

/// Compute the bucket size B for Pi_aTensor (Construction 3, Theorem 1).
///
/// Formula: `B = floor(SSP / log2(ell)) + 1` for `ell ≥ 2`, where SSP = 40.
/// For `ell ≤ 1`, the bucketing amplification is degenerate; fall back to
/// the naïve combining bound of B = SSP (paper §3.1 preamble).
///
/// Parameters:
///   ell — number of OUTPUT authenticated tensor triples desired (NOT n·m).
///
/// Examples (ell = count of output triples):
///   bucket_size_for(1)    = 40     (no amortization)
///   bucket_size_for(2)    = 41     (log2 = 1, so 40 + 1)
///   bucket_size_for(16)   = 11     (floor(40/4) + 1)
///   bucket_size_for(128)  = 6      (floor(40/7) + 1)
///   bucket_size_for(1024) = 5      (floor(40/10) + 1)
pub fn bucket_size_for(ell: usize) -> usize {
    const SSP: usize = 40;
    if ell <= 1 {
        return SSP;
    }
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

### Example 2: Updated `combine_leaky_triples` — thin fold wrapper (PROTO-11)

See **Architecture Patterns → Pattern 3** above for the full skeleton.

### Example 3: `two_to_one_combine` — the paper's algebra (PROTO-10)

See **Architecture Patterns → Pattern 4** above for the full skeleton.

### Example 4: TEST-05 happy-path + tamper-path (TEST-05)

```rust
// Source: NEW in src/auth_tensor_pre.rs test module. Leverages existing make_triples helper.
use super::{two_to_one_combine, make_triples};
use crate::sharing::AuthBitShare;

#[test]
fn test_two_to_one_combine_product_invariant() {
    // TEST-05 happy path: two concrete leaky triples, combine, verify
    // Z_combined[j*n+i] = x_combined[i] AND y_combined[j] (paper Theorem correctness).
    let n = 4;
    let m = 4;
    let triples = make_triples(n, m, 2);
    let t0 = triples[0].clone();
    let t1_ref = &triples[1];

    let combined = two_to_one_combine(t0, t1_ref);

    // MAC invariants on combined shares (sanity that d-reveal didn't corrupt anything).
    for i in 0..n {
        verify_cross_party(&combined.gen_x_shares[i], &combined.eval_x_shares[i],
                           &combined.delta_a, &combined.delta_b);
    }
    for j in 0..m {
        verify_cross_party(&combined.gen_y_shares[j], &combined.eval_y_shares[j],
                           &combined.delta_a, &combined.delta_b);
    }
    for k in 0..(n * m) {
        verify_cross_party(&combined.gen_z_shares[k], &combined.eval_z_shares[k],
                           &combined.delta_a, &combined.delta_b);
    }

    // Product invariant: Z_full[j*n+i] == x_full[i] AND y_full[j].
    let x_full: Vec<bool> = (0..n)
        .map(|i| combined.gen_x_shares[i].value ^ combined.eval_x_shares[i].value)
        .collect();
    let y_full: Vec<bool> = (0..m)
        .map(|j| combined.gen_y_shares[j].value ^ combined.eval_y_shares[j].value)
        .collect();
    for j in 0..m {
        for i in 0..n {
            let k = j * n + i;
            let z_full = combined.gen_z_shares[k].value ^ combined.eval_z_shares[k].value;
            assert_eq!(z_full, x_full[i] & y_full[j],
                "TEST-05 product invariant failed at (i={}, j={})", i, j);
        }
    }
}

#[test]
#[should_panic(expected = "MAC mismatch in share")]
fn test_two_to_one_combine_tampered_d_panics() {
    // TEST-05 tamper path: flip one y'' value bit — the MAC-verify on d
    // will notice the d share is inconsistent and abort.
    let n = 2;
    let m = 2;
    let triples = make_triples(n, m, 2);
    let t0 = triples[0].clone();
    let mut t1 = triples[1].clone();

    // Tamper: flip the value of eval_y_shares[0] without touching the MAC.
    // This makes the assembled d[0] share fail verify_cross_party.
    t1.eval_y_shares[0].value = !t1.eval_y_shares[0].value;

    let _ = two_to_one_combine(t0, &t1);  // must panic
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `bucket_size_for(n, m) with ell = n·m` | `bucket_size_for(ell)` with `ell` = output-triple count | Phase 5 | Breaking signature change; all call sites updated Wave 0. |
| Naïve XOR combine of B z shares | Iterative two-to-one combine with `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d` | Phase 5 | Fixes a silent correctness bug; TEST-05 enforces. |
| "Keep `x = x'`" (ROADMAP typo) | `x = x' ⊕ x''` per paper line 427 | Phase 5 (clarified in CONTEXT.md D-01) | Prevents downstream semantic bugs in the online phase. |

**Deprecated/outdated:**
- The doc-comment at `src/auth_tensor_pre.rs:32-37` claims "XOR of B independent AuthBitShares with the same delta preserves the MAC invariant" — true for the MAC field alone, but the *algebraic* invariant `z = x ⊗ y` is NOT preserved under naïve XOR. Doc-comment must be replaced to describe the new two-to-one combining semantics.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | — | — | — |

**All claims in this research were verified against the paper (`references/appendix_krrw_pre.tex`, lines 415–546) and/or the existing codebase (read in full: `src/auth_tensor_pre.rs`, `src/leaky_tensor_pre.rs`, `src/sharing.rs`, `src/preprocessing.rs`, `src/keys.rs`, `src/macs.rs`, `src/bcot.rs`, `src/feq.rs`, `src/delta.rs`, `src/matrix.rs`, `src/block.rs`, `src/lib.rs`) — no user confirmation needed.**

The only subtlety is the precise encoding of the `AuthBitShare` "zero share" when `d[j] == 0`. Per D-03 the paper requires a zero contribution; the codebase provides exactly this via `AuthBitShare::default()` (`#[derive(Default)]` at `src/sharing.rs:42`, producing `key = Key::default() = Key(Block::ZERO)`, `mac = Mac::default() = Mac(Block::ZERO)`, `value = false`). The XOR of any share with this zero share is itself (identity), which matches the paper algebra.

---

## Open Questions

None — CONTEXT.md is definitive on every Phase 5 decision, and the paper algebra (lines 427–443) is precise enough that no additional research is required. The `d` MAC-verify semantics were explicitly chosen in CONTEXT.md D-06 (in-process `verify_cross_party`).

---

## Environment Availability

Skipped — Phase 5 is pure Rust algorithm work in an existing, verified workspace. No external tools, services, runtimes, or CLI utilities beyond `rustc`/`cargo` (both [VERIFIED: 1.90.0] in Step 2.6).

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness (cargo test) |
| Config file | `Cargo.toml` (no separate test config) |
| Quick run command | `cargo test --lib 2>&1 \| tail -30` |
| Full suite command | `cargo test 2>&1 \| tail -40` |
| Estimated runtime | ~5-10 seconds (current baseline: ~0.03s for 66 tests) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROTO-10 | `two_to_one_combine` implements correct two-to-one combining (x = x' ⊕ x'', y = y', Z = Z' ⊕ Z'' ⊕ x'' ⊗ d) | unit + integration | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_product_invariant -x` | ❌ Wave 0 (new test) |
| PROTO-10 | MAC verify on d shares rejects tampered y'' | integration (#[should_panic]) | `cargo test --lib auth_tensor_pre::tests::test_two_to_one_combine_tampered_d_panics -x` | ❌ Wave 0 (new test) |
| PROTO-11 | Iterative combining folds B triples one at a time | integration | `cargo test --lib auth_tensor_pre::tests::test_full_pipeline_no_panic -x` | ✅ existing — re-enabled with new semantics |
| PROTO-11 | Combined triple satisfies product invariant Z = x ⊗ y for full bucket | integration | `cargo test --lib auth_tensor_pre::tests::test_combine_full_bucket_product_invariant -x` | ❌ Wave 0 (new test) |
| PROTO-12 | `bucket_size_for(ell)` returns correct B for various ell values | unit | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula -x` | ✅ existing — update expected values to the new formula |
| PROTO-12 | `bucket_size_for(ell ≤ 1)` returns SSP fallback | unit | `cargo test --lib auth_tensor_pre::tests::test_bucket_size_formula_edge_cases -x` | ❌ Wave 0 (new test) |
| TEST-05 | Covered by PROTO-10 tests above | see above | see above | see above |

### Sampling Rate
- **Per task commit:** `cargo test --lib 2>&1 | tail -30`
- **Per wave merge:** `cargo test 2>&1 | tail -40`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `src/auth_tensor_pre.rs` — update `bucket_size_for` signature and body (formula fix)
- [ ] `src/preprocessing.rs` — update `bucket_size_for(n, m)` call site to `bucket_size_for(count)`
- [ ] `src/auth_tensor_pre.rs` — `test_bucket_size_formula` assertions updated for new formula
- [ ] `src/auth_tensor_pre.rs` — new test `test_bucket_size_formula_edge_cases` for `ell ≤ 1`
- [ ] `src/auth_tensor_pre.rs` — new `pub(crate) fn two_to_one_combine` skeleton (body can start as `unimplemented!()` in Wave 0, filled in Wave 1)
- [ ] `src/auth_tensor_pre.rs` — new test stubs `test_two_to_one_combine_product_invariant` and `test_two_to_one_combine_tampered_d_panics` (as `unimplemented!()` placeholders)
- [ ] `src/auth_tensor_pre.rs` — new test `test_combine_full_bucket_product_invariant` (product invariant over full bucket)

Framework install: not needed — `cargo test` already runs.

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | N/A — protocol is library-internal; no user-facing auth. |
| V3 Session Management | no | N/A. |
| V4 Access Control | no | N/A. |
| V5 Input Validation | yes | Assertions on triple dimensions, same-delta, bucket size ≥ 1, `ell` edge cases — all via `assert!` / `assert_eq!` panics (codebase convention). |
| V6 Cryptography | yes | All MAC/key operations go through `AuthBitShare::verify`, `Key::auth`, and `AuthBitShare + AuthBitShare` — never hand-rolled. Reuses Phase 4 primitives verbatim. |

### Known Threat Patterns for Authenticated Bit Sharing Combining

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tampered `y''` during d reveal | Tampering | `verify_cross_party` on each assembled d share — aborts on MAC mismatch (test-05 tamper path covers this). |
| Triples from different `IdealBCot` instances mixed in one bucket | Tampering | Same-delta assertion at `src/auth_tensor_pre.rs:53-68`; kept in place per CONTEXT.md. |
| Adversary guesses bits of individual leaky triples | Information disclosure | Bucket-size amplification (`B = ⌊SSP/log ℓ⌋ + 1`) gives 2^−SSP = 2^−40 statistical error per paper Theorem 1 (line 484). |
| Cross-bucket interference | — | Each call to `combine_leaky_triples` processes ONE bucket; `run_preprocessing` runs the fold independently per output triple. |
| `bucket_size_for` edge cases (ell=0, ell=1) | Denial of service (crash) | Guard `if ell <= 1 { return SSP; }` (D-09); test the boundary in Wave 0. |

**Non-threats (explicitly out of scope):**
- Network attacks — not in scope; `feq` and `IdealBCot` are in-process ideal functionalities (Phase 4 TODO: replace with real protocols in v2).
- Timing side-channels — not in scope for correctness phase; the entire library consists of constant-branching field operations over `Block` (XORs and table lookups), which is already side-channel-hardened by construction.

---

## Sources

### Primary (HIGH confidence)

- `references/appendix_krrw_pre.tex` §3.1 "Combining Leaky Tensor Triples" (lines 415–444) — the two-to-one combine formula `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d`, d definition, public reveal, and local `x'' ⊗ d` computation without interaction (line 437). [VERIFIED: direct file read]
- `references/appendix_krrw_pre.tex` §3.2 "Randomized Bucketing" Construction 3 (lines 449–535) — iterative combining across B triples per bucket (line 474), bucket size `B = ⌊ssp / log ℓ⌋ + 1` (line 484), Theorem 1 statistical error bound `2ℓ^{1−B}`. [VERIFIED: direct file read]
- `src/auth_tensor_pre.rs` (187 lines, read in full) — existing `combine_leaky_triples` structure, `bucket_size_for` formula (to be fixed), `make_triples` test helper, `verify_cross_party` helper, and the XOR combining pattern that Phase 5 retains for x. [VERIFIED]
- `src/leaky_tensor_pre.rs` (648 lines, read in full) — `LeakyTriple` struct definition (field names locked in Phase 4), cross-party layout doc-comment (lines 29–35), `verify_cross_party` canonical helper (lines 344–362), `LeakyTensorPre::generate()` producing test fixtures. [VERIFIED]
- `src/sharing.rs` (209 lines, read in full) — `AuthBitShare` struct (line 42, `#[derive(Default)]`), four `Add` overloads (lines 66–117), `verify` method (lines 59–63, `"MAC mismatch in share"` abort message). [VERIFIED]
- `src/preprocessing.rs` (135 lines, read in full) — `run_preprocessing` pipeline and the `bucket_size_for(n, m)` call site at line 87. [VERIFIED]
- `.planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-CONTEXT.md` — all D-01 through D-12 locked decisions. [VERIFIED]
- `.planning/phases/04-m2-pi-leakytensor-f-eq-construction-2/04-CONTEXT.md` — upstream D-06/D-07/D-08 locking `LeakyTriple` field names and Z column-major storage. [VERIFIED]
- `.planning/phases/04-m2-pi-leakytensor-f-eq-construction-2/04-PATTERNS.md` — verified pattern catalog for cross-party `AuthBitShare` assembly, inline test skeleton, `#[should_panic]` convention. [VERIFIED]
- `.planning/codebase/CONVENTIONS.md` — MAC invariant rules, Key LSB=0, cross-party MAC layout, column-major tensor indexing, "protocol violation = `panic!`" convention (lines 105–111). [VERIFIED]
- `cargo test --lib` output 2026-04-22 — 66 tests pass, including all Phase 4 leaky-triple invariants that Phase 5 depends on. [VERIFIED]
- `rustc 1.90.0` / `cargo 1.90.0` — verified via `rustc --version` and `cargo --version`. [VERIFIED]

### Secondary (MEDIUM confidence)

None — all claims are grounded in primary sources.

### Tertiary (LOW confidence)

None.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; every primitive already in-crate and verified by passing tests.
- Architecture: HIGH — paper formula is precise (lines 427–443 for combine, lines 449–535 for bucketing); existing `LeakyTriple` / `AuthBitShare` APIs support direct composition.
- Pitfalls: HIGH — all six pitfalls are grounded in concrete codebase patterns (naïve XOR bug is in the file being rewritten; cross-party verify panic is documented in `src/leaky_tensor_pre.rs:29-35`; ownership pattern follows standard Rust fold idiom).

**Research date:** 2026-04-22
**Valid until:** 2026-05-22 (30 days — protocol spec and codebase are stable; no upstream drift risk)
