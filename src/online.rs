//! Online phase primitives that span both garbler and ev views.
//!
//! Hosts `block_check_zero` (per-index full-block equality) and
//! `block_hash_check_zero` (the paper's `H({V_w})` digest). `open()` and its
//! wrong-delta negative test are deferred per Phase 8 CONTEXT.md D-01 — they
//! will live in this module once the message-passing design is settled.

use crate::aes::FIXED_KEY_AES;
use crate::block::Block;

/// Block-form CheckZero — paper-faithful per-index full-block equality.
///
/// Each party computes its own share-block of a quantity that, for honest
/// parties, must reconstruct to zero (e.g. `[e_a D_ev]` per `5_online.tex`
/// §240–246, or `[c_α D_ev]` per `6_total.tex` §215–222). Honest parties'
/// share-blocks must satisfy `gen_block[i] == eval_block[i]` because their
/// XOR is `bit · δ` and `bit = 0`.
///
/// Detection is **full-block** (not LSB-only): any tampering that produces
/// `gen_block[i] ⊕ eval_block[i] ≠ 0` is caught, regardless of which bit was
/// flipped. Replaces the prior `check_zero(&[AuthBitShare], delta)` glue
/// layer, whose detection power was limited to LSB-flipping tampers.
///
/// SIMULATION ONLY in this in-process testbed: takes both parties' block
/// vectors directly. In a real two-party run, each party hashes its own
/// blocks via `block_hash_check_zero` and parties exchange digests; matching
/// digests imply per-index equality by collision-resistance of the hash.
///
/// Returns:
/// - `true` ("pass") if `gb_blocks.len() == ev_blocks.len()` and every
///   per-index pair is equal.
/// - `false` ("abort") on the first mismatch (or length mismatch).
pub fn block_check_zero(gb_blocks: &[Block], ev_blocks: &[Block]) -> bool {
    if gb_blocks.len() != ev_blocks.len() {
        return false;
    }
    for (g, e) in gb_blocks.iter().zip(ev_blocks.iter()) {
        if g != e {
            return false;
        }
    }
    true
}

/// Paper-faithful `H({V_w})` digest of a Block-form CheckZero share vector.
///
/// Models `5_online.tex` §226–247 / `6_total.tex` §205–215: each party hashes
/// its own assembled per-wire share-blocks and exchanges the digest; matching
/// digests imply per-index `gen_block[i] == eval_block[i]` for every wire,
/// which (per Lemma `lem:error`) implies zero error.
///
/// Construction: `h_0 = 0; h_{i+1} = cr(h_i ⊕ block_i)` using the fixed-key
/// AES correlation-robust hash (`src/aes.rs`). One-pass O(n+m) cost matching
/// the paper's `H_ccrnd` invocation shape.
///
/// Empty slice returns `Block::default()` (all zeros).
pub fn block_hash_check_zero(blocks: &[Block]) -> Block {
    let mut h = Block::default();
    for block in blocks {
        h = FIXED_KEY_AES.cr(h ^ *block);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_check_zero_passes_on_per_index_equality() {
        // Honest parties: gen_block[i] == eval_block[i] ⇔ combined XOR = 0.
        let blocks: Vec<Block> = (0..8).map(|i| Block::from([i as u8; 16])).collect();
        assert!(block_check_zero(&blocks, &blocks),
            "identical block vectors must pass (combined = 0 per index)");
    }

    #[test]
    fn block_check_zero_fails_on_any_mismatch() {
        // Single-block mismatch must abort. Tampering at any non-LSB bit is
        // detected — full-block comparison, not LSB extraction.
        let mut a: Vec<Block> = (0..4).map(|i| Block::from([i as u8; 16])).collect();
        let b = a.clone();
        // Flip a non-LSB bit at index 2 — would have been undetected by the
        // prior LSB-only check_zero.
        let mut bytes: [u8; 16] = a[2].to_bytes();
        bytes[8] ^= 0x01; // flip a middle byte
        a[2] = Block::from(bytes);
        assert!(!block_check_zero(&a, &b),
            "non-LSB-bit mismatch must be detected (full-block comparison)");
    }

    #[test]
    fn block_check_zero_fails_on_length_mismatch() {
        let a: Vec<Block> = (0..4).map(|i| Block::from([i as u8; 16])).collect();
        let b: Vec<Block> = (0..3).map(|i| Block::from([i as u8; 16])).collect();
        assert!(!block_check_zero(&a, &b),
            "length mismatch must abort");
    }

    #[test]
    fn block_check_zero_passes_on_empty_slices() {
        assert!(block_check_zero(&[], &[]),
            "empty vector pair is vacuously equal — must pass");
    }

    #[test]
    fn block_hash_check_zero_matches_on_equal_inputs() {
        let blocks: Vec<Block> = (0..16).map(|i| Block::from([i as u8; 16])).collect();
        let h_a = block_hash_check_zero(&blocks);
        let h_b = block_hash_check_zero(&blocks);
        assert_eq!(h_a, h_b,
            "block_hash_check_zero is deterministic; equal inputs → equal digests");
    }

    #[test]
    fn block_hash_check_zero_differs_on_unequal_inputs() {
        let a: Vec<Block> = (0..4).map(|i| Block::from([i as u8; 16])).collect();
        let mut b = a.clone();
        let mut bytes: [u8; 16] = b[1].to_bytes();
        bytes[5] ^= 0x80; // flip a non-LSB bit
        b[1] = Block::from(bytes);
        assert_ne!(block_hash_check_zero(&a), block_hash_check_zero(&b),
            "different block vectors must produce different digests w.h.p.");
    }

    #[test]
    fn block_hash_check_zero_empty_returns_zero_block() {
        assert_eq!(block_hash_check_zero(&[]), Block::default(),
            "empty slice yields zero block per the H_0 = 0 convention");
    }
}
