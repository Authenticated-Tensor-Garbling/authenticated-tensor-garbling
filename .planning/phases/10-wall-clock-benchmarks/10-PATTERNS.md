# Phase 10: Wall-Clock Benchmarks - Pattern Map

**Mapped:** 2026-04-24
**Files analyzed:** 2 (benches/benchmarks.rs modified, benches/network_simulator.rs kept read-only)
**Analogs found:** 2 / 2 — the file being modified IS the primary analog

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `benches/benchmarks.rs` | benchmark | request-response (sync iter_custom + async iter_batched) | `benches/benchmarks.rs` lines 438-495 (`bench_preprocessing` + `bench_*x*_runtime_with_networking`) | self-analog (refactor + extend) |
| `benches/network_simulator.rs` | utility | event-driven (async sleep simulation) | `benches/network_simulator.rs` (unchanged) | exact — keep as-is |

---

## Pattern Assignments

### `benches/benchmarks.rs` — sync `iter_custom` benchmarks (new "preprocessing" + "online" groups)

**Analog:** `benches/benchmarks.rs` lines 438-495 (`bench_preprocessing`) and lines 60-112 (`bench_4x4_runtime_with_networking`)

---

#### Imports pattern (lines 1-24)

```rust
use std::time::Duration;
use std::mem::size_of;

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};

mod network_simulator;
use network_simulator::SimpleNetworkSimulator;

use authenticated_tensor_garbling::{
    block::Block,
    auth_tensor_gen::AuthTensorGen,
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
    preprocessing::run_preprocessing,
};

use once_cell::sync::Lazy;

static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap()
});
```

**For new sync benches add these imports** (no tokio import needed; `RT` is already present for network benches):

```rust
use std::hint::black_box;
use std::time::Instant;
use authenticated_tensor_garbling::online::check_zero;
```

The P2 methods are already on `AuthTensorGen` / `AuthTensorEval` — no new imports for those. `assemble_c_gamma_shares_p2` lives in `src/lib.rs` as a `#[cfg(test)]` private function — the benchmark will need to either inline the assembly logic or the function will need to be made `pub` and moved out of `#[cfg(test)]`. **Planner decision required:** promote `assemble_c_gamma_shares_p2` to a `pub` function reachable from bench, or inline an equivalent in the benchmark.

---

#### BENCHMARK_PARAMS constant (lines 27-38)

```rust
const BENCHMARK_PARAMS: &[(usize, usize)] = &[
    (4, 4),
    (8, 8),
    (16, 16),
    (24, 24),
    (32, 32),
    (48, 48),
    (64, 64),
    (96, 96),
    (128, 128),
    (256, 256),
];
```

Reuse unchanged for all new sync benches. D-13 says sweep same params.

---

#### Setup helper pattern (lines 44-57)

```rust
fn setup_auth_gen(n: usize, m: usize, chunking_factor: usize) -> AuthTensorGen {
    let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT);
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize, chunking_factor: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT);
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}
```

Reuse unchanged for all new sync benches (both P1 and P2 online groups). These return a fully-initialized gen/eval pair from the ideal trusted dealer — the P2 `_p2` methods operate on the same `AuthTensorGen` / `AuthTensorEval` structs.

---

#### Preprocessing group — sync `iter_custom` pattern (replaces lines 438-481)

The current `bench_preprocessing` uses `b.to_async(&*RT).iter_batched(...)` even though `run_preprocessing` is synchronous. Convert to `iter_custom + Instant`. Concrete template:

```rust
fn bench_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("preprocessing");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(20));

    let chunking_factor = 1;

    for &(n, m) in BENCHMARK_PARAMS {
        // AND-gate throughput: use n*m (the gate count) for online-comparable units.
        // For preprocessing, n*m represents the correlated authenticated bits produced.
        group.throughput(Throughput::Elements((n * m) as u64));

        if n * m > 4096 {
            group.sample_size(10);
        }

        group.bench_with_input(
            BenchmarkId::new("real_preprocessing", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        let result = run_preprocessing(n, m, 1, chunking_factor);
                        total += start.elapsed();
                        let _ = black_box(result);
                    }
                    total
                });
            },
        );
    }
    group.finish();
}
```

Key points:
- `iter_custom` closure receives `iters: u64`; returns total `Duration` for all iterations.
- `Instant::now()` / `start.elapsed()` placed tightly around the measured call — no async overhead.
- `black_box(result)` applied to the output to prevent dead-code elimination (D-04).
- `SimpleNetworkSimulator` import NOT needed in preprocessing group (D-06 — drop it from this function).
- `Throughput::Elements((n * m) as u64)` so Criterion prints AND-gates/s in its output (D-12).

---

#### Online group — sync `iter_custom` pattern for P1 garble/eval/check

New function `bench_online_p1`. Sweeps `BENCHMARK_PARAMS` × chunking_factor 1..=8 (D-13). Template:

```rust
fn bench_online_p1(c: &mut Criterion) {
    let mut group = c.benchmark_group("online");

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        if n * m > 4096 {
            group.sample_size(10);
        }

        for chunking_factor in 1usize..=8 {
            group.bench_with_input(
                BenchmarkId::new(format!("p1_garble_eval_check_{}x{}", n, m), chunking_factor),
                &chunking_factor,
                |b, &chunking_factor| {
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            let mut generator = setup_auth_gen(n, m, chunking_factor);
                            let mut evaluator = setup_auth_eval(n, m, chunking_factor);

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half();
                            evaluator.evaluate_first_half(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half();
                            evaluator.evaluate_second_half(cl2, ct2);
                            generator.garble_final();
                            evaluator.evaluate_final();

                            // check_zero included in measured pipeline per D-09
                            let lambda_gb = generator.compute_lambda_gamma();
                            let l_gamma = evaluator.compute_lambda_gamma(&lambda_gb);
                            // assemble c_gamma and check_zero (inline or via promoted pub fn)
                            // let c_gamma = assemble_c_gamma_shares_p1(...);
                            // let _ = black_box(check_zero(&c_gamma, &generator.delta_a));

                            total += start.elapsed();
                            black_box(&evaluator);
                        }
                        total
                    });
                },
            );
        }
    }
    group.finish();
}
```

Note: setup (`setup_auth_gen` / `setup_auth_eval`) is OUTSIDE the timed region — it goes before `Instant::now()`. Only the garble/eval/check pipeline is timed.

---

#### Online group — sync `iter_custom` pattern for P2 garble/eval/check

New function `bench_online_p2`. Same structure as P1 but uses `_p2` method variants. Template:

```rust
fn bench_online_p2(c: &mut Criterion) {
    let mut group = c.benchmark_group("online");

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        if n * m > 4096 {
            group.sample_size(10);
        }

        for chunking_factor in 1usize..=8 {
            group.bench_with_input(
                BenchmarkId::new(format!("p2_garble_eval_check_{}x{}", n, m), chunking_factor),
                &chunking_factor,
                |b, &chunking_factor| {
                    b.iter_custom(|iters| {
                        let mut total = Duration::ZERO;
                        for _ in 0..iters {
                            let mut generator = setup_auth_gen(n, m, chunking_factor);
                            let mut evaluator = setup_auth_eval(n, m, chunking_factor);

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half_p2();
                            evaluator.evaluate_first_half_p2(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half_p2();
                            evaluator.evaluate_second_half_p2(cl2, ct2);
                            let (_d_gb, gb_d_ev_out) = generator.garble_final_p2();
                            let ev_d_ev_out = evaluator.evaluate_final_p2();

                            // assemble c_gamma_p2 and check_zero under ev.delta_b
                            // let c_gamma = assemble_c_gamma_shares_p2(...);
                            // let _ = black_box(check_zero(&c_gamma, &evaluator.delta_b));

                            total += start.elapsed();
                            black_box(&gb_d_ev_out);
                            black_box(&ev_d_ev_out);
                        }
                        total
                    });
                },
            );
        }
    }
    group.finish();
}
```

P2 method signatures (from `src/auth_tensor_gen.rs` lines 346-436 and `src/auth_tensor_eval.rs` lines 311-408):
- `generator.garble_first_half_p2()` → `(Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>)`
- `evaluator.evaluate_first_half_p2(cl, ct)` — ct is `Vec<Vec<(Block, Block)>>` (wide)
- `generator.garble_final_p2()` → `(Vec<Block>, Vec<Block>)` — (d_gb_out, d_ev_out)
- `evaluator.evaluate_final_p2()` → `Vec<Block>` — ev_d_ev_out

---

#### Throughput reporting pattern (D-11, D-12)

Both ms-per-tensor-op and ns-per-AND-gate are derived from the same `iter_custom` total elapsed. Inside the `iter_custom` closure, after accumulating `total: Duration`:

```rust
// Criterion uses Throughput::Elements(n * m) set on the group to display AND-gates/s
// automatically. The raw timing already gives ms/op. Both units come for free.
//
// For manual printing (optional, matches paper Table 1 style):
let elapsed_ns = total.as_nanos() as f64;
let ms_per_op = elapsed_ns / iters as f64 / 1_000_000.0;
let ns_per_and = elapsed_ns / (iters as f64 * (n * m) as f64);
println!("{}x{} cf={}: {:.3} ms/op, {:.2} ns/AND", n, m, chunking_factor, ms_per_op, ns_per_and);
```

`Throughput::Elements((n * m) as u64)` is already the pattern in `bench_preprocessing` (line 450 uses `n_auth_bits`; adapt to `n * m` for AND-gate units in online group per D-12).

---

#### Async network benchmark — parameterized helper (refactor of lines 60-435)

The seven `bench_*x*_runtime_with_networking` functions are identical except for the `n`/`m` values. Replace with one parameterized function:

```rust
fn bench_nxm_runtime_with_networking(c: &mut Criterion, n: usize, m: usize) {
    let mut group = c.benchmark_group(format!("Runtime with networking for {}x{}", n, m));
    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8usize {
        let mut generator = setup_auth_gen(n, m, chunking_factor);
        let (first_levels, first_cts)   = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();

        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;

        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &chunking_factor,
            |b, &chunking_factor| {
                b.to_async(&*RT).iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor),
                        setup_auth_eval(n, m, chunking_factor),
                        SimpleNetworkSimulator::new(100.0, 0),
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (fl, fc) = generator.garble_first_half();
                        let (sl, sc) = generator.garble_second_half();
                        generator.garble_final();
                        network.send_size_with_metrics(total_bytes).await;
                        evaluator.evaluate_first_half(fl, fc);
                        evaluator.evaluate_second_half(sl, sc);
                        let _ = black_box(evaluator.evaluate_final());
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

// One thin wrapper per size — Criterion requires named functions in criterion_group!
fn bench_4x4_runtime_with_networking(c: &mut Criterion)   { bench_nxm_runtime_with_networking(c, 4,   4)   }
fn bench_8x8_runtime_with_networking(c: &mut Criterion)   { bench_nxm_runtime_with_networking(c, 8,   8)   }
fn bench_16x16_runtime_with_networking(c: &mut Criterion) { bench_nxm_runtime_with_networking(c, 16,  16)  }
fn bench_32x32_runtime_with_networking(c: &mut Criterion) { bench_32x32_runtime_with_networking(c, 32,  32) }
fn bench_64x64_runtime_with_networking(c: &mut Criterion) { bench_nxm_runtime_with_networking(c, 64,  64)  }
fn bench_128x128_runtime_with_networking(c: &mut Criterion) { bench_nxm_runtime_with_networking(c, 128, 128) }
fn bench_256x256_runtime_with_networking(c: &mut Criterion) { bench_nxm_runtime_with_networking(c, 256, 256) }
```

`black_box` applied to the final `evaluate_final()` result in the async path (D-04).

---

#### criterion_group! / criterion_main! pattern (lines 483-495)

Current:
```rust
criterion_group!(
    benches,
    bench_4x4_runtime_with_networking,
    ...
    bench_preprocessing,
);
criterion_main!(benches);
```

Target after refactor — two named groups plus the network group:

```rust
criterion_group!(preprocessing_benches, bench_preprocessing);
criterion_group!(online_benches, bench_online_p1, bench_online_p2);
criterion_group!(
    network_benches,
    bench_4x4_runtime_with_networking,
    bench_8x8_runtime_with_networking,
    bench_16x16_runtime_with_networking,
    bench_32x32_runtime_with_networking,
    bench_64x64_runtime_with_networking,
    bench_128x128_runtime_with_networking,
    bench_256x256_runtime_with_networking,
);
criterion_main!(preprocessing_benches, online_benches, network_benches);
```

Separates "preprocessing" and "online" groups (D-07, D-08, D-09) while keeping network benches isolated (D-10).

---

#### Sample size guard (line 457-459 in existing bench)

```rust
if n * m > 4096 {
    group.sample_size(10);
}
```

Preserve in all new benchmark functions (preprocessing and online groups). This is the existing convention — keep or tune per planner judgment.

---

## Shared Patterns

### black_box (BENCH-01, D-04)
**Source:** `std::hint::black_box` — stdlib, no crate dependency
**Apply to:** Every benchmark output in both sync and async paths
```rust
let _ = black_box(result);         // sync iter_custom path
let _ = black_box(evaluator.evaluate_final());  // async iter_batched path
```

### iter_custom + Instant (sync wall-clock pattern, D-03)
**Source:** No existing usage in repo — introduced fresh per D-03. Canonical Criterion 0.7 pattern:
```rust
b.iter_custom(|iters| {
    let mut total = Duration::ZERO;
    for _ in 0..iters {
        // setup (NOT timed)
        let mut gen = setup_auth_gen(n, m, chunking_factor);
        let start = Instant::now();
        // measured work
        let result = /* ... */;
        total += start.elapsed();
        black_box(result);
    }
    total
})
```
**No `use tokio` in functions using this pattern.**

### Throughput::Elements (D-12)
**Source:** `benches/benchmarks.rs` line 450
```rust
group.throughput(Throughput::Elements(n_auth_bits as u64));
```
Adapt to `(n * m) as u64` for AND-gate count in online group.

### BenchmarkId naming (existing convention, lines 86, 461)
```rust
BenchmarkId::new("real_preprocessing", format!("{}x{}", n, m))
BenchmarkId::new("Chunking factor", format!("{}", chunking_factor))
```
For new online benches: `BenchmarkId::new(format!("p1_garble_eval_check_{}x{}", n, m), chunking_factor)` — or adopt a flat `BenchmarkId::new("p1", format!("{}x{}/cf={}", n, m, chunking_factor))` if a two-level name reads better. Planner decides the naming style.

### warm_up_time / measurement_time (existing convention, lines 440-441)
```rust
group.warm_up_time(Duration::from_secs(5));
group.measurement_time(Duration::from_secs(20));
```
Currently only set on preprocessing group. Apply same or paper-matching values (`Duration::from_secs(3)` warmup, ≥100 samples) to the online group per appendix_experiments.tex. Planner decides whether to match paper or keep current values.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| — | — | — | All patterns have direct codebase analogs |

The only genuinely new pattern is `iter_custom + Instant` (sync wall-clock). No analog exists in the repo today — it is standard Criterion 0.7 API. The planner should reference the Criterion docs or the template above.

---

## Key Notes for Planner

### `assemble_c_gamma_shares_p2` visibility issue
`assemble_c_gamma_shares_p2` is defined as a private function inside `#[cfg(test)] mod tests` in `src/lib.rs` (lines 414-483). Benchmarks cannot call `#[cfg(test)]` code. Options:
1. Promote it to `pub fn assemble_c_gamma_shares_p2(...)` in `src/lib.rs` outside the test block (or move to `src/online.rs`).
2. Inline equivalent logic directly in the benchmark (copy the ~30 lines of assembly code).
The planner must decide and include this as an explicit action in the plan.

### P1 `assemble_c_gamma_shares` same issue
The P1 helper `assemble_c_gamma_shares` is also `#[cfg(test)]` private (same file, same block). Same resolution needed.

### No new Cargo.toml changes needed
`criterion = { version = "0.7", features = ["async_tokio"] }` under `[dev-dependencies]` already covers both async and sync Criterion APIs. `std::time::Instant` and `std::hint::black_box` are stdlib — no new deps.

---

## Metadata

**Analog search scope:** `benches/`, `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`, `src/lib.rs`, `src/online.rs`
**Files scanned:** 6
**Pattern extraction date:** 2026-04-24
