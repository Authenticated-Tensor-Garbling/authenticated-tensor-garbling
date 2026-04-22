# Roadmap: Authenticated Tensor Garbling — Preprocessing Fix

## Overview

This roadmap delivers a paper-faithful implementation of the KRRW-style uncompressed preprocessing protocol (Appendix F) for authenticated tensor garbling. Work is split across two milestones: **M1** refactors the stable pre-April-10 codebase (primitives, online garbling, ideal `TensorFpre`, benchmarks) without algorithmic changes; **M2** replaces the broken Phase 1 preprocessing with correct implementations of the Generalized Tensor Macro (Construction 1), Pi_LeakyTensor (Construction 2), Pi_aTensor (Construction 3), and Pi_aTensor' (Construction 4), plus paper-invariant tests that verify the IT-MAC structure and combining identity rather than echoing the code.

## Milestones

- **M1 — Codebase Cleanup:** Phases 1-2 (structure, naming, abstraction on stable layers; no algorithm changes)
- **M2 — Protocol Implementation:** Phases 3-6 (correct protocol per paper Appendix F; paper-invariant tests)

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

- [ ] **Phase 1: M1 Primitives & Sharing Cleanup** - Refactor block/delta/keys/macs/aes/sharing/matrix/tensor_ops for invariants, naming, and docs
- [ ] **Phase 2: M1 Online + Ideal Fpre + Benches Cleanup** - Refactor `auth_tensor_gen/eval`, separate ideal `TensorFpre` from `TensorFpreGen/Eval`, clean benchmarks
- [ ] **Phase 3: M2 Generalized Tensor Macro (Construction 1)** - Implement GGM-tree garbler/evaluator tensor macro with correctness invariant tests
- [ ] **Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2)** - Rewrite bCOT consumption, two-macro construction, masked reveal, in-process F_eq
- [ ] **Phase 5: M2 Pi_aTensor Correct Combining (Construction 3)** - Implement paper-correct two-to-one combining and fix bucket size formula
- [ ] **Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches** - Implement permutation bucketing with `B = 1 + ceil(SSP / log2(n·ℓ))` and restore benchmarks

## Phase Details

### Phase 1: M1 Primitives & Sharing Cleanup
**Goal**: Stable pre-April-10 primitives (block, delta, keys, macs, aes, sharing, matrix, tensor_ops) enforce invariants at construction time, are correctly named, and have documentation calling out non-obvious behavior — with zero algorithmic changes.
**Depends on**: Nothing (first phase)
**Requirements**: CLEAN-01, CLEAN-02, CLEAN-03, CLEAN-04, CLEAN-05, CLEAN-06
**Success Criteria** (what must be TRUE):
  1. `Key` construction guarantees `lsb() == 0` at the type level; no code path can produce a `Key` with LSB set
  2. `AuthBitShare` (one party's view) and `AuthBit` (both parties' views) are distinct types with field names that reflect scope, and `build_share` respects the Key LSB=0 invariant
  3. `InputSharing.bit()` is either renamed or documented so its return value (XOR of shares, not the underlying input bit) is unambiguous
  4. `matrix` and `tensor_ops` have doc comments on column-major indexing and public API, with unused APIs removed or marked `pub(crate)`
  5. `cargo build` and the full existing test suite pass unchanged after cleanup
**Plans**: 3 plans
  - [ ] 01-PLAN-keys-sharing.md — Enforce Key LSB=0 invariant via Key::new; fix build_share; rename InputSharing::bit to shares_differ; add AuthBitShare/AuthBit docs (CLEAN-01..04)
  - [ ] 01-PLAN-matrix-ops-aes.md — Narrow tensor_ops and matrix view types to pub(crate); document column-major indexing; document FIXED_KEY_AES singleton (CLEAN-05, CLEAN-06)
  - [ ] 01-PLAN-bcot-migration.md — Migrate src/bcot.rs set_lsb+Key::from two-step to Key::new (CLEAN-01 follow-through)

### Phase 2: M1 Online + Ideal Fpre + Benches Cleanup
**Goal**: Online garbling (`auth_tensor_gen`, `auth_tensor_eval`), ideal `TensorFpre`, and benchmarks are refactored so the ideal trusted-dealer path is separated from real-protocol output structs, dead code is removed, and benchmark setup is deduplicated — with zero algorithmic changes.
**Depends on**: Phase 1
**Requirements**: CLEAN-07, CLEAN-08, CLEAN-09, CLEAN-10, CLEAN-11, CLEAN-12
**Success Criteria** (what must be TRUE):
  1. `TensorFpre::generate_with_input_values` is renamed (e.g. `generate_for_ideal_trusted_dealer`) and documented as the ideal functionality, not the real protocol
  2. `TensorFpreGen` and `TensorFpreEval` live in a `preprocessing` module separate from the ideal `TensorFpre`, and each field is documented with which party holds it and what it represents
  3. `auth_tensor_gen.rs` / `auth_tensor_eval.rs` have no dead code, magic constants are named or commented, and `auth_gen.rs` / `auth_eval.rs` are removed if unused
  4. `benches/benchmarks.rs` has shared setup helpers (no duplicated scaffolding) and each benchmark identifies the paper protocol it measures
  5. `cargo build`, full test suite, and `cargo bench` all run green after cleanup
**Plans**: 4 plans
  - [x] 02-01-PLAN.md — Wave 0 prerequisites: baseline test snapshot, empty src/preprocessing.rs skeleton + lib.rs module decl; CLEAN-11 trivially satisfied (auth_gen.rs / auth_eval.rs confirmed absent)
  - [x] 02-02-PLAN.md — Module migration + generate rename + gamma cascade end-to-end: move TensorFpreGen/Eval + run_preprocessing to preprocessing.rs; rename generate_with_input_values -> generate_for_ideal_trusted_dealer; remove gamma_* fields and populators across TensorFpre, TensorFpreGen/Eval, AuthTensorGen/Eval, combine_leaky_triples; add /// field docs (CLEAN-07, CLEAN-08, CLEAN-09, CLEAN-10 partial)
  - [x] 02-03-PLAN.md — Benchmark deduplication + rename follow-through + paper-protocol header comments: collapse bench_full_protocol_garbling (and _with_networking where structurally identical) into a single loop over [1, 2, 4, 6, 8] preserving Criterion BenchmarkIds; redirect run_preprocessing import (CLEAN-12)
  - [x] 02-04-PLAN.md — auth_tensor_gen/eval doc + comment audit (runs parallel with 02-03): /// doc on garble_final + evaluate_final; remove 'awful return type' comment; add GGM tweak domain-separation comment (CLEAN-10 completion)

### Phase 3: M2 Generalized Tensor Macro (Construction 1)
**Goal**: The Generalized Tensor Macro from paper Construction 1 exists as a reusable Rust primitive: garbler builds a GGM tree of depth n, produces ciphertexts G, and outputs `Z_garbler`; evaluator reproduces the untraversed subtree, recovers leaves, and outputs `Z_evaluator` such that `Z_garbler XOR Z_evaluator = a ⊗ T`.
**Depends on**: Phase 2
**Requirements**: PROTO-01, PROTO-02, PROTO-03, TEST-01
**Success Criteria** (what must be TRUE):
  1. `tensor_garbler(n, m, Δ_A, itmac{A}{Δ}, T^A)` builds a 2^n-leaf GGM tree, emits ciphertexts `G_{i,b}`, and returns `Z_garbler` and `G`
  2. `tensor_evaluator(n, m, G, itmac{A}{Δ}^eval, T^eval)` reproduces the untraversed subtree from `A_i ⊕ a_i·Δ`, recovers `X_{a,k}` using `G`, and returns `Z_evaluator`
  3. `Z_garbler XOR Z_evaluator == a ⊗ T` holds across a battery of `(n, m, T)` test vectors including edge cases (n=1, small m, large m)
  4. Macro primitive is module-scoped with clear input/output types and no dependency on LeakyTriple state
**Plans**: 3 plans
  - [x] 03-01-PLAN.md — Wave 0: capture test baseline; add BlockMatrix::elements_slice; generalize gen_populate_seeds_mem_optimized to &[Block]; hoist eval_populate_seeds_mem_optimized and eval_unary_outer_product into tensor_ops.rs; rewire tensor_gen/tensor_eval/auth_tensor_eval; create src/tensor_macro.rs skeleton + register in lib.rs (PROTO-01, PROTO-02 scaffolding)
  - [x] 03-02-PLAN.md — Implement tensor_garbler and tensor_evaluator bodies in src/tensor_macro.rs composing the tensor_ops kernels (PROTO-01, PROTO-02)
  - [x] 03-03-PLAN.md — Paper-invariant test battery verifying Z_gen XOR Z_eval == a ⊗ T across (n, m) edge cases (n=1, small m, large n/m) plus a deterministic regression seed (PROTO-03, TEST-01)

### Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2)
**Goal**: `Pi_LeakyTensor` is implemented per paper Construction 2: consume correlated randomness from `IdealBCot`, run two tensor-macro calls (A and B as garblers under their own Δ), XOR results, execute masked reveal, verify consistency via in-process `F_eq`, and output a leaky triple whose shape is exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` — no gamma, no wire labels.
**Depends on**: Phase 3
**Requirements**: PROTO-04, PROTO-05, PROTO-06, PROTO-07, PROTO-08, PROTO-09, TEST-02, TEST-03, TEST-04
**Success Criteria** (what must be TRUE):
  1. `Pi_LeakyTensor::generate` consumes `itmac{x_A}{Δ_B}`, `itmac{x_B}{Δ_A}`, `itmac{y_A}{Δ_B}`, `itmac{y_B}{Δ_A}`, `itmac{R}{Δ}` from `IdealBCot` and does NOT call `alpha AND beta` directly
  2. Two tensor-macro invocations (A and B as garblers) are XORed with the `C_A`/`C_B` correlations under `Δ_A ⊕ Δ_B`, and masked reveal yields public `D = lsb(S_1) ⊕ lsb(S_2)` with `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}`
  3. In-process `F_eq` receives `L_1 = S_1 ⊕ D·Δ_A` and `L_2 = S_2 ⊕ D·Δ_B`; matching inputs pass, mismatched inputs abort — verified by test
  4. `LeakyTriple` struct contains exactly `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})`; gamma bits and wire labels are removed
  5. Tests verify paper invariants: IT-MAC equation `mac = key XOR bit·Δ` holds on every share, and `XOR(gen, eval)` of `Z` equals tensor product of `XOR(gen, eval)` of `x` and `y`
**Plans**: 3 plans
  - [ ] 01-PLAN-keys-sharing.md — Enforce Key LSB=0 invariant via Key::new; fix build_share; rename InputSharing::bit to shares_differ; add AuthBitShare/AuthBit docs (CLEAN-01..04)
  - [ ] 01-PLAN-matrix-ops-aes.md — Narrow tensor_ops and matrix view types to pub(crate); document column-major indexing; document FIXED_KEY_AES singleton (CLEAN-05, CLEAN-06)
  - [ ] 01-PLAN-bcot-migration.md — Migrate src/bcot.rs set_lsb+Key::from two-step to Key::new (CLEAN-01 follow-through)

### Phase 5: M2 Pi_aTensor Correct Combining (Construction 3)
**Goal**: `Pi_aTensor` combines B leaky triples into one authenticated tensor triple using the paper's two-to-one procedure (keep `x = x'`, `y = y'`, reveal `d = y' ⊕ y''`, compute `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d`) with MAC verification on `d`, and the bucket size formula uses the correct `ℓ` (number of output triples).
**Depends on**: Phase 4
**Requirements**: PROTO-10, PROTO-11, PROTO-12, TEST-05
**Success Criteria** (what must be TRUE):
  1. Two-to-one combine implements `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d` with `d` revealed only after MAC verification; replaces the naive XOR of all shares
  2. Bucket size computation uses `B = floor(SSP / log2(ℓ)) + 1` with `ℓ` = number of output authenticated tensor triples (not `n·m`)
  3. Iterative combining folds B leaky triples one at a time into a single authenticated triple
  4. Test verifies `Z_combined = Z' ⊕ Z'' ⊕ x'' ⊗ d` on two concrete leaky triples and confirms MAC on `d` rejects tampered values
**Plans**: 3 plans
  - [ ] 01-PLAN-keys-sharing.md — Enforce Key LSB=0 invariant via Key::new; fix build_share; rename InputSharing::bit to shares_differ; add AuthBitShare/AuthBit docs (CLEAN-01..04)
  - [ ] 01-PLAN-matrix-ops-aes.md — Narrow tensor_ops and matrix view types to pub(crate); document column-major indexing; document FIXED_KEY_AES singleton (CLEAN-05, CLEAN-06)
  - [ ] 01-PLAN-bcot-migration.md — Migrate src/bcot.rs set_lsb+Key::from two-step to Key::new (CLEAN-01 follow-through)

### Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches
**Goal**: `Pi_aTensor'` is implemented per paper Construction 4 with uniform row-permutation bucketing and the improved bucket size `B = 1 + ceil(SSP / log2(n·ℓ))`; the end-to-end preprocessing pipeline produces a valid authenticated tensor triple, and benchmarks run after the full restructure.
**Depends on**: Phase 5
**Requirements**: PROTO-13, PROTO-14, PROTO-15, TEST-06, TEST-07
**Success Criteria** (what must be TRUE):
  1. A uniformly random permutation `π_j ∈ S_n` is sampled per triple before bucketing
  2. `π_j` is applied to rows of `itmac{x^(j)}{Δ}` and rows of `itmac{Z^(j)}{Δ}`; `itmac{y^(j)}{Δ}` is unchanged
  3. Bucket size is `B = 1 + ceil(SSP / log2(n·ℓ))` and matches the paper's Construction 4 formula
  4. End-to-end test: output authenticated tensor triple satisfies `itmac{Z}{Δ}` with `Z = x ⊗ y` where `x, y, Z` are the XOR of both parties' shares
  5. `cargo bench` runs the preprocessing benchmark successfully after the restructure is complete
**Plans**: 3 plans
  - [ ] 01-PLAN-keys-sharing.md — Enforce Key LSB=0 invariant via Key::new; fix build_share; rename InputSharing::bit to shares_differ; add AuthBitShare/AuthBit docs (CLEAN-01..04)
  - [ ] 01-PLAN-matrix-ops-aes.md — Narrow tensor_ops and matrix view types to pub(crate); document column-major indexing; document FIXED_KEY_AES singleton (CLEAN-05, CLEAN-06)
  - [ ] 01-PLAN-bcot-migration.md — Migrate src/bcot.rs set_lsb+Key::from two-step to Key::new (CLEAN-01 follow-through)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. M1 Primitives & Sharing Cleanup | 0/3 | Not started | - |
| 2. M1 Online + Ideal Fpre + Benches Cleanup | 0/TBD | Not started | - |
| 3. M2 Generalized Tensor Macro | 0/TBD | Not started | - |
| 4. M2 Pi_LeakyTensor + F_eq | 0/TBD | Not started | - |
| 5. M2 Pi_aTensor Correct Combining | 0/TBD | Not started | - |
| 6. M2 Pi_aTensor' Permutation Bucketing + Benches | 0/TBD | Not started | - |

## References

- `references/appendix_krrw_pre.tex` — protocol specification (Appendix F)
- `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf` — main paper
- `references/2017-030-2.pdf` — WRK17 (leaky AND triples + bucketing)
- `references/2018-578-3.pdf` — KRRW18 (preprocessing for authenticated garbling)
- `references/mpz-dev/` — MPZ reference implementation
