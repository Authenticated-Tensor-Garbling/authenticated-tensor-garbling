---
phase: 05-m2-pi-atensor-correct-combining-construction-3
reviewed: 2026-04-22T21:30:51Z
depth: standard
files_reviewed: 3
files_reviewed_list:
  - src/auth_tensor_pre.rs
  - src/leaky_tensor_pre.rs
  - src/preprocessing.rs
findings:
  critical: 0
  warning: 3
  info: 2
  total: 5
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-04-22T21:30:51Z
**Depth:** standard
**Files Reviewed:** 3
**Status:** issues_found

## Summary

Reviewed the three source files implementing Pi_aTensor Construction 3: the
two-to-one combining combiner (`auth_tensor_pre.rs`), the leaky tensor
preprocessing protocol (`leaky_tensor_pre.rs`), and the pipeline entry point
(`preprocessing.rs`).

The core cryptographic logic — MAC assembly, the `two_to_one_combine` fold, the
`verify_cross_party` helper, and the `leaky_tensor_pre::generate()` transcript
— is correctly structured and the test coverage is thorough. No critical
security vulnerabilities or data-loss risks were found.

Three warnings require attention: a factually incorrect LSB-invariant claim in
`TensorFpreEval`'s doc comment for `delta_b`, a misleading `run_preprocessing`
API signature that panics on any `count != 1`, and a dead-computation pattern
in `eval_z_shares` assembly. Two informational items cover a redundant clone
and a fixed seed in production-facing preprocessing.

---

## Warnings

### WR-01: `TensorFpreEval.delta_b` doc comment claims wrong LSB invariant

**File:** `src/preprocessing.rs:43`

**Issue:** The field doc comment reads "`as_block().lsb() == 1` invariant" for
`delta_b`. This is factually incorrect. `delta_b` is always produced by
`Delta::random_b`, which explicitly sets LSB=0 (see `bcot.rs:52` and the
`test_delta_xor_lsb_is_one` regression). The protocol requires
`lsb(delta_a XOR delta_b) == 1`, which means `delta_b.lsb()` **must** be 0.
The erroneous claim is the mirror-copy of the correct `TensorFpreGen.delta_a`
comment on line 19 and will mislead anyone writing code that inspects
`eval_out.delta_b.as_block().lsb()`.

**Fix:** Change the `TensorFpreEval.delta_b` doc comment to reflect the actual invariant:

```rust
/// Evaluator's (Party B) global correlation key. `as_block().lsb() == 0` invariant
/// (required so that `lsb(delta_a XOR delta_b) == 1` per Pi_LeakyTensor §F).
pub delta_b: Delta,
```

---

### WR-02: `run_preprocessing` API accepts `count` but panics on any value other than 1

**File:** `src/preprocessing.rs:79-103`

**Issue:** The function signature is `run_preprocessing(n, m, count, chunking_factor)`,
implying `count` is a meaningful parameter. Line 85 immediately hard-panics
with `assert_eq!(count, 1, ...)`. The return type `(TensorFpreGen, TensorFpreEval)`
is structurally incapable of returning `count > 1` triples. A caller passing
any other value gets a runtime panic with no compile-time indication of the
restriction. This is a latent API misuse trap: future callers will pass `count`
expecting batch output, receive a confusing panic message at runtime, and have
to dig into the implementation to understand why.

**Fix:** Either restrict the signature to make the constraint visible at compile
time, or document the panic prominently in the function signature comment. The
minimal correct fix:

```rust
// Option A — remove the parameter entirely (preferred; callers of count=1 adjust call sites):
pub fn run_preprocessing(
    n: usize,
    m: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) { ... }

// Option B — keep parameter but document the panic as part of the contract:
/// # Panics
/// Panics if `count != 1`. Batch output (count > 1) requires a Vec-returning
/// variant that is not yet implemented.
pub fn run_preprocessing(
    n: usize,
    m: usize,
    count: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) { ... }
```

---

### WR-03: Dead computation in `eval_z_shares` construction

**File:** `src/leaky_tensor_pre.rs:308-315`

**Issue:** The `eval_d` share is constructed with all-zero fields
(`key: Key::default()`, `mac: Mac::default()`, `value: false`) and then added
to `eval_r_shares[k]`. Adding an all-zero `AuthBitShare` via `Add` is a no-op
(XOR with zero). The result is always exactly `eval_r_shares[k]`. The dead
addition costs no correctness (and the tests confirm this), but it misleads
a reader into believing the evaluator accumulates a D-dependent MAC contribution.
The comment block above (lines 286-305) explicitly documents that "eval holds no
Delta mass for D", making the code contradictory: the comment is correct but the
code implies there is something to add.

**Fix:** Eliminate the no-op addition and move the comment to the assignment site
to make the zero-contribution explicit:

```rust
// eval side: evaluator holds no D contribution (D is public; eval_z = eval_r only).
let eval_z_shares: Vec<AuthBitShare> = eval_r_shares;
```

If the Vec copy overhead matters, this also eliminates one full `n*m`-element
allocation and XOR pass.

---

## Info

### IN-01: `combine_leaky_triples` clones `triples[0]` when consuming the Vec

**File:** `src/auth_tensor_pre.rs:196-199`

**Issue:** `triples` is an owned `Vec<LeakyTriple>` consumed by value, yet the
fold starts with `triples[0].clone()` and then iterates the rest by reference
(`triples.iter().skip(1)`). Since the Vec is owned, `triples` could be
destructured with `into_iter()` to avoid the clone and eliminate one full
`LeakyTriple` allocation:

```rust
let mut iter = triples.into_iter();
let mut acc: LeakyTriple = iter.next().expect("bucket_size >= 1 asserted above");
for next in iter {
    acc = two_to_one_combine(acc, &next);
}
```

The comment on line 195 ("Clone triples[0] because LeakyTriple is not Copy") is
technically accurate but the clone is unnecessary when the Vec is owned; Rust
ownership allows moving out of an iterator. This is a quality improvement, not a
correctness issue.

---

### IN-02: Fixed seeds `(0, 1)` used for `IdealBCot` in `run_preprocessing`

**File:** `src/preprocessing.rs:93`

**Issue:** `IdealBCot::new(0, 1)` produces deterministic `delta_a` and `delta_b`
values for every call to `run_preprocessing`. Since `IdealBCot` is documented
as an ideal trusted-dealer substitute ("no networking — both parties' views
computed locally"), the fixed seeds are acceptable for benchmarks. However, the
function is `pub` and currently the only production-facing entry point. When the
`TODO: Replace with a real OT protocol` in `bcot.rs` is addressed, any call site
that relies on `run_preprocessing` will need the seed/randomness source reworked.
A comment at the call site would prevent silent reuse in a real context:

```rust
// BENCHMARK ONLY: IdealBCot uses fixed seeds — not suitable for production.
// Replace with a real OT-backed bcot when moving beyond Pi_aTensor uncompressed.
let mut bcot = IdealBCot::new(0, 1);
```

---

_Reviewed: 2026-04-22T21:30:51Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
