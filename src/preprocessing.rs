//! Real-protocol preprocessing pipeline.
//!
//! This module holds the output structs (`TensorFpreGen`, `TensorFpreEval`) that the
//! real two-party preprocessing protocol produces, together with the `run_preprocessing`
//! entry point. The ideal trusted-dealer functionality stays in `auth_tensor_fpre`.

use crate::{block::Block, delta::Delta, sharing::{AuthBitShare, build_share}};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
use crate::auth_tensor_fpre::TensorFpre;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

pub struct TensorFpreGen {
    /// Tensor row dimension (number of alpha / x-input bits).
    pub n: usize,
    /// Tensor column dimension (number of beta / y-input bits).
    pub m: usize,
    /// GGM tree chunking factor; purely a performance knob (1..=8 used in benches).
    pub chunking_factor: usize,
    /// Garbler's (Party A) global correlation key. `as_block().lsb() == 1` invariant.
    pub delta_a: Delta,
    /// Garbler's `AuthBitShare` for each alpha_i (i in 0..n). MAC committed under delta_b.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each beta_j (j in 0..m). MAC committed under delta_b.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each correlated bit alpha_i AND beta_j; length n*m,
    /// column-major index j*n + i. MAC committed under delta_b.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's precomputed D_ev label for each `l_alpha` bit; length n.
    /// Each entry is `gen_share.mac` of the corresponding D_gb auth bit, i.e. `K_a ⊕ a·D_ev`.
    /// XORing with the evaluator's matching entry gives `l_alpha_i · D_ev`.
    /// Derived inside `TensorFpre::into_gen_eval`; left empty by `UncompressedPreprocessingBackend`.
    pub alpha_d_ev_shares: Vec<Block>,
    /// Garbler's precomputed D_ev label for each `l_beta` bit; length m.
    /// Same derivation as `alpha_d_ev_shares` but for beta.
    pub beta_d_ev_shares: Vec<Block>,
    /// Garbler's precomputed D_ev label for each `l_gamma*` correlated bit; length n*m,
    /// column-major index `j*n + i`. Same derivation as `alpha_d_ev_shares`.
    pub correlated_d_ev_shares: Vec<Block>,
    /// Garbler's `AuthBitShare` for each gate-output mask `l_gamma`; length n*m, column-major.
    /// MAC committed under delta_b. (Phase 9 D-05.)
    /// Distinct from `correlated_d_ev_shares` (which encodes l_gamma* = l_alpha · l_beta).
    pub gamma_d_ev_shares: Vec<AuthBitShare>,
}

pub struct TensorFpreEval {
    /// Tensor row dimension (matches TensorFpreGen.n).
    pub n: usize,
    /// Tensor column dimension (matches TensorFpreGen.m).
    pub m: usize,
    /// GGM tree chunking factor.
    pub chunking_factor: usize,
    /// Evaluator's (Party B) global correlation key. `as_block().lsb() == 0` invariant
    /// (required so that `lsb(delta_a XOR delta_b) == 1` per Pi_LeakyTensor §F).
    pub delta_b: Delta,
    /// Evaluator's `AuthBitShare` for each alpha_i. MAC committed under delta_a.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each beta_j. MAC committed under delta_a.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each correlated bit (column-major, length n*m,
    /// index j*n + i). MAC committed under delta_a.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's precomputed D_ev label for each `l_alpha` bit; length n.
    /// Each entry is `eval_share.key ⊕ eval_share.value·D_ev` of the corresponding D_gb auth bit,
    /// i.e. `K_a ⊕ b·D_ev`. XORing with the garbler's matching entry gives `l_alpha_i · D_ev`.
    /// Derived inside `TensorFpre::into_gen_eval`; left empty by `UncompressedPreprocessingBackend`.
    pub alpha_d_ev_shares: Vec<Block>,
    /// Evaluator's precomputed D_ev label for each `l_beta` bit; length m.
    pub beta_d_ev_shares: Vec<Block>,
    /// Evaluator's precomputed D_ev label for each `l_gamma*` correlated bit; length n*m,
    /// column-major index `j*n + i`.
    pub correlated_d_ev_shares: Vec<Block>,
    /// Evaluator's `AuthBitShare` for each `l_gamma` mask; length n*m, column-major.
    /// MAC committed under delta_a. (Phase 9 D-05.)
    pub gamma_d_ev_shares: Vec<AuthBitShare>,
}

/// Common interface for all preprocessing backends.
///
/// All implementations are zero-field (unit) structs — no state, just behavior.
/// Using `&self` even though no data is read: makes the trait object-safe
/// (usable as `dyn TensorPreprocessing`) consistent with the codebase's `&self` / `&mut self`
/// convention. See CONTEXT.md D-01.
pub trait TensorPreprocessing {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval);
}

/// Backend that wraps the real two-party uncompressed preprocessing protocol (Pi_aTensor',
/// Construction 4). Callers should use `UncompressedPreprocessingBackend.run(n, m, 1, cf)`
/// instead of calling `run_preprocessing` directly. See CONTEXT.md D-02.
///
/// Note: count > 1 retains the existing `assert_eq!(count, 1)` panic from `run_preprocessing`
/// until a batch variant is implemented. This matches existing behavior.
pub struct UncompressedPreprocessingBackend;

impl TensorPreprocessing for UncompressedPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        run_preprocessing(n, m, count, chunking_factor)
    }
}

/// Backend that uses an ideal trusted-dealer oracle (in-process, not cryptographically secure).
///
/// Fixed seed 0 used internally — matches the `IdealBCot::new(0, 1)` precedent (see
/// src/bcot.rs). For tests and benchmarks only. See CONTEXT.md D-03, D-07, D-08.
///
/// `gamma_d_ev_shares` is populated with `n*m` independent random IT-MAC authenticated
/// bits for l_gamma (the gate output mask). `gen_auth_bit()` calls MUST precede
/// `into_gen_eval()` because `into_gen_eval(self)` consumes `fpre` by value.
/// See RESEARCH.md Pitfall 2 and Pattern 3.
pub struct IdealPreprocessingBackend;

impl TensorPreprocessing for IdealPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        assert_eq!(
            count, 1,
            "IdealPreprocessingBackend::run: count > 1 is not yet supported; \
             the ideal backend returns exactly one (TensorFpreGen, TensorFpreEval) pair. \
             Use a loop calling run(n, m, 1, cf) for batch use."
        );
        let _ = count;

        let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
        fpre.generate_ideal();

        // CRITICAL ORDERING: into_gen_eval(self) consumes fpre by value.
        // All gen_auth_bit() calls must happen BEFORE into_gen_eval() is called.
        //
        // Phase 9 D-06: generate all four D_ev field pairs using fpre.gen_auth_bit().
        // Use distinct ChaCha12Rng seeds (42, 43, 44, 45) so the four fields are
        // independently random — same pattern as the existing gamma_d_ev_shares
        // generation (seed 42).
        // alpha/beta/correlated D_ev labels are derived from the D_gb auth bits inside
        // into_gen_eval() — no gen_auth_bit calls needed here.

        // gamma generation: D_ev-authenticated l_gamma shares (column-major n*m).
        let mut rng_gamma = ChaCha12Rng::seed_from_u64(42);
        let mut gamma_d_ev_bits: Vec<crate::sharing::AuthBit> = Vec::with_capacity(n * m);
        for _ in 0..(n * m) {
            let l_gamma: bool = rng_gamma.random_bool(0.5);
            gamma_d_ev_bits.push(fpre.gen_auth_bit(l_gamma));
        }

        // Now consume fpre — gen_auth_bit() can no longer be called after this line.
        // Note: use `gen_out` / `eval_out` bindings because `gen` is a reserved keyword
        // in Rust 2024 edition.
        let (mut gen_out, mut eval_out) = fpre.into_gen_eval();

        // Distribute gen_share / eval_share for gamma only.
        // alpha/beta/correlated D_ev labels were populated by into_gen_eval().
        gen_out.gamma_d_ev_shares  = gamma_d_ev_bits.iter().map(|b| b.gen_share).collect();
        eval_out.gamma_d_ev_shares = gamma_d_ev_bits.iter().map(|b| b.eval_share).collect();

        (gen_out, eval_out)
    }
}

/// Run the real two-party uncompressed preprocessing protocol (Pi_aTensor', Construction 4).
///
/// Generates `count` authenticated tensor triples using:
///   1. bucket_size_for(n, count) leaky triples per output triple (from Pi_LeakyTensor)
///   2. Pi_aTensor bucketing combiner to amplify security
///
/// CRITICAL: ONE shared IdealBCot is created before the generation loop. All
/// LeakyTensorPre instances borrow &mut bcot and therefore all triples share the
/// same delta_a and delta_b. This is required for the XOR combination in
/// combine_leaky_triples to preserve the MAC invariant mac = key XOR bit*delta.
/// Creating a separate IdealBCot per triple (each with different deltas) would
/// silently produce invalid combined triples.
///
/// Returns one (TensorFpreGen, TensorFpreEval) pair suitable for feeding into
/// AuthTensorGen::new_from_fpre_gen and AuthTensorEval::new_from_fpre_eval.
///
/// For Phase 1 benchmarking, count = 1. For future batch use, count > 1.
///
/// Preprocessing is fully input-independent per paper Construction 2. Triples are
/// sampled from LeakyTensorPre's internal ChaCha12Rng; no input values flow in here.
///
/// # Panics
///
/// Panics if `count != 1`. Batch output (count > 1) requires a Vec-returning
/// variant that is not yet implemented.
pub fn run_preprocessing(
    n: usize,
    m: usize,
    count: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(
        count, 1,
        "Phase 1: only count=1 is supported; batch output requires a Vec-returning variant. \
        Note: total_leaky = bucket_size * count generates enough leaky triples for 'count' \
        output authenticated triples, but combine_leaky_triples below only consumes \
        bucket_size of them and returns a single pair — remove this assert only after \
        adding a loop that calls combine_leaky_triples once per output triple."
    );

    let bucket_size = bucket_size_for(n, count);
    let total_leaky = bucket_size * count;

    // ONE shared IdealBCot for all triples — ensures all share the same delta_a and delta_b.
    // Seed choice: 0 for delta_a, 1 for delta_b. The internal rng seed is 0^1=1 (trivial),
    // but key generation inside each LeakyTensorPre uses its own per-instance rng.
    let mut bcot = IdealBCot::new(0, 1);

    let mut triples = Vec::with_capacity(total_leaky);
    for t in 0..total_leaky {
        // Each LeakyTensorPre borrows &mut bcot — shares delta_a and delta_b.
        // Per-instance seed `t+2` ensures independent key randomness across triples.
        let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, &mut bcot);
        triples.push(ltp.generate());
    }

    let (mut gen_out, mut eval_out) =
        combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42);

    // BUG-02 / Phase 1.2(c): the post-bucketing input-label populator was
    // removed here. Input wire labels are no longer a preprocessing artifact —
    // they are generated at garble time by AuthTensorGen::prepare_input_labels.
    // The previous block faked labels using x = y = 0 with the input-dependent
    // formula `(x XOR alpha) · delta_a`, which encoded the mask itself rather
    // than any real input — a "works because both sides use the same dummy
    // convention" hack flagged by LABELS-BUG-CONTEXT.md.

    // Post-bucketing D_ev population. Mirrors the formulas in
    // `TensorFpre::into_gen_eval` (auth_tensor_fpre.rs:180-226). Inputs (the
    // auth-bit shares and `delta_b`) are already finalized at this point.
    let delta_b = eval_out.delta_b;

    let (g, e) = derive_d_ev_blocks(
        &gen_out.alpha_auth_bit_shares,
        &eval_out.alpha_auth_bit_shares,
        &delta_b,
    );
    gen_out.alpha_d_ev_shares = g;
    eval_out.alpha_d_ev_shares = e;

    let (g, e) = derive_d_ev_blocks(
        &gen_out.beta_auth_bit_shares,
        &eval_out.beta_auth_bit_shares,
        &delta_b,
    );
    gen_out.beta_d_ev_shares = g;
    eval_out.beta_d_ev_shares = e;

    let (g, e) = derive_d_ev_blocks(
        &gen_out.correlated_auth_bit_shares,
        &eval_out.correlated_auth_bit_shares,
        &delta_b,
    );
    gen_out.correlated_d_ev_shares = g;
    eval_out.correlated_d_ev_shares = e;

    // Gamma: fresh n*m IT-MAC AuthBit pairs sampled from a dedicated ChaCha12Rng.
    // Mirrors `TensorFpre::gen_auth_bit` (auth_tensor_fpre.rs:66-86) inline; using
    // a distinct seed (43) from the bucketing permutation seed (42) above.
    let mut rng_gamma = ChaCha12Rng::seed_from_u64(43);
    let mut gen_gamma = Vec::with_capacity(n * m);
    let mut eval_gamma = Vec::with_capacity(n * m);
    for _ in 0..(n * m) {
        let l_gamma: bool = rng_gamma.random_bool(0.5);
        let a: bool = rng_gamma.random_bool(0.5);
        let b: bool = l_gamma ^ a;
        let a_share = build_share(&mut rng_gamma, a, &delta_b);
        let b_share = build_share(&mut rng_gamma, b, &gen_out.delta_a);
        gen_gamma.push(AuthBitShare {
            key: b_share.key,
            mac: a_share.mac,
            value: a,
        });
        eval_gamma.push(AuthBitShare {
            key: a_share.key,
            mac: b_share.mac,
            value: b,
        });
    }
    gen_out.gamma_d_ev_shares = gen_gamma;
    eval_out.gamma_d_ev_shares = eval_gamma;

    (gen_out, eval_out)
}

/// Derive D_ev block pairs from paired auth-bit shares.
///
/// Mirrors `TensorFpre::into_gen_eval` (auth_tensor_fpre.rs:180-226):
///   gen_d_ev[k]  = gen.mac
///   eval_d_ev[k] = eval.key XOR (eval.bit() ? delta_b : 0)
///
/// Together they satisfy `gen ^ eval = full_bit · delta_b`, where
/// `full_bit = gen.value ^ eval.value` (the IT-MAC identity in `gen_auth_bit`).
fn derive_d_ev_blocks(
    gen_shares: &[AuthBitShare],
    eval_shares: &[AuthBitShare],
    delta_b: &Delta,
) -> (Vec<Block>, Vec<Block>) {
    assert_eq!(gen_shares.len(), eval_shares.len());
    let gen_blocks: Vec<Block> = gen_shares
        .iter()
        .map(|s| *s.mac.as_block())
        .collect();
    let eval_blocks: Vec<Block> = eval_shares
        .iter()
        .map(|s| {
            let k = *s.key.as_block();
            if s.bit() { k ^ *delta_b.as_block() } else { k }
        })
        .collect();
    (gen_blocks, eval_blocks)
}

#[cfg(test)]
mod tests {
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::auth_tensor_pre::verify_cross_party;
    use crate::block::Block;
    use super::{TensorPreprocessing, UncompressedPreprocessingBackend, IdealPreprocessingBackend};

    #[test]
    fn test_run_preprocessing_dimensions() {
        let (gen_out, eval_out) = super::run_preprocessing(4, 4, 1, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), 16);
    }

    #[test]
    fn test_run_preprocessing_delta_lsb() {
        let (gen_out, _eval_out) = super::run_preprocessing(4, 4, 1, 1);
        assert!(gen_out.delta_a.as_block().lsb(), "delta_a LSB must be 1");
    }

    #[test]
    fn test_run_preprocessing_feeds_online_phase() {
        let (fpre_gen, fpre_eval) = super::run_preprocessing(4, 4, 1, 1);
        let _gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        // No panic = success
    }

    // PRE-01: trait is object-safe — both backends work through a dyn reference.
    // Note: bindings named `gen_out` / `eval_out` because `gen` is a reserved keyword
    // in Rust 2024 edition (same reason as IdealPreprocessingBackend::run body).
    #[test]
    fn test_trait_dispatch_ideal() {
        let backend: &dyn TensorPreprocessing = &IdealPreprocessingBackend;
        let (gen_out, eval_out) = backend.run(4, 4, 1, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
    }

    #[test]
    fn test_trait_dispatch_uncompressed() {
        let backend: &dyn TensorPreprocessing = &UncompressedPreprocessingBackend;
        let (gen_out, eval_out) = backend.run(4, 4, 1, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
    }

    // PRE-03: UncompressedPreprocessingBackend delegates to run_preprocessing exactly
    #[test]
    fn test_uncompressed_backend_delegates_to_run_preprocessing() {
        let (gen_out, _eval_out) = UncompressedPreprocessingBackend.run(4, 4, 1, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16,
            "correlated_auth_bit_shares must have n*m=16 entries");
    }

    // Uncompressed backend now populates all four D_ev fields post-bucketing.
    // alpha/beta/correlated are derived from the auth-bit shares; gamma is
    // freshly sampled from a dedicated ChaCha12Rng (seed 43) inside
    // `run_preprocessing`, mirroring the ideal backend.
    #[test]
    fn test_uncompressed_backend_gamma_field_is_populated() {
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(4, 4, 1, 1);
        assert_eq!(gen_out.gamma_d_ev_shares.len(), 4 * 4,
            "gen.gamma_d_ev_shares must have length n*m=16");
        assert_eq!(eval_out.gamma_d_ev_shares.len(), 4 * 4,
            "eval.gamma_d_ev_shares must have length n*m=16");
    }

    // Uncompressed backend: alpha/beta/correlated D_ev shares have correct lengths.
    #[test]
    fn test_uncompressed_backend_d_ev_shares_lengths() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1, 1);
        assert_eq!(gen_out.alpha_d_ev_shares.len(),       n);
        assert_eq!(eval_out.alpha_d_ev_shares.len(),      n);
        assert_eq!(gen_out.beta_d_ev_shares.len(),        m);
        assert_eq!(eval_out.beta_d_ev_shares.len(),       m);
        assert_eq!(gen_out.correlated_d_ev_shares.len(),  n * m);
        assert_eq!(eval_out.correlated_d_ev_shares.len(), n * m);
    }

    // Uncompressed backend: alpha/beta/correlated D_ev label pairs XOR to
    // `bit · delta_b`, where bit is the underlying auth bit's full value.
    // Mirrors `test_ideal_backend_d_ev_shares_bit_correlation` for the
    // uncompressed path.
    #[test]
    fn test_uncompressed_backend_d_ev_shares_bit_correlation() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1, 1);
        let delta_b = eval_out.delta_b;

        for k in 0..n {
            let bit = gen_out.alpha_auth_bit_shares[k].bit()
                    ^ eval_out.alpha_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.alpha_d_ev_shares[k] ^ eval_out.alpha_d_ev_shares[k], expected,
                "alpha D_ev XOR mismatch at {k}");
        }
        for k in 0..m {
            let bit = gen_out.beta_auth_bit_shares[k].bit()
                    ^ eval_out.beta_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.beta_d_ev_shares[k] ^ eval_out.beta_d_ev_shares[k], expected,
                "beta D_ev XOR mismatch at {k}");
        }
        for k in 0..(n * m) {
            let bit = gen_out.correlated_auth_bit_shares[k].bit()
                    ^ eval_out.correlated_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.correlated_d_ev_shares[k] ^ eval_out.correlated_d_ev_shares[k], expected,
                "correlated D_ev XOR mismatch at {k}");
        }
    }

    // Uncompressed backend: every gamma share satisfies the IT-MAC invariant
    // under both deltas. Mirrors `test_ideal_backend_gamma_d_ev_shares_mac_invariant`.
    #[test]
    fn test_uncompressed_backend_gamma_d_ev_shares_mac_invariant() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1, 1);
        for k in 0..(n * m) {
            verify_cross_party(
                &gen_out.gamma_d_ev_shares[k],
                &eval_out.gamma_d_ev_shares[k],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
    }

    // PRE-02: IdealPreprocessingBackend returns correctly dimensioned output.
    // Note: bindings named `gen_out` / `eval_out` because `gen` is a reserved keyword
    // in Rust 2024 edition.
    #[test]
    fn test_ideal_backend_dimensions() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), 4,  "alpha shares: length n=4");
        assert_eq!(gen_out.beta_auth_bit_shares.len(),  4,  "beta shares: length m=4");
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16, "correlated shares: length n*m=16");
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), 16);
    }

    // PRE-04: gamma_d_ev_shares length is n*m on both sides
    #[test]
    fn test_ideal_backend_gamma_d_ev_shares_length() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1, 1);
        assert_eq!(gen_out.gamma_d_ev_shares.len(),  4 * 4,
            "gen.gamma_d_ev_shares must have n*m=16 entries");
        assert_eq!(eval_out.gamma_d_ev_shares.len(), 4 * 4,
            "eval.gamma_d_ev_shares must have n*m=16 entries");
    }

    // PRE-04 + D-09: IT-MAC invariant (mac = key XOR bit * delta) holds for all gamma shares.
    // WARNING: Do NOT call share.verify(delta) directly — it panics on correctly-formed
    // cross-party shares. Always use verify_cross_party (see RESEARCH.md Pitfall 3).
    #[test]
    fn test_ideal_backend_gamma_d_ev_shares_mac_invariant() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1, 1);
        for k in 0..(4 * 4) {
            verify_cross_party(
                &gen_out.gamma_d_ev_shares[k],
                &eval_out.gamma_d_ev_shares[k],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
        // If no panic: all 16 gamma shares satisfy the IT-MAC invariant.
    }

    // P2-01 (Phase 9 D-04, D-06): All three new D_ev field pairs are populated
    // and have correct lengths n, m, n*m respectively.
    #[test]
    fn test_ideal_backend_d_ev_shares_lengths() {
        let n = 4;
        let m = 4;
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(n, m, 1, 1);
        assert_eq!(gen_out.alpha_d_ev_shares.len(),        n,
            "gen.alpha_d_ev_shares must have length n");
        assert_eq!(eval_out.alpha_d_ev_shares.len(),       n,
            "eval.alpha_d_ev_shares must have length n");
        assert_eq!(gen_out.beta_d_ev_shares.len(),         m,
            "gen.beta_d_ev_shares must have length m");
        assert_eq!(eval_out.beta_d_ev_shares.len(),        m,
            "eval.beta_d_ev_shares must have length m");
        assert_eq!(gen_out.correlated_d_ev_shares.len(),   n * m,
            "gen.correlated_d_ev_shares must have length n*m");
        assert_eq!(eval_out.correlated_d_ev_shares.len(),  n * m,
            "eval.correlated_d_ev_shares must have length n*m");
    }

    // P2-01: D_ev label pairs XOR to `bit * delta_b` — the core correctness invariant.
    // gen_label XOR eval_label == lambda_i * D_ev for each alpha/beta/correlated bit.
    #[test]
    fn test_ideal_backend_d_ev_shares_bit_correlation() {
        use crate::block::Block;
        let n = 4;
        let m = 4;
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let delta_b = eval_out.delta_b;

        for k in 0..n {
            let bit = gen_out.alpha_auth_bit_shares[k].bit() ^ eval_out.alpha_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.alpha_d_ev_shares[k] ^ eval_out.alpha_d_ev_shares[k], expected,
                "alpha D_ev XOR mismatch at {k}");
        }
        for k in 0..m {
            let bit = gen_out.beta_auth_bit_shares[k].bit() ^ eval_out.beta_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.beta_d_ev_shares[k] ^ eval_out.beta_d_ev_shares[k], expected,
                "beta D_ev XOR mismatch at {k}");
        }
        for k in 0..(n * m) {
            let bit = gen_out.correlated_auth_bit_shares[k].bit() ^ eval_out.correlated_auth_bit_shares[k].bit();
            let expected = if bit { *delta_b.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.correlated_d_ev_shares[k] ^ eval_out.correlated_d_ev_shares[k], expected,
                "correlated D_ev XOR mismatch at {k}");
        }
    }

    // PRE-04 + D-05: gamma_d_ev_shares (l_gamma) is a different random sample
    // from correlated_auth_bit_shares (l_gamma*). Basic distinctness: not all-zero bits.
    #[test]
    fn test_ideal_backend_gamma_distinct_from_correlated() {
        let (gen_out, _eval_out) = IdealPreprocessingBackend.run(4, 4, 1, 1);
        // gamma bits: gen and eval shares XOR to the actual l_gamma bit (gen.value XOR eval.value)
        // correlated bits: same XOR gives l_gamma* bit
        // They are independent random samples; expect at least one to differ from false.
        let any_gamma_set = gen_out.gamma_d_ev_shares.iter()
            .any(|s| s.value);
        let any_correlated_set = gen_out.correlated_auth_bit_shares.iter()
            .any(|s| s.value);
        // With overwhelming probability at least one of each is set (random bits with n*m=16).
        // This test is probabilistic but uses a fixed seed so it is deterministic.
        // If this fails, the RNG seeding is broken (all bits are the same constant).
        let _ = any_gamma_set;    // not asserted — just checking no panic in access
        let _ = any_correlated_set;
        // Verify that gamma and correlated shares are not byte-for-byte identical
        // (independent random samples from different RNG seeds must differ).
        let gamma_bits: Vec<bool> = gen_out.gamma_d_ev_shares.iter()
            .map(|s| s.value)
            .collect();
        let correlated_bits: Vec<bool> = gen_out.correlated_auth_bit_shares.iter()
            .map(|s| s.value)
            .collect();
        assert_ne!(
            gamma_bits,
            correlated_bits,
            "gamma and correlated auth bit shares must be independently sampled (different RNG seeds)"
        );
    }
}
