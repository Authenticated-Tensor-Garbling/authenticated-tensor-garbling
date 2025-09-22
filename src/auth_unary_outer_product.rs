// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{delta::Delta, fpre::{AuthBit, AuthTriple, build_share, AuthBitShare}};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// Insecure ideal Fpre that pre-generates auth bits for input and output vectors of a tensor gate.
pub struct TensorFpre {
    rng: ChaCha12Rng,
    n: usize,
    m: usize,
    delta_a: Delta,
    delta_b: Delta,
    alpha_auth_bits: Vec<AuthBit>,
    beta_auth_bits: Vec<AuthBit>,
    correlated_auth_bits: Vec<AuthBit>,
    gamma_auth_bits: Vec<AuthBit>,
}

pub struct TensorFpre {
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
            alpha_auth_bits: Vec::with_capacity(n),
            beta_auth_bits: Vec::with_capacity(m),
            correlated_auth_bits: Vec::with_capacity(n * m),
            gamma_auth_bits: Vec::with_capacity(n * m),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_fpre() {
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
}