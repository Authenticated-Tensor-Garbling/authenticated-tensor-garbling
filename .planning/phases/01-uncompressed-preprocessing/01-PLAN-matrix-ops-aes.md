---
phase: 01-uncompressed-preprocessing
plan: 01-PLAN-matrix-ops-aes
type: execute
wave: 1
depends_on: []
files_modified:
  - src/matrix.rs
  - src/tensor_ops.rs
  - src/aes.rs
autonomous: true
requirements:
  - CLEAN-05
  - CLEAN-06
tags:
  - refactor
  - docs
  - visibility
user_setup: []

must_haves:
  truths:
    - "Neither `gen_populate_seeds_mem_optimized` nor `gen_unary_outer_product` in tensor_ops.rs is reachable as `pub` from outside the crate"
    - "Neither `MatrixViewRef` nor `MatrixViewMut` in matrix.rs is reachable as `pub` from outside the crate"
    - "TypedMatrix (or its `flat_index` method) has a doc comment that states index = j*rows + i with i=row and j=column"
    - "FIXED_KEY_AES in aes.rs has a doc comment that explains why `once_cell::sync::Lazy` is used and that the fixed key is a protocol constant (not a secret)"
    - "Every existing in-crate caller of the functions/types whose visibility was tightened still compiles"
    - "cargo build and cargo test pass after the change"
  artifacts:
    - path: "src/tensor_ops.rs"
      provides: "pub(crate) gen_populate_seeds_mem_optimized and gen_unary_outer_product"
      contains: "pub(crate) fn gen_populate_seeds_mem_optimized"
    - path: "src/matrix.rs"
      provides: "pub(crate) MatrixViewRef and MatrixViewMut with column-major doc"
      contains: "pub(crate) struct MatrixViewRef"
    - path: "src/aes.rs"
      provides: "FIXED_KEY_AES with thread-safety/protocol-constant doc comment"
      contains: "once_cell"
  key_links:
    - from: "src/auth_tensor_gen.rs and src/tensor_gen.rs"
      to: "src/tensor_ops.rs (gen_populate_seeds_mem_optimized, gen_unary_outer_product)"
      via: "in-crate use statement; pub(crate) must remain reachable"
      pattern: "use crate::tensor_ops::\\{gen_populate_seeds_mem_optimized"
    - from: "src/auth_tensor_eval.rs, src/tensor_eval.rs, src/auth_tensor_gen.rs, src/tensor_gen.rs, src/tensor_ops.rs"
      to: "src/matrix.rs (MatrixViewRef, MatrixViewMut)"
      via: "in-crate use statements; pub(crate) must remain reachable"
      pattern: "use crate::matrix::(Matrix|\\{.*MatrixView)"
---

<objective>
Tighten visibility of internal helpers to `pub(crate)`, document column-major indexing at the type level, and document the `FIXED_KEY_AES` Lazy singleton. Zero algorithmic changes.

Purpose: Separate the library's actual public API surface from implementation details. Future phases (M2) will add new internal consumers of `MatrixViewRef`/`MatrixViewMut` and `tensor_ops` helpers; flagging them `pub(crate)` now prevents accidental external coupling and signals to readers "this is not the library API".
Output: `src/matrix.rs`, `src/tensor_ops.rs`, `src/aes.rs` updated with narrower visibility and inline docs. `cargo build --lib`, `cargo build --tests --benches`, and `cargo test --lib` all green.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/REQUIREMENTS.md
@.planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md

<interfaces>
<!-- Verified in-crate consumers of each public item being narrowed. Confirmed by grep. -->

Consumers of `tensor_ops::gen_populate_seeds_mem_optimized` and `tensor_ops::gen_unary_outer_product`:
- src/auth_tensor_gen.rs:11  `use crate::tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product};`
- src/tensor_gen.rs:12        `use crate::tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product};`

Consumers of `matrix::MatrixViewRef` / `matrix::MatrixViewMut`:
- src/auth_tensor_eval.rs:5   `use crate::matrix::MatrixViewRef;`
- src/auth_tensor_eval.rs:6   `use crate::matrix::MatrixViewMut;`
- src/tensor_eval.rs:7        `matrix::{ ... }` (part of a larger use group — verify the exact members imported when editing)
- src/auth_tensor_gen.rs:10   `matrix::{BlockMatrix, MatrixViewRef},`
- src/tensor_gen.rs:8         `matrix::{ ... }`
- src/tensor_ops.rs:5         `matrix::{MatrixViewMut, MatrixViewRef},`
- src/lib.rs:50               `matrix::BlockMatrix,` (BlockMatrix stays `pub`, only the view types are narrowed)

No consumers in `benches/`, `tests/`, or `examples/` — this was verified by Grep before the plan was written. `pub(crate)` is therefore safe.

From src/matrix.rs (current signatures — only the `pub` keyword on `struct MatrixViewRef<'a, T>` and `struct MatrixViewMut<'a, T>` is in scope for this plan; their method impls stay `pub` within the now-narrowed struct):
```rust
pub struct MatrixViewRef<'a, T: MatrixElement> { ... }
pub struct MatrixViewMut<'a, T: MatrixElement> { ... }
impl<T: MatrixElement> TypedMatrix<T> {
    #[inline]
    fn flat_index(&self, i: usize, j: usize) -> usize {
        j*self.rows + i    // <-- column-major: needs type-level doc
    }
}
```

From src/tensor_ops.rs (current signatures):
```rust
pub fn gen_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) { ... }

pub fn gen_unary_outer_product(
    seeds: &Vec<Block>,
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<Block> { ... }
```

From src/aes.rs:
```rust
pub static FIXED_KEY_AES: Lazy<FixedKeyAes> = Lazy::new(|| FixedKeyAes {
    aes: Aes128Enc::new_from_slice(&FIXED_KEY).unwrap(),
});
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
  <name>Task 1: Narrow tensor_ops visibility and document flat_index column-major indexing (CLEAN-05)</name>
  <files>src/tensor_ops.rs, src/matrix.rs</files>
  <read_first>
    - src/tensor_ops.rs (full file — two `pub fn` declarations to narrow; preserve all bodies)
    - src/matrix.rs (lines 19–80 — TypedMatrix and flat_index; lines 31–50 MatrixViewRef/MatrixViewMut struct headers)
    - src/auth_tensor_gen.rs lines 1–20 (confirm it imports via `use crate::tensor_ops::{...}` and `use crate::matrix::{...}` — in-crate, so `pub(crate)` is visible)
    - src/tensor_gen.rs lines 1–20 (same)
    - src/auth_tensor_eval.rs lines 1–10, src/tensor_eval.rs lines 1–15 (confirm in-crate imports of MatrixViewRef/Mut)
  </read_first>
  <behavior>
    - `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` in tensor_ops.rs are declared `pub(crate) fn`
    - `MatrixViewRef<'a, T>` and `MatrixViewMut<'a, T>` in matrix.rs are declared `pub(crate) struct`
    - Method impls inside those structs keep their existing `pub`/`#[inline]` qualifiers (Rust permits `pub` methods on a `pub(crate)` struct; the effective visibility is the stricter one — no code change needed inside the impls)
    - TypedMatrix has a `///` doc comment on the struct that documents column-major indexing, or the `flat_index` method has a `///` doc that documents the same — per D-11, either/both is acceptable
    - `cargo build --lib`, `cargo build --tests --benches`, and `cargo test --lib` all succeed
  </behavior>
  <action>
    Per D-10 and D-11. Make exactly these edits:

    1. **`src/tensor_ops.rs`** — narrow two function visibilities (line 9 and line 88):

    Replace (at approximately line 9)
    ```rust
    pub fn gen_populate_seeds_mem_optimized(
    ```
    with
    ```rust
    pub(crate) fn gen_populate_seeds_mem_optimized(
    ```

    Replace (at approximately line 88)
    ```rust
    pub fn gen_unary_outer_product(
    ```
    with
    ```rust
    pub(crate) fn gen_unary_outer_product(
    ```

    Do NOT change any other code in `tensor_ops.rs` — no body edits, no signature edits, no doc-comment reflow.

    2. **`src/matrix.rs`** — narrow two struct visibilities and add column-major doc:

    Replace (at approximately line 31)
    ```rust
    pub struct MatrixViewRef<'a, T: MatrixElement> {
    ```
    with
    ```rust
    pub(crate) struct MatrixViewRef<'a, T: MatrixElement> {
    ```

    Replace (at approximately line 42)
    ```rust
    pub struct MatrixViewMut<'a, T: MatrixElement> {
    ```
    with
    ```rust
    pub(crate) struct MatrixViewMut<'a, T: MatrixElement> {
    ```

    Do NOT change the `pub struct TypedMatrix`, `pub type KeyMatrix`, `pub type BlockMatrix`, or any `pub trait MatrixElement` — those remain public API.

    Add a doc comment to `TypedMatrix`. Replace the line

    ```rust
    #[derive(Debug, Clone)]
    pub struct TypedMatrix<T: MatrixElement> {
    ```

    with

    ```rust
    /// A dense two-dimensional matrix over a `MatrixElement` (currently `Key` or `Block`).
    ///
    /// **Storage is column-major**: element `(i, j)` (row `i`, column `j`) is stored at
    /// linear index `j * rows + i` in the underlying `Vec<T>`. The same convention is
    /// used by `MatrixViewRef` / `MatrixViewMut` and by every consumer of the auth-bit
    /// `n*m` vectors in the protocol (for example `correlated_auth_bit_shares[j*n+i]`
    /// in `auth_tensor_gen` / `auth_tensor_eval`). Do not change the convention without
    /// auditing every consumer.
    #[derive(Debug, Clone)]
    pub struct TypedMatrix<T: MatrixElement> {
    ```

    Additionally, add a doc comment directly above the existing `flat_index` method (currently at approximately line 54–57):

    Replace
    ```rust
        #[inline]
        fn flat_index(&self, i: usize, j: usize) -> usize {
            j*self.rows + i
        }
    ```
    with
    ```rust
        /// Converts a 2-D `(row, column)` pair to a linear index in the underlying
        /// column-major storage: `index = j * rows + i` where `i` is the row and
        /// `j` is the column.
        #[inline]
        fn flat_index(&self, i: usize, j: usize) -> usize {
            j*self.rows + i
        }
    ```

    Do NOT change any other method bodies, impls, or tests in `matrix.rs`. The existing unit tests (`test_flat_index`, `test_matrix_indexing`, etc.) must continue to pass.

    3. No changes to `benches/`, `tests/`, `examples/`, or any other `src/*.rs` file — by grep, no external consumer imports these items.
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -10 &amp;&amp; cargo build --tests --benches 2>&amp;1 | tail -10 &amp;&amp; cargo test --lib matrix::tests 2>&amp;1 | tail -20</automated>
  </verify>
  <acceptance_criteria>
    - `grep -n "pub(crate) fn gen_populate_seeds_mem_optimized" src/tensor_ops.rs` matches exactly one line
    - `grep -n "pub(crate) fn gen_unary_outer_product" src/tensor_ops.rs` matches exactly one line
    - `grep -cE "^pub fn (gen_populate_seeds_mem_optimized|gen_unary_outer_product)" src/tensor_ops.rs` outputs `0`
    - `grep -n "pub(crate) struct MatrixViewRef" src/matrix.rs` matches exactly one line
    - `grep -n "pub(crate) struct MatrixViewMut" src/matrix.rs` matches exactly one line
    - `grep -cE "^pub struct (MatrixViewRef|MatrixViewMut)" src/matrix.rs` outputs `0`
    - `grep -n "Storage is column-major" src/matrix.rs` matches at least one line
    - `grep -n "index = j \* rows + i" src/matrix.rs` matches at least one line (the flat_index doc)
    - `grep -n "pub struct TypedMatrix" src/matrix.rs` still matches one line (unchanged visibility)
    - `grep -n "pub type KeyMatrix" src/matrix.rs` still matches one line
    - `grep -n "pub type BlockMatrix" src/matrix.rs` still matches one line
    - `cargo build --lib` exits 0
    - `cargo build --tests --benches` exits 0
    - `cargo test --lib matrix::tests` exits 0 (all 11 pre-existing matrix tests pass)
  </acceptance_criteria>
  <done>Four visibility narrowings committed, column-major doc added to TypedMatrix and flat_index, full build and matrix tests green.</done>
</task>

<task type="auto" tdd="false">
  <name>Task 2: Document the FIXED_KEY_AES Lazy singleton (CLEAN-06)</name>
  <files>src/aes.rs</files>
  <read_first>
    - src/aes.rs (lines 1–20 — to see current `pub static FIXED_KEY_AES: Lazy<FixedKeyAes>` and `FIXED_KEY` constant)
    - .planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md (D-12 — implementer discretion on content and placement)
  </read_first>
  <behavior>
    - `FIXED_KEY_AES` carries a `///` doc comment that mentions `once_cell::sync::Lazy`, explains lazy initialization on first use, explains `Send + Sync` / thread-safety, and states the fixed key is a protocol constant (not a secret)
    - No API or behavior change
    - `cargo build --lib` and `cargo test --lib` succeed
  </behavior>
  <action>
    Per D-12 (Claude's discretion on exact wording). Make exactly this edit to `src/aes.rs`:

    Replace (at approximately lines 14–17)

    ```rust
    /// Fixed-key AES cipher
    pub static FIXED_KEY_AES: Lazy<FixedKeyAes> = Lazy::new(|| FixedKeyAes {
        aes: Aes128Enc::new_from_slice(&FIXED_KEY).unwrap(),
    });
    ```

    with

    ```rust
    /// Global fixed-key AES cipher used by the TCCR / CCR / CR hash constructions
    /// throughout the protocol (GGM seed expansion, outer-product correction
    /// ciphertexts, etc.).
    ///
    /// # Why `once_cell::sync::Lazy`
    ///
    /// `FixedKeyAes` wraps `aes::Aes128Enc`, which performs AES key expansion once
    /// when constructed. `Lazy` defers that one-time construction until first use
    /// and then caches the result for the lifetime of the process. `Lazy<T>` is
    /// `Send + Sync` whenever `T: Send + Sync`, which `Aes128Enc` is, so this
    /// static can be read from any thread (test threads, async runtime worker
    /// threads, benchmark threads) without synchronization. Internally `Lazy`
    /// uses a once-lock to serialize the first-initialization race.
    ///
    /// # Why a fixed key
    ///
    /// The value in `FIXED_KEY` is a **protocol constant**, not a secret. The
    /// security of the TCCR / CCR / CR constructions (see Guo et al., eprint
    /// 2019/074) depends on fixed-key AES being correlation-robust, not on the
    /// key being hidden. Any constant key works; the specific bytes are
    /// arbitrary. Do not replace this with a per-session key — doing so would
    /// break interoperability between garbler and evaluator.
    pub static FIXED_KEY_AES: Lazy<FixedKeyAes> = Lazy::new(|| FixedKeyAes {
        aes: Aes128Enc::new_from_slice(&FIXED_KEY).unwrap(),
    });
    ```

    Do NOT change the `FIXED_KEY` constant, the `FixedKeyAes` struct, any method in `impl FixedKeyAes`, or the `AesEncryptor` type — only the doc comment on the static is updated.
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -10 &amp;&amp; cargo test --lib aes_test 2>&amp;1 | tail -10</automated>
  </verify>
  <acceptance_criteria>
    - `grep -n "once_cell::sync::Lazy" src/aes.rs` matches at least one line (the doc comment)
    - `grep -n "Send + Sync" src/aes.rs` matches one line
    - `grep -n "protocol constant" src/aes.rs` matches one line (states the key is not a secret)
    - `grep -n "pub static FIXED_KEY_AES: Lazy<FixedKeyAes>" src/aes.rs` matches exactly one line (declaration unchanged)
    - `cargo build --lib` exits 0
    - `cargo test --lib aes_test` exits 0
  </acceptance_criteria>
  <done>Doc comment added explaining Lazy choice, thread safety, and protocol-constant status; aes_test still passes.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Crate internal API → downstream binaries / crates | Only items marked `pub` (without `(crate)`) are reachable; this plan narrows two structs and two functions to `pub(crate)` |
| Global static `FIXED_KEY_AES` → all callers | Shared mutable-in-cell state used across threads; already `Send + Sync` by construction |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-06 | Information disclosure | Narrowing `tensor_ops` helpers to `pub(crate)` could break an out-of-crate consumer | accept | Grep of `src/`, `benches/`, and `tests/` shows zero external users; only `auth_tensor_gen.rs` and `tensor_gen.rs` import these items, both in-crate. `pub(crate)` remains reachable to in-crate code. Acceptance criterion `cargo build --tests --benches` exits 0 enforces this empirically. |
| T-01-07 | Tampering | Doc comment on `FIXED_KEY_AES` could mislead a reader into thinking the fixed key must be kept secret, prompting harmful "rotation" work | mitigate | Doc explicitly states the key is a protocol constant, not a secret, and warns that replacing it would break garbler/evaluator interop. |
| T-01-08 | Repudiation | No audit trail of why visibility was narrowed | mitigate | `pub(crate)` change is visible in git diff; the plan references D-10 / D-11 explicitly. |
| T-01-09 | Denial of service | A thread race on `Lazy` first-initialization could deadlock | accept | `once_cell::sync::Lazy` uses a once-lock internally and is documented as race-free by `once_cell`. The doc comment calls this out for future readers. No code change. |

No new high-severity threats. The visibility narrowing strictly reduces the attack surface exposed to downstream crates; the doc additions are informational.
</threat_model>

<verification>
After both tasks:

```bash
cargo build --lib
cargo build --tests --benches
cargo test --lib
```

All must exit 0. Specifically verify the matrix test module (11 tests) still passes and `aes_test` still passes.

Spot checks:
```bash
# Visibility narrowings landed
grep -c "pub(crate) fn gen_populate_seeds_mem_optimized\|pub(crate) fn gen_unary_outer_product" src/tensor_ops.rs   # expect 2
grep -c "pub(crate) struct MatrixViewRef\|pub(crate) struct MatrixViewMut" src/matrix.rs                              # expect 2

# Public API items preserved
grep -c "pub struct TypedMatrix\|pub type KeyMatrix\|pub type BlockMatrix" src/matrix.rs                              # expect 3

# Docs present
grep -c "Storage is column-major\|index = j \* rows + i" src/matrix.rs                                                 # expect >=2
grep -c "once_cell::sync::Lazy\|protocol constant" src/aes.rs                                                           # expect >=2
```
</verification>

<success_criteria>
- `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` in `src/tensor_ops.rs` are `pub(crate)`
- `MatrixViewRef` and `MatrixViewMut` in `src/matrix.rs` are `pub(crate)`
- `TypedMatrix` has a doc comment stating column-major storage with the formula `index = j * rows + i`
- `flat_index` has a doc comment restating the same formula at the method level
- `FIXED_KEY_AES` in `src/aes.rs` has a doc comment covering why `once_cell::sync::Lazy` is used, thread-safety guarantees (`Send + Sync`), and the fact that the fixed key is a protocol constant, not a secret
- `cargo build --lib`, `cargo build --tests --benches`, `cargo test --lib` all exit 0
</success_criteria>

<output>
After completion, create `.planning/phases/01-uncompressed-preprocessing/01-matrix-ops-aes-SUMMARY.md`.
</output>
