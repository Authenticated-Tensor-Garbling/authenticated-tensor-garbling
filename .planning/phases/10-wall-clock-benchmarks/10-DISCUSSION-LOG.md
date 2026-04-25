# Phase 10: Wall-Clock Benchmarks - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-24
**Phase:** 10-wall-clock-benchmarks
**Areas discussed:** BENCH-05 scope, Async refactor strategy, Online group scope, Throughput units

---

## BENCH-05 Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to v2 | Section marked `\nakul{TODO, scrap}`; keep Phase 10 focused on BENCH-01/02/04/06 (2–3 plans) | ✓ |
| Include DHG only | Implement half-gates warm-up (§4.2) and comparison benchmark (~4 plans) | |
| Include both DHG + DTG | Full Section 4 implementation (5–6 plans, unstable paper section) | |

**User's choice:** Defer to v2
**Notes:** The `\nakul{TODO, scrap}` annotation in `4_distributed_garbling.tex` indicates the author may cut the section. Implementing against an unstable spec risks building dead code.

---

## Async Refactor Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Keep async + add new sync benches | Preserve 100 Mbps network sim (matches prior paper), add separate iter_custom sync benches | ✓ |
| Replace with sync + black_box | Drop network simulator entirely from all benches | |
| Keep async + replace preprocessing bench only | Partial refactor | |

**User's choice:** Keep async and add new sync benches. Also fix any code smells in the existing async code.
**Notes:** The prior paper benchmarks at 100 Mbps — keeping the async network benches is necessary for direct paper comparison. Code smell fix explicitly requested (the seven near-identical per-size functions are repetitive).

---

## Online Group Scope

| Option | Description | Selected |
|--------|-------------|----------|
| P1 and P2 garble/eval + consistency check | Both protocols, check_zero and assemble_c_gamma_shares_p2 included | ✓ |
| P2 garble/eval only | Only the final authenticated protocol | |

**User's choice:** P1 and P2 garble/eval, with the consistency check included
**Notes:** Benchmarking both protocols shows the authenticated-vs-unauthenticated cost overhead directly.

---

## Throughput Units (iter_custom)

| Option | Description | Selected |
|--------|-------------|----------|
| Both ms/tensor-op and ns/AND-gate | Report both from same iter_custom run | ✓ |
| ms per tensor op only | Matches paper units | |
| ns per AND gate only | Crypto literature standard | |

**User's choice:** Both would be useful to have — see which gives the most compelling result.
**Notes:** Implementation: elapsed_ns / iterations / 1_000_000.0 for ms-per-op (paper style); elapsed_ns / (iterations * n * m) for ns-per-AND-gate (literature style). Use Throughput::Elements(n * m) so Criterion also shows AND-gates/s. User initiated a paper lookup before deciding: `appendix_experiments.tex` reports ms + KB units; the intro references `1.5κ bits per AND gate` as the literature baseline.

---

## Claude's Discretion

- Whether to keep P1/P2 sync benches in `benchmarks.rs` or split to a new file
- Whether to drop `SimpleNetworkSimulator` from `bench_preprocessing` entirely (it's synchronous; the async wrapper is unnecessary overhead)
- Sample size tuning for large N×N

## Deferred Ideas

- BENCH-05 (DHG/DTG) — deferred to v2
- Parallelized tensor evaluation benchmarks — v2
- Real TCP network I/O replacing SimpleNetworkSimulator — v2
