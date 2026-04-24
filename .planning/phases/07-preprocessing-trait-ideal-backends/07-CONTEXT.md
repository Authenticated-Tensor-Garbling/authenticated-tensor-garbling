# Phase 7: Preprocessing Trait + Ideal Backends - Context

**Gathered:** 2026-04-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the `TensorPreprocessing` trait so all preprocessing backends are interchangeable; wrap the existing `run_preprocessing` in `UncompressedPreprocessingBackend`; add `IdealPreprocessingBackend` as a trusted-dealer oracle; extend `TensorFpreGen` / `TensorFpreEval` with the `gamma_auth_bit_shares` field required for the consistency check.

**Active requirements:** PRE-01, PRE-02, PRE-03, PRE-04. PRE-05 (`IdealCompressedPreprocessingBackend`) is **deferred to v3**.

</domain>

<decisions>
## Implementation Decisions

### Trait Location

- **D-01:** `TensorPreprocessing` trait lives in `src/preprocessing.rs`, alongside the existing `TensorFpreGen`, `TensorFpreEval`, and `run_preprocessing`. No new file; the module already owns the preprocessing boundary.

### Backend Naming and Wrappers

- **D-02:** The existing `run_preprocessing` function is wrapped in a zero-field struct named `UncompressedPreprocessingBackend` that implements `TensorPreprocessing`. Callers invoke it by type, not by calling `run_preprocessing` directly.
- **D-03:** `IdealPreprocessingBackend` is also a zero-field struct (unit struct). Fixed seed `0` is used internally — matches `IdealBCot` pattern; no caller configuration needed.

### PRE-04: gamma_auth_bit_shares Field

- **D-04:** `TensorFpreGen` and `TensorFpreEval` both get **one new field**: `gamma_auth_bit_shares: Vec<AuthBitShare>`. Length `n*m` per triple, column-major (same indexing as `correlated_auth_bit_shares`). Symmetric layout — same field name on both structs.
- **D-05:** `gamma_auth_bit_shares` holds D_ev-authenticated shares of **l_gamma** (the gate output mask), **not** l_gamma*. `correlated_auth_bit_shares` already encodes l_gamma*. These are two distinct values. REQUIREMENTS.md PRE-04 text contains an error: it says "l_gamma\*" where it should say "l_gamma".
- **D-06:** Every existing constructor of `TensorFpreGen` / `TensorFpreEval` must initialize `gamma_auth_bit_shares` in the same commit that adds the field — no intermediate broken state.

### IdealPreprocessingBackend Internals

- **D-07:** `IdealPreprocessingBackend::run()` delegates to `TensorFpre` internally: creates a `TensorFpre`, calls `generate_for_ideal_trusted_dealer()`, then `into_gen_eval()` to produce the base struct pair. It then generates the `gamma_auth_bit_shares` field on top.
- **D-08:** `gamma_auth_bit_shares` in the ideal backend is a **separate random authenticated bit** per (i,j) pair — `l_gamma` is an independent random wire mask, distinct from `correlated_auth_bit_shares` (l_gamma*). The ideal dealer calls `TensorFpre::gen_auth_bit()` once per (i,j) with a freshly sampled random bit.
- **D-09:** IT-MAC invariant (`mac = key XOR bit * delta`) must hold for `gamma_auth_bit_shares` entries on both structs. This is satisfied automatically by delegating to `gen_auth_bit()`.

### PRE-05 — Deferred

- **D-10:** `IdealCompressedPreprocessingBackend` and PRE-05 are **deferred to v3**. Phase 7 delivers PRE-01, PRE-02, PRE-03, PRE-04 only. Do not add any compressed-preprocessing scaffolding.

### Claude's Discretion

- Trait method signature: `fn run(n: usize, m: usize, count: usize, chunking_factor: usize) -> (TensorFpreGen, TensorFpreEval)` — use whatever Rust trait form (associated function vs `&self`) is most natural given the zero-field struct design. Can use `&self` for object-safety even though the struct holds no state.
- Count > 1 handling in `UncompressedPreprocessingBackend`: may retain the existing `assert_eq!(count, 1)` panic until a batch variant is implemented. Matching existing behavior is fine.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification

- `references/appendix_krrw_pre.tex` — Appendix F: Pi_aTensor, Pi_aTensor', Pi_LeakyTensor. Primary spec for the preprocessing pipeline being abstracted behind the trait.
- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Protocol 1 and Protocol 2 online phase. Defines how `TensorFpreGen` / `TensorFpreEval` fields are consumed — informs which fields must be present after PRE-04.

### Key Source Files

- `src/preprocessing.rs` — Where the trait and all backend structs go. Contains `TensorFpreGen`, `TensorFpreEval`, `run_preprocessing`.
- `src/auth_tensor_fpre.rs` — `TensorFpre` (ideal trusted dealer). `IdealPreprocessingBackend` delegates to this. Also contains `TensorFpre::gen_auth_bit()` used for `gamma_auth_bit_shares` generation.
- `src/bcot.rs` — `IdealBCot` pattern (zero-field struct with fixed seed) — structural precedent for `IdealPreprocessingBackend`.

### Requirements

- `.planning/REQUIREMENTS.md` — PRE-01..PRE-04 are active for this phase. Note: PRE-04 text contains "l_gamma\*" error (see D-05). PRE-05 is deferred.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `TensorFpre` (`src/auth_tensor_fpre.rs`): Trusted-dealer implementation. `IdealPreprocessingBackend::run()` creates one internally, calls `generate_for_ideal_trusted_dealer()` and `into_gen_eval()`, then appends `gamma_auth_bit_shares`.
- `TensorFpre::gen_auth_bit(bit: bool) -> AuthBit`: Generates a correctly structured IT-MAC authenticated bit. Use this to generate the `gamma_auth_bit_shares` entries.
- `IdealBCot` (`src/bcot.rs`): Precedent for zero-field struct backend pattern with fixed seed.
- `combine_leaky_triples` / `run_preprocessing` (`src/auth_tensor_pre.rs`, `src/preprocessing.rs`): The function being wrapped by `UncompressedPreprocessingBackend`.

### Established Patterns

- MAC invariant: `mac = key XOR bit * delta`. All `AuthBitShare` values in `gamma_auth_bit_shares` must satisfy this.
- Column-major indexing: `j * n + i` for (i, j) pairs — same as `correlated_auth_bit_shares`.
- Gen/Eval pair symmetry: `TensorFpreGen` holds garbler's shares (MAC under D_ev), `TensorFpreEval` holds evaluator's shares (MAC under D_gb). Same field name on both structs (D-04).

### Integration Points

- `AuthTensorGen::new_from_fpre_gen()` and `AuthTensorEval::new_from_fpre_eval()` consume `TensorFpreGen` / `TensorFpreEval` — these constructors must compile after PRE-04 field addition. They will need to initialize `gamma_auth_bit_shares` (or be updated to accept structs with the new field).
- `TensorFpre::into_gen_eval()` produces `TensorFpreGen` / `TensorFpreEval` — must be updated to initialize `gamma_auth_bit_shares` (plausibly `vec![]` for backward-compat callers that don't use the new field, or filled with a zero-length vec for the non-ideal path).

</code_context>

<specifics>
## Specific Ideas

- The design is intentional: `correlated_auth_bit_shares` = l_gamma* (the preprocessing triple output mask) and `gamma_auth_bit_shares` = l_gamma (the gate output mask). They are distinct. The consistency check in Phase 8 will use both.
- `UncompressedPreprocessingBackend` name was chosen over `PiATensorBackend` / `StandardPreprocessingBackend` — explicitly signals "uncompressed" as a property, meaningful once the compressed backend exists in v3.

</specifics>

<deferred>
## Deferred Ideas

- **PRE-05 / IdealCompressedPreprocessingBackend** — deferred to v3. Real Pi_cpre protocol is already out of scope (incomplete in paper); the ideal compressed oracle is now also deferred. The design (M·b* compressed mask derivation, sigma = O(SSP·log(κ))) was discussed and the decision was to not implement it in v1.1 at all.
  - Relevant paper: `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_cpre.tex`

</deferred>

---

*Phase: 07-preprocessing-trait-ideal-backends*
*Context gathered: 2026-04-23*
