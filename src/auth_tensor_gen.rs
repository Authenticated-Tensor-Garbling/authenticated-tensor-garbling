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

    /// D_ev-authenticated shares of `l_alpha`; length n. Phase 9 P2-01.
    pub alpha_d_ev_shares: Vec<AuthBitShare>,
    /// D_ev-authenticated shares of `l_beta`; length m. Phase 9 P2-01.
    pub beta_d_ev_shares: Vec<AuthBitShare>,
    /// D_ev-authenticated shares of `l_gamma*` (correlated bit); length n*m, column-major.
    /// Phase 9 P2-01.
    pub correlated_d_ev_shares: Vec<AuthBitShare>,
    /// D_ev-authenticated shares of `l_gamma`; length n*m, column-major. (Phase 9 D-05.)
    pub gamma_d_ev_shares: Vec<AuthBitShare>,

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
            alpha_d_ev_shares: fpre_gen.alpha_d_ev_shares,
            beta_d_ev_shares: fpre_gen.beta_d_ev_shares,
            correlated_d_ev_shares: fpre_gen.correlated_d_ev_shares,
            gamma_d_ev_shares: fpre_gen.gamma_d_ev_shares,
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
        assert!(
            !self.final_computed,
            "garble_final called twice on the same instance — \
             first_half_out would be double-XOR'd; create a new instance per gate"
        );
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
        self.final_computed = true;
    }

    /// Phase 9 P2-02 — y inputs (D_ev half) for `garble_first_half_p2`.
    ///
    /// The garbler does NOT hold `delta_b`. Per the IT-MAC layout in
    /// `auth_tensor_fpre.rs::gen_auth_bit` (lines 66-86), the garbler's
    /// `beta_d_ev_shares[i].mac` is built as `a_share.mac = key_a.auth(a, delta_b)
    /// = key_a XOR a*delta_b`, where `key_a` is held by the evaluator. Emitting
    /// `mac.as_block()` directly is the correct public-bit encoding under
    /// `delta_b` for the garbler's contribution — no XOR needed. The eval side
    /// XORs in its key view to recover the IT-MAC pair under `delta_b`.
    fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.m, 1);
        for i in 0..self.m {
            y_ev[i] = *self.beta_d_ev_shares[i].mac.as_block();
        }
        y_ev
    }

    /// Phase 9 P2-02 — y inputs (D_ev half) for `garble_second_half_p2`.
    /// Symmetric to `get_first_inputs_p2_y_d_ev`, using `alpha_d_ev_shares`.
    fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix {
        let mut y_ev = BlockMatrix::new(self.n, 1);
        for i in 0..self.n {
            y_ev[i] = *self.alpha_d_ev_shares[i].mac.as_block();
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
    /// so its public-bit encoding of `correlated_d_ev_shares[idx]` is simply
    /// `mac.as_block()` — no `delta_b` XOR. See
    /// `get_first_inputs_p2_y_d_ev` doc for derivation.
    pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>) {
        assert!(
            !self.final_computed,
            "garble_final_p2 called twice on the same instance — \
             first_half_out would be double-XOR'd; create a new instance per gate"
        );
        // D_gb path: identical to existing `garble_final`.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_gb = if self.correlated_auth_bit_shares[j * self.n + i].bit() {
                    self.delta_a.as_block() ^ self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
                } else {
                    *self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
                };
                self.first_half_out[(i, j)] ^=
                    self.second_half_out[(j, i)] ^
                    correlated_share_gb;
            }
        }

        // D_ev path: mirror of D_gb but using `correlated_d_ev_shares`. The
        // garbler emits `mac.as_block()` directly (no `delta_b` XOR — gb does
        // not hold `delta_b`). The eval-side mirror in `evaluate_final_p2`
        // applies its own `delta_b` to the eval-side `key` view to reconstruct
        // the IT-MAC pair under `delta_b`.
        for i in 0..self.n {
            for j in 0..self.m {
                let correlated_share_ev = *self.correlated_d_ev_shares[j * self.n + i].mac.as_block();
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

    /// Computes the garbler's masked output share `[L_gamma]^gb` per (i,j).
    ///
    /// MUST be called AFTER `garble_final()` — `first_half_out` only holds
    /// `[v_gamma D_gb]^gb` once `garble_final` has XORed in the correlated share.
    /// Calling earlier returns garbage.
    ///
    /// Per CONTEXT.md D-04 (paper 5_online.tex line 132):
    ///   `[L_gamma]^gb[j*n+i] = first_half_out[(i,j)].lsb()
    ///                          XOR gamma_d_ev_shares[j*n+i].bit()`
    ///
    /// Output is column-major: `vec[j * self.n + i]` corresponds to gate output (i, j).
    ///
    /// Note on D_gb vs D_ev: the paper writes `extbit([l_gamma D_gb])` but the Phase 7
    /// `gamma_d_ev_shares` field stores D_ev-authenticated shares. This is correct:
    /// `AuthBitShare::bit()` returns `self.value`, which is the per-party local share
    /// of the bit — independent of which delta authenticated the share. See
    /// 08-RESEARCH.md Pitfall 1 for the full justification.
    ///
    /// # Panics
    /// Panics if `gamma_d_ev_shares.len() != self.n * self.m`. The
    /// `UncompressedPreprocessingBackend` deliberately leaves this vec empty
    /// (Phase 7 stub); use `IdealPreprocessingBackend` for any caller invoking
    /// `compute_lambda_gamma`.
    pub fn compute_lambda_gamma(&self) -> Vec<bool> {
        assert!(
            self.final_computed,
            "compute_lambda_gamma called before garble_final — \
             first_half_out is not yet the combined v_gamma encoding"
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
                let v_extbit  = self.first_half_out[(i, j)].lsb();
                let lg_extbit = self.gamma_d_ev_shares[j * self.n + i].bit();
                out.push(v_extbit ^ lg_extbit);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_fpre::TensorFpre;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};

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

    #[test]
    fn test_compute_lambda_gamma_dimensions() {
        let n = 4;
        let m = 3;
        let (fpre_gen, _fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);

        assert_eq!(gar.gamma_d_ev_shares.len(), n * m,
            "gamma_d_ev_shares must be length n*m after new_from_fpre_gen");

        let (gen_chunk_levels_1, gen_chunk_cts_1) = gar.garble_first_half();
        let _ = (gen_chunk_levels_1, gen_chunk_cts_1);
        let (gen_chunk_levels_2, gen_chunk_cts_2) = gar.garble_second_half();
        let _ = (gen_chunk_levels_2, gen_chunk_cts_2);
        gar.garble_final();

        let lambda = gar.compute_lambda_gamma();
        assert_eq!(lambda.len(), n * m,
            "compute_lambda_gamma must return Vec<bool> of length n*m");
    }

    #[test]
    fn test_compute_lambda_gamma_uses_column_major() {
        let n = 4;
        let m = 3;
        let (fpre_gen, _fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ = gar.garble_first_half();
        let _ = gar.garble_second_half();
        gar.garble_final();
        let lambda = gar.compute_lambda_gamma();

        // Probe one specific (i, j) entry.
        let i = 2;
        let j = 1;
        let idx = j * n + i; // == 6
        let expected = gar.first_half_out[(i, j)].lsb()
                     ^ gar.gamma_d_ev_shares[idx].bit();
        assert_eq!(lambda[idx], expected,
            "lambda[j*n+i] at (i=2, j=1) does not match D-04 formula");
    }

    #[test]
    fn test_compute_lambda_gamma_full_consistency() {
        let n = 4;
        let m = 3;
        let (fpre_gen, _fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ = gar.garble_first_half();
        let _ = gar.garble_second_half();
        gar.garble_final();
        let lambda = gar.compute_lambda_gamma();

        for j in 0..m {
            for i in 0..n {
                let idx = j * n + i;
                let expected = gar.first_half_out[(i, j)].lsb()
                             ^ gar.gamma_d_ev_shares[idx].bit();
                assert_eq!(lambda[idx], expected,
                    "D-04 formula mismatch at (i={}, j={}, idx={})", i, j, idx);
            }
        }
    }

    #[test]
    fn test_garble_final_p2_returns_two_block_vecs_no_lambda() {
        // P2-02: garble_final_p2 return type contains NO masked wire value (no Vec<bool>).
        // Statically the type is (Vec<Block>, Vec<Block>) — there is no bool field.
        let n = 4;
        let m = 3;
        let (fpre_gen, _fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);

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
        let (fpre_gen, _) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);

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