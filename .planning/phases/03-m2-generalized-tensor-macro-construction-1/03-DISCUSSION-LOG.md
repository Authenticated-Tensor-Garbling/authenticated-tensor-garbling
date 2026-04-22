# Phase 3: M2 Generalized Tensor Macro - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 03-m2-generalized-tensor-macro-construction-1
**Areas discussed:** Module placement, GGM tree kernel reuse, G ciphertext type, Input/output types

---

## Module Placement

| Option | Description | Selected |
|--------|-------------|----------|
| New src/tensor_macro.rs | Standalone primitive matching paper structure; leaky_tensor_pre.rs stays focused on Construction 2 | ✓ |
| Inline in leaky_tensor_pre.rs | Fewer files, but couples the primitive to one protocol | |
| In preprocessing.rs | Awkward — mixes primitive and protocol layers | |

**User's choice:** New `src/tensor_macro.rs`
**Notes:** Paper treats Construction 1 as a standalone reusable primitive. Phase 4 calls it twice (A-as-garbler + B-as-garbler).

---

## GGM Tree Kernel Reuse

| Option | Description | Selected |
|--------|-------------|----------|
| Generalize gen_populate_seeds_mem_optimized to &[Block] | Single kernel, no duplication; tensor_gen.rs call site updated | ✓ |
| Add a parallel function in tensor_macro.rs | tensor_ops.rs untouched but GGM tree logic duplicated | |
| Duplicate inline as private helper | Self-contained Phase 3 but tech debt | |

**User's choice:** Generalize `gen_populate_seeds_mem_optimized` in `tensor_ops.rs` to `&[Block]`
**Notes:** Change is behavioral-neutral — MatrixViewRef<Block> and &[Block] are both indexed Block slices.

---

## G Ciphertext Type

| Option | Description | Selected |
|--------|-------------|----------|
| Named struct TensorMacroCiphertexts | Clear mapping to paper notation; Phase 4 call sites are readable | ✓ |
| Tuple (Vec<(Block,Block)>, Vec<Block>) | Matches existing tensor_ops.rs return style but fragile positionally | |
| Flat Vec<Block> | Simple but implicit layout | |

**User's choice:** `TensorMacroCiphertexts { level_cts: Vec<(Block, Block)>, leaf_cts: Vec<Block> }`
**Notes:** Phase 4 passes the entire struct from tensor_garbler output to tensor_evaluator input.

---

## Input/Output Types

| Option | Description | Selected |
|--------|-------------|----------|
| Vec<Key> / Vec<Mac> / BlockMatrix | Typed inputs enforce LSB=0 (Key) and allow LSB=1 (Mac); Z as BlockMatrix | ✓ |
| Vec<Block> everywhere | No type enforcement, max flexibility | |
| Vec<AuthBitShare> | Over-specified; couples primitive to sharing.rs | |

**User's choice:** `Vec<Key>` for garbler keys, `Vec<Mac>` for evaluator MACs, `BlockMatrix` for T and Z
**Notes:** Preserves Key LSB=0 invariant from Phase 1 at the tensor macro boundary.

---

## Kernel Signature Change

| Option | Description | Selected |
|--------|-------------|----------|
| Change to &[Block] | Single function, both semi-honest garbler and tensor macro use it | ✓ |
| Add parallel function | No change to existing code but GGM tree duplicated | |

**User's choice:** Change `gen_populate_seeds_mem_optimized` first parameter to `&[Block]`

---

## T Type

| Option | Description | Selected |
|--------|-------------|----------|
| BlockMatrix | Consistent with Z output type; matrix wrapper | ✓ |
| Vec<Block> column-major | Flatter, matches LeakyTriple correlated_shares storage | |

**User's choice:** `BlockMatrix` for T shares

---

## Claude's Discretion

- Nonce/tweak strategy for level and leaf ciphertexts
- Whether `TensorMacroCiphertexts` uses pub(crate) fields or getter methods
- Evaluator subtree reconstruction traversal loop structure

## Deferred Ideas

None — discussion stayed within Phase 3 scope.
