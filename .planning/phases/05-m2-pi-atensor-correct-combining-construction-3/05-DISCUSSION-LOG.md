# Phase 5: M2 Pi_aTensor Correct Combining (Construction 3) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-22
**Phase:** 05-m2-pi-atensor-correct-combining-construction-3
**Areas discussed:** x combining, d reveal & MAC verification, bucket_size_for(ell) edge case

---

## x combining — paper vs. ROADMAP

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — x = x' ⊕ x'' per paper | XOR gen_x_shares and eval_x_shares across all B triples. Required for combined triple to satisfy Z = x ⊗ y. | ✓ |
| Confirm against appendix first | Check appendix_krrw_pre.tex before committing | |

**User's choice:** Confirmed x = x' ⊕ x'' per paper.
**Notes:** User identified that ROADMAP "keep x = x'" was a shorthand error. Paper
(appendix_krrw_pre.tex line 427) explicitly sets `itmac{x}{Δ} := itmac{x'}{Δ} ⊕ itmac{x''}{Δ}`.
User asked where the notation came from; confirmed after seeing the exact TeX lines cited.

---

## d reveal & MAC verification

| Option | Description | Selected |
|--------|-------------|----------|
| Verify IT-MAC equation on d shares | Assemble d_j shares by XORing y' and y'' AuthBitShares, then call AuthBitShare::verify(delta). Paper-faithful. | ✓ |
| No explicit verification | Compute d locally, no assertion. Simpler but doesn't exercise MAC check. | |
| Reuse F_eq module | Pass d shares into feq::check. Wrong tool — F_eq checks two-party output equality, not MAC equation. | |

**User's choice:** Verify IT-MAC equation via AuthBitShare::verify.
**Notes:** In-process substitute for "publicly reveal with appropriate MACs" from paper.
Verification failure panics (consistent with F_eq convention).

---

## bucket_size_for(ell) edge case

| Option | Description | Selected |
|--------|-------------|----------|
| B = SSP when ell ≤ 1 | Return SSP (=40) for ell < 2. Matches paper §3.1 naive combining. Correct for current count=1 use case. | ✓ |
| Require ell ≥ 2, panic otherwise | Assert ell >= 2. Breaks existing run_preprocessing(count=1) call site. | |
| floor(SSP / max(log2_ell, 1)) + 1 | Clamp log2_ell to 1. Returns 41 for ell=1. Technically wrong. | |

**User's choice:** B = SSP when ell ≤ 1.
**Notes:** With count=1 in run_preprocessing, ell=1 → B=40. The call site changes from
bucket_size_for(n, m) to bucket_size_for(count).

---

## Claude's Discretion

- `two_to_one_combine(prime: LeakyTriple, dprime: &LeakyTriple) -> LeakyTriple` as a
  pub(crate) helper — user did not specify, Claude chose this for TEST-05 testability
- Exact zero-share representation for d[j]==0 in x'' ⊗ d computation
- Delta assertion placement (outer wrapper vs. inner combine step)

## Deferred Ideas

None — discussion stayed within Phase 5 scope.
