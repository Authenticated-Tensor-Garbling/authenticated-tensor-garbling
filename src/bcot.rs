use crate::{
    block::Block,
    delta::Delta,
    keys::Key,
    macs::Mac,
    sharing::AuthBitShare,
};
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

/// Ideal boolean correlated OT (bCOT) functionality.
///
/// Sender holds global correlation key Delta. For each OT:
///   Sender has (K[0], K[1]) where K[1] = K[0] XOR Delta, K[0].lsb() == 0
///   Receiver with choice bit b gets K[b] = K[0] XOR b*Delta
///
/// This is an in-process ideal functionality — no networking. Both parties'
/// views are computed locally. Use for uncompressed preprocessing benchmarks only.
///
/// TODO: Replace with a real OT protocol (e.g., Ferret/IKNP) for production.
pub struct IdealBCot {
    pub delta_a: Delta,   // Party A's global key
    pub delta_b: Delta,   // Party B's global key
    rng: ChaCha12Rng,
}

/// Output of a single bCOT batch transfer.
/// sender_keys: the sender holds K[0] for each position (LSB always 0).
/// receiver_macs: the receiver holds K[choice[i]] = K[0] XOR choice[i]*delta.
/// Both together form an AuthBitShare pair where sender holds Key and receiver holds Mac.
pub struct BcotOutput {
    /// Sender's view: holds the K[0] key for each position. LSB is always 0.
    pub sender_keys: Vec<Key>,
    /// Receiver's view: holds K[choice[i]] for each position.
    /// NOTE: This is a Mac value that may have LSB=1. NEVER cast receiver_macs to Key.
    /// To obtain a share where B holds the Key, run a separate transfer_b_to_a call
    /// where B is the sender — B's sender_keys will have LSB=0 by construction.
    pub receiver_macs: Vec<Mac>,
    /// The choice bits held by the receiver.
    pub choices: Vec<bool>,
}

impl IdealBCot {
    /// Create a new IdealBCot with seeded randomness.
    ///
    /// seed_a and seed_b are used to derive delta_a and delta_b respectively.
    /// The key generation RNG is seeded from seed_a ^ seed_b.
    pub fn new(seed_a: u64, seed_b: u64) -> Self {
        let mut rng_a = ChaCha12Rng::seed_from_u64(seed_a);
        let mut rng_b = ChaCha12Rng::seed_from_u64(seed_b);
        let delta_a = Delta::random(&mut rng_a);
        let delta_b = Delta::random(&mut rng_b);
        let rng = ChaCha12Rng::seed_from_u64(seed_a ^ seed_b);
        Self { delta_a, delta_b, rng }
    }

    /// Party A is the sender; Party B is the receiver with choice bits.
    ///
    /// A's correlation key is delta_b.
    /// For each choice bit b:
    ///   - A generates K[0] (LSB cleared to 0)
    ///   - B receives mac = K[0] XOR b*delta_b  (i.e., K[0].auth(b, delta_b))
    pub fn transfer_a_to_b(&mut self, choices: &[bool]) -> BcotOutput {
        let mut sender_keys = Vec::with_capacity(choices.len());
        let mut receiver_macs = Vec::with_capacity(choices.len());

        for &b in choices {
            let mut k0_block = Block::random(&mut self.rng);
            k0_block.set_lsb(false);
            let k0 = Key::from(k0_block);
            let mac = k0.auth(b, &self.delta_b);
            sender_keys.push(k0);
            receiver_macs.push(mac);
        }

        BcotOutput {
            sender_keys,
            receiver_macs,
            choices: choices.to_vec(),
        }
    }

    /// Party B is the sender; Party A is the receiver with choice bits.
    ///
    /// B's correlation key is delta_a.
    /// For each choice bit b:
    ///   - B generates K[0] (LSB cleared to 0)
    ///   - A receives mac = K[0] XOR b*delta_a  (i.e., K[0].auth(b, delta_a))
    pub fn transfer_b_to_a(&mut self, choices: &[bool]) -> BcotOutput {
        let mut sender_keys = Vec::with_capacity(choices.len());
        let mut receiver_macs = Vec::with_capacity(choices.len());

        for &b in choices {
            let mut k0_block = Block::random(&mut self.rng);
            k0_block.set_lsb(false);
            let k0 = Key::from(k0_block);
            let mac = k0.auth(b, &self.delta_a);
            sender_keys.push(k0);
            receiver_macs.push(mac);
        }

        BcotOutput {
            sender_keys,
            receiver_macs,
            choices: choices.to_vec(),
        }
    }

    /// Converts a BcotOutput to Vec<AuthBitShare> from the perspective where the SENDER holds the key.
    ///
    /// For position i: AuthBitShare { key: output.sender_keys[i], mac: output.receiver_macs[i], value: output.choices[i] }
    ///
    /// These represent: sender holds the key, receiver holds the mac.
    ///
    /// NOTE: Do NOT call this with reversed roles to make the receiver hold the key — that would
    /// require casting receiver_macs to Key, which violates the Key LSB=0 invariant. Instead,
    /// run a separate transfer_b_to_a call where B is the sender, and use B's sender_keys directly.
    pub fn output_to_auth_bit_shares_a_holds_key(output: &BcotOutput) -> Vec<AuthBitShare> {
        output
            .sender_keys
            .iter()
            .zip(output.receiver_macs.iter())
            .zip(output.choices.iter())
            .map(|((key, mac), &value)| AuthBitShare {
                key: *key,
                mac: *mac,
                value,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: transfer_a_to_b with all-false choices.
    /// When b=false, mac = K[0] XOR 0 = K[0], so receiver_mac == sender_key block.
    #[test]
    fn test_transfer_a_to_b_all_false() {
        let mut bcot = IdealBCot::new(42, 99);
        let choices = vec![false; 8];
        let out = bcot.transfer_a_to_b(&choices);

        for (i, &b) in choices.iter().enumerate() {
            let want_mac = out.sender_keys[i].auth(b, &bcot.delta_b);
            assert_eq!(out.receiver_macs[i], want_mac,
                "Test 1: MAC mismatch at position {}", i);
            // When b=false, mac block == key block
            assert_eq!(out.receiver_macs[i].as_block(), out.sender_keys[i].as_block(),
                "Test 1: receiver_mac should equal sender_key block when choice=false at position {}", i);
        }
    }

    /// Test 2: transfer_a_to_b with all-true choices.
    /// When b=true, mac = K[0] XOR delta_b, so mac block == key block XOR delta_b.
    #[test]
    fn test_transfer_a_to_b_all_true() {
        let mut bcot = IdealBCot::new(42, 99);
        let choices = vec![true; 8];
        let out = bcot.transfer_a_to_b(&choices);

        for (i, &b) in choices.iter().enumerate() {
            let want_mac = out.sender_keys[i].auth(b, &bcot.delta_b);
            assert_eq!(out.receiver_macs[i], want_mac,
                "Test 2: MAC mismatch at position {}", i);
            // When b=true, mac block == key block XOR delta_b
            let expected_block = out.sender_keys[i].as_block() ^ bcot.delta_b.as_block();
            assert_eq!(*out.receiver_macs[i].as_block(), expected_block,
                "Test 2: receiver_mac should equal sender_key XOR delta_b at position {}", i);
        }
    }

    /// Test 3: transfer_b_to_a with mixed choices.
    /// Verify mac == key XOR bit*delta_a for each position.
    #[test]
    fn test_transfer_b_to_a_mixed() {
        let mut bcot = IdealBCot::new(42, 99);
        let choices = vec![false, true, false, true, true, false, true, false];
        let out = bcot.transfer_b_to_a(&choices);

        for (i, &b) in choices.iter().enumerate() {
            let want_mac = out.sender_keys[i].auth(b, &bcot.delta_a);
            assert_eq!(out.receiver_macs[i], want_mac,
                "Test 3: MAC mismatch at position {} (choice={})", i, b);
        }
    }

    /// Test 4: All returned AuthBitShares pass verify() with correct delta.
    #[test]
    fn test_auth_bit_shares_verify() {
        let mut bcot = IdealBCot::new(123, 456);
        let choices_a = vec![false, true, false, true];
        let choices_b = vec![true, false, true, false];

        let out_a = bcot.transfer_a_to_b(&choices_a);
        let out_b = bcot.transfer_b_to_a(&choices_b);

        // A holds key, verified against delta_b
        let shares_a = IdealBCot::output_to_auth_bit_shares_a_holds_key(&out_a);
        for (i, share) in shares_a.iter().enumerate() {
            // share.key is A's key, share.mac = key.auth(bit, delta_b)
            // verify() checks: share.mac == share.key.auth(share.bit(), delta)
            share.verify(&bcot.delta_b);
            assert_eq!(share.value, choices_a[i], "Test 4a: bit mismatch at position {}", i);
        }

        // B holds key (from transfer_b_to_a), verified against delta_a
        let shares_b = IdealBCot::output_to_auth_bit_shares_a_holds_key(&out_b);
        for (i, share) in shares_b.iter().enumerate() {
            share.verify(&bcot.delta_a);
            assert_eq!(share.value, choices_b[i], "Test 4b: bit mismatch at position {}", i);
        }
    }

    /// Test 5: All Key values have LSB == 0.
    #[test]
    fn test_key_lsb_is_zero() {
        let mut bcot = IdealBCot::new(7, 13);
        let choices = vec![false, true, false, true, false, true, false, true];

        let out_a = bcot.transfer_a_to_b(&choices);
        let out_b = bcot.transfer_b_to_a(&choices);

        for (i, key) in out_a.sender_keys.iter().enumerate() {
            assert!(!key.as_block().lsb(),
                "Test 5a: Key LSB must be 0 at position {} (transfer_a_to_b)", i);
        }
        for (i, key) in out_b.sender_keys.iter().enumerate() {
            assert!(!key.as_block().lsb(),
                "Test 5b: Key LSB must be 0 at position {} (transfer_b_to_a)", i);
        }
    }

    /// Test 6: Stress test — IdealBCot can generate 256 COT pairs without panic.
    #[test]
    fn test_stress_256_pairs() {
        let mut bcot = IdealBCot::new(999, 888);
        let choices: Vec<bool> = (0..256).map(|i| (i % 3) == 0).collect();

        let out_a = bcot.transfer_a_to_b(&choices);
        let out_b = bcot.transfer_b_to_a(&choices);

        assert_eq!(out_a.sender_keys.len(), 256, "Test 6a: expected 256 sender keys from transfer_a_to_b");
        assert_eq!(out_a.receiver_macs.len(), 256, "Test 6a: expected 256 receiver macs from transfer_a_to_b");
        assert_eq!(out_b.sender_keys.len(), 256, "Test 6b: expected 256 sender keys from transfer_b_to_a");
        assert_eq!(out_b.receiver_macs.len(), 256, "Test 6b: expected 256 receiver macs from transfer_b_to_a");

        // Spot-check a few MACs
        for i in [0, 64, 128, 192, 255] {
            let b = choices[i];
            let want_a = out_a.sender_keys[i].auth(b, &bcot.delta_b);
            assert_eq!(out_a.receiver_macs[i], want_a,
                "Test 6a: MAC mismatch at position {}", i);
            let want_b = out_b.sender_keys[i].auth(b, &bcot.delta_a);
            assert_eq!(out_b.receiver_macs[i], want_b,
                "Test 6b: MAC mismatch at position {}", i);
        }
    }
}
