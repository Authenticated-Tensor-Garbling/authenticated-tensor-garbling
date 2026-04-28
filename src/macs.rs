use std::ops::Add;

use crate::block::Block;
use serde::{Deserialize, Serialize};

/// MAC.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mac(Block);

impl Mac {
    /// Creates a new MAC.
    #[inline]
    pub(crate) fn new(block: Block) -> Self {
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

    /// Returns the MAC encoded as bytes.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the MAC block.
    #[inline]
    pub fn as_block(&self) -> &Block {
        &self.0
    }

    /// Converts a slice of MACs to a slice of blocks.
    #[inline]
    pub fn as_blocks(slice: &[Self]) -> &[Block] {
        // Safety:
        // Mac is a newtype of block.
        unsafe { &*(slice as *const [Self] as *const [Block]) }
    }
}

impl From<Mac> for Block {
    #[inline]
    fn from(mac: Mac) -> Block {
        mac.0
    }
}

impl From<Block> for Mac {
    #[inline]
    fn from(block: Block) -> Mac {
        Mac(block)
    }
}

impl Add<Mac> for Mac {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Mac) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Add<&Mac> for Mac {
    type Output = Self;

    #[inline]
    fn add(self, rhs: &Mac) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Add<Mac> for &Mac {
    type Output = Mac;

    #[inline]
    fn add(self, rhs: Mac) -> Mac {
        Mac(self.0 ^ rhs.0)
    }
}

impl Add<&Mac> for &Mac {
    type Output = Mac;

    #[inline]
    fn add(self, rhs: &Mac) -> Mac {
        Mac(self.0 ^ rhs.0)
    }
}

