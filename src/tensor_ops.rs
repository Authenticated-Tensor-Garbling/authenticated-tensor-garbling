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

        // Iterate through the parent level to make seeds for the next level
        // Two seeds per parent: left child (even) and right child (odd)
        for j in (0..(1 << i)).rev() {
            seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);
            seeds[j * 2] = cipher.tccr(Block::from(1 as u128), seeds[j]);
            
            evens ^= seeds[j * 2];
            odds ^= seeds[j * 2 + 1];
        }
        
        // Add the key contributions to the sums
        evens ^= cipher.tccr(Block::from(0 as u128), key0);
        odds ^= cipher.tccr(Block::from(1 as u128), key1);
        
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

