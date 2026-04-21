# Phase 2: M1 Online + Ideal Fpre + Benches Cleanup - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 02-m1-online-ideal-fpre-benches-cleanup
**Areas discussed:** Module separation, Gamma dead code, Benchmark deduplication, run_preprocessing placement

---

## Module Separation

| Option | Description | Selected |
|--------|-------------|----------|
| New src/preprocessing.rs | New top-level file; TensorFpre stays in auth_tensor_fpre.rs; TensorFpreGen/Eval move to preprocessing.rs | ✓ |
| Inline mod in auth_tensor_fpre.rs | pub mod preprocessing { ... } inside auth_tensor_fpre.rs | |
| Stay in auth_tensor_fpre.rs | Reorganize within file with comment blocks | |

**User's choice:** New src/preprocessing.rs
**Notes:** Clean physical separation. auth_tensor_fpre.rs becomes purely the ideal trusted dealer.

---

## Import Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Direct import from preprocessing | Callers import from crate::preprocessing directly | ✓ |
| Re-export from auth_tensor_fpre | pub use crate::preprocessing::* in auth_tensor_fpre.rs | |

**User's choice:** Direct import — no hidden re-exports, no backward-compat shims.

---

## Gamma Dead Code

| Option | Description | Selected |
|--------|-------------|----------|
| Remove entirely | Delete _gamma_share computation + gamma_auth_bit_shares field from AuthTensorGen/Eval | ✓ |
| Keep with TODO comment | Leave code with // TODO Phase 3+ comment | |

**User's choice:** Remove entirely
**Notes:** User asked for context first: gamma was planned as output wire authentication but was never XORed into the output in garble_final(). evaluate_final() never referenced gamma_auth_bit_shares at all. Not "no output verification" — correlated_share provides the correlated preprocessing term; gamma was an additional mask that was never wired up. Since Phase 3-6 rewrites garble_final from the paper spec, there is no value in keeping a dead computation. Removal cascades: gamma_auth_bits out of TensorFpre, gamma_auth_bit_shares out of TensorFpreGen/Eval and AuthTensorGen/Eval, and related test assertions updated.

---

## Benchmark Deduplication

| Option | Description | Selected |
|--------|-------------|----------|
| Loop over chunking factors | for cf in [1, 2, 4, 6, 8] { ... } replaces 5 near-identical blocks | ✓ |
| Extract per-factor helper | bench_one_chunking_factor(group, n, m, cf) helper | |
| Leave structure, add comments | No restructure, just comments | |

**User's choice:** Loop over chunking factors

---

## Benchmark Paper Protocol Labels

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — add paper protocol comments | Header comments per benchmark group | ✓ |
| No — names are sufficient | | |

**User's choice:** Yes — add paper protocol comments to each benchmark group.

---

## run_preprocessing Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Move to src/preprocessing.rs | Real-protocol entry point alongside TensorFpreGen/Eval | ✓ |
| Stay in auth_tensor_fpre.rs | Keep next to TensorFpre | |

**User's choice:** Move to preprocessing.rs — auth_tensor_fpre.rs becomes purely the ideal trusted dealer.

---

## Claude's Discretion

- Exact wording of per-field doc comments on TensorFpreGen/TensorFpreEval
- Whether to rename or just remove the "awful return type" comment on gen_chunked_half_outer_product
- Exact placement of GGM tree traversal direction comments in eval_populate_seeds_mem_optimized

## Deferred Ideas

None — discussion stayed within Phase 2 scope.
