use crate::{
    delta::Delta,
    leaky_tensor_pre::LeakyTriple,
    preprocessing::{TensorFpreGen, TensorFpreEval},
    sharing::AuthBitShare,
};
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha12Rng;

/// Perform one two-to-one combining step from paper Construction 3
/// (references/appendix_krrw_pre.tex §3.1 lines 415-444).
///
/// Inputs: two LeakyTriples `prime` (consumed) and `dprime` (borrowed), both with the
/// same (n, m, delta_a, delta_b). Output: a single combined LeakyTriple.
///
/// Algorithm (paper lines 427-443):
///   x := x' XOR x''                           (D-01)
///   y := y'                                   (D-02)
///   d := y' XOR y'' (revealed with MACs)      (D-05, D-06)
///   Z := Z' XOR Z'' XOR itmac{x''}{Δ} ⊗ d    (D-03, D-04)
///
/// The `itmac{x''}{Δ} ⊗ d` term is computed locally since d is public (paper line
/// 437). For each (i, j), the IT-MAC share at column-major index j*n+i is
/// `dprime.gen_x_shares[i]` if d[j] == 1, else the zero share
/// `AuthBitShare::default()`.
///
/// Panics: "MAC mismatch in share" if any assembled d share fails MAC verification
/// (in-process substitute for the paper's "publicly reveal with appropriate MACs").
pub(crate) fn two_to_one_combine(
    prime: LeakyTriple,
    dprime: &LeakyTriple,
) -> LeakyTriple {
    // Precondition: same (n, m, delta_a, delta_b). The outer combine_leaky_triples
    // already asserts this, but re-assert for unit-test safety (per 05-CONTEXT D-11).
    assert_eq!(prime.n, dprime.n, "two_to_one_combine: n mismatch");
    assert_eq!(prime.m, dprime.m, "two_to_one_combine: m mismatch");
    assert_eq!(
        prime.delta_a.as_block(),
        dprime.delta_a.as_block(),
        "two_to_one_combine: delta_a mismatch"
    );
    assert_eq!(
        prime.delta_b.as_block(),
        dprime.delta_b.as_block(),
        "two_to_one_combine: delta_b mismatch"
    );
    let n = prime.n;
    let m = prime.m;
    let delta_a = prime.delta_a;
    let delta_b = prime.delta_b;

    // ---- Step A: assemble d shares (paper line 428: d := y' XOR y'') ----
    // AuthBitShare + AuthBitShare is XOR field-wise per src/sharing.rs:66-77.
    let gen_d: Vec<AuthBitShare> = (0..m)
        .map(|j| prime.gen_y_shares[j] + dprime.gen_y_shares[j])
        .collect();
    let eval_d: Vec<AuthBitShare> = (0..m)
        .map(|j| prime.eval_y_shares[j] + dprime.eval_y_shares[j])
        .collect();

    // ---- Step B: MAC-verify d and extract d bits (paper line 428) ----
    // In-process substitute for "publicly reveal with appropriate MACs".
    let mut d_bits: Vec<bool> = Vec::with_capacity(m);
    for j in 0..m {
        verify_cross_party(&gen_d[j], &eval_d[j], &delta_a, &delta_b);
        d_bits.push(gen_d[j].value ^ eval_d[j].value);
    }

    // ---- Step C: x = x' XOR x'' (paper line 427, D-01) ----
    let gen_x: Vec<AuthBitShare> = (0..n)
        .map(|i| prime.gen_x_shares[i] + dprime.gen_x_shares[i])
        .collect();
    let eval_x: Vec<AuthBitShare> = (0..n)
        .map(|i| prime.eval_x_shares[i] + dprime.eval_x_shares[i])
        .collect();

    // ---- Step D: Z = Z' XOR Z'' XOR (x'' tensor d), paper line 443 ----
    // Column-major nested loop: outer j in 0..m, inner i in 0..n, k = j*n + i.
    // Zero-share when d[j] == 0 (D-03).
    let zero_share = AuthBitShare::default();
    let mut gen_z: Vec<AuthBitShare> = Vec::with_capacity(n * m);
    let mut eval_z: Vec<AuthBitShare> = Vec::with_capacity(n * m);
    for j in 0..m {
        for i in 0..n {
            let k = j * n + i;
            // Rightmost term: x''_i if d[j] else ZERO
            let dx_gen = if d_bits[j] {
                dprime.gen_x_shares[i]
            } else {
                zero_share
            };
            let dx_eval = if d_bits[j] {
                dprime.eval_x_shares[i]
            } else {
                zero_share
            };
            gen_z.push(prime.gen_z_shares[k] + dprime.gen_z_shares[k] + dx_gen);
            eval_z.push(prime.eval_z_shares[k] + dprime.eval_z_shares[k] + dx_eval);
        }
    }

    // ---- Step E: y = y' (paper line 427, D-02) ----
    // Move the vectors out of prime (it is owned); no clone needed.
    let gen_y = prime.gen_y_shares;
    let eval_y = prime.eval_y_shares;

    LeakyTriple {
        n,
        m,
        delta_a,
        delta_b,
        gen_x_shares: gen_x,
        gen_y_shares: gen_y,
        gen_z_shares: gen_z,
        eval_x_shares: eval_x,
        eval_y_shares: eval_y,
        eval_z_shares: eval_z,
    }
}

/// Compute the bucket size B for Pi_aTensor' (Construction 4, Appendix F).
///
/// Formula: `B = 1 + ceil(SSP / log2(n * ell))` for `n * ell >= 2`, where SSP = 40.
/// For `n * ell <= 1`, the bucketing amplification is degenerate; fall back to
/// the naive combining bound B = SSP (paper §3.1 preamble).
///
/// Integer ceiling: `1 + (SSP + log2_floor(n*ell) - 1) / log2_floor(n*ell)`.
/// `log2_floor(k) = usize::BITS - k.leading_zeros() - 1`.
///
/// Parameters:
///   n   — tensor row dimension.
///   ell — number of OUTPUT authenticated tensor triples desired.
///
/// Examples:
///   bucket_size_for(4, 1)    = 21   (1 + ceil(40 / log2(4))  = 1 + 20)
///   bucket_size_for(4, 2)    = 15   (1 + ceil(40 / log2(8))  = 1 + ceil(40/3) = 1 + 14)
///   bucket_size_for(16, 1)   = 11   (1 + ceil(40 / log2(16)) = 1 + 10)
pub fn bucket_size_for(n: usize, ell: usize) -> usize {
    const SSP: usize = 40;
    let product = n.saturating_mul(ell);
    if product <= 1 {
        return SSP;
    }
    let log2_p = (usize::BITS - product.leading_zeros() - 1) as usize;
    1 + (SSP + log2_p - 1) / log2_p
}

/// Combine B leaky triples into one authenticated tensor triple (Pi_aTensor', Construction 4).
///
/// Implements the paper's two-to-one combining (references/appendix_krrw_pre.tex §3.1
/// lines 415-444) iteratively: start with `triples[0]`, fold the remaining B-1 triples
/// into the accumulator one at a time via `two_to_one_combine`.
///
/// PRECONDITION: All triples MUST share the same delta_a and delta_b. This is guaranteed
/// when run_preprocessing uses a single shared IdealBCot instance. An assertion enforces
/// this at runtime. If violated, the combining panics because XOR of shares under
/// different deltas cannot preserve the MAC invariant mac = key XOR bit*delta.
///
/// Output shapes: alpha_auth_bit_shares (length n), beta_auth_bit_shares (length m),
/// correlated_auth_bit_shares (length n*m, column-major j*n+i). Labels are stubbed to
/// Vec::new() per Phase 4 D-07.
///
/// triples: Vec of LeakyTriple, length must equal bucket_size.
/// chunking_factor: passed through to TensorFpreGen/Eval output.
/// shuffle_seed: seeds a per-triple `ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j))`
/// used to sample the Construction 4 row-permutation π_j ∈ S_n for triple j.
/// `wrapping_add` is used instead of XOR so that `shuffle_seed = 0` does not collapse
/// all seeds to `j` directly — triple 0 always gets seed 0 under XOR, eliminating
/// per-run freshness. With `wrapping_add`, distinct seeds are guaranteed for all j.
pub fn combine_leaky_triples(
    triples: Vec<LeakyTriple>,
    bucket_size: usize,
    n: usize,
    m: usize,
    chunking_factor: usize,
    shuffle_seed: u64,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(triples.len(), bucket_size, "triples.len() must equal bucket_size");
    assert!(bucket_size >= 1);

    // W-04: Assert all triples share the same delta_a and delta_b before combining.
    // This invariant is guaranteed by run_preprocessing using a single shared IdealBCot.
    // If violated, the XOR combination MAC invariant mac = key XOR bit*delta breaks
    // because keys and MACs from different deltas cannot be XOR-combined correctly.
    let delta_a = triples[0].delta_a;
    let delta_b = triples[0].delta_b;
    for (idx, t) in triples.iter().enumerate() {
        assert_eq!(
            t.delta_a.as_block(),
            delta_a.as_block(),
            "triple[{}] delta_a differs from triple[0] delta_a — all triples must share the same IdealBCot",
            idx
        );
        assert_eq!(
            t.delta_b.as_block(),
            delta_b.as_block(),
            "triple[{}] delta_b differs from triple[0] delta_b — all triples must share the same IdealBCot",
            idx
        );
    }

    // ---- Construction 4 permutation step (PROTO-13, PROTO-14) ----
    // For each triple j, sample a fresh per-triple ChaCha12Rng seeded
    // with shuffle_seed.wrapping_add(j), generate a uniform permutation
    // π_j ∈ S_n via Fisher-Yates (SliceRandom::shuffle), and apply it
    // to the x-rows and the i-index of the Z-rows (y-rows untouched).
    let mut triples = triples; // rebind as `mut` for in-place permutation.
    for (j, triple) in triples.iter_mut().enumerate() {
        let mut rng = ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j as u64));
        let mut perm: Vec<usize> = (0..n).collect();
        perm.shuffle(&mut rng);
        apply_permutation_to_triple(triple, &perm);
    }

    // Iterative fold per Construction 4: start with triples[0], combine each next
    // triple into the accumulator via two_to_one_combine (paper line 474).
    // (Clone triples[0] because LeakyTriple is not Copy — Rust ownership pitfall.)
    let mut acc: LeakyTriple = triples[0].clone();
    for next in triples.iter().skip(1) {
        acc = two_to_one_combine(acc, next);
    }

    // The parameters n and m are passed through to the output structs; assert they
    // agree with the combined triple shape to catch caller drift.
    assert_eq!(acc.n, n, "combine_leaky_triples: n parameter disagrees with triple.n");
    assert_eq!(acc.m, m, "combine_leaky_triples: m parameter disagrees with triple.m");

    // Package the combined LeakyTriple into the preprocessing output structs.
    // Labels stubbed to Vec::new() per Phase 4 D-07.
    (
        TensorFpreGen {
            n,
            m,
            chunking_factor,
            delta_a,
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: acc.gen_x_shares,
            beta_auth_bit_shares: acc.gen_y_shares,
            correlated_auth_bit_shares: acc.gen_z_shares,
        },
        TensorFpreEval {
            n,
            m,
            chunking_factor,
            delta_b,
            alpha_labels: Vec::new(),
            beta_labels: Vec::new(),
            alpha_auth_bit_shares: acc.eval_x_shares,
            beta_auth_bit_shares: acc.eval_y_shares,
            correlated_auth_bit_shares: acc.eval_z_shares,
        },
    )
}

/// Apply a row permutation `perm` (a permutation of `0..n`) to the x and
/// Z rows of `triple` IN PLACE. y rows are NOT permuted — per Construction 4
/// (Appendix F), only the alpha side and the correlated tensor rows carry the
/// row permutation; beta is untouched.
///
/// Permutation semantics:
///   new gen_x_shares[i]  = old gen_x_shares[perm[i]]   for i in 0..n
///   new eval_x_shares[i] = old eval_x_shares[perm[i]]  for i in 0..n
///   for each column j in 0..m, within the contiguous slice [j*n..(j+1)*n]:
///     new gen_z_shares[j*n + i]  = old gen_z_shares[j*n + perm[i]]
///     new eval_z_shares[j*n + i] = old eval_z_shares[j*n + perm[i]]
///
/// `perm.len()` must equal `triple.n`; otherwise this panics. The caller
/// is responsible for constructing `perm` as a valid permutation of 0..n.
pub(crate) fn apply_permutation_to_triple(
    triple: &mut LeakyTriple,
    perm: &[usize],
) {
    let n = triple.n;
    let m = triple.m;
    assert_eq!(
        perm.len(),
        n,
        "apply_permutation_to_triple: perm.len() must equal n"
    );

    // Permute x shares (length n) — build new vecs by reading position
    // perm[i] from the original snapshot.
    let orig_gen_x = triple.gen_x_shares.clone();
    let orig_eval_x = triple.eval_x_shares.clone();
    for i in 0..n {
        triple.gen_x_shares[i] = orig_gen_x[perm[i]];
        triple.eval_x_shares[i] = orig_eval_x[perm[i]];
    }

    // Permute Z shares column-major: for each column j, permute the
    // i-index within the contiguous slice [j*n .. (j+1)*n].
    let orig_gen_z = triple.gen_z_shares.clone();
    let orig_eval_z = triple.eval_z_shares.clone();
    for j in 0..m {
        for i in 0..n {
            triple.gen_z_shares[j * n + i] = orig_gen_z[j * n + perm[i]];
            triple.eval_z_shares[j * n + i] = orig_eval_z[j * n + perm[i]];
        }
    }
}

/// Cross-party `AuthBitShare` MAC verification — the in-process substitute for the
/// paper's "publicly reveal with appropriate MACs".
///
/// `gen_share.key` is A's sender key; `gen_share.mac` is A's sender MAC (committed
/// under delta_b). `eval_share.key` is B's sender key; `eval_share.mac` is B's sender
/// MAC (committed under delta_a). The two `.verify` calls below reassemble properly
/// aligned IT-MAC pairs so that each side checks `mac == key XOR bit*delta` under
/// the correct verifier's delta. Panics with "MAC mismatch in share" on tampered
/// shares.
///
/// NOTE: do NOT call `share.verify(&delta)` directly on a raw cross-party
/// AuthBitShare — it will panic even on correctly-formed shares because the key and
/// MAC fields come from different bCOT directions and commit under different deltas.
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare {
        key: eval_share.key,
        mac: gen_share.mac,
        value: gen_share.value,
    }
    .verify(delta_b);
    AuthBitShare {
        key: gen_share.key,
        mac: eval_share.mac,
        value: eval_share.value,
    }
    .verify(delta_a);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;
    use crate::leaky_tensor_pre::LeakyTensorPre;
    use crate::sharing::AuthBitShare;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;

    fn make_triples(n: usize, m: usize, count: usize) -> Vec<LeakyTriple> {
        // Single shared IdealBCot — ALL triples get the same delta_a and delta_b.
        let mut bcot = IdealBCot::new(42, 99);
        let mut triples = Vec::new();
        for seed in 0..count {
            let mut ltp = LeakyTensorPre::new(seed as u64, n, m, &mut bcot);
            triples.push(ltp.generate());
        }
        triples
    }

    /// Field-by-field AuthBitShare equality helper — AuthBitShare does NOT
    /// derive PartialEq (per src/sharing.rs line 42). Used by Task 1
    /// apply_permutation_to_triple tests to compare whole shares before/after
    /// the permutation. Returns true iff key, mac, and value all match.
    fn shares_eq(a: &AuthBitShare, b: &AuthBitShare) -> bool {
        a.key == b.key && a.mac == b.mac && a.value == b.value
    }

    /// Field-by-field equality over a slice of AuthBitShare.
    fn slices_eq(a: &[AuthBitShare], b: &[AuthBitShare]) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| shares_eq(x, y))
    }

    #[test]
    fn test_apply_permutation_identity_is_noop() {
        let triples = make_triples(4, 2, 1);
        let mut t = triples[0].clone();
        let before_gen_x = t.gen_x_shares.clone();
        let before_eval_x = t.eval_x_shares.clone();
        let before_gen_y = t.gen_y_shares.clone();
        let before_eval_y = t.eval_y_shares.clone();
        let before_gen_z = t.gen_z_shares.clone();
        let before_eval_z = t.eval_z_shares.clone();

        let perm: Vec<usize> = (0..t.n).collect();
        apply_permutation_to_triple(&mut t, &perm);

        assert!(slices_eq(&t.gen_x_shares, &before_gen_x), "identity perm must not move gen_x");
        assert!(slices_eq(&t.eval_x_shares, &before_eval_x), "identity perm must not move eval_x");
        assert!(slices_eq(&t.gen_y_shares, &before_gen_y), "y is never permuted");
        assert!(slices_eq(&t.eval_y_shares, &before_eval_y), "y is never permuted");
        assert!(slices_eq(&t.gen_z_shares, &before_gen_z), "identity perm must not move gen_z");
        assert!(slices_eq(&t.eval_z_shares, &before_eval_z), "identity perm must not move eval_z");
    }

    #[test]
    fn test_apply_permutation_swap_moves_x_and_z_rows_but_not_y() {
        let n = 4usize;
        let m = 2usize;
        let triples = make_triples(n, m, 1);
        let mut t = triples[0].clone();
        let before_gen_x = t.gen_x_shares.clone();
        let before_eval_x = t.eval_x_shares.clone();
        let before_gen_y = t.gen_y_shares.clone();
        let before_eval_y = t.eval_y_shares.clone();
        let before_gen_z = t.gen_z_shares.clone();
        let before_eval_z = t.eval_z_shares.clone();

        // Swap rows 0 and 1; leave 2 and 3 fixed.
        let perm = vec![1usize, 0, 2, 3];
        apply_permutation_to_triple(&mut t, &perm);

        // x: row 0 and row 1 swapped.
        assert!(shares_eq(&t.gen_x_shares[0], &before_gen_x[1]));
        assert!(shares_eq(&t.gen_x_shares[1], &before_gen_x[0]));
        assert!(shares_eq(&t.gen_x_shares[2], &before_gen_x[2]));
        assert!(shares_eq(&t.gen_x_shares[3], &before_gen_x[3]));
        assert!(shares_eq(&t.eval_x_shares[0], &before_eval_x[1]));
        assert!(shares_eq(&t.eval_x_shares[1], &before_eval_x[0]));

        // y must be unchanged.
        assert!(slices_eq(&t.gen_y_shares, &before_gen_y));
        assert!(slices_eq(&t.eval_y_shares, &before_eval_y));

        // Z: in each column j, indices 0 and 1 swap; indices 2 and 3 fixed.
        for j in 0..m {
            assert!(shares_eq(&t.gen_z_shares[j * n + 0], &before_gen_z[j * n + 1]));
            assert!(shares_eq(&t.gen_z_shares[j * n + 1], &before_gen_z[j * n + 0]));
            assert!(shares_eq(&t.gen_z_shares[j * n + 2], &before_gen_z[j * n + 2]));
            assert!(shares_eq(&t.gen_z_shares[j * n + 3], &before_gen_z[j * n + 3]));
            assert!(shares_eq(&t.eval_z_shares[j * n + 0], &before_eval_z[j * n + 1]));
            assert!(shares_eq(&t.eval_z_shares[j * n + 1], &before_eval_z[j * n + 0]));
        }
    }

    #[test]
    #[should_panic(expected = "apply_permutation_to_triple: perm.len() must equal n")]
    fn test_apply_permutation_wrong_length_panics() {
        let triples = make_triples(4, 2, 1);
        let mut t = triples[0].clone();
        let bad_perm = vec![0usize, 1, 2]; // length 3 != n=4
        apply_permutation_to_triple(&mut t, &bad_perm);
    }

    #[test]
    fn test_bucket_size_formula() {
        // Construction 4 (Appendix F): B = 1 + ceil(SSP / log2(n * ell)), SSP = 40.
        assert_eq!(bucket_size_for(4, 1), 21);   // 1 + ceil(40 / log2(4))  = 1 + 20
        assert_eq!(bucket_size_for(4, 2), 15);   // 1 + ceil(40 / log2(8))  = 1 + ceil(40/3) = 1 + 14
        assert_eq!(bucket_size_for(16, 1), 11);  // 1 + ceil(40 / log2(16)) = 1 + 10
    }

    #[test]
    fn test_bucket_size_formula_edge_cases() {
        // product = n * ell <= 1 → SSP fallback per D-02.
        assert_eq!(bucket_size_for(1, 0), 40, "n*ell=0 must return SSP fallback");
        assert_eq!(bucket_size_for(1, 1), 40, "n*ell=1 must return SSP fallback");
    }

    #[test]
    fn test_combine_dimensions() {
        let n = 4;
        let m = 4;
        let b = 2;
        let triples = make_triples(n, m, b);
        let (gen_out, eval_out) = combine_leaky_triples(triples, b, n, m, 1, 42);
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), n);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), n * m);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), n * m);
    }

    #[test]
    fn test_full_pipeline_no_panic() {
        let n = 4;
        let m = 4;
        let b = bucket_size_for(n, 1);
        let triples = make_triples(n, m, b);
        let (fpre_gen, fpre_eval) = combine_leaky_triples(triples, b, n, m, 1, 99);
        let _gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let _ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
        // No panic = success
    }

    #[test]
    fn test_two_to_one_combine_product_invariant() {
        // TEST-05 happy path: two concrete leaky triples, combine once, verify the paper's
        // product invariant Z_combined[j*n+i] = x_combined[i] AND y_combined[j]
        // (Construction 3 correctness, appendix_krrw_pre.tex line 443).
        let n = 4;
        let m = 4;
        let triples = make_triples(n, m, 2);
        let t0 = triples[0].clone();
        let t1_ref = &triples[1];

        let combined = two_to_one_combine(t0, t1_ref);

        // MAC invariant on every combined share (sanity that d-reveal didn't corrupt shares).
        for i in 0..n {
            verify_cross_party(
                &combined.gen_x_shares[i],
                &combined.eval_x_shares[i],
                &combined.delta_a,
                &combined.delta_b,
            );
        }
        for j in 0..m {
            verify_cross_party(
                &combined.gen_y_shares[j],
                &combined.eval_y_shares[j],
                &combined.delta_a,
                &combined.delta_b,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &combined.gen_z_shares[k],
                &combined.eval_z_shares[k],
                &combined.delta_a,
                &combined.delta_b,
            );
        }

        // Product invariant: Z_full[j*n+i] == x_full[i] AND y_full[j].
        let x_full: Vec<bool> = (0..n)
            .map(|i| combined.gen_x_shares[i].value ^ combined.eval_x_shares[i].value)
            .collect();
        let y_full: Vec<bool> = (0..m)
            .map(|j| combined.gen_y_shares[j].value ^ combined.eval_y_shares[j].value)
            .collect();
        for j in 0..m {
            for i in 0..n {
                let k = j * n + i;
                let z_full =
                    combined.gen_z_shares[k].value ^ combined.eval_z_shares[k].value;
                assert_eq!(
                    z_full,
                    x_full[i] & y_full[j],
                    "TEST-05 product invariant failed at (i={}, j={}, k={})",
                    i,
                    j,
                    k
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "MAC mismatch in share")]
    fn test_two_to_one_combine_tampered_d_panics() {
        // TEST-05 tamper path: flip one y'' value bit on the eval side without touching
        // the MAC. The assembled d[0] share (d = y' XOR y'') now has inconsistent
        // (value, mac, key) and verify_cross_party inside two_to_one_combine Step B
        // detects the mismatch and panics. Matches the paper's "publicly reveal with
        // appropriate MACs" abort semantics.
        let n = 2;
        let m = 2;
        let triples = make_triples(n, m, 2);
        let t0 = triples[0].clone();
        let mut t1 = triples[1].clone();

        // Tamper: flip the value bit of eval_y_shares[0] without updating the MAC.
        // The assembled d share for j=0 will fail verify_cross_party.
        t1.eval_y_shares[0].value = !t1.eval_y_shares[0].value;

        // Must panic with "MAC mismatch in share" inside two_to_one_combine Step B.
        let _ = two_to_one_combine(t0, &t1);
    }

    #[test]
    fn test_combine_full_bucket_product_invariant() {
        // TEST-05 complement: verify the iterative fold in combine_leaky_triples produces
        // a tensor triple that still satisfies the product invariant over a full bucket
        // (B = bucket_size_for(n, 1) = 21 for n=4). Catches regressions in the fold wrapper beyond
        // the two-triple unit test.
        let n = 4;
        let m = 4;
        let b = bucket_size_for(n, 1); // Construction 4: 1 + ceil(40/log2(4)) = 21
        assert_eq!(b, 21, "bucket_size_for(4, 1) must return 21 per Construction 4");

        let triples = make_triples(n, m, b);
        let (gen_out, eval_out) = combine_leaky_triples(triples, b, n, m, 1, 0);

        // delta invariants preserved through the fold
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), n);
        assert_eq!(gen_out.beta_auth_bit_shares.len(), m);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), n * m);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), n * m);

        // MAC invariant on every output share
        for i in 0..n {
            verify_cross_party(
                &gen_out.alpha_auth_bit_shares[i],
                &eval_out.alpha_auth_bit_shares[i],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
        for j in 0..m {
            verify_cross_party(
                &gen_out.beta_auth_bit_shares[j],
                &eval_out.beta_auth_bit_shares[j],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &gen_out.correlated_auth_bit_shares[k],
                &eval_out.correlated_auth_bit_shares[k],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }

        // Product invariant after B = 40 iterative folds
        let x_full: Vec<bool> = (0..n)
            .map(|i| {
                gen_out.alpha_auth_bit_shares[i].value
                    ^ eval_out.alpha_auth_bit_shares[i].value
            })
            .collect();
        let y_full: Vec<bool> = (0..m)
            .map(|j| {
                gen_out.beta_auth_bit_shares[j].value
                    ^ eval_out.beta_auth_bit_shares[j].value
            })
            .collect();
        for j in 0..m {
            for i in 0..n {
                let k = j * n + i;
                let z_full = gen_out.correlated_auth_bit_shares[k].value
                    ^ eval_out.correlated_auth_bit_shares[k].value;
                assert_eq!(
                    z_full,
                    x_full[i] & y_full[j],
                    "full-bucket product invariant failed at (i={}, j={}, k={}, B={})",
                    i,
                    j,
                    k,
                    b
                );
            }
        }
    }

    #[test]
    fn test_run_preprocessing_product_invariant_construction_4() {
        // TEST-06 (Phase 6): end-to-end Pi_aTensor' / Construction 4 invariant.
        // Generate an authenticated tensor triple via the full preprocessing
        // pipeline (IdealBCot → LeakyTensorPre × B → combine_leaky_triples with
        // per-triple permutation → TensorFpreGen/Eval) and assert:
        //   1. MAC invariant on every x, y, z share (verify_cross_party).
        //   2. Product invariant Z_full[j*n+i] == x_full[i] & y_full[j]
        //      (identical shape as test_combine_full_bucket_product_invariant
        //      but entered via run_preprocessing, not combine_leaky_triples).
        //   3. Dimensions: |alpha| = n, |beta| = m, |correlated| = n*m
        //      on both parties' outputs.
        //   4. D-12 bucket-size improvement: bucket_size_for(4, 1) == 21 < 40.

        let n = 4usize;
        let m = 4usize;

        // D-12 pin: confirm Construction 4's bucket is smaller than Construction 3's 40.
        let b_new = bucket_size_for(n, 1);
        assert_eq!(b_new, 21, "Construction 4 bucket_size_for(4, 1) must be 21");
        assert!(b_new < 40, "Construction 4 B must be smaller than Construction 3 B=40");

        // Full pipeline (includes permutation + iterative fold + bucket reduction).
        let (gen_out, eval_out) = crate::preprocessing::run_preprocessing(n, m, 1, 1);

        // (3) Dimensions on both sides.
        assert_eq!(gen_out.alpha_auth_bit_shares.len(), n);
        assert_eq!(gen_out.beta_auth_bit_shares.len(), m);
        assert_eq!(gen_out.correlated_auth_bit_shares.len(), n * m);
        assert_eq!(eval_out.alpha_auth_bit_shares.len(), n);
        assert_eq!(eval_out.beta_auth_bit_shares.len(), m);
        assert_eq!(eval_out.correlated_auth_bit_shares.len(), n * m);

        // (1) Cross-party MAC invariant on every share.
        for i in 0..n {
            verify_cross_party(
                &gen_out.alpha_auth_bit_shares[i],
                &eval_out.alpha_auth_bit_shares[i],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
        for j in 0..m {
            verify_cross_party(
                &gen_out.beta_auth_bit_shares[j],
                &eval_out.beta_auth_bit_shares[j],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }
        for k in 0..(n * m) {
            verify_cross_party(
                &gen_out.correlated_auth_bit_shares[k],
                &eval_out.correlated_auth_bit_shares[k],
                &gen_out.delta_a,
                &eval_out.delta_b,
            );
        }

        // (2) Product invariant: Z_full[j*n+i] == x_full[i] & y_full[j].
        let x_full: Vec<bool> = (0..n)
            .map(|i| {
                gen_out.alpha_auth_bit_shares[i].value
                    ^ eval_out.alpha_auth_bit_shares[i].value
            })
            .collect();
        let y_full: Vec<bool> = (0..m)
            .map(|j| {
                gen_out.beta_auth_bit_shares[j].value
                    ^ eval_out.beta_auth_bit_shares[j].value
            })
            .collect();
        for j in 0..m {
            for i in 0..n {
                let k = j * n + i;
                let z_full = gen_out.correlated_auth_bit_shares[k].value
                    ^ eval_out.correlated_auth_bit_shares[k].value;
                assert_eq!(
                    z_full,
                    x_full[i] & y_full[j],
                    "TEST-06 product invariant failed at (i={}, j={}, k={})",
                    i,
                    j,
                    k
                );
            }
        }
    }
}
