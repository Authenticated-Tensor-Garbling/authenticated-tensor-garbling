# Roadmap: Authenticated Tensor Garbling — Preprocessing Fix

## Milestones

- ✅ **v1.0 Preprocessing Fix** — Phases 1–6 (shipped 2026-04-23)
- ◻ **v1.1 Full Protocol Demonstration + Benchmarks** — Phases 7–10 (active)

## Phases

<details>
<summary>✅ v1.0 Preprocessing Fix (Phases 1–6) — SHIPPED 2026-04-23</summary>

- [x] Phase 1: M1 Primitives & Sharing Cleanup (3/3 plans) — completed 2026-04-22
- [x] Phase 2: M1 Online + Ideal Fpre + Benches Cleanup (4/4 plans) — completed 2026-04-22
- [x] Phase 3: M2 Generalized Tensor Macro (Construction 1) (3/3 plans) — completed 2026-04-22
- [x] Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) (3/3 plans) — completed 2026-04-22
- [x] Phase 5: M2 Pi_aTensor Correct Combining (Construction 3) (3/3 plans) — completed 2026-04-22
- [x] Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches (3/3 plans) — completed 2026-04-23

Full phase details: [`.planning/milestones/v1.0-ROADMAP.md`](.planning/milestones/v1.0-ROADMAP.md)

</details>

## v1.1 Phases

- [ ] **Phase 7: Preprocessing Trait + Ideal Backends** - Define TensorPreprocessing trait; IdealPreprocessingBackend; extend TensorFpreGen/Eval with gamma_auth_bit_shares consistency-check field (PRE-05 deferred to v3)
- [ ] **Phase 8: Open() + Protocol 1 Garble/Eval/Check** - open() free function; Protocol 1 complete garble, evaluate, and CheckZero; positive and negative tests
- [ ] **Phase 9: Protocol 2 Garble/Eval/Check** - Wide seed expansion; Protocol 2 garble (_p2), evaluate (_p2), consistency check; end-to-end test
- [x] **Phase 10: Wall-Clock Benchmarks** - black_box all outputs; iter_custom throughput benchmarks; preprocessing vs online comparison; distributed half gates vs naive tensor (completed 2026-04-25)

## Phase Details

### Phase 7: Preprocessing Trait + Ideal Backends
**Goal**: All preprocessing backends are interchangeable through a single trait; TensorFpreGen/Eval carry the fields needed for the consistency check
**Depends on**: Phase 6 (v1.0 complete)
**Requirements**: PRE-01, PRE-02, PRE-03, PRE-04 (PRE-05 deferred to v3 per D-10)
**Plans**: 3 plans

Plans:
- [ ] 07-01-PLAN.md — PRE-04: Add gamma_auth_bit_shares field to TensorFpreGen/TensorFpreEval; update all construction sites atomically
- [ ] 07-02-PLAN.md — PRE-01/02/03: Define TensorPreprocessing trait; implement UncompressedPreprocessingBackend and IdealPreprocessingBackend
- [ ] 07-03-PLAN.md — Tests: PRE-01/02/03/04 verification (trait dispatch, backend correctness, IT-MAC invariant on gamma shares)

**Success Criteria** (what must be TRUE):
  1. A caller can swap IdealPreprocessingBackend for pi'_atensor (run_preprocessing) by changing only the concrete type — no call-site changes to the online phase
  2. TensorFpreGen and TensorFpreEval each compile with the new gamma_auth_bit_shares field and every existing constructor initializes it without error
  3. IdealPreprocessingBackend::run() returns a (TensorFpreGen, TensorFpreEval) pair whose D_gb and D_ev MAC values satisfy the IT-MAC invariant (mac = key XOR bit * delta)
  4. cargo test passes with zero regressions after the struct field and trait additions

### Phase 8: Open() + Protocol 1 Garble/Eval/Check
**Goal**: Users of the online phase can execute a full Protocol 1 tensor gate — garble, evaluate, open masked wire values, and confirm output correctness via CheckZero — with tests catching both wrong-delta and tampered-mask failure modes
**Depends on**: Phase 7
**Requirements**: ONL-01, ONL-02, P1-01, P1-02, P1-03, P1-04, P1-05
**Success Criteria** (what must be TRUE):
  1. open() called with the correct D_gb delta on a garbler-input wire and D_ev delta on an output wire returns the correct unmasked bit in both cases
  2. open() called with the wrong delta returns an incorrect bit (not a panic), asserting this in a dedicated negative test
  3. Protocol 1 garble + evaluate together produce Z_garbler XOR Z_evaluator == correct tensor product (extends the v1.0 battery to the full online protocol including l_gamma* XOR)
  4. CheckZero on D_ev MAC shares including the l_gamma* preprocessing term passes for honest parties and aborts when L_gamma is tampered (two separate test cases)
  5. cargo test passes with zero regressions on all v1.0 tests
**Plans**: 3 plans

Plans:
- [x] 08-01-PLAN.md — P1-01/P1-02: Forward gamma_auth_bit_shares + add compute_lambda_gamma to AuthTensorGen and AuthTensorEval
- [x] 08-02-PLAN.md — P1-03 (+ ONL-01/02 deferred per D-01): Create src/online.rs with check_zero primitive; wire `pub mod online;` in lib.rs
- [x] 08-03-PLAN.md — P1-04/P1-05: End-to-end Protocol 1 positive test + tampered-lambda negative test in src/lib.rs (uses IdealPreprocessingBackend)

**UI hint**: no

### Phase 9: Protocol 2 Garble/Eval/Check
**Goal**: Protocol 2 tensor gate is complete — garbler never reveals masked values; evaluator holds D_ev-authenticated output shares; consistency check verifies correctness without leaking garbler secrets
**Depends on**: Phase 7
**Requirements**: P2-01, P2-02, P2-03, P2-04, P2-05
**Success Criteria** (what must be TRUE):
  1. gen_unary_outer_product_wide produces (kappa+rho)-bit leaf seed expansions and the D_gb/D_ev shares it generates satisfy the IT-MAC invariant
  2. Protocol 2 garble (_p2 variant) completes without ever sending the masked wire value to the evaluator — verifiable by inspecting the function's return type and call sites
  3. Protocol 2 evaluate (_p2 variant) produces output wire shares that are D_ev-authenticated (MAC check passes)
  4. Protocol 2 consistency check passes for honest parties and the Protocol 1 tests remain unmodified and green
  5. A single end-to-end Protocol 2 test verifies garbler XOR evaluator output equals the correct tensor product under the _p2 variant path
**Plans**: 4 plans

Plans:
- [x] 09-01-PLAN.md — P2-01: Rename gamma_auth_bit_shares -> gamma_d_ev_shares + add three new D_ev preprocessing fields (alpha/beta/correlated_d_ev_shares) + IdealPreprocessingBackend gen for all four; atomic across preprocessing.rs, auth_tensor_gen/eval.rs, auth_tensor_pre/fpre.rs, lib.rs
- [x] 09-02-PLAN.md — P2-01: gen_unary_outer_product_wide and eval_unary_outer_product_wide in tensor_ops.rs (κ+ρ wide leaf expansion via even/odd TCCR tweak); unit tests for tweak independence + round-trip
- [x] 09-03-PLAN.md — P2-02/P2-03: _p2 garble/evaluate methods on AuthTensorGen and AuthTensorEval (garble_first_half_p2 / garble_second_half_p2 / garble_final_p2; evaluate_*_p2); D_ev accumulator matrices; wide chunked helpers
- [x] 09-04-PLAN.md — P2-04/P2-05: assemble_c_gamma_shares_p2 helper + test_auth_tensor_product_full_protocol_2 integration test (check_zero under delta_b; D_gb correctness mirroring P1-04)

### Phase 10: Wall-Clock Benchmarks
**Goal**: All garbling benchmarks correctly measure wall-clock time (no dead-code elimination, no async overhead), preprocessing and online phases are isolated into separate criterion groups, and Protocol 2 garble/evaluate/check is benchmarked alongside Protocol 1 with dual-unit throughput reporting (ms-per-tensor-op + ns-per-AND-gate). BENCH-05 (distributed half-gates) is deferred to v2 per Phase 10 CONTEXT D-01 (paper section marked "TODO, scrap").
**Depends on**: Phase 8, Phase 9
**Requirements**: BENCH-01, BENCH-02, BENCH-04, BENCH-05 (deferred to v2), BENCH-06
**Success Criteria** (what must be TRUE):
  1. Every garbling benchmark output is wrapped in std::hint::black_box; cargo bench --release runs without the compiler eliminating any measured computation
  2. Protocol 2 garbling throughput benchmark uses iter_custom + std::time::Instant and reports nanoseconds per gate alongside total throughput
  3. A criterion benchmark group named "preprocessing" and a separate group named "online" exist and isolate the two phases cleanly
  4. cargo bench --no-run exits with zero errors and all benchmark binaries compile in release mode
**Plans**: 3 plans

Plans:
- [x] 10-01-PLAN.md — Promote assemble_c_gamma_shares + assemble_c_gamma_shares_p2 from #[cfg(test)] private to pub fn at crate root (unblocks bench access)
- [x] 10-02-PLAN.md — Refactor bench_preprocessing to sync iter_custom + Instant; deduplicate seven async network benches into one parameterized helper; apply black_box to all measured outputs
- [x] 10-03-PLAN.md — Add bench_online_p1 + bench_online_p2 in new "online" group with dual-unit throughput; restructure criterion_main! to three groups (preprocessing/online/network)

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. M1 Primitives & Sharing Cleanup | v1.0 | 3/3 | Complete | 2026-04-22 |
| 2. M1 Online + Ideal Fpre + Benches Cleanup | v1.0 | 4/4 | Complete | 2026-04-22 |
| 3. M2 Generalized Tensor Macro | v1.0 | 3/3 | Complete | 2026-04-22 |
| 4. M2 Pi_LeakyTensor + F_eq | v1.0 | 3/3 | Complete | 2026-04-22 |
| 5. M2 Pi_aTensor Correct Combining | v1.0 | 3/3 | Complete | 2026-04-22 |
| 6. M2 Pi_aTensor' Permutation Bucketing + Benches | v1.0 | 3/3 | Complete | 2026-04-23 |
| 7. Preprocessing Trait + Ideal Backends | v1.1 | 0/3 | Not started | - |
| 8. Open() + Protocol 1 Garble/Eval/Check | v1.1 | 0/3 | Planned | - |
| 9. Protocol 2 Garble/Eval/Check | v1.1 | 0/4 | Planned | - |
| 10. Wall-Clock Benchmarks | v1.1 | 4/4 | Complete    | 2026-04-25 |

## References

- `references/appendix_krrw_pre.tex` — protocol specification (Appendix F)
- `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf` — main paper
- `references/2017-030-2.pdf` — WRK17 (leaky AND triples + bucketing)
- `references/2018-578-3.pdf` — KRRW18 (preprocessing for authenticated garbling)
- `references/mpz-dev/` — MPZ reference implementation
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Protocol 1 and Protocol 2 online phase
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_cpre.tex` — compressed preprocessing (F_cpre oracle)
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/4_distributed_garbling.tex` — distributed half gates
