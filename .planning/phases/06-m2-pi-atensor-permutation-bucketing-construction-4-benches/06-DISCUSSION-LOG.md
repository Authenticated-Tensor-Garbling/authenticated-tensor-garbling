# Phase 6: M2 Pi_aTensor' Permutation Bucketing (Construction 4) + Benches - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-22
**Phase:** 06-m2-pi-atensor-permutation-bucketing-construction-4-benches
**Areas discussed:** bucket_size_for API change, Permutation application site, Permutation RNG / seed, TEST-06 scope

---

## bucket_size_for API change

| Option | Description | Selected |
|--------|-------------|----------|
| Replace with 2-arg | Change bucket_size_for(ell) to bucket_size_for(n, ell). One function, no dead code. Construction 3 formula disappears. | ✓ |
| Add bucket_size_for_prime(n, ell) | Keep old function, add new one alongside. Preserves Construction 3 formula for reference. | |

**User's choice:** Replace existing function with 2-arg version.
**Notes:** No parallel function needed. All call sites in this single crate are updated.

---

## Permutation application site

| Option | Description | Selected |
|--------|-------------|----------|
| Inside combine_leaky_triples | Activate _shuffle_seed; permute each triple before the fold loop. Callers unchanged. | ✓ |
| New permute_triple helper (external) | pub(crate) function applied by run_preprocessing before passing triples in. combine_leaky_triples unchanged. | |

**User's choice:** Inside `combine_leaky_triples`.
**Notes:** `_shuffle_seed` was already reserved for this exact purpose in Phase 5.

---

## Permutation RNG / seed

| Option | Description | Selected |
|--------|-------------|----------|
| shuffle_seed XOR triple-index | ChaCha12Rng::seed_from_u64(shuffle_seed ^ j). Deterministic, reproducible. | ✓ |
| thread_rng inside combine | Truly random, non-reproducible. Tests rely on product invariant statistically. | |
| Single seed, sequential advance | One ChaCha12Rng advanced across all triples (same as LeakyTensorPre approach). | |

**User's choice:** `shuffle_seed ^ triple_index as u64` seeding.
**Notes:** `run_preprocessing` passes fixed seed 42 for deterministic test behavior.

---

## TEST-06 scope

| Option | Description | Selected |
|--------|-------------|----------|
| Product invariant only | Z = x ⊗ y + MAC invariant through run_preprocessing. Same structure as TEST-05. | ✓ |
| Product invariant + permutation check | Also assert at least one triple had non-identity permutation applied. | |

**User's choice:** Product invariant only.
**Notes:** Product invariant holding after permutation implies permutation is algebraically consistent. No explicit non-triviality assertion needed.

---

## Claude's Discretion

- Naming of renamed `_shuffle_seed` parameter (`shuffle_seed`)
- Whether `apply_permutation_to_triple` is a standalone free function or inline logic
- Fisher-Yates vs `SliceRandom::shuffle` for permutation sampling
- Whether D-12 bucket size comparison assertion is a separate test or folded into TEST-06

## Deferred Ideas

None — discussion stayed within Phase 6 scope.
