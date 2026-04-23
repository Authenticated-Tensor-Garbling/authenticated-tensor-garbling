# Research Summary — v1.1: Full Protocol Demonstration + Benchmarks

## Executive Summary

Milestone v1.1 extends the v1.0 preprocessing infrastructure into a complete, demonstrable 2PC online phase. The v1.0 codebase already implements the tensor macro primitives (`tensorgb`/`tensorev`), the full uncompressed preprocessing pipeline (Pi_aTensor'), and the `AuthTensorGen`/`AuthTensorEval` garble/evaluate skeleton. What is missing is the thin but security-critical layer on top: a preprocessing trait abstraction, the `Open()` function for mask revelation, Protocol 1's consistency check (`CheckZero` on D_ev shares), and wall-clock benchmarks that cleanly isolate online-phase cost. All four are implementable with zero new crates using only existing types and `std::time::Instant`.

## Stack Additions

**Zero new crates required.**

- Rust generics (`impl TensorPreprocessing`) — not `dyn Trait`. `Box<dyn PreprocessingBackend>` has an implicit `'static` bound that `IdealBCot`-backed types cannot satisfy.
- `criterion 0.7.0` with `iter_custom` + `std::hint::black_box` — correct wall-clock benchmark pattern; drop `to_async`/tokio wrappers for pure crypto measurements.
- `AuthBitShare` / `Block` / `Delta` (existing) — all MAC arithmetic for `Open()` and `CheckZero`.
- `rand` / `rand_chacha` (existing) — `IdealCompressedPre` matrix sampling.

## Feature Table Stakes (Must-Have)

| Feature | Description |
|---------|-------------|
| `TensorPreprocessing` trait + `IdealPreprocessingBackend` | Enables everything; oracle that owns all data, no lifetime params |
| `TensorFpreGen`/`TensorFpreEval` field extensions | Add `gamma_auth_bit_shares` (D_ev) + `output_mask_auth_bit_shares` (D_gb) |
| `Open()` free function in `src/online.rs` | XOR-merge IT-MAC shares + extract-bit; required by Protocol 1 steps 3, 4, 8 |
| Protocol 1 garble/eval end-to-end | Existing skeleton completed; mostly done |
| Protocol 1 consistency check | CheckZero on D_ev shares including `l_gamma*` term from preprocessing |
| Wall-clock benchmarks | `iter_custom` + `black_box`; isolated from async overhead |

## Feature Differentiators (Secondary Priority)

| Feature | Description |
|---------|-------------|
| Protocol 2 garble/eval | `_p2` variants with `(kappa+rho)`-bit seed expansion for simultaneous Δ_gb/Δ_ev propagation |
| Protocol 2 consistency check | Garbler reveals D_ev share; evaluator checks locally with L_gamma |
| `IdealCompressedPreprocessingBackend` | `F_cpre` oracle (trusted dealer) only — real `Pi_cpre` is commented-out draft in paper |

## Stretch Goals

| Feature | Gate |
|---------|------|
| Distributed half gates + naive vs tensor benchmark | Requires author confirmation — paper section marked `\nakul{TODO, scrap}` |

## Critical Architecture Decisions

- **One new module**: `src/online.rs` for `Open()` and `CheckZero` free functions
- **Trait signature**: `fn run(&self, n, m, count, chunking_factor) -> (TensorFpreGen, TensorFpreEval)` — wraps existing function signature, zero changes to gen/eval constructors
- **Two new fields on TensorFpreGen/Eval**: must be added atomically (all constructors in one commit)
- **Protocol 2**: `gen_unary_outer_product_wide` variant in `tensor_ops.rs`; do NOT modify the existing `gen_unary_outer_product` (Protocol 1 callers must not break)
- **`assert_eq!(count, 1)` in `run_preprocessing`**: work around with loop in trait impl; do not remove the assert

## Watch Out For

| Pitfall | Severity | Prevention |
|---------|----------|------------|
| `Open()` wrong-delta bug — wrong delta produces silent wrong bit, not panic | CRITICAL | Make delta explicit at every call site; write a negative test asserting wrong-delta fails |
| Missing `l_gamma*` term in `c_gamma` — check passes for honest parties but provides zero malicious security | CRITICAL | Expand formula algebraically first; write "wrong L_gamma" negative test |
| D_gb vs D_ev confusion in `c_gamma` shares — silently passes everything | CRITICAL | "wrong L_gamma" test must fail when check is applied to wrong shares |
| `Box<dyn PreprocessingBackend>` implicit `'static` bound | HIGH | Use `impl TensorPreprocessing` or concrete generics throughout |
| Missing `black_box` on garbling outputs — compiler eliminates entire computation in `--release` | HIGH | Wrap all output in `std::hint::black_box`; fix before adding any throughput numbers |

## Recommended Phase Order

| Phase | Deliverable | Confidence | Dependencies |
|-------|-------------|------------|--------------|
| 7 | Preprocessing trait + `IdealPreprocessing` + struct field extensions | HIGH | Foundation for all other phases |
| 8 | `Open()` + Protocol 1 consistency check + end-to-end P1 test (with negative tests) | HIGH | Phase 7 |
| 9 | Wall-clock benchmarks (P1 garbling throughput, preprocessing vs online) | HIGH | Phase 8 |
| 10 | Protocol 2 garble/eval (`_p2` variants) + P2 consistency check | MEDIUM | Phase 7 |
| 11 | Ideal compressed preprocessing (`F_cpre` oracle) | MEDIUM | Phase 7 |
| 12 | (Stretch) Distributed half gates + naive vs tensor benchmark | LOW/gated | Author confirmation |

## Research Flags for Planning

- **Phase 10 (Protocol 2)**: Concrete Rust representation for `(kappa+rho) = 168`-bit leaf values needs a design decision before committing the `_p2` interface
- **Phase 11 (Compressed pre)**: `sigma` formula (appendix line 28) and column-major AND-to-tensor index mapping must be verified against codebase convention
- **Phase 12**: Do not begin without author confirmation that Section 4 is not being cut from the paper
