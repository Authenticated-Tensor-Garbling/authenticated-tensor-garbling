# Phase 9: Protocol 2 Garble/Eval/Check - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the online phase to Protocol 2 — the "authenticated" variant from `6_total.tex`. The GGM tree leaves expand to (κ+ρ) bits so the garbled circuit propagates D_ev-MACs alongside D_gb-shares. The garbler never reveals masked wire values; the evaluator performs the consistency check locally. Implements P2-01 through P2-05.

**Active requirements:** P2-01, P2-02, P2-03, P2-04, P2-05

</domain>

<decisions>
## Implementation Decisions

### Wide Leaf Type (P2-01)

- **D-01:** `gen_unary_outer_product_wide` uses `(Block, Block)` tuples — no new types. The pair represents the (κ ‖ ρ)-bit concatenation of one wide value, not two independent values. First `Block` = κ-prefix (feeds Z_gb), second `Block` = ρ-suffix (feeds Z'_gb). Wide ciphertexts returned as `Vec<(Block, Block)>`.
- **D-02:** Both κ and ρ are 128 bits in this codebase: `Delta` is a newtype over `Block([u8; 16])` for both `delta_a` (D_gb) and `delta_b` (D_ev). κ+ρ = 256 bits = two `Block` values.
- **D-03:** Wide leaf expansion uses the same even/odd tweak convention as `gen_populate_seeds_mem_optimized`:
  ```
  base = seeds.len() * j + i
  kappa_half = cipher.tccr(Block::from(base << 1),     seeds[i])
  rho_half   = cipher.tccr(Block::from(base << 1 | 1), seeds[i])
  ```
  Two TCCR calls per (leaf, column) pair. Consistent with the existing GGM tree convention.

### D_ev Preprocessing Fields (P2-01 / P2-02 / P2-03)

- **D-04:** `TensorFpreGen` and `TensorFpreEval` each get **three new fields**:
  - `alpha_d_ev_shares: Vec<AuthBitShare>` — D_ev-authenticated shares of l_alpha, length n
  - `beta_d_ev_shares: Vec<AuthBitShare>` — D_ev-authenticated shares of l_beta, length m
  - `correlated_d_ev_shares: Vec<AuthBitShare>` — D_ev-authenticated shares of l_gamma*, length n*m, column-major
- **D-05:** `gamma_auth_bit_shares` (added in Phase 7) is **renamed** to `gamma_d_ev_shares` for consistency with the new D_ev field naming convention. All call sites in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, and `lib.rs` tests must be updated in the same commit.
- **D-06:** `IdealPreprocessingBackend::run()` generates all four D_ev fields using `TensorFpre::gen_auth_bit()` per entry — same pattern as `gamma_d_ev_shares` generation in Phase 7.
- **D-07:** All existing constructors of `TensorFpreGen` / `TensorFpreEval` must initialize the three new fields (and use the renamed `gamma_d_ev_shares`) in the same commit — no intermediate broken state. Follows Phase 7 D-06 pattern.
- **D-08:** On the garbler side, `TensorFpreGen.beta_d_ev_shares` holds `[l_beta D_ev]^gb` (garbler's IT-MAC shares of l_beta under D_ev). On the evaluator side, `TensorFpreEval.beta_d_ev_shares` holds `[l_beta D_ev]^ev`. Same symmetric layout as all other field pairs across Gen/Eval.

### _p2 Function Placement (P2-02 / P2-03 / P2-04)

- **D-09:** Protocol 2 garble and evaluate are **new methods on `AuthTensorGen` and `AuthTensorEval`** respectively, with `_p2` suffix on the method names — e.g., `garble_first_half_p2()`, `garble_second_half_p2()`, `garble_final_p2()` on `AuthTensorGen`. Mirrors the P1 method naming pattern. No new files or modules.
- **D-10:** `garble_final_p2()` returns **`(Vec<Block>, Vec<Block>)`** — the first `Vec<Block>` is the D_gb output share (`[v_gamma D_gb]^gb`), the second is the D_ev output share (`[v_gamma D_ev]^gb`). No new fields added to `AuthTensorGen` for tracking D_ev wire values; the caller receives and stores them.
- **D-11:** `evaluate_p2()` (or its split equivalents) produces D_ev-authenticated output wire shares `[v_gamma D_ev]^ev` as part of its return value, alongside the D_gb shares it already produces.

### P2 Consistency Check (P2-04)

- **D-12:** The Protocol 2 consistency check (from `6_total.tex` step 9) uses the existing `check_zero()` primitive in `src/online.rs` — no new check_zero variant. The caller assembles `c_gamma` shares from the D_ev-authenticated wire value shares and calls `check_zero()` with `delta_b` (D_ev) as the verifying delta.
- **D-13:** `c_gamma` assembly for Protocol 2:
  - Both parties compute `[L_gamma D_ev] := [v_gamma D_ev] XOR [l_gamma D_ev]` (using `gamma_d_ev_shares` from preprocessing)
  - Evaluator's share: `[c_gamma]^ev := [L_gamma D_ev]^ev XOR L_gamma * D_ev` (evaluator XORs with known masked value × delta_ev)
  - Garbler's share: `[c_gamma]^gb := [L_gamma D_ev]^gb`
  - Combined: `c_gamma = [c_gamma]^gb XOR [c_gamma]^ev = 0` for honest parties

### P2 End-to-End Test (P2-05)

- **D-14:** Single tensor gate test, mirroring P1-04. One `AuthTensorGen` + one `AuthTensorEval`, driven by `IdealPreprocessingBackend`. Verifies garbler XOR evaluator output == correct tensor product under the `_p2` path. `cargo test` must also pass all existing Protocol 1 tests unchanged.

### Claude's Discretion

- Exact `v_alpha D_ev` initialization for input wires in the single-gate test: per `6_total.tex`, set `[v_w D_ev]^gb := [l_w D_ev]^gb` on input wires (garbler side) and `[v_w D_ev]^ev := [l_w D_ev]^ev XOR L_w * D_ev` on evaluator side.
- Whether to split `evaluate_p2` into `evaluate_first_half_p2`/`evaluate_second_half_p2`/`evaluate_final_p2` — follow whatever structure `garble_*_p2` takes for symmetry.
- Exact parameter names for the new `_p2` methods — follow existing method conventions.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification

- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/6_total.tex` — Construction 4 (Authenticated tensor macros: `AuthTensor.Gb` / `AuthTensor.Ev`), Construction 5 (Garbling and Evaluation Algorithms for Protocol 2), Protocol 2 full 2PC protocol including consistency check. **Primary spec for this phase.**
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Protocol 1 reference; Construction 3 defines the tensor macro that Protocol 2 extends.

### Key Source Files

- `src/tensor_ops.rs` — `gen_unary_outer_product` (lines 80–114): the function being extended to the wide `_wide` variant. Study its tweak and accumulation logic before writing `gen_unary_outer_product_wide`.
- `src/auth_tensor_gen.rs` — `AuthTensorGen`. Existing `garble_first_half`, `garble_second_half`, `garble_final`, `compute_lambda_gamma` methods. The `_p2` methods go here.
- `src/auth_tensor_eval.rs` — `AuthTensorEval`. Existing evaluate methods. The `_p2` evaluate methods go here.
- `src/preprocessing.rs` — `TensorFpreGen`, `TensorFpreEval`. Receives the four D_ev fields (three new + one rename).
- `src/online.rs` — `check_zero()` primitive. Used as-is for P2 consistency check.
- `src/auth_tensor_fpre.rs` — `TensorFpre::gen_auth_bit()`: used by `IdealPreprocessingBackend` to generate the new D_ev field entries.

### Prior Phase Context

- `.planning/phases/07-preprocessing-trait-ideal-backends/07-CONTEXT.md` — D-04/D-05/D-06/D-07/D-08: `gamma_auth_bit_shares` field pattern (now renamed `gamma_d_ev_shares`). The three new D_ev fields follow the same generation logic.
- `.planning/phases/08-open-protocol-1-garble-eval-check/08-CONTEXT.md` — D-07/D-08/D-09: `check_zero()` contract and `c_gamma` assembly pattern. P2 reuses `check_zero()` with different input assembly.

### Requirements

- `.planning/REQUIREMENTS.md` — P2-01..P2-05 active.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `gen_unary_outer_product` (`src/tensor_ops.rs:80`): Direct template for `gen_unary_outer_product_wide`. Same GGM tree traversal; only the leaf expansion changes from one TCCR call to two (even/odd tweaks).
- `check_zero()` (`src/online.rs:52`): Used as-is for P2 consistency check — no new variant needed.
- `TensorFpre::gen_auth_bit()` (`src/auth_tensor_fpre.rs`): Generates correctly-structured IT-MAC authenticated bits. Used by `IdealPreprocessingBackend` to populate all four D_ev fields.
- `AuthBitShare` (`src/sharing.rs`): Standard type for all IT-MAC authenticated shares. All new D_ev fields use `Vec<AuthBitShare>`.

### Established Patterns

- Column-major indexing: `j * n + i` for (i,j) in n×m fields — same for all new D_ev fields.
- Gen/Eval symmetry: same field names on both `TensorFpreGen` and `TensorFpreEval`, different shares.
- Atomic constructor update (Phase 7 D-06): every constructor must initialize new fields in the same commit.
- Even/odd GGM tweak: `Block::from((level << 1) as u128)` and `Block::from((level << 1 | 1) as u128)` — matches the new wide leaf expansion tweak scheme.

### Integration Points

- `src/preprocessing.rs`: three new fields + rename. `IdealPreprocessingBackend::run()` updated to generate them.
- `src/auth_tensor_gen.rs` / `src/auth_tensor_eval.rs`: rename `gamma_auth_bit_shares` → `gamma_d_ev_shares` at all use sites; add `_p2` methods.
- `src/tensor_ops.rs`: add `gen_unary_outer_product_wide` alongside the existing function.
- `src/lib.rs`: existing P1 tests must remain green. New P2 E2E test added here using `IdealPreprocessingBackend`.

</code_context>

<specifics>
## Specific Ideas

- Wide ciphertext type is `(Block, Block)` as the (κ ‖ ρ) concatenation. The first element is always the κ-half (D_gb), the second is the ρ-half (D_ev). Never treat them as independent values.
- `gen_unary_outer_product_wide` signature (tentative):
  ```rust
  pub(crate) fn gen_unary_outer_product_wide(
      seeds: &[Block],
      y_d_gb: &MatrixViewRef<Block>,
      y_d_ev: &MatrixViewRef<Block>,
      out_gb: &mut MatrixViewMut<Block>,
      out_ev: &mut MatrixViewMut<Block>,
      cipher: &FixedKeyAes,
  ) -> Vec<(Block, Block)>  // wide ciphertexts G_k = (kappa_half || rho_half)
  ```
- In the single-gate P2 test, input wire D_ev shares are initialized per `6_total.tex` §4 step 3: `[v_w D_ev]^gb := [l_w D_ev]^gb`; `[v_w D_ev]^ev := [l_w D_ev]^ev XOR L_w * D_ev`.

</specifics>

<deferred>
## Deferred Ideas

- `open()` (ONL-01/ONL-02) — still deferred; P2 consistency check does not require it.
- Multi-gate circuit test for P2 — single-gate test is sufficient for Phase 9; multi-gate exercises only arise once a circuit-level eval loop is built.
- D_ev wire-value propagation across multiple gates in a real circuit — the single-gate test harness sets D_ev shares directly; a multi-gate harness would need to propagate them through `evaluate_first_half_p2`/`evaluate_second_half_p2` return values.

</deferred>

---

*Phase: 09-protocol-2-garble-eval-check*
*Context gathered: 2026-04-24*
