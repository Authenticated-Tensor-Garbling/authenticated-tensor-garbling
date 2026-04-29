pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;
pub mod sharing;

pub mod matrix;

pub mod aes;

pub mod tensor_pre;
pub mod tensor_gen;
pub mod tensor_eval;
pub mod tensor_ops;
pub mod tensor_macro;

pub mod auth_tensor_fpre;
pub mod auth_tensor_gen;
pub mod auth_tensor_eval;
pub mod input_encoding;

pub mod bcot;
pub mod feq;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;
pub mod preprocessing;
pub mod online;

use crate::block::Block;
use crate::auth_tensor_gen::AuthTensorGen;
use crate::auth_tensor_eval::AuthTensorEval;

/// κ — computational security parameter (in bits). Determines `Block` width
/// and all κ-bit cipher / hash output widths.
pub const CSP: usize = 128;
/// ρ — statistical security parameter (in bits). Bench accounting reads this
/// via `RHO_BYTES = (SSP + 7) / 8` so reported communication and the network
/// simulator's transit time track the paper's κ + ρ leaf-ciphertext width.
pub const SSP: usize = 40;

/// Paper-faithful Protocol 1 input-wire CheckZero assembly under `delta_b`.
///
/// Implements the Protocol 1 consistency check per `5_online.tex` §240–246
/// (and `6_total.tex` §215–222 for P2; same formula). For each tensor-gate
/// input wire, each party computes its share-block of:
///
/// ```text
///   [e_a[i] D_ev] := [a[i] D_ev] ⊕ [l_a[i] D_ev] ⊕ (a[i] ⊕ l_a[i]) D_ev
///   [e_b[j] D_ev] := [b[j] D_ev] ⊕ [l_b[j] D_ev] ⊕ (b[j] ⊕ l_b[j]) D_ev
/// ```
///
/// For honest parties this reduces to zero by Lemma `lem:protocol1-correctness`
/// (line 297) — i.e., `gen_block[k] == eval_block[k]` per index. CheckZero
/// (paper line 246) detects deviation: a malicious garbler that lies about
/// `[v D_ev]^gb` makes the per-index pair unequal.
///
/// Returns `(gen_blocks, eval_blocks)` — each length `n + m`, layout
/// `[e_a[0..n], e_b[0..m]]`. Pass to `block_check_zero` (full-block equality)
/// or hash each side via `block_hash_check_zero` for the paper-faithful
/// `H({V_w})` digest semantics.
///
/// # SIMULATION ONLY
///
/// Takes both parties' state in-process; in a real two-party run each party
/// would compute its own block vector locally from its own `_eval` fields and
/// `[v D_ev]` shares, then exchange digests. To make the simulation sensitive
/// to a malicious garbler that lies about `[v_a D_ev]^gb`, the helper accepts
/// `[v_a D_ev]^gb` as an explicit parameter rather than aliasing it to
/// `gb.alpha_eval[i]`. Honest callers pass `gb.alpha_eval.clone()`; negative
/// tests pass tampered Blocks.
///
/// # Detection power vs the prior `assemble_e_input_wire_shares_p1`
///
/// The prior helper extracted `combined_block.lsb()` and emitted
/// `Vec<AuthBitShare>` for `check_zero` consumption — detection was LSB-only
/// (caught only tampers whose XOR delta has LSB=1). This helper emits the
/// full per-party blocks so `block_check_zero` can detect any non-zero
/// combined block. Aligns with paper §246 (`H({V_w})` digest comparison).
///
/// # Inputs (unchanged from prior helper)
/// - `n`, `m`: input vector lengths.
/// - `gb_v_alpha_eval` / `ev_v_alpha_eval`: `[v_a D_ev]` shares (length n).
///   Honest: gb's = `gb.alpha_eval`; ev's = `ev.alpha_eval[i] ⊕ L_a·δ_b`.
/// - `gb_v_beta_eval` / `ev_v_beta_eval`: same for β (length m).
/// - `l_alpha_pub` / `l_beta_pub`: announced masked-input vectors
///   `vec a ⊕ vec l_a`, `vec b ⊕ vec l_b`.
/// - `gb`, `ev`: party state for `_eval` Block fields.
#[allow(clippy::too_many_arguments)]
pub fn assemble_e_input_wire_blocks_p1(
    n: usize,
    m: usize,
    gb_v_alpha_eval: &[Block],
    ev_v_alpha_eval: &[Block],
    gb_v_beta_eval: &[Block],
    ev_v_beta_eval: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> (Vec<Block>, Vec<Block>) {
    assert_eq!(gb_v_alpha_eval.len(), n);
    assert_eq!(ev_v_alpha_eval.len(), n);
    assert_eq!(gb_v_beta_eval.len(),  m);
    assert_eq!(ev_v_beta_eval.len(),  m);
    assert_eq!(l_alpha_pub.len(), n);
    assert_eq!(l_beta_pub.len(),  m);
    assert_eq!(gb.alpha_eval.len(), n);
    assert_eq!(gb.beta_eval.len(),  m);
    assert_eq!(ev.alpha_eval.len(), n);
    assert_eq!(ev.beta_eval.len(),  m);

    let mut gen_blocks: Vec<Block> = Vec::with_capacity(n + m);
    let mut eval_blocks: Vec<Block> = Vec::with_capacity(n + m);

    // e_a per α-input wire: paper §242
    //   gb's share-block = [v_a D_ev]^gb ⊕ [l_a D_ev]^gb
    //   ev's share-block = [v_a D_ev]^ev ⊕ [l_a D_ev]^ev ⊕ L_a·D_ev
    for i in 0..n {
        let l_a_correction = if l_alpha_pub[i] {
            *ev.delta_b.as_block()
        } else {
            Block::default()
        };
        gen_blocks.push(gb_v_alpha_eval[i] ^ gb.alpha_eval[i]);
        eval_blocks.push(ev_v_alpha_eval[i] ^ ev.alpha_eval[i] ^ l_a_correction);
    }

    // e_b per β-input wire: symmetric.
    for j in 0..m {
        let l_b_correction = if l_beta_pub[j] {
            *ev.delta_b.as_block()
        } else {
            Block::default()
        };
        gen_blocks.push(gb_v_beta_eval[j] ^ gb.beta_eval[j]);
        eval_blocks.push(ev_v_beta_eval[j] ^ ev.beta_eval[j] ^ l_b_correction);
    }

    (gen_blocks, eval_blocks)
}

/// Paper-faithful Protocol 2 input-wire CheckZero assembly — alias for the P1
/// routine.
///
/// Per `6_total.tex` §215–222, the P2 consistency check builds:
/// ```text
///   [c_α D_ev] := [v_α D_ev] ⊕ [l_α D_ev] ⊕ L_α · D_ev    (length n)
///   [c_β D_ev] := [v_β D_ev] ⊕ [l_β D_ev] ⊕ L_β · D_ev    (length m)
/// ```
/// Algebraically identical to P1's `e_a / e_b`. The paper uses different
/// variable names (`c_α/c_β` in P2 vs `e_a/e_b` in P1) to match its narrative;
/// this thin alias preserves the paper-mapped name at P2 call sites without
/// duplicating logic.
///
/// AUDIT-2.3 C2 — alias coupling note: the P1/P2 algebraic equivalence holds
/// only because both protocols assemble the same three-term XOR over the same
/// `[v D_ev]` / `[l D_ev]` / `L · D_ev` shares. If P2's input-encoding spec
/// ever diverges from P1's (e.g., different L-vector semantics, additional
/// per-wire correction term), this alias must be split — silent breakage is
/// the failure mode since the static signatures stay identical. Keep this
/// note adjacent to any future P2 input-encoding edit.
#[allow(clippy::too_many_arguments)]
pub fn assemble_c_alpha_beta_blocks_p2(
    n: usize,
    m: usize,
    gb_v_alpha_eval: &[Block],
    ev_v_alpha_eval: &[Block],
    gb_v_beta_eval: &[Block],
    ev_v_beta_eval: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> (Vec<Block>, Vec<Block>) {
    assemble_e_input_wire_blocks_p1(
        n, m,
        gb_v_alpha_eval,
        ev_v_alpha_eval,
        gb_v_beta_eval,
        ev_v_beta_eval,
        l_alpha_pub,
        l_beta_pub,
        gb,
        ev,
    )
}

#[cfg(test)]
mod tests {

    use crate::delta::Delta;

    use crate::{
        tensor_gen::TensorProductGen,
        tensor_eval::TensorProductEval,
        matrix::BlockMatrix,
        block::Block
    };

    fn verify_vector_sharing(
        clear_val: usize,
        gb_share: &Vec<Block>,
        ev_share: &Vec<Block>,
        delta: &Delta,
        n: usize
    ) -> bool {
        assert_eq!(gb_share.len(), n);
        assert_eq!(gb_share.len(), ev_share.len());
        for i in 0..gb_share.len() {
            let expected_val = ((clear_val>>i)&1) != 0;
            if expected_val {
                if gb_share[i] != ev_share[i] ^ delta.as_block() {
                    return false;
                }
            } else {
                if gb_share[i] != ev_share[i] {
                    return false;
                }
            }
        }
        true
    }

    fn verify_column_matrix_sharing(
        clear_val: usize,
        gb_share: &BlockMatrix,
        ev_share: &BlockMatrix,
        delta: &Delta,
        n: usize,
    ) -> bool {
        assert_eq!(gb_share.rows(), n);
        assert_eq!(gb_share.rows(), ev_share.rows());
        for i in 0..gb_share.rows() {
            let expected_val = ((clear_val>>i)&1) != 0;
            if expected_val {
                if gb_share[i] != ev_share[i] ^ delta.as_block() {
                    return false;
                }
            } else {
                if gb_share[i] != ev_share[i] {
                    return false;
                }
            }
        }
        true
    }

    fn verify_tensor_output(
        clear_x: usize,
        clear_y: usize,
        n: usize,
        m: usize,
        gb_out: &BlockMatrix,
        ev_out: &BlockMatrix,
        delta: &Delta,
    ) -> bool {
        for i in 0..n {
            for k in 0..m {
                let expected_val = (((clear_x>>i)&1) & ((clear_y>>k)&1)) != 0;
                if expected_val {
                    if gb_out[(i, k)] != ev_out[(i, k)] ^ delta.as_block() {
                        return false;
                    }
                } else {
                    if gb_out[(i, k)] != ev_out[(i, k)] {
                        return false;
                    }
                }
            }
        }
        true
    }

    use crate::tensor_pre::SemiHonestTensorPre;
    
    #[test]
    fn test_semihonest_tensor_product() {
        
        let mut rng = rand::rng();
        let delta = Delta::random(&mut rng);

        let n = 2;
        let m = 3;
        let clear_x = 0b01;
        let clear_y = 0b101;

        let mut pre = SemiHonestTensorPre::new_with_delta(3, n, m, 6, delta);
        pre.gen_inputs(clear_x, clear_y);

        assert!(
            verify_vector_sharing(clear_x, &pre.x_labels.iter().map(|share| share.gen_share).collect(), &pre.x_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(clear_y, &pre.y_labels.iter().map(|share| share.gen_share).collect(), &pre.y_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let (alpha, beta) = pre.gen_masks();


        assert!(
            verify_vector_sharing(alpha, &pre.alpha_labels.iter().map(|share| share.gen_share).collect(), &pre.alpha_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(beta, &pre.beta_labels.iter().map(|share| share.gen_share).collect(), &pre.beta_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let (masked_x, masked_y) = pre.mask_inputs();
        
        
        assert!(
            verify_vector_sharing(masked_x, &pre.x_labels.iter().map(|share| share.gen_share).collect(), &pre.x_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(masked_y, &pre.y_labels.iter().map(|share| share.gen_share).collect(), &pre.y_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let n_bitmask = (1<<n)-1;
        let m_bitmask = (1<<m)-1;

        assert_eq!(masked_x, (clear_x ^ alpha) & n_bitmask);
        assert_eq!(masked_y, (clear_y ^ beta) & m_bitmask);


        let (pre_gen, pre_eval) = pre.into_gen_eval();

        let mut gb = TensorProductGen::new_from_fpre_gen(pre_gen);
        let mut ev = TensorProductEval::new_from_fpre_eval(pre_eval);

        assert!(
            verify_vector_sharing(masked_x, &gb.x_labels, &ev.x_labels, &delta, n)
        );
        assert!(
            verify_vector_sharing(masked_y, &gb.y_labels, &ev.y_labels, &delta, m)
        );


        //check the inputs to the first half outer product: masked_x (x) beta
        let (gen_x, gen_y) = gb.get_first_inputs();
        let (eval_x, eval_y) = ev.get_first_inputs();

        assert!(
            verify_column_matrix_sharing(masked_x & n_bitmask, &gen_x, &eval_x, &delta, n)
        );
        assert!(
            verify_column_matrix_sharing(clear_y & m_bitmask, &gen_y, &eval_y, &delta, m)
        );


        let (gen_levels, gen_cts) = gb.garble_first_half_outer_product();
        ev.evaluate_first_half_outer_product(gen_levels, gen_cts);

        assert!(
            verify_tensor_output(masked_x & n_bitmask, clear_y & m_bitmask, n, m, &gb.first_half_out, &ev.first_half_out, &delta)
        );


        let (gen_x, gen_y) = gb.get_second_inputs();
        let (eval_x, eval_y) = ev.get_second_inputs();

        assert!(
            verify_column_matrix_sharing(masked_y & m_bitmask, &gen_x, &eval_x, &delta, m)
        );
        assert!(
            verify_column_matrix_sharing(alpha & n_bitmask, &gen_y, &eval_y, &delta, n)
        );


        // second half outer product: (y ^ beta) (x) alpha
        let (gen_levels, gen_cts) = gb.garble_second_half_outer_product();
        ev.evaluate_second_half_outer_product(gen_levels, gen_cts);

        // check that first_out has the correct value
        assert!(
            verify_tensor_output(masked_y & m_bitmask, alpha & n_bitmask, m, n, &gb.second_half_out, &ev.second_half_out, &delta)
        );

        
        // final outer product
        let gen_result = gb.garble_final_outer_product();
        let eval_result = ev.evaluate_final_outer_product();

        // check that final_out has the correct value
        assert!(
            verify_tensor_output(clear_x, clear_y, n, m, &gen_result, &eval_result, &delta)
        );
    }

    use crate::auth_tensor_fpre::TensorFpre;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::input_encoding::encode_inputs;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing, UncompressedPreprocessingBackend};
    use crate::online::block_check_zero;
    use super::{
        assemble_e_input_wire_blocks_p1,
        assemble_c_alpha_beta_blocks_p2,
    };

    #[test]
    fn test_auth_tensor_product() {
        let mut rng = rand::rng();
        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random_b(&mut rng);

        let n = 16;
        let m = 16;

        let input_x = 0b101;
        let input_y = 0b110;

        let mut fpre = TensorFpre::new_with_delta(54, n, m, 8, delta_a, delta_b);
        fpre.generate_ideal();

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: input encoding phase. Populates gb/ev x_gen/y_gen,
        // masked_x_gen/masked_y_gen, and cleartext masked-bit vectors
        // (gar.masked_*_bits = 0-vec; ev.masked_*_bits = d-vector) per the
        // GGM-tree convention. get_first_inputs / evaluate_first_half / etc.
        // read from these instead of the preprocessing-faked x_labels.
        encode_inputs(&mut gb, &mut ev, input_x, input_y, &mut rng);

        // Reconstruct cleartext bitfields from input-encoding output state.
        // `ev.masked_x_bits` is the cleartext d_x vector (gen-side is 0-vec).
        // masked_x := x ⊕ α  ⇒  α = x ⊕ masked_x. Same for β/y.
        let masked_x: usize = ev.masked_x_bits.iter().enumerate()
            .map(|(i, &b)| (b as usize) << i).sum();
        let masked_y: usize = ev.masked_y_bits.iter().enumerate()
            .map(|(j, &b)| (b as usize) << j).sum();
        let alpha = input_x ^ masked_x;
        let beta = input_y ^ masked_y;

        // check the inputs to the first half outer product: masked_x (x) beta
        let (gen_x, gen_y) = gb.get_first_inputs();
        let (eval_x, eval_y) = ev.get_first_inputs();

        assert!(
            verify_column_matrix_sharing(masked_x, &gen_x, &eval_x, &delta_a, n)
        );
        assert!(
            verify_column_matrix_sharing(beta, &gen_y, &eval_y, &delta_a, m)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_first_half();
        ev.evaluate_first_half(gen_chunk_levels, gen_chunk_cts);

        // check that first_out has the correct value
        // first_out should be masked_x (tensor) beta  (paper: (a⊕λ_a) ⊗ λ_b)
        assert!(
            verify_tensor_output(masked_x, beta, n, m, &gb.first_half_out, &ev.first_half_out, &delta_a)
        );


        // check the inputs to the second half outer product: masked_y (x) input_x
        let (gen_x, gen_y) = gb.get_second_inputs();
        let (eval_x, eval_y) = ev.get_second_inputs();

        assert!(
            verify_column_matrix_sharing(masked_y, &gen_x, &eval_x, &delta_a, m)
        );
        assert!(
            verify_column_matrix_sharing(input_x, &gen_y, &eval_y, &delta_a, n)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_second_half();
        ev.evaluate_second_half(gen_chunk_levels, gen_chunk_cts);

        // check that second_out has the correct value
        // second_out should be masked_y (tensor) input_x  (paper: (b⊕λ_b) ⊗ a)
        assert!(
            verify_tensor_output(masked_y, input_x, m, n, &gb.second_half_out, &ev.second_half_out, &delta_a)
        );

        // check that final_out has the correct value
        for i in 0..n {
            for j in 0..m {
                let expected_val = (((alpha>>i)&1) & ((beta>>j)&1)) != 0;
                let gb_share = gb.correlated_gen[j * n + i];
                let ev_share = ev.correlated_gen[j * n + i];

                if expected_val {
                    assert_eq!(gb_share, ev_share ^ delta_a.as_block(), "At position ({},{}): gb_out should equal ev_out ^ delta when expected_val=1", i, j);
                } else {
                    assert_eq!(gb_share, ev_share, "At position ({},{}): gb_out should equal ev_out when expected_val=0", i, j);
                }
            }
        }

        gb.garble_final();
        ev.evaluate_final();

        // check each element for correctness
        for i in 0..n {
            let x_bit = ((input_x >> i) & 1) != 0;
            for j in 0..m {
                let y_bit = ((input_y >> j) & 1) != 0;
                let expected_val = x_bit & y_bit;
                print!("{} ", expected_val);

                let gb_val = gb.first_half_out[(i, j)];
                let ev_val = ev.first_half_out[(i, j)];

                if expected_val {
                    assert_eq!(gb_val, ev_val ^ delta_a.as_block(), "At position ({},{}): gb_out should equal ev_out ^ delta when expected_val=1", i, j);
                } else {
                    assert_eq!(gb_val, ev_val, "At position ({},{}): gb_out should equal ev_out when expected_val=0", i, j);
                }
            }
        }
    }

    /// Body of the paper-faithful Protocol 1 honest-run test, parameterized by
    /// preprocessing backend. Both `IdealPreprocessingBackend` and
    /// `UncompressedPreprocessingBackend` must satisfy the same CheckZero
    /// invariant — preprocessing must populate all four `*_eval_shares` fields
    /// with shares that XOR to `bit · delta_b`.
    ///
    /// Per `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex`
    /// lines 226–247 (CheckZero) and 211–217 (input encoding), the consistency check
    /// builds e_a, e_b shares on tensor-gate INPUT wires under D_ev (delta_b), with
    /// the formula `e = v ⊕ l ⊕ L`. For honest input-encoded shares this reconstructs
    /// to zero by paper Lemma `lem:protocol1-correctness` (line 297).
    ///
    /// With both backends' internal input = (0, 0), masked_x = alpha and
    /// masked_y = beta — i.e. v_alpha = v_beta = 0.
    fn run_full_protocol_1(backend: &dyn TensorPreprocessing, x: usize, y: usize) {
        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = backend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        let mut prep_rng = rand::rng();
        encode_inputs(&mut gb, &mut ev, x, y, &mut prep_rng);

        // Standard Protocol 1 garble + evaluate sequence.
        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        // Verify cleartext tensor-product output: post-garble_final
        // first_half_out per (i, j) must reconstruct to `(x_i AND y_j) · δ_a`.
        // This is the end-to-end correctness check on the gate output.
        assert!(
            verify_tensor_output(x, y, n, m, &gb.first_half_out, &ev.first_half_out, &gb.delta_a),
            "P1 garble_final must produce shares that reconstruct to x⊗y · δ_a"
        );

        // Reconstruct masked input values (paper L_a, L_b) from joint state. In the
        // single-gate test the gate inputs are circuit inputs, so L_a = l_a and
        // L_b = l_b (since v=0 with x=y=0 input).
        // L_α / L_β are the cleartext masked input vectors `vec a ⊕ vec λ_a` /
        // `vec b ⊕ vec λ_b` (paper `5_online.tex` §242, `6_total.tex` §218).
        // After encode_inputs(x, y), `ev.masked_x_bits[i] = x_i ⊕ α_i` (and
        // gen-side is the 0-vec by the asymmetric sharing); with x = y = 0 in
        // these tests this equals λ_α / λ_β exactly.
        let l_alpha_pub: Vec<bool> = ev.masked_x_bits.clone();
        let l_beta_pub:  Vec<bool> = ev.masked_y_bits.clone();

        // Honest input-encoding (paper line 214):
        //   gb sets [v_a D_ev]^gb := [l_a D_ev]^gb
        //   ev sets [v_a D_ev]^ev := [l_a D_ev]^ev XOR L_a · D_ev
        let gb_v_alpha_eval: Vec<Block> = gb.alpha_eval.clone();
        let ev_v_alpha_eval: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_eval[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_eval[i]
            })
            .collect();
        let gb_v_beta_eval: Vec<Block> = gb.beta_eval.clone();
        let ev_v_beta_eval: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_eval[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_eval[j]
            })
            .collect();

        let (e_gen_blocks, e_eval_blocks) = assemble_e_input_wire_blocks_p1(
            n, m,
            &gb_v_alpha_eval,
            &ev_v_alpha_eval,
            &gb_v_beta_eval,
            &ev_v_beta_eval,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        assert_eq!(e_gen_blocks.len(), n + m);
        assert_eq!(e_eval_blocks.len(), n + m);

        // Paper-faithful CheckZero: full-block per-index equality under D_ev,
        // per 5_online.tex §246. Honest parties' share-blocks satisfy
        // gen_block[k] == eval_block[k] (their XOR is bit·δ_b with bit=0).
        assert!(
            block_check_zero(&e_gen_blocks, &e_eval_blocks),
            "honest Protocol 1 run must pass paper-faithful CheckZero under D_ev"
        );
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_1_ideal() {
        run_full_protocol_1(&IdealPreprocessingBackend, 0, 0);
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_1_uncompressed() {
        run_full_protocol_1(&UncompressedPreprocessingBackend, 0, 0);
    }

    /// End-to-end Protocol 1 with NON-ZERO inputs:
    ///   - encode_inputs(gb, ev, x=0b1011, y=0b101)
    ///   - full garble + evaluate sequence
    ///   - verify_tensor_output asserts post-garble_final shares reconstruct to (x⊗y)·δ_a
    ///   - paper-faithful CheckZero on input wires under D_ev
    ///
    /// Closes the coverage gap surfaced by the 1.2(h) audit: prior
    /// `run_full_protocol_*` only exercised x=y=0 (where masked_x = α, masked_y = β
    /// trivially). This stresses the input-encoding path, the garble decomposition,
    /// and CheckZero with non-trivial values where v_α, v_β, v_γ are not identically
    /// zero. Backend-parameterized so both Ideal and Uncompressed are exercised.
    #[test]
    fn test_full_protocol_1_nonzero_inputs_ideal() {
        run_full_protocol_1(&IdealPreprocessingBackend, 0b1011, 0b101);
    }

    #[test]
    fn test_full_protocol_1_nonzero_inputs_uncompressed() {
        run_full_protocol_1(&UncompressedPreprocessingBackend, 0b1011, 0b101);
    }

    /// Body of the Protocol 2 honest-run test, parameterized by preprocessing
    /// backend. Mirrors `run_full_protocol_1` but exercises the `_p2` garble
    /// path and the paper-faithful α/β input-wire check
    /// (`assemble_c_alpha_beta_shares_p2`) per `6_total.tex` lines 207–214.
    ///
    /// Both backends must:
    ///   - reconstruct D_gb output blocks matching the correct tensor product
    ///   - pass the D_ev consistency check (c_α, c_β == 0) under ev.delta_b
    ///
    /// `garble_final_p2`'s return type — `(Vec<Block>, Vec<Block>)` with no
    /// `bool` / `Vec<bool>` — statically enforces that the garbler never sends
    /// a masked wire value.
    fn run_full_protocol_2(backend: &dyn TensorPreprocessing, x: usize, y: usize) {
        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = backend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        let mut prep_rng = rand::rng();
        encode_inputs(&mut gb, &mut ev, x, y, &mut prep_rng);

        // Protocol 2 garble + evaluate sequence (wide ciphertexts).
        let (cl1, ct1) = gb.garble_first_half_p2();
        ev.evaluate_first_half_p2(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half_p2();
        ev.evaluate_second_half_p2(cl2, ct2);
        let (gb_d_gb_out, gb_d_ev_out) = gb.garble_final_p2();
        let ev_d_ev_out = ev.evaluate_final_p2();

        assert_eq!(gb_d_gb_out.len(), n * m);
        assert_eq!(gb_d_ev_out.len(), n * m);
        assert_eq!(ev_d_ev_out.len(), n * m);

        // PART A: D_gb correctness via verify_tensor_output. Post-garble_final_p2,
        // gb_d_gb_out[j*n+i] equals gb.first_half_out[(i, j)] (column-major layout)
        // and ev.first_half_out[(i, j)] holds eval's matching share. Combined under
        // δ_a, they reconstruct to (x ⊗ y) · δ_a per position.
        assert!(
            verify_tensor_output(x, y, n, m, &gb.first_half_out, &ev.first_half_out, &gb.delta_a),
            "P2 garble_final_p2 must produce shares that reconstruct to x⊗y · δ_a"
        );

        // AUDIT-2.4 D2: symmetric D_ev correctness. The wide-leaf `_ev`
        // accumulators (`first_half_out_ev` / `second_half_out_ev`) are folded
        // into `first_half_out_ev` by `garble_final_p2` (paired-with-`evaluate_final_p2`
        // on eval's side). Combined under δ_b, they must reconstruct to
        // (x ⊗ y) · δ_b — the headline P2 invariant on the D_ev path that
        // distinguishes Protocol 2 from Protocol 1. Closes the SCAFFOLDING-
        // flagged D_ev output coverage gap. (`gb` has no `delta_b` field;
        // `ev.delta_b` is the simulation reach into eval's state — same
        // pattern used at lines 695-708 below for `c_α` / `c_β` assembly.)
        assert!(
            verify_tensor_output(x, y, n, m, &gb.first_half_out_ev, &ev.first_half_out_ev, &ev.delta_b),
            "P2 garble_final_p2 must produce D_ev shares that reconstruct to x⊗y · δ_b"
        );

        // ===========================================================================
        // PART B: P2 consistency check — c_α / c_β assembled under delta_b
        // pass `check_zero`. Mirrors `run_full_protocol_1`'s input-encoding
        // setup: the paper's P1 e_a/e_b (5_online.tex:242–246) and P2 c_α/c_β
        // (6_total.tex:207–214) are algebraically identical — three-term XOR
        // [v D_ev] ⊕ [λ D_ev] ⊕ L·D_ev on tensor-gate input wires.
        // ===========================================================================
        // L_α / L_β are the cleartext masked input vectors `vec a ⊕ vec λ_a` /
        // `vec b ⊕ vec λ_b` (paper `5_online.tex` §242, `6_total.tex` §218).
        // After encode_inputs(x, y), `ev.masked_x_bits[i] = x_i ⊕ α_i` (and
        // gen-side is the 0-vec by the asymmetric sharing); with x = y = 0 in
        // these tests this equals λ_α / λ_β exactly.
        let l_alpha_pub: Vec<bool> = ev.masked_x_bits.clone();
        let l_beta_pub:  Vec<bool> = ev.masked_y_bits.clone();

        // Honest input encoding (6_total.tex:191–198):
        //   gb sets [v D_ev]^gb := [l D_ev]^gb
        //   ev sets [v D_ev]^ev := [l D_ev]^ev XOR L · D_ev
        let gb_v_alpha_eval: Vec<Block> = gb.alpha_eval.clone();
        let ev_v_alpha_eval: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_eval[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_eval[i]
            })
            .collect();
        let gb_v_beta_eval: Vec<Block> = gb.beta_eval.clone();
        let ev_v_beta_eval: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_eval[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_eval[j]
            })
            .collect();

        let (c_gen_blocks_p2, c_eval_blocks_p2) = assemble_c_alpha_beta_blocks_p2(
            n, m,
            &gb_v_alpha_eval,
            &ev_v_alpha_eval,
            &gb_v_beta_eval,
            &ev_v_beta_eval,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        assert_eq!(c_gen_blocks_p2.len(), n + m);
        assert_eq!(c_eval_blocks_p2.len(), n + m);

        // Honest CheckZero under delta_b per 6_total.tex §222.
        assert!(
            block_check_zero(&c_gen_blocks_p2, &c_eval_blocks_p2),
            "honest Protocol 2 run must pass block_check_zero on c_α/c_β under D_ev (delta_b)"
        );
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_2_ideal() {
        run_full_protocol_2(&IdealPreprocessingBackend, 0, 0);
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_2_uncompressed() {
        run_full_protocol_2(&UncompressedPreprocessingBackend, 0, 0);
    }

    /// End-to-end Protocol 2 with NON-ZERO inputs. Mirrors
    /// `test_full_protocol_1_nonzero_inputs_*` but exercises the wide-ciphertext
    /// `_p2` garble path. Closes the same coverage gap surfaced by the 1.2(h)
    /// audit.
    #[test]
    fn test_full_protocol_2_nonzero_inputs_ideal() {
        run_full_protocol_2(&IdealPreprocessingBackend, 0b1011, 0b101);
    }

    #[test]
    fn test_full_protocol_2_nonzero_inputs_uncompressed() {
        run_full_protocol_2(&UncompressedPreprocessingBackend, 0b1011, 0b101);
    }

    #[test]
    fn test_protocol_1_e_input_wire_check_aborts_on_garbler_d_ev_tamper() {
        // Paper-faithful P1 CheckZero must abort when the garbler lies about its
        // [v_a D_ev]^gb during input encoding (5_online.tex §214). We model this
        // by passing a tampered `gb_v_alpha_eval[0]` to the helper.
        //
        // The XOR is `gb.delta_a.as_block()`, whose LSB is 1 — so the tamper leaks
        // into `combined_e_block.lsb()` and breaks the e_a_bit = 0 invariant,
        // causing `block_check_zero` under δ_b to return false.

        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels (x = y = 0).
        let mut prep_rng = rand::rng();
        encode_inputs(&mut gb, &mut ev, 0, 0, &mut prep_rng);

        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        // L_α / L_β are the cleartext masked input vectors `vec a ⊕ vec λ_a` /
        // `vec b ⊕ vec λ_b` (paper `5_online.tex` §242, `6_total.tex` §218).
        // After encode_inputs(x, y), `ev.masked_x_bits[i] = x_i ⊕ α_i` (and
        // gen-side is the 0-vec by the asymmetric sharing); with x = y = 0 in
        // these tests this equals λ_α / λ_β exactly.
        let l_alpha_pub: Vec<bool> = ev.masked_x_bits.clone();
        let l_beta_pub:  Vec<bool> = ev.masked_y_bits.clone();

        // Honest [v_a D_ev]^gb / [v_b D_ev]^gb starting points.
        let mut gb_v_alpha_eval: Vec<Block> = gb.alpha_eval.clone();
        let gb_v_beta_eval: Vec<Block> = gb.beta_eval.clone();

        // TAMPER: corrupt gb's [v_a D_ev]^gb at i=0 by XORing in delta_a (LSB=1).
        // This simulates a malicious garbler whose announced share deviates from
        // what input encoding requires.
        gb_v_alpha_eval[0] ^= *gb.delta_a.as_block();

        // ev's [v D_ev]^ev shares per honest input encoding.
        let ev_v_alpha_eval: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_eval[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_eval[i]
            })
            .collect();
        let ev_v_beta_eval: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_eval[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_eval[j]
            })
            .collect();

        let (e_gen_blocks_tampered, e_eval_blocks_tampered) = assemble_e_input_wire_blocks_p1(
            n, m,
            &gb_v_alpha_eval,
            &ev_v_alpha_eval,
            &gb_v_beta_eval,
            &ev_v_beta_eval,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        // Paper-faithful CheckZero MUST abort for the tampered run.
        assert!(
            !block_check_zero(&e_gen_blocks_tampered, &e_eval_blocks_tampered),
            "tampered gb_v_alpha_eval must cause paper-faithful block_check_zero to abort"
        );
    }

    /// 1.2(i) regression: a tamper whose XOR delta has LSB=0 was UNDETECTED by
    /// the prior LSB-only `check_zero(&[AuthBitShare], delta)`. The Block-form
    /// `block_check_zero` does full per-index block equality, so any non-zero
    /// XOR — including LSB=0 deltas — must be caught.
    ///
    /// Concretely: tamper `gb_v_alpha_eval[0]` by XORing `δ_b` (which has
    /// LSB=0). With the old helper, the combined block's LSB was unchanged,
    /// `e_a_bit = combined.lsb()` stayed `false`, and `check_zero` would have
    /// passed silently. With `block_check_zero`, `gen_block[0] != eval_block[0]`
    /// and the check aborts.
    #[test]
    fn test_protocol_1_e_input_wire_block_check_aborts_on_lsb_zero_tamper() {
        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        let mut prep_rng = rand::rng();
        encode_inputs(&mut gb, &mut ev, 0, 0, &mut prep_rng);

        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        let l_alpha_pub: Vec<bool> = ev.masked_x_bits.clone();
        let l_beta_pub:  Vec<bool> = ev.masked_y_bits.clone();

        let mut gb_v_alpha_eval: Vec<Block> = gb.alpha_eval.clone();
        let gb_v_beta_eval: Vec<Block> = gb.beta_eval.clone();

        // TAMPER: XOR δ_b (LSB=0 by `Delta::random_b` invariant) into
        // gb_v_alpha_eval[0]. The combined block's LSB is unchanged, so the
        // prior LSB-only check would have missed this.
        debug_assert_eq!(ev.delta_b.as_block().lsb(), false,
            "δ_b must have LSB=0 for this test to exercise the missed-by-LSB path");
        gb_v_alpha_eval[0] ^= *ev.delta_b.as_block();

        let ev_v_alpha_eval: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_eval[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_eval[i]
            })
            .collect();
        let ev_v_beta_eval: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_eval[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_eval[j]
            })
            .collect();

        let (e_gen_blocks, e_eval_blocks) = assemble_e_input_wire_blocks_p1(
            n, m,
            &gb_v_alpha_eval,
            &ev_v_alpha_eval,
            &gb_v_beta_eval,
            &ev_v_beta_eval,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        // Sanity: confirm the tamper actually flips the block (combined ≠ 0)
        // and that LSB extraction would NOT have caught it.
        let combined_at_0 = e_gen_blocks[0] ^ e_eval_blocks[0];
        assert_ne!(combined_at_0, Block::default(),
            "tamper must produce a non-zero combined block");
        assert_eq!(combined_at_0.lsb(), false,
            "this tamper must keep LSB=0 (otherwise it would have been caught by the prior check)");

        // Block-form CheckZero MUST detect — full-block equality, not LSB.
        assert!(
            !block_check_zero(&e_gen_blocks, &e_eval_blocks),
            "block_check_zero must detect an LSB=0 tamper that the prior \
             LSB-only check would have missed"
        );
    }
}