use crate::{
    bcot::IdealBCot,
    block::Block,
    delta::Delta,
    macs::Mac,
    sharing::AuthBitShare,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// One leaky tensor triple (output of a single Pi_LeakyTensor execution).
/// Both gen and eval views are stored together for in-process use.
pub struct LeakyTriple {
    pub n: usize,
    pub m: usize,
    // Garbler A's view
    pub gen_alpha_shares: Vec<AuthBitShare>,
    pub gen_beta_shares: Vec<AuthBitShare>,
    /// length n*m, column-major: index = j*n+i (j = beta/y index, i = alpha/x index)
    pub gen_correlated_shares: Vec<AuthBitShare>,
    pub gen_gamma_shares: Vec<AuthBitShare>,
    pub gen_alpha_labels: Vec<Block>,
    pub gen_beta_labels: Vec<Block>,
    // Evaluator B's view
    pub eval_alpha_shares: Vec<AuthBitShare>,
    pub eval_beta_shares: Vec<AuthBitShare>,
    /// length n*m, column-major: index = j*n+i (j = beta/y index, i = alpha/x index)
    pub eval_correlated_shares: Vec<AuthBitShare>,
    pub eval_gamma_shares: Vec<AuthBitShare>,
    pub eval_alpha_labels: Vec<Block>,
    pub eval_beta_labels: Vec<Block>,
    // The deltas (shared across all triples produced by one run_preprocessing call)
    pub delta_a: Delta,
    pub delta_b: Delta,
}

/// Pi_LeakyTensor preprocessing protocol (Construction 2).
///
/// LeakyTensorPre BORROWS &mut IdealBCot (does not own it). This ensures all leaky triples
/// produced by one run_preprocessing call share the SAME delta_a and delta_b. If each
/// LeakyTensorPre owned a separate IdealBCot, different deltas would break the XOR combination
/// MAC invariant in Pi_aTensor bucketing.
pub struct LeakyTensorPre<'a> {
    pub n: usize,
    pub m: usize,
    bcot: &'a mut IdealBCot,
    rng: ChaCha12Rng,
}

impl<'a> LeakyTensorPre<'a> {
    pub fn new(seed: u64, n: usize, m: usize, bcot: &'a mut IdealBCot) -> Self {
        Self {
            n,
            m,
            bcot,
            rng: ChaCha12Rng::seed_from_u64(seed),
        }
    }

    /// Generate one leaky tensor triple for inputs x_clear (n-bit) and y_clear (m-bit).
    ///
    /// Layout invariant (matches gen_auth_bit canonical layout from auth_tensor_fpre.rs,
    /// and the paper's convention [x_pa]^{Δ_b}, [x_pb]^{Δ_a} — verifier's delta):
    ///   gen_share.key  = cot_a_to_b.sender_keys[i]   (A's sender key from A→B COT, LSB=0)
    ///   gen_share.mac  = cot_b_to_a.receiver_macs[i] (A's MAC on A's bit under delta_b)
    ///   eval_share.key = cot_b_to_a.sender_keys[i]   (B's sender key from B→A COT, LSB=0)
    ///   eval_share.mac = cot_a_to_b.receiver_macs[i] (B's MAC on B's bit under delta_a)
    pub fn generate(&mut self, x_clear: usize, y_clear: usize) -> LeakyTriple {
        // ---- Step 1: Random full alpha and beta bits ----
        let alpha_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
        let beta_bits: Vec<bool> = (0..self.m).map(|_| self.rng.random_bool(0.5)).collect();

        // ---- Step 2: Alpha auth shares via TWO COT calls ----
        let gen_alpha_portions: Vec<bool> =
            (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
        let eval_alpha_portions: Vec<bool> = gen_alpha_portions
            .iter()
            .zip(alpha_bits.iter())
            .map(|(&g, &full)| g ^ full)
            .collect();

        // COT A→B: A is sender with delta_a. Eval's bits as choice → [x_pb]^{Δ_a}.
        let cot_alpha_a_to_b = self.bcot.transfer_a_to_b(&eval_alpha_portions);
        // COT B→A: B is sender with delta_b. Gen's bits as choice → [x_pa]^{Δ_b}.
        let cot_alpha_b_to_a = self.bcot.transfer_b_to_a(&gen_alpha_portions);

        let gen_alpha_shares: Vec<AuthBitShare> = (0..self.n)
            .map(|i| AuthBitShare {
                key: cot_alpha_a_to_b.sender_keys[i], // A's sender key (A→B, LSB=0)
                mac: Mac::new(*cot_alpha_b_to_a.receiver_macs[i].as_block()), // A's MAC under delta_b
                value: gen_alpha_portions[i],
            })
            .collect();

        let eval_alpha_shares: Vec<AuthBitShare> = (0..self.n)
            .map(|i| AuthBitShare {
                key: cot_alpha_b_to_a.sender_keys[i], // B's sender key (B→A, LSB=0)
                mac: Mac::new(*cot_alpha_a_to_b.receiver_macs[i].as_block()), // B's MAC under delta_a
                value: eval_alpha_portions[i],
            })
            .collect();

        // ---- Step 3: Beta auth shares via TWO COT calls ----
        let gen_beta_portions: Vec<bool> =
            (0..self.m).map(|_| self.rng.random_bool(0.5)).collect();
        let eval_beta_portions: Vec<bool> = gen_beta_portions
            .iter()
            .zip(beta_bits.iter())
            .map(|(&g, &full)| g ^ full)
            .collect();

        let cot_beta_a_to_b = self.bcot.transfer_a_to_b(&eval_beta_portions);
        let cot_beta_b_to_a = self.bcot.transfer_b_to_a(&gen_beta_portions);

        let gen_beta_shares: Vec<AuthBitShare> = (0..self.m)
            .map(|i| AuthBitShare {
                key: cot_beta_a_to_b.sender_keys[i],
                mac: Mac::new(*cot_beta_b_to_a.receiver_macs[i].as_block()),
                value: gen_beta_portions[i],
            })
            .collect();

        let eval_beta_shares: Vec<AuthBitShare> = (0..self.m)
            .map(|i| AuthBitShare {
                key: cot_beta_b_to_a.sender_keys[i],
                mac: Mac::new(*cot_beta_a_to_b.receiver_macs[i].as_block()),
                value: eval_beta_portions[i],
            })
            .collect();

        // ---- Step 4: Alpha and beta labels ----
        let mut gen_alpha_labels: Vec<Block> = Vec::with_capacity(self.n);
        let mut eval_alpha_labels: Vec<Block> = Vec::with_capacity(self.n);
        for i in 0..self.n {
            let mut label_0 = Block::random(&mut self.rng);
            label_0.set_lsb(false);
            let masked_bit = (((x_clear >> i) & 1) != 0) ^ alpha_bits[i];
            let label_b = if masked_bit {
                label_0 ^ self.bcot.delta_a.as_block()
            } else {
                label_0
            };
            gen_alpha_labels.push(label_0);
            eval_alpha_labels.push(label_b);
        }

        let mut gen_beta_labels: Vec<Block> = Vec::with_capacity(self.m);
        let mut eval_beta_labels: Vec<Block> = Vec::with_capacity(self.m);
        for j in 0..self.m {
            let mut label_0 = Block::random(&mut self.rng);
            label_0.set_lsb(false);
            let masked_bit = (((y_clear >> j) & 1) != 0) ^ beta_bits[j];
            let label_b = if masked_bit {
                label_0 ^ self.bcot.delta_a.as_block()
            } else {
                label_0
            };
            gen_beta_labels.push(label_0);
            eval_beta_labels.push(label_b);
        }

        // ---- Step 5: Correlated bits (alpha_i AND beta_j) via TWO COT calls, column-major ----
        // column-major: index = j*n+i (j = beta/y index, i = alpha/x index)
        let mut corr_bits: Vec<bool> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                corr_bits.push(alpha_bits[i] && beta_bits[j]);
            }
        }
        let gen_corr_portions: Vec<bool> = (0..self.n * self.m)
            .map(|_| self.rng.random_bool(0.5))
            .collect();
        let eval_corr_portions: Vec<bool> = gen_corr_portions
            .iter()
            .zip(corr_bits.iter())
            .map(|(&g, &full)| g ^ full)
            .collect();

        let cot_corr_a_to_b = self.bcot.transfer_a_to_b(&eval_corr_portions);
        let cot_corr_b_to_a = self.bcot.transfer_b_to_a(&gen_corr_portions);

        let gen_correlated_shares: Vec<AuthBitShare> = (0..self.n * self.m)
            .map(|k| AuthBitShare {
                key: cot_corr_a_to_b.sender_keys[k],
                mac: Mac::new(*cot_corr_b_to_a.receiver_macs[k].as_block()),
                value: gen_corr_portions[k],
            })
            .collect();

        let eval_correlated_shares: Vec<AuthBitShare> = (0..self.n * self.m)
            .map(|k| AuthBitShare {
                key: cot_corr_b_to_a.sender_keys[k],
                mac: Mac::new(*cot_corr_a_to_b.receiver_macs[k].as_block()),
                value: eval_corr_portions[k],
            })
            .collect();

        // ---- Step 6: Gamma bits (uniform random) via TWO COT calls ----
        let gamma_bits: Vec<bool> = (0..self.n * self.m)
            .map(|_| self.rng.random_bool(0.5))
            .collect();
        let gen_gamma_portions: Vec<bool> = (0..self.n * self.m)
            .map(|_| self.rng.random_bool(0.5))
            .collect();
        let eval_gamma_portions: Vec<bool> = gen_gamma_portions
            .iter()
            .zip(gamma_bits.iter())
            .map(|(&g, &full)| g ^ full)
            .collect();

        let cot_gamma_a_to_b = self.bcot.transfer_a_to_b(&eval_gamma_portions);
        let cot_gamma_b_to_a = self.bcot.transfer_b_to_a(&gen_gamma_portions);

        let gen_gamma_shares: Vec<AuthBitShare> = (0..self.n * self.m)
            .map(|k| AuthBitShare {
                key: cot_gamma_a_to_b.sender_keys[k],
                mac: Mac::new(*cot_gamma_b_to_a.receiver_macs[k].as_block()),
                value: gen_gamma_portions[k],
            })
            .collect();

        let eval_gamma_shares: Vec<AuthBitShare> = (0..self.n * self.m)
            .map(|k| AuthBitShare {
                key: cot_gamma_b_to_a.sender_keys[k],
                mac: Mac::new(*cot_gamma_a_to_b.receiver_macs[k].as_block()),
                value: eval_gamma_portions[k],
            })
            .collect();

        // ---- Step 7: Assemble and return LeakyTriple ----
        LeakyTriple {
            n: self.n,
            m: self.m,
            delta_a: self.bcot.delta_a,
            delta_b: self.bcot.delta_b,
            gen_alpha_shares,
            eval_alpha_shares,
            gen_beta_shares,
            eval_beta_shares,
            gen_correlated_shares,
            eval_correlated_shares,
            gen_gamma_shares,
            eval_gamma_shares,
            gen_alpha_labels,
            eval_alpha_labels,
            gen_beta_labels,
            eval_beta_labels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;

    fn make_bcot() -> IdealBCot {
        IdealBCot::new(42, 99)
    }

    /// Cross-party MAC consistency check (Pa = garbler, Pb = evaluator).
    ///
    /// Each AuthBitShare is a cross-party struct mixing fields from both COT calls.
    /// "pa_share" means the struct whose .value is Pa's committed bit (not "what Pa holds").
    /// "pb_share" means the struct whose .value is Pb's committed bit (not "what Pb holds").
    ///
    /// Field ownership in the real protocol:
    ///   pa_share.value = Pa's bit                                   (Pa commits to this)
    ///   pa_share.mac   = K_Pa XOR Pa's_bit * delta_b               (Pa holds this MAC, under B's delta)
    ///   pa_share.key   = K_Pb  (Pb's sender key from cot_a_to_b)   (Pb holds this key)
    ///
    ///   pb_share.value = Pb's bit                                   (Pb commits to this)
    ///   pb_share.mac   = K_Pb XOR Pb's_bit * delta_a               (Pb holds this MAC, under A's delta)
    ///   pb_share.key   = K_Pa  (Pa's sender key from cot_b_to_a)   (Pa holds this key)
    fn verify_cross_party(
        pa_share: &AuthBitShare,
        pb_share: &AuthBitShare,
        delta_a: &Delta,
        delta_b: &Delta,
    ) {
        // Verify Pa's commitment: pa's bit under delta_b (verifier B's key)
        AuthBitShare { key: pb_share.key, mac: pa_share.mac, value: pa_share.value }
            .verify(delta_b);
        // Verify Pb's commitment: pb's bit under delta_a (verifier A's key)
        AuthBitShare { key: pa_share.key, mac: pb_share.mac, value: pb_share.value }
            .verify(delta_a);
    }

    // ---- Task 1a tests ----

    #[test]
    fn test_alpha_beta_dimensions() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(0, 4, 4, &mut bcot);
        let triple = ltp.generate(0b1010, 0b1100);
        assert_eq!(triple.gen_alpha_shares.len(), 4);
        assert_eq!(triple.gen_beta_shares.len(), 4);
        assert_eq!(triple.eval_alpha_shares.len(), 4);
    }

    #[test]
    fn test_alpha_beta_mac_invariants() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(1, 4, 4, &mut bcot);
        let t = ltp.generate(0b1010, 0b1100);
        // Pa's share is first arg, Pb's share is second. Use verify_cross_party — direct verify panics.
        for i in 0..4 {
            verify_cross_party(
                &t.gen_alpha_shares[i],
                &t.eval_alpha_shares[i],
                &t.delta_a,
                &t.delta_b,
            );
            verify_cross_party(
                &t.gen_beta_shares[i],
                &t.eval_beta_shares[i],
                &t.delta_a,
                &t.delta_b,
            );
        }
    }

    #[test]
    fn test_alpha_label_sharing() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(3, 4, 4, &mut bcot);
        let t = ltp.generate(0b1010, 0b1100);
        for i in 0..4 {
            let alpha_full = t.gen_alpha_shares[i].value ^ t.eval_alpha_shares[i].value;
            let x_bit = ((0b1010 >> i) & 1) != 0;
            let masked_bit = x_bit ^ alpha_full;
            if masked_bit {
                assert_eq!(
                    t.gen_alpha_labels[i],
                    t.eval_alpha_labels[i] ^ t.delta_a.as_block()
                );
            } else {
                assert_eq!(t.gen_alpha_labels[i], t.eval_alpha_labels[i]);
            }
        }
    }

    #[test]
    fn test_key_lsb_zero() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(5, 4, 4, &mut bcot);
        let t = ltp.generate(0, 0);
        for s in &t.gen_alpha_shares {
            assert!(!s.key.as_block().lsb(), "gen alpha key LSB must be 0");
        }
        for s in &t.eval_alpha_shares {
            assert!(!s.key.as_block().lsb(), "eval alpha key LSB must be 0");
        }
    }

    // ---- Task 1b tests ----

    #[test]
    fn test_correlated_bit_correctness() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(2, 4, 4, &mut bcot);
        let t = ltp.generate(0b1010, 0b1100);
        let alpha_full: Vec<bool> = (0..4)
            .map(|i| t.gen_alpha_shares[i].value ^ t.eval_alpha_shares[i].value)
            .collect();
        let beta_full: Vec<bool> = (0..4)
            .map(|j| t.gen_beta_shares[j].value ^ t.eval_beta_shares[j].value)
            .collect();
        for j in 0..4 {
            for i in 0..4 {
                // column-major: index = j*n+i
                let k = j * 4 + i;
                let full_corr =
                    t.gen_correlated_shares[k].value ^ t.eval_correlated_shares[k].value;
                assert_eq!(
                    full_corr,
                    alpha_full[i] && beta_full[j],
                    "correlated[j={} * 4 + i={}] mismatch: expected {} AND {} = {}",
                    j,
                    i,
                    alpha_full[i],
                    beta_full[j],
                    alpha_full[i] && beta_full[j]
                );
            }
        }
    }

    #[test]
    fn test_correlated_mac_invariants() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(6, 4, 4, &mut bcot);
        let t = ltp.generate(0b1010, 0b1100);
        // Pa's share is first arg, Pb's share is second. Use verify_cross_party — direct verify panics.
        for k in 0..16 {
            verify_cross_party(
                &t.gen_correlated_shares[k],
                &t.eval_correlated_shares[k],
                &t.delta_a,
                &t.delta_b,
            );
            verify_cross_party(
                &t.gen_gamma_shares[k],
                &t.eval_gamma_shares[k],
                &t.delta_a,
                &t.delta_b,
            );
        }
    }

    #[test]
    fn test_generate_dimensions_full() {
        let mut bcot = make_bcot();
        let mut ltp = LeakyTensorPre::new(0, 4, 4, &mut bcot);
        let triple = ltp.generate(0b1010, 0b1100);
        assert_eq!(triple.gen_correlated_shares.len(), 16);
        assert_eq!(triple.gen_gamma_shares.len(), 16);
        assert_eq!(triple.eval_correlated_shares.len(), 16);
    }

    #[test]
    fn test_large_n_m() {
        let mut bcot = IdealBCot::new(7, 8);
        let mut ltp = LeakyTensorPre::new(4, 8, 8, &mut bcot);
        let _t = ltp.generate(0xFF, 0xAA); // must not panic
    }
}
