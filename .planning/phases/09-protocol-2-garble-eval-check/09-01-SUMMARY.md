---
phase: 09-protocol-2-garble-eval-check
plan: 01
subsystem: preprocessing
tags: [rust, preprocessing, rename, ideal-backend, atomic-commit, d_ev]
requires:
  - "Phase 8 complete (gamma_auth_bit_shares plumbed end-to-end)"
provides:
  - "TensorFpreGen.alpha_d_ev_shares (D_ev IT-MAC under delta_b, length n)"
  - "TensorFpreGen.beta_d_ev_shares (D_ev IT-MAC under delta_b, length m)"
  - "TensorFpreGen.correlated_d_ev_shares (D_ev IT-MAC under delta_b, length n*m)"
  - "TensorFpreGen.gamma_d_ev_shares (renamed from gamma_auth_bit_shares)"
  - "TensorFpreEval mirrors the four D_ev fields under delta_a"
  - "AuthTensorGen carries the four D_ev fields"
  - "AuthTensorEval carries the four D_ev fields"
  - "IdealPreprocessingBackend::run populates all four D_ev field pairs"
affects:
  - src/preprocessing.rs
  - src/auth_tensor_gen.rs
  - src/auth_tensor_eval.rs
  - src/auth_tensor_pre.rs
  - src/auth_tensor_fpre.rs
  - src/lib.rs
tech-stack:
  added: []
  patterns:
    - "Four-field D_ev preprocessing pattern (alpha/beta/correlated/gamma)"
    - "Distinct-seed independent randomness (ChaCha12Rng seeds 42/43/44/45)"
    - "Stub initialization for uncompressed path (vec![] for all four D_ev fields)"
key-files:
  created: []
  modified:
    - src/preprocessing.rs
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs
    - src/auth_tensor_pre.rs
    - src/auth_tensor_fpre.rs
    - src/lib.rs
decisions:
  - "Use distinct ChaCha12Rng seeds 42/43/44/45 for independent l_gamma/l_alpha/l_beta/l_gamma* randomness in IdealPreprocessingBackend::run"
  - "Place new D_ev fields after the existing _auth_bit_shares fields (D_gb) in struct field order — preserves existing constructor order, minimizes diff"
  - "Stub all four D_ev fields as vec![] in the uncompressed preprocessing path — matches the existing gamma_auth_bit_shares -> gamma_d_ev_shares stub pattern; real population deferred to a future plan"
  - "Field reordering: in AuthTensorGen/Eval, place all four D_ev fields together after the existing four _auth_bit_shares fields rather than interleaving — keeps logical groups distinct in source"
metrics:
  duration: ~6 minutes
  completed_date: 2026-04-25
  task_count: 4
  file_count: 6
  test_count_before: 95
  test_count_after: 97
---

# Phase 9 Plan 01: Preprocessing D_ev Field Plumbing Summary

Land the rename `gamma_auth_bit_shares -> gamma_d_ev_shares` and add three new D_ev preprocessing fields (`alpha_d_ev_shares`, `beta_d_ev_shares`, `correlated_d_ev_shares`) end-to-end through `TensorFpreGen`/`TensorFpreEval`/`AuthTensorGen`/`AuthTensorEval`, with `IdealPreprocessingBackend::run` populating all four pairs via `fpre.gen_auth_bit()`. Uncompressed stub paths leave all four fields as `vec![]`. All 95 prior tests remain green; 2 new tests added for `test_ideal_backend_d_ev_shares_lengths` and `test_ideal_backend_d_ev_shares_mac_invariant` — final test count 97 passing, 0 failed.

## Field Declarations Added

### `TensorFpreGen` (src/preprocessing.rs)
```rust
pub alpha_d_ev_shares: Vec<AuthBitShare>,        // length n, MAC under delta_b
pub beta_d_ev_shares: Vec<AuthBitShare>,         // length m, MAC under delta_b
pub correlated_d_ev_shares: Vec<AuthBitShare>,   // length n*m, MAC under delta_b
pub gamma_d_ev_shares: Vec<AuthBitShare>,        // length n*m, MAC under delta_b (renamed)
```

### `TensorFpreEval` (src/preprocessing.rs)
Mirror of the gen-side block; MACs under `delta_a` instead of `delta_b`.

### `AuthTensorGen` (src/auth_tensor_gen.rs)
Same four-field block. Initialized to `Vec::new()` in `new()` and moved by name from `fpre_gen` in `new_from_fpre_gen()`.

### `AuthTensorEval` (src/auth_tensor_eval.rs)
Mirror of the gen-side block.

## ChaCha12Rng Seeds (IdealPreprocessingBackend::run)

| Seed | Field driven                                              | Length |
| ---- | --------------------------------------------------------- | ------ |
| 42   | `gamma_d_ev_shares` (l_gamma, gate output mask)           | n*m    |
| 43   | `alpha_d_ev_shares` (l_alpha)                             | n      |
| 44   | `beta_d_ev_shares` (l_beta)                               | m      |
| 45   | `correlated_d_ev_shares` (l_gamma* = l_alpha · l_beta)    | n*m    |

Distinct seeds ensure the four random samples are independent. Seed 42 retained for the gamma path so that prior gamma test fixtures remain bit-identical post-rename (sanity continuity).

Critical ordering preserved: ALL `fpre.gen_auth_bit()` calls happen BEFORE `fpre.into_gen_eval()` because `into_gen_eval(self)` consumes `fpre` by value.

## Rename Verification

`grep -rn "gamma_auth_bit_shares" src/` returns **0 matches**. The rename is complete across all six modified source files.

## Test Results

```
test result: ok. 97 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

New tests confirmed passing:
- `preprocessing::tests::test_ideal_backend_d_ev_shares_lengths ... ok`
- `preprocessing::tests::test_ideal_backend_d_ev_shares_mac_invariant ... ok`

P1 regression tests confirmed passing:
- `tests::test_auth_tensor_product ... ok`
- `tests::test_auth_tensor_product_full_protocol_1 ... ok`
- `tests::test_protocol_1_check_zero_aborts_on_tampered_lambda ... ok`

## Commits

| Task | Description                                              | Commit  |
| ---- | -------------------------------------------------------- | ------- |
| 1    | preprocessing.rs: rename + 3 new fields + tests          | 78535dc |
| 2    | AuthTensorGen/Eval: 4 D_ev fields + constructor updates  | 68b5fd5 |
| 3    | Stub callers (auth_tensor_pre, auth_tensor_fpre, lib.rs) | f2aa9fe |
| 4    | Verification only — no code change                       | (none)  |

## Deviations from Plan

### Test Naming Adaptations

The plan referenced two test names that did not match the actual file state:

1. **Plan said:** `test_uncompressed_backend_gamma_auth_bit_shares_empty` (rename to `..._gamma_d_ev_shares_empty`).
   **Actual:** Existing test was named `test_uncompressed_backend_gamma_field_is_empty`. **Action:** kept the existing test function name (the field rename is the substantive change); replaced `gamma_auth_bit_shares` with `gamma_d_ev_shares` in the body and assertion messages.

2. **Plan said:** `test_l_gamma_independent_of_l_gamma_star` — rename to use `gamma_d_ev_shares`.
   **Actual:** Existing test was named `test_ideal_backend_gamma_distinct_from_correlated`. **Action:** function name unchanged (matches plan instruction "function name unchanged" semantics); replaced field references in body.

These are surface-level test name discrepancies in the plan; the substantive intent (rename the field references in the bodies, keep the tests passing) was honored in full. Recorded as `[Rule 3 - Documentation Drift]` for traceability.

### Rename References in Comments and Doc-Strings

Per Task 1's Step D acceptance criterion `grep -n "gamma_auth_bit_shares" src/preprocessing.rs | wc -l` returns 0, doc-comment textual references like `RENAMED from gamma_auth_bit_shares` were also updated to `(Phase 9 D-05.)` to satisfy zero-occurrence. The plan implied "rename the symbol everywhere"; comment-only references would technically pass compilation but fail the strict grep check, so these were edited to keep the criterion clean.

## Threat Surface Scan

No new security-relevant surface introduced beyond what is already in the plan's `<threat_model>`. The rename + addition operates entirely within the existing trust boundary (Garbler ↔ Evaluator, honest-but-curious). All four new D_ev fields satisfy the IT-MAC invariant under `delta_b` — verified by `test_ideal_backend_d_ev_shares_mac_invariant`.

## Self-Check: PASSED

- src/preprocessing.rs: FOUND
- src/auth_tensor_gen.rs: FOUND
- src/auth_tensor_eval.rs: FOUND
- src/auth_tensor_pre.rs: FOUND
- src/auth_tensor_fpre.rs: FOUND
- src/lib.rs: FOUND
- Commit 78535dc: FOUND
- Commit 68b5fd5: FOUND
- Commit f2aa9fe: FOUND
- `grep -rn "gamma_auth_bit_shares" src/`: 0 matches (PASS)
- `cargo build`: clean (PASS)
- `cargo test`: 97 passed; 0 failed (PASS)
