use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    block::Block,
    matrix::{
        BlockMatrix,
        MatrixViewRef
    }, tensor_pre::SemiHonestTensorPreEval
};

pub struct TensorProductEval {
    pub cipher: &'static FixedKeyAes,
    pub chunking_factor: usize,
    pub n: usize,
    pub m: usize,

    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,
    
    pub alpha_labels: BlockMatrix,
    pub beta_labels: BlockMatrix,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,
}

impl TensorProductEval {
    pub fn new(tensor_eval: SemiHonestTensorPreEval) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            chunking_factor: tensor_eval.chunking_factor,
            n: tensor_eval.n,
            m: tensor_eval.m,
            x_labels: tensor_eval.x_labels,
            y_labels: tensor_eval.y_labels,
            alpha_labels: BlockMatrix::constant(tensor_eval.n, 1, Block::default()),
            beta_labels: BlockMatrix::constant(tensor_eval.m, 1, Block::default()),
            first_half_out: BlockMatrix::new(tensor_eval.n, tensor_eval.m),
            second_half_out: BlockMatrix::new(tensor_eval.m, tensor_eval.n),
        }
    }

    pub fn new_from_fpre_eval(fpre_eval: SemiHonestTensorPreEval) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            chunking_factor: fpre_eval.chunking_factor,
            n: fpre_eval.n,
            m: fpre_eval.m,
            x_labels: fpre_eval.x_labels,
            y_labels: fpre_eval.y_labels,
            alpha_labels: BlockMatrix::constant(fpre_eval.n, 1, Block::default()),
            beta_labels: BlockMatrix::constant(fpre_eval.m, 1, Block::default()),
            first_half_out: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
        }
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
                let (eval_seeds, _missing_derived) = crate::tensor_ops::eval_populate_seeds_mem_optimized(
                    slice.elements_slice(),
                    chunk_levels[s].clone(),
                    cipher,
                );
                let _eval_cts = crate::tensor_ops::eval_unary_outer_product(
                    &eval_seeds,
                    &y,
                    part,
                    cipher,
                    slice_clear,
                    &chunk_cts[s],
                );
            });
        }
    }

    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut eval_x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            eval_x[i] = self.x_labels[i];
        }
        let mut eval_y = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            eval_y[j] = self.y_labels[j];
        }

        (eval_x, eval_y)
    }

    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut eval_x = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            eval_x[j] = self.y_labels[j];
        }

        let mut eval_y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            eval_y[i] = self.alpha_labels[i];
        }

        (eval_x, eval_y)
    }

    pub fn evaluate_first_half_outer_product(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<Block>>,
    ) {
        let (eval_x, eval_y) = self.get_first_inputs();
        self.eval_chunked_half_outer_product(&eval_x.as_view(), &eval_y.as_view(), chunk_levels, chunk_cts, true);
    }

    pub fn evaluate_second_half_outer_product(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<Block>>,
    ) {
        let (eval_x, eval_y) = self.get_second_inputs();
        self.eval_chunked_half_outer_product(&eval_x.as_view(), &eval_y.as_view(), chunk_levels, chunk_cts, false);

    }

    pub fn evaluate_final_outer_product(
        &mut self,
    ) -> BlockMatrix {
        // NOTE: In the semi-honest variant the evaluator's correlated share
        // (alpha ⊗ beta) is always zero — alpha_labels is never populated from
        // preprocessing data, so there is no eval_alpha_beta term to XOR in here.
        // The authenticated path (auth_tensor_eval.rs) handles the non-zero case
        // via correlated_auth_bit_shares[j*n+i].mac.
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^= self.second_half_out[(j, i)];
            }
        }

        self.first_half_out.clone()
    }

}