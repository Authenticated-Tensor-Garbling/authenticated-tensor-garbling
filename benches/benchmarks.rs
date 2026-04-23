use std::time::Duration;
use std::mem::size_of;

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};

mod network_simulator;
use network_simulator::SimpleNetworkSimulator;

use authenticated_tensor_garbling::{
    block::Block,
    tensor_gen::TensorProductGen,
    tensor_eval::TensorProductEval,
    tensor_pre::SemiHonestTensorPre,
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

// Benchmark parameters - different (n, m) combinations
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
const Y_INPUT: usize= 0b110;

// Setup functions for semi-honest protocols
fn _setup_semihonest_gen(n: usize, m: usize, chunking_factor: usize) -> TensorProductGen {

    let mut pre = SemiHonestTensorPre::new(0, n, m, chunking_factor);
    
    pre.gen_inputs(X_INPUT, Y_INPUT); // Example input values
    pre.gen_masks();
    pre.mask_inputs();

    let (fpre_gen, _) = pre.into_gen_eval();
    TensorProductGen::new_from_fpre_gen(fpre_gen)
}

fn _setup_semihonest_eval(n: usize, m: usize, chunking_factor: usize) -> TensorProductEval {
    let mut pre = SemiHonestTensorPre::new(1, n, m, chunking_factor);
    
    pre.gen_inputs(X_INPUT, Y_INPUT); // Example input values
    pre.gen_masks();
    pre.mask_inputs();

    let (_, fpre_eval) = pre.into_gen_eval();
    TensorProductEval::new_from_fpre_eval(fpre_eval)
}

// Setup functions for authenticated protocols
fn setup_auth_gen(n: usize, m: usize, chunking_factor: usize) -> AuthTensorGen {

    let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT); // Example input values
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize, chunking_factor: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT); // Example input values
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}

// Benchmarks online garbling for the authenticated tensor gate (Pi_Garble, §4 / auth_tensor_gen): first half, second half, and final combine across BENCHMARK_PARAMS dimensions and chunking factors [1, 2, 4, 6, 8].
fn bench_full_protocol_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_garbling");

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        // Authenticated full protocol evaluation across the original sweep
        // of chunking factors. The literal [1, 2, 4, 6, 8] is intentional —
        // these are the factors historically benchmarked; 3/5/7 are skipped
        // by design. cf.to_string() preserves the prior BenchmarkId strings
        // ("1"/"2"/"4"/"6"/"8") so Criterion baselines under
        // target/criterion/full_protocol_garbling/{1,2,4,6,8}/ remain valid.
        for cf in [1usize, 2, 4, 6, 8] {
            let mut generator = setup_auth_gen(n, m, cf);
            group.bench_with_input(
                BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m)),
                &(n, m),
                |b, &(_n, _m)| {
                    b.iter(|| {
                        let (_first_levels, _first_cts) = generator.garble_first_half();
                        let (_second_levels, _second_cts) = generator.garble_second_half();
                        generator.garble_final();
                    })
                },
            );
        }
    }
    group.finish();
}

// Benchmarks online garbling + evaluation for the authenticated tensor gate (auth_tensor_gen + auth_tensor_eval) with simulated network I/O between parties at 100 Mbps; sweeps chunking factors [1, 2, 4, 6, 8] across BENCHMARK_PARAMS.
fn bench_full_protocol_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_with_networking");
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(30));

    let block_sz = size_of::<Block>();

    for &(n, m) in BENCHMARK_PARAMS {
        // Sweep over the same chunking factors as bench_full_protocol_garbling.
        // Explicit list (not 1..=8) preserves pre-refactor BenchmarkId strings
        // "1"/"2"/"4"/"6"/"8" via cf.to_string(), keeping prior Criterion
        // baselines at target/criterion/full_protocol_with_networking/{1,2,4,6,8}/
        // valid (RESEARCH Q4 recommendation extends D-17 to this function).
        for cf in [1usize, 2, 4, 6, 8] {
            let chunking_factor = cf;

            let mut generator = setup_auth_gen(n, m, chunking_factor);

            let (first_levels, first_cts) = generator.garble_first_half();
            let (second_levels, second_cts) = generator.garble_second_half();
            generator.garble_final();

            let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
            let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
            let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
            let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();

            let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;

            group.throughput(Throughput::Bytes(total_bytes as u64));

            group.bench_with_input(
                BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m)),
                &(n, m),
                |b, &(n, m)| {
                    b.to_async(&*RT)
                    .iter_batched(
                        || (
                            setup_auth_gen(n, m, chunking_factor),
                            setup_auth_eval(n, m, chunking_factor),
                            SimpleNetworkSimulator::new(100.0, 0)
                        ),
                        |(mut generator, mut evaluator, network)| async move {
                            let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                            let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                            generator.garble_final();

                            network.send_size_with_metrics(total_bytes).await;

                            evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                            evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                            evaluator.evaluate_final();
                    },
                    BatchSize::SmallInput
                )},
            );
        }
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 4x4, sweeping chunking factors 1..=8 (auth_tensor_gen + auth_tensor_eval + SimpleNetworkSimulator).
fn bench_4x4_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 4x4");
    // group.warm_up_time(Duration::from_secs(10));
    // group.measurement_time(Duration::from_secs(30));

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 4;
        let m = 4;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);
        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 8x8, sweeping chunking factors 1..=8.
fn bench_8x8_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 8x8");
    // group.warm_up_time(Duration::from_secs(10));
    // group.measurement_time(Duration::from_secs(30));

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 8;
        let m = 8;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);
        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 16x16, sweeping chunking factors 1..=8.
fn bench_16x16_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 16x16");

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 16;
        let m = 16;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);
        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 32x32, sweeping chunking factors 1..=8.
fn bench_32x32_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 32x32");

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 32;
        let m = 32;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);

        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 64x64, sweeping chunking factors 1..=8.
fn bench_64x64_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 64x64");

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 64;
        let m = 64;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);

        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 128x128, sweeping chunking factors 1..=8.
fn bench_128x128_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 128x128");

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 128;
        let m = 128;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);

        
        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks online garbling + network I/O for the authenticated tensor gate at fixed dimension 256x256, sweeping chunking factors 1..=8.
fn bench_256x256_runtime_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("Runtime with networking for 256x256");

    let block_sz = size_of::<Block>();

    for chunking_factor in 1..=8 {
        let n = 256;
        let m = 256;

        let mut generator = setup_auth_gen(n, m, chunking_factor);

        let (first_levels, first_cts) = generator.garble_first_half();
        let (second_levels, second_cts) = generator.garble_second_half();
        generator.garble_final();
        
        let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
        let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
        let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
                        
        let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
        println!("Total bytes for input size {}x{} with chunking factor {} is {}", n, m, chunking_factor, total_bytes);

        group.bench_with_input(
            BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
            &(chunking_factor),
            |b, &chunking_factor| {
                b.to_async(&*RT)
                .iter_batched(
                    || (
                        setup_auth_gen(n, m, chunking_factor), 
                        setup_auth_eval(n, m, chunking_factor), 
                        SimpleNetworkSimulator::new(100.0, 0)
                    ),
                    |(mut generator, mut evaluator, network)| async move {
                        let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
                        let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
                        generator.garble_final();

                        network.send_size_with_metrics(total_bytes).await;

                        evaluator.evaluate_first_half(first_levels_inner, first_cts_inner);
                        evaluator.evaluate_second_half(second_levels_inner, second_cts_inner);
                        evaluator.evaluate_final();
                },
                BatchSize::SmallInput
            )},
        );
    }
    group.finish();
}

// Benchmarks the uncompressed preprocessing pipeline (Pi_aTensor' / Construction 4, Appendix F): ideal F_bCOT + leaky_tensor_pre + auth_tensor_pre producing TensorFpreGen / TensorFpreEval output, plus simulated bCOT network bandwidth accounting.
fn bench_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("preprocessing");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(20));

    let block_sz = size_of::<Block>();
    let chunking_factor = 1;

    for &(n, m) in BENCHMARK_PARAMS {
        // Throughput: total authenticated bits produced per preprocessing call.
        // n alpha_bits + m beta_bits + n*m correlated_bits + n*m gamma_bits = n + m + 2*n*m
        let n_auth_bits = n + m + 2 * n * m;
        group.throughput(Throughput::Elements(n_auth_bits as u64));

        // Communication estimate for benchmark annotation:
        //   bCOT phase: 2 rounds * (n + m + 2*n*m) authenticated bits * 16 bytes per Block
        let bcot_bytes = 2 * (n + m + 2 * n * m) * block_sz;
        let total_comm_bytes = bcot_bytes;

        if n * m > 4096 {
            group.sample_size(10);
        }

        group.bench_with_input(
            BenchmarkId::new("real_preprocessing", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.to_async(&*RT)
                .iter_batched(
                    || SimpleNetworkSimulator::new(100.0, 0),
                    |network| async move {
                        // run_preprocessing is the measured operation
                        let (_fpre_gen, _fpre_eval) = run_preprocessing(n, m, 1, chunking_factor);
                        // Simulate the communication cost of bCOT (recorded for bandwidth
                        // accounting; not measured in wall time)
                        network.send_size_with_metrics(total_comm_bytes).await;
                    },
                    BatchSize::SmallInput
                )
            },
        );
    }
    group.finish();
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