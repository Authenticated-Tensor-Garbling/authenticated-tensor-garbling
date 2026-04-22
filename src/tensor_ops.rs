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
    let mut tree: Vec<Block> = Vec::new();
    let mut odd_evens: Vec<(Block, Block)> = Vec::new();

    let n: usize = x.len();

    // Seed buffer for level-by-level computation
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

    // Add Level 0 seeds to the tree
    for idx in 0..2 {
        if seeds[idx] != Block::default() {
            tree.push(seeds[idx]);
        }
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
        
        // Add all non-default seeds from this level to the tree
        for idx in 0..(1 << (i+1)) {
            if seeds[idx] != Block::default() {
                tree.push(seeds[idx]);
            }
        }
    }

    let seeds = tree[tree.len() - (1 << n)..tree.len()].to_vec();

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
/// `A_i XOR a_i*Delta` where `Delta.lsb() = 1`, so `x[i].lsb() = a_i`)
/// and the level ciphertexts `levels` from the garbler, reconstruct the
/// 2^n leaf seeds `Label_l` for l != missing. The seed at index `missing`
/// (the clear integer value of the bit vector `a`) is set to
/// `Block::default()` as a sentinel for the downstream leaf-expansion step.
///
/// Endianness: index 0 is LSB, index n-1 is MSB of the bit vector. The
/// tree is traversed MSB-first, consuming `x[n-1]` at level 0.
///
/// Returns `(leaf_seeds, missing)` where `leaf_seeds.len() == 2^n` and
/// `leaf_seeds[missing] == Block::default()`.
///
/// Implements Step 2-3 of Construction 1's tensorev (paper Appendix F).
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    levels: Vec<(Block, Block)>,
    cipher: &FixedKeyAes,
) -> (Vec<Block>, usize) {
    let mut tree: Vec<Block> = Vec::new();

    let n: usize = x.len();
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Endianness note (little-endian vectors):
    // Index 0 is LSB, index n-1 is MSB. Start from x[n-1] as the first branching bit.
    seeds[!x[n-1].lsb() as usize] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);

    // Missing path is constructed MSB-to-LSB by shifting in x[n-i-1].lsb() at each level.
    let mut missing = x[n-1].lsb() as usize;

    // Add Level 0 seeds to the tree
    for idx in 0..2 {
        tree.push(seeds[idx]);
    }

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
        let bit = x[n-i-1].lsb();
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

        // Add all seeds to the tree (missing nodes will be Block::default())
        for idx in 0..(1 << (i+1)) {
            tree.push(seeds[idx]);
        }
    }

    // Extract only the final seeds (leaves of the tree)
    let final_seeds = tree[tree.len() - (1 << n)..tree.len()].to_vec();
    (final_seeds, missing)
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

