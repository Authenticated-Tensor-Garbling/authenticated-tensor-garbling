use crate::{aes::FixedKeyAes, block::Block, delta::Delta, key_matrix::MatrixViewRef};

/// Generates a complete GGM tree and returns both the tree and ciphertexts for the evaluator.
/// 
/// # Arguments
/// * `x` - Input vector of blocks, where the highest index has the lowest subscript
/// * `_missing` - Unused parameter (kept for interface consistency)
/// * `cipher` - Fixed-key AES cipher for cryptographic operations
/// * `delta` - Global offset used in GGM tree construction
/// 
/// # Returns
/// * `Vec<Block>` - Complete GGM tree as a flat vector
/// * `Vec<(Block, Block)>` - Ciphertexts (even/odd sums) for each level
pub fn gen_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    _missing: &usize,
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
    let mut tree: Vec<Block> = Vec::new();
    let mut odd_evens: Vec<(Block, Block)> = Vec::new();

    let n: usize = x.len();

    // Seed buffer for level-by-level computation
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Set the base case (Level 0)
    // If LSB of x[n-1] is 1, then S_1 = x[n-1] and we compute S_0 = x[n-1] ^ delta
    // If LSB of x[n-1] is 0, then S_0 = x[n-1] and we compute S_1 = x[n-1] ^ delta
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

    (tree, odd_evens)
}

/// Evaluates a sparse GGM tree using the provided ciphertexts from the generator.
/// 
/// # Arguments
/// * `x` - Input vector of blocks, where the highest index has the lowest subscript
/// * `levels` - Ciphertexts (even/odd sums) from the generator for each level
/// * `_missing` - Unused parameter (kept for interface consistency)
/// * `cipher` - Fixed-key AES cipher for cryptographic operations
/// 
/// # Returns
/// * `Vec<Block>` - Sparse GGM tree with missing nodes set to Block::default()
pub fn eval_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    levels: Vec<(Block, Block)>,
    _missing: &usize,
    cipher: &FixedKeyAes,
) -> Vec<Block> {
    let mut tree: Vec<Block> = Vec::new();

    let n: usize = x.len();
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Get the one seed that evaluator knows initially
    seeds[!x[n-1].lsb() as usize] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
    
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

        // Compute seeds for the next level, skipping the missing node
        for j in (0..(1 << i)).rev() {
            if j == missing {
                seeds[j * 2 + 1] = Block::default();
                seeds[j * 2] = Block::default();
            } else {
                seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);
                seeds[j * 2] = cipher.tccr(Block::from(1 as u128), seeds[j]);
                
                e_evens ^= seeds[j * 2];
                e_odds ^= seeds[j * 2 + 1];
            }
        }

        let bit = x[n-i-1].lsb();
        missing = (missing << 1) | bit as usize;
        
        // Reconstruct the sibling of the missing node using the ciphertext
        let (tweak, mask) = if bit {
            (Block::from(0 as u128), g_evens ^ e_evens)
        } else {
            (Block::from(1 as u128), g_odds ^ e_odds)
        };
        
        let sibling_index = missing ^ 1;
        let computed_seed = cipher.tccr(tweak, x[n-i-1]) ^ mask;
        seeds[sibling_index] = computed_seed;

        // Add all seeds to the tree (missing nodes will be Block::default())
        for idx in 0..(1 << (i+1)) {
            tree.push(seeds[idx]);
        }
    }
    
    tree
}

/// Computes the unary outer product [|T(f) * (U(x + c) & y)|] where & is the vector outer product
/// and T(f) is the truth table of f. The resulting matrix is l x m.
/// 
/// This function is a placeholder for future implementation.
pub fn unary_outer_product() {
    // TODO: Implement unary outer product computation
    unimplemented!("Unary outer product computation not yet implemented");
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{aes::FIXED_KEY_AES, block::Block, key_matrix::BlockMatrix};
    use rand;

    /// Generates random input for testing GGM tree operations.
    /// 
    /// # Arguments
    /// * `n` - Length of the input vector
    /// 
    /// # Returns
    /// * `usize` - Missing path index derived from LSBs of input blocks
    /// * `Vec<Block>` - Random input vector
    #[allow(dead_code)]
    pub fn prep_random_input(n: usize) -> (usize, Vec<Block>) {
        let mut rng = rand::rng();
        
        let input: Vec<Block> = (0..n).map(|_| Block::random(&mut rng)).collect();
        
        let missing: usize = input.iter().rev().fold(0, |acc, b| {
            (acc << 1) | b.lsb() as usize
        });

        (missing, input)
    }

    /// Helper function to check if a node index is on the missing path.
    /// 
    /// # Arguments
    /// * `idx` - Node index within the level
    /// * `missing_path` - Missing path index (parsed from MSB to LSB)
    /// * `level` - Current level in the tree
    /// 
    /// # Returns
    /// * `bool` - True if the node is on the missing path
    fn is_on_path(idx: usize, missing_path: usize, level: usize) -> bool {
        let mut place = 0;
        
        for i in 0..=level {
            let bit = (missing_path >> (3 - i)) & 1;
            if bit == 1 {
                place = place * 2 + 2;
            } else {
                place = place * 2 + 1;
            }
        }

        idx == (place - 1)
    }

    #[test]
    fn test_path_helper() {
        let missing: usize = 0b0000;
        assert!(is_on_path(0, missing, 0));
        assert!(is_on_path(2, missing, 1));
        assert!(is_on_path(6, missing, 2));
        assert!(is_on_path(14, missing, 3));

        let missing = 0b1010;
        assert!(is_on_path(1, missing, 0));
        assert!(is_on_path(4, missing, 1));
        assert!(is_on_path(11, missing, 2));
        assert!(is_on_path(24, missing, 3));
    }

    #[test]
    fn test_ggm_tree() {
        let cipher = &FIXED_KEY_AES;
        let delta = Delta::random(&mut rand::rng());

        let input = BlockMatrix::random(4, 1);
        let missing = input.get_clear_value();

        // Run generator to get tree and ciphertexts
        let (gen_tree, levels) = gen_populate_seeds_mem_optimized(&input.as_view(), &0, cipher, delta);

        // Run evaluator to get tree
        let eval_tree = eval_populate_seeds_mem_optimized(&input.as_view(), levels, &0, cipher);
        
        // Verify tree sizes match
        assert_eq!(gen_tree.len(), eval_tree.len(), "Tree sizes should match");

        // Define level boundaries for a 4-level tree
        // Level 0: indices 0-1 (2 nodes)
        // Level 1: indices 2-5 (4 nodes) 
        // Level 2: indices 6-13 (8 nodes)
        // Level 3: indices 14-29 (16 nodes)
        let level_boundaries = vec![(0, 1), (2, 5), (6, 13), (14, 29)];
        
        // Check each level
        for (level, (start, end)) in level_boundaries.iter().enumerate() {
            for idx in *start..=*end {
                let gen_node = gen_tree[idx];
                let eval_node = eval_tree[idx];
                
                // Check if this node is on the missing path
                let is_on_missing_path = is_on_path(idx, missing, level);
                
                if is_on_missing_path {
                    // Node should be Block::default() in evaluator tree
                    assert_eq!(eval_node, Block::default(), 
                        "Node {} on missing path should be Block::default() in evaluator tree", idx);
                } else {
                    // Node should match between generator and evaluator
                    assert_eq!(gen_node, eval_node, 
                        "Node {} should match between generator and evaluator trees", idx);
                }
            }
        }
    }
}