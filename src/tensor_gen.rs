use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    block::Block,
    delta::Delta,
    matrix::{
        BlockMatrix,
        MatrixViewRef},
    tensor_pre::SemiHonestTensorPreGen,
    tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product},
};

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
    
    pub gb_first_half_out_dgb: BlockMatrix,
    pub gb_second_half_out_dgb: BlockMatrix,
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
            gb_first_half_out_dgb: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            gb_second_half_out_dgb: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
        }
    }



    pub fn gen_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
        first_half: bool,
    ) -> (Vec<Vec<Block>>, Vec<Vec<Block>>) {

        let mut chunk_levels: Vec<Vec<Block>> = Vec::new();
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
                self.gb_first_half_out_dgb.as_view_mut()
            } else {
                self.gb_second_half_out_dgb.as_view_mut()
            };

            out.with_subrows(self.chunking_factor * s, slice_size, |part| {
    
                let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(slice.elements_slice(), cipher, delta);
                let gen_cts = gen_unary_outer_product(&gen_seeds, &y, part, cipher);
    
                chunk_levels.push(levels);
                chunk_cts.push(gen_cts);
            });
        }

        (chunk_levels, chunk_cts)
    }

    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut x_dgb = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            x_dgb[i] = self.x_labels[i];
        }
        let mut y_dgb = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_dgb[i] = self.y_labels[i] ^ self.beta_labels[i];
        }
        (x_dgb, y_dgb)
    }
    
    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut x_dgb = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            x_dgb[j] = self.y_labels[j];
        }
        let mut y_dgb = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y_dgb[i] = self.alpha_labels[i];
        }
        (x_dgb, y_dgb)
    }

    pub fn garble_first_half_outer_product(
        &mut self
    ) -> (Vec<Vec<Block>>, Vec<Vec<Block>>) {

        let (x_dgb, y_dgb) = self.get_first_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x_dgb.as_view(), &y_dgb.as_view(), true);

        (chunk_levels, chunk_cts)
    }

    pub fn garble_second_half_outer_product(
        &mut self
    ) -> (Vec<Vec<Block>>, Vec<Vec<Block>>) {

        let (x_dgb, y_dgb) = self.get_second_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x_dgb.as_view(), &y_dgb.as_view(), false);

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
                self.gb_first_half_out_dgb[(i, j)] ^= self.gb_second_half_out_dgb[(j, i)] ^ gen_alpha_beta[(i, j)];
            }
        }

        self.gb_first_half_out_dgb.clone() 
    }
}

