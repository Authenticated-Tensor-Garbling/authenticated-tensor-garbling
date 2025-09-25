pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;

pub mod matrix;

pub mod circuit;
pub mod aes;

pub mod fpre;
pub mod auth_gen;
pub mod auth_eval;

pub mod unary_outer_product;
pub mod tensor_pre;
pub mod tensor_gen;
pub mod tensor_eval;

mod auth_tensor_fpre;
mod auth_tensor_gen;
mod auth_tensor_eval;

// Re-export circuits for convenience
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

    use crate::{delta::Delta, keys::Key, auth_gen::AuthGen, auth_eval::AuthEval};
    use ::aes::Aes128;
    use ::cipher::KeyInit;
    use ::cipher::BlockCipherEncrypt;
    use itybity::IntoBitIterator;
    use crate::fpre::Fpre;
    use rand::Rng;
    use crate::macs::Mac;

    use mpz_circuits::circuits::AES128;


    use crate::auth_gen::AuthGenOutput;
    use crate::auth_eval::AuthEvalOutput;
    
    use itybity::FromBitIterator;

    
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

        println!("number of ands: {:?}", circuit.and_count());
        println!("number of and authbits: {:?}", gen_and_shares.len());
        println!("feed_count: {:?}", circuit.feed_count());

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

    use crate::{aes::FIXED_KEY_AES, tensor_gen::TensorProductGen, tensor_eval::TensorProductEval, unary_outer_product::{gen_chunked_half_outer_product, eval_chunked_half_outer_product, gen_masks}, matrix::BlockMatrix, tensor_pre::get_gen_eval_vecs, block::Block};

    #[test]
    fn test_first_half_outer_product() {
        // Test that the first half outer product works correctly
        let cipher = &FIXED_KEY_AES;
        
        // Use a fixed seed to ensure deterministic results
        let mut rng = rand::rng();
        let delta = Delta::random(&mut rng);
        
        let n = 3;
        let m = 4;
        let clear_x = 5;  // Fixed value instead of random
        let clear_y = 3;  // Fixed value instead of random
        
        // Setup using reference implementation first to get the exact inputs
        // Use the same rng to ensure deterministic results
        let (gen_x, eval_x) = get_gen_eval_vecs(delta, n, clear_x);
        let (gen_y, eval_y) = get_gen_eval_vecs(delta, m, clear_y);
        let (alpha, beta) = gen_masks(n, m, &delta);
        
        let gen_x_masked = gen_x.clone();
        let eval_x_masked = &eval_x ^ &alpha;
        let gen_y_unmasked = &gen_y ^ &beta;
        let eval_y_unmasked = &eval_y ^ &beta;
        
        // Execute first half using reference implementation
        let mut ref_gen_first_half_out = BlockMatrix::new(n, m);
        let mut ref_eval_first_half_out = BlockMatrix::new(n, m);
        
        let (ref_chunk_levels, ref_chunk_cts) = gen_chunked_half_outer_product(
            &gen_x_masked.as_view(), 
            &gen_y_unmasked.as_view(), 
            &mut ref_gen_first_half_out.as_view_mut(), 
            delta, 
            cipher
        );
        eval_chunked_half_outer_product(
            &eval_x_masked.as_view(), 
            &eval_y_unmasked.as_view(), 
            &mut ref_eval_first_half_out.as_view_mut(), 
            ref_chunk_levels, 
            ref_chunk_cts, 
            cipher
        );
        
        // Now setup using refactored code with the SAME inputs
        // We need to manually create the pre_gen and pre_eval with the exact same values
        // that the reference implementation computed
        let pre_gen = crate::tensor_pre::TensorProductPreGen::new(
            cipher, 6, n, m, delta, 
            gen_x.clone(), gen_y.clone(), 
            alpha.clone(), beta.clone()
        );
        let pre_eval = crate::tensor_pre::TensorProductPreEval::new(
            cipher, 6, n, m, 
            eval_x_masked.clone(), eval_y_unmasked.clone()
        );
        let mut gar = TensorProductGen::new(pre_gen);
        let mut eval = TensorProductEval::new(pre_eval);
        
        // Execute first half using refactored code
        let (gen_chunk_levels, gen_chunk_cts) = gar.execute_first_half_outer_product();
        eval.execute_first_half_outer_product(gen_chunk_levels, gen_chunk_cts);
        
        let refactored_gen_result = gar.first_half_out.clone();
        let refactored_eval_result = eval.first_half_out.clone();
        
        // Compare results
        println!("=== FIRST HALF COMPARISON ===");
        println!("Inputs used:");
        println!("  clear_x: {}, clear_y: {}", clear_x, clear_y);
        println!("  delta: {:?}", delta);
        println!("  gen_x_masked clear value: {}", gen_x_masked.get_clear_value());
        println!("  eval_x_masked clear value: {}", eval_x_masked.get_clear_value());
        println!("  gen_y_unmasked clear value: {}", gen_y_unmasked.get_clear_value());
        println!("  eval_y_unmasked clear value: {}", eval_y_unmasked.get_clear_value());
        println!("  alpha clear value: {}", alpha.get_clear_value());
        println!("  beta clear value: {}", beta.get_clear_value());
        println!();
        println!("Reference gen result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", ref_gen_first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Reference eval result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", ref_eval_first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Refactored gen result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", refactored_gen_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Reference gen result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", ref_gen_first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Refactored eval result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", refactored_eval_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Reference eval result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", ref_eval_first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        
        // Check if results match element-wise
        for i in 0..n {
            for j in 0..m {
                assert_eq!(refactored_gen_result[(i,j)], ref_gen_first_half_out[(i,j)], 
                    "Generator results differ at position ({},{})", i, j);
                assert_eq!(refactored_eval_result[(i,j)], ref_eval_first_half_out[(i,j)],
                    "Evaluator results differ at position ({},{})", i, j); 
            }
        }
        println!("First half outer product test PASSED!");
    }

    #[test]
    fn test_second_half_outer_product() {
        // Test that the second half outer product works correctly
        let cipher = &FIXED_KEY_AES;
        
        // Use a fixed seed to ensure deterministic results
        let mut rng = rand::rng();
        let delta = Delta::random(&mut rng);
        
        let n = 4;
        let m = 3;
        let clear_x = 5;  // Fixed value instead of random
        let clear_y = 3;  // Fixed value instead of random
        
            // Setup using reference implementation first to get the exact inputs
            let (gen_x, eval_x) = get_gen_eval_vecs(delta, n, clear_x);
            let (gen_y, eval_y) = get_gen_eval_vecs(delta, m, clear_y);
            let (alpha, beta) = gen_masks(n, m, &delta);
            
            // For second half, we need the masked versions
            let gen_y_masked = gen_y.clone();
            let eval_y_masked = &eval_y ^ &beta;
            let gen_alpha = alpha.clone();
            let eval_alpha = BlockMatrix::constant(n, 1, Block::default());
            
            // Execute second half using reference implementation
            // Second half produces (y ⊕ β) ⊗ α which is m × n
            let mut ref_gen_second_half_out = BlockMatrix::new(m, n);
            let mut ref_eval_second_half_out = BlockMatrix::new(m, n);
        
            let (ref_chunk_levels, ref_chunk_cts) = gen_chunked_half_outer_product(
                &gen_y_masked.as_view(), 
                &gen_alpha.as_view(), 
                &mut ref_gen_second_half_out.as_view_mut(), 
                delta, 
                cipher
            );
            eval_chunked_half_outer_product(
                &eval_y_masked.as_view(), 
                &eval_alpha.as_view(), 
                &mut ref_eval_second_half_out.as_view_mut(), 
                ref_chunk_levels, 
                ref_chunk_cts, 
                cipher
            );
        
        // Now setup using refactored code with the SAME inputs
        let pre_gen = crate::tensor_pre::TensorProductPreGen::new(
            cipher, 6, n, m, delta, 
            gen_x.clone(), gen_y.clone(),
            alpha.clone(), beta.clone()
        );
        let pre_eval = crate::tensor_pre::TensorProductPreEval::new(
            cipher, 6, n, m, 
            (&eval_x ^ &alpha).clone(), (&eval_y ^ &beta).clone()
        );
        let mut gar = TensorProductGen::new(pre_gen);
        let mut eval = TensorProductEval::new(pre_eval);
        
        // Execute first half to set up the phase
        let (gen_chunk_levels, gen_chunk_cts) = gar.execute_first_half_outer_product();
        eval.execute_first_half_outer_product(gen_chunk_levels, gen_chunk_cts);
        
        // Execute second half using refactored code
        let (gen_chunk_levels, gen_chunk_cts) = gar.execute_second_half_outer_product();
        eval.execute_second_half_outer_product(gen_chunk_levels, gen_chunk_cts);
        
        let refactored_gen_result = gar.second_half_out.clone();
        let refactored_eval_result = eval.second_half_out.clone();
        
        // Compare results
        println!("=== SECOND HALF COMPARISON ===");
        println!("Inputs used:");
        println!("  clear_x: {}, clear_y: {}", clear_x, clear_y);
        println!("  delta: {:?}", delta);
        println!("  gen_y_masked clear value: {}", gen_y_masked.get_clear_value());
        println!("  eval_y_masked clear value: {}", eval_y_masked.get_clear_value());
        println!("  gen_alpha clear value: {}", gen_alpha.get_clear_value());
        println!("  eval_alpha clear value: {}", eval_alpha.get_clear_value());
        println!();
        println!("Reference gen result:");
        for i in 0..m {
            for j in 0..n {
                print!("{:02x} ", ref_gen_second_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Reference eval result:");
        for i in 0..m {
            for j in 0..n {
                print!("{:02x} ", ref_eval_second_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Refactored gen result:");
        for i in 0..m {
            for j in 0..n {
                print!("{:02x} ", refactored_gen_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Refactored eval result:");
        for i in 0..m {
            for j in 0..n {
                print!("{:02x} ", refactored_eval_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        
        // Check if results match element-wise
        for i in 0..m {
            for j in 0..n {
                assert_eq!(refactored_gen_result[(i,j)], ref_gen_second_half_out[(i,j)], 
                    "Generator second half results differ at position ({},{})", i, j);
                assert_eq!(refactored_eval_result[(i,j)], ref_eval_second_half_out[(i,j)],
                    "Evaluator second half results differ at position ({},{})", i, j); 
            }
        }
        println!("Second half outer product test PASSED!");
    }

    #[test]
    fn test_semihonest_tensor_product() {
        let cipher = &FIXED_KEY_AES;
        
        let mut rng = rand::rng();
        let delta = Delta::random(&mut rng);

        let n = 2;
        let m = 3;
        let clear_x = 3;  // Fixed value instead of random
        let clear_y = 6;  // Fixed value instead of random

        // Setup using reference implementation first to get the exact inputs
        let (gen_x, eval_x) = get_gen_eval_vecs(delta, n, clear_x);
        let (gen_y, eval_y) = get_gen_eval_vecs(delta, m, clear_y);
        let (alpha, beta) = gen_masks(n, m, &delta);

        let pre_gen = crate::tensor_pre::TensorProductPreGen::new(
            cipher, 6, n, m, delta, 
            gen_x.clone(), gen_y.clone(), 
            alpha.clone(), beta.clone()
        );
        let pre_eval = crate::tensor_pre::TensorProductPreEval::new(
            cipher, 6, n, m, 
            (&eval_x ^ &alpha).clone(), (&eval_y ^ &beta).clone()
        );

        // Debug: Print setup values
        println!("=== SETUP DEBUG ===");
        println!("clear_x: {:b} ({}), clear_y: {:b} ({})", clear_x, clear_x, clear_y, clear_y);
        println!("delta: {:02x}", delta.as_block().as_bytes()[0]);
        println!("gen_x clear value: {}", pre_gen.x.get_clear_value());
        println!("gen_y clear value: {}", pre_gen.y.get_clear_value());
        println!("eval_x clear value: {}", pre_eval.x.get_clear_value());
        println!("eval_y clear value: {}", pre_eval.y.get_clear_value());
        println!("alpha clear value: {}", pre_gen.alpha.get_clear_value());
        println!("beta clear value: {}", pre_gen.beta.get_clear_value());
        println!("==================");

        let mut tensor_gen = TensorProductGen::new(pre_gen);
        let mut tensor_eval = TensorProductEval::new(pre_eval);

        // first half outer product: H(blinded_x) (x) y
        let (gen_levels, gen_cts) = tensor_gen.execute_first_half_outer_product();
        tensor_eval.execute_first_half_outer_product(gen_levels, gen_cts);

        // second half outer product: (y ^ beta) (x) alpha
        let (gen_levels, gen_cts) = tensor_gen.execute_second_half_outer_product();
        tensor_eval.execute_second_half_outer_product(gen_levels, gen_cts);

        // final outer product
        let gen_result = tensor_gen.execute_final_outer_product();
        let eval_result = tensor_eval.execute_final_outer_product();
        
        // Debug: print the results
        println!("Gen result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", gen_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("Eval result:");
        for i in 0..n {
            for j in 0..m {
                print!("{:02x} ", eval_result[(i, j)].as_bytes()[0]);
            }
            println!();
        }

        let mut expected_result = vec![vec![false; m]; n];
        for i in 0..n {
            print!("[ ");
            for j in 0..m {
                let x_bit = ((clear_x >> i) & 1) == 1;
                let y_bit = ((clear_y >> j) & 1) == 1;
                expected_result[i][j] = x_bit & y_bit;
                print!("{} ", expected_result[i][j] as usize);
            }
            println!("]");
        }

        for k in 0..n {
            print!("[ ");
            for j in 0..m {
                let gen_val = gen_result[(k, j)];
                let eval_val = eval_result[(k, j)];
                let expected_bit = expected_result[k][j];
                
                if expected_bit {
                    // Where expected_result = 1, they should differ by delta
                    let expected_eval = gen_val ^ delta;
                    assert_eq!(eval_val, expected_eval, 
                               "At position ({},{}): eval_out should equal gen_out ^ delta when expected=1", k, j);
                    print!("{} ", 1);
                } else {
                    // Where expected_result = 0, they should be identical
                    assert_eq!(gen_val, eval_val, 
                               "At position ({},{}): gen_out should equal eval_out when expected=0", k, j);
                    print!("{} ", 0);
                }
                
            }
            println!("]");
        }
    }

    use crate::auth_tensor_fpre::TensorFpre;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;

    #[test]
    fn test_auth_tensor_product() {
        
        let n = 2;
        let m = 3;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_with_input_values(0b101, 0b110);

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthTensorGen::new_from_fpre_gen(1, fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(1, fpre_eval);

        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_first_half();
        ev.evaluate_first_half(gen_chunk_levels, gen_chunk_cts);

        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_second_half();
        ev.evaluate_second_half(gen_chunk_levels, gen_chunk_cts);

        gb.garble_final();
        ev.evaluate_final();
    }
}