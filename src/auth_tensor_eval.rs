use crate::{aes::FixedKeyAes, block::Block, delta::Delta, matrix::BlockMatrix};
use crate::aes::FIXED_KEY_AES;
use crate::preprocessing::TensorFpreEval;
use crate::matrix::MatrixViewRef;
use crate::tensor_ops::eval_unary_outer_product_wide;

pub struct AuthTensorEval {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,

    n: usize,
    m: usize,

    pub delta_ev: Delta,

    /// Block-form sharings under δ_gb (`_gen`) and δ_ev (`_eval`); see
    /// `TensorFpreEval` field doc for semantics. Online layer operates
    /// purely on these XOR-share Blocks per the paper's tensor macros.
    pub alpha_dev: Vec<Block>,
    pub alpha_dgb:  Vec<Block>,
    pub beta_dev: Vec<Block>,
    pub beta_dgb:  Vec<Block>,
    pub correlated_dev: Vec<Block>,
    pub correlated_dgb:  Vec<Block>,
    pub gamma_dev: Vec<Block>,
    pub gamma_dgb:  Vec<Block>,

    /// Eval's half of (sharing of x under δ_gb). Length n. Populated by
    /// `install_input_labels`. Auth-bit-style: this
    /// is the `key` half handed off by gen; gen retains the `mac` half
    /// in `AuthTensorGen.x_dgb`. Pair encodes `x_i · δ_gb`.
    pub x_dgb: Vec<Block>,
    /// Eval's half of (sharing of y under δ_gb). Length m.
    pub y_dgb: Vec<Block>,
    /// Eval's half of (sharing of x XOR α under δ_gb) — the input wire
    /// label, ev side. Length n. Populated by `install_input_labels`.
    /// Equals `x_dgb[i] XOR mac_e_α` where `mac_e_α` is eval's
    /// α-share `mac` from `alpha_auth_bit_shares`. Pair with gen's
    /// `masked_x_dgb` encodes `(x XOR α) · δ_gb`. Used as the GGM-tree
    /// seed input by `evaluate_first_half`.
    pub masked_x_dgb: Vec<Block>,
    /// Eval's half of (sharing of y XOR β under δ_gb). Length m.
    pub masked_y_dgb: Vec<Block>,
    /// Cleartext masked bits d_x[i] = x_i XOR α_i. Length n. Populated
    /// by `install_input_labels` from the gen handoff. Used as choice
    /// bits for first-half GGM-tree traversal in
    /// `eval_chunked_half_outer_product`, replacing the prior
    /// LSB-of-wire-label readout (which is no longer correct under
    /// auth-bit-style input encoding — eval's wire-label LSB is now
    /// `b_i`, not `d_i`).
    pub ev_masked_x_bits: Vec<bool>,
    /// Cleartext masked bits d_y[j] = y_j XOR β_j. Length m. Used for
    /// second-half GGM-tree traversal.
    pub ev_masked_y_bits: Vec<bool>,

    /// D_gb accumulator for the first half-outer-product. Populated by
    /// `evaluate_first_half` (P1) or `evaluate_first_half_p2` (P2). Read via
    /// `ev_first_half_out_dgb()` once the populating step has run.
    pub(crate) ev_first_half_out_dgb: BlockMatrix,
    /// D_gb accumulator for the second half-outer-product. Populated by
    /// `evaluate_second_half` (P1) or `evaluate_second_half_p2` (P2). Read via
    /// `ev_second_half_out_dgb()`.
    pub(crate) ev_second_half_out_dgb: BlockMatrix,

    /// D_ev (rho-half) accumulator for the first half-outer-product.    /// Mirrors `ev_first_half_out_dgb` but accumulates the rho-half PRG outputs from
    /// `eval_unary_outer_product_wide`. Written by `evaluate_first_half_p2` /
    /// `evaluate_second_half_p2`; consumed by `evaluate_final_p2`. Read via
    /// `ev_first_half_out_dev()`.
    pub(crate) ev_first_half_out_dev: BlockMatrix,
    /// D_ev (rho-half) accumulator for the second half-outer-product.    /// Read via `ev_second_half_out_dev()`.
    pub(crate) ev_second_half_out_dev: BlockMatrix,

    /// Set to `true` by `evaluate_final()`. `compute_lambda_gamma()` asserts
    /// this flag to prevent silent garbage output when called out of order.
    final_computed: bool,
}

impl AuthTensorEval {
    pub fn new_from_fpre_eval(fpre_eval: TensorFpreEval) -> Self {
        // See `AuthTensorGen::new_from_fpre_gen` for rationale.
        assert!(
            fpre_eval.chunking_factor > 0,
            "fpre_eval.chunking_factor must be at least 1"
        );
        Self {
            cipher: &(*FIXED_KEY_AES),
            chunking_factor: fpre_eval.chunking_factor,
            n: fpre_eval.n,
            m: fpre_eval.m,
            delta_ev: fpre_eval.delta_ev,
            alpha_dev: fpre_eval.alpha_dev,
            alpha_dgb: fpre_eval.alpha_dgb,
            beta_dev: fpre_eval.beta_dev,
            beta_dgb: fpre_eval.beta_dgb,
            correlated_dev: fpre_eval.correlated_dev,
            correlated_dgb: fpre_eval.correlated_dgb,
            gamma_dev: fpre_eval.gamma_dev,
            gamma_dgb: fpre_eval.gamma_dgb,
            x_dgb: Vec::new(),
            y_dgb: Vec::new(),
            masked_x_dgb: Vec::new(),
            masked_y_dgb: Vec::new(),
            ev_masked_x_bits: Vec::new(),
            ev_masked_y_bits: Vec::new(),
            ev_first_half_out_dgb: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            ev_second_half_out_dgb: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
            ev_first_half_out_dev: BlockMatrix::new(fpre_eval.n, fpre_eval.m),
            ev_second_half_out_dev: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
            final_computed: false,
        }
    }

/// Choice bits for GGM-tree traversal MUST be supplied explicitly via
    /// `choice_bits` (typically `&self.ev_masked_x_bits` for first half, or
    /// `&self.ev_masked_y_bits` for second half). The previous
    /// LSB-of-wire-label readout is no longer correct under the
    /// auth-bit-style construction — eval's wire-label LSB is now the
    /// local α-share (`b_α`), not the masked input bit `d_α`.
    ///
    /// `choice_bits.len()` must equal `x.rows()`.
    fn eval_chunked_half_outer_product(
        &mut self,
        x: &MatrixViewRef<Block>,
        y: &MatrixViewRef<Block>,
        choice_bits: &[bool],
        chunk_levels: Vec<Vec<Block>>,
        chunk_cts: Vec<Vec<Block>>,
        first_half: bool,
    ) {
        assert_eq!(choice_bits.len(), x.rows(),
            "choice_bits.len() ({}) must equal x.rows() ({})",
            choice_bits.len(), x.rows());

        let chunking_factor = self.chunking_factor;

        for s in 0..((x.rows() + chunking_factor-1)/chunking_factor) {
            let slice_size: usize;
            if chunking_factor *(s+1) > x.rows() {slice_size = x.rows() % chunking_factor;} else {slice_size = chunking_factor;}
            let mut slice = BlockMatrix::new(slice_size, 1);
            for i in 0..slice_size {
                slice[i] = x[i + s * chunking_factor];
            }

            let cipher = self.cipher;

            // Slice the explicit choice bits for this chunk and pack
            // them into `slice_clear` (bit-i ↔ position-i, matching the
            // prior `slice.get_clear_value()` semantics under the
            // pre-Phase-1.2 LSB convention).
            let chunk_choice_bits: Vec<bool> = (0..slice_size)
                .map(|i| choice_bits[i + s * chunking_factor])
                .collect();
            let mut slice_clear: usize = 0;
            for (i, &b) in chunk_choice_bits.iter().enumerate() {
                if b { slice_clear |= 1usize << i; }
            }

            // IMPORTANT: transpose the out matrix before calling with_subrows for the second half
            let mut out = if first_half {
                self.ev_first_half_out_dgb.as_view_mut()
            } else {
                self.ev_second_half_out_dgb.as_view_mut()
            };

            out.with_subrows(chunking_factor * s, slice_size, |part| {
                let (eval_seeds, _missing_derived) = crate::tensor_ops::eval_populate_seeds_mem_optimized(
                    slice.elements_slice(),
                    &chunk_choice_bits,
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

    /// Wide-leaf variant of `eval_chunked_half_outer_product`.
    /// Consumes paper-faithful single-Block-per-level tree cts
    /// `chunk_levels: Vec<Vec<Block>>` (Construction 4) plus wide leaf cts
    /// `chunk_cts: Vec<Vec<(Block, Block)>>` (κ-half AND ρ-half), and writes
    /// BOTH the D_gb output (`ev_first_half_out_dgb` / `ev_second_half_out_dgb`) AND the
    /// D_ev output (`ev_first_half_out_dev` / `ev_second_half_out_dev`) in a single
    /// pass.
    ///
    /// Choice bits MUST be supplied explicitly via `choice_bits`.
    /// `choice_bits.len()` must equal `x.rows()`.
    fn eval_chunked_half_outer_product_wide(
        &mut self,
        x: &MatrixViewRef<Block>,
        y_d_gb: &MatrixViewRef<Block>,
        y_d_ev: &MatrixViewRef<Block>,
        choice_bits: &[bool],
        chunk_levels: Vec<Vec<Block>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
        first_half: bool,
    ) {
        assert_eq!(choice_bits.len(), x.rows(),
            "choice_bits.len() ({}) must equal x.rows() ({})",
            choice_bits.len(), x.rows());

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

            let chunk_choice_bits: Vec<bool> = (0..slice_size)
                .map(|i| choice_bits[i + s * chunking_factor])
                .collect();
            let mut slice_clear: usize = 0;
            for (i, &b) in chunk_choice_bits.iter().enumerate() {
                if b { slice_clear |= 1usize << i; }
            }

            // Disjoint-field split: borrow both D_gb and D_ev output halves.
            let (out_gb_full, out_ev_full): (
                &mut BlockMatrix,
                &mut BlockMatrix,
            ) = if first_half {
                (&mut self.ev_first_half_out_dgb, &mut self.ev_first_half_out_dev)
            } else {
                (&mut self.ev_second_half_out_dgb, &mut self.ev_second_half_out_dev)
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
                            &chunk_choice_bits,
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

    /// Read-only view of the first-half D_gb accumulator. Meaningful only
    /// after `evaluate_first_half` (P1) or `evaluate_first_half_p2` (P2) has
    /// run; before that, the matrix is zero-initialized.
    #[inline]
    pub fn ev_first_half_out_dgb(&self) -> &BlockMatrix {
        &self.ev_first_half_out_dgb
    }

    /// Read-only view of the second-half D_gb accumulator. Meaningful only
    /// after `evaluate_second_half` (P1) or `evaluate_second_half_p2` (P2) has run.
    #[inline]
    pub fn ev_second_half_out_dgb(&self) -> &BlockMatrix {
        &self.ev_second_half_out_dgb
    }

    /// Read-only view of the first-half D_ev (rho-half) accumulator.
    /// Meaningful only after `evaluate_first_half_p2` has run.
    #[inline]
    pub fn ev_first_half_out_dev(&self) -> &BlockMatrix {
        &self.ev_first_half_out_dev
    }

    /// Read-only view of the second-half D_ev (rho-half) accumulator.
    /// Meaningful only after `evaluate_second_half_p2` has run.
    #[inline]
    pub fn ev_second_half_out_dev(&self) -> &BlockMatrix {
        &self.ev_second_half_out_dev
    }

    /// Eval-side counterpart of `AuthTensorGen::get_first_inputs`.
    ///
    /// Paper-aligned (`5_online.tex` §178, `6_total.tex` §157):
    /// the first half is `tensorev(n, m, ..., [(a ⊕ λ_a) D_gb]^ev, [λ_b D_gb]^ev)`.
    /// In codebase naming with `a = x` and `λ_b = β`:
    /// - `x[i] = masked_x_dgb[i]` — ev.s share `[(x ⊕ α) D_gb]^ev` from input encoding.
    /// - `y[j] = beta_dgb[j]`     — ev.s share `[β D_gb]^ev` from preprocessing.
    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        assert_eq!(self.masked_x_dgb.len(), self.n,
            "get_first_inputs: masked_x_dgb not populated; call encode_inputs first");
        assert_eq!(self.beta_dgb.len(), self.m,
            "get_first_inputs: beta_dgb not populated by preprocessing");

        let mut x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            x[i] = self.masked_x_dgb[i];
        }

        let mut y = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            y[j] = self.beta_dgb[j];
        }

        (x, y)
    }

    /// Eval-side counterpart of `AuthTensorGen::get_second_inputs`.
    ///
    /// Paper-aligned (`5_online.tex` §179, `6_total.tex` §158):
    /// the ev side calls `tensorev(m, n, ..., [(b ⊕ λ_b) D_gb]^ev, [a D_gb]^ev)`.
    /// With `a = x`, `b = y`:
    /// - `x[j] = masked_y_dgb[j]` — ev.s share `[(y ⊕ β) D_gb]^ev` from input encoding.
    /// - `y[i] = x_dgb[i]`        — ev.s share `[x D_gb]^ev` from input encoding.
    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        assert_eq!(self.masked_y_dgb.len(), self.m,
            "get_second_inputs: masked_y_dgb not populated; call encode_inputs first");
        assert_eq!(self.x_dgb.len(), self.n,
            "get_second_inputs: x_dgb not populated; call encode_inputs first");

        let mut x = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            x[j] = self.masked_y_dgb[j];
        }

        let mut y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y[i] = self.x_dgb[i];
        }

        (x, y)
    }

    /// Eval-side counterpart of `AuthTensorGen::get_first_inputs_p2_y_d_ev`.
    ///
    /// Paper-aligned with `get_first_inputs`'s D_gb side: the first-half y
    /// operand is `β`. This emits ev.s share of `[β D_ev]` from
    /// preprocessing — i.e. `beta_dev[i]`. Combined with the D_gb track
    /// (`beta_dgb[i]` via `get_first_inputs`), the wide GGM expansion
    /// reconstructs `(x ⊕ α) ⊗ β` under both deltas.
    fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_ev[i] = self.beta_dev[i];
        }
        y_ev
    }

    /// Eval-side counterpart of `AuthTensorGen::get_second_inputs_p2_y_d_ev`.
    ///
    /// Paper-aligned with `get_second_inputs`'s D_gb side: the second-half y
    /// operand is `x`. Per `5_online.tex` §211 the ev side sets
    /// `[v_a D_ev]^ev := [λ_a D_ev]^ev XOR L_a · D_ev` — so ev.s share of
    /// `[x D_ev]` is `alpha_dev[i] XOR (ev_masked_x_bits[i] · δ_ev)`. Combined
    /// with gen's `alpha_dev[i]` (= `[x D_ev]^gb` per the same §211),
    /// the XOR-share reconstructs to `x_i · δ_ev`.
    fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.n, 1);
        let delta_b_block = *self.delta_ev.as_block();
        for i in 0..self.n {
            y_ev[i] = if self.ev_masked_x_bits[i] {
                self.alpha_dev[i] ^ delta_b_block
            } else {
                self.alpha_dev[i]
            };
        }
        y_ev
    }

    pub fn evaluate_first_half(&mut self, chunk_levels: Vec<Vec<Block>>, chunk_cts: Vec<Vec<Block>>) {
        let (x, y) = self.get_first_inputs();
        // Choice bits cloned out so we don't hold &self while &mut self is in use.
        let choice_bits = self.ev_masked_x_bits.clone();
        self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), &choice_bits, chunk_levels, chunk_cts, true);
    }

    pub fn evaluate_second_half(&mut self, chunk_levels: Vec<Vec<Block>>, chunk_cts: Vec<Vec<Block>>) {
        let (x, y) = self.get_second_inputs();
        let choice_bits = self.ev_masked_y_bits.clone();
        self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), &choice_bits, chunk_levels, chunk_cts, false);
    }

    /// Combines both half-outer-product outputs with the correlated preprocessing
    /// share to produce the ev's share of the garbled tensor gate output.
    /// Per `5_online.tex` §180: `[c D_gb]^ev := Z_{c,0}^ev ⊕ (Z_{c,1}^ev)^T ⊕ [(λ_a ⊗ λ_b) D_gb]^ev`,
    /// where the third term is eval's preprocessing share `correlated_dgb[idx]`.
    pub fn evaluate_final(&mut self) {
        assert!(
            !self.final_computed,
            "evaluate_final called twice on the same instance — \
             ev_first_half_out_dgb would be double-XOR'd; create a new instance per gate"
        );
        assert_eq!(self.correlated_dgb.len(), self.n * self.m,
            "evaluate_final: correlated_dgb not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.ev_first_half_out_dgb[(i, j)] ^=
                    self.ev_second_half_out_dgb[(j, i)] ^
                    self.correlated_dgb[j * self.n + i];
            }
        }
        self.final_computed = true;
    }

    /// Drives the wide GGM tree expansion for the first
    /// half-outer-product on the ev side. Consumes wide ciphertexts emitted
    /// by `AuthTensorGen::garble_first_half_p2`. Writes BOTH `ev_first_half_out_dgb`
    /// (D_gb) and `ev_first_half_out_dev` (D_ev) in a single pass.
    pub fn evaluate_first_half_p2(
        &mut self,
        chunk_levels: Vec<Vec<Block>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
    ) {
        let (x, y_d_gb) = self.get_first_inputs();
        let y_d_ev = self.get_first_inputs_p2_y_d_ev();
        let choice_bits = self.ev_masked_x_bits.clone();
        self.eval_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            &choice_bits,
            chunk_levels,
            chunk_cts,
            true,
        );
    }

    /// Drives the wide GGM tree expansion for the second
    /// half-outer-product on the ev side.
    pub fn evaluate_second_half_p2(
        &mut self,
        chunk_levels: Vec<Vec<Block>>,
        chunk_cts: Vec<Vec<(Block, Block)>>,
    ) {
        let (x, y_d_gb) = self.get_second_inputs();
        let y_d_ev = self.get_second_inputs_p2_y_d_ev();
        let choice_bits = self.ev_masked_y_bits.clone();
        self.eval_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            &choice_bits,
            chunk_levels,
            chunk_cts,
            false,
        );
    }

    /// Combines both halves into the final D_ev output share.
    ///
    /// Returns `[v_gamma D_ev]^ev` — length `n * m` in column-major (j*n + i)
    /// order. Also XORs the D_gb half into `ev_first_half_out_dgb` for symmetry with
    /// `evaluate_final` so callers reading `ev_first_half_out_dgb` after this method
    /// see the same D_gb output as after the P1 path.
    ///
    /// D_ev encoding rule (ev side): the eval HOLDS `delta_ev`. Its public-bit
    /// encoding of `correlated_dev[idx]` under `delta_ev` is:
    ///
    ///   `if bit() then delta_ev ^ key else key`
    ///
    /// This mirrors the P1 garbler-side encoding (`if bit() then delta_gb ^ key
    /// else key`) under the opposite delta — by symmetry of `gen_auth_bit` the
    /// `correlated_dev.key` on the ev side is the local key view that
    /// pairs with `correlated_dev.mac` on the gb side under `delta_ev`.
    ///
    /// Returns the eval's D_ev output share vector;
    /// struct does not gain new persistent fields beyond `ev_first_half_out_dev` /
    /// `ev_second_half_out_dev` (private accumulators).
    pub fn evaluate_final_p2(&mut self) -> Vec<Block> {
        assert!(
            !self.final_computed,
            "evaluate_final_p2 called twice on the same instance — \
             ev_first_half_out_dgb would be double-XOR'd; create a new instance per gate"
        );
        // D_gb path: identical to existing `evaluate_final` — per `6_total.tex` §168,
        // `[c D_gb]^ev := Z_{c,0}^ev ⊕ (Z_{c,1}^ev)^T ⊕ [(λ_a ⊗ λ_b) D_gb]^ev`,
        // where the third term is eval's preprocessing share `correlated_dgb[idx]`.
        assert_eq!(self.correlated_dgb.len(), self.n * self.m,
            "evaluate_final_p2: correlated_dgb not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.ev_first_half_out_dgb[(i, j)] ^=
                    self.ev_second_half_out_dgb[(j, i)] ^
                    self.correlated_dgb[j * self.n + i];
            }
        }

        // D_ev path: precomputed label, read directly.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_d_ev = self.correlated_dev[j * self.n + i];
                self.ev_first_half_out_dev[(i, j)] ^=
                    self.ev_second_half_out_dev[(j, i)] ^
                    correlated_share_d_ev;
            }
        }

        // Collect D_ev output (column-major).
        let mut d_ev_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                d_ev_out.push(self.ev_first_half_out_dev[(i, j)]);
            }
        }

        self.final_computed = true;
        d_ev_out
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;

    /// Build a paired (gb, ev) and run the input-encoding phase for
    /// x = y = 0 with a fixed seed so unit tests stay deterministic.
    /// `garble_*_half` and `evaluate_*_half`
    /// require `encode_inputs` to have been called first.
    /// Tests needing non-zero inputs should call the builders directly.
    fn build_pair(n: usize, m: usize) -> (AuthTensorGen, AuthTensorEval) {
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        let mut rng = ChaCha12Rng::seed_from_u64(0xDEAD_BEEF);
        crate::input_encoding::encode_inputs(&mut gb, &mut ev, 0, 0, &mut rng);
        (gb, ev)
    }

    #[test]
    fn test_evaluate_final_p2_returns_d_ev_share_vec() {
        // evaluate_final_p2 returns Vec<Block> of length n*m (D_ev output share).
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