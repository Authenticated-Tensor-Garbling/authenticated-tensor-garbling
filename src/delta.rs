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

    /// Generate a random block using the provided RNG
    #[inline]
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        // Self::new() sets the LSB to 1
        Self::new(Block::from(rng.random::<[u8; 16]>()))
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