use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::{BlockMatrix, MatrixViewMut, MatrixViewRef}, tensor_pre::TensorProductPreGen};

#[derive(PartialEq, Clone, Copy)]
enum ProtocolPhase {
    Setup,
    FirstHalf,
    SecondHalf,
    Final,
}

pub struct TensorProductGen {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,
    n: usize,
    m: usize,
    delta: Delta,

    x: BlockMatrix,
    y: BlockMatrix,
    alpha: BlockMatrix,
    beta: BlockMatrix,
    
    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,

    phase: ProtocolPhase,
}

impl TensorProductGen {
    pub fn new(tensor_pre: TensorProductPreGen) -> Self {
        
        Self {
            cipher: tensor_pre.cipher,
            chunking_factor: tensor_pre.chunking_factor, 
            n: tensor_pre.n, 
            m: tensor_pre.m, 
            delta: tensor_pre.delta, 
            x: tensor_pre.x, 
            y: tensor_pre.y, 
            alpha: tensor_pre.alpha, 
            beta: tensor_pre.beta, 
            first_half_out: BlockMatrix::new(tensor_pre.n, tensor_pre.m),
            second_half_out: BlockMatrix::new(tensor_pre.m, tensor_pre.n),
            phase: ProtocolPhase::Setup,
        }
    }

    fn gen_populate_seeds_mem_optimized(
        x: &MatrixViewRef<Block>, // TODO: should be a slice or a range
        cipher: &FixedKeyAes,
        delta: Delta,
    ) -> (Vec<Block>, Vec<(Block, Block)>) {

        let mut tree: Vec<Block> = Vec::new();
        let mut odd_evens: Vec<(Block, Block)> = Vec::new();
    
        let n: usize = x.len();
        
        println!("DEBUG: gen_populate_seeds_mem_optimized - n: {}, x LSBs: {:?}", n, 
                 (0..n).map(|i| x[i].lsb()).collect::<Vec<_>>());
    
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
            println!("DEBUG: gen_populate_seeds_mem_optimized - level {}: added (evens, odds) to odd_evens", i);
            
            // Add all non-default seeds from this level to the tree
            for idx in 0..(1 << (i+1)) {
                if seeds[idx] != Block::default() {
                    tree.push(seeds[idx]);
                }
            }
        }

        let seeds = tree[tree.len() - (1 << x.rows())..tree.len()].to_vec();
    
        (seeds, odd_evens)
    }

    /// Computes the unary outer product [|T(f) * (U(x + c) & y)|] where & is the vector outer product
    /// and T(f) is the truth table of f. The resulting matrix is l x m.
    fn gen_unary_outer_product(
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

    pub fn  gen_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) { // awful return type
    
        let mut chunk_levels: Vec<Vec<(Block, Block)>> = Vec::new();
        let mut chunk_cts: Vec<Vec<Block>> = Vec::new();
    
        for s in 0..((x.rows() + self.chunking_factor-1)/self.chunking_factor) {
            let slice_size: usize;
            if self.chunking_factor *(s+1) > x.rows() {slice_size = x.rows() % self.chunking_factor;} else {slice_size = self.chunking_factor;}
            let mut slice = BlockMatrix::new(slice_size, 1);
            for i in 0..slice_size {
                slice[i] = x[i + s * self.chunking_factor];
            }

            // Extract fields before closure
            let cipher = self.cipher;
            let delta = self.delta;
        
            // IMPORTANT: transpose the out matrix before calling with_subrows for the second half
            let mut out = if self.phase == ProtocolPhase::SecondHalf {
                self.second_half_out.as_view_mut()
            } else {
                self.first_half_out.as_view_mut()
            };

            out.with_subrows(self.chunking_factor * s, slice_size, |part| {
    
                let (gen_seeds, levels) = Self::gen_populate_seeds_mem_optimized(&slice.as_view(), cipher, delta);
                let gen_cts = Self::gen_unary_outer_product(&gen_seeds, &y, part, cipher);
    
                chunk_levels.push(levels);
                chunk_cts.push(gen_cts);
            });
        }

        (chunk_levels, chunk_cts)
    }

    pub fn execute_first_half_outer_product(
        &mut self
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
        assert!(self.phase == ProtocolPhase::Setup);
        self.phase = ProtocolPhase::FirstHalf;

        // get gen_x and gen_y for this half
        let gen_x = &self.x.clone();
        let gen_y = &self.y.clone() ^ &self.beta;

        // Debug: Print first half inputs
        println!("=== FIRST HALF DEBUG (GEN) ===");
        println!("gen_x clear value: {}", gen_x.get_clear_value());
        println!("gen_y clear value: {}", gen_y.get_clear_value());
        println!("self.x clear value: {}", self.x.get_clear_value());
        println!("self.y clear value: {}", self.y.get_clear_value());
        println!("self.beta clear value: {}", self.beta.get_clear_value());
        println!("Computed gen_y = self.y ^ self.beta = {} ^ {} = {}", 
                 self.y.get_clear_value(), self.beta.get_clear_value(), 
                 (&self.y ^ &self.beta).get_clear_value());
        println!("==============================");

        // run the protocol
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&gen_x.as_view(), &gen_y.as_view());

        // Debug: Print intermediate result after first half
        println!("=== AFTER FIRST HALF (GEN) ===");
        println!("out matrix after first half:");
        for i in 0..self.n {
            for j in 0..self.m {
                print!("{:02x} ", self.first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("=============================");

        (chunk_levels, chunk_cts)
    }

    pub fn execute_second_half_outer_product(
        &mut self
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
        assert!(self.phase == ProtocolPhase::FirstHalf);
        self.phase = ProtocolPhase::SecondHalf;

        // get gen_x and gen_y for this half
        let gen_x = self.y.clone();
        let gen_y = self.alpha.clone();

        // Debug: Print second half inputs
        println!("=== SECOND HALF DEBUG (GEN) ===");
        println!("gen_x (self.y ^ self.beta) clear value: {}", gen_x.get_clear_value());
        println!("gen_y (self.alpha) clear value: {}", gen_y.get_clear_value());
        println!("self.alpha clear value: {}", self.alpha.get_clear_value());
        println!("self.y clear value: {}", self.y.get_clear_value());
        println!("===============================");

        // run the protocol
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&gen_x.as_view(), &gen_y.as_view());
        
        // Debug: Print what generator produces
        println!("DEBUG: Generator second half produces:");
        println!("  chunk_levels len: {}", chunk_levels.len());
        for (i, levels) in chunk_levels.iter().enumerate() {
            println!("  chunk_levels[{}] len: {}", i, levels.len());
        }
        println!("  chunk_cts len: {}", chunk_cts.len());
        for (i, cts) in chunk_cts.iter().enumerate() {
            println!("  chunk_cts[{}] len: {}", i, cts.len());
        }

        // Debug: Print intermediate result after second half
        println!("=== AFTER SECOND HALF (GEN) ===");
        println!("out matrix after second half:");
        for i in 0..self.m {
            for j in 0..self.n {
                print!("{:02x} ", self.second_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("==============================");

        (chunk_levels, chunk_cts)
    }

    pub fn execute_final_outer_product(
        &mut self
    ) -> BlockMatrix {
        assert!(self.phase == ProtocolPhase::SecondHalf);
        self.phase = ProtocolPhase::Final;
        
        let gen_alpha_beta = self.alpha.color_cross_product(&self.beta, self.delta);

        // accumulate the results into the first matrix
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^= self.second_half_out[(j, i)] ^ gen_alpha_beta[(i, j)];
            }
        }

        // Debug: Print final phase
        println!("=== FINAL PHASE DEBUG (GEN) ===");
        println!("alpha clear value: {}", self.alpha.get_clear_value());
        println!("beta clear value: {}", self.beta.get_clear_value());
        println!("alpha_beta clear values:");
        for i in 0..self.n {
            for j in 0..self.m {
                print!("{} ", gen_alpha_beta[(i, j)].lsb() as u8);
            }
            println!();
        }
        println!("out matrix before final correction:");
        for i in 0..self.n {
            for j in 0..self.m {
                print!("{:02x} ", self.first_half_out[(i, j)].as_bytes()[0]);
            }
            println!();
        }
        println!("===============================");


        self.first_half_out.clone()
    }
}

