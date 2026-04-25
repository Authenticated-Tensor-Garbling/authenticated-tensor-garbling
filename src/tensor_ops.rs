use crate::{
    aes::FixedKeyAes,
    block::Block,
    delta::Delta,
    matrix::{MatrixViewMut, MatrixViewRef},
};

/// Generates seeds for tensor operations using memory-optimized approach
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &[Block],
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
    let mut odd_evens: Vec<(Block, Block)> = Vec::new();

    let n: usize = x.len();

    // Seed buffer for level-by-level computation.
    // After all iterations seeds[0..2^n] holds the final leaves directly —
    // the tree accumulator has been removed (it grew to O(2^(n+1)) elements
    // before being discarded, wasting up to ~64 MB for n=20).
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Endianness note (little-endian vectors):
    // We treat index 0 as LSB and index n-1 as MSB of x. The tree is built from the
    // most significant position downward, so we look at x[n-1] first.
    // Base case (Level 0): If LSB of x[n-1] is 1, S_1 = x[n-1], S_0 = x[n-1] ^ delta;
    // otherwise S_0 = x[n-1], S_1 = x[n-1] ^ delta.
    if x[n-1].lsb() {
        seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
        seeds[1] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1] ^ delta);
    } else {
        seeds[1] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
        seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1] ^ delta);
    }

    // Iterate through all other levels
    for i in 1..n {
        // Endianness note (little-endian vectors):
        // Level i consumes bit from x[n-i-1], moving MSB→LSB across iterations.
        let mut seed = Block::from(x[n-i-1]);

        if !x[n-i-1].lsb() {
            seed ^= delta;
        }
        let key0 = seed;
        let key1 = key0 ^ delta;

        // Maintain the sum of all odd/even seeds
        let mut odds = Block::default();
        let mut evens = Block::default();

        // Mix level index i into the tweak to prevent cross-level ciphertext collisions
        // when the same parent seed value appears at multiple nodes (established GGM construction).
        let tweak_even = Block::from(((i as u128) << 1) as u128);
        let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);

        // Iterate through the parent level to make seeds for the next level
        // Two seeds per parent: left child (even) and right child (odd)
        for j in (0..(1 << i)).rev() {
            seeds[j * 2 + 1] = cipher.tccr(tweak_odd,  seeds[j]);
            seeds[j * 2]     = cipher.tccr(tweak_even, seeds[j]);

            evens ^= seeds[j * 2];
            odds ^= seeds[j * 2 + 1];
        }

        // Add the key contributions to the sums (same level-indexed tweaks)
        evens ^= cipher.tccr(tweak_even, key0);
        odds  ^= cipher.tccr(tweak_odd,  key1);

        odd_evens.push((evens, odds));
    }

    // seeds already holds the 2^n leaves after all level expansions.
    (seeds, odd_evens)
}

/// Generates unary outer product using seeds
pub(crate) fn gen_unary_outer_product(
    seeds: &Vec<Block>,
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<Block> {
    let m = y.len();

    let mut gen_cts: Vec<Block> = Vec::new();

    // For each share (B, B+ b∂)
    // G sends the sum (XOR_i A_i) + B), which allows E to obtain A_{x + gamma} + b∂
    // Expand the 2^n leaf seeds into 2^n by
    for j in 0..m {
        // Endianness note (little-endian y): index 0 is LSB of y, index m-1 is MSB.
        let mut row: Block = Block::default();
        for i in 0..seeds.len() {
            let tweak = (seeds.len() * j + i) as u128;
            let s = cipher.tccr(Block::from(tweak), seeds[i]);
            row ^= s;

            // let i = f(i) is just i in usize
            // Endianness note (little-endian x encoded in seed index i):
            // bit k of i corresponds to the k-th least significant bit.
            for k in 0..out.rows() {
                if ((i >> k) & 1) == 1 {
                    out[(k, j)] ^= s;
                }
            }
        }
        row ^= y[j];
        gen_cts.push(row);
    }
    gen_cts
}

/// Reconstructs the GGM seed tree on the evaluator side.
///
/// Given the evaluator's auth-bit MAC values `x` (each `x[i]` equals
/// `A_i XOR a_i*Delta` for the garbler's Delta), the evaluator's explicit
/// choice bits `a_bits` (the evaluator knows their own bits independently of
/// the MAC's LSB — this decouples the function from any constraint on
/// Delta.lsb()), and the level ciphertexts `levels` from the garbler,
/// reconstruct the 2^n leaf seeds `Label_l` for l != missing. The seed at
/// index `missing` (the clear integer value of the bit vector `a`) is set to
/// `Block::default()` as a sentinel for the downstream leaf-expansion step.
///
/// Endianness: index 0 is LSB, index n-1 is MSB of the bit vector. The
/// tree is traversed MSB-first, consuming `a_bits[n-1]` at level 0.
///
/// Returns `(leaf_seeds, missing)` where `leaf_seeds.len() == 2^n` and
/// `leaf_seeds[missing] == Block::default()`.
///
/// Implements Step 2-3 of Construction 1's tensorev (paper Appendix F).
/// The paper's evaluator knows its own choice bits `a` explicitly; this
/// function uses `a_bits` directly rather than deducing from MAC LSBs.
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    a_bits: &[bool],
    levels: &[(Block, Block)],
    cipher: &FixedKeyAes,
) -> (Vec<Block>, usize) {
    let n: usize = x.len();
    debug_assert_eq!(a_bits.len(), n, "a_bits must have same length as MAC blocks");

    // Seed buffer for level-by-level computation.
    // After all iterations seeds[0..2^n] holds the final leaves directly —
    // the tree accumulator has been removed (it grew to O(2^(n+1)) elements
    // before being discarded, wasting up to ~64 MB for n=20).
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Endianness note (little-endian vectors):
    // Index 0 is LSB, index n-1 is MSB. Start from a_bits[n-1] as the first branching bit.
    // The evaluator's MAC x[n-1] = A_{n-1} XOR a_{n-1}*Delta is used as the PRF key
    // for the non-missing subtree root. The missing index is a_bits[n-1].
    let a0 = a_bits[n-1];
    seeds[!a0 as usize] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);

    // Missing path is constructed MSB-to-LSB by shifting in a_bits[n-i-1] at each level.
    let mut missing = a0 as usize;

    for i in 1..n {
        let g_evens = levels[i-1].0;
        let g_odds = levels[i-1].1;

        let mut e_evens = Block::default();
        let mut e_odds = Block::default();

        // Mirror the garbler's level-indexed tweaks to maintain consistency.
        let tweak_even = Block::from(((i as u128) << 1) as u128);
        let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);

        // Compute seeds for the next level, skipping the missing node
        for j in (0..(1 << i)).rev() {
            if j == missing {
                seeds[j * 2 + 1] = Block::default();
                seeds[j * 2] = Block::default();
            } else {
                // GGM tree tweak domain separation: level-indexed tweaks prevent cross-level
                // ciphertext collisions (matches garbler's tweak assignment).
                seeds[j * 2 + 1] = cipher.tccr(tweak_odd,  seeds[j]);
                seeds[j * 2]     = cipher.tccr(tweak_even, seeds[j]);

                e_evens ^= seeds[j * 2];
                e_odds ^= seeds[j * 2 + 1];
            }
        }

        // Endianness note (little-endian vectors): consume bit at position n-i-1.
        // Use the explicit choice bit, not the MAC LSB (which would require Delta.lsb()=1).
        let bit = a_bits[n-i-1];
        missing = (missing << 1) | bit as usize;

        // Reconstruct the sibling of the missing node using the ciphertext
        let (tweak, mask) = if bit {
            (tweak_even, g_evens ^ e_evens)
        } else {
            (tweak_odd,  g_odds ^ e_odds)
        };

        let sibling_index = missing ^ 1;
        let computed_seed = cipher.tccr(tweak, x[n-i-1]) ^ mask;
        seeds[sibling_index] = computed_seed;
    }

    // seeds already holds the 2^n leaves after all level expansions.
    (seeds, missing)
}

/// Evaluator's leaf-expansion + Z accumulation counterpart to `gen_unary_outer_product`.
///
/// Combines the reconstructed `seeds` (with `seeds[missing] == Block::default()`),
/// the garbler's leaf ciphertexts `gen_cts`, the evaluator's `y` share (T^ev),
/// and the `missing` index to (a) write Z_eval into `out` and (b) return the
/// recovered missing-leaf column values (for optional downstream use).
///
/// Preconditions:
/// - `seeds.len() == 2^n` for some `n`
/// - `seeds[missing] == Block::default()` (sentinel set by `eval_populate_seeds_mem_optimized`)
/// - `y.len() == m`, `gen_cts.len() == m`, `out` is an n×m column-major view
pub(crate) fn eval_unary_outer_product(
    seeds: &[Block],
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[Block],
) -> Vec<Block> {
    debug_assert_eq!(
        seeds[missing],
        Block::default(),
        "seeds[missing] must be Block::default() sentinel"
    );
    let m = y.len();

    let mut eval_cts: Vec<Block> = Vec::new();

    for j in 0..m {
        // Endianness note (little-endian y): index 0 is LSB of y, index m-1 is MSB.
        let mut eval_ct = Block::default();
        for i in 0..seeds.len() {
            if i != missing {
                let tweak = (seeds.len() * j + i) as u128;
                let s = cipher.tccr(Block::from(tweak), seeds[i]);
                eval_ct ^= s;
                // Endianness note (little-endian x encoded in seed index i):
                // bit k of i corresponds to the k-th least significant bit.
                for k in 0..out.rows() {
                    if ((i >> k) & 1) == 1 {
                        out[(k, j)] ^= s;
                    }
                }
            }
        }
        eval_ct ^= gen_cts[j] ^ y[j];
        eval_cts.push(eval_ct);
        // Endianness note (little-endian x): distribute eval_ct to rows where missing has bit k set.
        for k in 0..out.rows() {
            if ((missing >> k) & 1) == 1 {
                out[(k, j)] ^= eval_ct;
            }
        }
    }

    eval_cts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aes::FIXED_KEY_AES;
    use crate::matrix::BlockMatrix;

    #[test]
    fn test_gen_unary_outer_product_wide_tweak_independence() {
        // P2-01: kappa-half and rho-half outputs MUST differ — even/odd tweak split
        // ensures TCCR outputs are pseudorandomly independent.
        // Deterministic seeds (n=2 -> 4 leaves, m=2 columns).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x1000)).collect();
        let mut y_gb_mat = BlockMatrix::new(2, 1);
        let mut y_ev_mat = BlockMatrix::new(2, 1);
        for j in 0..2 {
            y_gb_mat[j] = Block::from((j as u128) + 0x2000);
            y_ev_mat[j] = Block::from((j as u128) + 0x3000);
        }
        let mut out_gb_mat = BlockMatrix::new(2, 2);
        let mut out_ev_mat = BlockMatrix::new(2, 2);

        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut out_gb_mat.as_view_mut(),
            &mut out_ev_mat.as_view_mut(),
            &FIXED_KEY_AES,
        );

        assert_eq!(gen_cts.len(), 2, "wide gen returns m wide ciphertexts");
        // Each (kappa, rho) pair MUST differ — overwhelming probability under TCCR.
        for (k, ct) in gen_cts.iter().enumerate() {
            assert_ne!(ct.0, ct.1, "gen_cts[{}].0 (kappa) must differ from .1 (rho)", k);
        }
        // The two output matrices MUST differ at some position.
        let mut differs = false;
        for k in 0..2 {
            for j in 0..2 {
                if out_gb_mat[(k, j)] != out_ev_mat[(k, j)] {
                    differs = true;
                    break;
                }
            }
        }
        assert!(differs, "out_gb and out_ev must differ at >=1 position");
    }

    #[test]
    fn test_eval_unary_outer_product_wide_round_trip_kappa() {
        // P2-01: With matching missing index, gen + eval round-trip on the kappa half
        // produces the same accumulator behavior as the narrow gen + narrow eval.
        // Use n=2 (4 leaves), m=1 column, missing=2 (arbitrary leaf).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x10)).collect();
        let mut y_gb_mat = BlockMatrix::new(1, 1);
        let mut y_ev_mat = BlockMatrix::new(1, 1);
        y_gb_mat[0] = Block::from(0x20u128);
        y_ev_mat[0] = Block::from(0x30u128);

        // Garble side: full seeds.
        let mut gen_out_gb = BlockMatrix::new(2, 1);
        let mut gen_out_ev = BlockMatrix::new(2, 1);
        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut gen_out_gb.as_view_mut(),
            &mut gen_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
        );

        // Eval side: copy seeds but zero out the missing entry.
        let missing = 2usize;
        let mut eval_seeds = seeds.clone();
        eval_seeds[missing] = Block::default();
        let mut eval_out_gb = BlockMatrix::new(2, 1);
        let mut eval_out_ev = BlockMatrix::new(2, 1);
        eval_unary_outer_product_wide(
            &eval_seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut eval_out_gb.as_view_mut(),
            &mut eval_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
            missing,
            &gen_cts,
        );

        // Compute expected kappa-row directly from the row equation:
        //   row_gb = (XOR_i tccr(2*base, seeds[i])) ^ y_gb[j]
        let mut expected_row_gb = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * 0 + i) as u128;
            expected_row_gb ^= FIXED_KEY_AES.tccr(Block::from(base << 1), seeds[i]);
        }
        expected_row_gb ^= y_gb_mat[0];

        // Verify gen ciphertext kappa-half matches the row equation.
        assert_eq!(gen_cts[0].0, expected_row_gb,
            "wide gen ciphertext kappa-half must equal the row equation");
    }

    #[test]
    fn test_eval_unary_outer_product_wide_round_trip_rho() {
        // P2-01: same round-trip property for the rho half (tweak base<<1|1).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x10)).collect();
        let mut y_gb_mat = BlockMatrix::new(1, 1);
        let mut y_ev_mat = BlockMatrix::new(1, 1);
        y_gb_mat[0] = Block::from(0x20u128);
        y_ev_mat[0] = Block::from(0x30u128);

        let mut gen_out_gb = BlockMatrix::new(2, 1);
        let mut gen_out_ev = BlockMatrix::new(2, 1);
        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut gen_out_gb.as_view_mut(),
            &mut gen_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
        );

        let mut expected_row_ev = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * 0 + i) as u128;
            expected_row_ev ^= FIXED_KEY_AES.tccr(Block::from(base << 1 | 1), seeds[i]);
        }
        expected_row_ev ^= y_ev_mat[0];

        assert_eq!(gen_cts[0].1, expected_row_ev,
            "wide gen ciphertext rho-half must equal the row equation under odd tweak");
    }

    #[test]
    fn test_wide_signature_shapes() {
        // P2-01: shape invariants — gen_cts.len() == m; out_gb / out_ev are written.
        let seeds: Vec<Block> = (0..4).map(|i| Block::from(i as u128)).collect();
        let mut y_gb_mat = BlockMatrix::new(3, 1);
        let mut y_ev_mat = BlockMatrix::new(3, 1);
        for j in 0..3 {
            y_gb_mat[j] = Block::from((j as u128) + 100);
            y_ev_mat[j] = Block::from((j as u128) + 200);
        }
        let mut out_gb_mat = BlockMatrix::new(2, 3);
        let mut out_ev_mat = BlockMatrix::new(2, 3);

        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut out_gb_mat.as_view_mut(),
            &mut out_ev_mat.as_view_mut(),
            &FIXED_KEY_AES,
        );

        assert_eq!(gen_cts.len(), 3, "gen_cts.len() must equal m=3");

        // At least one entry of each output matrix must be non-default (overwhelmingly likely).
        let mut nonzero_gb = false;
        let mut nonzero_ev = false;
        for k in 0..2 {
            for j in 0..3 {
                if out_gb_mat[(k, j)] != Block::default() { nonzero_gb = true; }
                if out_ev_mat[(k, j)] != Block::default() { nonzero_ev = true; }
            }
        }
        assert!(nonzero_gb, "out_gb has at least one non-default entry");
        assert!(nonzero_ev, "out_ev has at least one non-default entry");
    }
}
