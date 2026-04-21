// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{block::Block, delta::Delta, sharing::{AuthBit, build_share, AuthBitShare, InputSharing}};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};

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
    gamma_auth_bits: Vec<AuthBit>,
}

pub struct TensorFpreGen {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
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
    pub chunking_factor: usize,
    pub delta_b: Delta,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
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
            gamma_auth_bits:        Vec::with_capacity(n * m),
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
    pub fn generate_with_input_values(&mut self, x: usize, y: usize) -> (usize, usize) {

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

        // gamma wires
        // column-major indexing
        for j in 0..self.m {
            for i in 0..self.n {
                let g = self.rng.random_bool(0.5);
                let gamma_auth_bit = self.gen_auth_bit(g);
                self.gamma_auth_bits.push(gamma_auth_bit);        

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
            gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|bit| bit.gen_share).collect(),
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
            gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|bit| bit.eval_share).collect(),
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

/// Run the real two-party uncompressed preprocessing protocol (Pi_aTensor, Construction 3).
///
/// Generates `count` authenticated tensor triples using:
///   1. bucket_size_for(n, m) leaky triples per output triple (from Pi_LeakyTensor)
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
/// x_clear and y_clear are zero — preprocessing generates masks without specific
/// input binding. Actual inputs are provided during the online phase.
pub fn run_preprocessing(
    n: usize,
    m: usize,
    count: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(count, 1, "Phase 1: only count=1 is supported; batch output requires Vec return");

    let bucket_size = bucket_size_for(n, m);
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
        triples.push(ltp.generate(0, 0));
    }

    combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_fpre_auth_bits() {
        let n = 3;
        let m = 4;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_with_input_values(0b101, 0b110);

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
                assert!(alpha_beta.full_bit() == (alpha.full_bit() && beta.full_bit()), "alpha_beta must equal alpha.full_bit() && beta.full_bit()");
            }
        }
    }
    
    #[test]
    fn test_tensor_fpre_input_sharings() {
        let n = 3;
        let m = 4;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_with_input_values(0b101, 0b110);

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
        assert_eq!(fpre_gen.gamma_auth_bit_shares.len(), n * m);

        
        assert_eq!(fpre_eval.alpha_labels.len(), n);
        assert_eq!(fpre_eval.beta_labels.len(), m);

        assert_eq!(fpre_eval.alpha_auth_bit_shares.len(), n);
        assert_eq!(fpre_eval.beta_auth_bit_shares.len(), m);

        assert_eq!(fpre_eval.correlated_auth_bit_shares.len(), n * m);
        assert_eq!(fpre_eval.gamma_auth_bit_shares.len(), n * m);
    }

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
    fn test_run_preprocessing_mac_invariants() {
        use crate::sharing::AuthBitShare;
        let (gen_out, eval_out) = super::run_preprocessing(4, 4, 1, 1);
        // gen_share.key = A's sender key; eval_share.key = B's sender key.
        // Gen commits under delta_b; eval commits under delta_a.
        let verify_pair = |g: &AuthBitShare, e: &AuthBitShare| {
            AuthBitShare { key: e.key, mac: g.mac, value: g.value }.verify(&eval_out.delta_b);
            AuthBitShare { key: g.key, mac: e.mac, value: e.value }.verify(&gen_out.delta_a);
        };
        for i in 0..gen_out.alpha_auth_bit_shares.len() {
            verify_pair(&gen_out.alpha_auth_bit_shares[i], &eval_out.alpha_auth_bit_shares[i]);
        }
        for i in 0..gen_out.beta_auth_bit_shares.len() {
            verify_pair(&gen_out.beta_auth_bit_shares[i], &eval_out.beta_auth_bit_shares[i]);
        }
        for i in 0..gen_out.correlated_auth_bit_shares.len() {
            verify_pair(&gen_out.correlated_auth_bit_shares[i], &eval_out.correlated_auth_bit_shares[i]);
        }
        for i in 0..gen_out.gamma_auth_bit_shares.len() {
            verify_pair(&gen_out.gamma_auth_bit_shares[i], &eval_out.gamma_auth_bit_shares[i]);
        }
    }

    #[test]
    fn test_run_preprocessing_feeds_online_phase() {
        let (fpre_gen, fpre_eval) = super::run_preprocessing(4, 4, 1, 1);
        let _gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        // No panic = success
    }
}