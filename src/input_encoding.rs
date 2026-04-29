//! Input encoding phase: sits between preprocessing and garbling.
//!
//! Generates input wire labels for both parties' input bits and produces the
//! cleartext masked-input bits (`d_x[i] = x_i ⊕ α_i`, `d_y[j] = y_j ⊕ β_j`)
//! used as GGM-tree choice bits during garble/evaluate.
//!
//! # Math (per wire i, mirror for y/β/m)
//!
//! Let `δ_gb` be gen's global key. Per `gen_auth_bit` IT-MAC layout, the
//! preprocessing exit boundary supplies the `_gen` Block-form components of
//! the α sharing under δ_gb:
//!
//!   gb.alpha_dgb[i]  = K_b ⊕ a_i·δ_gb
//!   ev.alpha_dgb[i] = M_b = K_b ⊕ b_i·δ_gb
//!   (XOR reveals α_i·δ_gb)
//!
//! Input encoding samples a fresh wire-label sharing of `x_i` under δ_gb:
//!
//!   K_x  ←  random Block, LSB cleared
//!   M_x  =  K_x ⊕ x_i·δ_gb               (held by gen, written to gb.x_dgb[i])
//!   eval receives K_x                    (written to ev.x_dgb[i])
//!
//! XOR of components yields `x_i·δ_gb`. The masked-input wire-label sharing
//! `(x_i ⊕ α_i)·δ_gb` is the linear sum of the two sharings:
//!
//!   gb.masked_x_dgb[i]  = M_x ⊕ gb.alpha_dgb[i]  ⊕ lsb_shift
//!   ev.masked_x_dgb[i] = K_x ⊕ ev.alpha_dgb[i] ⊕ lsb_shift
//!
//! The `lsb_shift = (x_i ⊕ a_i)·δ_gb` is applied to **both** sides so it
//! cancels in the combined XOR sum, while landing the LSBs on (0, d_i) per
//! the GGM-tree convention (gen's seed LSB=0, eval's seed LSB=d_i).
//!
//! # Cleartext masked bits
//!
//! `d_i = x_i ⊕ a_i ⊕ b_i = x_i ⊕ α_i`. Each party's α-bit (`a_i` for gen,
//! `b_i` for eval) is recovered locally from its own Block-form components
//! via `LSB(party.alpha_dev[i] ⊕ party.alpha_dgb[i])` (inverse of
//! `derive_sharing_blocks`). In a real two-party deployment the gen→eval
//! "interaction" is gen sending `(x_i ⊕ a_i)` (its share of d_i); eval
//! XORs in its own `b_i` to recover `d_i`. The cleartext masked-bit
//! sharing is asymmetric:
//!   gb.gb_masked_x_bits  = vec![false; n]   (0-vec; gen covers both branches)
//!   ev.ev_masked_x_bits = vec [d_x_0, d_x_1, ...]
//! XOR of components yields the cleartext d-vector.

use rand::{CryptoRng, Rng};

use crate::auth_tensor_eval::AuthTensorEval;
use crate::auth_tensor_gen::AuthTensorGen;
use crate::block::Block;

/// Encode gen's input vectors `x` (length n bits) and `y` (length m bits)
/// into IT-MAC wire-label sharings under δ_gb, and populate both parties'
/// post-preprocessing state needed for the garble / evaluate phase.
///
/// Preconditions:
/// - Both structs must already hold preprocessing output: the Block-form
///   `alpha_dev` / `alpha_dgb` (length n) and `beta_dev` / `beta_dgb`
///   (length m) sharings.
/// - `gb.delta_gb` is the garbler's key (also implicit in `*_gen` field
///   construction).
///
/// Side effects on `gen`:
/// - `x_dgb` (length n), `y_dgb` (m): gen's `mac` halves of input wire-label
///   sharings under δ_gb. LSB of x_dgb[i] equals `x_i`; same for y_dgb.
/// - `masked_x_dgb` (n), `masked_y_dgb` (m): gen's halves of `(x_i ⊕ α_i)·δ_gb`
///   sharings. LSBs land on 0 (GGM-tree convention).
/// - `masked_x_bits = vec![false; n]`, `masked_y_bits = vec![false; m]`.
///   See module-level doc for the asymmetric cleartext masked-bit sharing.
///
/// Side effects on `eval`:
/// - `x_dgb` (n), `y_dgb` (m): eval's `key` halves of input wire-label
///   sharings under δ_gb (received from gen).
/// - `masked_x_dgb` (n), `masked_y_dgb` (m): eval's halves. LSBs land on
///   `d_i` (GGM-tree choice bit).
/// - `masked_x_bits` / `masked_y_bits`: cleartext `d_x` / `d_y` vectors.
///
/// # Bit-packing convention
/// `x` and `y` are bit-packed in a `usize`: `x_i = (x >> i) & 1`. For
/// `n > usize::BITS` (typically 64) the right-shift saturates and bits past
/// index 63 read as zero. The asserts below enforce that any non-zero
/// input fits in `usize::BITS`; zero inputs are permitted at any `n` since
/// `(0 >> i) & 1 == 0` for all `i` — bench harnesses that exercise wide
/// matrices with zero inputs (correctness verified by the lib-level tests
/// at smaller sizes) thus remain unaffected.
///
/// # Panics
/// Panics if `gb.alpha_dev.len()` (the source of n) doesn't match
/// `gb.alpha_dgb` / `ev.alpha_dev` / `ev.alpha_dgb`. Same for β/m.
/// Panics if `x != 0` with `n > usize::BITS` (or `y != 0` with `m > usize::BITS`).
pub fn encode_inputs<R: Rng + CryptoRng>(
    gb: &mut AuthTensorGen,
    ev: &mut AuthTensorEval,
    x: usize,
    y: usize,
    rng: &mut R,
) {
    let n = gb.alpha_dgb.len();
    let m = gb.beta_dgb.len();

    assert!(x == 0 || n <= usize::BITS as usize,
        "encode_inputs: n={} exceeds usize::BITS={} with non-zero x={:#x}; bit-packed `x` would silently truncate",
        n, usize::BITS, x);
    assert!(y == 0 || m <= usize::BITS as usize,
        "encode_inputs: m={} exceeds usize::BITS={} with non-zero y={:#x}; bit-packed `y` would silently truncate",
        m, usize::BITS, y);

    assert_eq!(gb.alpha_dev.len(), n,
        "encode_inputs: gb.alpha_dev must be populated by preprocessing; len={} expected={}",
        gb.alpha_dev.len(), n);
    assert_eq!(gb.beta_dev.len(), m,
        "encode_inputs: gb.beta_dev must be populated; len={} expected={}",
        gb.beta_dev.len(), m);
    assert_eq!(ev.alpha_dgb.len(), n,
        "encode_inputs: ev.alpha_dgb must be populated; len={} expected={}",
        ev.alpha_dgb.len(), n);
    assert_eq!(ev.alpha_dev.len(), n,
        "encode_inputs: ev.alpha_dev must be populated; len={} expected={}",
        ev.alpha_dev.len(), n);
    assert_eq!(ev.beta_dgb.len(), m,
        "encode_inputs: ev.beta_dgb must be populated; len={} expected={}",
        ev.beta_dgb.len(), m);
    assert_eq!(ev.beta_dev.len(), m,
        "encode_inputs: ev.beta_dev must be populated; len={} expected={}",
        ev.beta_dev.len(), m);

    let delta_a_block = *gb.delta_gb.as_block();

    gb.x_dgb = Vec::with_capacity(n);
    gb.masked_x_dgb = Vec::with_capacity(n);
    ev.x_dgb = Vec::with_capacity(n);
    ev.masked_x_dgb = Vec::with_capacity(n);
    let mut d_x: Vec<bool> = Vec::with_capacity(n);

    for i in 0..n {
        let x_i = ((x >> i) & 1) != 0;
        // Bit-recovery: own party's local α-bit = LSB(party._eval ^ party._gen)
        // (inverse of derive_sharing_blocks). Each party reads its OWN bit from
        // its OWN state — gb.alpha_*[i] for a_i, ev.alpha_*[i] for b_i.
        let a_i = (gb.alpha_dev[i] ^ gb.alpha_dgb[i]).lsb();
        let b_i = (ev.alpha_dev[i] ^ ev.alpha_dgb[i]).lsb();
        let d_i = x_i ^ a_i ^ b_i;

        // Sample a fresh wire-label sharing of x_i under δ_gb.
        let mut input_key = Block::random(rng);
        input_key.set_lsb(false);
        let input_mac = if x_i { input_key ^ delta_a_block } else { input_key };

        // lsb_shift = (x_i ⊕ a_i)·δ_gb -- gen's local knowledge (its share of
        // d_i lifted to a Block). Applied to both sides; cancels in the sum
        // while flipping the LSBs to (0, d_i) per the GGM-tree convention.
        let lsb_shift = if x_i ^ a_i { delta_a_block } else { Block::ZERO };

        gb.x_dgb.push(input_mac);
        ev.x_dgb.push(input_key);
        gb.masked_x_dgb.push(input_mac ^ gb.alpha_dgb[i] ^ lsb_shift);
        ev.masked_x_dgb.push(input_key ^ ev.alpha_dgb[i] ^ lsb_shift);
        d_x.push(d_i);
    }

    gb.y_dgb = Vec::with_capacity(m);
    gb.masked_y_dgb = Vec::with_capacity(m);
    ev.y_dgb = Vec::with_capacity(m);
    ev.masked_y_dgb = Vec::with_capacity(m);
    let mut d_y: Vec<bool> = Vec::with_capacity(m);

    for j in 0..m {
        let y_j = ((y >> j) & 1) != 0;
        // Bit-recovery (β analog) — see α loop.
        let beta_a_j = (gb.beta_dev[j] ^ gb.beta_dgb[j]).lsb();
        let beta_b_j = (ev.beta_dev[j] ^ ev.beta_dgb[j]).lsb();
        let d_j = y_j ^ beta_a_j ^ beta_b_j;

        let mut input_key = Block::random(rng);
        input_key.set_lsb(false);
        let input_mac = if y_j { input_key ^ delta_a_block } else { input_key };

        let lsb_shift = if y_j ^ beta_a_j { delta_a_block } else { Block::ZERO };

        gb.y_dgb.push(input_mac);
        ev.y_dgb.push(input_key);
        gb.masked_y_dgb.push(input_mac ^ gb.beta_dgb[j] ^ lsb_shift);
        ev.masked_y_dgb.push(input_key ^ ev.beta_dgb[j] ^ lsb_shift);
        d_y.push(d_j);
    }

    // Cleartext masked-bit sharing: gen's component is the 0-vec (gen
    // covers both GGM branches); eval's component is the d-vector.
    gb.gb_masked_x_bits = vec![false; n];
    gb.gb_masked_y_bits = vec![false; m];
    ev.ev_masked_x_bits = d_x;
    ev.ev_masked_y_bits = d_y;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};

    #[test]
    #[should_panic(expected = "exceeds usize::BITS")]
    fn encode_inputs_panics_when_n_exceeds_usize_bits_with_nonzero_x() {
        // n = 65 makes `(x >> i) & 1` saturate for i >= 64, silently zeroing
        // high-index bits. The assert must catch this at the entry boundary
        // when x != 0 (zero inputs are bit-packing-invariant; benches that use
        // wide matrices with x=y=0 are explicitly permitted).
        let n = (usize::BITS as usize) + 1;
        let m = 1;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        let mut rng = rand::rng();
        encode_inputs(&mut gb, &mut ev, 0b1, 0, &mut rng);
    }
}
