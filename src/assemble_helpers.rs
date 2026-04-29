//! Input-wire CheckZero assembly helpers (Protocol 1 + Protocol 2).
//!
//! These are crate-internal helpers used by the online protocol body and by
//! the `benches/benchmarks.rs` Criterion target. They are exposed externally
//! only through the feature-gated `crate::bench_internals` re-export module
//! (TD-01, Phase 3.1) — the helpers themselves are not part of the stable
//! public API.

use crate::auth_tensor_eval::AuthTensorEval;
use crate::auth_tensor_gen::AuthTensorGen;
use crate::block::Block;

/// Paper-faithful Protocol 1 input-wire CheckZero assembly under `delta_ev`.
///
/// Implements the Protocol 1 consistency check per `5_online.tex` §240–246
/// (and `6_total.tex` §215–222 for P2; same formula). For each tensor-gate
/// input wire, each party computes its share-block of:
///
/// ```text
///   [e_a[i] D_ev] := [a[i] D_ev] ⊕ [l_a[i] D_ev] ⊕ (a[i] ⊕ l_a[i]) D_ev
///   [e_b[j] D_ev] := [b[j] D_ev] ⊕ [l_b[j] D_ev] ⊕ (b[j] ⊕ l_b[j]) D_ev
/// ```
///
/// For honest parties this reduces to zero by Lemma `lem:protocol1-correctness`
/// (line 297) — i.e., `gen_block[k] == eval_block[k]` per index. CheckZero
/// (paper line 246) detects deviation: a malicious garbler that lies about
/// `[v D_ev]^gb` makes the per-index pair unequal.
///
/// Returns `(gb_blocks, ev_blocks)` — each length `n + m`, layout
/// `[e_a[0..n], e_b[0..m]]`. Pass to `block_check_zero` (full-block equality)
/// or hash each side via `block_hash_check_zero` for the paper-faithful
/// `H({V_w})` digest semantics.
///
/// # SIMULATION ONLY
///
/// Takes both parties' state in-process; in a real two-party run each party
/// would compute its own block vector locally from its own `_eval` fields and
/// `[v D_ev]` shares, then exchange digests. To make the simulation sensitive
/// to a malicious garbler that lies about `[v_a D_ev]^gb`, the helper accepts
/// `[v_a D_ev]^gb` as an explicit parameter rather than aliasing it to
/// `gb.alpha_dev[i]`. Honest callers pass `gb.alpha_dev.clone()`; negative
/// tests pass tampered Blocks.
///
/// # Detection power vs the prior `assemble_e_input_wire_shares_p1`
///
/// The prior helper extracted `combined_block.lsb()` and emitted
/// `Vec<AuthBitShare>` for `check_zero` consumption — detection was LSB-only
/// (caught only tampers whose XOR delta has LSB=1). This helper emits the
/// full per-party blocks so `block_check_zero` can detect any non-zero
/// combined block. Aligns with paper §246 (`H({V_w})` digest comparison).
///
/// # Inputs (unchanged from prior helper)
/// - `n`, `m`: input vector lengths.
/// - `gb_v_alpha_dev` / `ev_v_alpha_dev`: `[v_a D_ev]` shares (length n).
///   Honest: gb's = `gb.alpha_dev`; ev's = `ev.alpha_dev[i] ⊕ L_a·δ_ev`.
/// - `gb_v_beta_dev` / `ev_v_beta_dev`: same for β (length m).
/// - `l_alpha_pub` / `l_beta_pub`: announced masked-input vectors
///   `vec a ⊕ vec l_a`, `vec b ⊕ vec l_b`.
/// - `gb`, `ev`: party state for `_eval` Block fields.
#[allow(clippy::too_many_arguments)]
pub fn assemble_e_input_wire_blocks_p1(
    n: usize,
    m: usize,
    gb_v_alpha_dev: &[Block],
    ev_v_alpha_dev: &[Block],
    gb_v_beta_dev: &[Block],
    ev_v_beta_dev: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> (Vec<Block>, Vec<Block>) {
    assert_eq!(gb_v_alpha_dev.len(), n);
    assert_eq!(ev_v_alpha_dev.len(), n);
    assert_eq!(gb_v_beta_dev.len(),  m);
    assert_eq!(ev_v_beta_dev.len(),  m);
    assert_eq!(l_alpha_pub.len(), n);
    assert_eq!(l_beta_pub.len(),  m);
    assert_eq!(gb.alpha_dev.len(), n);
    assert_eq!(gb.beta_dev.len(),  m);
    assert_eq!(ev.alpha_dev.len(), n);
    assert_eq!(ev.beta_dev.len(),  m);

    let mut gb_blocks: Vec<Block> = Vec::with_capacity(n + m);
    let mut ev_blocks: Vec<Block> = Vec::with_capacity(n + m);

    // e_a per α-input wire: paper §242
    //   gb's share-block = [v_a D_ev]^gb ⊕ [l_a D_ev]^gb
    //   ev's share-block = [v_a D_ev]^ev ⊕ [l_a D_ev]^ev ⊕ L_a·D_ev
    for i in 0..n {
        let l_a_correction = if l_alpha_pub[i] {
            *ev.delta_ev.as_block()
        } else {
            Block::default()
        };
        gb_blocks.push(gb_v_alpha_dev[i] ^ gb.alpha_dev[i]);
        ev_blocks.push(ev_v_alpha_dev[i] ^ ev.alpha_dev[i] ^ l_a_correction);
    }

    // e_b per β-input wire: symmetric.
    for j in 0..m {
        let l_b_correction = if l_beta_pub[j] {
            *ev.delta_ev.as_block()
        } else {
            Block::default()
        };
        gb_blocks.push(gb_v_beta_dev[j] ^ gb.beta_dev[j]);
        ev_blocks.push(ev_v_beta_dev[j] ^ ev.beta_dev[j] ^ l_b_correction);
    }

    (gb_blocks, ev_blocks)
}

/// Paper-faithful Protocol 2 input-wire CheckZero assembly — alias for the P1
/// routine.
///
/// Per `6_total.tex` §215–222, the P2 consistency check builds:
/// ```text
///   [c_α D_ev] := [v_α D_ev] ⊕ [l_α D_ev] ⊕ L_α · D_ev    (length n)
///   [c_β D_ev] := [v_β D_ev] ⊕ [l_β D_ev] ⊕ L_β · D_ev    (length m)
/// ```
/// Algebraically identical to P1's `e_a / e_b`. The paper uses different
/// variable names (`c_α/c_β` in P2 vs `e_a/e_b` in P1) to match its narrative;
/// this thin alias preserves the paper-mapped name at P2 call sites without
/// duplicating logic.
///
/// AUDIT-2.3 C2 — alias coupling note: the P1/P2 algebraic equivalence holds
/// only because both protocols assemble the same three-term XOR over the same
/// `[v D_ev]` / `[l D_ev]` / `L · D_ev` shares. If P2's input-encoding spec
/// ever diverges from P1's (e.g., different L-vector semantics, additional
/// per-wire correction term), this alias must be split — silent breakage is
/// the failure mode since the static signatures stay identical. Keep this
/// note adjacent to any future P2 input-encoding edit.
#[allow(clippy::too_many_arguments)]
pub fn assemble_c_alpha_beta_blocks_p2(
    n: usize,
    m: usize,
    gb_v_alpha_dev: &[Block],
    ev_v_alpha_dev: &[Block],
    gb_v_beta_dev: &[Block],
    ev_v_beta_dev: &[Block],
    l_alpha_pub: &[bool],
    l_beta_pub: &[bool],
    gb: &AuthTensorGen,
    ev: &AuthTensorEval,
) -> (Vec<Block>, Vec<Block>) {
    assemble_e_input_wire_blocks_p1(
        n, m,
        gb_v_alpha_dev,
        ev_v_alpha_dev,
        gb_v_beta_dev,
        ev_v_beta_dev,
        l_alpha_pub,
        l_beta_pub,
        gb,
        ev,
    )
}
