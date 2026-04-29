use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    delta::Delta,
    preprocessing::TensorFpreGen,
    block::Block,
    matrix::{BlockMatrix, MatrixViewRef},
    tensor_ops::{
        gen_populate_seeds_mem_optimized,
        gen_unary_outer_product,
        gen_unary_outer_product_wide,
    },
};

pub struct AuthTensorGen {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,

    n: usize,
    m: usize,

    pub delta_gb: Delta,

    /// Block-form sharings under δ_gb (`_gen`) and δ_ev (`_eval`); see
    /// `TensorFpreGen` field doc for semantics. The Online layer operates
    /// purely on these XOR-share Blocks (paper `5_online.tex` §155–180,
    /// `6_total.tex` §136–180), no MAC/Key recovery required.
    pub alpha_dev: Vec<Block>,
    pub alpha_dgb:  Vec<Block>,
    pub beta_dev: Vec<Block>,
    pub beta_dgb:  Vec<Block>,
    pub correlated_dev: Vec<Block>,
    pub correlated_dgb:  Vec<Block>,
    pub gamma_dev: Vec<Block>,
    pub gamma_dgb:  Vec<Block>,

    /// Gen's half of `[x D_gb]^gb` — the gb-side IT-MAC wire-label sharing
    /// of input `x` under δ_gb. Length n. Populated by `encode_inputs`.
    pub x_dgb: Vec<Block>,
    /// Gen's half of `[y D_gb]^gb`. Length m.
    pub y_dgb: Vec<Block>,
    /// Gen's half of `[(x ⊕ α) D_gb]^gb` — the masked-input wire-label
    /// sharing under δ_gb, gb side. Length n. Populated by `encode_inputs`.
    /// Used as the GGM-tree seed input by `garble_first_half`.
    pub masked_x_dgb: Vec<Block>,
    /// Gen's half of (sharing of y XOR β under δ_gb). Length m.
    pub masked_y_dgb: Vec<Block>,
    /// Gen's component of the cleartext masked-bit sharing for `d_x`. The
    /// 0-vec by convention -- gen covers both GGM-tree branches; eval owns
    /// the d-vector for traversal choice. Populated by input encoding.
    pub gb_masked_x_bits: Vec<bool>,
    /// Gen's component of the cleartext masked-bit sharing for `d_y`. 0-vec.
    pub gb_masked_y_bits: Vec<bool>,

    pub gb_first_half_out_dgb: BlockMatrix,
    pub gb_second_half_out_dgb: BlockMatrix,

    /// D_ev (rho-half) accumulator for the first half-outer-product. Phase 9 P2-02.
    /// Mirrors `gb_first_half_out_dgb` but accumulates the rho-half PRG outputs from
    /// `gen_unary_outer_product_wide`. Written by `garble_first_half_p2` /
    /// `garble_second_half_p2`; consumed by `garble_final_p2`.
    pub gb_first_half_out_dev: BlockMatrix,
    /// D_ev (rho-half) accumulator for the second half-outer-product. Phase 9 P2-02.
    pub gb_second_half_out_dev: BlockMatrix,

    /// Set to `true` by `garble_final()`. `compute_lambda_gamma()` asserts
    /// this flag to prevent silent garbage output when called out of order.
    final_computed: bool,
}

impl AuthTensorGen {
    pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
        // AUDIT-2.3 D7: paper's chunking-size matching invariant requires the
        // GGM-tree tile boundaries used by P1 to match those baked into
        // preprocessing. A zero chunking_factor (no tiles) silently breaks tile
        // alignment downstream and breaks `gen_chunked_half_outer_product`'s
        // chunk-iteration arithmetic. Per-party defense-in-depth check; the
        // cross-party `gb.chunking_factor == ev.chunking_factor` invariant is
        // verified by `verify_chunking_factor_cross_party` at preprocessing exit.
        assert!(
            fpre_gen.chunking_factor > 0,
            "fpre_gen.chunking_factor must be at least 1 (AUDIT-2.3 D7)"
        );
        Self {
            cipher: &(*FIXED_KEY_AES),
            n: fpre_gen.n,
            m: fpre_gen.m,
            chunking_factor: fpre_gen.chunking_factor,
            delta_gb: fpre_gen.delta_gb,
            alpha_dev: fpre_gen.alpha_dev,
            alpha_dgb: fpre_gen.alpha_dgb,
            beta_dev: fpre_gen.beta_dev,
            beta_dgb: fpre_gen.beta_dgb,
            correlated_dev: fpre_gen.correlated_dev,
            correlated_dgb: fpre_gen.correlated_dgb,
            gamma_dev: fpre_gen.gamma_dev,
            gamma_dgb: fpre_gen.gamma_dgb,
            x_dgb: Vec::new(),
            y_dgb: Vec::new(),
            masked_x_dgb: Vec::new(),
            masked_y_dgb: Vec::new(),
            gb_masked_x_bits: Vec::new(),
            gb_masked_y_bits: Vec::new(),
            gb_first_half_out_dgb: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            gb_second_half_out_dgb: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
            gb_first_half_out_dev: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            gb_second_half_out_dev: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
            final_computed: false,
        }
    }

    pub(crate) fn gen_chunked_half_outer_product(
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
            let delta = self.delta_gb;

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

    /// Wide-leaf variant of `gen_chunked_half_outer_product`. Phase 9 P2-02.
    /// Writes BOTH the D_gb output (`gb_first_half_out_dgb` or `gb_second_half_out_dgb`) AND the
    /// D_ev output (`gb_first_half_out_dev` or `gb_second_half_out_dev`) in a single pass
    /// over the GGM tree.
    ///
    /// Returns `(chunk_levels, chunk_cts)` where:
    /// - `chunk_levels: Vec<Vec<Block>>` — paper-faithful single-Block-per-level
    ///   tree ciphertexts (Construction 4 / `5_online.tex:43-72`); leaf-level only.
    /// - `chunk_cts: Vec<Vec<(Block, Block)>>` — wide leaf cts at `(κ, κ)` width.
    ///   Paper-faithful `(κ, ρ)` width via `RhoBlock` is deferred (AUDIT-2.4 D1
    ///   second half); current shape preserves the κ-bit ρ-half.
    pub(crate) fn gen_chunked_half_outer_product_wide(
        &mut self,
        x: &MatrixViewRef<Block>,
        y_d_gb: &MatrixViewRef<Block>,
        y_d_ev: &MatrixViewRef<Block>,
        first_half: bool,
    ) -> (Vec<Vec<Block>>, Vec<Vec<(Block, Block)>>) {
        let mut chunk_levels: Vec<Vec<Block>> = Vec::new();
        let mut chunk_cts: Vec<Vec<(Block, Block)>> = Vec::new();

        for s in 0..((x.rows() + self.chunking_factor - 1) / self.chunking_factor) {
            let slice_size: usize = if self.chunking_factor * (s + 1) > x.rows() {
                x.rows() % self.chunking_factor
            } else {
                self.chunking_factor
            };

            let mut slice = BlockMatrix::new(slice_size, 1);
            for i in 0..slice_size {
                slice[i] = x[i + s * self.chunking_factor];
            }

            let cipher = self.cipher;
            let delta = self.delta_gb;
            let chunking_factor = self.chunking_factor;

            // Borrow both D_gb and D_ev output halves disjointly. We split the
            // ownership manually to obtain two simultaneous &mut BlockMatrix
            // borrows on different struct fields.
            let (out_gb_full, out_ev_full): (
                &mut BlockMatrix,
                &mut BlockMatrix,
            ) = if first_half {
                (&mut self.gb_first_half_out_dgb, &mut self.gb_first_half_out_dev)
            } else {
                (&mut self.gb_second_half_out_dgb, &mut self.gb_second_half_out_dev)
            };

            let mut out_gb = out_gb_full.as_view_mut();
            let mut out_ev = out_ev_full.as_view_mut();

            // Nested with_subrows: each call yields a sub-view scoped to this
            // chunk's row range. The two views are over distinct backing storage
            // (different BlockMatrix fields), so the outer-then-inner closure
            // ordering is purely lexical — no aliasing concern.
            out_gb.with_subrows(chunking_factor * s, slice_size, |part_gb| {
                out_ev.with_subrows(chunking_factor * s, slice_size, |part_ev| {
                    let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(
                        slice.elements_slice(),
                        cipher,
                        delta,
                    );
                    let gen_cts_wide = gen_unary_outer_product_wide(
                        &gen_seeds,
                        y_d_gb,
                        y_d_ev,
                        part_gb,
                        part_ev,
                        cipher,
                    );

                    chunk_levels.push(levels);
                    chunk_cts.push(gen_cts_wide);
                });
            });
        }

        (chunk_levels, chunk_cts)
    }

    /// returns: the garbler's x and y inputs to the first tensor half gate.
    ///
    /// Paper-aligned (`5_online.tex` §157, `6_total.tex` §137):
    /// the first half is `tensorgb(n, m, D_gb, [(a ⊕ λ_a) D_gb], [λ_b D_gb])`.
    /// In codebase naming with `a = x` and `λ_b = β`:
    /// - `x[i] = masked_x_dgb[i]` — gb.s share `[(x ⊕ α) D_gb]^gb` from input encoding.
    /// - `y[j] = beta_dgb[j]`     — gb.s share `[β D_gb]^gb` from preprocessing.
    ///
    /// MUST be called after `encode_inputs` has populated `masked_x_dgb`.
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

    /// returns: the garbler's x and y inputs to the second tensor half gate.
    ///
    /// Paper-aligned (`5_online.tex` §158, `6_total.tex` §138):
    /// the second half is `tensorgb(m, n, D_gb, [(b ⊕ λ_b) D_gb], [a D_gb])`.
    /// In codebase naming with `a = x` and `b = y`:
    /// - `x[j] = masked_y_dgb[j]` — gb.s share `[(y ⊕ β) D_gb]^gb` from input encoding.
    /// - `y[i] = x_dgb[i]`        — gb.s share `[x D_gb]^gb` from input encoding.
    ///
    /// MUST be called after `encode_inputs` has populated `masked_y_dgb` / `x_dgb`.
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

    pub fn garble_first_half(&mut self) -> (Vec<Vec<Block>>, Vec<Vec<Block>>) {
        let (x, y) = self.get_first_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x.as_view(), &y.as_view(), true);

        (chunk_levels, chunk_cts)
    }

    pub fn garble_second_half(&mut self) -> (Vec<Vec<Block>>, Vec<Vec<Block>>) {
        let (x, y) = self.get_second_inputs();
        let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x.as_view(), &y.as_view(), false);

        (chunk_levels, chunk_cts)
    }

    /// Combines both half-outer-product outputs with the correlated preprocessing
    /// share to produce the garbled tensor gate output. Per `5_online.tex` §160:
    /// `[c D_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) D_gb]`. The third term
    /// is gen's preprocessing share `correlated_dgb[idx]`.
    pub fn garble_final(&mut self) {
        assert!(
            !self.final_computed,
            "garble_final called twice on the same instance — \
             gb_first_half_out_dgb would be double-XOR'd; create a new instance per gate"
        );
        assert_eq!(self.correlated_dgb.len(), self.n * self.m,
            "garble_final: correlated_dgb not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.gb_first_half_out_dgb[(i, j)] ^=
                    self.gb_second_half_out_dgb[(j, i)] ^
                    self.correlated_dgb[j * self.n + i];
            }
        }
        self.final_computed = true;
    }

    /// y inputs (D_ev half) for `garble_first_half_p2`.
    ///
    /// Paper-aligned with `get_first_inputs`'s D_gb side: the first-half y
    /// operand is `β` (paper's `λ_b`). This emits gb.s share of `[β D_ev]`
    /// from preprocessing — i.e. `beta_dev[i]`. Combined with the D_gb track
    /// (`beta_dgb[i]` via `get_first_inputs`), the wide GGM expansion produces
    /// `(x ⊕ α) ⊗ β` under both deltas.
    fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_ev[i] = self.beta_dev[i];
        }
        y_ev
    }

    /// y inputs (D_ev half) for `garble_second_half_p2`.
    ///
    /// Paper-aligned with `get_second_inputs`'s D_gb side: the second-half y
    /// operand is `x` (paper's `a`). Per `5_online.tex` §211, gb sets
    /// `[v_a D_ev]^gb := [λ_a D_ev]^gb`, so gb.s share of `[x D_ev]` equals
    /// its share of `[α D_ev]` — i.e. `alpha_dev[i]`. The ev side XORs in
    /// `L_a · D_ev` (see `AuthTensorEval::get_second_inputs_p2_y_d_ev`).
    fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y_ev[i] = self.alpha_dev[i];
        }
        y_ev
    }

    /// Phase 9 P2-02. Drives the wide GGM tree expansion for the first
    /// half-outer-product. Returns `(chunk_levels, chunk_cts_wide)` where:
    /// - `chunk_levels: Vec<Vec<Block>>` is the paper-faithful single-Block-
    ///   per-level tree-cts shape (Construction 4).
    /// - `chunk_cts_wide: Vec<Vec<(Block, Block)>>` carries the kappa-half AND
    ///   rho-half ciphertexts (paper-faithful (κ, ρ) width via `RhoBlock` is
    ///   deferred — see AUDIT-2.4 D1 second half).
    ///
    /// Writes BOTH `gb_first_half_out_dgb` (D_gb) and `gb_first_half_out_dev` (D_ev) in
    /// a single pass.
    pub fn garble_first_half_p2(&mut self) -> (Vec<Vec<Block>>, Vec<Vec<(Block, Block)>>) {
        let (x, y_d_gb) = self.get_first_inputs();
        let y_d_ev = self.get_first_inputs_p2_y_d_ev();
        self.gen_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            true,
        )
    }

    /// Phase 9 P2-02. Drives the wide GGM tree expansion for the second
    /// half-outer-product. Mirrors `garble_first_half_p2` with second-half
    /// inputs.
    pub fn garble_second_half_p2(&mut self) -> (Vec<Vec<Block>>, Vec<Vec<(Block, Block)>>) {
        let (x, y_d_gb) = self.get_second_inputs();
        let y_d_ev = self.get_second_inputs_p2_y_d_ev();
        self.gen_chunked_half_outer_product_wide(
            &x.as_view(),
            &y_d_gb.as_view(),
            &y_d_ev.as_view(),
            false,
        )
    }

    /// Phase 9 P2-02. Combines both halves into the final D_gb and D_ev output
    /// shares.
    ///
    /// Returns `(d_gb_out, d_ev_out)`:
    /// - `d_gb_out[j*n + i]` = `[v_gamma D_gb]^gb` for gate (i, j); identical to
    ///   the value `garble_final` writes into `gb_first_half_out_dgb[(i, j)]`.
    /// - `d_ev_out[j*n + i]` = `[v_gamma D_ev]^gb` for gate (i, j); the new D_ev
    ///   path.
    ///
    /// CRITICAL — Protocol-2 garbler privacy (CONTEXT.md D-10): this method
    /// NEVER returns a masked wire value (no `bool` / no `Vec<bool>`). The
    /// garbler retains both shares privately. The static return type
    /// `(Vec<Block>, Vec<Block>)` enforces the privacy property at compile
    /// time.
    ///
    /// D_ev encoding rule (garbler side): the garbler does NOT hold `delta_ev`,
    /// so its share of `[(λ_a ⊗ λ_b) D_ev]^gb` is the Block-form value
    /// `correlated_dev[idx]` (paper-side `mac` of the auth-bit, lowered to a
    /// raw `Block` by `derive_sharing_blocks` during `run_preprocessing`).
    /// Folded into `gb_first_half_out_dev` directly with no `delta_ev` XOR. The
    /// ev-side mirror in `evaluate_final_p2` adds its own `delta_ev`-bearing
    /// term to reconstruct the IT-MAC pair under `delta_ev`. See
    /// `get_first_inputs_p2_y_d_ev` doc for derivation.
    pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>) {
        assert!(
            !self.final_computed,
            "garble_final_p2 called twice on the same instance — \
             gb_first_half_out_dgb would be double-XOR'd; create a new instance per gate"
        );
        // D_gb path: identical to existing `garble_final` — per `6_total.tex` §140,
        // `[c D_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) D_gb]`, where the third
        // term is gen's preprocessing share `correlated_dgb[idx]`.
        assert_eq!(self.correlated_dgb.len(), self.n * self.m,
            "garble_final_p2: correlated_dgb not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.gb_first_half_out_dgb[(i, j)] ^=
                    self.gb_second_half_out_dgb[(j, i)] ^
                    self.correlated_dgb[j * self.n + i];
            }
        }

        // D_ev path: mirror of D_gb but using `correlated_dev`. The
        // garbler folds `correlated_dev[idx]` (a Block already lowered from the
        // paper-side `mac` by `derive_sharing_blocks` at preprocessing) directly
        // into `gb_first_half_out_dev` — no `delta_ev` XOR, since gb does not hold
        // `delta_ev`. The ev-side mirror in `evaluate_final_p2` applies its
        // own `delta_ev`-bearing term to reconstruct the IT-MAC pair under
        // `delta_ev`.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_ev = self.correlated_dev[j * self.n + i];
                self.gb_first_half_out_dev[(i, j)] ^=
                    self.gb_second_half_out_dev[(j, i)] ^
                    correlated_share_ev;
            }
        }

        // Collect output vecs in column-major order (j*n + i).
        let mut d_gb_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        let mut d_ev_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                d_gb_out.push(self.gb_first_half_out_dgb[(i, j)]);
                d_ev_out.push(self.gb_first_half_out_dev[(i, j)]);
            }
        }

        self.final_computed = true;
        (d_gb_out, d_ev_out)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_fpre::TensorFpre;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};
    use rand::SeedableRng;
    use rand_chacha::ChaCha12Rng;

    /// Test helper: run the input-encoding phase so
    /// `gb.garble_*_half` and `ev.evaluate_*_half` can run. Per BUG-02 /
    /// Phase 1.2, input wire labels are no longer populated by
    /// preprocessing — tests must call this between `new_from_fpre_*`
    /// and the first `garble_*` / `evaluate_*` call.
    fn install_test_input_labels(
        gb: &mut AuthTensorGen,
        ev: &mut AuthTensorEval,
        x: usize,
        y: usize,
    ) {
        let mut rng = ChaCha12Rng::seed_from_u64(0xDEAD_BEEF);
        crate::input_encoding::encode_inputs(gb, ev, x, y, &mut rng);
    }

    #[test]
    fn test_garble_first_half() {
        let n = 4;
        let m = 3;

        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_ideal();

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        assert_eq!(fpre_gen.alpha_auth_bit_shares.len(), n);
        assert_eq!(fpre_gen.beta_auth_bit_shares.len(), m);

        assert_eq!(fpre_gen.correlated_auth_bit_shares.len(), n * m);

        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Block-form sharings populated from preprocessing.
        assert_eq!(gb.alpha_dev.len(), n);
        assert_eq!(gb.alpha_dgb.len(),  n);
        assert_eq!(gb.beta_dev.len(),  m);
        assert_eq!(gb.beta_dgb.len(),   m);
        assert_eq!(gb.correlated_dev.len(), n * m);
        assert_eq!(gb.correlated_dgb.len(),  n * m);

        install_test_input_labels(&mut gb, &mut ev, 0b1101, 0b110);

        assert_eq!(gb.masked_x_dgb.len(), n);
        assert_eq!(gb.masked_y_dgb.len(), m);

        let (_chunk_levels, _chunk_cts) = gb.garble_first_half();
    }

    #[test]
    fn test_garble_final_p2_returns_two_block_vecs_no_lambda() {
        // P2-02: garble_final_p2 return type contains NO masked wire value (no Vec<bool>).
        // Statically the type is (Vec<Block>, Vec<Block>) — there is no bool field.
        let n = 4;
        let m = 3;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        install_test_input_labels(&mut gb, &mut ev, 0, 0);

        let (_cl1, _ct1) = gb.garble_first_half_p2();
        let (_cl2, _ct2) = gb.garble_second_half_p2();
        let (d_gb, d_ev) = gb.garble_final_p2();

        assert_eq!(d_gb.len(), n * m, "D_gb output share has length n*m");
        assert_eq!(d_ev.len(), n * m, "D_ev output share has length n*m");
    }

    #[test]
    fn test_garble_first_half_p2_returns_wide_ciphertexts() {
        // P2-01/P2-02: garble_first_half_p2 returns wide chunk_cts of type Vec<Vec<(Block, Block)>>.
        let n = 4;
        let m = 3;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        install_test_input_labels(&mut gb, &mut ev, 0, 0);

        let (_chunk_levels, chunk_cts) = gb.garble_first_half_p2();
        // Each ciphertext entry is (Block, Block) — verifies wide type at compile time.
        for chunk in &chunk_cts {
            for (kappa, rho) in chunk {
                let _: &Block = kappa;
                let _: &Block = rho;
            }
        }
        assert!(!chunk_cts.is_empty(), "chunk_cts must be non-empty");
    }

    /// AUDIT-2.1 D2 / AUDIT-2.3 D3: chunked wrappers must propagate the paper-
    /// faithful single-Block-per-level shape (Construction 4 / 5_online.tex:43-72).
    /// HK21's two-ct-per-level shape would surface as `Vec<Vec<(Block, Block)>>`
    /// here; the static type bound + element count check catches both regressions.
    #[test]
    fn test_chunked_levels_paper_one_hot_shape() {
        let n = 8;  // > chunking_factor to force multiple chunks
        let m = 3;
        let cf = 4;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, cf);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        install_test_input_labels(&mut gb, &mut ev, 0, 0);

        // P1 narrow path.
        let (cl_narrow, _ct_narrow) = gb.garble_first_half();
        assert_eq!(cl_narrow.len(), n / cf, "narrow chunks = ceil(n/cf)");
        for chunk in &cl_narrow {
            // Each chunk is the level-cts for a 2^cf-leaf GGM tree → cf - 1 levels.
            assert_eq!(chunk.len(), cf - 1, "narrow chunk levels = cf - 1");
            // Static type check: element type is `Block` (not `(Block, Block)`).
            for level_ct in chunk {
                let _: &Block = level_ct;
            }
        }

        // P2 wide path.
        let (cl_wide, _ct_wide) = gb.garble_first_half_p2();
        assert_eq!(cl_wide.len(), n / cf, "wide chunks = ceil(n/cf)");
        for chunk in &cl_wide {
            assert_eq!(chunk.len(), cf - 1, "wide chunk levels = cf - 1");
            for level_ct in chunk {
                let _: &Block = level_ct;
            }
        }
    }
}