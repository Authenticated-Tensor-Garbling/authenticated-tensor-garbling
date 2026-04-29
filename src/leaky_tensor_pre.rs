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
    tensor_macro::{chunked_tensor_garbler, chunked_tensor_evaluator},
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
///   gen_*_share.key  = A's sender key from transfer_gb_to_ev   (LSB = 0)
///   gen_*_share.mac  = A's MAC from transfer_ev_to_gb           (MAC of A's bit under Δ_ev)
///   eval_*_share.key = B's sender key from transfer_ev_to_gb   (LSB = 0)
///   eval_*_share.mac = B's MAC from transfer_gb_to_ev           (MAC of B's bit under Δ_gb)
///
/// Never call `share.verify(&delta)` directly on a cross-party share —
/// it panics. Use `verify_cross_party(gen, eval, &Δ_gb, &Δ_ev)` from the
/// test module (preserved verbatim from the pre-rewrite file).
#[derive(Clone)]
pub struct LeakyTriple {
    pub n: usize,
    pub m: usize,
    // Garbler A's view — paper notation x / y / Z.
    pub gb_x_shares: Vec<AuthBitShare>,   // length n
    pub gb_y_shares: Vec<AuthBitShare>,   // length m
    /// length n*m, column-major: index = j*n + i (j = y index, i = x index).
    pub gb_z_shares: Vec<AuthBitShare>,
    // Evaluator B's view.
    pub ev_x_shares: Vec<AuthBitShare>,  // length n
    pub ev_y_shares: Vec<AuthBitShare>,  // length m
    /// length n*m, column-major.
    pub ev_z_shares: Vec<AuthBitShare>,
    // Shared correlation keys for the run (same for every triple in one run_preprocessing).
    pub delta_gb: Delta,
    pub delta_ev: Delta,
}

/// Pi_LeakyTensor preprocessing protocol (Construction 2, Appendix F).
///
/// Borrows `&mut IdealBCot` so every triple produced by a single
/// `run_preprocessing` call shares the same Δ_gb and Δ_ev (required for
/// Phase 5 XOR combining to preserve the MAC invariant).
///
/// `chunking_factor` controls the GGM-tree sub-tiling inside the two
/// `chunked_tensor_garbler` calls in `generate()` — required so
/// preprocessing's GGM expansion stays within memory at production-sized
/// `n` (paper Construction 4 / AUDIT-2.2 B2 / AUDIT-2.3 D7). Must equal
/// the `chunking_factor` that downstream `AuthTensor{Gen, Eval}` consume
/// from `TensorFpre*`; the cross-party parity assertion at AuthTensor
/// construction enforces this once preprocessing's chunked output flows
/// through.
pub struct LeakyTensorPre<'a> {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub(crate) bcot: &'a mut IdealBCot,
    pub(crate) rng: ChaCha12Rng,
}

impl<'a> LeakyTensorPre<'a> {
    pub fn new(seed: u64, n: usize, m: usize, chunking_factor: usize, bcot: &'a mut IdealBCot) -> Self {
        assert!(chunking_factor > 0, "chunking_factor must be at least 1");
        Self {
            n,
            m,
            chunking_factor,
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
        //   transfer_gb_to_ev(&v_b_bits): A is sender (Δ_ev correlation), B picks on v_B.
        //   transfer_ev_to_gb(&v_a_bits): B is sender (Δ_gb correlation), A picks on v_A.
        // This is Pattern 1 in 04-PATTERNS.md.
        let cot_x_gb_to_ev = self.bcot.transfer_gb_to_ev(&x_b_bits);
        let cot_x_ev_to_gb = self.bcot.transfer_ev_to_gb(&x_a_bits);
        let cot_y_gb_to_ev = self.bcot.transfer_gb_to_ev(&y_b_bits);
        let cot_y_ev_to_gb = self.bcot.transfer_ev_to_gb(&y_a_bits);
        let cot_r_gb_to_ev = self.bcot.transfer_gb_to_ev(&r_b_bits);
        let cot_r_ev_to_gb = self.bcot.transfer_ev_to_gb(&r_a_bits);

        // Cross-party AuthBitShare assembly.
        let gb_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
            key:   cot_x_gb_to_ev.sender_keys[i],
            mac:   Mac::new(*cot_x_ev_to_gb.receiver_macs[i].as_block()),
            value: x_a_bits[i],
        }).collect();
        let ev_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
            key:   cot_x_ev_to_gb.sender_keys[i],
            mac:   Mac::new(*cot_x_gb_to_ev.receiver_macs[i].as_block()),
            value: x_b_bits[i],
        }).collect();
        let gb_y_shares: Vec<AuthBitShare> = (0..self.m).map(|j| AuthBitShare {
            key:   cot_y_gb_to_ev.sender_keys[j],
            mac:   Mac::new(*cot_y_ev_to_gb.receiver_macs[j].as_block()),
            value: y_a_bits[j],
        }).collect();
        let ev_y_shares: Vec<AuthBitShare> = (0..self.m).map(|j| AuthBitShare {
            key:   cot_y_ev_to_gb.sender_keys[j],
            mac:   Mac::new(*cot_y_gb_to_ev.receiver_macs[j].as_block()),
            value: y_b_bits[j],
        }).collect();
        let gb_r_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| AuthBitShare {
            key:   cot_r_gb_to_ev.sender_keys[k],
            mac:   Mac::new(*cot_r_ev_to_gb.receiver_macs[k].as_block()),
            value: r_a_bits[k],
        }).collect();
        let ev_r_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| AuthBitShare {
            key:   cot_r_ev_to_gb.sender_keys[k],
            mac:   Mac::new(*cot_r_gb_to_ev.receiver_macs[k].as_block()),
            value: r_b_bits[k],
        }).collect();

        // ========================================================
        // Step 2: Compute C_A, C_B, C_A^(R), C_B^(R) inline (PROTO-05, D-10)
        // ========================================================
        //
        // Per paper Construction 2 (Appendix F, lines 208-216):
        //   C_A[j]    := y_A[j]·Δ_gb ⊕ key(y_B@A)[j]  ⊕ mac(y_A@B)[j]
        //   C_B[j]    := y_B[j]·Δ_ev ⊕ mac(y_B@A)[j]  ⊕ key(y_A@B)[j]
        //   C_A^(R)[k]:= R_A[k]·Δ_gb ⊕ key(R_B@A)[k]  ⊕ mac(R_A@B)[k]
        //   C_B^(R)[k]:= R_B[k]·Δ_ev ⊕ mac(R_B@A)[k]  ⊕ key(R_A@B)[k]
        //
        // Field mapping (Pitfall 3 in RESEARCH.md):
        //   key(y_B@A)[j] = gb_y_shares[j].key     mac(y_A@B)[j] = gb_y_shares[j].mac
        //   mac(y_B@A)[j] = ev_y_shares[j].mac    key(y_A@B)[j] = ev_y_shares[j].key

        let delta_a_block: Block = *self.bcot.delta_gb.as_block();
        let delta_b_block: Block = *self.bcot.delta_ev.as_block();

        let mut c_a: Vec<Block> = Vec::with_capacity(self.m);
        let mut c_b: Vec<Block> = Vec::with_capacity(self.m);
        for j in 0..self.m {
            let y_a_term = if gb_y_shares[j].value  { delta_a_block } else { Block::ZERO };
            let y_b_term = if ev_y_shares[j].value { delta_b_block } else { Block::ZERO };
            c_a.push(
                y_a_term
                    ^ *gb_y_shares[j].key.as_block()
                    ^ *gb_y_shares[j].mac.as_block(),
            );
            c_b.push(
                y_b_term
                    ^ *ev_y_shares[j].mac.as_block()
                    ^ *ev_y_shares[j].key.as_block(),
            );
        }

        let mut c_a_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
        let mut c_b_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for k in 0..(self.n * self.m) {
            let r_a_term = if gb_r_shares[k].value  { delta_a_block } else { Block::ZERO };
            let r_b_term = if ev_r_shares[k].value { delta_b_block } else { Block::ZERO };
            c_a_r.push(
                r_a_term
                    ^ *gb_r_shares[k].key.as_block()
                    ^ *gb_r_shares[k].mac.as_block(),
            );
            c_b_r.push(
                r_b_term
                    ^ *ev_r_shares[k].mac.as_block()
                    ^ *ev_r_shares[k].key.as_block(),
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

        // Macro Call 1: A garbles under Δ_gb, B evaluates.
        //   Keys: cot_x_gb_to_ev.sender_keys (A's keys; LSB=0 by Key invariant).
        //   MACs: cot_x_gb_to_ev.receiver_macs (B's MACs = K[0] XOR x_B * Δ_gb; lsb=x_b since Δ_gb.lsb()=1).
        //   Explicit bits x_b_bits passed to ev for GGM tree navigation.
        // Chunked variant: sub-tile the n-dimension into ⌈n / cf⌉ trees
        // (AUDIT-2.2 B2). Output Z shape unchanged.
        let (z_gb1, g_1) = chunked_tensor_garbler(
            self.n, self.m, self.chunking_factor, self.bcot.delta_gb,
            &cot_x_gb_to_ev.sender_keys,
            &t_a,
        );
        let e_1 = chunked_tensor_evaluator(
            self.n, self.m, self.chunking_factor, &g_1,
            &cot_x_gb_to_ev.receiver_macs,
            &x_b_bits,
            &t_b,
        );

        // Macro Call 2: B garbles under Δ_ev, A evaluates.
        //   Keys: cot_x_ev_to_gb.sender_keys (B's keys; LSB=0 by Key invariant).
        //   MACs: cot_x_ev_to_gb.receiver_macs (A's MACs = K[0] XOR x_A * Δ_ev; lsb != x_a since Δ_ev.lsb()=0).
        //   Explicit bits x_a_bits passed to ev — mandatory since MAC LSB is unreliable.
        let (z_gb2, g_2) = chunked_tensor_garbler(
            self.n, self.m, self.chunking_factor, self.bcot.delta_ev,
            &cot_x_ev_to_gb.sender_keys,
            &t_b,
        );
        let e_2 = chunked_tensor_evaluator(
            self.n, self.m, self.chunking_factor, &g_2,
            &cot_x_ev_to_gb.receiver_macs,
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
        // Paper correctness precondition: lsb(Δ_gb ⊕ Δ_ev) == 1 (enforced by Plan 1
        // via Δ_ev.lsb() == 0 in IdealBCot::new; regression test
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
        // L_1, L_2 are n×m BlockMatrix with L_1 = S_1 ⊕ D·Δ_gb and L_2 = S_2 ⊕ D·Δ_ev.
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
        //   gb_d: holds the bit value AND the Δ_ev MAC mass (so verify_cross_party
        //          step 1 sees: mac = gen_r.mac XOR d*Δ_ev = K_B[0] XOR (r_a XOR d)*Δ_ev ✓)
        //   ev_d: zero key, zero mac, zero value (eval holds no Δ mass for D)
        //
        // Trace: verify_cross_party(gen_z, eval_z, Δ_gb, Δ_ev) step 1:
        //   {key=eval_r.key, mac=gen_r.mac XOR d*Δ_ev, value=r_a XOR d}.verify(Δ_ev)
        //   = K_B[0] XOR (r_a XOR d)*Δ_ev == auth(r_a XOR d, Δ_ev) ✓
        // step 2:
        //   {key=gen_r.key, mac=eval_r.mac, value=r_b}.verify(Δ_gb)
        //   = K_A[0] XOR r_b*Δ_gb == auth(r_b, Δ_gb) ✓
        let gb_z_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| {
            let mac_block = if d_bits[k] { delta_b_block } else { Block::ZERO };
            let gb_d = AuthBitShare {
                key:   Key::default(),
                mac:   Mac::new(mac_block),
                value: d_bits[k],
            };
            gb_r_shares[k] + gb_d
        }).collect();

        // ev side: ev holds no D contribution (D is public; eval_z = eval_r only).
        let ev_z_shares: Vec<AuthBitShare> = ev_r_shares;

        LeakyTriple {
            n: self.n,
            m: self.m,
            gb_x_shares,
            gb_y_shares,
            gb_z_shares,
            ev_x_shares,
            ev_y_shares,
            ev_z_shares,
            delta_gb: self.bcot.delta_gb,
            delta_ev: self.bcot.delta_ev,
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
        let delta_gb = bcot.delta_gb;
        let delta_ev = bcot.delta_ev;
        let triple = LeakyTriple {
            n: 2,
            m: 3,
            gb_x_shares: vec![AuthBitShare::default(); 2],
            gb_y_shares: vec![AuthBitShare::default(); 3],
            gb_z_shares: vec![AuthBitShare::default(); 6],
            ev_x_shares: vec![AuthBitShare::default(); 2],
            ev_y_shares: vec![AuthBitShare::default(); 3],
            ev_z_shares: vec![AuthBitShare::default(); 6],
            delta_gb,
            delta_ev,
        };
        assert_eq!(triple.n, 2);
        assert_eq!(triple.m, 3);
        assert_eq!(triple.gb_x_shares.len(), 2);
        assert_eq!(triple.gb_y_shares.len(), 3);
        assert_eq!(triple.gb_z_shares.len(), 6);
        assert_eq!(triple.ev_x_shares.len(), 2);
        assert_eq!(triple.ev_y_shares.len(), 3);
        assert_eq!(triple.ev_z_shares.len(), 6);
        let _ = &triple.delta_gb;
        let _ = &triple.delta_ev;

        // Plan 3 extension (PROTO-09): real generate() output respects the same shape.
        let mut bcot2 = IdealBCot::new(42, 99);
        let real = LeakyTensorPre::new(7, 2, 3, 2, &mut bcot2).generate();
        assert_eq!(real.n, 2);
        assert_eq!(real.m, 3);
        assert_eq!(real.gb_x_shares.len(), 2);
        assert_eq!(real.gb_y_shares.len(), 3);
        assert_eq!(real.gb_z_shares.len(), 6);
        assert_eq!(real.ev_x_shares.len(), 2);
        assert_eq!(real.ev_y_shares.len(), 3);
        assert_eq!(real.ev_z_shares.len(), 6);
    }

    // ===== Task 3.1: PROTO-04, PROTO-05, PROTO-09 (extended), Key-LSB regression =====

    #[test]
    fn test_correlated_randomness_dimensions() {
        for (seed, n, m) in [(0u64, 1, 1), (1, 4, 4), (2, 8, 3)] {
            let mut bcot = IdealBCot::new(42, 99);
            let triple = LeakyTensorPre::new(seed, n, m, 2, &mut bcot).generate();
            assert_eq!(triple.n, n, "triple.n mismatch at seed={}", seed);
            assert_eq!(triple.m, m, "triple.m mismatch at seed={}", seed);
            assert_eq!(triple.gb_x_shares.len(), n,
                "gb_x_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.ev_x_shares.len(), n,
                "ev_x_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.gb_y_shares.len(), m,
                "gb_y_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.ev_y_shares.len(), m,
                "ev_y_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.gb_z_shares.len(), n * m,
                "gb_z_shares len (seed={}, n={}, m={})", seed, n, m);
            assert_eq!(triple.ev_z_shares.len(), n * m,
                "ev_z_shares len (seed={}, n={}, m={})", seed, n, m);
        }
    }

    #[test]
    fn test_c_a_c_b_xor_invariant() {
        // Paper identity (Construction 2 appendix, lines 262-279):
        //     C_A[j] XOR C_B[j] == y_full[j] * (Delta_gb XOR Delta_ev)
        //
        // C_A and C_B are local to generate(); verify the equivalent cross-party
        // BDOZ identity on the exposed Y shares:
        //     y_dgb.key XOR y_dgb.mac XOR ev_y.key XOR ev_y.mac
        //         == y_full * (Delta_gb XOR Delta_ev)
        //
        // (The Delta_gb term inside C_A (y_A * Delta_gb) and the Delta_ev term
        // inside C_B (y_B * Delta_ev) XOR to (y_A XOR y_B) * (Delta_gb XOR Delta_ev)
        // = y_full * (Delta_gb XOR Delta_ev) exactly when the four cross-party
        // field XORs cancel in the expected pattern. See Pattern 2 in 04-PATTERNS.md.)
        let (n, m) = (4, 4);
        let mut bcot = IdealBCot::new(42, 99);
        let delta_xor_block: Block = *bcot.delta_gb.as_block() ^ *bcot.delta_ev.as_block();
        let triple = LeakyTensorPre::new(5, n, m, 2, &mut bcot).generate();

        for j in 0..m {
            let y_full = triple.gb_y_shares[j].value ^ triple.ev_y_shares[j].value;
            let lhs = *triple.gb_y_shares[j].key.as_block()
                ^ *triple.gb_y_shares[j].mac.as_block()
                ^ *triple.ev_y_shares[j].key.as_block()
                ^ *triple.ev_y_shares[j].mac.as_block();
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
        let triple = LeakyTensorPre::new(9, n, m, 2, &mut bcot).generate();
        let all: [(&str, &Vec<AuthBitShare>); 6] = [
            ("x_dgb",  &triple.gb_x_shares),
            ("ev_x", &triple.ev_x_shares),
            ("y_dgb",  &triple.gb_y_shares),
            ("ev_y", &triple.ev_y_shares),
            ("gen_z",  &triple.gb_z_shares),
            ("eval_z", &triple.ev_z_shares),
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
        // For each share in x, y, Z: mac_A == key_A XOR bit_A * Delta_ev (verified by
        // verify_cross_party). A failure on z_shares signals Plan 2's A1
        // convention for itmac{D}{Delta} needs to be swapped (see Plan 2 Step 5 doc).
        let (n, m) = (4, 4);
        let mut bcot = IdealBCot::new(42, 99);
        let triple = LeakyTensorPre::new(17, n, m, 2, &mut bcot).generate();
        for i in 0..n {
            verify_cross_party(
                &triple.gb_x_shares[i],
                &triple.ev_x_shares[i],
                &triple.delta_gb,
                &triple.delta_ev,
            );
        }
        for j in 0..m {
            verify_cross_party(
                &triple.gb_y_shares[j],
                &triple.ev_y_shares[j],
                &triple.delta_gb,
                &triple.delta_ev,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &triple.gb_z_shares[k],
                &triple.ev_z_shares[k],
                &triple.delta_gb,
                &triple.delta_ev,
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
            let triple = LeakyTensorPre::new(seed, n, m, 2, &mut bcot).generate();
            let x_full: Vec<bool> = (0..n)
                .map(|i| triple.gb_x_shares[i].value ^ triple.ev_x_shares[i].value)
                .collect();
            let y_full: Vec<bool> = (0..m)
                .map(|j| triple.gb_y_shares[j].value ^ triple.ev_y_shares[j].value)
                .collect();
            for j in 0..m {
                for i in 0..n {
                    let k = j * n + i;
                    let z_full_k = triple.gb_z_shares[k].value ^ triple.ev_z_shares[k].value;
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
        let t1 = LeakyTensorPre::new(31, n, m, 2, &mut b1).generate();
        let mut b2 = IdealBCot::new(42, 99);
        let t2 = LeakyTensorPre::new(31, n, m, 2, &mut b2).generate();
        for k in 0..(n * m) {
            assert_eq!(
                t1.gb_z_shares[k].value, t2.gb_z_shares[k].value,
                "PROTO-06 determinism: gen_z[{}].value diverged", k
            );
            assert_eq!(
                t1.ev_z_shares[k].value, t2.ev_z_shares[k].value,
                "PROTO-06 determinism: eval_z[{}].value diverged", k
            );
            assert_eq!(
                t1.gb_z_shares[k].key.as_block(), t2.gb_z_shares[k].key.as_block(),
                "PROTO-06 determinism: gen_z[{}].key diverged", k
            );
            assert_eq!(
                t1.gb_z_shares[k].mac.as_block(), t2.gb_z_shares[k].mac.as_block(),
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
        let triple = LeakyTensorPre::new(41, n, m, 2, &mut bcot).generate();
        for k in 0..(n * m) {
            verify_cross_party(
                &triple.gb_z_shares[k],
                &triple.ev_z_shares[k],
                &triple.delta_gb,
                &triple.delta_ev,
            );
        }
    }

    #[test]
    fn test_feq_passes_on_honest_run() {
        // PROTO-08: honest execution of generate() invokes feq::check internally
        // and does NOT panic. Any panic here signals a transcript inconsistency
        // (either in macro wiring, C_A/C_B construction, or L_1/L_2 assembly).
        let mut bcot = IdealBCot::new(42, 99);
        let _ = LeakyTensorPre::new(53, 3, 5, 2, &mut bcot).generate();
        // no panic = success
    }

    #[test]
    fn test_chunking_factor_varied_invariant() {
        // AUDIT-2.2 B2 / Phase 2.6.1: vary `chunking_factor` over values that
        // trigger 1, 2, 4, and ⌈n/cf⌉=non-power-of-2 chunks. The product
        // invariant z_full = x_full ⊗ y_full must hold regardless of cf.
        // This catches: (a) cf-vs-non-cf path divergence, (b) non-uniform
        // last-chunk size (n=8, cf=3 → chunks of 3, 3, 2), (c) cf > n
        // (n=4, cf=8 → single chunk smaller than cf).
        for (seed, n, m, cf) in [
            (101u64, 4, 4, 1),  // cf=1: maximal chunking, n chunks of 1 leaf
            (102,    8, 3, 2),  // cf=2: 4 chunks of 2 leaves
            (103,    8, 3, 4),  // cf=4: 2 chunks of 4 leaves
            (104,    8, 3, 3),  // non-divisor: chunks of 3, 3, 2
            (105,    4, 8, 8),  // cf > n: single chunk of n leaves
        ] {
            let mut bcot = IdealBCot::new(42, 99);
            let triple = LeakyTensorPre::new(seed, n, m, cf, &mut bcot).generate();

            let x_full: Vec<bool> = (0..n)
                .map(|i| triple.gb_x_shares[i].value ^ triple.ev_x_shares[i].value)
                .collect();
            let y_full: Vec<bool> = (0..m)
                .map(|j| triple.gb_y_shares[j].value ^ triple.ev_y_shares[j].value)
                .collect();

            for j in 0..m {
                for i in 0..n {
                    let k = j * n + i;
                    let z_full = triple.gb_z_shares[k].value ^ triple.ev_z_shares[k].value;
                    let expected = x_full[i] & y_full[j];
                    assert_eq!(
                        z_full, expected,
                        "chunked LeakyTensor product invariant violated at (i={}, j={}) for n={}, m={}, cf={}, seed={}",
                        i, j, n, m, cf, seed,
                    );
                }
            }
        }
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
