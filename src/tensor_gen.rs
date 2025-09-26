use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    block::Block,
    delta::Delta,
    matrix::{
        BlockMatrix,
        MatrixViewMut,
        MatrixViewRef},
    tensor_pre::SemiHonestTensorPreGen,
};

#[derive(PartialEq, Clone, Copy)]
enum ProtocolPhase {
    Setup,
    FirstHalf,
    SecondHalf,
    Final,
}

pub struct TensorProductGen {
    pub cipher: &'static FixedKeyAes,
    pub chunking_factor: usize,
    pub n: usize,
    pub m: usize,
    pub delta: Delta,

    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    
    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,
}

impl TensorProductGen {

    pub fn new_from_fpre_gen(fpre_gen: SemiHonestTensorPreGen) -> Self {

        Self {
            cipher: &FIXED_KEY_AES,
            n: fpre_gen.n,
            m: fpre_gen.m,
            chunking_factor: fpre_gen.chunking_factor,
            delta: fpre_gen.delta,
            x_labels: fpre_gen.x_labels,
            y_labels: fpre_gen.y_labels,
            alpha_labels: fpre_gen.alpha_labels,
            beta_labels: fpre_gen.beta_labels,
            first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
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

    pub fn gen_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
        first_half: bool,
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

            let mut out = if first_half {
                self.first_half_out.as_view_mut()
            } else {
                self.second_half_out.as_view_mut()
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

    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut gen_x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            gen_x[i] = self.x_labels[i];
        }
        let mut gen_y = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            gen_y[i] = self.y_labels[i] ^ self.beta_labels[i];
        }
        (gen_x, gen_y)
    }
    
    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut gen_x = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            gen_x[j] = self.y_labels[j];
        }
        let mut gen_y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            gen_y[i] = self.alpha_labels[i];
        }
        (gen_x, gen_y)
    }

    pub fn garble_first_half_outer_product(
        &mut self
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {

        let (gen_x, gen_y) = self.get_first_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&gen_x.as_view(), &gen_y.as_view(), true);

        (chunk_levels, chunk_cts)
    }

    pub fn garble_second_half_outer_product(
        &mut self
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {

        let (gen_x, gen_y) = self.get_second_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&gen_x.as_view(), &gen_y.as_view(), false);

        (chunk_levels, chunk_cts)
    }

    pub fn color_cross_product(&self, delta: Delta) -> BlockMatrix {

        let mut out = BlockMatrix::new(self.n, self.m);
        for i in 0..self.n {
            for j in 0..self.m {
                let a1 = self.alpha_labels[i] != Block::ZERO;
                let b1 = self.beta_labels[j] != Block::ZERO;
                out[(i, j)] = if a1 && b1 { *delta.as_block() } else { Block::default() };
            }
        }
        out
    }

    pub fn garble_final_outer_product(
        &mut self
    ) -> BlockMatrix {
        let gen_alpha_beta = self.color_cross_product(self.delta);

        // accumulate the results into the first matrix
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^= self.second_half_out[(j, i)] ^ gen_alpha_beta[(i, j)];
            }
        }

        self.first_half_out.clone() 
    }
}

