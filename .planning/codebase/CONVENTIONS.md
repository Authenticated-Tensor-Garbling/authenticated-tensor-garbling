# Coding Conventions

**Analysis Date:** 2026-04-19

## Naming Patterns

**Files:**
- `snake_case` for all module files: `tensor_gen.rs`, `auth_tensor_eval.rs`, `tensor_ops.rs`
- Protocol roles appended as suffixes: `_gen` for garbler/generator, `_eval` for evaluator, `_fpre` for the Fpre ideal functionality, `_pre` for pre-processing
- Auth-prefixed modules are the authenticated variants: `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `auth_tensor_fpre.rs`

**Structs:**
- `PascalCase` throughout: `Block`, `Delta`, `Key`, `Mac`, `AuthBitShare`, `AuthBit`, `TensorProductGen`, `AuthTensorEval`
- Protocol roles as suffixes: `TensorProductGen`, `TensorProductEval`, `SemiHonestTensorPre`, `TensorFpre`, `TensorFpreGen`, `TensorFpreEval`
- View types suffixed `Ref`/`Mut` to distinguish borrow semantics: `MatrixViewRef`, `MatrixViewMut`

**Functions and Methods:**
- `snake_case` universally: `gen_inputs`, `gen_masks`, `mask_inputs`, `into_gen_eval`, `garble_first_half`, `evaluate_final`
- Constructor pattern: `new()` for default construction, `new_from_fpre_gen()` / `new_from_fpre_eval()` for construction from a pre-processed state object
- `new_with_delta()` variant when the caller supplies a `Delta` value (used heavily in tests)
- Private helpers prefixed with nothing (just private visibility): `eval_populate_seeds_mem_optimized`, `eval_unary_outer_product`
- Setup functions in benchmarks prefixed with `setup_`: `setup_auth_gen`, `setup_auth_eval`

**Variables:**
- Single-letter names are used freely for mathematical indices: `i`, `j`, `k`, `s`, `n`, `m`
- Cryptographic values use domain names directly: `delta`, `alpha`, `beta`, `gamma`, `seeds`, `cts` (ciphertexts), `levels`
- Underscore-prefixed variables for intentionally unused bindings: `_clear_value`, `_eval_cts`, `_chunk_levels`, `_first_levels`

**Constants:**
- `SCREAMING_SNAKE_CASE`: `CSP`, `SSP`, `MAC_ZERO`, `MAC_ONE`, `FIXED_KEY`, `FIXED_KEY_AES`, `BENCHMARK_PARAMS`, `X_INPUT`, `Y_INPUT`

**Type Aliases:**
- Used for backward compatibility and domain clarity: `KeyMatrix = TypedMatrix<Key>`, `BlockMatrix = TypedMatrix<Block>`

**Traits:**
- `PascalCase` describing capability: `MatrixElement`, `BlockSerialize`
- Sealed traits placed in a private `mod sealed {}` within the same file: `src/matrix.rs`

## Code Style

**Formatting:**
- No `rustfmt.toml` present; default `rustfmt` formatting is assumed
- Rust 2024 edition (`edition = "2024"` in `Cargo.toml`)
- Inline closures and lambdas used freely for iterator chains

**Visibility:**
- Public API uses `pub` with no module-level re-exports through `lib.rs`; each module exposes its own items directly
- `pub(crate)` used for crate-internal items not intended for external users: `MAC_ZERO`, `MAC_ONE` in `src/lib.rs` and `src/macs.rs`
- Internal implementation details kept `pub` on struct fields when the struct is used as a data bundle (e.g., `TensorFpreGen`, `AuthTensorGen`)
- Private fields used when the type has meaningful invariants: `TensorFpre`, `Block`, `Delta`, `Key`, `Mac`

**Derives:**
- Cryptographic primitives derive `Copy, Clone, PartialEq` by default
- Serializable types derive `Serialize, Deserialize` from serde and `Pod, Zeroable` from bytemuck
- Debug is derived on most public types; Display is manually implemented when a custom format is needed

**Inline Annotations:**
- `#[inline]` on small pure methods: `lsb()`, `as_block()`, `pointer()`, `auth()`
- `#[inline(always)]` on the hottest cryptographic paths: `bitxor_assign`, `bitand_assign`, `sigma`, `AesEncryptor::new`, `para_encrypt`
- `#[repr(transparent)]` on `Block` to make newtype transmutes safe

**Suppression Attributes:**
- `#[allow(dead_code)]` used on constants and methods not yet wired up: `CSP`, `SSP`, `MAC_ZERO`, `MAC_ONE` in `src/lib.rs`, `Block::as_array`, `Block::as_array_slice`

## Operator Overloading Idiom

Cryptographic types (`Block`, `Key`, `Mac`, `Delta`) implement the full set of `BitXor`, `BitXorAssign`, `BitAnd`, `BitAndAssign`, and `Add` (where addition is XOR in GF(2)) for all reference combinations (`T op T`, `T op &T`, `&T op T`, `&T op &T`). This avoids unnecessary copies in hot code. Example from `src/block.rs`:

```rust
impl BitXor for Block { ... }
impl BitXor<&Block> for Block { ... }
impl BitXor<Block> for &Block { ... }
impl BitXor<&Block> for &Block { ... }
impl BitXorAssign<&Block> for Block { ... }
impl BitXorAssign for Block { ... }
```

Key and Mac overload `Add` to mean XOR (GF(2) field addition), keeping cryptographic semantics explicit.

## Builder / Conversion Pattern

Fpre types follow a builder pattern:
1. Construct with `new()` or `new_with_delta()` — sets up empty state
2. Call `generate_with_input_values()` / `gen_inputs()` / `gen_masks()` / `mask_inputs()` — populates data
3. Call `into_gen_eval()` — consumes `self` and returns a `(GenState, EvalState)` tuple

The split into Gen/Eval pair via `into_gen_eval()` is the standard handoff from offline pre-processing to online protocol roles. See `src/tensor_pre.rs` and `src/auth_tensor_fpre.rs`.

## Error Handling

There is no user-facing error type or `Result`-returning public API. The project uses `thiserror` as a dependency (listed in `Cargo.toml`) but no custom error types are defined in the source — error handling is minimal:

- `panic!` / `assert!` / `assert_eq!` used for programming invariants (`assert!(x < 1<<self.n)` in `src/tensor_pre.rs`)
- `debug_assert!` used for bounds-checked vector/matrix indexing in debug builds (`src/matrix.rs`)
- `.unwrap()` used in initialization of singletons where failure is impossible given hardcoded key data (`src/aes.rs` lines 16, 149)
- No propagation of errors to callers; this is a research prototype, not a production library

## Unsafe Code

Unsafe is used in three specific patterns, all with safety comments:

1. `Block::as_flattened_bytes` / `Block::array_as_flattened_bytes` in `src/block.rs` — safe because `Block` is `repr(transparent)` over `[u8; 16]`
2. Newtype slice transmutes for `Key` and `Mac` in `src/keys.rs` and `src/macs.rs` — safe because each is a single-field newtype of `Block`
3. `Block::as_array_slice` / `Block::as_array_mut_slice` in `src/block.rs` — transmute between `[Block]` and `[Array<u8, U16>]`, layout-identical types

Pattern:
```rust
// Safety:
// Key is a newtype of block.
unsafe { std::mem::transmute(blocks) }
```

## Documentation Style

- Module-level doc comments with `//!` used on `src/block.rs` and `src/aes.rs`
- Public methods documented with `///` single-line summaries
- Multi-line doc comments used on complex methods with mathematical notation, citing papers: `tccr`, `cr`, `ccr` in `src/aes.rs`
- Paper references use angle-bracket URLs: `/// See <https://eprint.iacr.org/2019/074> (Section 7.4)`
- Algorithm-level inline comments document endianness conventions throughout `src/tensor_ops.rs` and `src/tensor_eval.rs`:
  ```
  // Endianness note (little-endian vectors):
  // Index 0 is LSB, index n-1 is MSB. ...
  ```
- Commented-out code left in place with `//` prefix (not removed): see `src/keys.rs` lines 42–47 (a `commit` method stub)

## Common Idioms

**Collecting iterator output:**
```rust
self.x_labels.iter().map(|share| share.gen_share).collect()
```

**Bit extraction from integer (little-endian):**
```rust
let x_bit = (x >> i) & 1 != 0;
```

**Conditional label selection (garbling pattern):**
```rust
let ev_share = if x_bit { gb_share ^ self.delta.as_block() } else { gb_share };
```

**Fold to recover integer from label vector (little-endian):**
```rust
(0..view.rows()).rev().fold(0, |acc, i| {
    (acc << 1) | view[i].lsb() as usize
})
```

**Static singleton via `once_cell::Lazy`:**
```rust
pub static FIXED_KEY_AES: Lazy<FixedKeyAes> = Lazy::new(|| FixedKeyAes { ... });
```

---

*Convention analysis: 2026-04-19*
