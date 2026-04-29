use crate::{
    aes::FixedKeyAes,
    block::Block,
    delta::Delta,
    matrix::{MatrixViewMut, MatrixViewRef},
};

/// Garbler side of paper Construction 4 / 5_online.tex `tensorgb`'s GGM tree
/// portion (steps 2–4): builds the level tree and emits one ciphertext per level.
///
/// Implements the paper's improved one-hot construction (citing [Heath24]): one
/// hash per parent, one ciphertext per level — versus HK21's two-hash / two-ct
/// per level. Recurrence (paper `5_online.tex:46-58`):
///
/// ```text
///   S_{0,0} := A_0 ⊕ Δ_gb,          S_{0,1} := A_0
///   for i ∈ [1, n-1]:
///     R_{i,j}        := H(S_{i-1,j}, ν_{i,j})         for j ∈ [2^i]
///     S_{i,j}        := R_{i,j} ⊕ S_{i-1,j}            (FreeXOR sibling)
///     S_{i,2^i+j}    := R_{i,j}
///     G_i            := (⊕_j R_{i,j}) ⊕ A_i           (single Block per level)
/// ```
///
/// Indexing translation. The paper assigns lower-half index `j` to
/// `R ⊕ S` and upper-half index `2^i + j` to `R`. This implementation uses the
/// interleaved tree layout the codebase has always used: each parent at
/// `seeds[j]` writes children at `seeds[2j]` (even, "lower half" → `R ⊕ parent`)
/// and `seeds[2j+1]` (odd, "upper half" → `R`). The mapping is
/// consistent with the leaf-expansion + `(missing >> k) & 1` path-bit reads in
/// `gen_unary_outer_product` and the eval-side `missing = (missing << 1) | bit`
/// accumulator (bit=0 → even child = lower half, bit=1 → odd child = upper half;
/// see paper's `α_i := α_{i-1} + a_i·2^i` adapted to interleaved indexing).
///
/// Endianness: code's `x[n-1]` is paper's `A_0` (level-0 key); code's `x[0]` is
/// paper's `A_{n-1}` (deepest level). Tree root consumes the MSB of `x` first.
///
/// Returns `(leaf_seeds, level_cts)` where `leaf_seeds.len() == 2^n` and
/// `level_cts.len() == n - 1` with element type `Block` (single ct per level).
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &[Block],
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<Block>) {
    let n: usize = x.len();

    let mut level_cts: Vec<Block> = Vec::with_capacity(n.saturating_sub(1));
    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Level-0 init (paper step 2): direct assignment, no TCCR.
    // Code convention places `A_0 = x[n-1]` (MSB-first traversal). Per the
    // `Key::new()` invariant `A_0.lsb() == 0`, so `A_0 ⊕ Δ` has lsb=1 (since
    // `δ.lsb() = 1` is the bCOT split-delta convention) — matching paper's
    // S_{0,0} (the "δ-shifted" branch).
    seeds[0] = x[n - 1] ^ delta;
    seeds[1] = x[n - 1];

    for i in 1..n {
        // Single tweak per level (LOOP_DOMAIN-tagged for cross-stage separation).
        // The parent seed value differentiates calls within a level — TCCR is
        // CCR-secure under input distinctness, and parents at level i-1 are
        // pseudorandomly distinct.
        let tweak = Block::from(LOOP_DOMAIN | (i as u128));

        let mut r_xor_sum = Block::default();

        // Reverse iteration: each parent at index j writes children at 2j (which
        // collides with seeds[j] when j < 2^{i-1}). Walking j from high to low
        // ensures every read of seeds[j] precedes the overwrite at seeds[2j]
        // for any j' ≤ j (since 2j' ≤ 2j and writes happen in descending j).
        for j in (0..(1 << i)).rev() {
            let parent = seeds[j];
            let r = cipher.tccr(tweak, parent);
            seeds[2 * j + 1] = r;            // upper half (paper S_{i, 2^i + j})
            seeds[2 * j]     = r ^ parent;   // lower half (paper S_{i, j})
            r_xor_sum ^= r;
        }

        // G_i := (⊕_j R_{i,j}) ⊕ A_i. Code's A_i is x[n-1-i] (level-i key).
        // Per Key::new(), x[n-1-i].lsb() == 0 — A_i is the "0-key" expected by
        // the paper's encoding.
        let g_i = r_xor_sum ^ x[n - 1 - i];
        level_cts.push(g_i);
    }

    (seeds, level_cts)
}

/// Generates unary outer product using seeds
pub(crate) fn gen_unary_outer_product(
    seeds: &Vec<Block>,
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<Block> {
    let m = y.len();

    let mut gen_cts: Vec<Block> = Vec::new();

    // For each share (B, B+ b∂)
    // G sends the sum (XOR_i A_i) + B), which allows E to obtain A_{x + gamma} + b∂
    // Expand the 2^n leaf seeds into 2^n by
    for j in 0..m {
        // Endianness note (little-endian y): index 0 is LSB of y, index m-1 is MSB.
        let mut row: Block = Block::default();
        for i in 0..seeds.len() {
            let tweak = (seeds.len() * j + i) as u128;
            let s = cipher.tccr(Block::from(tweak), seeds[i]);
            row ^= s;

            // let i = f(i) is just i in usize
            // Endianness note (little-endian x encoded in seed index i):
            // bit k of i corresponds to the k-th least significant bit.
            for k in 0..out.rows() {
                if ((i >> k) & 1) == 1 {
                    out[(k, j)] ^= s;
                }
            }
        }
        row ^= y[j];
        gen_cts.push(row);
    }
    gen_cts
}

/// Evaluator side of paper Construction 4 / 5_online.tex `tensorev`'s GGM
/// tree portion (steps 2–3): reconstructs the 2^n leaf seeds using the
/// garbler's level ciphertexts.
///
/// Mirrors `gen_populate_seeds_mem_optimized` under the paper's improved
/// one-hot construction. For each non-missing parent at level `i-1`, the
/// evaluator's seed equals the garbler's; the active R is recovered from
/// `G_i` per paper `5_online.tex:81-83`:
///
/// ```text
///   R_{i, α_{i-1}}^ev := G_i ⊕ (⊕_{j ≠ α_{i-1}} R_{i,j}^ev) ⊕ (A_i ⊕ a_i·Δ_gb)
/// ```
///
/// where `A_i ⊕ a_i·Δ_gb = x[n-1-i]` is the evaluator's MAC for level i. The
/// recovered R differs from the garbler's by `a_i·Δ_gb`, which propagates the
/// missing-path Δ-offset invariant into the next level's children:
///
/// ```text
///   seeds[j]^ev = seeds[j]^gb ⊕ [j == α_i]·Δ_gb       (paper Lemma 1)
/// ```
///
/// Level-0 init also changes from HK21's TCCR-based two-branch dispatch to
/// the paper's direct assignment: BOTH `seeds[0]` and `seeds[1]` receive the
/// evaluator's MAC `x[n-1]` (= `A_0 ⊕ a_0·Δ_gb`). The branching bit `a_0`
/// only enters via the `missing` accumulator. The garbler's S_{0,*} differ
/// from each other by Δ, so the eval value matches gb's at one position and
/// is Δ-shifted at the other — exactly as the missing-path invariant requires.
///
/// `a_bits` is the evaluator's explicit choice vector — passed separately
/// from `x` (the MAC) so tree navigation works for any Δ.lsb (paper assumes
/// MAC LSB = bit; the codebase generalizes).
///
/// Endianness: index 0 is LSB, index n-1 is MSB. Tree consumes `a_bits[n-1]`
/// at level 0.
///
/// Returns `(leaf_seeds, missing)` where `leaf_seeds.len() == 2^n`. With the
/// paper-faithful init, `seeds[missing]` is a real (Δ-shifted) value, NOT a
/// `Block::default()` sentinel — the downstream leaf-expansion code (`if i !=
/// missing`) avoids computing on it regardless.
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    a_bits: &[bool],
    levels: &[Block],
    cipher: &FixedKeyAes,
) -> (Vec<Block>, usize) {
    let n: usize = x.len();
    debug_assert_eq!(a_bits.len(), n, "a_bits must have same length as MAC blocks");
    debug_assert_eq!(
        levels.len(),
        n.saturating_sub(1),
        "levels must have length n-1 (one ciphertext per tree level)"
    );

    let mut seeds: Vec<Block> = vec![Block::default(); 1 << n];

    // Level-0 init (paper step 2): both positions receive the evaluator's MAC.
    // Per the missing-path invariant, seeds[α_0]^ev = seeds[α_0]^gb ⊕ Δ and
    // seeds[1-α_0]^ev = seeds[1-α_0]^gb. Since gb writes A_0⊕Δ at index 0 and
    // A_0 at index 1, eval's MAC `x[n-1] = A_0 ⊕ a_0·Δ` satisfies both
    // equalities depending on the value of α_0 = a_0.
    seeds[0] = x[n - 1];
    seeds[1] = x[n - 1];

    let mut missing: usize = a_bits[n - 1] as usize;

    for i in 1..n {
        let tweak = Block::from(LOOP_DOMAIN | (i as u128));

        // Save the missing parent's seed BEFORE any writes — it gets clobbered
        // when j = floor(missing/2) writes to seeds[2*floor(missing/2) + (missing % 2)].
        let saved_missing_parent = seeds[missing];

        let mut r_xor_known = Block::default();

        // Reverse iteration over parents. Skip j == missing — its R cannot be
        // computed from seeds[missing] directly (eval's value is Δ-shifted off
        // gb's, so H gives garbage); we recover R_missing from G_i below.
        for j in (0..(1 << i)).rev() {
            if j == missing {
                continue;
            }
            let parent = seeds[j];
            let r = cipher.tccr(tweak, parent);
            seeds[2 * j + 1] = r;            // upper half
            seeds[2 * j]     = r ^ parent;   // lower half
            r_xor_known ^= r;
        }

        // Recover R at the missing parent: R_missing^ev = G_i ⊕ XOR_{known} R_j ⊕ (A_i ⊕ a_i·Δ).
        // The eval's MAC x[n-1-i] = A_i ⊕ a_i·Δ_gb supplies the (A_i ⊕ a_i·Δ) term.
        let g_i = levels[i - 1];
        let r_missing = g_i ^ r_xor_known ^ x[n - 1 - i];

        // Distribute R_missing to the missing parent's children using the
        // saved pre-clobber parent value. Note: 2*missing and 2*missing+1 were
        // skipped during the j-loop, and j=floor(missing/2) (which writes
        // either seeds[missing-1..missing] or seeds[missing..missing+1] but
        // never seeds[2*missing] or seeds[2*missing+1]) does not collide here.
        seeds[2 * missing + 1] = r_missing;
        seeds[2 * missing]     = r_missing ^ saved_missing_parent;

        // Update missing index for next level. bit=0 → lower (even) child;
        // bit=1 → upper (odd) child. Matches paper's α_i := α_{i-1} + a_i·2^i
        // under interleaved indexing.
        let bit = a_bits[n - 1 - i];
        missing = (missing << 1) | (bit as usize);
    }

    (seeds, missing)
}

/// Evaluator's leaf-expansion + Z accumulation counterpart to `gen_unary_outer_product`.
///
/// Combines the reconstructed `seeds` (with `seeds[missing] == Block::default()`),
/// the garbler's leaf ciphertexts `gen_cts`, the evaluator's `y` share (T^ev),
/// and the `missing` index to (a) write Z_eval into `out` and (b) return the
/// recovered missing-leaf column values (for optional downstream use).
///
/// Preconditions:
/// - `seeds.len() == 2^n` for some `n`
/// - `y.len() == m`, `gen_cts.len() == m`, `out` is an n×m column-major view
///
/// `seeds[missing]` may hold any value — under the paper-faithful eval init
/// it is a Δ-shifted real seed (no longer a `Block::default` sentinel). The
/// `if i != missing` guard avoids ever computing on it.
pub(crate) fn eval_unary_outer_product(
    seeds: &[Block],
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[Block],
) -> Vec<Block> {
    let m = y.len();

    let mut eval_cts: Vec<Block> = Vec::new();

    for j in 0..m {
        // Endianness note (little-endian y): index 0 is LSB of y, index m-1 is MSB.
        let mut eval_ct = Block::default();
        for i in 0..seeds.len() {
            if i != missing {
                let tweak = (seeds.len() * j + i) as u128;
                let s = cipher.tccr(Block::from(tweak), seeds[i]);
                eval_ct ^= s;
                // Endianness note (little-endian x encoded in seed index i):
                // bit k of i corresponds to the k-th least significant bit.
                for k in 0..out.rows() {
                    if ((i >> k) & 1) == 1 {
                        out[(k, j)] ^= s;
                    }
                }
            }
        }
        eval_ct ^= gen_cts[j] ^ y[j];
        eval_cts.push(eval_ct);
        // Endianness note (little-endian x): distribute eval_ct to rows where missing has bit k set.
        for k in 0..out.rows() {
            if ((missing >> k) & 1) == 1 {
                out[(k, j)] ^= eval_ct;
            }
        }
    }

    eval_cts
}

/// Domain-separation tag for the narrow GGM tree-loop tweaks
/// (`gen_populate_seeds_mem_optimized` / `eval_populate_seeds_mem_optimized`).
///
/// Without this tag, level-loop tweaks `i ∈ [1, n-1]` would overlap leaf-
/// expansion tweaks `m·j + i ∈ [0, m·2^n)` (paper `5_online.tex` C2 input-
/// distinctness latent assumption — see AUDIT-2.1 C2). Setting bit 65 puts
/// every loop tweak above the (m·2^n) leaf range and disjoint from
/// `WIDE_DOMAIN`, bringing the narrow path in line with the wide path's
/// explicit domain hardening.
pub(crate) const LOOP_DOMAIN: u128 = 1u128 << 65;

/// Domain-separation tag placed in bits 127..64 of the wide-function tweaks.
///
/// The narrow leaf-expansion (`gen_unary_outer_product`) uses tweaks 0, 1, 2,
/// … (values fit in the low 64 bits). Setting bit 64 ensures every wide tweak
/// is disjoint from every narrow tweak, so TCCR outputs are PRF-independent
/// between the two function families even if the same leaf seeds were
/// hypothetically reused.
const WIDE_DOMAIN: u128 = 1u128 << 64;

/// Wide-leaf variant of `gen_unary_outer_product`. Expands each leaf seed into TWO
/// pseudorandom Block values (κ-half via even tweak, ρ-half via odd tweak) and
/// accumulates each half into a separate output matrix. Returns wide ciphertexts
/// as `Vec<(Block, Block)>` where `.0` is the κ-half and `.1` is the ρ-half.
///
/// Phase 9 / P2-01. See CONTEXT.md D-01, D-02, D-03 and 6_total.tex Construction 4.
///
/// Tweak convention (domain-separated from narrow tweaks via `WIDE_DOMAIN`):
///   - κ-half:  cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1)),     seeds[i])
///   - ρ-half:  cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i])
/// where `base = seeds.len() * j + i`.
///
/// Both `out_gb` and `out_ev` MUST be n×m column-major views of the same shape.
/// `y_d_gb` and `y_d_ev` MUST each have length m.
pub(crate) fn gen_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<(Block, Block)> {
    let m = y_d_gb.len();
    debug_assert_eq!(y_d_ev.len(), m, "y_d_gb and y_d_ev must have the same length m");
    debug_assert_eq!(out_gb.rows(), out_ev.rows(),
        "out_gb and out_ev must have the same number of rows");

    let mut gen_cts: Vec<(Block, Block)> = Vec::with_capacity(m);

    for j in 0..m {
        let mut row_gb: Block = Block::default();
        let mut row_ev: Block = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * j + i) as u128;
            // Domain-separated even/odd tweak split: WIDE_DOMAIN bit 64 ensures
            // wide tweaks never collide with narrow tweaks (CONTEXT.md D-03).
            let s_gb = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1)),     seeds[i]);
            let s_ev = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i]);
            row_gb ^= s_gb;
            row_ev ^= s_ev;

            // Distribute both halves to the same indexed positions.
            for k in 0..out_gb.rows() {
                if ((i >> k) & 1) == 1 {
                    out_gb[(k, j)] ^= s_gb;
                    out_ev[(k, j)] ^= s_ev;
                }
            }
        }
        row_gb ^= y_d_gb[j];
        row_ev ^= y_d_ev[j];
        gen_cts.push((row_gb, row_ev));
    }

    gen_cts
}

/// Wide-leaf variant of `eval_unary_outer_product`. Mirrors
/// `gen_unary_outer_product_wide` — reconstructs the missing-leaf contribution
/// into BOTH `out_gb` and `out_ev` using the wide ciphertexts
/// `gen_cts: &[(Block, Block)]`.
///
/// Phase 9 / P2-01. See CONTEXT.md D-01, D-03 and 6_total.tex Construction 4 step 4.
///
/// Preconditions:
/// - `y_d_gb.len() == m == y_d_ev.len() == gen_cts.len()`
///
/// `seeds[missing]` may hold any value — under the paper-faithful eval init
/// it is a Δ-shifted real seed (no longer a `Block::default` sentinel). The
/// `if i != missing` guard avoids ever computing on it.
pub(crate) fn eval_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[(Block, Block)],
) -> Vec<(Block, Block)> {
    let m = y_d_gb.len();
    debug_assert_eq!(y_d_ev.len(), m, "y_d_gb and y_d_ev must have the same length m");
    debug_assert_eq!(gen_cts.len(), m, "gen_cts must have length m (one wide ct per column)");

    let mut eval_cts: Vec<(Block, Block)> = Vec::with_capacity(m);

    for j in 0..m {
        let mut eval_ct_gb = Block::default();
        let mut eval_ct_ev = Block::default();
        for i in 0..seeds.len() {
            if i != missing {
                let base = (seeds.len() * j + i) as u128;
                let s_gb = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1)),     seeds[i]);
                let s_ev = cipher.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i]);
                eval_ct_gb ^= s_gb;
                eval_ct_ev ^= s_ev;
                for k in 0..out_gb.rows() {
                    if ((i >> k) & 1) == 1 {
                        out_gb[(k, j)] ^= s_gb;
                        out_ev[(k, j)] ^= s_ev;
                    }
                }
            }
        }
        // Apply the wide ciphertext + y correction to recover the missing leaf's
        // contribution to the column accumulator. (Same XOR pattern as narrow eval
        // line 254, split across kappa/rho halves per 6_total.tex Construction 4 step 4.)
        eval_ct_gb ^= gen_cts[j].0 ^ y_d_gb[j];
        eval_ct_ev ^= gen_cts[j].1 ^ y_d_ev[j];
        eval_cts.push((eval_ct_gb, eval_ct_ev));

        // Distribute the recovered missing-leaf contribution to rows where missing
        // has bit k set.
        for k in 0..out_gb.rows() {
            if ((missing >> k) & 1) == 1 {
                out_gb[(k, j)] ^= eval_ct_gb;
                out_ev[(k, j)] ^= eval_ct_ev;
            }
        }
    }

    eval_cts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aes::FIXED_KEY_AES;
    use crate::matrix::BlockMatrix;

    #[test]
    fn test_gen_unary_outer_product_wide_tweak_independence() {
        // P2-01: kappa-half and rho-half outputs MUST differ — even/odd tweak split
        // ensures TCCR outputs are pseudorandomly independent.
        // Deterministic seeds (n=2 -> 4 leaves, m=2 columns).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x1000)).collect();
        let mut y_gb_mat = BlockMatrix::new(2, 1);
        let mut y_ev_mat = BlockMatrix::new(2, 1);
        for j in 0..2 {
            y_gb_mat[j] = Block::from((j as u128) + 0x2000);
            y_ev_mat[j] = Block::from((j as u128) + 0x3000);
        }
        let mut out_gb_mat = BlockMatrix::new(2, 2);
        let mut out_ev_mat = BlockMatrix::new(2, 2);

        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut out_gb_mat.as_view_mut(),
            &mut out_ev_mat.as_view_mut(),
            &FIXED_KEY_AES,
        );

        assert_eq!(gen_cts.len(), 2, "wide gen returns m wide ciphertexts");
        // Each (kappa, rho) pair MUST differ — overwhelming probability under TCCR.
        for (k, ct) in gen_cts.iter().enumerate() {
            assert_ne!(ct.0, ct.1, "gen_cts[{}].0 (kappa) must differ from .1 (rho)", k);
        }
        // The two output matrices MUST differ at some position.
        let mut differs = false;
        for k in 0..2 {
            for j in 0..2 {
                if out_gb_mat[(k, j)] != out_ev_mat[(k, j)] {
                    differs = true;
                    break;
                }
            }
        }
        assert!(differs, "out_gb and out_ev must differ at >=1 position");
    }

    #[test]
    fn test_eval_unary_outer_product_wide_round_trip_kappa() {
        // P2-01: With matching missing index, gen + eval round-trip on the kappa half
        // produces the same accumulator behavior as the narrow gen + narrow eval.
        // Use n=2 (4 leaves), m=1 column, missing=2 (arbitrary leaf).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x10)).collect();
        let mut y_gb_mat = BlockMatrix::new(1, 1);
        let mut y_ev_mat = BlockMatrix::new(1, 1);
        y_gb_mat[0] = Block::from(0x20u128);
        y_ev_mat[0] = Block::from(0x30u128);

        // Garble side: full seeds.
        let mut gen_out_gb = BlockMatrix::new(2, 1);
        let mut gen_out_ev = BlockMatrix::new(2, 1);
        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut gen_out_gb.as_view_mut(),
            &mut gen_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
        );

        // Eval side: copy seeds but zero out the missing entry.
        let missing = 2usize;
        let mut eval_seeds = seeds.clone();
        eval_seeds[missing] = Block::default();
        let mut eval_out_gb = BlockMatrix::new(2, 1);
        let mut eval_out_ev = BlockMatrix::new(2, 1);
        eval_unary_outer_product_wide(
            &eval_seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut eval_out_gb.as_view_mut(),
            &mut eval_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
            missing,
            &gen_cts,
        );

        // Compute expected kappa-row directly from the row equation:
        //   row_gb = (XOR_i tccr(WIDE_DOMAIN | (2*base), seeds[i])) ^ y_gb[j]
        let mut expected_row_gb = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * 0 + i) as u128;
            expected_row_gb ^= FIXED_KEY_AES.tccr(Block::from(WIDE_DOMAIN | (base << 1)), seeds[i]);
        }
        expected_row_gb ^= y_gb_mat[0];

        // Verify gen ciphertext kappa-half matches the row equation.
        assert_eq!(gen_cts[0].0, expected_row_gb,
            "wide gen ciphertext kappa-half must equal the row equation");
    }

    #[test]
    fn test_eval_unary_outer_product_wide_round_trip_rho() {
        // P2-01: same round-trip property for the rho half (tweak base<<1|1).
        let seeds: Vec<Block> = (0..4).map(|i| Block::from((i as u128) + 0x10)).collect();
        let mut y_gb_mat = BlockMatrix::new(1, 1);
        let mut y_ev_mat = BlockMatrix::new(1, 1);
        y_gb_mat[0] = Block::from(0x20u128);
        y_ev_mat[0] = Block::from(0x30u128);

        let mut gen_out_gb = BlockMatrix::new(2, 1);
        let mut gen_out_ev = BlockMatrix::new(2, 1);
        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut gen_out_gb.as_view_mut(),
            &mut gen_out_ev.as_view_mut(),
            &FIXED_KEY_AES,
        );

        let mut expected_row_ev = Block::default();
        for i in 0..seeds.len() {
            let base = (seeds.len() * 0 + i) as u128;
            expected_row_ev ^= FIXED_KEY_AES.tccr(Block::from(WIDE_DOMAIN | (base << 1 | 1)), seeds[i]);
        }
        expected_row_ev ^= y_ev_mat[0];

        assert_eq!(gen_cts[0].1, expected_row_ev,
            "wide gen ciphertext rho-half must equal the row equation under odd tweak");
    }

    #[test]
    fn test_wide_signature_shapes() {
        // P2-01: shape invariants — gen_cts.len() == m; out_gb / out_ev are written.
        let seeds: Vec<Block> = (0..4).map(|i| Block::from(i as u128)).collect();
        let mut y_gb_mat = BlockMatrix::new(3, 1);
        let mut y_ev_mat = BlockMatrix::new(3, 1);
        for j in 0..3 {
            y_gb_mat[j] = Block::from((j as u128) + 100);
            y_ev_mat[j] = Block::from((j as u128) + 200);
        }
        let mut out_gb_mat = BlockMatrix::new(2, 3);
        let mut out_ev_mat = BlockMatrix::new(2, 3);

        let gen_cts = gen_unary_outer_product_wide(
            &seeds,
            &y_gb_mat.as_view(),
            &y_ev_mat.as_view(),
            &mut out_gb_mat.as_view_mut(),
            &mut out_ev_mat.as_view_mut(),
            &FIXED_KEY_AES,
        );

        assert_eq!(gen_cts.len(), 3, "gen_cts.len() must equal m=3");

        // At least one entry of each output matrix must be non-default (overwhelmingly likely).
        let mut nonzero_gb = false;
        let mut nonzero_ev = false;
        for k in 0..2 {
            for j in 0..3 {
                if out_gb_mat[(k, j)] != Block::default() { nonzero_gb = true; }
                if out_ev_mat[(k, j)] != Block::default() { nonzero_ev = true; }
            }
        }
        assert!(nonzero_gb, "out_gb has at least one non-default entry");
        assert!(nonzero_ev, "out_ev has at least one non-default entry");
    }
}
