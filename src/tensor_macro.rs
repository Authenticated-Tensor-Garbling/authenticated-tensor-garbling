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
    block::Block,
    delta::Delta,
    keys::Key,
    macs::Mac,
    matrix::BlockMatrix,
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

/// Garbler side of Construction 1 (paper Appendix F). **Stub — body delivered in Plan 02.**
///
/// Builds a 2^n-leaf GGM tree from the garbler's IT-MAC keys `a_keys`,
/// computes level ciphertexts and leaf ciphertexts, and returns
/// `(Z_garbler, G)` such that `Z_garbler XOR Z_evaluator == a ⊗ T`.
///
/// Preconditions (enforced via `assert_eq!`):
/// - `a_keys.len() == n`
/// - `t_gen.rows() == m` and `t_gen.cols() == 1` (length-m column vector)
/// - Every `a_keys[i].lsb() == 0` (enforced by `Key::new()`)
pub(crate) fn tensor_garbler(
    _n: usize,
    _m: usize,
    _delta: Delta,
    _a_keys: &[Key],
    _t_gen: &BlockMatrix,
) -> (BlockMatrix, TensorMacroCiphertexts) {
    unimplemented!("tensor_garbler body is delivered in Plan 02")
}

/// Evaluator side of Construction 1 (paper Appendix F). **Stub — body delivered in Plan 02.**
///
/// Reconstructs the untraversed GGM subtree from `a_macs` (each encoding
/// `A_i XOR a_i·Δ`) and the garbler's ciphertexts `g`, then produces
/// `Z_evaluator`.
///
/// Preconditions (enforced via `assert_eq!`):
/// - `a_macs.len() == n`
/// - `t_eval.rows() == m` and `t_eval.cols() == 1`
/// - `g.level_cts.len() == n - 1`
/// - `g.leaf_cts.len() == m`
pub(crate) fn tensor_evaluator(
    _n: usize,
    _m: usize,
    _g: &TensorMacroCiphertexts,
    _a_macs: &[Mac],
    _t_eval: &BlockMatrix,
) -> BlockMatrix {
    unimplemented!("tensor_evaluator body is delivered in Plan 02")
}

#[cfg(test)]
mod tests {
    // Tests delivered in Plan 03 (paper-invariant test battery).
}
