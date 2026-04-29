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
    tensor_ops::{
        gen_populate_seeds_mem_optimized,
        gen_unary_outer_product,
        gen_unary_outer_product_wide,
    },
};
use rand::{CryptoRng, Rng};

pub struct AuthTensorGen {
    cipher: &'static FixedKeyAes,
    chunking_factor: usize,

    n: usize,
    m: usize,

    pub delta_a: Delta,

    /// Block-form sharings under δ_a (`_gen`) and δ_b (`_eval`); see
    /// `TensorFpreGen` field doc for semantics. The Online layer operates
    /// purely on these XOR-share Blocks (paper `5_online.tex` §155–180,
    /// `6_total.tex` §136–180), no MAC/Key recovery required.
    pub alpha_eval: Vec<Block>,
    pub alpha_gen:  Vec<Block>,
    pub beta_eval: Vec<Block>,
    pub beta_gen:  Vec<Block>,
    pub correlated_eval: Vec<Block>,
    pub correlated_gen:  Vec<Block>,
    pub gamma_eval: Vec<Block>,
    pub gamma_gen:  Vec<Block>,

    /// Gen's half of `[x D_a]^gb` — the gen-side IT-MAC wire-label sharing
    /// of input `x` under δ_a. Length n. Populated by `encode_inputs`.
    pub x_gen: Vec<Block>,
    /// Gen's half of `[y D_a]^gb`. Length m.
    pub y_gen: Vec<Block>,
    /// Gen's half of `[(x ⊕ α) D_a]^gb` — the masked-input wire-label
    /// sharing under δ_a, gen side. Length n. Populated by `encode_inputs`.
    /// Used as the GGM-tree seed input by `garble_first_half`.
    pub masked_x_gen: Vec<Block>,
    /// Gen's half of (sharing of y XOR β under δ_a). Length m.
    pub masked_y_gen: Vec<Block>,
    /// Gen's component of the cleartext masked-bit sharing for `d_x`. The
    /// 0-vec by convention -- gen covers both GGM-tree branches; eval owns
    /// the d-vector for traversal choice. Populated by input encoding.
    pub masked_x_bits: Vec<bool>,
    /// Gen's component of the cleartext masked-bit sharing for `d_y`. 0-vec.
    pub masked_y_bits: Vec<bool>,

    pub first_half_out: BlockMatrix,
    pub second_half_out: BlockMatrix,

    /// D_ev (rho-half) accumulator for the first half-outer-product. Phase 9 P2-02.
    /// Mirrors `first_half_out` but accumulates the rho-half PRG outputs from
    /// `gen_unary_outer_product_wide`. Written by `garble_first_half_p2` /
    /// `garble_second_half_p2`; consumed by `garble_final_p2`.
    pub first_half_out_ev: BlockMatrix,
    /// D_ev (rho-half) accumulator for the second half-outer-product. Phase 9 P2-02.
    pub second_half_out_ev: BlockMatrix,

    /// Set to `true` by `garble_final()`. `compute_lambda_gamma()` asserts
    /// this flag to prevent silent garbage output when called out of order.
    final_computed: bool,
}

impl AuthTensorGen {
    pub fn new(n: usize, m: usize, chunking_factor: usize) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            n,
            m,
            chunking_factor,
            delta_a: Delta::random(&mut rand::rng()),
            alpha_eval: Vec::new(),
            alpha_gen: Vec::new(),
            beta_eval: Vec::new(),
            beta_gen: Vec::new(),
            correlated_eval: Vec::new(),
            correlated_gen: Vec::new(),
            gamma_eval: Vec::new(),
            gamma_gen: Vec::new(),
            x_gen: Vec::new(),
            y_gen: Vec::new(),
            masked_x_gen: Vec::new(),
            masked_y_gen: Vec::new(),
            masked_x_bits: Vec::new(),
            masked_y_bits: Vec::new(),
            first_half_out: BlockMatrix::new(n, m),
            second_half_out: BlockMatrix::new(m, n),
            first_half_out_ev: BlockMatrix::new(n, m),
            second_half_out_ev: BlockMatrix::new(m, n),
            final_computed: false,
        }
    }

    pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
        Self {
            cipher: &(*FIXED_KEY_AES),
            n: fpre_gen.n,
            m: fpre_gen.m,
            chunking_factor: fpre_gen.chunking_factor,
            delta_a: fpre_gen.delta_a,
            alpha_eval: fpre_gen.alpha_eval,
            alpha_gen: fpre_gen.alpha_gen,
            beta_eval: fpre_gen.beta_eval,
            beta_gen: fpre_gen.beta_gen,
            correlated_eval: fpre_gen.correlated_eval,
            correlated_gen: fpre_gen.correlated_gen,
            gamma_eval: fpre_gen.gamma_eval,
            gamma_gen: fpre_gen.gamma_gen,
            x_gen: Vec::new(),
            y_gen: Vec::new(),
            masked_x_gen: Vec::new(),
            masked_y_gen: Vec::new(),
            masked_x_bits: Vec::new(),
            masked_y_bits: Vec::new(),
            first_half_out: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
            first_half_out_ev: BlockMatrix::new(fpre_gen.n, fpre_gen.m),
            second_half_out_ev: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
            final_computed: false,
        }
    }

    pub(crate) fn gen_chunked_half_outer_product(
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

    /// Wide-leaf variant of `gen_chunked_half_outer_product`. Phase 9 P2-02.
    /// Writes BOTH the D_gb output (`first_half_out` or `second_half_out`) AND the
    /// D_ev output (`first_half_out_ev` or `second_half_out_ev`) in a single pass
    /// over the GGM tree. Returns wide chunk_cts of type `Vec<Vec<(Block, Block)>>`.
    pub(crate) fn gen_chunked_half_outer_product_wide(
        &mut self,
        x: &MatrixViewRef<Block>,
        y_d_gb: &MatrixViewRef<Block>,
        y_d_ev: &MatrixViewRef<Block>,
        first_half: bool,
    ) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>) {
        let mut chunk_levels: Vec<Vec<(Block, Block)>> = Vec::new();
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
            let delta = self.delta_a;
            let chunking_factor = self.chunking_factor;

            // Borrow both D_gb and D_ev output halves disjointly. We split the
            // ownership manually to obtain two simultaneous &mut BlockMatrix
            // borrows on different struct fields.
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
    /// Under the auth-bit-style construction (BUG-02 / Phase 1.2):
    /// - `x[i] = masked_x_gen[i]` — gen's half of the wire-label sharing
    ///   `(x XOR α) · δ_a` (= input-mac XOR (key_g_α XOR a · δ_a)).
    /// - `y[i] = y_gen[i]` — gen's half of the input sharing of y under
    ///   δ_a (= input-mac for y). The β-share cancellation is implicit:
    ///   `masked_y_gen XOR (key_g_β XOR a_β · δ_a) = y_gen`.
    ///
    /// MUST be called after `prepare_input_labels` has populated
    /// `masked_x_gen` and `y_gen`.
    pub fn get_first_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        assert_eq!(self.masked_x_gen.len(), self.n,
            "get_first_inputs: masked_x_gen not populated; call prepare_input_labels first");
        assert_eq!(self.y_gen.len(), self.m,
            "get_first_inputs: y_gen not populated; call prepare_input_labels first");

        let mut x = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            x[i] = self.masked_x_gen[i];
        }

        let mut y = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            y[j] = self.y_gen[j];
        }

        (x, y)
    }

    /// returns: the garbler's x and y inputs to the second tensor half gate.
    ///
    /// Paper-aligned (`5_online.tex` §155–158, `6_total.tex` §136–141):
    /// the second half is `tensorgb(m, n, D_gb, [(b ⊕ λ_b) D_gb], [a D_gb])`.
    /// In codebase naming with `a = x` and `b = y`:
    /// - `x[j] = masked_y_gen[j]` — gen's share `[(y ⊕ β) D_a]^gb` from input encoding.
    /// - `y[i] = alpha_gen[i]`    — gen's share `[α D_a]^gb` from preprocessing.
    ///
    /// MUST be called after `encode_inputs` has populated `masked_y_gen`.
    pub fn get_second_inputs(&self) -> (BlockMatrix, BlockMatrix) {
        assert_eq!(self.masked_y_gen.len(), self.m,
            "get_second_inputs: masked_y_gen not populated; call encode_inputs first");
        assert_eq!(self.alpha_gen.len(), self.n,
            "get_second_inputs: alpha_gen not populated by preprocessing");

        let mut x = BlockMatrix::new(self.m, 1);
        for j in 0..self.m {
            x[j] = self.masked_y_gen[j];
        }

        let mut y = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y[i] = self.alpha_gen[i];
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
    /// share to produce the garbled tensor gate output. Per `5_online.tex` §160:
    /// `[c D_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) D_gb]`. The third term
    /// is gen's preprocessing share `correlated_gen[idx]`.
    pub fn garble_final(&mut self) {
        assert!(
            !self.final_computed,
            "garble_final called twice on the same instance — \
             first_half_out would be double-XOR'd; create a new instance per gate"
        );
        assert_eq!(self.correlated_gen.len(), self.n * self.m,
            "garble_final: correlated_gen not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^=
                    self.second_half_out[(j, i)] ^
                    self.correlated_gen[j * self.n + i];
            }
        }
        self.final_computed = true;
    }

    /// Phase 9 P2-02 — y inputs (D_ev half) for `garble_first_half_p2`.
    ///
    /// The garbler does NOT hold `delta_b`. Per the IT-MAC layout in
    /// `auth_tensor_fpre.rs::gen_auth_bit` (lines 66-86), the garbler's
    /// `beta_eval[i].mac` is built as `a_share.mac = key_a.auth(a, delta_b)
    /// = key_a XOR a*delta_b`, where `key_a` is held by the evaluator. Emitting
    /// `mac.as_block()` directly is the correct public-bit encoding under
    /// `delta_b` for the garbler's contribution — no XOR needed. The eval side
    /// XORs in its key view to recover the IT-MAC pair under `delta_b`.
    fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_ev[i] = self.beta_eval[i];
        }
        y_ev
    }

    fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y_ev[i] = self.alpha_eval[i];
        }
        y_ev
    }

    /// Phase 9 P2-02. Drives the wide GGM tree expansion for the first
    /// half-outer-product. Returns `(chunk_levels, chunk_cts_wide)` where
    /// `chunk_cts_wide: Vec<Vec<(Block, Block)>>` carries the kappa-half AND
    /// rho-half ciphertexts. Writes BOTH `first_half_out` (D_gb) and
    /// `first_half_out_ev` (D_ev) in a single pass.
    pub fn garble_first_half_p2(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>) {
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
    pub fn garble_second_half_p2(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>) {
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
    ///   the value `garble_final` writes into `first_half_out[(i, j)]`.
    /// - `d_ev_out[j*n + i]` = `[v_gamma D_ev]^gb` for gate (i, j); the new D_ev
    ///   path.
    ///
    /// CRITICAL — Protocol-2 garbler privacy (CONTEXT.md D-10): this method
    /// NEVER returns a masked wire value (no `bool` / no `Vec<bool>`). The
    /// garbler retains both shares privately. The static return type
    /// `(Vec<Block>, Vec<Block>)` enforces the privacy property at compile
    /// time.
    ///
    /// D_ev encoding rule (garbler side): the garbler does NOT hold `delta_b`,
    /// so its public-bit encoding of `correlated_eval[idx]` is simply
    /// `mac.as_block()` — no `delta_b` XOR. See
    /// `get_first_inputs_p2_y_d_ev` doc for derivation.
    pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>) {
        assert!(
            !self.final_computed,
            "garble_final_p2 called twice on the same instance — \
             first_half_out would be double-XOR'd; create a new instance per gate"
        );
        // D_gb path: identical to existing `garble_final` — per `6_total.tex` §140,
        // `[c D_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) D_gb]`, where the third
        // term is gen's preprocessing share `correlated_gen[idx]`.
        assert_eq!(self.correlated_gen.len(), self.n * self.m,
            "garble_final_p2: correlated_gen not populated by preprocessing");
        for i in 0..self.n {
            for j in 0..self.m {
                self.first_half_out[(i, j)] ^=
                    self.second_half_out[(j, i)] ^
                    self.correlated_gen[j * self.n + i];
            }
        }

        // D_ev path: mirror of D_gb but using `correlated_eval`. The
        // garbler emits `mac.as_block()` directly (no `delta_b` XOR — gb does
        // not hold `delta_b`). The eval-side mirror in `evaluate_final_p2`
        // applies its own `delta_b` to the eval-side `key` view to reconstruct
        // the IT-MAC pair under `delta_b`.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_ev = self.correlated_eval[j * self.n + i];
                self.first_half_out_ev[(i, j)] ^=
                    self.second_half_out_ev[(j, i)] ^
                    correlated_share_ev;
            }
        }

        // Collect output vecs in column-major order (j*n + i).
        let mut d_gb_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        let mut d_ev_out: Vec<Block> = Vec::with_capacity(self.n * self.m);
        for j in 0..self.m {
            for i in 0..self.n {
                d_gb_out.push(self.first_half_out[(i, j)]);
                d_ev_out.push(self.first_half_out_ev[(i, j)]);
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
    /// `gar.garble_*_half` and `ev.evaluate_*_half` can run. Per BUG-02 /
    /// Phase 1.2, input wire labels are no longer populated by
    /// preprocessing — tests must call this between `new_from_fpre_*`
    /// and the first `garble_*` / `evaluate_*` call.
    fn install_test_input_labels(
        gar: &mut AuthTensorGen,
        ev: &mut AuthTensorEval,
        x: usize,
        y: usize,
    ) {
        let mut rng = ChaCha12Rng::seed_from_u64(0xDEAD_BEEF);
        crate::input_encoding::encode_inputs(gar, ev, x, y, &mut rng);
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

        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Block-form sharings populated from preprocessing.
        assert_eq!(gar.alpha_eval.len(), n);
        assert_eq!(gar.alpha_gen.len(),  n);
        assert_eq!(gar.beta_eval.len(),  m);
        assert_eq!(gar.beta_gen.len(),   m);
        assert_eq!(gar.correlated_eval.len(), n * m);
        assert_eq!(gar.correlated_gen.len(),  n * m);

        install_test_input_labels(&mut gar, &mut ev, 0b1101, 0b110);

        assert_eq!(gar.masked_x_gen.len(), n);
        assert_eq!(gar.masked_y_gen.len(), m);

        let (_chunk_levels, _chunk_cts) = gar.garble_first_half();
    }

    #[test]
    fn test_garble_final_p2_returns_two_block_vecs_no_lambda() {
        // P2-02: garble_final_p2 return type contains NO masked wire value (no Vec<bool>).
        // Statically the type is (Vec<Block>, Vec<Block>) — there is no bool field.
        let n = 4;
        let m = 3;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        install_test_input_labels(&mut gar, &mut ev, 0, 0);

        let (_cl1, _ct1) = gar.garble_first_half_p2();
        let (_cl2, _ct2) = gar.garble_second_half_p2();
        let (d_gb, d_ev) = gar.garble_final_p2();

        assert_eq!(d_gb.len(), n * m, "D_gb output share has length n*m");
        assert_eq!(d_ev.len(), n * m, "D_ev output share has length n*m");
    }

    #[test]
    fn test_garble_first_half_p2_returns_wide_ciphertexts() {
        // P2-01/P2-02: garble_first_half_p2 returns wide chunk_cts of type Vec<Vec<(Block, Block)>>.
        let n = 4;
        let m = 3;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        install_test_input_labels(&mut gar, &mut ev, 0, 0);

        let (_chunk_levels, chunk_cts) = gar.garble_first_half_p2();
        // Each ciphertext entry is (Block, Block) — verifies wide type at compile time.
        for chunk in &chunk_cts {
            for (kappa, rho) in chunk {
                let _: &Block = kappa;
                let _: &Block = rho;
            }
        }
        assert!(!chunk_cts.is_empty(), "chunk_cts must be non-empty");
    }
}