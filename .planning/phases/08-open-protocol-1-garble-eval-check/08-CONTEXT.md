# Phase 8: Open() + Protocol 1 Garble/Eval/Check - Context

**Gathered:** 2026-04-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement Protocol 1 complete garble, evaluate, and CheckZero — including `compute_lambda_gamma()` on both protocol structs, a `check_zero()` function in a new `src/online.rs`, and tests for both honest-party correctness and tampered-mask failure.

`open()` (ONL-01, ONL-02) is **deferred** — not implemented in this phase.

**Active requirements:** P1-01, P1-02, P1-03, P1-04, P1-05
**Deferred requirements:** ONL-01, ONL-02

</domain>

<decisions>
## Implementation Decisions

### open() — Deferred

- **D-01:** `open()` and its wrong-delta negative test (ONL-01, ONL-02) are **out of scope for Phase 8**. They will be implemented in a later phase.

### Module Layout

- **D-02:** `src/online.rs` is created in this phase. It hosts `check_zero()` only. `open()` is added here in a later phase.
- **D-03:** Protocol 1 garble/eval logic (`compute_lambda_gamma()`) stays in `src/auth_tensor_gen.rs` and `src/auth_tensor_eval.rs` as new methods on the existing structs. `online.rs` stays thin.

### L_gamma Computation

- **D-04:** `AuthTensorGen::compute_lambda_gamma() -> Vec<bool>` — new method. Computes the garbler's masked output share per (i,j):
  ```
  [L_gamma]^gb[j*n+i] = first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()
  ```
  `first_half_out` holds `[v_gamma D_gb]^gb` after `garble_final()`; `gamma_auth_bit_shares` holds `[l_gamma D_gb]^gb` (Phase 7, D-04/D-05).
- **D-05:** `AuthTensorEval::compute_lambda_gamma(lambda_gb: &[bool]) -> Vec<bool>` — new method, takes the garbler's `[L_gamma]^gb` vec as input. Computes the evaluator's masked output per (i,j):
  ```
  L_gamma[j*n+i] = lambda_gb[j*n+i] XOR first_half_out[(i,j)].lsb() XOR gamma_auth_bit_shares[j*n+i].bit()
  ```
- **D-06:** The `TODO(Phase 8)` comments in `src/auth_tensor_gen.rs:64` and `src/auth_tensor_eval.rs:57` must be resolved — `gamma_auth_bit_shares` from `TensorFpreGen`/`TensorFpreEval` is forwarded to a new `gamma_auth_bit_shares: Vec<AuthBitShare>` field on `AuthTensorGen`/`AuthTensorEval` respectively.

### CheckZero

- **D-07:** `check_zero()` signature: `fn check_zero(c_gamma_shares: &[AuthBitShare], delta_ev: &Delta) -> bool`. Returns `true` (pass) or `false` (abort).
- **D-08:** Callers pre-compute `c_gamma` from the D_ev-authenticated shares of `l_alpha`, `l_beta`, `l_gamma`, `l_gamma*` and pass the combined share vec. `check_zero()` is a thin primitive — it does not know about the struct types.
- **D-09:** `c_gamma` formula per gate (from `5_online.tex`):
  ```
  c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)
           = v_alpha ⊗ v_beta XOR v_gamma   [= 0 for honest parties]
  ```
  This is a linear combination of `[l_alpha D_ev]`, `[l_beta D_ev]`, `[l_gamma D_ev]`, `[l_gamma* D_ev]` with coefficients determined by the `L` values. Computed locally from preprocessing shares.

### Claude's Discretion

- Exact `check_zero()` MAC verification mechanic (e.g., sum-of-MACs XOR expected value vs per-share checks) — use whatever is consistent with `AuthBitShare::verify()` patterns.
- Whether `gamma_auth_bit_shares` new field on `AuthTensorGen`/`AuthTensorEval` uses `Vec<AuthBitShare>` (matching TensorFpreGen/Eval) or is stored differently — match the existing field type.
- Indexing order for `compute_lambda_gamma()` output — use column-major `j * n + i` consistent with all other n×m field vecs.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification

- `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Construction 3 (Garbling and Evaluation Algorithms for Protocol 1), full Protocol 1 description including the consistency check formula for `c_gamma`. Primary spec for this phase.

### Key Source Files

- `src/auth_tensor_gen.rs` — `AuthTensorGen`. Existing `garble_first_half`, `garble_second_half`, `garble_final` methods. Line 64 has `TODO(Phase 8)` for forwarding `gamma_auth_bit_shares`.
- `src/auth_tensor_eval.rs` — `AuthTensorEval`. Existing evaluate methods. Line 57 has `TODO(Phase 8)` for forwarding `gamma_auth_bit_shares`.
- `src/sharing.rs` — `AuthBit`, `AuthBitShare`, `build_share`. `check_zero()` operates on `AuthBitShare` vecs.
- `src/preprocessing.rs` — `TensorFpreGen`, `TensorFpreEval` with the new `gamma_auth_bit_shares` field added in Phase 7.

### Prior Phase Context

- `.planning/phases/07-preprocessing-trait-ideal-backends/07-CONTEXT.md` — D-04/D-05: `gamma_auth_bit_shares` is `Vec<AuthBitShare>`, length `n*m`, column-major, holds D_ev-authenticated shares of **l_gamma** (not l_gamma*).

### Requirements

- `.planning/REQUIREMENTS.md` — P1-01..P1-05 active. ONL-01/ONL-02 deferred.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `AuthBitShare` (`src/sharing.rs`): `.bit()` returns the local share value; `.verify(delta)` checks `mac == key XOR bit * delta`. Used throughout; `gamma_auth_bit_shares` vecs use this type.
- `AuthTensorGen::garble_final()` / `AuthTensorEval::evaluate_final()`: mutate `first_half_out` in-place; return nothing. `compute_lambda_gamma()` is called after these.
- `Delta` (`src/delta.rs`): LSB = 1 invariant holds. `block.lsb()` extracts the bit from a D_gb-authenticated label (`extbit` in paper notation).

### Established Patterns

- Column-major indexing: `j * n + i` for (i,j) in an n×m field.
- Gen/Eval pair symmetry: same field name on both structs (see `correlated_auth_bit_shares`, `gamma_auth_bit_shares`).
- `bool` return for pass/fail: consistent with `AuthBitShare::verify()` (panics on failure there — `check_zero()` uses `bool` instead per D-07).
- All new struct fields initialized at construction (Phase 7 D-06 pattern).

### Integration Points

- `auth_tensor_gen.rs:64` and `auth_tensor_eval.rs:57` `TODO(Phase 8)` comments: add `gamma_auth_bit_shares` field to both structs and populate in `new_from_fpre_gen` / `new_from_fpre_eval`.
- `src/online.rs` (new file): imports `AuthBitShare` from `sharing`, `Delta` from `delta`. Exports `check_zero()`. Added to `lib.rs` module list.
- c_gamma computation: test harness (or a helper) combines the D_ev-share fields from `AuthTensorGen` and `AuthTensorEval` — `alpha_auth_bit_shares`, `beta_auth_bit_shares`, `correlated_auth_bit_shares` (l_gamma*), and the new `gamma_auth_bit_shares` (l_gamma) — then passes the combined vec to `check_zero()`.

</code_context>

<specifics>
## Specific Ideas

- `extbit` in paper notation = `block.lsb()` for a D_gb-authenticated block (exploits Delta LSB = 1 invariant).
- `[L_gamma]^gb` is a `Vec<bool>` of length `n*m`; garbler sends this to evaluator as part of the garbled circuit (gc). In the single-process simulation, it is passed directly as a `&[bool]` argument.
- P1-05 negative test: flip one entry of `L_gamma` before calling the consistency check → `c_gamma ≠ 0` for that gate → `check_zero()` returns `false`.
- `check_zero()` semantics: in simulation, both parties' D_ev-shares of `c_gamma` are available together; the function verifies the combined IT-MAC (the bit reconstruction of the shares equals zero and the MAC checks out under `delta_ev`).

</specifics>

<deferred>
## Deferred Ideas

- `open()` free function (ONL-01) — future phase. Will live in `src/online.rs` once designed.
- `open()` wrong-delta negative test (ONL-02) — same future phase.

</deferred>

---

*Phase: 08-open-protocol-1-garble-eval-check*
*Context gathered: 2026-04-23*
