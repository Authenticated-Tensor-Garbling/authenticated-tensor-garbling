# Roadmap

## Phase 1: Uncompressed Preprocessing Protocol

**Goal:** Implement the KRRW-style uncompressed preprocessing protocol described in Appendix F of the paper, replacing the ideal `TensorFpre` placeholder in `src/auth_tensor_fpre.rs` with a real two-party protocol that generates dual-authenticated tensor triples.

**Deliverables:**
- `Pi_LeakyTensor`: leaky authenticated tensor triple generation via GGM tree + correlated-OT
- `Pi_aTensor`: bucketing combiner to produce full dual-authenticated triples
- `Pi_aTensor'` (optional): permutation-bucketing variant with improved bucket size
- Benchmark integration in `benches/benchmarks.rs`

**Plan Progress:**

| Wave | Plan | Status |
|------|------|--------|
| 1 | 01-PLAN-cot — IdealBCot (boolean correlated OT) | complete |
| 2 | 01-PLAN-leaky-tensor — Pi_LeakyTensor + Pi_aTensor bucketing | pending |
| 3 | 01-PLAN-fpre-replace — run_preprocessing entry point | pending |
| 4 | 01-PLAN-benchmarks — bench_preprocessing benchmark | pending |

**References:**
- `references/appendix_krrw_pre.tex` — protocol specification
- `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf` — Appendix F
- `references/2017-030-2.pdf` — WRK17 (leaky AND triples + bucketing)
- `references/2018-578-3.pdf` — KRRW18 (preprocessing for authenticated garbling)
- `references/mpz-dev/` — MPZ reference implementation
