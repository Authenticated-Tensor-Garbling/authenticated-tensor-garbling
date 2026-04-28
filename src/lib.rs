pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;
pub mod sharing;

pub mod matrix;

pub mod aes;

pub mod tensor_pre;
pub mod tensor_gen;
pub mod tensor_eval;
pub mod tensor_ops;
pub mod tensor_macro;

pub mod auth_tensor_fpre;
pub mod auth_tensor_gen;
pub mod auth_tensor_eval;

pub mod bcot;
pub mod feq;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;
pub mod preprocessing;
pub mod online;

use crate::block::Block;
use crate::auth_tensor_gen::AuthTensorGen;
use crate::auth_tensor_eval::AuthTensorEval;
use crate::sharing::AuthBitShare;
use crate::keys::Key;

/// κ — computational security parameter (in bits). Determines `Block` width
/// and all κ-bit cipher / hash output widths.
pub const CSP: usize = 128;
/// ρ — statistical security parameter (in bits). Bench accounting reads this
/// via `RHO_BYTES = (SSP + 7) / 8` so reported communication and the network
/// simulator's transit time track the paper's κ + ρ leaf-ciphertext width.
pub const SSP: usize = 40;

/// Gate-semantics sanity check — verifies that an honestly garbled tensor gate
/// produces `v_γ = v_α · v_β` at every output position.
///
/// **NOT the paper's Protocol 1 consistency check.** For the paper-faithful P1
/// abort check (per `5_online.tex` lines 226–247), see
/// `assemble_e_input_wire_shares_p1` instead. That helper checks `e_a, e_b` on
/// input wires under `delta_b` (D_ev) — the structurally-correct CheckZero for
/// catching a malicious garbler. This one checks gate semantics under `delta_a`
/// (D_gb) and so its threat-model direction (catches a cheating evaluator,
/// not garbler) is opposite to what P1's abort logic requires. It is retained
/// as a regression check on AND-truth-table garbling correctness, exercised by
/// `test_gate_semantics_check_aborts_on_tampered_lambda`.
///
/// For each (i, j) output position, computes the linear combination:
/// ```text
///   c_gamma[(i,j)] = (L_alpha[i] AND L_beta[j])              [public bit]
///                  XOR L_alpha[i] · l_beta[j]                 [shared, conditional]
///                  XOR L_beta [j] · l_alpha[i]                [shared, conditional]
///                  XOR l_gamma_star[(i,j)]                    [= l_alpha · l_beta]
///                  XOR L_gamma[(i,j)]                          [public bit]
///                  XOR l_gamma[(i,j)]                          [shared]
/// ```
/// which simplifies algebraically to `v_α · v_β ⊕ v_γ`. This is zero iff the
/// AND truth table was honestly garbled. Verified under `delta_a`.
///
/// SIMULATION ONLY: requires both parties' state. In a real protocol each
/// party assembles its half independently and the parties run `check_zero` on
/// the combined share over the network. The combined-key construction sums
/// gen-side IT-MAC keys (which authenticate eval-side values under delta_a per
/// `gen_auth_bit` in `src/auth_tensor_fpre.rs:66-86`), and the MAC is freshly
/// recomputed via `combined_key.auth(c_gamma_bit, &gb.delta_a)` (never naive
/// cross-party MAC XOR — see `src/online.rs:30-36`).
///
/// Returns `Vec<AuthBitShare>` of length `n * m` in column-major order
/// (`j * n + i`).
#[allow(clippy::too_many_arguments)]
pub fn assemble_gate_semantics_shares(
    n: usize,
    m: usize,
    l_alpha_pub: &[bool],          // length n — public masked alpha bits
    l_beta_pub: &[bool],           // length m — public masked beta bits
    l_gamma_pub: &[bool],          // length n*m — public masked gamma bits (column-major)
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> Vec<AuthBitShare> {
    assert_eq!(l_alpha_pub.len(), n);
    assert_eq!(l_beta_pub.len(),  m);
    assert_eq!(l_gamma_pub.len(), n * m);
    assert_eq!(gb.alpha_auth_bit_shares.len(),       n);
    assert_eq!(gb.beta_auth_bit_shares.len(),        m);
    assert_eq!(gb.correlated_auth_bit_shares.len(),  n * m);
    assert_eq!(gb.gamma_d_ev_shares.len(),           n * m);
    assert_eq!(ev.alpha_auth_bit_shares.len(),       n);
    assert_eq!(ev.beta_auth_bit_shares.len(),        m);
    assert_eq!(ev.correlated_auth_bit_shares.len(),  n * m);
    assert_eq!(ev.gamma_d_ev_shares.len(),           n * m);

    let mut out: Vec<AuthBitShare> = Vec::with_capacity(n * m);
    for j in 0..m {
        for i in 0..n {
            let idx = j * n + i;

            // Accumulate the combined key (XOR of gen-side B-keys) and the
            // full reconstructed c_gamma bit (XOR of both parties' values).
            // The gen-side keys are the D_ev-structure keys (B's keys, Kb_i)
            // that authenticate the eval's share values under delta_a.
            let mut combined_key = Key::from(Block::ZERO);
            let mut c_gamma_bit = false;

            // Term: L_alpha[i] · l_beta[j]   (include iff L_alpha[i] is true)
            if l_alpha_pub[i] {
                combined_key = combined_key + gb.beta_auth_bit_shares[j].key;
                c_gamma_bit ^= gb.beta_auth_bit_shares[j].value
                             ^ ev.beta_auth_bit_shares[j].value;
            }

            // Term: L_beta[j] · l_alpha[i]   (include iff L_beta[j] is true)
            if l_beta_pub[j] {
                combined_key = combined_key + gb.alpha_auth_bit_shares[i].key;
                c_gamma_bit ^= gb.alpha_auth_bit_shares[i].value
                             ^ ev.alpha_auth_bit_shares[i].value;
            }

            // Term: l_gamma*[(i,j)] = l_alpha[i] · l_beta[j]   (always)
            combined_key = combined_key + gb.correlated_auth_bit_shares[idx].key;
            c_gamma_bit ^= gb.correlated_auth_bit_shares[idx].value
                         ^ ev.correlated_auth_bit_shares[idx].value;

            // Term: l_gamma[(i,j)]   (always)
            combined_key = combined_key + gb.gamma_d_ev_shares[idx].key;
            c_gamma_bit ^= gb.gamma_d_ev_shares[idx].value
                         ^ ev.gamma_d_ev_shares[idx].value;

            // Fold the PUBLIC-bit value contribution.
            // (L_alpha[i] AND L_beta[j]) XOR L_gamma[(i,j)] contribute only to
            // the bit value — no IT-MAC structure.
            let public_bit = (l_alpha_pub[i] & l_beta_pub[j]) ^ l_gamma_pub[idx];
            c_gamma_bit ^= public_bit;

            // Build a properly-formed AuthBitShare verified under delta_a.
            // key = XOR(gen-side B-keys), mac = key.auth(c_gamma_bit, delta_a).
            // value = c_gamma_bit.
            let combined_mac = combined_key.auth(c_gamma_bit, &gb.delta_a);
            let share = AuthBitShare {
                key: combined_key,
                mac: combined_mac,
                value: c_gamma_bit,
            };

            out.push(share);
        }
    }
    out
}

/// Paper-faithful Protocol 1 input-wire CheckZero assembly under `delta_b`.
///
/// Implements the Protocol 1 consistency check from
/// `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex`
/// lines 226–247, in conjunction with the input-encoding identity at lines
/// 211–217:
///
/// ```text
///   gb sets [v_x D_ev]^gb := [l_x D_ev]^gb
///   ev sets [v_x D_ev]^ev := [l_x D_ev]^ev XOR (x XOR l_x) D_ev
/// ```
///
/// For each tensor gate with input vectors `a` (length n) and `b` (length m),
/// the paper builds two shares per input wire (line 242–243):
///
/// ```text
///   [e_a[i] D_ev] := [a[i] D_ev] XOR [l_a[i] D_ev] XOR (a[i] XOR l_a[i]) D_ev
///   [e_b[j] D_ev] := [b[j] D_ev] XOR [l_b[j] D_ev] XOR (b[j] XOR l_b[j]) D_ev
/// ```
///
/// CheckZero (paper line 246) verifies these are all zero under `delta_b`.
/// For honest parties the identity reduces to zero by definition of input
/// encoding (paper Lemma `lem:protocol1-correctness`, line 297). The
/// substantive security comes from the IT-MAC structure under `delta_b`: a
/// malicious garbler that lies about its `[v D_ev]^gb` share during input
/// encoding produces a non-zero combined block, breaking the bit-zero
/// invariant.
///
/// # Differences from `assemble_gate_semantics_shares`
/// - **Wire**: input wires `a, b`, not output wire `c`. Length `n + m`, not
///   `n * m`.
/// - **Formula**: 3 terms (paper line 242–243), no `correlated_d_ev_shares`
///   (no tensor-product term).
/// - **Delta**: this helper verifies under `delta_b` (D_ev), the paper-
///   faithful direction for catching a malicious garbler.
///   `assemble_gate_semantics_shares` uses `delta_a` for a *different*
///   (gate-semantics) check, not the paper's protocol abort check.
///
/// Used by both Protocol 1 (`5_online.tex` lines 242–246) and Protocol 2
/// (`6_total.tex` lines 207–214 — same formula on `c_α/c_β`); see
/// `assemble_c_alpha_beta_shares_p2` for the paper-mapped P2 alias.
///
/// # Combined-key choice
///
/// Per `gen_auth_bit` in `src/auth_tensor_fpre.rs:66-86`, under `delta_b`
/// `ev.alpha_auth_bit_shares[i].key` is the canonical "key authenticating
/// `gb.value` under `delta_b`". The helper uses these eval-side keys to build
/// a combined `AuthBitShare` verifying under `delta_b` for the full
/// reconstructed `e_a` bit.
///
/// # SIMULATION ONLY
///
/// Requires both parties' state. In a real protocol each party assembles its
/// half independently using only its own preprocessing and `[v D_ev]` shares,
/// then runs `check_zero` over the network.
///
/// To make the simulation sensitive to a malicious garbler that lies about its
/// `[v_a D_ev]^gb` (NOT just its preprocessing), this helper accepts `[v_a
/// D_ev]^gb` as an explicit parameter rather than aliasing it to
/// `gb.alpha_d_ev_shares[i]`. Honest callers pass `gb.alpha_d_ev_shares.clone()`;
/// negative tests can pass tampered Blocks.
///
/// # Inputs
/// - `n`, `m`: input vector lengths.
/// - `gb_v_alpha_d_ev`: `[v_a D_ev]^gb` for each i (length n). Honest input
///   encoding sets this equal to `gb.alpha_d_ev_shares`.
/// - `ev_v_alpha_d_ev`: `[v_a D_ev]^ev` for each i (length n). Honest input
///   encoding sets this equal to `ev.alpha_d_ev_shares[i] ^ (L_a · delta_b)`.
/// - `gb_v_beta_d_ev`, `ev_v_beta_d_ev`: same for the β input vector (length m).
/// - `l_alpha_pub`, `l_beta_pub`: announced masked input values
///   `vec a XOR vec l_a` and `vec b XOR vec l_b`.
/// - `gb`, `ev`: party preprocessing/state for `_d_ev_shares` and
///   `_auth_bit_shares` reads.
///
/// # Returns
///
/// `Vec<AuthBitShare>` of length `n + m`, layout
/// `[e_a_0, …, e_a_{n-1}, e_b_0, …, e_b_{m-1}]`. Each share verifies under
/// `ev.delta_b`.
#[allow(clippy::too_many_arguments)]
pub fn assemble_e_input_wire_shares_p1(
    n: usize,
    m: usize,
    gb_v_alpha_d_ev: &[Block],
    ev_v_alpha_d_ev: &[Block],
    gb_v_beta_d_ev: &[Block],
    ev_v_beta_d_ev: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> Vec<AuthBitShare> {
    assert_eq!(gb_v_alpha_d_ev.len(), n);
    assert_eq!(ev_v_alpha_d_ev.len(), n);
    assert_eq!(gb_v_beta_d_ev.len(),  m);
    assert_eq!(ev_v_beta_d_ev.len(),  m);
    assert_eq!(l_alpha_pub.len(), n);
    assert_eq!(l_beta_pub.len(),  m);
    assert_eq!(gb.alpha_auth_bit_shares.len(), n);
    assert_eq!(gb.beta_auth_bit_shares.len(),  m);
    assert_eq!(gb.alpha_d_ev_shares.len(), n);
    assert_eq!(gb.beta_d_ev_shares.len(),  m);
    assert_eq!(ev.alpha_auth_bit_shares.len(), n);
    assert_eq!(ev.beta_auth_bit_shares.len(),  m);
    assert_eq!(ev.alpha_d_ev_shares.len(), n);
    assert_eq!(ev.beta_d_ev_shares.len(),  m);

    let mut out: Vec<AuthBitShare> = Vec::with_capacity(n + m);

    // e_a checks: one per α-input wire.
    for i in 0..n {
        // L_a · delta_b correction Block.
        let l_a_correction = if l_alpha_pub[i] {
            *ev.delta_b.as_block()
        } else {
            Block::default()
        };

        // Each party's half-share of [e_a D_ev] per paper line 242:
        //   gb's half = [v_a D_ev]^gb XOR [l_a D_ev]^gb
        //   ev's half = [v_a D_ev]^ev XOR [l_a D_ev]^ev XOR L_a · D_ev
        let gb_e_block = gb_v_alpha_d_ev[i] ^ gb.alpha_d_ev_shares[i];
        let ev_e_block = ev_v_alpha_d_ev[i]
                         ^ ev.alpha_d_ev_shares[i]
                         ^ l_a_correction;
        let combined_e_block = gb_e_block ^ ev_e_block;

        // For honest parties combined_e_block is the zero block. For a
        // malicious garbler whose `gb_v_alpha_d_ev[i]` deviates from
        // `gb.alpha_d_ev_shares[i]`, the combined block is non-zero. The
        // .lsb() reading captures tampers whose XOR delta has bit 0 set
        // (e.g., XOR delta_a with LSB=1) — sufficient for the negative test.
        let e_a_bit = combined_e_block.lsb();

        // Combined key under delta_b: eval-side key of alpha_auth_bit_shares.
        // ev.alpha_auth_bit_shares[i].key authenticates gb.value under delta_b
        // per gen_auth_bit symmetry (auth_tensor_fpre.rs:66-86).
        let combined_key = ev.alpha_auth_bit_shares[i].key;

        // Recompute MAC freshly under delta_b. Per check_zero doc
        // (online.rs:30-36): never naive-XOR cross-party MACs.
        let combined_mac = combined_key.auth(e_a_bit, &ev.delta_b);

        out.push(AuthBitShare {
            key: combined_key,
            mac: combined_mac,
            value: e_a_bit,
        });
    }

    // e_b checks: symmetric to α loop.
    for j in 0..m {
        let l_b_correction = if l_beta_pub[j] {
            *ev.delta_b.as_block()
        } else {
            Block::default()
        };

        let gb_e_block = gb_v_beta_d_ev[j] ^ gb.beta_d_ev_shares[j];
        let ev_e_block = ev_v_beta_d_ev[j]
                         ^ ev.beta_d_ev_shares[j]
                         ^ l_b_correction;
        let combined_e_block = gb_e_block ^ ev_e_block;

        let e_b_bit = combined_e_block.lsb();
        let combined_key = ev.beta_auth_bit_shares[j].key;
        let combined_mac = combined_key.auth(e_b_bit, &ev.delta_b);

        out.push(AuthBitShare {
            key: combined_key,
            mac: combined_mac,
            value: e_b_bit,
        });
    }

    out
}

/// Paper-faithful Protocol 2 input-wire CheckZero assembly under `delta_b`.
///
/// Implements the Protocol 2 consistency check from
/// `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/6_total.tex`
/// lines 205–215:
///
/// ```text
///   [c_α D_ev] := [v_α D_ev] XOR [l_α D_ev] XOR L_α · D_ev    (length n)
///   [c_β D_ev] := [v_β D_ev] XOR [l_β D_ev] XOR L_β · D_ev    (length m)
///   CheckZero({[c_α D_ev], [c_β D_ev]})
/// ```
///
/// Algebraically identical to the Protocol 1 input-wire check
/// (`5_online.tex` lines 242–246, implemented by
/// `assemble_e_input_wire_shares_p1`): both protocols' abort check is on
/// tensor-gate input wires, three-term XOR `[v D_ev] ⊕ [λ D_ev] ⊕ L·D_ev`,
/// verified under `delta_b`. Variable names differ only because the paper
/// uses `e_a/e_b` in P1 and `c_α/c_β` in P2. This helper is therefore a
/// thin alias that delegates to the P1 routine — keeping the paper-mapped
/// name in P2 call sites while avoiding duplicated code.
///
/// Distinct from the (now removed) `c_gamma`-flavored P2 check that operated
/// over γ output wires: that variant did not match `6_total.tex` line 207–212
/// — the paper checks input wires α, β, not output γ.
///
/// # Inputs (mirror P1)
/// - `n`, `m`: input vector lengths.
/// - `gb_v_alpha_d_ev` / `ev_v_alpha_d_ev`: `[v_α D_ev]` shares (length n).
///   Honest input encoding sets these consistent with `alpha_d_ev_shares` on
///   each party (gb's equals its `alpha_d_ev_shares`; ev's equals its share
///   XOR'd with `L_α · delta_b`).
/// - `gb_v_beta_d_ev` / `ev_v_beta_d_ev`: same for β (length m).
/// - `l_alpha_pub` / `l_beta_pub`: announced masked-input bits `L_α = v_α ⊕ l_α`
///   and `L_β = v_β ⊕ l_β`.
/// - `gb`, `ev`: party state for `_d_ev_shares` and `_auth_bit_shares` reads.
///
/// # Returns
/// `Vec<AuthBitShare>` of length `n + m`, layout
/// `[c_α[0..n-1], c_β[0..m-1]]`. Each share verifies under `ev.delta_b`.
#[allow(clippy::too_many_arguments)]
pub fn assemble_c_alpha_beta_shares_p2(
    n: usize,
    m: usize,
    gb_v_alpha_d_ev: &[Block],
    ev_v_alpha_d_ev: &[Block],
    gb_v_beta_d_ev: &[Block],
    ev_v_beta_d_ev: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> Vec<AuthBitShare> {
    assemble_e_input_wire_shares_p1(
        n, m,
        gb_v_alpha_d_ev,
        ev_v_alpha_d_ev,
        gb_v_beta_d_ev,
        ev_v_beta_d_ev,
        l_alpha_pub,
        l_beta_pub,
        gb,
        ev,
    )
}

#[cfg(test)]
mod tests {

    use crate::delta::Delta;

    use crate::{
        tensor_gen::TensorProductGen,
        tensor_eval::TensorProductEval,
        matrix::BlockMatrix,
        block::Block
    };

    fn verify_vector_sharing(
        clear_val: usize,
        gb_share: &Vec<Block>,
        ev_share: &Vec<Block>,
        delta: &Delta,
        n: usize
    ) -> bool {
        assert_eq!(gb_share.len(), n);
        assert_eq!(gb_share.len(), ev_share.len());
        for i in 0..gb_share.len() {
            let expected_val = ((clear_val>>i)&1) != 0;
            if expected_val {
                if gb_share[i] != ev_share[i] ^ delta.as_block() {
                    return false;
                }
            } else {
                if gb_share[i] != ev_share[i] {
                    return false;
                }
            }
        }
        true
    }

    fn verify_column_matrix_sharing(
        clear_val: usize,
        gb_share: &BlockMatrix,
        ev_share: &BlockMatrix,
        delta: &Delta,
        n: usize,
    ) -> bool {
        assert_eq!(gb_share.rows(), n);
        assert_eq!(gb_share.rows(), ev_share.rows());
        for i in 0..gb_share.rows() {
            let expected_val = ((clear_val>>i)&1) != 0;
            if expected_val {
                if gb_share[i] != ev_share[i] ^ delta.as_block() {
                    return false;
                }
            } else {
                if gb_share[i] != ev_share[i] {
                    return false;
                }
            }
        }
        true
    }

    fn verify_tensor_output(
        clear_x: usize,
        clear_y: usize,
        n: usize,
        m: usize,
        gb_out: &BlockMatrix,
        ev_out: &BlockMatrix,
        delta: &Delta,
    ) -> bool {
        for i in 0..n {
            for k in 0..m {
                let expected_val = (((clear_x>>i)&1) & ((clear_y>>k)&1)) != 0;
                if expected_val {
                    if gb_out[(i, k)] != ev_out[(i, k)] ^ delta.as_block() {
                        return false;
                    }
                } else {
                    if gb_out[(i, k)] != ev_out[(i, k)] {
                        return false;
                    }
                }
            }
        }
        true
    }

    use crate::tensor_pre::SemiHonestTensorPre;
    
    #[test]
    fn test_semihonest_tensor_product() {
        
        let mut rng = rand::rng();
        let delta = Delta::random(&mut rng);

        let n = 2;
        let m = 3;
        let clear_x = 0b01;
        let clear_y = 0b101;

        let mut pre = SemiHonestTensorPre::new_with_delta(3, n, m, 6, delta);
        pre.gen_inputs(clear_x, clear_y);

        assert!(
            verify_vector_sharing(clear_x, &pre.x_labels.iter().map(|share| share.gen_share).collect(), &pre.x_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(clear_y, &pre.y_labels.iter().map(|share| share.gen_share).collect(), &pre.y_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let (alpha, beta) = pre.gen_masks();


        assert!(
            verify_vector_sharing(alpha, &pre.alpha_labels.iter().map(|share| share.gen_share).collect(), &pre.alpha_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(beta, &pre.beta_labels.iter().map(|share| share.gen_share).collect(), &pre.beta_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let (masked_x, masked_y) = pre.mask_inputs();
        
        
        assert!(
            verify_vector_sharing(masked_x, &pre.x_labels.iter().map(|share| share.gen_share).collect(), &pre.x_labels.iter().map(|share| share.eval_share).collect(), &delta, n)
        );
        assert!(
            verify_vector_sharing(masked_y, &pre.y_labels.iter().map(|share| share.gen_share).collect(), &pre.y_labels.iter().map(|share| share.eval_share).collect(), &delta, m)
        );


        let n_bitmask = (1<<n)-1;
        let m_bitmask = (1<<m)-1;

        assert_eq!(masked_x, (clear_x ^ alpha) & n_bitmask);
        assert_eq!(masked_y, (clear_y ^ beta) & m_bitmask);


        let (pre_gen, pre_eval) = pre.into_gen_eval();

        let mut gb = TensorProductGen::new_from_fpre_gen(pre_gen);
        let mut ev = TensorProductEval::new_from_fpre_eval(pre_eval);

        assert!(
            verify_vector_sharing(masked_x, &gb.x_labels, &ev.x_labels, &delta, n)
        );
        assert!(
            verify_vector_sharing(masked_y, &gb.y_labels, &ev.y_labels, &delta, m)
        );


        //check the inputs to the first half outer product: masked_x (x) beta
        let (gen_x, gen_y) = gb.get_first_inputs();
        let (eval_x, eval_y) = ev.get_first_inputs();

        assert!(
            verify_column_matrix_sharing(masked_x & n_bitmask, &gen_x, &eval_x, &delta, n)
        );
        assert!(
            verify_column_matrix_sharing(clear_y & m_bitmask, &gen_y, &eval_y, &delta, m)
        );


        let (gen_levels, gen_cts) = gb.garble_first_half_outer_product();
        ev.evaluate_first_half_outer_product(gen_levels, gen_cts);

        assert!(
            verify_tensor_output(masked_x & n_bitmask, clear_y & m_bitmask, n, m, &gb.first_half_out, &ev.first_half_out, &delta)
        );


        let (gen_x, gen_y) = gb.get_second_inputs();
        let (eval_x, eval_y) = ev.get_second_inputs();

        assert!(
            verify_column_matrix_sharing(masked_y & m_bitmask, &gen_x, &eval_x, &delta, m)
        );
        assert!(
            verify_column_matrix_sharing(alpha & n_bitmask, &gen_y, &eval_y, &delta, n)
        );


        // second half outer product: (y ^ beta) (x) alpha
        let (gen_levels, gen_cts) = gb.garble_second_half_outer_product();
        ev.evaluate_second_half_outer_product(gen_levels, gen_cts);

        // check that first_out has the correct value
        assert!(
            verify_tensor_output(masked_y & m_bitmask, alpha & n_bitmask, m, n, &gb.second_half_out, &ev.second_half_out, &delta)
        );

        
        // final outer product
        let gen_result = gb.garble_final_outer_product();
        let eval_result = ev.evaluate_final_outer_product();

        // check that final_out has the correct value
        assert!(
            verify_tensor_output(clear_x, clear_y, n, m, &gen_result, &eval_result, &delta)
        );
    }

    use crate::auth_tensor_fpre::TensorFpre;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing, UncompressedPreprocessingBackend};
    use crate::online::check_zero;
    use crate::sharing::AuthBitShare;
    use super::{
        assemble_gate_semantics_shares,
        assemble_e_input_wire_shares_p1,
        assemble_c_alpha_beta_shares_p2,
    };

    #[test]
    fn test_auth_tensor_product() {
        let mut rng = rand::rng();
        let delta_a = Delta::random(&mut rng);
        let delta_b = Delta::random(&mut rng);

        let n = 16;
        let m = 16;

        let input_x = 0b101;
        let input_y = 0b110;

        let mut fpre = TensorFpre::new_with_delta(54, n, m, 8, delta_a, delta_b);
        fpre.generate_ideal();

        let (fpre_gen, fpre_eval) = fpre.into_gen_eval();

        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels via the new
        // auth-bit-style API. After this call, gb.masked_x_gen / ev.masked_x_gen
        // and the cleartext masked-bit vectors (ev.masked_x_bits) are populated;
        // get_first_inputs / evaluate_first_half / etc. read from them instead
        // of the preprocessing-faked x_labels.
        let labels = gb.prepare_input_labels(
            &mut rng, input_x, input_y,
            &ev.alpha_auth_bit_shares, &ev.beta_auth_bit_shares,
        );
        ev.install_input_labels(labels);

        //check the inputs to the first half outer product: masked_x (x) beta
        let (gen_x, gen_y) = gb.get_first_inputs();
        let (eval_x, eval_y) = ev.get_first_inputs();

        assert!(
            verify_column_matrix_sharing(masked_x, &gen_x, &eval_x, &delta_a, n)
        );
        assert!(
            verify_column_matrix_sharing(input_y, &gen_y, &eval_y, &delta_a, m)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_first_half();
        ev.evaluate_first_half(gen_chunk_levels, gen_chunk_cts);

        // check that first_out has the correct value
        // first_out should be masked_x (tensor) input_y
        assert!(
            verify_tensor_output(masked_x, input_y, n, m, &gb.first_half_out, &ev.first_half_out, &delta_a)
        );


        //check the inputs to the second half outer product: masked_y (x) alpha
        let (gen_x, gen_y) = gb.get_second_inputs();
        let (eval_x, eval_y) = ev.get_second_inputs();

        assert!(
            verify_column_matrix_sharing(masked_y, &gen_x, &eval_x, &delta_a, m)
        );
        assert!(
            verify_column_matrix_sharing(alpha, &gen_y, &eval_y, &delta_a, n)
        );


        let (gen_chunk_levels, gen_chunk_cts) = gb.garble_second_half();
        ev.evaluate_second_half(gen_chunk_levels, gen_chunk_cts);

        // check that second_out has the correct value
        // second_out should be masked_y (tensor) alpha
        assert!(
            verify_tensor_output(masked_y, alpha, m, n, &gb.second_half_out, &ev.second_half_out, &delta_a)
        );

        // check that final_out has the correct value
        for i in 0..n {
            for j in 0..m {
                let expected_val = (((alpha>>i)&1) & ((beta>>j)&1)) != 0;
                let gb_share =
                    if gb.correlated_auth_bit_shares[j * n + i].bit() {
                        gb.delta_a.as_block() ^ gb.correlated_auth_bit_shares[j * n + i].key.as_block()
                    } else {
                        *gb.correlated_auth_bit_shares[j * n + i].key.as_block()
                    };
                let ev_share = *ev.correlated_auth_bit_shares[j * n + i].mac.as_block();

                if expected_val {
                    assert_eq!(gb_share, ev_share ^ delta_a.as_block(), "At position ({},{}): gb_out should equal ev_out ^ delta when expected_val=1", i, j);
                } else {
                    assert_eq!(gb_share, ev_share, "At position ({},{}): gb_out should equal ev_out when expected_val=0", i, j);
                }
            }
        }

        gb.garble_final();
        ev.evaluate_final();

        // check each element for correctness
        for i in 0..n {
            let x_bit = ((input_x >> i) & 1) != 0;
            for j in 0..m {
                let y_bit = ((input_y >> j) & 1) != 0;
                let expected_val = x_bit & y_bit;
                print!("{} ", expected_val);

                let gb_val = gb.first_half_out[(i, j)];
                let ev_val = ev.first_half_out[(i, j)];

                if expected_val {
                    assert_eq!(gb_val, ev_val ^ delta_a.as_block(), "At position ({},{}): gb_out should equal ev_out ^ delta when expected_val=1", i, j);
                } else {
                    assert_eq!(gb_val, ev_val, "At position ({},{}): gb_out should equal ev_out when expected_val=0", i, j);
                }
            }
        }
    }

    /// Body of the paper-faithful Protocol 1 honest-run test, parameterized by
    /// preprocessing backend. Both `IdealPreprocessingBackend` and
    /// `UncompressedPreprocessingBackend` must satisfy the same CheckZero
    /// invariant — preprocessing must populate all four `*_d_ev_shares` fields
    /// with shares that XOR to `bit · delta_b`.
    ///
    /// Per `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex`
    /// lines 226–247 (CheckZero) and 211–217 (input encoding), the consistency check
    /// builds e_a, e_b shares on tensor-gate INPUT wires under D_ev (delta_b), with
    /// the formula `e = v ⊕ l ⊕ L`. For honest input-encoded shares this reconstructs
    /// to zero by paper Lemma `lem:protocol1-correctness` (line 297).
    ///
    /// With both backends' internal input = (0, 0), masked_x = alpha and
    /// masked_y = beta — i.e. v_alpha = v_beta = 0.
    fn run_full_protocol_1(backend: &dyn TensorPreprocessing) {
        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = backend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels with cleartext
        // input = (0, 0). The doc above explicitly notes "v_alpha = v_beta = 0
        // because masked_x = alpha and masked_y = beta with x = y = 0".
        let mut prep_rng = rand::rng();
        let labels = gb.prepare_input_labels(
            &mut prep_rng, 0, 0,
            &ev.alpha_auth_bit_shares, &ev.beta_auth_bit_shares,
        );
        ev.install_input_labels(labels);

        // Standard Protocol 1 garble + evaluate sequence.
        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        // Reconstruct masked input values (paper L_a, L_b) from joint state. In the
        // single-gate test the gate inputs are circuit inputs, so L_a = l_a and
        // L_b = l_b (since v=0 with x=y=0 input).
        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        // Honest input-encoding (paper line 214):
        //   gb sets [v_a D_ev]^gb := [l_a D_ev]^gb
        //   ev sets [v_a D_ev]^ev := [l_a D_ev]^ev XOR L_a · D_ev
        let gb_v_alpha_d_ev: Vec<Block> = gb.alpha_d_ev_shares.clone();
        let ev_v_alpha_d_ev: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_d_ev_shares[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_d_ev_shares[i]
            })
            .collect();
        let gb_v_beta_d_ev: Vec<Block> = gb.beta_d_ev_shares.clone();
        let ev_v_beta_d_ev: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_d_ev_shares[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_d_ev_shares[j]
            })
            .collect();

        let e_shares = assemble_e_input_wire_shares_p1(
            n, m,
            &gb_v_alpha_d_ev,
            &ev_v_alpha_d_ev,
            &gb_v_beta_d_ev,
            &ev_v_beta_d_ev,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        assert_eq!(e_shares.len(), n + m);

        // Paper-faithful CheckZero: verify under delta_b (D_ev), per 5_online.tex line 246.
        assert!(
            check_zero(&e_shares, &ev.delta_b),
            "honest Protocol 1 run must pass paper-faithful CheckZero under D_ev"
        );
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_1_ideal() {
        run_full_protocol_1(&IdealPreprocessingBackend);
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_1_uncompressed() {
        run_full_protocol_1(&UncompressedPreprocessingBackend);
    }

    /// Body of the Protocol 2 honest-run test, parameterized by preprocessing
    /// backend. Mirrors `run_full_protocol_1` but exercises the `_p2` garble
    /// path and the paper-faithful α/β input-wire check
    /// (`assemble_c_alpha_beta_shares_p2`) per `6_total.tex` lines 207–214.
    ///
    /// Both backends must:
    ///   - reconstruct D_gb output blocks matching the correct tensor product
    ///   - pass the D_ev consistency check (c_α, c_β == 0) under ev.delta_b
    ///
    /// `garble_final_p2`'s return type — `(Vec<Block>, Vec<Block>)` with no
    /// `bool` / `Vec<bool>` — statically enforces that the garbler never sends
    /// a masked wire value.
    fn run_full_protocol_2(backend: &dyn TensorPreprocessing) {
        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = backend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels (x = y = 0).
        let mut prep_rng = rand::rng();
        let labels = gb.prepare_input_labels(
            &mut prep_rng, 0, 0,
            &ev.alpha_auth_bit_shares, &ev.beta_auth_bit_shares,
        );
        ev.install_input_labels(labels);

        // Protocol 2 garble + evaluate sequence (wide ciphertexts).
        let (cl1, ct1) = gb.garble_first_half_p2();
        ev.evaluate_first_half_p2(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half_p2();
        ev.evaluate_second_half_p2(cl2, ct2);
        let (gb_d_gb_out, gb_d_ev_out) = gb.garble_final_p2();
        let ev_d_ev_out = ev.evaluate_final_p2();

        assert_eq!(gb_d_gb_out.len(), n * m);
        assert_eq!(gb_d_ev_out.len(), n * m);
        assert_eq!(ev_d_ev_out.len(), n * m);

        // ===========================================================================
        // PART A: D_gb correctness — same property the existing P1 test verifies but
        // applied to the `_p2` path. With `IdealPreprocessingBackend` the trusted
        // dealer is invoked with input=(0, 0), so masked_x = alpha and masked_y =
        // beta — therefore v_alpha = v_beta = 0 and v_gamma = 0. For honest
        // parties the combined D_gb output share for each (i, j) is the zero
        // block (key XOR mac == 0 means bit value 0).
        //
        // `evaluate_final_p2` writes the D_gb half into `ev.first_half_out`
        // (mirroring `evaluate_final` per the post-Plan-03 doc comment) and
        // `gb_d_gb_out[j * n + i]` equals `gb.first_half_out[(i, j)]`. So the
        // combined share is `gb_d_gb_out[idx] XOR ev.first_half_out[(i, j)]`.
        // Same correctness check approach as P1 (lines 487-503) with
        // expected_val=false everywhere.
        // ===========================================================================
        for j in 0..m {
            for i in 0..n {
                let idx = j * n + i;
                let combined = gb_d_gb_out[idx] ^ ev.first_half_out[(i, j)];
                assert_eq!(
                    combined,
                    Block::default(),
                    "P2-05: at ({},{}) expected v_gamma=0 (input=0 ideal preprocessing), got non-zero combined D_gb share",
                    i, j
                );
            }
        }

        // ===========================================================================
        // PART B: P2 consistency check — c_α / c_β assembled under delta_b
        // pass `check_zero`. Mirrors `run_full_protocol_1`'s input-encoding
        // setup: the paper's P1 e_a/e_b (5_online.tex:242–246) and P2 c_α/c_β
        // (6_total.tex:207–214) are algebraically identical — three-term XOR
        // [v D_ev] ⊕ [λ D_ev] ⊕ L·D_ev on tensor-gate input wires.
        // ===========================================================================
        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        // Honest input encoding (6_total.tex:191–198):
        //   gb sets [v D_ev]^gb := [l D_ev]^gb
        //   ev sets [v D_ev]^ev := [l D_ev]^ev XOR L · D_ev
        let gb_v_alpha_d_ev: Vec<Block> = gb.alpha_d_ev_shares.clone();
        let ev_v_alpha_d_ev: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_d_ev_shares[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_d_ev_shares[i]
            })
            .collect();
        let gb_v_beta_d_ev: Vec<Block> = gb.beta_d_ev_shares.clone();
        let ev_v_beta_d_ev: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_d_ev_shares[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_d_ev_shares[j]
            })
            .collect();

        let c_shares_p2 = assemble_c_alpha_beta_shares_p2(
            n, m,
            &gb_v_alpha_d_ev,
            &ev_v_alpha_d_ev,
            &gb_v_beta_d_ev,
            &ev_v_beta_d_ev,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        assert_eq!(c_shares_p2.len(), n + m);

        // Honest CheckZero under delta_b per 6_total.tex:214.
        assert!(
            check_zero(&c_shares_p2, &ev.delta_b),
            "honest Protocol 2 run must pass check_zero on c_α/c_β under D_ev (delta_b)"
        );
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_2_ideal() {
        run_full_protocol_2(&IdealPreprocessingBackend);
    }

    #[test]
    fn test_auth_tensor_product_full_protocol_2_uncompressed() {
        run_full_protocol_2(&UncompressedPreprocessingBackend);
    }

    #[test]
    fn test_gate_semantics_check_aborts_on_tampered_lambda() {
        // Regression for `assemble_gate_semantics_shares` (renamed from
        // `assemble_c_gamma_shares`). Tampering with lambda_gb (the garbler-emitted
        // [L_gamma]^gb) corrupts the gate-semantics identity v_α · v_β ⊕ v_γ at
        // index 0, so check_zero under delta_a must abort.
        //
        // NOTE: This is a test of the gate-semantics sanity check, NOT the paper's
        // P1 consistency check. The paper-faithful P1 abort is tested by
        // `test_protocol_1_e_input_wire_check_aborts_on_garbler_d_ev_tamper`.

        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels (x = y = 0).
        let mut prep_rng = rand::rng();
        let labels = gb.prepare_input_labels(
            &mut prep_rng, 0, 0,
            &ev.alpha_auth_bit_shares, &ev.beta_auth_bit_shares,
        );
        ev.install_input_labels(labels);

        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        let lambda_gb = gb.compute_lambda_gamma();

        // Tamper: flip ONE bit at index 0 of the garbler-emitted lambda vec.
        let mut tampered_lambda_gb = lambda_gb.clone();
        tampered_lambda_gb[0] ^= true;
        assert_ne!(tampered_lambda_gb, lambda_gb,
            "tampered vec must differ from honest vec");

        let l_gamma_combined_tampered = ev.compute_lambda_gamma(&tampered_lambda_gb);
        assert_eq!(l_gamma_combined_tampered.len(), n * m);

        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        let c_gamma_shares_tampered = assemble_gate_semantics_shares(
            n, m,
            &l_alpha_pub,
            &l_beta_pub,
            &l_gamma_combined_tampered,
            &gb,
            &ev,
        );

        assert!(
            !check_zero(&c_gamma_shares_tampered, &gb.delta_a),
            "tampered lambda_gb must cause gate-semantics check_zero to abort"
        );
    }

    #[test]
    fn test_protocol_1_e_input_wire_check_aborts_on_garbler_d_ev_tamper() {
        // Paper-faithful P1 CheckZero must abort when the garbler lies about its
        // [v_a D_ev]^gb during input encoding (5_online.tex line 214). We model this
        // by passing a tampered `gb_v_alpha_d_ev[0]` to the helper.
        //
        // The XOR is `gb.delta_a.as_block()`, whose LSB is 1 — so the tamper leaks
        // into `combined_e_block.lsb()` and breaks the e_a_bit = 0 invariant,
        // causing `check_zero` under delta_b to return false.
        //
        // We additionally verify that the gate-semantics check (renamed
        // `assemble_gate_semantics_shares` under delta_a) is NOT sensitive to this
        // class of tamper — concretely demonstrating that the new helper catches a
        // tamper the old check misses.

        let n = 4;
        let m = 3;

        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);

        // Phase 1.2 / BUG-02: install garble-time input labels (x = y = 0).
        let mut prep_rng = rand::rng();
        let labels = gb.prepare_input_labels(
            &mut prep_rng, 0, 0,
            &ev.alpha_auth_bit_shares, &ev.beta_auth_bit_shares,
        );
        ev.install_input_labels(labels);

        let (cl1, ct1) = gb.garble_first_half();
        ev.evaluate_first_half(cl1, ct1);
        let (cl2, ct2) = gb.garble_second_half();
        ev.evaluate_second_half(cl2, ct2);
        gb.garble_final();
        ev.evaluate_final();

        let l_alpha_pub: Vec<bool> = (0..n)
            .map(|i| gb.alpha_auth_bit_shares[i].value ^ ev.alpha_auth_bit_shares[i].value)
            .collect();
        let l_beta_pub: Vec<bool> = (0..m)
            .map(|j| gb.beta_auth_bit_shares[j].value ^ ev.beta_auth_bit_shares[j].value)
            .collect();

        // Honest [v_a D_ev]^gb / [v_b D_ev]^gb starting points.
        let mut gb_v_alpha_d_ev: Vec<Block> = gb.alpha_d_ev_shares.clone();
        let gb_v_beta_d_ev: Vec<Block> = gb.beta_d_ev_shares.clone();

        // TAMPER: corrupt gb's [v_a D_ev]^gb at i=0 by XORing in delta_a (LSB=1).
        // This simulates a malicious garbler whose announced share deviates from
        // what input encoding requires.
        gb_v_alpha_d_ev[0] ^= *gb.delta_a.as_block();

        // ev's [v D_ev]^ev shares per honest input encoding.
        let ev_v_alpha_d_ev: Vec<Block> = (0..n)
            .map(|i| if l_alpha_pub[i] {
                ev.alpha_d_ev_shares[i] ^ *ev.delta_b.as_block()
            } else {
                ev.alpha_d_ev_shares[i]
            })
            .collect();
        let ev_v_beta_d_ev: Vec<Block> = (0..m)
            .map(|j| if l_beta_pub[j] {
                ev.beta_d_ev_shares[j] ^ *ev.delta_b.as_block()
            } else {
                ev.beta_d_ev_shares[j]
            })
            .collect();

        let e_shares_tampered = assemble_e_input_wire_shares_p1(
            n, m,
            &gb_v_alpha_d_ev,
            &ev_v_alpha_d_ev,
            &gb_v_beta_d_ev,
            &ev_v_beta_d_ev,
            &l_alpha_pub,
            &l_beta_pub,
            &gb,
            &ev,
        );

        // Paper-faithful CheckZero MUST abort for the tampered run.
        assert!(
            !check_zero(&e_shares_tampered, &ev.delta_b),
            "tampered gb_v_alpha_d_ev must cause paper-faithful CheckZero to abort"
        );

        // Cross-check: the OLD gate-semantics check is insensitive to this tamper
        // because it doesn't read `*_d_ev_shares` blocks at all.
        let lambda_gb = gb.compute_lambda_gamma();
        let l_gamma_combined = ev.compute_lambda_gamma(&lambda_gb);
        let gate_sem_shares = assemble_gate_semantics_shares(
            n, m,
            &l_alpha_pub,
            &l_beta_pub,
            &l_gamma_combined,
            &gb,
            &ev,
        );
        assert!(
            check_zero(&gate_sem_shares, &gb.delta_a),
            "gate-semantics check should still pass for a D_ev-block tamper \
             (it doesn't read d_ev_shares, so the tamper is invisible to it)"
        );
    }

}