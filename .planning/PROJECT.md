# Project: Authenticated Tensor Garbling — Preprocessing Fix

## What This Is

A correctness fix and completion of the KRRW-style uncompressed preprocessing protocol for authenticated tensor garbling in Rust (Appendix F of the paper). The existing Phase 1 implementation is algorithmically wrong in multiple critical ways — the GGM tree-based generalized tensor macro is absent, the combining procedure does not match the paper, the bucket size formula is incorrect, and tests verify code behavior rather than paper invariants. This project fixes all bugs, implements missing protocol pieces (F_eq, Pi_aTensor' permutation bucketing), and rewrites tests to verify paper-specified properties.

## Core Value

**Correct paper-faithful implementation of Pi_LeakyTensor and Pi_aTensor** — both the protocol mechanics (GGM tree macro, F_eq check, correct combining) and the security properties (triple structure, combining correctness, bucket size formula).

## Context

- **Codebase:** Rust library, no networking. Authenticated garbling for secure two-party computation.
- **Paper:** KRRW18 + Appendix F of this project's in-progress paper. Protocol described in `references/appendix_krrw_pre.tex`.
- **Phase 1 status:** Marked "complete" in STATE.md but contains deep algorithmic bugs discovered on review.
- **Protocol chain:** `bcot → leaky_tensor_pre → auth_tensor_pre → auth_tensor_fpre → auth_tensor_gen/eval`
- **Online phase:** `auth_tensor_gen.rs` / `auth_tensor_eval.rs` — untouched, feeds from preprocessing output.

## Known Bugs (from paper review)

### Bug 1 — GGM Tree Macro Not Implemented
`LeakyTensorPre::generate()` does NOT implement Pi_LeakyTensor. The paper uses the Generalized Tensor Macro (Construction 1: GGM tree expansion + tensor operations on leaves) to compute XOR shares of x ⊗ y(Δ_A ⊕ Δ_B). The current code directly computes `alpha_i AND beta_j` via bCOT — this is a completely different computation that bypasses the core protocol.

### Bug 2 — F_eq Consistency Check Missing
Pi_LeakyTensor ends with a consistency check (§3.1 "Consistency check and output"): both parties send L_1 and L_2 to F_eq and abort if they don't match. This is entirely absent.

### Bug 3 — Wrong Pi_aTensor Combining Algorithm
The paper's combining procedure (§3.2): keep x = x', y = y', reveal d = y' ⊕ y'' publicly (with MACs), compute Z = Z' ⊕ Z'' ⊕ (x'' ⊗ d). Current `combine_leaky_triples` XOR-combines all shares naively — this does not match the paper's combining step, and carries "gamma" bits that are not part of the leaky triple format.

### Bug 4 — Gamma Bits in Wrong Place
The paper's leaky triple output is `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})`. There are no "gamma" bits in Pi_LeakyTensor output. Gamma is a random authenticated bit for the online garbling phase. Current code generates gamma as part of preprocessing.

### Bug 5 — Wrong Bucket Size Formula
`bucket_size_for(n, m)` uses `ell = n * m` (tensor dimension product) as ℓ in the formula `B = floor(SSP / log2(ℓ)) + 1`. But ℓ is the number of OUTPUT authenticated tensor triples, not the tensor dimensions. Pi_aTensor' uses `B = 1 + ceil(SSP / log2(n * ℓ))` which correctly involves both n and ℓ.

### Bug 6 — Wire Labels in Wrong Layer
Alpha/beta labels (label_0 per wire) are generated inside `LeakyTensorPre::generate()` and stored on the LeakyTriple. These are online garbling setup, not preprocessing output. The preprocessing output from F_aTensor does not include wire labels.

### Bug 7 — `generate_with_input_values` Violates Input Independence
`TensorFpre::generate_with_input_values(x, y)` takes concrete input values and embeds them into preprocessing structures. Preprocessing must be completely input-independent — actual inputs are provided during the online phase.

### Bug 8 — Tests Echo Code, Not Paper
Tests like `test_correlated_bit_correctness` verify that `gen_corr XOR eval_corr == alpha AND beta` — but this is exactly what the code computes, not a paper invariant. Paper invariant tests would verify the IT-MAC structure of each share, the correctness of the triple under the paper's definition, and the combining procedure output.

## Requirements

### Validated
*(from existing working online phase)*
- ✓ AuthBitShare IT-MAC structure: `mac = key XOR bit * delta` — existing
- ✓ Key LSB = 0 invariant — existing
- ✓ Delta LSB = 1 invariant — existing
- ✓ Shared IdealBCot for same Δ across triples — existing

### Validated

**Phase 3 — Generalized Tensor Macro (2026-04-22):**
- ✓ PROTO-01: Implemented `tensor_garbler` (Construction 1 garbler side) — GGM tree expansion, emits level + leaf ciphertexts, returns Z_garbler
- ✓ PROTO-02: Implemented `tensor_evaluator` (Construction 1 evaluator side) — reconstructs untraversed subtree, returns Z_evaluator
- ✓ PROTO-03: `Z_garbler XOR Z_evaluator == a ⊗ T` verified by 10-test TDD battery (9 (n,m) tuples + fixed-seed regression)
- ✓ TEST-01: Paper-invariant test battery in `tensor_macro::tests` — 10 passed / 0 failed

### Active

**Protocol Correctness:**
- [ ] PROTO-04: Implement correct Pi_aTensor combining (§3.2): d = y' ⊕ y'', Z = Z' ⊕ Z'' ⊕ x'' ⊗ d
- [ ] PROTO-05: Implement Pi_aTensor' (Construction 4): permutation bucketing with bucket size B = 1 + ceil(SSP / log2(n * ℓ))
- [ ] PROTO-06: Fix bucket size formula to use ℓ (number of output triples) not n*m

**Structure Fixes:**
- [ ] STRUCT-01: Remove gamma bits from LeakyTriple / preprocessing output — move to online phase
- [ ] STRUCT-02: Remove wire labels from LeakyTriple — labels belong in online garbling setup
- [ ] STRUCT-03: Remove or fix `generate_with_input_values` — preprocessing must be input-independent

**Tests:**
- [ ] TEST-01: Tests verify paper invariants: triple structure `itmac{Z}{Δ}` where Z = x⊗y (not code behavior)
- [ ] TEST-02: Tests verify Pi_aTensor combining: Z_combined = Z' ⊕ Z'' ⊕ x'' ⊗ d
- [ ] TEST-03: Tests verify F_eq abort behavior
- [ ] TEST-04: Tests verify GGM tree correctness (garbler/evaluator macro outputs XOR to x ⊗ T)
- [ ] TEST-05: Benchmarks preserved and working after restructure

### Out of Scope

- Real network OT (Ferret/IKNP) — ideal F_bCOT stays for benchmarking
- Malicious security proof verification — correctness of honest-party execution only
- Online garbling phase changes beyond what preprocessing restructure requires

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement Pi_LeakyTensor via GGM tree macro | Paper spec; current direct-AND approach is not the protocol | Done (Phase 3) |
| In-process F_eq (ideal) | Matches IdealBCot pattern; no networking needed | Pending (Phase 4) |
| Pi_aTensor' (permutation bucketing) over Pi_aTensor | Better bucket size (log(nℓ) vs log(ℓ)), user explicitly wants it | Pending |
| Keep TensorFpreGen/Eval interface | Online phase already correct; minimize scope | Pending |
| tensor_macro as standalone pub(crate) module | No dependency on leaky_tensor_pre or preprocessing — clean separation | Done (Phase 3) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-22 after Phase 3 completion (M2 Generalized Tensor Macro — Construction 1)*
