use crate::block::Block;
use crate::keys::Key;
use crate::macs::Mac;
use crate::delta::Delta;

use std::ops::Add;
use rand::{CryptoRng, Rng};

#[derive(Debug, Clone, Copy)]

pub struct InputSharing {
    pub gen_share: Block,
    pub eval_share: Block,
}

impl InputSharing {
    /// Returns whether the two parties' share blocks differ.
    ///
    /// Under the BDOZ-style XOR sharing used here, a bit `b` is encoded as
    /// `gen_share XOR eval_share`. This method returns `gen_share != eval_share`
    /// — it does **not** recover the underlying input bit of a masked wire
    /// (which would require knowing both parties' deltas). Historical name
    /// `bit()` was ambiguous; use this instead.
    #[inline]
    pub fn shares_differ(&self) -> bool {
        self.gen_share != self.eval_share
    }
}

/// One party's view of an authenticated bit.
///
/// In the two-party BDOZ-style IT-MAC sharing, each bit `b` is held by two
/// parties simultaneously. This struct holds **one party's** view:
/// - `key`: the bCOT sender's key for this position (`lsb() == 0` invariant)
/// - `mac`: the bCOT receiver's chosen MAC, authenticating `value` under the
///          **verifying party's** delta
/// - `value`: the committed bit the holder claims
///
/// Invariant: `mac == key.auth(value, verifier_delta)` where `verifier_delta`
/// is the other party's global correlation key. `AuthBitShare::verify(&delta)`
/// checks this equation.
#[derive(Debug, Clone, Default, Copy)]
pub struct AuthBitShare {
    /// bCOT sender key for this position. `Key::new` enforces `lsb() == 0`.
    /// Crate-internal callers (e.g. `verify_cross_party`, `AuthBit::verify`)
    /// reach this directly to assemble swapped shares for cross-party MAC
    /// checks; external callers go through `key()`.
    pub(crate) key: Key,
    /// bCOT receiver MAC authenticating `value` under the **verifying party's**
    /// delta. Holds the IT-MAC invariant `mac == key.auth(value, verifier_delta)`
    /// only on standalone shares — cross-party reconstructions intentionally
    /// pair `key` and `mac` from different bCOT directions.
    pub(crate) mac: Mac,
    /// Committed bit this share claims. Read via `bit()`; mutate via
    /// `with_bit(bit, delta)` (recomputes the MAC under the same `key`).
    pub(crate) value: bool,
}

impl AuthBitShare {
    /// Returns the committed bit `value`.
    #[inline]
    pub fn bit(&self) -> bool {
        self.value
    }

    /// Returns a reference to the bCOT sender key (`lsb() == 0` invariant).
    #[inline]
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Returns a reference to the bCOT receiver MAC.
    #[inline]
    pub fn mac(&self) -> &Mac {
        &self.mac
    }

    /// Returns a new share with `bit` as its committed value, recomputing the
    /// MAC under the same `key` so the IT-MAC invariant
    /// `new.mac == new.key.auth(bit, delta)` holds. `delta` must be the
    /// **verifying party's** global correlation key — the same delta used
    /// when `self` was originally built.
    #[inline]
    pub fn with_bit(self, bit: bool, delta: &Delta) -> Self {
        let mac = self.key.auth(bit, delta);
        Self { key: self.key, mac, value: bit }
    }

    /// Checks that `share.mac == share.key.auth(share.bit, delta)`.
    pub fn verify(&self, delta: &Delta) {
        let want: Mac = self.key.auth(self.bit(), delta);
        assert_eq!(self.mac, want, "MAC mismatch in share");
    }
}

impl Add<AuthBitShare> for AuthBitShare {
    type Output = Self;

    #[inline]
    fn add(self, rhs: AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<&AuthBitShare> for AuthBitShare {
    type Output = Self;

    #[inline]
    fn add(self, rhs: &AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<AuthBitShare> for &AuthBitShare {
    type Output = AuthBitShare;

    #[inline]
    fn add(self, rhs: AuthBitShare) -> AuthBitShare {
        AuthBitShare {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

impl Add<&AuthBitShare> for &AuthBitShare {
    type Output = AuthBitShare;

    #[inline]
    fn add(self, rhs: &AuthBitShare) -> AuthBitShare {
        AuthBitShare {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}

/// Builds one `AuthBitShare` for the given `bit` under the verifying party's `delta`.
///
/// `delta` is the **verifying party's** global correlation key (for example,
/// when A holds the key and B holds the MAC, `delta` is B's delta). The returned
/// share satisfies the IT-MAC invariant `mac == key.auth(bit, delta)` and its
/// `key` has `lsb() == 0` (enforced by `Key::new`).
pub fn build_share<R: Rng + CryptoRng>(rng: &mut R, bit: bool, delta: &Delta) -> AuthBitShare {
    let key: Key = Key::new(Block::random(rng));
    let mac: Mac = key.auth(bit, delta);
    AuthBitShare { key, mac, value: bit }
}

/// Cross-party `AuthBitShare` MAC verification — the in-process substitute for the
/// paper's "publicly reveal with appropriate MACs".
///
/// `gen_share.key` is A's sender key; `gen_share.mac` is A's sender MAC (committed
/// under δ_ev). `eval_share.key` is B's sender key; `eval_share.mac` is B's sender
/// MAC (committed under δ_gb). The two `.verify` calls below reassemble properly
/// aligned IT-MAC pairs so that each side checks `mac == key XOR bit*delta` under
/// the correct verifier's delta. Panics with "MAC mismatch in share" on tampered
/// shares.
///
/// NOTE: do NOT call `share.verify(&delta)` directly on a raw cross-party
/// `AuthBitShare` — it will panic even on correctly-formed shares because the key
/// and MAC fields come from different bCOT directions and commit under different
/// deltas.
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_gb: &Delta,
    delta_ev: &Delta,
) {
    AuthBitShare {
        key: eval_share.key,
        mac: gen_share.mac,
        value: gen_share.value,
    }
    .verify(delta_ev);
    AuthBitShare {
        key: gen_share.key,
        mac: eval_share.mac,
        value: eval_share.value,
    }
    .verify(delta_gb);
}

/// Both parties' views of an authenticated bit, paired together.
///
/// `AuthBit` holds an `AuthBitShare` for each party (gen and eval) and is
/// used in the ideal trusted-dealer `TensorFpre` and in tests that need to
/// reconstruct the full two-party state. Compare with `AuthBitShare`, which
/// holds only one party's view.
///
/// The additive-sharing relation is `[x] = gen_share.value XOR eval_share.value`
/// (see `full_bit()`); MAC invariants are verified under each party's delta by
/// `verify(&delta_gb, &delta_ev)`.
#[derive(Debug, Clone)]
pub struct AuthBit {
    /// Generator's share of the auth bit
    pub gen_share: AuthBitShare,
    /// Evaluator's share of the auth bit
    pub eval_share: AuthBitShare,
}

impl AuthBit {
    /// Recover the full bit x = r ^ s
    pub fn full_bit(&self) -> bool {
        self.gen_share.bit() ^ self.eval_share.bit()
    }

    /// verify auth bits
    pub fn verify(&self, delta_gb: &Delta, delta_ev: &Delta) {
        // Reconstruct shares for testing
        let r = AuthBitShare {
            key: self.eval_share.key,
            mac: self.gen_share.mac,
            value: self.gen_share.bit(),
        };
        let s = AuthBitShare {
            key: self.gen_share.key,
            mac: self.eval_share.mac,
            value: self.eval_share.bit(),
        };
        r.verify(delta_ev);
        s.verify(delta_gb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    #[test]
    fn test_build_share_key_lsb_is_zero() {
        let mut rng = ChaCha12Rng::seed_from_u64(7);
        let delta = Delta::random_gb(&mut rng);
        for bit in [false, true] {
            let share = build_share(&mut rng, bit, &delta);
            assert!(!share.key.as_block().lsb(),
                "build_share must clear Key LSB (bit={})", bit);
        }
    }

    #[test]
    fn test_build_share_mac_invariant_holds() {
        let mut rng = ChaCha12Rng::seed_from_u64(11);
        let delta = Delta::random_gb(&mut rng);
        for bit in [false, true] {
            let share = build_share(&mut rng, bit, &delta);
            share.verify(&delta);   // panics on mismatch
        }
    }

    #[test]
    fn test_input_sharing_shares_differ() {
        let a = Block::new([1u8; 16]);
        let b = Block::new([1u8; 16]);
        let c = Block::new([2u8; 16]);
        assert_eq!(InputSharing { gen_share: a, eval_share: b }.shares_differ(), false);
        assert_eq!(InputSharing { gen_share: a, eval_share: c }.shares_differ(), true);
    }
}
