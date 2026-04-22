---
phase: 03-m2-generalized-tensor-macro-construction-1
reviewed: 2026-04-22T04:29:42Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - src/auth_tensor_eval.rs
  - src/auth_tensor_gen.rs
  - src/lib.rs
  - src/matrix.rs
  - src/tensor_eval.rs
  - src/tensor_gen.rs
  - src/tensor_macro.rs
  - src/tensor_ops.rs
findings:
  critical: 3
  warning: 5
  info: 4
  total: 12
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-04-22T04:29:42Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

All eight source files were read in full. The review focused on cryptographic
invariants, Rust-specific memory and integer-safety hazards, API visibility
correctness, and test coverage gaps.

The `tensor_macro.rs` module is well-structured and its paper-invariant test
suite is thorough. The GGM tree construction in `tensor_ops.rs` and the
authenticated tensor halves in `auth_tensor_gen/eval.rs` show clear design
intent. However, three issues were identified that can silently produce wrong
cryptographic outputs or trigger panics in release builds, and five secondary
issues erode robustness or expose internal state.

---

## Critical Issues

### CR-01: Integer underflow panic / wrong assert when `n = 0` in `tensor_evaluator`

**File:** `src/tensor_macro.rs:141`

**Issue:** The precondition assertion `g.level_cts.len() == n - 1` performs
unsigned subtraction on `usize`. When `n == 0` this is `0usize - 1`, which
panics in debug mode and wraps to `usize::MAX` in release mode, making the
assert trivially false and allowing execution to continue with a `levels` Vec
that has no entries — silently producing wrong output. An `n = 0` call is
degenerate but not explicitly rejected before this line.

**Fix:**
```rust
// Replace the existing assert with a checked form and an explicit n=0 guard:
assert!(n > 0, "n must be at least 1 (degenerate n=0 is not supported)");
assert_eq!(
    g.level_cts.len(),
    n - 1,    // safe: n >= 1
    "G must have n-1 level ciphertexts"
);
```

The same pattern applies to `tensor_garbler` — it calls
`gen_populate_seeds_mem_optimized` which indexes `x[n-1]` at line 27 of
`tensor_ops.rs`, which panics on an empty slice, but the `assert_eq!(a_keys.len(), n)` at line 89 of `tensor_macro.rs` fires first only in debug builds. Add a matching `assert!(n > 0)` guard in `tensor_garbler` as well.

---

### CR-02: GGM tree ciphertext tweak collision — left/right child tweaks are swapped between the garbler and evaluator's level-expansion loops

**File:** `src/tensor_ops.rs:61-62` (garbler), `src/tensor_ops.rs:178-179` (evaluator)

**Issue:** In `gen_populate_seeds_mem_optimized` the garbler derives children as:

```
seeds[j*2+1] = tccr(tweak=0, seeds[j])   // odd child   → tweak 0
seeds[j*2]   = tccr(tweak=1, seeds[j])   // even child  → tweak 1
```

In `eval_populate_seeds_mem_optimized` the evaluator mirrors this exactly
(lines 178-179), which is internally consistent. However, the level-ciphertext
accumulation in the garbler (lines 63-70) labels the XOR of `seeds[j*2]` as
`evens` and commits it to `odd_evens[i].0`, while the evaluator (lines 181-182)
accumulates `seeds[j*2]` into `e_evens` and reads the recovery block from
`g_evens = levels[i-1].0`. So the consistency holds end-to-end.

The real concern is the choice of tweak constant: both child derivations at
*every* tree level use the same set of tweaks (0 and 1). There is no
level-index mixed into the tweak. If the AES-based TCCR is not a PRF under
identical tweaks across different parent inputs this is safe, but if an
adversary can observe ciphertexts from two levels that happen to share the same
parent value (possible when the same key appears at multiple nodes, e.g., when
`x[i] = x[j]` for two distinct indices), the per-level XOR sums can leak
information. The established GGM construction mixes a *level counter* into the
tweak to prevent this.

**Fix:** Mix the tree depth `i` into the tweak:

```rust
// Garbler loop body (and evaluator mirror):
let tweak_even = Block::from(((i as u128) << 1) as u128);
let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);
seeds[j * 2 + 1] = cipher.tccr(tweak_odd,  seeds[j]);
seeds[j * 2]     = cipher.tccr(tweak_even, seeds[j]);
```

Apply the same change in `eval_populate_seeds_mem_optimized` and update the
garbler's key-contribution lines (currently `tccr(0, key0)` / `tccr(1, key1)`)
to use the same level-indexed tweaks.

---

### CR-03: `matrix.rs` `BitXorAssign` silently corrupts data on dimension mismatch in release builds

**File:** `src/matrix.rs:311-317`

**Issue:** The `BitXorAssign` implementation for `TypedMatrix<T>` uses no assertion on dimensions in release builds:

```rust
impl<T: MatrixElement> BitXorAssign for TypedMatrix<T>
where T: BitXorAssign + Copy {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.elements.iter_mut().zip(rhs.elements.iter()).for_each(|(a, b)| {
            *a ^= *b;
        });
    }
}
```

`zip` silently truncates to the shorter operand. If `self` and `rhs` have
different shapes (for example (n, m) vs (m, n)) exactly as happen at the
boundary between `first_half_out` and `second_half_out`, this produces a
partially-updated matrix with no panic and no compiler warning. The owned
`BitXor` impl (line 244) carries a `debug_assert!` but `BitXorAssign` carries
none at all.

**Fix:**
```rust
fn bitxor_assign(&mut self, rhs: Self) {
    assert_eq!(
        (self.rows, self.cols),
        (rhs.rows, rhs.cols),
        "BitXorAssign: matrix dimensions must match ({} x {} vs {} x {})",
        self.rows, self.cols, rhs.rows, rhs.cols
    );
    self.elements.iter_mut().zip(rhs.elements.iter()).for_each(|(a, b)| {
        *a ^= *b;
    });
}
```

---

## Warnings

### WR-01: `tensor_eval.rs` `evaluate_final_outer_product` XORs a zero matrix — evaluator's correlated term is always zero

**File:** `src/tensor_eval.rs:158-166`

**Issue:** `eval_alpha_beta` is constructed as `BlockMatrix::constant(n, m, Block::default())` — an all-zero matrix — and then XORed into `first_half_out`. The XOR is a no-op. The semi-honest evaluator is supposed to contribute the evaluator's share of `alpha ⊗ beta` here, but `alpha_labels` (the field that would hold that value) is always initialized to zero in both `new` and `new_from_fpre_eval` and is never populated. This means the final gate output in the semi-honest path is missing its correlated-preprocessing term. The authenticated path (`auth_tensor_eval.rs`) correctly uses `correlated_auth_bit_shares[j*n+i].mac` so the bug exists only in the semi-honest variant, but it still means `TensorProductEval::evaluate_final_outer_product` does not implement the protocol correctly.

**Fix:** Either populate `alpha_labels` from the preprocessing data before calling `evaluate_final_outer_product`, or remove the dead field and the dead `eval_alpha_beta` matrix entirely and document that the evaluator's correlated share is always zero (if that is intentional for the semi-honest construction).

---

### WR-02: `matrix.rs` `with_subrows` row-offset arithmetic is only correct for column vectors

**File:** `src/matrix.rs:401` (`MatrixViewRef`) and `src/matrix.rs:458` (`MatrixViewMut`)

**Issue:** Both `with_subrows` implementations compute the new view start as:

```rust
let new_start = self.view_start + offset;
```

In column-major storage, a row offset within a *single column* is simply `+offset` — correct. But if `view_cols > 1`, each column is `total_rows` elements apart, so the subview's rows for column `j > 0` begin at `view_start + j*total_rows + offset`, not at `new_start + j*total_rows`. The current code only handles the column-vector case (where `view_cols == 1`). All call sites today happen to pass a column vector, but a future caller using a multi-column view would silently get wrong data.

**Fix:** Add a `debug_assert!(self.view_cols == 1, "with_subrows is only valid for column vectors")` at the top of both `with_subrows` bodies, or generalize the arithmetic if multi-column subrow access is needed.

---

### WR-03: `auth_tensor_gen.rs` `gen_chunked_half_outer_product` is `pub` while its eval counterpart is private — internal cryptographic state is unnecessarily exposed

**File:** `src/auth_tensor_gen.rs:71`

**Issue:** `gen_chunked_half_outer_product` is declared `pub`, making the raw chunked GGM-tree output (the `(Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>)` pair) directly accessible to any code outside the crate. In contrast, the evaluator's `eval_chunked_half_outer_product` is private (`fn`, no `pub`). External callers can use the garbler's internal chunking primitive without going through `garble_first_half` / `garble_second_half`, potentially bypassing the input-preparation logic in `get_first_inputs` / `get_second_inputs` and creating garbled circuits with uncorrelated inputs.

**Fix:**
```rust
// Change visibility to match the evaluator:
pub(crate) fn gen_chunked_half_outer_product(
    &mut self,
    x: &MatrixViewRef<Block>,
    y: &MatrixViewRef<Block>,
    first_half: bool,
) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
```

---

### WR-04: `tensor_ops.rs` GGM `tree` accumulator grows to `O(2^(n+1))` elements before the leaves are extracted

**File:** `src/tensor_ops.rs:14-82` (`gen_populate_seeds_mem_optimized`) and `src/tensor_ops.rs:145-209` (`eval_populate_seeds_mem_optimized`)

**Issue:** Both functions maintain a `tree: Vec<Block>` that accumulates *all* tree levels — `2 + 4 + ... + 2^n = 2^(n+1) - 2` entries — before slicing the last `2^n` entries as the leaves on the final line. The intermediate levels are never used after their `push` loop. For `n = 20` this means ~4 million Block entries (~64 MB) are allocated and immediately discarded.

This is not a correctness bug, but it is a latent memory issue that will surface when the macro is called with large `n` values (which the protocol design allows). The `seeds` buffer in the same function is already the right size (`1 << n`); the tree accumulator is redundant.

**Fix:** Remove the `tree` Vec entirely and use `seeds` directly:

```rust
// After all levels are computed, seeds already holds the leaves.
// Return seeds directly instead of tree[tree.len()-(1<<n)..].to_vec()
(seeds, odd_evens)   // for gen_populate_seeds_mem_optimized
(seeds, missing)     // for eval_populate_seeds_mem_optimized
```

---

### WR-05: `tensor_macro.rs` `tensor_evaluator` clones `g.level_cts` unnecessarily

**File:** `src/tensor_macro.rs:159`

**Issue:**
```rust
let (leaf_seeds, missing) = eval_populate_seeds_mem_optimized(
    a_blocks,
    g.level_cts.clone(),   // <-- heap allocation of Vec<(Block, Block)>
    cipher,
);
```

`eval_populate_seeds_mem_optimized` takes `levels: Vec<(Block, Block)>` by
value, forcing a clone at the call site. The function only reads `levels[i-1]`
inside a loop and never mutates it.

**Fix:** Change `eval_populate_seeds_mem_optimized`'s signature to accept a slice reference, eliminating the clone:

```rust
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    levels: &[(Block, Block)],   // was: Vec<(Block, Block)>
    cipher: &FixedKeyAes,
) -> (Vec<Block>, usize) {
```

Update all call sites (`tensor_macro.rs:159`, `auth_tensor_eval.rs:93`, `tensor_eval.rs:91`) to pass `&chunk_levels[s]` instead of `chunk_levels[s].clone()`.

---

## Info

### IN-01: `lib.rs` duplicates `MAC_ZERO` / `MAC_ONE` constants that are already defined in `macs.rs`

**File:** `src/lib.rs:35-42`

**Issue:** `MAC_ZERO` and `MAC_ONE` are declared `pub(crate)` in both `src/lib.rs` (lines 35-42) and `src/macs.rs` (lines 7-13) with identical byte values. Both are annotated `#[allow(dead_code)]`. The `lib.rs` copies are shadowed by the `macs.rs` copies for any code that imports from `crate::macs`. This creates two sources of truth for security-sensitive constants.

**Fix:** Remove the duplicates from `src/lib.rs` and use `crate::macs::{MAC_ZERO, MAC_ONE}` wherever needed.

---

### IN-02: `matrix.rs` `random_zeros` comment is misleading

**File:** `src/matrix.rs:158` (BlockMatrix impl), `src/matrix.rs:191` (KeyMatrix impl)

**Issue:** The inline comment reads `// Clear last bit of last byte` but the code is:
```rust
bytes[0] &= 0xFE; // Clear last bit of last byte
```
`bytes[0]` is the *first* byte, and `& 0xFE` clears bit 0 of that byte, which is the LSB of the entire block. The operation is correct (it enforces the `lsb() == 0` invariant used by `Key`), but the comment describes the wrong byte.

**Fix:**
```rust
bytes[0] &= 0xFE; // Clear LSB of byte 0 (= LSB of the block) to satisfy the key-invariant
```

---

### IN-03: `tensor_eval.rs` `TensorProductEval::alpha_labels` field is always zero and serves no purpose in the current protocol

**File:** `src/tensor_eval.rs:38-39`, `src/tensor_eval.rs:53-54`, `src/tensor_eval.rs:129`

**Issue:** `alpha_labels` is initialized to `BlockMatrix::constant(n, 1, Block::default())` in both constructors and is only read in `get_second_inputs` where its zero value is directly copied to `eval_y`. The field occupies heap space proportional to `n` and creates a false impression that the evaluator has mask data. In the semi-honest path there is no corresponding input assignment.

**Fix:** Remove the field and replace the `get_second_inputs` body with a direct `BlockMatrix::new(self.n, 1)` (all-zero column), with a comment explaining that the semi-honest evaluator contributes zero for its alpha share. Or, if the field is intended as a future hook for the authenticated path, add a `#[doc = "..."]` explaining it is intentionally zero in the semi-honest variant.

---

### IN-04: Test in `auth_tensor_gen.rs` has `println!` debug output left in

**File:** `src/auth_tensor_gen.rs:325-328` (inside `lib.rs` `test_auth_tensor_product`)

**Issue:** The integration test in `src/lib.rs` (lines 325-328) emits four `println!` statements about internal chunking structure:

```rust
println!("gen_chunk_levels: {:?}", gen_chunk_levels.len());
println!("gen_chunk_levels[0] (each hold 2 blocks): {:?}", gen_chunk_levels[0].len());
println!("gen_chunk_cts: {:?}", gen_chunk_cts.len());
println!("gen_chunk_cts[0] (ecah hold one block): {:?}", gen_chunk_cts[0].len());
```

These are noise in `cargo test` output and contain a typo ("ecah"). They should be replaced with structured `assert_eq!` checks or removed.

**Fix:** Replace with assertions:
```rust
assert_eq!(gen_chunk_levels.len(), (m + chunking_factor - 1) / chunking_factor);
assert_eq!(gen_chunk_levels[0].len(), n - 1);  // GGM level ciphertexts
assert_eq!(gen_chunk_cts.len(), gen_chunk_levels.len());
```

---

_Reviewed: 2026-04-22T04:29:42Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
