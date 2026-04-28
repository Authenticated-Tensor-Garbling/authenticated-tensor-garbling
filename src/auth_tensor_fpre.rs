// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{delta::Delta, sharing::{AuthBit, build_share, AuthBitShare}};
use crate::preprocessing::{TensorFpreGen, TensorFpreEval};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// Insecure ideal Fpre that pre-generates auth bits for input and output vectors of a tensor gate.
pub struct TensorFpre {
    rng: ChaCha12Rng,
    n: usize,
    m: usize,
    chunking_factor: usize,
    delta_a: Delta,
    delta_b: Delta,
    alpha_auth_bits: Vec<AuthBit>,
    beta_auth_bits: Vec<AuthBit>,
    correlated_auth_bits: Vec<AuthBit>,
}

impl TensorFpre {
    /// Creates a new tensor_fpre with random `delta_a`, `delta_b`.
    pub fn new(seed: u64, n: usize, m: usize, chunking_factor: usize) -> Self {
        let mut rng = ChaCha12Rng::seed_from_u64(seed);

        // δ_a has LSB=1 (gen's pointer-bit convention); δ_b has LSB=0
        // so that lsb(δ_a XOR δ_b) = 1 (paper §F invariant; required for
        // bit recovery from Block-form _eval/_gen sharings via LSB).
        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random_b(&mut rng);

        Self {
            rng,
            n,
            m,
            chunking_factor,
            delta_a,
            delta_b,
            alpha_auth_bits:        Vec::with_capacity(n),
            beta_auth_bits:         Vec::with_capacity(m),
            correlated_auth_bits:   Vec::with_capacity(n * m),
        }
    }

    /// Creates a new TensorFpre with given `delta_a`, `delta_b`.
    pub fn new_with_delta(seed: u64, n: usize, m: usize, chunking_factor: usize, delta_a: Delta, delta_b: Delta) -> Self {
        let rng = ChaCha12Rng::seed_from_u64(seed);

        Self {
            rng,
            n,
            m,
            chunking_factor,
            delta_a,
            delta_b,
            alpha_auth_bits: Vec::with_capacity(n),
            beta_auth_bits: Vec::with_capacity(m),
            correlated_auth_bits: Vec::with_capacity(n * m),
        }
    }

    /// Generates an auth bit for a given input bit: x = a ^ b
    pub fn gen_auth_bit(&mut self, x: bool) -> AuthBit {

        let a = self.rng.random_bool(0.5);
        let b = x ^ a;

        let a_share = build_share(&mut self.rng, a, &self.delta_b);
        let b_share = build_share(&mut self.rng, b, &self.delta_a);

        AuthBit {
            gen_share: AuthBitShare {
                key: b_share.key,
                mac: a_share.mac,
                value: a,
            },
            eval_share: AuthBitShare {
                key: a_share.key,
                mac: b_share.mac,
                value: b,
            },
        }
    }

    /// Generates all authenticated permutation bits for the ideal trusted dealer.
    /// This is NOT the real preprocessing protocol — it is the ideal functionality
    /// (trusted dealer) that the online phase consumes directly in tests and benchmarks.
    ///
    /// Produces alpha (length n), beta (length m), and correlated alpha·beta
    /// (length n·m, column-major) auth bits. Input wire labels and masked-input
    /// values are NOT generated here — those belong to the input-encoding phase
    /// that sits between preprocessing and garbling.
    pub fn generate_ideal(&mut self) {
        for _ in 0..self.n {
            let alpha_bit = self.rng.random_bool(0.5);
            let alpha_auth_bit = self.gen_auth_bit(alpha_bit);
            self.alpha_auth_bits.push(alpha_auth_bit);
        }

        for _ in 0..self.m {
            let beta_bit = self.rng.random_bool(0.5);
            let beta_auth_bit = self.gen_auth_bit(beta_bit);
            self.beta_auth_bits.push(beta_auth_bit);
        }

        // column-major indexing
        for j in 0..self.m {
            for i in 0..self.n {
                let alpha = &self.alpha_auth_bits[i];
                let beta = &self.beta_auth_bits[j];
                let alpha_beta = self.gen_auth_bit(alpha.full_bit() && beta.full_bit());
                self.correlated_auth_bits.push(alpha_beta);
            }
        }
    }

    pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {
        // Pre-collect each AuthBit vec into separate gen/eval AuthBitShare vecs
        // (one pass each), then feed them into the shared lowering helper for
        // both the δ_b (`_eval`) and δ_a (`_gen`) sharings.
        let alpha_gen_shares: Vec<AuthBitShare> = self.alpha_auth_bits.iter().map(|b| b.gen_share).collect();
        let alpha_eval_shares: Vec<AuthBitShare> = self.alpha_auth_bits.iter().map(|b| b.eval_share).collect();
        let beta_gen_shares: Vec<AuthBitShare> = self.beta_auth_bits.iter().map(|b| b.gen_share).collect();
        let beta_eval_shares: Vec<AuthBitShare> = self.beta_auth_bits.iter().map(|b| b.eval_share).collect();
        let correlated_gen_shares: Vec<AuthBitShare> = self.correlated_auth_bits.iter().map(|b| b.gen_share).collect();
        let correlated_eval_shares: Vec<AuthBitShare> = self.correlated_auth_bits.iter().map(|b| b.eval_share).collect();

        let delta_a = self.delta_a;
        let delta_b = self.delta_b;

        let (alpha_eval_g, alpha_eval_e) = crate::preprocessing::derive_sharing_blocks(
            &alpha_gen_shares, &alpha_eval_shares, &delta_b);
        let (alpha_gen_e, alpha_gen_g)   = crate::preprocessing::derive_sharing_blocks(
            &alpha_eval_shares, &alpha_gen_shares, &delta_a);

        let (beta_eval_g, beta_eval_e) = crate::preprocessing::derive_sharing_blocks(
            &beta_gen_shares, &beta_eval_shares, &delta_b);
        let (beta_gen_e, beta_gen_g)   = crate::preprocessing::derive_sharing_blocks(
            &beta_eval_shares, &beta_gen_shares, &delta_a);

        let (correlated_eval_g, correlated_eval_e) = crate::preprocessing::derive_sharing_blocks(
            &correlated_gen_shares, &correlated_eval_shares, &delta_b);
        let (correlated_gen_e, correlated_gen_g)   = crate::preprocessing::derive_sharing_blocks(
            &correlated_eval_shares, &correlated_gen_shares, &delta_a);

        // gamma is not generated by `generate_ideal` -- it is produced by
        // `IdealPreprocessingBackend` post-hoc; leave all gamma fields empty.
        (TensorFpreGen {
            n: self.n,
            m: self.m,
            chunking_factor: self.chunking_factor,
            delta_a,
            alpha_auth_bit_shares: alpha_gen_shares,
            alpha_eval: alpha_eval_g,
            alpha_gen:  alpha_gen_g,
            beta_auth_bit_shares: beta_gen_shares,
            beta_eval: beta_eval_g,
            beta_gen:  beta_gen_g,
            correlated_auth_bit_shares: correlated_gen_shares,
            correlated_eval: correlated_eval_g,
            correlated_gen:  correlated_gen_g,
            gamma_auth_bit_shares: vec![],
            gamma_eval: vec![],
            gamma_gen: vec![],
        }, TensorFpreEval {
            n: self.n,
            m: self.m,
            chunking_factor: self.chunking_factor,
            delta_b,
            alpha_auth_bit_shares: alpha_eval_shares,
            alpha_eval: alpha_eval_e,
            alpha_gen:  alpha_gen_e,
            beta_auth_bit_shares: beta_eval_shares,
            beta_eval: beta_eval_e,
            beta_gen:  beta_gen_e,
            correlated_auth_bit_shares: correlated_eval_shares,
            correlated_eval: correlated_eval_e,
            correlated_gen:  correlated_gen_e,
            gamma_auth_bit_shares: vec![],
            gamma_eval: vec![],
            gamma_gen: vec![],
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_fpre_auth_bits() {
        let n = 3;
        let m = 4;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_ideal();

        // confirm dimensions
        assert_eq!(fpre.alpha_auth_bits.len(), n);
        assert_eq!(fpre.beta_auth_bits.len(), m);
        assert_eq!(fpre.correlated_auth_bits.len(), n * m);

        // verify auth bits
        for bit in &fpre.alpha_auth_bits {
            bit.verify(&fpre.delta_a, &fpre.delta_b);
        }

        for bit in &fpre.beta_auth_bits {
            bit.verify(&fpre.delta_a, &fpre.delta_b);
        }

        for bit in &fpre.correlated_auth_bits {
            bit.verify(&fpre.delta_a, &fpre.delta_b);
        }

        // verify correlated auth bits
        for j in 0..m {
            let base = j * n;
            for i in 0..n {
                let alpha = &fpre.alpha_auth_bits[i];
                let beta = &fpre.beta_auth_bits[j];
                let alpha_beta = &fpre.correlated_auth_bits[base + i];
                assert!(alpha_beta.full_bit() == (alpha.full_bit() && beta.full_bit()), "alpha_beta must equal alpha.full_bit() && beta.full_bit()");
            }
        }
    }

}
