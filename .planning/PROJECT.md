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

### Validated

**Phase 4 — Pi_LeakyTensor + F_eq (2026-04-22):**
- ✓ PROTO-04..08: `Pi_LeakyTensor::generate` consumes bCOT, runs two tensor-macro calls, XORs with correlations, executes masked reveal, assembles `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}`
- ✓ PROTO-09: In-process `feq::check` aborts on mismatched inputs; `LeakyTriple` is exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` — gamma and wire labels removed
- ✓ TEST-02..04: MAC invariant, product invariant, F_eq abort, and cross-party consistency verified; 7-test battery passing

**Phase 5 — Pi_aTensor Correct Combining (2026-04-22):**
- ✓ PROTO-10: `two_to_one_combine` helper implements paper Construction 3 algebra (d assembly, MAC verify, x=x'⊕x'', Z=Z'⊕Z''⊕x''⊗d, y preserved from prime)
- ✓ PROTO-11: `combine_leaky_triples` is a thin iterative fold over `two_to_one_combine`; output sources all share vectors from acc (fixes silent x-bug)
- ✓ PROTO-12: `bucket_size_for(ell)` uses paper Theorem 1 formula with `ell<=1` SSP=40 fallback guard; all call sites updated
- ✓ TEST-05: Three-test battery — happy-path product invariant (2 triples), tamper-path `#[should_panic]`, full B=40 bucket fold; 70/70 tests passing

**Phase 6 — Pi_aTensor' Permutation Bucketing + Benches (2026-04-23):**
- ✓ PROTO-13: `apply_permutation_to_triple(&mut LeakyTriple, &[usize])` — permutes x-rows and Z-row i-indices in lockstep, y-rows untouched
- ✓ PROTO-14: `combine_leaky_triples` activates per-triple `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j)` Fisher-Yates shuffle; `run_preprocessing` threads `shuffle_seed=42`
- ✓ PROTO-15: `bucket_size_for(n, ell)` implements Construction 4 formula `B = 1 + ceil(SSP / log2(n*ell))`; values (4,1)=21, (4,2)=15, (16,1)=11 pinned in tests
- ✓ TEST-06: `test_run_preprocessing_product_invariant_construction_4` — end-to-end regression over full pipeline; 74/74 tests passing
- ✓ TEST-07: `cargo bench --no-run` compiles clean; bench doc identifies `Pi_aTensor' / Construction 4`

### Active

None — all requirements validated.

### Out of Scope

- Real network OT (Ferret/IKNP) — ideal F_bCOT stays for benchmarking
- Malicious security proof verification — correctness of honest-party execution only
- Online garbling phase changes beyond what preprocessing restructure requires

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement Pi_LeakyTensor via GGM tree macro | Paper spec; current direct-AND approach is not the protocol | Done (Phase 3) |
| In-process F_eq (ideal) | Matches IdealBCot pattern; no networking needed | Done (Phase 4) |
| Pi_aTensor' (permutation bucketing) over Pi_aTensor | Better bucket size (log(nℓ) vs log(ℓ)), user explicitly wants it | Done (Phase 6) |
| Keep TensorFpreGen/Eval interface | Online phase already correct; minimize scope | Done (Phase 2) |
| tensor_macro as standalone pub(crate) module | No dependency on leaky_tensor_pre or preprocessing — clean separation | Done (Phase 3) |
| Hard-code shuffle_seed=42 in run_preprocessing | Deterministic permutations for test reproducibility; preprocessing output is not secret from holder | Done (Phase 6) |
| Construction 4 replaces Construction 3 fully — no parallel path | Single bucket-sizer, no dead one-argument version remains | Done (Phase 6) |

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
*Last updated: 2026-04-23 after Phase 6 completion (Pi_aTensor' Permutation Bucketing + Benches — Construction 4). Milestone v1.0 complete — all 6 phases done, 74/74 tests passing.*
