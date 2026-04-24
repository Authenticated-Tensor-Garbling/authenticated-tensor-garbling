//! Online phase primitives that span both garbler and evaluator views.
//!
//! Currently hosts `check_zero()` only. `open()` (ONL-01) and its wrong-delta
//! negative test (ONL-02) are deferred to a later phase per Phase 8 CONTEXT.md
//! D-01 — they will live in this module once the message-passing design is
//! settled.

use crate::sharing::AuthBitShare;
use crate::delta::Delta;

/// Verifies that the per-gate consistency-check vector reconstructs to zero
/// AND that its IT-MAC under `delta_ev` is valid.
///
/// # Stub
///
/// Implementation pending — always returns false. Tests should fail on the
/// pass paths until the implementation is filled in (RED phase).
pub fn check_zero(_c_gamma_shares: &[AuthBitShare], _delta_ev: &Delta) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::keys::Key;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    #[test]
    fn test_check_zero_passes_on_zero_bit_with_valid_mac() {
        let mut rng = ChaCha12Rng::seed_from_u64(1);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(false, &delta);
        let share = AuthBitShare { key, mac, value: false };
        assert!(check_zero(&[share], &delta),
            "honest c_gamma=0 share with valid MAC must pass");
    }

    #[test]
    fn test_check_zero_fails_on_nonzero_bit() {
        let mut rng = ChaCha12Rng::seed_from_u64(2);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(true, &delta);
        let share = AuthBitShare { key, mac, value: true };
        assert!(!check_zero(&[share], &delta),
            "share with value=true must abort regardless of MAC validity");
    }

    #[test]
    fn test_check_zero_fails_on_invalid_mac() {
        let mut rng = ChaCha12Rng::seed_from_u64(3);
        let delta = Delta::random(&mut rng);
        let wrong_delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(false, &wrong_delta);
        let share = AuthBitShare { key, mac, value: false };
        assert!(!check_zero(&[share], &delta),
            "share with mac authenticated under the wrong delta must abort");
    }

    #[test]
    fn test_check_zero_passes_on_empty_slice() {
        let mut rng = ChaCha12Rng::seed_from_u64(4);
        let delta = Delta::random(&mut rng);
        assert!(check_zero(&[], &delta),
            "empty c_gamma slice is vacuously zero — must pass");
    }

    #[test]
    fn test_check_zero_short_circuits_on_mixed_slice() {
        let mut rng = ChaCha12Rng::seed_from_u64(5);
        let delta = Delta::random(&mut rng);

        let make_zero = |rng: &mut ChaCha12Rng| {
            let key = Key::new(Block::random(rng));
            let mac = key.auth(false, &delta);
            AuthBitShare { key, mac, value: false }
        };
        let make_bad_bit = |rng: &mut ChaCha12Rng| {
            let key = Key::new(Block::random(rng));
            let mac = key.auth(true, &delta);
            AuthBitShare { key, mac, value: true }
        };

        let s0 = make_zero(&mut rng);
        let s1 = make_bad_bit(&mut rng);
        let s2 = make_zero(&mut rng);

        assert!(!check_zero(&[s0, s1, s2], &delta),
            "any failing share in the slice must cause check_zero to abort");
    }
}
