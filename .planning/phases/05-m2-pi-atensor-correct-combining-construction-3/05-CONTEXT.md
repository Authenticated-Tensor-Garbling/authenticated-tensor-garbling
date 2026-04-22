# Phase 5: M2 Pi_aTensor Correct Combining (Construction 3) - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Rewrite `combine_leaky_triples` in `src/auth_tensor_pre.rs` to implement the paper's
two-to-one combining procedure (§3, "Combining Leaky Tensor Triples"): set
`x = x' ⊕ x''`, keep `y = y'`, publicly reveal `d = y' ⊕ y''` with IT-MAC
verification, compute `Z = Z' ⊕ Z'' ⊕ (itmac{x''}{Δ} ⊗ d)`.

Fix `bucket_size_for` to accept `ell` (number of output triples) instead of `n·m`.

Add TEST-05: verify `Z_combined = Z' ⊕ Z'' ⊕ x'' ⊗ d` on two concrete leaky triples
and confirm IT-MAC verification on d rejects tampered values.

Requirements in scope: PROTO-10, PROTO-11, PROTO-12, TEST-05.
Out of scope: permutation bucketing and the Pi_aTensor' improved bucket size formula
(Phase 6).

</domain>

<decisions>
## Implementation Decisions

### Two-to-One Combining: x output (PROTO-10)

- **D-01:** The combined `itmac{x}{Δ}` is `itmac{x'}{Δ} ⊕ itmac{x''}{Δ}` — XOR of
  both triples' x shares. The ROADMAP description "keep x = x'" was a shorthand error;
  the paper (appendix_krrw_pre.tex line 427) is unambiguous. Concretely: `gen_x_shares`
  and `eval_x_shares` are XOR-combined across all B triples in the bucket (same loop
  structure as z combining).
- **D-02:** `itmac{y}{Δ} := itmac{y'}{Δ}` — keep the first triple's y shares unchanged
  (no combining needed for y). This is correct per the paper.

### Two-to-One Combining: Z output (PROTO-10)

- **D-03:** `Z = Z' ⊕ Z'' ⊕ (itmac{x''}{Δ} ⊗ d)` where d is the publicly revealed
  bit vector of length m. The tensor product `itmac{x''}{Δ} ⊗ d` is computed locally
  by both parties: for each (i, j) pair, the IT-MAC share at column-major index j*n+i
  is `x''_shares[i]` if `d[j] == 1`, else `AuthBitShare::zero_share` (key=0, mac=0,
  value=false). No GGM macro call needed — d is public so the computation requires no
  interaction.
- **D-04:** Z storage convention is unchanged from Phase 4: `Vec<AuthBitShare>` in
  column-major order (index j*n+i), length n*m.

### d Reveal and MAC Verification (PROTO-10)

- **D-05:** `d[j] = y'_j ⊕ y''_j` (bit XOR of the value fields of the two y shares at
  index j). Each party assembles their IT-MAC share of d_j by XORing the key/mac/value
  fields of their y' and y'' AuthBitShares.
- **D-06:** Before using d to compute Z, call `AuthBitShare::verify(delta)` on each
  assembled d share (both gen-side and eval-side). This is the in-process substitute for
  "publicly reveal with appropriate MACs" from the paper. Verification failure panics
  (same convention as F_eq).
- **D-07:** d is a `Vec<bool>` of length m extracted from the assembled d shares after
  verification. The tensor product computation (D-03) uses these bool values directly.

### Bucket Size Formula Fix (PROTO-12)

- **D-08:** `bucket_size_for(ell: usize) -> usize` replaces `bucket_size_for(n: usize,
  m: usize) -> usize`. The parameter `ell` is the number of OUTPUT authenticated tensor
  triples (not the tensor dimensions n·m).
- **D-09:** Formula: `B = floor(SSP / log2(ell)) + 1` for ell ≥ 2. When `ell ≤ 1`,
  return `SSP` (= 40). This matches the naive combining approach from the paper's §3.1:
  without bucketing amortization, you need SSP triples to reach 2^−ρ security.
- **D-10:** Call site in `run_preprocessing`: change `bucket_size_for(n, m)` to
  `bucket_size_for(count)`. With `count = 1` (current use), B = SSP = 40.

### Iterative Combining Structure (PROTO-11)

- **D-11:** Implement a pub(crate) `fn two_to_one_combine(prime: LeakyTriple,
  dprime: &LeakyTriple) -> LeakyTriple` helper that performs one combining step. This
  makes TEST-05 directly testable on two LeakyTriples without going through the full
  bucket pipeline, and keeps `combine_leaky_triples` as a thin iterative wrapper.
- **D-12:** `combine_leaky_triples` folds B triples one at a time:
  start with `triples[0]`, then iteratively call `two_to_one_combine(acc, &triples[i])`
  for i in 1..B. The final accumulated LeakyTriple is converted to
  (TensorFpreGen, TensorFpreEval) at the end — same return type as today.

### Claude's Discretion

- Exact zero-share representation for `AuthBitShare` when `d[j] == 0` in the tensor
  product — use `AuthBitShare { key: Key::ZERO, mac: Mac::ZERO, value: false }` if
  those constants exist, else construct from Block::ZERO.
- Whether the delta_a/delta_b same-delta assertion in `combine_leaky_triples` is moved
  into `two_to_one_combine` or kept in the outer wrapper.
- Loop ordering (iterate over j then i, or flat index k) for the d ⊗ x'' computation.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `references/appendix_krrw_pre.tex` — §3 "Combining Leaky Tensor Triples" (lines
  415–444): the two-to-one combining step, x/y/Z formulas, d definition and public
  reveal, local computation of itmac{x''}{Δ} ⊗ d
- `references/appendix_krrw_pre.tex` — §3 "Randomized Bucketing" Construction 3
  (lines 449–546): bucket size formula B = floor(SSP / log2(ℓ)) + 1, iterative
  combining across each bucket

### Source Files Under Rewrite
- `src/auth_tensor_pre.rs` — `combine_leaky_triples` (algorithm rewrite) and
  `bucket_size_for` (signature + formula fix); new `two_to_one_combine` helper added
- `src/preprocessing.rs` — `bucket_size_for(n, m)` call site changes to
  `bucket_size_for(count)`

### Source Files to Call (Existing, Unchanged API)
- `src/leaky_tensor_pre.rs` — `LeakyTriple { gen_x_shares, eval_x_shares,
  gen_y_shares, eval_y_shares, gen_z_shares, eval_z_shares, n, m, delta_a, delta_b }`;
  output of Phase 4; inputs to two_to_one_combine
- `src/sharing.rs` — `AuthBitShare { key, mac, value }` and `AuthBitShare::verify(&delta)`
  for IT-MAC verification on d shares
- `src/block.rs` — `Block::ZERO` for zero-share construction
- `src/delta.rs` — `Delta` type for verify calls

### Upstream Context
- `.planning/phases/04-m2-pi-leakytensor-f-eq-construction-2/04-CONTEXT.md` — D-08:
  Z stored as Vec<AuthBitShare> column-major (j*n+i); Phase 5 combining works directly
  on this without conversion. D-06/D-07: LeakyTriple field names (gen_x_shares etc.)
- `.planning/ROADMAP.md` — Phase 5 goal and success criteria (PROTO-10, PROTO-11,
  PROTO-12, TEST-05); note that ROADMAP's "keep x = x'" is a shorthand error — paper
  mandates x = x' ⊕ x'' (confirmed by user)
- `.planning/REQUIREMENTS.md` — full requirements listing

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AuthBitShare + AuthBitShare` (XOR via `Add` impl, `src/sharing.rs`): the existing
  combining loop in `combine_leaky_triples` already uses `combined_gen_z[k] =
  combined_gen_z[k] + t.gen_z_shares[k]` — same pattern applies to x combining
- `AuthBitShare::verify(&delta)` (`src/sharing.rs`): used in Phase 4 tests for IT-MAC
  verification; reuse directly for d share verification
- `LeakyTensorPre::generate()` (`src/leaky_tensor_pre.rs`): produces LeakyTriple with
  correct IT-MAC structure; use in TEST-05 to generate real test triples

### Established Patterns
- Single shared `IdealBCot` for all triples — delta_a/delta_b same-delta assertion
  already in `combine_leaky_triples`; preserve it
- `#[should_panic]` tests for abort-path verification (Phase 3, Phase 4); use same
  pattern for the tampered-d test in TEST-05
- `make_triples(n, m, count)` helper already exists in `auth_tensor_pre.rs` tests;
  reuse or extend for TEST-05

### Integration Points
- `src/preprocessing.rs:87`: `let bucket_size = bucket_size_for(n, m);` → change to
  `bucket_size_for(count)`; with count=1 → B=40, generating 40 leaky triples
- `TensorFpreGen::alpha_auth_bit_shares` / `TensorFpreEval::alpha_auth_bit_shares` in
  `src/preprocessing.rs`: must receive the XOR-combined x shares (not just triple[0]'s
  x) after the Phase 5 fix
- Phase 6 (`Pi_aTensor'`): will add permutation bucketing on top of Phase 5's combining;
  `_shuffle_seed` parameter in `combine_leaky_triples` is already reserved for this

</code_context>

<specifics>
## Specific Ideas

- User confirmed x = x' ⊕ x'' per paper; ROADMAP "keep x = x'" was a typo
- User chose IT-MAC verification on assembled d shares (AuthBitShare::verify) as the
  in-process substitute for "publicly reveal with MACs"
- User chose B = SSP for ell ≤ 1 as the edge-case fix for the bucket size formula
- paper line 437: "Since d is public, itmac{x''}{Δ} ⊗ d is computed locally by scaling
  each authenticated row of itmac{x''}{Δ} by the corresponding public bit d_k, with no
  additional interaction" — implementation is a simple local loop, not a macro call

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 5 scope.

</deferred>

---

*Phase: 05-m2-pi-atensor-correct-combining-construction-3*
*Context gathered: 2026-04-22*
