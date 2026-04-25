// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{block::Block, delta::Delta, sharing::{AuthBit, build_share, AuthBitShare, InputSharing}};
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
    x_labels: Vec<InputSharing>,
    y_labels: Vec<InputSharing>,
    alpha_auth_bits: Vec<AuthBit>,
    beta_auth_bits: Vec<AuthBit>,
    correlated_auth_bits: Vec<AuthBit>,
}

impl TensorFpre {
    /// Creates a new tensor_fpre with random `delta_a`, `delta_b`.
    pub fn new(seed: u64, n: usize, m: usize, chunking_factor: usize) -> Self {
        let mut rng = ChaCha12Rng::seed_from_u64(seed);

        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random(&mut rng);

        Self {
            rng,
            n,
            m,
            chunking_factor,
            delta_a,
            delta_b,
            x_labels: Vec::with_capacity(n),
            y_labels: Vec::with_capacity(m),
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
            x_labels: Vec::with_capacity(n),
            y_labels: Vec::with_capacity(m),
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

    /// Generates all authenticated bits and input sharings for the ideal trusted dealer.
    /// This is NOT the real preprocessing protocol — it is the ideal functionality
    /// (trusted dealer) that the online phase consumes directly in tests and benchmarks.
    pub fn generate_for_ideal_trusted_dealer(&mut self, x: usize, y: usize) -> (usize, usize) {
        assert!(
            self.n <= usize::BITS as usize - 1,
            "generate_for_ideal_trusted_dealer: n={} exceeds usize bit width minus 1; \
             x must be representable as usize", self.n
        );
        assert!(
            self.m <= usize::BITS as usize - 1,
            "generate_for_ideal_trusted_dealer: m={} exceeds usize bit width minus 1; \
             y must be representable as usize", self.m
        );

        let mut alpha: usize = 0;
        for i in 0..self.n {
            // generate the auth bit
            let alpha_bit = self.rng.random_bool(0.5);
            let alpha_auth_bit = self.gen_auth_bit(alpha_bit);
            self.alpha_auth_bits.push(alpha_auth_bit);

            // accumulate alpha bits in little-endian order
            alpha |= (alpha_bit as usize) << i;


            // generate the label sharing of x ^ alpha
            let mut gen_label = Block::random(&mut self.rng);
            gen_label.set_lsb(false);

            let eval_label: Block;
            let bit = ((1<<i & x) != 0) ^ alpha_bit;
            if bit {
                eval_label = gen_label ^ self.delta_a.as_block();
            } else {
                eval_label = gen_label.clone();
            }

            self.x_labels.push(InputSharing { gen_share: gen_label, eval_share: eval_label });

        }

        let mut beta: usize = 0;
        for j in 0..self.m {
            // generate the auth bit
            let beta_bit = self.rng.random_bool(0.5);
            let beta_auth_bit = self.gen_auth_bit(beta_bit);
            self.beta_auth_bits.push(beta_auth_bit);

            // accumulate beta bits in little-endian order
            beta |= (beta_bit as usize) << j;

            // generate the label sharing of y ^ beta
            let mut gen_label = Block::random(&mut self.rng);
            gen_label.set_lsb(false);

            let eval_label: Block;

            let bit = ((1<<j & y) != 0) ^ beta_bit;
            if bit {
                eval_label = gen_label ^ self.delta_a.as_block();
            } else {
                eval_label = gen_label.clone();
            }

            self.y_labels.push(InputSharing { gen_share: gen_label, eval_share: eval_label });
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
        (alpha, beta)
    }

    pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {

        (TensorFpreGen {
            n: self.n,
            m: self.m,
            chunking_factor: self.chunking_factor,
            delta_a: self.delta_a,
            alpha_labels: self.x_labels.iter().map(|share| share.gen_share).collect(),
            beta_labels: self.y_labels.iter().map(|share| share.gen_share).collect(),
            alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.gen_share).collect(),
            alpha_d_ev_shares: vec![],      // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            beta_d_ev_shares: vec![],       // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            correlated_d_ev_shares: vec![], // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            gamma_d_ev_shares: vec![],      // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
        }, TensorFpreEval {
            n: self.n,
            m: self.m,
            chunking_factor: self.chunking_factor,
            delta_b: self.delta_b,
            alpha_labels: self.x_labels.iter().map(|share| share.eval_share).collect(),
            beta_labels: self.y_labels.iter().map(|share| share.eval_share).collect(),
            alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            beta_auth_bit_shares: self.beta_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|bit| bit.eval_share).collect(),
            alpha_d_ev_shares: vec![],      // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            beta_d_ev_shares: vec![],       // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            correlated_d_ev_shares: vec![], // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
            gamma_d_ev_shares: vec![],      // Phase 9: D_ev fields populated by IdealPreprocessingBackend; into_gen_eval leaves them empty
        })
    }

    /// Gets the clear values of the input and output vectors and the auth bits.
    /// x_label holds x^alpha
    /// y_label holds y^beta
    /// alpha_auth_bits holds alpha
    /// beta_auth_bits holds beta
    /// Returns (x^alpha, y^beta, alpha, beta)
    pub fn get_clear_values(&self) -> (usize, usize, usize, usize) {

        let mut x: usize = 0;
        let mut y: usize = 0;

        let mut alpha: usize = 0;
        let mut beta: usize = 0;
        for i in 0..self.n {
            x |= (self.x_labels[i].shares_differ() as usize) << i;
            alpha |= (self.alpha_auth_bits[i].full_bit() as usize) << i;
        }
        for j in 0..self.m {
            y |= (self.y_labels[j].shares_differ() as usize) << j;
            beta |= (self.beta_auth_bits[j].full_bit() as usize) << j;
        }

        (x^alpha, y^beta, alpha, beta)
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
        fpre.generate_for_ideal_trusted_dealer(0b101, 0b110);

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

    #[test]
    fn test_tensor_fpre_input_sharings() {
        let n = 3;
        let m = 4;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_for_ideal_trusted_dealer(0b101, 0b110);

        assert_eq!(fpre.x_labels.len(), n);
        assert_eq!(fpre.y_labels.len(), m);

        for (i, label_sharing) in fpre.x_labels.iter().enumerate() {
            let alpha = &fpre.alpha_auth_bits[i].full_bit();

            let bit = ((1<<i & 0b101) != 0) ^ alpha;
            if bit {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share ^ fpre.delta_a.as_block());
            } else {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share);
            }
        }

        for (i, label_sharing) in fpre.y_labels.iter().enumerate() {
            let beta = &fpre.beta_auth_bits[i].full_bit();
            let bit = ((1<<i & 0b110) != 0) ^ beta;
            if bit {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share ^ fpre.delta_a.as_block());
            } else {
                assert_eq!(label_sharing.eval_share, label_sharing.gen_share);
            }
        }

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        assert_eq!(fpre_gen.alpha_labels.len(), n);
        assert_eq!(fpre_gen.beta_labels.len(), m);

        assert_eq!(fpre_gen.alpha_auth_bit_shares.len(), n);
        assert_eq!(fpre_gen.beta_auth_bit_shares.len(), m);

        assert_eq!(fpre_gen.correlated_auth_bit_shares.len(), n * m);


        assert_eq!(fpre_eval.alpha_labels.len(), n);
        assert_eq!(fpre_eval.beta_labels.len(), m);

        assert_eq!(fpre_eval.alpha_auth_bit_shares.len(), n);
        assert_eq!(fpre_eval.beta_auth_bit_shares.len(), m);

        assert_eq!(fpre_eval.correlated_auth_bit_shares.len(), n * m);
    }
}
