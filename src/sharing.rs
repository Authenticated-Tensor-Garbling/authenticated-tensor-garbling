use crate::block::Block;
use crate::keys::Key;
use crate::macs::Mac;
use crate::delta::Delta;

use std::ops::Add;
use rand_chacha::ChaCha12Rng;
use rand::Rng;

#[derive(Debug, Clone, Copy)]

pub struct InputSharing {
    pub gen_share: Block,
    pub eval_share: Block,
}

impl InputSharing {
    pub fn bit(&self) -> bool {
        if self.gen_share == self.eval_share {
            false
        } else {
            true
        }
    }
}

/// AuthBitShare consisting of a bool and a (key, mac) pair
#[derive(Debug, Clone, Default, Copy)]
pub struct AuthBitShare {
    /// Key
    pub key: Key,
    /// MAC
    pub mac: Mac,
    /// Value
    pub value: bool,
}

impl AuthBitShare {
    /// Retrieves the embedded bit from the LSB of `mac`.
    #[inline]
    pub fn bit(&self) -> bool {
        self.value
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

/// Builds one `AuthBitShare` from a bit and delta, ensuring `key.lsb()==false`.
pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
    let key: Key = Key::from(rng.random::<[u8; 16]>());
    let mac: Mac = key.auth(bit, delta);
    AuthBitShare { key, mac, value: bit }
}

/// Represents an auth bit [x] = [r]+[s] where [r] is known to gen, auth by eval and [s] is known to eval, auth by gen.
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
    pub fn verify(&self, delta_a: &Delta, delta_b: &Delta) {
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
        r.verify(delta_b);
        s.verify(delta_a);
    }
}