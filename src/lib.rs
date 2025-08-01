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

mod tests {

    use crate::{aes::FIXED_KEY_AES, delta::Delta, fpre::AuthBitShare};
    use ::aes::Aes128;
    use ::cipher::KeyInit;
    use ::cipher::BlockCipherEncrypt;
    use crate::fpre::Fpre;

    use super::*;
    
    #[test]
    fn test_auth_garble() {
        let mut rng = rand::rng();

        let key = [69u8; 16];
        let msg = [42u8; 16];

        let circuit = &mpz_circuits::circuits::AES128;

        let expected: [u8; 16] = {
            let cipher = Aes128::new_from_slice(&key).unwrap();
            let mut out = msg.into();
            cipher.encrypt_block(&mut out);
            out.into()
        };

        let delta_a = Delta::random(&mut rng).set_lsb(true);
        let delta_b = Delta::random(&mut rng).set_lsb(false);

        let num_input_shares = circuit.inputs().len();
        let num_and_shares = circuit.and_count();
        let total_number_shares = num_input_shares + num_and_shares;

        // Generate Fpre
        let mut fpre = Fpre::new_with_delta(0, num_input_shares, num_and_shares, delta_a, delta_b);
        fpre.generate();

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        
    }
}