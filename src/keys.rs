use std::ops::{Add, BitXor, BitXorAssign};
use std::fmt::Display;

use crate::block::Block;
use crate::delta::Delta;
use crate::macs::Mac;

use rand::Rng;

/// MAC key.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Key(Block);

impl Key {
    /// Safe constructor that enforces the `Key.lsb() == 0` invariant at construction time.
    ///
    /// The LSB of a `Key` must be zero so the prover can store the authenticated bit in
    /// `LSB(MAC)` (see `auth`). This constructor clears the LSB before wrapping the block.
    ///
    /// Prefer `Key::new` over `Key::from(block)` whenever the caller has not already
    /// cleared the LSB. `From<Block>` is retained as a zero-cost cast for callers that
    /// have already enforced the invariant themselves (for example via `Key::adjust`).
    #[inline]
    pub fn new(mut block: Block) -> Self {
        block.set_lsb(false);
        Self(block)
    }

    /// Returns the pointer bit.
    #[inline]
    pub fn pointer(&self) -> bool {
        self.0.lsb()
    }

    /// Sets the pointer bit.
    #[inline]
    pub fn set_pointer(&mut self, bit: bool) {
        self.0.set_lsb(bit);
    }

    /// Adjusts the truth value of the corresponding MAC.
    #[inline]
    pub fn adjust(&mut self, adjust: bool, delta: &Delta) {
        self.0 ^= if adjust {
            delta.as_block()
        } else {
            &Block::ZERO
        };

        // Setting LSB(key) == 0 to enable the prover to store the authenticated bit in
        // LSB(MAC).
        self.0.set_lsb(false);
    }

    // /// Commits to the MACs of a value.
    // #[inline]
    // pub fn commit(&self, id: u64, delta: &Delta, hasher: &FixedKeyAes) -> MacCommitment {
    //     let mut macs = [self.0, self.0 ^ delta.as_block()];
    //     let tweak = Block::from((id as u128).to_be_bytes());
    //     hasher.tccr_many(&[tweak, tweak], &mut macs);
    //     MacCommitment(macs)
    // }

    /// Returns a MAC for the given bit.
    #[inline]
    pub fn auth(&self, bit: bool, delta: &Delta) -> Mac {
        Mac::new(self.0 ^ if bit { delta.as_block() } else { &Block::ZERO })
    }

    /// Returns the key block.
    #[inline]
    pub fn as_block(&self) -> &Block {
        &self.0
    }

    /// Converts a slice of keys to a slice of blocks.
    #[inline]
    pub fn as_blocks(slice: &[Self]) -> &[Block] {
        // Safety:
        // Key is a newtype of block.
        unsafe { &*(slice as *const [Self] as *const [Block]) }
    }

    /// Converts a `Vec` of blocks to a `Vec` of keys.
    #[inline]
    pub fn from_blocks(blocks: Vec<Block>) -> Vec<Self> {
        // Safety:
        // Key is a newtype of block.
        unsafe { std::mem::transmute(blocks) }
    }

    /// Converts a `Vec` of keys to a `Vec` of blocks.
    #[inline]
    pub fn into_blocks(keys: Vec<Self>) -> Vec<Block> {
        // Safety:
        // Key is a newtype of block.
        unsafe { std::mem::transmute(keys) }
    }

    #[inline]
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        Self::new(Block::random(rng))
    }
}

impl From<Key> for Block {
    #[inline]
    fn from(key: Key) -> Block {
        key.0
    }
}

impl From<Block> for Key {
    #[inline]
    fn from(block: Block) -> Key {
        Key(block)
    }
}

impl From<[u8; 16]> for Key {
    #[inline]
    fn from(bytes: [u8; 16]) -> Self {
        Self(Block::from(bytes))
    }
}

impl From<Key> for [u8; 16] {
    #[inline]
    fn from(key: Key) -> Self {
        key.0.into()
    }
}

impl Add<Key> for Key {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Key) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Add<&Key> for Key {
    type Output = Self;

    #[inline]
    fn add(self, rhs: &Key) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Add<Key> for &Key {
    type Output = Key;

    #[inline]
    fn add(self, rhs: Key) -> Key {
        Key(self.0 ^ rhs.0)
    }
}

impl Add<&Key> for &Key {
    type Output = Key;

    #[inline]
    fn add(self, rhs: &Key) -> Key {
        Key(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for Key {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl BitXorAssign<&Key> for Key {
    #[inline]
    fn bitxor_assign(&mut self, rhs: &Self) {
        self.0 ^= &rhs.0;
    }
}

impl BitXor for Key {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXor<&Key> for Key {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: &Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXor<Key> for &Key {
    type Output = Key;

    #[inline]
    fn bitxor(self, rhs: Key) -> Self::Output {
        Key(self.0 ^ rhs.0)
    }
}

impl BitXor<&Key> for &Key {
    type Output = Key;

    #[inline]
    fn bitxor(self, rhs: &Key) -> Self::Output {
        Key(self.0 ^ rhs.0)
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    #[test]
    fn test_key_new_clears_lsb_when_set() {
        let b = Block::new([0xFF; 16]);
        assert!(b.lsb());
        let k = Key::new(b);
        assert!(!k.as_block().lsb(), "Key::new must clear LSB");
    }

    #[test]
    fn test_key_new_idempotent_when_already_cleared() {
        let mut b = Block::new([0xFF; 16]);
        b.set_lsb(false);
        let k = Key::new(b);
        assert!(!k.as_block().lsb());
    }

    #[test]
    fn test_key_random_lsb_is_zero() {
        let mut rng = ChaCha12Rng::seed_from_u64(0xC0FFEE);
        for _ in 0..64 {
            let k = Key::random(&mut rng);
            assert!(!k.as_block().lsb(), "Key::random must produce lsb==0");
        }
    }

    #[test]
    fn test_key_from_block_preserves_lsb_for_backward_compat() {
        // From<Block> is documented as zero-cost cast; it must NOT clear LSB.
        let mut b = Block::new([0xFF; 16]);
        b.set_lsb(true);
        let k = Key::from(b);
        assert!(k.as_block().lsb(), "Key::from<Block> must not clear LSB (zero-cost cast)");
    }
}
