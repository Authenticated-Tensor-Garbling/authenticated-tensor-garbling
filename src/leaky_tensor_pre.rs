//! Pi_LeakyTensor preprocessing protocol — paper Construction 2 (Appendix F).
//!
//! This file contains the `LeakyTriple` output struct and the
//! `LeakyTensorPre` orchestrator. The `generate()` body is scaffolded
//! (`unimplemented!()`) in Plan 1; Plan 2 replaces it with the 5-step
//! paper transcript; Plan 3 adds paper-invariant tests.

use crate::{
    bcot::IdealBCot,
    delta::Delta,
    sharing::AuthBitShare,
};
use rand::SeedableRng;
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
    ///
    /// Body is implemented in Plan 2 of Phase 4. This scaffold exists so
    /// callers (`run_preprocessing`, `combine_leaky_triples`) compile.
    pub fn generate(&mut self) -> LeakyTriple {
        let _ = &mut self.rng;    // silence unused warnings until Plan 2
        let _ = &mut self.bcot;
        unimplemented!("Pi_LeakyTensor generate() body is Plan 2 of Phase 4");
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
