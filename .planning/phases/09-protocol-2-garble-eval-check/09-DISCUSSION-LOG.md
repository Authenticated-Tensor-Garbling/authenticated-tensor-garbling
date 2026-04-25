# Phase 9: Protocol 2 Garble/Eval/Check - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-24
**Phase:** 09-protocol-2-garble-eval-check
**Areas discussed:** Wide leaf type, D_ev preprocessing fields, _p2 function placement, P2 test scope

---

## Wide leaf type

| Option | Description | Selected |
|--------|-------------|----------|
| (Block, Block) tuple | No new type. Wide ciphertexts as Vec<(Block, Block)>. Simple, consistent with existing Block usage. | ✓ |
| New WideBlock struct | Named wrapper with kappa/rho fields. More explicit but adds a new type to maintain. | |

**User's choice:** `(Block, Block)` tuple

**Notes:** User clarified that both κ and ρ are 128 bits in this codebase (both `Delta` and `Block` are 128-bit), so κ+ρ=256=two Blocks. The STATE.md "168-bit" estimate was incorrect.

User confirmed that the tuple represents a concatenation (not two independent values) — first Block is κ-prefix (D_gb half), second is ρ-suffix (D_ev half).

**Wide expansion tweak follow-up:**

| Option | Description | Selected |
|--------|-------------|----------|
| Even/odd tweak split | `kappa_half = tccr(2*base, seed)`, `rho_half = tccr(2*base+1, seed)`. Same convention as existing GGM tree code. | ✓ |
| Claude's discretion | Let researcher/planner choose domain-separation scheme. | |

**Notes:** User proposed the even/odd approach themselves after finding the initial two-tweak example confusing. The even/odd pattern is already used in `gen_populate_seeds_mem_optimized` for left/right child seeds, making this the natural consistent choice.

---

## D_ev preprocessing fields

| Option | Description | Selected |
|--------|-------------|----------|
| Extend TensorFpreGen/Eval | Add alpha_d_ev_shares, beta_d_ev_shares, correlated_d_ev_shares fields. Follow Phase 7 pattern. | ✓ |
| Test-construction only | Phase 9 test harness directly constructs D_ev shares; no preprocessing struct extension. | |

**User's choice:** Extend TensorFpreGen/Eval with three new fields.

**Naming follow-up:**

| Option | Description | Selected |
|--------|-------------|----------|
| Leave gamma_auth_bit_shares unchanged | No rename; inconsistent naming but avoids touching Phase 7/8 code. | |
| Rename to gamma_d_ev_shares | Consistent naming across all four D_ev fields. Manageable ripple (auth_tensor_gen.rs, auth_tensor_eval.rs, lib.rs tests). | ✓ |

**User's choice:** Rename `gamma_auth_bit_shares` → `gamma_d_ev_shares`.

---

## _p2 function placement

| Option | Description | Selected |
|--------|-------------|----------|
| New methods on AuthTensorGen/Eval | `garble_*_p2()` on AuthTensorGen; `evaluate_*_p2()` on AuthTensorEval. Matches P1 pattern. | ✓ |
| Standalone functions in online.rs | Free functions taking structs by reference. Long parameter lists. | |
| New src/online_p2.rs file | Clean separation but adds a file and requires struct internals to be exposed. | |

**User's choice:** New methods on existing structs with `_p2` suffix.

**D_ev output follow-up:**

| Option | Description | Selected |
|--------|-------------|----------|
| Return as Vec<Block> alongside existing outputs | `garble_final_p2()` returns `(Vec<Block>, Vec<Block>)` — D_gb and D_ev shares. No new struct fields. | ✓ |
| New field on AuthTensorGen | Add `first_half_out_d_ev` etc. fields, mirroring P1 in-place accumulation. | |

**User's choice:** Return D_ev shares as part of the return tuple, no new struct fields.

---

## P2 test scope

| Option | Description | Selected |
|--------|-------------|----------|
| Single tensor gate | Mirrors P1-04. One AuthTensorGen + one AuthTensorEval, IdealPreprocessingBackend. | ✓ |
| Two-gate circuit | Sequential tensor gates with D_ev-share propagation. More harness complexity. | |

**User's choice:** Single tensor gate.

**Notes:** Consistent with Phase 8 precedent (P1-04 tested a single gate). Multi-gate D_ev propagation is deferred.

---

## Claude's Discretion

- Exact `v_alpha D_ev` initialization for input wires in the P2 test harness
- Whether to split `evaluate_p2` into sub-methods (evaluate_first_half_p2 etc.) — follow garble_p2 structure for symmetry
- Exact parameter names for the new `_p2` methods

## Deferred Ideas

- `open()` (ONL-01/ONL-02) — still not needed for P2 consistency check; stays deferred
- Multi-gate P2 circuit test — single gate sufficient for Phase 9
