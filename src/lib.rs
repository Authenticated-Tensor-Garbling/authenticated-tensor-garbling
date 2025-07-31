pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;

pub mod aes;

pub mod circuit;

pub mod fpre;
pub mod auth_gen;
pub mod auth_eval;



use crate::block::Block;

const CSP: usize = 128;
const SSP: usize = 40;

const BYTES_PER_GATE: usize = 32;
const KB: usize = 1024;
const MAX_BATCH_SIZE: usize = 4 * KB;
pub(crate) const DEFAULT_BATCH_SIZE: usize = MAX_BATCH_SIZE / BYTES_PER_GATE;

/// Block for public 0 MAC.
pub(crate) const MAC_ZERO: Block = Block::new([
    146, 239, 91, 41, 80, 62, 197, 196, 204, 121, 176, 38, 171, 216, 63, 120,
]);
/// Block for public 1 MAC.
pub(crate) const MAC_ONE: Block = Block::new([
    219, 104, 26, 50, 91, 130, 201, 178, 144, 31, 95, 155, 206, 113, 5, 103,
]);