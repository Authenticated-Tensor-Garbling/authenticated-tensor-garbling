pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;
pub mod sharing;

pub mod matrix;

pub mod circuit;
pub mod aes;

pub mod fpre;
pub mod auth_gen;
pub mod auth_eval;

// pub mod unary_outer_product;
pub mod tensor_pre;
pub mod tensor_gen;
pub mod tensor_eval;
pub mod tensor_ops;

pub mod auth_tensor_fpre;
pub mod auth_tensor_gen;
pub mod auth_tensor_eval;

pub use mpz_circuits::{Circuit, CircuitBuilder, CircuitError, Gate, GateType, evaluate};
use crate::block::Block;

#[allow(dead_code)]
const CSP: usize = 128;
#[allow(dead_code)]
const SSP: usize = 40;

const BYTES_PER_GATE: usize = 32;
const KB: usize = 1024;
const MAX_BATCH_SIZE: usize = 4 * KB;
pub(crate) const DEFAULT_BATCH_SIZE: usize = MAX_BATCH_SIZE / BYTES_PER_GATE;

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

    use mpz_circuits::CircuitBuilder;
    use rand::Rng;
    use itybity::FromBitIterator;
    use itybity::IntoBitIterator;
    use mpz_circuits::circuits::AES128;
    use aes::Aes128;
    use cipher::{KeyInit, BlockCipherEncrypt};
    use crate::{
        delta::Delta,
        keys::Key,
        auth_gen::AuthGen,
        auth_eval::AuthEval,
        fpre::Fpre,
        macs::Mac,
        auth_gen::AuthGenOutput,
        auth_eval::AuthEvalOutput
    };
    
    #[test]
    fn test_tensor_and() {
        
        const N: usize = 8;
        let x_input = [2u8; 1];
        let y_input = [3u8; 1];
        
        let mut builder = CircuitBuilder::new();
        let x: [_; N] = std::array::from_fn(|_| builder.add_input()); // any way to denote constexpr to set the size like in C++?
        let y: [_; N] = std::array::from_fn(|_| builder.add_input());
        
        let mut outputs = Vec::new();
        
        for i in 0..4 {
            for j in 0..4 {
                outputs.push(builder.add_and_gate(x[i], y[j]));
            }
        }
        
        for out in outputs {
            builder.add_output(out);
        }
        
        let circ = builder.build().unwrap();
        
        
        // no need for expected; who cares about correctness?
        
        // set up the protocol
        
        // Set up fpre and get AuthBit shares
        let mut rng = rand::rng();
        
        let delta_a = Delta::random(&mut rng).set_lsb(true);
        let delta_b = Delta::random(&mut rng).set_lsb(false);

        let num_input_shares = circ.inputs().len();
        let num_and_shares = circ.and_count();
        let total_number_shares = num_input_shares + num_and_shares;

        // Generate Fpre
        let mut fpre = Fpre::new_with_delta(0, num_input_shares, num_and_shares, delta_a, delta_b);
        fpre.generate();

        let seed = 5;

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthGen::new(seed, 0);
        let mut ev = AuthEval::new(seed, 0);

        // Set up inputs
        let input_keys = (0..circ.inputs().len())
        .map(|_| Key::from(rng.random::<[u8; 16]>()))
        .collect::<Vec<Key>>();

        if fpre_gen.wire_shares.len() != total_number_shares {
            panic!("fpre_gen.wire_shares.len() != total_number_shares");
        }

        if fpre_eval.wire_shares.len() != total_number_shares {
            panic!("fpre_eval.wire_shares.len() != total_number_shares");
        }

        let (gen_input_shares, gen_and_shares) = fpre_gen.wire_shares.split_at(num_input_shares);
        let (eval_input_shares, eval_and_shares) = fpre_eval.wire_shares.split_at(num_input_shares);

        let masked_inputs = x_input.iter().copied().chain(y_input).into_iter_lsb0()
        .enumerate()
        .map(|(i, b)| {b ^ gen_input_shares[i].bit() ^ eval_input_shares[i].bit()})
        .collect::<Vec<bool>>();

        let input_macs = masked_inputs.iter()
            .enumerate()
            .map(|(i, b)| {
                if *b {
                    Mac::from(input_keys[i].as_block().clone()) + Mac::from(delta_a.as_block().clone())
                } else {
                    Mac::from(input_keys[i].as_block().clone())
                }
            })
            .collect::<Vec<Mac>>();

        gb.generate_pre_ideal(&circ, gen_input_shares, gen_and_shares, fpre_gen.triple_shares.as_slice()).unwrap();
        ev.generate_pre_ideal(&circ, eval_input_shares, eval_and_shares, fpre_eval.triple_shares.as_slice()).unwrap();

        gb.generate_free(&circ).unwrap();
        ev.evaluate_free(&circ).unwrap();
        
        let (px_gen, py_gen) = gb.generate_de(&circ).unwrap();
        let (px_eval, py_eval) = ev.evaluate_de(&circ).unwrap();

        // apply the corrections to shares
        let _ = gb.generate_batched(&circ, delta_a, &input_keys, px_eval, py_eval).unwrap();
        let _ = ev.evaluate_batched(&circ, delta_b, &input_macs, masked_inputs, px_gen, py_gen).unwrap();

        let half_gates = gb.garble(&circ, delta_a).unwrap();
        let _ = ev.process(&circ, delta_b, half_gates);
        
        let AuthEvalOutput {
            output_labels: eval_output_labels,
            output_auth_bits: eval_output_auth_bits,
            auth_hash: eval_auth_hash,
            masked_output_values,
            masked_values,
        } = ev.finish(&circ, delta_b).unwrap();

        let AuthGenOutput {
            output_labels: gen_output_labels,
            output_auth_bits: gen_output_auth_bits,
            auth_hash: gen_auth_hash,
        } = gb.finish(&circ, delta_a, masked_values).unwrap();

        // authentication check
        assert_eq!(gen_auth_hash, eval_auth_hash, "auth hash mismatch");

        let masks = gen_output_auth_bits.iter()
            .zip(eval_output_auth_bits.iter())
            .map(|(gen_auth_bit, eval_auth_bit)| gen_auth_bit.bit() ^ eval_auth_bit.bit())
            .collect::<Vec<bool>>();
        
        // Unmask the output
        let output: Vec<u8> = Vec::from_lsb0_iter(
            masked_output_values
                .clone()
                .into_iter()
                .enumerate()
                .map(|(i, masked_value)| masked_value ^ masks[i]),
        );

        // assert_eq!(output, expected, "output mismatch");

        // Check output labels
        // for (i, (gen_label, eval_label)) in gen_output_labels.iter().zip(eval_output_labels.iter()).enumerate() {
        //     let xor = gen_label.as_block() ^ eval_label.as_block();
        //     let masked_value = masked_output_values[i];
        //     let expected = delta_a.mul_bool(masked_value);
        //     assert_eq!(xor, expected, "output label mismatch");
        // }

    }

    #[test]
    fn test_auth_garble() {
        let mut rng = rand::rng();

        // Set up AES circuit
        let key = [69u8; 16];
        let msg = [42u8; 16];

        let circuit = &mpz_circuits::circuits::AES128;

        let expected: [u8; 16] = {
            let cipher = Aes128::new_from_slice(&key).unwrap();
            let mut out = msg.into();
            cipher.encrypt_block(&mut out);
            out.into()
        };

        // Set up fpre and get AuthBit shares

        let delta_a = Delta::random(&mut rng).set_lsb(true);
        let delta_b = Delta::random(&mut rng).set_lsb(false);

        let num_input_shares = circuit.inputs().len();
        let num_and_shares = circuit.and_count();
        let total_number_shares = num_input_shares + num_and_shares;

        // Generate Fpre
        let mut fpre = Fpre::new_with_delta(0, num_input_shares, num_and_shares, delta_a, delta_b);
        fpre.generate();

        let seed = 5;

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthGen::new(seed, 0);
        let mut ev = AuthEval::new(seed, 0);

        // // Set up inputs
        let input_keys = (0..circuit.inputs().len())
            .map(|_| Key::from(rng.random::<[u8; 16]>()))
            .collect::<Vec<Key>>();

        if fpre_gen.wire_shares.len() != total_number_shares {
            panic!("fpre_gen.wire_shares.len() != total_number_shares");
        }

        if fpre_eval.wire_shares.len() != total_number_shares {
            panic!("fpre_eval.wire_shares.len() != total_number_shares");
        }
        
        let (gen_input_shares, gen_and_shares) = fpre_gen.wire_shares.split_at(num_input_shares);
        let (eval_input_shares, eval_and_shares) = fpre_eval.wire_shares.split_at(num_input_shares);

        let masked_inputs = key.iter().copied().chain(msg).into_iter_lsb0()
            .enumerate()
            .map(|(i, b)| {b ^ gen_input_shares[i].bit() ^ eval_input_shares[i].bit()})
            .collect::<Vec<bool>>();

        let input_macs = masked_inputs.iter()
            .enumerate()
            .map(|(i, b)| {
                if *b {
                    Mac::from(input_keys[i].as_block().clone()) + Mac::from(delta_a.as_block().clone())
                } else {
                    Mac::from(input_keys[i].as_block().clone())
                }
            })
            .collect::<Vec<Mac>>();

        gb.generate_pre_ideal(&circuit, gen_input_shares, gen_and_shares, fpre_gen.triple_shares.as_slice()).unwrap();
        ev.generate_pre_ideal(&circuit, eval_input_shares, eval_and_shares, fpre_eval.triple_shares.as_slice()).unwrap();

        gb.generate_free(&circuit).unwrap();
        ev.evaluate_free(&circuit).unwrap();

        let (px_gen, py_gen) = gb.generate_de(&circuit).unwrap();
        let (px_eval, py_eval) = ev.evaluate_de(&circuit).unwrap();

        // apply the corrections to shares
        let _ = gb.generate_batched(&AES128, delta_a, &input_keys, px_eval, py_eval).unwrap();
        let _ = ev.evaluate_batched(&AES128, delta_b, &input_macs, masked_inputs, px_gen, py_gen).unwrap();

        let half_gates = gb.garble(&circuit, delta_a).unwrap();
        let _ = ev.process(&circuit, delta_b, half_gates);
        
        let AuthEvalOutput {
            output_labels: eval_output_labels,
            output_auth_bits: eval_output_auth_bits,
            auth_hash: eval_auth_hash,
            masked_output_values,
            masked_values,
        } = ev.finish(&circuit, delta_b).unwrap();

        let AuthGenOutput {
            output_labels: gen_output_labels,
            output_auth_bits: gen_output_auth_bits,
            auth_hash: gen_auth_hash,
        } = gb.finish(&circuit, delta_a, masked_values).unwrap();

        // authentication check
        assert_eq!(gen_auth_hash, eval_auth_hash, "auth hash mismatch");

        let masks = gen_output_auth_bits.iter()
            .zip(eval_output_auth_bits.iter())
            .map(|(gen_auth_bit, eval_auth_bit)| gen_auth_bit.bit() ^ eval_auth_bit.bit())
            .collect::<Vec<bool>>();
        
        // Unmask the output
        let output: Vec<u8> = Vec::from_lsb0_iter(
            masked_output_values
                .clone()
                .into_iter()
                .enumerate()
                .map(|(i, masked_value)| masked_value ^ masks[i]),
        );

        assert_eq!(output, expected, "output mismatch");

        // Check output labels
        for (i, (gen_label, eval_label)) in gen_output_labels.iter().zip(eval_output_labels.iter()).enumerate() {
            let xor = gen_label.as_block() ^ eval_label.as_block();
            let masked_value = masked_output_values[i];
            let expected = delta_a.mul_bool(masked_value);
            assert_eq!(xor, expected, "output label mismatch");
        }

    }

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
        let clear_x = 0b01;  // Fixed value instead of random
        let clear_y = 0b101;  // Fixed value instead of random

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
        // first_out should be masked_x (tensor) input_y
        assert!(
            verify_tensor_output(masked_y & m_bitmask, alpha & n_bitmask, m, n, &gb.second_half_out, &ev.second_half_out, &delta)
        );


        
        // final outer product
        let gen_result = gb.garble_final_outer_product();
        let eval_result = ev.evaluate_final_outer_product();

        // check that final_out has the correct value
        // final_out should be masked_x (tensor) input_y (tensor) alpha
        assert!(
            verify_tensor_output(clear_x, clear_y, n, m, &gen_result, &eval_result, &delta)
        );
    }

    use crate::auth_tensor_fpre::TensorFpre;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;

    #[test]
    fn test_auth_tensor_product() {
        let mut rng = rand::rng();
        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random(&mut rng);

        let n = 2;
        let m = 3;

        let input_x = 0b101;
        let input_y = 0b110;

        let mut fpre = TensorFpre::new_with_delta(54, n, m, 6, delta_a, delta_b);
        fpre.generate_with_input_values(input_x, input_y);

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
        // x_labels should be masked_x ^ alpha
        // y_labels should be masked_y ^ beta
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
        // final_out should be masked_x (tensor) input_y (tensor) alpha
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
            println!();
        }
    }
}