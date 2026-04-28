---
slug: fix-cot-convention
title: Fix COT choice-bit convention in leaky_tensor_pre::generate()
date: 2026-04-21
status: in_progress
---

## Task

Fix the IT-MAC convention bug in `leaky_tensor_pre::generate()`: the COT choice bits are
swapped relative to the paper's specification, authenticating each party's bits under their
OWN delta (convention 2) instead of the VERIFIER's delta (convention 1) as required by
the paper (appendix_krrw_pre.tex, Blueprint of Π_LeakyTensor, line 166).

## Bug

Paper specifies (convention 1 — verifier's delta):
- `[x_pa]^{Δ_b}` — A's bits authenticated under B's delta
- `[x_pb]^{Δ_a}` — B's bits authenticated under A's delta

Code does (convention 2 — committer's own delta):
- `transfer_a_to_b(&gen_alpha_portions)` → A's bits under Δ_a (WRONG)
- `transfer_b_to_a(&eval_alpha_portions)` → B's bits under Δ_b (WRONG)

## Fix

### leaky_tensor_pre.rs — generate()
For alpha, beta, correlated, and gamma:
1. Swap choice bits between COT calls:
   - `transfer_a_to_b(&eval_*_portions)` — B's bits as choice → B's bits under Δ_a ✓
   - `transfer_b_to_a(&gen_*_portions)` — A's bits as choice → A's bits under Δ_b ✓
2. Swap key/mac field sources to match the new COT assignment:
   - gen_share.key = cot_a_to_b.sender_keys (A's sender key)
   - gen_share.mac = cot_b_to_a.receiver_macs (A's MAC under delta_b)
   - eval_share.key = cot_b_to_a.sender_keys (B's sender key)
   - eval_share.mac = cot_a_to_b.receiver_macs (B's MAC under delta_a)
3. Update function-level comments.

### Tests — verify_cross_party / verify_pair
In all three files (leaky_tensor_pre.rs, auth_tensor_pre.rs, auth_tensor_fpre.rs):
- Swap delta_a ↔ delta_b in the verification calls to match convention 1:
  - Gen's commitment now verified under delta_b (was delta_a)
  - Eval's commitment now verified under delta_a (was delta_b)

## Verification
`cargo test` must pass with 0 failures.
