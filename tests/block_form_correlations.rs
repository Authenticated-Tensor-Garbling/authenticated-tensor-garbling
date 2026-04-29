//! Verifies that the Block-form `_eval` and `_gen` sharings on
//! `AuthTensorGen` / `AuthTensorEval` correctly encode the same
//! authenticated bit values as the underlying `*_auth_bit_shares` triples.
//!
//! For each authenticated bit category (alpha, beta, correlated, gamma)
//! and each lowering form (`_eval` under δ_b, `_gen` under δ_a):
//!
//!   gen_block[i] XOR eval_block[i] == bit_i · delta
//!
//! where `bit_i = gen.auth_bit_shares[i].bit() XOR eval.auth_bit_shares[i].bit()`.
//!
//! Once these invariants hold for both backends, the `*_auth_bit_shares`
//! fields are redundant given `_eval` + `_gen` + own δ:
//!   bit  = LSB(party._eval XOR party._gen)        // since LSB(δ_a XOR δ_b) = 1
//!   mac  = party._eval                  (gen-side) | party._gen  (eval-side)
//!   key  = party._gen XOR (bit · δ_a)   (gen-side) | party._eval XOR (bit · δ_b) (eval-side)
//!
//! Run as an integration test in `tests/` so it isolates from lib.rs's
//! pre-existing red `test_auth_tensor_product` (carried over from 1.2(d)
//! pending consumer migration in 1.2(h)).

use authenticated_tensor_garbling::auth_tensor_eval::AuthTensorEval;
use authenticated_tensor_garbling::auth_tensor_gen::AuthTensorGen;
use authenticated_tensor_garbling::block::Block;
use authenticated_tensor_garbling::delta::Delta;
use authenticated_tensor_garbling::preprocessing::{
    IdealPreprocessingBackend, TensorPreprocessing, UncompressedPreprocessingBackend,
};
use authenticated_tensor_garbling::sharing::AuthBitShare;

fn check_one(
    label: &str,
    gen_blocks: &[Block],
    eval_blocks: &[Block],
    gen_shares: &[AuthBitShare],
    eval_shares: &[AuthBitShare],
    delta: &Delta,
) {
    let len = gen_shares.len();
    assert_eq!(eval_shares.len(), len, "{label}: auth_bit_shares len mismatch");
    assert_eq!(gen_blocks.len(), len, "{label}: gen Block-form len mismatch");
    assert_eq!(eval_blocks.len(), len, "{label}: eval Block-form len mismatch");
    for k in 0..len {
        let bit = gen_shares[k].bit() ^ eval_shares[k].bit();
        let expected = if bit { *delta.as_block() } else { Block::ZERO };
        let got = gen_blocks[k] ^ eval_blocks[k];
        assert_eq!(
            got, expected,
            "{label} XOR mismatch at index {k}: full_bit={bit}",
        );
    }
}

/// Truth-source `*_auth_bit_shares` slices for each preprocessing category.
/// Read from `TensorFpreGen` / `TensorFpreEval` before they are consumed by
/// `AuthTensorGen::new_from_fpre_gen` / `AuthTensorEval::new_from_fpre_eval`,
/// since those structs no longer carry alpha / beta / correlated auth_bit_shares
/// (gamma stays per the deferred `compute_lambda_gamma`).
struct AuthBitTruth {
    gen_alpha: Vec<AuthBitShare>,
    gen_beta:  Vec<AuthBitShare>,
    gen_corr:  Vec<AuthBitShare>,
    gen_gamma: Vec<AuthBitShare>,
    eval_alpha: Vec<AuthBitShare>,
    eval_beta:  Vec<AuthBitShare>,
    eval_corr:  Vec<AuthBitShare>,
    eval_gamma: Vec<AuthBitShare>,
}

fn check_all(gen_out: &AuthTensorGen, eval_out: &AuthTensorEval, t: &AuthBitTruth) {
    let delta_a = gen_out.delta_a;
    let delta_b = eval_out.delta_b;

    // alpha (length n) — both forms
    check_one("alpha _eval",
        &gen_out.alpha_eval, &eval_out.alpha_eval,
        &t.gen_alpha, &t.eval_alpha,
        &delta_b);
    check_one("alpha _gen",
        &gen_out.alpha_gen, &eval_out.alpha_gen,
        &t.gen_alpha, &t.eval_alpha,
        &delta_a);

    // beta (length m) — both forms
    check_one("beta _eval",
        &gen_out.beta_eval, &eval_out.beta_eval,
        &t.gen_beta, &t.eval_beta,
        &delta_b);
    check_one("beta _gen",
        &gen_out.beta_gen, &eval_out.beta_gen,
        &t.gen_beta, &t.eval_beta,
        &delta_a);

    // correlated `l_gamma*` (length n*m, column-major) — both forms
    check_one("correlated _eval",
        &gen_out.correlated_eval, &eval_out.correlated_eval,
        &t.gen_corr, &t.eval_corr,
        &delta_b);
    check_one("correlated _gen",
        &gen_out.correlated_gen, &eval_out.correlated_gen,
        &t.gen_corr, &t.eval_corr,
        &delta_a);

    // gamma `l_gamma` output mask (length n*m, column-major) — both forms.
    // Truth slices come from `t.gen_gamma` / `t.eval_gamma` extracted from
    // fpre_* before construction; AuthTensorGen/Eval no longer carry
    // gamma_auth_bit_shares (Option B for compute_lambda_gamma retired
    // it along with the gate-semantics check).
    check_one("gamma _eval",
        &gen_out.gamma_eval, &eval_out.gamma_eval,
        &t.gen_gamma, &t.eval_gamma,
        &delta_b);
    check_one("gamma _gen",
        &gen_out.gamma_gen, &eval_out.gamma_gen,
        &t.gen_gamma, &t.eval_gamma,
        &delta_a);
}

/// Bonus: also verify the recovery formula -- given (_eval, _gen, own_delta)
/// the bit value is recoverable as `LSB(party._eval XOR party._gen)` for both
/// parties on every authenticated bit. If this passes, removing
/// `*_auth_bit_shares` from the structs loses no information.
fn check_bit_recovery(gen_out: &AuthTensorGen, eval_out: &AuthTensorEval, t: &AuthBitTruth) {
    fn check_recovery(
        label: &str,
        gen_eval_blocks: &[Block],
        gen_gen_blocks:  &[Block],
        eval_eval_blocks: &[Block],
        eval_gen_blocks:  &[Block],
        gen_shares:  &[AuthBitShare],
        eval_shares: &[AuthBitShare],
    ) {
        let len = gen_shares.len();
        for k in 0..len {
            let recovered_gen  = (gen_eval_blocks[k]  ^ gen_gen_blocks[k]).lsb();
            let recovered_eval = (eval_eval_blocks[k] ^ eval_gen_blocks[k]).lsb();
            assert_eq!(recovered_gen,  gen_shares[k].bit(),
                "{label} gen-side bit recovery mismatch at index {k}");
            assert_eq!(recovered_eval, eval_shares[k].bit(),
                "{label} eval-side bit recovery mismatch at index {k}");
        }
    }

    check_recovery("alpha",
        &gen_out.alpha_eval, &gen_out.alpha_gen,
        &eval_out.alpha_eval, &eval_out.alpha_gen,
        &t.gen_alpha, &t.eval_alpha);
    check_recovery("beta",
        &gen_out.beta_eval, &gen_out.beta_gen,
        &eval_out.beta_eval, &eval_out.beta_gen,
        &t.gen_beta, &t.eval_beta);
    check_recovery("correlated",
        &gen_out.correlated_eval, &gen_out.correlated_gen,
        &eval_out.correlated_eval, &eval_out.correlated_gen,
        &t.gen_corr, &t.eval_corr);
    check_recovery("gamma",
        &gen_out.gamma_eval, &gen_out.gamma_gen,
        &eval_out.gamma_eval, &eval_out.gamma_gen,
        &t.gen_gamma, &t.eval_gamma);
}

/// Extract the auth-bit truth slices from `fpre_*` before constructing the
/// AuthTensor* (which consumes them and no longer carries alpha / beta /
/// correlated auth_bit_shares).
fn run_backend(backend: &dyn TensorPreprocessing, n: usize, m: usize)
    -> (AuthTensorGen, AuthTensorEval, AuthBitTruth)
{
    let (fpre_gen, fpre_eval) = backend.run(n, m, 1, 1);
    let truth = AuthBitTruth {
        gen_alpha: fpre_gen.alpha_auth_bit_shares.clone(),
        gen_beta:  fpre_gen.beta_auth_bit_shares.clone(),
        gen_corr:  fpre_gen.correlated_auth_bit_shares.clone(),
        gen_gamma: fpre_gen.gamma_auth_bit_shares.clone(),
        eval_alpha: fpre_eval.alpha_auth_bit_shares.clone(),
        eval_beta:  fpre_eval.beta_auth_bit_shares.clone(),
        eval_corr:  fpre_eval.correlated_auth_bit_shares.clone(),
        eval_gamma: fpre_eval.gamma_auth_bit_shares.clone(),
    };
    let gen_out = AuthTensorGen::new_from_fpre_gen(fpre_gen);
    let eval_out = AuthTensorEval::new_from_fpre_eval(fpre_eval);
    (gen_out, eval_out, truth)
}

#[test]
fn ideal_backend_block_form_correlations() {
    let (gen_out, eval_out, truth) = run_backend(&IdealPreprocessingBackend, 4, 4);
    check_all(&gen_out, &eval_out, &truth);
    check_bit_recovery(&gen_out, &eval_out, &truth);
}

#[test]
fn uncompressed_backend_block_form_correlations() {
    let (gen_out, eval_out, truth) = run_backend(&UncompressedPreprocessingBackend, 4, 4);
    check_all(&gen_out, &eval_out, &truth);
    check_bit_recovery(&gen_out, &eval_out, &truth);
}

#[test]
fn ideal_backend_block_form_correlations_asymmetric_dims() {
    // n != m to catch any column-major / row-major confusion in correlated/gamma.
    let (gen_out, eval_out, truth) = run_backend(&IdealPreprocessingBackend, 3, 5);
    check_all(&gen_out, &eval_out, &truth);
    check_bit_recovery(&gen_out, &eval_out, &truth);
}
