# AUDIT 2.4 — Protocol 2 (wide-leaf with D_ev MAC propagation)

## Scope

**Paper:**
- Authenticated tensor macros (`cons:auth-tensor-macro`): `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/6_total.tex:25-86`, with correctness lemma `lem:auth-tensor-macro-correctness` at `:88-110`.
- Wide-leaf XOR-share definition: `6_total.tex:43, :77-79` (`(κ + ρ)`-bit `X_{ℓ,k}`; `G_k = (B_k ‖ B'_k)`).
- Communication-cost claim: `6_total.tex:112` — "communication overhead over `tensorgb` is `ρm` bits per call."
- Garbling/Evaluation algorithms (`cons:wrk-algo`): `6_total.tex:119-176`.
- Protocol `Π_2pc^t,auth` (`prot:wrk`): `6_total.tex:182-229`.
- Correctness lemma (`lem:protocol2-correctness`): `6_total.tex:231+`.
- CheckZero on `c_α / c_β`: `6_total.tex:215-222`.

**Code:**
- Wide GGM helpers: `src/tensor_ops.rs::gen_unary_outer_product_wide` (`:289-330`) and `eval_unary_outer_product_wide` (`:342-396`); `WIDE_DOMAIN` constant (`:267-273`).
- Garbler: `src/auth_tensor_gen.rs::*_p2` — `gen_chunked_half_outer_product_wide`, `garble_first_half_p2`, `garble_second_half_p2`, `garble_final_p2`; D_ev input setup `get_first_inputs_p2_y_d_ev`, `get_second_inputs_p2_y_d_ev` (`:148-216, 317-434`).
- Evaluator: `src/auth_tensor_eval.rs::*_p2` — mirror of the garbler with `eval_chunked_half_outer_product_wide`, `evaluate_first_half_p2`, `evaluate_second_half_p2`, `evaluate_final_p2` (`:181-257, 311-485`).
- Input-wire CheckZero: `src/lib.rs::assemble_c_alpha_beta_blocks_p2` (`:155-178`) — thin alias for `assemble_e_input_wire_blocks_p1` (P1 helper) per the paper's algebraic equivalence between `e_a/e_b` and `c_α/c_β`.
- End-to-end test: `src/lib.rs::run_full_protocol_2` (`:651-738`).

**Out of scope (covered by other audits):**
- Tree-level GGM primitives `gen_populate_seeds_mem_optimized` / `eval_populate_seeds_mem_optimized` — AUDIT-2.1. Wide variant reuses these unchanged for the level tree; inherits AUDIT-2.1 B1 (HK21 2-ct shape) and B2 (extra init TCCR).
- Preprocessing (`TensorFpreGen` / `TensorFpreEval` field semantics, F_pre / F_cpre invocation) — AUDIT-2.2. Note: `Π_2pc^t,auth` uses `F_cpre` (compressed preprocessing) not `F_pre`; current code uses the same `IdealPreprocessingBackend` for both protocols (compressed-preprocessing distinction not yet reflected in the type system).
- P1 input encoding + per-gate scaffolding — AUDIT-2.3. Code's `_p2` variants share `encode_inputs` with P1; same B3 (collapsed encoding) and B1 (per-gate not per-circuit, no `open()` primitive) findings apply.

## (a) Matches

This audit focuses on what's distinctive about Protocol 2; tree-level GGM (AUDIT-2.1) and per-gate orchestration (AUDIT-2.3) are referenced but not re-traced. Three sub-sections:

1. Wide leaf expansion + Z accumulation (where `cons:auth-tensor-macro` differs from `cons:tensor-macro`).
2. P2 final-combine with dual-delta accumulators (`cons:wrk-algo` `:140-141`).
3. P2 input-wire CheckZero (`prot:wrk` `:215-222`) — alias for P1 helper.

### Wide leaf expansion + Z accumulation

`AuthTensor.Gb` step 5-7 / `AuthTensor.Ev` step 4-6 (`6_total.tex:43-54, :76-84`).

| Paper step | Code | Match |
|---|---|---|
| Tree levels (`AuthTensor.Gb` steps 1-4): identical to Construction 1, with `R_{i,j} = H(S_{i-1,j}, ν_{i,j})`, FreeXOR sibling, `G_i = ⊕_j R_{i,j} ⊕ A_i` (`6_total.tex:29-42`) | `gen_unary_outer_product_wide` reuses `gen_populate_seeds_mem_optimized` for the tree expansion (`tensor_ops.rs:289-296` calls levels-side via the chunked wrappers in `auth_tensor_gen.rs:195-199`) | ✗ inherits AUDIT-2.1 B1 (HK21 2-ct-per-level) and B2 (extra init TCCR) — see B2 below |
| Wide leaf expansion: `X_{ℓ,k}` is `(κ + ρ)`-bit; `G_k := (⊕_ℓ X_{ℓ,k}) ⊕ (B_k ‖ B'_k)` (`6_total.tex:43-46`) | `s_gb = TCCR(WIDE_DOMAIN \| (base<<1), seeds[i])`; `s_ev = TCCR(WIDE_DOMAIN \| (base<<1\|1), seeds[i])` per leaf — TWO independent κ-bit TCCR outputs; `gen_cts.push((row_gb, row_ev))` (`tensor_ops.rs:308-326`) | ✗ deviates — see B1 (`(Block, Block) = 2κ` bits vs paper's `κ + ρ` bits) |
| Domain separation between wide and narrow tweaks (`tensor_ops.rs:267-273`): `WIDE_DOMAIN = 1 << 64` reserves bit 64 to ensure wide tweaks never collide with narrow tweaks regardless of input | (No paper requirement — code-internal hardening) | ✓ positive — wide path resolves AUDIT-2.1 C2's "input-distinctness" concern via explicit tweak-domain separation |
| Wide Z accumulation: `(Z_gb ‖ Z'_gb) := truthtable(I)^T · X_gb` — Z_gb is the κ-prefix, Z'_gb is the ρ-suffix (`6_total.tex:50-54`) | Two output matrices `out_gb` and `out_ev`: `if ((i >> k) & 1) == 1 { out_gb[(k,j)] ^= s_gb; out_ev[(k,j)] ^= s_ev; }` (`tensor_ops.rs:316-323`) | ✓ matches semantically; the prefix/suffix concatenation is split into two parallel matrices keyed by κ-half and ρ-half tweaks |
| Eval-side missing-leaf recovery: `X_{α,k}^ev := ⊕_{ℓ ≠ α} X_{ℓ,k}^ev ⊕ G_k ⊕ ((B_k ⊕ b_k δ_gb) ‖ (B'_k ⊕ b_k δ_ev))` (`6_total.tex:79`) | `eval_ct_gb ^= gen_cts[j].0 ^ y_d_gb[j]; eval_ct_ev ^= gen_cts[j].1 ^ y_d_ev[j]` per column (`tensor_ops.rs:381-382`); both eval_cts distributed to rows where `bit_k(missing) = 1` | ✓ matches with the same split (κ-half handles `(B_k ⊕ b_k δ_gb)`, ρ-half handles `(B'_k ⊕ b_k δ_ev)`) |

### P2 final-combine with dual-delta accumulators

`cons:wrk-algo` `:138-142, :164-170`.

| Paper step | Code | Match |
|---|---|---|
| Garbler combine: `[c δ_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a⊗λ_b) δ_gb]`; `[c δ_ev] := Z'_{c,0} ⊕ (Z'_{c,1})^T ⊕ [(λ_a⊗λ_b) δ_ev]` (`:140-141`) | `garble_final_p2`: parallel D_gb and D_ev folds — `first_half_out[(i,j)] ^= second_half_out[(j,i)] ^ correlated_gen[j*n+i]` (D_gb path), then `first_half_out_ev[(i,j)] ^= second_half_out_ev[(j,i)] ^ correlated_eval[j*n+i]` (D_ev path) (`auth_tensor_gen.rs:400-419`) | ✓ matches; D_ev third term uses `correlated_eval` (the δ_ev-keyed share per project's Block-form convention) |
| Garbler privacy (paper §16): "The masked values revealed to the evaluator are never sent to the garbler" | `garble_final_p2` returns `(Vec<Block>, Vec<Block>)` with no `bool` / `Vec<bool>` field (`auth_tensor_gen.rs:381-388, :423-433`); doc comment explicitly cites this as compile-time enforcement of P2 privacy | ✓ matches; static return type encodes the privacy property |
| Garbler emits `correlated_eval` directly (no δ_b XOR — gb does not hold δ_b); evaluator XORs δ_b on its key view to reconstruct the IT-MAC pair | Garbler reads `correlated_eval[j*n+i]` directly; eval reads its own `correlated_eval[j*n+i]` (`auth_tensor_eval.rs:466-472`). Block-form fields embed the δ_b adjustment at preprocessing time via `derive_sharing_blocks` (per AUDIT-2.2 cross-cutting facts), not at consumption time | ✓ matches functionally; doc comment at `auth_tensor_gen.rs:386-388` references the pre-§1.2 `mac.as_block()` API and is now stale — see C2 |
| Evaluator combine: `[c δ_gb]^ev := Z_{c,0}^ev ⊕ (Z_{c,1}^ev)^T ⊕ [(λ_a⊗λ_b) δ_gb]^ev`; `[c δ_ev]^ev` symmetric (`:168-170`) | `evaluate_final_p2`: identical structural form to gen-side but reads its own `correlated_gen` and `correlated_eval` (`auth_tensor_eval.rs:457-473`); also returns `Vec<Block>` of length `n·m` (the eval's `[c δ_ev]^ev` share vector) (`:475-485`) | ✓ matches |

### P2 input-wire CheckZero

`prot:wrk` `:215-222`.

| Paper step | Code | Match |
|---|---|---|
| Per tensor gate: `[c_α δ_ev] := [v_a δ_ev] ⊕ [λ_a δ_ev] ⊕ (a ⊕ λ_a)·δ_ev`; `[c_β δ_ev]` symmetric (length `m`) | `assemble_c_alpha_beta_blocks_p2(n, m, gb_v_alpha_eval, ev_v_alpha_eval, gb_v_beta_eval, ev_v_beta_eval, l_alpha_pub, l_beta_pub, gb, ev)` is a thin alias for `assemble_e_input_wire_blocks_p1` (`lib.rs:155-178`) | ✓ matches algebraically — paper's P1 `e_a/e_b` and P2 `c_α/c_β` formulas are identical three-term XOR; aliasing avoids duplicating logic; see C1 (alias-coupling latent assumption) |
| `CheckZero({[c_α δ_ev], [c_β δ_ev]})`; ev aborts if any non-zero | `block_check_zero(c_gen_blocks_p2, c_eval_blocks_p2)` returns `false` on any per-index mismatch (`lib.rs:734-737`) | ✓ matches; uses the simulation form, not the paper-faithful `block_hash_check_zero` digest — same as AUDIT-2.3 C1 |

### P2-specific input-encoding setup (input handoff to wide tensor)

`AuthTensor.Gb` requires `[b δ_gb]` AND `[b δ_ev]` for the second input. P1 only required `[b δ_gb]`. The δ_ev input is supplied via `get_first_inputs_p2_y_d_ev` and `get_second_inputs_p2_y_d_ev` on each side.

| Paper input | Code | Match |
|---|---|---|
| First-half: `b = λ_b = β`; gb's `[β δ_ev]^gb = β_eval`; ev's `[β δ_ev]^ev = β_eval` | `auth_tensor_gen.rs:317-323` returns `gar.beta_eval` for first-half y_d_ev; `auth_tensor_eval.rs:318-324` returns `ev.beta_eval` symmetrically — no L_b correction needed because β IS λ_b directly (β is preprocessed and never revealed during encode) | ✓ matches paper's `[λ_b δ_ev]` shape |
| Second-half: `b = a = x`; gb's `[x δ_ev]^gb := [λ_a δ_ev]^gb` (paper §211 P1 path inherited); ev's `[x δ_ev]^ev := [λ_a δ_ev]^ev ⊕ (x ⊕ λ_a)·δ_ev` | `auth_tensor_gen.rs:332-338` returns `gar.alpha_eval` for second-half y_d_ev; `auth_tensor_eval.rs:334-345` returns `ev.alpha_eval[i] ⊕ (masked_x_bits[i] ? δ_b : 0)` — explicit L_a · δ_b correction on the eval side | ✓ matches paper's `[v_a δ_ev]^ev` formula |

### Correctness invariant verification

Paper Lemma `lem:auth-tensor-macro-correctness` (`6_total.tex:88-110`): `Z_gb ⊕ Z_ev = (a⊗b)·δ_gb` AND `Z'_gb ⊕ Z'_ev = (a⊗b)·δ_ev`.

Code analytically reduces to: per `(k, j)`, `out_gb[(k,j)]_gen ⊕ out_gb[(k,j)]_eval = a_k · b_j · δ_gb` (κ-half) and `out_ev[(k,j)]_gen ⊕ out_ev[(k,j)]_eval = a_k · b_j · δ_ev` (ρ-half-stored-as-κ). Verified by the same analytic argument as AUDIT-2.1 with the wide path's two parallel TCCR streams.

Existing tests verify the κ-half (D_gb path) end-to-end via `verify_tensor_output` in `run_full_protocol_2` (`lib.rs:678-681`) and the input-wire CheckZero passes under δ_b (`:734-737`). The ρ-half (D_ev output reconstruction `Z'_gb ⊕ Z'_ev = (a⊗b)·δ_ev`) is **not** directly asserted in the test — see B3 below.

## (b) Deviations

### B1 — Wide leaf ciphertext shape uses `2 × Block` (= `2κ` bits) instead of paper's `(κ + ρ)`-bit width

**Paper (`6_total.tex:43-46, :112`):**
- `X_{ℓ,k}` is a `(κ + ρ)`-bit seed produced via `H` with one-time nonces.
- `G_k := (⊕_ℓ X_{ℓ,k}) ⊕ (B_k ‖ B'_k) ∈ {0,1}^{κ+ρ}` — single concatenated ciphertext per leaf-column.
- "The communication overhead over `tensorgb` is `ρm` bits per call, from widening the m leaf ciphertexts `{G_k}` from `κ` to `κ + ρ` bits."

**Code (`src/tensor_ops.rs:289-330` `gen_unary_outer_product_wide`):**
```rust
let s_gb = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1)),     seeds[i]);
let s_ev = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i]);
…
gen_cts.push((row_gb, row_ev));
```
- Each leaf produces TWO independent `Block` (κ-bit) outputs via even/odd tweak split — κ-half (D_gb path) + ρ-half (D_ev path).
- Wide leaf cts stored as `Vec<(Block, Block)>` (`tensor_ops.rs:296`); each entry is `2 × κ = 2κ` bits.

**Cost mismatch:**
- Paper: per `AuthTensor.Gb` call, leaf cts = `m × (κ + ρ)` bits. Overhead vs P1's `tensorgb`: `m × ρ`.
- Code: per call, leaf cts = `m × 2κ` bits. Overhead vs P1: `m × κ`.
- With typical `κ = 128, ρ = 40`: paper claims `40m`-bit leaf overhead; code emits `128m`-bit overhead — **3.2× the paper's claim** at the leaf level.

**Combined with AUDIT-2.1 B1** (which also doubles tree-level cts vs the paper's improved one-hot): total wide-tensor communication cost is **substantially higher than paper's claimed overhead**. `Π_2pc^t,auth`'s headline communication advantage over `Π_2pc^t` (P1) is therefore not realized in the current code.

**Correctness:** the protocol invariant `Z_gb ⊕ Z_eval = (a⊗b)·δ_gb` and `Z'_gb ⊕ Z'_eval = (a⊗b)·δ_ev` (paper Lemma `lem:auth-tensor-macro-correctness`) holds — the (Block, Block) split is algebraically equivalent to `H(Label, ν) ↦ (κ-prefix ‖ ρ-suffix)` with the ρ-half oversized to κ bits, so reconstruction works regardless. Existing wide round-trip tests (`tensor_ops.rs::tests::test_eval_unary_outer_product_wide_round_trip_{kappa,rho}`) verify this.

**Required fix (queued as (d)):** add a `RhoBlock`-like type (or a packed `[u8; (κ+ρ)/8]`) to represent the ρ-half at its true paper width. Update `gen_unary_outer_product_wide` / `eval_unary_outer_product_wide` to emit `(Block, RhoBlock)` pairs. Cost is significantly larger than just changing the type: `WIDE_DOMAIN` tweak structure may need to change (currently emits a full Block from a single TCCR call); ρ might need to come from a truncated Block or a different PRG. Couple with AUDIT-2.1 D1 (paper's improved one-hot) since both are wide-tensor communication-cost fixes.

### B2 — Wide-tensor primitives inherit AUDIT-2.1 B1 + B2 (HK21 2-ct-per-level + extra init TCCR) at the tree level

Wide-tensor primitives reuse `gen_populate_seeds_mem_optimized` / `eval_populate_seeds_mem_optimized` for the level tree (called from `auth_tensor_gen.rs:195-199` and `auth_tensor_eval.rs:237-243`). The wide-leaf changes are confined to the leaf-expansion stage; tree internals are byte-for-byte identical to P1.

**Cumulative communication mismatch:** combining B1 (this audit, leaf cts `2κ` not `κ+ρ`) with AUDIT-2.1 B1 (tree-level cts `2(n-1)` not `n-1`):
- Paper `Π_2pc^t,auth` per-call cost: `(n-1)·κ + m·(κ+ρ)` bits.
- Code per-call cost: `2(n-1)·κ + m·2κ` bits.
- At typical `κ=128, ρ=40, n=4, m=4`: paper = `384 + 672 = 1056` bits/call; code = `768 + 1024 = 1792` bits/call (~70% higher).

**Required fix (queued as (d)):** subsumed by AUDIT-2.1 D1 + this audit's D1. Both rewrites must land jointly to realize paper's claimed wide-tensor communication cost.

### B3 — `run_full_protocol_2` does not assert D_ev output reconstruction `Z'_gb ⊕ Z'_ev = (a⊗b)·δ_ev`

SCAFFOLDING-flagged. Paper Lemma `lem:auth-tensor-macro-correctness` (`6_total.tex:88-110`) gives **both** `Z_gb ⊕ Z_ev = (a⊗b)·δ_gb` (κ-half) AND `Z'_gb ⊕ Z'_ev = (a⊗b)·δ_ev` (ρ-half).

**Test (`src/lib.rs:651-738`) only verifies:**
- `verify_tensor_output(x, y, n, m, &gb.first_half_out, &ev.first_half_out, &gb.delta_a)` — D_gb reconstruction (`:678-681`).
- `block_check_zero(c_gen_blocks_p2, c_eval_blocks_p2)` — input-wire CheckZero under δ_ev (`:734-737`).

**Missing:** an analogous `verify_tensor_output(..., &gb.first_half_out_ev, &ev.first_half_out_ev, &gb.delta_b)` over the D_ev output matrices. Without it, a regression that silently corrupts `correlated_eval` flow into `first_half_out_ev` would pass tests as long as the input-wire CheckZero stays consistent — leaving `Z'_gb ⊕ Z'_ev = (a⊗b)·δ_ev` unverified end-to-end.

**Required fix (queued as (d)):** add D_ev output assertion to `run_full_protocol_2` and the non-zero-input variants. Trivial — one extra `verify_tensor_output(...)` call pointing at the `_ev` matrices under `gb.delta_b`.

### B4 — F_cpre / F_pre distinction not reflected in the type system

**Paper:**
- `Π_2pc^t` (P1) realizes `F_2pc` in the `(F_pre, F_eq)`-hybrid model (`5_online.tex:3`).
- `Π_2pc^t,auth` (P2) realizes `F_2pc` in the `(F_cpre, F_eq)`-hybrid model (`6_total.tex:6`).
- F_cpre outputs the same correlations as F_pre but with **lower-entropy evaluator shares** (compressed preprocessing — `6_total.tex:7-12`).
- Lower-entropy ev shares motivate P2's dual-δ propagation: revealing masked values to gb (as in P1 consistency check) would leak entropy from F_cpre's compressed shares.

**Code:**
- `IdealPreprocessingBackend::run(n, m, cf)` returns `(TensorFpreGen, TensorFpreEval)` consumed by both P1 (`AuthTensorGen::new_from_fpre_gen` + `garble_*` / `evaluate_*`) and P2 (`+ _p2` methods).
- No type-level distinction between "compressed" and "uncompressed" preprocessing output.
- `run_full_protocol_1` and `run_full_protocol_2` both call `backend.run(...)` identically (`src/lib.rs`).
- `IdealCompressedPreprocessingBackend` (PRE-05) is deferred to v3 per `STATE.md` Deferred Items list / ROADMAP.

**Implication:** today's P2 code path is paper-compatible (consumes valid F_pre output, which is a strict superset of F_cpre output) but not paper-faithful at the API boundary. Once F_cpre lands, the type system should distinguish the two so callers can't accidentally feed F_cpre output into P1 (where the missing entropy compromises security).

**Required fix (queued as (d)):** when `IdealCompressedPreprocessingBackend` lands (PRE-05, v3), introduce a marker type or `Compressed` / `Uncompressed` tag on `TensorFpreGen` to prevent cross-protocol misuse. Documentation-only fix until then: comment in `IdealPreprocessingBackend::run` that current output is uncompressed (suitable for both protocols, not paper-faithful for P2).

## (c) Latent assumptions

### C1 — `assemble_c_alpha_beta_blocks_p2` is a thin alias for `assemble_e_input_wire_blocks_p1`; alias-coupling latent assumption

**Code (`src/lib.rs:155-178`):** `assemble_c_alpha_beta_blocks_p2` body is a single forwarding call to `assemble_e_input_wire_blocks_p1`.

**Doc comment (`:142-153`):** cites the paper's algebraic equivalence:
- P1 `e_a := [v_a δ_ev] ⊕ [λ_a δ_ev] ⊕ (a⊕λ_a)·δ_ev` (paper §242).
- P2 `c_α := [v_α δ_ev] ⊕ [λ_α δ_ev] ⊕ L_α·δ_ev` (paper §218).
The alias keeps the paper-mapped name at P2 call sites without code duplication.

**Latent assumption:** the alias depends on the helper's algebra remaining identical between protocols. The paper's equivalence is invariant under chunking but **not** invariant under compressed preprocessing, where `[λ_α δ_ev]` may live in a different sharing form on the eval side. Once F_cpre lands (see B4), the alias likely breaks and needs a P2-specific implementation that consumes the compressed share representation directly.

**Required fix (queued as (d)):** when F_cpre lands, audit whether the alias still satisfies P2's algebra; un-alias if not. Documentation-only until then: tighten the `assemble_c_alpha_beta_blocks_p2` doc comment to call out the F_cpre coupling.

### C2 — Stale doc comment in `garble_final_p2` references `mac.as_block()` after fields were refactored to Block-form

**Doc comment (`src/auth_tensor_gen.rs:385-388`):**
> "D_ev encoding rule (garbler side): the garbler does NOT hold `delta_b`, so its public-bit encoding of `correlated_eval[idx]` is simply `mac.as_block()` — no `delta_b` XOR. See `get_first_inputs_p2_y_d_ev` doc for derivation."

**Reality:** the reference to `mac.as_block()` is pre-§1.2. At that time, `correlated_eval` was an `AuthBitShare` (with separate `mac`, `key`, `value` fields), and the doc described how to extract the MAC component. Post-§1.2, `correlated_eval` is a Block-form field (single `Block` per entry), and the code reads it directly as a Block. The doc's `mac.as_block()` reference no longer maps to anything in the current code.

**Required fix (queued as (d)):** rewrite the `garble_final_p2` doc comment to describe the Block-form D_ev encoding without the stale `mac.as_block()` reference. Trivial documentation cleanup.

## (d) Required code changes

Queued as a follow-up sub-phase. Each item is a separate atomic commit per the Track 2 interaction model. Numbering reflects suggested execution order.

| # | Source finding | Scope | Notes |
|---|---|---|---|
| D1 | B1 + B2 | Land paper-faithful wide leaf cts: introduce `RhoBlock` (or a packed `[u8; (κ+ρ)/8]`) representing the ρ-half at true paper width; update `gen_unary_outer_product_wide` / `eval_unary_outer_product_wide` to emit `(Block, RhoBlock)`; revisit `WIDE_DOMAIN` tweak structure (single TCCR call may need a different PRG when ρ < κ); thread the new shape through wide chunked wrappers (`gen_chunked_half_outer_product_wide`, `eval_chunked_half_outer_product_wide`) and `_p2` orchestrators; update bench fixtures and tests. | Coupled with AUDIT-2.1 D1 (paper's improved one-hot for tree levels). Both rewrites must land jointly to realize paper's claimed wide-tensor communication cost. |
| D2 | B3 | Add `verify_tensor_output(x, y, n, m, &gb.first_half_out_ev, &ev.first_half_out_ev, &gb.delta_b)` to `run_full_protocol_2` and the non-zero-input variants `test_full_protocol_2_nonzero_inputs_{ideal,uncompressed}`. | Trivial; can land standalone today. Closes the SCAFFOLDING-flagged D_ev output coverage gap. |
| D3 | B4 | When `IdealCompressedPreprocessingBackend` (PRE-05, v3) lands: introduce a marker type or `Compressed`/`Uncompressed` tag on `TensorFpreGen` / `TensorFpreEval` to prevent cross-protocol misuse. Trait `TensorPreprocessing::run` may need to split into `run_uncompressed` / `run_compressed` returning marker-typed outputs. | Gated on PRE-05 landing first; deferred to v3 per ROADMAP. Documentation-only fix until then: note in `IdealPreprocessingBackend::run` that current output is uncompressed. |
| D4 | C1 | When F_cpre lands (D3 / PRE-05): audit whether `assemble_c_alpha_beta_blocks_p2` still satisfies P2's algebra under compressed preprocessing; un-alias from `assemble_e_input_wire_blocks_p1` if not. | Coupled with D3. Documentation-only until then: tighten doc comment to call out F_cpre coupling. |
| D5 | C2 | Rewrite `garble_final_p2` doc comment (`auth_tensor_gen.rs:385-388`) to describe the Block-form D_ev encoding without the stale `mac.as_block()` reference. | Trivial documentation cleanup; can land standalone. |

### Coordination notes

- **D1 must land with AUDIT-2.1 D1.** Wide-tensor's tree-level + leaf-cts rewrites are tightly coupled — both consume the same primitives and bench fixtures; partial fix would leave the cumulative cost asymmetry in place.
- **D2 is the smallest blast radius.** Closes the SCAFFOLDING-flagged D_ev output coverage gap with no other dependencies; can land at any time.
- **D3 + D4 are paired with the PRE-05 v3 work** (`IdealCompressedPreprocessingBackend`). Documentation-only fix in current codebase; full fix deferred.
- **D5 can land with any Block-form-fields touch** — pure doc cleanup.

## Track 2 closure

This is the final per-surface audit. Closing summary across all four:

| Surface | Doc | Commit | Major (b) findings |
|---|---|---|---|
| 2.1 | `AUDIT-2.1.md` | `a6b5e8b` | HK21 2-ct-per-level vs paper's improved one-hot |
| 2.2 | `AUDIT-2.2.md` | `c23e785` | Π_pre single-bucket only; preprocessing has no chunking |
| 2.3 | `AUDIT-2.3.md` | `9ede423` | Per-gate not per-circuit; output-decode `open()` deferred |
| 2.4 | `AUDIT-2.4.md` | (this commit) | Wide leaf cts `2κ` not `κ+ρ`; F_cpre / F_pre type-system non-distinction |

(d) sub-phase items are now fully cataloged across all four audits and ready to be planned. Suggested execution order:
1. **AUDIT-2.1 D1 + AUDIT-2.3 D3 + AUDIT-2.4 D1** (paper's improved one-hot rewrite + chunking-wrapper signature update + wide-leaf shape fix) — load-bearing for paper-faithful communication cost.
2. **AUDIT-2.4 D2** (D_ev output assertion) — independent; closes coverage gap.
3. **AUDIT-2.1 D4 + D5 + AUDIT-2.4 D5** (doc/assertion tweaks across the codebase) — independent; can land in any order.
4. **AUDIT-2.2 D1+D2** (batched Π_pre + chunked preprocessing) — gated on AUDIT-2.1 D1 to align chunking factors.
5. **AUDIT-2.3 D1+D2** (circuit orchestrator + `open()` primitive) — major scope; defer until protocol-2-demonstration goals require it.
6. **AUDIT-2.2 D3+D4 + AUDIT-2.3 D4+D5 + AUDIT-2.4 D3+D4** (multi-process transition + F_cpre marker types) — paired with v2/v3 multi-process and compressed-preprocessing work; deferred.
