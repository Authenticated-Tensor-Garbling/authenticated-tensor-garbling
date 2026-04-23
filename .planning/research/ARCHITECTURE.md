# Architecture Research — v1.1

**Researched:** 2026-04-23
**Source files read:** PROJECT.md, ARCHITECTURE.md, STRUCTURE.md, 5_online.tex, 6_total.tex, appendix_cpre.tex, auth_tensor_gen.rs, auth_tensor_eval.rs, auth_tensor_fpre.rs, preprocessing.rs
**Overall confidence:** HIGH — all analysis derived from actual source + paper

---

## New Components

### 1. `IdealPreprocessing` trait — `src/preprocessing.rs` (addition)

A trait that every preprocessing backend satisfies, enabling interchangeable production
of `(TensorFpreGen, TensorFpreEval)` pairs. `TensorFpreGen` and `TensorFpreEval` are
already the clean handoff types; the trait needs no new output types.

```rust
// Proposed minimal trait — add to src/preprocessing.rs
pub trait TensorPreprocessing {
    /// Generate `count` authenticated tensor triple pairs.
    /// Returns a Vec of (Gen, Eval) pairs.
    fn generate(
        &mut self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> Vec<(TensorFpreGen, TensorFpreEval)>;
}
```

Implementors:
- `IdealPreprocessingBackend` — wraps `TensorFpre::generate_for_ideal_trusted_dealer`,
  no COT calls. Lives in `auth_tensor_fpre.rs` or a new `ideal_preprocessing.rs`.
- `RealPreprocessingBackend` — wraps `run_preprocessing` (Construction 4, Pi_aTensor').
  Already implemented; just needs to implement the trait.
- `CompressedPreprocessingBackend` — wraps F_cpre logic if added in v1.1.
  See "Compressed preprocessing" section below.

**Design rationale:** The trait's `generate` signature matches the existing
`run_preprocessing(n, m, count, chunking_factor)` signature. No existing type needs to
change. `count` returns a Vec rather than a single pair; callers that only need one triple
index `[0]`. This also lifts the `assert_eq!(count, 1)` restriction in `run_preprocessing`
when a batch-returning variant is implemented.

---

### 2. `Open` function — `src/sharing.rs` or `src/online.rs` (new)

`Open` reveals an authenticated bit to one party and is used in two places in Protocol 1
(5_online.tex §"Encode inputs" and §"Decode outputs"):

```
open([l_w D_gb]) → l_w revealed to garbler
open([l_w D_ev]) → l_w revealed to evaluator
```

In the in-process setting (no network), Open is a local operation:

```rust
/// Reveal an IT-MAC authenticated bit to the verifier.
/// In-process version: both shares are available; returns the clear bit.
/// The verifier side checks: mac == key XOR bit*delta.
pub fn open_auth_bit(bit_share: &AuthBit, delta_a: &Delta, delta_b: &Delta) -> bool {
    bit_share.verify(delta_a, delta_b);  // panics if MAC invalid
    bit_share.full_bit()
}
```

`Open` is NOT a new struct or a step inside `AuthTensorGen`/`AuthTensorEval`. It is a
protocol-level step that operates on `AuthBit` (the full two-party view), not on
`AuthBitShare` (a single party's view). The right home is either:
- A free function in `src/sharing.rs` alongside the existing `AuthBit` and `AuthBitShare`
  types, or
- A new `src/online.rs` module if the consistency check also lives there.

**Where `Open` fits in Protocol 1 (5_online.tex):**
1. Before garbling: Open mask `l_alpha` and `l_beta` to the garbler so it knows the
   WFree labels. This maps to the existing `generate_for_ideal_trusted_dealer` step
   where the garbler gets `alpha_labels` (= WFree labels for x XOR alpha).
2. After evaluation: Open output masks `l_w` on output wires to the evaluator.

In v1.0, the garbler already receives `alpha_labels` and `beta_labels` as WFree labels;
the Open step for input masks is therefore already implicitly implemented inside
`TensorFpre::generate_for_ideal_trusted_dealer`. What is missing is the post-evaluation
Open for output wire masks.

---

### 3. Consistency check — `src/online.rs` (new module)

Protocol 1 consistency check (5_online.tex §"Consistency check"):

```
For each tensor gate output gamma:
  c_gamma = (L_alpha XOR l_alpha) tensor (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)
  Parties hold [c_gamma D_ev] shares; run CheckZero.
```

Protocol 2 consistency check (6_total.tex §"Consistency check") is simpler:
```
For each tensor gate output gamma:
  [L_gamma D_ev] = [v_gamma D_ev] XOR [l_gamma D_ev]
  ev sets [c_gamma]^ev = [L_gamma D_ev]^ev XOR L_gamma * D_ev
  gb sets [c_gamma]^gb = [L_gamma D_ev]^gb
  Run CheckZero on {[c_gamma]}.
```

`CheckZero` in the in-process setting: XOR both shares together and assert the result is
zero. For Protocol 1, this requires access to `D_ev` shares of the masks, which means
`TensorFpreGen` and `TensorFpreEval` must carry `D_ev`-authenticated shares in addition
to the current `D_gb`-authenticated shares.

**Where the consistency check runs:** After `evaluate_final()` on the evaluator side.
It is a post-evaluation protocol step, not part of the garbling or evaluation itself.
It logically belongs in a new `src/online.rs` alongside `Open`.

**Key insight for Protocol 1 vs Protocol 2:**
- Protocol 1: evaluator reveals `L_gamma` to garbler; garbler locally computes `c_gamma`
  share from its `D_ev`-shares; CheckZero requires both parties' `D_ev` shares of masks.
- Protocol 2: garbler reveals its `D_ev`-share of `v_gamma` to evaluator; evaluator
  computes `c_gamma` locally. No need to reveal `L_gamma` to garbler.

The in-process implementation can implement both checks as functions taking the full
`AuthBit` (both party views) and running both verification paths.

---

### 4. `gamma_auth_bit_shares` field in `TensorFpreGen`/`TensorFpreEval`

Currently `TensorFpreGen` stores `correlated_auth_bit_shares` (alpha_i AND beta_j under
D_gb) but does NOT store `D_ev`-authenticated shares of the same bits. Protocol 1 and
Protocol 2 consistency checks require `D_ev`-MAC'd shares of the output mask `l_gamma*`.

The `LeakyTriple` already carries `gamma_shares` (the `n*m` random noise shares). The
`combine_leaky_triples` function combines them. These need to be propagated through to
`TensorFpreGen`/`TensorFpreEval` as a new field:

```rust
// Addition to TensorFpreGen and TensorFpreEval:
pub gamma_auth_bit_shares: Vec<AuthBitShare>,  // n*m, col-major j*n+i, D_ev-authenticated
```

Currently `gamma_auth_bit_shares` is computed but then dropped after `combine_leaky_triples`
returns (it is not in the output struct). Adding this field is a backward-compatible struct
extension.

---

### 5. Compressed preprocessing — `src/compressed_pre.rs` (new, DEFERRED assessment)

From `appendix_cpre.tex`: The paper presents F_cpre as an ideal functionality and cites
CWYY23 for the concrete protocol Pi_cpre. The appendix contains only the functionality
specification and a commented-out protocol skeleton (all lines are commented out). The
concrete protocol requires F_COT, F_bCOT, F_DVZK, F_EQ, and F_Rand as sub-functionalities.

**Assessment:** Pi_cpre is NOT derivable from the paper appendix alone. The full protocol
is in CWYY23 (EC:CWYY23), which is a separate paper not vendored in `references/`. The
appendix notes that "a tensor triple is simply a combination of structured AND triples" and
uses `nm` AND triples to produce one tensor triple. The F_cpre ideal functionality can
be implemented as an ideal trusted-dealer variant (analogous to `TensorFpre` for F_pre),
but the real Pi_cpre protocol requires real OT and ZK machinery that is explicitly out of
scope until v2.

**Recommendation:** Implement `IdealCompressedPreprocessingBackend` — a trusted-dealer
that satisfies `TensorPreprocessing` but internally uses the CWYY23 compressed structure
(nm AND triples expanded via a public matrix M). Mark the real Pi_cpre as a v2 item.
This gives the trait a third concrete implementor without requiring real OT.

---

## Modified Components

### `src/preprocessing.rs` — Add trait + extend output structs

**What changes:**
1. Add `TensorPreprocessing` trait (see New Components §1).
2. Add `gamma_auth_bit_shares: Vec<AuthBitShare>` to `TensorFpreGen` and `TensorFpreEval`.
3. Extend `combine_leaky_triples` to propagate combined gamma shares into the output.
4. Lift `assert_eq!(count, 1)` restriction once a batch loop is added.
5. Implement `TensorPreprocessing` for `RealPreprocessingBackend` struct wrapping
   `run_preprocessing`.

**What does NOT change:** The struct field names for the existing alpha/beta/correlated
shares, the `TensorFpreGen`/`TensorFpreEval` split, or the `run_preprocessing` function
signature.

---

### `src/auth_tensor_fpre.rs` — Dual-delta material for Protocol 2

**What changes:**
1. `TensorFpre::generate_for_ideal_trusted_dealer` currently only produces D_gb-authenticated
   shares. Protocol 2 (`AuthTensor.Gb`/`AuthTensor.Ev`) requires `[b D_ev]` shares in
   addition to `[b D_gb]` shares.
2. New field: `correlated_auth_bit_shares_ev: Vec<AuthBitShare>` (D_ev-authenticated) to
   mirror `correlated_auth_bit_shares` (D_gb-authenticated).
3. `into_gen_eval()` must propagate the new field.

**What does NOT change:** The `TensorFpre::new`, `new_with_delta`, or `gen_auth_bit`
interfaces. The existing D_gb share production stays intact.

---

### `src/auth_tensor_gen.rs` — Protocol 2 support

**What changes (Protocol 2 only):**
1. `garble_first_half` and `garble_second_half` must produce wider ciphertexts when
   operating in Protocol 2 mode. In Protocol 2, each leaf seed is expanded to
   `(kappa + rho)` bits instead of `kappa` bits. The `gen_unary_outer_product` function
   and the PRG expansion step in `tensor_ops.rs` need to handle the wider output.
2. `garble_final` must split the combined `Z` output into `Z_gb` (D_gb share) and
   `Z'_gb` (D_ev share) after the XOR combination.
3. New output field: `second_delta_out: BlockMatrix` — the garbler's D_ev shares
   of the tensor gate output (Protocol 2 only).

**What does NOT change (Protocol 1):**
The existing `garble_first_half` / `garble_second_half` / `garble_final` sequence and
output in `first_half_out` are exactly Protocol 1 as specified in Construction 1
(tensor macros, 5_online.tex). The existing code implements Protocol 1 already — what
is missing is the consistency check using the D_ev shares.

**Protocol 1 status:** The existing garble/evaluate sequence is a correct implementation
of Construction 1 (tensor macros) and is already used in the preprocessing chain tests.
No changes to the core garbling logic are needed for Protocol 1; only the consistency
check and Open functions are missing.

**Protocol 2 strategy:** Rather than modifying the existing methods in place, add
`garble_first_half_p2` / `garble_second_half_p2` / `garble_final_p2` variants that
implement Construction 4 (authenticated tensor macros, 6_total.tex). This avoids
breaking Protocol 1 existing tests and allows benchmarking both protocols side by side.

---

### `src/auth_tensor_eval.rs` — Protocol 2 support

**What changes (Protocol 2 only):**
1. `evaluate_first_half_p2` / `evaluate_second_half_p2` variants accepting wider
   ciphertexts (`(kappa + rho)` bits per entry).
2. `evaluate_final_p2` splits its output into `first_half_out` (D_gb shares) and
   `second_delta_out` (D_ev shares).
3. Must carry D_ev-authenticated shares of `l_beta` and `v_alpha` in the evaluator's
   fpre material (requires `TensorFpreEval` extension above).

**What does NOT change:** `evaluate_first_half`, `evaluate_second_half`, `evaluate_final`
for Protocol 1 — these stay as-is, passing all existing tests.

---

### `src/tensor_ops.rs` — PRG expansion width

**What changes:** `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` expand
each leaf seed to exactly `kappa` bits. Protocol 2 requires `kappa + rho` bits per seed.
Two options:
- Option A: Add a `output_width: usize` parameter and generalize internally.
- Option B: Add a parallel `_wide` variant function.

Option B is preferred to avoid breaking existing callers. The narrow variant stays
unchanged; a new `gen_unary_outer_product_wide` handles Protocol 2.

**What does NOT change:** All existing function signatures.

---

### `benches/benchmarks.rs` — New benchmark groups

**What changes:**
1. Add a benchmark group for the full online phase (preprocessing + garble + evaluate),
   replacing the current fragmentary benchmarks.
2. Add a benchmark comparing Protocol 1 vs Protocol 2 communication/computation cost.
3. Clean up existing wall-clock benchmark code as specified in CLEAN-12.

---

## Data Flow Changes

### Current flow (v1.0, Protocol 1 skeleton)

```
run_preprocessing(n, m, 1, cf)
    -> (TensorFpreGen, TensorFpreEval)      [D_gb shares only]

AuthTensorGen::new_from_fpre_gen(TensorFpreGen)
    -> garble_first_half()  -> (chunk_levels_1, chunk_cts_1)
    -> garble_second_half() -> (chunk_levels_2, chunk_cts_2)
    -> garble_final()
    -> gen.first_half_out   [garbler's D_gb shares of x tensor y]

AuthTensorEval::new_from_fpre_eval(TensorFpreEval)
    -> evaluate_first_half(chunk_levels_1, chunk_cts_1)
    -> evaluate_second_half(chunk_levels_2, chunk_cts_2)
    -> evaluate_final()
    -> ev.first_half_out    [evaluator's D_gb shares of x tensor y]

MISSING: Open(), consistency check, output decoding
```

### v1.1 target flow (Protocol 1)

```
[Preprocessing]
IdealPreprocessingBackend::generate(n, m, 1, cf)  // or RealPreprocessingBackend
    -> (TensorFpreGen, TensorFpreEval)
       now includes: gamma_auth_bit_shares (D_ev shares of l_gamma*)

[Garble]
AuthTensorGen::new_from_fpre_gen(TensorFpreGen)
    -> garble_first_half()
    -> garble_second_half()
    -> garble_final()
    -> gen.first_half_out   [garbler's D_gb share of v_gamma]

[Open input masks — Protocol 1 step 3]
open_auth_bit(l_alpha_auth_bit, delta_a, delta_b) -> l_alpha (to garbler)
open_auth_bit(l_beta_auth_bit, delta_a, delta_b)  -> l_beta  (to evaluator)
// In-process: both shares available; real protocol would use network

[Evaluate]
AuthTensorEval::new_from_fpre_eval(TensorFpreEval)
    -> evaluate_first_half(chunk_levels_1, chunk_cts_1)
    -> evaluate_second_half(chunk_levels_2, chunk_cts_2)
    -> evaluate_final()
    -> ev.first_half_out    [evaluator's D_gb share of v_gamma]
    -> ev.L_gamma           [masked output: v_gamma XOR l_gamma]

[Consistency check — Protocol 1 step 6-8]
// ev sends L_gamma to gb (in-process: L_gamma already available)
// Both parties compute c_gamma shares from D_ev shares of masks
check_zero_protocol1(
    L_alpha, L_beta, L_gamma,
    l_alpha_ev_share, l_beta_ev_share, l_gamma_ev_share, l_gamma_star_ev_share,
    delta_ev
) -> Ok(()) or Err(ConsistencyFailed)

[Decode outputs — Protocol 1 step 9]
open_auth_bit(l_output_auth_bit, ...) -> l_output (to evaluator)
output = L_output XOR l_output
```

### v1.1 target flow (Protocol 2 — F_cpre variant)

```
[Preprocessing]
IdealCompressedPreprocessingBackend::generate(n, m, 1, cf)
    -> (TensorFpreGen, TensorFpreEval)
       includes: D_gb AND D_ev authenticated shares of alpha, beta, correlated, gamma

[Garble — Protocol 2, Construction 4]
AuthTensorGen::garble_first_half_p2()   // wider seeds: kappa+rho bits
    -> (chunk_levels_1, chunk_cts_1)     // cts are (kappa+rho) bits wide
AuthTensorGen::garble_second_half_p2()
    -> (chunk_levels_2, chunk_cts_2)
AuthTensorGen::garble_final_p2()
    -> gen.first_half_out   [D_gb share of v_gamma]
    -> gen.second_delta_out [D_ev share of v_gamma — new]

[Evaluate — Protocol 2, Construction 4]
AuthTensorEval::evaluate_first_half_p2(chunk_levels_1, chunk_cts_1)
AuthTensorEval::evaluate_second_half_p2(chunk_levels_2, chunk_cts_2)
AuthTensorEval::evaluate_final_p2()
    -> ev.first_half_out    [D_gb share of v_gamma]
    -> ev.second_delta_out  [D_ev share of v_gamma — new]
    -> ev.L_gamma           [masked output]

[Consistency check — Protocol 2 step 5-6]
// gb reveals D_ev share of v_gamma; ev checks locally; no L_gamma revealed to gb
check_zero_protocol2(
    L_gamma,
    gen_v_gamma_ev_share,   // garbler's D_ev share (revealed to ev)
    ev_v_gamma_ev_share,    // evaluator's D_ev share
    ev_l_gamma_ev_share,    // from fpre
    delta_ev
) -> Ok(()) or Err(ConsistencyFailed)
```

---

## Preprocessing Trait Design

### Rationale for minimal interface

The existing `run_preprocessing` and `TensorFpre::generate_for_ideal_trusted_dealer`
both produce `(TensorFpreGen, TensorFpreEval)`. The trait wraps this common output.
No new output type is needed. The trait takes `n`, `m`, `count`, `chunking_factor` as
parameters because all existing implementations require these four values.

### Proposed trait

```rust
// src/preprocessing.rs

/// Common interface for all preprocessing backends.
/// Produces authenticated tensor triple material for the online phase.
pub trait TensorPreprocessing {
    /// Generate `count` authenticated tensor triple pairs.
    /// Each pair is suitable for one AuthTensorGen/AuthTensorEval invocation.
    /// Implementations must ensure all returned pairs share the same delta_a and delta_b
    /// (required for the MAC invariant in combine_leaky_triples).
    fn generate(
        &mut self,
        n: usize,
        m: usize,
        count: usize,
        chunking_factor: usize,
    ) -> Vec<(TensorFpreGen, TensorFpreEval)>;
}

/// Ideal trusted-dealer backend (F_pre). No COT calls.
/// Uses TensorFpre internally. Input-independent: masks are uniform random.
pub struct IdealPreprocessingBackend {
    seed: u64,
}

impl TensorPreprocessing for IdealPreprocessingBackend {
    fn generate(&mut self, n: usize, m: usize, count: usize, chunking_factor: usize)
        -> Vec<(TensorFpreGen, TensorFpreEval)>
    {
        (0..count).map(|i| {
            let mut fpre = TensorFpre::new(self.seed + i as u64, n, m, chunking_factor);
            fpre.generate_for_ideal_trusted_dealer(0, 0); // masks are random; inputs ignored
            fpre.into_gen_eval()
        }).collect()
    }
}

/// Real two-party backend (Pi_aTensor', Construction 4). Uses COT (ideal F_bCOT).
pub struct RealPreprocessingBackend;

impl TensorPreprocessing for RealPreprocessingBackend {
    fn generate(&mut self, n: usize, m: usize, count: usize, chunking_factor: usize)
        -> Vec<(TensorFpreGen, TensorFpreEval)>
    {
        // run_preprocessing currently panics if count != 1;
        // extend to batch loop here, or call in a loop for count > 1
        (0..count).map(|_| run_preprocessing(n, m, 1, chunking_factor)).collect()
    }
}
```

### Compatibility with existing types

- `TensorFpreGen` and `TensorFpreEval` remain the output types — no change.
- `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` remain
  the consumption points — no change.
- `TensorFpre` stays as the ideal functionality object — `IdealPreprocessingBackend`
  wraps it; no modification to `TensorFpre` required to add the trait.

### Constraint: same-delta requirement

All triples produced in one batch must share `delta_a` and `delta_b`. For
`IdealPreprocessingBackend`, each call to `TensorFpre::new` produces independent deltas;
for count > 1 this is fine because each triple pair is independent. For
`RealPreprocessingBackend`, `run_preprocessing` creates its own `IdealBCot` per call,
so each returned pair has its own delta pair — this is correct behavior because each
output triple is used independently by a separate gate.

---

## Build Order

The following order respects all dependencies. Earlier phases must be complete before
later phases begin.

### Phase A — Preprocessing trait + IdealPreprocessing (foundation)

**Dependencies:** None new — builds on existing `TensorFpre` and `run_preprocessing`.
**Deliverables:**
- `TensorPreprocessing` trait in `src/preprocessing.rs`
- `IdealPreprocessingBackend` struct + impl (wraps `TensorFpre`)
- `RealPreprocessingBackend` struct + impl (wraps `run_preprocessing`)
- Extend `TensorFpreGen`/`TensorFpreEval` with `gamma_auth_bit_shares`
- Propagate gamma shares through `combine_leaky_triples` into output structs
- Tests: trait is satisfied by both backends; dimensions correct

**Why first:** Everything else depends on the trait being defined and on
`TensorFpreGen`/`TensorFpreEval` being stable. Extending the structs here prevents
needing to touch them again in later phases.

---

### Phase B — Open() and Protocol 1 consistency check

**Dependencies:** Phase A (needs stable `TensorFpreGen`/`TensorFpreEval` with gamma shares,
needs `IdealPreprocessingBackend` to produce D_ev shares of masks).
**Deliverables:**
- `open_auth_bit` free function in `src/sharing.rs`
- `check_zero_protocol1` function (new `src/online.rs` or at bottom of `src/sharing.rs`)
- End-to-end Protocol 1 integration test:
  preprocess → garble → evaluate → consistency check passes
- Edge case test: perturb a ciphertext, check consistency check catches it

**Why after Phase A:** Consistency check needs the gamma (D_ev) shares that Phase A adds
to `TensorFpreGen`/`TensorFpreEval`.

---

### Phase C — Protocol 2 authenticated tensor macros

**Dependencies:** Phase A (stable output structs), Phase B (Open is defined).
**Deliverables:**
- Extend `TensorFpre::generate_for_ideal_trusted_dealer` to also produce D_ev-authenticated
  shares of `l_beta` and the correlated bits (required for AuthTensor.Gb/Ev input).
- `garble_first_half_p2` / `garble_second_half_p2` / `garble_final_p2` in `auth_tensor_gen.rs`
- `evaluate_first_half_p2` / `evaluate_second_half_p2` / `evaluate_final_p2` in `auth_tensor_eval.rs`
- New `second_delta_out: BlockMatrix` field on both gen and eval structs
- `gen_unary_outer_product_wide` in `tensor_ops.rs` (kappa+rho output width)
- `check_zero_protocol2` function in `src/online.rs`
- End-to-end Protocol 2 integration test

**Why after Phase B:** Protocol 2's consistency check is a simplification of Protocol 1's;
understanding Protocol 1 check first makes Protocol 2 easier to implement correctly.
Also shares `open_auth_bit` from Phase B.

---

### Phase D — Benchmarks

**Dependencies:** Phase A (both backends available), Phase B (P1 complete),
Phase C (P2 complete).
**Deliverables:**
- Clean benchmark groups: preprocessing-only, garble-only, evaluate-only, full round-trip
- Protocol 1 vs Protocol 2 communication cost comparison (byte counts)
- Wall-clock benchmark for tensor product at standard (n, m) pairs
- Remove duplicate benchmark code identified in CLEAN-12

**Why last:** Benchmarks need complete implementations to measure. Running benchmarks
before Phase C would measure an incomplete Protocol 2.

---

### Phase E — Compressed preprocessing ideal backend (optional, if feasible in v1.1)

**Dependencies:** Phase A (TensorPreprocessing trait), Phase D (baseline benchmarks
to compare against).
**Deliverables:**
- `IdealCompressedPreprocessingBackend` that implements `TensorPreprocessing`
- Internally: nm AND triples → one tensor triple via public matrix M expansion
- Test: output satisfies same invariants as `IdealPreprocessingBackend`
- Benchmark: compare vs uncompressed ideal preprocessing

**Why optional / last:** The real Pi_cpre protocol is explicitly out of scope (requires
real OT, F_DVZK, F_EQ, F_Rand). The ideal version is derivable from the functionality
spec in `appendix_cpre.tex` and adds a useful third implementor for the trait, but is
not required for Protocol 1 or Protocol 2 demonstrations.

---

## Integration Risks

### Risk 1 — `TensorFpreGen`/`TensorFpreEval` struct extension

**Conflict:** Adding `gamma_auth_bit_shares` is backward-compatible as a struct
addition, but `into_gen_eval()` in `auth_tensor_fpre.rs` constructs both structs
directly. It will fail to compile until the new field is populated.

**Affected constructors:** `TensorFpre::into_gen_eval`, `combine_leaky_triples` return.
Both must be updated in the same Phase A commit.

**Mitigation:** Make Phase A a single atomic change: add the field, update all
constructors, update all tests in one PR. Do not add the field and leave constructors
broken between commits.

---

### Risk 2 — `TensorFpre` only produces D_gb shares

**Conflict:** Protocol 2 requires `[b D_ev]` shares for the `l_beta` and `v_alpha`
inputs to `AuthTensor.Gb`. `TensorFpre::generate_for_ideal_trusted_dealer` currently
generates D_gb-authenticated shares only. Adding D_ev shares requires extending
`TensorFpre` fields and `into_gen_eval` in Phase C.

**Mitigation:** Do not modify `TensorFpre` in Phase A or Phase B. Lock the interface in
Phase A, then extend in Phase C with Protocol 2 fields. Protocol 1 uses only D_gb shares
so Phase B tests will remain valid throughout.

---

### Risk 3 — `gen_unary_outer_product` output width assumption

**Conflict:** The existing PRG expansion in `gen_unary_outer_product` emits `kappa`-bit
values. Protocol 2 needs `(kappa + rho)`-bit values from the same leaf seeds. Widening
the existing function would break Protocol 1 callers.

**Mitigation:** Add `_wide` variants (Phase C). Do not modify the existing functions.
Use a feature flag or separate call path in `AuthTensorGen`. The `Block` type is 128 bits
(`kappa`); for `kappa + rho = 128 + 40 = 168` bits, the wide variant will need to return
a pair `(Block, [u8; 5])` or a `[u8; 21]`. Design this interface in Phase C.

---

### Risk 4 — `assert_eq!(count, 1)` in `run_preprocessing`

**Conflict:** The trait's `generate` taking `count` promises batch support, but
`run_preprocessing` panics for count > 1.

**Mitigation:** In Phase A's `RealPreprocessingBackend::generate`, call
`run_preprocessing` in a loop (once per output triple), each with count=1. This is
correct: each call creates its own `IdealBCot` with independent deltas, which is
valid for independent gates. The batch-within-one-bcot optimization is deferred.
Remove the assert only when a genuine batch implementation is added.

---

### Risk 5 — Consistency check requires L_gamma on the evaluator side

**Conflict:** Protocol 1's consistency check requires the evaluator to send `L_gamma`
to the garbler. In the current code, `first_half_out` holds the D_gb share of
`v_gamma`, not `L_gamma`. Computing `L_gamma = v_gamma XOR l_gamma` requires access
to the mask `l_gamma`, which is the output wire mask — distinct from `l_gamma*`
(the correlated triple output mask).

**Root cause:** The current preprocessing does not produce a D_gb-share of `l_gamma`
(the output wire mask), only `l_gamma*` (the product mask for the triple). Protocol 1
requires both. This is an additional field that needs to be added to `TensorFpreGen`
and `TensorFpreEval` beyond `gamma_auth_bit_shares`.

**Mitigation:** In Phase B, when adding the consistency check, also add
`output_mask_auth_bit_shares: Vec<AuthBitShare>` to both structs. This is a one-gate
protocol, so the output mask is the single tensor gate's output mask (n*m bits).

---

*Architecture analysis: 2026-04-23*
