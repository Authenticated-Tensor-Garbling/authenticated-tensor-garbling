use std::hint::black_box;
use std::mem::size_of;
use std::time::Instant;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};

mod network_simulator;
use network_simulator::SimpleNetworkSimulator;

use authenticated_tensor_garbling::{
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
    auth_tensor_gen::AuthTensorGen,
    block::Block,
    preprocessing::run_preprocessing,
};

use once_cell::sync::Lazy;

static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap()
});

// Benchmark parameters — (n, m) pairs matching the paper's sweep
// (appendix_experiments.tex, §Methodology).
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

const X_INPUT: usize = 0b1101;
const Y_INPUT: usize = 0b110;

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// "preprocessing" criterion group
//
// Measures the uncompressed preprocessing pipeline (Pi_aTensor' / Construction 4,
// Appendix F): ideal F_bCOT + leaky_tensor_pre + auth_tensor_pre producing
// TensorFpreGen / TensorFpreEval.
//
// Uses iter_custom + std::time::Instant (pure wall-clock, no async scheduler
// overhead). Communication cost of bCOT is noted as a comment; it is NOT
// part of the measured time.
// ---------------------------------------------------------------------------

/// Benchmarks uncompressed preprocessing (Construction 4) using sync wall-clock
/// measurement via `iter_custom` + `std::time::Instant`.
///
/// Throughput is reported in two complementary units:
///   - ms per tensor op  — elapsed_ns / iterations / 1_000_000  (paper style)
///   - Criterion's AND-gates/s via `Throughput::Elements(n * m)`  (literature style)
fn bench_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("preprocessing");
    group.warm_up_time(std::time::Duration::from_secs(5));
    group.measurement_time(std::time::Duration::from_secs(20));

    let block_sz = size_of::<Block>();
    let chunking_factor = 1;

    for &(n, m) in BENCHMARK_PARAMS {
        // Throughput: total authenticated bits produced per preprocessing call.
        // n alpha_bits + m beta_bits + n*m correlated_bits + n*m gamma_bits = n + m + 2*n*m
        // Reported as AND-gate count (n*m) for literature-style ns-per-AND-gate comparison.
        group.throughput(Throughput::Elements((n * m) as u64));

        // Communication estimate (not measured, for reference):
        //   bCOT phase: 2 rounds × (n + m + 2·n·m) authenticated bits × 16 bytes per Block
        let _bcot_bytes = 2 * (n + m + 2 * n * m) * block_sz;

        if n * m > 4096 {
            group.sample_size(10);
        }

        group.bench_with_input(
            BenchmarkId::new("real_preprocessing", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter_custom(|iters| {
                    let mut total = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        let (fpre_gen, fpre_eval) = run_preprocessing(n, m, 1, chunking_factor);
                        total += start.elapsed();
                        // black_box prevents dead-code elimination of the preprocessing output.
                        black_box(fpre_gen);
                        black_box(fpre_eval);
                    }
                    total
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Network-simulation async benchmarks (paper comparison group)
//
// These preserve the 100 Mbps async network simulation matching the paper's
// experimental setup (appendix_experiments.tex §Methodology). They are kept
// separately from the "online" sync group so paper-comparison numbers remain
// reproducible.
//
// The seven near-identical per-size functions are replaced by a single
// parameterized helper `bench_online_with_networking_for_size` called once
// per (n, m) in BENCHMARK_PARAMS.
// ---------------------------------------------------------------------------

fn bench_online_with_networking_for_size(c: &mut Criterion, n: usize, m: usize) {
    let mut group = c.benchmark_group(format!("Runtime with networking for {}x{}", n, m));
    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8_usize {
        // Pre-compute garble output byte count outside the timed loop for
        // accurate network-cost accounting (matches existing per-size approach).
        let mut generator = setup_auth_gen(n, m, chunking_factor);
        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();

        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize = second_cts.iter().map(|row| row.len() * block_sz).sum();
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;

        println!(
            "Total bytes for input size {}x{} with chunking factor {} is {}",
            n, m, chunking_factor, total_bytes
        );

        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &chunking_factor,
            |b, &chunking_factor| {
                b.to_async(&*RT).iter_batched(
                    || {
                        (
                            setup_auth_gen(n, m, chunking_factor),
                            setup_auth_eval(n, m, chunking_factor),
                            SimpleNetworkSimulator::new(100.0, 0),
                        )
                    },
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) =
                            generator.garble_second_half();
                        black_box(generator.garble_final());

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator
                            .evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator
                            .evaluate_second_half(second_levels_inner, second_cts_inner);
                        black_box(evaluator.evaluate_final());
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

// One benchmark function per size — required by criterion_group! macro which
// expects `fn(&mut Criterion)` items.

fn bench_4x4_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 4, 4);
}
fn bench_8x8_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 8, 8);
}
fn bench_16x16_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 16, 16);
}
fn bench_32x32_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 32, 32);
}
fn bench_64x64_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 64, 64);
}
fn bench_128x128_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 128, 128);
}
fn bench_256x256_runtime_with_networking(c: &mut Criterion) {
    bench_online_with_networking_for_size(c, 256, 256);
}

criterion_group!(
    benches,
    bench_4x4_runtime_with_networking,
    bench_8x8_runtime_with_networking,
    bench_16x16_runtime_with_networking,
    bench_32x32_runtime_with_networking,
    bench_64x64_runtime_with_networking,
    bench_128x128_runtime_with_networking,
    bench_256x256_runtime_with_networking,
    bench_preprocessing,
);

criterion_main!(benches);
