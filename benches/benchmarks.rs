use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

use authenticated_tensor_garbling::{
    tensor_gen::TensorProductGen,
    tensor_eval::TensorProductEval,
    tensor_pre::SemiHonestTensorPre,
    auth_tensor_gen::AuthTensorGen,
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
};

// Benchmark parameters - different (n, m) combinations
const BENCHMARK_PARAMS: &[(usize, usize)] = &[
    (4, 4),     // Small
    (8, 8),     // Medium
    (16, 16),   // Large
    (32, 32),   // Very large
];

const CHUNKING_FACTOR: usize = 6;

// Setup functions for semi-honest protocols
fn setup_semihonest_gen(n: usize, m: usize) -> TensorProductGen {
    let mut pre = SemiHonestTensorPre::new(0, n, m, CHUNKING_FACTOR);
    
    pre.gen_inputs(0b1101, 0b110); // Example input values
    pre.gen_masks();
    pre.mask_inputs();

    let (fpre_gen, _) = pre.into_gen_eval();
    TensorProductGen::new_from_fpre_gen(fpre_gen)
}

fn setup_semihonest_eval(n: usize, m: usize) -> TensorProductEval {
    let mut pre = SemiHonestTensorPre::new(1, n, m, CHUNKING_FACTOR);
    
    pre.gen_inputs(0b1101, 0b110); // Example input values
    pre.gen_masks();
    pre.mask_inputs();

    let (_, fpre_eval) = pre.into_gen_eval();
    TensorProductEval::new_from_fpre_eval(fpre_eval)
}

// Setup functions for authenticated protocols
fn setup_auth_gen(n: usize, m: usize) -> AuthTensorGen {
    let mut fpre = TensorFpre::new(0, n, m, CHUNKING_FACTOR);
    fpre.generate_with_input_values(0b1101, 0b110); // Example input values
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, CHUNKING_FACTOR);
    fpre.generate_with_input_values(0b1101, 0b110); // Example input values
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}

// Benchmark first half garbling
fn bench_first_half_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_half_garbling");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Semi-honest first half garbling
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_semihonest_gen(n, m);
                    generator.garble_first_half_outer_product()
                })
            },
        );
        
        // Authenticated first half garbling
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_auth_gen(n, m);
                    generator.garble_first_half()
                })
            },
        );
    }
    group.finish();
}

// Benchmark first half evaluation
fn bench_first_half_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_half_evaluation");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Semi-honest first half evaluation
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_semihonest_gen(n, m);
                let (chunk_levels, chunk_cts) = generator.garble_first_half_outer_product();
                
                b.iter(|| {
                    let mut evaluator = setup_semihonest_eval(n, m);
                    evaluator.evaluate_first_half_outer_product(chunk_levels.clone(), chunk_cts.clone())
                })
            },
        );
        
        // Authenticated first half evaluation
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m);
                let (chunk_levels, chunk_cts) = generator.garble_first_half();
                
                b.iter(|| {
                    let mut evaluator = setup_auth_eval(n, m);
                    evaluator.evaluate_first_half(chunk_levels.clone(), chunk_cts.clone())
                })
            },
        );
    }
    group.finish();
}

// Benchmark second half garbling
fn bench_second_half_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("second_half_garbling");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Semi-honest second half garbling
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_semihonest_gen(n, m);
                    generator.garble_second_half_outer_product()
                })
            },
        );
        
        // Authenticated second half garbling
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_auth_gen(n, m);
                    generator.garble_second_half()
                })
            },
        );
    }
    group.finish();
}

// Benchmark second half evaluation
fn bench_second_half_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("second_half_evaluation");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Semi-honest second half evaluation
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_semihonest_gen(n, m);
                let (chunk_levels, chunk_cts) = generator.garble_second_half_outer_product();
                
                b.iter(|| {
                    let mut evaluator = setup_semihonest_eval(n, m);
                    evaluator.evaluate_second_half_outer_product(chunk_levels.clone(), chunk_cts.clone())
                })
            },
        );
        
        // Authenticated second half evaluation
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m);
                let (chunk_levels, chunk_cts) = generator.garble_second_half();
                
                b.iter(|| {
                    let mut evaluator = setup_auth_eval(n, m);
                    evaluator.evaluate_second_half(chunk_levels.clone(), chunk_cts.clone())
                })
            },
        );
    }
    group.finish();
}

// Benchmark full protocol (first + second + final)
fn bench_full_protocol_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_garbling");
    
    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));
        
        // Semi-honest full protocol
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_semihonest_gen(n, m);
                    let _first = generator.garble_first_half_outer_product();
                    let _second = generator.garble_second_half_outer_product();
                    generator.garble_final_outer_product()
                })
            },
        );
        
        // Authenticated full protocol
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                b.iter(|| {
                    let mut generator = setup_auth_gen(n, m);
                    let _first = generator.garble_first_half();
                    let _second = generator.garble_second_half();
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
        
        // Semi-honest full protocol evaluation
        group.bench_with_input(
            BenchmarkId::new("semihonest", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_semihonest_gen(n, m);
                let (first_levels, first_cts) = generator.garble_first_half_outer_product();
                let (second_levels, second_cts) = generator.garble_second_half_outer_product();
                let _final_result = generator.garble_final_outer_product();
                
                b.iter(|| {
                    let mut evaluator = setup_semihonest_eval(n, m);
                    evaluator.evaluate_first_half_outer_product(first_levels.clone(), first_cts.clone());
                    evaluator.evaluate_second_half_outer_product(second_levels.clone(), second_cts.clone());
                    evaluator.evaluate_final_outer_product()
                })
            },
        );
        
        // Authenticated full protocol evaluation
        group.bench_with_input(
            BenchmarkId::new("authenticated", format!("{}x{}", n, m)),
            &(n, m),
            |b, &(n, m)| {
                // Setup generator to get garbled data
                let mut generator = setup_auth_gen(n, m);
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();
                
                b.iter(|| {
                    let mut evaluator = setup_auth_eval(n, m);
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
    bench_first_half_garbling,
    bench_first_half_evaluation,
    bench_second_half_garbling,
    bench_second_half_evaluation,
    bench_full_protocol_garbling,
    bench_full_protocol_evaluation
);

criterion_main!(benches);