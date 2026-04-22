# Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Rewrite `src/leaky_tensor_pre.rs` to implement `Pi_LeakyTensor` per paper Construction 2 (Appendix F): consume correlated randomness from `IdealBCot`, derive `C_A`/`C_B` correlations, run two tensor-macro calls (A as garbler, then B as garbler), XOR results, execute masked reveal to obtain public `D`, verify consistency via in-process `F_eq`, and output a leaky triple whose shape is exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` — no gamma bits, no wire labels.

Add `src/feq.rs` as the ideal `F_eq` module. Remove gamma fields and wire labels from `LeakyTriple`. Rename struct fields alpha→x, beta→y, correlated→z to match paper notation.

Requirements in scope: PROTO-04, PROTO-05, PROTO-06, PROTO-07, PROTO-08, PROTO-09, TEST-02, TEST-03, TEST-04.
Out of scope: Pi_aTensor combining (Phase 5); permutation bucketing (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### generate() API (PROTO-04, PROTO-09)

- **D-01:** Signature is `generate(&mut self) -> LeakyTriple` — no `x_clear`/`y_clear` arguments. x and y bits are sampled uniformly at random internally using the `LeakyTensorPre`'s own `ChaCha12Rng`. Preprocessing must be fully input-independent; the old signature taking concrete input values violated this invariant.
- **D-02:** `LeakyTensorPre` struct itself is unchanged in shape (`n`, `m`, `bcot: &'a mut IdealBCot`, `rng: ChaCha12Rng`) and construction (`LeakyTensorPre::new(seed, n, m, bcot)`). Only `generate` is rewritten.

### F_eq Module (PROTO-08, TEST-04)

- **D-03:** `F_eq` lives in a new `src/feq.rs` module, matching the `IdealBCot` pattern. Add `pub mod feq;` to `src/lib.rs`. The module exposes a single public function (or struct with a check method) for the ideal equality check.
- **D-04:** On L_1 ≠ L_2, F_eq calls `panic!("F_eq abort: consistency check failed — L_1 != L_2")`. Abort is unconditional and immediate, matching the ideal functionality semantics. Tests for TEST-04 (verifying abort on malformed inputs) use `#[should_panic]`.
- **D-05:** Correct inputs (L_1 == L_2 element-wise) return normally (no return value needed beyond unit). F_eq takes `l1: &BlockMatrix` and `l2: &BlockMatrix` and does element-wise Block comparison.

### LeakyTriple Struct Cleanup (PROTO-09)

- **D-06:** Rename fields to match paper notation throughout:
  - `gen_alpha_shares` → `gen_x_shares`
  - `eval_alpha_shares` → `eval_x_shares`
  - `gen_beta_shares` → `gen_y_shares`
  - `eval_beta_shares` → `eval_y_shares`
  - `gen_correlated_shares` → `gen_z_shares`
  - `eval_correlated_shares` → `eval_z_shares`
- **D-07:** Remove `gen_gamma_shares`, `eval_gamma_shares`, `gen_alpha_labels`, `eval_alpha_labels`, `gen_beta_labels`, `eval_beta_labels` entirely. These do not appear in the paper's Pi_LeakyTensor output.
- **D-08:** Z is stored as `Vec<AuthBitShare>` in column-major order (index = `j*n+i`, matching existing convention). Length = `n*m`. Phase 5 combining works directly on this Vec without conversion.
- **D-09:** The `n`, `m`, `delta_a`, `delta_b` fields remain on `LeakyTriple`.

### C_A/C_B Computation (PROTO-05)

- **D-10:** C_A and C_B are computed inline in `generate()` — no separate helper function. Each is a length-m `Vec<Block>` computed as Block-level XOR per entry:
  ```
  C_A[j] := y_A[j]·Δ_A ⊕ key(y_B@A)[j] ⊕ mac(y_A@B)[j]
  C_B[j] := y_B[j]·Δ_B ⊕ mac(y_B@A)[j] ⊕ key(y_A@B)[j]
  ```
  where `y_A[j]·Δ_A` means `if y_A_bit { Δ_A.as_block() } else { Block::ZERO }`.
  Analogously, `C_A^(R)` and `C_B^(R)` are computed the same way using R shares.

### R (Random Authenticated Tensor Mask) (PROTO-04)

- **D-11:** `itmac{R}{Δ}` is obtained via n×m bCOT calls each way (`transfer_a_to_b` and `transfer_b_to_a`) — the same pattern as x and y shares. R bits are sampled uniformly at random internally. No new methods added to `IdealBCot`.
- **D-12:** R shares are assembled into `gen_r_shares` and `eval_r_shares` (local to `generate()`, not stored on `LeakyTriple`). They are used to compute `C_A^(R)`, `C_B^(R)`, and then `itmac{R}{Δ}` for the final output Z.

### Tensor Macro Calls (PROTO-06, PROTO-07)

- **D-13:** Macro call 1: `tensor_garbler(n, m, Δ_A, keys_of_x_B@A, C_A)` → `(Z_gb1, G_1)` — A is garbler, B is evaluator. `tensor_evaluator(n, m, G_1, macs_of_x_B@A, C_B)` → `E_1`.
- **D-14:** Macro call 2: `tensor_garbler(n, m, Δ_B, keys_of_x_A@B, C_B)` → `(Z_gb2, G_2)` — B is garbler, A is evaluator. `tensor_evaluator(n, m, G_2, macs_of_x_A@B, C_A)` → `E_2`.
- **D-15:** `t_gen` and `t_eval` arguments to the macro must be `BlockMatrix` (m×1 column vectors). The `C_A`/`C_B` vecs are wrapped into a `BlockMatrix` before being passed to the macro.
- **D-16:** S_1 = Z_gb1 ⊕ E_2 ⊕ C_A^(R); S_2 = Z_gb2 ⊕ E_1 ⊕ C_B^(R). D = lsb(S_1) ⊕ lsb(S_2) (element-wise, producing an `n×m` bit matrix).

### Final Z Output (PROTO-07, PROTO-08)

- **D-17:** After F_eq passes, `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}`. Since D is public, `itmac{D}{Δ}` is locally computable from D bits and Δ_A/Δ_B. The combining XORs key/mac/value fields of R shares with the D-derived shares element-wise.

### Claude's Discretion

- Exact loop structure for assembling `Vec<Key>` / `Vec<Mac>` from bCOT output before passing to `tensor_garbler`/`tensor_evaluator` — straightforward extraction from `BcotOutput.sender_keys` and `receiver_macs`.
- Whether `BlockMatrix::from_blocks(blocks: Vec<Block>)` or `BlockMatrix::new` + manual fill is used to wrap C_A/C_B vecs — match the pattern in `tensor_macro.rs` tests.
- Exact nonce/ordering of bCOT calls inside `generate()` (x first, then y, then R) — matches the paper's "obtain correlated randomness" step ordering.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `references/appendix_krrw_pre.tex` — Construction 2 (Protocol Pi_LeakyTensor, lines 198–254): C_A/C_B formulas, two tensor-macro calls, masked reveal, F_eq consistency check and output
- `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf` — §3 (Pi_LeakyTensor context and blueprint)

### Source Files Being Rewritten / Created
- `src/leaky_tensor_pre.rs` — file under rewrite; existing `LeakyTensorPre` struct shape preserved, `generate()` body replaced, `LeakyTriple` struct fields changed
- `src/feq.rs` — NEW file; ideal F_eq module to create

### Source Files to Call (Existing, Unchanged API)
- `src/tensor_macro.rs` — `tensor_garbler` and `tensor_evaluator` (Phase 3 output); function signatures locked (D-07 through D-11 in 03-CONTEXT.md)
- `src/bcot.rs` — `IdealBCot::transfer_a_to_b`, `IdealBCot::transfer_b_to_a`, `BcotOutput`; `delta_a` and `delta_b` fields
- `src/matrix.rs` — `BlockMatrix::new(rows, cols)`, column-major indexing, `.as_view()`, `.as_view_mut()`; needed to wrap C_A/C_B as m×1 vectors
- `src/block.rs` — `Block::ZERO`, `Block::lsb()`, XOR via `^` operator
- `src/delta.rs` — `Delta::as_block()`, used in C_A/C_B and L_1/L_2 computation
- `src/keys.rs` — `Key`, `Key::as_blocks()`; garbler keys for tensor_garbler input
- `src/macs.rs` — `Mac`, `Mac::as_blocks()`; evaluator macs for tensor_evaluator input
- `src/sharing.rs` — `AuthBitShare { key, mac, value }`; output type for x, y, Z shares

### Upstream Context
- `.planning/phases/03-m2-generalized-tensor-macro-construction-1/03-CONTEXT.md` — locked tensor_garbler/tensor_evaluator signatures and TensorMacroCiphertexts type
- `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/02-CONTEXT.md` — gamma removal complete in TensorFpre; same must happen in LeakyTriple
- `.planning/ROADMAP.md` — Phase 4 goal and success criteria (PROTO-04 through PROTO-09, TEST-02 through TEST-04)
- `.planning/REQUIREMENTS.md` — full requirements listing

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tensor_garbler(n, m, delta, a_keys: &[Key], t_gen: &BlockMatrix) -> (BlockMatrix, TensorMacroCiphertexts)` (`src/tensor_macro.rs`): ready to call; takes `&[Key]` (extract from `BcotOutput.sender_keys`) and `BlockMatrix` T share (wrap C_A/C_B vec)
- `tensor_evaluator(n, m, g: &TensorMacroCiphertexts, a_macs: &[Mac], t_eval: &BlockMatrix) -> BlockMatrix` (`src/tensor_macro.rs`): ready to call
- `IdealBCot::transfer_a_to_b(&choices) -> BcotOutput` and `transfer_b_to_a` (`src/bcot.rs`): existing call sites in current `leaky_tensor_pre.rs` show the pattern
- `AuthBitShare { key: Key, mac: Mac, value: bool }` (`src/sharing.rs`): output type for all x/y/Z shares; `verify(&delta)` for tests
- `BlockMatrix::new(rows, cols)` with `[k]` column-major indexing (`src/matrix.rs`): used to wrap C_A/C_B

### Established Patterns
- `IdealBCot` borrowed (not owned) in `LeakyTensorPre<'a>` — keep the `&'a mut IdealBCot` pattern; do not change ownership model
- Inline `#[cfg(test)] mod tests` at bottom of each source file — same for `feq.rs` and `leaky_tensor_pre.rs`
- `#[should_panic]` tests for abort-path verification — matches Phase 3 test approach
- `Key::as_blocks(keys: &[Key]) -> &[Block]` for batch conversion before passing to tensor ops
- `Mac::as_blocks(macs: &[Mac]) -> &[Block]` — same pattern for evaluator inputs

### Integration Points
- `src/lib.rs`: add `pub mod feq;` alongside existing module declarations
- `src/preprocessing.rs`: `run_preprocessing` calls `LeakyTensorPre::generate()` — the no-arg signature change must propagate here
- Phase 5 (`Pi_aTensor combining`): will consume `LeakyTriple.gen_z_shares` / `eval_z_shares` directly as `Vec<AuthBitShare>` column-major — no conversion needed

</code_context>

<specifics>
## Specific Ideas

- User chose `generate() -> LeakyTriple` (no args) — cleanest alignment with input-independent preprocessing; the `LeakyTensorPre` struct's own RNG handles all randomness
- User chose `src/feq.rs` as a separate module matching `IdealBCot` pattern — easy to identify, easy to replace with a real implementation later
- User chose `panic!` on F_eq mismatch — immediate unconditional abort; `#[should_panic]` covers the negative test path
- User chose alpha→x, beta→y, correlated→z rename — direct paper notation; makes Construction 2 code auditable line-by-line against the appendix
- User chose `Vec<AuthBitShare>` column-major for Z — consistent with x and y; Phase 5 combining works directly without wrapping

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 4 scope.

</deferred>

---

*Phase: 04-m2-pi-leakytensor-f-eq-construction-2*
*Context gathered: 2026-04-21*
