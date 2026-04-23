# Milestones

## v1.0 — Authenticated Tensor Garbling Preprocessing Fix

**Shipped:** 2026-04-23
**Phases:** 1–6 | **Plans:** 19 | **Tests:** 74/74 passing
**Git range:** b2bc782 → 2f5a061 | **Timeline:** 2026-04-20 → 2026-04-22 (2 days)
**Codebase:** 199 Rust source files, ~54,842 LOC

### Delivered

Full correctness fix and paper-faithful implementation of the KRRW-style uncompressed preprocessing protocol for authenticated tensor garbling. All 8 known algorithmic bugs resolved; all 4 paper constructions implemented from scratch with paper-invariant tests.

### Key Accomplishments

1. **Primitive layer hardened (Phase 1):** Key LSB=0 enforced at construction; AuthBitShare/AuthBit scoped correctly; InputSharing::shares_differ() replaces ambiguous bit(); column-major indexing documented throughout
2. **Ideal/real separation (Phase 2):** TensorFpre (ideal trusted dealer) separated from TensorFpreGen/TensorFpreEval (real protocol structs) in dedicated preprocessing.rs module; gamma cascade removed across 6 files; benchmarks deduplicated
3. **Generalized Tensor Macro implemented (Phase 3):** tensor_garbler + tensor_evaluator composing GGM tree expansion; Z_garbler XOR Z_evaluator = a ⊗ T verified by 10-test paper-invariant battery
4. **Pi_LeakyTensor Construction 2 implemented (Phase 4):** Five bCOT batch pairs; verifier-delta COT convention; two tensor macro calls; masked reveal; feq::check; LeakyTriple reduced to exact paper shape (itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})
5. **Pi_aTensor Construction 3 correct combining (Phase 5):** two_to_one_combine implements paper §3.2 algebra (Z=Z'⊕Z''⊕x''⊗d); silent x-bug fixed; bucket_size_for(ell) uses output triple count
6. **Pi_aTensor' Construction 4 + benchmarks (Phase 6):** Per-triple Fisher-Yates row permutation; Construction 4 bucket formula B=1+ceil(SSP/log2(n·ℓ)); end-to-end regression test; benchmarks compile clean

### Known Deferred Items at Close: 1 (see STATE.md Deferred Items)

- `fix-cot-convention` quick task (2026-04-21): Opened before Phase 4; resolved by Phase 4 rewrite which applied verifier-delta convention correctly from scratch. Not a gap.

### Archive

- Roadmap: `.planning/milestones/v1.0-ROADMAP.md`
- Requirements: `.planning/milestones/v1.0-REQUIREMENTS.md`
