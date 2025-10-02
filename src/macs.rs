use std::ops::Add;

use crate::block::Block;
use serde::{Deserialize, Serialize};

/// Block for public 0 MAC.
pub(crate) const MAC_ZERO: Block = Block::new([
    146, 239, 91, 41, 80, 62, 197, 196, 204, 121, 176, 38, 171, 216, 63, 120,
]);
/// Block for public 1 MAC.
pub(crate) const MAC_ONE: Block = Block::new([
    219, 104, 26, 50, 91, 130, 201, 178, 144, 31, 95, 155, 206, 113, 5, 103,
]);


/// MAC.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mac(Block);

impl Mac {
    /// Public MACs.
    pub const PUBLIC: [Mac; 2] = [Mac(MAC_ZERO), Mac(MAC_ONE)];

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

    /// Converts a `Vec` of blocks to a `Vec` of MACs.
    #[inline]
    pub fn from_blocks(blocks: Vec<Block>) -> Vec<Self> {
        // Safety:
        // Mac is a newtype of block.
        unsafe { std::mem::transmute(blocks) }
    }

    /// Returns MACs for public data.
    #[inline]
    pub fn public(data: impl IntoIterator<Item = bool>) -> impl Iterator<Item = Self> {
        data.into_iter().map(|bit| Self::PUBLIC[bit as usize])
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

