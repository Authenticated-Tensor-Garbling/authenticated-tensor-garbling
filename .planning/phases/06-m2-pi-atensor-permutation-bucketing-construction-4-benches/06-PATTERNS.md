# Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches - Pattern Map

**Mapped:** 2026-04-22
**Files analyzed:** 3
**Analogs found:** 3 / 3

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/auth_tensor_pre.rs` | utility/service | transform | `src/leaky_tensor_pre.rs` | exact (same module role; ChaCha12Rng seeding pattern) |
| `src/preprocessing.rs` | service | request-response | `src/preprocessing.rs` (self) | self-referential edit |
| `benches/benchmarks.rs` | config | request-response | `benches/benchmarks.rs` (self) | self-referential edit |

---

## Pattern Assignments

### `src/auth_tensor_pre.rs` — Three targeted changes

All three changes happen in the same file. The analog for the new patterns
is `src/leaky_tensor_pre.rs`, which is the only other file in the codebase
that uses `ChaCha12Rng::seed_from_u64` and index-based permutation logic
over `LeakyTriple` vectors.

---

#### Change 1: `bucket_size_for` — signature + formula replacement

**Current signature** (`src/auth_tensor_pre.rs` lines 134–141):
```rust
pub fn bucket_size_for(ell: usize) -> usize {
    const SSP: usize = 40;
    if ell <= 1 {
        return SSP;
    }
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

**New signature and formula** (replace in full):
```rust
/// Compute the bucket size B for Pi_aTensor' (Construction 4, Appendix F).
///
/// Formula: `B = 1 + ceil(SSP / log2(n * ell))` for `n * ell >= 2`, where SSP = 40.
/// For `n * ell <= 1`, fall back to B = SSP (degenerate amplification bound).
///
/// Integer ceiling: `1 + (SSP + log2_floor(n*ell) - 1) / log2_floor(n*ell)`.
/// `log2_floor(k) = usize::BITS - k.leading_zeros() - 1`.
///
/// Parameters:
///   n   — tensor row dimension.
///   ell — number of OUTPUT authenticated tensor triples desired.
///
/// Examples:
///   bucket_size_for(4, 1)    = 21   (1 + ceil(40 / log2(4)) = 1 + 20)
///   bucket_size_for(4, 2)    = 14   (1 + ceil(40 / log2(8)) = 1 + ceil(40/3))
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
```

**integer-ceil pattern:** `(numerator + denominator - 1) / denominator` — this is the
standard Rust integer ceiling division idiom used in the codebase (no external dep).

**log2_floor helper:** `(usize::BITS - k.leading_zeros() - 1) as usize` — already used
verbatim in the current `bucket_size_for` at `src/auth_tensor_pre.rs` line 139. Copy
directly; the only change is the argument (`n*ell` instead of `ell`) and the ceiling
vs floor distinction.

---

#### Change 2: `apply_permutation_to_triple` — new `pub(crate)` helper

**Analog for `pub(crate)` visibility:** `src/auth_tensor_pre.rs` line 247:
```rust
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
```

**Analog for ChaCha12Rng seeding** (`src/leaky_tensor_pre.rs` lines 18–19, 73):
```rust
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
// ...
rng: ChaCha12Rng::seed_from_u64(seed),
```

**Analog for column-major Z indexing** (`src/auth_tensor_pre.rs` lines 81–83):
```rust
for j in 0..m {
    for i in 0..n {
        let k = j * n + i;
```

**New helper to implement:**
```rust
/// Apply a row permutation `perm` (a permutation of 0..n) to the x and Z rows
/// of `triple` in-place. y rows are NOT permuted (per Construction 4, D-06).
///
/// Permutes:
///   gen_x_shares  and eval_x_shares  by perm[i] → position i   (length n)
///   gen_z_shares  and eval_z_shares: for each column j in 0..m,
///     permute the contiguous slice [j*n .. (j+1)*n] by perm.
///
/// `perm` must be a valid permutation of 0..n; panics if perm.len() != n.
pub(crate) fn apply_permutation_to_triple(triple: &mut LeakyTriple, perm: &[usize]) {
    let n = triple.n;
    let m = triple.m;
    assert_eq!(perm.len(), n, "apply_permutation_to_triple: perm.len() must equal n");

    // Permute x shares (length n) —
    // build new vecs by reading position perm[i] from the original.
    let orig_gen_x = triple.gen_x_shares.clone();
    let orig_eval_x = triple.eval_x_shares.clone();
    for i in 0..n {
        triple.gen_x_shares[i]  = orig_gen_x[perm[i]];
        triple.eval_x_shares[i] = orig_eval_x[perm[i]];
    }

    // Permute Z shares column-major: for each column j, permute the i-index
    // within the contiguous slice [j*n .. (j+1)*n].
    let orig_gen_z  = triple.gen_z_shares.clone();
    let orig_eval_z = triple.eval_z_shares.clone();
    for j in 0..m {
        for i in 0..n {
            triple.gen_z_shares[j * n + i]  = orig_gen_z[j * n + perm[i]];
            triple.eval_z_shares[j * n + i] = orig_eval_z[j * n + perm[i]];
        }
    }
}
```

**Note on shuffle crate API:** `rand::seq::SliceRandom::shuffle` is available in
rand 0.9 (Cargo.toml line 13: `rand = "0.9"`). It is NOT yet used elsewhere in this
codebase, so the planner must add the import. Alternatively, build `perm` via
Fisher-Yates on `(0..n).collect::<Vec<usize>>()` using `rng.random_range(i..n)` swaps
— either works; `SliceRandom::shuffle` is the idiomatic choice.

---

#### Change 3: `combine_leaky_triples` — activate shuffle_seed + add permutation step

**Current signature** (`src/auth_tensor_pre.rs` lines 161–168):
```rust
pub fn combine_leaky_triples(
    triples: Vec<LeakyTriple>,
    bucket_size: usize,
    n: usize,
    m: usize,
    chunking_factor: usize,
    _shuffle_seed: u64,
) -> (TensorFpreGen, TensorFpreEval) {
```

**Change:** rename `_shuffle_seed` to `shuffle_seed` (remove leading underscore).
No change to the public type signature — all callers already pass a `u64`.

**Per-triple RNG seeding pattern** (analog: `src/leaky_tensor_pre.rs` line 73):
```rust
rng: ChaCha12Rng::seed_from_u64(seed),
```

**Permutation loop to insert** — place immediately after the delta-consistency
assertion block (currently ending at line 191) and before the iterative fold
starting at line 196. Use `triples` as `mut` for in-place permutation:

```rust
// ---- Construction 4 permutation step (PROTO-13, PROTO-14) ----
// For each triple j, sample a fresh per-triple ChaCha12 RNG from
// (shuffle_seed XOR j), generate π_j as a permutation of 0..n,
// and apply it to the x and Z rows (y rows are left unchanged).
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use rand::seq::SliceRandom;

let mut triples = triples; // rebind as mut (was already owned)
for (j, triple) in triples.iter_mut().enumerate() {
    let mut rng = ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64);
    let mut perm: Vec<usize> = (0..n).collect();
    perm.shuffle(&mut rng);
    apply_permutation_to_triple(triple, &perm);
}
```

**Fold loop** (unchanged, lines 196–199 — copy verbatim):
```rust
let mut acc: LeakyTriple = triples[0].clone();
for next in triples.iter().skip(1) {
    acc = two_to_one_combine(acc, next);
}
```

**Import additions** needed at top of `src/auth_tensor_pre.rs` (lines 1–6 currently):
```rust
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha12Rng;
```

These are already present in `src/leaky_tensor_pre.rs` lines 18–19 for exact reference.

---

### `src/preprocessing.rs` — Two-line edit in `run_preprocessing`

**Current call sites** (lines 93 and 109):
```rust
let bucket_size = bucket_size_for(count);          // line 93
// ...
combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 0)  // line 109
```

**New call sites:**
```rust
let bucket_size = bucket_size_for(n, count);       // D-03: add n as first arg
// ...
combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42) // D-09: seed 42
```

**Doc comment** (line 60): update "Construction 3" reference to "Construction 4" to
match the implementation. The rest of the function body is unchanged.

**Test updates** in `src/preprocessing.rs` lines 112–139: the three existing tests
call `super::run_preprocessing(4, 4, 1, 1)` with no direct `bucket_size_for` call,
so they require no change. Their assertions remain valid under Construction 4.

---

### `benches/benchmarks.rs` — Comment update only

**Target location** (line 557):
```rust
// Benchmarks the uncompressed preprocessing pipeline (Pi_aTensor / Construction 3, Appendix F): ...
fn bench_preprocessing(c: &mut Criterion) {
```

**Change:** replace `Construction 3` with `Construction 4` in the doc comment on
line 557. No other structural change — `bench_preprocessing` calls
`run_preprocessing(n, m, 1, chunking_factor)` at line 590, which will automatically
use Construction 4 after `preprocessing.rs` is updated.

**Confirm:** `bucket_size_for` is NOT called directly in `benches/benchmarks.rs`
(verified by full read). No call site update needed in this file.

---

## Shared Patterns

### pub(crate) helper visibility
**Source:** `src/auth_tensor_pre.rs` lines 247–265 (`verify_cross_party`)
**Apply to:** `apply_permutation_to_triple`
```rust
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    ...
) {
```
Use the same `pub(crate)` visibility; place the new helper above or below
`verify_cross_party` in the same file.

### ChaCha12Rng per-instance seeding
**Source:** `src/leaky_tensor_pre.rs` lines 18–19 and 73
```rust
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
// ...
rng: ChaCha12Rng::seed_from_u64(seed),
```
**Apply to:** per-triple RNG in `combine_leaky_triples`.
Per-triple seed = `shuffle_seed ^ j as u64` (D-08).

### Column-major tensor indexing
**Source:** `src/auth_tensor_pre.rs` lines 81–83 and `src/leaky_tensor_pre.rs` lines 243–246
```rust
for j in 0..m {
    for i in 0..n {
        let k = j * n + i;
```
**Apply to:** Z-slice permutation inside `apply_permutation_to_triple`.

### inline test module convention
**Source:** `src/auth_tensor_pre.rs` lines 267–487 (`#[cfg(test)] mod tests { ... }`)
**Apply to:** TEST-06 and D-12 tests — add inside the existing `mod tests` block,
after `test_combine_full_bucket_product_invariant`.

### make_triples / verify_cross_party reuse in tests
**Source:** `src/auth_tensor_pre.rs` lines 277–286 (`make_triples`) and lines 247–265
(`verify_cross_party`)
**Apply to:** TEST-06 setup. Call `make_triples(n, m, b)` with `b = bucket_size_for(n, 1)`,
then `combine_leaky_triples(triples, b, n, m, 1, 42)` and apply the same MAC + product
invariant loop from `test_combine_full_bucket_product_invariant` (lines 432–484).

### bucket_size_for test assertions
**Source:** `src/auth_tensor_pre.rs` lines 289–301 (`test_bucket_size_formula`,
`test_bucket_size_formula_edge_cases`)
**Apply to:** Replace these two tests entirely with Construction 4 values:
```rust
// Construction 4: bucket_size_for(n, ell)
assert_eq!(bucket_size_for(4, 1), 21);   // 1 + ceil(40/log2(4)) = 1+20
assert_eq!(bucket_size_for(4, 2), 14);   // 1 + ceil(40/log2(8)) = 1+ceil(13.3)=1+14
assert_eq!(bucket_size_for(16, 1), 11);  // 1 + ceil(40/log2(16)) = 1+10
// Edge cases
assert_eq!(bucket_size_for(1, 0), 40);   // product=0 -> SSP fallback
assert_eq!(bucket_size_for(1, 1), 40);   // product=1 -> SSP fallback
```

### Test D-12 (bucket size improvement confirmation)
**Inline** into the TEST-06 function or as a separate function:
```rust
// D-12: Construction 4 gives smaller B than Construction 3 for n=4, ell=1
let b_new = bucket_size_for(4, 1);  // 21
assert_eq!(b_new, 21, "Construction 4 bucket_size_for(4,1) must be 21");
assert!(b_new < 40, "Construction 4 B must be smaller than Construction 3 B=40");
```

---

## No Analog Found

None — all three files have direct analogs or are self-referential edits.

---

## Metadata

**Analog search scope:** `src/`, `benches/`
**Files read:** `src/auth_tensor_pre.rs` (488 lines), `src/preprocessing.rs` (140 lines),
`benches/benchmarks.rs` (615 lines), `src/leaky_tensor_pre.rs` (643 lines), `Cargo.toml`
**Pattern extraction date:** 2026-04-22
