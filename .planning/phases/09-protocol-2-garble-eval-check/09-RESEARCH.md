# Phase 9: Protocol 2 Garble/Eval/Check - Research

**Researched:** 2026-04-24
**Domain:** Authenticated garbled circuits — Protocol 2 (dual-delta authenticated tensor macros)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Wide Leaf Type (P2-01)**
- D-01: `gen_unary_outer_product_wide` uses `(Block, Block)` tuples. The pair is the (κ ‖ ρ)-bit concatenation. First `Block` = κ-prefix (feeds Z_gb), second `Block` = ρ-suffix (feeds Z'_gb). Wide ciphertexts returned as `Vec<(Block, Block)>`.
- D-02: Both κ and ρ are 128 bits. `Delta` is a newtype over `Block([u8; 16])` for both `delta_a` and `delta_b`. κ+ρ = 256 bits = two `Block` values.
- D-03: Wide leaf expansion uses even/odd tweak convention: `base = seeds.len() * j + i`; `kappa_half = cipher.tccr(Block::from(base << 1), seeds[i])`; `rho_half = cipher.tccr(Block::from(base << 1 | 1), seeds[i])`. Two TCCR calls per (leaf, column) pair.

**D_ev Preprocessing Fields (P2-01 / P2-02 / P2-03)**
- D-04: `TensorFpreGen` and `TensorFpreEval` each get three new fields: `alpha_d_ev_shares`, `beta_d_ev_shares`, `correlated_d_ev_shares` — all `Vec<AuthBitShare>` of lengths n, m, n*m respectively.
- D-05: `gamma_auth_bit_shares` is **renamed** to `gamma_d_ev_shares`. All call sites in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, and `lib.rs` tests must be updated in the same commit.
- D-06: `IdealPreprocessingBackend::run()` generates all four D_ev fields using `TensorFpre::gen_auth_bit()` per entry.
- D-07: All existing constructors of `TensorFpreGen`/`TensorFpreEval` must initialize the three new fields (and use the renamed `gamma_d_ev_shares`) in the same commit.
- D-08: Gen side holds garbler's IT-MAC shares, eval side holds evaluator's IT-MAC shares — same symmetric layout as all other field pairs.

**_p2 Function Placement (P2-02 / P2-03 / P2-04)**
- D-09: Protocol 2 garble and evaluate are new methods on `AuthTensorGen` and `AuthTensorEval` with `_p2` suffix. No new files.
- D-10: `garble_final_p2()` returns `(Vec<Block>, Vec<Block>)` — first is D_gb output share, second is D_ev output share. No new fields added to `AuthTensorGen`.
- D-11: `evaluate_p2()` (or split equivalents) produces D_ev-authenticated output wire shares as part of its return value.

**P2 Consistency Check (P2-04)**
- D-12: Uses existing `check_zero()` in `src/online.rs` with `delta_b` (D_ev) as the verifying delta.
- D-13: `c_gamma` assembly: both parties compute `[L_gamma D_ev] := [v_gamma D_ev] XOR [l_gamma D_ev]`; evaluator's share: `[c_gamma]^ev := [L_gamma D_ev]^ev XOR L_gamma * D_ev`; garbler's share: `[c_gamma]^gb := [L_gamma D_ev]^gb`.

**P2 End-to-End Test (P2-05)**
- D-14: Single tensor gate test, mirroring P1-04. `cargo test` must pass all existing P1 tests unchanged.

### Claude's Discretion

- Exact `v_alpha D_ev` initialization for input wires in the single-gate test: `[v_w D_ev]^gb := [l_w D_ev]^gb` on input wires (garbler side); `[v_w D_ev]^ev := [l_w D_ev]^ev XOR L_w * D_ev` on evaluator side.
- Whether to split `evaluate_p2` into `evaluate_first_half_p2`/`evaluate_second_half_p2`/`evaluate_final_p2` — follow whatever structure `garble_*_p2` takes for symmetry.
- Exact parameter names for the new `_p2` methods — follow existing method conventions.

### Deferred Ideas (OUT OF SCOPE)

- `open()` (ONL-01/ONL-02) — still deferred; P2 consistency check does not require it.
- Multi-gate circuit test for P2 — single-gate test is sufficient for Phase 9.
- D_ev wire-value propagation across multiple gates in a real circuit.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| P2-01 | `gen_unary_outer_product_wide` produces (kappa+rho)-bit leaf expansions; D_gb/D_ev shares satisfy IT-MAC invariant | Confirmed by `6_total.tex` Construction 4, existing `gen_unary_outer_product` template in `tensor_ops.rs:80`. D-03 specifies exact tweak scheme. |
| P2-02 | Protocol 2 garble (`_p2` variant) — garbler never reveals masked values to evaluator | Confirmed by `6_total.tex` Construction 5. Return types from D-10 enforce the privacy property statically. |
| P2-03 | Protocol 2 evaluate (`_p2` variant) produces D_ev-authenticated output wire shares | Confirmed by `6_total.tex` Construction 5, eval side. Symmetric to D-10 via D-11. |
| P2-04 | Protocol 2 consistency check passes for honest parties; P1 tests unmodified | Confirmed by `6_total.tex` Protocol 2 step 9. D-12/D-13 specify exact `c_gamma` assembly. Reuses `check_zero()`. |
| P2-05 | Single end-to-end P2 test verifies garbler XOR evaluator output equals correct tensor product | Mirrors existing `test_auth_tensor_product_full_protocol_1` structure in `lib.rs`. |
</phase_requirements>

---

## Summary

Phase 9 extends the existing Protocol 1 garble/eval machinery to Protocol 2 by widening the GGM leaf expansion from κ-bit to (κ+ρ)-bit outputs, adding three new D_ev preprocessing fields to `TensorFpreGen`/`TensorFpreEval`, renaming `gamma_auth_bit_shares` to `gamma_d_ev_shares`, and implementing `_p2`-suffixed methods on `AuthTensorGen`/`AuthTensorEval`. The protocol's key security property — that the garbler never reveals masked wire values — is enforced by the `_p2` method return type: garble returns `(Vec<Block>, Vec<Block>)` (D_gb and D_ev output shares, never raw wire values) rather than sending L_gamma to the evaluator.

The consistency check for Protocol 2 reuses the existing `check_zero()` primitive from `src/online.rs` without modification. The key difference from Protocol 1 is that c_gamma is assembled from D_ev-authenticated shares (under `delta_b`) rather than D_gb-authenticated shares (under `delta_a`), and the evaluator computes its c_gamma share locally using its recovered L_gamma and D_ev, rather than having L_gamma revealed by the garbler. This is architecturally simpler than Protocol 1's check because no masked-value revelation to the garbler is needed.

The existing test suite has 95 passing tests and provides the Protocol 1 E2E test (`test_auth_tensor_product_full_protocol_1`) as the direct structural template for the P2-05 test. All Phase 9 changes are additive (new methods, new fields, one rename) and must keep all 95 existing tests green.

**Primary recommendation:** Implement in four atomic commits: (1) rename + new preprocessing fields, (2) `gen_unary_outer_product_wide`, (3) `_p2` methods on AuthTensorGen/AuthTensorEval, (4) P2 E2E test.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Wide leaf expansion (κ+ρ PRG) | `tensor_ops.rs` | — | All GGM tree ops live in tensor_ops; wide variant is purely a leaf-expansion change |
| D_ev preprocessing field generation | `preprocessing.rs` (`IdealPreprocessingBackend`) | `auth_tensor_fpre.rs` (via `gen_auth_bit`) | Preprocessing owns field generation; fpre provides the per-bit helper |
| Protocol 2 garble | `auth_tensor_gen.rs` (`AuthTensorGen`) | `tensor_ops.rs` (wide ops) | AuthTensorGen owns all garble logic; tensor_ops provides primitives |
| Protocol 2 evaluate | `auth_tensor_eval.rs` (`AuthTensorEval`) | `tensor_ops.rs` (wide ops) | AuthTensorEval owns all eval logic |
| P2 consistency check | `online.rs` (existing `check_zero`) | caller assembly in `lib.rs` test | check_zero is a thin primitive; c_gamma assembly is caller's responsibility |
| P2 E2E test | `src/lib.rs` (integration tests) | — | All E2E protocol tests live here per existing convention |

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust / cargo | edition 2024 | Language and build | Project standard |
| `rand` / `rand_chacha` | (existing) | RNG for preprocessing | Already in use throughout codebase [VERIFIED: Cargo.toml pattern in codebase] |
| `FixedKeyAes` / `tccr` | (internal) | TCCR-based PRG expansion | Used by all existing GGM tree ops [VERIFIED: tensor_ops.rs] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `BlockMatrix` / `MatrixViewMut` | (internal) | Dense n×m Block storage | All wide output accumulation, same as P1 path |
| `AuthBitShare` | (internal) | IT-MAC share type | All new D_ev fields use this type [VERIFIED: sharing.rs] |

**Installation:** No new dependencies. Phase 9 is purely additive to the existing codebase.

---

## Architecture Patterns

### System Architecture Diagram

```
IdealPreprocessingBackend::run()
       |
       | generates all 7 fields:
       | alpha/beta/correlated (D_gb) + alpha/beta/correlated/gamma (D_ev)
       v
TensorFpreGen / TensorFpreEval
       |
       | consumed by constructors
       v
AuthTensorGen ──────────────────────────────────── AuthTensorEval
  .garble_first_half_p2()                            .evaluate_first_half_p2()
       |                                                    ^
       | chunk_levels, chunk_cts (wide: Vec<(Block,Block)>) |
       +----------------------------------------------------+
  .garble_second_half_p2()                           .evaluate_second_half_p2()
       |                                                    ^
       | chunk_levels, chunk_cts (wide)                     |
       +----------------------------------------------------+
  .garble_final_p2()
       | returns (Vec<Block> D_gb_out, Vec<Block> D_ev_out)
       |                                            .evaluate_final_p2()
       |                                               | returns (Vec<Block> D_ev_out)
       |                                               |
       +--- [v_gamma D_ev]^gb ----> c_gamma assembly (both parties)
                                            |
                                            v
                              check_zero(&c_gamma_shares, &delta_b)
                                       (D_ev verification)
```

### Recommended Project Structure

No new files. All additions go into existing modules:
```
src/
├── preprocessing.rs    # +3 new fields + rename (gamma_auth_bit_shares -> gamma_d_ev_shares)
├── tensor_ops.rs       # +gen_unary_outer_product_wide
├── auth_tensor_gen.rs  # +garble_first_half_p2, garble_second_half_p2, garble_final_p2
├── auth_tensor_eval.rs # +evaluate_first_half_p2, evaluate_second_half_p2, evaluate_final_p2
└── lib.rs              # +test_auth_tensor_product_full_protocol_2 (P2-05)
```

### Pattern 1: gen_unary_outer_product_wide — Dual Output Accumulation

**What:** Like `gen_unary_outer_product` but expands each leaf seed into two pseudorandom `Block` values (κ-half and ρ-half) via even/odd TCCR tweaks. Accumulates into two separate output matrices (`out_gb` and `out_ev`) simultaneously. Returns `Vec<(Block, Block)>` wide ciphertexts instead of `Vec<Block>`.

**When to use:** Anywhere the P2 garble path calls the garbler-side outer product (both first and second half gates).

**Key insight from `6_total.tex` Construction 4 step 3:** The wide ciphertext `G_k = (XOR_l X_{l,k}) XOR (B_k || B'_k)` where `X_{l,k}` is a `(κ+ρ)`-bit value. In the codebase this is `(row_gb XOR y_d_gb[j], row_ev XOR y_d_ev[j])` where `row_gb` accumulates the κ-halves and `row_ev` accumulates the ρ-halves. [VERIFIED: 6_total.tex Construction 4; VERIFIED: gen_unary_outer_product in tensor_ops.rs:80 as template]

**Example:**
```rust
// Source: D-03 in 09-CONTEXT.md; template from src/tensor_ops.rs:96-112
pub(crate) fn gen_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,   // B_k values (D_gb half)
    y_d_ev: &MatrixViewRef<Block>,   // B'_k values (D_ev half)
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<(Block, Block)> {           // wide ciphertexts G_k
    let m = y_d_gb.len();
    let mut gen_cts: Vec<(Block, Block)> = Vec::new();

    for j in 0..m {
        let mut row_gb = Block::default();
        let mut row_ev = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * j + i) as u128;
            let s_gb = cipher.tccr(Block::from(base << 1),     seeds[i]);
            let s_ev = cipher.tccr(Block::from(base << 1 | 1), seeds[i]);
            row_gb ^= s_gb;
            row_ev ^= s_ev;

            for k in 0..out_gb.rows() {
                if ((i >> k) & 1) == 1 {
                    out_gb[(k, j)] ^= s_gb;
                    out_ev[(k, j)] ^= s_ev;
                }
            }
        }
        row_gb ^= y_d_gb[j];
        row_ev ^= y_d_ev[j];
        gen_cts.push((row_gb, row_ev));
    }
    gen_cts
}
```

### Pattern 2: eval_unary_outer_product_wide — Dual Output Recovery

**What:** Evaluator-side counterpart to `gen_unary_outer_product_wide`. Reconstructs missing leaf's κ-half and ρ-half from the wide ciphertext, writes into both `out_gb` and `out_ev` simultaneously.

**Key:** The evaluator receives `Vec<(Block, Block)>` wide ciphertexts from the garbler. For each column j, it computes:
```
X_{a,k} = (XOR_{l!=a} X_{l,k}) XOR G_k XOR ((B_k XOR b_k*D_gb) || (B'_k XOR b_k*D_ev))
```
Split across κ-half and ρ-half. [VERIFIED: 6_total.tex Construction 4 step 4]

### Pattern 3: P2 c_gamma Assembly (Consistency Check)

**What:** Differs from Protocol 1's `assemble_c_gamma_shares`. Protocol 2 uses D_ev shares throughout.

Per `6_total.tex` step 9 and CONTEXT.md D-13:
```
[L_gamma D_ev] = [v_gamma D_ev] XOR [l_gamma D_ev]

[c_gamma]^gb = [L_gamma D_ev]^gb          (garbler's share; no correction needed)
[c_gamma]^ev = [L_gamma D_ev]^ev XOR L_gamma * D_ev   (evaluator corrects with known L_gamma)

c_gamma = [c_gamma]^gb XOR [c_gamma]^ev
        = [L_gamma D_ev]^gb XOR [L_gamma D_ev]^ev XOR L_gamma * D_ev
        = L_gamma * D_ev XOR L_gamma * D_ev = 0   (for honest parties)
```

In the in-process simulation test:
- `[v_gamma D_ev]^gb` comes from `garble_final_p2()` second return value
- `[v_gamma D_ev]^ev` comes from `evaluate_final_p2()` return value
- `[l_gamma D_ev]` comes from `gamma_d_ev_shares` on both sides
- `L_gamma` is the reconstructed masked wire value (available to the evaluator)
- Combined key for check_zero: XOR of garbler-side keys from `gamma_d_ev_shares`
- MAC recomputed as `combined_key.auth(c_gamma_bit, &delta_b)` [VERIFIED: online.rs check_zero contract; 6_total.tex Construction 5 step 9]

### Pattern 4: Input Wire D_ev Initialization (P2-05 Test)

Per `6_total.tex` Protocol 2 step 5 (encode inputs) and CONTEXT.md:
```
[v_w D_ev]^gb := [l_w D_ev]^gb          (garbler side — no change needed)
[v_w D_ev]^ev := [l_w D_ev]^ev XOR L_w * D_ev   (evaluator adds L_w correction)
```
For the single-gate test with `IdealPreprocessingBackend` (input=0), `L_w = input_w XOR l_w`. The test can initialize D_ev wire shares directly from the preprocessing fields. [VERIFIED: 6_total.tex step 5; CITED: 09-CONTEXT.md Specifics]

### Pattern 5: Preprocessing Field Extension (Atomic Commit)

The rename and three new fields must all land in a single atomic commit per D-05/D-07. This is the same "constructor atomicity" pattern established in Phase 7. Every struct constructor (`new`, `new_with_delta`, `new_from_fpre_gen`, `new_from_fpre_eval`, `into_gen_eval`) must be updated simultaneously. Leaving any constructor with `gamma_auth_bit_shares` (old name) or missing new fields causes a compile error that cannot be staged. [VERIFIED: preprocessing.rs, auth_tensor_fpre.rs, auth_tensor_gen.rs, auth_tensor_eval.rs constructors]

### Anti-Patterns to Avoid

- **Treating (Block, Block) as two independent values:** The first Block is always κ-half (D_gb), second is always ρ-half (D_ev). Never swap them.
- **Separate D_gb / D_ev matrices instead of parallel accumulation:** `gen_unary_outer_product_wide` must accumulate BOTH halves in the same loop pass over seeds. A separate function call for D_ev would redo the GGM expansion unnecessarily.
- **Using `delta_a` for P2 consistency check:** Protocol 2's `c_gamma` is D_ev-authenticated, so `check_zero` must be called with `delta_b`, not `delta_a`. Protocol 1 uses `delta_a`. [VERIFIED: 6_total.tex step 9 — "L_gamma D_ev"; VERIFIED: online.rs check_zero doc]
- **Calling `check_zero` with raw cross-party MACs:** The `check_zero` doc explicitly warns against naively XORing gen.mac and ev.mac. Always recompute MAC from the full reconstructed bit using `key.auth(bit, delta_b)`. [VERIFIED: online.rs:36]
- **Intermediate broken state on rename:** The rename from `gamma_auth_bit_shares` to `gamma_d_ev_shares` touches at least 6 files. It must compile clean — never leave a partial rename that fails to build.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| IT-MAC auth bit generation | Custom bit+key+mac assembly | `TensorFpre::gen_auth_bit()` | Handles key LSB clearing, mac = key.auth(bit, delta) invariant; already verified |
| GGM leaf PRG expansion | Custom PRG | `cipher.tccr(tweak, seed)` | Established TCCR with level-indexed tweaks; changing this would break the GGM security argument |
| IT-MAC consistency check | Custom MAC verification loop | `check_zero()` in `online.rs` | Already tested; handles both bit-zero check and MAC invariant check correctly |
| Cross-party share verification in tests | Custom invariant checks | `verify_cross_party()` in `auth_tensor_pre.rs` | Per `online.rs` test comments: calling `share.verify(delta)` directly on cross-party shares panics |

**Key insight:** The entire Phase 9 cryptographic machinery reuses existing primitives — no new cryptographic building blocks are needed. The novelty is purely in how they are combined (wide leaf expansion, dual output accumulation, D_ev-path check).

---

## Common Pitfalls

### Pitfall 1: c_gamma Check Delta Confusion (D_gb vs D_ev)

**What goes wrong:** Calling `check_zero(&c_gamma_shares, &gb.delta_a)` for the P2 consistency check, matching the P1 pattern. The check passes trivially with wrong delta or fails silently.

**Why it happens:** Protocol 1's c_gamma is D_gb-authenticated (under `delta_a`); Protocol 2's c_gamma is D_ev-authenticated (under `delta_b`). The method signatures look identical. [VERIFIED: online.rs check_zero signature; 6_total.tex step 9]

**How to avoid:** Protocol 2 consistency check always passes `&ev.delta_b` (or `&gb.delta_a`... wait — D_ev is `delta_b` which is the evaluator's delta). The verifying delta for D_ev-authenticated shares is `delta_b`. Pass `&ev.delta_b` or equivalently `&gb.delta_b` (same value, shared delta).

**Warning signs:** P2 check_zero passes even when `v_gamma != l_gamma` (wrong delta makes every share's MAC check vacuously pass or fail in the wrong direction).

### Pitfall 2: Missing l_gamma* D_ev in P2 c_gamma Assembly

**What goes wrong:** P2's `c_gamma` formula is simpler than P1's — it uses D_ev shares directly without the L_alpha / L_beta masking terms that Protocol 1 needs. Copying the P1 `assemble_c_gamma_shares` function and replacing delta_a with delta_b without understanding the formula produces incorrect c_gamma.

**Why it happens:** Protocol 1's c_gamma has 4+ terms (L_alpha · l_beta, L_beta · l_alpha, l_gamma*, l_gamma). Protocol 2's c_gamma is exactly 2 terms: `[L_gamma D_ev] := [v_gamma D_ev] XOR [l_gamma D_ev]` — no L_alpha/L_beta masking, no l_gamma* D_ev term needed independently (it is already folded into `[v_gamma D_ev]` by the wide tensor macro output). [VERIFIED: 6_total.tex step 9 — the formula is literally two-line]

**How to avoid:** Read `6_total.tex` step 9 directly for the P2 formula. Do not copy-paste P1's `assemble_c_gamma_shares`.

### Pitfall 3: Tweak Collision Between κ-half and ρ-half Expansions

**What goes wrong:** Using the same tweak for both κ-half and ρ-half TCCR calls in `gen_unary_outer_product_wide`. This makes the κ-half and ρ-half PRG outputs identical, breaking the independence between D_gb and D_ev shares.

**Why it happens:** The original `gen_unary_outer_product` uses a single tweak `seeds.len() * j + i`. When adding a second TCCR call for the ρ-half, it's tempting to reuse the same tweak.

**How to avoid:** D-03 specifies the even/odd tweak convention: `base << 1` for κ-half, `base << 1 | 1` for ρ-half. This is the same convention as `gen_populate_seeds_mem_optimized` level tweaks. [VERIFIED: CONTEXT.md D-03; tensor_ops.rs:55-56]

### Pitfall 4: gamma_auth_bit_shares Rename Scope

**What goes wrong:** Partial rename — updating `preprocessing.rs` and `auth_tensor_gen.rs` but missing the field references inside `lib.rs` test helper `assemble_c_gamma_shares` (which accesses `gb.gamma_auth_bit_shares` and `ev.gamma_auth_bit_shares` directly).

**Why it happens:** `gamma_auth_bit_shares` is accessed via struct field syntax in 3+ files plus at least one test. `cargo check` catches all instances, but a manual grep or find-replace may miss them.

**How to avoid:** Run `grep -rn "gamma_auth_bit_shares" src/` before committing. There must be zero matches after the rename.

**Warning signs:** `cargo build` reports `no field gamma_auth_bit_shares on type AuthTensorGen` in exactly the files that were not updated.

### Pitfall 5: Wide Ciphertext Eval Recovery — Wrong y Inputs

**What goes wrong:** In `eval_unary_outer_product_wide`, using only the D_gb-half of the evaluator's y shares (ignoring the D_ev half), producing correct Z_gb but incorrect Z'_gb.

**Why it happens:** The evaluator counterpart to `gen_unary_outer_product_wide` must receive both `y_d_gb` (length m) AND `y_d_ev` (length m) to reconstruct `X_{a,k} = (...) XOR (B_k XOR b_k*D_gb || B'_k XOR b_k*D_ev)`. The eval side needs both halves from the wide ciphertext. [VERIFIED: 6_total.tex Construction 4 step 4]

**How to avoid:** Eval signature must mirror garble signature in width — `y_d_gb` and `y_d_ev` are both required parameters, not just `y_d_gb`.

---

## Code Examples

Verified patterns from official sources:

### Existing gen_unary_outer_product (template for wide variant)
```rust
// Source: src/tensor_ops.rs:80-114 [VERIFIED]
// The loop structure, tweak computation, and out accumulation are identical
// in the wide variant — only the leaf expansion changes (one call -> two calls)
for j in 0..m {
    let mut row: Block = Block::default();
    for i in 0..seeds.len() {
        let tweak = (seeds.len() * j + i) as u128;
        let s = cipher.tccr(Block::from(tweak), seeds[i]);
        row ^= s;
        for k in 0..out.rows() {
            if ((i >> k) & 1) == 1 {
                out[(k, j)] ^= s;
            }
        }
    }
    row ^= y[j];
    gen_cts.push(row);
}
```

### Even/odd tweak convention (established in gen_populate_seeds_mem_optimized)
```rust
// Source: src/tensor_ops.rs:55-56 [VERIFIED]
let tweak_even = Block::from(((i as u128) << 1) as u128);
let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);
```
The wide leaf expansion reuses this exact pattern with `base = seeds.len() * j + i`.

### gen_auth_bit pattern for D_ev field generation
```rust
// Source: src/auth_tensor_fpre.rs:66-86 [VERIFIED]
// IdealPreprocessingBackend uses this for all 4 D_ev fields
let l_bit: bool = rng.random_bool(0.5);
let auth_bit: AuthBit = fpre.gen_auth_bit(l_bit);
// auth_bit.gen_share -> TensorFpreGen field
// auth_bit.eval_share -> TensorFpreEval field
```

### check_zero call with D_ev delta (P2 consistency check)
```rust
// Source: src/online.rs:52-68 [VERIFIED]; delta choice per 6_total.tex step 9
// P2 uses delta_b (D_ev), P1 used delta_a (D_gb)
assert!(check_zero(&c_gamma_shares, &ev.delta_b),
    "P2-04: honest Protocol 2 run must pass check_zero under D_ev");
```

### garble_final_p2 return type (enforces garbler privacy)
```rust
// Source: CONTEXT.md D-10 [CITED: 09-CONTEXT.md]
// Returns (D_gb_out, D_ev_out) — never a raw masked value
pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>) {
    // first Vec<Block>:  [v_gamma D_gb]^gb (length n*m)
    // second Vec<Block>: [v_gamma D_ev]^gb (length n*m)
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Protocol 1: κ-bit GGM leaves, single D_gb propagation | Protocol 2: (κ+ρ)-bit GGM leaves, dual D_gb+D_ev propagation | Phase 9 (this phase) | P2 avoids revealing masked values to garbler; enables compressed preprocessing |
| `gamma_auth_bit_shares` field name | `gamma_d_ev_shares` field name | Phase 9 rename | Consistent with the naming convention of the three new D_ev fields |
| P1 consistency check: garbler reveals L_gamma to evaluator | P2 consistency check: L_gamma stays with evaluator; garbler opens D_ev shares instead | Phase 9 | Garbler learns nothing about masked values; stronger privacy |

**Deprecated/outdated in this phase:**
- `gamma_auth_bit_shares` field name: replaced by `gamma_d_ev_shares` across all structs/methods.

---

## Runtime State Inventory

> Phase 9 is a code-only extension/rename within a Rust library crate. No databases, external services, OS-registered state, or persistent data stores are involved.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — verified by grep; no ChromaDB/Mem0/Redis usage in codebase | None |
| Live service config | None — no n8n, Datadog, or external service integration | None |
| OS-registered state | None — no Task Scheduler/pm2/launchd entries | None |
| Secrets/env vars | None — no .env files or SOPS keys in scope | None |
| Build artifacts | `target/` directory (Rust incremental build cache) — stale after rename | `cargo clean` optional; Cargo handles correctly via incremental compilation |

The rename from `gamma_auth_bit_shares` to `gamma_d_ev_shares` is a source-level change only. It has no runtime state implications beyond the standard build cache.

---

## Open Questions

1. **eval_chunked_half_outer_product_wide signature**
   - What we know: The existing `eval_chunked_half_outer_product` takes `x`, `y`, `chunk_levels`, `chunk_cts` — the wide variant needs `y_d_gb` AND `y_d_ev`, and receives `Vec<(Block, Block)>` ciphertexts instead of `Vec<Block>`.
   - What's unclear: Whether to add a `wide: bool` flag vs a completely separate `_wide` function. The existing chunked path has both garbler and evaluator versions; symmetry favors a `_wide` function pair.
   - Recommendation: Separate `eval_unary_outer_product_wide` and `eval_chunked_half_outer_product_wide` per the D-09 pattern (no new files, `_p2` or `_wide` suffix). Claude's discretion per CONTEXT.md.

2. **`first_half_out_ev` storage — where does the evaluator accumulate [v_gamma D_ev]^ev?**
   - What we know: `first_half_out` in `AuthTensorEval` currently holds the D_gb output accumulation. The P2 path also needs a D_ev accumulation matrix of the same n×m dimensions.
   - What's unclear: CONTEXT.md D-11 says `evaluate_p2()` produces D_ev output shares as part of its return value, explicitly noting "No new fields added to `AuthTensorGen` for tracking D_ev wire values; the caller receives and stores them." The same likely applies to `AuthTensorEval`.
   - Recommendation (Claude's discretion): Return `(BlockMatrix, BlockMatrix)` from `evaluate_final_p2()` — the first is D_gb output (existing `first_half_out`), the second is a freshly-constructed D_ev output matrix. Avoids adding a persistent field to the struct.

---

## Environment Availability

Step 2.6: SKIPPED — Phase 9 is a pure Rust library code change with no external tool dependencies beyond the existing `cargo` toolchain.

`cargo test` currently passes 95/95 tests. [VERIFIED: cargo test output]

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` / cargo test |
| Config file | none (standard cargo) |
| Quick run command | `cargo test 2>&1` |
| Full suite command | `cargo test 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| P2-01 | `gen_unary_outer_product_wide` produces correct D_gb and D_ev share outputs | unit | `cargo test tensor_ops::tests -x` | ❌ Wave 0 |
| P2-01 | IT-MAC invariant holds on wide output shares | unit | `cargo test tensor_ops::tests::test_wide_output_mac_invariant -x` | ❌ Wave 0 |
| P2-02 | `garble_final_p2` return type contains no masked values (checked by inspection + type) | unit | `cargo test auth_tensor_gen::tests -x` | ❌ Wave 0 |
| P2-03 | `evaluate_final_p2` returns D_ev-authenticated output shares | unit | `cargo test auth_tensor_eval::tests -x` | ❌ Wave 0 |
| P2-04 | P2 consistency check passes for honest parties | integration | `cargo test tests::test_auth_tensor_product_full_protocol_2` | ❌ Wave 0 |
| P2-04 | P1 tests remain green after P2 additions | regression | `cargo test tests::test_auth_tensor_product_full_protocol_1` | ✅ (existing) |
| P2-05 | Garbler XOR evaluator output equals correct tensor product under `_p2` path | integration | `cargo test tests::test_auth_tensor_product_full_protocol_2` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test 2>&1`
- **Per wave merge:** `cargo test 2>&1`
- **Phase gate:** Full suite green (all 95 existing + new P2 tests) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] Unit tests for `gen_unary_outer_product_wide` in `src/tensor_ops.rs` (tests module)
- [ ] Unit tests for `_p2` methods on `AuthTensorGen` and `AuthTensorEval`
- [ ] Integration test `test_auth_tensor_product_full_protocol_2` in `src/lib.rs`

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | n/a — no user auth |
| V3 Session Management | no | n/a — no sessions |
| V4 Access Control | no | n/a — library crate |
| V5 Input Validation | yes | Rust type system + `assert_eq!` dimension checks (established pattern) |
| V6 Cryptography | yes | TCCR via `FixedKeyAes`; IT-MAC via `AuthBitShare`; never hand-rolled |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Garbler revealing masked wire value to evaluator | Disclosure | Return type `(Vec<Block>, Vec<Block>)` never exposes raw L_gamma; enforced by Rust type system |
| Wrong delta in consistency check | Tampering | `check_zero` called with `delta_b` (D_ev); test must verify with correct delta |
| Tweak reuse across κ/ρ halves | Tampering | Even/odd tweak convention (D-03) ensures independence; test verifies distinct outputs |
| Cross-party MAC verification failure | Repudiation | `verify_cross_party()` (not `share.verify(delta)`) used in all test MAC checks |

**Note:** This phase implements honest-party correctness only. Malicious security proof is deferred to v2 per REQUIREMENTS.md.

---

## Sources

### Primary (HIGH confidence)
- `src/tensor_ops.rs` — `gen_unary_outer_product` (lines 80-114): direct template for wide variant [VERIFIED]
- `src/auth_tensor_gen.rs` — `AuthTensorGen` struct, all garble methods [VERIFIED]
- `src/auth_tensor_eval.rs` — `AuthTensorEval` struct, all evaluate methods [VERIFIED]
- `src/preprocessing.rs` — `TensorFpreGen`, `TensorFpreEval`, `IdealPreprocessingBackend::run()` [VERIFIED]
- `src/online.rs` — `check_zero()` signature and contract [VERIFIED]
- `src/auth_tensor_fpre.rs` — `TensorFpre::gen_auth_bit()` [VERIFIED]
- `src/lib.rs` — `test_auth_tensor_product_full_protocol_1` (P2-05 template) [VERIFIED]
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/6_total.tex` — Construction 4 (authenticated tensor macros), Construction 5 (garbling/evaluation algorithms), Protocol 2 steps 1-9 [VERIFIED]

### Secondary (MEDIUM confidence)
- `.planning/phases/09-protocol-2-garble-eval-check/09-CONTEXT.md` — All D-01..D-14 decisions [CITED]
- `.planning/phases/08-open-protocol-1-garble-eval-check/08-CONTEXT.md` — check_zero contract, c_gamma pattern from P1 [CITED]
- `.planning/phases/07-preprocessing-trait-ideal-backends/07-CONTEXT.md` — gamma_auth_bit_shares field pattern [CITED]

---

## Assumptions Log

> All claims in this research were verified against the actual codebase source files or the canonical spec (`6_total.tex`).

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `delta_b` is the D_ev delta (evaluator's delta) in this codebase | Standard Stack, Pitfall 1 | check_zero called with wrong delta; consistency check passes vacuously [ASSUMED based on naming convention in preprocessing.rs/auth_tensor_eval.rs — `delta_b: Delta` on `AuthTensorEval`] |

**All other claims in this research were verified against source files or the spec.** Only A1 is [ASSUMED] — it is consistent with the codebase naming pattern (`delta_a` on gen side, `delta_b` on eval side) and with `TensorFpreEval.delta_b` field in `preprocessing.rs`, but was inferred from naming rather than a single definitive doc comment.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all primitives verified in source
- Architecture: HIGH — both spec (6_total.tex) and existing codebase patterns verified; Construction 4/5 read in full
- Pitfalls: HIGH — identified from spec cross-reading, existing test patterns, and CONTEXT.md explicit warnings

**Research date:** 2026-04-24
**Valid until:** Stable — this is a pure Rust codebase with locked design decisions; no external dependency churn expected
