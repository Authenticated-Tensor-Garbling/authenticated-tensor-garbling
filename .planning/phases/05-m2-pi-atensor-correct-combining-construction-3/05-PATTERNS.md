# Phase 5: M2 Pi_aTensor Correct Combining (Construction 3) - Pattern Map

**Mapped:** 2026-04-22
**Files analyzed:** 2 modified (no new files)
**Analogs found:** 2 / 2 (both exact — same file, body rewrite)

---

## File Classification

| New/Modified File | Status | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|--------|------|-----------|----------------|---------------|
| `src/auth_tensor_pre.rs` | REWRITTEN (body) + EXTENDED (new helper + new tests) | module (`bucket_size_for`, `combine_leaky_triples`, NEW `two_to_one_combine`) | transform (pure fold over leaky triples → authenticated triple) | self (existing file) + `src/leaky_tensor_pre.rs` (cross-party layout + verify helpers) | exact (same file; function body rewrites + additions) |
| `src/preprocessing.rs` | MODIFIED (one-line call site) | caller (`run_preprocessing`) | config / wiring | self (existing `bucket_size_for` call site at line 87) | exact (signature argument change) |

No new files. No new external dependencies. Every symbol Phase 5 needs is already in-crate.

---

## Shared Patterns

These cross-cutting patterns apply to multiple functions being added/rewritten in Phase 5. Source paths and line numbers are verified against the files read (snapshot `src/auth_tensor_pre.rs` 184 lines, `src/sharing.rs` 209 lines, `src/leaky_tensor_pre.rs` 648 lines, `src/preprocessing.rs` 135 lines).

### Pattern S1: `AuthBitShare + AuthBitShare` field-wise XOR (the combining atom)
**Source:** `src/sharing.rs:66-77` (and three more `Add` overloads at lines 79-116)
**Apply to:** `two_to_one_combine` — d-assembly, x combining, Z combining, tensor-product accumulation

```rust
impl Add<AuthBitShare> for AuthBitShare {
    type Output = Self;

    #[inline]
    fn add(self, rhs: AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}
```

**Usage in Phase 4 (already live, verified at `src/auth_tensor_pre.rs:77-78`):**

```rust
combined_gen_z[k] = combined_gen_z[k] + t.gen_z_shares[k];
combined_eval_z[k] = combined_eval_z[k] + t.eval_z_shares[k];
```

All three Phase 5 XOR combinations reduce to this single primitive; no custom helper needed.

### Pattern S2: Cross-party MAC verification (`verify_cross_party`)
**Source:** `src/auth_tensor_pre.rs:134-152` (test helper) AND `src/leaky_tensor_pre.rs:344-362` (test helper, verbatim duplicate)
**Apply to:** `two_to_one_combine` Step B (d-reveal MAC check) — PROMOTE from test-only to a `pub(crate)` function so the non-test combining body can call it. Also reused in TEST-05 happy-path assertions.

```rust
// Source: src/auth_tensor_pre.rs:134-152 (verified)
fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare {
        key: eval_share.key,
        mac: gen_share.mac,
        value: gen_share.value,
    }
    .verify(delta_b);
    AuthBitShare {
        key: gen_share.key,
        mac: eval_share.mac,
        value: eval_share.value,
    }
    .verify(delta_a);
}
```

**Underlying panic contract (`src/sharing.rs:59-63`):**

```rust
pub fn verify(&self, delta: &Delta) {
    let want: Mac = self.key.auth(self.bit(), delta);
    assert_eq!(self.mac, want, "MAC mismatch in share");
}
```

The substring `"MAC mismatch in share"` is the exact panic message to match in the tamper-path `#[should_panic(expected = ...)]` attribute (TEST-05 tamper test).

**CRITICAL:** Never call `share.verify(&delta)` directly on a raw cross-party `AuthBitShare` — it panics because `gen.key` comes from one bCOT direction and `gen.mac` from another. Always use `verify_cross_party`. This constraint is documented at `src/leaky_tensor_pre.rs:29-35`.

### Pattern S3: `AuthBitShare::default()` for the zero share
**Source:** `src/sharing.rs:42` (`#[derive(Debug, Clone, Default, Copy)]`)
**Apply to:** `two_to_one_combine` Step D — the `d[j] == 0` branch of the `x'' ⊗ d` tensor product.

```rust
let zero_share = AuthBitShare::default();
// zero_share.key = Key::default() (Block::ZERO), mac = Mac::default() (Block::ZERO), value = false
```

**Live precedent (`src/leaky_tensor_pre.rs:297-314`):** Phase 4's own Z-assembly constructs a zero share inline when `d_bits[k] == false`, using `Key::default()` and `Mac::default()` directly. Phase 5 can use either idiom — `AuthBitShare::default()` is the tersest.

### Pattern S4: Column-major nested loop `for j in 0..m { for i in 0..n { k = j*n + i } }`
**Source:** `src/leaky_tensor_pre.rs:240-247, 256-261, 269-278` (three live examples)
**Apply to:** `two_to_one_combine` Step D — the `x'' ⊗ d` assembly.

```rust
// Source: src/leaky_tensor_pre.rs:269-278 (L_1/L_2 assembly)
for j in 0..self.m {
    for i in 0..self.n {
        let k = j * self.n + i;
        let d_term_a = if d_bits[k] { delta_a_block } else { Block::ZERO };
        let d_term_b = if d_bits[k] { delta_b_block } else { Block::ZERO };
        l_1[(i, j)] = s_1[(i, j)] ^ d_term_a;
        l_2[(i, j)] = s_2[(i, j)] ^ d_term_b;
    }
}
```

**Invariant:** outer `j` / inner `i` with flat index `k = j*n + i`. Matches `LeakyTriple.gen_z_shares` column-major storage (see `src/leaky_tensor_pre.rs:42`: `length n*m, column-major: index = j*n + i (j = y index, i = x index)`).

### Pattern S5: Same-delta assertion over a Vec<LeakyTriple>
**Source:** `src/auth_tensor_pre.rs:53-68` (verified, live)
**Apply to:** `combine_leaky_triples` (retain verbatim). Per D-12 Claude-discretion, re-assert weakly inside `two_to_one_combine` for unit-test safety.

```rust
let delta_a = triples[0].delta_a;
let delta_b = triples[0].delta_b;
for (idx, t) in triples.iter().enumerate() {
    assert_eq!(
        t.delta_a.as_block(),
        delta_a.as_block(),
        "triple[{}] delta_a differs from triple[0] delta_a — all triples must share the same IdealBCot",
        idx
    );
    assert_eq!(
        t.delta_b.as_block(),
        delta_b.as_block(),
        "triple[{}] delta_b differs from triple[0] delta_b — all triples must share the same IdealBCot",
        idx
    );
}
```

### Pattern S6: `#[cfg(test)] mod tests` inline module with `#[should_panic(expected = "...")]`
**Source:** `src/auth_tensor_pre.rs:110-184` (existing test module — extend it), `src/leaky_tensor_pre.rs:331-647` (extensive precedent), `src/feq.rs` and `src/bcot.rs` test modules
**Apply to:** New TEST-05 tests (happy-path + tamper-path) added to the existing `src/auth_tensor_pre.rs:110` test module.

```rust
// Source: src/leaky_tensor_pre.rs:627-646 (verified — exact abort-path pattern)
#[test]
#[should_panic(expected = "F_eq abort")]
fn test_f_eq_abort_on_tampered_transcript() {
    // ... construct deliberately-inconsistent state ...
    crate::feq::check(&l_1, &l_2);  // must panic with the expected substring
}
```

Phase 5 tamper-test swaps the expected substring to `"MAC mismatch in share"` (Pattern S2's panic message).

### Pattern S7: `make_triples(n, m, count)` test fixture — reuse as-is
**Source:** `src/auth_tensor_pre.rs:120-129`
**Apply to:** All TEST-05 tests — call `make_triples(n, m, 2)` for the two-triple combine test; `make_triples(n, m, B)` for the full-bucket test.

```rust
fn make_triples(n: usize, m: usize, count: usize) -> Vec<LeakyTriple> {
    // Single shared IdealBCot — ALL triples get the same delta_a and delta_b.
    let mut bcot = IdealBCot::new(42, 99);
    let mut triples = Vec::new();
    for seed in 0..count {
        let mut ltp = LeakyTensorPre::new(seed as u64, n, m, &mut bcot);
        triples.push(ltp.generate());
    }
    triples
}
```

### Pattern S8: Deterministic seeded `IdealBCot::new(42, 99)` in tests
**Source:** `src/auth_tensor_pre.rs:122`, `src/leaky_tensor_pre.rs:366, 454, 507, 542, 573, 605, 622`
**Apply to:** All TEST-05 tests.

Test seeds `(42, 99)` for the bCOT are the codebase convention and produce deltas with `lsb(Δ_A ⊕ Δ_B) == 1` (required precondition, see `src/bcot.rs` test `test_delta_xor_lsb_is_one`). Per-`LeakyTensorPre` seeds vary to get independent triples inside one test.

### Pattern S9: Module-internal import style (path-grouped `use crate::{...}`)
**Source:** `src/auth_tensor_pre.rs:1-4` (existing imports), extended pattern in `src/leaky_tensor_pre.rs:7-17` and `src/preprocessing.rs:7-10`. CONVENTIONS.md lines 90-103 document this as project-wide.
**Apply to:** Any newly-required symbols for `two_to_one_combine` — add to the existing `use crate::{...}` block at the top of `src/auth_tensor_pre.rs`.

```rust
// Existing at src/auth_tensor_pre.rs:1-4
use crate::{
    preprocessing::{TensorFpreGen, TensorFpreEval},
    leaky_tensor_pre::LeakyTriple,
};

// Phase 5 additions (needed for two_to_one_combine and its use of verify_cross_party):
use crate::sharing::AuthBitShare;
use crate::delta::Delta;
// (Already imported in the test module at lines 113-116; promote to file-level when
// verify_cross_party is lifted out of #[cfg(test)].)
```

### Pattern S10: Panic-not-Result for protocol violations
**Source:** `.planning/codebase/CONVENTIONS.md:105-111`, `src/sharing.rs:62`, `src/bcot.rs:48`, `src/auth_tensor_pre.rs:46, 56-68`
**Apply to:** All new assertions (dimension checks in `two_to_one_combine`, `ell <= 1` guard in `bucket_size_for`, MAC-verify panic from `AuthBitShare::verify`).

Never return `Result<(), Error>` from protocol code. Every precondition/invariant violation is an immediate `panic!` / `assert!` / `assert_eq!` with a descriptive message.

---

## Pattern Assignments

### `src/auth_tensor_pre.rs` — MODIFIED (module: transform / CRUD-style fold)

**Role:** module hosting `bucket_size_for` (pure arithmetic utility), `combine_leaky_triples` (iterative fold wrapper), and the new `two_to_one_combine` helper (single-step combine — paper algebra). Plus inline `#[cfg(test)]` module.

**Data flow:** transform (Vec<LeakyTriple> → LeakyTriple → (TensorFpreGen, TensorFpreEval)); no I/O, no interaction.

**Primary analog:** self — verified read of the entire 184-line file. The three functions under edit already exist (or their shells do); Phase 5 rewrites bodies and adds one new helper.

**Secondary analog for the cross-party share handling:** `src/leaky_tensor_pre.rs:297-362` (Z-assembly with zero shares + verify_cross_party test helper).

**Secondary analog for panic message conventions:** `src/sharing.rs:59-63` (`"MAC mismatch in share"`), `src/feq.rs` (`"F_eq abort"`), Pattern S10.

#### Imports pattern

Existing at `src/auth_tensor_pre.rs:1-4`:

```rust
use crate::{
    preprocessing::{TensorFpreGen, TensorFpreEval},
    leaky_tensor_pre::LeakyTriple,
};
```

**Phase 5 extension** (add to the same `use crate::{...}` block, keeping alphabetical order within the group per CONVENTIONS.md lines 88-103):

```rust
use crate::{
    delta::Delta,
    leaky_tensor_pre::LeakyTriple,
    preprocessing::{TensorFpreGen, TensorFpreEval},
    sharing::AuthBitShare,
};
```

Rationale: `AuthBitShare` is needed for the new `two_to_one_combine` body and the promoted `verify_cross_party`; `Delta` is needed for the `verify_cross_party` signature.

#### Fix `bucket_size_for` — copy arithmetic shape, swap `n*m` → `ell`

**Existing code (`src/auth_tensor_pre.rs:15-21`):**

```rust
pub fn bucket_size_for(n: usize, m: usize) -> usize {
    const SSP: usize = 40;
    let ell = n * m;
    // floor(log2(ell)) for ell >= 2
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

**Replacement pattern (per D-08 / D-09 / RESEARCH.md Example 1):** preserve the `(usize::BITS - ell.leading_zeros() - 1)` integer-log2 trick verbatim (keeps the code style consistent with the old version) and add the `ell <= 1` guard (Pitfall 3):

```rust
/// Compute the bucket size B for Pi_aTensor (Construction 3, Theorem 1).
///
/// Formula: `B = floor(SSP / log2(ell)) + 1` for `ell >= 2`, where SSP = 40.
/// For `ell <= 1`, the bucketing amplification is degenerate; fall back to
/// the naïve combining bound of B = SSP (paper §3.1 preamble).
///
/// Parameters:
///   ell — number of OUTPUT authenticated tensor triples desired (NOT n*m).
///
/// Examples:
///   bucket_size_for(1)    = 40   (naïve fallback)
///   bucket_size_for(2)    = 41   (log2 = 1 → 40 + 1)
///   bucket_size_for(16)   = 11   (floor(40/4) + 1)
///   bucket_size_for(128)  = 6    (floor(40/7) + 1)
///   bucket_size_for(1024) = 5    (floor(40/10) + 1)
pub fn bucket_size_for(ell: usize) -> usize {
    const SSP: usize = 40;
    if ell <= 1 {
        return SSP;
    }
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

#### NEW `two_to_one_combine` helper — paper algebra, composed from Shared Patterns S1–S5

Per D-11, add a `pub(crate)` helper. The skeleton assembles:
- Pattern S1 three times (d assembly, x combining, Z combining)
- Pattern S2 inside a per-`j` loop (promoted from test-only to crate-visible)
- Pattern S3 for the `d[j] == 0` branch of `x'' ⊗ d`
- Pattern S4 for the column-major Z loop
- Pattern S5 (same-delta assertion, weak form) at the entry

Full skeleton is documented in RESEARCH.md "Architecture Patterns → Pattern 4" (lines 313-376) and copies every idiom from existing code. Literal paper-to-code mapping:

| Paper (appendix_krrw_pre.tex) | Code |
|-------------------------------|------|
| Line 427: `x := x' XOR x''` | `gen_x_shares[i] + dprime.gen_x_shares[i]` (Pattern S1) |
| Line 427: `y := y'` | `prime.gen_y_shares` (move, no operation) |
| Line 428: `d := y' XOR y''` | `gen_y_shares[j] + dprime.gen_y_shares[j]` (Pattern S1) |
| Line 428: "publicly reveal d with appropriate MACs" | `verify_cross_party(&gen_d[j], &eval_d[j], &delta_a, &delta_b)` (Pattern S2) |
| Line 429/443: `Z := Z' XOR Z'' XOR itmac{x''}{Δ} ⊗ d` | column-major loop Pattern S4, with Pattern S1 XOR and Pattern S3 zero share |
| Line 437: "computed locally by scaling each authenticated row of itmac{x''}{Δ} by the corresponding public bit d_k, with no additional interaction" | no bCOT/macro call — pure local XOR fold |

The helper signature consumes `prime: LeakyTriple` (by value) and borrows `dprime: &LeakyTriple`. This matches the fold pattern in Pattern 3 of RESEARCH.md and forces the `prime.gen_y_shares` move in Step E (no clone needed).

**Rust ownership pitfall (RESEARCH.md Pitfall 4):** `LeakyTriple` contains `Vec<AuthBitShare>` which is `Clone` but not `Copy`. In the fold, `triples[0]` must be explicitly cloned into `acc` before iteration; subsequent iterations consume `acc` by value and re-assign the returned `LeakyTriple`. See Pattern 3 of RESEARCH.md for the exact fold shape.

#### Rewrite `combine_leaky_triples` body — thin fold wrapper

**Existing body (`src/auth_tensor_pre.rs:38-107`):** XORs all B triples' `gen_z_shares` / `eval_z_shares` element-wise into `combined_gen_z` / `combined_eval_z`, keeps `triples[0]`'s x/y shares unchanged, and packages into `(TensorFpreGen, TensorFpreEval)`. The XOR-combination of Z is the silent-correctness bug (RESEARCH.md Pitfall 1).

**Replacement pattern (RESEARCH.md Pattern 3 / Example 2):** keep the precondition asserts (length check, `bucket_size >= 1`, same-delta check — all verbatim copies of Pattern S5 + existing lines 46-68). Replace the Z-XOR loop with an iterative fold:

```rust
let mut acc: LeakyTriple = triples[0].clone();  // Pitfall 4: clone, don't move
for next in triples.iter().skip(1) {
    acc = two_to_one_combine(acc, next);
}
```

Then pack `acc` into `(TensorFpreGen, TensorFpreEval)` using the existing packaging code at `src/auth_tensor_pre.rs:83-107` (the `alpha_auth_bit_shares`, `beta_auth_bit_shares`, `correlated_auth_bit_shares`, `alpha_labels: Vec::new()`, `beta_labels: Vec::new()` skeleton is unchanged — only the source of those fields changes from `t0.gen_x_shares.clone()` to `acc.gen_x_shares` and similar).

The `alpha_labels` / `beta_labels` `Vec::new()` stubs are preserved per Phase 4 D-07 (CONTEXT.md for Phase 4, referenced at `src/auth_tensor_pre.rs:90-91, 101-102`).

#### Promote `verify_cross_party` from `#[cfg(test)]` to `pub(crate)` at file scope

The existing definition at `src/auth_tensor_pre.rs:134-152` lives inside the `#[cfg(test)] mod tests` block. Since `two_to_one_combine` (non-test code) needs to call it, move it to file scope and mark it `pub(crate)`:

```rust
// Copy verbatim from src/auth_tensor_pre.rs:134-152, change to pub(crate),
// remove from inside #[cfg(test)] mod tests.
pub(crate) fn verify_cross_party(
    gen_share: &AuthBitShare,
    eval_share: &AuthBitShare,
    delta_a: &Delta,
    delta_b: &Delta,
) {
    AuthBitShare { key: eval_share.key, mac: gen_share.mac, value: gen_share.value }
        .verify(delta_b);
    AuthBitShare { key: gen_share.key, mac: eval_share.mac, value: eval_share.value }
        .verify(delta_a);
}
```

The duplicate at `src/leaky_tensor_pre.rs:344-362` can stay (it lives in `#[cfg(test)] mod tests` and is consumed only by Phase 4 tests; no reason to perturb it in Phase 5).

#### Test patterns — extend existing `mod tests` at `src/auth_tensor_pre.rs:110`

Keep the three existing tests (`test_bucket_size_formula`, `test_combine_dimensions`, `test_full_pipeline_no_panic`) but update their signatures/expected values for the new API.

**Update `test_bucket_size_formula` (lines 154-159):**

```rust
// BEFORE:
assert_eq!(bucket_size_for(16, 16), 6);
assert_eq!(bucket_size_for(128, 128), 3);
assert_eq!(bucket_size_for(4, 4), 11);

// AFTER (new signature + recomputed expected values per D-09):
assert_eq!(bucket_size_for(2), 41);    // log2(2) = 1
assert_eq!(bucket_size_for(16), 11);   // log2(16) = 4
assert_eq!(bucket_size_for(128), 6);   // log2(128) = 7
assert_eq!(bucket_size_for(1024), 5);  // log2(1024) = 10
```

**New `test_bucket_size_formula_edge_cases`:**

```rust
#[test]
fn test_bucket_size_formula_edge_cases() {
    assert_eq!(bucket_size_for(0), 40);  // SSP fallback
    assert_eq!(bucket_size_for(1), 40);  // SSP fallback
}
```

**Update `test_full_pipeline_no_panic` (line 177):** change `bucket_size_for(n, m)` to `bucket_size_for(1)` (single output triple). Expected B = 40, so `make_triples(n, m, 40)`. [Note: after the Phase 5 rewrite produces correct triples, the downstream `AuthTensorGen::new_from_fpre_gen` / `AuthTensorEval::new_from_fpre_eval` calls should still succeed — those constructors don't validate `alpha_labels`/`beta_labels` emptiness per Phase 4 D-07.]

**New `test_two_to_one_combine_product_invariant` (TEST-05 happy path):** verify the product invariant `z_full[j*n+i] == x_full[i] AND y_full[j]` after one two-to-one combine. Template at RESEARCH.md Example 4 lines 511-557. Copies Pattern S7 (`make_triples(n, m, 2)`), Pattern S2 (`verify_cross_party` on all combined shares), and the product-loop idiom from `src/leaky_tensor_pre.rs:549-562` (`test_leaky_triple_product_invariant`).

**New `test_two_to_one_combine_tampered_d_panics` (TEST-05 tamper path):**

```rust
#[test]
#[should_panic(expected = "MAC mismatch in share")]
fn test_two_to_one_combine_tampered_d_panics() {
    let n = 2;
    let m = 2;
    let triples = make_triples(n, m, 2);
    let t0 = triples[0].clone();
    let mut t1 = triples[1].clone();

    // Tamper: flip the value of eval_y_shares[0] without touching the MAC.
    // The assembled d[0] share now fails verify_cross_party.
    t1.eval_y_shares[0].value = !t1.eval_y_shares[0].value;

    let _ = two_to_one_combine(t0, &t1);  // must panic
}
```

The `#[should_panic(expected = "...")]` pattern is verified identical to `src/leaky_tensor_pre.rs:628` and referenced as Pattern S6. The substring `"MAC mismatch in share"` comes from `src/sharing.rs:62` (Pattern S2 panic contract).

**Optional new `test_combine_full_bucket_product_invariant`:** exercises the iterative fold at `bucket_size = B` (e.g., B = 40 or a smaller nonzero multiple of two). Same product-loop body as the two-triple test but over the full bucket. Catches regressions in `combine_leaky_triples`'s fold wrapper.

---

### `src/preprocessing.rs` — MODIFIED (call site update)

**Role:** caller — the only non-test site of `bucket_size_for` outside the module itself.

**Data flow:** config / wiring.

**Primary analog:** self (`src/preprocessing.rs:87`).

#### Existing call (`src/preprocessing.rs:85-87`):

```rust
assert_eq!(count, 1, "Phase 1: only count=1 is supported; batch output requires Vec return");

let bucket_size = bucket_size_for(n, m);
let total_leaky = bucket_size * count;
```

#### Pattern to apply — one-line swap (per D-10)

```rust
let bucket_size = bucket_size_for(count);
let total_leaky = bucket_size * count;
```

With `count = 1`, `bucket_size_for(1)` returns `SSP = 40`, so `total_leaky = 40`. The loop at `src/preprocessing.rs:96-101` then generates 40 leaky triples, which `combine_leaky_triples` folds via 40 invocations of `two_to_one_combine` (Pattern 3 of RESEARCH.md).

**Side effects:** this is the only behavioral change required in `preprocessing.rs`. No test body needs updating — the four tests in `src/preprocessing.rs:106-134` call `run_preprocessing(4, 4, 1, 1)` and assert only on output dimensions / delta LSB / `AuthTensorGen` construction success. None of those depend on the specific bucket size value.

**Phase 5 verification side effect:** once `combine_leaky_triples` correctly implements the paper algebra, the existing `test_run_preprocessing_feeds_online_phase` (lines 128-133) may exercise more correctness (no silent Z corruption). No direct Z assertion in this test file, though — TEST-05 lives in `auth_tensor_pre.rs`.

---

## No Analog Found

None. Every Phase 5 change reuses existing crate primitives:

- The combining fold wrapper: Pattern 3 of RESEARCH.md (`src/auth_tensor_pre.rs:71-80` existing loop shape, plus the Rust-standard first-element-then-fold idiom).
- The `two_to_one_combine` helper: direct composition of Patterns S1, S2, S3, S4, S5.
- `bucket_size_for` arithmetic: unchanged formula skeleton, only the `ell` parameter semantics + edge-case guard are new.
- TEST-05 structure: copied from the Phase 4 product-invariant and `#[should_panic]` templates (Patterns S6, S7, S8).

The only "novel" element is the paper's `Z = Z' ⊕ Z'' ⊕ x'' ⊗ d` algebra itself, which is three XORs plus one conditional — nothing algorithmic to build from scratch.

---

## Cross-Codebase Call-Site Audit

**`bucket_size_for` callers (verified by inspection):**

1. `src/preprocessing.rs:87` — `let bucket_size = bucket_size_for(n, m);` → change to `bucket_size_for(count)` per D-10.
2. `src/auth_tensor_pre.rs:156-158` — test `test_bucket_size_formula` — update assertions to the new signature values.
3. `src/auth_tensor_pre.rs:177` — test `test_full_pipeline_no_panic` — calls `bucket_size_for(n, m)`; update to `bucket_size_for(1)`.

No other callers. The rename is a compile-time break for any forgotten caller — impossible to silently drift.

---

## Shared Pattern Index

| Pattern | Source | Apply To |
|---------|--------|----------|
| S1 — `AuthBitShare + AuthBitShare` XOR | `src/sharing.rs:66-77` | two_to_one_combine (d, x, Z) |
| S2 — `verify_cross_party` | `src/auth_tensor_pre.rs:134-152` | two_to_one_combine Step B, TEST-05 happy path |
| S3 — `AuthBitShare::default()` zero share | `src/sharing.rs:42` | two_to_one_combine Step D (`d[j] == 0`) |
| S4 — Column-major `j/i/k = j*n+i` loop | `src/leaky_tensor_pre.rs:240-247, 256-261, 269-278` | two_to_one_combine Step D |
| S5 — Same-delta assertion | `src/auth_tensor_pre.rs:53-68` | combine_leaky_triples (retain), two_to_one_combine (weak re-assertion) |
| S6 — `#[should_panic(expected = ...)]` | `src/leaky_tensor_pre.rs:627-646` | TEST-05 tamper-path test |
| S7 — `make_triples` fixture | `src/auth_tensor_pre.rs:120-129` | all TEST-05 tests |
| S8 — `IdealBCot::new(42, 99)` seeds | `src/auth_tensor_pre.rs:122` and 7 more | all TEST-05 tests |
| S9 — `use crate::{...}` import grouping | `src/auth_tensor_pre.rs:1-4`, CONVENTIONS.md:88-103 | file-level imports in auth_tensor_pre.rs |
| S10 — Panic-not-Result | CONVENTIONS.md:105-111 | every precondition / invariant check |

---

## Metadata

**Analog search scope:**
- `src/auth_tensor_pre.rs` (184 lines, read in full) — the file under rewrite + existing `make_triples` and `verify_cross_party` helpers
- `src/sharing.rs` (209 lines, read in full) — `AuthBitShare`, `Add` overloads, `verify`, `Default` derive
- `src/leaky_tensor_pre.rs` (648 lines, read in full) — `LeakyTriple` struct, cross-party layout doc, Z-assembly precedent, extensive test module
- `src/preprocessing.rs` (135 lines, read in full) — call-site consumer
- `src/delta.rs` (158 lines, read in full) — `Delta` type used in `verify_cross_party`
- `.planning/phases/04-m2-pi-leakytensor-f-eq-construction-2/04-PATTERNS.md` — prior-phase pattern style + cross-party layout (Pattern 1, 2)
- `.planning/codebase/CONVENTIONS.md` (partial, around lines 88-111) — import grouping and panic-not-Result convention

**Files scanned (targeted):**
- `src/auth_tensor_pre.rs:15-21` (bucket_size_for), `:38-107` (combine_leaky_triples), `:120-152` (test helpers), `:154-183` (existing tests)
- `src/sharing.rs:40-63` (AuthBitShare + Default + verify), `:66-117` (four Add overloads)
- `src/leaky_tensor_pre.rs:29-52` (LeakyTriple + cross-party layout doc), `:240-314` (column-major + Z assembly), `:344-362` (verify_cross_party duplicate), `:499-646` (TEST-02/03/04 test shapes incl. should_panic)
- `src/preprocessing.rs:79-104` (run_preprocessing body), `:106-134` (tests)
- `.planning/phases/05-.../05-CONTEXT.md` (D-01 through D-12) + `05-RESEARCH.md` (Patterns 1-4, Pitfalls 1-6, Example 4)

**Pattern extraction date:** 2026-04-22

---

## Ready for Planning

Phase 5 pattern mapping complete. Planner can now assemble plans by referring to:

- **Pattern Assignments** for the two files under edit (one modified, one touched call-site).
- **Shared Pattern Index** (S1–S10) for cross-cutting idioms that appear in multiple plan actions.
- **Cross-Codebase Call-Site Audit** for the bucket_size_for signature break fanout (three sites).

Every new line of code in Phase 5 has a direct analog in the existing crate, cited with file path and line numbers. The planner should cite these references in plan action bullets to keep implementers aligned with codebase conventions.
