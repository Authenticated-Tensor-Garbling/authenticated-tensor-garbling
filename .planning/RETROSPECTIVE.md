# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — Authenticated Tensor Garbling Preprocessing Fix

**Shipped:** 2026-04-23
**Phases:** 6 | **Plans:** 19 | **Timeline:** 2 days (2026-04-20 → 2026-04-22)
**Tests:** 74/74 passing

### What Was Built

- Generalized Tensor Macro (Construction 1) — GGM tree garbler/evaluator; Z_garbler XOR Z_evaluator = a ⊗ T
- Pi_LeakyTensor (Construction 2) — full generate() with five bCOT batch pairs, two macro calls, masked reveal, F_eq; LeakyTriple reduced to exact paper shape
- Pi_aTensor (Construction 3) — paper-correct two_to_one_combine (Z=Z'⊕Z''⊕x''⊗d) with MAC verify on revealed d; iterative fold
- Pi_aTensor' (Construction 4) — per-triple Fisher-Yates row permutation; Construction 4 bucket formula; end-to-end regression
- M1 cleanup — Key LSB=0 enforced at construction; ideal/real TensorFpre separated; benchmarks deduplicated; gamma cascade removed across 6 files

### What Worked

- **Bug list as roadmap:** Starting with 8 concrete paper-review bugs gave clear, verifiable goals — each phase had an obvious definition of done
- **Paper-invariant tests:** Writing tests that verify Z_full = x_full ⊗ y_full (not code behavior) caught the silent x-bug in combine_leaky_triples before it shipped
- **Phase sequencing:** M1 cleanup before M2 implementation paid off — primitives were stable and correctly named when the protocol implementation started
- **Wave-based execution within phases:** Scaffolding plans (01) before implementation plans (02) before test plans (03) kept each phase self-contained and reviewable

### What Was Inefficient

- **Quick task opened too early:** `fix-cot-convention` was captured before Phase 4 started but the fix was implicit in the Phase 4 rewrite — the task wasn't needed and created noise at milestone close
- **REQUIREMENTS.md traceability not updated during execution:** All validation was tracked in PROJECT.md per-phase, leaving REQUIREMENTS.md traceability table showing all "Pending" at close — required manual archive update

### Patterns Established

- `verify_cross_party(gen, eval, Δ_A, Δ_B)` — cross-party MAC verification helper; preserved verbatim across phase rewrites as a fixed reference point
- Verifier-delta COT convention: `transfer_a_to_b(&b_bits)`, `transfer_b_to_a(&a_bits)` — A's bits under Δ_B, B's bits under Δ_A
- Column-major LeakyTriple Z indexing: `k = j * n + i` (j = y index, i = x index) — consistent throughout tensor_macro, leaky_tensor_pre, auth_tensor_pre
- `pub(crate)` default for protocol internals; only LeakyTriple fields and run_preprocessing are pub

### Key Lessons

1. **Rewrite from spec, not from bugs:** Pi_LeakyTensor generate() was faster to rewrite correctly from the paper than to patch incrementally — the COT convention quick task became moot because the whole function was replaced
2. **Paper one-liners belong in tests:** TEST-03 (`z_full = x_full ⊗ y_full`) is the entire correctness claim of Pi_LeakyTensor in one assertion — write it first, implement until it passes
3. **Gamma cascade is a smell:** The gamma fields proliferated because they were added to the wrong layer. Removing them required touching 6 files — a clear sign the abstraction boundary was wrong from the start

### Cost Observations

- Model mix: primarily Sonnet 4.6 throughout
- Notable: 2-day wall time for 6 phases / 19 plans / 8 bug fixes on a cryptographic protocol — aggressive but manageable with paper-invariant tests as guardrails

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 6 | 19 | First milestone; established paper-invariant test pattern |

### Cumulative Quality

| Milestone | Tests | Zero-Dep Additions |
|-----------|-------|-------------------|
| v1.0 | 74 | tensor_macro.rs, feq.rs, preprocessing.rs |

### Top Lessons (Verified Across Milestones)

1. Paper-invariant tests (not code-echo tests) are the only reliable correctness signal for cryptographic protocol implementations
2. Scaffolding plans before implementation plans prevents integration surprises within a phase
