//! Input encoding phase: sits between preprocessing and garbling.
//!
//! Generates input wire labels for both parties' input bits and produces the
//! cleartext masked-input bits (`d_x[i] = x_i ⊕ α_i`, `d_y[j] = y_j ⊕ β_j`)
//! used as GGM-tree choice bits during garble/evaluate.
//!
//! # Math (per wire i, mirror for y/β/m)
//!
//! Let `δ_a` be gen's global key. Per `gen_auth_bit` IT-MAC layout, the
//! preprocessing exit boundary supplies the `_gen` Block-form components of
//! the α sharing under δ_a:
//!
//!   gar.alpha_gen[i]  = K_b ⊕ a_i·δ_a
//!   ev.alpha_gen[i] = M_b = K_b ⊕ b_i·δ_a
//!   (XOR reveals α_i·δ_a)
//!
//! Input encoding samples a fresh wire-label sharing of `x_i` under δ_a:
//!
//!   K_x  ←  random Block, LSB cleared
//!   M_x  =  K_x ⊕ x_i·δ_a               (held by gen, written to gar.x_gen[i])
//!   eval receives K_x                    (written to ev.x_gen[i])
//!
//! XOR of components yields `x_i·δ_a`. The masked-input wire-label sharing
//! `(x_i ⊕ α_i)·δ_a` is the linear sum of the two sharings:
//!
//!   gar.masked_x_gen[i]  = M_x ⊕ gar.alpha_gen[i]  ⊕ lsb_shift
//!   ev.masked_x_gen[i] = K_x ⊕ ev.alpha_gen[i] ⊕ lsb_shift
//!
//! The `lsb_shift = (x_i ⊕ a_i)·δ_a` is applied to **both** sides so it
//! cancels in the combined XOR sum, while landing the LSBs on (0, d_i) per
//! the GGM-tree convention (gen's seed LSB=0, eval's seed LSB=d_i).
//!
//! # Cleartext masked bits
//!
//! `d_i = x_i ⊕ a_i ⊕ b_i = x_i ⊕ α_i`. Each party's α-bit (`a_i` for gen,
//! `b_i` for eval) is recovered locally from its own Block-form components
//! via `LSB(party.alpha_eval[i] ⊕ party.alpha_gen[i])` (inverse of
//! `derive_sharing_blocks`). In a real two-party deployment the gen→eval
//! "interaction" is gen sending `(x_i ⊕ a_i)` (its share of d_i); eval
//! XORs in its own `b_i` to recover `d_i`. The cleartext masked-bit
//! sharing is asymmetric:
//!   gar.masked_x_bits  = vec![false; n]   (0-vec; gen covers both branches)
//!   ev.masked_x_bits = vec [d_x_0, d_x_1, ...]
//! XOR of components yields the cleartext d-vector.

use rand::{CryptoRng, Rng};

use crate::auth_tensor_eval::AuthTensorEval;
use crate::auth_tensor_gen::AuthTensorGen;
use crate::block::Block;

/// Encode gen's input vectors `x` (length n bits) and `y` (length m bits)
/// into IT-MAC wire-label sharings under δ_a, and populate both parties'
/// post-preprocessing state needed for the garble / evaluate phase.
///
/// Preconditions:
/// - Both structs must already hold preprocessing output: the Block-form
///   `alpha_eval` / `alpha_gen` (length n) and `beta_eval` / `beta_gen`
///   (length m) sharings.
/// - `gar.delta_a` is the garbler's key (also implicit in `*_gen` field
///   construction).
///
/// Side effects on `gen`:
/// - `x_gen` (length n), `y_gen` (m): gen's `mac` halves of input wire-label
///   sharings under δ_a. LSB of x_gen[i] equals `x_i`; same for y_gen.
/// - `masked_x_gen` (n), `masked_y_gen` (m): gen's halves of `(x_i ⊕ α_i)·δ_a`
///   sharings. LSBs land on 0 (GGM-tree convention).
/// - `masked_x_bits = vec![false; n]`, `masked_y_bits = vec![false; m]`.
///   See module-level doc for the asymmetric cleartext masked-bit sharing.
///
/// Side effects on `eval`:
/// - `x_gen` (n), `y_gen` (m): eval's `key` halves of input wire-label
///   sharings under δ_a (received from gen).
/// - `masked_x_gen` (n), `masked_y_gen` (m): eval's halves. LSBs land on
///   `d_i` (GGM-tree choice bit).
/// - `masked_x_bits` / `masked_y_bits`: cleartext `d_x` / `d_y` vectors.
///
/// # Bit-packing convention
/// `x` and `y` are bit-packed in a `usize`: `x_i = (x >> i) & 1`. Therefore
/// `n` and `m` MUST be `<= usize::BITS` (64 on 64-bit targets) — beyond
/// that the right-shift would silently saturate and bits past index 63
/// would read as zero. The asserts below enforce this.
///
/// # Panics
/// Panics if `gar.alpha_eval.len()` (the source of n) doesn't match
/// `gar.alpha_gen` / `ev.alpha_eval` / `ev.alpha_gen`. Same for β/m.
/// Panics if `n > usize::BITS` or `m > usize::BITS`.
pub fn encode_inputs<R: Rng + CryptoRng>(
    gar: &mut AuthTensorGen,
    ev: &mut AuthTensorEval,
    x: usize,
    y: usize,
    rng: &mut R,
) {
    let n = gar.alpha_gen.len();
    let m = gar.beta_gen.len();

    assert!(n <= usize::BITS as usize,
        "encode_inputs: n={} exceeds usize::BITS={}; bit-packed `x` would silently truncate",
        n, usize::BITS);
    assert!(m <= usize::BITS as usize,
        "encode_inputs: m={} exceeds usize::BITS={}; bit-packed `y` would silently truncate",
        m, usize::BITS);

    assert_eq!(gar.alpha_eval.len(), n,
        "encode_inputs: gar.alpha_eval must be populated by preprocessing; len={} expected={}",
        gar.alpha_eval.len(), n);
    assert_eq!(gar.beta_eval.len(), m,
        "encode_inputs: gar.beta_eval must be populated; len={} expected={}",
        gar.beta_eval.len(), m);
    assert_eq!(ev.alpha_gen.len(), n,
        "encode_inputs: ev.alpha_gen must be populated; len={} expected={}",
        ev.alpha_gen.len(), n);
    assert_eq!(ev.alpha_eval.len(), n,
        "encode_inputs: ev.alpha_eval must be populated; len={} expected={}",
        ev.alpha_eval.len(), n);
    assert_eq!(ev.beta_gen.len(), m,
        "encode_inputs: ev.beta_gen must be populated; len={} expected={}",
        ev.beta_gen.len(), m);
    assert_eq!(ev.beta_eval.len(), m,
        "encode_inputs: ev.beta_eval must be populated; len={} expected={}",
        ev.beta_eval.len(), m);

    let delta_a_block = *gar.delta_a.as_block();

    gar.x_gen = Vec::with_capacity(n);
    gar.masked_x_gen = Vec::with_capacity(n);
    ev.x_gen = Vec::with_capacity(n);
    ev.masked_x_gen = Vec::with_capacity(n);
    let mut d_x: Vec<bool> = Vec::with_capacity(n);

    for i in 0..n {
        let x_i = ((x >> i) & 1) != 0;
        // Bit-recovery: own party's local α-bit = LSB(party._eval ^ party._gen)
        // (inverse of derive_sharing_blocks). Each party reads its OWN bit from
        // its OWN state — gar.alpha_*[i] for a_i, ev.alpha_*[i] for b_i.
        let a_i = (gar.alpha_eval[i] ^ gar.alpha_gen[i]).lsb();
        let b_i = (ev.alpha_eval[i] ^ ev.alpha_gen[i]).lsb();
        let d_i = x_i ^ a_i ^ b_i;

        // Sample a fresh wire-label sharing of x_i under δ_a.
        let mut input_key = Block::random(rng);
        input_key.set_lsb(false);
        let input_mac = if x_i { input_key ^ delta_a_block } else { input_key };

        // lsb_shift = (x_i ⊕ a_i)·δ_a -- gen's local knowledge (its share of
        // d_i lifted to a Block). Applied to both sides; cancels in the sum
        // while flipping the LSBs to (0, d_i) per the GGM-tree convention.
        let lsb_shift = if x_i ^ a_i { delta_a_block } else { Block::ZERO };

        gar.x_gen.push(input_mac);
        ev.x_gen.push(input_key);
        gar.masked_x_gen.push(input_mac ^ gar.alpha_gen[i] ^ lsb_shift);
        ev.masked_x_gen.push(input_key ^ ev.alpha_gen[i] ^ lsb_shift);
        d_x.push(d_i);
    }

    gar.y_gen = Vec::with_capacity(m);
    gar.masked_y_gen = Vec::with_capacity(m);
    ev.y_gen = Vec::with_capacity(m);
    ev.masked_y_gen = Vec::with_capacity(m);
    let mut d_y: Vec<bool> = Vec::with_capacity(m);

    for j in 0..m {
        let y_j = ((y >> j) & 1) != 0;
        // Bit-recovery (β analog) — see α loop.
        let beta_a_j = (gar.beta_eval[j] ^ gar.beta_gen[j]).lsb();
        let beta_b_j = (ev.beta_eval[j] ^ ev.beta_gen[j]).lsb();
        let d_j = y_j ^ beta_a_j ^ beta_b_j;

        let mut input_key = Block::random(rng);
        input_key.set_lsb(false);
        let input_mac = if y_j { input_key ^ delta_a_block } else { input_key };

        let lsb_shift = if y_j ^ beta_a_j { delta_a_block } else { Block::ZERO };

        gar.y_gen.push(input_mac);
        ev.y_gen.push(input_key);
        gar.masked_y_gen.push(input_mac ^ gar.beta_gen[j] ^ lsb_shift);
        ev.masked_y_gen.push(input_key ^ ev.beta_gen[j] ^ lsb_shift);
        d_y.push(d_j);
    }

    // Cleartext masked-bit sharing: gen's component is the 0-vec (gen
    // covers both GGM branches); eval's component is the d-vector.
    gar.masked_x_bits = vec![false; n];
    gar.masked_y_bits = vec![false; m];
    ev.masked_x_bits = d_x;
    ev.masked_y_bits = d_y;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};

    #[test]
    #[should_panic(expected = "exceeds usize::BITS")]
    fn encode_inputs_panics_when_n_exceeds_usize_bits() {
        // n = 65 makes `(x >> i) & 1` saturate for i >= 64, silently zeroing
        // high-index bits. The assert must catch this at the entry boundary.
        let n = (usize::BITS as usize) + 1;
        let m = 1;
        let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1);
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        let mut rng = rand::rng();
        encode_inputs(&mut gar, &mut ev, 0, 0, &mut rng);
    }
}
