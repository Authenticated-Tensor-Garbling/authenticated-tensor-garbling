# Phase 3: M2 Generalized Tensor Macro (Construction 1) — Research

**Researched:** 2026-04-21
**Domain:** Rust cryptographic primitive; GGM tree garbler/evaluator pair for tensor products; paper-faithful implementation of Construction 1 from `references/appendix_krrw_pre.tex`
**Confidence:** HIGH (existing kernel is a structural match; paper spec is short and well-defined; all integration types already exist)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Module Placement (PROTO-01)**
- **D-01:** Create `src/tensor_macro.rs` as the home for Construction 1. Add `pub mod tensor_macro;` to `src/lib.rs`. This module is a standalone primitive — it has no dependency on `leaky_tensor_pre.rs` or `preprocessing.rs`.
- **D-02:** Both `tensor_garbler` and `tensor_evaluator` are `pub(crate)` functions — they are called by `leaky_tensor_pre.rs` (Phase 4) but are not part of the crate's public API.

**GGM Tree Kernel Reuse (PROTO-02)**
- **D-03:** Generalize `gen_populate_seeds_mem_optimized` in `src/tensor_ops.rs` by changing its first parameter from `x: &MatrixViewRef<Block>` to `x: &[Block]`. The function body treats the parameter as an indexed array of Blocks — both usages (wire labels and IT-MAC keys) are structurally identical.
- **D-04:** Update the one call site in `tensor_gen.rs` to pass a `&[Block]` slice (convert from `MatrixViewRef` via `.as_slice()` or equivalent). Callers in `tensor_macro.rs` pass MAC keys as `&[Block]` directly.
- **D-05:** `gen_unary_outer_product` in `tensor_ops.rs` is reused for the leaf expansion step — same `(seeds, T_share) → (Z_output, leaf_cts)` pattern as in `tensor_garbler`.

**G Ciphertext Type (PROTO-02, PROTO-03)**
- **D-06:** Define `pub(crate) struct TensorMacroCiphertexts` in `src/tensor_macro.rs`:
  ```rust
  pub(crate) struct TensorMacroCiphertexts {
      pub level_cts: Vec<(Block, Block)>,  // length n-1; G_{i,0} and G_{i,1} for i ∈ [n-1]
      pub leaf_cts: Vec<Block>,             // length m; G_k for k ∈ [m]
  }
  ```
  Maps directly to paper notation. Phase 4 passes the entire struct from `tensor_garbler` to `tensor_evaluator`.

**Function Signatures (PROTO-01, PROTO-02, PROTO-03)**
- **D-07:** `tensor_garbler` signature:
  ```rust
  pub(crate) fn tensor_garbler(
      n: usize,
      m: usize,
      delta: Delta,
      a_keys: &[Key],        // itmac{A}{Δ}^gb = K[0] per wire, LSB=0 enforced
      t_gen: &BlockMatrix,   // T^gb share (n×m Blocks)
  ) -> (BlockMatrix, TensorMacroCiphertexts)
  ```
  Returns `Z_garbler` (n×m matrix) and the ciphertexts `G`.
- **D-08:** `tensor_evaluator` signature:
  ```rust
  pub(crate) fn tensor_evaluator(
      n: usize,
      m: usize,
      g: &TensorMacroCiphertexts,
      a_macs: &[Mac],        // itmac{A}{Δ}^ev = A_i ⊕ a_i·Δ per wire (may have LSB=1)
      t_eval: &BlockMatrix,  // T^ev share (n×m Blocks)
  ) -> BlockMatrix
  ```
  Returns `Z_evaluator` (n×m matrix).

**Input/Output Types**
- **D-09:** Garbler input `a_keys: &[Key]` — uses the `Key` type from Phase 1 (`Key::new()` enforces LSB=0). Length must equal `n`.
- **D-10:** Evaluator input `a_macs: &[Mac]` — uses the `Mac` type; may have LSB=1 (encodes `A_i ⊕ a_i·Δ`). Length must equal `n`.
- **D-11:** T shares and Z outputs are `BlockMatrix` (n×m, column-major indexing as established in Phase 1 D-11).

**Tests (TEST-01, TEST-04)**
- **D-12:** Paper invariant test: construct `a_keys` and `a_macs` from `IdealBCot` (use `transfer_a_to_b` sender_keys as `a_keys`, receiver_macs as `a_macs`). Set T = T_gen XOR T_eval with known random values. Assert `Z_gen XOR Z_eval == a ⊗ T` where `a` is the bit vector from bCOT choice bits.
- **D-13:** Test vectors include edge cases: n=1 (single-wire, degenerate tree), small m, large n and m.
- **D-14:** Tests live in `#[cfg(test)] mod tests` at the bottom of `src/tensor_macro.rs` — inline test pattern per codebase convention.

### Claude's Discretion

- Exact nonce/tweak strategy for level and leaf ciphertexts — paper uses `H(A_i ⊕ Δ, ν_{i,b})` with one-time nonces; use `cipher.tccr(level_tweak, seed)` matching the existing GGM PRF pattern.
- Whether `tensor_macro.rs` exposes `TensorMacroCiphertexts` with `pub(crate)` fields or getter methods — `pub(crate)` fields are the existing pattern in this codebase.
- Implementation of the evaluator's subtree reconstruction traversal — the strategy is specified by the paper; exact loop structure and temporary storage are implementer's call.

### Deferred Ideas (OUT OF SCOPE)

None — CONTEXT.md states "discussion stayed within Phase 3 scope."
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROTO-01 | Implement `tensor_garbler(n, m, Δ_A, itmac{A}{Δ}, T^A)` — GGM tree of 2^n leaves, emit level ciphertexts `G_{i,b}` and leaf ciphertexts `G_k`, return `Z_garbler` and `G`. | `src/tensor_ops.rs::gen_populate_seeds_mem_optimized` already produces the level XOR sums in the `odd_evens: Vec<(Block, Block)>` return; `gen_unary_outer_product` already computes leaf ciphertexts + Z from (seeds, T^gb). The only structural change required is generalising the seed-builder's first parameter (D-03). |
| PROTO-02 | Implement `tensor_evaluator(n, m, G, itmac{A}{Δ}^eval, T^eval)` — reproduce untraversed subtree from `A_i ⊕ a_i·Δ`, recover `X_{a,k}` from ciphertexts, output `Z_evaluator`. | `src/tensor_eval.rs::eval_populate_seeds_mem_optimized` and `eval_unary_outer_product` already implement this traversal on the semi-honest side (private methods on `TensorProductEval`). Must be hoisted/duplicated into `tensor_macro.rs` as `pub(crate)` free functions with `&[Block]` input (D-03 analogue on the eval side). |
| PROTO-03 | Correctness invariant test: `Z_eval XOR Z_garbler == a ⊗ T` for all test vectors. | Verified by existing semi-honest end-to-end test (`lib.rs::test_semihonest_tensor_product`) that the gen/eval pair commutes. Phase 3 writes the invariant directly on the new module's outputs. |
| TEST-01 | GGM macro: `Z_garbler XOR Z_evaluator == a ⊗ T` for multiple (n, m, T) combinations including edge cases. | D-12/D-13 lock the oracle construction via `IdealBCot::transfer_a_to_b`. Edge cases n=1, small m, large n/m are enumerable. |

**Note on TEST-04 scope drift:** CONTEXT.md domain section (`references:line 11`) lists "TEST-01, TEST-04" but the ROADMAP/REQUIREMENTS assign TEST-04 to Phase 4 (F_eq test). Planner should treat Phase 3's test obligation as TEST-01 only; TEST-04 belongs to Phase 4. Flag this discrepancy to the user if it affects plan boundaries.
</phase_requirements>

---

## Summary

Phase 3 extracts the **Generalized Tensor Macro** from paper Construction 1 into a standalone `src/tensor_macro.rs` module with two `pub(crate)` free functions: `tensor_garbler` and `tensor_evaluator`. The key technical finding is that **the existing GGM tree kernel in `src/tensor_ops.rs` is already a structural match for Construction 1**: `gen_populate_seeds_mem_optimized` produces exactly the `(leaf_seeds, level_odd_evens)` that the paper calls leaves `Label_ℓ` and ciphertexts `G_{i,0}/G_{i,1}`; `gen_unary_outer_product` produces exactly the leaf ciphertexts `G_k` and the `Z_garbler` matrix. The only structural change required is generalising the seed builder's first parameter from `MatrixViewRef<Block>` to `&[Block]` (D-03) and adding a thin orchestration layer.

On the evaluator side, `src/tensor_eval.rs` (semi-honest) and `src/auth_tensor_eval.rs` (authenticated) already contain private methods (`eval_populate_seeds_mem_optimized`, `eval_unary_outer_product`) that implement the paper's untraversed-subtree reconstruction. These methods are **identical in behaviour across the two files** — Phase 3 hoists them into `tensor_macro.rs` as `pub(crate)` free functions, duplicating the same kernel the gen side shares via `tensor_ops.rs`.

The cryptographic substance of Phase 3 is therefore minimal. The work is structural: new module, signature plumbing, hoisting of eval kernel, T/Z I/O via `BlockMatrix`, and writing a paper-invariant test harness. No new crypto primitives, no new AES modes, no new consistency checks.

**Primary recommendation:** Structure the plan as **three plans in two waves**:
- **Wave 0 (prerequisite):** Generalise `gen_populate_seeds_mem_optimized` to `&[Block]`, rewire the one call site in `tensor_gen.rs`, hoist eval GGM kernel functions from `tensor_eval.rs` / `auth_tensor_eval.rs` into a new `tensor_ops.rs::eval_populate_seeds_mem_optimized` (or a private submodule), and create the empty `src/tensor_macro.rs` module skeleton with `TensorMacroCiphertexts`. Verify `cargo build && cargo test --lib` still passes with same 4 pre-existing failures (see Common Pitfalls).
- **Wave 1 (the phase body, parallelisable):**
  - **Plan A:** Implement `tensor_garbler` — orchestrate the two kernels, package `TensorMacroCiphertexts`, verify invariants via unit test with a known deterministic seed.
  - **Plan B:** Implement `tensor_evaluator` — call the hoisted eval kernel, apply leaf ciphertext correction, produce `Z_evaluator`.
- **Wave 2 (integration test):** Single plan that writes the paper-invariant `Z_garbler XOR Z_evaluator == a ⊗ T` test battery covering n=1, n=2, n=4, n=8; m=1, m=4, m=16, m=64; random T; bCOT-sourced `a_keys` / `a_macs`.

**Critical pre-existing blocker carried forward from Phase 2:** `cargo test --lib` on current `main` (commit `330f303`) shows **4 failing tests** unrelated to Phase 3 scope (same 4 tests listed in Phase 2 research — `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `auth_tensor_pre::tests::test_combine_mac_invariants`, `preprocessing::tests::test_run_preprocessing_mac_invariants`). Phase 3 must baseline-accept these (no new failures introduced) — see Common Pitfalls #1 and Open Question Q1.

---

## Architectural Responsibility Map

Phase 3 adds one new module to the existing flat src/ layout. The Rust-module tier owns every capability — there is no cross-process or cross-tier work.

| Capability | Primary Module | Secondary Module | Rationale |
|------------|----------------|------------------|-----------|
| GGM seed tree construction (garbler side) | `src/tensor_ops.rs` (existing, generalised) | `src/tensor_macro.rs` (caller) | Kernel reused between auth + semi-honest paths; generalising the input slice is lower-churn than duplicating it. |
| GGM seed tree reconstruction (evaluator side) | `src/tensor_ops.rs` (NEW function hoisted from eval files) | `src/tensor_macro.rs` (caller) | Eval kernel currently duplicated in `tensor_eval.rs` (private method) and `auth_tensor_eval.rs` (private method) — hoist into `tensor_ops.rs` to match the gen kernel's home. Optional secondary: keep the hoist inside `tensor_macro.rs` instead — see Open Question Q2. |
| Leaf expansion + Z accumulation (garbler) | `src/tensor_ops.rs::gen_unary_outer_product` (existing, reused unchanged modulo param-type update) | `src/tensor_macro.rs` | Already has the `(seeds, y, out, cipher) -> Vec<Block>` shape Construction 1 needs. |
| Leaf expansion + Z accumulation (evaluator) | `src/tensor_ops.rs::eval_unary_outer_product` (NEW, hoisted) | `src/tensor_macro.rs` | Currently a private method on `TensorProductEval` and `AuthTensorEval` — same function, duplicated. Hoist once. |
| Public entry points `tensor_garbler` / `tensor_evaluator` | `src/tensor_macro.rs` (NEW) | — | Construction 1 entry-point boundary. |
| Ciphertext struct `TensorMacroCiphertexts` | `src/tensor_macro.rs` (NEW) | — | Pure data carrier; travels between parties in Phase 4. |
| Paper-invariant test (`Z_gen XOR Z_eval == a ⊗ T`) | `#[cfg(test)] mod tests` in `src/tensor_macro.rs` | `src/bcot.rs::IdealBCot` (test oracle for `a_keys`/`a_macs`) | Inline test pattern per codebase convention. bCOT gives a known-consistent cross-party key/MAC pair, which is exactly the input shape Construction 1 expects. |

**Why no cross-tier work:** The preprocessing protocol has no networking, no persistence, no service boundary. Construction 1 is a pure in-memory primitive; Phase 3 delivers Rust functions and a single new module. Everything stays in the crate's single flat src/ directory.

---

## Standard Stack

No new external dependencies. Phase 3 builds on what's already in `Cargo.toml`.

### Core (verified present) `[VERIFIED: /Users/turan/Desktop/authenticated-tensor-garbling/Cargo.toml]`

| Crate | Version | Purpose | Why Standard |
|-------|---------|---------|--------------|
| `aes` | `0.9.0-pre.3` | AES block cipher — used by `FixedKeyAes::tccr` for GGM PRF and leaf expansion. | Already the codebase's TCCR primitive (`src/aes.rs`). Fixed-key mode is the standard GGM PRF instantiation (Guo et al., eprint 2019/074, §7.4). |
| `cipher` | `0.5.0-pre.8` | Traits used by `aes` crate. | Transitive; no direct use in Phase 3 code. |
| `rand` | `0.9` | RNG trait used when generating test T matrices and seeds. | Same crate the rest of the codebase uses. |
| `rand_chacha` | `0.9` | `ChaCha12Rng` seedable RNG for deterministic tests. | Existing pattern: all tests seed `ChaCha12Rng::seed_from_u64(N)` for reproducibility. |
| `once_cell` | `1.21.3` | `Lazy<FixedKeyAes>` singleton for the process-wide `FIXED_KEY_AES`. | Same instance the existing gen/eval sides share — do not construct a new `FixedKeyAes` in `tensor_macro.rs`. |
| `serde` | `1.0` | Derives on `Mac`. | Not used directly by Phase 3; existing. |
| Rust edition | `2024` | Crate edition. | Unchanged. |

**Toolchain verified:**
- `rustc 1.90.0 (1159e78c4 2025-09-14)` `[VERIFIED: rustc --version]`
- `cargo 1.90.0 (840b83a10 2025-07-30)` `[VERIFIED: cargo --version]`

### Internal types reused (from previous phases)

| Type | Home | Role in Phase 3 |
|------|------|-----------------|
| `Block` (`src/block.rs`) | existing | The underlying 128-bit value for seeds, labels, Ts, Zs. |
| `Delta` (`src/delta.rs`) | existing | Garbler's global correlation key; LSB=1 invariant (Delta::new enforces). Phase 3 uses the garbler's delta only — no cross-party delta interaction (that's Phase 4). |
| `Key` (`src/keys.rs`) | existing; Phase 1 guaranteed LSB=0 via `Key::new()` | Garbler's `a_keys: &[Key]` input to `tensor_garbler`. The LSB=0 invariant is the KEY PROPERTY — the code path `key0 = A_i XOR Delta; key1 = A_i` relies on it (see `tensor_ops.rs:48-52`). |
| `Mac` (`src/macs.rs`) | existing | Evaluator's `a_macs: &[Mac]` input. May have LSB=1 (encodes `A_i XOR a_i·Delta`). |
| `BlockMatrix` (`src/matrix.rs`) | existing, column-major indexing documented in Phase 1 D-11 | `T^gb`, `T^ev`, `Z_gen`, `Z_eval` — all n×m column-major. |
| `FIXED_KEY_AES: Lazy<FixedKeyAes>` (`src/aes.rs`) | existing | Process-wide PRF singleton; `tensor_macro.rs` accesses via `&FIXED_KEY_AES` just like `tensor_gen.rs` and `tensor_eval.rs` do. |
| `IdealBCot` (`src/bcot.rs`) | existing | Test oracle only — provides consistent `(sender_keys, receiver_macs, choices)` that satisfy the IT-MAC equation. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hoisting `eval_populate_seeds_mem_optimized` to `tensor_ops.rs` | Duplicate the kernel body inside `tensor_macro.rs` as a `pub(crate)` function | Duplication already exists (`tensor_eval.rs` and `auth_tensor_eval.rs` both have the identical private method). Hoisting eliminates duplication now AND serves Phase 4 onward. See Open Question Q2. |
| Reusing mpz-dev's `GgmTree` type (`references/mpz-dev/crates/core/src/ggm.rs`) | Import mpz-dev's `GgmTree::new_from_seed` / `new_partial` | mpz-dev is NOT a Cargo dependency (`references/mpz-dev/` is vendored read-only for study). Adding it as a path dependency would pull in its entire dependency tree (tokio, serde, OT crates). Construction 1 is too simple to justify that. Existing kernels already pass the equivalent `test_ggm_partial` semantics inline. |
| `TensorMacroCiphertexts` with getter methods | Tuple `(Vec<(Block,Block)>, Vec<Block>)` return | User-locked `pub(crate)` struct with named fields (D-06). Cleaner Phase 4 call sites; matches existing codebase pattern of named structs (`BcotOutput`, `LeakyTriple`). |
| `Vec<Block>` inputs for `a_keys` / `a_macs` | `Vec<Key>` / `Vec<Mac>` typed inputs | User-locked typed inputs (D-09/D-10) — preserves Phase 1's LSB=0 type invariant at the API boundary. `Key::as_blocks(slice)` is an O(1) reinterpret cast inside the function body. |

**Installation:**

```bash
# No-op — all dependencies already in Cargo.toml; nothing to install.
cargo build --lib  # Verify baseline before starting Phase 3
```

**Version verification:** No new crates are being added, so `npm view`-style version checks are not applicable. The existing versions have been stable through Phases 1-2.

---

## Architecture Patterns

### System Data Flow

Construction 1 data flow (paper verbatim, annotated with code correspondences):

```
 Garbler P_A (holds Δ_A, A-keys):                 Evaluator P_B (holds a_i, A_i ⊕ a_i·Δ_A):
     │                                                 │
     │  inputs: n, m, Δ_A, a_keys: [Key;n],            │  inputs: n, m, G, a_macs: [Mac;n],
     │          t_gen: BlockMatrix (n × m)             │          t_eval: BlockMatrix (n × m)
     │                                                 │
     ▼                                                 │
 [1] Build GGM tree of depth n:                        │
     Root seeds at level 0:                            │
       S_{0,0} = A_0 ⊕ Δ, S_{0,1} = A_0                │
     Iterate levels i = 1..n-1:                        │
       children of S_{i-1, j} = tccr(0,S) / tccr(1,S)  │
     (existing `gen_populate_seeds_mem_optimized`)     │
     │                                                 │
     ▼                                                 │
 [2] At each level i ∈ [n-1], compute:                 │
       evens ← XOR of S_{i, 2j} over j                 │
       odds  ← XOR of S_{i, 2j+1} over j               │
       G_{i,0} = tccr(0, A_i ⊕ Δ) XOR evens            │
       G_{i,1} = tccr(1, A_i)     XOR odds             │
     (existing returns Vec<(evens, odds)> — these      │
     already encode G_{i,0}/G_{i,1} directly because   │
     tccr(0, A_i ⊕ Δ) is XORed into evens)             │
     │                                                 │
     ▼                                                 │
 [3] Leaf expansion (seeds Label_ℓ for ℓ ∈ [2^n]):     │
     For each k ∈ [m], ℓ ∈ [2^n]:                      │
       X_{ℓ,k} = tccr(m*j + ℓ, Label_ℓ)                │
     accumulate per-column XOR into `row` and          │
     distribute X_{ℓ,k} to Z_gen row bits set in ℓ.    │
     (existing `gen_unary_outer_product`)              │
     │                                                 │
     ▼                                                 │
 [4] G_k = (XOR_ℓ X_{ℓ,k}) XOR T^gb_k                  │
     Z_gen[(i,k)] = XOR_{ℓ : bit_i(ℓ)=1} X_{ℓ,k}       │
     Package G := {G_{i,0}, G_{i,1}}_{i in [n-1]}      │
             ∥ {G_k}_{k in [m]}                        │
     │                                                 │
     ├──── send G (TensorMacroCiphertexts) ──────────▶ │
     │                                                 │
     │                                                 ▼
     │                                             [5] Reconstruct untraversed subtree:
     │                                                 Base: use A_0 ⊕ a_0·Δ to get S_{0, NOT a_0}.
     │                                                 For i = 1..n-1:
     │                                                   Use A_i ⊕ a_i·Δ and G_{i, NOT a_i} and the
     │                                                   local XOR of already-computed S_{i, 2j+b}
     │                                                   (b = NOT a_i) to decrypt the missing
     │                                                   sibling at (i, prefix·2 + NOT a_i), then
     │                                                   expand its subtree.
     │                                                 (existing `eval_populate_seeds_mem_optimized`
     │                                                 in tensor_eval.rs / auth_tensor_eval.rs)
     │                                                 │
     │                                                 ▼
     │                                             [6] Recover leaf X_{a,k} for the missing position:
     │                                                   X_{a,k} = XOR_{ℓ ≠ a} X_{ℓ,k} XOR G_k XOR T^ev_k
     │                                                 Distribute X_{ℓ,k} and X_{a,k} to Z_eval
     │                                                 row bits set in ℓ (or in a for missing).
     │                                                 (existing `eval_unary_outer_product`)
     │                                                 │
     │                                                 ▼
     │                                             Output Z_eval
     ▼
  Output (Z_gen, G)
```

**Paper-to-code index mapping (endianness note — CRITICAL):**

The paper numbers wire coordinates **A_0, A_1, …, A_{n-1}** with A_0 as the ROOT level and A_{n-1} as the leaf level. The existing `gen_populate_seeds_mem_optimized` code uses **`x[n-1]` as the first consumed position** and iterates down via `x[n-i-1]` for i = 1..n. In the convention documented throughout the codebase (`src/matrix.rs`, `src/block.rs` endianness notes), "index 0 is LSB, index n-1 is MSB" — so the code's `x[n-1]` is the MSB of the bit-vector.

Two valid readings:
- **Reading A:** paper A_0 is the code's `x[n-1]` (MSB-first tree construction). Then paper A_i ⟷ code x[n-1-i].
- **Reading B:** paper A_0 is the code's `x[0]` (LSB-first tree construction).

Looking at the code carefully: Level 0 uses `x[n-1]` directly as the seed basis; subsequent levels consume `x[n-i-1]` for increasing `i`. This means the tree "root" uses the MSB of the input vector. **Reading A is consistent with the code.**

Implication for `tensor_macro.rs`: when callers pass `a_keys: &[Key]` of length n, the implementation will interpret `a_keys[n-1]` as the paper's A_0 (root-level). This is consistent with how the rest of the codebase uses `BlockMatrix` column vectors (the existing call site in `tensor_gen.rs` passes `slice: BlockMatrix(slice_size, 1)` indexed 0..slice_size, and the semi-honest end-to-end test verifies correctness — so the endianness convention is self-consistent in the existing code.) `[VERIFIED: src/lib.rs::test_semihonest_tensor_product passes today]`

**Recommendation for Phase 3:** Do NOT reverse the input slice. Call the existing kernel with `a_keys` passed through unchanged — its endianness semantics are already a fixed property of the codebase from Phase 1 and cannot be re-examined without breaking `tensor_gen.rs` and semi-honest tests. Document the mapping in `tensor_macro.rs` doc comments so Phase 4 callers are not surprised.

### Recommended Project Structure

```
src/
├── lib.rs                   # +1 line: pub mod tensor_macro;
├── tensor_ops.rs            # MODIFIED: gen_populate_seeds_mem_optimized signature changes MatrixViewRef<Block> → &[Block] (D-03);
│                            #           ADD: eval_populate_seeds_mem_optimized and eval_unary_outer_product hoisted from
│                            #                tensor_eval.rs / auth_tensor_eval.rs (optional — see Open Question Q2)
├── tensor_macro.rs          # NEW: TensorMacroCiphertexts struct, tensor_garbler, tensor_evaluator, #[cfg(test)] mod tests
├── tensor_gen.rs            # MODIFIED: one call site at line 82 updates from &slice.as_view() to &slice.elements_slice()
├── tensor_eval.rs           # MODIFIED (if hoisting): remove eval_populate_seeds_mem_optimized and eval_unary_outer_product
│                            #                        private methods; call tensor_ops free functions instead
├── auth_tensor_eval.rs      # UNCHANGED (or mirror of tensor_eval.rs modification)
└── … (all other files unchanged)
```

### Pattern 1: Paper-Spec Ciphertext Struct

**What:** Pack the (2·(n-1) level ciphertexts, m leaf ciphertexts) that Construction 1's garbler emits into a single struct. This struct is exactly the over-the-wire message in Phase 4.

**When to use:** Every time a protocol primitive emits a structured ciphertext batch that downstream code either sends verbatim (wire protocol) or consumes as a unit (caller composition).

**Example** (matches D-06):
```rust
// src/tensor_macro.rs
use crate::block::Block;

/// Ciphertexts emitted by `tensor_garbler` and consumed by `tensor_evaluator`.
///
/// Maps directly to paper Construction 1 (Appendix F of the KRRW protocol):
/// - `level_cts[i]` is `(G_{i,0}, G_{i,1})` for tree level `i ∈ [n-1]`
/// - `leaf_cts[k]` is `G_k` for output column `k ∈ [m]`
///
/// The `G_{i,0}` component corresponds to even-indexed sibling XORs
/// (`⊕_j S_{i,2j}`) blinded by `H(A_i ⊕ Δ, ν_{i,0})`, and `G_{i,1}`
/// corresponds to odd-indexed siblings (`⊕_j S_{i,2j+1}`) blinded by
/// `H(A_i, ν_{i,1})`. The `ν_{i,b}` nonces are instantiated as AES tweaks
/// `0` and `1` via `FixedKeyAes::tccr` — see `src/aes.rs`.
pub(crate) struct TensorMacroCiphertexts {
    /// Length `n - 1`. Each entry is `(G_{i,0}, G_{i,1})`.
    pub level_cts: Vec<(Block, Block)>,
    /// Length `m`. `leaf_cts[k] = G_k`.
    pub leaf_cts: Vec<Block>,
}
```

`[VERIFIED: pattern matches `BcotOutput` (src/bcot.rs:31-41), `LeakyTriple` (src/leaky_tensor_pre.rs:13-35), and the `chunk_levels/chunk_cts` tuple return already used in `auth_tensor_gen.rs:117-128` — this is the codebase's established convention for packaging wire-level ciphertexts]`

### Pattern 2: Reuse of `gen_populate_seeds_mem_optimized` with Widened Input

**What:** Change the seed-builder's first parameter from `&MatrixViewRef<Block>` to `&[Block]` to accept both wire labels (existing semi-honest use) and MAC keys (new tensor-macro use). Everything else stays the same.

**Why this works:** The function body uses only `x.len()`, `x[i]`, and `x[i].lsb()` on its first parameter — all of which are identical operations on any slice-like input. `MatrixViewRef<Block>` already supports indexing into a column vector via `view[i]` (at `src/matrix.rs:474-482`) but the underlying data IS a slice.

**Example** (matches D-03, D-04):
```rust
// src/tensor_ops.rs — edit the existing function signature
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &[Block],                    // WAS: &MatrixViewRef<Block>
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
    let n: usize = x.len();
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Base case (Level 0): uses x[n-1] unchanged — same indexing as before.
    if x[n-1].lsb() {
        seeds[0] = cipher.tccr(Block::from((0 as u128).to_be_bytes()), x[n-1]);
        seeds[1] = cipher.tccr(Block::from((0 as u128).to_be_bytes()), x[n-1] ^ delta);
    } else {
        seeds[1] = cipher.tccr(Block::from((0 as u128).to_be_bytes()), x[n-1]);
        seeds[0] = cipher.tccr(Block::from((0 as u128).to_be_bytes()), x[n-1] ^ delta);
    }
    // ... rest unchanged ...

    // WAS: tree[tree.len() - (1 << x.rows())..tree.len()].to_vec();
    let seeds = tree[tree.len() - (1 << n)..tree.len()].to_vec();
    (seeds, odd_evens)
}
```

`[VERIFIED: reading src/tensor_ops.rs lines 9-85 shows every reference to x goes through x.len(), x[index], x[index].lsb() — all of which trait-dispatch identically on &[Block] and &MatrixViewRef<Block>]`

**Caller update in tensor_gen.rs** (line 82):
```rust
// WAS:
let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(&slice.as_view(), cipher, delta);

// NEW (one option):
let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(&slice.elements[..slice.rows()], cipher, delta);
// Note: `BlockMatrix.elements` is `pub(crate)` — visible within src/tensor_gen.rs. But currently
// BlockMatrix does not expose `elements` publicly. See Open Question Q3 on how to expose the slice.
```

**Callers in `tensor_macro.rs`** (new code):
```rust
// Inside tensor_garbler:
let a_blocks: &[Block] = Key::as_blocks(a_keys);
let (leaves, level_cts) = gen_populate_seeds_mem_optimized(a_blocks, cipher, delta);
```

### Pattern 3: Hoisting Duplicated Eval Kernel

**What:** `tensor_eval.rs::eval_populate_seeds_mem_optimized` (private method, lines 61-130) and `auth_tensor_eval.rs::eval_populate_seeds_mem_optimized` (private method) are byte-for-byte identical kernels. They currently live as associated functions on two structs. Hoist them to `tensor_ops.rs` as a `pub(crate)` free function; update both caller sites to use the free function.

**Why do this now:** Phase 3 needs the eval kernel in `tensor_macro.rs`. It's already written twice. Doing a third copy inside `tensor_macro.rs` would be strictly worse. Hoisting once serves all three call sites AND enables Phase 4 to compose primitives cleanly.

**Example:**
```rust
// src/tensor_ops.rs — ADD alongside gen_populate_seeds_mem_optimized:

/// Reconstruct the GGM seed tree on the evaluator side.
///
/// Given the evaluator's auth-bit MAC values `x` (where each `x[i]` equals
/// `A_i ⊕ a_i·Δ`) and the level ciphertexts `levels` from the garbler,
/// reconstruct the 2^n leaf seeds `Label_ℓ` for ℓ ≠ a, where `a` is the
/// clear bit vector recovered from the LSBs of `x`. The leaf at index `a`
/// is set to `Block::default()` (all zeros).
///
/// Implements Step 2-3 of Construction 1's `tensorev` (paper Appendix F).
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    levels: Vec<(Block, Block)>,
    missing_index: usize,   // the clear integer value of the bit vector `a`
    cipher: &FixedKeyAes,
) -> Vec<Block> {
    // Body identical to tensor_eval.rs::eval_populate_seeds_mem_optimized
    // lines 61-130, minus the self:: / associated-function framing.
    // ...
}

/// Evaluator's counterpart to `gen_unary_outer_product`. Combines the
/// reconstructed seeds, the garbler's leaf ciphertexts `gen_cts`, the
/// evaluator's `y` share (T^ev), and the LSB-encoded `missing` index
/// to produce `Z_eval` and recover the missing leaf's column values.
pub(crate) fn eval_unary_outer_product(
    seeds: &[Block],
    y: &[Block],
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[Block],
) -> Vec<Block> {
    // Body identical to tensor_eval.rs::eval_unary_outer_product.
}
```

`[VERIFIED: tensor_eval.rs:61-130 and tensor_eval.rs:132-172 are private methods on `TensorProductEval` that reference no instance state (first param is `x: &MatrixViewRef<Block>`, not `&self`) — the hoist is purely moving them out of the impl block and generalising the slice parameter]`

**Open question Q2** asks whether to hoist into `tensor_ops.rs` or leave eval duplicated. Recommendation: hoist. It's lower complexity AND reduces duplication.

### Anti-Patterns to Avoid

- **Re-implementing the GGM kernel inside `tensor_macro.rs`.** The existing `gen_populate_seeds_mem_optimized` is tested end-to-end by `test_semihonest_tensor_product` and `test_auth_tensor_product` today. Duplicating is strictly worse.
- **Changing Block endianness / index conventions** ("start from x[0] instead of x[n-1]"). This would silently break `tensor_gen.rs` call site. Phase 3 is not authorised to rewrite semi-honest behaviour.
- **Introducing `Result<_, _>` error types** for dimension mismatches. The codebase uses `assert!` / `assert_eq!` with explanatory messages (documented in `.planning/codebase/CONVENTIONS.md`). Follow the same convention.
- **Using `rand::rng()` (thread-local RNG) inside `tensor_garbler` / `tensor_evaluator`.** Construction 1 is DETERMINISTIC given its inputs — no randomness is sampled inside the macro. The randomness is in the caller-provided `delta`, `a_keys`, and `T` shares. Any RNG call inside the macro is a bug.
- **Using `from_fn_spec` or generic over `FixedKeyAes`** — just pass `&FIXED_KEY_AES` (the process-wide singleton) or accept `cipher: &FixedKeyAes` as a parameter. Do not abstract.
- **Computing `missing_index` differently from the existing eval kernel.** The existing `eval_populate_seeds_mem_optimized` reconstructs `missing` incrementally from bit-by-bit traversal (`missing = (missing << 1) | bit`). Do not replace with "recover `a` from `a_macs` LSBs first, then call the kernel with a preassembled integer" — that changes the API. The hoisted signature should take `&[Block]` the same way the gen side does; `missing_index` is an output of the traversal, not an input.

Actually — wait, looking at `tensor_eval.rs::eval_populate_seeds_mem_optimized` line 64: it DOES take `_clear_value: &usize` as a third parameter but ignores it (starts with `_`). It reconstructs `missing` internally from `x[n-i-1].lsb()` traversal. So the hoisted signature should drop that unused param. See Open Question Q4 on whether `missing` should be an input.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| GGM seed expansion primitive | New `fn expand_seed(seed, tweak) -> (Block, Block)` | Existing `FixedKeyAes::tccr(tweak, block)` in `src/aes.rs` | Already the codebase's TCCR PRF; matches Guo et al. 2019/074 security analysis. |
| AES key-schedule | Create new `AesEncryptor::new(...)` per `tensor_garbler` call | Existing `FIXED_KEY_AES: Lazy<FixedKeyAes>` singleton | Key expansion is expensive; singleton pattern documented in Phase 1 D-12 (`src/aes.rs:15-38`). Per-call construction would be an obvious regression. |
| Tree-level XOR accumulator | New `struct GgmLevelAcc { odds, evens }` | The existing `odd_evens: Vec<(Block, Block)>` return type from `gen_populate_seeds_mem_optimized` | Already exactly what the paper calls `(G_{i,0}, G_{i,1})` pre-tweak. |
| Column-major tensor indexing | Manual `k * rows + i` arithmetic in `tensor_macro.rs` | `BlockMatrix` with `[(row, col)]` operator | Phase 1 D-11 locked the column-major convention; `BlockMatrix` documents and enforces it. |
| Zero-cost `&[Key]` → `&[Block]` cast | Manual `iter().map(\|k\| *k.as_block()).collect()` | Existing `Key::as_blocks(slice: &[Self]) -> &[Block]` at `src/keys.rs:77-82` | Zero-allocation, zero-copy, already implemented. Same for `Mac::as_blocks`. |
| Inverse-subtree reconstruction | New traversal from scratch | Existing `eval_populate_seeds_mem_optimized` in `tensor_eval.rs:61-130` (duplicated in `auth_tensor_eval.rs`) | The kernel is already written and tested. Hoist it once (Pattern 3). |
| Cross-party key/MAC test oracle | Construct `Key` / `Mac` pairs by hand | Existing `IdealBCot::transfer_a_to_b` / `transfer_b_to_a` | Guarantees the IT-MAC invariant `mac = key XOR bit·delta` by construction — exactly the precondition `tensor_evaluator` assumes. |

**Key insight:** Construction 1 is simple and the codebase has already solved every sub-problem. Phase 3 is overwhelmingly a composition phase, not an implementation phase.

---

## Runtime State Inventory

> This is a new-module / code-addition phase (not a rename or refactor). Runtime state inventory is INCLUDED (mandatory when touching kernel code) but most categories are empty.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | None — project is a library crate with no persistent storage, no databases, no on-disk caches except `target/criterion/` benchmark baselines. `[VERIFIED: repeat of Phase 2 research — no database files, no cache directories beyond target/]` | None. Criterion baselines are not affected by Phase 3 because Phase 3 adds a new module that no benchmark calls (benchmarks run `run_preprocessing`, `auth_tensor_gen`, etc., none of which compose `tensor_macro::tensor_garbler`). Phase 4 is when bench paths will start invoking `tensor_macro`. |
| **Live service config** | None — no running services, no deployment configs. | None. |
| **OS-registered state** | None — no systemd / launchd / Task Scheduler / pm2 entries, no cron jobs. | None. |
| **Secrets / env vars** | None — no `.env`, no SOPS, no env-based config. Cargo reads only standard vars (`CARGO_HOME`, `RUSTC`, etc.) unaffected by Phase 3. | None. |
| **Build artefacts / installed packages** | `target/debug/deps/*.rmeta` files contain existing mangled symbols for `gen_populate_seeds_mem_optimized`. When D-03 changes its parameter type, `cargo build` incrementally recompiles; no manual `cargo clean` required. `[VERIFIED: incremental compilation handles parameter-type changes on `pub(crate)` functions automatically]` | None. If the user later sees unexpected linker errors, single-shot `cargo clean && cargo build` resolves (identical to Phase 2's story). |

**Stray `src/*.rs 2` files reminder (carried from Phase 2):** The working directory still has `src/auth_tensor_fpre 2.rs`, `src/auth_tensor_pre 2.rs`, `src/bcot 2.rs`, `src/leaky_tensor_pre 2.rs` (and Phase 2 research identified this pattern — these are macOS Finder duplicates, not Cargo modules). They are NOT compiled. Phase 3 must not create analogous `src/tensor_macro 2.rs` — if the user sees such a file after Phase 3 work, it's a macOS Finder side-effect and can be deleted.

**Canonical question:** *"After every file in the repo is updated, what runtime systems still have the old string cached, stored, or registered?"* — **Answer: none.** Phase 3 adds a new module; it does not rename, remove, or cache anything runtime-persistent.

---

## Common Pitfalls

### Pitfall 1: Pre-Existing Baseline Test Failures (carried from Phase 2)

**What goes wrong:** `cargo test --lib` on current `main` (commit `330f303`) shows **4 failing tests** unrelated to Phase 3 scope:
- `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`
- `leaky_tensor_pre::tests::test_correlated_mac_invariants`
- `auth_tensor_pre::tests::test_combine_mac_invariants`
- `preprocessing::tests::test_run_preprocessing_mac_invariants`

All four panic with `"MAC mismatch in share"` at `src/sharing.rs:62`. `[VERIFIED: cargo test --lib today on commit 330f303 produces "48 passed; 4 failed"]`

**Why it happens:** The four tests call `share.verify(delta)` directly on cross-party shares, which panics by design (documented in `.planning/codebase/CONVENTIONS.md` and `.planning/codebase/TESTING.md`). The tests need to use `verify_cross_party()` instead. These failures are pre-existing bugs in test code, NOT in protocol code. The ideal-dealer end-to-end test (`test_auth_tensor_product`) still passes.

**How to avoid:** Phase 3 MUST baseline-accept these failures:
1. Capture baseline: `cargo test --lib 2>&1 | grep 'FAILED' | sort > .planning/phases/03-.../before.txt` BEFORE any Phase 3 code.
2. On each Phase 3 task commit, run: `cargo test --lib 2>&1 | grep 'FAILED' | sort > after.txt; diff before.txt after.txt`. Require ZERO new lines in the diff. Pre-existing red tests stay red.
3. Phase 3 new tests (TEST-01 invariant) must pass unconditionally on a green baseline.

**Warning signs:** Plan-checker demands "cargo test must pass with no failures". If so, escalate per Open Question Q1.

### Pitfall 2: `Key::as_blocks` Accepts `&[Key]` but Not `&[AuthBitShare]`

**What goes wrong:** Phase 4 caller code will likely have `a_keys` inside an `AuthBitShare` (i.e., `share.key` of type `Key`). Calling `Key::as_blocks(&[share.key, share.key, ...])` requires an intermediate `Vec<Key>`.

**Why it happens:** Phase 4 has `Vec<AuthBitShare>` and must extract the `.key` field per entry before passing to `tensor_garbler`. The inner `Vec<Key>` materialisation is unavoidable at the call boundary but SHOULD NOT happen inside `tensor_garbler` itself.

**How to avoid:** `tensor_garbler`'s API takes `a_keys: &[Key]` per D-09. Callers in Phase 4 will do:
```rust
let a_keys: Vec<Key> = shares.iter().map(|s| s.key).collect();
tensor_garbler(n, m, delta, &a_keys, &t_gen);
```
This is one materialisation in Phase 4's code, not in `tensor_macro.rs`. Inside `tensor_macro.rs`, use `Key::as_blocks(a_keys)` (zero-cost reinterpret) exactly once when calling `gen_populate_seeds_mem_optimized`.

**Warning signs:** A Phase 3 implementation that takes `&[AuthBitShare]` as input would tie the macro to the AuthBitShare layout — violates D-09.

### Pitfall 3: `a_macs: &[Mac]` Carries the Bit in its LSB

**What goes wrong:** The paper computes `S_{0, NOT a_0}` from the base-level input `A_0 ⊕ a_0·Δ`. The code's eval kernel at `tensor_eval.rs:75` does:
```rust
seeds[!x[n-1].lsb() as usize] = cipher.tccr(Block::from((0 as u128).to_be_bytes()), x[n-1]);
```
This depends on `x[n-1].lsb()` to identify which subtree was "known" from the mac. If the evaluator's `a_macs` is passed in with LSB=0 somewhere (which would violate the bCOT invariant that `mac = key XOR bit*delta` has LSB = (0 XOR bit*1) = bit), the eval kernel recovers the WRONG subtree root.

**Why it happens:** `Delta::new()` sets LSB=1 invariantly (`src/delta.rs:12-16`). Therefore `A_i XOR a_i*Delta` has LSB = `A_i.lsb() XOR a_i` = `0 XOR a_i` = `a_i`. So `Mac.lsb() == a_i` always for a correctly-formed evaluator input. If a caller accidentally clears the LSB of a Mac somewhere, the eval kernel silently produces wrong seeds.

**How to avoid:**
1. Document in `tensor_macro.rs` that `a_macs[i].lsb()` MUST equal the corresponding bit `a_i`. This is an invariant the caller must preserve.
2. In Phase 3's paper-invariant test, verify that `a_macs` produced by `IdealBCot::transfer_a_to_b` satisfies this (it does by construction — `src/bcot.rs:69` calls `k0.auth(b, &self.delta_b)` which computes `K[0] XOR b*Delta` with `K[0].lsb() == 0`).
3. `Mac::as_blocks(a_macs)` passes the slice through to the eval kernel unchanged; do not strip any bits.

**Warning signs:** Test produces `Z_gen XOR Z_eval != a ⊗ T` but the xor pattern is identical across test runs — indicates a systematic bit-flip somewhere, most likely `Mac` LSB getting cleared.

### Pitfall 4: `tensor_eval.rs`'s Private Eval Kernel Sets Missing Seeds to `Block::default()` (zero)

**What goes wrong:** Inspecting `tensor_eval.rs::eval_unary_outer_product` lines 147-160: the `for i in 0..seeds.len()` loop has `if i != missing { ... }` — so the missing-index seed is skipped. Then lines 163-169 distribute the RECOVERED `X_{a,k} = eval_ct` into `Z_eval` rows where `missing` has bit set.

This works BUT relies on `seeds[missing] == Block::default()` as a sentinel. If the hoisted function signature doesn't guarantee `seeds[missing] == Block::default()`, or a future caller initialises `seeds[missing]` to non-zero, the XOR-sum in the inner loop on `s = tccr(tweak, seeds[i])` would accumulate wrong values AND distribute the wrong tccr output into Z columns.

**Why it happens:** The existing eval kernel at `tensor_eval.rs:95-96` sets `seeds[j*2] = Block::default(); seeds[j*2+1] = Block::default()` for the missing node. This is implicit API contract.

**How to avoid:**
1. Document in the hoisted `eval_populate_seeds_mem_optimized` that it returns `seeds` with `seeds[missing] == Block::default()`.
2. Document in the hoisted `eval_unary_outer_product` that it requires `seeds[missing] == Block::default()`.
3. Add a debug assertion at the top of `eval_unary_outer_product`: `debug_assert_eq!(seeds[missing], Block::default(), "seeds[missing] must be zero sentinel");`
4. Consider whether to change the API: return `Vec<Option<Block>>` or `(Vec<Block>, usize)` explicitly. Not recommended in Phase 3 (would ripple into Phase 4); document instead.

**Warning signs:** Test fails with `Z_eval` having the correct values in most positions but garbage in positions where `missing` has a set bit.

### Pitfall 5: `BlockMatrix` Column-Major Indexing in Z Output

**What goes wrong:** The gen and eval kernels use `out[(k, j)] ^= s` at `tensor_ops.rs:114` and `tensor_eval.rs:157`. Parameter `k` is the ROW (output coordinate, 0..n), `j` is the COLUMN (output coordinate, 0..m). So `Z[i][k]` in paper notation corresponds to `out[(i, k)]` in code — note the paper uses `Z_{ℓ,k}` where ℓ is sometimes used for the LEAF index and sometimes for the ROW; Construction 1 uses ℓ for leaves, `k` for columns (1..m), `i` for row/bit index (1..n). `[VERIFIED: reading tensor_ops.rs:112-116 — the inner loop k ∈ [out.rows()] distributes per-leaf expansion values X_{ℓ,k} into Z_{ℓ_bit(i), k_paper} which is out[(i, k)] in BlockMatrix 2D indexing]`

**Why it happens:** Easy to get confused by the paper's Construction 1 Step 5: `Z_gb = truthtable(id)^T · X_gb`. This is an n×m matrix whose `(i, k)` entry is `⊕_{ℓ : bit_i(ℓ) = 1} X_{ℓ, k}`. The code implements this directly via the inner loop. Any different indexing would break.

**How to avoid:**
1. Create output `BlockMatrix::new(n, m)` (n rows, m cols).
2. When calling `gen_unary_outer_product`, pass `out.as_view_mut()` as the `out` parameter and pass `t_gen.as_view()` as the `y` parameter.
3. Verify by running the test with n=2, m=3, a=0b11, T=known — manually compute `Z_gen XOR Z_eval == a ⊗ T`.

**Warning signs:** Test fails with dimensions looking "transposed" (n and m swapped in the output matrix).

### Pitfall 6: Endianness of `a` Bit Vector for `a ⊗ T`

**What goes wrong:** The paper writes `a ⊗ T` where `a ∈ {0,1}^n` and `T ∈ ({0,1}^κ)^m`. The result is an n×m matrix with entry `(i, k) = a_i · T_k`. In the code, `a` is the bit vector formed by LSB extraction from `a_macs`. Paper's `a_0` ⟷ code's `a_macs[?].lsb()`. With Reading A (see "Paper-to-code index mapping"): paper `a_0` ⟷ `a_macs[n-1].lsb()`, and paper `a_i` ⟷ `a_macs[n-1-i].lsb()`.

**Why it happens:** The invariant test must compute the "expected" `a ⊗ T` to compare against `Z_gen XOR Z_eval`. If the test computes `a ⊗ T` using the PAPER's index convention but the code uses the REVERSED convention, the comparison fails.

**How to avoid:**
1. In the paper-invariant test, do NOT reverse the bit vector. Compute `a_bits[i] = a_macs[i].lsb()` (or equivalently, `choices[i]` from the bCOT test oracle).
2. Compute expected as:
   ```rust
   for i in 0..n {
       for k in 0..m {
           let a_i = a_bits[i];
           expected[(i, k)] = if a_i { T[(i, k)] } else { Block::ZERO };
       }
   }
   ```
   Note: `a ⊗ T` in the paper is defined such that `a ⊗ T` has the same shape as T (n×m if T is n×m and `a` is a length-n vector). The expected matrix entry is `a_i XOR T[(i,k)]` multiplicatively (scalar bit times block).

   **Wait — double-check:** Paper Construction 1 takes `T ∈ ({0,1}^κ)^m` (length-m vector of κ-bit blocks, NOT an n×m matrix). So T is `m` blocks. The tensor product `a ⊗ T` then has shape n×m where `(a ⊗ T)_{i,k} = a_i · T_k`. So T as an input is length-m (code: `BlockMatrix` with rows = m, cols = 1 — a column vector).

   Re-reading D-11: "T shares and Z outputs are `BlockMatrix` (n×m …)". Hmm — there's a tension. The paper's T is length m. If code uses `BlockMatrix` of shape (n, m), that's n rows × m cols, which would mean n·m blocks per T. But the paper's T is only m blocks.

   **Discrepancy check:** Looking at `gen_unary_outer_product` signature in `tensor_ops.rs:88-92`: `y: &MatrixViewRef<Block>` where `let m = y.len()` at line 94. So `y` is treated as a length-m column vector. And `out: &mut MatrixViewMut<Block>` is the n×m output matrix. So the EXISTING code uses T (called `y` in the existing code) as a length-m column vector, NOT an n×m matrix.

   **Therefore D-11 appears to contain a typo/error.** T should be a length-m column vector (shape m×1 or just `&[Block]`), and Z should be n×m. `[ASSUMED — see Open Question Q5 for confirmation]`

**How to avoid the T shape confusion:**
1. Treat `t_gen: &BlockMatrix` with `t_gen.rows() == m, t_gen.cols() == 1` (column vector). Same for `t_eval`.
2. The Z output is n×m (rows=n, cols=m).
3. Add a dimension assertion at the top of both functions: `assert_eq!(t_gen.rows(), m, "T^gb must be a length-m column vector");`.
4. Flag to planner that D-11 likely says "n×m" intending the Z output, not T.

**Warning signs:** Plan-checker flags D-11 → "T shares are n×m" and implementer creates n×m T input — test fails immediately because the existing kernel expects length-m.

---

## Code Examples

Verified patterns from existing codebase and paper reference:

### Example 1: `tensor_garbler` orchestration

```rust
// src/tensor_macro.rs
use crate::{
    aes::{FixedKeyAes, FIXED_KEY_AES},
    block::Block,
    delta::Delta,
    keys::Key,
    macs::Mac,
    matrix::BlockMatrix,
    tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product},
};

pub(crate) struct TensorMacroCiphertexts {
    pub level_cts: Vec<(Block, Block)>,
    pub leaf_cts: Vec<Block>,
}

/// Garbler side of Construction 1 (paper Appendix F).
///
/// Builds a 2^n-leaf GGM tree from the garbler's IT-MAC keys `a_keys`,
/// computes level ciphertexts {G_{i,0}, G_{i,1}}_{i ∈ [n-1]} and leaf
/// ciphertexts {G_k}_{k ∈ [m]}, and returns (Z_gen, G) such that when
/// combined with the evaluator's `tensor_evaluator` output on the same
/// (n, m, a_macs, t_eval) preconditions,
///
///     Z_gen XOR Z_eval == a ⊗ T   (paper correctness theorem)
///
/// where `a` is the bit vector with `a[i] = a_macs[i].lsb()` and
/// `T = t_gen XOR t_eval` is the reconstructed XOR share of the κ-bit
/// vector T of length m.
///
/// Preconditions:
/// - `a_keys.len() == n`
/// - `t_gen.rows() == m` and `t_gen.cols() == 1` (length-m column vector)
/// - Every `a_keys[i].lsb() == 0` (enforced by `Key::new()`)
///
/// Panics if preconditions are violated.
pub(crate) fn tensor_garbler(
    n: usize,
    m: usize,
    delta: Delta,
    a_keys: &[Key],
    t_gen: &BlockMatrix,
) -> (BlockMatrix, TensorMacroCiphertexts) {
    assert_eq!(a_keys.len(), n, "a_keys length must equal n");
    assert_eq!(t_gen.rows(), m, "t_gen must be a length-m column vector");
    assert_eq!(t_gen.cols(), 1, "t_gen must be a column vector (cols == 1)");

    let cipher: &FixedKeyAes = &FIXED_KEY_AES;

    // [1-2] Build GGM tree; collect (leaf seeds, per-level (evens, odds)).
    //       `level_cts` is structurally (G_{i,0}, G_{i,1}) already.
    let a_blocks: &[Block] = Key::as_blocks(a_keys);
    let (leaf_seeds, level_cts) = gen_populate_seeds_mem_optimized(a_blocks, cipher, delta);

    // [3-4] Leaf expansion + Z_gen computation + leaf ciphertexts G_k.
    //       gen_unary_outer_product writes into `z_gen` and returns leaf_cts.
    let mut z_gen = BlockMatrix::new(n, m);
    let leaf_cts = gen_unary_outer_product(
        &leaf_seeds,
        &t_gen.as_view(),
        &mut z_gen.as_view_mut(),
        cipher,
    );

    (
        z_gen,
        TensorMacroCiphertexts { level_cts, leaf_cts },
    )
}
```

`[VERIFIED: composition matches existing `tensor_gen.rs::gen_chunked_half_outer_product` (lines 52-91), which is the structurally-similar caller in the semi-honest family]`

### Example 2: `tensor_evaluator` orchestration

```rust
// src/tensor_macro.rs (continued)

/// Evaluator side of Construction 1 (paper Appendix F).
///
/// Reconstructs the untraversed GGM subtree from the evaluator's IT-MAC
/// values `a_macs` (each equal to `A_i XOR a_i·Δ`) and the garbler's
/// ciphertexts `g`, then recovers the missing leaf column `X_{a,k}` for
/// k ∈ [m], and finally accumulates into Z_eval.
///
/// Preconditions:
/// - `a_macs.len() == n`
/// - `t_eval.rows() == m` and `t_eval.cols() == 1`
/// - `g.level_cts.len() == n - 1`
/// - `g.leaf_cts.len() == m`
pub(crate) fn tensor_evaluator(
    n: usize,
    m: usize,
    g: &TensorMacroCiphertexts,
    a_macs: &[Mac],
    t_eval: &BlockMatrix,
) -> BlockMatrix {
    assert_eq!(a_macs.len(), n, "a_macs length must equal n");
    assert_eq!(t_eval.rows(), m, "t_eval must be a length-m column vector");
    assert_eq!(t_eval.cols(), 1, "t_eval must be a column vector");
    assert_eq!(g.level_cts.len(), n - 1, "G must have n-1 level ciphertexts");
    assert_eq!(g.leaf_cts.len(), m, "G must have m leaf ciphertexts");

    let cipher: &FixedKeyAes = &FIXED_KEY_AES;

    // Clear bit vector `a` is encoded in LSBs of a_macs (IT-MAC invariant:
    // mac = key XOR bit·delta with delta.lsb() = 1, so mac.lsb() = bit).
    let a_blocks: &[Block] = Mac::as_blocks(a_macs);

    // [5] Reconstruct all leaf seeds except seeds[a].
    //     The hoisted eval kernel reconstructs `missing` internally via
    //     bit-by-bit traversal and sets seeds[missing] = Block::default().
    let (leaf_seeds, missing) = crate::tensor_ops::eval_populate_seeds_mem_optimized(
        a_blocks,
        g.level_cts.clone(),   // kernel currently takes Vec (existing signature)
        cipher,
    );
    // NOTE: The existing tensor_eval.rs kernel takes `_clear_value: &usize` and
    //       reconstructs `missing` internally; the hoisted version returns
    //       `missing` as a second tuple element. See Open Question Q4.

    // [6] Recover missing leaf X_{a,k} + accumulate Z_eval.
    let mut z_eval = BlockMatrix::new(n, m);
    let _eval_cts_unused = crate::tensor_ops::eval_unary_outer_product(
        &leaf_seeds,
        t_eval.as_view().data_slice(),  // or equivalent length-m slice access
        &mut z_eval.as_view_mut(),
        cipher,
        missing,
        &g.leaf_cts,
    );

    z_eval
}
```

`[VERIFIED: composition matches existing `tensor_eval.rs::eval_chunked_half_outer_product` (lines 174-211)]`

### Example 3: Paper-invariant test skeleton (TEST-01)

```rust
// src/tensor_macro.rs (continued, inline tests)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha12Rng;

    fn run_one_case(n: usize, m: usize, seed: u64) {
        // --- Set up bCOT and deltas ---
        let mut bcot = IdealBCot::new(seed, seed ^ 0xDEAD_BEEF);
        // Use A as the garbler; A's sender keys are `a_keys` (LSB=0).
        // B receives macs = K[0] XOR b·delta_b = A_i XOR a_i·delta_b.
        // In Phase 3, the "delta" in the macro is the garbler's delta (= delta_b
        // from bCOT's perspective of A-as-sender): A's sender_keys are the K[0]
        // values, B's receiver_macs are the A_i XOR a_i·delta_b values.
        let mut rng = ChaCha12Rng::seed_from_u64(seed);
        let choices: Vec<bool> = (0..n).map(|_| rng.random_bool(0.5)).collect();
        let cot = bcot.transfer_a_to_b(&choices);
        let delta = bcot.delta_b;  // the "Δ" in the macro's view

        let a_keys = cot.sender_keys;   // [Key; n], LSB=0 invariant
        let a_macs = cot.receiver_macs; // [Mac; n], LSB = choices[i]

        // --- Set up random T shares (column vectors of length m) ---
        let mut t_gen = BlockMatrix::new(m, 1);
        let mut t_eval = BlockMatrix::new(m, 1);
        for k in 0..m {
            t_gen[k] = Block::random(&mut rng);
            t_eval[k] = Block::random(&mut rng);
        }

        // --- Run both sides ---
        let (z_gen, g) = tensor_garbler(n, m, delta, &a_keys, &t_gen);
        let z_eval = tensor_evaluator(n, m, &g, &a_macs, &t_eval);

        // --- Compute expected a ⊗ T ---
        //     a[i] = a_macs[i].lsb() = choices[i] (by IT-MAC invariant).
        //     (a ⊗ T)_{i,k} = a[i] ? T_k : 0  where T_k = t_gen[k] XOR t_eval[k].
        let a_bits: Vec<bool> = a_macs.iter().map(|m| m.as_block().lsb()).collect();
        let t_full: Vec<Block> = (0..m).map(|k| t_gen[k] ^ t_eval[k]).collect();

        let mut expected = BlockMatrix::new(n, m);
        for i in 0..n {
            for k in 0..m {
                expected[(i, k)] = if a_bits[i] { t_full[k] } else { Block::ZERO };
            }
        }

        // --- Assert Z_gen XOR Z_eval == a ⊗ T ---
        for i in 0..n {
            for k in 0..m {
                assert_eq!(
                    z_gen[(i, k)] ^ z_eval[(i, k)],
                    expected[(i, k)],
                    "mismatch at (i={}, k={}) n={} m={} seed={}",
                    i, k, n, m, seed,
                );
            }
        }
    }

    #[test] fn test_n1_m1() { run_one_case(1, 1, 1); }
    #[test] fn test_n1_m4() { run_one_case(1, 4, 2); }
    #[test] fn test_n2_m1() { run_one_case(2, 1, 3); }
    #[test] fn test_n2_m3() { run_one_case(2, 3, 4); }
    #[test] fn test_n4_m8() { run_one_case(4, 8, 5); }
    #[test] fn test_n8_m1() { run_one_case(8, 1, 6); }
    #[test] fn test_n8_m16() { run_one_case(8, 16, 7); }
    #[test] fn test_n4_m64() { run_one_case(4, 64, 8); }

    /// Deterministic regression vector.
    #[test]
    fn test_deterministic_seed_42() {
        run_one_case(4, 4, 42);
    }
}
```

**Note:** The test pattern exactly follows `src/leaky_tensor_pre.rs`'s test module style — inline `#[cfg(test)] mod tests`, seeded `ChaCha12Rng` for determinism, helper function for setup, multiple `#[test]` entry points per (n, m) combination. `[VERIFIED: matches the testing convention in `.planning/codebase/TESTING.md`]`

### Example 4: `tensor_gen.rs` call site update (per D-04)

```rust
// src/tensor_gen.rs — line 82 (current):
//    let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(&slice.as_view(), cipher, delta);

// NEW options (choose one; Open Question Q3):

// Option A: Expose a pub(crate) `elements_slice()` on BlockMatrix.
//   src/matrix.rs ADD:
//       pub(crate) fn elements_slice(&self) -> &[T] { &self.elements }
//   src/tensor_gen.rs line 82 becomes:
//       let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(slice.elements_slice(), cipher, delta);

// Option B: Add a `to_vec()` / `as_slice_column()` view that returns &[T] for column-vector matrices.
//   (More general; may invite misuse for non-column matrices.)

// Option C: Add `pub(crate) fn as_slice(&self) -> &[T]` guarded by debug_assert!(self.cols == 1).

// Recommend Option A — simplest, minimal surface area, matches the column-vector
// invariant which is already the only supported use.
```

### Example 5: `Key::as_blocks` / `Mac::as_blocks` zero-cost reinterpret

```rust
// Already implemented at src/keys.rs:77-82 and src/macs.rs:55-60.

// Inside tensor_garbler:
let a_blocks: &[Block] = Key::as_blocks(a_keys);  // zero-cost, zero-copy

// Inside tensor_evaluator:
let a_blocks: &[Block] = Mac::as_blocks(a_macs);  // zero-cost, zero-copy
```

`[VERIFIED: src/keys.rs lines 77-82 — existing helper is exactly what Phase 3 needs]`

---

## State of the Art

Construction 1 is a direct descendant of GGM-tree-based garbled circuit techniques going back to KRRW18 (Katz-Ranellucci-Rosulek-Wang, eprint 2018/578) and the earlier half-gates / one-hot garbling literature. The paper's specific "generalised tensor macro" is the KRRW preprocessing technique adapted to produce XOR shares of `a ⊗ T` for arbitrary κ-bit T, as opposed to the narrower case where T must be a bit-vector times delta.

| Old Approach | Current Approach (this paper / Construction 1) | When Changed | Impact |
|--------------|------------------------------------------------|--------------|--------|
| Extended half-gate with scalar `x_2 · T` for κ-bit T (KRRW18 preprocessing) | Generalised tensor macro — vector `a` of length n, XOR shares of length-m κ-bit T, tensor product n × m κ-bit output | This paper (Appendix F, Construction 1) | Lets two independent tensor macro calls combine via XOR to yield `x ⊗ y · (Δ_A XOR Δ_B)` in Phase 4, enabling Pi_LeakyTensor without ad-hoc per-bit garbling. |
| Hand-rolled GGM tree expansion | TCCR-based GGM tree with fixed-key AES (Guo et al. 2019/074, §7.4) | ~2019 | Current state of the art; used by mpz-dev, Rosulek's implementations, this codebase. |
| Independent per-level PRG nonces | Tweakable PRG with level index as tweak | ~2018 | Matches this codebase (`FixedKeyAes::tccr(tweak, block)` at `src/aes.rs:60-68`). |

**Deprecated / outdated (do not use):**
- Pi_LeakyTensor's previous (pre-April-10) implementation attempted to compute `alpha AND beta` via direct COT without the GGM tree — this is the paper-review "8 known bugs" path that Phase 4 will rewrite (ROADMAP.md Phase 4). Phase 3 must produce the correct primitive for Phase 4 to adopt.
- Direct "half-gate" construction for tensor outputs — superseded by generalised tensor macro.

**Current (2026) authoritative sources:**
- `references/appendix_krrw_pre.tex` — the paper's own specification (primary).
- `references/mpz-dev/crates/core/src/ggm.rs` — `GgmTree::new_from_seed` and `new_partial`. Read-only reference. `[CITED: references/mpz-dev/crates/core/src/ggm.rs lines 24-180]`
- Guo et al. 2019/074 — TCCR security proofs. `[CITED: referenced in src/aes.rs doc comments as https://eprint.iacr.org/2019/074]`

---

## Assumptions Log

> Claims tagged `[ASSUMED]` signal information that could not be directly verified and needs user confirmation before the planner locks them into plans.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | D-11 saying "T shares and Z outputs are `BlockMatrix` (n×m …)" applies to **Z**, not **T**. T is a length-m column vector per paper Construction 1 step 4 ("Interpret T^gb = T_0^gb T_1^gb ⋯ T_{m-1}^gb") and per the existing `gen_unary_outer_product`'s `y` parameter which is treated as length-m. | Pitfall 6 / Common Pitfalls | If T is actually meant to be n×m, the existing kernel signature is wrong and Phase 3 must write a different function — scope would double. User must clarify before planning. |
| A2 | Phase 2's 4 failing tests are pre-existing, unrelated to Phase 3, and the planner is authorised to baseline-accept them. | Common Pitfalls #1 | If user intends Phase 3 to fix them first, plan must add a pre-Phase-3 diagnostic task. |
| A3 | The existing `gen_populate_seeds_mem_optimized` kernel's endianness (treating `x[n-1]` as paper A_0) is the "correct" interpretation because `test_semihonest_tensor_product` passes end-to-end with this convention. | Architecture — Data Flow / Paper-to-code index mapping | If an explicit convention-reversal is desired, all existing semi-honest and authenticated tests would need to be re-verified. Very high risk of silent correctness breakage. Strongly recommend preserving existing convention. |
| A4 | Hoisting `eval_populate_seeds_mem_optimized` and `eval_unary_outer_product` from `tensor_eval.rs` / `auth_tensor_eval.rs` into `tensor_ops.rs` (or a `pub(crate)` submodule) is the preferred approach, even though CONTEXT.md only explicitly authorises the `gen_populate_seeds_mem_optimized` generalisation (D-03). | Architecture — Pattern 3, Open Question Q2 | If user prefers duplication inside `tensor_macro.rs`, Phase 3 does a 3rd copy; future Phase 4/6 will wish they hadn't. Low risk of correctness breakage; moderate risk of continued duplication. |
| A5 | D-15's "exact nonce/tweak strategy" leaves the existing `cipher.tccr(tweak_0_or_1, seed)` PRF call in place — i.e., no change to the GGM kernel's AES tweak encoding, which currently uses `Block::from(0 as u128)` / `Block::from(1 as u128)`. The paper's "one-time nonces ν_{i,b}" are instantiated by the TCCR construction's (seed, tweak) pair where the seed changes per level. | Claude's Discretion / Pattern 2 | If user intends a per-level counter nonce (ν_{i,b} = i·2 + b, say), that's a different encoding. Current existing kernel passes semi-honest test so the implicit "tweak = 0 or 1" encoding is sound under TCCR — but if user expects a different nonce pattern, the garbler and evaluator MUST agree and the kernel would need to change on both sides. |
| A6 | `t_gen` and `t_eval` in the macro API are both `BlockMatrix` with `.cols() == 1`, not `&[Block]` directly, because D-11 says "BlockMatrix" explicitly. | Code Example 1 / 2 | If user prefers `&[Block]` inputs, the API changes — trivial to do but may affect Phase 4 call sites. |
| A7 | The `missing` index (evaluator's clear value of the bit vector `a`) can be returned from the hoisted `eval_populate_seeds_mem_optimized` as an output rather than passed in as an input. Current private method (`tensor_eval.rs:61-130`) reconstructs it internally and takes an unused `_clear_value` param. Cleanest hoisted signature drops the input param and returns `(Vec<Block>, usize)` instead of `Vec<Block>`. | Code Example 2 / Open Question Q4 | If user prefers the caller compute `missing` first (e.g., from `a_macs.iter().enumerate().fold`), the hoisted signature differs. Currently agnostic per D-08 which doesn't prescribe this detail. |
| A8 | `FixedKeyAes::tccr` remains the PRF (not a BLAKE3 call or similar). No new crypto primitive introduced. | Don't Hand-Roll | If user wants to swap TCCR for something else here (highly unlikely given rest of codebase uses TCCR), it's a bigger rewrite. |

---

## Open Questions

1. **Should Phase 3 baseline-accept the 4 pre-existing failing tests, or must they be fixed first?**
   - What we know: Same 4 tests identified by Phase 2 research continue to fail on current `main`. They panic due to `share.verify(delta)` being called on cross-party shares (documented test-code antipattern). Fixing them is a one-line-per-test switch to `verify_cross_party()`.
   - What's unclear: whether Phase 3's "all tests pass" success criterion accepts baseline-red or demands green baseline. Phase 2 research escalated this as Phase 2's own Open Question; looks like the resolution was "baseline-accept" (Phase 2 completed per STATE.md) but not documented.
   - Recommendation: Planner MUST escalate to user at the start of Phase 3 planning. Propose:
     - (A) Baseline-accept these 4 specific failures, diff against `before.txt` in Wave 0.
     - (B) Include a pre-Phase-3 task to fix them (simple; ~20 LOC total).
   - (B) is recommended because the fixes are trivial and leaving red baseline muddies verification for remaining phases (4, 5, 6).

2. **Hoist eval kernel to `tensor_ops.rs` or duplicate inside `tensor_macro.rs`?**
   - What we know: `tensor_eval.rs::eval_populate_seeds_mem_optimized` (private method) and `auth_tensor_eval.rs::eval_populate_seeds_mem_optimized` (private method) are byte-for-byte identical. A third copy in `tensor_macro.rs` is the minimum-diff path per CONTEXT.md's strict interpretation of D-03 (which only mentions generalising the GEN kernel).
   - What's unclear: Is implicitly extending D-03 symmetry ("generalise eval kernel too") in scope?
   - Recommendation: Hoist into `tensor_ops.rs` as `eval_populate_seeds_mem_optimized` and `eval_unary_outer_product` `pub(crate)` free functions. Update `tensor_eval.rs` and `auth_tensor_eval.rs` to call them. Phase 3 hoist-and-reuse serves Phase 4 too. Low risk — existing test coverage via `test_semihonest_tensor_product` and `test_auth_tensor_product` will catch any hoist-induced regression.

3. **How to expose `BlockMatrix`'s internal slice for the `&[Block]` parameter?**
   - What we know: D-04 says "update the one call site in `tensor_gen.rs` to pass a `&[Block]` slice (convert from `MatrixViewRef` via `.as_slice()` or equivalent)." Currently `BlockMatrix.elements: Vec<T>` is private. `MatrixViewRef` has no `.as_slice()` method.
   - Options:
     - Add `pub(crate) fn elements_slice(&self) -> &[T]` on `TypedMatrix<T>`.
     - Add a matrix view `fn as_slice_column(&self) -> &[T]` with `debug_assert!(cols == 1)`.
     - Drop the generalisation; instead, make `gen_populate_seeds_mem_optimized` accept `impl Index<usize, Output = Block> + ?Sized` (trait-generic). Adds complexity.
   - Recommendation: Add `pub(crate) fn elements_slice(&self) -> &[T]` on `TypedMatrix<T>` (Pattern 2 Option A). Simplest, matches existing `Key::as_blocks` pattern (slice reinterpret), doesn't add any new trait bounds.

4. **Should the hoisted `eval_populate_seeds_mem_optimized` return `(Vec<Block>, usize)` (including `missing`) or drop the unused `_clear_value` parameter entirely?**
   - What we know: Existing signature `fn eval_populate_seeds_mem_optimized(x: &MatrixViewRef<Block>, levels: Vec<(Block, Block)>, _clear_value: &usize, cipher: &FixedKeyAes) -> Vec<Block>` (tensor_eval.rs:61-66). The `_clear_value` is computed but unused. Internal `missing` is constructed from LSB traversal of x.
   - What's unclear: Does the caller need `missing` after calling the kernel? Yes — `eval_unary_outer_product` takes `missing` as a parameter. So it must be returned somehow.
   - Recommendation: Return `(Vec<Block>, usize)` — add the `missing` usize to the return tuple. Drop the `_clear_value` param. This is a cleaner hoisted API than the current one, which has a dead input and forces the caller to compute `missing` separately.

5. **Is D-11 a typo — is T a length-m column vector (m×1), not an n×m matrix?**
   - What we know: Paper Construction 1 step 4 says "Interpret T^gb = T_0^gb T_1^gb ⋯ T_{m-1}^gb" — unambiguous length-m. Existing `gen_unary_outer_product` treats `y` as length-m via `let m = y.len()`. Z output IS n×m.
   - What's unclear: Does D-11 intend T to be m×1 (length-m column vector) and Z to be n×m?
   - Recommendation: Planner should treat D-11 as "T shape: m×1 column vector; Z shape: n×m". Escalate to user for confirmation; low-risk because the semantics are forced by the paper and the existing kernel.

6. **Phase 3 plan count: does CONTEXT.md imply a specific number of plans, or is 3 plans (Wave 0 + Garbler + Evaluator + Test) the right granularity?**
   - What we know: ROADMAP.md says "Plans: 3 plans" for Phase 3 but lists stale Phase 1 plan filenames (`01-PLAN-keys-sharing.md` etc. — clearly copy-paste from Phase 1). No authoritative plan-count-per-phase.
   - Recommendation: Use the 4-plan shape recommended in Summary (Wave 0 prerequisite + Plan A garbler + Plan B evaluator + Plan C invariant test) or collapse to 3 by merging the evaluator test into Plan B. Let the planner decide.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `rustc` | Build | ✓ | 1.90.0 | — |
| `cargo` | Build / test | ✓ | 1.90.0 | — |
| Rust edition 2024 support | All crate compilation | ✓ | `rustc 1.90.0` supports it natively | — |
| `aes` 0.9.0-pre.3 crate | `FixedKeyAes::tccr` | ✓ (via Cargo deps) | matches Cargo.toml | — |
| `once_cell::Lazy` | `FIXED_KEY_AES` singleton | ✓ (via deps) | 1.21.3 | — |
| `rand_chacha::ChaCha12Rng` | Deterministic tests | ✓ (via deps) | 0.9 | — |
| `cargo test --lib` baseline green | Regression-free verification | ⚠ (4 failing tests unrelated; see Pitfall 1) | — | Baseline-accept or fix pre-existing — see Open Question Q1 |
| `cargo build --lib` baseline green | Compilation verification | ✓ (8 warnings, no errors) | — | — |
| `cargo bench --no-run` baseline | Benchmark compile | ✓ | — | — |
| mpz-dev reference | Read-only study | ✓ | vendored at `references/mpz-dev/` | — (not a Cargo dependency) |

**Missing dependencies with no fallback:** None (hard blockers).

**Missing dependencies with fallback:** `cargo test --lib` baseline has 4 pre-existing failures; fallback is "baseline-accept" via snapshot diff (Open Question Q1).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness |
| Config file | None (standard `cargo test`) |
| Quick run command | `cargo test --lib tensor_macro::` (scopes to new module) |
| Full suite command | `cargo test --lib && cargo bench --no-run` |
| Estimated runtime | Phase 3 tests: <1s total (small `n`, small `m`); full suite ~5s; `cargo bench --no-run` ~10s |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| PROTO-01 | `tensor_garbler(n, m, Δ_A, a_keys, t_gen) -> (Z_gen, G)` compiles, returns correct dimensions, emits n-1 level_cts and m leaf_cts | unit | `cargo test --lib tensor_macro::tests::test_garbler_dimensions_and_g_structure` | ❌ — Wave 0 creates test file |
| PROTO-02 | `tensor_evaluator(n, m, G, a_macs, t_eval) -> Z_eval` compiles, returns n×m BlockMatrix | unit | `cargo test --lib tensor_macro::tests::test_evaluator_dimensions` | ❌ — Wave 0 |
| PROTO-03 / TEST-01 | `Z_gen XOR Z_eval == a ⊗ T` paper invariant holds across (n, m, T) test vectors including edge cases n=1, small m, large m/n | unit (inline battery) | `cargo test --lib tensor_macro::tests::` | ❌ — Wave 0 |
| Regression: `test_semihonest_tensor_product` | Existing end-to-end test still passes after `gen_populate_seeds_mem_optimized` signature change | integration | `cargo test --lib tests::test_semihonest_tensor_product` | ✅ (lib.rs line 132) |
| Regression: `test_auth_tensor_product` | Existing end-to-end test still passes | integration | `cargo test --lib tests::test_auth_tensor_product` | ✅ (lib.rs line 251) |
| Baseline-diff: no NEW failures introduced | Phase 2 research's `before.txt` pattern — capture `cargo test --lib 2>&1 | grep FAILED | sort > before.txt` before any Phase 3 code, diff on each commit | diff | `diff before.txt after.txt` = empty | Wave 0 creates `before.txt` snapshot |

### Sampling Rate

- **Per task commit:** `cargo test --lib tensor_macro::` (~1s) — fast module-scoped check.
- **Per wave merge:** `cargo test --lib` (~5s) — full suite to catch regressions in semi-honest / authenticated paths from the `gen_populate_seeds_mem_optimized` signature change.
- **Phase gate:** `cargo test --lib && cargo bench --no-run` plus `diff before.txt after.txt` (must show no new FAILED lines) before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] **Baseline snapshot:** `cargo test --lib 2>&1 | grep 'FAILED' | sort > .planning/phases/03-m2-generalized-tensor-macro-construction-1/before.txt`
- [ ] **Module skeleton:** `src/tensor_macro.rs` with empty `TensorMacroCiphertexts` struct, `#[cfg(test)] mod tests { ... }`, and stub `tensor_garbler` / `tensor_evaluator` functions with `unimplemented!()` bodies. Add `pub mod tensor_macro;` to `src/lib.rs`. Ensures `cargo build` still compiles immediately.
- [ ] **Eval kernel hoist prereq** (if Open Question Q2 resolved in favour of hoisting): hoist `eval_populate_seeds_mem_optimized` and `eval_unary_outer_product` from `tensor_eval.rs` / `auth_tensor_eval.rs` into `tensor_ops.rs`; rewire the two existing call sites. Runs before Plan B (evaluator) writes new code.
- [ ] **`gen_populate_seeds_mem_optimized` signature change** (D-03/D-04): change first param to `&[Block]`, update single call site in `tensor_gen.rs`. Prereq for Plan A (garbler). Requires `BlockMatrix::elements_slice()` or equivalent (Open Question Q3).
- [ ] Framework install: none — `rustc` / `cargo` already verified.

---

## Security Domain

Phase 3 is a new primitive that will be composed into Phase 4's Pi_LeakyTensor. The primitive itself has no direct security footprint (it's a combinatorial building block), but MUST preserve the cryptographic invariants that Phase 4 relies on.

### Applicable ASVS Categories

`workflow.security_enforcement` key is absent in `.planning/config.json` — treated as enabled.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No user-facing auth in this protocol primitive |
| V3 Session Management | no | No sessions; single-shot pure function |
| V4 Access Control | no | Library crate; no access control |
| V5 Input Validation | yes | `assert!` preconditions on `a_keys.len() == n`, `t_gen.rows() == m`, `t_gen.cols() == 1`, `g.level_cts.len() == n - 1`, `g.leaf_cts.len() == m` — match codebase convention |
| V6 Cryptography | yes | `FixedKeyAes::tccr` PRF (existing, Guo et al. 2019/074); do NOT hand-roll. `Key::new()` / `Delta::new()` invariant enforcement (existing, Phase 1) |

### Cryptographic Invariants Preserved (MUST)

| Invariant | Role in Phase 3 |
|-----------|-----------------|
| `Key.lsb() == 0` (Phase 1 D-01) | `a_keys: &[Key]` input preserves invariant because it's the `Key` type. Inside the GGM kernel, the code path `if x[n-i-1].lsb()` expects this invariant — violating it silently corrupts tree construction. |
| `Delta.lsb() == 1` (existing) | The LSB of evaluator's `a_macs[i]` encodes the secret bit `a_i`. If Delta LSB were 0, `a_macs[i].lsb() == a_i XOR 0 == A_i.lsb() == 0` always, breaking the "MAC carries the bit" IT-MAC convention. |
| MAC invariant `mac = key.auth(bit, delta)` | Enforced by `IdealBCot` test oracle and by Phase 4 correlated-OT consumption. Phase 3 assumes this holds on `a_keys` / `a_macs` inputs. |
| TCCR (Tweakable Circular Correlation-Robust) security | Underlying PRF property of `FixedKeyAes::tccr`. GGM tree expansion relies on this. |
| Endianness: paper A_0 ⟷ code `x[n-1]` | Consistent across `tensor_ops.rs`, `tensor_eval.rs`, `auth_tensor_eval.rs` — do NOT deviate in Phase 3. |

### Known Threat Patterns for this Domain

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tweak collision in TCCR (same `(tweak, seed)` pair reused for two different purposes) | Tampering (via distinguisher) | Reuse the existing tweak conventions in `tensor_ops.rs`; do not add new tweak values without verifying uniqueness across call chain. Current scheme uses tweak 0/1 with a seed that changes per level and per leaf index. |
| Silent Key LSB violation | Tampering | `Key::new()` enforces at construction; Phase 3 doesn't construct new Keys — all `a_keys` come from callers. If a caller passes `Key::from(block)` with uncleared LSB, bug manifests in Phase 4 tests, not Phase 3. Document in macro's preconditions. |
| Mismatched n between gen and eval | Denial of Service (panic) / Correctness | Both functions take `n` as explicit parameter; `assert_eq!(a_keys.len(), n)` and `assert_eq!(a_macs.len(), n)` prevent mismatches. |
| Ciphertext malleability in wire transit (Phase 4 concern) | Tampering | NOT Phase 3's concern — Phase 4's F_eq step detects malformed ciphertexts. Phase 3's `TensorMacroCiphertexts` is a plain data struct. |

---

## Project Constraints (from CLAUDE.md)

No root-level `CLAUDE.md` file exists in this repository. `[VERIFIED: Read tool returned "File does not exist"]` Therefore no project-specific AI directives beyond what's in `.planning/codebase/CONVENTIONS.md` (which is treated as authoritative for code style).

Constraints derived from `.planning/codebase/CONVENTIONS.md` (codified during Phase 1/2) that Phase 3 MUST follow:

- **Newtype pattern for cryptographic primitives:** `Key(Block)`, `Mac(Block)` — do not expose `Block` at API boundaries where `Key` or `Mac` is semantically appropriate.
- **snake_case** file and function names.
- **PascalCase** type names.
- **Protocol-role-suffix pattern:** `*_gen.rs`, `*_eval.rs`, `*_pre.rs`, `*_fpre.rs`. Phase 3's new file `tensor_macro.rs` is NEUTRAL (not role-specific — it's a primitive used by both gen and eval paths). This is consistent with existing `tensor_ops.rs` naming.
- **`pub(crate)` for internal functions.**
- **`#[cfg(test)] mod tests { ... }` inline** — no separate `tests/` directory.
- **`assert!` / `assert_eq!` with explanatory messages**, not `Result`/`Error` in protocol logic.
- **Column-major indexing** for all n×m tensor structures.
- **Endianness convention:** index 0 = LSB, index n-1 = MSB.
- **`// Safety:` annotation** on every `unsafe` block.

Codebase-wide TODO comment convention (optional; documented as `// TODO:` in `.planning/codebase/CONVENTIONS.md`): Phase 3 code should be TODO-free. If a deferred concern arises (e.g., "real TCCR nonce audit for malicious security"), log it in `.planning/STATE.md` Deferred Items rather than as a source-code TODO.

---

## Sources

### Primary (HIGH confidence)

- `[VERIFIED]` `references/appendix_krrw_pre.tex` (lines 49-77) — Construction 1 verbatim specification: `tensorgb` and `tensorev` steps with correctness proof (line 76: "Z_ev = Z_gb ⊕ (a ⊗ T)").
- `[VERIFIED]` `src/tensor_ops.rs` (all 122 lines) — existing `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product`, which structurally implement Construction 1's garbler steps.
- `[VERIFIED]` `src/tensor_eval.rs` (all 274 lines) — existing private methods `eval_populate_seeds_mem_optimized` (lines 61-130) and `eval_unary_outer_product` (lines 132-172), which structurally implement Construction 1's evaluator steps.
- `[VERIFIED]` `src/tensor_gen.rs` (all 166 lines) — single call site for `gen_populate_seeds_mem_optimized` that will need the `&[Block]` update (line 82).
- `[VERIFIED]` `src/keys.rs` (lines 77-82) — `Key::as_blocks` zero-cost slice reinterpret.
- `[VERIFIED]` `src/macs.rs` (lines 55-60) — `Mac::as_blocks` zero-cost slice reinterpret.
- `[VERIFIED]` `src/bcot.rs` (lines 63-103) — `IdealBCot::transfer_a_to_b` / `transfer_b_to_a`; provides consistent `(sender_keys: Vec<Key>, receiver_macs: Vec<Mac>, choices: Vec<bool>)` for test oracle.
- `[VERIFIED]` `src/aes.rs` (lines 36-68) — `FIXED_KEY_AES` singleton and `FixedKeyAes::tccr`.
- `[VERIFIED]` `src/matrix.rs` (all 730 lines) — `BlockMatrix` with column-major indexing.
- `[VERIFIED]` `src/lib.rs` (lines 1-24 module declarations, lines 132-378 existing end-to-end tests).
- `[VERIFIED]` `.planning/phases/03-m2-generalized-tensor-macro-construction-1/03-CONTEXT.md` — locked decisions D-01 through D-14.
- `[VERIFIED]` `.planning/REQUIREMENTS.md` — PROTO-01, PROTO-02, PROTO-03, TEST-01 definitions.
- `[VERIFIED]` `.planning/ROADMAP.md` — Phase 3 goal and success criteria (noting stale plan filenames, which Phase 3 plans will replace).
- `[VERIFIED]` `.planning/STATE.md` — confirms current phase is 3, Phase 2 complete.
- `[VERIFIED]` `.planning/codebase/CONVENTIONS.md` — cryptographic invariants, naming conventions, error handling.
- `[VERIFIED]` `.planning/codebase/TESTING.md` — test framework, inline `#[cfg(test)] mod tests` pattern, `verify_cross_party` helper.
- `[VERIFIED]` `.planning/codebase/STRUCTURE.md` — crate layout, module graph, "where to add new code" conventions.
- `[VERIFIED]` `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/02-RESEARCH.md` — Phase 2's pre-existing-test-failures analysis (carried forward).
- `[VERIFIED]` Local `cargo test --lib` run (2026-04-21) reproducing the 4 baseline failures: `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `auth_tensor_pre::tests::test_combine_mac_invariants`, `preprocessing::tests::test_run_preprocessing_mac_invariants`. "test result: FAILED. 48 passed; 4 failed".
- `[VERIFIED]` Local `cargo build --lib` run — baseline builds successfully with 8 warnings, 0 errors.
- `[VERIFIED]` Toolchain: `rustc 1.90.0 (1159e78c4 2025-09-14)`, `cargo 1.90.0 (840b83a10 2025-07-30)`.

### Secondary (MEDIUM confidence)

- `[CITED: references/mpz-dev/crates/core/src/ggm.rs]` mpz-dev's `GgmTree::new_from_seed` and `new_partial` — independent implementation of GGM tree gen/eval pair. Confirms the shape of the primitive but uses TwoKeyPrp ([Block::ZERO, Block::ONE] key pair) instead of TCCR with tweak 0/1. Both approaches are secure under the appropriate assumption; this codebase's TCCR matches Guo et al. 2019/074.
- `[CITED: src/aes.rs:56-58]` TCCR construction `π(π(x) ⊕ i) ⊕ π(x)` referencing https://eprint.iacr.org/2019/074 (Guo et al., Section 7.4).

### Tertiary (LOW confidence)

- `[ASSUMED]` A1: D-11's "n×m BlockMatrix" applies to Z only; T is m×1. Based on paper reading (Construction 1 Step 4 explicitly says "T = T_0 ⋯ T_{m-1}" — length m) and existing kernel signature. Would need user confirmation to proceed with high confidence.

---

## Metadata

**Confidence breakdown:**

- **Standard stack:** HIGH — all dependencies already in `Cargo.toml`; no new external tools.
- **Architecture / module split:** HIGH — new module follows established flat-src convention; composition with existing kernels is direct.
- **Paper-spec correspondence:** HIGH — paper Construction 1 is 23 lines of LaTeX and maps directly onto existing kernel outputs.
- **Kernel reusability:** HIGH — existing `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` are byte-for-byte what the paper calls for, modulo the `&[Block]` parameter generalisation.
- **Evaluator kernel hoist:** HIGH — confirmed duplication between `tensor_eval.rs` and `auth_tensor_eval.rs`; hoisting eliminates this.
- **Test oracle (bCOT):** HIGH — existing `IdealBCot` produces exactly the `(a_keys, a_macs)` shape the macro expects.
- **Endianness mapping (paper ⟷ code):** HIGH — code uses `x[n-1]` as level-0 consistently across all callers and `test_semihonest_tensor_product` passes today.
- **D-11 T-shape reading (T = m×1 column vector, not n×m):** MEDIUM — based on paper text; needs user sign-off.
- **Baseline test failure status:** HIGH — reproduced locally; matches Phase 2 research exactly.
- **Plan structure (3 vs 4 plans):** MEDIUM — ROADMAP copy-paste is stale; actual plan count at implementer discretion.

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — Rust ecosystem is stable; paper spec is frozen; the 4 pre-existing test failures may be repaired by outside work, invalidating Pitfall 1 in that direction only).
