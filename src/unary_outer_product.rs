use crate::{aes::FixedKeyAes, block::Block, delta::Delta, key_matrix::MatrixViewRef, key_matrix::MatrixViewMut};

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

    // Endianness note (little-endian vectors):
    // Index 0 is LSB, index n-1 is MSB. We start from x[n-1] as the first branching bit.
    // Get the one seed that evaluator knows initially
    seeds[!x[n-1].lsb() as usize] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
    
    // Missing path is constructed MSB→LSB by shifting in x[n-i-1].lsb() at each level.
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

        // Endianness note (little-endian vectors): consume bit at position n-i-1.
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
pub fn gen_unary_outer_product(
    // f: &Table,
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

pub fn eval_unary_outer_product(
    // f: &Table,
    seeds: &Vec<Block>,
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &Vec<Block>,
) -> Vec<Block> {
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
        println!("--------------------------------");
    }

    eval_cts
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
        let (gen_tree, levels) = gen_populate_seeds_mem_optimized(&input.as_view(), cipher, delta);

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

    #[test]
    fn test_unary_outer_product() {

        //======================================================
        //                      SETUP
        //======================================================
        // instantiate global cipher and value
        let cipher = &FIXED_KEY_AES;
        let delta = Delta::random(&mut rand::rng());

        // instantiate test sizes
        let n = 4; // x is nx1
        let m = 2; // y is mx1
        let l = 3; // out is lxm
        
        // instantiate clear values
        let clear_y = rand::random_range(0..(1 << m));
        let clear_x = rand::random_range(0..(1 << n));
        
        // instantiate gen_x and eval_x, gen_y and eval_y:
        // gen_x is a vector of random labels, LSBs 0 for testing
        // eval_x is gen_x with the indices corresponding to 1 having the delta offset
        // TODO push to a helper function
        let gen_x = BlockMatrix::random_zeros(n, 1);
        debug_assert!((0..n).all(|i| gen_x[i].lsb() == false), "gen_x LSBs must be 0");
        let mut eval_x = BlockMatrix::constant(n, 1, Block::default());
        for i in 0..n {
            eval_x[i] = if ((clear_x >> i) & 1) == 0 {
                gen_x[i]
            } else {
                gen_x[i] ^ delta
            };
        }
        let gen_y = BlockMatrix::random_zeros(m, 1);
        let mut eval_y = BlockMatrix::constant(m, 1, Block::default());
        for i in 0..m {
            eval_y[i] = if ((clear_y >> i) & 1) == 0 {
                gen_y[i]
            } else {
                gen_y[i] ^ delta
            };
        }

        // sanity check for endianness
        assert_eq!(clear_x, eval_x.get_clear_value());
        
        // instantiate output matrices
        let mut gen_out = BlockMatrix::new(l, m);
        let mut eval_out = BlockMatrix::new(l, m);



        //======================================================
        //                  PROTOCOL
        //======================================================

        // generator: generate the GGM tree and seeds
        let (gen_tree, levels) = gen_populate_seeds_mem_optimized(&gen_x.as_view(), cipher, delta);
        let gen_seeds = &gen_tree[gen_tree.len() - (1 << gen_x.rows())..gen_tree.len()].to_vec();
        
        // generator: get expanded ciphertexts XOR with Yj
        let gen_cts = gen_unary_outer_product(gen_seeds, &gen_y.as_view(), &mut gen_out.as_view_mut(), cipher);

        // evaluator: get the GGM tree, with the missing path set to Block::default()
        let eval_tree = eval_populate_seeds_mem_optimized(&eval_x.as_view(), levels, &clear_x, cipher);
        let eval_seeds = &eval_tree[(eval_tree.len() - (1 << eval_x.rows()))..eval_tree.len()].to_vec();
        
        // evaluator: get expanded ciphertexts XOR with Yj xor yjDelta and gen_cts
        let eval_cts = eval_unary_outer_product(eval_seeds, &eval_y.as_view(), &mut eval_out.as_view_mut(), cipher, clear_x, &gen_cts);


        //======================================================
        //                GGM TREE CHECK
        //======================================================
        
        // Refactored to above test



        //======================================================
        //                CIPHERTEXT CHECK
        //======================================================

        // Check: gen_cts[j] XOR eval_cts[j] equals expanded missing-seed
        // contribution, additionally XOR delta when y[j].lsb() == 1.
        for (j,  eval_ct) in eval_cts.iter().enumerate() {
            let tweak = (gen_seeds.len() * j + clear_x) as u128;
            let expanded_missing = cipher.tccr(Block::from(tweak), gen_seeds[clear_x]);
            let y_diff = gen_y[j] ^ eval_y[j]; // equals delta if eval_y[j].lsb() is 1, else 0
            assert_eq!(*eval_ct, expanded_missing ^ y_diff);
        }

        // check that the eval_cts match the expected missing values
        let mut expected_missing_values = Vec::new();
        for j in 0..m {
            let tweak = (gen_seeds.len() * j + clear_x) as u128;
            let missing_contribution = cipher.tccr(Block::from(tweak), gen_seeds[clear_x]);
            // The evaluator's result should be the missing contribution XORed with the difference in y values
            let y_diff = gen_y[j] ^ eval_y[j];
            expected_missing_values.push(missing_contribution ^ y_diff);
        }

        for (i, (result, expected)) in eval_cts.iter().zip(expected_missing_values.iter()).enumerate() {
            assert_eq!(result, expected, 
                       "Row {}: Evaluator result should match expected missing value", i);
        }
        


        //======================================================
        //                OUTPUT MATRIX CHECK
        //======================================================

        // The protocol computes [|T(f) * (U(x + c) & y) |] where:
        // - T(f) is the truth table of function f (currently identity)
        // - U(x + c) is the unary representation of x + c
        // - & is the vector outer product
        // - c is the missing path (color)
        
        // Compute the expected result: x OUTER y (outer product of x and y)
        // But truncate x to l bits (MSBs of x should be truncated to be l bits long)
        // For each bit position i in the first l bits of x and each bit position j in y:
        // result[i][j] = x[i] & y[j]
        let mut expected_result = vec![vec![false; m]; l];
        for i in 0..l {
            for j in 0..m {
                let x_bit = ((clear_x >> i) & 1) == 1;
                let y_bit = ((clear_y >> j) & 1) == 1;
                expected_result[i][j] = x_bit & y_bit;
            }
        }
        
        // Now verify that the output matrices follow the expected pattern:
        // - Where expected_result[k][j] = 0: gen_out[k][j] should equal eval_out[k][j]
        // - Where expected_result[k][j] = 1: gen_out[k][j] should equal eval_out[k][j] ^ delta
        for k in 0..l {
            for j in 0..m {
                let gen_val = gen_out[(k, j)];
                let eval_val = eval_out[(k, j)];
                let expected_bit = expected_result[k][j];
                
                if expected_bit {
                    // Where expected_result = 1, they should differ by delta
                    let expected_eval = gen_val ^ delta;
                    assert_eq!(eval_val, expected_eval, 
                               "At position ({},{}): eval_out should equal gen_out ^ delta when expected=1", k, j);
                } else {
                    // Where expected_result = 0, they should be identical
                    assert_eq!(gen_val, eval_val, 
                               "At position ({},{}): gen_out should equal eval_out when expected=0", k, j);
                }
            }
        }
    }
}