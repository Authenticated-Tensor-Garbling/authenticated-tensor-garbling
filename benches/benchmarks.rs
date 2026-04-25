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
    online::check_zero,
    preprocessing::run_preprocessing,
    assemble_c_gamma_shares,
    assemble_c_gamma_shares_p2,
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
// "online" criterion group — Protocol 1 and Protocol 2
//
// Sync iter_custom + std::time::Instant benchmarks for the full online phase:
// garble + evaluate + lambda reconstruction + c_gamma assembly + check_zero.
// No tokio, no network simulation. Sweeps BENCHMARK_PARAMS × chunking_factor 1..=8.
// ---------------------------------------------------------------------------

/// Phase 10 BENCH-02 / BENCH-04 / BENCH-06 — Protocol 1 online-phase
/// throughput benchmark. Sync `iter_custom + Instant` (no tokio, no
/// network simulation). Sweeps BENCHMARK_PARAMS × chunking_factor 1..=8
/// (D-13). Each measured iteration runs the full P1 pipeline:
///   garble_first_half -> evaluate_first_half
///   garble_second_half -> evaluate_second_half
///   garble_final -> evaluate_final
///   gen.compute_lambda_gamma -> ev.compute_lambda_gamma
///   assemble_c_gamma_shares -> online::check_zero (under delta_a).
/// Reports throughput two ways (D-11): ms-per-tensor-op (paper Table 1
/// units) and ns-per-AND-gate (crypto literature units). Both are
/// printed via println! after each iter_custom completes; Criterion's
/// own AND-gates/s line is driven by Throughput::Elements (D-12).
fn bench_online_p1(c: &mut Criterion) {
    let mut group = c.benchmark_group("online");
    // Match Plan 02's preprocessing group convention.
    group.warm_up_time(std::time::Duration::from_secs(3));
    group.measurement_time(std::time::Duration::from_secs(20));

    for &(n, m) in BENCHMARK_PARAMS {
        // BENCH-06 / D-12: Criterion auto-prints AND-gates/s when
        // Throughput::Elements is set to the AND-gate count per call.
        group.throughput(Throughput::Elements((n * m) as u64));

        if n * m > 4096 {
            group.sample_size(10);
        }

        for chunking_factor in 1usize..=8 {
            group.bench_with_input(
                BenchmarkId::new(
                    format!("p1_garble_eval_check_{}x{}", n, m),
                    chunking_factor,
                ),
                &chunking_factor,
                |b, &chunking_factor| {
                    b.iter_custom(|iters| {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            // Setup OUTSIDE timed region.
                            let mut generator = setup_auth_gen(n, m, chunking_factor);
                            let mut evaluator = setup_auth_eval(n, m, chunking_factor);

                            // For benchmarking the per-gate compute
                            // cost we use FIXED zero masks for
                            // l_alpha_pub / l_beta_pub. The c_gamma
                            // assembly's INSTRUCTION COUNT does not
                            // depend on whether the mask bits are 0 or
                            // 1 — both branches do the same XOR work,
                            // just selectively (under `if l_alpha_pub[i]`
                            // / `if l_beta_pub[j]`). Using all-zeros
                            // gives a consistent, reproducible cost
                            // measurement. l_gamma_pub is reconstructed
                            // exactly as the P1 integration test does
                            // (gb.compute_lambda_gamma -> ev.compute_lambda_gamma).
                            let l_alpha_pub: Vec<bool> = vec![false; n];
                            let l_beta_pub:  Vec<bool> = vec![false; m];

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half();
                            evaluator.evaluate_first_half(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half();
                            evaluator.evaluate_second_half(cl2, ct2);
                            generator.garble_final();
                            evaluator.evaluate_final();

                            // Lambda_gamma reconstruction (consumes
                            // garbled output state on both sides).
                            let lambda_gb = generator.compute_lambda_gamma();
                            let l_gamma_pub = evaluator
                                .compute_lambda_gamma(&lambda_gb);

                            // c_gamma assembly + check_zero (D-09).
                            let c_gamma = assemble_c_gamma_shares(
                                n, m,
                                &l_alpha_pub,
                                &l_beta_pub,
                                &l_gamma_pub,
                                &generator,
                                &evaluator,
                            );
                            let check_ok = check_zero(&c_gamma, &generator.delta_a);

                            total += start.elapsed();

                            // BENCH-01 / D-04: black_box every output.
                            let _ = black_box(c_gamma);
                            let _ = black_box(check_ok);
                            // Also black_box the mutated structs so
                            // the optimizer cannot prove the entire
                            // pipeline dead.
                            let _ = black_box(&generator);
                            let _ = black_box(&evaluator);
                        }

                        // D-11 dual-unit throughput print. Criterion's
                        // built-in output already covers ns-per-element
                        // via Throughput::Elements; this println adds
                        // the paper-style ms-per-op alongside.
                        let elapsed_ns = total.as_nanos() as f64;
                        let iters_f = iters as f64;
                        let ms_per_op = elapsed_ns / iters_f / 1_000_000.0;
                        let ns_per_and = elapsed_ns / (iters_f * (n * m) as f64);
                        println!(
                            "[online][p1] {}x{} cf={}: {:.3} ms/op, {:.2} ns/AND",
                            n, m, chunking_factor, ms_per_op, ns_per_and
                        );

                        total
                    });
                },
            );
        }
    }
    group.finish();
}

/// Phase 10 BENCH-02 / BENCH-04 / BENCH-06 — Protocol 2 online-phase
/// throughput benchmark. Sync `iter_custom + Instant` mirroring P1.
/// Each measured iteration runs the full P2 pipeline:
///   garble_first_half_p2 -> evaluate_first_half_p2
///   garble_second_half_p2 -> evaluate_second_half_p2
///   garble_final_p2 -> evaluate_final_p2
///   assemble_c_gamma_shares_p2 -> online::check_zero (UNDER DELTA_B,
///   not delta_a — P2's c_gamma assembly verifies under the
///   evaluator's delta per CONTEXT.md D-09 and src/lib.rs P2 helper).
/// Reports dual-unit throughput (ms-per-op + ns-per-AND-gate, D-11).
fn bench_online_p2(c: &mut Criterion) {
    let mut group = c.benchmark_group("online");
    group.warm_up_time(std::time::Duration::from_secs(3));
    group.measurement_time(std::time::Duration::from_secs(20));

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        if n * m > 4096 {
            group.sample_size(10);
        }

        for chunking_factor in 1usize..=8 {
            group.bench_with_input(
                BenchmarkId::new(
                    format!("p2_garble_eval_check_{}x{}", n, m),
                    chunking_factor,
                ),
                &chunking_factor,
                |b, &chunking_factor| {
                    b.iter_custom(|iters| {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let mut generator = setup_auth_gen(n, m, chunking_factor);
                            let mut evaluator = setup_auth_eval(n, m, chunking_factor);

                            // l_gamma_pub for the P2 check is the same
                            // reconstruction shape as P1 (length n*m,
                            // column-major). For benchmarking we use
                            // a fixed-zero vector — P2's
                            // assemble_c_gamma_shares_p2 cost is O(n*m)
                            // regardless of bit values (no branches on
                            // l_gamma_pub bits, only XORs). See
                            // src/lib.rs:assemble_c_gamma_shares_p2.
                            let l_gamma_pub: Vec<bool> = vec![false; n * m];

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half_p2();
                            evaluator.evaluate_first_half_p2(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half_p2();
                            evaluator.evaluate_second_half_p2(cl2, ct2);
                            let (_d_gb_out, gb_d_ev_out) = generator.garble_final_p2();
                            let ev_d_ev_out = evaluator.evaluate_final_p2();

                            // c_gamma assembly + check_zero under
                            // delta_b (D-09).
                            let c_gamma = assemble_c_gamma_shares_p2(
                                n, m,
                                &gb_d_ev_out,
                                &ev_d_ev_out,
                                &l_gamma_pub,
                                &generator,
                                &evaluator,
                            );
                            let check_ok = check_zero(&c_gamma, &evaluator.delta_b);

                            total += start.elapsed();

                            // BENCH-01 / D-04.
                            let _ = black_box(c_gamma);
                            let _ = black_box(check_ok);
                            let _ = black_box(gb_d_ev_out);
                            let _ = black_box(ev_d_ev_out);
                            let _ = black_box(&generator);
                            let _ = black_box(&evaluator);
                        }

                        let elapsed_ns = total.as_nanos() as f64;
                        let iters_f = iters as f64;
                        let ms_per_op = elapsed_ns / iters_f / 1_000_000.0;
                        let ns_per_and = elapsed_ns / (iters_f * (n * m) as f64);
                        println!(
                            "[online][p2] {}x{} cf={}: {:.3} ms/op, {:.2} ns/AND",
                            n, m, chunking_factor, ms_per_op, ns_per_and
                        );

                        total
                    });
                },
            );
        }
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

// One benchmark function per size — required by the criterion group macro which
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
