---
phase: 08-open-protocol-1-garble-eval-check
reviewed: 2026-04-23T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - src/auth_tensor_eval.rs
  - src/auth_tensor_gen.rs
  - src/lib.rs
  - src/online.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 08: Code Review Report

**Reviewed:** 2026-04-23
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Phase 08 adds `gamma_auth_bit_shares` to both `AuthTensorGen` and `AuthTensorEval`,
implements `compute_lambda_gamma()` on both sides, introduces `src/online.rs` with
`check_zero()`, and wires them together in two integration tests (`test_auth_tensor_product_full_protocol_1`,
`test_protocol_1_check_zero_aborts_on_tampered_lambda`).

The cryptographic core — the IT-MAC invariant, column-major indexing, delta domains,
and LSB extraction — is correct. The check_zero primitive is well-specified and the
ordering requirement (call after `garble_final` / `evaluate_final`) is properly documented.

Three warnings are raised: one about an IT-MAC linearity mismatch between the
`check_zero` docstring and the actual assembly pattern, one about an unenforced call-order
invariant that returns silently wrong values instead of panicking, and one about
`assemble_c_gamma_shares` not being usable in a real protocol (it references both
parties' private state). Two info-level findings cover debug `println!` left in test
code and a minor naming inconsistency in the `check_zero` parameter.

---

## Warnings

### WR-01: `check_zero` docstring describes the `Add`-operator combination pattern but `assemble_c_gamma_shares` does not follow it

**File:** `src/lib.rs:317-375` / `src/online.rs:28-42`

**Issue:** The `check_zero` docstring (lines 28-42 of `online.rs`) tells callers to
"pre-XOR the garbler-side and evaluator-side shares pairwise (using the `+` operator
on `AuthBitShare`)" so that `share.key = gen.key XOR ev.key` and `share.mac = gen.mac
XOR ev.mac`. However, `assemble_c_gamma_shares` in `lib.rs` does NOT follow this pattern.
It accumulates only the gen-side keys (`combined_key = XOR(gb.*.key)`), ignores all
eval-side keys and MACs entirely, and freshly recomputes `combined_mac = combined_key.auth(c_gamma_bit, &gb.delta_a)`.

This divergence has two consequences:

1. **Docstring contract is not exercised by any test.** No test validates the
   `AuthBitShare::add` combination path described in the docstring — the only callers of
   `check_zero` use freshly-minted MACs. If a future caller uses `AuthBitShare +
   AuthBitShare` naively (as the doc says) and passes those to `check_zero`, the check
   will fail even on correctly-formed shares because `gen.mac XOR ev.mac` is not equal
   to `combined_key.auth(c_gamma_bit, delta_a)` under the bCOT structure (the two
   parties' MACs commit under opposite deltas and have different roles).

2. **The comment in the docstring is actively misleading.** A maintainer following
   the documented pattern will produce an incorrect share and a false `check_zero` abort.

**Fix:** Reconcile the `check_zero` docstring with the actual usage pattern. Two options:

Option A — Update the docstring to document the freshly-computed-MAC pattern that
`assemble_c_gamma_shares` actually uses:
```rust
/// # Caller contract (per CONTEXT.md D-08)
///
/// The caller MUST assemble each `share` in `c_gamma_shares` with:
///   - `share.value`  = full reconstructed c_gamma bit (gen.value XOR ev.value)
///   - `share.key`    = XOR of all gen-side B-keys contributing to this gate
///   - `share.mac`    = `share.key.auth(share.value, delta_ev)`   ← freshly computed
///
/// Do NOT use the `AuthBitShare::add` (`+`) operator to combine cross-party shares
/// directly — the two parties' MACs are not in the same delta domain and will not
/// combine correctly without recomputing the MAC.
```

Option B — Add a dedicated `combine_for_check_zero` helper that encapsulates the
assembly pattern and documents the invariant in one place, removing the fragile
prose contract from the docstring.

---

### WR-02: `compute_lambda_gamma` silently returns garbage when called before `garble_final` / `evaluate_final`

**File:** `src/auth_tensor_gen.rs:220-238`, `src/auth_tensor_eval.rs:192-216`

**Issue:** Both `compute_lambda_gamma` implementations document "MUST be called AFTER
`garble_final()` / `evaluate_final()`" but this is enforced only by a prose comment —
the functions themselves do not check. Calling either method before the corresponding
`*_final()` step reads stale `first_half_out` values and returns a wrong `lambda` /
`L_gamma` vec without panicking.

This is a meaningful hazard in the protocol context: a caller that invokes
`compute_lambda_gamma` at the wrong point in the pipeline gets a silently wrong masked
output and either the consistency check incorrectly passes or the output decoding is
wrong. In a real protocol execution a logic bug of this kind could translate to an
undetected authentication failure.

**Fix:** Add a `bool` flag (e.g. `final_computed: bool`) to each struct and set it
in `garble_final` / `evaluate_final`. Assert on it at the top of `compute_lambda_gamma`:

```rust
// In AuthTensorGen / AuthTensorEval struct:
final_computed: bool,

// In garble_final() / evaluate_final():
self.final_computed = true;

// At the start of compute_lambda_gamma():
assert!(
    self.final_computed,
    "compute_lambda_gamma called before garble_final/evaluate_final — \
     first_half_out is not yet the combined v_gamma encoding"
);
```

Alternatively, if the Rust type system is preferred over a runtime flag, split the
struct into a pre-final and post-final newtype so the compiler enforces the ordering.

---

### WR-03: `assemble_c_gamma_shares` accesses both parties' private fields — not usable in a real protocol

**File:** `src/lib.rs:296-375`

**Issue:** `assemble_c_gamma_shares` takes both `&AuthTensorGen` and `&AuthTensorEval`
as arguments and directly reads `.value`, `.key`, and `.mac` from both parties'
`alpha_auth_bit_shares`, `beta_auth_bit_shares`, `correlated_auth_bit_shares`, and
`gamma_auth_bit_shares`. In a real two-party execution these fields are held on separate
machines; Party A never has access to Party B's `value` or `key` fields.

The function is scoped inside `#[cfg(test)]` and the comment correctly labels it as
"in-process simulation approach", so it will not appear in production builds. However,
if this function is ever promoted outside the test module (for example, into a
multi-party simulation harness), the fact that it silently requires both parties' state
would make it insecure in an MPC context.

Additionally, since the function freshly computes `combined_mac = combined_key.auth(c_gamma_bit, &gb.delta_a)`, the assembled share is trivially valid (MAC is crafted to match value) — `check_zero` verifies the MAC but the MAC was just constructed to pass. This means `test_auth_tensor_product_full_protocol_1` is not testing the MAC verification branch of `check_zero` for a genuinely independently-derived MAC; it only tests the `value == 0` branch. The MAC check in `check_zero` is only meaningfully exercised by `test_check_zero_fails_on_invalid_mac` in `online.rs`.

**Fix:** Add a `#[cfg(test)]` or `// SIMULATION ONLY` comment at the function signature
level (not just in the doc comment) to make the constraint more visible. Also add a test
note acknowledging that the MAC-verification branch of `check_zero` is not independently
exercised by the integration tests:

```rust
// SIMULATION ONLY: This function requires both parties' private state.
// In a real protocol, each party assembles its own half of c_gamma independently
// using only its own preprocessing shares, then runs check_zero on the combined share.
#[cfg(test)]
fn assemble_c_gamma_shares(...) { ... }
```

---

## Info

### IN-01: Debug `println!` left in `test_auth_tensor_product`

**File:** `src/lib.rs:451-454, 503`

**Issue:** Five `println!` calls remain in the `test_auth_tensor_product` test. These
are scoped inside `#[cfg(test)]` so they do not affect production code, but they
produce noise in `cargo test` output (the test runner shows output from passing tests
when run with `--nocapture`, and these will also appear in any CI log that captures
stdout). The comment on line 454 also has a typo ("ecah" instead of "each").

```rust
// lines 451-454
println!("gen_chunk_levels: {:?}", gen_chunk_levels.len());
println!("gen_chunk_levels[0] (each hold 2 blocks): {:?}", gen_chunk_levels[0].len());
println!("gen_chunk_cts: {:?}", gen_chunk_cts.len());
println!("gen_chunk_cts[0] (ecah hold one block): {:?}", gen_chunk_cts[0].len());
// line 503
println!();
```

**Fix:** Remove all five `println!` calls. If these diagnostics are wanted during
debugging, gate them behind `if cfg!(feature = "debug-output")` or replace with
`eprintln!` and suppress via test harness configuration.

---

### IN-02: `check_zero` parameter named `delta_ev` but called with `delta_a` (garbler's delta)

**File:** `src/online.rs:51`, `src/lib.rs:572, 629`

**Issue:** The `check_zero` function signature names its delta parameter `delta_ev`
(line 51), and the doc comment refers to it as "delta_ev" (lines 14, 59). However,
both call sites in `lib.rs` pass `&gb.delta_a` — the garbler's delta, not the evaluator's.

This is not a bug: the shares assembled by `assemble_c_gamma_shares` are authenticated
under `delta_a` (the gen-side global key, which is Party B's correlation key in the
D_ev-authenticated structure), and `check_zero` correctly verifies against that delta.
The naming confusion arises from the dual meaning of "D_ev" in the paper (the IT-MAC
structure used by the evaluator's preprocessing) versus "evaluator's own delta". In this
codebase `delta_a` IS the verifying delta for D_ev-authenticated shares.

**Fix:** Rename the parameter and update the doc comment to use a delta-neutral name
that does not imply it must be the evaluator's delta:

```rust
/// `delta_mac` is the global correlation key under which the `c_gamma_shares`
/// IT-MACs are authenticated (the verifying party's delta for the shares in question).
pub fn check_zero(c_gamma_shares: &[AuthBitShare], delta_mac: &Delta) -> bool {
```

This eliminates the ambiguity for future readers who map parameter names to the
two-party protocol roles.

---

_Reviewed: 2026-04-23_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
