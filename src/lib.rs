pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;

pub mod circuit;
pub mod aes;

pub mod fpre;
pub mod auth_gen;
pub mod auth_eval;

// Re-export circuits for convenience
pub use mpz_circuits::{Circuit, CircuitBuilder, CircuitError, Gate, GateType, evaluate};



use crate::block::Block;

const CSP: usize = 128;
const SSP: usize = 40;

const BYTES_PER_GATE: usize = 32;
const KB: usize = 1024;
const MAX_BATCH_SIZE: usize = 4 * KB;
pub(crate) const DEFAULT_BATCH_SIZE: usize = MAX_BATCH_SIZE / BYTES_PER_GATE;

/// Block for public 0 MAC.
pub(crate) const MAC_ZERO: Block = Block::new([
    146, 239, 91, 41, 80, 62, 197, 196, 204, 121, 176, 38, 171, 216, 63, 120,
]);
/// Block for public 1 MAC.
pub(crate) const MAC_ONE: Block = Block::new([
    219, 104, 26, 50, 91, 130, 201, 178, 144, 31, 95, 155, 206, 113, 5, 103,
]);

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{aes::FIXED_KEY_AES, delta::Delta, fpre::AuthBitShare, keys::Key, auth_gen::AuthGen, auth_eval::AuthEval};
    use ::aes::Aes128;
    use ::cipher::KeyInit;
    use ::cipher::BlockCipherEncrypt;
    use itybity::IntoBitIterator;
    use crate::fpre::Fpre;
    use rand::Rng;
    use crate::macs::Mac;

    use mpz_circuits::circuits::AES128;

    use crate::auth_gen::AuthEncryptedGateBatchIter;
    use crate::auth_eval::AuthEncryptedGateBatchConsumer;
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
            .map(|_| rng.random())
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
}