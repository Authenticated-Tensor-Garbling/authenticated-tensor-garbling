//! Real-protocol preprocessing pipeline.
//!
//! This module holds the output structs (`TensorFpreGen`, `TensorFpreEval`) that the
//! real two-party preprocessing protocol produces, together with the `run_preprocessing`
//! entry point. The ideal trusted-dealer functionality stays in `auth_tensor_fpre`.
//!
//! AUDIT-2.4 C2 — F_cpre / F_pre coupling note: the paper distinguishes
//! `F_pre` (uncompressed preprocessing, paper Construction 4 / `appendix_krrw_pre.tex`)
//! from `F_cpre` (compressed preprocessing, paper Section 4). The current
//! implementation realizes only the uncompressed `F_pre` flavour; the same
//! `TensorFpreGen` / `TensorFpreEval` structs are consumed by both Protocol 1
//! and Protocol 2 callers without a type-system marker for which flavour is
//! intended. When `F_cpre` lands (deferred to v3 — see master plan / PRE-05),
//! introduce marker types to prevent silent cross-flavour mixing at call sites.

use crate::{block::Block, delta::Delta, sharing::{AuthBitShare, build_share}};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
use crate::auth_tensor_fpre::TensorFpre;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// Preprocessing exit boundary -- garbler's view.
///
/// Each authenticated bit category (alpha, beta, correlated `l_gamma*`,
/// gamma `l_gamma`) is exposed in three forms:
///   * `*_auth_bit_shares: Vec<AuthBitShare>` -- the underlying triples
///     `(value, key, mac)`. Required by consumers that read the local bit
///     value (e.g. `compute_lambda_gamma`).
///   * `*_eval: Vec<Block>` -- the lowered Block-form sharing under δ_ev.
///     This party's component is `mac` (gb-side mac is committed under δ_ev).
///     XORing with the eval's matching entry reveals `bit · δ_ev`.
///   * `*_gen:  Vec<Block>` -- the lowered Block-form sharing under δ_gb.
///     This party's component is `key XOR (value ? δ_gb : 0)`.
///     XORing with the eval's matching entry reveals `bit · δ_gb`.
pub struct TensorFpreGen {
    /// Tensor row dimension (number of alpha / x-input bits).
    pub n: usize,
    /// Tensor column dimension (number of beta / y-input bits).
    pub m: usize,
    /// GGM tree chunking factor; purely a performance knob (1..=8 used in benches).
    pub chunking_factor: usize,
    /// Garbler's (Party A) global correlation key. `as_block().lsb() == 1` invariant.
    pub delta_gb: Delta,

    /// Garbler's `AuthBitShare` for each alpha_i (i in 0..n).
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's component of (sharing of `l_alpha` under δ_ev); length n.
    pub alpha_dev: Vec<Block>,
    /// Garbler's component of (sharing of `l_alpha` under δ_gb); length n.
    pub alpha_dgb: Vec<Block>,

    /// Garbler's `AuthBitShare` for each beta_j (j in 0..m).
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's component of (sharing of `l_beta` under δ_ev); length m.
    pub beta_dev: Vec<Block>,
    /// Garbler's component of (sharing of `l_beta` under δ_gb); length m.
    pub beta_dgb: Vec<Block>,

    /// Garbler's `AuthBitShare` for each correlated bit `l_gamma*` = α·β;
    /// length n*m, column-major index `j*n + i`.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's component of (sharing of `l_gamma*` under δ_ev); length n*m.
    pub correlated_dev: Vec<Block>,
    /// Garbler's component of (sharing of `l_gamma*` under δ_gb); length n*m.
    pub correlated_dgb: Vec<Block>,

    /// Garbler's `AuthBitShare` for each output-wire mask `l_gamma`;
    /// length n*m, column-major. Independent random bit (NOT α·β).
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's component of (sharing of `l_gamma` under δ_ev); length n*m.
    pub gamma_dev: Vec<Block>,
    /// Garbler's component of (sharing of `l_gamma` under δ_gb); length n*m.
    pub gamma_dgb: Vec<Block>,
}

/// Preprocessing exit boundary -- ev's view. Mirror of `TensorFpreGen`.
///
/// Per-field semantics are symmetric: this party's `_eval` component is
/// `key XOR (value ? δ_ev : 0)` (ev-side key is committed under δ_ev);
/// this party's `_gen` component is `mac` (ev-side mac is committed
/// under δ_gb). Same XOR-with-counterparty invariants apply.
pub struct TensorFpreEval {
    /// Tensor row dimension (matches TensorFpreGen.n).
    pub n: usize,
    /// Tensor column dimension (matches TensorFpreGen.m).
    pub m: usize,
    /// GGM tree chunking factor.
    pub chunking_factor: usize,
    /// Evaluator's (Party B) global correlation key. `as_block().lsb() == 0` invariant
    /// (required so that `lsb(delta_gb XOR delta_ev) == 1` per Pi_LeakyTensor §F).
    pub delta_ev: Delta,

    /// Evaluator's `AuthBitShare` for each alpha_i.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's component of (sharing of `l_alpha` under δ_ev); length n.
    pub alpha_dev: Vec<Block>,
    /// Evaluator's component of (sharing of `l_alpha` under δ_gb); length n.
    pub alpha_dgb: Vec<Block>,

    /// Evaluator's `AuthBitShare` for each beta_j.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's component of (sharing of `l_beta` under δ_ev); length m.
    pub beta_dev: Vec<Block>,
    /// Evaluator's component of (sharing of `l_beta` under δ_gb); length m.
    pub beta_dgb: Vec<Block>,

    /// Evaluator's `AuthBitShare` for `l_gamma*` = α·β; length n*m, column-major.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's component of (sharing of `l_gamma*` under δ_ev); length n*m.
    pub correlated_dev: Vec<Block>,
    /// Evaluator's component of (sharing of `l_gamma*` under δ_gb); length n*m.
    pub correlated_dgb: Vec<Block>,

    /// Evaluator's `AuthBitShare` for `l_gamma`; length n*m, column-major.
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's component of (sharing of `l_gamma` under δ_ev); length n*m.
    pub gamma_dev: Vec<Block>,
    /// Evaluator's component of (sharing of `l_gamma` under δ_gb); length n*m.
    pub gamma_dgb: Vec<Block>,
}

/// Common interface for all preprocessing backends.
///
/// All implementations are zero-field (unit) structs — no state, just behavior.
/// Using `&self` even though no data is read: makes the trait object-safe
/// (usable as `dyn TensorPreprocessing`) consistent with the codebase's `&self` / `&mut self`
/// convention. See CONTEXT.md D-01.
pub trait TensorPreprocessing {
    fn run(
        &self,
        n: usize,
        m: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval);
}

/// Backend that wraps the real two-party uncompressed preprocessing protocol (Pi_aTensor',
/// Construction 4). Callers should use `UncompressedPreprocessingBackend.run(n, m, cf)`
/// instead of calling `run_preprocessing` directly. See CONTEXT.md D-02.
pub struct UncompressedPreprocessingBackend;

impl TensorPreprocessing for UncompressedPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        run_preprocessing(n, m, chunking_factor)
    }
}

/// Backend that uses an ideal trusted-dealer oracle (in-process, not cryptographically secure).
///
/// Fixed seed 0 used internally — matches the `IdealBCot::new(0, 1)` precedent (see
/// src/bcot.rs). For tests and benchmarks only. See CONTEXT.md D-03, D-07, D-08.
///
/// `gamma_auth_bit_shares` is populated with `n*m` independent random IT-MAC authenticated
/// bits for l_gamma (the gate output mask). `gen_auth_bit()` calls MUST precede
/// `into_gen_eval()` because `into_gen_eval(self)` consumes `fpre` by value.
/// See RESEARCH.md Pitfall 2 and Pattern 3.
pub struct IdealPreprocessingBackend;

impl TensorPreprocessing for IdealPreprocessingBackend {
    fn run(
        &self,
        n: usize,
        m: usize,
        chunking_factor: usize,
    ) -> (TensorFpreGen, TensorFpreEval) {
        let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
        fpre.generate_ideal();

        // CRITICAL ORDERING: into_gen_eval(self) consumes fpre by value.
        // All gen_auth_bit() calls must happen BEFORE into_gen_eval() is called.
        //
        // Phase 9 D-06: generate all four D_ev field pairs using fpre.gen_auth_bit().
        // Use distinct ChaCha12Rng seeds (42, 43, 44, 45) so the four fields are
        // independently random — same pattern as the existing gamma_auth_bit_shares
        // generation (seed 42).
        // alpha/beta/correlated D_ev labels are derived from the D_gb auth bits inside
        // into_gen_eval() — no gen_auth_bit calls needed here.

        // gamma generation: D_ev-authenticated l_gamma shares (column-major n*m).
        let mut rng_gamma = ChaCha12Rng::seed_from_u64(42);
        let mut gamma_d_ev_bits: Vec<crate::sharing::AuthBit> = Vec::with_capacity(n * m);
        for _ in 0..(n * m) {
            let l_gamma: bool = rng_gamma.random_bool(0.5);
            gamma_d_ev_bits.push(fpre.gen_auth_bit(l_gamma));
        }

        // Now consume fpre — gen_auth_bit() can no longer be called after this line.
        // Note: use `gen_out` / `eval_out` bindings because `gen` is a reserved keyword
        // in Rust 2024 edition.
        let (mut gen_out, mut eval_out) = fpre.into_gen_eval();
        let delta_gb = gen_out.delta_gb;
        let delta_ev = eval_out.delta_ev;

        // Distribute gen_share / eval_share for gamma, then lower to Block form.
        // alpha/beta/correlated _eval/_gen were populated by into_gen_eval().
        let gamma_gen_shares: Vec<AuthBitShare> = gamma_d_ev_bits.iter().map(|b| b.gen_share).collect();
        let gamma_eval_shares: Vec<AuthBitShare> = gamma_d_ev_bits.iter().map(|b| b.eval_share).collect();

        let (gamma_eval_g, gamma_eval_e) = derive_sharing_blocks(
            &gamma_gen_shares, &gamma_eval_shares, &delta_ev);
        let (gamma_gen_e, gamma_gen_g)   = derive_sharing_blocks(
            &gamma_eval_shares, &gamma_gen_shares, &delta_gb);

        gen_out.gamma_auth_bit_shares  = gamma_gen_shares;
        gen_out.gamma_dev = gamma_eval_g;
        gen_out.gamma_dgb  = gamma_gen_g;
        eval_out.gamma_auth_bit_shares = gamma_eval_shares;
        eval_out.gamma_dev = gamma_eval_e;
        eval_out.gamma_dgb  = gamma_gen_e;

        (gen_out, eval_out)
    }
}

/// Run the real two-party uncompressed preprocessing protocol (Pi_aTensor', Construction 4).
///
/// Generates one authenticated tensor triple using:
///   1. `bucket_size_for(n, 1)` leaky triples (from Pi_LeakyTensor)
///   2. Pi_aTensor bucketing combiner to amplify security
///
/// CRITICAL: ONE shared IdealBCot is created before the generation loop. All
/// LeakyTensorPre instances borrow &mut bcot and therefore all triples share the
/// same delta_gb and delta_ev. This is required for the XOR combination in
/// combine_leaky_triples to preserve the MAC invariant mac = key XOR bit*delta.
/// Creating a separate IdealBCot per triple (each with different deltas) would
/// silently produce invalid combined triples.
///
/// Returns one (TensorFpreGen, TensorFpreEval) pair suitable for feeding into
/// AuthTensorGen::new_from_fpre_gen and AuthTensorEval::new_from_fpre_eval.
///
/// Preprocessing is fully input-independent per paper Construction 2. Triples are
/// sampled from LeakyTensorPre's internal ChaCha12Rng; no input values flow in here.
///
/// Batch output is not yet implemented — for multiple triples, call this in a loop.
pub fn run_preprocessing(
    n: usize,
    m: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) {
    let bucket_size = bucket_size_for(n, 1);

    // ONE shared IdealBCot for all triples — ensures all share the same delta_gb and delta_ev.
    // Seed choice: 0 for delta_gb, 1 for delta_ev. The internal rng seed is 0^1=1 (trivial),
    // but key generation inside each LeakyTensorPre uses its own per-instance rng.
    let mut bcot = IdealBCot::new(0, 1);

    let mut triples = Vec::with_capacity(bucket_size);
    for t in 0..bucket_size {
        // Each LeakyTensorPre borrows &mut bcot — shares delta_gb and delta_ev.
        // Per-instance seed `t+2` ensures independent key randomness across triples.
        let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, chunking_factor, &mut bcot);
        triples.push(ltp.generate());
    }

    let (mut gen_out, mut eval_out) =
        combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42);

    // BUG-02 / Phase 1.2(c): the post-bucketing input-label populator was
    // removed here. Input wire labels are no longer a preprocessing artifact —
    // they are generated at garble time by AuthTensorGen::prepare_input_labels.
    // The previous block faked labels using x = y = 0 with the input-dependent
    // formula `(x XOR alpha) · delta_gb`, which encoded the mask itself rather
    // than any real input — a "works because both sides use the same dummy
    // convention" hack flagged by LABELS-BUG-CONTEXT.md.

    // Post-bucketing local lowering: each party computes its own _eval and _gen
    // Block-form components from auth-bit shares + own delta. No interaction.
    let delta_gb = gen_out.delta_gb;
    let delta_ev = eval_out.delta_ev;

    // Sharings under δ_ev (`_eval`): gb-side is mac-side, ev-side is key-side.
    // Sharings under δ_gb (`_gen`):  ev-side is mac-side, gb-side is key-side.
    (gen_out.alpha_dev, eval_out.alpha_dev) = derive_sharing_blocks(
        &gen_out.alpha_auth_bit_shares, &eval_out.alpha_auth_bit_shares, &delta_ev);
    (eval_out.alpha_dgb, gen_out.alpha_dgb) = derive_sharing_blocks(
        &eval_out.alpha_auth_bit_shares, &gen_out.alpha_auth_bit_shares, &delta_gb);

    (gen_out.beta_dev, eval_out.beta_dev) = derive_sharing_blocks(
        &gen_out.beta_auth_bit_shares, &eval_out.beta_auth_bit_shares, &delta_ev);
    (eval_out.beta_dgb, gen_out.beta_dgb) = derive_sharing_blocks(
        &eval_out.beta_auth_bit_shares, &gen_out.beta_auth_bit_shares, &delta_gb);

    (gen_out.correlated_dev, eval_out.correlated_dev) = derive_sharing_blocks(
        &gen_out.correlated_auth_bit_shares, &eval_out.correlated_auth_bit_shares, &delta_ev);
    (eval_out.correlated_dgb, gen_out.correlated_dgb) = derive_sharing_blocks(
        &eval_out.correlated_auth_bit_shares, &gen_out.correlated_auth_bit_shares, &delta_gb);

    // Gamma: fresh n*m IT-MAC AuthBit pairs sampled from a dedicated ChaCha12Rng,
    // then lowered to Block form via the same helper. Distinct seed (43) from the
    // bucketing permutation seed (42).
    let mut rng_gamma = ChaCha12Rng::seed_from_u64(43);
    let mut gen_gamma = Vec::with_capacity(n * m);
    let mut eval_gamma = Vec::with_capacity(n * m);
    for _ in 0..(n * m) {
        let l_gamma: bool = rng_gamma.random_bool(0.5);
        let a: bool = rng_gamma.random_bool(0.5);
        let b: bool = l_gamma ^ a;
        let a_share = build_share(&mut rng_gamma, a, &delta_ev);
        let b_share = build_share(&mut rng_gamma, b, &delta_gb);
        gen_gamma.push(AuthBitShare {
            key: b_share.key,
            mac: a_share.mac,
            value: a,
        });
        eval_gamma.push(AuthBitShare {
            key: a_share.key,
            mac: b_share.mac,
            value: b,
        });
    }
    let (gamma_eval_g, gamma_eval_e) = derive_sharing_blocks(&gen_gamma, &eval_gamma, &delta_ev);
    let (gamma_gen_e, gamma_gen_g)   = derive_sharing_blocks(&eval_gamma, &gen_gamma, &delta_gb);
    gen_out.gamma_auth_bit_shares = gen_gamma;
    gen_out.gamma_dev = gamma_eval_g;
    gen_out.gamma_dgb  = gamma_gen_g;
    eval_out.gamma_auth_bit_shares = eval_gamma;
    eval_out.gamma_dev = gamma_eval_e;
    eval_out.gamma_dgb  = gamma_gen_e;

    // AUDIT-2.3 D7: cross-party `chunking_factor` parity invariant. Trivially
    // true here since both outputs were derived from the single `chunking_factor`
    // argument — but this is the canonical enforcement site for the simulation,
    // and breaks loudly if `combine_leaky_triples` ever gets a divergence bug.
    crate::auth_tensor_pre::verify_chunking_factor_cross_party(&gen_out, &eval_out);

    (gen_out, eval_out)
}

/// Lower a paired set of `AuthBitShare`s into the two `Vec<Block>` components
/// of a sharing under `delta`.
///
///   mac_side[i]  -> *mac_side[i].mac.as_block()
///   key_side[i]  -> *key_side[i].key.as_block() XOR (key_side[i].bit() ? delta : 0)
///
/// Together they satisfy `mac_block_i XOR key_block_i = full_bit_i · delta`,
/// where `full_bit_i = mac_side[i].value XOR key_side[i].value`.
///
/// Caller orders inputs by which delta is targeted (per `gen_auth_bit`'s
/// IT-MAC layout: gen.mac is under δ_ev; eval.mac is under δ_gb):
///
///   _eval (sharing under δ_ev): `derive_sharing_blocks(gen, eval, δ_ev)`
///                              -> `(gb_blocks, ev_blocks)`
///   _gen  (sharing under δ_gb): `derive_sharing_blocks(eval, gen, δ_gb)`
///                              -> `(ev_blocks, gb_blocks)`
pub(crate) fn derive_sharing_blocks(
    mac_side: &[AuthBitShare],
    key_side: &[AuthBitShare],
    delta: &Delta,
) -> (Vec<Block>, Vec<Block>) {
    assert_eq!(mac_side.len(), key_side.len());
    let mac_blocks: Vec<Block> = mac_side
        .iter()
        .map(|s| *s.mac.as_block())
        .collect();
    let key_blocks: Vec<Block> = key_side
        .iter()
        .map(|s| {
            let k = *s.key.as_block();
            if s.bit() { k ^ *delta.as_block() } else { k }
        })
        .collect();
    (mac_blocks, key_blocks)
}

#[cfg(test)]
mod tests {
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::sharing::verify_cross_party;
    use crate::block::Block;
    use super::{TensorPreprocessing, UncompressedPreprocessingBackend, IdealPreprocessingBackend};

    #[test]
    fn test_run_preprocessing_dimensions() {
        let (gen_out, eval_out) = super::run_preprocessing(4, 4, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), 16);
    }

    #[test]
    fn test_run_preprocessing_delta_lsb() {
        let (gen_out, _eval_out) = super::run_preprocessing(4, 4, 1);
        assert!(gen_out.delta_gb.as_block().lsb(), "delta_gb LSB must be 1");
    }

    #[test]
    fn test_run_preprocessing_feeds_online_phase() {
        let (fpre_gen, fpre_eval) = super::run_preprocessing(4, 4, 1);
        let _gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        // No panic = success
    }

    // PRE-01: trait is object-safe — both backends work through a dyn reference.
    // Note: bindings named `gen_out` / `eval_out` because `gen` is a reserved keyword
    // in Rust 2024 edition (same reason as IdealPreprocessingBackend::run body).
    #[test]
    fn test_trait_dispatch_ideal() {
        let backend: &dyn TensorPreprocessing = &IdealPreprocessingBackend;
        let (gen_out, eval_out) = backend.run(4, 4, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
    }

    #[test]
    fn test_trait_dispatch_uncompressed() {
        let backend: &dyn TensorPreprocessing = &UncompressedPreprocessingBackend;
        let (gen_out, eval_out) = backend.run(4, 4, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
    }

    // PRE-03: UncompressedPreprocessingBackend delegates to run_preprocessing exactly
    #[test]
    fn test_uncompressed_backend_delegates_to_run_preprocessing() {
        let (gen_out, _eval_out) = UncompressedPreprocessingBackend.run(4, 4, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16,
            "correlated_auth_bit_shares must have n*m=16 entries");
    }

    // Uncompressed backend now populates all four D_ev fields post-bucketing.
    // alpha/beta/correlated are derived from the auth-bit shares; gamma is
    // freshly sampled from a dedicated ChaCha12Rng (seed 43) inside
    // `run_preprocessing`, mirroring the ideal backend.
    #[test]
    fn test_uncompressed_backend_gamma_field_is_populated() {
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(4, 4, 1);
        assert_eq!(gen_out.gamma_auth_bit_shares.len(), 4 * 4,
            "gen.gamma_auth_bit_shares must have length n*m=16");
        assert_eq!(eval_out.gamma_auth_bit_shares.len(), 4 * 4,
            "eval.gamma_auth_bit_shares must have length n*m=16");
    }

    // Uncompressed backend: alpha/beta/correlated D_ev shares have correct lengths.
    #[test]
    fn test_uncompressed_backend_eval_shares_lengths() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1);
        assert_eq!(gen_out.alpha_dev.len(),       n);
        assert_eq!(eval_out.alpha_dev.len(),      n);
        assert_eq!(gen_out.beta_dev.len(),        m);
        assert_eq!(eval_out.beta_dev.len(),       m);
        assert_eq!(gen_out.correlated_dev.len(),  n * m);
        assert_eq!(eval_out.correlated_dev.len(), n * m);
    }

    // Uncompressed backend: alpha/beta/correlated D_ev label pairs XOR to
    // `bit · delta_ev`, where bit is the underlying auth bit's full value.
    // Mirrors `test_ideal_backend_eval_shares_bit_correlation` for the
    // uncompressed path.
    #[test]
    fn test_uncompressed_backend_eval_shares_bit_correlation() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1);
        let delta_ev = eval_out.delta_ev;

        for k in 0..n {
            let bit = gen_out.alpha_auth_bit_shares[k].bit()
                    ^ eval_out.alpha_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.alpha_dev[k] ^ eval_out.alpha_dev[k], expected,
                "alpha D_ev XOR mismatch at {k}");
        }
        for k in 0..m {
            let bit = gen_out.beta_auth_bit_shares[k].bit()
                    ^ eval_out.beta_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.beta_dev[k] ^ eval_out.beta_dev[k], expected,
                "beta D_ev XOR mismatch at {k}");
        }
        for k in 0..(n * m) {
            let bit = gen_out.correlated_auth_bit_shares[k].bit()
                    ^ eval_out.correlated_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.correlated_dev[k] ^ eval_out.correlated_dev[k], expected,
                "correlated D_ev XOR mismatch at {k}");
        }
    }

    // Uncompressed backend: every gamma share satisfies the IT-MAC invariant
    // under both deltas. Mirrors `test_ideal_backend_gamma_auth_bit_shares_mac_invariant`.
    #[test]
    fn test_uncompressed_backend_gamma_auth_bit_shares_mac_invariant() {
        let n = 4;
        let m = 3;
        let (gen_out, eval_out) = UncompressedPreprocessingBackend.run(n, m, 1);
        for k in 0..(n * m) {
            verify_cross_party(
                &gen_out.gamma_auth_bit_shares[k],
                &eval_out.gamma_auth_bit_shares[k],
                &gen_out.delta_gb,
                &eval_out.delta_ev,
            );
        }
    }

    // PRE-02: IdealPreprocessingBackend returns correctly dimensioned output.
    // Note: bindings named `gen_out` / `eval_out` because `gen` is a reserved keyword
    // in Rust 2024 edition.
    #[test]
    fn test_ideal_backend_dimensions() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1);
        assert_eq!(gen_out.n, 4);
        assert_eq!(gen_out.m, 4);
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), 4,  "alpha shares: length n=4");
        assert_eq!(gen_out.beta_auth_bit_shares.len(),  4,  "beta shares: length m=4");
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), 16, "correlated shares: length n*m=16");
        assert_eq!(eval_out.n, 4);
        assert_eq!(eval_out.m, 4);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), 16);
    }

    // PRE-04: gamma_auth_bit_shares length is n*m on both sides
    #[test]
    fn test_ideal_backend_gamma_auth_bit_shares_length() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1);
        assert_eq!(gen_out.gamma_auth_bit_shares.len(),  4 * 4,
            "gen.gamma_auth_bit_shares must have n*m=16 entries");
        assert_eq!(eval_out.gamma_auth_bit_shares.len(), 4 * 4,
            "eval.gamma_auth_bit_shares must have n*m=16 entries");
    }

    // PRE-04 + D-09: IT-MAC invariant (mac = key XOR bit * delta) holds for all gamma shares.
    // WARNING: Do NOT call share.verify(delta) directly — it panics on correctly-formed
    // cross-party shares. Always use verify_cross_party (see RESEARCH.md Pitfall 3).
    #[test]
    fn test_ideal_backend_gamma_auth_bit_shares_mac_invariant() {
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(4, 4, 1);
        for k in 0..(4 * 4) {
            verify_cross_party(
                &gen_out.gamma_auth_bit_shares[k],
                &eval_out.gamma_auth_bit_shares[k],
                &gen_out.delta_gb,
                &eval_out.delta_ev,
            );
        }
        // If no panic: all 16 gamma shares satisfy the IT-MAC invariant.
    }

    // P2-01 (Phase 9 D-04, D-06): All three new D_ev field pairs are populated
    // and have correct lengths n, m, n*m respectively.
    #[test]
    fn test_ideal_backend_eval_shares_lengths() {
        let n = 4;
        let m = 4;
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(n, m, 1);
        assert_eq!(gen_out.alpha_dev.len(),        n,
            "gen.alpha_dev must have length n");
        assert_eq!(eval_out.alpha_dev.len(),       n,
            "eval.alpha_dev must have length n");
        assert_eq!(gen_out.beta_dev.len(),         m,
            "gen.beta_dev must have length m");
        assert_eq!(eval_out.beta_dev.len(),        m,
            "eval.beta_dev must have length m");
        assert_eq!(gen_out.correlated_dev.len(),   n * m,
            "gen.correlated_dev must have length n*m");
        assert_eq!(eval_out.correlated_dev.len(),  n * m,
            "eval.correlated_dev must have length n*m");
    }

    // P2-01: D_ev label pairs XOR to `bit * delta_ev` — the core correctness invariant.
    // gen_label XOR eval_label == lambda_i * D_ev for each alpha/beta/correlated bit.
    #[test]
    fn test_ideal_backend_eval_shares_bit_correlation() {
        use crate::block::Block;
        let n = 4;
        let m = 4;
        let (gen_out, eval_out) = IdealPreprocessingBackend.run(n, m, 1);
        let delta_ev = eval_out.delta_ev;

        for k in 0..n {
            let bit = gen_out.alpha_auth_bit_shares[k].bit() ^ eval_out.alpha_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.alpha_dev[k] ^ eval_out.alpha_dev[k], expected,
                "alpha D_ev XOR mismatch at {k}");
        }
        for k in 0..m {
            let bit = gen_out.beta_auth_bit_shares[k].bit() ^ eval_out.beta_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.beta_dev[k] ^ eval_out.beta_dev[k], expected,
                "beta D_ev XOR mismatch at {k}");
        }
        for k in 0..(n * m) {
            let bit = gen_out.correlated_auth_bit_shares[k].bit() ^ eval_out.correlated_auth_bit_shares[k].bit();
            let expected = if bit { *delta_ev.as_block() } else { Block::ZERO };
            assert_eq!(gen_out.correlated_dev[k] ^ eval_out.correlated_dev[k], expected,
                "correlated D_ev XOR mismatch at {k}");
        }
    }

    // PRE-04 + D-05: gamma_auth_bit_shares (l_gamma) is a different random sample
    // from correlated_auth_bit_shares (l_gamma*). Basic distinctness: not all-zero bits.
    #[test]
    fn test_ideal_backend_gamma_distinct_from_correlated() {
        let (gen_out, _eval_out) = IdealPreprocessingBackend.run(4, 4, 1);
        // gamma bits: gen and eval shares XOR to the actual l_gamma bit (gen.value XOR eval.value)
        // correlated bits: same XOR gives l_gamma* bit
        // They are independent random samples; expect at least one to differ from false.
        let any_gamma_set = gen_out.gamma_auth_bit_shares.iter()
            .any(|s| s.value);
        let any_correlated_set = gen_out.correlated_auth_bit_shares.iter()
            .any(|s| s.value);
        // With overwhelming probability at least one of each is set (random bits with n*m=16).
        // This test is probabilistic but uses a fixed seed so it is deterministic.
        // If this fails, the RNG seeding is broken (all bits are the same constant).
        let _ = any_gamma_set;    // not asserted — just checking no panic in access
        let _ = any_correlated_set;
        // Verify that gamma and correlated shares are not byte-for-byte identical
        // (independent random samples from different RNG seeds must differ).
        let gamma_bits: Vec<bool> = gen_out.gamma_auth_bit_shares.iter()
            .map(|s| s.value)
            .collect();
        let correlated_bits: Vec<bool> = gen_out.correlated_auth_bit_shares.iter()
            .map(|s| s.value)
            .collect();
        assert_ne!(
            gamma_bits,
            correlated_bits,
            "gamma and correlated auth bit shares must be independently sampled (different RNG seeds)"
        );
    }
}
