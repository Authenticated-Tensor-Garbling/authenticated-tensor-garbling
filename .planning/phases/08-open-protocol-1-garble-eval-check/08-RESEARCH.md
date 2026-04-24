# Phase 8: Open() + Protocol 1 Garble/Eval/Check - Research

**Researched:** 2026-04-23
**Domain:** Authenticated 2PC online phase — Protocol 1 garble/eval/CheckZero (paper §5)
**Confidence:** HIGH

## Summary

Phase 8 closes the Protocol 1 online loop on top of the Phase 7 preprocessing trait. Construction 3 (paper `5_online.tex`, lines 111-167) defines two new primitives that must land:

1. `compute_lambda_gamma()` on `AuthTensorGen` and `AuthTensorEval` — produces `[L_gamma]^gb` (a `Vec<bool>` of length `n*m`) on the garbler side and reconstructs `L_gamma` on the evaluator side. This realises the `[L_gamma]^gb := extbit([v_gamma D_gb]^gb) XOR extbit([l_gamma D_gb]^gb)` line of the paper, which the existing `garble_final()` does not yet emit.
2. `check_zero()` in a new `src/online.rs` — verifies a precomputed `Vec<AuthBitShare>` of `[c_gamma D_ev]` reconstructs to zero and that the IT-MAC under `delta_ev` checks out. `c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma) = v_alpha ⊗ v_beta XOR v_gamma`, which equals 0 for honest parties.

The `gamma_auth_bit_shares` field added in Phase 7 (D_ev-shares of `l_gamma`, length `n*m`, column-major) must finally be forwarded from `TensorFpreGen`/`TensorFpreEval` to new fields on `AuthTensorGen`/`AuthTensorEval`. The `TODO(Phase 8)` comments at `src/auth_tensor_gen.rs:64` and `src/auth_tensor_eval.rs:57` mark exactly where the two-line edits live.

`open()` (ONL-01, ONL-02) is **deferred** by user decision D-01 and is out of scope. `online.rs` exists after this phase but contains only `check_zero()` until a later phase adds `open()` plus its message-passing design.

**Primary recommendation:** Land work in three tightly-scoped tasks: (1) forward `gamma_auth_bit_shares` field on both protocol structs and add `compute_lambda_gamma()` methods (both auth_tensor_gen and auth_tensor_eval files; symmetric); (2) create `src/online.rs` with `check_zero(c_gamma_shares, delta_ev) -> bool` and wire it into `lib.rs`; (3) end-to-end positive + tampered-mask negative test extending the existing `test_auth_tensor_product` in `src/lib.rs`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**open() — Deferred**

- **D-01:** `open()` and its wrong-delta negative test (ONL-01, ONL-02) are **out of scope for Phase 8**. They will be implemented in a later phase.

**Module Layout**

- **D-02:** `src/online.rs` is created in this phase. It hosts `check_zero()` only. `open()` is added here in a later phase.
- **D-03:** Protocol 1 garble/eval logic (`compute_lambda_gamma()`) stays in `src/auth_tensor_gen.rs` and `src/auth_tensor_eval.rs` as new methods on the existing structs. `online.rs` stays thin.

**L_gamma Computation**

- **D-04:** `AuthTensorGen::compute_lambda_gamma() -> Vec<bool>` — new method. Computes the garbler's masked output share per (i,j):
  ```
  [L_gamma]^gb[j*n+i] = first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()
  ```
  `first_half_out` holds `[v_gamma D_gb]^gb` after `garble_final()`; `gamma_auth_bit_shares` holds `[l_gamma D_gb]^gb` (Phase 7, D-04/D-05).
- **D-05:** `AuthTensorEval::compute_lambda_gamma(lambda_gb: &[bool]) -> Vec<bool>` — new method, takes the garbler's `[L_gamma]^gb` vec as input. Computes the evaluator's masked output per (i,j):
  ```
  L_gamma[j*n+i] = lambda_gb[j*n+i] XOR first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()
  ```
- **D-06:** The `TODO(Phase 8)` comments in `src/auth_tensor_gen.rs:64` and `src/auth_tensor_eval.rs:57` must be resolved — `gamma_auth_bit_shares` from `TensorFpreGen`/`TensorFpreEval` is forwarded to a new `gamma_auth_bit_shares: Vec<AuthBitShare>` field on `AuthTensorGen`/`AuthTensorEval` respectively.

**CheckZero**

- **D-07:** `check_zero()` signature: `fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool`. Returns `true` (pass) or `false` (abort).
- **D-08:** Callers pre-compute `c_gamma` from the D_ev-authenticated shares of `l_alpha`, `l_beta`, `l_gamma`, `l_gamma*` and pass the combined share vec. `check_zero()` is a thin primitive — it does not know about the struct types.
- **D-09:** `c_gamma` formula per gate (from `5_online.tex`):
  ```
  c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)
           = v_alpha ⊗ v_beta XOR v_gamma   [= 0 for honest parties]
  ```
  This is a linear combination of `[l_alpha D_ev]`, `[l_beta D_ev]`, `[l_gamma D_ev]`, `[l_gamma* D_ev]` with coefficients determined by the `L` values. Computed locally from preprocessing shares.

### Claude's Discretion

- Exact `check_zero()` MAC verification mechanic (e.g., sum-of-MACs XOR expected value vs per-share checks) — use whatever is consistent with `AuthBitShare::verify()` patterns.
- Whether `gamma_auth_bit_shares` new field on `AuthTensorGen`/`AuthTensorEval` uses `Vec<AuthBitShare>` (matching TensorFpreGen/Eval) or is stored differently — match the existing field type.
- Indexing order for `compute_lambda_gamma()` output — use column-major `j * n + i` consistent with all other n×m field vecs.

### Deferred Ideas (OUT OF SCOPE)

- `open()` free function (ONL-01) — future phase. Will live in `src/online.rs` once designed.
- `open()` wrong-delta negative test (ONL-02) — same future phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ONL-01 | `open()` free function in `src/online.rs` | **DEFERRED** by D-01 — not addressed in Phase 8. Module skeleton (`src/online.rs`) is created so future phase can drop in `open()`. |
| ONL-02 | `open()` wrong-delta negative test | **DEFERRED** by D-01 — paired with ONL-01. |
| P1-01 | Protocol 1 garble algorithm complete — two `tensorgb` calls, XOR with `[l_gamma* D_gb]^gb` | Existing `garble_first_half/second_half/final` already implement the two `tensorgb` calls + l_gamma* XOR (verified — see `garble_final` lines 179-193). The remaining garble step is emitting `[L_gamma]^gb` via `compute_lambda_gamma()` (D-04). |
| P1-02 | Protocol 1 evaluate algorithm complete — `tensorev` calls, produces masked output `Λ_gamma` | Existing `evaluate_first_half/second_half/final` already implement the two `tensorev` calls + l_gamma* MAC XOR. `compute_lambda_gamma(&lambda_gb)` (D-05) finishes the masked output reconstruction. |
| P1-03 | Protocol 1 consistency check (CheckZero) — local `c_gamma` from D_ev-MAC'd shares including `l_gamma*` term; CheckZero verifies | `check_zero()` in `src/online.rs` (D-02, D-07). Caller-side combiner builds `c_gamma_shares` from `alpha_auth_bit_shares`, `beta_auth_bit_shares`, `correlated_auth_bit_shares` (l_gamma*), and `gamma_auth_bit_shares` (l_gamma) per the formula in D-09. |
| P1-04 | Protocol 1 end-to-end test: `Z_gb XOR Z_ev == correct tensor product` (extends v1.0 battery) | Extend `src/lib.rs::test_auth_tensor_product` to call the new `compute_lambda_gamma()` methods after the existing `garble_final/evaluate_final` and assert correctness of `L_gamma == (input_x ⊗ input_y) XOR l_gamma`. |
| P1-05 | CheckZero negative test: tampered `L_gamma` causes abort | New test that flips one entry of `lambda_gb` (or one entry of an `L` value used in c_gamma assembly) before evaluator runs `check_zero()` — assert `check_zero()` returns `false`. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| `compute_lambda_gamma` (gen side) | `auth_tensor_gen.rs` (struct method) | — | D-03: lives next to `garble_final` because it consumes `first_half_out`. |
| `compute_lambda_gamma` (eval side) | `auth_tensor_eval.rs` (struct method) | — | D-03 + symmetry with gen side. |
| `gamma_auth_bit_shares` field forwarding | `auth_tensor_gen.rs` / `auth_tensor_eval.rs` constructors | — | TODO comments already mark the lines. |
| `check_zero()` primitive | `src/online.rs` (new module) | — | D-02: thin primitive, decoupled from struct types. |
| `c_gamma` share assembly (linear combination) | Test harness / caller code in `src/lib.rs` | Future phase may extract a helper | D-08: caller pre-computes `c_gamma`; `check_zero()` is generic. Phase 8 keeps it inline in tests; if a helper proves repetitive across protocols 1 and 2, extract later. |
| End-to-end Protocol 1 verification (positive + negative) | `src/lib.rs::tests` | Could grow to a separate `online.rs` test module | Mirrors existing `test_auth_tensor_product` location and bench setup. |
| `lib.rs` module declaration | `src/lib.rs` | — | Add `pub mod online;` next to existing `pub mod preprocessing;`. |

## Standard Stack

### Core (Already Present — Use As-Is)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust` | 2024 edition | Language | Project standard [VERIFIED: Cargo.toml + project conventions doc] |
| `rand_chacha` | (project pinned) | Deterministic test RNG (`ChaCha12Rng::seed_from_u64`) | Used everywhere for test determinism — see `bcot.rs`, `auth_tensor_fpre.rs`, `preprocessing.rs` [VERIFIED: grep] |
| `rand` | (project pinned) | `Rng::random_bool` etc. | Same as above [VERIFIED: grep] |

### Supporting (Internal Modules)

| Module | Purpose | When to Use |
|--------|---------|-------------|
| `crate::sharing::AuthBitShare` | Per-party authenticated bit (key + mac + value) | `gamma_auth_bit_shares` field type; `c_gamma_shares` parameter type for `check_zero()` |
| `crate::sharing::AuthBitShare::Add` impls | XOR-combine two shares (key, mac, value all XORed) | Building `c_gamma` linear combination from constituent `[l D_ev]` shares |
| `crate::delta::Delta` | Global correlation key, LSB=1 invariant | `delta_ev` parameter to `check_zero()`; reconstruction of bits inside `check_zero()` |
| `crate::block::Block::lsb()` | Extract pointer/extbit from a Block | `extbit` in paper notation = `block.lsb()`; used in `compute_lambda_gamma()` |
| `crate::preprocessing::TensorFpreGen` / `TensorFpreEval` | Source of `gamma_auth_bit_shares` (Phase 7 added field) | Forward in `new_from_fpre_gen` / `new_from_fpre_eval` |

**No new external dependencies required.** [VERIFIED: All needed primitives already in-tree.]

### Alternatives Considered (Rejected per CONTEXT.md)

| Instead of | Could Use | Tradeoff | Rejected because |
|------------|-----------|----------|------------------|
| Method `compute_lambda_gamma` on struct | Free function in `online.rs` | Cleaner separation | D-03 (decision tree explored in DISCUSSION-LOG) |
| `check_zero` returning `Result<(), CheckZeroError>` | Idiomatic Rust fallible | Better error info | D-07 → `bool` to match codebase precedent (`AuthBitShare::verify` uses panics; `check_zero()` chose `bool` for non-panic call site) |
| `check_zero(gen, eval)` taking the structs | Tighter coupling | Less plumbing | D-08 → keep `check_zero()` thin; caller assembles |

**Installation:** No new crates needed. Verified via inspection of `Cargo.toml`-equivalent declarations in source. [VERIFIED: src grep shows no new external types]

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────┐                                       ┌─────────────────────┐
│   TensorFpreGen     │                                       │   TensorFpreEval    │
│  (Phase 7 output)   │                                       │  (Phase 7 output)   │
│   alpha_auth_bit    │                                       │   alpha_auth_bit    │
│   beta_auth_bit     │                                       │   beta_auth_bit     │
│   correlated (l_g*) │                                       │   correlated (l_g*) │
│   gamma_auth (l_g)  │                                       │   gamma_auth (l_g)  │
└─────────┬───────────┘                                       └──────────┬──────────┘
          │ new_from_fpre_gen                                            │ new_from_fpre_eval
          ▼                                                              ▼
┌─────────────────────┐                                       ┌─────────────────────┐
│   AuthTensorGen     │                                       │   AuthTensorEval    │
│  + gamma_auth_bit   │                                       │  + gamma_auth_bit   │
│       _shares       │   ◀── PHASE 8 ADDS THIS FIELD ──▶    │       _shares       │
└─────────┬───────────┘                                       └──────────┬──────────┘
          │                                                              │
          │ garble_first_half ──────► chunk_levels, chunk_cts ────────► │ evaluate_first_half
          │ garble_second_half ─────► chunk_levels, chunk_cts ────────► │ evaluate_second_half
          │ garble_final                                                 │ evaluate_final
          │   (writes [v_gamma D_gb]^gb to first_half_out)               │   (writes [v_gamma D_gb]^ev to first_half_out)
          ▼                                                              ▼
┌─────────────────────┐                                       ┌─────────────────────┐
│ compute_lambda_gamma│                                       │ compute_lambda_gamma│
│                     │ ──── lambda_gb: Vec<bool> ──────────▶│ (lambda_gb)         │
│ returns Vec<bool>   │     (length n*m, in-process arg)      │ returns Vec<bool>   │
│  = first_half_out   │                                       │  = lambda_gb        │
│      .lsb() XOR     │                                       │     XOR             │
│    gamma_share.bit  │                                       │   first_half_out    │
│  per (i,j)          │                                       │      .lsb() XOR     │
│                     │                                       │   gamma_share.bit   │
└─────────────────────┘                                       └──────────┬──────────┘
                                                                         │
                            ┌────────────────────────────────────────────┘
                            │  L_gamma  (the masked output value v_gamma XOR l_gamma)
                            │
                            ▼
            ┌──────────────────────────────────┐
            │ Caller assembles c_gamma_shares  │
            │  (linear combination of D_ev     │
            │   shares of l_alpha, l_beta,     │
            │   l_gamma, l_gamma* per D-09)    │
            └──────────────┬───────────────────┘
                           │
                           ▼
                ┌────────────────────┐
                │   check_zero(      │
                │     c_gamma_shares,│
                │     delta_ev)      │
                │   -> bool          │     src/online.rs (NEW FILE)
                │                    │
                │  reconstruct bit   │
                │  + verify MAC      │
                │  under delta_ev    │
                └────────────────────┘
```

### Recommended Project Structure

```
src/
├── auth_tensor_gen.rs       # Add: gamma_auth_bit_shares field + compute_lambda_gamma()
├── auth_tensor_eval.rs      # Add: gamma_auth_bit_shares field + compute_lambda_gamma(&[bool])
├── online.rs                # NEW FILE: check_zero(c_gamma_shares, delta_ev) -> bool
├── lib.rs                   # Add: `pub mod online;` declaration; extend test_auth_tensor_product
├── preprocessing.rs         # No changes — already exposes gamma_auth_bit_shares (Phase 7)
└── sharing.rs               # No changes — AuthBitShare + Add impls already sufficient
```

### Pattern 1: Symmetric Field Forwarding (Phase 7 Precedent)

**What:** Add the same field name `gamma_auth_bit_shares: Vec<AuthBitShare>` to both `AuthTensorGen` and `AuthTensorEval`, initialize from `TensorFpreGen`/`TensorFpreEval` in the corresponding `new_from_fpre_*` constructor. Preserve exact field ordering / mirror the existing `correlated_auth_bit_shares` line.

**When to use:** Always — established pattern from Phase 7 (`gamma_auth_bit_shares` was added symmetrically to both Fpre structs in the same commit).

**Example:**
```rust
// src/auth_tensor_gen.rs — replace TODO at line 64
// Source: existing pattern at line 63 (correlated_auth_bit_shares)
correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,   // NEW
first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
```
[CITED: src/auth_tensor_gen.rs:63 + src/preprocessing.rs:43 (Phase 7 field declaration)]

### Pattern 2: extbit via `Block::lsb()` (paper notation → code)

**What:** The paper writes `extbit(X)` to mean "extract the bit committed in label `X` via the LSB pointer". In code this is `block.lsb()`. The `Delta` LSB=1 invariant guarantees that `(X XOR b*Delta).lsb() == X.lsb() XOR b`, so the LSB carries the authenticated bit "for free" — no extra masking needed.

**When to use:** Anywhere the paper writes `extbit` over a D_gb-authenticated label. Specifically, both lines:
- Garbler: `[L_gamma]^gb := extbit([v_gamma D_gb]^gb) XOR extbit([l_gamma D_gb]^gb)`
- Evaluator: `L_gamma := [L_gamma]^gb XOR extbit([l_gamma D_gb]^ev) XOR extbit([v_gamma D_gb]^ev)`

**Example:**
```rust
// src/auth_tensor_gen.rs — new method after garble_final
// Source: paper 5_online.tex line 132
pub fn compute_lambda_gamma(&self) -> Vec<bool> {
    let mut out = Vec::with_capacity(self.n * self.m);
    for j in 0..self.m {
        for i in 0..self.n {
            // [v_gamma D_gb]^gb is in first_half_out after garble_final
            let v_extbit  = self.first_half_out[(i, j)].lsb();
            // [l_gamma D_gb]^gb is the gamma share's bit/value
            let lg_extbit = self.gamma_auth_bit_shares[j * self.n + i].bit();
            out.push(v_extbit ^ lg_extbit);
        }
    }
    out
}
```
[CITED: paper 5_online.tex line 132; codebase pattern src/auth_tensor_gen.rs:179-193 column-major loop]

**Note on bit semantics:** `AuthBitShare::bit()` returns `self.value` (the held bit), NOT a re-derivation from the MAC LSB. This is correct here because the gamma_auth_bit_share is already a D_ev-authenticated share — its `value` is the local share of `l_gamma`, and XORing the two parties' values reconstructs `l_gamma`. **However**, the formula in D-04 says "extbit of `[l_gamma D_gb]`", which is a **D_gb**-authenticated share. The Phase 7 field stores **D_ev**-authenticated shares. **OPEN QUESTION** — see Open Questions section below.

### Pattern 3: Linear Combination of `AuthBitShare` (using `Add`)

**What:** Building `[c_gamma D_ev]^party` is XOR of D_ev-authenticated shares of `l_alpha`, `l_beta`, `l_gamma`, `l_gamma*` weighted by public `L_alpha`, `L_beta`, `L_gamma` coefficients. Because the coefficients are public, "weighting" reduces to "include if coefficient bit = 1, omit otherwise" — i.e., a sum of selected shares plus optional public-bit MACs.

**When to use:** Inside the test harness (or a future helper) that builds `c_gamma_shares` for `check_zero()`.

**Example (one gate, coordinate (i,j)):**
```rust
// Source: paper 5_online.tex line 206
//   c_gamma[(i,j)] = (L_alpha[i] XOR l_alpha[i]) AND (L_beta[j] XOR l_beta[j])
//                    XOR (L_gamma[(i,j)] XOR l_gamma[(i,j)])
// Expanding the AND:
//   = L_alpha[i] L_beta[j]
//     XOR L_alpha[i] l_beta[j]
//     XOR l_alpha[i] L_beta[j]
//     XOR l_alpha[i] l_beta[j]
//     XOR L_gamma[(i,j)]
//     XOR l_gamma[(i,j)]
// l_alpha[i] l_beta[j] is l_gamma*[(i,j)] (preprocessing identity).
// L_alpha[i] L_beta[j] and L_gamma[(i,j)] are public bits — represent with Mac::PUBLIC.
//
// Per-party D_ev share assembly (omitting public-bit MAC handling for brevity):
let s = AuthBitShare::default()                                           // 0
    + (if L_alpha[i] { beta_auth_bit_shares[j] } else { default })        // L_alpha · l_beta
    + (if L_beta[j]  { alpha_auth_bit_shares[i] } else { default })       // l_alpha · L_beta
    + correlated_auth_bit_shares[j * n + i]                               // l_alpha · l_beta = l_gamma*
    + gamma_auth_bit_shares[j * n + i];                                   // l_gamma
// Public terms (L_alpha · L_beta and L_gamma) cancel out the share's value
// when XORed with the public Mac::PUBLIC representations — implementation detail.
```
[VERIFIED: src/sharing.rs:66-115 — Add impls for AuthBitShare are field-wise XOR. CITED: paper 5_online.tex lines 204-209.]

### Anti-Patterns to Avoid

- **Direct `share.verify(delta)` on a cross-party `AuthBitShare`:** Will panic with "MAC mismatch in share" even on correctly formed shares. The codebase has a 78-line warning comment about this in `src/auth_tensor_pre.rs:305-336`. Use `verify_cross_party` (test helper) or, inside `check_zero()`, reconstruct the per-party MAC explicitly. [VERIFIED: src/sharing.rs:60-63 + src/auth_tensor_pre.rs:315-317]
- **Calling `compute_lambda_gamma()` before `garble_final()` / `evaluate_final()`:** `first_half_out` only holds `[v_gamma D_gb]` *after* `garble_final()` completes. Calling earlier yields garbage (it would still hold the un-combined first half). Document the ordering in the method's doc comment.
- **Using row-major indexing `i*m+j`:** All `n*m` field vecs (`correlated_auth_bit_shares`, `gamma_auth_bit_shares`, output BlockMatrices) are column-major `j*n+i`. Mixing layouts silently corrupts results. [VERIFIED: src/auth_tensor_gen.rs:182, src/preprocessing.rs:36-43, codebase CONVENTIONS.md line 67]
- **Mutating `lambda_gb` after garbler emits it:** P1-05 negative test should construct a *separate* tampered copy and pass it to evaluator's `compute_lambda_gamma`. Don't mutate the garbler's vec.
- **Hand-rolling a "verify MAC under delta_ev" routine that ignores both sides:** `c_gamma` reconstruction needs both gen's and eval's D_ev-shares (XOR) to recover the bit, and the IT-MAC check needs the gen's `key` aligned with eval's `mac` (or vice versa) under `delta_ev`. The thin `check_zero(c_gamma_shares, delta_ev)` signature implies the caller has already pair-XORed two parties' shares per gate index — confirm this in the contract.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Authenticated-bit XOR | Custom struct + manual key/mac/value mixing | `AuthBitShare + AuthBitShare` (Add impl, src/sharing.rs:66) | Already implemented and audited; field-wise XOR. |
| extbit extraction | Bit masking on raw `[u8; 16]` | `Block::lsb()` (src/block.rs:97) | LSB is the canonical pointer-bit slot; pre-existing helper. |
| MAC reconstruction `mac == key XOR bit*delta` | Manual XOR of bytes | `Key::auth(bit, delta)` (src/keys.rs:66) | Single canonical computation; tested. |
| Cross-party MAC verification | Reinvent | `verify_cross_party` pattern from src/auth_tensor_pre.rs:318 | Comes with a 32-line comment explaining the layout pitfall. Re-export or inline the same pattern in `check_zero()`. |
| RNG for tests | `rand::rng()` | `ChaCha12Rng::seed_from_u64(N)` | Determinism — all existing tests use seeded ChaCha12. [VERIFIED: src/* grep] |
| Public-bit MAC representation | Inventing constants | `Mac::PUBLIC` (src/macs.rs:22) | Already includes `MAC_ZERO` and `MAC_ONE` for committing to public bits. |

**Key insight:** Phase 7 already added every primitive Phase 8 needs. The only file *creation* is `src/online.rs` for `check_zero()`. The remainder is wiring (forward field, add method) plus tests. Resist building generic "online phase orchestrator" types — the user explicitly chose thin separation in DISCUSSION-LOG.

## Common Pitfalls

### Pitfall 1: D_gb vs D_ev Confusion in `gamma_auth_bit_shares`

**What goes wrong:** Paper line 132 says `extbit([l_gamma D_gb]^gb)` but the Phase 7 `gamma_auth_bit_shares` field stores **D_ev-authenticated** shares (per Phase 7 D-04/D-05, also confirmed in src/preprocessing.rs:38-43 docstring). Treating the D_ev share as if it were the D_gb share will produce wrong `[L_gamma]^gb` bits, but tests may still appear to pass because the identical wrong XOR happens on both sides — they cancel each other in `gen XOR eval = correct` tensor product checks but **fail** the consistency check (which is exactly what P1-04 + P1-05 detect).

**Why it happens:** The paper uses `[l_gamma D_gb]` for the extbit operation but the same wire mask `l_gamma` is also dual-authenticated under D_ev for the consistency check. Phase 7 only stored the D_ev variant (because that is the one needed for the consistency check). The garbler's local `value` of `l_gamma` (which is what's needed for extbit of `l_gamma`) is the same regardless of which delta authenticates the share — `value` is the share of the bit, independent of the MAC delta.

**How to avoid:** `AuthBitShare::bit()` returns `self.value` and `value` is the per-party share of the bit *itself*, not of `bit*delta`. So `gamma_share.bit()` gives the garbler's local share of `l_gamma`, which is exactly what the formula needs. **Verify this in the planning step** by reading `AuthBitShare`'s `bit()` definition (src/sharing.rs:54-57) and `gen_auth_bit` in `auth_tensor_fpre.rs:66-86` to confirm the `value` field semantic. This is the canonical resolution but it MUST be documented in `compute_lambda_gamma`'s doc comment so future readers don't trip on the paper-vs-code mismatch.

**Warning signs:** If P1-04 (positive end-to-end) passes but P1-05 (negative tampered-mask) also passes (meaning the check did NOT abort on tampered input), there is likely a delta confusion bug.

### Pitfall 2: Calling `gamma_auth_bit_shares.is_empty()` Path on Uncompressed Backend

**What goes wrong:** `UncompressedPreprocessingBackend` deliberately leaves `gamma_auth_bit_shares = vec![]` (per src/preprocessing.rs:289-295 — Phase 7 stub). `compute_lambda_gamma()` will index out-of-bounds at `[j*n+i]` → panic.

**Why it happens:** Real `Pi_aTensor'` does not produce `l_gamma` shares (only `l_gamma*`). The ideal backend manufactures them; the real backend would need a separate preprocessing step.

**How to avoid:** Phase 8 tests must use `IdealPreprocessingBackend` (or the older `TensorFpre::generate_for_ideal_trusted_dealer` path that has been similarly updated). Document in `compute_lambda_gamma()` that it requires `gamma_auth_bit_shares.len() == n*m`; assert this at method entry with a clear panic message: `"compute_lambda_gamma requires gamma_auth_bit_shares.len() == n*m; UncompressedPreprocessingBackend leaves this vec empty — use IdealPreprocessingBackend"`.

**Warning signs:** Switching the test from `IdealPreprocessingBackend` to `UncompressedPreprocessingBackend` panics. This is *expected* until Phase 8+ adds gamma generation to the real backend (a future-phase concern, NOT scope for Phase 8).

### Pitfall 3: Forgetting that `AuthBitShare` Has No `PartialEq`

**What goes wrong:** Test code that wants to compare two `AuthBitShare` vecs (e.g., before/after a permutation) cannot use `assert_eq!` directly — `AuthBitShare` does not derive `PartialEq` (src/sharing.rs:42).

**Why it happens:** Defensive: the type holds a key + mac + value; bit-pattern equality is rarely the intended semantic in cryptographic code.

**How to avoid:** Use the `shares_eq` / `slices_eq` helpers from `src/auth_tensor_pre.rs:362-369` if you need byte-equality; otherwise compare `.bit()`, `.value`, or use the `verify_cross_party` pattern. For Phase 8, the most common need is comparing `bit()` results — that's a `bool`, comparable directly.

**Warning signs:** Compiler error `the trait bound 'AuthBitShare: PartialEq' is not satisfied`.

### Pitfall 4: `into_gen_eval(self)` Consumes the Fpre

**What goes wrong:** `TensorFpre::into_gen_eval(self)` is by-value. Any `gen_auth_bit()` call must happen *before* `into_gen_eval()`. Phase 7's `IdealPreprocessingBackend` already documented this in src/preprocessing.rs:140-149 ("CRITICAL ORDERING").

**Why it relates to Phase 8:** Phase 8 doesn't add new ideal-backend logic, but `c_gamma` test scaffolding might want extra auth bits — same constraint applies.

**How to avoid:** All `gen_auth_bit()` calls before `into_gen_eval()`. Already enforced by current code structure.

**Warning signs:** Compiler error "borrow of moved value `fpre`" if a test naively calls `gen_auth_bit` after `into_gen_eval`.

### Pitfall 5: `c_gamma` Coefficient Selection — Public Bits Must Be MAC-Aware

**What goes wrong:** `c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)` contains four expanded cross-terms. Two of them (`L_alpha · L_beta`, `L_gamma`) are pairs of *public* bits with no preprocessing share — they contribute to the *value* but their contribution to the *MAC* must use the public-bit MAC convention (`Mac::PUBLIC[bit as usize]`, src/macs.rs:22).

**Why it happens:** Standard MPC trap — when adding a cleartext bit to an authenticated share, the share's MAC must absorb the cleartext via a deterministic public MAC; otherwise the IT-MAC invariant breaks.

**How to avoid:** When building `c_gamma_shares`:
1. The XORed *value* component is straightforward — XOR the bool coefficients with the share `.value` fields.
2. For the MAC and key components: public bits contribute `Mac::PUBLIC[L_alpha & L_beta]` and `Mac::PUBLIC[L_gamma]` (and a deterministic key — typically `Block::ZERO` or matching constant). The codebase has `MAC_ZERO` and `MAC_ONE` constants in `src/macs.rs:7-13` and `src/lib.rs:36-43`.
3. Cross-check that `check_zero` on a known-zero `c_gamma` (i.e., honest run) passes — the public terms must cancel out.

**Warning signs:** Honest-party `check_zero()` returns `false`. Cause: forgot to fold public bits into the assembled share's `value` (or used the wrong MAC constant).

**Mitigation:** Phase 8 may sidestep MAC subtleties for the *value-reconstruction* part of `check_zero()` by reconstructing the bit from `gen_share.value XOR eval_share.value` and asserting it equals 0 — a "shares reconstruct to zero" check is sufficient for the in-process simulation per D-08's "thin primitive" framing. The full MAC verification can verify that each share's MAC equals `key XOR bit*delta_ev` under cross-party reconstruction (using the `verify_cross_party` pattern). Document in `check_zero()` doc comment what exact mechanic is used.

## Code Examples

### Example 1: Field Forwarding (auth_tensor_gen.rs)

```rust
// src/auth_tensor_gen.rs
// Source: existing Phase 7 stub at line 64; pattern from line 63

pub struct AuthTensorGen {
    // ... existing fields ...
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,   // NEW
    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,
}

impl AuthTensorGen {
    pub fn new(n: usize, m: usize, chunking_factor: usize) -> Self {
        Self {
            // ... existing fields ...
            correlated_auth_bit_shares: Vec::new(),
            gamma_auth_bit_shares: Vec::new(),       // NEW
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
        }
    }

    pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
        Self {
            // ... existing fields ...
            correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
            gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,   // REPLACES TODO at line 64
            first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
        }
    }
}
```
[VERIFIED: src/auth_tensor_gen.rs:14-68]

Same shape applies to `AuthTensorEval` at src/auth_tensor_eval.rs:7-61 (replacing TODO at line 57).

### Example 2: `compute_lambda_gamma` on Garbler

```rust
// src/auth_tensor_gen.rs
// Source: paper 5_online.tex line 132

impl AuthTensorGen {
    /// Computes the garbler's masked output share `[L_gamma]^gb` per (i,j).
    ///
    /// MUST be called after `garble_final()` — `first_half_out` holds
    /// `[v_gamma D_gb]^gb` only after the final XOR with the correlated share.
    ///
    /// Per CONTEXT.md D-04:
    ///   `[L_gamma]^gb[j*n+i] = first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()`
    ///
    /// Output is column-major: `vec[j*n+i]` corresponds to gate output (i,j).
    pub fn compute_lambda_gamma(&self) -> Vec<bool> {
        assert_eq!(
            self.gamma_auth_bit_shares.len(),
            self.n * self.m,
            "compute_lambda_gamma requires gamma_auth_bit_shares.len() == n*m; \
             UncompressedPreprocessingBackend leaves this vec empty — \
             use IdealPreprocessingBackend"
        );

        let mut out = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let v_extbit  = self.first_half_out[(i, j)].lsb();
                let lg_extbit = self.gamma_auth_bit_shares[j * self.n + i].bit();
                out.push(v_extbit ^ lg_extbit);
            }
        }
        out
    }
}
```

### Example 3: `compute_lambda_gamma` on Evaluator

```rust
// src/auth_tensor_eval.rs
// Source: paper 5_online.tex line 160

impl AuthTensorEval {
    /// Computes the evaluator's masked output `Λ_gamma` per (i,j) given the
    /// garbler's `[L_gamma]^gb` from the garbled circuit.
    ///
    /// MUST be called after `evaluate_final()` — `first_half_out` holds
    /// `[v_gamma D_gb]^ev` only after the final XOR with the correlated MAC.
    ///
    /// Per CONTEXT.md D-05:
    ///   `L_gamma[j*n+i] = lambda_gb[j*n+i] XOR first_half_out[(i,j)].lsb()
    ///                     XOR gamma_auth_bit_shares[j*n+i].bit()`
    ///
    /// Returns the unmasked `Λ_w := v_w XOR l_w` — used by both the consistency
    /// check (sent back to garbler) and the output decoding step.
    pub fn compute_lambda_gamma(&self, lambda_gb: &[bool]) -> Vec<bool> {
        assert_eq!(lambda_gb.len(), self.n * self.m, "lambda_gb length mismatch");
        assert_eq!(
            self.gamma_auth_bit_shares.len(),
            self.n * self.m,
            "compute_lambda_gamma requires gamma_auth_bit_shares populated"
        );

        let mut out = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let v_extbit  = self.first_half_out[(i, j)].lsb();
                let lg_extbit = self.gamma_auth_bit_shares[j * self.n + i].bit();
                out.push(lambda_gb[j * self.n + i] ^ v_extbit ^ lg_extbit);
            }
        }
        out
    }
}
```

### Example 4: `check_zero` Skeleton (src/online.rs)

```rust
// src/online.rs (NEW FILE — Phase 8)
//! Online phase primitives that span both garbler and evaluator views.
//!
//! Currently hosts `check_zero()` only. `open()` will be added in a future
//! phase (deferred per Phase 8 CONTEXT.md D-01).

use crate::sharing::AuthBitShare;
use crate::delta::Delta;

/// Verifies that the per-gate consistency check vector reconstructs to zero
/// and that its IT-MAC under `delta_ev` is valid.
///
/// `c_gamma_shares` is the caller-assembled vector of D_ev-authenticated
/// shares of `c_gamma` (per Construction 1 in 5_online.tex, line 206):
///
///   c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)
///           = v_alpha ⊗ v_beta XOR v_gamma   [= 0 for honest parties]
///
/// Returns `true` (pass — equivalent to "do not abort") when:
///   - every share's reconstructed bit is 0, AND
///   - every share's MAC is valid under `delta_ev` (per IT-MAC invariant).
///
/// Returns `false` (abort) on any failure.
///
/// Per CONTEXT.md D-07/D-08: thin primitive, caller does linear-combination assembly.
pub fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool {
    for share in c_gamma_shares {
        // (1) Reconstructed bit must be 0.
        // In the in-process simulation, callers pre-XOR the two parties' D_ev
        // shares so that `share.value` is the reconstructed c_gamma bit.
        if share.value {
            return false;
        }
        // (2) IT-MAC invariant under delta_ev.
        // `share.mac == share.key XOR share.value * delta_ev`
        // (when value=0, simplifies to `mac == key`)
        let want = share.key.auth(share.value, delta_ev);
        if share.mac != want {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::keys::Key;
    use crate::macs::Mac;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    #[test]
    fn test_check_zero_passes_on_zero_bit_with_valid_mac() {
        let mut rng = ChaCha12Rng::seed_from_u64(1);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(false, &delta);
        let share = AuthBitShare { key, mac, value: false };
        assert!(check_zero(&[share], &delta));
    }

    #[test]
    fn test_check_zero_fails_on_nonzero_bit() {
        let mut rng = ChaCha12Rng::seed_from_u64(2);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(true, &delta);
        let share = AuthBitShare { key, mac, value: true };
        assert!(!check_zero(&[share], &delta));
    }

    #[test]
    fn test_check_zero_fails_on_invalid_mac() {
        let mut rng = ChaCha12Rng::seed_from_u64(3);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        // Use the *wrong* delta to compute the MAC.
        let wrong_delta = Delta::random(&mut rng);
        let mac = key.auth(false, &wrong_delta);
        let share = AuthBitShare { key, mac, value: false };
        assert!(!check_zero(&[share], &delta));
    }
}
```
[CITED: src/sharing.rs:60-63 verify pattern; src/keys.rs:66 auth method]

**Note on `check_zero` implementation choice:** The caller-pre-XOR approach is the simplest interpretation of D-08 ("thin primitive") and matches the in-process simulation semantic (both parties' shares are accessible together in the test). An alternative would be a `check_zero(gen_shares, eval_shares, delta_ev)` two-vec signature that does the per-pair XOR internally — D-07 fixed the one-vec signature, so callers must pre-XOR. Document this clearly.

### Example 5: lib.rs Module Declaration

```rust
// src/lib.rs (insert near existing module declarations)
pub mod preprocessing;
pub mod online;       // NEW
```
[VERIFIED: src/lib.rs:25 location of preprocessing module decl]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `garble_final()` produces only `[v_gamma D_gb]^gb` (output bit shares) | + `compute_lambda_gamma()` produces `[L_gamma]^gb` (the masked-output bit shares for transmission) | Phase 8 | Closes the missing `[L_gamma]^gb := extbit(...) XOR extbit(...)` line of paper Construction 3 |
| No CheckZero implementation | `check_zero()` in `src/online.rs` | Phase 8 | Enables P1-03 consistency-check requirement |
| `gamma_auth_bit_shares` declared on Fpre structs but unused | Forwarded to `AuthTensorGen`/`Eval` and consumed by `compute_lambda_gamma()` | Phase 8 | Resolves Phase 7 → Phase 8 handoff TODO |
| `online.rs` does not exist | `online.rs` exists; hosts `check_zero()` only | Phase 8 | Module skeleton ready for `open()` (later phase) |

**Deprecated/outdated:** None.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `AuthBitShare::bit()` returning `self.value` is the correct "extbit of `[l_gamma D_gb]`" semantic, despite the field being D_ev-authenticated | Pitfall 1, Example 2/3 | Wrong `[L_gamma]^gb` values; positive end-to-end test passes (errors cancel) but consistency check is wrong; Pitfall 1 explains why the bit value is delta-independent. **Confirm in planning** by re-reading sharing.rs:43-57. [VERIFIED: bit() returns value; value is the local bit share, independent of the MAC's delta] |
| A2 | The thin `check_zero(c_gamma_shares, delta_ev)` signature with caller-pre-XOR is consistent with D-08 | Example 4 | Wrong signature would force a refactor — but D-07 explicitly fixes this signature and D-08 says "thin primitive, callers pre-compute". |
| A3 | Public-bit MAC handling for `L_alpha · L_beta` and `L_gamma` terms uses `Mac::PUBLIC` | Pattern 3, Pitfall 5 | If the simpler "value-only reconstruction" mechanic is chosen for `check_zero`, MAC handling of public terms can be skipped (caller XORs the public bits into the assembled `share.value`). Recommend adopting the simpler mechanic and noting it in the doc comment. |
| A4 | Phase 8 does NOT need to update the `UncompressedPreprocessingBackend` — its empty `gamma_auth_bit_shares` is allowed to remain stubbed and tests use `IdealPreprocessingBackend` only | Pitfall 2 | Out-of-bounds panic if a future phase test forgets and uses uncompressed backend with `compute_lambda_gamma`. Mitigated by the explicit length assertion in Example 2/3. |

## Open Questions

1. **Exact MAC mechanic for `check_zero()` value reconstruction**
   - What we know: D-08 says "thin primitive"; D-07 fixes the signature; CONTEXT.md "Claude's Discretion" explicitly leaves the exact verification mechanic open.
   - What's unclear: whether `check_zero()` should (a) just check `value == false && mac == key.auth(false, delta_ev)` per pre-XORed share, or (b) take two-party share pairs and XOR internally, or (c) perform full `verify_cross_party`-style checks.
   - Recommendation: **Pick (a)** — the caller pre-XORs the two parties' D_ev-shares (key XOR key, mac XOR mac, value XOR value) and passes the combined share. After XOR, the MAC invariant becomes `(mac_gen XOR mac_eval) == (key_gen XOR key_eval) XOR (value_gen XOR value_eval) * delta_ev` — which is the same algebraic shape as `share.verify(delta_ev)` because each side individually satisfies its IT-MAC and the relation is GF(2)-linear. **Confirm with planner / quick mathematical verification.**

2. **Where `c_gamma` assembly lives**
   - What we know: D-08 says "callers pre-compute". CONTEXT.md says "test harness (or a helper)".
   - What's unclear: whether to extract a `assemble_c_gamma(...)` helper now or inline in tests.
   - Recommendation: **Inline in test for Phase 8** (one test = one inline assembly). Extract a helper only when Phase 9 (Protocol 2's check) shows a similar enough pattern. YAGNI applies.

3. **Whether to expose `[L_gamma]^gb` as part of `AuthTensorGen`'s state**
   - What we know: `compute_lambda_gamma()` returns `Vec<bool>` (D-04). The garbler conceptually transmits this as part of `gc`.
   - What's unclear: should it be cached on the struct (like `first_half_out`) or stay a return value?
   - Recommendation: **Return value only.** Matches D-04 verbatim; avoids state. If a benchmark wants to time transmission, it can measure the returned vec size externally.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (Rust toolchain) | Test execution, compile check | ✓ | (project pinned, edition 2024) | — |
| `rand_chacha` | Test determinism | ✓ | (project Cargo.toml) | — |
| `rand` | Random bit generation in tests | ✓ | (project Cargo.toml) | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None.

[VERIFIED: All dependencies are already in use throughout the codebase — Phase 8 adds zero new external dependencies.]

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness (no external runner) |
| Config file | None — pure `cargo test` |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| P1-01 | Garble outputs `[L_gamma]^gb` | unit | `cargo test --lib auth_tensor_gen::tests::test_compute_lambda_gamma_dimensions` | ❌ Wave 0 — extend src/auth_tensor_gen.rs |
| P1-01 | Garble runs end-to-end with new method | integration | `cargo test --lib tests::test_auth_tensor_product` | ✅ extends existing |
| P1-02 | Eval runs `compute_lambda_gamma(lambda_gb)` and reconstructs `L_gamma` | unit | `cargo test --lib auth_tensor_eval::tests::test_compute_lambda_gamma_reconstruction` | ❌ Wave 0 — create src/auth_tensor_eval.rs `mod tests` (currently has none — see TESTING.md coverage gap) |
| P1-03 | `check_zero` returns `true` on honest c_gamma=0 | unit | `cargo test --lib online::tests::test_check_zero_passes_on_zero_bit_with_valid_mac` | ❌ Wave 0 — create src/online.rs |
| P1-03 | `check_zero` returns `false` on bit=1 | unit | `cargo test --lib online::tests::test_check_zero_fails_on_nonzero_bit` | ❌ Wave 0 |
| P1-03 | `check_zero` returns `false` on tampered MAC | unit | `cargo test --lib online::tests::test_check_zero_fails_on_invalid_mac` | ❌ Wave 0 |
| P1-04 | End-to-end positive: `L_gamma_ev == (input_x ⊗ input_y) XOR l_gamma` | integration | `cargo test --lib tests::test_auth_tensor_product_full_protocol_1` | ❌ Wave 0 — extends src/lib.rs |
| P1-05 | Negative: tampered `L_gamma_gb` causes `check_zero` to abort | integration | `cargo test --lib tests::test_protocol_1_check_zero_aborts_on_tampered_lambda` | ❌ Wave 0 — new test in src/lib.rs |

### Sampling Rate
- **Per task commit:** `cargo test --lib <module>` for the modified module (e.g., `cargo test --lib online`, `cargo test --lib auth_tensor_gen`)
- **Per wave merge:** `cargo test --lib`
- **Phase gate:** `cargo test` (full suite, including doctests if any) green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `src/online.rs` — new file with `check_zero()` + 3+ unit tests covering pass/fail-bit/fail-MAC paths
- [ ] `src/auth_tensor_eval.rs` — add a `#[cfg(test)] mod tests {}` block (currently has no test module per TESTING.md "Coverage Gaps")
- [ ] `src/auth_tensor_gen.rs` — extend existing `mod tests` with `compute_lambda_gamma` dimension + correctness test
- [ ] `src/lib.rs` — extend `test_auth_tensor_product` (or duplicate as `test_auth_tensor_product_full_protocol_1`) to (1) call `compute_lambda_gamma` on both sides, (2) assemble `c_gamma_shares` per D-09, (3) assert `check_zero` returns `true`; add a paired negative test that flips one bit of the garbler's `lambda_gb` before evaluator processing

### Test framework install
Not needed — `cargo` is the standard Rust toolchain; `#[test]` is built-in.

## Project Constraints (from CLAUDE.md)

`./CLAUDE.md` does not exist in the project root. No project-level CLAUDE directives apply.

**Codebase conventions (extracted from `.planning/codebase/CONVENTIONS.md`) that Phase 8 MUST honor:**
- `snake_case` files, `PascalCase` types — new file `online.rs` follows; `check_zero` is `snake_case`.
- Column-major indexing `j*n+i` for all `n*m` tensor structures. Phase 8 vecs (`compute_lambda_gamma()` output) use the same convention. [Codebase CONVENTIONS line 67]
- Key LSB=0 invariant — not directly relevant to Phase 8 (no new key generation).
- Delta LSB=1 invariant — not directly relevant to Phase 8 (consumed via `delta_ev` as input only).
- MAC invariant `mac == key.auth(bit, delta)` — `check_zero()` enforces this for delta_ev.
- Cross-party MAC layout — `check_zero()` consumes pre-XORed shares per D-08, so the cross-party pitfall is pushed to the caller. Document the caller contract.
- Tests use `ChaCha12Rng::seed_from_u64(N)` for determinism — Phase 8 tests follow.
- No `rand::rng()` in tests.
- Tests co-located in `#[cfg(test)] mod tests {}` at file bottom — `online.rs` and the new `auth_tensor_eval.rs` test module follow.

## Sources

### Primary (HIGH confidence)
- **Paper Construction 3 (`references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex`)** — primary spec for garble/eval algorithms (lines 111-167) and consistency check (lines 201-210)
- **Paper Lemma 2 (Correctness, lines 218-251)** — proves c_gamma = 0 for honest parties; defines exactly what `check_zero()` must enforce
- **`src/sharing.rs`** — `AuthBitShare`, `AuthBit`, `Add` impls, `bit()` method semantics, `verify(delta)` cross-party warning
- **`src/auth_tensor_gen.rs:14-194`** — `AuthTensorGen` structure; `garble_final()` shows the existing column-major loop pattern; line 64 marks Phase 8 TODO
- **`src/auth_tensor_eval.rs:7-164`** — `AuthTensorEval` structure; `evaluate_final()` shows symmetric pattern; line 57 marks Phase 8 TODO
- **`src/preprocessing.rs:38-72`** — `gamma_auth_bit_shares` field declarations on `TensorFpreGen`/`TensorFpreEval`; doc comments explaining D_ev authentication
- **`src/preprocessing.rs:119-163`** — `IdealPreprocessingBackend::run()` showing how `gamma_auth_bit_shares` is populated (relevant for understanding what's available)
- **`src/keys.rs:66`** — `Key::auth(bit, delta) -> Mac` canonical IT-MAC computation
- **`src/macs.rs:7-22`** — `Mac::PUBLIC` constants for handling public bits in MAC arithmetic
- **`src/auth_tensor_pre.rs:305-336`** — `verify_cross_party` function with detailed doc comment about the cross-party MAC pitfall (the canonical reference for Pitfall 1 / cross-party MAC handling)
- **`src/feq.rs`** — established pattern for an "abort on mismatch" check (panic-based); contrast for `check_zero` returning `bool`
- **`.planning/codebase/CONVENTIONS.md`** — project conventions (column-major, naming, MAC invariants)
- **`.planning/codebase/TESTING.md`** — test framework, helpers, and coverage gaps
- **`.planning/phases/07-preprocessing-trait-ideal-backends/07-CONTEXT.md`** — D-04/D-05 confirming `gamma_auth_bit_shares` semantics

### Secondary (MEDIUM confidence)
- **Phase 7 RESEARCH.md** (referenced indirectly via 07-CONTEXT.md) — additional patterns and pitfalls already absorbed into Phase 7 implementation
- **Existing `test_auth_tensor_product` in `src/lib.rs:252-380`** — template for end-to-end test extension

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all primitives in-tree, no new dependencies
- Architecture: HIGH — methods/files explicitly fixed by user decisions D-02..D-09
- Pitfalls: HIGH — derived from existing codebase patterns (verify_cross_party, Phase 7 PHASE 7 RESEARCH learnings); D_gb vs D_ev concern verified by reading `bit()` semantics
- Validation: HIGH — Wave 0 gaps explicitly enumerated; existing `cargo test` infrastructure suffices

**Research date:** 2026-04-23
**Valid until:** 2026-05-23 (30 days — codebase conventions and paper spec are stable)

---

*Phase: 08-open-protocol-1-garble-eval-check*
*Researched: 2026-04-23*
