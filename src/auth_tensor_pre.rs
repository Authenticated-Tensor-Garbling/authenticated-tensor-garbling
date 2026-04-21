use crate::{
    auth_tensor_fpre::{TensorFpreGen, TensorFpreEval},
    leaky_tensor_pre::LeakyTriple,
};

/// Compute the bucket size B for Pi_aTensor (Construction 3).
///
/// Formula: B = floor(SSP / log2(n * m)) + 1
/// where SSP = 40 (statistical security parameter).
///
/// Examples:
///   bucket_size_for(16, 16)   = floor(40 / 8)  + 1 = 6
///   bucket_size_for(128, 128) = floor(40 / 14) + 1 = 3
///   bucket_size_for(4, 4)     = floor(40 / 4)  + 1 = 11
pub fn bucket_size_for(n: usize, m: usize) -> usize {
    const SSP: usize = 40;
    let ell = n * m;
    // floor(log2(ell)) for ell >= 2
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}

/// Combine B leaky triples into one authenticated tensor triple (Pi_aTensor, Construction 3).
///
/// PRECONDITION: All triples MUST share the same delta_a and delta_b. This is guaranteed
/// when run_preprocessing uses a single shared IdealBCot instance for all triple generations.
/// Violated if each LeakyTensorPre owns a separate IdealBCot (which gives different deltas,
/// breaking the XOR combination MAC invariant). An assertion enforces this at runtime.
///
/// Algorithm (XOR combination):
///   Keep first triple's alpha/beta/labels.
///   XOR-combine all B triples' correlated and gamma shares.
///   The XOR of B independent AuthBitShares with the same delta preserves the MAC invariant.
///
/// triples: Vec of LeakyTriple, length must equal bucket_size.
/// chunking_factor: passed through to TensorFpreGen/Eval output.
/// shuffle_seed: reserved for future use (Construction 3 calls for shuffling before bucketing).
pub fn combine_leaky_triples(
    triples: Vec<LeakyTriple>,
    bucket_size: usize,
    n: usize,
    m: usize,
    chunking_factor: usize,
    _shuffle_seed: u64,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(triples.len(), bucket_size, "triples.len() must equal bucket_size");
    assert!(bucket_size >= 1);

    // W-04: Assert all triples share the same delta_a and delta_b before combining.
    // This invariant is guaranteed by run_preprocessing using a single shared IdealBCot.
    // If violated, the XOR combination MAC invariant mac = key XOR bit*delta breaks
    // because keys and MACs from different deltas cannot be XOR-combined correctly.
    let delta_a = triples[0].delta_a;
    let delta_b = triples[0].delta_b;
    for (idx, t) in triples.iter().enumerate() {
        assert_eq!(
            t.delta_a.as_block(),
            delta_a.as_block(),
            "triple[{}] delta_a differs from triple[0] delta_a — all triples must share the same IdealBCot",
            idx
        );
        assert_eq!(
            t.delta_b.as_block(),
            delta_b.as_block(),
            "triple[{}] delta_b differs from triple[0] delta_b — all triples must share the same IdealBCot",
            idx
        );
    }

    // XOR-combine correlated and gamma shares across all B triples
    let mut combined_gen_corr = triples[0].gen_correlated_shares.clone();
    let mut combined_eval_corr = triples[0].eval_correlated_shares.clone();
    let mut combined_gen_gamma = triples[0].gen_gamma_shares.clone();
    let mut combined_eval_gamma = triples[0].eval_gamma_shares.clone();

    for t in triples[1..].iter() {
        for k in 0..(n * m) {
            // column-major: index k = j*n+i
            combined_gen_corr[k] = combined_gen_corr[k] + t.gen_correlated_shares[k];
            combined_eval_corr[k] = combined_eval_corr[k] + t.eval_correlated_shares[k];
            combined_gen_gamma[k] = combined_gen_gamma[k] + t.gen_gamma_shares[k];
            combined_eval_gamma[k] = combined_eval_gamma[k] + t.eval_gamma_shares[k];
        }
    }

    // Keep first triple's alpha, beta, and labels
    let t0 = &triples[0];
    (
        TensorFpreGen {
            n,
            m,
            chunking_factor,
            delta_a,
            alpha_labels: t0.gen_alpha_labels.clone(),
            beta_labels: t0.gen_beta_labels.clone(),
            alpha_auth_bit_shares: t0.gen_alpha_shares.clone(),
            beta_auth_bit_shares: t0.gen_beta_shares.clone(),
            correlated_auth_bit_shares: combined_gen_corr,
            gamma_auth_bit_shares: combined_gen_gamma,
        },
        TensorFpreEval {
            n,
            m,
            chunking_factor,
            delta_b,
            alpha_labels: t0.eval_alpha_labels.clone(),
            beta_labels: t0.eval_beta_labels.clone(),
            alpha_auth_bit_shares: t0.eval_alpha_shares.clone(),
            beta_auth_bit_shares: t0.eval_beta_shares.clone(),
            correlated_auth_bit_shares: combined_eval_corr,
            gamma_auth_bit_shares: combined_eval_gamma,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;
    use crate::delta::Delta;
    use crate::leaky_tensor_pre::LeakyTensorPre;
    use crate::sharing::AuthBitShare;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;

    fn make_triples(n: usize, m: usize, count: usize) -> Vec<LeakyTriple> {
        // Single shared IdealBCot — ALL triples get the same delta_a and delta_b.
        let mut bcot = IdealBCot::new(42, 99);
        let mut triples = Vec::new();
        for seed in 0..count {
            let mut ltp = LeakyTensorPre::new(seed as u64, n, m, &mut bcot);
            triples.push(ltp.generate(0b1010, 0b1100));
        }
        triples
    }

    /// Cross-party verify helper (same logic as in leaky_tensor_pre tests).
    /// gen_share.key = A's sender key; eval_share.key = B's sender key.
    /// Gen commits under delta_b; eval commits under delta_a.
    fn verify_cross_party(
        gen_share: &AuthBitShare,
        eval_share: &AuthBitShare,
        delta_a: &Delta,
        delta_b: &Delta,
    ) {
        AuthBitShare {
            key: eval_share.key,
            mac: gen_share.mac,
            value: gen_share.value,
        }
        .verify(delta_b);
        AuthBitShare {
            key: gen_share.key,
            mac: eval_share.mac,
            value: eval_share.value,
        }
        .verify(delta_a);
    }

    #[test]
    fn test_bucket_size_formula() {
        assert_eq!(bucket_size_for(16, 16), 6);
        assert_eq!(bucket_size_for(128, 128), 3);
        assert_eq!(bucket_size_for(4, 4), 11);
    }

    #[test]
    fn test_combine_dimensions() {
        let n = 4;
        let m = 4;
        let b = 2;
        let triples = make_triples(n, m, b);
        let (gen_out, eval_out) = combine_leaky_triples(triples, b, n, m, 1, 42);
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), n);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), n * m);
        assert_eq!(eval_out.gamma_auth_bit_shares.len(), n * m);
    }

    #[test]
    fn test_combine_mac_invariants() {
        let n = 4;
        let m = 4;
        let b = 2;
        let triples = make_triples(n, m, b);
        let delta_a = triples[0].delta_a;
        let delta_b = triples[0].delta_b;
        let (gen_out, eval_out) = combine_leaky_triples(triples, b, n, m, 1, 42);
        // Cross-party verify: gen fields hold B's keys; eval fields hold A's keys.
        // Direct s.verify(&delta) WILL PANIC — use verify_cross_party.
        for i in 0..n {
            verify_cross_party(
                &gen_out.alpha_auth_bit_shares[i],
                &eval_out.alpha_auth_bit_shares[i],
                &delta_a,
                &delta_b,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &gen_out.correlated_auth_bit_shares[k],
                &eval_out.correlated_auth_bit_shares[k],
                &delta_a,
                &delta_b,
            );
        }
    }

    #[test]
    fn test_full_pipeline_no_panic() {
        let n = 4;
        let m = 4;
        let b = bucket_size_for(n, m);
        let triples = make_triples(n, m, b);
        let (fpre_gen, fpre_eval) = combine_leaky_triples(triples, b, n, m, 1, 99);
        let _gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        // No panic = success
    }
}
