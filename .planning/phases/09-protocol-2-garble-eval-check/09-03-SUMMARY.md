---
phase: 09-protocol-2-garble-eval-check
plan: 03
subsystem: protocol
tags: [rust, protocol-2, garble, evaluate, dual-delta, ggm-tree, wide-leaf, it-mac]

# Dependency graph
requires:
  - phase: 09-01
    provides: "Four D_ev share fields on AuthTensorGen / AuthTensorEval (alpha, beta, correlated, gamma) — required to source y inputs and the correlated combine for the rho-half"
  - phase: 09-02
    provides: "gen_unary_outer_product_wide / eval_unary_outer_product_wide — wide-leaf GGM expansions producing/consuming Vec<(Block, Block)> ciphertexts"

provides:
  - "AuthTensorGen::garble_first_half_p2 / garble_second_half_p2 / garble_final_p2 — Protocol-2 garble path with dual D_gb/D_ev accumulation"
  - "AuthTensorEval::evaluate_first_half_p2 / evaluate_second_half_p2 / evaluate_final_p2 — Protocol-2 evaluate path consuming wide ciphertexts"
  - "Private second_half_out_ev / first_half_out_ev BlockMatrix accumulators on both structs"
  - "Compile-time enforcement of P2 garbler privacy: garble_final_p2 returns (Vec<Block>, Vec<Block>) — no Vec<bool>, no masked wire value"

affects: [09-04, end-to-end-protocol-2, output-share-consistency-check]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Dual-accumulator pattern: each public method writes BOTH first_half_out (D_gb) and first_half_out_ev (D_ev) in a single GGM tree pass via the wide kernel"
    - "Disjoint-field borrow + nested with_subrows: split &mut self into two non-conflicting BlockMatrix borrows then nest with_subrows closures over disjoint backing storage"
    - "_p2 method-suffix convention for protocol-2 variants on shared structs (CONTEXT.md D-09)"

key-files:
  created: []
  modified:
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs

key-decisions:
  - "Garbler-side D_ev correlated encoding emits mac.as_block() directly with NO delta_b XOR — gb does not hold delta_b. Eval-side mirror applies delta_b to the local key view: if bit() then delta_b ^ key else key."
  - "Nested with_subrows over two disjoint BlockMatrix fields (first_half_out + first_half_out_ev) compiles and runs correctly — split-borrow at the top of each chunk iteration yields two independently-scoped sub-views."
  - "evaluate_final_p2 also runs the D_gb assembly (same as evaluate_final) so callers reading first_half_out after evaluate_final_p2 see the full P1-equivalent value, matching the symmetric behavior on the gen side."

patterns-established:
  - "Wide chunked helper signature: (&mut self, x, y_d_gb, y_d_ev, [chunk_levels, chunk_cts], first_half) returning Vec<Vec<(Block, Block)>> — kappa+rho ciphertexts paired."
  - "y_d_ev helper functions return BlockMatrix(m,1) or BlockMatrix(n,1) of *.<field>_d_ev_shares[i].mac.as_block() — no y_labels XOR (rho-half carries no labels), no delta XOR on the gb side."

requirements-completed: [P2-02, P2-03]

# Metrics
duration: ~22 min
completed: 2026-04-25
---

# Phase 9 Plan 3: Protocol-2 Garble/Evaluate _p2 Methods Summary

**Dual-delta authenticated tensor garbling: `garble_first_half_p2` / `garble_second_half_p2` / `garble_final_p2` on `AuthTensorGen` plus the symmetric eval methods on `AuthTensorEval`. The garbler returns `(Vec<Block>, Vec<Block>)` — D_gb AND D_ev shares — and never sends a masked wire value across the boundary, enforcing P2 privacy at the type level.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-04-25T02:04:00Z (approx, captured at task start)
- **Completed:** 2026-04-25T02:26:16Z
- **Tasks:** 3 (Task 1: gen, Task 2: eval, Task 3: full-suite verification)
- **Files modified:** 2 (`src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`)

## Accomplishments

- Six new public `_p2` methods (3 gen + 3 eval) wired through Plan-02's wide GGM kernel.
- Two new private accumulator BlockMatrix fields per struct (`first_half_out_ev`, `second_half_out_ev`) initialized in both `new` and `new_from_fpre*` constructors.
- Two new private wide chunked helpers (`gen_chunked_half_outer_product_wide`, `eval_chunked_half_outer_product_wide`) using a disjoint-field split-borrow + nested `with_subrows` pattern to write both D_gb and D_ev outputs in a single tree pass.
- Compile-time P2 garbler-privacy enforcement: `garble_final_p2 -> (Vec<Block>, Vec<Block>)` — no `bool` / no `Vec<bool>`.
- All P1 method bodies untouched.
- Zero regressions: full cargo test suite passes 104/104 (was 101/101 prior; 3 new tests added in this plan).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add _p2 garble methods + wide chunked helper to AuthTensorGen** — `736104a` (feat)
2. **Task 2: Add _p2 evaluate methods + wide chunked helper to AuthTensorEval** — `d750ff7` (feat)
3. **Task 3: Full-suite verification** — no code changes (verification-only task; committed implicitly via Task 2's green test run)

## Method Signatures

### AuthTensorGen (`src/auth_tensor_gen.rs`)

```rust
pub fn garble_first_half_p2(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>);
pub fn garble_second_half_p2(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>);
pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>);  // (D_gb shares, D_ev shares)

// Private helpers:
pub(crate) fn gen_chunked_half_outer_product_wide(
    &mut self,
    x: &MatrixViewRef<Block>,
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    first_half: bool,
) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>);

fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix;
fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix;
```

### AuthTensorEval (`src/auth_tensor_eval.rs`)

```rust
pub fn evaluate_first_half_p2(
    &mut self,
    chunk_levels: Vec<Vec<(Block, Block)>>,
    chunk_cts: Vec<Vec<(Block, Block)>>,
);
pub fn evaluate_second_half_p2(
    &mut self,
    chunk_levels: Vec<Vec<(Block, Block)>>,
    chunk_cts: Vec<Vec<(Block, Block)>>,
);
pub fn evaluate_final_p2(&mut self) -> Vec<Block>;  // [v_gamma D_ev]^ev, length n*m

// Private helpers:
fn eval_chunked_half_outer_product_wide(
    &mut self,
    x: &MatrixViewRef<Block>,
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    chunk_levels: Vec<Vec<(Block, Block)>>,
    chunk_cts: Vec<Vec<(Block, Block)>>,
    first_half: bool,
);

fn get_first_inputs_p2_y_d_ev(&self) -> BlockMatrix;
fn get_second_inputs_p2_y_d_ev(&self) -> BlockMatrix;
```

## New Struct Fields

Both `AuthTensorGen` and `AuthTensorEval` gain:

```rust
pub first_half_out_ev: BlockMatrix,   // n × m
pub second_half_out_ev: BlockMatrix,  // m × n
```

Initialized to zero (`BlockMatrix::new(...)`) in both `new(n, m, chunking_factor)` and `new_from_fpre_*` constructors.

## D_ev Encoding Rule for the Correlated Share

The plan's careful re-derivation of the IT-MAC layout (CONTEXT.md D-09 + `gen_auth_bit` symmetry in `auth_tensor_fpre.rs:66-86`) drives the asymmetric encoding:

| Side | Public-bit encoding of `correlated_d_ev_shares[idx]` under `delta_b` |
|------|----------------------------------------------------------------------|
| **Gen** (`garble_final_p2`) | `mac.as_block()` directly — NO `delta_b` XOR. Garbler does not hold `delta_b`; its `mac` already equals `eval.key XOR bit*delta_b`. |
| **Eval** (`evaluate_final_p2`) | `if bit() then delta_b ^ key else key` — eval HOLDS `delta_b`, applies it to its local key. |

This is the precise mirror of the P1 `correlated_auth_bit_shares` encoding under `delta_a` with gen↔eval roles swapped. After both sides XOR their contributions, the result is a valid IT-MAC pair `(key, mac)` under `delta_b` for the bit `[v_gamma D_ev]`.

## y-input encoding for D_ev half (alpha / beta)

Both gen-side and eval-side `get_*_inputs_p2_y_d_ev` helpers use the same shape:

```rust
y_ev[i] = *self.<alpha|beta>_d_ev_shares[i].mac.as_block();
```

No `y_labels` XOR (the rho-half carries no wire labels — labels are only on the kappa-half / D_gb path) and no delta XOR on either side. The mac encoding is what the wide kernel expects to combine with the GGM seed accumulator on each side.

## Files Created/Modified

- `src/auth_tensor_gen.rs` — +246 lines (struct fields, constructor init, wide chunked helper, three `_p2` garble methods, two y_d_ev helpers, two unit tests)
- `src/auth_tensor_eval.rs` — +229 lines (struct fields, constructor init, wide chunked helper, three `_p2` evaluate methods, two y_d_ev helpers, one unit test)

## Decisions Made

1. **Disjoint-field split-borrow pattern works without restructuring.** The plan flagged a contingency: if nested `with_subrows` over two `MatrixViewMut` instances did not compile, the implementation should fall back to per-chunk temp matrices + XOR-back. In practice, splitting `&mut self` into `(&mut self.first_half_out, &mut self.first_half_out_ev)` at the top of each chunk yields two independent `MatrixViewMut` borrows over disjoint backing storage; nested `with_subrows` closures compose cleanly because each borrow tracks its own `&'a mut [T]`. Adopted this clean approach — no fallback needed.
2. **`evaluate_final_p2` also runs the D_gb assembly** (identical to `evaluate_final`) so that callers reading `first_half_out` after `evaluate_final_p2` see the same combined value as after the P1 path. This keeps the eval struct in a consistent post-P2 state and matches the symmetric behavior of `garble_final_p2` on the gen side.
3. **No P1 method bodies were modified** — the entire P2 path lives in new method bodies with `_p2` suffix. P1 callers remain bit-identical.

## Deviations from Plan

None — plan executed exactly as written. The plan's contingency notes (alternative borrow strategies if nested `with_subrows` failed) did not trigger; the preferred approach worked.

## Issues Encountered

None. Both task commits compiled clean on the first build, and all per-task and full-suite tests passed.

## Test Results

Full `cargo test` suite: **104 passed, 0 failed, 0 ignored** (baseline before this plan: 101 passing).

New tests added in this plan:
- `auth_tensor_gen::tests::test_garble_final_p2_returns_two_block_vecs_no_lambda` — verifies return type at compile time AND length n*m.
- `auth_tensor_gen::tests::test_garble_first_half_p2_returns_wide_ciphertexts` — verifies wide ciphertext type and non-empty chunks.
- `auth_tensor_eval::tests::test_evaluate_final_p2_returns_d_ev_share_vec` — end-to-end gen→eval P2 path returning Vec<Block> of length n*m.

All P1 regression tests (`test_auth_tensor_product`, `test_auth_tensor_product_full_protocol_1`, `test_protocol_1_check_zero_aborts_on_tampered_lambda`, all `auth_tensor_gen::tests::test_compute_lambda_gamma_*`, all `auth_tensor_eval::tests::test_compute_lambda_gamma_*`) remain green.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- `_p2` methods are wired and ready for Plan 04's end-to-end Protocol-2 integration test (`test_auth_tensor_product_full_protocol_2`).
- The garbler retains both shares privately; the evaluator produces a `Vec<Block>` D_ev output share. Plan 04 will combine these via Protocol-2's consistency check (parallel to Plan 08's P1 `check_zero` flow but with D_ev assembly).
- No blockers; no concerns for Plan 04.

## Self-Check: PASSED

Verified files exist:
- FOUND: src/auth_tensor_gen.rs
- FOUND: src/auth_tensor_eval.rs
- FOUND: .planning/phases/09-protocol-2-garble-eval-check/09-03-SUMMARY.md (this file)

Verified commits exist:
- FOUND: 736104a (Task 1)
- FOUND: d750ff7 (Task 2)

---
*Phase: 09-protocol-2-garble-eval-check*
*Plan: 03*
*Completed: 2026-04-25*
