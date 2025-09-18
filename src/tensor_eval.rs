use crate::{aes::FixedKeyAes, block::Block, matrix::{BlockMatrix, MatrixViewMut, MatrixViewRef}, tensor_pre::TensorProductPreEval};

#[derive(PartialEq, Clone, Copy)]
enum ProtocolPhase {
    Setup,
    FirstHalf,
    SecondHalf,
    Final,
}

pub struct TensorProductEval {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,
    n: usize,
    m: usize,

    x: BlockMatrix,
    y: BlockMatrix,
    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,

    phase: ProtocolPhase,
}

impl TensorProductEval {
    pub fn new(tensor_eval: TensorProductPreEval) -> Self {
        Self {
            cipher: tensor_eval.cipher,
            chunking_factor: tensor_eval.chunking_factor,
            n: tensor_eval.n,
            m: tensor_eval.m,
            x: tensor_eval.x,
            y: tensor_eval.y,
            first_half_out: BlockMatrix::new(tensor_eval.n, tensor_eval.m),
            second_half_out: BlockMatrix::new(tensor_eval.m, tensor_eval.n),
            phase: ProtocolPhase::Setup,
        }
    }


    fn eval_populate_seeds_mem_optimized(
        x: &MatrixViewRef<Block>,
        levels: Vec<(Block, Block)>,
        clear_value: &usize,
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
        
        println!("DEBUG: eval_unary_outer_product - seeds len: {}, missing: {}, gen_cts len: {}", 
                 seeds.len(), missing, gen_cts.len());
    
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
            let mut out = if self.phase == ProtocolPhase::SecondHalf {
                self.second_half_out.as_view_mut()
            } else {
                self.first_half_out.as_view_mut()
            };
            

            out.with_subrows(chunking_factor * s, slice_size, |part| {
                
                println!("DEBUG: chunk {} - slice_clear: {}, slice LSBs: {:?}", s, slice_clear, 
                         (0..slice_size).map(|i| slice[i].lsb()).collect::<Vec<_>>());
                println!("DEBUG: chunk {} - chunk_levels[{}] len: {}", s, s, chunk_levels[s].len());
                println!("DEBUG: chunk {} - chunk_cts[{}] len: {}", s, s, chunk_cts[s].len());
                
                let eval_seeds = Self::eval_populate_seeds_mem_optimized(&slice.as_view(), chunk_levels[s].clone(), &slice_clear, cipher);
                println!("DEBUG: chunk {} - eval_seeds len: {}", s, eval_seeds.len());
                let _eval_cts = Self::eval_unary_outer_product(&eval_seeds, &y, part, cipher, slice_clear, &chunk_cts[s]);

            });
        }
    }

    pub fn execute_first_half_outer_product(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<Block>>,
    ) {
        assert!(self.phase == ProtocolPhase::Setup);
        self.phase = ProtocolPhase::FirstHalf;

        let x = self.x.clone();
        let y = self.y.clone();

        // Debug: Print first half inputs
        println!("=== FIRST HALF DEBUG (EVAL) ===");
        println!("eval_x clear value: {}", x.get_clear_value());
        println!("eval_y clear value: {}", y.get_clear_value());
        println!("===============================");

        self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), chunk_levels, chunk_cts);

        // Debug: Print intermediate result after first half
        println!("=== AFTER FIRST HALF (EVAL) ===");
        println!("out matrix after first half:");
        for i in 0..self.n {
            for j in 0..self.m {
                print!("{:02x} ", self.first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("==============================");


    }

    pub fn execute_second_half_outer_product(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<Block>>,
    ) {
        assert!(self.phase == ProtocolPhase::FirstHalf);
        self.phase = ProtocolPhase::SecondHalf;

        let y = self.y.clone();
        let zeros = BlockMatrix::constant(self.n, 1, Block::default());

        // Debug: Print second half inputs
        println!("=== SECOND HALF DEBUG (EVAL) ===");
        println!("eval_x (self.y) clear value: {}", y.get_clear_value());
        println!("eval_y (zeros) clear value: {}", zeros.get_clear_value());
        println!("self.y LSBs: {:?}", (0..self.m).map(|i| self.y[i].lsb()).collect::<Vec<_>>());
        println!("y LSBs: {:?}", (0..y.rows()).map(|i| y[i].lsb()).collect::<Vec<_>>());
        println!("=================================");

        self.eval_chunked_half_outer_product(&y.as_view(), &zeros.as_view(), chunk_levels, chunk_cts);

        // Debug: Print intermediate result after second half
        println!("=== AFTER SECOND HALF (EVAL) ===");
        println!("out matrix after second half:");
        for i in 0..self.m {
            for j in 0..self.n {
                print!("{:02x} ", self.second_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("===============================");

    }

    pub fn execute_final_outer_product(
        &mut self,
    ) -> BlockMatrix {
        assert!(self.phase == ProtocolPhase::SecondHalf);
        self.phase = ProtocolPhase::Final;

        let eval_alpha_beta = BlockMatrix::constant(self.n, self.m, Block::default());
        
        // accumulate the results into the first matrix
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^= self.second_half_out[(j, i)] ^ eval_alpha_beta[(i, j)];
            }
        }
        
        // Debug: Print final phase
        println!("=== FINAL PHASE DEBUG (EVAL) ===");
        println!("out matrix (no final correction needed):");
        for i in 0..self.n {
            for j in 0..self.m {
                print!("{:02x} ", self.first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("=================================");
        
        // The result is already accumulated in self.out from both halves
        // No additional computation needed - just return the accumulated result
        self.first_half_out.clone()
    }

}