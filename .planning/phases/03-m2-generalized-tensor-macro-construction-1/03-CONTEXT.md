# Phase 3: M2 Generalized Tensor Macro (Construction 1) - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement the Generalized Tensor Macro from paper Construction 1 as a reusable Rust primitive in `src/tensor_macro.rs`. Two functions: `tensor_garbler(n, m, Δ, A_keys, T^gb)` and `tensor_evaluator(n, m, G, A_macs, T^ev)` such that `Z_garbler XOR Z_evaluator = a ⊗ T` holds. No dependency on LeakyTriple state or Pi_LeakyTensor protocol.

Requirements in scope: PROTO-01, PROTO-02, PROTO-03, TEST-01, TEST-04.

Out of scope: Pi_LeakyTensor (Construction 2), F_eq, bCOT consumption (Phase 4); Pi_aTensor combining (Phase 5); permutation bucketing (Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Module Placement (PROTO-01)

- **D-01:** Create `src/tensor_macro.rs` as the home for Construction 1. Add `pub mod tensor_macro;` to `src/lib.rs`. This module is a standalone primitive — it has no dependency on `leaky_tensor_pre.rs` or `preprocessing.rs`.
- **D-02:** Both `tensor_garbler` and `tensor_evaluator` are `pub(crate)` functions — they are called by `leaky_tensor_pre.rs` (Phase 4) but are not part of the crate's public API.

### GGM Tree Kernel Reuse (PROTO-02)

- **D-03:** Generalize `gen_populate_seeds_mem_optimized` in `src/tensor_ops.rs` by changing its first parameter from `x: &MatrixViewRef<Block>` to `x: &[Block]`. The function body treats the parameter as an indexed array of Blocks — both usages (wire labels and IT-MAC keys) are structurally identical.
- **D-04:** Update the one call site in `tensor_gen.rs` to pass a `&[Block]` slice (convert from `MatrixViewRef` via `.as_slice()` or equivalent). Callers in `tensor_macro.rs` pass MAC keys as `&[Block]` directly.
- **D-05:** `gen_unary_outer_product` in `tensor_ops.rs` is reused for the leaf expansion step — same `(seeds, T_share) → (Z_output, leaf_cts)` pattern as in `tensor_garbler`.

### G Ciphertext Type (PROTO-02, PROTO-03)

- **D-06:** Define `pub(crate) struct TensorMacroCiphertexts` in `src/tensor_macro.rs`:
  ```rust
  pub(crate) struct TensorMacroCiphertexts {
      pub level_cts: Vec<(Block, Block)>,  // length n-1; G_{i,0} and G_{i,1} for i ∈ [n-1]
      pub leaf_cts: Vec<Block>,             // length m; G_k for k ∈ [m]
  }
  ```
  Maps directly to paper notation. Phase 4 passes the entire struct from `tensor_garbler` to `tensor_evaluator`.

### Function Signatures (PROTO-01, PROTO-02, PROTO-03)

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

### Input/Output Types

- **D-09:** Garbler input `a_keys: &[Key]` — uses the `Key` type from Phase 1 (`Key::new()` enforces LSB=0). Length must equal `n`.
- **D-10:** Evaluator input `a_macs: &[Mac]` — uses the `Mac` type; may have LSB=1 (encodes `A_i ⊕ a_i·Δ`). Length must equal `n`.
- **D-11:** T shares and Z outputs are `BlockMatrix` (n×m, column-major indexing as established in Phase 1 D-11).

### Tests (TEST-01, TEST-04)

- **D-12:** Paper invariant test: construct `a_keys` and `a_macs` from `IdealBCot` (use `transfer_a_to_b` sender_keys as `a_keys`, receiver_macs as `a_macs`). Set T = T_gen XOR T_eval with known random values. Assert `Z_gen XOR Z_eval == a ⊗ T` where `a` is the bit vector from bCOT choice bits.
- **D-13:** Test vectors include edge cases: n=1 (single-wire, degenerate tree), small m, large n and m.
- **D-14:** Tests live in `#[cfg(test)] mod tests` at the bottom of `src/tensor_macro.rs` — inline test pattern per codebase convention.

### Claude's Discretion

- Exact nonce/tweak strategy for level and leaf ciphertexts — paper uses `H(A_i ⊕ Δ, ν_{i,b})` with one-time nonces; use `cipher.tccr(level_tweak, seed)` matching the existing GGM PRF pattern.
- Whether `tensor_macro.rs` exposes `TensorMacroCiphertexts` with `pub(crate)` fields or getter methods — `pub(crate)` fields are the existing pattern in this codebase.
- Implementation of the evaluator's subtree reconstruction traversal — the strategy is specified by the paper; exact loop structure and temporary storage are implementer's call.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `references/appendix_krrw_pre.tex` — Construction 1 (Generalized Tensor Macro), `tensorgb` and `tensorev` steps; correctness proof
- `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf` — main paper; §3 leaky tensor preprocessing context

### Source Files in Scope
- `src/tensor_ops.rs` — `gen_populate_seeds_mem_optimized` (GGM tree kernel, being generalized to `&[Block]`) and `gen_unary_outer_product` (leaf expansion)
- `src/tensor_gen.rs` — single call site for `gen_populate_seeds_mem_optimized` that will need the `&[Block]` update
- `src/keys.rs` — `Key` type (LSB=0 invariant, `Key::new()` constructor)
- `src/macs.rs` — `Mac` type (MAC values, may have any LSB)
- `src/matrix.rs` — `BlockMatrix` type (n×m column-major storage)
- `src/aes.rs` — `FixedKeyAes`, `FIXED_KEY_AES` singleton, `tccr()` PRF

### Upstream Context
- `.planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md` — Key::new(), pub(crate) convention, column-major indexing
- `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/02-CONTEXT.md` — preprocessing.rs module structure, gamma removal complete
- `.planning/ROADMAP.md` — Phase 3 goal and success criteria (PROTO-01 to PROTO-03, TEST-01, TEST-04)
- `.planning/REQUIREMENTS.md` — full requirements listing

### Reference Implementation
- `references/mpz-dev/` — read-only reference implementation; check for GGM tree patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `gen_populate_seeds_mem_optimized` (`src/tensor_ops.rs`): builds the GGM tree level-by-level, returns leaf seeds and per-level XOR sums (odd/even) — directly maps to `G_{i,0}`, `G_{i,1}` after generalization
- `gen_unary_outer_product` (`src/tensor_ops.rs`): expands leaf seeds into an n×m matrix and XORs with a T share — directly maps to the leaf ciphertext generation and Z computation step
- `FIXED_KEY_AES` singleton (`src/aes.rs`): thread-safe PRF instance; `TensorProductGen` uses it as `&'static FixedKeyAes` — same pattern for `tensor_macro.rs`
- `BlockMatrix::new(n, m)` and column-major indexing — Z_garbler and Z_evaluator are BlockMatrix

### Established Patterns
- `pub(crate)` for internal functions (Phase 1) — `tensor_garbler`, `tensor_evaluator`, `TensorMacroCiphertexts` should all be `pub(crate)`
- Inline `#[cfg(test)] mod tests` at bottom of the source file — no separate `tests/` directory
- `IdealBCot` used as test oracle in `leaky_tensor_pre.rs` tests — same pattern for tensor macro tests
- `Key::new()` for safe key construction — garbler key inputs are `Vec<Key>` not `Vec<Block>`

### Integration Points
- `src/tensor_ops.rs`: parameter type change on `gen_populate_seeds_mem_optimized` affects `tensor_gen.rs` call site (semi-honest garbler). No behavioral change — just `MatrixViewRef<Block>` → `&[Block]`.
- `src/lib.rs`: add `pub mod tensor_macro;` alongside existing module declarations
- Phase 4 (`leaky_tensor_pre.rs` rewrite): will call `tensor_garbler` and `tensor_evaluator` — the types decided here define Phase 4's call sites

</code_context>

<specifics>
## Specific Ideas

- User explicitly chose `src/tensor_macro.rs` as a standalone module — matches the paper's treatment of Construction 1 as a reusable primitive that both Construction 2 calls use
- User chose to generalize `gen_populate_seeds_mem_optimized` to `&[Block]` rather than writing a parallel function — one GGM tree implementation, no duplication
- User chose `TensorMacroCiphertexts` named struct over tuple — cleaner call sites in Phase 4 where G is passed between two tensor macro calls
- User chose `Vec<Key>` / `Vec<Mac>` typed inputs — preserves the Key LSB=0 invariant established in Phase 1 at the tensor macro boundary

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 3 scope.

</deferred>

---

*Phase: 03-m2-generalized-tensor-macro-construction-1*
*Context gathered: 2026-04-21*
