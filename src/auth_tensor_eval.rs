use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::BlockMatrix};
use crate::sharing::AuthBitShare;
use crate::aes::FIXED_KEY_AES;
use crate::preprocessing::TensorFpreEval;
use crate::matrix::MatrixViewRef;

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

    /// Combines both half-outer-product outputs with the correlated preprocessing
    /// MAC to produce the evaluator's share of the garbled tensor gate output.
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