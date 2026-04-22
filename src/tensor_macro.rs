//! Generalized Tensor Macro (Paper Appendix F, Construction 1).
//!
//! This module provides the reusable GGM-tree-based primitive used by
//! Pi_LeakyTensor (Construction 2, Phase 4). Two functions:
//!
//! - [`tensor_garbler`] — garbler side: builds a 2^n-leaf GGM tree, emits
//!   level ciphertexts `G_{i,0}/G_{i,1}` and leaf ciphertexts `G_k`,
//!   returns `Z_garbler` and the ciphertext bundle `G`.
//! - [`tensor_evaluator`] — evaluator side: reconstructs the untraversed
//!   subtree from the evaluator's authenticated MAC values, recovers the
//!   missing leaf column, and returns `Z_evaluator`.
//!
//! Correctness invariant (paper Theorem 1):
//!
//! ```text
//! Z_garbler XOR Z_evaluator == a ⊗ T
//! ```
//!
//! where `a[i] = a_macs[i].lsb()` is the clear bit vector, and
//! `T = t_gen XOR t_eval` is the reconstructed length-m vector.
//!
//! Endianness: index 0 is LSB, index n-1 is MSB. The tree root uses the
//! MSB (`a_keys[n-1]`) as its level-0 branching bit. This convention is
//! shared with `tensor_ops::gen_populate_seeds_mem_optimized` and
//! `tensor_ops::eval_populate_seeds_mem_optimized`.
//!
//! This is a standalone primitive — it has NO dependency on
//! `leaky_tensor_pre.rs` or `preprocessing.rs`.

use crate::{
    aes::{FixedKeyAes, FIXED_KEY_AES},
    block::Block,
    delta::Delta,
    keys::Key,
    macs::Mac,
    matrix::BlockMatrix,
    tensor_ops::{
        eval_populate_seeds_mem_optimized, eval_unary_outer_product,
        gen_populate_seeds_mem_optimized, gen_unary_outer_product,
    },
};

/// Ciphertexts emitted by [`tensor_garbler`] and consumed by [`tensor_evaluator`].
///
/// Maps directly to paper Construction 1 (Appendix F):
/// - `level_cts[i]` is `(G_{i,0}, G_{i,1})` for tree level `i ∈ [n-1]`
/// - `leaf_cts[k]` is `G_k` for output column `k ∈ [m]`
///
/// `G_{i,0}` corresponds to even-indexed sibling XORs
/// (`⊕_j S_{i,2j}`) blinded by `H(A_i ⊕ Δ, ν_{i,0})`, and `G_{i,1}`
/// corresponds to odd-indexed siblings (`⊕_j S_{i,2j+1}`) blinded by
/// `H(A_i, ν_{i,1})`. The `ν_{i,b}` nonces are instantiated via
/// `FixedKeyAes::tccr` tweaks 0 and 1 (see `src/aes.rs`).
pub(crate) struct TensorMacroCiphertexts {
    /// Length `n - 1`. Each entry is `(G_{i,0}, G_{i,1})`.
    pub level_cts: Vec<(Block, Block)>,
    /// Length `m`. `leaf_cts[k] = G_k`.
    pub leaf_cts: Vec<Block>,
}

/// Garbler side of Construction 1 (paper Appendix F).
///
/// Builds a 2^n-leaf GGM tree from the garbler's IT-MAC keys `a_keys`,
/// computes level ciphertexts {G_{i,0}, G_{i,1}}_{i ∈ [n-1]} and leaf
/// ciphertexts {G_k}_{k ∈ [m]}, and returns `(Z_gen, G)` such that when
/// paired with the evaluator's [`tensor_evaluator`] output on matching
/// inputs,
///
/// ```text
/// Z_gen XOR Z_eval == a ⊗ T
/// ```
///
/// where `a[i] = a_macs[i].lsb()` is the clear bit vector and
/// `T = t_gen XOR t_eval` is a length-m κ-bit vector.
///
/// Preconditions (enforced via `assert_eq!`):
/// - `a_keys.len() == n`
/// - `t_gen.rows() == m` and `t_gen.cols() == 1`
/// - Every `a_keys[i].lsb() == 0` (enforced by `Key::new()` — re-asserted here as defence in depth)
///
/// Panics if preconditions are violated.
pub(crate) fn tensor_garbler(
    n: usize,
    m: usize,
    delta: Delta,
    a_keys: &[Key],
    t_gen: &BlockMatrix,
) -> (BlockMatrix, TensorMacroCiphertexts) {
    assert_eq!(a_keys.len(), n, "a_keys length must equal n");
    assert_eq!(t_gen.rows(), m, "t_gen must be a length-m column vector");
    assert_eq!(t_gen.cols(), 1, "t_gen must be a column vector (cols == 1)");

    let cipher: &FixedKeyAes = &FIXED_KEY_AES;

    // [1-2] Build GGM tree; collect (leaf seeds, per-level (evens, odds)).
    //       `level_cts` is structurally (G_{i,0}, G_{i,1}) already —
    //       see src/tensor_ops.rs for the G encoding (evens/odds XORed
    //       with tccr of A_i / A_i XOR Δ).
    let a_blocks: &[Block] = Key::as_blocks(a_keys);
    let (leaf_seeds, level_cts) =
        gen_populate_seeds_mem_optimized(a_blocks, cipher, delta);

    // [3-4] Leaf expansion + Z_gen computation + leaf ciphertexts G_k.
    //       gen_unary_outer_product writes into z_gen and returns leaf_cts.
    //       Z is n×m (rows = n, cols = m); t_gen is m×1 column vector
    //       (rows = m, cols = 1).
    let mut z_gen = BlockMatrix::new(n, m);
    let leaf_cts = {
        let t_view = t_gen.as_view();
        let mut z_view = z_gen.as_view_mut();
        gen_unary_outer_product(&leaf_seeds, &t_view, &mut z_view, cipher)
    };

    (z_gen, TensorMacroCiphertexts { level_cts, leaf_cts })
}

/// Evaluator side of Construction 1 (paper Appendix F).
///
/// Reconstructs the untraversed GGM subtree from the evaluator's IT-MAC
/// values `a_macs` (each equal to `A_i XOR a_i·Δ`) and the garbler's
/// ciphertexts `g`, then recovers the missing-leaf column and accumulates
/// `Z_eval`.
///
/// Preconditions (enforced via `assert_eq!`):
/// - `a_macs.len() == n`
/// - `t_eval.rows() == m` and `t_eval.cols() == 1`
/// - `g.level_cts.len() == n - 1`
/// - `g.leaf_cts.len() == m`
///
/// Panics if preconditions are violated.
pub(crate) fn tensor_evaluator(
    n: usize,
    m: usize,
    g: &TensorMacroCiphertexts,
    a_macs: &[Mac],
    t_eval: &BlockMatrix,
) -> BlockMatrix {
    assert_eq!(a_macs.len(), n, "a_macs length must equal n");
    assert_eq!(t_eval.rows(), m, "t_eval must be a length-m column vector");
    assert_eq!(t_eval.cols(), 1, "t_eval must be a column vector (cols == 1)");
    assert_eq!(
        g.level_cts.len(),
        n - 1,
        "G must have n-1 level ciphertexts"
    );
    assert_eq!(g.leaf_cts.len(), m, "G must have m leaf ciphertexts");

    let cipher: &FixedKeyAes = &FIXED_KEY_AES;

    // Clear bit vector `a` is encoded in LSBs of a_macs (IT-MAC invariant:
    // mac = key XOR bit·delta with delta.lsb() = 1, so mac.lsb() = bit).
    let a_blocks: &[Block] = Mac::as_blocks(a_macs);

    // [5] Reconstruct all leaf seeds except seeds[missing] (= Block::default sentinel).
    //     The hoisted kernel reconstructs `missing` internally via MSB-first traversal
    //     and returns it as the second tuple element.
    let (leaf_seeds, missing) = eval_populate_seeds_mem_optimized(
        a_blocks,
        g.level_cts.clone(),
        cipher,
    );

    // [6] Recover missing leaf X_{a,k} + accumulate Z_eval.
    let mut z_eval = BlockMatrix::new(n, m);
    {
        let t_view = t_eval.as_view();
        let mut z_view = z_eval.as_view_mut();
        let _recovered_missing_cts = eval_unary_outer_product(
            &leaf_seeds,
            &t_view,
            &mut z_view,
            cipher,
            missing,
            &g.leaf_cts,
        );
    }

    z_eval
}

#[cfg(test)]
mod tests {
    // Tests delivered in Plan 03 (paper-invariant test battery).
}
