use crate::block::Block;

use rand::Rng;
use std::ops::{BitXor, BitXorAssign};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Delta(Block);

impl Delta {
    /// Creates a new Delta, setting the pointer bit to 1.
    #[inline]
    pub fn new(mut value: Block) -> Self {
        value.set_lsb(true);
        Self(value)
    }

    #[inline]
    pub fn lsb(&self) -> bool {
        self.0.lsb()
    }

    /// Set the pointer bit of the Delta
    #[inline]
    pub fn set_lsb(mut self, value: bool) -> Self {
        self.0.set_lsb(value);
        self
    }

    /// Generate a random Δ_gb (the garbler's global key; LSB = 1).
    ///
    /// Pairs with `Delta::random_ev` (LSB=0) so that
    /// `lsb(Δ_gb ⊕ Δ_ev) = 1`, the precondition required by
    /// Π_LeakyTensor Construction 2's masked reveal (paper §F).
    #[inline]
    pub fn random_gb<R: Rng>(rng: &mut R) -> Self {
        // Self::new() sets the LSB to 1
        Self::new(Block::from(rng.random::<[u8; 16]>()))
    }

    /// Creates a new Delta with an explicit pointer-bit value.
    ///
    /// Used when the two parties' deltas must satisfy `lsb(Δ_gb ⊕ Δ_ev) == 1`
    /// (paper §F requires this for Π_LeakyTensor Construction 2's masked reveal).
    #[inline]
    pub fn new_with_lsb(mut value: Block, lsb_value: bool) -> Self {
        value.set_lsb(lsb_value);
        Self(value)
    }

    /// Generate a random Δ_ev (the ev's global key; LSB cleared to 0).
    ///
    /// Pairs with `Delta::random_gb` (LSB=1) so that
    /// `lsb(Δ_gb ⊕ Δ_ev) = 1`, the precondition required by
    /// Π_LeakyTensor Construction 2's masked reveal (paper §F).
    #[inline]
    pub fn random_ev<R: Rng>(rng: &mut R) -> Self {
        Self::new_with_lsb(Block::from(rng.random::<[u8; 16]>()), false)
    }

    #[inline]
    pub fn mul_bool(self, value: bool) -> Block {
        if value {
            self.0
        } else {
            Block::ZERO
        }
    }

    /// Returns the inner block
    #[inline]
    pub fn as_block(&self) -> &Block {
        &self.0
    }

    /// Returns the inner block
    #[inline]
    pub fn into_inner(self) -> Block {
        self.0
    }
}

impl From<Delta> for Block {
    fn from(val: Delta) -> Self {
        val.0
    }
}

impl AsRef<Block> for Delta {
    fn as_ref(&self) -> &Block {
        &self.0
    }
}

impl BitXor<Block> for Delta {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: Block) -> Block {
        self.0 ^ rhs
    }
}

impl BitXor<Delta> for Block {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: Delta) -> Block {
        self ^ rhs.0
    }
}

impl BitXor<Block> for &Delta {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: Block) -> Block {
        self.0 ^ rhs
    }
}

impl BitXor<&Block> for Delta {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: &Block) -> Block {
        self.0 ^ rhs
    }
}

impl BitXor<&Delta> for Block {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: &Delta) -> Block {
        self ^ rhs.0
    }
}

impl BitXor<Delta> for &Block {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: Delta) -> Block {
        self ^ rhs.0
    }
}

impl BitXor<&Delta> for &Block {
    type Output = Block;

    #[inline]
    fn bitxor(self, rhs: &Delta) -> Block {
        self ^ rhs.0
    }
}

impl BitXorAssign<Delta> for Block {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Delta) {
        *self ^= rhs.0;
    }
}