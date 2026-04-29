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
#[derive(Clone)]
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

        // ========================================================
        // Step 3: Two tensor_macro calls (PROTO-06, D-13/D-14/D-15)
        // ========================================================
        //
        // Wrap C_A and C_B as m×1 BlockMatrix column vectors for tensor_macro.
        let mut t_a = BlockMatrix::new(self.m, 1);
        let mut t_b = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            t_a[j] = c_a[j];
            t_b[j] = c_b[j];
        }

        // Macro Call 1: A garbles under Δ_A, B evaluates.
        //   Keys: cot_x_a_to_b.sender_keys (A's keys; LSB=0 by Key invariant).
        //   MACs: cot_x_a_to_b.receiver_macs (B's MACs = K[0] XOR x_B * Δ_A; lsb=x_b since Δ_A.lsb()=1).
        //   Explicit bits x_b_bits passed to evaluator for GGM tree navigation.
        let (z_gb1, g_1) = tensor_garbler(
            self.n, self.m, self.bcot.delta_a,
            &cot_x_a_to_b.sender_keys,
            &t_a,
        );
        let e_1 = tensor_evaluator(
            self.n, self.m, &g_1,
            &cot_x_a_to_b.receiver_macs,
            &x_b_bits,
            &t_b,
        );

        // Macro Call 2: B garbles under Δ_B, A evaluates.
        //   Keys: cot_x_b_to_a.sender_keys (B's keys; LSB=0 by Key invariant).
        //   MACs: cot_x_b_to_a.receiver_macs (A's MACs = K[0] XOR x_A * Δ_B; lsb != x_a since Δ_B.lsb()=0).
        //   Explicit bits x_a_bits passed to evaluator — mandatory since MAC LSB is unreliable.
        let (z_gb2, g_2) = tensor_garbler(
            self.n, self.m, self.bcot.delta_b,
            &cot_x_b_to_a.sender_keys,
            &t_b,
        );
        let e_2 = tensor_evaluator(
            self.n, self.m, &g_2,
            &cot_x_b_to_a.receiver_macs,
            &x_a_bits,
            &t_a,
        );

        // ========================================================
        // Step 4: Masked reveal — S_1, S_2, D (PROTO-07, D-16)
        // ========================================================
        //
        // Wrap C_A^(R) and C_B^(R) (Vec<Block> length n*m, column-major k=j*n+i)
        // as n×m BlockMatrix so the borrowed XOR impl can combine them with
        // the n×m z_gb / e matrices.
        let mut c_a_r_mat = BlockMatrix::new(self.n, self.m);
        let mut c_b_r_mat = BlockMatrix::new(self.n, self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let k = j * self.n + i;
                c_a_r_mat[(i, j)] = c_a_r[k];
                c_b_r_mat[(i, j)] = c_b_r[k];
            }
        }

        let s_1: BlockMatrix = &(&z_gb1 ^ &e_2) ^ &c_a_r_mat;
        let s_2: BlockMatrix = &(&z_gb2 ^ &e_1) ^ &c_b_r_mat;

        // D = lsb(S_1) ⊕ lsb(S_2), stored column-major (k = j*n + i).
        // Paper correctness precondition: lsb(Δ_A ⊕ Δ_B) == 1 (enforced by Plan 1
        // via Δ_B.lsb() == 0 in IdealBCot::new; regression test
        // bcot::tests::test_delta_xor_lsb_is_one).
        let mut d_bits: Vec<bool> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                d_bits.push(s_1[(i, j)].lsb() ^ s_2[(i, j)].lsb());
            }
        }

        // ========================================================
        // Step 5: F_eq check + final Z assembly (PROTO-07 + PROTO-08, D-17)
        // ========================================================
        //
        // L_1, L_2 are n×m BlockMatrix with L_1 = S_1 ⊕ D·Δ_A and L_2 = S_2 ⊕ D·Δ_B.
        let mut l_1 = BlockMatrix::new(self.n, self.m);
        let mut l_2 = BlockMatrix::new(self.n, self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let k = j * self.n + i;
                let d_term_a = if d_bits[k] { delta_a_block } else { Block::ZERO };
                let d_term_b = if d_bits[k] { delta_b_block } else { Block::ZERO };
                l_1[(i, j)] = s_1[(i, j)] ^ d_term_a;
                l_2[(i, j)] = s_2[(i, j)] ^ d_term_b;
            }
        }

        // In-process ideal F_eq. Panics with "F_eq abort: ..." on mismatch (D-04).
        feq::check(&l_1, &l_2);

        // itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}.
        // D is public ⇒ each party locally constructs a "trivial" share of D.
        //
        // Corrected A1 convention (TEST-02 gate, Plan 3 fix):
        //   gen_d: holds the bit value AND the Δ_B MAC mass (so verify_cross_party
        //          step 1 sees: mac = gen_r.mac XOR d*Δ_B = K_B[0] XOR (r_a XOR d)*Δ_B ✓)
        //   eval_d: zero key, zero mac, zero value (eval holds no Δ mass for D)
        //
        // Trace: verify_cross_party(gen_z, eval_z, Δ_A, Δ_B) step 1:
        //   {key=eval_r.key, mac=gen_r.mac XOR d*Δ_B, value=r_a XOR d}.verify(Δ_B)
        //   = K_B[0] XOR (r_a XOR d)*Δ_B == auth(r_a XOR d, Δ_B) ✓
        // step 2:
        //   {key=gen_r.key, mac=eval_r.mac, value=r_b}.verify(Δ_A)
        //   = K_A[0] XOR r_b*Δ_A == auth(r_b, Δ_A) ✓
        let gen_z_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| {
            let mac_block = if d_bits[k] { delta_b_block } else { Block::ZERO };
            let gen_d = AuthBitShare {
                key:   Key::default(),
                mac:   Mac::new(mac_block),
                value: d_bits[k],
            };
            gen_r_shares[k] + gen_d
        }).collect();

        // eval side: evaluator holds no D contribution (D is public; eval_z = eval_r only).
        let eval_z_shares: Vec<AuthBitShare> = eval_r_shares;

        LeakyTriple {
            n: self.n,
            m: self.m,
            gen_x_shares,
            gen_y_shares,
            gen_z_shares,
            eval_x_shares,
            eval_y_shares,
            eval_z_shares,
            delta_a: self.bcot.delta_a,
            delta_b: self.bcot.delta_b,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;
    use crate::sharing::verify_cross_party;

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
        let bcot = make_bcot();
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

        // Plan 3 extension (PROTO-09): real generate() output respects the same shape.
        let mut bcot2 = IdealBCot::new(42, 99);
        let real = LeakyTensorPre::new(7, 2, 3, &mut bcot2).generate();
        assert_eq!(real.n, 2);
        assert_eq!(real.m, 3);
        assert_eq!(real.gen_x_shares.len(), 2);
        assert_eq!(real.gen_y_shares.len(), 3);
        assert_eq!(real.gen_z_shares.len(), 6);
        assert_eq!(real.eval_x_shares.len(), 2);
        assert_eq!(real.eval_y_shares.len(), 3);
        assert_eq!(real.eval_z_shares.len(), 6);
    }

    // ===== Task 3.1: PROTO-04, PROTO-05, PROTO-09 (extended), Key-LSB regression =====

    #[test]
    fn test_correlated_randomness_dimensions() {
        for (seed, n, m) in [(0u64, 1, 1), (1, 4, 4), (2, 8, 3)] {
            let mut bcot = IdealBCot::new(42, 99);
            let triple = LeakyTensorPre::new(seed, n, m, &mut bcot).generate();
            assert_eq!(triple.n, n, "triple.n mismatch at seed={}", seed);
            assert_eq!(triple.m, m, "triple.m mismatch at seed={}", seed);
            assert_eq!(triple.gen_x_shares.len(), n,
                "gen_x_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.eval_x_shares.len(), n,
                "eval_x_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.gen_y_shares.len(), m,
                "gen_y_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.eval_y_shares.len(), m,
                "eval_y_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.gen_z_shares.len(), n * m,
                "gen_z_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.eval_z_shares.len(), n * m,
                "eval_z_shares len (seed={}, n={}, m={})", seed, n, m);
        }
    }

    #[test]
    fn test_c_a_c_b_xor_invariant() {
        // Paper identity (Construction 2 appendix, lines 262-279):
        //     C_A[j] XOR C_B[j] == y_full[j] * (Delta_A XOR Delta_B)
        //
        // C_A and C_B are local to generate(); verify the equivalent cross-party
        // BDOZ identity on the exposed Y shares:
        //     gen_y.key XOR gen_y.mac XOR eval_y.key XOR eval_y.mac
        //         == y_full * (Delta_A XOR Delta_B)
        //
        // (The Delta_A term inside C_A (y_A * Delta_A) and the Delta_B term
        // inside C_B (y_B * Delta_B) XOR to (y_A XOR y_B) * (Delta_A XOR Delta_B)
        // = y_full * (Delta_A XOR Delta_B) exactly when the four cross-party
        // field XORs cancel in the expected pattern. See Pattern 2 in 04-PATTERNS.md.)
        let (n, m) = (4, 4);
        let mut bcot = IdealBCot::new(42, 99);
        let delta_xor_block: Block = *bcot.delta_a.as_block() ^ *bcot.delta_b.as_block();
        let triple = LeakyTensorPre::new(5, n, m, &mut bcot).generate();

        for j in 0..m {
            let y_full = triple.gen_y_shares[j].value ^ triple.eval_y_shares[j].value;
            let lhs = *triple.gen_y_shares[j].key.as_block()
                ^ *triple.gen_y_shares[j].mac.as_block()
                ^ *triple.eval_y_shares[j].key.as_block()
                ^ *triple.eval_y_shares[j].mac.as_block();
            let rhs = if y_full { delta_xor_block } else { Block::ZERO };
            assert_eq!(
                lhs, rhs,
                "PROTO-05 cross-party BDOZ identity violated at j={} (y_full={})",
                j, y_full
            );
        }
    }

    #[test]
    fn test_key_lsb_zero_all_shares() {
        let (n, m) = (4, 4);
        let mut bcot = IdealBCot::new(42, 99);
        let triple = LeakyTensorPre::new(9, n, m, &mut bcot).generate();
        let all: [(&str, &Vec<AuthBitShare>); 6] = [
            ("gen_x",  &triple.gen_x_shares),
            ("eval_x", &triple.eval_x_shares),
            ("gen_y",  &triple.gen_y_shares),
            ("eval_y", &triple.eval_y_shares),
            ("gen_z",  &triple.gen_z_shares),
            ("eval_z", &triple.eval_z_shares),
        ];
        for (label, shares) in all {
            for (i, s) in shares.iter().enumerate() {
                assert!(
                    !s.key.as_block().lsb(),
                    "Key LSB invariant violated at {}[{}]: key.lsb() == true",
                    label, i
                );
            }
        }
    }

    // ===== Task 3.2: PROTO-06, PROTO-07, PROTO-08, TEST-02, TEST-03, TEST-04 =====

    #[test]
    fn test_leaky_triple_mac_invariants() {
        // TEST-02: IT-MAC invariant under cross-party layout.
        //
        // For each share in x, y, Z: mac_A == key_A XOR bit_A * Delta_B (verified by
        // verify_cross_party). A failure on z_shares signals Plan 2's A1
        // convention for itmac{D}{Delta} needs to be swapped (see Plan 2 Step 5 doc).
        let (n, m) = (4, 4);
        let mut bcot = IdealBCot::new(42, 99);
        let triple = LeakyTensorPre::new(17, n, m, &mut bcot).generate();
        for i in 0..n {
            verify_cross_party(
                &triple.gen_x_shares[i],
                &triple.eval_x_shares[i],
                &triple.delta_a,
                &triple.delta_b,
            );
        }
        for j in 0..m {
            verify_cross_party(
                &triple.gen_y_shares[j],
                &triple.eval_y_shares[j],
                &triple.delta_a,
                &triple.delta_b,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &triple.gen_z_shares[k],
                &triple.eval_z_shares[k],
                &triple.delta_a,
                &triple.delta_b,
            );
        }
    }

    #[test]
    fn test_leaky_triple_product_invariant() {
        // TEST-03: z_full[j*n+i] == x_full[i] AND y_full[j] for all (i, j).
        //
        // This is the headline correctness property of Pi_LeakyTensor — the
        // whole phase exists to make this test pass.
        for (seed, n, m) in [(21u64, 1, 1), (22, 2, 3), (23, 4, 4)] {
            let mut bcot = IdealBCot::new(42, 99);
            let triple = LeakyTensorPre::new(seed, n, m, &mut bcot).generate();
            let x_full: Vec<bool> = (0..n)
                .map(|i| triple.gen_x_shares[i].value ^ triple.eval_x_shares[i].value)
                .collect();
            let y_full: Vec<bool> = (0..m)
                .map(|j| triple.gen_y_shares[j].value ^ triple.eval_y_shares[j].value)
                .collect();
            for j in 0..m {
                for i in 0..n {
                    let k = j * n + i;
                    let z_full_k = triple.gen_z_shares[k].value ^ triple.eval_z_shares[k].value;
                    let expected = x_full[i] & y_full[j];
                    assert_eq!(
                        z_full_k, expected,
                        "TEST-03 product invariant: z_full[{}] = {} but x_full[{}]({}) & y_full[{}]({}) = {} (seed={}, n={}, m={})",
                        k, z_full_k, i, x_full[i], j, y_full[j], expected, seed, n, m
                    );
                }
            }
        }
    }

    #[test]
    fn test_macro_outputs_xor_invariant() {
        // PROTO-06 regression: the two internal tensor_macro calls are deterministic
        // under a fixed seed, so repeating generate() twice on fresh (but equally-seeded)
        // IdealBCot instances yields bit-identical Z outputs. A change in macro
        // wiring or RNG consumption order breaks this assertion and surfaces as a
        // regression rather than a silent protocol-level anomaly.
        let (n, m) = (4, 4);
        let mut b1 = IdealBCot::new(42, 99);
        let t1 = LeakyTensorPre::new(31, n, m, &mut b1).generate();
        let mut b2 = IdealBCot::new(42, 99);
        let t2 = LeakyTensorPre::new(31, n, m, &mut b2).generate();
        for k in 0..(n * m) {
            assert_eq!(
                t1.gen_z_shares[k].value, t2.gen_z_shares[k].value,
                "PROTO-06 determinism: gen_z[{}].value diverged", k
            );
            assert_eq!(
                t1.eval_z_shares[k].value, t2.eval_z_shares[k].value,
                "PROTO-06 determinism: eval_z[{}].value diverged", k
            );
            assert_eq!(
                t1.gen_z_shares[k].key.as_block(), t2.gen_z_shares[k].key.as_block(),
                "PROTO-06 determinism: gen_z[{}].key diverged", k
            );
            assert_eq!(
                t1.gen_z_shares[k].mac.as_block(), t2.gen_z_shares[k].mac.as_block(),
                "PROTO-06 determinism: gen_z[{}].mac diverged", k
            );
        }
    }

    #[test]
    fn test_d_extraction_and_z_assembly() {
        // PROTO-07: on every (i, j), the final Z share pair satisfies the
        // cross-party IT-MAC invariant — the direct observable consequence of
        // correct D extraction and itmac{Z}{Delta} = itmac{R}{Delta} XOR itmac{D}{Delta}
        // assembly. (This overlaps with TEST-02 but is kept separate for
        // traceability to PROTO-07 in the validation map.)
        let (n, m) = (2, 2);
        let mut bcot = IdealBCot::new(42, 99);
        let triple = LeakyTensorPre::new(41, n, m, &mut bcot).generate();
        for k in 0..(n * m) {
            verify_cross_party(
                &triple.gen_z_shares[k],
                &triple.eval_z_shares[k],
                &triple.delta_a,
                &triple.delta_b,
            );
        }
    }

    #[test]
    fn test_feq_passes_on_honest_run() {
        // PROTO-08: honest execution of generate() invokes feq::check internally
        // and does NOT panic. Any panic here signals a transcript inconsistency
        // (either in macro wiring, C_A/C_B construction, or L_1/L_2 assembly).
        let mut bcot = IdealBCot::new(42, 99);
        let _ = LeakyTensorPre::new(53, 3, 5, &mut bcot).generate();
        // no panic = success
    }

    #[test]
    #[should_panic(expected = "F_eq abort")]
    fn test_f_eq_abort_on_tampered_transcript() {
        // TEST-04 integration: construct a deliberately-inconsistent pair of
        // L-matrices (one bit flipped) and confirm feq::check aborts with the
        // expected message. Complements the unit tests in feq::tests.
        use crate::matrix::BlockMatrix;
        let mut l_1 = BlockMatrix::new(2, 2);
        let mut l_2 = BlockMatrix::new(2, 2);
        // Make them match at (0,0), (0,1), (1,0), differ at (1,1).
        let common = Block::new([0xAA; 16]);
        for j in 0..2 {
            for i in 0..2 {
                l_1[(i, j)] = common;
                l_2[(i, j)] = common;
            }
        }
        l_2[(1, 1)] = Block::new([0x55; 16]); // tamper
        crate::feq::check(&l_1, &l_2);
    }
}
