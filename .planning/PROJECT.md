# Project: Authenticated Tensor Garbling — Preprocessing Fix

## What This Is

A correct, paper-faithful implementation of the KRRW-style uncompressed preprocessing protocol for authenticated tensor garbling in Rust (Appendix F). All four paper constructions are implemented: the Generalized Tensor Macro (Construction 1), Pi_LeakyTensor with F_eq (Construction 2), Pi_aTensor correct combining (Construction 3), and Pi_aTensor' with permutation bucketing (Construction 4). The codebase has been refactored for correctness, the 8 known algorithmic bugs are fixed, and tests verify paper-specified invariants rather than echoing code behavior.

## Core Value

**Correct paper-faithful implementation of Pi_LeakyTensor and Pi_aTensor** — both the protocol mechanics (GGM tree macro, F_eq check, correct combining, permutation bucketing) and the security properties (triple structure, combining correctness, bucket size formula).

## Context

- **Codebase:** Rust library, no networking. ~54,842 LOC across 199 source files.
- **Tech stack:** Rust, rand/rand_chacha (ChaCha12Rng), once_cell (FIXED_KEY_AES), criterion (benchmarks)
- **Paper:** KRRW18 + Appendix F. Protocol in `references/appendix_krrw_pre.tex`.
- **v1.0 shipped:** 2026-04-23. All 6 phases complete. 74/74 tests passing.
- **Protocol chain:** `bcot → leaky_tensor_pre → auth_tensor_pre → auth_tensor_fpre → auth_tensor_gen/eval`
- **Online phase:** `auth_tensor_gen.rs` / `auth_tensor_eval.rs` — untouched; feeds from preprocessing output.

## Current Milestone: v1.1 Full Protocol Demonstration + Benchmarks

**Goal:** Extend the v1.0 preprocessing foundation into a complete, demonstrable protocol with interchangeable preprocessing, a working online phase (Open + consistency check), and coherent wall-clock benchmarks.

**Target features:**
- IdealPreprocessing interface — produces correct authenticated triples as oracle; interchangeable with real preprocessing implementations
- Preprocessing trait abstraction — pi_atensor, pi'_atensor, FbCOT, IdealPreprocessing all satisfy common interface
- Compressed preprocessing — if derivable from paper (appendix_cpre.tex), implement as additional interchangeable backend
- Unauthenticated tensor macros (Protocol 1) — standalone garble/eval functions per Construction 1
- Authenticated tensor macros (Protocol 2) — full authenticated garble/eval using preprocessing output
- Open() — correct reveal of authenticated values per Protocol 1 and Protocol 2
- Consistency check — verifies output correctness for both protocols
- Coherent wall-clock benchmarks for tensor product; cleaned benchmark code
- Distributed half gates + comparison of naive tensor vs tensor product (if feasible from 4_distributed_garbling.tex)

**Paper reference:** `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/` — 5_online.tex (Protocol 1/2), appendix_cpre.tex (compressed preprocessing), 4_distributed_garbling.tex (distributed half gates)

## Requirements

### Validated

*(from existing working online phase — pre-milestone)*
- ✓ AuthBitShare IT-MAC structure: `mac = key XOR bit * delta` — existing
- ✓ Key LSB = 0 invariant — existing
- ✓ Delta LSB = 1 invariant — existing
- ✓ Shared IdealBCot for same Δ across triples — existing

**Phase 1 — M1 Primitives & Sharing Cleanup (v1.0):**
- ✓ CLEAN-01: Key::new enforces LSB=0 at construction — v1.0
- ✓ CLEAN-02: AuthBitShare/AuthBit scopes documented — v1.0
- ✓ CLEAN-03: InputSharing::shares_differ() replaces bit() — v1.0
- ✓ CLEAN-04: build_share documents Key LSB=0 dependency — v1.0
- ✓ CLEAN-05: pub(crate) narrowing; column-major docs — v1.0
- ✓ CLEAN-06: FIXED_KEY_AES Lazy thread-safety documented — v1.0

**Phase 2 — M1 Online + Ideal Fpre + Benches (v1.0):**
- ✓ CLEAN-07: generate_for_ideal_trusted_dealer rename — v1.0
- ✓ CLEAN-08: preprocessing.rs module separation — v1.0
- ✓ CLEAN-09: TensorFpreGen/Eval per-field docs — v1.0
- ✓ CLEAN-10: auth_tensor_gen/eval dead code removed, comments added — v1.0
- ✓ CLEAN-11: auth_gen.rs / auth_eval.rs confirmed absent — v1.0
- ✓ CLEAN-12: benchmarks deduplicated, paper-protocol headers — v1.0

**Phase 3 — Generalized Tensor Macro (v1.0):**
- ✓ PROTO-01: tensor_garbler (Construction 1 garbler) — v1.0
- ✓ PROTO-02: tensor_evaluator (Construction 1 evaluator) — v1.0
- ✓ PROTO-03: Z_garbler XOR Z_evaluator == a ⊗ T (10-test battery) — v1.0
- ✓ TEST-01: Paper-invariant GGM test battery — v1.0

**Phase 4 — Pi_LeakyTensor + F_eq (v1.0):**
- ✓ PROTO-04..08: Full Pi_LeakyTensor generate() body — v1.0
- ✓ PROTO-09: LeakyTriple = exact paper shape (no gamma, no wire labels) — v1.0
- ✓ TEST-02..04: MAC invariant, product invariant, F_eq abort — v1.0

**Phase 5 — Pi_aTensor Correct Combining (v1.0):**
- ✓ PROTO-10: two_to_one_combine (paper §3.2 algebra) — v1.0
- ✓ PROTO-11: combine_leaky_triples iterative fold — v1.0
- ✓ PROTO-12: bucket_size_for(ell) with correct ℓ — v1.0
- ✓ TEST-05: Combining correctness 3-test battery — v1.0

**Phase 6 — Pi_aTensor' Permutation Bucketing (v1.0):**
- ✓ PROTO-13: Per-triple Fisher-Yates row permutation — v1.0
- ✓ PROTO-14: apply_permutation_to_triple (x-rows + Z i-indices; y unchanged) — v1.0
- ✓ PROTO-15: bucket_size_for(n, ell) Construction 4 formula — v1.0
- ✓ TEST-06: End-to-end Construction 4 regression — v1.0
- ✓ TEST-07: cargo bench --no-run clean — v1.0

### Active

v1.1 requirements — see REQUIREMENTS.md (to be defined).

### Out of Scope

- Real network OT (Ferret/IKNP) — ideal F_bCOT stays for now; real OT is v2
- Malicious security proof verification — correctness of honest-party execution only
- Online garbling phase changes beyond what preprocessing restructure requires
- Circuit-level changes (`circuits/` submodule)
- Semi-honest family (`tensor_gen`, `tensor_eval`, `tensor_pre`) — referenced in lib.rs integration test only

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Implement Pi_LeakyTensor via GGM tree macro | Paper spec; current direct-AND approach was not the protocol | ✓ Done (Phase 3) |
| In-process F_eq (ideal) | Matches IdealBCot pattern; no networking needed | ✓ Done (Phase 4) |
| Pi_aTensor' (permutation bucketing) over Pi_aTensor | Better bucket size (log(nℓ) vs log(ℓ)), user explicitly wants it | ✓ Done (Phase 6) |
| Keep TensorFpreGen/Eval interface | Online phase already correct; minimize scope | ✓ Done (Phase 2) |
| tensor_macro as standalone pub(crate) module | No dependency on leaky_tensor_pre or preprocessing — clean separation | ✓ Done (Phase 3) |
| Hard-code shuffle_seed=42 in run_preprocessing | Deterministic permutations for test reproducibility | ✓ Done (Phase 6) — caller should supply seed for production |
| Construction 4 replaces Construction 3 fully — no parallel path | Single bucket-sizer; no dead one-argument version | ✓ Done (Phase 6) |
| Verifier-delta COT convention throughout | Paper specifies A's bits under Δ_B, B's bits under Δ_A | ✓ Done (Phase 4) — COT convention task became moot |

## Evolution

**After v1.0 milestone:**
- All 8 known algorithmic bugs resolved
- All 34 v1 requirements validated
- 95/95 tests passing (Phase 8 added 13 new tests)
- Preprocessing pipeline is paper-correct end-to-end
- Phase 8 complete: online phase for Protocol 1 — gamma_auth_bit_shares forwarded, compute_lambda_gamma on both structs, check_zero primitive, end-to-end P1-04/P1-05 integration tests

Next milestone: After v1.1 — v2.0 focusing on real OT (Ferret/IKNP) and network layer.

---
*Last updated: 2026-04-25 — Phase 9 complete: Protocol 2 garble/eval implemented and verified (P2-01..P2-05).*
