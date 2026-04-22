# Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) - Research

**Researched:** 2026-04-21
**Domain:** Two-party authenticated correlated-OT-based preprocessing protocol; in-process ideal F_eq; Rust 2024 cryptographic implementation.
**Confidence:** HIGH (every claim is verified against the paper appendix, the existing codebase, or both — see Sources)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**generate() API (PROTO-04, PROTO-09):**
- **D-01:** Signature is `generate(&mut self) -> LeakyTriple` — no `x_clear`/`y_clear` arguments. x and y bits are sampled uniformly at random internally using the `LeakyTensorPre`'s own `ChaCha12Rng`. Preprocessing must be fully input-independent; the old signature taking concrete input values violated this invariant.
- **D-02:** `LeakyTensorPre` struct itself is unchanged in shape (`n`, `m`, `bcot: &'a mut IdealBCot`, `rng: ChaCha12Rng`) and construction (`LeakyTensorPre::new(seed, n, m, bcot)`). Only `generate` is rewritten.

**F_eq Module (PROTO-08, TEST-04):**
- **D-03:** `F_eq` lives in a new `src/feq.rs` module, matching the `IdealBCot` pattern. Add `pub mod feq;` to `src/lib.rs`. The module exposes a single public function (or struct with a check method) for the ideal equality check.
- **D-04:** On L_1 ≠ L_2, F_eq calls `panic!("F_eq abort: consistency check failed — L_1 != L_2")`. Abort is unconditional and immediate, matching the ideal functionality semantics. Tests for TEST-04 (verifying abort on malformed inputs) use `#[should_panic]`.
- **D-05:** Correct inputs (L_1 == L_2 element-wise) return normally (no return value needed beyond unit). F_eq takes `l1: &BlockMatrix` and `l2: &BlockMatrix` and does element-wise Block comparison.

**LeakyTriple Struct Cleanup (PROTO-09):**
- **D-06:** Rename fields to match paper notation throughout (gen_alpha → gen_x, gen_beta → gen_y, gen_correlated → gen_z, etc.).
- **D-07:** Remove `gen_gamma_shares`, `eval_gamma_shares`, `gen_alpha_labels`, `eval_alpha_labels`, `gen_beta_labels`, `eval_beta_labels` entirely. These do not appear in the paper's Pi_LeakyTensor output.
- **D-08:** Z is stored as `Vec<AuthBitShare>` in column-major order (index = `j*n+i`, matching existing convention). Length = `n*m`. Phase 5 combining works directly on this Vec without conversion.
- **D-09:** The `n`, `m`, `delta_a`, `delta_b` fields remain on `LeakyTriple`.

**C_A/C_B Computation (PROTO-05):**
- **D-10:** C_A and C_B are computed inline in `generate()` — no separate helper function. Each is a length-m `Vec<Block>` computed as Block-level XOR per entry:
  - `C_A[j] := y_A[j]·Δ_A ⊕ key(y_B@A)[j] ⊕ mac(y_A@B)[j]`
  - `C_B[j] := y_B[j]·Δ_B ⊕ mac(y_B@A)[j] ⊕ key(y_A@B)[j]`
  - where `y_A[j]·Δ_A` means `if y_A_bit { Δ_A.as_block() } else { Block::ZERO }`.
  - Analogously, `C_A^(R)` and `C_B^(R)` are computed the same way using R shares.

**R (Random Authenticated Tensor Mask) (PROTO-04):**
- **D-11:** `itmac{R}{Δ}` is obtained via n×m bCOT calls each way (`transfer_a_to_b` and `transfer_b_to_a`) — the same pattern as x and y shares. R bits are sampled uniformly at random internally. No new methods added to `IdealBCot`.
- **D-12:** R shares are assembled into `gen_r_shares` and `eval_r_shares` (local to `generate()`, not stored on `LeakyTriple`). They are used to compute `C_A^(R)`, `C_B^(R)`, and then `itmac{R}{Δ}` for the final output Z.

**Tensor Macro Calls (PROTO-06, PROTO-07):**
- **D-13:** Macro call 1: `tensor_garbler(n, m, Δ_A, keys_of_x_B@A, C_A)` → `(Z_gb1, G_1)` — A is garbler, B is evaluator. `tensor_evaluator(n, m, G_1, macs_of_x_B@A, C_B)` → `E_1`.
- **D-14:** Macro call 2: `tensor_garbler(n, m, Δ_B, keys_of_x_A@B, C_B)` → `(Z_gb2, G_2)` — B is garbler, A is evaluator. `tensor_evaluator(n, m, G_2, macs_of_x_A@B, C_A)` → `E_2`.
- **D-15:** `t_gen` and `t_eval` arguments to the macro must be `BlockMatrix` (m×1 column vectors). The `C_A`/`C_B` vecs are wrapped into a `BlockMatrix` before being passed to the macro.
- **D-16:** S_1 = Z_gb1 ⊕ E_2 ⊕ C_A^(R); S_2 = Z_gb2 ⊕ E_1 ⊕ C_B^(R). D = lsb(S_1) ⊕ lsb(S_2) (element-wise, producing an `n×m` bit matrix).

**Final Z Output (PROTO-07, PROTO-08):**
- **D-17:** After F_eq passes, `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}`. Since D is public, `itmac{D}{Δ}` is locally computable from D bits and Δ_A/Δ_B. The combining XORs key/mac/value fields of R shares with the D-derived shares element-wise.

### Claude's Discretion

- Exact loop structure for assembling `Vec<Key>` / `Vec<Mac>` from bCOT output before passing to `tensor_garbler`/`tensor_evaluator` — straightforward extraction from `BcotOutput.sender_keys` and `receiver_macs`.
- Whether `BlockMatrix::from_blocks(blocks: Vec<Block>)` or `BlockMatrix::new` + manual fill is used to wrap C_A/C_B vecs — match the pattern in `tensor_macro.rs` tests.
- Exact nonce/ordering of bCOT calls inside `generate()` (x first, then y, then R) — matches the paper's "obtain correlated randomness" step ordering.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within Phase 4 scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **PROTO-04** | Obtain correlated randomness from F_bCOT: `itmac{x_A}{Δ_B}`, `itmac{x_B}{Δ_A}`, `itmac{y_A}{Δ_B}`, `itmac{y_B}{Δ_A}`, `itmac{R}{Δ}` | Existing `IdealBCot::transfer_a_to_b` / `transfer_b_to_a` produce these shares; the existing `leaky_tensor_pre.rs` already uses this pattern (5 places — alpha/beta/correlated/gamma) which we adapt to x/y/R per D-11. See **Architecture Patterns → Pattern 1**. |
| **PROTO-05** | Compute C_A and C_B (XOR combinations of y and R correlations under Δ_A ⊕ Δ_B) | Block-level XOR via `^` operator on `Block` is in place (`src/block.rs:303-352`). `Delta::as_block()` and conditional `Block::ZERO` selection are stable APIs. See **Architecture Patterns → Pattern 2** for the exact 4-term XOR per entry. |
| **PROTO-06** | Execute two tensor macro calls (A as garbler with Δ_A, B as garbler with Δ_B) and XOR results | Phase 3 delivered `tensor_garbler(n, m, delta, &[Key], &BlockMatrix) -> (BlockMatrix, TensorMacroCiphertexts)` and `tensor_evaluator(n, m, &TensorMacroCiphertexts, &[Mac], &BlockMatrix) -> BlockMatrix` — see `src/tensor_macro.rs:82-181`. They are `pub(crate)`, callable from `leaky_tensor_pre.rs`. |
| **PROTO-07** | Masked tensor reveal: compute lsb(S1) ⊕ lsb(S2) = D (revealed publicly), then compute `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}` | `Block::lsb()` returns `bool` (`src/block.rs:96-99`). For each `(i,k)` matrix entry we extract one bit then compute the local `itmac{D}{Δ}` from the public bit. **Pitfall 1** below explains the critical `lsb(Δ_A ⊕ Δ_B) == 1` precondition that does NOT currently hold in the codebase. |
| **PROTO-08** | F_eq consistency check: parties compute L_1 = S_1 ⊕ D·Δ_A and L_2 = S_2 ⊕ D·Δ_B, ideal F_eq checks equality; abort if check fails | New `src/feq.rs` module per D-03; element-wise `BlockMatrix` equality with `panic!` on mismatch per D-04/D-05. **Architecture Patterns → Pattern 3**. |
| **PROTO-09** | LeakyTriple output is `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` only — remove gamma bits and wire labels from the struct | D-06/D-07 specify field renames and removals. `Vec<AuthBitShare>` shape preserved per D-08. |
| **TEST-02** | Leaky triple IT-MAC invariant: `mac = key XOR bit · delta` under verifier's delta for each share in the triple | Test pattern: re-use `verify_cross_party` helper from `src/leaky_tensor_pre.rs:275-287` (also exists in `src/auth_tensor_pre.rs:134-152`). Cross-party verification is mandatory; direct `share.verify(delta)` panics for cross-party shares. See **Common Pitfalls → Pitfall 4**. |
| **TEST-03** | Leaky triple product invariant: `Z_full = x_full ⊗ y_full` (XOR of gen+eval Z shares = tensor product of XOR of gen+eval x and y shares) | Per-entry test loop: for each `(i,j)` reconstruct `x_i = gen_x_shares[i].value ^ eval_x_shares[i].value`, `y_j = ...`, `z_ij = gen_z_shares[j*n+i].value ^ eval_z_shares[j*n+i].value`, assert `z_ij == x_i & y_j`. The paper proves this holds when F_eq passes (Theorem 1 — see Sources). |
| **TEST-04** | F_eq: correct L values pass; malformed L values cause abort | Two tests: (a) feed equal `BlockMatrix`es → no panic; (b) feed differing `BlockMatrix`es with `#[should_panic(expected = "F_eq abort")]` per D-04. |
</phase_requirements>

---

## Summary

Phase 4 rewrites `src/leaky_tensor_pre.rs` to implement Construction 2 of the paper (Pi_LeakyTensor, lines 198-254 of `references/appendix_krrw_pre.tex`) and adds a new `src/feq.rs` module for the in-process ideal F_eq functionality. The protocol consumes correlated randomness from `IdealBCot` for `x_A, x_B, y_A, y_B, R` (each a separate batch of `transfer_a_to_b` + `transfer_b_to_a` calls), computes `C_A`/`C_B` and their R-twins, performs two `tensor_macro` calls with parties swapping garbler roles, executes a masked reveal `D = lsb(S_1) ⊕ lsb(S_2)` (whose correctness depends on `lsb(Δ_A ⊕ Δ_B) == 1`), runs F_eq for consistency, and outputs a leaky triple of three `Vec<AuthBitShare>` plus the deltas.

The phase is **purely a rewrite** of `generate()` plus a new module — no new external dependencies, no schema changes outside the planning area, no migrations. Phase 3 already shipped the working `tensor_macro` primitive (`src/tensor_macro.rs`, 10 passing tests) and the ideal `IdealBCot` is stable (`src/bcot.rs`). All 4 currently failing tests in `cargo test --lib` are baseline failures in the very file being rewritten (`leaky_tensor_pre.rs`, `auth_tensor_pre.rs`, `preprocessing.rs`) that the rewrite is expected to eliminate by design — see **Validation Architecture** below.

**Primary recommendation:** Implement `generate()` as a strict 5-step transcript matching the paper appendix's Construction 2 line-by-line (correlated randomness → C_A/C_B → two macro calls → masked reveal → F_eq + Z assembly). Wave 0 must include a fix to `Delta::random` or to one of the two delta constructors so that `lsb(Δ_A ⊕ Δ_B) == 1` (currently both deltas have lsb=1 making the XOR lsb 0, which silently breaks the masked-reveal D extraction). Without this fix, all PROTO-07/TEST-03 tests will fail in subtle ways that look like protocol bugs.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Correlated randomness production (5 batches: x_A, x_B, y_A, y_B, R) | bCOT layer (`src/bcot.rs`) | `LeakyTensorPre::generate` | `IdealBCot` is the dedicated ideal F_bCOT functionality; `generate` only orchestrates calls |
| C_A / C_B / C_A^(R) / C_B^(R) assembly | `LeakyTensorPre::generate` (inline) | — | Per D-10: inline computation, no helper function. Trivial 4-term Block XOR per entry. |
| Generalized tensor macro evaluation | `tensor_macro` module (`src/tensor_macro.rs`) | — | Phase 3 primitive, `pub(crate)`. `LeakyTensorPre` is a pure consumer. |
| Masked reveal & D extraction | `LeakyTensorPre::generate` (inline) | — | Element-wise `lsb(Block)` accumulation into an `n×m` bit matrix; no need for a helper. |
| F_eq consistency check | `feq` module (NEW: `src/feq.rs`) | — | Per D-03: separate module mirrors the `IdealBCot` pattern, isolating the ideal functionality so it can be replaced by a real implementation in v2. |
| Leaky triple final assembly (XOR of R-shares with D-derived shares) | `LeakyTensorPre::generate` (inline) | — | Local to `generate`; element-wise `AuthBitShare` arithmetic via existing `Add` impls (`src/sharing.rs:66-117`). |
| Cross-party MAC verification (test only) | `verify_cross_party` test helper | — | Already exists in `src/leaky_tensor_pre.rs:275-287`; reuse verbatim. |
| `LeakyTriple` data carrier | `src/leaky_tensor_pre.rs` (struct definition) | — | Plain struct; no behavior. Removed fields per D-06/D-07 simplify the surface. |

---

## Standard Stack

### Core (in-tree, no new external dependencies)

| Component | Source File | Purpose | Why Standard |
|-----------|-------------|---------|--------------|
| `IdealBCot` | `src/bcot.rs` | Ideal F_bCOT — produces matched (sender_keys, receiver_macs) per choice bits | Already locked as the F_bCOT instantiation for M1+M2 (CONTEXT.md upstream + STATE.md decision log) |
| `tensor_garbler` / `tensor_evaluator` | `src/tensor_macro.rs` | Generalized tensor macro (Construction 1) — `pub(crate)` | Phase 3 deliverable; 10/10 tests passing per `03-VERIFICATION.md` |
| `BlockMatrix` (alias of `TypedMatrix<Block>`) | `src/matrix.rs` | Column-major n×m Block storage; supports `as_view()`, `as_view_mut()`, indexing `[(i,j)]` and `[k]` for column vectors | Established in Phase 1 (`CLEAN-05`); used by `tensor_macro` for T shares and Z outputs |
| `AuthBitShare` | `src/sharing.rs` | One party's view of an authenticated bit (`key`, `mac`, `value`); has `Add` impl for XOR | Used throughout the codebase as the canonical IT-MAC share carrier |
| `Key`, `Mac` | `src/keys.rs`, `src/macs.rs` | Newtypes of `Block` with LSB invariants; `Key::as_blocks` / `Mac::as_blocks` for zero-cost slice reinterpret to `&[Block]` | Phase 1 invariants; `tensor_garbler`/`tensor_evaluator` consume `&[Key]` and `&[Mac]` directly |
| `Delta` | `src/delta.rs` | Newtype of `Block` with `lsb=1` invariant set in `Delta::new`; `as_block()` returns `&Block` | Same convention used everywhere — but see **Pitfall 1** for the cross-party XOR caveat |
| `ChaCha12Rng` | `rand_chacha 0.9` (already in `Cargo.toml:14`) | Deterministic seedable RNG for x/y/R bit sampling | Already used in `LeakyTensorPre::new` via `ChaCha12Rng::seed_from_u64(seed)` (`src/leaky_tensor_pre.rs:56`) |

### Supporting

| Component | Source | Purpose | When to Use |
|-----------|--------|---------|-------------|
| `Block::lsb()` / `set_lsb()` | `src/block.rs:85-99` | Get/set LSB of a 128-bit block | Extract `D = lsb(S_1) ⊕ lsb(S_2)` (PROTO-07) |
| `Block::ZERO` constant | `src/block.rs:23` | All-zero 128-bit block | The "off" branch when computing `bit·Δ` (e.g., `if y_A_bit { Δ.as_block() } else { Block::ZERO }`) |
| `BlockMatrix::elements_slice()` | `src/matrix.rs:83-85` (Phase 3) | Returns `&[Block]` view of column-major storage | Wrap C_A/C_B vec as `BlockMatrix(m, 1)` then pass to tensor_macro via `as_view()` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff | Why Rejected |
|------------|-----------|----------|--------------|
| In-process `feq` panic on mismatch | Networked F_eq with simulated abort | Closer to real protocol semantics | v2 deferred per STATE.md; in-process keeps preprocessing testable without network harness. |
| `Vec<Block>` for C_A/C_B | `BlockMatrix(m, 1)` wrapping at the macro boundary | Saves a wrap step | Per D-15: `tensor_garbler`/`tensor_evaluator` already require `&BlockMatrix` (m×1). Wrapping is `~1 line`. Saving the wrap would force changing the Phase 3 macro signature. |
| Helper function for C_A/C_B computation | Inline 4-term XOR loop | Slight readability win | Per D-10: user explicitly chose inline. Only 4 terms × m entries; helper would obscure the paper-line correspondence. |
| Add `transfer_*_n_m` methods on `IdealBCot` for the R batch | Reuse existing `transfer_a_to_b` with `n*m` length choices | Symmetry with x/y batches | Per D-11: explicit decision to NOT add new methods; the n×m length is just a `Vec<bool>` of length `n*m` like any other batch. |

**Installation (none — all dependencies present):**

```toml
# Already in Cargo.toml — no edits required
rand = "0.9"
rand_chacha = "0.9"
```

**Version verification:**

```bash
cargo tree | grep -E "rand|rand_chacha"
```

[VERIFIED: cargo --version → 1.90.0 (840b83a10 2025-07-30); rustc 1.90.0 (1159e78c4 2025-09-14); Cargo.toml line 13-14 confirms `rand = "0.9"`, `rand_chacha = "0.9"`]

---

## Architecture Patterns

### System Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                       LeakyTensorPre::generate(&mut self) -> LeakyTriple  │
│                                                                            │
│   ┌──────────────────────────── STEP 1: Correlated Randomness ────────┐   │
│   │                                                                    │   │
│   │   (RNG samples uniformly random bit vectors locally)              │   │
│   │   x_A ∈ {0,1}^n,  x_B ∈ {0,1}^n,                                  │   │
│   │   y_A ∈ {0,1}^m,  y_B ∈ {0,1}^m,                                  │   │
│   │   R_A ∈ {0,1}^(n*m), R_B ∈ {0,1}^(n*m)        (column-major)     │   │
│   │                                                                    │   │
│   │   FIVE bCOT batch pairs (10 calls total to &mut bcot):            │   │
│   │     bcot.transfer_a_to_b(&x_B_choices) → A's keys, B's MACs       │   │
│   │     bcot.transfer_b_to_a(&x_A_choices) → B's keys, A's MACs       │   │
│   │     [same shape for y, R]                                          │   │
│   │                                                                    │   │
│   │   Yields: itmac{x_A}{Δ_B}, itmac{x_B}{Δ_A},                       │   │
│   │           itmac{y_A}{Δ_B}, itmac{y_B}{Δ_A},                       │   │
│   │           itmac{R}{Δ}                                              │   │
│   └────────────────────────────────────────────────────────────────────┘   │
│                                  │                                          │
│                                  ▼                                          │
│   ┌──────────────────────────── STEP 2: C_A, C_B, C_A^(R), C_B^(R) ───┐   │
│   │                                                                    │   │
│   │   For each j ∈ [m]:                                                │   │
│   │     C_A[j] := y_A[j]·Δ_A ⊕ key(y_B@A)[j] ⊕ mac(y_A@B)[j]          │   │
│   │     C_B[j] := y_B[j]·Δ_B ⊕ mac(y_B@A)[j] ⊕ key(y_A@B)[j]          │   │
│   │   For each k ∈ [n*m]:                                              │   │
│   │     C_A^(R)[k] := R_A[k]·Δ_A ⊕ key(R_B@A)[k] ⊕ mac(R_A@B)[k]      │   │
│   │     C_B^(R)[k] := R_B[k]·Δ_B ⊕ mac(R_B@A)[k] ⊕ key(R_A@B)[k]      │   │
│   │                                                                    │   │
│   │   NOTE: C_A^(R)/C_B^(R) are length n*m (one per output entry),     │   │
│   │   while C_A/C_B are length m (one per "T column" in the macro).    │   │
│   └────────────────────────────────────────────────────────────────────┘   │
│                                  │                                          │
│                                  ▼                                          │
│   ┌──────────────── STEP 3: Two Tensor Macro Calls ──────────────────────┐ │
│   │                                                                       │ │
│   │   wrap C_A → BlockMatrix(m, 1); wrap C_B → BlockMatrix(m, 1)         │ │
│   │                                                                       │ │
│   │   Macro Call 1 (A is garbler):                                        │ │
│   │     keys_x_B@A := bcot output sender_keys from transfer_a_to_b       │ │
│   │                   on x_B's choice bits  (Vec<Key>, len n)            │ │
│   │     macs_x_B@A := bcot output receiver_macs (Vec<Mac>, len n)        │ │
│   │     (Z_gb1, G_1) := tensor_garbler(n, m, Δ_A, &keys_x_B@A, &C_A)    │ │
│   │     E_1          := tensor_evaluator(n, m, &G_1, &macs_x_B@A, &C_B) │ │
│   │                                                                       │ │
│   │   Macro Call 2 (B is garbler):                                        │ │
│   │     keys_x_A@B / macs_x_A@B from the second bCOT direction           │ │
│   │     (Z_gb2, G_2) := tensor_garbler(n, m, Δ_B, &keys_x_A@B, &C_B)    │ │
│   │     E_2          := tensor_evaluator(n, m, &G_2, &macs_x_A@B, &C_A) │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                  │                                          │
│                                  ▼                                          │
│   ┌─────────────────── STEP 4: Masked Reveal ────────────────────────────┐ │
│   │                                                                       │ │
│   │   Wrap C_A^(R) → BlockMatrix(n, m); wrap C_B^(R) → BlockMatrix(n, m)│ │
│   │                                                                       │ │
│   │   S_1 := Z_gb1 ⊕ E_2 ⊕ C_A^(R)     (BlockMatrix n×m)                │ │
│   │   S_2 := Z_gb2 ⊕ E_1 ⊕ C_B^(R)     (BlockMatrix n×m)                │ │
│   │                                                                       │ │
│   │   For each (i, k):                                                    │ │
│   │     D[(i,k)] := S_1[(i,k)].lsb() ^ S_2[(i,k)].lsb()                  │ │
│   │   Result: D ∈ {0,1}^(n×m)  (Vec<bool> column-major)                  │ │
│   │                                                                       │ │
│   │   ⚠ CRITICAL: requires lsb(Δ_A ⊕ Δ_B) == 1 — see Pitfall 1.         │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                  │                                          │
│                                  ▼                                          │
│   ┌─────────────────── STEP 5: F_eq + Z assembly ─────────────────────────┐│
│   │                                                                        ││
│   │   For each (i, k):                                                     ││
│   │     L_1[(i,k)] := S_1[(i,k)] ⊕ (D[(i,k)] ? Δ_A.as_block() : ZERO)    ││
│   │     L_2[(i,k)] := S_2[(i,k)] ⊕ (D[(i,k)] ? Δ_B.as_block() : ZERO)    ││
│   │                                                                        ││
│   │   feq::check(&L_1, &L_2)   ← panics if any entry differs              ││
│   │                                                                        ││
│   │   For each k ∈ [n*m]:                                                  ││
│   │     // itmac{D}{Δ}: D is public, so each party can locally form        ││
│   │     // a "trivial" share. The simplest: A holds the bit value, B's    ││
│   │     // key for the bit is Block::ZERO (since A's "MAC" is trivially   ││
│   │     // 0 ⊕ D·Δ_B = D·Δ_B). Same symmetric story for B.                ││
│   │     gen_z_shares[k]  := gen_R_shares[k]  + D_shareA[k]                ││
│   │     eval_z_shares[k] := eval_R_shares[k] + D_shareB[k]                ││
│   │                                                                        ││
│   │   Output: LeakyTriple { gen_x_shares, eval_x_shares,                  ││
│   │                         gen_y_shares, eval_y_shares,                  ││
│   │                         gen_z_shares, eval_z_shares,                  ││
│   │                         delta_a, delta_b, n, m }                      ││
│   └────────────────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure (delta from current)

```
src/
├── bcot.rs                  # UNCHANGED — IdealBCot, BcotOutput
├── block.rs                 # UNCHANGED — Block, lsb()
├── delta.rs                 # POSSIBLY MODIFIED — see Pitfall 1; one of delta_a/delta_b
│                            #   must have lsb=0 OR the protocol must adjust
├── feq.rs                   # NEW — F_eq ideal functionality module
├── keys.rs                  # UNCHANGED — Key, Key::as_blocks
├── leaky_tensor_pre.rs      # REWRITTEN — generate() body replaced; LeakyTriple
│                            #   fields renamed (alpha→x, beta→y, correlated→z)
│                            #   and gamma/labels removed
├── lib.rs                   # MODIFIED — add `pub mod feq;`
├── macs.rs                  # UNCHANGED — Mac, Mac::as_blocks
├── matrix.rs                # UNCHANGED — BlockMatrix
├── preprocessing.rs         # MODIFIED — run_preprocessing's call to
│                            #   ltp.generate(0,0) becomes ltp.generate()
├── sharing.rs               # UNCHANGED — AuthBitShare, Add impls
└── tensor_macro.rs          # UNCHANGED — tensor_garbler, tensor_evaluator
```

### Pattern 1: bCOT batch pair → cross-party AuthBitShare vectors

**What:** For each correlation `[v]_pa^{Δ_b}` and `[v]_pb^{Δ_a}` we need, run a paired (`transfer_a_to_b`, `transfer_b_to_a`) on the appropriate choice bits, then assemble two `Vec<AuthBitShare>` that store the cross-party layout (gen holds A's key + A's MAC, eval holds B's key + B's MAC).

**When to use:** Every batch in Step 1 — five batches total: x_A, x_B, y_A, y_B, R. (Note: per the paper, we need `itmac{x_A}{Δ_B}` and `itmac{x_B}{Δ_A}` — these are DIFFERENT correlations and require the existing pattern of TWO bCOT calls per logical share, with each party's bit playing the choice in one direction. Re-read existing `leaky_tensor_pre.rs:73-101` for the exact convention; the rewrite preserves this layout.)

**Example — pattern from existing `src/leaky_tensor_pre.rs:74-101`:**

```rust
// Sample full and gen-portion bits for x (length n)
let x_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
let gen_x_portions: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
let eval_x_portions: Vec<bool> = gen_x_portions.iter().zip(x_bits.iter())
    .map(|(&g, &full)| g ^ full).collect();

// Two bCOT calls: A as sender (B chooses), B as sender (A chooses)
let cot_x_a_to_b = self.bcot.transfer_a_to_b(&eval_x_portions);
let cot_x_b_to_a = self.bcot.transfer_b_to_a(&gen_x_portions);

// Cross-party assembly:
//   gen_share holds A's view: A's sender key (LSB=0), A's MAC under Δ_B, A's bit value
//   eval_share holds B's view: B's sender key (LSB=0), B's MAC under Δ_A, B's bit value
let gen_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_a_to_b.sender_keys[i],
    mac: Mac::new(*cot_x_b_to_a.receiver_macs[i].as_block()),
    value: gen_x_portions[i],
}).collect();
let eval_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_b_to_a.sender_keys[i],
    mac: Mac::new(*cot_x_a_to_b.receiver_macs[i].as_block()),
    value: eval_x_portions[i],
}).collect();
```

[VERIFIED: copied from existing `src/leaky_tensor_pre.rs:73-101`, which currently passes `test_alpha_label_sharing` and `test_correlated_bit_correctness` — only the gamma+labels portions break the existing tests, not the bCOT pattern itself.]

**Critical detail:** "x_A" in the paper is the Pa's own bit; the bCOT direction matters because A is the SENDER in `transfer_a_to_b`, so B's MAC is under Δ_B (A's correlation key for this batch). Cross-checking against `src/bcot.rs:57-79`: `transfer_a_to_b` produces `B's MAC = A's K[0] XOR B's choice * delta_b`. So A's `eval_x_portions` (passed as B's choices) becomes the bit being authenticated. This is `itmac{x_B}{Δ_A}` from the paper (A holds the key under Δ_B's complement convention — read carefully). The existing `verify_cross_party` helper validates that the cross-party layout works regardless of which "side" is in scope.

### Pattern 2: C_A / C_B / C_A^(R) / C_B^(R) per-entry XOR

**What:** Compute four length-m (or length-n*m for R variants) `Vec<Block>`s by XOR-ing 3 terms per entry.

**When to use:** Step 2 of `generate()`, immediately after Step 1.

**Example:**

```rust
// C_A and C_B are length-m
let mut c_a: Vec<Block> = Vec::with_capacity(self.m);
let mut c_b: Vec<Block> = Vec::with_capacity(self.m);
let delta_a_block = *self.bcot.delta_a.as_block();
let delta_b_block = *self.bcot.delta_b.as_block();
for j in 0..self.m {
    let y_a_term = if gen_y_shares[j].value { delta_a_block } else { Block::ZERO };
    let y_b_term = if eval_y_shares[j].value { delta_b_block } else { Block::ZERO };
    // key(y_B@A)[j]: A's key for B's y_B bit. In our cross-party layout,
    //   gen_y_shares[j].key  IS A's sender key from cot_y_a_to_b — this is
    //   the key B's MAC was generated against, i.e. key(y_B@A).
    // mac(y_A@B)[j]: B's MAC of A's y_A bit, held by A. In our layout,
    //   gen_y_shares[j].mac (a Mac) IS A's MAC under Δ_B = mac(y_A@B).
    c_a.push(y_a_term ^ *gen_y_shares[j].key.as_block() ^ *gen_y_shares[j].mac.as_block());
    c_b.push(y_b_term ^ *eval_y_shares[j].mac.as_block() ^ *eval_y_shares[j].key.as_block());
}

// C_A^(R), C_B^(R) are length n*m, column-major (j*n+i indexing)
let mut c_a_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
let mut c_b_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
for k in 0..(self.n * self.m) {
    let r_a_term = if gen_r_shares[k].value { delta_a_block } else { Block::ZERO };
    let r_b_term = if eval_r_shares[k].value { delta_b_block } else { Block::ZERO };
    c_a_r.push(r_a_term ^ *gen_r_shares[k].key.as_block() ^ *gen_r_shares[k].mac.as_block());
    c_b_r.push(r_b_term ^ *eval_r_shares[k].mac.as_block() ^ *eval_r_shares[k].key.as_block());
}
```

[VERIFIED: paper formulas at `references/appendix_krrw_pre.tex:208-216`. Term-to-field correspondence verified by reading the cross-party layout doc-comment at `src/leaky_tensor_pre.rs:60-67` plus the C_A/C_B correctness proof at `references/appendix_krrw_pre.tex:262-279` (which shows `C_A ⊕ C_B = y(Δ_A ⊕ Δ_B)`).]

### Pattern 3: F_eq module (mirroring IdealBCot's shape)

**What:** New `src/feq.rs` exposing one public function (or struct + check method per D-03/D-05) for element-wise BlockMatrix equality with panic on mismatch.

**When to use:** Step 5 of `generate()`, after L_1 and L_2 are assembled.

**Example:**

```rust
// src/feq.rs
//! Ideal F_eq functionality: element-wise BlockMatrix equality check.
//!
//! In the real protocol, parties send L_1 and L_2 to F_eq, which compares them
//! and returns 0 (abort) if they differ, 1 (continue) otherwise. This in-process
//! ideal version panics on mismatch, matching the protocol's abort semantics.
//!
//! TODO: Replace with a real equality-check protocol (e.g., commit-and-open hash)
//! for production.

use crate::matrix::BlockMatrix;

/// Ideal F_eq check. Panics with `"F_eq abort: ..."` on mismatch.
pub fn check(l1: &BlockMatrix, l2: &BlockMatrix) {
    assert_eq!(l1.rows(), l2.rows(), "F_eq: row dimension mismatch");
    assert_eq!(l1.cols(), l2.cols(), "F_eq: column dimension mismatch");
    for j in 0..l1.cols() {
        for i in 0..l1.rows() {
            if l1[(i, j)] != l2[(i, j)] {
                panic!("F_eq abort: consistency check failed — L_1 != L_2 at ({}, {})", i, j);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    #[test]
    fn test_check_equal_matrices_passes() {
        let mut a = BlockMatrix::new(3, 4);
        let mut b = BlockMatrix::new(3, 4);
        for j in 0..4 { for i in 0..3 {
            let v = Block::new([i as u8 ^ j as u8; 16]);
            a[(i, j)] = v; b[(i, j)] = v;
        }}
        check(&a, &b);  // must not panic
    }

    #[test]
    #[should_panic(expected = "F_eq abort")]
    fn test_check_differing_matrices_panics() {
        let a = BlockMatrix::new(2, 2);
        let mut b = BlockMatrix::new(2, 2);
        b[(0, 0)] = Block::new([1; 16]);  // differs from a's default zero
        check(&a, &b);
    }

    #[test]
    #[should_panic(expected = "F_eq: row dimension mismatch")]
    fn test_check_dimension_mismatch_panics() {
        let a = BlockMatrix::new(2, 2);
        let b = BlockMatrix::new(3, 2);
        check(&a, &b);
    }
}
```

[CITED: `src/bcot.rs` "TODO: Replace with a real OT protocol" doc-comment as the pattern for ideal-functionality module documentation; CONTEXT.md D-03/D-04/D-05 for the API shape and abort message.]

### Pattern 4: itmac{D}{Δ} from public bit D — local share construction

**What:** When a bit `d` is publicly known, both parties can construct an `AuthBitShare` for it with no interaction. The simplest convention: one party (say A) sets `value = d`, `key = Block::ZERO`, `mac = if d { Δ_B } else { Block::ZERO }` — i.e. the trivial sharing where the "key" is zero. The other party sets `value = false`, `key = if d { Δ_A } else { Block::ZERO }` (?? — see open question), `mac = Block::ZERO`. Then `value_A ⊕ value_B = d` (correct), and `mac_A == key_B XOR value_A * Δ_B` etc. (verifying the MAC invariant).

**Open question (Q1):** The paper does not pin down the exact construction, only requires `itmac{D}{Δ}` to be locally derivable. The simplest and most common convention is:
- Both parties hold `value = d`, `key = Block::ZERO`, `mac = Block::ZERO`. Then XOR-combining with `itmac{R}{Δ}` gives `gen_z = gen_r + d`, etc. The MAC invariant `mac == key ⊕ value·Δ` holds when `value=0` (both ZERO) but NOT when `value=1` (mac is ZERO, but key⊕Δ ≠ ZERO). This means the trivial sharing is NOT MAC-valid by itself.

The correct construction is: one party (e.g., A) sets `value = d`, MAC trivially = ZERO, key trivially = ZERO; the OTHER party (B) sets `value = false`, key = `d ? Δ_A : Block::ZERO`, MAC = ZERO. Then:
- `gen_value ⊕ eval_value = d ⊕ 0 = d` ✓
- A's MAC under Δ_B: A holds value=d, key (held by B) = Block::ZERO ⊕ (d ? Δ_A : ZERO). But verifier B uses Δ_B ... this gets tangled.

**Recommendation:** During planning, do a quick whiteboard derivation to pick the exact convention. The cleanest is probably:
- `gen_d_share[k] := AuthBitShare { value: d_bits[k], key: Block::ZERO, mac: Block::ZERO }` (A's view)
- `eval_d_share[k] := AuthBitShare { value: false, key: Block::ZERO, mac: if d_bits[k] { Δ_B } else { ZERO } }` (B's view? — but that puts A's MAC in B's slot)

Then, after XOR-combining with the R shares (which DO satisfy the MAC invariant):
- `gen_z[k] = gen_r[k] + gen_d[k] = (R_A.key, R_A.mac, R_A.bit ^ d_bits[k])` ← key/mac unchanged, value updated
- `eval_z[k] = eval_r[k] + eval_d[k] = (R_B.key, R_B.mac ⊕ d·Δ_B, R_B.bit ^ 0)`? — but adding to mac under value=false breaks the invariant.

This is one of the deepest correctness questions in Phase 4. The correct derivation is in the paper proof at `references/appendix_krrw_pre.tex:280-323` (essentially: `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}` as a formal IT-MAC algebra; one side absorbs Δ_A, the other absorbs Δ_B; the trivial sharing of D puts the public-bit term into the value field and zero into key/mac of one party while the other party absorbs the Δ-multiple into its mac).

[ASSUMED] Recommended convention: A owns the public-bit value (puts d in `value`), key/mac both ZERO; B's "share" carries the corresponding Δ_B mass in mac (key ZERO, mac = `d ? Δ_B : Block::ZERO`, value `false`). Then `eval_z[k].mac = R_B.mac ⊕ (d ? Δ_B : ZERO)` (this works because adding a public bit with a known MAC computed under the other party's delta is exactly `K = K_R, M = M_R ⊕ d·Δ_B, value = R_B.value ⊕ 0`).

This is the highest-risk piece of the implementation. **Action for planner:** include a short "design D's local sharing" task before the main `generate()` rewrite, with a paper-cross-check.

[CITED: `references/appendix_krrw_pre.tex:251-253` for the high-level "locally compute itmac{D}{Δ}" requirement; algebraic derivation needs to happen in plan.]

### Anti-Patterns to Avoid

- **Constructing `Mac` from a `Key` block (or vice-versa) by direct cast.** `Key` has the `lsb=0` invariant; `Mac` may have lsb=1. The codebase already documents this at `src/bcot.rs:33-38`. When extracting receiver_macs to use as macs in a `Vec<Mac>`, use `Mac::new(*output.receiver_macs[i].as_block())` (already a `Mac`, no cast needed) — never `Key::from(...)` on a Mac block.
- **Reusing the existing gamma-bit / label-generation code.** The rewrite REMOVES gamma+labels from `LeakyTriple` per D-07. Do not preserve them "just in case" — Phase 5 (combining) does not consume them, and `auth_tensor_pre::combine_leaky_triples` is being rewritten in Phase 5 anyway.
- **Calling `share.verify(delta)` on a cross-party share in tests.** This panics; use the existing `verify_cross_party(pa_share, pb_share, delta_a, delta_b)` helper at `src/leaky_tensor_pre.rs:275-287` (or the inline `verify_pair` closure in `src/preprocessing.rs:133-136`). See **Common Pitfalls → Pitfall 4** and `.planning/codebase/TESTING.md` lines 60-90.
- **Sampling x_A/y_A/R inside `generate` and then deriving x_B/y_B/R as `x_A XOR full`.** The OLD code does this for alpha (`src/leaky_tensor_pre.rs:74-80`) — it's algebraically equivalent but obscures the paper's symmetric formulation. Per D-01 the new code should sample BOTH `x_A`, `x_B` (and likewise y_A, y_B, R_A, R_B) directly as independent uniform bits from the same RNG. Then `x = x_A ⊕ x_B`, `y = y_A ⊕ y_B`, `R = R_A ⊕ R_B`, just as the paper says.
- **Mixing Block and BlockMatrix in `feq::check`.** Per D-05, F_eq receives `&BlockMatrix` arguments. Pass L_1 and L_2 as `BlockMatrix(n, m)` constructed in Step 5; do not pass flat `&[Block]` (would force the helper to know the dims separately).
- **Returning a `Result` instead of panicking.** Per `.planning/codebase/CONVENTIONS.md` lines 105-111: codebase-wide convention is `assert!`/`assert_eq!`/`panic!` for protocol violations. F_eq abort is a panic.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| GGM tree construction | A new tree builder in `leaky_tensor_pre.rs` | `tensor_garbler` / `tensor_evaluator` from Phase 3 (`src/tensor_macro.rs`) | Phase 3 deliverable; 10/10 passing tests; correct endianness; battle-tested AES tweaks |
| AES PRF with tweak | Per-call `FixedKeyAes::new(...)` | `&FIXED_KEY_AES` singleton (`src/aes.rs`) — accessed inside `tensor_macro` already | Singleton is cached + thread-safe via `once_cell::Lazy` (`.planning/codebase/CONVENTIONS.md:36`) |
| Random bit vectors | Manual byte-shuffling | `rng.random_bool(0.5)` in a `(0..n).map(...)` loop — pattern at `src/leaky_tensor_pre.rs:70` | rand 0.9 idiomatic, deterministic with seeded ChaCha12Rng |
| Cross-party AuthBitShare assembly | Custom struct or builder | The existing 5-line per-share `AuthBitShare { key, mac: Mac::new(...), value }` literal pattern at `src/leaky_tensor_pre.rs:87-93` | Already used 4 places in current file; matches the cross-party layout doc-comment |
| Element-wise XOR on n×m matrices | Manual nested loops with index arithmetic | `BlockMatrix` `BitXor` impl (`src/matrix.rs:238-307`) — `&matrix1 ^ &matrix2` returns a new `BlockMatrix` | Already exists; respects column-major; supports both owned and borrowed forms |
| Bit extraction from a Block | Manual byte indexing | `Block::lsb()` (`src/block.rs:96-99`) | One-line method, branch-free |
| `Vec<bool>` ↔ `BlockMatrix` of bits | Custom converter | Don't convert — store D as `Vec<bool>` (length n*m, column-major) and use `if d { Δ_X.as_block() } else { Block::ZERO }` inline when computing L_1/L_2 | Avoids a roundtrip through a Block-of-bits encoding |
| Element-wise BlockMatrix equality | Custom comparator | `feq::check(&l1, &l2)` (Pattern 3) — single helper that's the F_eq abstraction itself | Per D-03; isolates the ideal functionality cleanly |

**Key insight:** Phase 4 is essentially a glue layer — five batches of bCOT calls, two macro calls, one F_eq call, plus arithmetic in the GF(2) field over Block. EVERY heavy primitive already exists; the planner should structure tasks around the 5-step transcript without inventing new abstractions. The biggest implementation risk is the algebra of constructing `itmac{D}{Δ}` from a public bit (Pattern 4 / Q1 above), not the engineering.

---

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | None — repo is a Rust crate with no databases, datastores, or serialized state. `.planning/STATE.md` is documentation, not runtime state. | None |
| Live service config | None — no running services, no Datadog/Cloudflare/external service registrations | None |
| OS-registered state | None — no scheduled tasks, no systemd/launchd, no pm2 | None |
| Secrets/env vars | None — no `.env` files, no SOPS keys, no credentials in source. `Cargo.toml` exposes only public crate metadata. | None |
| Build artifacts | `target/` directory (Rust build cache) is regenerated on `cargo build`; no stale package metadata since field renames are at the type level (no package rename, no egg-info equivalent) | `cargo clean && cargo build` after the rewrite if odd compiler errors persist (unusual for incremental builds) |

**Nothing found in any category — verified by:**
- `Bash(ls /Users/turan/Desktop/authenticated-tensor-garbling/.env*)` → not present
- `Cargo.toml` declares only library + bench targets, no system services
- No external test fixtures or DBs referenced in `src/` or `benches/`

---

## Common Pitfalls

### Pitfall 1: `lsb(Δ_A ⊕ Δ_B) == 1` is required by the paper but does NOT hold in the current codebase

**What goes wrong:** The masked-reveal step (PROTO-07) computes `D = lsb(S_1) ⊕ lsb(S_2)` and the paper's correctness proof (lines 295-313 of `references/appendix_krrw_pre.tex`) shows `S_1 ⊕ S_2 = (x ⊗ y ⊕ R)·(Δ_A ⊕ Δ_B)`. For `lsb(S_1 ⊕ S_2)` to equal `x ⊗ y ⊕ R` per coordinate, we need `lsb(Δ_A ⊕ Δ_B) == 1`. The paper makes this explicit: `references/appendix_krrw_pre.tex:6` says "We require `lsb(Δ_A ⊕ Δ_B)=1`" and lines 17-23 specify `lsb(Δ_A)=1, lsb(Δ_B)=0`.

**The codebase currently has BOTH `lsb(Δ_A) == 1` and `lsb(Δ_B) == 1`.** Per `src/delta.rs:11-15`, `Delta::new` always sets `lsb=true`; per `src/bcot.rs:51-52`, both `delta_a` and `delta_b` are made via `Delta::random` which calls `Delta::new`. So `lsb(Δ_A ⊕ Δ_B) == 0` — the masked reveal silently produces garbage in PROTO-07 and the paper invariant TEST-03 (`Z_full == x_full ⊗ y_full`) WILL fail.

**Why it happens:** The current codebase uses both deltas only for cross-party MAC verification (where the Δ values are independent), not for the masked-reveal optimization that requires the XOR's LSB to be set. `auth_tensor_fpre.rs` and the existing `leaky_tensor_pre.rs` never compute `lsb(Δ_A ⊕ Δ_B)` because they do not implement Construction 2's reveal step.

**How to avoid:** Wave 0 task: modify `Delta::random_b` (new constructor) or modify `IdealBCot::new` to ensure `Δ_B.lsb() == 0`. Recommended approach (least invasive):

Option A — add `Delta::new_with_lsb(block, lsb_value: bool)`:

```rust
impl Delta {
    pub fn new_with_lsb(mut value: Block, lsb_value: bool) -> Self {
        value.set_lsb(lsb_value);
        Self(value)
    }
    pub fn random_b<R: Rng>(rng: &mut R) -> Self {
        Self::new_with_lsb(Block::from(rng.random::<[u8; 16]>()), false)
    }
}
```

Then in `IdealBCot::new`:
```rust
let delta_a = Delta::random(&mut rng_a);    // lsb=1
let delta_b = Delta::random_b(&mut rng_b);  // lsb=0
```

Option B — invert one delta inside `IdealBCot::new` after sampling. Less clean.

**Warning signs:** If you skip this fix and run the implementation: TEST-03 will fail with apparently-random `z_ij != x_i & y_j` mismatches; F_eq will pass (because L_1/L_2 are still consistent — the bug only affects D extraction, not the consistency check). The failure looks like a bug in C_A/C_B or in the macro, not a Δ LSB issue. Highly hard to debug; flag it before any other tests run.

**Cross-codebase impact of changing `Δ_B.lsb()`:** the `verify_cross_party` helper does NOT depend on the LSB of Δ_B; the existing tests in `bcot.rs`, `leaky_tensor_pre.rs`, `preprocessing.rs` that check `delta.as_block().lsb() == 1` for `delta_a` (e.g., `src/preprocessing.rs:124`) are scoped to `delta_a` only. The `auth_tensor_fpre.rs:92-115` label-generation code uses `delta_a` (not `delta_b`) for the masked-bit branch, so it is unaffected. Search the codebase for `delta_b.as_block().lsb()` before the change — none found per Grep:

[VERIFIED via Grep: only `delta_a.as_block().lsb()` is asserted in the codebase; `delta_b.lsb()` is never asserted. Safe to change `delta_b.lsb()` to 0.]

### Pitfall 2: The "old" leaky_tensor_pre tests (4 currently failing) were designed against a buggy cross-party convention

**What goes wrong:** `cargo test --lib` shows 4 baseline failures in `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `auth_tensor_pre::tests::test_combine_mac_invariants`, `preprocessing::tests::test_run_preprocessing_mac_invariants`. These tests use the `verify_cross_party` helper which tries to swap keys and macs across parties — but the OLD `LeakyTensorPre::generate()` does NOT produce shares in the canonical cross-party layout the helper expects. (See `before.txt` capture in `.planning/phases/03-...` — the 4 failures are pre-existing.)

**Why it happens:** Phase 3's `before.txt` baseline accepted these 4 failures explicitly. They are an artifact of the old, paper-noncompliant `generate()`. The Phase 4 rewrite is expected to either fix them (because the new `generate()` produces shares in the correct cross-party layout) OR rewrite the tests to match the new semantics.

**How to avoid:** When writing the new tests for the new `LeakyTriple` (with `gen_x_shares`, `eval_x_shares`, etc.), validate the cross-party convention end-to-end. Do not blindly preserve the old test bodies — they reference field names (`alpha_*`, `gamma_*`, `*_labels`) that no longer exist after D-06/D-07. The new tests should use the new field names AND use `verify_cross_party` to validate the layout. The 4 currently-failing tests will be DELETED in this phase (the file is being rewritten).

**Warning signs:** After the rewrite, if any of `test_alpha_beta_mac_invariants` etc. still appears in `cargo test --lib` output, it's because the rewrite preserved the old tests' code; it should have replaced them. New test names should reflect the new semantics (`test_x_shares_mac_invariants`, `test_z_shares_product_invariant`, etc.).

### Pitfall 3: Confusing `key(y_B@A)[j]` in C_A formula with `gen_y_shares[j].mac`

**What goes wrong:** The paper notation `key(y_B@A)` means "the key A holds for the MAC of B's bit y_B" — i.e., A is the SENDER, B is the receiver. In the cross-party layout, this is stored in `gen_y_shares[j].key` (A's sender key from the `transfer_a_to_b` call on B's choice bit). It is NOT `gen_y_shares[j].mac`, which is A's MAC under Δ_B (= `mac(y_A@B)` in paper notation).

**Why it happens:** The paper notation is dense — `mac` and `key` flip roles depending on whose bit is being authenticated. Reading the proof carefully (lines 264-279 of `appendix_krrw_pre.tex`) makes the assignments unambiguous, but a quick read of just the C_A formula leads to easy off-by-one in field selection.

**How to avoid:** Use the cross-party layout doc-comment at `src/leaky_tensor_pre.rs:60-67` as the canonical mapping:
- `gen_share.key`  = A's sender key from `transfer_a_to_b` (LSB=0)  = "key A holds in this batch" = `key(other_party_bit @ A)`
- `gen_share.mac`  = A's MAC from `transfer_b_to_a` under Δ_B       = "MAC A holds in this batch" = `mac(A's_bit @ B)`
- (mirror for eval_share)

Therefore:
- `key(y_B@A)[j]` = `gen_y_shares[j].key.as_block()` ← A's key for B's y_B bit
- `mac(y_A@B)[j]` = `gen_y_shares[j].mac.as_block()` ← A's MAC of A's y_A bit under Δ_B
- `mac(y_B@A)[j]` = `eval_y_shares[j].mac.as_block()` ← B's MAC of B's y_B bit under Δ_A
- `key(y_A@B)[j]` = `eval_y_shares[j].key.as_block()` ← B's key for A's y_A bit

**Warning signs:** If `C_A ⊕ C_B != y(Δ_A ⊕ Δ_B)` in a unit test that bypasses the macro (i.e., a direct `test_c_a_c_b_correctness`), the field selection is wrong. Add such a unit test in Wave 1 to catch this before the macro layer.

### Pitfall 4: `share.verify(&delta)` panics on cross-party shares — always use `verify_cross_party`

**What goes wrong:** Direct `gen_share.verify(&delta_b)` panics with `"MAC mismatch in share"` because in the cross-party layout, the `key` field of `gen_share` is A's sender key (for which the matching MAC is `gen_share.mac` ONLY if you think of A as the verifier under Δ_A, but `gen_share.mac` is actually under Δ_B because it came from the OTHER bCOT call).

**Why it happens:** `AuthBitShare::verify(delta)` at `src/sharing.rs:60-63` checks `mac == key.auth(value, delta)`. In the cross-party layout, `key` and `mac` come from different bCOT calls and are authenticated against different deltas — the pair satisfies the IT-MAC invariant only after swapping fields between gen_share and eval_share.

**How to avoid:** Reuse the existing helper:

```rust
fn verify_cross_party(
    pa_share: &AuthBitShare, pb_share: &AuthBitShare,
    delta_a: &Delta, delta_b: &Delta,
) {
    AuthBitShare { key: pb_share.key, mac: pa_share.mac, value: pa_share.value }
        .verify(delta_b);
    AuthBitShare { key: pa_share.key, mac: pb_share.mac, value: pb_share.value }
        .verify(delta_a);
}
```

[VERIFIED: present at `src/leaky_tensor_pre.rs:275-287`; identical helper at `src/auth_tensor_pre.rs:134-152`. `.planning/codebase/TESTING.md:60-90` documents this as the codebase's most-important test helper.]

**Warning signs:** `cargo test --lib 2>&1 | grep "MAC mismatch"` — any hit means a test is calling `share.verify(...)` directly on a cross-party share.

### Pitfall 5: Using `Mac::new` requires `pub(crate)` access, not `pub`

**What goes wrong:** `Mac::new(...)` is `pub(crate)` (`src/macs.rs:25`); calling it from outside the crate would be a compile error. Inside `leaky_tensor_pre.rs` (same crate) this works — no action needed.

**Why it happens:** Phase 1 deliberately tightened constructors. `Mac::new` is the only safe way to wrap a `Block` as a `Mac` (there's no public From impl that doesn't exist already at `src/macs.rs:84-89`).

**How to avoid:** Already correctly used at `src/leaky_tensor_pre.rs:90` (`Mac::new(*cot_x_b_to_a.receiver_macs[i].as_block())`); preserve this pattern in the rewrite. Note: the `From<Block> for Mac` impl IS public (`src/macs.rs:84-89`), so `Mac::from(block)` also works. Either is fine.

### Pitfall 6: `t_gen` / `t_eval` arguments to tensor_macro are m×1 column vectors, NOT n×m

**What goes wrong:** Passing `BlockMatrix(n, m)` to `tensor_garbler` triggers an `assert_eq!(t_gen.cols(), 1, ...)` panic at `src/tensor_macro.rs:92`. The macro's T input is the vector being tensored against `a`, which has length m (one Block per output column).

**Why it happens:** Easy confusion — the macro's OUTPUT Z is `n × m`, but its INPUT T is `m × 1`. The ciphertext G_k for `k ∈ [m]` confirms T is length-m.

**How to avoid:** When wrapping C_A and C_B (each `Vec<Block>` of length m), construct as `BlockMatrix::new(m, 1)` and fill via `[k]` indexing (column-vector form). The tensor_macro test already does this correctly at `src/tensor_macro.rs:220-225`:

```rust
let mut t_gen = BlockMatrix::new(m, 1);
for k in 0..m {
    t_gen[k] = Block::random(&mut rng);
}
```

**Warning signs:** Compile-time fine, but runtime panic at first macro call if the dimension is wrong.

### Pitfall 7: Forgetting to update `src/preprocessing.rs::run_preprocessing` for the no-arg `generate()` signature

**What goes wrong:** `run_preprocessing` at `src/preprocessing.rs:99-100` calls `ltp.generate(0, 0)` with two `usize` arguments. After D-01 changes the signature to `generate(&mut self) -> LeakyTriple`, this call site becomes a compile error.

**Why it happens:** The `LeakyTensorPre::generate` API change is the only behavioral change crossing module boundaries.

**How to avoid:** Wave 1 task: update `src/preprocessing.rs:99-100` to `triples.push(ltp.generate());` (no args). Also remove the `_ = (x_clear, y_clear);` line if any local variable ties to the old API exists (none found per `Grep` of `generate(` in `preprocessing.rs`).

[VERIFIED: `src/preprocessing.rs:99` uses `ltp.generate(0, 0)`; this is the only external call to `LeakyTensorPre::generate` in the codebase. Test files in `src/leaky_tensor_pre.rs` (lines 295, 305, etc.) also need updating.]

### Pitfall 8: `auth_tensor_pre::combine_leaky_triples` reads removed `LeakyTriple` fields

**What goes wrong:** `combine_leaky_triples` at `src/auth_tensor_pre.rs:71-105` accesses `triples[i].gen_correlated_shares`, `gen_alpha_labels`, `gen_alpha_shares`, etc. After D-06 renames `gen_alpha_shares → gen_x_shares` and D-07 removes `gen_alpha_labels`, this is a compile error chain.

**Why it happens:** `combine_leaky_triples` is currently the consumer of LeakyTriple. It will be rewritten in Phase 5 to match the new struct shape, but Phase 4's compile must still pass.

**How to avoid:** Phase 4's planner has two viable options:
- **Option A (recommended):** Update `combine_leaky_triples` to reference the new field names (`gen_x_shares`, etc.) and stub out the missing labels by passing `Vec::new()` to `TensorFpreGen { alpha_labels: ..., beta_labels: ... }`. This keeps the build green; Phase 5 will rewrite the combine logic anyway.
- **Option B:** Stub out `combine_leaky_triples` with `unimplemented!("Phase 5 rewrite")` and similarly stub out the broken tests with `#[ignore = "Phase 5"]`. Cleaner separation but more aggressive.

The user did not explicitly choose between A and B in CONTEXT.md. Recommend Option A (less disruptive) and surface this as a planner decision in `04-DISCUSSION-LOG.md` if it isn't already.

[VERIFIED: `Grep` of `gen_alpha_shares\|gen_correlated_shares\|gen_alpha_labels` in `src/auth_tensor_pre.rs` — present at lines 70-104; `src/preprocessing.rs:144` references `correlated_auth_bit_shares` indirectly. All three files (`leaky_tensor_pre.rs`, `auth_tensor_pre.rs`, `preprocessing.rs`) need coordinated updates.]

### Pitfall 9: TestPanics inside `IdealBCot` due to `&mut self` capture across multiple bCOT batches

**What goes wrong:** Each bCOT call mutates `IdealBCot`'s internal `rng`. Borrowing `&mut self.bcot` 10 times in `generate()` (5 batches × 2 directions) is fine because each call is a distinct statement, but if you try to capture `let bcot_ref = &mut self.bcot;` and then call `bcot_ref.transfer_a_to_b(...)` in a loop closure, you may run into borrow-checker friction.

**Why it happens:** `&'a mut IdealBCot` lifetime in `LeakyTensorPre<'a>` is fine for sequential calls but constrains higher-order patterns.

**How to avoid:** Write `generate()` as a flat sequential function (10 statements for the 10 bCOT calls), exactly mirroring the existing `src/leaky_tensor_pre.rs:73-228` style. No closures, no helper methods that borrow `&mut self.bcot`.

[VERIFIED: existing `LeakyTensorPre::generate` already does this at line 83, 85, 112, 113, 179, 180, 211, 212 — 8 calls in sequence, no issues. Adding 2 more for R is mechanically the same.]

---

## Code Examples

Verified patterns from existing codebase + paper appendix.

### Example 1: Sample x_A, x_B as independent uniform bits (replacing the old "sample x and gen_portion derive eval_portion" pattern)

```rust
// Per D-01: sample BOTH party shares directly (paper-symmetric).
let x_a_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
let x_b_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
// y_a_bits, y_b_bits, r_a_bits, r_b_bits — same pattern, different lengths
```

[CITED: pattern from `src/leaky_tensor_pre.rs:70-72` (existing) but adapted per D-01 to sample both parties' bits independently.]

### Example 2: One bCOT batch pair with cross-party AuthBitShare assembly (the canonical pattern, repeated 5 times)

```rust
// The choice bit each batch sends to bcot is the OTHER party's bit:
//   transfer_a_to_b(eval_x_b_choices) means B (receiver) picks K[x_B[i]]
//   transfer_b_to_a(gen_x_a_choices) means A (receiver) picks K[x_A[i]]
let cot_x_a_to_b = self.bcot.transfer_a_to_b(&x_b_bits);  // A is sender, B picks based on x_B
let cot_x_b_to_a = self.bcot.transfer_b_to_a(&x_a_bits);  // B is sender, A picks based on x_A

let gen_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_a_to_b.sender_keys[i],                                    // A's K[0], LSB=0
    mac: Mac::new(*cot_x_b_to_a.receiver_macs[i].as_block()),            // A's MAC under Δ_B
    value: x_a_bits[i],                                                   // A's bit
}).collect();
let eval_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_b_to_a.sender_keys[i],                                    // B's K[0], LSB=0
    mac: Mac::new(*cot_x_a_to_b.receiver_macs[i].as_block()),            // B's MAC under Δ_A
    value: x_b_bits[i],                                                   // B's bit
}).collect();
```

[CITED: structurally adapted from `src/leaky_tensor_pre.rs:83-101` (alpha shares); only the bit-source variables change (alpha_bits → x_a_bits/x_b_bits per D-01).]

### Example 3: Wrap C_A as `BlockMatrix(m, 1)` and call tensor_garbler

```rust
let mut t_a = BlockMatrix::new(self.m, 1);
for j in 0..self.m { t_a[j] = c_a[j]; }                  // c_a: Vec<Block>
let mut t_b = BlockMatrix::new(self.m, 1);
for j in 0..self.m { t_b[j] = c_b[j]; }

let (z_gb1, g_1) = tensor_garbler(self.n, self.m, self.bcot.delta_a,
                                  &eval_x_shares.iter().map(|s| s.key).collect::<Vec<Key>>(),
                                  &t_a);
let e_1 = tensor_evaluator(self.n, self.m, &g_1,
                           &gen_x_shares.iter().map(|s| s.mac).collect::<Vec<Mac>>(),
                           &t_b);
```

**Note:** the macro takes `&[Key]` for keys and `&[Mac]` for macs — these come from the bCOT output directly (no field-extraction needed). The `eval_x_shares.iter().map(...).collect()` step above is wasteful; better to retain the raw `cot_x_a_to_b.sender_keys` (a `Vec<Key>`) from Step 1 and pass it directly:

```rust
// Cleaner — use bCOT outputs directly
let (z_gb1, g_1) = tensor_garbler(self.n, self.m, self.bcot.delta_a,
                                  &cot_x_a_to_b.sender_keys, &t_a);
let e_1 = tensor_evaluator(self.n, self.m, &g_1,
                           &cot_x_a_to_b.receiver_macs, &t_b);
```

Wait — careful. The paper formula (line 222 of `appendix_krrw_pre.tex`) calls `tensor_garbler(n, m, Δ_A, key(x_B@A), C_A)`. `key(x_B@A)` is "A's keys for B's x_B bit" — these are A's SENDER keys from `transfer_a_to_b` (where B is the receiver picking based on x_B). So `cot_x_a_to_b.sender_keys` is correct. Similarly, `mac(x_B@A)` (passed to `tensor_evaluator`) is "B's MAC for B's bit under A's delta_a" = `cot_x_a_to_b.receiver_macs`. CORRECT — the receiver's MAC is B's, authenticating x_B under Δ_A (since A was the sender).

[VERIFIED: paper line 222 vs. `src/bcot.rs:57-79` — `transfer_a_to_b` returns `sender_keys` (A's K[0]) and `receiver_macs` (B's K[0] ⊕ choice·Δ_B). But wait, the paper says under Δ_A, not Δ_B! Re-read…]

**This is a critical paper-vs-code semantic check the planner must do**. Paper notation `itmac{x_B}{Δ_A}` means "B's bit x_B authenticated under A's delta Δ_A" — the verifier here is A (verifier holds Δ). In `transfer_a_to_b`, A is sender holding K[0], B is receiver picking K[choice·Δ_B]. So this batch produces `mac(x_B) = K[0] ⊕ x_B·Δ_B`, authenticated under **Δ_B**, not Δ_A. To get `itmac{x_B}{Δ_A}`, we need the OPPOSITE direction — use `transfer_b_to_a(&x_b_bits)` so B is sender, A is receiver, B's K[0] gets selected by A based on x_B... but then A's MAC is for A's bit, not B's.

This is a deep ambiguity that needs careful resolution in plan step 1. **Recommendation:** the planner allocates the first task to "audit existing leaky_tensor_pre.rs cross-party convention against the paper notation, document the mapping table, and have a human reviewer check before coding the new generate()". This audit could change the paper's choice-bit assignments.

[ASSUMED] — the existing code's convention (which passes 5 of 9 tests) might be inverted relative to the paper notation. The 4 failing tests may be symptoms of this mismatch, NOT just outdated tests. Resolution requires reading both `src/leaky_tensor_pre.rs:60-67` doc-comment AND the paper carefully, side by side. **This is the highest-risk research finding in this document.**

### Example 4: Compute D = lsb(S_1) ⊕ lsb(S_2) into `Vec<bool>`

```rust
let mut d_bits: Vec<bool> = Vec::with_capacity(self.n * self.m);
for j in 0..self.m {
    for i in 0..self.n {
        d_bits.push(s_1[(i, j)].lsb() ^ s_2[(i, j)].lsb());
    }
}
// d_bits is column-major, length n*m, indexed by k = j*n + i
```

[CITED: column-major iteration matches the existing convention at `src/leaky_tensor_pre.rs:163-168`.]

### Example 5: Build L_1, L_2 as BlockMatrix and call F_eq

```rust
let mut l_1 = BlockMatrix::new(self.n, self.m);
let mut l_2 = BlockMatrix::new(self.n, self.m);
let delta_a_block = *self.bcot.delta_a.as_block();
let delta_b_block = *self.bcot.delta_b.as_block();
for j in 0..self.m {
    for i in 0..self.n {
        let k = j * self.n + i;
        let d_term_a = if d_bits[k] { delta_a_block } else { Block::ZERO };
        let d_term_b = if d_bits[k] { delta_b_block } else { Block::ZERO };
        l_1[(i, j)] = s_1[(i, j)] ^ d_term_a;
        l_2[(i, j)] = s_2[(i, j)] ^ d_term_b;
    }
}
crate::feq::check(&l_1, &l_2);  // panics on abort
```

[CITED: paper lines 248-249 of `appendix_krrw_pre.tex` for the L_1, L_2 formula.]

---

## State of the Art

| Old Approach (pre-rewrite) | Current Approach (Phase 4) | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct-AND ideal-trusted-dealer-style preprocessing in `leaky_tensor_pre.rs::generate(x_clear, y_clear)` | Paper Construction 2: tensor-macro-based with masked reveal + F_eq | This phase | Cannot mix — old `generate(x, y)` and new `generate()` are incompatible signatures; downstream `run_preprocessing` MUST be updated |
| `gamma` bits + wire `labels` stored on `LeakyTriple` | Removed from `LeakyTriple` (D-07); only x, y, Z shares + deltas remain | This phase | Phase 5 combine logic must be rewritten to not reference these fields |
| `LeakyTriple` field names: `alpha`, `beta`, `correlated` | Renamed to `x`, `y`, `z` (paper notation) per D-06 | This phase | Mass field renames in `auth_tensor_pre.rs`, `preprocessing.rs`, and any tests |
| In-process `F_eq` does not exist | New `src/feq.rs` module with `check(&BlockMatrix, &BlockMatrix)` panic-on-mismatch | This phase | New module; v2 will replace with networked F_eq |

**Deprecated/outdated (post-Phase-4):**
- The 4 currently-failing `*_mac_invariants` tests in `leaky_tensor_pre.rs`/`auth_tensor_pre.rs`/`preprocessing.rs` are based on the old struct shape and old generate() output; they will be deleted (replaced by new tests with new field names).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The cleanest convention for `itmac{D}{Δ}` from a public bit `d` is: A sets `value=d, key=ZERO, mac=ZERO`; B sets `value=false, key=ZERO, mac=if d {Δ_B} else {ZERO}` (one party absorbs the Δ-mass into mac). | Pattern 4 / Open Question Q1 | If the convention is wrong, the final XOR `gen_z = gen_r ⊕ gen_d` may not satisfy the cross-party MAC invariant, causing TEST-02 (mac=key⊕bit·Δ) to fail. **HIGH risk** — needs paper/whiteboard derivation in plan step 1. |
| A2 | The existing cross-party AuthBitShare layout (`gen.key=A's_K0_from_a_to_b; gen.mac=A's_MAC_from_b_to_a; eval.key=B's_K0_from_b_to_a; eval.mac=B's_MAC_from_a_to_b`) corresponds to paper `itmac{x_A}{Δ_B}` and `itmac{x_B}{Δ_A}`. | Pitfall 3, Example 3, footnote | If inverted, the C_A/C_B formula will compute the wrong term and TEST-03 will fail with apparently-random (but reproducible) bit-pattern mismatches. **HIGH risk** — explicit audit task recommended in plan. The 4 currently-failing baseline tests may be evidence this is wrong. |
| A3 | Modifying `IdealBCot::new` to make `Δ_B.lsb() == 0` (Pitfall 1 fix) does not break any existing tests in `bcot.rs`, `leaky_tensor_pre.rs`, `auth_tensor_pre.rs`, `preprocessing.rs`. | Pitfall 1 | If wrong, fixing the LSB invariant cascades into other test breakages. **LOW** — verified via `Grep` that no test asserts `delta_b.lsb()`. |
| A4 | `combine_leaky_triples` in `src/auth_tensor_pre.rs` should be updated in this phase (Option A in Pitfall 8) so the build stays green; full rewrite of combine logic happens in Phase 5. | Pitfall 8 | If user prefers Option B (`unimplemented!()`), the planner needs to confirm. **LOW** — both options work; this is taste. Surface as a planner decision. |
| A5 | The R_A, R_B uniform bit vectors are sampled fresh per `generate()` call (not derived from x or y). Length n*m, column-major. | D-11/D-12, Pattern 1 | Per D-11 explicit, very low risk. |
| A6 | The `Vec<AuthBitShare>` for Z (length n*m, column-major) can be assembled from per-entry XOR of `gen_r_shares[k]` with the public-D-derived share without any helper. | D-08 / Pattern 4 / Example assumes inline | LOW — assuming A1 resolves correctly. |
| A7 | The 4 currently-failing baseline tests are pre-existing artifacts of the file being rewritten and will be DELETED (not preserved) in this phase. | Pitfall 2, Validation Architecture | LOW — confirmed by phase 3 baseline-acceptance pattern (see `before.txt` in phase 03 dir). |
| A8 | F_eq's check operates on `BlockMatrix` of dimensions exactly `n × m` (matching L_1, L_2 dimensions); not flattened, not n*m × 1. | Pattern 3, D-05 | LOW — explicit in D-05. |

**This table is non-empty:** A1 and A2 in particular are HIGH-risk and need a pre-implementation audit/whiteboard task in the plan. A3, A4 are user-confirmation items (LOW–MEDIUM). A5–A8 are routine.

---

## Open Questions

1. **Q1 (HIGH-risk):** What is the correct local-derivation of `itmac{D}{Δ}` from public bit D such that XOR-combining with `itmac{R}{Δ}` preserves the cross-party MAC invariant?
   - What we know: D is public, both parties know it; `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}` per paper.
   - What's unclear: the exact assignment of (key, mac, value) on each party's side for the public-D share. Multiple valid conventions exist; we need the one that lets element-wise `AuthBitShare + AuthBitShare` (existing `Add` impl at `src/sharing.rs:66-117`) produce a valid Z share.
   - Recommendation: First task in Phase 4 plan should be a paper-derivation task (no code) producing a markdown doc that maps D → (gen_d_share, eval_d_share) with a worked numerical example, reviewed by user before coding.

2. **Q2 (HIGH-risk):** Does the existing cross-party AuthBitShare layout (`gen.key/eval.key` from one bCOT direction, `gen.mac/eval.mac` from the other) correspond to paper `itmac{x_A}{Δ_B}` or its inverse?
   - What we know: 4 baseline tests fail with `MAC mismatch in share` (`cargo test --lib`). The doc-comment at `src/leaky_tensor_pre.rs:60-67` describes the layout but does not justify it against paper notation.
   - What's unclear: whether the failing tests are evidence of a bug in the layout (and the rewrite must fix it) OR are simply outdated against the new struct shape.
   - Recommendation: audit task before any C_A/C_B code is written. Construct a 2×2 truth table of (party, bCOT direction) → (paper notation, code field) and verify all four corners.

3. **Q3 (MEDIUM):** Should `combine_leaky_triples` (`src/auth_tensor_pre.rs:38-108`) be updated in Phase 4 to use new field names (Option A in Pitfall 8) or stubbed with `unimplemented!()` (Option B)?
   - What we know: Phase 5 will rewrite the combining logic anyway; CONTEXT.md does not specify.
   - What's unclear: user's preference between "minimal changes outside leaky_tensor_pre" (B) vs. "build green at every commit" (A).
   - Recommendation: Option A (rename only, keep semantics broken-in-Phase-5-anyway). Surface to user in plan-check phase if there's no clear signal.

4. **Q4 (LOW):** What seed should be used for sampling x_A, x_B, y_A, y_B, R inside `generate()` — derive deterministically from `self.rng` (the existing `ChaCha12Rng` in `LeakyTensorPre`) or pass per-call?
   - What we know: D-01 specifies "using the LeakyTensorPre's own ChaCha12Rng".
   - What's unclear: nothing — this is fully resolved by D-01. Documenting for completeness.
   - Recommendation: derive from `self.rng` (existing field).

5. **Q5 (LOW):** Should `feq::check` take `&BlockMatrix` (D-05) or `&MatrixViewRef<Block>` (more flexible)?
   - What we know: D-05 says `&BlockMatrix`.
   - What's unclear: nothing.
   - Recommendation: follow D-05.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (Rust toolchain) | Build, test, bench | ✓ | 1.90.0 (840b83a10 2025-07-30) | — |
| `rustc` | Build | ✓ | 1.90.0 (1159e78c4 2025-09-14) | — |
| `rand`, `rand_chacha` | RNG inside `generate()` | ✓ | rand 0.9, rand_chacha 0.9 (per Cargo.toml:13-14) | — |

**No missing dependencies.** All tooling is the same toolchain that built Phase 3 successfully.

[VERIFIED via Bash: `cargo --version` and `rustc --version` outputs above.]

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness (cargo test) |
| Config file | `Cargo.toml` (no separate test config) |
| Quick run command | `cargo test --lib 2>&1 \| tail -30` |
| Full suite command | `cargo test 2>&1 \| tail -40` |
| Estimated runtime | ~5-10 seconds for `--lib` (current baseline ~5s) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROTO-04 | Five bCOT batch pairs produce x_A, x_B, y_A, y_B, R authenticated shares with correct dimensions | unit | `cargo test --lib leaky_tensor_pre::tests::test_correlated_randomness_dimensions` | ❌ Wave 0 (new test) |
| PROTO-05 | C_A ⊕ C_B == y(Δ_A ⊕ Δ_B) (paper proof line 262-279) — element-wise check | unit | `cargo test --lib leaky_tensor_pre::tests::test_c_a_c_b_xor_invariant` | ❌ Wave 0 |
| PROTO-06 | Z_gb1 ⊕ E_1 == x_B ⊗ y(Δ_A ⊕ Δ_B) and symmetric for macro 2 | integration | `cargo test --lib leaky_tensor_pre::tests::test_macro_outputs_xor_invariant` | ❌ Wave 0 |
| PROTO-07 | D = lsb(S_1) ⊕ lsb(S_2) == x ⊗ y ⊕ R; full Z assembly correct | integration | `cargo test --lib leaky_tensor_pre::tests::test_d_extraction_and_z_assembly` | ❌ Wave 0 |
| PROTO-08 | F_eq passes when L_1 == L_2 in honest run; F_eq.check is callable | unit (in `feq.rs`) + integration (in `leaky_tensor_pre.rs`) | `cargo test --lib feq::` | ❌ Wave 0 |
| PROTO-09 | LeakyTriple struct has only `n`, `m`, `gen_x_shares`, `eval_x_shares`, `gen_y_shares`, `eval_y_shares`, `gen_z_shares`, `eval_z_shares`, `delta_a`, `delta_b` fields | compile-time + grep | `cargo build --lib && cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_shape` | ❌ Wave 0 |
| TEST-02 | All x/y/z shares satisfy IT-MAC invariant under verifier's delta (cross-party) | integration | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_mac_invariants` | ❌ Wave 0 |
| TEST-03 | Z_full(i,j) == x_full(i) AND y_full(j) for all (i,j) reconstructed from gen+eval shares | integration | `cargo test --lib leaky_tensor_pre::tests::test_leaky_triple_product_invariant` | ❌ Wave 0 |
| TEST-04 | F_eq passes equal matrices; F_eq panics on differing matrices (`#[should_panic]`) | unit | `cargo test --lib feq::tests::test_check_equal_matrices_passes && cargo test --lib feq::tests::test_check_differing_matrices_panics` | ❌ Wave 0 (in new feq.rs) |

### Sampling Rate

- **Per task commit:** `cargo test --lib 2>&1 | tail -30`
- **Per wave merge:** `cargo test 2>&1 | tail -40`
- **Phase gate:** Full suite green (or baseline-accepted failures explicitly listed) before `/gsd-verify-work`. The 4 currently-failing tests will be DELETED in this phase; the new test suite must be 100% green.

### Wave 0 Gaps

- [ ] **Δ_B LSB fix (Pitfall 1) — Critical, must precede ALL implementation tasks.** Modify `src/delta.rs` and `src/bcot.rs` so `delta_b.as_block().lsb() == 0`. Add a new test `bcot::tests::test_delta_xor_lsb_is_one`.
- [ ] **`src/feq.rs` module stub** — file with `pub fn check(&BlockMatrix, &BlockMatrix)` signature + 3 inline tests (equal-passes, differing-panics, dim-mismatch-panics). Add `pub mod feq;` to `src/lib.rs`.
- [ ] **New `LeakyTriple` struct** — fields per D-06/D-07/D-08/D-09; `LeakyTensorPre::generate(&mut self) -> LeakyTriple` signature with `unimplemented!()` body (compile only).
- [ ] **Update `combine_leaky_triples` field references** (Option A per Pitfall 8) — change `gen_alpha_shares` → `gen_x_shares`, etc.; remove references to `*_labels` and `*_gamma_shares`.
- [ ] **Update `preprocessing::run_preprocessing`** — change `ltp.generate(0, 0)` to `ltp.generate()`.
- [ ] **Delete the 4 broken tests** (`test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `test_combine_mac_invariants`, `test_run_preprocessing_mac_invariants`); replace with new equivalents that use the new struct shape.
- [ ] **New paper-invariant test stubs** (PROTO-04 through TEST-04 per the table above) — `#[test] fn test_xxx() { unimplemented!() }` placeholders in Wave 0; bodies filled in implementation waves.
- [ ] **Audit task: cross-party AuthBitShare layout vs. paper notation (Q2)** — markdown doc in `04-PATTERNS.md` or as a Wave 0 artifact, mapping (party, bCOT direction) → (paper notation, code field). MUST review before C_A/C_B code is written.
- [ ] **Audit task: `itmac{D}{Δ}` local derivation (Q1)** — markdown doc with the chosen convention and a worked 2×2 example showing all four invariants hold.

---

## Security Domain

> Phase 4 deals with cryptographic protocol implementation; security is the reason for the project's existence. The standard ASVS taxonomy applies in a transformed form (most ASVS categories are about web/auth, less applicable to a crypto-protocol library).

### Applicable ASVS Categories (adapted to crypto-protocol library)

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | No human auth in this layer; this IS the auth layer (IT-MAC) |
| V3 Session Management | no | Stateless protocol step; LeakyTensorPre is the session abstraction |
| V4 Access Control | no | Library-level; no users |
| V5 Input Validation | yes (adapted) | Dimension assertions on `Vec<bool>` lengths and BlockMatrix dimensions match paper-required values; bCOT outputs must pass IT-MAC invariant before being used (verified by `verify_cross_party` tests) |
| V6 Cryptography | yes | Use existing `FixedKeyAes` singleton; never construct a new AES key in protocol code; never roll a new GGM tree (use `tensor_macro`) |
| V11 Business Logic | yes | F_eq abort on mismatch is the security-critical path; aborts must be unconditional and immediate (not logged-and-continue) |
| V12 Files and Resources | n/a | No file I/O in this layer |
| V13 API and Web Service | n/a | Library, no service surface |

### Known Threat Patterns for two-party authenticated preprocessing

| Pattern | STRIDE | Standard Mitigation | Where Mitigated in Phase 4 |
|---------|--------|---------------------|----------------------------|
| Selective failure attack on `x_B` via malformed GGM ciphertexts | Tampering | F_eq consistency check catches malformed transcripts with prob 1 - 2^(-csp) | F_eq call (PROTO-08) — abort path is unconditional |
| Tampering with C_A/C_B during reveal | Tampering | F_eq check on L_1, L_2 includes the C terms transitively (S_1, S_2 carry them) | F_eq call (PROTO-08) |
| Adversarial bit-guessing of `x_B` via tree query | Information Disclosure | Pi_LeakyTensor admits bounded leakage (one bit per level), absorbed by Pi_aTensor combining (Phase 5) | This phase exposes the leakage; mitigation is Phase 5/6 |
| Cross-party MAC inversion (using gen_share's own .key/.mac directly) | Tampering / spoofing | Always use `verify_cross_party` helper; never call `share.verify()` on cross-party shares | Tests (TEST-02, repeated across the new test suite) |
| Δ leakage via low-entropy public reveals | Information Disclosure | D is one-time-padded by R (random tensor mask); revealing D leaks nothing about x ⊗ y | D-derivation in PROTO-07; R is freshly sampled per `generate()` call |
| Constant-time leakage via boolean branching on secret bits | Side-channel | Library does NOT claim constant-time; bench/test only | Out of scope per project (research/proto code) |
| Use of weak / non-cryptographic RNG | Tampering | All RNG uses `ChaCha12Rng` (CSPRNG); seeded for determinism in tests | `LeakyTensorPre::new(seed, ...)` already does this |

**Critical security invariant for this phase:** `F_eq.check` MUST `panic!` on mismatch (D-04). A "soft fail" that returns Result<()> and lets the caller continue would silently downgrade security to semi-honest. The unconditional `panic!` matches the ideal F_eq abort. Tests with `#[should_panic(expected = "F_eq abort")]` enforce this at compile/run time.

[CITED: paper §3 + Theorem 2 (`references/appendix_krrw_pre.tex:325-413`) for security against malicious A; same logic by symmetry for malicious B.]

---

## Sources

### Primary (HIGH confidence)

- **`references/appendix_krrw_pre.tex`** — paper appendix F, Construction 2 (lines 198-254): full Pi_LeakyTensor protocol transcript. Used for: protocol structure, C_A/C_B formulas, S_1/S_2 expressions, L_1/L_2 derivation, F_eq abort semantics, security proof outline. Referenced 30+ times above.
- **`references/Authenticated_Garbling_with_Tensor_Gates-7.pdf`** — paper §3 — context for Pi_LeakyTensor's role in the preprocessing pipeline (one-shot leaky → bucket-combined `aTensor`).
- **`src/tensor_macro.rs`** — Phase 3 deliverable; `tensor_garbler` and `tensor_evaluator` signatures and preconditions (file lines 82-181). 10 passing tests demonstrate the primitive is correct.
- **`src/bcot.rs`** — `IdealBCot::transfer_a_to_b` / `transfer_b_to_a` semantics; sender_keys/receiver_macs convention; cross-party share construction template at lines 105-127.
- **`src/leaky_tensor_pre.rs`** (existing) — cross-party AuthBitShare layout doc-comment (lines 60-67); `verify_cross_party` test helper (lines 275-287); pattern of bCOT batch pairs (lines 73-228).
- **`src/sharing.rs`** — `AuthBitShare` definition (lines 42-50), `Add` impls for XOR (lines 66-117), `verify(&Delta)` (lines 60-63).
- **`src/delta.rs`** — `Delta::new` always sets `lsb=true` (line 13); `Delta::random` (lines 30-34) uses `Delta::new`. **This is the source of Pitfall 1.**
- **`src/preprocessing.rs:99-100`** — call site of `ltp.generate(0, 0)` that needs updating.
- **`src/auth_tensor_pre.rs:71-105`** — `combine_leaky_triples` reads removed `LeakyTriple` fields.

### Secondary (MEDIUM confidence)

- **`.planning/codebase/CONVENTIONS.md`** — codebase-wide conventions (cross-party MAC, column-major, assert-on-violation patterns); used to derive Pattern 4, Pitfall 4, Anti-Patterns.
- **`.planning/codebase/TESTING.md`** — testing conventions, `verify_cross_party` doc, test layout patterns.
- **`.planning/phases/03-m2-generalized-tensor-macro-construction-1/03-VERIFICATION.md`** — confirms Phase 3 outputs and the 4-failure baseline.
- **`.planning/phases/03-m2-generalized-tensor-macro-construction-1/03-CONTEXT.md`** — locked tensor_macro signatures and TensorMacroCiphertexts type.

### Tertiary (LOW confidence — flagged for validation)

- **Q1 / Pattern 4** — exact `itmac{D}{Δ}` local-derivation convention. Paper does not give explicit field assignments; planner needs whiteboard derivation.
- **Q2** — whether the existing cross-party layout matches paper notation `itmac{x_A}{Δ_B}` directly or is inverted. Needs audit before code.

---

## Project Constraints (from CLAUDE.md)

No `./CLAUDE.md` file exists at the repo root (verified via `Bash(ls .../CLAUDE.md)` → exit 1). All project conventions come from `.planning/codebase/*.md` (CONVENTIONS, TESTING, ARCHITECTURE, etc.) which were folded into the **Architecture Patterns** and **Common Pitfalls** sections above.

Implicit project constraints from `.planning/codebase/CONVENTIONS.md` worth re-stating:

- **No Result/Error types in protocol logic** — use `assert!`/`panic!`. F_eq abort is a panic (matches D-04).
- **Newtype wrapping at API boundaries** — pass `&[Key]`, `&[Mac]` to tensor_macro (NOT `&[Block]`); use `Key::as_blocks` only inside the macro body.
- **Cross-party MAC layout** — gen_share holds A's key + A's MAC under Δ_B; eval_share holds B's key + B's MAC under Δ_A. Never call `share.verify()` directly on cross-party shares.
- **Column-major n×m indexing** — `index = j*n + i` everywhere; `BlockMatrix` storage matches.
- **`pub(crate)` for internal helpers** — `feq::check` should be `pub` since `LeakyTensorPre` (in a different module) calls it; or `pub(crate)` since both are in the same crate. Recommend `pub` for clarity (the function is the module's whole API surface).
- **Inline `#[cfg(test)] mod tests`** at bottom of each source file — no separate `tests/` directory.
- **Deterministic seeded RNG** in tests via `ChaCha12Rng::seed_from_u64`; never `rand::rng()` / `thread_rng()` in protocol code or tests.

---

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — every component verified to exist via direct file Reads; versions confirmed via `Cargo.toml` and `cargo --version`.
- Architecture: **HIGH** — protocol transcript matches paper line-by-line; tensor_macro primitive verified working in Phase 3.
- Pitfalls: **HIGH** for Pitfall 1 (Δ LSB — verified by Grep + reading `src/delta.rs:11-15` and `src/bcot.rs:51-52`); **HIGH** for Pitfalls 2-9 (each grounded in a specific source file location).
- Open Questions Q1, Q2: **LOW** confidence in the resolution (need pre-implementation audit) but **HIGH** confidence that they ARE the open questions.

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — codebase changes infrequently; only Δ-LSB fix or paper notation re-interpretation could invalidate findings)
