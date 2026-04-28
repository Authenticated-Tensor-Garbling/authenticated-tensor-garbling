# Fix: IdealPreprocessingBackend D_ev bits must reuse D_gb bits

## What to do

Implement this fix now. It is small, fully understood, and localized to one function.

## The bug

`IdealPreprocessingBackend::run` in `src/preprocessing.rs` (line ~153) samples **fresh independent random bits** for the three D_ev mask fields:

```rust
let l_alpha: bool = rng_alpha.random_bool(0.5);  // seed 43 — wrong: fresh bit
let l_beta:  bool = rng_beta.random_bool(0.5);   // seed 44 — wrong: fresh bit
let l_corr:  bool = rng_corr.random_bool(0.5);   // seed 45 — wrong: fresh bit
```

According to F_cpre (`references/.../appendix_cpre.tex` line 64), the D_ev-authenticated
shares authenticate the **same underlying bits** as the D_gb-authenticated shares — just
under Δ_ev instead of Δ_gb. The ideal backend should call `fpre.gen_auth_bit(same_bit)`
on the bits already stored in `fpre.alpha_auth_bits`, `fpre.beta_auth_bits`, and
`fpre.correlated_auth_bits`, not sample new ones.

## The fix

In `IdealPreprocessingBackend::run`, replace the three independently-seeded RNG loops
with calls to `fpre.gen_auth_bit` on the bits already stored by
`generate_for_ideal_trusted_dealer`:

| Field | Old (wrong) | New (correct) |
|---|---|---|
| `alpha_d_ev_bits[i]` | `fpre.gen_auth_bit(rng_alpha.random_bool(0.5))` | `fpre.gen_auth_bit(fpre.alpha_auth_bits[i].full_bit())` |
| `beta_d_ev_bits[j]` | `fpre.gen_auth_bit(rng_beta.random_bool(0.5))` | `fpre.gen_auth_bit(fpre.beta_auth_bits[j].full_bit())` |
| `correlated_d_ev_bits[k]` | `fpre.gen_auth_bit(rng_corr.random_bool(0.5))` | `fpre.gen_auth_bit(fpre.correlated_auth_bits[k].full_bit())` |

`gamma_d_ev_bits` stays independently sampled (seed 42) — `l_gamma` is a fresh output
mask, not a re-authentication of an existing input mask.

## Access constraint

`alpha_auth_bits`, `beta_auth_bits`, and `correlated_auth_bits` are private fields of
`TensorFpre` (`src/auth_tensor_fpre.rs`). The fix must either:

- **Option A (preferred):** Move the D_ev generation inside `TensorFpre` —
  add a method like `generate_d_ev_shares(&mut self)` that reads its own private
  fields and calls `gen_auth_bit` on them, called before `into_gen_eval()`.
- **Option B:** Make the three fields `pub` temporarily and do it in
  `IdealPreprocessingBackend::run` as today.

Either way, all `gen_auth_bit` calls must happen **before** `into_gen_eval()` is called,
because `into_gen_eval(self)` consumes `fpre` by value.

## What to verify after the fix

- Existing tests in `preprocessing.rs` still pass (dimensions, MAC invariant, etc.)
- `test_ideal_backend_gamma_d_ev_shares_mac_invariant` still passes
- `test_ideal_backend_d_ev_shares_mac_invariant` still passes
- The bit value of `alpha_d_ev_shares[i]` now matches `alpha_auth_bit_shares[i]`
  (both parties' XOR of `.value` gives the same underlying bit)
- `gamma_d_ev_shares` bits remain distinct from `correlated_auth_bit_shares` bits
  (existing `test_ideal_backend_gamma_distinct_from_correlated` should still pass,
  since gamma stays independent)
