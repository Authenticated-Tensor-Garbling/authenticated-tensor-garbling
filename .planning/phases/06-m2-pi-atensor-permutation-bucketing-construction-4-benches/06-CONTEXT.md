# Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement Pi_aTensor' (Construction 4, `references/appendix_krrw_pre.tex`) on top of the
Phase 5 two-to-one combining infrastructure. Three concrete changes:

1. **Permutation bucketing (PROTO-13, PROTO-14):** Before folding, apply a uniformly
   random row-permutation π_j ∈ S_n to each triple's `itmac{x^(j)}{Δ}` and
   `itmac{Z^(j)}{Δ}` rows; leave `itmac{y^(j)}{Δ}` unchanged. Permutation is
   seeded per-triple from `_shuffle_seed XOR triple_index`.

2. **Improved bucket size formula (PROTO-15):** Replace Construction 3 formula
   `floor(SSP / log2(ell)) + 1` with Construction 4 formula
   `1 + ceil(SSP / log2(n·ell))` — better amortization because it exploits both
   tensor dimension n and output count ell.

3. **Benchmarks (TEST-07):** Update `bucket_size_for` call site in `run_preprocessing`
   (now 2-arg) and bench label (Construction 3 → 4); confirm `cargo bench` runs clean.

Requirements in scope: PROTO-13, PROTO-14, PROTO-15, TEST-06, TEST-07.
Out of scope: real OT, batch output (count > 1), network layer.

</domain>

<decisions>
## Implementation Decisions

### bucket_size_for API Change (PROTO-15)

- **D-01:** Replace `bucket_size_for(ell: usize) -> usize` with
  `bucket_size_for(n: usize, ell: usize) -> usize`. One function, no dead code —
  Construction 3 formula disappears from the codebase.
- **D-02:** New formula: `B = 1 + ceil(SSP / log2(n * ell))` for `n * ell >= 2`.
  When `n * ell <= 1`, fall back to `B = SSP` (= 40) — same edge-case logic as
  Phase 5's `ell <= 1` guard.
  Concrete: `1 + (SSP + log2_floor(n*ell) - 1) / log2_floor(n*ell)` using integer
  arithmetic, where `log2_floor(k) = usize::BITS - k.leading_zeros() - 1`.
- **D-03:** Update ALL call sites in this crate:
  - `src/preprocessing.rs:run_preprocessing` — change `bucket_size_for(count)` to
    `bucket_size_for(n, count)`.
  - `auth_tensor_pre.rs` tests that call `bucket_size_for(...)` directly.
- **D-04:** Update `bucket_size_for` doc examples and test values to match new formula.
  `bucket_size_for(4, 1)` = `1 + ceil(40 / log2(4)) = 1 + 20 = 21`; compare with old
  `bucket_size_for(1) = 40`.

### Permutation Application Site (PROTO-13, PROTO-14)

- **D-05:** Permutation happens **inside `combine_leaky_triples`**, activating the
  existing `_shuffle_seed: u64` parameter (rename to `shuffle_seed`). No change to
  callers. The combiner is now Construction 4.
- **D-06:** Permutation step: before the iterative fold loop, apply π_j to every triple
  in the input `Vec<LeakyTriple>`. Permute in-place (or a permuted copy):
  - Reorder `gen_x_shares[0..n]` and `eval_x_shares[0..n]` by π_j.
  - For Z (column-major j*n+i): reorder the i-index within each column — `Z_new[j*n+i]
    = Z_old[j*n + π_j(i)]` for all j in 0..m, i in 0..n. Equivalently, for each column
    j, permute the contiguous slice `Z[j*n .. (j+1)*n]` by π_j.
  - `gen_y_shares` and `eval_y_shares` are NOT permuted.
- **D-07:** Add a `pub(crate) fn apply_permutation_to_triple(triple: &mut LeakyTriple,
  perm: &[usize])` helper. This keeps `combine_leaky_triples` readable and makes the
  permutation step directly unit-testable.

### Permutation RNG / Seed (PROTO-13)

- **D-08:** Per-triple permutation RNG: `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)`
  where `j` is the triple's index in the input vec (0-based). This gives each triple an
  independent, deterministic, reproducible permutation from a single caller-provided seed.
- **D-09:** `run_preprocessing` passes `shuffle_seed = 42` (non-zero fixed seed for
  determinism in tests; a constant). For bench runs the seed value doesn't affect
  benchmark correctness.
- **D-10:** Fisher-Yates shuffle on `(0..n).collect::<Vec<usize>>()` with the per-triple
  `ChaCha12Rng` produces π_j. Consistent with the codebase's existing `rand::Rng` usage
  pattern in `leaky_tensor_pre.rs`.

### TEST-06: End-to-End Product Invariant

- **D-11:** TEST-06 verifies: after `run_preprocessing` with Construction 4, the output
  `(TensorFpreGen, TensorFpreEval)` satisfies the product invariant `Z_full = x_full ⊗
  y_full` (same assertion pattern as TEST-05's `test_combine_full_bucket_product_invariant`)
  and the MAC invariant holds on every share. No explicit permutation-was-applied assertion
  (product invariant passing implies permutation is consistent with the algebra).
- **D-12:** TEST-06 also verifies that the new `bucket_size_for(n, ell)` returns a smaller
  B than the old formula: for n=4, ell=1, new B=21 < old B=40, confirming the formula
  improvement is live.

### Benchmarks (TEST-07)

- **D-13:** The existing preprocessing bench (`benches/benchmarks.rs`, comment:
  "Pi_aTensor / Construction 3, Appendix F") gets its comment updated to
  "Pi_aTensor' / Construction 4, Appendix F" and the `bucket_size_for` call site updated.
  No structural change to the bench — it already calls `run_preprocessing` which will
  automatically use Construction 4 after Phase 6.
- **D-14:** TEST-07 success criterion: `cargo bench --no-run` compiles cleanly (full
  `cargo bench` run is slow; compile check is sufficient for CI-style verification).

### Claude's Discretion

- Whether `apply_permutation_to_triple` lives in `auth_tensor_pre.rs` or as a standalone
  free function; either is fine as long as it is `pub(crate)`.
- Whether to use `rand::seq::SliceRandom::shuffle` or a manual Fisher-Yates — either
  produces a uniform permutation with the ChaCha12Rng.
- Exact name for the renamed `_shuffle_seed` parameter — `shuffle_seed` is the obvious
  choice.
- Whether test D-12 (bucket size comparison) is a separate test function or folded into
  the main TEST-06 product-invariant test.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `references/appendix_krrw_pre.tex` — Construction 4 (Pi_aTensor'): permutation
  bucketing step (sample π_j, apply to x and Z rows, leave y unchanged), improved
  bucket size formula `B = 1 + ceil(SSP / log2(n·ℓ))`. Compare with Construction 3
  (Theorem 1) formula used in Phase 5.

### Source Files Under Rewrite
- `src/auth_tensor_pre.rs` — `bucket_size_for` (signature + formula), `combine_leaky_triples`
  (activate shuffle_seed, add permutation step before fold), new `apply_permutation_to_triple`
  helper.
- `src/preprocessing.rs` — `run_preprocessing`: update `bucket_size_for(count)` call to
  `bucket_size_for(n, count)`, change `_shuffle_seed` argument from `0` to `42`.

### Upstream Context (Phase 5)
- `.planning/phases/05-m2-pi-atensor-correct-combining-construction-3/05-CONTEXT.md` —
  D-03: Z stored column-major (j*n+i); D-05/D-06: `_shuffle_seed` parameter already
  reserved; D-11/D-12: `two_to_one_combine` and `combine_leaky_triples` signatures.

### Benchmarks
- `benches/benchmarks.rs` — preprocessing bench at bottom of file; comment update only
  (Construction 3 → 4), no structural change.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, _shuffle_seed)` —
  already has the permutation seed parameter stubbed; activate it and add the permutation
  step before the fold loop.
- `ChaCha12Rng::seed_from_u64` — used in `leaky_tensor_pre.rs` for per-instance seed;
  same pattern for per-triple permutation RNG.
- `rand::Rng` (already in Cargo.toml) — provides `shuffle` or index sampling for
  Fisher-Yates permutation.
- `make_triples(n, m, count)` in `auth_tensor_pre.rs` tests — reuse for TEST-06 setup.
- `verify_cross_party` helper (`pub(crate)`) — reuse in TEST-06 MAC invariant assertions.

### Established Patterns
- Per-instance seeded RNG: `ChaCha12Rng::seed_from_u64(seed)` in `leaky_tensor_pre.rs`
- Column-major tensor indexing: `j * n + i` — must be respected when permuting Z rows
- `#[cfg(test)] mod tests { ... }` inline at bottom of module — all tests go there
- `pub(crate)` for internal helpers (`verify_cross_party`, `two_to_one_combine`)

### Integration Points
- `run_preprocessing` in `src/preprocessing.rs:85`: `bucket_size_for(count)` → `bucket_size_for(n, count)`
- `combine_leaky_triples(..., 0)` → `combine_leaky_triples(..., 42)` in `run_preprocessing`
- `benches/benchmarks.rs`: comment update, `bucket_size_for` call site if directly referenced

</code_context>

<specifics>
## Specific Ideas

- User confirmed: replace `bucket_size_for(ell)` with 2-arg `bucket_size_for(n, ell)` — no
  parallel Construction 3 function needed.
- User confirmed: permutation goes inside `combine_leaky_triples`, activating `_shuffle_seed`.
- User confirmed: per-triple seed = `ChaCha12Rng::seed_from_u64(shuffle_seed ^ j as u64)`.
- User confirmed: TEST-06 = product invariant only (no explicit permutation non-triviality
  check); permutation correctness implied by product invariant holding.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 6 scope.

</deferred>

---

*Phase: 06-m2-pi-atensor-permutation-bucketing-construction-4-benches*
*Context gathered: 2026-04-22*
