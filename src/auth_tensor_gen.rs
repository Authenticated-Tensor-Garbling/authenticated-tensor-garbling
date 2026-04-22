use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    delta::Delta,
    sharing::AuthBitShare,
    preprocessing::TensorFpreGen,
    block::Block,
    matrix::{BlockMatrix, MatrixViewRef},
    tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product},
};

pub struct AuthTensorGen {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,
    
    n: usize,
    m: usize,
    
    pub delta_a: Delta,
    
    pub x_labels: Vec<Block>,
    pub y_labels: Vec<Block>,

    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,
}

impl AuthTensorGen {
    pub fn new(n: usize, m: usize, chunking_factor: usize) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            n,
            m,
            chunking_factor,
            delta_a: Delta::random(&mut rand::rng()),
            x_labels: Vec::new(),
            y_labels: Vec::new(),
            alpha_auth_bit_shares: Vec::new(),
            beta_auth_bit_shares: Vec::new(),
            correlated_auth_bit_shares: Vec::new(),
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
        }
    }

    pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            n: fpre_gen.n,
            m: fpre_gen.m,
            chunking_factor: fpre_gen.chunking_factor,
            delta_a: fpre_gen.delta_a,
            x_labels: fpre_gen.alpha_labels,
            y_labels: fpre_gen.beta_labels,
            alpha_auth_bit_shares: fpre_gen.alpha_auth_bit_shares,
            beta_auth_bit_shares: fpre_gen.beta_auth_bit_shares,
            correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
            first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
        }
    }



    pub fn gen_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
        first_half: bool,
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
    
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
            let delta = self.delta_a;

            let mut out = if first_half {
                self.first_half_out.as_view_mut()
            } else {
                self.second_half_out.as_view_mut()
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

    /// returns: the garbler's x and y inputs to the first tensor half gate
    /// x <= input_x (x) alpha == x_labels
    /// y <= beta
    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {

        let mut x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            x[i] = self.x_labels[i];
        }
        
        let mut y = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            let b_share =
                if self.beta_auth_bit_shares[i].bit()
                {
                    self.delta_a.as_block() ^ self.beta_auth_bit_shares[i].key.as_block() ^ self.y_labels[i]
                } else {
                    *self.beta_auth_bit_shares[i].key.as_block() ^ self.y_labels[i]
                };
            
            y[i] = b_share;
        }

        (x, y)
    }

    /// returns: the evaluator's x and y inputs to the second tensor half gate
    /// x <= beta
    /// y <= masked_x
    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        let mut x = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            x[i] = self.y_labels[i];
        }
        
        let mut y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            let alpha_share =
                if self.alpha_auth_bit_shares[i].bit()
                {
                    self.delta_a.as_block() ^ self.alpha_auth_bit_shares[i].key.as_block()
                } else {
                    *self.alpha_auth_bit_shares[i].key.as_block()
                };
            y[i] = alpha_share;
        }

        (x, y)
    }

    pub fn garble_first_half(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
        let (x, y) = self.get_first_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x.as_view(), &y.as_view(), true);

        (chunk_levels, chunk_cts)
    }

    pub fn garble_second_half(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
        let (x, y) = self.get_second_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x.as_view(), &y.as_view(), false);

        (chunk_levels, chunk_cts)
    }

    /// Combines both half-outer-product outputs with the correlated preprocessing
    /// share to produce the garbled tensor gate output.
    pub fn garble_final(&mut self) {
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share = if self.correlated_auth_bit_shares[j * self.n + i].bit() {
                    self.delta_a.as_block() ^ self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
                } else {
                    *self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
                };

                self.first_half_out[(i, j)] ^=
                    self.second_half_out[(j, i)] ^
                    correlated_share;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth_tensor_fpre::TensorFpre};

    #[test]
    fn test_garble_first_half() {
        let n = 4;
        let m = 3;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_for_ideal_trusted_dealer(0b1101, 0b110);

        let (fpre_gen, _) = fpre.into_gen_eval();

        assert_eq!(fpre_gen.alpha_labels.len(), n);
        assert_eq!(fpre_gen.beta_labels.len(), m);

        assert_eq!(fpre_gen.alpha_auth_bit_shares.len(), n);
        assert_eq!(fpre_gen.beta_auth_bit_shares.len(), m);

        assert_eq!(fpre_gen.correlated_auth_bit_shares.len(), n * m);

        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);

        assert_eq!(gar.x_labels.len(), n);
        assert_eq!(gar.y_labels.len(), m);

        assert_eq!(gar.alpha_auth_bit_shares.len(), n);
        assert_eq!(gar.beta_auth_bit_shares.len(), m);

        assert_eq!(gar.correlated_auth_bit_shares.len(), n * m);

        let (_chunk_levels, _chunk_cts) = gar.garble_first_half();

    }
}