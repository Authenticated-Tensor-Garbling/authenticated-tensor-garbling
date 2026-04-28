# Phase 3: M2 Generalized Tensor Macro (Construction 1) - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 6 (1 new, 5 modified)
**Analogs found:** 6 / 6 (all exact matches — phase is composition of existing kernels)

---

## File Classification

| New/Modified File | Status | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|--------|------|-----------|----------------|---------------|
| `src/tensor_macro.rs` | NEW | module (primitive: garbler + evaluator pair) | transform (in-memory, deterministic) | `src/tensor_gen.rs` (garbler) + `src/tensor_eval.rs` (evaluator) | role-match (standalone module vs. struct-bound; combines both halves) |
| `src/tensor_ops.rs` | MODIFIED | utility / kernel (GGM tree primitives) | transform | self (existing `gen_populate_seeds_mem_optimized`, `gen_unary_outer_product`) | exact (signature-only edits + added eval functions) |
| `src/tensor_gen.rs` | MODIFIED | caller (semi-honest garbler) | transform | self | exact (one-line call-site update at line 82) |
| `src/tensor_eval.rs` | MODIFIED | caller (semi-honest evaluator) | transform | self | exact (delete private methods, call hoisted free functions) |
| `src/auth_tensor_eval.rs` | MODIFIED | caller (authenticated evaluator) | transform | self | exact (same as `tensor_eval.rs`) |
| `src/matrix.rs` | MODIFIED (optional) | type (`BlockMatrix`) | data-carrier | self | exact (add `pub(crate) fn elements_slice(&self) -> &[T]`) |
| `src/lib.rs` | MODIFIED | module-graph root | config | self (existing `pub mod` declarations) | exact (one added line) |

---

## Pattern Assignments

### `src/tensor_macro.rs` — NEW (module; primitive transform)

**Primary analog for garbler side:** `src/tensor_ops.rs::gen_populate_seeds_mem_optimized` + `src/tensor_ops.rs::gen_unary_outer_product` (existing) as called from `src/tensor_gen.rs:82-83`.

**Primary analog for evaluator side:** `src/tensor_eval.rs::eval_populate_seeds_mem_optimized` (lines 61-130) + `src/tensor_eval.rs::eval_unary_outer_product` (lines 132-172) as called from `src/tensor_eval.rs:206-207`.

**Secondary analog for struct / ciphertext packaging:** `src/bcot.rs::BcotOutput` (lines 31-41).

**Secondary analog for inline-test module:** `src/leaky_tensor_pre.rs::mod tests` (lines 252-end).

#### Imports pattern — copy from `src/tensor_gen.rs:1-13`

```rust
use crate::{
    aes::{
        FixedKeyAes,
        FIXED_KEY_AES
    },
    block::Block,
    delta::Delta,
    matrix::{
        BlockMatrix,
        MatrixViewRef},
    tensor_pre::SemiHonestTensorPreGen,
    tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product},
};
```

For `tensor_macro.rs`, adapt: drop `tensor_pre` / `MatrixViewRef`, add `keys::Key`, `macs::Mac`, and (once hoisted) `tensor_ops::{eval_populate_seeds_mem_optimized, eval_unary_outer_product}`.

#### Ciphertext struct pattern — copy from `src/bcot.rs:31-41`

```rust
pub struct BcotOutput {
    /// Sender's view: holds the K[0] key for each position. LSB is always 0.
    pub sender_keys: Vec<Key>,
    /// Receiver's view: holds K[choice[i]] for each position.
    /// NOTE: This is a Mac value that may have LSB=1. [...]
    pub receiver_macs: Vec<Mac>,
    /// The choice bits held by the receiver.
    pub choices: Vec<bool>,
}
```

**Apply to `TensorMacroCiphertexts`**: `pub(crate) struct` with named `pub` fields (per D-06). Document field roles ("maps to paper `G_{i,0}/G_{i,1}`", "maps to paper `G_k`") as `bcot.rs` does for sender_keys / receiver_macs. No methods — plain data carrier.

#### Garbler orchestration pattern — copy from `src/tensor_gen.rs:80-87`

```rust
out.with_subrows(self.chunking_factor * s, slice_size, |part| {

    let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(&slice.as_view(), cipher, delta);
    let gen_cts = gen_unary_outer_product(&gen_seeds, &y, part, cipher);

    chunk_levels.push(levels);
    chunk_cts.push(gen_cts);
});
```

**Apply to `tensor_garbler`**: two-step composition `(seeds, levels) = gen_populate_seeds_mem_optimized(...)` then `leaf_cts = gen_unary_outer_product(...)`. Drop the chunking/subrows wrapper (Phase 3 is unchunked — one GGM tree of depth n). After the D-03 signature change, pass `Key::as_blocks(a_keys)` as first arg instead of `&slice.as_view()`.

#### Evaluator orchestration pattern — copy from `src/tensor_eval.rs:204-213`

```rust
out.with_subrows(chunking_factor * s, slice_size, |part| {

    let eval_seeds = Self::eval_populate_seeds_mem_optimized(&slice.as_view(), chunk_levels[s].clone(), &slice_clear, cipher);
    let _eval_cts = Self::eval_unary_outer_product(&eval_seeds, &y, part, cipher, slice_clear, &chunk_cts[s]);

});
```

**Apply to `tensor_evaluator`**: same two-step composition, dropping the chunking wrapper. After hoisting (Open Question Q2), call free functions `tensor_ops::eval_populate_seeds_mem_optimized` / `tensor_ops::eval_unary_outer_product` (no `Self::`). Pass `Mac::as_blocks(a_macs)` in place of `&slice.as_view()`.

#### Precondition-assertion pattern — copy from `src/tensor_ops.rs` style + `.planning/codebase/CONVENTIONS.md`

The codebase's convention for dimension validation is `assert!` / `assert_eq!` with explanatory messages (not `Result`). Representative example from `src/bcot.rs:241-244`:

```rust
assert_eq!(out_a.sender_keys.len(), 256, "Test 6a: expected 256 sender keys from transfer_a_to_b");
```

**Apply to `tensor_garbler`/`tensor_evaluator`**: at top of function body, insert:

```rust
assert_eq!(a_keys.len(), n, "a_keys length must equal n");
assert_eq!(t_gen.rows(), m, "t_gen must be a length-m column vector");
assert_eq!(t_gen.cols(), 1, "t_gen must be a column vector (cols == 1)");
// evaluator also checks:
assert_eq!(g.level_cts.len(), n.saturating_sub(1), "G must have n-1 level ciphertexts");
assert_eq!(g.leaf_cts.len(), m, "G must have m leaf ciphertexts");
```

(See Pitfall 6 / Assumption A1 in RESEARCH.md — T is m×1 column, not n×m.)

#### Inline test module pattern — copy from `src/leaky_tensor_pre.rs:252-256` + `src/bcot.rs:129-131`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcot::IdealBCot;

    fn make_bcot() -> IdealBCot {
        IdealBCot::new(42, 99)
    }
    ...
```

**Apply to `tensor_macro.rs` tests**: inline `#[cfg(test)] mod tests` at bottom of file (codebase convention per D-14). Use `IdealBCot::transfer_a_to_b` as the oracle. Seed all RNGs via `ChaCha12Rng::seed_from_u64` for determinism (`leaky_tensor_pre.rs:56`, `bcot.rs:49`). One `#[test]` per `(n, m)` tuple covering edge cases from D-13.

#### Test invariant pattern — copy from `src/lib.rs::verify_tensor_output` (lines 103-127)

```rust
fn verify_tensor_output(
    clear_x: usize, clear_y: usize, n: usize, m: usize,
    gb_out: &BlockMatrix, ev_out: &BlockMatrix, delta: &Delta,
) -> bool {
    for i in 0..n {
        for k in 0..m {
            let expected_val = (((clear_x>>i)&1) & ((clear_y>>k)&1)) != 0;
            if expected_val {
                if gb_out[(i, k)] != ev_out[(i, k)] ^ delta.as_block() { return false; }
            } else {
                if gb_out[(i, k)] != ev_out[(i, k)] { return false; }
            }
        }
    }
    true
}
```

**Apply to `tensor_macro` tests**: build an analogous `verify_paper_invariant` helper that asserts `Z_gen XOR Z_eval == a ⊗ T` — for each `(i, k)`, the XOR of `z_gen[(i,k)]` and `z_eval[(i,k)]` equals `T[k]` if `a[i]` else `Block::ZERO`. Note Phase 3's invariant is pure XOR (not delta-adjusted like semi-honest) — the macro emits the XOR share of `a ⊗ T` directly.

---

### `src/tensor_ops.rs` — MODIFIED (kernel)

**Analog:** self (this is the file being edited).

#### Signature-change pattern (D-03) — edit existing function at lines 9-13

```rust
// BEFORE (src/tensor_ops.rs:9-13):
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {

// AFTER:
pub(crate) fn gen_populate_seeds_mem_optimized(
    x: &[Block],
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
```

#### Body-edit pattern — line 82 of `src/tensor_ops.rs`

```rust
// BEFORE:
let seeds = tree[tree.len() - (1 << x.rows())..tree.len()].to_vec();

// AFTER:
let seeds = tree[tree.len() - (1 << n)..tree.len()].to_vec();
```

Rationale: `MatrixViewRef<Block>.rows()` becomes unavailable on `&[Block]`; use the already-computed local `n = x.len()` (line 17, unchanged).

#### Hoisted evaluator function pattern (Q2 resolution → hoist) — copy body verbatim from `src/tensor_eval.rs:61-130`

Source body is 70 lines; hoist as free `pub(crate)` function. Two signature changes versus the existing private method:

1. First param: `x: &MatrixViewRef<Block>` → `x: &[Block]` (matches D-03 symmetry; body changes required: line 69 `n = x.len()` already slice-compatible; line 128 `x.len()` also slice-compatible).
2. Drop the `_clear_value: &usize` third param (it is unused in current body — line 64 uses `_` prefix).
3. Return type: `Vec<Block>` → `(Vec<Block>, usize)` to emit `missing` (the value reconstructed at lines 78, 108).

```rust
pub(crate) fn eval_populate_seeds_mem_optimized(
    x: &[Block],
    levels: Vec<(Block, Block)>,
    cipher: &FixedKeyAes,
) -> (Vec<Block>, usize) {
    // body copied verbatim from src/tensor_eval.rs:67-128
    // final line:
    (final_seeds, missing)
}
```

#### Hoisted leaf-expansion function pattern — copy body verbatim from `src/tensor_eval.rs:132-172`

```rust
pub(crate) fn eval_unary_outer_product(
    seeds: &[Block],                      // was &Vec<Block>; accept slice for flexibility
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[Block],                    // was &Vec<Block>
) -> Vec<Block> {
    // body copied verbatim from src/tensor_eval.rs:140-171
}
```

`seeds.len()` and indexing still work on `&[Block]`. `.len()` on `&[Block]` is identical.

---

### `src/tensor_gen.rs` — MODIFIED (one call-site update)

**Analog:** self.

#### Call-site update pattern (D-04) — edit line 82

```rust
// BEFORE (src/tensor_gen.rs:82):
let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(&slice.as_view(), cipher, delta);

// AFTER (Option A — requires adding elements_slice() on TypedMatrix):
let (gen_seeds, levels) = gen_populate_seeds_mem_optimized(slice.elements_slice(), cipher, delta);
```

`slice` at this call site is `BlockMatrix::new(slice_size, 1)` (constructed at line 65) — a column vector. Because column-major storage of an m×1 matrix IS a length-m slice in row order, `elements_slice()` returns exactly the length-`slice_size` slice the kernel expects.

---

### `src/tensor_eval.rs` — MODIFIED (delete private methods, call free functions)

**Analog:** self + the hoisted destination.

#### Delete-and-delegate pattern — remove lines 61-172 (two private methods), update `eval_chunked_half_outer_product` at lines 204-213

```rust
// BEFORE (src/tensor_eval.rs:204-213):
out.with_subrows(chunking_factor * s, slice_size, |part| {
    let eval_seeds = Self::eval_populate_seeds_mem_optimized(&slice.as_view(), chunk_levels[s].clone(), &slice_clear, cipher);
    let _eval_cts = Self::eval_unary_outer_product(&eval_seeds, &y, part, cipher, slice_clear, &chunk_cts[s]);
});

// AFTER:
out.with_subrows(chunking_factor * s, slice_size, |part| {
    let (eval_seeds, _missing_derived) = crate::tensor_ops::eval_populate_seeds_mem_optimized(
        slice.elements_slice(), chunk_levels[s].clone(), cipher,
    );
    // Pre-existing caller computed slice_clear via BlockMatrix::get_clear_value (line 194)
    // which equals the internally-derived missing. Use _missing_derived for consistency.
    let _eval_cts = crate::tensor_ops::eval_unary_outer_product(
        &eval_seeds, &y, part, cipher, slice_clear, &chunk_cts[s],
    );
});
```

Also: remove the `use crate::matrix::MatrixViewRef` if the removed private methods are the only consumers (verify before pruning the import).

---

### `src/auth_tensor_eval.rs` — MODIFIED (same hoist application as `tensor_eval.rs`)

**Analog:** `src/tensor_eval.rs` (post-modification).

Source private methods at `auth_tensor_eval.rs:63-135` and `auth_tensor_eval.rs:137-177` are byte-for-byte identical to the `tensor_eval.rs` equivalents. Apply the exact same delete-and-delegate pattern as in `tensor_eval.rs`. The `eval_chunked_half_outer_product` call-site on line 210-211 uses the same `Self::` qualifier and the same argument shape — one edit pattern covers both files.

---

### `src/matrix.rs` — MODIFIED (add `elements_slice` helper; Q3 resolution)

**Analog:** self. Existing helpers `rows()` / `cols()` / `as_view()` at lines 70-84.

#### New accessor pattern — copy from `src/matrix.rs:70-76`

```rust
// EXISTING at lines 70-76:
pub fn rows(&self) -> usize {
    self.rows
}

pub fn cols(&self) -> usize {
    self.cols
}

// ADD (same position / style):
pub(crate) fn elements_slice(&self) -> &[T] {
    &self.elements
}
```

Justification per Q3: narrow surface, `pub(crate)`, doesn't require any new trait bounds. Callers that want a length-`rows() * cols()` view into the column-major storage get `O(1)` access with no conversion.

---

### `src/lib.rs` — MODIFIED (module registration)

**Analog:** self, lines 11-14.

#### Module declaration pattern — copy from `src/lib.rs:11-14`

```rust
// EXISTING:
pub mod tensor_pre;
pub mod tensor_gen;
pub mod tensor_eval;
pub mod tensor_ops;

// ADD (alphabetical-by-group, same position as siblings):
pub mod tensor_macro;
```

No other edits. `tensor_macro` has no dependents inside the crate until Phase 4.

---

## Shared Patterns

### Cipher acquisition (FIXED_KEY_AES singleton)

**Source:** `src/tensor_gen.rs:36`, `src/tensor_eval.rs:33, 48`.

**Apply to:** `tensor_macro.rs` — both `tensor_garbler` and `tensor_evaluator`.

```rust
// src/tensor_gen.rs:36 (inside struct init):
cipher: &FIXED_KEY_AES,

// src/tensor_eval.rs:33:
cipher: &(*FIXED_KEY_AES),
```

**For free functions (no struct):** bind locally at top of each function body:

```rust
let cipher: &FixedKeyAes = &FIXED_KEY_AES;
```

This matches how all existing callers acquire the process-wide singleton — do NOT construct a new `FixedKeyAes` per call (see "Don't Hand-Roll" in RESEARCH.md).

### Zero-cost slice reinterpret at API boundary

**Source:** `src/keys.rs:77-82`, `src/macs.rs:54-60`.

**Apply to:** inside `tensor_garbler` (one call) and `tensor_evaluator` (one call).

```rust
// src/keys.rs:77-82:
#[inline]
pub fn as_blocks(slice: &[Self]) -> &[Block] {
    // Safety:
    // Key is a newtype of block.
    unsafe { &*(slice as *const [Self] as *const [Block]) }
}
```

```rust
// Inside tensor_garbler:
let a_blocks: &[Block] = Key::as_blocks(a_keys);
// Inside tensor_evaluator:
let a_blocks: &[Block] = Mac::as_blocks(a_macs);
```

Zero-alloc, zero-copy — safe because `Key` and `Mac` are `#[repr(Rust)]` newtypes of `Block` with identical memory layout (documented in the Safety comments at `keys.rs:78-81` and `macs.rs:57-60`).

### Endianness convention (paper A_0 ⟷ code `x[n-1]`)

**Source:** `src/tensor_ops.rs:22-33` (verbatim comment), `src/tensor_eval.rs:72-78` (verbatim comment).

**Apply to:** `tensor_macro.rs` doc comments — document that callers pass `a_keys` / `a_macs` in the same little-endian convention used throughout the codebase. DO NOT reverse the slice inside the macro (see Pitfalls 3 & 6 in RESEARCH.md).

```rust
// Endianness note (little-endian vectors):
// We treat index 0 as LSB and index n-1 as MSB of x. The tree is built from the
// most significant position downward, so we look at x[n-1] first.
```

Reproduce the essence of this note in `tensor_macro.rs` function docs so Phase 4 callers are not surprised.

### Deterministic seeded RNG for tests

**Source:** `src/leaky_tensor_pre.rs:56` (production: `ChaCha12Rng::seed_from_u64`), `src/bcot.rs:49-53` (also `ChaCha12Rng::seed_from_u64`).

**Apply to:** `#[cfg(test)] mod tests` at bottom of `src/tensor_macro.rs`.

```rust
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;

fn run_one_case(n: usize, m: usize, seed: u64) {
    let mut bcot = IdealBCot::new(seed, seed ^ 0xDEAD_BEEF);
    let mut rng = ChaCha12Rng::seed_from_u64(seed);
    // ...
}
```

Match the style of `leaky_tensor_pre.rs::tests::test_alpha_beta_mac_invariants` (lines 301-321) for per-test setup.

### Test oracle via `IdealBCot::transfer_a_to_b`

**Source:** `src/bcot.rs:63-79`, used for test oracle in `src/leaky_tensor_pre.rs:83` (production call).

**Apply to:** `tensor_macro.rs::tests::run_one_case` per D-12.

```rust
let choices: Vec<bool> = (0..n).map(|_| rng.random_bool(0.5)).collect();
let cot = bcot.transfer_a_to_b(&choices);
let delta = bcot.delta_b;  // garbler's Δ in the macro's view
let a_keys = cot.sender_keys;     // Vec<Key>, LSB=0 invariant enforced by Key::new
let a_macs = cot.receiver_macs;   // Vec<Mac>, LSB == choices[i] by IT-MAC invariant
```

`IdealBCot` guarantees the IT-MAC invariant `mac = key XOR bit·delta` by construction (see `src/bcot.rs:68-69`), which is exactly the precondition `tensor_evaluator` assumes (see Pitfall 3 in RESEARCH.md).

### Dimension assertions with explanatory messages

**Source:** `src/bcot.rs:240-244`, `src/lib.rs:62-63` (test helpers).

**Apply to:** top of `tensor_garbler` / `tensor_evaluator` bodies.

```rust
assert_eq!(gb_share.len(), n);
assert_eq!(gb_share.len(), ev_share.len());
```

Use `assert_eq!(..., "message")` form — no `Result<_, _>` returns (codebase-wide convention, `.planning/codebase/CONVENTIONS.md`).

### GGM tweak convention (zero for even/left, one for odd/right)

**Source:** `src/tensor_ops.rs:61-62` (gen side), `src/tensor_eval.rs:98-99` (eval side), `src/auth_tensor_eval.rs:103-104`.

**Apply to:** DO NOT CHANGE. Phase 3 reuses the existing kernels; their tweak encoding is part of the cross-party protocol contract. Any deviation would silently break semi-honest & authenticated end-to-end tests.

```rust
// src/tensor_ops.rs:61-62 (gen) — authoritative:
seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);
seeds[j * 2] = cipher.tccr(Block::from(1 as u128), seeds[j]);

// Level-0 tweak convention — src/tensor_ops.rs:28-32:
seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
```

---

## Patterns NOT to Copy (Anti-Patterns from Existing Code)

These are patterns present in the codebase that Phase 3 should explicitly avoid — documented here so the planner can steer away.

| Anti-Pattern | Found In | Reason to Avoid in Phase 3 |
|--------------|----------|----------------------------|
| Private associated functions for kernel logic | `src/tensor_eval.rs:61-172` (pre-hoist), `src/auth_tensor_eval.rs:63-177` | Causes the duplication this phase eliminates. New code in `tensor_macro.rs` uses free `pub(crate)` functions. |
| Manual `Vec<Block>` inputs instead of typed `Key` / `Mac` | `gen_populate_seeds_mem_optimized` (pre-D-03) took `&MatrixViewRef<Block>` — loses Key/Mac newtype info at kernel boundary | At the `tensor_macro.rs` API boundary, use `&[Key]` / `&[Mac]`. Drop to `&[Block]` only via `Key::as_blocks` / `Mac::as_blocks` inside the body. |
| Chunking wrapper for GGM tree | `src/tensor_gen.rs:52-91`, `src/tensor_eval.rs:174-211`, `src/auth_tensor_eval.rs:179-215` | Phase 3 is unchunked (single tree of depth n). Chunking is a semi-honest / authenticated concern from Phases 1-2 and not part of Construction 1. |
| `chunk_levels.push(levels)` packaging | `src/tensor_gen.rs:85-86` | Phase 3's `TensorMacroCiphertexts` holds one set of `(level_cts, leaf_cts)` directly — no nesting. |
| `RngCore::rng()` / `rand::thread_rng()` usage inside protocol functions | — none found in protocol kernels; tests use `ChaCha12Rng::seed_from_u64` | Phase 3 macro is deterministic; any RNG call inside `tensor_garbler`/`tensor_evaluator` would be a correctness bug. |

---

## No Analog Found

All files have close or exact analogs. No new pattern research required.

(The primitive is an extraction from `tensor_gen.rs` + `tensor_eval.rs` with minor signature cleanup; every code artifact Phase 3 produces has a direct structural precedent.)

---

## Metadata

**Analog search scope:** `src/` (all .rs files, excluding `src/*.rs 2` macOS Finder duplicates)

**Files scanned:**
- `src/tensor_ops.rs` (122 lines — kernel reference)
- `src/tensor_gen.rs` (166 lines — garbler orchestration analog)
- `src/tensor_eval.rs` (274 lines — evaluator orchestration analog; hoist source)
- `src/auth_tensor_eval.rs` (target of hoist application)
- `src/bcot.rs` (257 lines — ciphertext struct analog + test oracle)
- `src/keys.rs` (267 lines — Key::as_blocks pattern)
- `src/macs.rs` (127 lines — Mac::as_blocks pattern)
- `src/aes.rs` (244 lines — cipher singleton pattern)
- `src/matrix.rs` (lines 1-150 — elements_slice accessor siting)
- `src/lib.rs` (379 lines — module declaration siting + `verify_tensor_output` test helper)
- `src/leaky_tensor_pre.rs` (lines 1-150, 252-400 — inline test module convention)

**Pattern extraction date:** 2026-04-21

**Phase:** 03-m2-generalized-tensor-macro-construction-1
