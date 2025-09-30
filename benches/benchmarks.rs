use std::{fmt::format, panic::panic_any};

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

use authenticated_tensor_garbling::{
    tensor_gen::TensorProductGen,
    tensor_eval::TensorProductEval,
    tensor_pre::SemiHonestTensorPre,
    auth_tensor_gen::AuthTensorGen,
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
};
use mpz_circuits::{Circuit, CircuitBuilder};

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
        group.bench_with_input(
            BenchmarkId::new("1", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 1);
                
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("2", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 2);
                
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("4", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 4);
                
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("6", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 6);
                
                b.iter(|| {
                    let (_first_levels, _first_cts) = generator.garble_first_half();
                    let (_second_levels, _second_cts) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );


        group.bench_with_input(
            BenchmarkId::new("8", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 8);
                
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
fn bench_full_protocol_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_evaluation");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Authenticated full protocol evaluation
        group.bench_with_input(
            BenchmarkId::new("1", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 1);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();
                let mut evaluator = setup_auth_eval(n, m, 1);
                
                b.iter(|| {
                        evaluator.evaluate_first_half(first_levels.clone(), first_cts.clone());
                        evaluator.evaluate_second_half(second_levels.clone(), second_cts.clone());
                        evaluator.evaluate_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("2", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 2);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();

                let mut evaluator = setup_auth_eval(n, m, 2);
                
                b.iter(|| {
                        evaluator.evaluate_first_half(first_levels.clone(), first_cts.clone());
                        evaluator.evaluate_second_half(second_levels.clone(), second_cts.clone());
                        evaluator.evaluate_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("4", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 4);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();

                let mut evaluator = setup_auth_eval(n, m, 4);
                
                b.iter(|| {
                        evaluator.evaluate_first_half(first_levels.clone(), first_cts.clone());
                        evaluator.evaluate_second_half(second_levels.clone(), second_cts.clone());
                        evaluator.evaluate_final();
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("6", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 6);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();
                
                let mut evaluator = setup_auth_eval(n, m, 6);

                b.iter(|| {
                        evaluator.evaluate_first_half(first_levels.clone(), first_cts.clone());
                        evaluator.evaluate_second_half(second_levels.clone(), second_cts.clone());
                        evaluator.evaluate_final();
                })
            },
        );


        group.bench_with_input(
            BenchmarkId::new("8", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m, 8);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();

                let mut evaluator = setup_auth_eval(n, m, 8);
                
                b.iter(|| {
                        evaluator.evaluate_first_half(first_levels.clone(), first_cts.clone());
                        evaluator.evaluate_second_half(second_levels.clone(), second_cts.clone());
                        evaluator.evaluate_final();
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_full_protocol_garbling,
    bench_full_protocol_evaluation,
);

criterion_main!(benches);