# Phase 10: Wall-Clock Benchmarks - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix all existing benchmarks to produce correct wall-clock measurements (no dead-code
elimination, no async overhead in pure computation paths), isolate preprocessing and
online phases into separate criterion groups, add Protocol 2 garble/eval/check benchmarks,
and report throughput in both paper-style (ms per tensor op) and literature-style
(ns per AND gate).

Distributed half-gates (BENCH-05) is deferred to v2 — `4_distributed_garbling.tex`
is marked `\nakul{TODO, scrap}` by the author and that section is not stable enough
to implement against.

Requirements in scope: BENCH-01, BENCH-02, BENCH-04, BENCH-06.
BENCH-05 deferred to v2.

</domain>

<decisions>
## Implementation Decisions

### BENCH-05: Distributed Half-Gates (DHG/DTG)
- **D-01:** Deferred to v2. `4_distributed_garbling.tex` opens with `\nakul{TODO, scrap}`.
  The section is not stable; implementing against it now would produce dead code. Remove
  from Phase 10 scope entirely.

### Async Benchmark Strategy
- **D-02:** Keep existing async/network benchmarks (`bench_*x*_runtime_with_networking`)
  intact — they simulate a 100 Mbps connection matching the paper's experimental setup
  (`appendix_experiments.tex §Methodology`) and are needed for direct paper comparison.
- **D-03:** Add new **sync** `iter_custom` + `std::time::Instant` benchmarks alongside
  the async ones for pure wall-clock throughput (no network overhead, no tokio async
  scheduler). These go in the "online" and "preprocessing" criterion groups.
- **D-04:** Apply `std::hint::black_box` to all benchmark outputs — both the existing
  async benches and the new sync benches. This satisfies BENCH-01.
- **D-05:** If the existing async benches have code smells (copy-paste across per-size
  functions, dead variables, redundant setup), fix them during the refactor. The
  per-size functions (`bench_4x4_runtime_with_networking`, etc.) are repetitive; a
  shared parameterized helper is acceptable if it eliminates duplication.
- **D-06:** `bench_preprocessing` (`bench_preprocessing` in current `benchmarks.rs`)
  uses tokio async even though `run_preprocessing` is synchronous — the tokio wrapper
  is only for the `SimpleNetworkSimulator`. Convert `bench_preprocessing` to use
  `iter_custom + Instant` directly (the sync path), removing the unnecessary async
  dependency. The network communication simulation can be noted in comments or as a
  separate non-measured annotation rather than a measured async step.

### Criterion Group Structure (BENCH-04)
- **D-07:** Two criterion groups: `"preprocessing"` (already named correctly in the
  existing `bench_preprocessing` function) and `"online"` (new group).
- **D-08:** `"preprocessing"` group contains: `run_preprocessing` sweeping N×N sizes
  from `BENCHMARK_PARAMS`, sync `iter_custom` measurement.
- **D-09:** `"online"` group contains:
  - Protocol 1 garble/eval (existing `garble_first_half` / `garble_second_half` /
    `garble_final` + `evaluate_*` variants), sync measurement
  - Protocol 2 garble/eval (`garble_first_half_p2` / `garble_second_half_p2` /
    `garble_final_p2` + `evaluate_*_p2` variants), sync `iter_custom` measurement
  - Consistency check for both P1 (`check_zero`) and P2
    (`assemble_c_gamma_shares_p2`) included in the measured pipeline
- **D-10:** The existing `bench_*x*_runtime_with_networking` async functions are kept
  as a separate `criterion_group!` for the paper's network-simulation benchmarks but are
  NOT folded into the `"online"` criterion group — they serve a different purpose
  (end-to-end with simulated network, matching paper figures).

### Throughput Units (BENCH-02, BENCH-06)
- **D-11:** `iter_custom` benchmarks report throughput **two ways** from a single timing
  run:
  1. **ms per tensor op** — elapsed_ns / iterations / 1_000_000.0 — matches the paper's
     reported units in `appendix_experiments.tex` (Table 1 and Fig. benchmarks).
  2. **ns per AND gate** — elapsed_ns / (iterations * n * m) — the crypto literature
     standard (comparable to `1.5κ bits per AND gate` in the introduction).
- **D-12:** Use `criterion::Throughput::Elements(n * m as u64)` so Criterion also
  displays AND-gates/s in its standard output alongside the raw timing.
- **D-13:** Sweep the same parameters as the paper: N×N for N in
  `BENCHMARK_PARAMS = [(4,4), (8,8), ..., (256,256)]`, chunking factors 1..=8 for the
  online phase (matching paper's tile-size sweep). Preprocessing benchmarks use
  chunking_factor = 1 (fixed, matching current `bench_preprocessing`).

### Claude's Discretion
- Exact placement of P1/P2 benchmarks within `benchmarks.rs` vs a new file — Claude
  may split into separate files if `benchmarks.rs` becomes unwieldy (it's already 494
  lines).
- Whether to keep or remove `SimpleNetworkSimulator` dependency from the `"preprocessing"`
  group entirely — since preprocessing is synchronous, the network simulator call in the
  current `bench_preprocessing` adds no measurement value and can be dropped.
- Sample sizes for large N×N (current code already has `if n * m > 4096 { group.sample_size(10); }` — keep as-is or tune).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Paper Benchmark Methodology
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_experiments.tex` — MUST READ: benchmark setup (100 Mbps, Criterion, 3s warmup, ≥100 samples), reported units (ms + KB), sweep parameters (N×N for N∈{8..256}, tile sizes 1..8), and the key results table (128×128: 47.9ms→14.3ms, 524.3KB→96.9KB). This defines what the benchmarks must reproduce.

### Protocol Constructions Being Benchmarked
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Protocol 1 and Protocol 2 tensor gate garble/eval constructions (what the online benchmarks measure)

### Deferred (Do Not Implement in Phase 10)
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/4_distributed_garbling.tex` — DHG/DTG, deferred to v2; marked `\nakul{TODO, scrap}`. Do not implement.

### Existing Code to Extend
- `benches/benchmarks.rs` — current 494-line benchmark file; contains `bench_preprocessing` ("preprocessing" group, uses tokio async), seven per-size `bench_*x*_runtime_with_networking` async functions (P1 online + network sim), `BENCHMARK_PARAMS`, `setup_auth_gen`, `setup_auth_eval`
- `benches/network_simulator.rs` — `SimpleNetworkSimulator` (100 Mbps async simulation); keep for network benches, remove from preprocessing bench

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `setup_auth_gen(n, m, chunking_factor) -> AuthTensorGen` — existing helper; reuse for P1 online benches and new P2 benches
- `setup_auth_eval(n, m, chunking_factor) -> AuthTensorEval` — same
- `BENCHMARK_PARAMS: &[(usize, usize)]` — existing sweep table; reuse for all new benches
- `bench_preprocessing` — exists with "preprocessing" group name; refactor to sync `iter_custom` rather than rewrite

### Protocol 2 Methods (Phase 9 output)
- `AuthTensorGen::garble_first_half_p2()`, `garble_second_half_p2()`, `garble_final_p2()` — in `src/auth_tensor_gen.rs`
- `AuthTensorEval::evaluate_first_half_p2()`, `evaluate_second_half_p2()`, `evaluate_final_p2()` — in `src/auth_tensor_eval.rs`
- `assemble_c_gamma_shares_p2()` — consistency check helper in `src/lib.rs`
- `check_zero` — P1 consistency check in `src/online.rs`

### Established Patterns
- Criterion 0.7.0 with `async_tokio` feature — keep for network benches; do NOT use for new sync benches
- `Throughput::Elements(n_auth_bits as u64)` — already used in `bench_preprocessing`; adapt to `n * m` for AND-gate throughput in online group
- `BenchmarkId::new(label, format!("{}x{}", n, m))` — existing naming convention; continue

### Integration Points
- `Cargo.toml`: `[[bench]] name = "benchmarks" harness = false` — single benchmark binary; adding functions to `benchmarks.rs` or splitting into a new file both work (planner decides)
- New sync benches must not import `tokio` — use only `std::time::Instant` and `criterion::Criterion`
- `cargo bench --no-run` (BENCH-06 compile check) must exit zero after all changes

</code_context>

<specifics>
## Specific Ideas

- The paper's Table 1 reports 128×128 at tile=6: 47.9ms (naive) vs 14.3ms (ours). The
  async network benches should reproduce numbers in this ballpark — if they diverge
  significantly, that's worth noting.
- "Both would be useful to have" (user's words on throughput units) — implement both
  ms-per-tensor-op and ns-per-AND-gate in the same `iter_custom` closure; no need to
  choose.
- Code smell fix explicitly requested: the seven near-identical `bench_*x*_runtime_with_networking`
  functions are repetitive; a parameterized loop or shared helper is cleaner.

</specifics>

<deferred>
## Deferred Ideas

- **BENCH-05 (DHG/DTG)** — Implement distributed half-gates and distributed tensor gates
  from `4_distributed_garbling.tex` with a naive-vs-tensor comparison benchmark. Deferred
  to v2 because the paper section is marked for removal by the author.
- **Parallel tensor evaluation** — The paper notes (appendix_experiments.tex) it did not
  parallelize tensor products but expects parallelization would amplify the contribution.
  Potential v2 benchmark target.
- **Real network I/O** — Replace `SimpleNetworkSimulator` with real TCP socket timing.
  Deferred to v2 (real network layer is out of scope for v1.1).

</deferred>

---

*Phase: 10-wall-clock-benchmarks*
*Context gathered: 2026-04-24*
