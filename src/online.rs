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
/// `c_gamma_shares` is the caller-assembled vector of D_ev-authenticated
/// shares of `c_gamma`, where (per Construction 3 in 5_online.tex line 206):
///
///   c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta)
///                                  XOR (L_gamma XOR l_gamma)
///           = v_alpha ⊗ v_beta XOR v_gamma   [= 0 for honest parties]
///
/// # Caller contract (per CONTEXT.md D-08)
///
/// `check_zero` is a thin primitive — it does NOT know about the protocol
/// structs or the c_gamma assembly. The caller MUST assemble each `share`
/// in `c_gamma_shares` with:
///   - `share.value` = full reconstructed c_gamma bit (gen.value XOR ev.value)
///   - `share.key`   = XOR of all gen-side B-keys contributing to this gate
///   - `share.mac`   = `share.key.auth(share.value, delta_ev)`  ← freshly computed
///
/// **Do NOT** use the `AuthBitShare::add` (`+`) operator to combine
/// cross-party shares directly. The two parties' MACs are authenticated
/// under opposite deltas (gen side vs eval side in the bCOT structure) and
/// will NOT combine correctly without recomputing the MAC — a naive XOR of
/// gen.mac and ev.mac does not yield a valid IT-MAC under either party's
/// delta. Always recompute `mac = key.auth(value, delta_ev)` after
/// accumulating the full reconstructed bit and the combined key.
///
/// See `assemble_c_gamma_shares` in the test module (src/lib.rs) for the
/// reference implementation of this pattern.
///
/// # Returns
///
/// - `true`  ("pass" / "do not abort") if every share has `value == false`
///   AND `mac == key.auth(value, delta_ev)`.
/// - `false` ("abort") on the first share that fails either check.
///
/// Empty slice returns `true` (vacuously zero).
pub fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool {
    for share in c_gamma_shares {
        // (1) Reconstructed bit must be 0. The caller pre-XORed the two
        // parties' shares, so `share.value` IS the reconstructed bit.
        if share.value {
            return false;
        }
        // (2) IT-MAC invariant under delta_ev.
        // mac == key XOR (value * delta_ev) === key.auth(value, delta_ev).
        // When value is false (the only path that reaches here) this
        // simplifies to `mac == key` (XOR with zero block).
        let want = share.key.auth(share.value, delta_ev);
        if share.mac != want {
            return false;
        }
    }
    true
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
        // When value == false, key.auth(false, delta) = key.0 XOR Block::ZERO = key.0,
        // which is independent of delta. So "wrong delta" only produces a distinguishable
        // MAC when value == true: key.auth(true, wrong_delta) = key.0 XOR wrong_delta.0,
        // which differs from key.auth(true, delta) = key.0 XOR delta.0 (with high probability).
        //
        // Construct a share with value=false but whose mac was produced as
        // key.auth(true, delta) — i.e., mac = key.0 XOR delta.0, which does NOT equal
        // key.auth(false, delta) = key.0. This exercises the MAC-mismatch branch
        // in check_zero() for a zero-value share with a corrupted MAC.
        let mut rng = ChaCha12Rng::seed_from_u64(3);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        // Deliberately build mac as if bit were true — this corrupts the IT-MAC invariant
        // for a value=false share, because mac should equal key.0 but instead equals
        // key.0 XOR delta.0 (nonzero with overwhelming probability).
        let corrupted_mac = key.auth(true, &delta);
        let share = AuthBitShare { key, mac: corrupted_mac, value: false };
        assert!(!check_zero(&[share], &delta),
            "share with corrupted mac (wrong bit in auth) must abort even when value=false");
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
