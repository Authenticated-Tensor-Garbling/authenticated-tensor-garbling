---
phase: 09-protocol-2-garble-eval-check
plan: 04
subsystem: protocol
tags: [rust, protocol-2, integration-test, check-zero, e2e, c-gamma, delta-b]

# Dependency graph
requires:
  - phase: 09-01
    provides: "AuthTensorGen.gamma_d_ev_shares / AuthTensorEval.gamma_d_ev_shares (renamed from gamma_auth_bit_shares) plus alpha_d_ev_shares, beta_d_ev_shares, correlated_d_ev_shares — needed to source the eval-side keys for the c_gamma combined key under delta_b and to read the l_gamma share value for the c_gamma bit reconstruction"
  - phase: 09-02
    provides: "gen_unary_outer_product_wide / eval_unary_outer_product_wide — wide-leaf GGM expansions consumed transitively by the _p2 garble/eval methods"
  - phase: 09-03
    provides: "AuthTensorGen::garble_first_half_p2 / garble_second_half_p2 / garble_final_p2 and AuthTensorEval::evaluate_first_half_p2 / evaluate_second_half_p2 / evaluate_final_p2 — the full _p2 path the integration test drives end-to-end"

provides:
  - "assemble_c_gamma_shares_p2 test helper in src/lib.rs::tests — assembles c_gamma AuthBitShares verified under delta_b for the P2 path (3-term formula, eval-side keys from gamma_d_ev_shares)"
  - "test_auth_tensor_product_full_protocol_2 — single-gate P2 end-to-end integration test driving the _p2 garble/eval sequence, verifying D_gb correctness and check_zero passing under delta_b"
  - "Compile-time enforcement (via gb.garble_final_p2 return type Vec<Block>, Vec<Block>) verified end-to-end: garbler never sends a masked wire value; the test never reads or sends a Vec<bool> from the garbler"

affects: [end-to-end-protocol-2, p2-consistency-check, phase-9-completion]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Test-only c_gamma assembly under verifier delta: combined key from cross-party shares (eval-side keys for delta_b path; gen-side for delta_a path), MAC freshly recomputed via key.auth(c_gamma_bit, &delta) — never naively XOR cross-party MACs"
    - "Three-term P2 c_gamma formula: c_gamma_bit = v_gamma_bit XOR l_gamma_bit XOR L_gamma_pub — distinct from P1's six-term formula (no L_alpha · l_beta or L_beta · l_alpha or l_gamma* terms; correlated_d_ev_shares contribution is already folded into [v_gamma D_ev] by garble_final_p2 / evaluate_final_p2)"
    - "End-to-end honest-run correctness: under IdealPreprocessingBackend (which calls generate_for_ideal_trusted_dealer(0, 0)), masked_x = alpha and masked_y = beta — so v_alpha = v_beta = v_gamma = 0 and combined D_gb output is the zero block at every (i, j)"

key-files:
  created: []
  modified:
    - src/lib.rs

key-decisions:
  - "c_gamma combined key under delta_b uses ONLY the eval-side key from gamma_d_ev_shares (single field), mirroring P1 which uses gen-side keys from gamma_d_ev_shares + correlated_auth_bit_shares + alpha/beta_auth_bit_shares (multi-term sum). Justification: per gen_auth_bit symmetry under delta_b in auth_tensor_fpre.rs, ev.gamma_d_ev_shares[idx].key authenticates gb.value under delta_b. The other terms ([v_gamma D_ev]'s correlated contribution, alpha/beta) are already folded into the P2 c_gamma BIT through the wide-leaf D_ev accumulation in garble_final_p2 / evaluate_final_p2 — they don't appear as independent IT-MAC terms in the helper."
  - "Three-term bit reconstruction formula confirmed against CONTEXT.md D-13 / RESEARCH.md Pattern 3 / 6_total.tex step 9: c_gamma_bit = v_gamma_bit XOR l_gamma_bit XOR L_gamma_pub, where v_gamma_bit = (gb_d_ev_out[idx] XOR ev_d_ev_out[idx]).lsb() and l_gamma_bit = gb.gamma_d_ev_shares[idx].value XOR ev.gamma_d_ev_shares[idx].value. Pitfall 2 explicitly forbids copying P1's six-term assemble_c_gamma_shares formula."
  - "Verification delta is ev.delta_b (D_ev), NOT gb.delta_a (D_gb). Pitfall 1 warns that wrong-delta check_zero either passes vacuously or fails silently. Acceptance criterion `grep -c 'check_zero(&c_gamma_shares_p2, &ev.delta_b)'` enforces."
  - "D_gb correctness assertion uses combined == Block::default() rather than an x*y truth-table check. Justification: IdealPreprocessingBackend.run() invokes generate_for_ideal_trusted_dealer(0, 0) internally, so masked_x = alpha and masked_y = beta — therefore v_alpha = v_beta = 0 and v_gamma = 0 at every (i, j). The combined D_gb wire share is the zero block (key XOR key = 0). This mirrors the existing P1 test's tensor-product correctness loop with expected_val = false everywhere; a richer non-zero input case would require either tampering with masked_x/masked_y or using a non-trivial input setup, which is out of scope for the P2-05 single-gate test (RESEARCH.md Pattern 4: single-gate test is sufficient for Phase 9)."

patterns-established:
  - "P2 integration test idiom: drive `_p2` garble/eval sequence in-process, call `garble_final_p2` to get (D_gb, D_ev) tuple and `evaluate_final_p2` for the eval D_ev, reconstruct L_gamma per (i, j) from LSB-XOR of D_ev outputs and gamma_d_ev_shares, assemble c_gamma via the helper, assert check_zero under ev.delta_b."
  - "Cross-party combined key sourcing under delta_b: take the eval-side AuthBitShare.key (NOT gen-side). Symmetric to P1 which takes gen-side keys under delta_a. Documented in the helper's doc comment with the gen_auth_bit symmetry argument."

requirements-completed: [P2-04, P2-05]

# Metrics
duration: ~25 min
completed: 2026-04-25
---

# Phase 9 Plan 4: P2 End-to-End Test + check_zero Under delta_b Summary

**Single-gate end-to-end Protocol 2 honest run: drives `garble_first_half_p2 → evaluate_first_half_p2 → ... → garble_final_p2 / evaluate_final_p2`, asserts D_gb output is the zero block under input=0 IdealPreprocessing, and verifies the P2 consistency check `check_zero(&c_gamma_shares_p2, &ev.delta_b)` passes for honest parties — closing P2-04 and P2-05.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-25T02:10:00Z (worktree base verification)
- **Completed:** 2026-04-25T02:35:45Z

## What Was Built

### `assemble_c_gamma_shares_p2` Helper (src/lib.rs::tests)

Test-module helper that builds `Vec<AuthBitShare>` of length `n*m` verified under `ev.delta_b` for the P2 consistency check. Key differences from the existing P1 `assemble_c_gamma_shares` helper:

1. **Verifier delta:** `&ev.delta_b` (D_ev), not `&gb.delta_a` (D_gb).
2. **Combined key sourcing:** XOR of `ev.gamma_d_ev_shares[idx].key` (single field, eval-side) — not the multi-field XOR of `gb.alpha_auth_bit_shares[i].key + gb.beta_auth_bit_shares[j].key + gb.correlated_auth_bit_shares[idx].key + gb.gamma_d_ev_shares[idx].key` that P1 builds.
3. **Bit formula:** Three-term `c_gamma_bit = v_gamma_bit XOR l_gamma_bit XOR L_gamma_pub[idx]`, NOT P1's six-term formula. Per Pitfall 2 / 6_total.tex step 9, the `[v_gamma D_ev]` term already incorporates `correlated_d_ev_shares` and `alpha/beta` contributions through `garble_final_p2` / `evaluate_final_p2`'s wide-leaf D_ev accumulation, so the helper must NOT add those terms again.

The helper is `SIMULATION ONLY` — it requires both parties' state, mirroring the existing P1 helper. In a real protocol each party would assemble its own half independently and then run `check_zero` over the network.

### `test_auth_tensor_product_full_protocol_2` (src/lib.rs::tests)

Single-gate end-to-end Protocol 2 honest-run integration test:

1. Builds `IdealPreprocessingBackend` for `n=4, m=3`.
2. Runs the full `_p2` sequence: `garble_first_half_p2 → evaluate_first_half_p2 → garble_second_half_p2 → evaluate_second_half_p2 → garble_final_p2 → evaluate_final_p2`.
3. **Part A (D_gb correctness, P2-05):** Asserts `gb_d_gb_out[idx] XOR ev.first_half_out[(i, j)] == Block::default()` for all `(i, j)`. With `IdealPreprocessingBackend` (which uses `generate_for_ideal_trusted_dealer(0, 0)` internally), masked_x = alpha and masked_y = beta — so `v_gamma = 0` everywhere and the combined D_gb wire share is the zero block.
4. **Part B (P2 consistency check, P2-04):** Reconstructs `L_gamma_pub: Vec<bool>` from the LSB of `gb_d_ev_out[idx] XOR ev_d_ev_out[idx]` plus the XOR of `gb.gamma_d_ev_shares[idx].value` and `ev.gamma_d_ev_shares[idx].value`. Calls `assemble_c_gamma_shares_p2` to build c_gamma shares under `delta_b`. Asserts `check_zero(&c_gamma_shares_p2, &ev.delta_b)` returns `true`.

## Exact P2 c_gamma Formula Used

Per CONTEXT.md D-13 / RESEARCH.md Pattern 3 / `6_total.tex` step 9:

```
c_gamma = [v_gamma D_ev] XOR [l_gamma D_ev] XOR L_gamma * D_ev

  Garbler's share:    [c_gamma]^gb := [L_gamma D_ev]^gb            (no L_gamma correction)
  Evaluator's share:  [c_gamma]^ev := [L_gamma D_ev]^ev XOR L_gamma * D_ev

  Combined: c_gamma = [c_gamma]^gb XOR [c_gamma]^ev
                    = ([L_gamma D_ev]^gb XOR [L_gamma D_ev]^ev) XOR L_gamma * D_ev
                    = L_gamma * D_ev XOR L_gamma * D_ev = 0   (for honest parties)
```

In the helper, the bit-level reconstruction is implemented as:

```rust
let v_gamma_bit = (gb_d_ev_out[idx] ^ ev_d_ev_out[idx]).lsb();
let l_gamma_bit = gb.gamma_d_ev_shares[idx].value ^ ev.gamma_d_ev_shares[idx].value;
let c_gamma_bit = v_gamma_bit ^ l_gamma_bit ^ l_gamma_pub[idx];
```

For honest parties: `v_gamma_bit XOR l_gamma_bit = (v_gamma + l_gamma) bit = L_gamma_bit`, so `c_gamma_bit = L_gamma_bit XOR L_gamma_pub[idx] = 0`.

### Why three terms, not six (Pitfall 2)

P1's `c_gamma` (in `assemble_c_gamma_shares`) has six terms because Protocol 1's `c_gamma = v_gamma XOR (L_alpha tensor L_beta) XOR (l_gamma)` expands into masked-input × secret-mask cross terms. P2's `c_gamma` has only three terms because the wide-leaf D_ev path in `gen_unary_outer_product_wide` / `eval_unary_outer_product_wide` already accumulates the `correlated_d_ev_shares` contribution (and the masked-input × secret-mask cross terms) into `[v_gamma D_ev]` itself. The helper must therefore NOT re-add those terms — doing so would be a double-count and break the consistency check. This is the explicit Pitfall 2 warning in `09-RESEARCH.md`.

## Combined-Key Derivation Under delta_b

The combined key for the c_gamma `AuthBitShare` is sourced from the eval-side keys of `gamma_d_ev_shares`:

```rust
let combined_key = ev.gamma_d_ev_shares[idx].key;
let combined_mac = combined_key.auth(c_gamma_bit, &ev.delta_b);
```

### Justification

Per `gen_auth_bit` in `src/auth_tensor_fpre.rs` (the helper that populates all D_ev fields):

- `gen.mac = a_share.mac` where `a_share` is built under `delta_b` — this is the gen-side MAC authenticating `gen.value` under `delta_b` using the eval's `a_share.key`.
- `ev.key = a_share.key` — this is the canonical "key authenticating gb.value under delta_b".

So the eval-side `key` field on the cross-party `gamma_d_ev_shares[idx]` pair IS the IT-MAC verifier key for the combined bit under `delta_b`. The MAC is then freshly recomputed via `combined_key.auth(c_gamma_bit, &ev.delta_b)` — never via naive XOR of cross-party MACs (which would not satisfy the IT-MAC invariant under either delta). This is the exact mirror of the P1 helper's pattern (which uses gen-side keys under `delta_a`) and follows `online.rs::check_zero`'s caller contract verbatim.

## Test Suite Status

```
$ cargo test 2>&1 | grep -E "^test result"
test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

- **105/105 lib tests pass** (95 prior + 2 from Plan 01 + 4 from Plan 02 + 3 from Plan 03 + 1 from Plan 04 = 105). 0 failed, 0 ignored.
- **0 doctests** (no doctests in this codebase).
- **`cargo build` clean**: 0 errors. Pre-existing warnings (unused methods on internal `MatrixViewMut`/`MatrixViewRef`) only — none introduced by this plan.

### Phase 9 test inventory — all green

| Plan | Test | Status |
|------|------|--------|
| 09-01 | `preprocessing::tests::test_ideal_backend_d_ev_shares_lengths` | ok |
| 09-01 | `preprocessing::tests::test_ideal_backend_d_ev_shares_mac_invariant` | ok |
| 09-02 | `tensor_ops::tests::test_gen_unary_outer_product_wide_tweak_independence` | ok |
| 09-02 | `tensor_ops::tests::test_eval_unary_outer_product_wide_round_trip_kappa` | ok |
| 09-02 | `tensor_ops::tests::test_eval_unary_outer_product_wide_round_trip_rho` | ok |
| 09-02 | `tensor_ops::tests::test_wide_signature_shapes` | ok |
| 09-03 | `auth_tensor_gen::tests::test_garble_final_p2_returns_two_block_vecs_no_lambda` | ok |
| 09-03 | `auth_tensor_gen::tests::test_garble_first_half_p2_returns_wide_ciphertexts` | ok |
| 09-03 | `auth_tensor_eval::tests::test_evaluate_final_p2_returns_d_ev_share_vec` | ok |
| 09-04 | `tests::test_auth_tensor_product_full_protocol_2` | ok |

### P1 regression check — all green

| Test | Status |
|------|--------|
| `tests::test_auth_tensor_product` | ok |
| `tests::test_auth_tensor_product_full_protocol_1` | ok |
| `tests::test_protocol_1_check_zero_aborts_on_tampered_lambda` | ok |

## Rename Audit

```
$ grep -rn "gamma_auth_bit_shares" src/
$ # (zero matches)
```

Confirmed: zero matches. Plan 01's rename of `gamma_auth_bit_shares → gamma_d_ev_shares` remains complete with no stragglers introduced by this plan's changes.

## Key Acceptance Criteria

| Criterion | Result |
|-----------|--------|
| `grep -n "fn assemble_c_gamma_shares_p2" src/lib.rs \| wc -l` | 1 |
| `grep -n "fn test_auth_tensor_product_full_protocol_2" src/lib.rs \| wc -l` | 1 |
| `grep -n "ev.delta_b" src/lib.rs \| wc -l` (>= 2) | 4 |
| `grep -c "check_zero(&c_gamma_shares_p2, &ev.delta_b)" src/lib.rs` | 1 |
| `grep -c "garble_final_p2" src/lib.rs` (>= 1) | 4 |
| `grep -c "evaluate_final_p2" src/lib.rs` (>= 1) | 4 |
| `cargo test FAILED count` | 0 |
| `cargo test "test result: ok"` lines | 2 (>= 1) |
| `grep -rn "gamma_auth_bit_shares" src/ \| wc -l` | 0 |
| `grep -rn "gen_unary_outer_product_wide" src/ \| wc -l` (>= 3) | 10 |
| `grep -rn "eval_unary_outer_product_wide" src/ \| wc -l` (>= 3) | 7 |

All Task 1 and Task 2 acceptance criteria pass.

## Deviations from Plan

### One scope-of-execution recovery (no behavioral deviation)

**Initial wrong-path edit:** First edit was applied to the parent repository's `src/lib.rs` instead of the worktree's `src/lib.rs` because the absolute path `/Users/turan/Desktop/authenticated-tensor-garbling/src/lib.rs` resolves to the main checkout, not the worktree at `/Users/turan/Desktop/authenticated-tensor-garbling/.claude/worktrees/agent-ad44be12fc32af2d1/src/lib.rs`. The wrong-path change was stashed (`git stash drop` after recovery — not committed to main) and the same edit was re-applied to the worktree's `src/lib.rs`. Net effect: the worktree's commit contains the intended code; the main checkout is untouched.

**This is not a behavioral deviation** — the actual code change is identical to what the plan specifies. Logged here for audit completeness.

### Otherwise: plan executed exactly as written

- Helper signature, formula, and key-derivation match `09-04-PLAN.md` Step B verbatim.
- Test scaffolding matches `09-04-PLAN.md` Step C verbatim, including the Part A `combined == Block::default()` assertion (the plan's NOTE on Part A explicitly justifies this assertion under input=0 IdealPreprocessing).
- No Rule 1 / Rule 2 / Rule 3 fixes needed — code compiled and the test passed first try after applying to the correct worktree path.
- No CLAUDE.md present in the working tree, so no project-wide enforcement adjustments.
- No threat-model surface introduced beyond the threat register's `T-9-04` (mitigated by the `&ev.delta_b` acceptance criterion) and `T-9-10` (mitigated by the three-term formula). `T-9-11` (delta_b leak in test) is `accept` — in-process simulation has both parties' state by construction.

## Self-Check

Created files exist:

- `[ ] FOUND` — no new files created (plan was modify-only on `src/lib.rs`).

Modified files contain new code:

- `grep -c "fn assemble_c_gamma_shares_p2" src/lib.rs` → 1 ✓
- `grep -c "fn test_auth_tensor_product_full_protocol_2" src/lib.rs` → 1 ✓

Commits exist on this branch:

- `git log --oneline | grep ec6191c` → `ec6191c feat(09-04): add P2 c_gamma helper and end-to-end Protocol 2 test` ✓

## Self-Check: PASSED
