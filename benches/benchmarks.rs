use std::cell::OnceCell;
use std::hint::black_box;
use std::mem::size_of;
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};

mod network_simulator;
use network_simulator::SimpleNetworkSimulator;

use authenticated_tensor_garbling::{
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
    auth_tensor_gen::AuthTensorGen,
    block::Block,
    input_encoding::encode_inputs,
    online::block_hash_check_zero,
    preprocessing::run_preprocessing,
    assemble_e_input_wire_blocks_p1,
    assemble_c_alpha_beta_blocks_p2,
    CSP, SSP,
};
use authenticated_tensor_garbling::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};

// Network model from `appendix_experiments.tex` line 13: 100 Mbps, no jitter,
// no delay. The new `online` group simulates transit deterministically by
// computing `bytes * 8 / NETWORK_BANDWIDTH_BPS` ns per round and adding it
// onto the measured compute time — no tokio, no scheduler jitter.
const NETWORK_BANDWIDTH_BPS: u64 = 100_000_000;
// Byte widths derived from κ (CSP) and ρ (SSP). KAPPA_BYTES is the on-wire
// width for κ-bit objects (GGM-tree ciphertexts, P1 narrow ciphertexts, the
// CheckZero digest, and the κ-half of P2 wide ciphertexts); RHO_BYTES is the
// width for the ρ-half of P2 wide leaf ciphertexts (`6_total.tex:90`,
// Construction 4). When ρ later changes in the actual computation, bumping
// `SSP` in `src/lib.rs` is the single source-of-truth knob — bench accounting
// and the network-simulator transit time track automatically.
const KAPPA_BYTES: usize = (CSP + 7) / 8;
const RHO_BYTES:   usize = (SSP + 7) / 8;

#[inline]
fn transit_ns(bytes: usize) -> u64 {
    (bytes as u64) * 8 * 1_000_000_000 / NETWORK_BANDWIDTH_BPS
}

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
    // (4, 4),
    // (8, 8),
    // (16, 16),
    // (24, 24),
    // (32, 32),
    // (48, 48),
    (64, 64),
    // (96, 96),
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
    fpre.generate_ideal();
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize, chunking_factor: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, chunking_factor);
    fpre.generate_ideal();
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}

/// Build a CORRELATED (AuthTensorGen, AuthTensorEval) pair for the online-phase
/// benches.
///
/// Unlike `setup_auth_gen` / `setup_auth_eval` — which each spin up an independent
/// `TensorFpre` and call `into_gen_eval()` (leaving `gamma_auth_bit_shares` empty per
/// `src/auth_tensor_fpre.rs:180-183, 194-197`) — this helper invokes the ideal
/// trusted-dealer backend `IdealPreprocessingBackend::run`, which populates the
/// four D_ev field pairs (`alpha_eval` length n, `beta_eval` length m,
/// `correlated_eval` length n*m, `gamma_auth_bit_shares` length n*m) on BOTH
/// the generator and the evaluator with matching IT-MAC shares.
///
/// Required for any online benchmark that calls `assemble_e_input_wire_shares_p1`
/// (paper-faithful P1) or `assemble_c_alpha_beta_shares_p2` (paper-faithful P2).
/// Both helpers assert the `*_eval_shares` lengths (n and m) and the
/// `*_auth_bit_shares` lengths. Without a correlated pair, those asserts
/// panic on the first iteration.
///
/// `count = 1` matches `IdealPreprocessingBackend`'s only supported batch size
/// (see `src/preprocessing.rs:145-150`). The `chunking_factor` is forwarded
/// unchanged.
fn setup_auth_pair(n: usize, m: usize, chunking_factor: usize) -> (AuthTensorGen, AuthTensorEval) {
    let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, chunking_factor);
    (
        AuthTensorGen::new_from_fpre_gen(fpre_gen),
        AuthTensorEval::new_from_fpre_eval(fpre_eval),
    )
}

/// Total GC byte count for Protocol 1 garble output at `(n, m, chunking_factor)`.
/// Sum is `chunk_levels.len() * 2 * KAPPA_BYTES + chunk_cts.len() * KAPPA_BYTES`
/// per chunk over both halves, matching `bench_online_with_networking_for_size:413–417`.
/// Output count is determined by `(n, m, chunking_factor)` alone, so this is
/// called once per cell outside the timed iter loop.
///
/// All P1 ciphertexts are κ-wide (no ρ widening); the protocol is unauthenticated.
fn gc_bytes_p1(n: usize, m: usize, chunking_factor: usize) -> usize {
    let (mut generator, mut evaluator) = setup_auth_pair(n, m, chunking_factor);
    encode_inputs(&mut generator, &mut evaluator, X_INPUT, Y_INPUT, &mut rand::rng());
    let (first_levels, first_cts) = generator.garble_first_half();
    let (second_levels, second_cts) = generator.garble_second_half();
    // GGM-tree internal-node ciphertexts: 2 × κ bits per row.
    let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * KAPPA_BYTES).sum();
    // P1 narrow leaf ciphertext: κ bits per row.
    let cts_bytes_1:    usize = first_cts.iter().map(|row| row.len() * KAPPA_BYTES).sum();
    let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * KAPPA_BYTES).sum();
    let cts_bytes_2:    usize = second_cts.iter().map(|row| row.len() * KAPPA_BYTES).sum();
    levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2
}

/// Total GC byte count for Protocol 2 garble output. P2's `chunk_cts` is a
/// `Vec<Vec<(Block, Block)>>` (wide leaf ciphertexts per `6_total.tex:90`, the
/// κ + ρ extension): the .0 component carries the κ-bit Δ_gb-label material,
/// .1 carries the ρ-bit Δ_ev-MAC material. GGM-tree level ciphertexts stay
/// κ-wide.
///
/// Per-row width on the wire:
///   * `levels`: 2 × KAPPA_BYTES (left + right child of GGM-tree internal node).
///   * `cts`:    KAPPA_BYTES + RHO_BYTES (κ-half + ρ-half of wide leaf cipher).
///
/// In-memory the ρ-half is still a full `Block` (the cryptographic computation
/// is unchanged). This function reports only the on-wire byte count, which is
/// what the network simulator sleeps on and what the paper's communication
/// formulas refer to.
fn gc_bytes_p2(n: usize, m: usize, chunking_factor: usize) -> usize {
    let (mut generator, mut evaluator) = setup_auth_pair(n, m, chunking_factor);
    encode_inputs(&mut generator, &mut evaluator, X_INPUT, Y_INPUT, &mut rand::rng());
    let (first_levels, first_cts) = generator.garble_first_half_p2();
    let (second_levels, second_cts) = generator.garble_second_half_p2();
    // GGM-tree internal-node ciphertexts stay κ-wide (`6_total.tex:90`).
    let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * KAPPA_BYTES).sum();
    // Wide leaf ciphertexts: κ + ρ bits per row (`6_total.tex:90`, Construction 4).
    let cts_bytes_1:    usize = first_cts.iter().map(|row| row.len() * (KAPPA_BYTES + RHO_BYTES)).sum();
    let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * KAPPA_BYTES).sum();
    let cts_bytes_2:    usize = second_cts.iter().map(|row| row.len() * (KAPPA_BYTES + RHO_BYTES)).sum();
    levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2
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
// "online_p1" / "online_p2" criterion groups — Protocol 1 and Protocol 2
//
// Sync iter_custom + std::time::Instant benchmarks for the online phase
// (garble + GC transfer + evaluate + consistency check) under a 100 Mbps
// network model with deterministic computed transit times. Sweeps
// BENCHMARK_PARAMS × chunking_factor 1..=8.
//
// Per-cell output (in addition to Criterion's own per-cell summary in µs):
// one `KB,p{1|2},N=…,M=…,tile=…,kb=…` line emitted on first invocation of the
// outer `bench_with_input` closure (deduped via OnceCell since Criterion calls
// the closure once per sample). A plotting script joins these `KB,…` lines
// with Criterion's `target/criterion/online/<id>/new/estimates.json` (ms in
// `time.point_estimate`, ns) to regenerate the paper's
// `Figures/{N}x{N}_wallclock_bar.pdf` / `_communication.pdf` series.
//
// Sample size: Criterion default (100). Large cells (256×256 × tile 8) take
// minutes per cell — budget accordingly when running the full sweep.
// ---------------------------------------------------------------------------

/// Protocol 1 online-phase throughput benchmark, paper-faithful per
/// `5_online.tex` Construction `prot:krrw`. Each measured iteration runs:
///   - garble_first_half / second_half / final  (gb compute)
///   - GC transfer at 100 Mbps  (computed transit time)
///   - evaluate_first_half / second_half / final  (ev compute)
///   - ev → gb send-back of masked values `a⊕λ_a, b⊕λ_b`  (`5_online.tex:228`)
///   - assemble_e_input_wire_shares_p1
///   - both parties hash via `online::hash_check_zero`, gb sends 16-byte digest,
///     ev compares  (paper-faithful `H({V_w})`, `5_online.tex:226–247`)
///
/// Reports ms-per-op, ns-per-AND, and KB-per-op per (n, m, tile_size).
fn bench_online_p1(c: &mut Criterion) {
    let mut group = c.benchmark_group("online_p1");
    group.warm_up_time(std::time::Duration::from_secs(3));
    group.measurement_time(std::time::Duration::from_secs(20));

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        for chunking_factor in 1usize..=8 {
            // Criterion calls the `bench_with_input` closure once per sample
            // (warm-up + measurement, ~30+ times per cell). Cache gc_bytes in
            // a `OnceCell` so the (potentially expensive) garble-only setup
            // runs once per cell, and emit the KB line on first init only.
            let gc_cache: OnceCell<usize> = OnceCell::new();

            group.bench_with_input(
                BenchmarkId::new(
                    format!("p1_garble_eval_check_{}x{}", n, m),
                    chunking_factor,
                ),
                &chunking_factor,
                |b, &chunking_factor| {
                    let gc_bytes = *gc_cache.get_or_init(|| {
                        let bytes = gc_bytes_p1(n, m, chunking_factor);
                        // P1 send-back of masked values ev → gb
                        // (`5_online.tex:228`): (n + m) bits, ceil-div to bytes.
                        let sendback_bytes = (n + m + 7) / 8;
                        // CheckZero digest (`5_online.tex:226–247`) is κ bits.
                        let comm_bytes_per_op = bytes + sendback_bytes + KAPPA_BYTES;
                        // One KB line per cell — emitted lazily so we never
                        // print for filtered-out cells. `kappa`/`rho` embedded
                        // so figures self-label which parameter set generated
                        // them; P1 has no ρ component but the field is kept
                        // for parser-uniformity with the P2 line.
                        println!(
                            "KB,p1,N={},M={},tile={},kappa={},rho={},kb={:.4}",
                            n, m, chunking_factor, CSP, SSP,
                            (comm_bytes_per_op as f64) / 1024.0,
                        );
                        bytes
                    });
                    let sendback_bytes = (n + m + 7) / 8;
                    let transit_per_iter = Duration::from_nanos(
                        transit_ns(gc_bytes) + transit_ns(sendback_bytes) + transit_ns(KAPPA_BYTES),
                    );

                    b.iter_custom(|iters| {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let (mut generator, mut evaluator) = setup_auth_pair(n, m, chunking_factor);
                            // Input encoding (preprocessing → input encoding → garbling).
                            // Done outside the timed region to preserve prior bench
                            // semantics (online compute = garble + evaluate + CheckZero).
                            // Use input=(0, 0) per run_full_protocol_1's convention.
                            encode_inputs(&mut generator, &mut evaluator, 0, 0, &mut rand::rng());

                            let l_alpha_pub: Vec<bool> = vec![false; n];
                            let l_beta_pub:  Vec<bool> = vec![false; m];

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half();
                            evaluator.evaluate_first_half(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half();
                            evaluator.evaluate_second_half(cl2, ct2);
                            generator.garble_final();
                            evaluator.evaluate_final();

                            let gb_v_alpha_eval: Vec<Block> = generator.alpha_eval.clone();
                            let ev_v_alpha_eval: Vec<Block> = evaluator.alpha_eval.clone();
                            let gb_v_beta_eval:  Vec<Block> = generator.beta_eval.clone();
                            let ev_v_beta_eval:  Vec<Block> = evaluator.beta_eval.clone();

                            let (e_gen_blocks, e_eval_blocks) = assemble_e_input_wire_blocks_p1(
                                n, m,
                                &gb_v_alpha_eval,
                                &ev_v_alpha_eval,
                                &gb_v_beta_eval,
                                &ev_v_beta_eval,
                                &l_alpha_pub,
                                &l_beta_pub,
                                &generator,
                                &evaluator,
                            );

                            // Each party hashes its own block vector; in a real
                            // protocol they exchange digests over the network. For
                            // honest parties the digests must match (since per-index
                            // gen_block == eval_block). Both hashes are computed here
                            // so the timing captures each party's CheckZero cost.
                            // Correctness of mismatch detection is exercised by
                            // tampering tests in src/lib.rs, not here.
                            let h_gb = block_hash_check_zero(&e_gen_blocks);
                            let h_ev = block_hash_check_zero(&e_eval_blocks);
                            let _h_simulated_match = h_gb == h_ev;

                            total += start.elapsed() + transit_per_iter;

                            let _ = black_box(e_gen_blocks);
                            let _ = black_box(e_eval_blocks);
                            let _ = black_box(_h_simulated_match);
                            let _ = black_box(&generator);
                            let _ = black_box(&evaluator);
                        }

                        total
                    });
                },
            );
        }
    }
    group.finish();
}

/// Protocol 2 online-phase throughput benchmark, paper-faithful per
/// `6_total.tex` Construction `prot:wrk`. Each measured iteration runs:
///   - garble_first_half_p2 / second_half_p2 / final_p2  (gb compute)
///   - GC transfer at 100 Mbps  (computed transit time)
///   - evaluate_first_half_p2 / second_half_p2 / final_p2  (ev compute)
///   - assemble_c_alpha_beta_shares_p2  (input-wire check, `6_total.tex:207–214`)
///   - both parties hash via `online::hash_check_zero`, gb sends 16-byte digest,
///     ev compares  (paper-faithful CheckZero, `6_total.tex:214`)
///
/// No masked-value send-back (P2 keeps masked values inside the GC and never
/// reveals them to gb). GC byte count differs from P1: both halves' chunk_cts
/// are `(Block, Block)` tuples (wide ciphertexts, `6_total.tex:90`), so
/// per-cell bytes are computed via `gc_bytes_p2`.
///
/// Reports ms-per-op, ns-per-AND, and KB-per-op per (n, m, tile_size).
fn bench_online_p2(c: &mut Criterion) {
    let mut group = c.benchmark_group("online_p2");
    group.warm_up_time(std::time::Duration::from_secs(3));
    group.measurement_time(std::time::Duration::from_secs(20));

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        for chunking_factor in 1usize..=8 {
            // See bench_online_p1 for OnceCell rationale.
            let gc_cache: OnceCell<usize> = OnceCell::new();

            group.bench_with_input(
                BenchmarkId::new(
                    format!("p2_garble_eval_check_{}x{}", n, m),
                    chunking_factor,
                ),
                &chunking_factor,
                |b, &chunking_factor| {
                    let gc_bytes = *gc_cache.get_or_init(|| {
                        let bytes = gc_bytes_p2(n, m, chunking_factor);
                        // CheckZero digest (`6_total.tex:214`) is κ bits.
                        let comm_bytes_per_op = bytes + KAPPA_BYTES;
                        println!(
                            "KB,p2,N={},M={},tile={},kappa={},rho={},kb={:.4}",
                            n, m, chunking_factor, CSP, SSP,
                            (comm_bytes_per_op as f64) / 1024.0,
                        );
                        bytes
                    });
                    let transit_per_iter = Duration::from_nanos(
                        transit_ns(gc_bytes) + transit_ns(KAPPA_BYTES),
                    );

                    b.iter_custom(|iters| {
                        let mut total = std::time::Duration::ZERO;
                        for _ in 0..iters {
                            let (mut generator, mut evaluator) = setup_auth_pair(n, m, chunking_factor);
                            // Input encoding (see P1 bench above for rationale).
                            encode_inputs(&mut generator, &mut evaluator, 0, 0, &mut rand::rng());

                            let l_alpha_pub: Vec<bool> = vec![false; n];
                            let l_beta_pub:  Vec<bool> = vec![false; m];

                            let start = Instant::now();

                            let (cl1, ct1) = generator.garble_first_half_p2();
                            evaluator.evaluate_first_half_p2(cl1, ct1);
                            let (cl2, ct2) = generator.garble_second_half_p2();
                            evaluator.evaluate_second_half_p2(cl2, ct2);
                            let (_d_gb_out, _gb_d_ev_out) = generator.garble_final_p2();
                            let _ev_d_ev_out = evaluator.evaluate_final_p2();

                            let gb_v_alpha_eval: Vec<Block> = generator.alpha_eval.clone();
                            let ev_v_alpha_eval: Vec<Block> = evaluator.alpha_eval.clone();
                            let gb_v_beta_eval:  Vec<Block> = generator.beta_eval.clone();
                            let ev_v_beta_eval:  Vec<Block> = evaluator.beta_eval.clone();

                            let (c_gen_blocks, c_eval_blocks) = assemble_c_alpha_beta_blocks_p2(
                                n, m,
                                &gb_v_alpha_eval,
                                &ev_v_alpha_eval,
                                &gb_v_beta_eval,
                                &ev_v_beta_eval,
                                &l_alpha_pub,
                                &l_beta_pub,
                                &generator,
                                &evaluator,
                            );

                            // Each party hashes its own block vector; honest
                            // parties' digests match per the paper's H({V_w})
                            // semantics (5_online.tex §246, 6_total.tex §222).
                            // Both hashes are computed to capture each party's
                            // CheckZero cost; correctness of mismatch detection
                            // belongs in unit tests, not this timing bench.
                            let h_gb = block_hash_check_zero(&c_gen_blocks);
                            let h_ev = block_hash_check_zero(&c_eval_blocks);
                            let _h_simulated_match = h_gb == h_ev;

                            total += start.elapsed() + transit_per_iter;

                            let _ = black_box(c_gen_blocks);
                            let _ = black_box(c_eval_blocks);
                            let _ = black_box(_h_simulated_match);
                            let _ = black_box(&generator);
                            let _ = black_box(&evaluator);
                        }

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
// separately from the "online_p1" / "online_p2" sync groups so paper-comparison numbers remain
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
        // NOTE: generator and evaluator are intentionally uncorrelated — each is
        // constructed from an independent TensorFpre instance. The evaluator's
        // wire-label decoding produces garbage because the MACs were authenticated
        // under a different delta. This benchmark measures garble-time + network-
        // transfer latency only; correctness of the evaluate output is not tested.
        //
        // Pre-compute garble output byte count outside the timed loop for
        // accurate network-cost accounting (matches existing per-size approach).
        // Byte-counting setup (outside timed loop). Use a correlated pair just
        // for getting valid post-encode_inputs state — only the gen-side garble
        // outputs are read. Bench timing path below remains uncorrelated.
        let mut generator = setup_auth_gen(n, m, chunking_factor);
        let mut throwaway_eval = setup_auth_eval(n, m, chunking_factor);
        // Independent fpre instances → encode_inputs populates each side's
        // masked_*_gen / y_gen from its OWN _eval/_gen (no cross-correlation
        // dependency in the function); the resulting state is enough to
        // run garble_first_half / second_half / final without panicking.
        encode_inputs(&mut generator, &mut throwaway_eval, 0, 0, &mut rand::rng());
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
                        // Uncorrelated setup: timing-only benchmark, not a correctness check.
                        // encode_inputs runs in setup (not timed) per the P1/P2 bench convention.
                        let mut generator = setup_auth_gen(n, m, chunking_factor);
                        let mut evaluator = setup_auth_eval(n, m, chunking_factor);
                        encode_inputs(&mut generator, &mut evaluator, 0, 0, &mut rand::rng());
                        (
                            generator,
                            evaluator,
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
// Only the new paper-faithful online group is wired into criterion_main. The
// `preprocessing_benches` and `network_benches` groups are still defined above
// so they can be reactivated with a one-line change, but they're skipped under
// `cargo bench` while the focus is on regenerating the paper's ms+KB figures.
criterion_main!(online_benches);
