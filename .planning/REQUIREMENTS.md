# Requirements: v1.1 — Full Protocol Demonstration + Benchmarks

## Milestone Goal

Extend the v1.0 preprocessing foundation into a complete, demonstrable 2PC protocol with interchangeable preprocessing backends, a working online phase (Open + consistency check for both Protocol 1 and Protocol 2), and coherent wall-clock benchmarks.

---

## Active Requirements

### PREPROCESSING — Interface & Abstraction

- [ ] **PRE-01**: `TensorPreprocessing` trait is defined with a common interface (`run(n, m, count, chunking_factor) -> (TensorFpreGen, TensorFpreEval)`) that all preprocessing implementations satisfy
- [ ] **PRE-02**: `IdealPreprocessingBackend` implements `TensorPreprocessing` — trusted-dealer oracle that produces correct authenticated triples (D_gb and D_ev shares) without running the real protocol
- [ ] **PRE-03**: All existing preprocessing implementations (pi_atensor via `run_preprocessing`, pi'_atensor, the FbCOT-backed pipeline) satisfy `TensorPreprocessing` and are interchangeable with the online phase
- [ ] **PRE-04**: `TensorFpreGen` and `TensorFpreEval` are extended with `gamma_auth_bit_shares` (D_ev-authenticated shares of `l_gamma*`) and `output_mask_auth_bit_shares` (D_gb-authenticated shares of `l_gamma`) — required for consistency check
- [ ] **PRE-05**: `IdealCompressedPreprocessingBackend` implements `TensorPreprocessing` as a trusted-dealer oracle for the `F_cpre` ideal functionality (reduced authentication cost `O(SSP * log(kappa))` vs `O(n)`); real `Pi_cpre` protocol is out of scope (incomplete in paper)

### ONLINE — Open() and Consistency Check

- [ ] **ONL-01**: `open()` free function in `src/online.rs` operates on `AuthBit` (full two-party view) to reveal masked wire values; correctly uses D_gb delta for garbler-input wires and D_ev delta for output wire decoding
- [ ] **ONL-02**: `open()` is tested with a negative test that asserts wrong-delta produces incorrect output (guards against silent MAC-domain bugs)

### ONLINE — Protocol 1 (Unauthenticated Tensor Macros)

- [ ] **P1-01**: Protocol 1 garble algorithm is complete — two `tensorgb` calls per tensor gate with correct input wiring per `5_online.tex`, XOR with `[l_gamma* D_gb]^gb`
- [ ] **P1-02**: Protocol 1 evaluate algorithm is complete — `tensorev` calls with correct wiring, produces masked output wire value `Λ_gamma`
- [ ] **P1-03**: Protocol 1 consistency check (`CheckZero`) is implemented — evaluator sends `Λ_w` to garbler; both compute `c_gamma` from D_ev-MAC'd shares including the `l_gamma*` preprocessing term; `CheckZero` verifies
- [ ] **P1-04**: Protocol 1 end-to-end test verifies garbler XOR evaluator output equals the correct tensor product (extends existing v1.0 battery)
- [ ] **P1-05**: Protocol 1 consistency check negative test verifies that a wrong `L_gamma` (tampered output mask) causes `CheckZero` to abort, not silently pass

### ONLINE — Protocol 2 (Authenticated Tensor Macros)

- [ ] **P2-01**: `gen_unary_outer_product_wide` variant produces `(kappa+rho)`-bit leaf seed expansions for simultaneous D_gb and D_ev share propagation per `6_total.tex` and `5_online.tex`
- [ ] **P2-02**: Protocol 2 garble algorithm (`_p2` variant) uses wide seed expansion; garbler never reveals masked values to evaluator
- [ ] **P2-03**: Protocol 2 evaluate algorithm (`_p2` variant) produces D_ev-authenticated output wire shares
- [ ] **P2-04**: Protocol 2 consistency check — garbler opens its D_ev share of `v_gamma`; evaluator checks locally using `L_gamma`; `CheckZero` verifies
- [ ] **P2-05**: Protocol 2 end-to-end test verifies correctness; Protocol 1 tests remain valid and unmodified after Protocol 2 is added

### BENCHMARKS

- [ ] **BENCH-01**: All garbling benchmark outputs are wrapped in `std::hint::black_box` to prevent compiler dead-code elimination in `--release` mode
- [ ] **BENCH-02**: Wall-clock benchmarks for Protocol 2 garbling throughput use `criterion::iter_custom` + `std::time::Instant` (no async/tokio wrappers)
- [ ] **BENCH-04**: Preprocessing vs online phase comparison benchmark isolates the two phases into separate criterion groups
- [ ] **BENCH-05**: Distributed half gates (`dtg`/`dhg`) from `4_distributed_garbling.tex` are implemented and benchmarked; a comparison benchmark demonstrates naive tensor (nm AND half-gates) vs GGM tensor product at ideal chunk sizes
- [ ] **BENCH-06**: Benchmark output reports wall-clock time per gate in nanoseconds alongside total throughput

---

## Future Requirements (Deferred)

- Real `Pi_cpre` protocol — requires `F_DVZK`, `F_EQ`, `F_Rand`, `F_COT`; appendix is incomplete draft → v2
- Real `F_bcot` / `F_cot` over network (Ferret/IKNP) → v2
- Network communication layer → v2
- Malicious security simulation proof → v2

---

## Out of Scope

| Item | Reason |
|------|--------|
| Real `Pi_cpre` protocol body | `appendix_cpre.tex` is an explicitly incomplete draft; all protocol steps commented out in paper |
| `F_DVZK`, `F_EQ`, `F_Rand` ideal functionalities | Only needed for real Pi_cpre; out of scope for v1.1 |
| Network-based OT | Real F_bcot/F_cot remains v2; IdealBCot stays |
| Circuit-level changes (`circuits/` submodule) | No scope change from v1.0 |
| Semi-honest family (`tensor_gen`, `tensor_eval`, `tensor_pre`) | Referenced only in lib.rs integration test |
| Malicious security proof | Correctness of honest-party execution only |

---

## Traceability

| Requirement | Phase |
|-------------|-------|
| PRE-01..04  | TBD (roadmapper) |
| PRE-05      | TBD |
| ONL-01..02  | TBD |
| P1-01..05   | TBD |
| P2-01..05   | TBD |
| BENCH-01..05 | TBD |
