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
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,

    /// Set to `true` by `evaluate_final()`. `compute_lambda_gamma()` asserts
    /// this flag to prevent silent garbage output when called out of order.
    final_computed: bool,
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
            gamma_auth_bit_shares: Vec::new(),
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
            final_computed: false,
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
            gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,
            first_half_out: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
            final_computed: false,
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
            

            // Extract explicit choice bits from slice LSBs (index 0 = LSB of bit vector).
            let slice_bits: Vec<bool> = slice.elements_slice().iter().map(|b| b.lsb()).collect();

            out.with_subrows(chunking_factor * s, slice_size, |part| {
                let (eval_seeds, _missing_derived) = crate::tensor_ops::eval_populate_seeds_mem_optimized(
                    slice.elements_slice(),
                    &slice_bits,
                    &chunk_levels[s],
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
        self.final_computed = true;
    }

    /// Reconstructs the masked output `L_gamma` per (i,j) given the garbler's
    /// `[L_gamma]^gb` from the garbled circuit.
    ///
    /// MUST be called AFTER `evaluate_final()` — `first_half_out` only holds
    /// `[v_gamma D_gb]^ev` once `evaluate_final` has XORed in the correlated MAC.
    /// Calling earlier returns garbage.
    ///
    /// Per CONTEXT.md D-05 (paper 5_online.tex line 160):
    ///   `L_gamma[j*n+i] = lambda_gb[j*n+i]
    ///                     XOR first_half_out[(i,j)].lsb()
    ///                     XOR gamma_auth_bit_shares[j*n+i].bit()`
    ///
    /// Output is column-major: `vec[j * self.n + i]` corresponds to gate output (i, j).
    /// Returns the reconstructed `L_gamma := v_gamma XOR l_gamma` (the masked output
    /// value that the consistency check and the output decoding step both consume).
    ///
    /// Note on D_gb vs D_ev: see the corresponding doc on AuthTensorGen's method —
    /// `AuthBitShare::bit()` is delta-independent so the D_ev-authenticated
    /// gamma_auth_bit_shares yields the correct extbit value despite the paper's
    /// D_gb notation. See 08-RESEARCH.md Pitfall 1.
    ///
    /// # Panics
    /// - Panics if `lambda_gb.len() != self.n * self.m`.
    /// - Panics if `gamma_auth_bit_shares.len() != self.n * self.m`
    ///   (UncompressedPreprocessingBackend stub leaves it empty — use IdealPreprocessingBackend).
    pub fn compute_lambda_gamma(&self, lambda_gb: &[bool]) -> Vec<bool> {
        assert!(
            self.final_computed,
            "compute_lambda_gamma called before evaluate_final — \
             first_half_out is not yet the combined v_gamma encoding"
        );
        assert_eq!(
            lambda_gb.len(),
            self.n * self.m,
            "compute_lambda_gamma: lambda_gb length must equal n*m"
        );
        assert_eq!(
            self.gamma_auth_bit_shares.len(),
            self.n * self.m,
            "compute_lambda_gamma requires gamma_auth_bit_shares.len() == n*m; \
             UncompressedPreprocessingBackend leaves this vec empty — \
             use IdealPreprocessingBackend"
        );

        let mut out = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let idx = j * self.n + i;
                let v_extbit  = self.first_half_out[(i, j)].lsb();
                let lg_extbit = self.gamma_auth_bit_shares[idx].bit();
                out.push(lambda_gb[idx] ^ v_extbit ^ lg_extbit);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};

    fn build_pair(n: usize, m: usize) -> (AuthTensorGen, AuthTensorEval) {
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        (gb, ev)
    }

    fn run_full_garble_eval(gb: &mut AuthTensorGen, ev: &mut AuthTensorEval) {
        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();
    }

    #[test]
    fn test_compute_lambda_gamma_reconstruction() {
        let n = 4;
        let m = 3;
        let (mut gb, mut ev) = build_pair(n, m);
        assert_eq!(ev.gamma_auth_bit_shares.len(), n * m,
            "ev.gamma_auth_bit_shares must be length n*m after new_from_fpre_eval");

        run_full_garble_eval(&mut gb, &mut ev);
        let lambda_gb = gb.compute_lambda_gamma();
        assert_eq!(lambda_gb.len(), n * m);

        let result = ev.compute_lambda_gamma(&lambda_gb);
        assert_eq!(result.len(), n * m,
            "ev.compute_lambda_gamma must return Vec<bool> of length n*m");
    }

    #[test]
    fn test_compute_lambda_gamma_xors_three_inputs() {
        let n = 4;
        let m = 3;
        let (mut gb, mut ev) = build_pair(n, m);
        run_full_garble_eval(&mut gb, &mut ev);
        let lambda_gb = gb.compute_lambda_gamma();
        let result = ev.compute_lambda_gamma(&lambda_gb);

        // Probe one specific (i, j) entry to verify the three-way XOR per D-05.
        let i = 2;
        let j = 1;
        let idx = j * n + i; // == 6
        let expected = lambda_gb[idx]
                     ^ ev.first_half_out[(i, j)].lsb()
                     ^ ev.gamma_auth_bit_shares[idx].bit();
        assert_eq!(result[idx], expected,
            "ev.compute_lambda_gamma at (i=2, j=1) does not match D-05 formula");
    }

    #[test]
    #[should_panic(expected = "compute_lambda_gamma: lambda_gb length must equal n*m")]
    fn test_compute_lambda_gamma_panics_on_wrong_lambda_length() {
        let n = 4;
        let m = 3;
        let (mut gb, mut ev) = build_pair(n, m);
        run_full_garble_eval(&mut gb, &mut ev);
        // Pass a slice of the wrong length (5 instead of n*m=12).
        let bogus = vec![false; 5];
        let _ = ev.compute_lambda_gamma(&bogus);
    }
}