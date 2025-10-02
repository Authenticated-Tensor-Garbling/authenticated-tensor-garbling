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
};

use mpz_circuits::{Circuit, CircuitBuilder};

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
    fpre.generate_with_input_values(X_INPUT, Y_INPUT); // Example input values
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize, chunking_factor: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, chunking_factor);
    fpre.generate_with_input_values(X_INPUT, Y_INPUT); // Example input values
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}

fn _tensor_and_circuit<const N: usize>() -> Circuit {

    let mut builder = CircuitBuilder::new();
    let x: [_; N] = std::array::from_fn(|_| builder.add_input()); // any way to denote constexpr to set the size like in C++?
    let y: [_; N] = std::array::from_fn(|_| builder.add_input());

    let mut outputs = Vec::new();

    for i in 0..N {
        for j in 0..N {
            outputs.push(builder.add_and_gate(x[i], y[j]));
        }
    }

    for out in outputs {
        builder.add_output(out);
    }

    builder.build().unwrap()
}

// Benchmark full protocol (first + second + final)
fn bench_full_protocol_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_garbling");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        
        // Authenticated full protocol evaluation
        let mut generator = setup_auth_gen(n, m, 1);
        group.bench_with_input(
            BenchmarkId::new("1", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(_n, _m)| {
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        let mut generator = setup_auth_gen(n, m, 2);
        group.bench_with_input(
            BenchmarkId::new("2", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(_n, _m)| {
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        let mut generator = setup_auth_gen(n, m, 4);
        group.bench_with_input(
            BenchmarkId::new("4", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(_n, _m)| {
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        let mut generator = setup_auth_gen(n, m, 6);
        group.bench_with_input(
            BenchmarkId::new("6", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(_n, _m)| {
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );


        let mut generator = setup_auth_gen(n, m, 8);
        group.bench_with_input(
            BenchmarkId::new("8", format!("{}x{}", n, m)),
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
    group.finish();
}

// Benchmark full protocol evaluation
fn bench_full_protocol_with_networking(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_with_networking");
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(30));

    let block_sz = size_of::<Block>();
    
    for &(n, m) in BENCHMARK_PARAMS {
        
        let chunking_factor = 1;

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
            BenchmarkId::new("1", format!("{}x{}", n, m)),
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

        let chunking_factor = 2;

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
            BenchmarkId::new("2", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.to_async(&*RT)
                .iter_batched(
                    || (setup_auth_gen(n, m, chunking_factor), setup_auth_eval(n, m, chunking_factor), SimpleNetworkSimulator::new(100.0, 0)),
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

        let chunking_factor = 4;

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
            BenchmarkId::new("4", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.to_async(&*RT)
                .iter_batched(
                    || (setup_auth_gen(n, m, chunking_factor), setup_auth_eval(n, m, chunking_factor), SimpleNetworkSimulator::new(100.0, 0)),
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

        let chunking_factor = 6;

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
            BenchmarkId::new("6", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.to_async(&*RT)
                .iter_batched(
                    || (setup_auth_gen(n, m, chunking_factor), setup_auth_eval(n, m, chunking_factor), SimpleNetworkSimulator::new(100.0, 0)),
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

        let chunking_factor = 8;

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
            BenchmarkId::new("8", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.to_async(&*RT)
                .iter_batched(
                    || (setup_auth_gen(n, m, chunking_factor), setup_auth_eval(n, m, chunking_factor), SimpleNetworkSimulator::new(100.0, 0)),
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

criterion_group!(
    benches,
    bench_4x4_runtime_with_networking,
    bench_8x8_runtime_with_networking,
    bench_16x16_runtime_with_networking,
    bench_32x32_runtime_with_networking,
    bench_64x64_runtime_with_networking,
    bench_128x128_runtime_with_networking,
    bench_256x256_runtime_with_networking,
);

criterion_main!(benches);