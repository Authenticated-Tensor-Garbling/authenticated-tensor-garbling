//! Real-protocol preprocessing pipeline.
//!
//! This module holds the output structs (`TensorFpreGen`, `TensorFpreEval`) that the
//! real two-party preprocessing protocol produces, together with the `run_preprocessing`
//! entry point. The ideal trusted-dealer functionality stays in `auth_tensor_fpre`.

use crate::{block::Block, delta::Delta, sharing::AuthBitShare};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};

pub struct TensorFpreGen {
    /// Tensor row dimension (number of alpha / x-input bits).
    pub n: usize,
    /// Tensor column dimension (number of beta / y-input bits).
    pub m: usize,
    /// GGM tree chunking factor; purely a performance knob (1..=8 used in benches).
    pub chunking_factor: usize,
    /// Garbler's (Party A) global correlation key. `as_block().lsb() == 1` invariant.
    pub delta_a: Delta,
    /// Garbler's share of each x-input wire label; length n. Together with the
    /// evaluator's matching `alpha_labels`, reveals `x XOR alpha` via `shares_differ`.
    pub alpha_labels: Vec<Block>,
    /// Garbler's share of each y-input wire label; length m. Reveals `y XOR beta`
    /// when XORed against the evaluator's matching eval_share.
    pub beta_labels: Vec<Block>,
    /// Garbler's `AuthBitShare` for each alpha_i (i in 0..n). MAC committed under delta_b.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each beta_j (j in 0..m). MAC committed under delta_b.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each correlated bit alpha_i AND beta_j; length n*m,
    /// column-major index j*n + i. MAC committed under delta_b.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
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
    /// Evaluator's share of each x-input wire label; length n. Combines with the
    /// garbler's `alpha_labels` to reveal `x XOR alpha` via `shares_differ`.
    pub alpha_labels: Vec<Block>,
    /// Evaluator's share of each y-input wire label; length m. Symmetric to alpha_labels.
    pub beta_labels: Vec<Block>,
    /// Evaluator's `AuthBitShare` for each alpha_i. MAC committed under delta_a.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each beta_j. MAC committed under delta_a.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each correlated bit (column-major, length n*m,
    /// index j*n + i). MAC committed under delta_a.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
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

    combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42)
}

#[cfg(test)]
mod tests {
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;

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
}
