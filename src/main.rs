use authenticated_tensor_garbling::circuit::Circuit;

use authenticated_tensor_garbling::fpre::Fpre;
use authenticated_tensor_garbling::auth_gen::AuthGen;
use authenticated_tensor_garbling::auth_eval::AuthEval;

fn main() {

    println!("Beginning online benchmarking for KRRW Distributed Garbling Scheme");

    // Create a test circuit with 25 XOR gates and 50 AND gates
    let num_xor = 0;
    let num_and = 2;

    let circ = Circuit::from_params(num_xor, num_and);

    // Generate the Fpre for the circuit
    let mut fpre = Fpre::new(5, circ.get_xor_input_wires(), circ.get_and_wires());
    fpre.generate();

    let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

    // Run Garble_g
    let mut auth_gen = AuthGen::new_with_pre(fpre_gen);
    let (lz, half_gates) = auth_gen.generate(&circ);

    let masked_inputs = lz.iter().map(|label| label.lsb()).collect::<Vec<_>>();

    // Run Garble_e
    let mut auth_eval = AuthEval::new_with_pre(fpre_eval, &circ);
    let (labels, masked_values) = auth_eval.evaluate(&circ, &half_gates, &lz, masked_inputs).unwrap();

    // run consistency check and decode
    let gen_hash = auth_gen.verify(&circ, &masked_values);
    let eval_hash = auth_eval.verify(&circ, &masked_values);
    
    // Verify hashes match
    assert_eq!(gen_hash, eval_hash, "Authentication failed!");
    
    // Extract and decode outputs
    // TODO circuit function that returns output wire IDs
    // let output_labels_gen = &labels[circ.outputs()];
    // let output_labels_eval = &labels[circ.outputs()];
    // let output_masked_values = &masked_values[circ.outputs()];
    
    // let outputs = decode_outputs(output_labels_gen, output_labels_eval, output_masked_values)?;
    
    println!("Circuit evaluation completed successfully!");
    // println!("Output: {:?}", outputs);
}


mod tests {

    use super::*;
    
    #[test]
    fn test_fpre_insecure() {
        let num_xor = 25;
        let num_and = 50;

        let circ = Circuit::from_params(num_xor, num_and);
        let total_wires = circ.total_wires();

        let mut frpe = Fpre::new(5, circ.get_xor_input_wires(), circ.get_and_wires());
        frpe.generate();

        assert_eq!(frpe.auth_bits.len(), total_wires);
        assert_eq!(frpe.auth_triples.len(), num_and);

        for bit in &frpe.auth_bits {
            bit.verify(frpe.delta_a(), frpe.delta_b());
        }

        for triple in &frpe.auth_triples {
            triple.verify(frpe.delta_a(), frpe.delta_b());
        }

        let (fpre_gen, fpre_eval) = frpe.into_gen_eval();

        // wire shares length
        assert_eq!(fpre_gen.wire_shares.len(), total_wires);
        assert_eq!(fpre_eval.wire_shares.len(), total_wires);

        // triple shares length
        assert_eq!(fpre_gen.triple_shares.len(), num_and);
        assert_eq!(fpre_eval.triple_shares.len(), num_and);

        println!("FpreGen: {:#?}", fpre_gen.triple_shares.len());
        println!("FpreEval: {:#?}", fpre_eval.triple_shares.len());
    }
}