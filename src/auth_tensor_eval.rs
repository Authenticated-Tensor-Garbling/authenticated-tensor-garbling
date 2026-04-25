use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::BlockMatrix};
use crate::sharing::AuthBitShare;
use crate::aes::FIXED_KEY_AES;
use crate::preprocessing::TensorFpreEval;
use crate::matrix::MatrixViewRef;
use crate::tensor_ops::eval_unary_outer_product_wide;

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

    /// Evaluator's D_ev-authenticated shares of `l_alpha`; length n. Phase 9 P2-01.
    pub alpha_d_ev_shares: Vec<AuthBitShare>,
    /// Evaluator's D_ev-authenticated shares of `l_beta`; length m. Phase 9 P2-01.
    pub beta_d_ev_shares: Vec<AuthBitShare>,
    /// Evaluator's D_ev-authenticated shares of `l_gamma*`; length n*m, column-major.
    /// Phase 9 P2-01.
    pub correlated_d_ev_shares: Vec<AuthBitShare>,
    /// Evaluator's D_ev-authenticated shares of `l_gamma`; length n*m, column-major.
    /// (Phase 9 D-05.)
    pub gamma_d_ev_shares: Vec<AuthBitShare>,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,

    /// D_ev (rho-half) accumulator for the first half-outer-product. Phase 9 P2-03.
    /// Mirrors `first_half_out` but accumulates the rho-half PRG outputs from
    /// `eval_unary_outer_product_wide`. Written by `evaluate_first_half_p2` /
    /// `evaluate_second_half_p2`; consumed by `evaluate_final_p2`.
    pub first_half_out_ev: BlockMatrix,
    /// D_ev (rho-half) accumulator for the second half-outer-product. Phase 9 P2-03.
    pub second_half_out_ev: BlockMatrix,

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
            alpha_d_ev_shares: Vec::new(),
            beta_d_ev_shares: Vec::new(),
            correlated_d_ev_shares: Vec::new(),
            gamma_d_ev_shares: Vec::new(),
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
            first_half_out_ev: BlockMatrix::new(n, m),
            second_half_out_ev: BlockMatrix::new(m, n),
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
            alpha_d_ev_shares: fpre_eval.alpha_d_ev_shares,
            beta_d_ev_shares: fpre_eval.beta_d_ev_shares,
            correlated_d_ev_shares: fpre_eval.correlated_d_ev_shares,
            gamma_d_ev_shares: fpre_eval.gamma_d_ev_shares,
            first_half_out: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
            first_half_out_ev: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            second_half_out_ev: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
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

    /// Phase 9 P2-03. Wide-leaf variant of `eval_chunked_half_outer_product`.
    /// Consumes wide ciphertexts `Vec<Vec<(Block, Block)>>` and writes BOTH the
    /// D_gb output (`first_half_out` / `second_half_out`) AND the D_ev output
    /// (`first_half_out_ev` / `second_half_out_ev`) in a single pass.
    fn eval_chunked_half_outer_product_wide(
        &mut self,
        x: &MatrixViewRef<Block>,
        y_d_gb: &MatrixViewRef<Block>,
        y_d_ev: &MatrixViewRef<Block>,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
        first_half: bool,
    ) {
        let chunking_factor = self.chunking_factor;

        for s in 0..((x.rows() + chunking_factor - 1) / chunking_factor) {
            let slice_size: usize = if chunking_factor * (s + 1) > x.rows() {
                x.rows() % chunking_factor
            } else {
                chunking_factor
            };

            let mut slice = BlockMatrix::new(slice_size, 1);
            for i in 0..slice_size {
                slice[i] = x[i + s * chunking_factor];
            }

            let cipher = self.cipher;
            let slice_clear = slice.get_clear_value();
            let slice_bits: Vec<bool> =
                slice.elements_slice().iter().map(|b| b.lsb()).collect();

            // Disjoint-field split: borrow both D_gb and D_ev output halves.
            let (out_gb_full, out_ev_full): (
                &mut BlockMatrix,
                &mut BlockMatrix,
            ) = if first_half {
                (&mut self.first_half_out, &mut self.first_half_out_ev)
            } else {
                (&mut self.second_half_out, &mut self.second_half_out_ev)
            };

            let mut out_gb = out_gb_full.as_view_mut();
            let mut out_ev = out_ev_full.as_view_mut();

            // Nested with_subrows: each call yields a sub-view scoped to this
            // chunk's row range. The two views are over distinct backing
            // storage (different BlockMatrix fields).
            out_gb.with_subrows(chunking_factor * s, slice_size, |part_gb| {
                out_ev.with_subrows(chunking_factor * s, slice_size, |part_ev| {
                    let (eval_seeds, _missing_derived) =
                        crate::tensor_ops::eval_populate_seeds_mem_optimized(
                            slice.elements_slice(),
                            &slice_bits,
                            &chunk_levels[s],
                            cipher,
                        );
                    let _eval_cts = eval_unary_outer_product_wide(
                        &eval_seeds,
                        y_d_gb,
                        y_d_ev,
                        part_gb,
                        part_ev,
                        cipher,
                        slice_clear,
                        &chunk_cts[s],
                    );
                });
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

    /// Phase 9 P2-03 — y inputs (D_ev half) for `evaluate_first_half_p2`.
    ///
    /// The eval-side counterpart of the garbler's
    /// `get_first_inputs_p2_y_d_ev`. Mirrors `get_first_inputs` (D_gb path
    /// with `y_labels` XOR + `beta_auth_bit_shares.mac`) but for the D_ev
    /// rho-half: the rho-half does NOT carry wire labels, so there is no
    /// `y_labels` XOR. The eval emits `beta_d_ev_shares[i].mac.as_block()`
    /// directly — symmetric to the garbler-side encoding (mac is committed
    /// under the opposite party's delta per `gen_auth_bit`'s symmetric
    /// IT-MAC layout).
    fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_ev[i] = *self.beta_d_ev_shares[i].mac.as_block();
        }
        y_ev
    }

    /// Phase 9 P2-03 — y inputs (D_ev half) for `evaluate_second_half_p2`.
    fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y_ev[i] = *self.alpha_d_ev_shares[i].mac.as_block();
        }
        y_ev
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

    /// Phase 9 P2-03. Drives the wide GGM tree expansion for the first
    /// half-outer-product on the eval side. Consumes wide ciphertexts emitted
    /// by `AuthTensorGen::garble_first_half_p2`. Writes BOTH `first_half_out`
    /// (D_gb) and `first_half_out_ev` (D_ev) in a single pass.
    pub fn evaluate_first_half_p2(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
    ) {
        let (x, y_d_gb) = self.get_first_inputs();
        let y_d_ev = self.get_first_inputs_p2_y_d_ev();
        self.eval_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            chunk_levels,
            chunk_cts,
            true,
        );
    }

    /// Phase 9 P2-03. Drives the wide GGM tree expansion for the second
    /// half-outer-product on the eval side.
    pub fn evaluate_second_half_p2(
        &mut self,
        chunk_levels: Vec<Vec<(Block, Block)>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
    ) {
        let (x, y_d_gb) = self.get_second_inputs();
        let y_d_ev = self.get_second_inputs_p2_y_d_ev();
        self.eval_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            chunk_levels,
            chunk_cts,
            false,
        );
    }

    /// Phase 9 P2-03. Combines both halves into the final D_ev output share.
    ///
    /// Returns `[v_gamma D_ev]^ev` — length `n * m` in column-major (j*n + i)
    /// order. Also XORs the D_gb half into `first_half_out` for symmetry with
    /// `evaluate_final` so callers reading `first_half_out` after this method
    /// see the same D_gb output as after the P1 path.
    ///
    /// D_ev encoding rule (eval side): the eval HOLDS `delta_b`. Its public-bit
    /// encoding of `correlated_d_ev_shares[idx]` under `delta_b` is:
    ///
    ///   `if bit() then delta_b ^ key else key`
    ///
    /// This mirrors the P1 garbler-side encoding (`if bit() then delta_a ^ key
    /// else key`) under the opposite delta — by symmetry of `gen_auth_bit` the
    /// `correlated_d_ev_shares.key` on the eval side is the local key view that
    /// pairs with `correlated_d_ev_shares.mac` on the gen side under `delta_b`.
    ///
    /// Per CONTEXT.md D-11: returns the eval's D_ev output share vector;
    /// struct does not gain new persistent fields beyond `first_half_out_ev` /
    /// `second_half_out_ev` (private accumulators).
    pub fn evaluate_final_p2(&mut self) -> Vec<Block> {
        // D_gb path: identical to existing `evaluate_final`.
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^=
                    self.second_half_out[(j, i)] ^
                    self.correlated_auth_bit_shares[j * self.n + i].mac.as_block();
            }
        }

        // D_ev path: eval-side public-bit encoding under delta_b.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_d_ev =
                    if self.correlated_d_ev_shares[j * self.n + i].bit() {
                        self.delta_b.as_block()
                            ^ self.correlated_d_ev_shares[j * self.n + i].key.as_block()
                    } else {
                        *self.correlated_d_ev_shares[j * self.n + i].key.as_block()
                    };
                self.first_half_out_ev[(i, j)] ^=
                    self.second_half_out_ev[(j, i)] ^
                    correlated_share_d_ev;
            }
        }

        // Collect D_ev output (column-major).
        let mut d_ev_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                d_ev_out.push(self.first_half_out_ev[(i, j)]);
            }
        }

        self.final_computed = true;
        d_ev_out
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
    ///                     XOR gamma_d_ev_shares[j*n+i].bit()`
    ///
    /// Output is column-major: `vec[j * self.n + i]` corresponds to gate output (i, j).
    /// Returns the reconstructed `L_gamma := v_gamma XOR l_gamma` (the masked output
    /// value that the consistency check and the output decoding step both consume).
    ///
    /// Note on D_gb vs D_ev: see the corresponding doc on AuthTensorGen's method —
    /// `AuthBitShare::bit()` is delta-independent so the D_ev-authenticated
    /// gamma_d_ev_shares yields the correct extbit value despite the paper's
    /// D_gb notation. See 08-RESEARCH.md Pitfall 1.
    ///
    /// # Panics
    /// - Panics if `lambda_gb.len() != self.n * self.m`.
    /// - Panics if `gamma_d_ev_shares.len() != self.n * self.m`
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
            self.gamma_d_ev_shares.len(),
            self.n * self.m,
            "compute_lambda_gamma requires gamma_d_ev_shares.len() == n*m; \
             UncompressedPreprocessingBackend leaves this vec empty — \
             use IdealPreprocessingBackend"
        );

        let mut out = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                let idx = j * self.n + i;
                let v_extbit  = self.first_half_out[(i, j)].lsb();
                let lg_extbit = self.gamma_d_ev_shares[idx].bit();
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
        assert_eq!(ev.gamma_d_ev_shares.len(), n * m,
            "ev.gamma_d_ev_shares must be length n*m after new_from_fpre_eval");

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
                     ^ ev.gamma_d_ev_shares[idx].bit();
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

    #[test]
    fn test_evaluate_final_p2_returns_d_ev_share_vec() {
        // P2-03: evaluate_final_p2 returns Vec<Block> of length n*m (D_ev output share).
        let n = 4;
        let m = 3;
        let (mut gb, mut ev) = build_pair(n, m);

        let (cl1, ct1) = gb.garble_first_half_p2();
        ev.evaluate_first_half_p2(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half_p2();
        ev.evaluate_second_half_p2(cl2, ct2);
        let (_d_gb_gb, _d_ev_gb) = gb.garble_final_p2();
        let d_ev_ev = ev.evaluate_final_p2();

        assert_eq!(d_ev_ev.len(), n * m,
            "evaluate_final_p2 returns Vec<Block> of length n*m");
    }
}