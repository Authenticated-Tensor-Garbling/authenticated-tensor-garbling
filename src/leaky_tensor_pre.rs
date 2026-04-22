//! Pi_LeakyTensor preprocessing protocol — paper Construction 2 (Appendix F).
//!
//! This file contains the `LeakyTriple` output struct and the
//! `LeakyTensorPre` orchestrator. The `generate()` body is implemented in
//! Plan 2 of Phase 4; Plan 3 adds paper-invariant tests.

use crate::{
    bcot::IdealBCot,
    block::Block,
    delta::Delta,
    keys::Key,
    macs::Mac,
    matrix::BlockMatrix,
    sharing::AuthBitShare,
    tensor_macro::{tensor_garbler, tensor_evaluator},
    feq,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// One leaky tensor triple — output of a single Pi_LeakyTensor execution.
///
/// Field shape matches paper Construction 2 exactly (no gamma, no wire
/// labels — those belonged to the pre-rewrite algorithm). Both parties'
/// views are stored together for in-process use.
///
/// Cross-party layout (canonical codebase convention):
///   gen_*_share.key  = A's sender key from transfer_a_to_b   (LSB = 0)
///   gen_*_share.mac  = A's MAC from transfer_b_to_a           (MAC of A's bit under Δ_B)
///   eval_*_share.key = B's sender key from transfer_b_to_a   (LSB = 0)
///   eval_*_share.mac = B's MAC from transfer_a_to_b           (MAC of B's bit under Δ_A)
///
/// Never call `share.verify(&delta)` directly on a cross-party share —
/// it panics. Use `verify_cross_party(gen, eval, &Δ_A, &Δ_B)` from the
/// test module (preserved verbatim from the pre-rewrite file).
pub struct LeakyTriple {
    pub n: usize,
    pub m: usize,
    // Garbler A's view — paper notation x / y / Z.
    pub gen_x_shares: Vec<AuthBitShare>,   // length n
    pub gen_y_shares: Vec<AuthBitShare>,   // length m
    /// length n*m, column-major: index = j*n + i (j = y index, i = x index).
    pub gen_z_shares: Vec<AuthBitShare>,
    // Evaluator B's view.
    pub eval_x_shares: Vec<AuthBitShare>,  // length n
    pub eval_y_shares: Vec<AuthBitShare>,  // length m
    /// length n*m, column-major.
    pub eval_z_shares: Vec<AuthBitShare>,
    // Shared correlation keys for the run (same for every triple in one run_preprocessing).
    pub delta_a: Delta,
    pub delta_b: Delta,
}

/// Pi_LeakyTensor preprocessing protocol (Construction 2, Appendix F).
///
/// Borrows `&mut IdealBCot` so every triple produced by a single
/// `run_preprocessing` call shares the same Δ_A and Δ_B (required for
/// Phase 5 XOR combining to preserve the MAC invariant).
pub struct LeakyTensorPre<'a> {
    pub n: usize,
    pub m: usize,
    pub(crate) bcot: &'a mut IdealBCot,
    pub(crate) rng: ChaCha12Rng,
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

    /// Generate one leaky tensor triple per paper Construction 2.
    ///
    /// Preprocessing is input-independent: x, y, and R are all sampled
    /// uniformly at random from `self.rng`. The output shape is exactly
    /// `(itmac{x}{Δ}, itmac{y}{Δ}, itmac{Z}{Δ})` with no extra fields.
    pub fn generate(&mut self) -> LeakyTriple {
        // ========================================================
        // Step 1: Correlated randomness from IdealBCot (PROTO-04)
        // ========================================================

        // Sample BOTH parties' bits independently (D-01; paper-symmetric).
        let x_a_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
        let x_b_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
        let y_a_bits: Vec<bool> = (0..self.m).map(|_| self.rng.random_bool(0.5)).collect();
        let y_b_bits: Vec<bool> = (0..self.m).map(|_| self.rng.random_bool(0.5)).collect();
        let r_a_bits: Vec<bool> = (0..(self.n * self.m)).map(|_| self.rng.random_bool(0.5)).collect();
        let r_b_bits: Vec<bool> = (0..(self.n * self.m)).map(|_| self.rng.random_bool(0.5)).collect();

        // Five bCOT batch pairs — 6 calls into IdealBCot.
        //   transfer_a_to_b(&v_b_bits): A is sender (Δ_B correlation), B picks on v_B.
        //   transfer_b_to_a(&v_a_bits): B is sender (Δ_A correlation), A picks on v_A.
        // This is Pattern 1 in 04-PATTERNS.md.
        let cot_x_a_to_b = self.bcot.transfer_a_to_b(&x_b_bits);
        let cot_x_b_to_a = self.bcot.transfer_b_to_a(&x_a_bits);
        let cot_y_a_to_b = self.bcot.transfer_a_to_b(&y_b_bits);
        let cot_y_b_to_a = self.bcot.transfer_b_to_a(&y_a_bits);
        let cot_r_a_to_b = self.bcot.transfer_a_to_b(&r_b_bits);
        let cot_r_b_to_a = self.bcot.transfer_b_to_a(&r_a_bits);

        // Cross-party AuthBitShare assembly.
        let gen_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
            key:   cot_x_a_to_b.sender_keys[i],
            mac:   Mac::new(*cot_x_b_to_a.receiver_macs[i].as_block()),
            value: x_a_bits[i],
        }).collect();
        let eval_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
            key:   cot_x_b_to_a.sender_keys[i],
            mac:   Mac::new(*cot_x_a_to_b.receiver_macs[i].as_block()),
            value: x_b_bits[i],
        }).collect();
        let gen_y_shares: Vec<AuthBitShare> = (0..self.m).map(|j| AuthBitShare {
            key:   cot_y_a_to_b.sender_keys[j],
            mac:   Mac::new(*cot_y_b_to_a.receiver_macs[j].as_block()),
            value: y_a_bits[j],
        }).collect();
        let eval_y_shares: Vec<AuthBitShare> = (0..self.m).map(|j| AuthBitShare {
            key:   cot_y_b_to_a.sender_keys[j],
            mac:   Mac::new(*cot_y_a_to_b.receiver_macs[j].as_block()),
            value: y_b_bits[j],
        }).collect();
        let gen_r_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| AuthBitShare {
            key:   cot_r_a_to_b.sender_keys[k],
            mac:   Mac::new(*cot_r_b_to_a.receiver_macs[k].as_block()),
            value: r_a_bits[k],
        }).collect();
        let eval_r_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| AuthBitShare {
            key:   cot_r_b_to_a.sender_keys[k],
            mac:   Mac::new(*cot_r_a_to_b.receiver_macs[k].as_block()),
            value: r_b_bits[k],
        }).collect();

        // ========================================================
        // Step 2: Compute C_A, C_B, C_A^(R), C_B^(R) inline (PROTO-05, D-10)
        // ========================================================
        //
        // Per paper Construction 2 (Appendix F, lines 208-216):
        //   C_A[j]    := y_A[j]·Δ_A ⊕ key(y_B@A)[j]  ⊕ mac(y_A@B)[j]
        //   C_B[j]    := y_B[j]·Δ_B ⊕ mac(y_B@A)[j]  ⊕ key(y_A@B)[j]
        //   C_A^(R)[k]:= R_A[k]·Δ_A ⊕ key(R_B@A)[k]  ⊕ mac(R_A@B)[k]
        //   C_B^(R)[k]:= R_B[k]·Δ_B ⊕ mac(R_B@A)[k]  ⊕ key(R_A@B)[k]
        //
        // Field mapping (Pitfall 3 in RESEARCH.md):
        //   key(y_B@A)[j] = gen_y_shares[j].key     mac(y_A@B)[j] = gen_y_shares[j].mac
        //   mac(y_B@A)[j] = eval_y_shares[j].mac    key(y_A@B)[j] = eval_y_shares[j].key

        let delta_a_block: Block = *self.bcot.delta_a.as_block();
        let delta_b_block: Block = *self.bcot.delta_b.as_block();

        let mut c_a: Vec<Block> = Vec::with_capacity(self.m);
        let mut c_b: Vec<Block> = Vec::with_capacity(self.m);
        for j in 0..self.m {
            let y_a_term = if gen_y_shares[j].value  { delta_a_block } else { Block::ZERO };
            let y_b_term = if eval_y_shares[j].value { delta_b_block } else { Block::ZERO };
            c_a.push(
                y_a_term
                    ^ *gen_y_shares[j].key.as_block()
                    ^ *gen_y_shares[j].mac.as_block(),
            );
            c_b.push(
                y_b_term
                    ^ *eval_y_shares[j].mac.as_block()
                    ^ *eval_y_shares[j].key.as_block(),
            );
        }

        let mut c_a_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
        let mut c_b_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for k in 0..(self.n * self.m) {
            let r_a_term = if gen_r_shares[k].value  { delta_a_block } else { Block::ZERO };
            let r_b_term = if eval_r_shares[k].value { delta_b_block } else { Block::ZERO };
            c_a_r.push(
                r_a_term
                    ^ *gen_r_shares[k].key.as_block()
                    ^ *gen_r_shares[k].mac.as_block(),
            );
            c_b_r.push(
                r_b_term
                    ^ *eval_r_shares[k].mac.as_block()
                    ^ *eval_r_shares[k].key.as_block(),
            );
        }

        // The following bindings are consumed by Task 2.2 (Step 3-5).
        // Silence unused-variable warnings for values that Task 2.2 reads.
        let _ = (&gen_x_shares, &eval_x_shares,
                 &gen_r_shares, &eval_r_shares,
                 &c_a, &c_b, &c_a_r, &c_b_r,
                 &cot_x_a_to_b, &cot_x_b_to_a);

        unimplemented!("Step 3-5 — Task 2.2");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;
    use crate::delta::Delta;

    /// Cross-party MAC verification helper — PRESERVE VERBATIM.
    ///
    /// Direct `share.verify(delta)` panics on cross-party shares because
    /// `gen.key` and `gen.mac` come from different bCOT directions. This
    /// helper reconstructs the properly-aligned pair and verifies under
    /// the correct delta.
    #[allow(dead_code)]
    pub(crate) fn verify_cross_party(
        pa_share: &AuthBitShare,
        pb_share: &AuthBitShare,
        delta_a: &Delta,
        delta_b: &Delta,
    ) {
        AuthBitShare {
            key: pb_share.key,
            mac: pa_share.mac,
            value: pa_share.value,
        }
        .verify(delta_b);
        AuthBitShare {
            key: pa_share.key,
            mac: pb_share.mac,
            value: pb_share.value,
        }
        .verify(delta_a);
    }

    #[allow(dead_code)]
    fn make_bcot() -> IdealBCot {
        IdealBCot::new(42, 99)
    }

    // ===== Plan 1 shape test (compile-time + runtime struct access) =====

    #[test]
    fn test_leaky_triple_shape_field_access() {
        // Prove the 10 fields exist and have the expected types by touching
        // them via a default-initialized instance. This is NOT a semantic
        // test — Plan 3 adds the real PROTO-09 / paper-invariant tests.
        let mut bcot = make_bcot();
        let delta_a = bcot.delta_a;
        let delta_b = bcot.delta_b;
        let triple = LeakyTriple {
            n: 2,
            m: 3,
            gen_x_shares: vec![AuthBitShare::default(); 2],
            gen_y_shares: vec![AuthBitShare::default(); 3],
            gen_z_shares: vec![AuthBitShare::default(); 6],
            eval_x_shares: vec![AuthBitShare::default(); 2],
            eval_y_shares: vec![AuthBitShare::default(); 3],
            eval_z_shares: vec![AuthBitShare::default(); 6],
            delta_a,
            delta_b,
        };
        assert_eq!(triple.n, 2);
        assert_eq!(triple.m, 3);
        assert_eq!(triple.gen_x_shares.len(), 2);
        assert_eq!(triple.gen_y_shares.len(), 3);
        assert_eq!(triple.gen_z_shares.len(), 6);
        assert_eq!(triple.eval_x_shares.len(), 2);
        assert_eq!(triple.eval_y_shares.len(), 3);
        assert_eq!(triple.eval_z_shares.len(), 6);
        let _ = &triple.delta_a;
        let _ = &triple.delta_b;
    }

    // ===== Plan 2 / Plan 3 placeholders (bodies filled in later plans) =====

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_correlated_randomness_dimensions() { /* PROTO-04 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_c_a_c_b_xor_invariant() { /* PROTO-05 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_macro_outputs_xor_invariant() { /* PROTO-06 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_d_extraction_and_z_assembly() { /* PROTO-07 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_feq_passes_on_honest_run() { /* PROTO-08 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_leaky_triple_mac_invariants() { /* TEST-02 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_leaky_triple_product_invariant() { /* TEST-03 — Plan 3 */ }

    #[test]
    #[ignore = "Plan 2 — generate() body"]
    fn test_key_lsb_zero_all_shares() { /* preserved invariant — Plan 3 */ }
}
