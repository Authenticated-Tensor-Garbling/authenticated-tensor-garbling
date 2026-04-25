---
phase: 10-wall-clock-benchmarks
fixed_at: 2026-04-24T00:00:00Z
fix_scope: critical_warning
findings_in_scope: 1
fixed: 1
skipped: 0
iteration: 1
status: all_fixed
---

# Phase 10: Code Review Fix Report

**Fixed:** 2026-04-24
**Scope:** critical_warning (Critical + Warning only)
**Findings in scope:** 1
**Fixed:** 1
**Skipped:** 0
**Status:** all_fixed

## Fixed

### WR-01: Networking benchmark uses uncorrelated generator/evaluator pairs
**File:** `benches/benchmarks.rs`
**Commit:** 2520f2e
**Applied fix:** Added documentation comments clarifying the intentional mismatch — that generator and evaluator are constructed from independent TensorFpre instances with different random seeds. The comments explain this is a timing-only benchmark measuring garble-time + network-transfer latency; correctness of the evaluate output is not tested. One block comment was added before the sizing-run `setup_auth_gen` call (lines 395-400), and a shorter inline comment was added inside the `iter_batched` setup closure before the uncorrelated tuple construction (line 425).

## Skipped

None.

## Out of Scope (Info)

IN-01, IN-02, IN-03, IN-04 — excluded from critical_warning fix scope.

---

_Fixed: 2026-04-24_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
