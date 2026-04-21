# Requirements

## Scope

Two milestones. Milestone 1 (this document) covers M1 + M2 together, split into two phases of the roadmap.

**M1 — Codebase Cleanup:** Refactor stable pre-April-10 layers (primitives, online garbling, ideal TensorFpre, matrix/tensor ops, benchmarks). No algorithmic changes — only structure, abstraction, and naming improvements.

**M2 — Protocol Implementation:** Implement the correct Pi_LeakyTensor, Pi_aTensor, and Pi_aTensor' protocols per Appendix F. Fix all known algorithmic bugs. Write paper-invariant tests.

---

## v1 Requirements

### M1 — Codebase Cleanup (Stable Layers)

**Primitives (block, delta, keys, macs, aes, sharing, matrix, tensor_ops)**
- [ ] **CLEAN-01**: Enforce and document the `Key.lsb() == 0` invariant at the type level (construction-time guarantee, not just asserts)
- [ ] **CLEAN-02**: Clarify `AuthBitShare` vs `AuthBit` — `AuthBitShare` holds one party's view; `AuthBit` holds both views. Field names (`gen_share`/`eval_share`) should reflect what's in scope, not both parties' full state
- [ ] **CLEAN-03**: `InputSharing.bit()` is confusingly named — it returns whether gen_share != eval_share, not the actual input bit. Rename or document clearly
- [ ] **CLEAN-04**: `build_share` in `sharing.rs` ignores the Key LSB=0 invariant (uses raw random without clearing LSB). Fix or remove
- [ ] **CLEAN-05**: Matrix / tensor_ops — audit for unused public API, missing doc comments on non-obvious invariants, and column-major indexing documented at the type level
- [ ] **CLEAN-06**: `aes.rs` singleton pattern — document why `once_cell::Lazy` is used and thread-safety guarantees

**Ideal TensorFpre**
- [ ] **CLEAN-07**: `TensorFpre::generate_with_input_values` — rename to `generate_for_ideal_trusted_dealer` or clearly separate it from the real protocol. Add a doc comment explaining it is the ideal functionality (trusted dealer), NOT the real preprocessing protocol
- [ ] **CLEAN-08**: Separate `TensorFpre` (ideal trusted dealer) from `TensorFpreGen`/`TensorFpreEval` (real protocol output structs). The real structs belong in a `preprocessing` module, not mixed with the ideal functionality
- [ ] **CLEAN-09**: `TensorFpreGen` and `TensorFpreEval` should have doc comments specifying exactly what each field represents and which party holds it

**Online Garbling (auth_tensor_gen, auth_tensor_eval)**
- [ ] **CLEAN-10**: Audit `auth_tensor_gen.rs` and `auth_tensor_eval.rs` for dead code, unexplained magic constants, and comment any non-obvious protocol steps
- [ ] **CLEAN-11**: Remove or isolate `src/auth_gen.rs`, `src/auth_eval.rs` if they are unused legacy files

**Benchmarks**
- [ ] **CLEAN-12**: `benches/benchmarks.rs` — remove duplicated setup code, extract shared helpers, add comments identifying which benchmark corresponds to which paper protocol

### M2 — Protocol Implementation (Phase 1 Rewrite)

**Generalized Tensor Macro (Construction 1)**
- [ ] **PROTO-01**: Implement `tensor_garbler(n, m, Δ_A, itmac{A}{Δ}, T^A)` — GGM tree construction with 2^n leaves, ciphertext generation G_{i,b}, leaf expansion to X_{l,k}, output Z_garbler and G
- [ ] **PROTO-02**: Implement `tensor_evaluator(n, m, G, itmac{A}{Δ}^eval, T^eval)` — reproduce untraversed subtree from A_i ⊕ a_i·Δ, recover X_{a,k} from ciphertexts, output Z_evaluator
- [ ] **PROTO-03**: Correctness invariant test: `Z_eval XOR Z_garbler == a ⊗ T` for all test vectors

**Pi_LeakyTensor (Construction 2)**
- [ ] **PROTO-04**: Obtain correlated randomness from F_bCOT: `itmac{x_A}{Δ_B}`, `itmac{x_B}{Δ_A}`, `itmac{y_A}{Δ_B}`, `itmac{y_B}{Δ_A}`, `itmac{R}{Δ}`
- [ ] **PROTO-05**: Compute C_A and C_B (XOR combinations of y and R correlations under Δ_A ⊕ Δ_B)
- [ ] **PROTO-06**: Execute two tensor macro calls (A as garbler with Δ_A, B as garbler with Δ_B) and XOR results
- [ ] **PROTO-07**: Masked tensor reveal: compute lsb(S1) ⊕ lsb(S2) = D (revealed publicly), then compute `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}`
- [ ] **PROTO-08**: F_eq consistency check: parties compute L_1 = S_1 ⊕ D·Δ_A and L_2 = S_2 ⊕ D·Δ_B, ideal F_eq checks equality; abort if check fails
- [ ] **PROTO-09**: LeakyTriple output is `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` only — remove gamma bits and wire labels from the struct

**Pi_aTensor (Construction 3)**
- [ ] **PROTO-10**: Implement correct combining procedure: keep x = x', y = y', reveal d = y' ⊕ y'' (with MAC verification), compute `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d`
- [ ] **PROTO-11**: Iterative combining: fold B leaky triples one at a time using the two-to-one combine
- [ ] **PROTO-12**: Fix bucket size formula: `B = floor(SSP / log2(ℓ)) + 1` where ℓ is the number of OUTPUT authenticated tensor triples (not n·m)

**Pi_aTensor' (Construction 4)**
- [ ] **PROTO-13**: Sample uniformly random permutation π_j ∈ S_n per triple before bucketing
- [ ] **PROTO-14**: Apply π_j to rows of `itmac{x^(j)}{Δ}` and rows of `itmac{Z^(j)}{Δ}`; leave `itmac{y^(j)}{Δ}` unchanged
- [ ] **PROTO-15**: Bucket size formula: `B = 1 + ceil(SSP / log2(n·ℓ))`

**Paper-Invariant Tests**
- [ ] **TEST-01**: GGM macro: `Z_garbler XOR Z_evaluator == a ⊗ T` for multiple (n, m, T) combinations
- [ ] **TEST-02**: Leaky triple IT-MAC invariant: `mac = key XOR bit · delta` under verifier's delta for each share in the triple
- [ ] **TEST-03**: Leaky triple product invariant: `Z_full = x_full ⊗ y_full` (XOR of gen+eval Z shares = tensor product of XOR of gen+eval x and y shares)
- [ ] **TEST-04**: F_eq: correct L values pass; malformed L values cause abort
- [ ] **TEST-05**: Pi_aTensor combining: `Z_combined = Z' ⊕ Z'' ⊕ x'' ⊗ d` for two test triples
- [ ] **TEST-06**: Pi_aTensor' output triple: combined Z satisfies `itmac{Z}{Δ}` where Z = x ⊗ y
- [ ] **TEST-07**: Benchmarks compile and run after restructure

---

## v2 Requirements (Deferred)

- Real OT protocol (Ferret/IKNP) replacing ideal F_bCOT
- Actual network communication layer
- Malicious security simulation proof

---

## Out of Scope

- Online garbling phase algorithmic changes — only consume the preprocessing output as-is
- Circuit-level changes (`circuits/` submodule)
- Semi-honest family (`tensor_gen`, `tensor_eval`, `tensor_pre`) — referenced in lib.rs integration test only

---

## Traceability

| Req      | Phase   | Status  |
|----------|---------|---------|
| CLEAN-01 | Phase 1 | Pending |
| CLEAN-02 | Phase 1 | Pending |
| CLEAN-03 | Phase 1 | Pending |
| CLEAN-04 | Phase 1 | Pending |
| CLEAN-05 | Phase 1 | Pending |
| CLEAN-06 | Phase 1 | Pending |
| CLEAN-07 | Phase 2 | Pending |
| CLEAN-08 | Phase 2 | Pending |
| CLEAN-09 | Phase 2 | Pending |
| CLEAN-10 | Phase 2 | Pending |
| CLEAN-11 | Phase 2 | Pending |
| CLEAN-12 | Phase 2 | Pending |
| PROTO-01 | Phase 3 | Pending |
| PROTO-02 | Phase 3 | Pending |
| PROTO-03 | Phase 3 | Pending |
| PROTO-04 | Phase 4 | Pending |
| PROTO-05 | Phase 4 | Pending |
| PROTO-06 | Phase 4 | Pending |
| PROTO-07 | Phase 4 | Pending |
| PROTO-08 | Phase 4 | Pending |
| PROTO-09 | Phase 4 | Pending |
| PROTO-10 | Phase 5 | Pending |
| PROTO-11 | Phase 5 | Pending |
| PROTO-12 | Phase 5 | Pending |
| PROTO-13 | Phase 6 | Pending |
| PROTO-14 | Phase 6 | Pending |
| PROTO-15 | Phase 6 | Pending |
| TEST-01  | Phase 3 | Pending |
| TEST-02  | Phase 4 | Pending |
| TEST-03  | Phase 4 | Pending |
| TEST-04  | Phase 4 | Pending |
| TEST-05  | Phase 5 | Pending |
| TEST-06  | Phase 6 | Pending |
| TEST-07  | Phase 6 | Pending |

**Coverage:** 34 / 34 v1 requirements mapped (100%)
