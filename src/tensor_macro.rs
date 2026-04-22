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
    assert!(n > 0, "n must be at least 1 (degenerate n=0 is not supported)");
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
/// `a_bits` are the evaluator's explicit choice bits — index 0 is the LSB
/// of the `a` vector, index `n-1` is the MSB. These are passed separately
/// from `a_macs` to allow the tree traversal to work even when the garbler's
/// Δ has `lsb == 0` (in which case `mac.lsb() != a_i`).
///
/// Preconditions (enforced via `assert_eq!`):
/// - `a_macs.len() == n`
/// - `a_bits.len() == n`
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
    a_bits: &[bool],
    t_eval: &BlockMatrix,
) -> BlockMatrix {
    assert!(n > 0, "n must be at least 1 (degenerate n=0 is not supported)");
    assert_eq!(a_macs.len(), n, "a_macs length must equal n");
    assert_eq!(a_bits.len(), n, "a_bits length must equal n");
    assert_eq!(t_eval.rows(), m, "t_eval must be a length-m column vector");
    assert_eq!(t_eval.cols(), 1, "t_eval must be a column vector (cols == 1)");
    assert_eq!(
        g.level_cts.len(),
        n - 1,    // safe: n >= 1
        "G must have n-1 level ciphertexts"
    );
    assert_eq!(g.leaf_cts.len(), m, "G must have m leaf ciphertexts");

    let cipher: &FixedKeyAes = &FIXED_KEY_AES;

    let a_blocks: &[Block] = Mac::as_blocks(a_macs);

    // [5] Reconstruct all leaf seeds except seeds[missing] (= Block::default sentinel).
    //     Explicit a_bits are passed so tree navigation works regardless of Δ.lsb().
    let (leaf_seeds, missing) = eval_populate_seeds_mem_optimized(
        a_blocks,
        a_bits,
        &g.level_cts,
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
    use super::*;
    use crate::bcot::IdealBCot;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha12Rng;

    /// Paper-invariant oracle:
    ///
    /// 1. Produce matched `(a_keys: Vec<Key>, a_macs: Vec<Mac>)` from
    ///    `IdealBCot::transfer_b_to_a(&choices)` — guaranteed to satisfy
    ///    `mac = key XOR bit·delta_a` with `key.lsb() == 0` and
    ///    `mac.lsb() == choice_bit` (relies on `delta_a.lsb() == 1`).
    /// 2. Generate random T shares `t_gen`, `t_eval` (length-m column vectors).
    /// 3. Run `tensor_garbler` and `tensor_evaluator`.
    /// 4. Compute expected `a ⊗ T` using the same endianness convention the
    ///    kernels use: `a[i] = a_macs[i].lsb()`, `T[k] = t_gen[k] XOR t_eval[k]`.
    /// 5. Assert `z_gen XOR z_eval == a ⊗ T` entry-wise.
    fn run_one_case(n: usize, m: usize, seed: u64) {
        // ----- Set up bCOT and derive the macro's Δ -----
        let mut bcot = IdealBCot::new(seed, seed ^ 0xDEAD_BEEF);

        // Garbler uses delta_a (LSB=1). Same-delta convention: transfer_a_to_b uses
        // delta_a, so receiver MACs have lsb = choice bit. The garbler holds the sender
        // keys; the evaluator holds the receiver MACs. Explicit choice bits are passed
        // separately to tensor_evaluator so tree navigation works for any delta.
        let delta = bcot.delta_a;

        // ----- Sample random choice bits -----
        let mut rng = ChaCha12Rng::seed_from_u64(seed);
        let choices: Vec<bool> = (0..n).map(|_| rng.random_bool(0.5)).collect();

        // ----- Perform the batch bCOT (A sends, B evaluates with choices) -----
        // transfer_a_to_b uses delta_a → mac = K[0] XOR choice * delta_a
        // Garbler holds sender_keys; evaluator holds receiver_macs.
        let cot = bcot.transfer_a_to_b(&choices);
        let a_keys: Vec<Key> = cot.sender_keys;    // LSB = 0 invariant (Key::new)
        let a_macs: Vec<Mac> = cot.receiver_macs;  // mac = K[0] XOR choice * delta_a

        // ----- Sample random T shares (each a length-m column vector) -----
        let mut t_gen = BlockMatrix::new(m, 1);
        let mut t_eval = BlockMatrix::new(m, 1);
        for k in 0..m {
            t_gen[k] = Block::random(&mut rng);
            t_eval[k] = Block::random(&mut rng);
        }

        // ----- Run both sides of the macro -----
        let (z_gen, g) = tensor_garbler(n, m, delta, &a_keys, &t_gen);
        // Pass explicit choices as a_bits — decoupled from mac.lsb().
        let z_eval = tensor_evaluator(n, m, &g, &a_macs, &choices, &t_eval);

        // Sanity on dimensions
        assert_eq!(z_gen.rows(), n, "z_gen rows mismatch");
        assert_eq!(z_gen.cols(), m, "z_gen cols mismatch");
        assert_eq!(z_eval.rows(), n, "z_eval rows mismatch");
        assert_eq!(z_eval.cols(), m, "z_eval cols mismatch");
        assert_eq!(g.level_cts.len(), n.saturating_sub(1), "G level_cts length");
        assert_eq!(g.leaf_cts.len(), m, "G leaf_cts length");

        // ----- Compute expected a ⊗ T -----
        // Use explicit choices (not mac.lsb()) for correctness with any delta convention.
        let a_bits: Vec<bool> = choices.clone();
        let t_full: Vec<Block> = (0..m).map(|k| t_gen[k] ^ t_eval[k]).collect();

        // ----- Assert Z_gen XOR Z_eval == a ⊗ T -----
        for i in 0..n {
            for k in 0..m {
                let expected = if a_bits[i] { t_full[k] } else { Block::ZERO };
                let actual = z_gen[(i, k)] ^ z_eval[(i, k)];
                assert_eq!(
                    actual, expected,
                    "paper invariant failed at (i={}, k={}) for n={} m={} seed={} a_bits[i]={}",
                    i, k, n, m, seed, a_bits[i],
                );
            }
        }
    }

    // Edge case: single wire, single column (degenerate tree).
    #[test]
    fn test_n1_m1() { run_one_case(1, 1, 1); }

    // Edge case: single wire, multiple columns.
    #[test]
    fn test_n1_m4() { run_one_case(1, 4, 2); }

    // Edge case: two wires, single column.
    #[test]
    fn test_n2_m1() { run_one_case(2, 1, 3); }

    // Small non-power-of-2 m.
    #[test]
    fn test_n2_m3() { run_one_case(2, 3, 4); }

    // Balanced case.
    #[test]
    fn test_n4_m4() { run_one_case(4, 4, 5); }

    // m > n.
    #[test]
    fn test_n4_m8() { run_one_case(4, 8, 6); }

    // Large m (exercises leaf-expansion loop bounds).
    #[test]
    fn test_n4_m64() { run_one_case(4, 64, 7); }

    // Large n, tiny m (exercises tree depth).
    #[test]
    fn test_n8_m1() { run_one_case(8, 1, 8); }

    // Both large.
    #[test]
    fn test_n8_m16() { run_one_case(8, 16, 9); }

    /// Deterministic regression vector. Fixed seed keeps Δ, choices, and T
    /// reproducible; any future change that silently breaks the invariant
    /// will fail this test first.
    #[test]
    fn test_deterministic_seed_42() { run_one_case(4, 4, 42); }
}
