use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::BlockMatrix};
use crate::sharing::AuthBitShare;
use crate::aes::FIXED_KEY_AES;
use crate::preprocessing::TensorFpreEval;
use crate::matrix::MatrixViewRef;
use crate::matrix::MatrixViewMut;

pub struct AuthTensorEval {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,

    n: usize,
    m: usize,

    pub delta_b: Delta,

    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,

    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,
}

impl AuthTensorEval {
    pub fn new(n: usize, m: usize, chunking_factor: usize) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            chunking_factor,
            n,
            m,
            delta_b: Delta::random(&mut rand::rng()),
            x_labels: Vec::new(),
            y_labels: Vec::new(),
            alpha_auth_bit_shares: Vec::new(),
            beta_auth_bit_shares: Vec::new(),
            correlated_auth_bit_shares: Vec::new(),
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
        }
    }

    pub fn new_from_fpre_eval(fpre_eval: TensorFpreEval) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            chunking_factor: fpre_eval.chunking_factor,
            n: fpre_eval.n,
            m: fpre_eval.m,
            delta_b: fpre_eval.delta_b,
            x_labels: fpre_eval.alpha_labels,
            y_labels: fpre_eval.beta_labels,
            alpha_auth_bit_shares: fpre_eval.alpha_auth_bit_shares,
            beta_auth_bit_shares: fpre_eval.beta_auth_bit_shares,
            correlated_auth_bit_shares: fpre_eval.correlated_auth_bit_shares,
            first_half_out: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
        }
    }

    fn eval_populate_seeds_mem_optimized(
        x: &MatrixViewRef<Block>,
        levels: Vec<(Block, Block)>,
        _clear_value: &usize,
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
        
        // Extract only the final seeds (leaves of the tree)
        let final_seeds = tree[tree.len() - (1 << x.len())..tree.len()].to_vec();
        final_seeds
    }

    fn eval_unary_outer_product(
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
        }
    
        eval_cts
    }

    fn eval_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<Block>>,
        first_half: bool,
    ) {
    
        let chunking_factor = self.chunking_factor;
    
        for s in 0..((x.rows() + chunking_factor-1)/chunking_factor) {
            let slice_size: usize;
            if chunking_factor *(s+1) > x.rows() {slice_size = x.rows() % chunking_factor;} else {slice_size = chunking_factor;}
            let mut slice = BlockMatrix::new(slice_size, 1);
            for i in 0..slice_size {
                slice[i] = x[i + s * chunking_factor];
            }

            let cipher = self.cipher;
            let slice_clear = slice.get_clear_value();

            // IMPORTANT: transpose the out matrix before calling with_subrows for the second half
            let mut out = if first_half {
                self.first_half_out.as_view_mut()
            } else {
                self.second_half_out.as_view_mut()
            };
            

            out.with_subrows(chunking_factor * s, slice_size, |part| {
                let eval_seeds = Self::eval_populate_seeds_mem_optimized(&slice.as_view(), chunk_levels[s].clone(), &slice_clear, cipher);
                let _eval_cts = Self::eval_unary_outer_product(&eval_seeds, &y, part, cipher, slice_clear, &chunk_cts[s]);

            });
        }
    }

    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            x[i] = self.x_labels[i];
        }

        let mut y = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y[i] = self.y_labels[i] ^ self.beta_auth_bit_shares[i].mac.as_block();
        }

        (x, y)
    }

    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut x = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            x[i] = self.y_labels[i];
        }
        
        let mut y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y[i] = *self.alpha_auth_bit_shares[i].mac.as_block();
        }

        (x, y)
    }

    pub fn evaluate_first_half(&mut self, chunk_levels: Vec<Vec<(Block, Block)>>, chunk_cts: Vec<Vec<Block>>) {
        let (x, y) = self.get_first_inputs();
        self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), chunk_levels, chunk_cts, true);
    }   

    pub fn evaluate_second_half(&mut self, chunk_levels: Vec<Vec<(Block, Block)>>, chunk_cts: Vec<Vec<Block>>) {
        let (x, y) = self.get_second_inputs();
        self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), chunk_levels, chunk_cts, false);
    }

    pub fn evaluate_final(&mut self) {
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^= 
                    self.second_half_out[(j, i)] ^
                    self.correlated_auth_bit_shares[j * self.n + i].mac.as_block();
            }
        }
    }
}