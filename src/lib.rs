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

pub mod bcot;
pub mod feq;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;
pub mod preprocessing;
pub mod online;

use crate::block::Block;

#[allow(dead_code)]
const CSP: usize = 128;
#[allow(dead_code)]
const SSP: usize = 40;

/// Block for public 0 MAC.
#[allow(dead_code)]
pub(crate) const MAC_ZERO: Block = Block::new([
    146, 239, 91, 41, 80, 62, 197, 196, 204, 121, 176, 38, 171, 216, 63, 120,
]);
/// Block for public 1 MAC.
#[allow(dead_code)]
pub(crate) const MAC_ONE: Block = Block::new([
    219, 104, 26, 50, 91, 130, 201, 178, 144, 31, 95, 155, 206, 113, 5, 103,
]);

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
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};
    use crate::online::check_zero;
    use crate::sharing::AuthBitShare;

    /// Assembles the per-gate D_ev-authenticated `c_gamma_shares` vec for Protocol 1
    /// CheckZero per Construction 3 (5_online.tex line 206) and Phase 8 CONTEXT.md D-09.
    ///
    /// For each (i, j) gate, c_gamma is the linear combination:
    ///   c_gamma[(i,j)] = (L_alpha[i] AND L_beta[j])              [public bit]
    ///                  XOR L_alpha[i] · l_beta[j]                 [shared, include iff L_alpha[i]]
    ///                  XOR L_beta [j] · l_alpha[i]                [shared, include iff L_beta[j]]
    ///                  XOR l_gamma_star[(i,j)]                    [shared, always — = l_alpha · l_beta]
    ///                  XOR L_gamma[(i,j)]                          [public bit]
    ///                  XOR l_gamma[(i,j)]                          [shared, always]
    ///
    /// **In-process simulation approach:**
    ///
    /// For the in-process integration test, both parties' preprocessing shares are
    /// available. We compute the FULL reconstructed c_gamma bit from both parties'
    /// share values, and assemble an IT-MAC share verified under `delta_a` using
    /// the evaluator's MAC structure (which is committed under delta_a per
    /// src/preprocessing.rs:61-71 and gen_auth_bit in auth_tensor_fpre.rs:66-86).
    ///
    /// Specifically, per `gen_auth_bit`:
    ///   gen_share.key = B's sender key Kb
    ///   eval_share.mac = Kb.auth(eval.value, delta_a)
    ///
    /// So `{ key: gb.key, mac: ev.mac }` satisfies `mac == key.auth(ev.value, delta_a)`.
    ///
    /// The full c_gamma bit is `gb.value XOR ev.value`. In the in-process simulation
    /// this is computed directly; then the MAC is adjusted to reflect the full bit:
    ///   combined.mac = combined.key.auth(c_gamma_bit, delta_a)
    ///                = combined.key.auth(XOR(gb_i.v XOR ev_i.v), delta_a)
    ///
    /// This produces valid `AuthBitShare`s that `check_zero` can verify:
    ///   - value == 0 iff c_gamma_bit == 0
    ///   - mac == key.auth(value, delta_a) always holds
    ///
    /// The combined key is `XOR(gb_i.key)` (sum of gen-side keys, which are the
    /// evaluator's B-keys in the D_ev structure). The MAC is freshly computed from
    /// the full reconstructed value.
    ///
    /// Returns a Vec<AuthBitShare> of length n*m in column-major order (j*n + i).
    // SIMULATION ONLY: This function requires both parties' private state and is
    // only valid inside #[cfg(test)]. In a real protocol each party assembles its
    // own half of c_gamma independently using only its own preprocessing shares,
    // then the parties run check_zero on the combined share over the network.
    #[allow(clippy::too_many_arguments)]
    fn assemble_c_gamma_shares(
        n: usize,
        m: usize,
        l_alpha_pub: &[bool],          // length n — public masked alpha bits
        l_beta_pub: &[bool],           // length m — public masked beta bits
        l_gamma_pub: &[bool],          // length n*m — public masked gamma bits (column-major)
        gb: &AuthTensorGen,
        ev: &AuthTensorEval,
    ) -> Vec<AuthBitShare> {
        assert_eq!(l_alpha_pub.len(), n);
        assert_eq!(l_beta_pub.len(),  m);
        assert_eq!(l_gamma_pub.len(), n * m);
        assert_eq!(gb.alpha_auth_bit_shares.len(),       n);
        assert_eq!(gb.beta_auth_bit_shares.len(),        m);
        assert_eq!(gb.correlated_auth_bit_shares.len(),  n * m);
        assert_eq!(gb.gamma_d_ev_shares.len(),           n * m);
        assert_eq!(ev.alpha_auth_bit_shares.len(),       n);
        assert_eq!(ev.beta_auth_bit_shares.len(),        m);
        assert_eq!(ev.correlated_auth_bit_shares.len(),  n * m);
        assert_eq!(ev.gamma_d_ev_shares.len(),           n * m);

        let mut out: Vec<AuthBitShare> = Vec::with_capacity(n * m);
        for j in 0..m {
            for i in 0..n {
                let idx = j * n + i;

                // Accumulate the combined key (XOR of gen-side B-keys) and the
                // full reconstructed c_gamma bit (XOR of both parties' values).
                // The gen-side keys are the D_ev-structure keys (B's keys, Kb_i)
                // that authenticate the eval's share values under delta_a.
                use crate::keys::Key;
                use crate::block::Block;
                let mut combined_key = Key::from(Block::ZERO);
                let mut c_gamma_bit = false;

                // Term: L_alpha[i] · l_beta[j]   (include iff L_alpha[i] is true)
                if l_alpha_pub[i] {
                    combined_key = combined_key + gb.beta_auth_bit_shares[j].key;
                    c_gamma_bit ^= gb.beta_auth_bit_shares[j].value
                                 ^ ev.beta_auth_bit_shares[j].value;
                }

                // Term: L_beta[j] · l_alpha[i]   (include iff L_beta[j] is true)
                if l_beta_pub[j] {
                    combined_key = combined_key + gb.alpha_auth_bit_shares[i].key;
                    c_gamma_bit ^= gb.alpha_auth_bit_shares[i].value
                                 ^ ev.alpha_auth_bit_shares[i].value;
                }

                // Term: l_gamma*[(i,j)] = l_alpha[i] · l_beta[j]   (always)
                combined_key = combined_key + gb.correlated_auth_bit_shares[idx].key;
                c_gamma_bit ^= gb.correlated_auth_bit_shares[idx].value
                             ^ ev.correlated_auth_bit_shares[idx].value;

                // Term: l_gamma[(i,j)]   (always)
                combined_key = combined_key + gb.gamma_d_ev_shares[idx].key;
                c_gamma_bit ^= gb.gamma_d_ev_shares[idx].value
                             ^ ev.gamma_d_ev_shares[idx].value;

                // Fold the PUBLIC-bit value contribution.
                // (L_alpha[i] AND L_beta[j]) XOR L_gamma[(i,j)] contribute only to
                // the bit value — no IT-MAC structure.
                let public_bit = (l_alpha_pub[i] & l_beta_pub[j]) ^ l_gamma_pub[idx];
                c_gamma_bit ^= public_bit;

                // Build a properly-formed AuthBitShare verified under delta_a.
                // key = XOR(gen-side B-keys), mac = key.auth(c_gamma_bit, delta_a).
                // value = c_gamma_bit.
                let combined_mac = combined_key.auth(c_gamma_bit, &gb.delta_a);
                let share = AuthBitShare {
                    key: combined_key,
                    mac: combined_mac,
                    value: c_gamma_bit,
                };

                out.push(share);
            }
        }
        out
    }

    #[test]
    fn test_auth_tensor_product() {
        let mut rng = rand::rng();
        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random(&mut rng);

        let n = 16;
        let m = 16;

        let input_x = 0b101;
        let input_y = 0b110;

        let mut fpre = TensorFpre::new_with_delta(54, n, m, 8, delta_a, delta_b);
        fpre.generate_for_ideal_trusted_dealer(input_x, input_y);

        let (clear_x, clear_y, alpha, beta) = fpre.get_clear_values();
        let masked_x = clear_x ^ alpha;
        let masked_y = clear_y ^ beta;

        let n_bitmask = (1<<n)-1;
        let m_bitmask = (1<<m)-1;

        assert_eq!(input_x & n_bitmask, clear_x);
        assert_eq!(input_y & m_bitmask, clear_y);

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // check that gb and ev have correct masks
        assert!(
            verify_vector_sharing(masked_x, &gb.x_labels, &ev.x_labels, &delta_a, n)
        );
        assert!(
            verify_vector_sharing(masked_y, &gb.y_labels, &ev.y_labels, &delta_a, m)
        );


        //check the inputs to the first half outer product: masked_x (x) beta
        let (gen_x, gen_y) = gb.get_first_inputs();
        let (eval_x, eval_y) = ev.get_first_inputs();

        assert!(
            verify_column_matrix_sharing(masked_x, &gen_x, &eval_x, &delta_a, n)
        );
        assert!(
            verify_column_matrix_sharing(input_y, &gen_y, &eval_y, &delta_a, m)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_first_half();
        ev.evaluate_first_half(gen_chunk_levels, gen_chunk_cts);

        // check that first_out has the correct value
        // first_out should be masked_x (tensor) input_y
        assert!(
            verify_tensor_output(masked_x, input_y, n, m, &gb.first_half_out, &ev.first_half_out, &delta_a)
        );


        //check the inputs to the second half outer product: masked_y (x) alpha
        let (gen_x, gen_y) = gb.get_second_inputs();
        let (eval_x, eval_y) = ev.get_second_inputs();

        assert!(
            verify_column_matrix_sharing(masked_y, &gen_x, &eval_x, &delta_a, m)
        );
        assert!(
            verify_column_matrix_sharing(alpha, &gen_y, &eval_y, &delta_a, n)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_second_half();
        ev.evaluate_second_half(gen_chunk_levels, gen_chunk_cts);

        // check that second_out has the correct value
        // second_out should be masked_y (tensor) alpha
        assert!(
            verify_tensor_output(masked_y, alpha, m, n, &gb.second_half_out, &ev.second_half_out, &delta_a)
        );

        // check that final_out has the correct value
        for i in 0..n {
            for j in 0..m {
                let expected_val = (((alpha>>i)&1) & ((beta>>j)&1)) != 0;
                let gb_share =
                    if gb.correlated_auth_bit_shares[j * n + i].bit() {
                        gb.delta_a.as_block() ^ gb.correlated_auth_bit_shares[j * n + i].key.as_block()
                    } else {
                        *gb.correlated_auth_bit_shares[j * n + i].key.as_block()
                    };
                let ev_share = *ev.correlated_auth_bit_shares[j * n + i].mac.as_block();

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

    #[test]
    fn test_auth_tensor_product_full_protocol_1() {
        // P1-04: end-to-end honest run produces L_gamma reconstructions consistent with
        // c_gamma == 0, and check_zero returns true.
        //
        // Key: L_alpha[i] and L_beta[j] must be the ACTUAL masked input values used
        // by the garble pipeline. IdealPreprocessingBackend.run() calls
        // generate_for_ideal_trusted_dealer(0, 0) internally, so masked_x = alpha
        // and masked_y = beta. The reconstructed clear masked-input bits are:
        //   L_alpha[i] = gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value
        //               = l_alpha[i] (the preprocessing alpha bit)
        // which gives v_alpha[i] = L_alpha[i] ^ l_alpha[i] = 0. Then
        // v_gamma = v_alpha ⊗ v_beta = 0, so L_gamma = l_gamma and c_gamma = 0.
        //
        // To exercise non-trivial L_alpha/L_beta patterns while keeping c_gamma = 0,
        // we reconstruct the ACTUAL alpha/beta bits and use them directly.

        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Standard Protocol 1 garble + evaluate sequence (mirrors test_auth_tensor_product).
        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        // Phase 8 NEW: emit and reconstruct L_gamma.
        let lambda_gb = gb.compute_lambda_gamma();
        assert_eq!(lambda_gb.len(), n * m);
        let l_gamma_combined = ev.compute_lambda_gamma(&lambda_gb);
        assert_eq!(l_gamma_combined.len(), n * m);

        // The public masked-input bits (L_alpha, L_beta) must match what the garble
        // pipeline used. For IdealPreprocessingBackend (internally uses input=0),
        // L_alpha[i] = reconstructed alpha[i] = gen.value ^ ev.value.
        // This ensures v_alpha = L_alpha ^ l_alpha = 0 for each bit, so
        // v_gamma = v_alpha ⊗ v_beta = 0 and c_gamma = 0 in honest run.
        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        // L_gamma_pub is the masked output value reconstructed by the evaluator —
        // it IS the l_gamma_combined vec we just produced (= l_gamma when v_gamma=0).
        let c_gamma_shares = assemble_c_gamma_shares(
            n, m,
            &l_alpha_pub,
            &l_beta_pub,
            &l_gamma_combined,
            &gb,
            &ev,
        );

        assert_eq!(c_gamma_shares.len(), n * m);

        // Honest-party CheckZero: c_gamma must reconstruct to 0 and the MAC must verify.
        assert!(
            check_zero(&c_gamma_shares, &gb.delta_a),
            "P1-04: honest Protocol 1 run must pass check_zero"
        );
    }

    #[test]
    fn test_protocol_1_check_zero_aborts_on_tampered_lambda() {
        // P1-05: tampering with lambda_gb (the garbler-emitted [L_gamma]^gb) must cause
        // check_zero to return false, NOT silently pass.

        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        let lambda_gb = gb.compute_lambda_gamma();

        // Tamper: clone the garbler's emitted vec, flip ONE bit at index 0.
        // (We do not mutate `lambda_gb` itself — keep it as the honest reference.)
        let mut tampered_lambda_gb = lambda_gb.clone();
        tampered_lambda_gb[0] ^= true;
        assert_ne!(tampered_lambda_gb, lambda_gb,
            "tampered vec must differ from honest vec");

        // Evaluator processes the tampered lambda_gb — this corrupts L_gamma_combined[0].
        let l_gamma_combined_tampered = ev.compute_lambda_gamma(&tampered_lambda_gb);
        assert_eq!(l_gamma_combined_tampered.len(), n * m);

        // Use the ACTUAL reconstructed masked-input bits (same approach as P1-04).
        // This ensures the c_gamma formula is correct regardless of the L values chosen.
        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        let c_gamma_shares_tampered = assemble_c_gamma_shares(
            n, m,
            &l_alpha_pub,
            &l_beta_pub,
            &l_gamma_combined_tampered,
            &gb,
            &ev,
        );

        // The tampered L_gamma corrupts c_gamma at index 0. check_zero MUST return false.
        assert!(
            !check_zero(&c_gamma_shares_tampered, &gb.delta_a),
            "P1-05: tampered lambda_gb must cause check_zero to abort, not silently pass"
        );
    }

}