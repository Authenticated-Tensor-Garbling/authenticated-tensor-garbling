// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{block::Block, delta::Delta, fpre::{AuthBit, build_share, AuthBitShare}};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// Insecure ideal Fpre that pre-generates auth bits for input and output vectors of a tensor gate.
pub struct TensorFpre {
    rng: ChaCha12Rng,
    n: usize,
    m: usize,
    delta_a: Delta,
    delta_b: Delta,
    alpha_labels: Vec<InputSharing>,
    beta_labels: Vec<InputSharing>,
    alpha_auth_bits: Vec<AuthBit>,
    beta_auth_bits: Vec<AuthBit>,
    correlated_auth_bits: Vec<AuthBit>,
    gamma_auth_bits: Vec<AuthBit>,
}

pub struct TensorFpreGen {
    pub n: usize,
    pub m: usize,
    pub delta_a: Delta,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}

pub struct TensorFpreEval {
    pub n: usize,
    pub m: usize,
    pub delta_b: Delta,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}

struct InputSharing {
    pub gen_share: Block,
    pub eval_share: Block,
}

impl TensorFpre {
    /// Creates a new tensor_fpre with random `delta_a`, `delta_b`.
    pub fn new(seed: u64, n: usize, m: usize) -> Self {
        let mut rng = ChaCha12Rng::seed_from_u64(seed);

        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random(&mut rng);

        Self {
            rng,
            n,
            m,
            delta_a,
            delta_b,
            alpha_labels: Vec::with_capacity(n),
            beta_labels: Vec::with_capacity(m),
            alpha_auth_bits:        Vec::with_capacity(n),
            beta_auth_bits:         Vec::with_capacity(m),
            correlated_auth_bits:   Vec::with_capacity(n * m),
            gamma_auth_bits:        Vec::with_capacity(n * m),
        }
    }

    /// Creates a new TensorFpre with given `delta_a`, `delta_b`.
    pub fn new_with_delta(seed: u64, n: usize, m: usize, delta_a: Delta, delta_b: Delta) -> Self {
        let rng = ChaCha12Rng::seed_from_u64(seed);

        Self {
            rng,
            n,
            m,
            delta_a,
            delta_b,
            alpha_labels: Vec::with_capacity(n),
            beta_labels: Vec::with_capacity(m),
            alpha_auth_bits: Vec::with_capacity(n),
            beta_auth_bits: Vec::with_capacity(m),
            correlated_auth_bits: Vec::with_capacity(n * m),
            gamma_auth_bits: Vec::with_capacity(n * m),
        }
    }

    pub fn gen_input_sharings(&mut self, alpha: usize, beta: usize) {
        assert!(alpha < 1<<self.n, "alpha must be < 2^n");
        assert!(beta < 1<<self.m, "beta must be < 2^m");

        for i in 0..self.n {
            let mut gen_label = Block::random(&mut self.rng);
            gen_label.set_lsb(false);

            let eval_label: Block;

            let bit = 1<<i & alpha;
            if bit != 0 {
                eval_label = gen_label ^ self.delta_a.as_block();
            } else {
                eval_label = gen_label.clone();
            }

            self.alpha_labels.push(InputSharing { gen_share: gen_label, eval_share: eval_label });
        }

        for i in 0..self.m {
            let mut gen_label = Block::random(&mut self.rng);
            gen_label.set_lsb(false);

            let eval_label: Block;

            let bit = 1<<i & beta;
            if bit != 0 {
                eval_label = gen_label ^ self.delta_a.as_block();
            } else {
                eval_label = gen_label.clone();
            }

            self.beta_labels.push(InputSharing { gen_share: gen_label, eval_share: eval_label });
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

    /// Generates all auth bits for the input and output vectors of a tensor gate.
    /// alpha, beta, ab* = alpha * beta
    /// gamma
    pub fn generate(&mut self) {

        for _ in 0..self.n {
            let x = self.rng.random_bool(0.5);
            let alpha = self.gen_auth_bit(x);
            self.alpha_auth_bits.push(alpha);
        }

        for _ in 0..self.m {
            let x = self.rng.random_bool(0.5);
            let beta =self.gen_auth_bit(x);
            self.beta_auth_bits.push(beta);
        }

        // gamma wires
        // column-major indexing
        for j in 0..self.m {
            for i in 0..self.n {
                let g = self.rng.random_bool(0.5);
                let gamma = self.gen_auth_bit(g);
                self.gamma_auth_bits.push(gamma);

                let alpha = &self.alpha_auth_bits[i];
                let beta = &self.beta_auth_bits[j];
                let alpha_beta = self.gen_auth_bit(alpha.full_bit() && beta.full_bit());
                println!("pushing to correlated_auth_bits: {}",j * self.n + i);
                self.correlated_auth_bits.push(alpha_beta);
            }
        }
    }

    pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {

        (TensorFpreGen {
            n: self.n,
            m: self.m,
            delta_a: self.delta_a,
            alpha_labels: self.alpha_labels.iter().map(|share| share.gen_share).collect(),
            beta_labels: self.beta_labels.iter().map(|share| share.gen_share).collect(),
            alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|bit| bit.gen_share).collect(),
        }, TensorFpreEval {
            n: self.n,
            m: self.m,
            delta_b: self.delta_b,
            alpha_labels: self.alpha_labels.iter().map(|share| share.eval_share).collect(),
            beta_labels: self.beta_labels.iter().map(|share| share.eval_share).collect(),
            alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|bit| bit.eval_share).collect(),
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

        let mut fpre = TensorFpre::new(0, n, m);
        fpre.generate();

        // confirm dimensions
        assert_eq!(fpre.alpha_auth_bits.len(), n);
        assert_eq!(fpre.beta_auth_bits.len(), m);
        assert_eq!(fpre.correlated_auth_bits.len(), n * m);
        assert_eq!(fpre.gamma_auth_bits.len(), n * m);

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

        for bit in &fpre.gamma_auth_bits {
            bit.verify(&fpre.delta_a, &fpre.delta_b);
        }

        // verify correlated auth bits
        for j in 0..m {
            let base = j * n;
            for i in 0..n {
                let alpha = &fpre.alpha_auth_bits[i];
                let beta = &fpre.beta_auth_bits[j];
                let alpha_beta = &fpre.correlated_auth_bits[base + i];
                println!("verifying correlated_auth_bits: {}",j * n + i);
                assert!(alpha_beta.full_bit() == (alpha.full_bit() && beta.full_bit()), "alpha_beta must equal alpha.full_bit() && beta.full_bit()");
            }
        }
    }
    
    #[test]
    fn test_tensor_fpre_input_sharings() {
        let n = 3;
        let m = 4;

        let mut fpre = TensorFpre::new(0, n, m);
        fpre.gen_input_sharings(0b101, 0b110);

        assert_eq!(fpre.alpha_labels.len(), n);
        assert_eq!(fpre.beta_labels.len(), m);

        for (i, label_sharing) in fpre.alpha_labels.iter().enumerate() {
            let bit = 1<<i & 0b101;
            if bit != 0 {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share ^ fpre.delta_a.as_block());
            } else {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share);
            }
        }

        for (i, label_sharing) in fpre.beta_labels.iter().enumerate() {
            let bit = 1<<i & 0b110;
            if bit != 0 {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share ^ fpre.delta_a.as_block());
            } else {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share);
            }
        }
    }
}