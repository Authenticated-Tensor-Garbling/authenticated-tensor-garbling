# Coding Conventions

**Analysis Date:** 2026-04-28

## Rust Edition

**Edition:** 2024 (declared in `Cargo.toml` line 4)

- `gen` is a reserved keyword in Rust 2024; bindings use `gen_out` / `eval_out`
  instead. Grep for `// Note: bindings named gen_out / eval_out because gen is a
  reserved keyword in Rust 2024 edition` in `src/preprocessing.rs` and
  `src/auth_tensor_pre.rs` tests for examples of this in-flight adaptation.

## Naming Patterns

**Types (structs, enums, traits):**
- PascalCase throughout: `Block`, `Delta`, `Key`, `Mac`, `AuthBitShare`,
  `AuthBit`, `InputSharing`, `TensorFpre`, `AuthTensorGen`, `AuthTensorEval`,
  `TensorFpreGen`, `TensorFpreEval`, `BlockMatrix`, `IdealBCot`, `LeakyTriple`,
  `FixedKeyAes`, `AesEncryptor`, `MatrixViewRef`, `MatrixViewMut`,
  `TensorMacroCiphertexts`.
- Trait names: `TensorPreprocessing`, `BlockSerialize`, `MatrixElement`.
- No deviations found.

**Functions and methods:**
- `snake_case` throughout: `check_zero`, `hash_check_zero`, `build_share`,
  `gen_auth_bit`, `verify_cross_party`, `bucket_size_for`, `combine_leaky_triples`,
  `garble_first_half`, `evaluate_second_half_p2`, `new_from_fpre_gen`,
  `run_preprocessing`, `two_to_one_combine`, `apply_permutation_to_triple`.
- Methods that map to paper symbols use Greek letter names where clear:
  `alpha_d_ev_shares`, `beta_auth_bit_shares`, `delta_a`, `delta_b`.

**Variables and fields:**
- `snake_case`: `chunking_factor`, `l_gamma`, `c_gamma_bit`, `combined_key`,
  `gb_v_alpha_d_ev`, `l_alpha_pub`, `e_a_bit`, `d_bits`.
- Fields representing mathematical objects use paper notation verbatim where
  unambiguous: `alpha_auth_bit_shares`, `gamma_d_ev_shares`, `correlated_d_ev_shares`.

**Constants:**
- `SCREAMING_SNAKE_CASE`: `CSP`, `SSP`, `MAC_ZERO`, `MAC_ONE`, `FIXED_KEY`,
  `FIXED_KEY_AES`, `NETWORK_BANDWIDTH_BPS`, `KAPPA_BYTES`, `RHO_BYTES`,
  `WIDE_DOMAIN`, `AES_BLOCK_COUNT`. No deviations found.

**Module names:**
- `snake_case`: `block`, `delta`, `keys`, `macs`, `sharing`, `matrix`, `aes`,
  `tensor_pre`, `tensor_gen`, `tensor_eval`, `tensor_ops`, `tensor_macro`,
  `auth_tensor_fpre`, `auth_tensor_gen`, `auth_tensor_eval`, `bcot`, `feq`,
  `leaky_tensor_pre`, `auth_tensor_pre`, `preprocessing`, `online`.
- Module names directly mirror their primary type where applicable
  (`block` → `Block`, `delta` → `Delta`).

**Protocol party naming:**
- Generator / garbler: `gb` for locals, `gen_out` for return bindings in Rust 2024 scope.
- Evaluator: `ev` for locals, `eval_out` for return bindings.
- `delta_a` = garbler's global correlation key (LSB=1); `delta_b` = evaluator's (LSB=0).

## Linting and Clippy

**No clippy.toml or rustfmt.toml** in the project root. No `.cargo/config.toml`.
The reference library in `references/mpz-dev/rustfmt.toml` sets
`imports_granularity = "Crate"` and `wrap_comments = true` — these appear to
influence the project style by convention but are not enforced automatically.

**Inline `#[allow(...)]` suppressions only — no crate-level `#![deny]`:**
- `#[allow(clippy::too_many_arguments)]` on `assemble_gate_semantics_shares`,
  `assemble_e_input_wire_shares_p1`, `assemble_c_alpha_beta_shares_p2`
  (`src/lib.rs` lines 88, 247, 385). These are intentionally wide entry-point
  functions that combine both parties' state for simulation.
- `#[allow(dead_code)]` on `MAC_ZERO`, `MAC_ONE` (`src/lib.rs` lines 43, 48);
  `Block::as_array`, `Block::as_array_slice` (`src/block.rs` lines 143, 156);
  `LeakyTensorPre` fields (`src/leaky_tensor_pre.rs` lines 338, 359).
- No `#![deny(unsafe_code)]` — `unsafe` is used deliberately in `block.rs` for
  zero-copy slice reinterpretation (`from_raw_parts`, `transmute`) with safety
  comments on each site.

## Import Organization

**Order within a file (observed pattern):**
1. `use crate::...` — intra-crate imports, often grouped by sub-module
2. `use rand::...` / `use rand_chacha::...` — external crates
3. `use std::...` — std imports

No enforced separation between groups (no blank-line between `crate::` groups).
Imports in `#[cfg(test)] mod tests { ... }` blocks are written as additional
`use` statements inside the `mod tests` body, not re-exported.

**Example (from `src/preprocessing.rs`):**
```rust
use crate::{block::Block, delta::Delta, sharing::{AuthBitShare, build_share}};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
use crate::auth_tensor_fpre::TensorFpre;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
```

**Path aliases:** None. All imports use full paths.

## Error Handling

**Dominant pattern: `panic!` / `assert!` / `assert_eq!`**

There is no `Result<>` anywhere in the protocol logic. `thiserror` is listed as a
dependency (`Cargo.toml` line 23) but is not used — no `#[derive(Error)]`,
no `enum *Error`, no `type Result` aliases. This appears to be a leftover
dependency stub, not active usage.

**Protocol-abort semantics are modeled as panics:**
- `assert_eq!` guards at function entry validate length invariants (all public
  entry-point functions gate on slice lengths).
- `panic!("F_eq abort: ...")` in `src/feq.rs:25` — ideal F_eq consistency check
  that should abort the protocol panics instead.
- `share.verify(&delta)` in `src/sharing.rs:60-63` panics with `"MAC mismatch
  in share"` on failure.
- `two_to_one_combine` panics on delta mismatch (`src/auth_tensor_pre.rs:35-46`).

**`unwrap()` is used only at initialization sites:**
- `Aes128Enc::new_from_slice(&FIXED_KEY).unwrap()` in `src/aes.rs:37` and `:170`
  — the key slice is a known-correct constant; `unwrap()` is safe here.

**`expect()` / `anyhow` / error propagation: not used.**

**Convention:** For cryptographic correctness failures (MAC mismatch, dimension
mismatch, protocol invariant violation), panic with a descriptive message that
names the invariant. For flow-control failures in the simulation layer (e.g.,
`count != 1` before batch support is added), panic with a message explaining
what is not yet supported and how to work around it.

## Documentation Conventions

**Module-level doc comments (`//!`):**
- Used consistently for modules with substantial structure:
  `src/online.rs`, `src/feq.rs`, `src/block.rs`, `src/aes.rs`, `src/preprocessing.rs`.
- Module docs explain the paper correspondence and in-process simulation intent.

**Item-level doc comments (`///`):**
- Required on all `pub` functions, all `pub` types, and most `pub` fields that
  encode paper concepts.
- Doc comments on public functions consistently include:
  - What the function implements and which paper section / line numbers it maps to.
  - Explanation of the mathematical formula, often with a `code block` rendering
    of the LaTeX.
  - `# Inputs` / `# Returns` sections for multi-argument functions.
  - `# Panics` section when a function has non-trivial panic paths.
  - `SIMULATION ONLY` warnings when both parties' state is required simultaneously.
  - Paper references using the form:
    `` `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` ``
    followed by `lines NNN–NNN`.

**Paper reference style:**
```rust
/// Implements the Protocol 1 consistency check from
/// `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex`
/// lines 226–247
```

**Formula rendering style (inside doc comments):**
```rust
/// ```text
///   [e_a[i] D_ev] := [a[i] D_ev] XOR [l_a[i] D_ev] XOR (a[i] XOR l_a[i]) D_ev
/// ```
```

**Inline comments:**
- Inline `//` comments on cryptographically sensitive lines explain the
  security rationale, not just what the code does. Example from `src/online.rs`:
  "Do NOT use the `AuthBitShare::add` (`+`) operator to combine cross-party shares
  directly."
- Protocol step labels (`D-01`, `D-03`, `W-04`) appear in comments to link back
  to phase design docs in `.planning/phases/`.

**Comments on commented-out code:**
- Commented-out code is left in place with a `//` prefix and a short rationale
  (e.g., the `commit` method in `src/keys.rs:55-62` has an explanatory comment).

## Visibility Patterns

**`pub`:** All types, structs, and functions intended for crate-external use
(or for benchmark access). Most structs expose their fields as `pub` directly
rather than using accessor methods — this is deliberate for a research codebase
where simulation tests need direct field access.

**`pub(crate)`:** Used for internal helpers shared between modules but not
intended for external use:
- `MatrixViewRef`, `MatrixViewMut` (`src/matrix.rs:39, 50`)
- `TensorMacroCiphertexts` (`src/tensor_macro.rs:54`)
- `two_to_one_combine`, `apply_permutation_to_triple`, `verify_cross_party`
  (`src/auth_tensor_pre.rs:29, 278, 324`)
- `gen_populate_seeds_mem_optimized`, `eval_populate_seeds_mem_optimized`,
  `gen_unary_outer_product`, `eval_unary_outer_product`,
  `gen_unary_outer_product_wide`, `eval_unary_outer_product_wide`
  (`src/tensor_ops.rs`)
- `Block::as_array`, `Block::as_array_mut`, etc. (`src/block.rs`)

**Private (no modifier):**
- `AuthTensorGen::final_computed` (`src/auth_tensor_gen.rs:57`) — protocol
  ordering guard not exposed externally.
- Test helper functions inside `mod tests` blocks.

**Rule:** If a function is only needed to verify an invariant inside tests,
it stays private to the `mod tests` block. If it needs to be shared across
test modules, it is `pub(crate)` on the item.

## Function Design

**Size:** Functions are long when they match paper construction steps. The three
public entry points in `src/lib.rs` exceed 80 lines each because they inline
the full simulation. Internal functions (`derive_d_ev_blocks`, `check_zero`,
`build_share`) are compact.

**Parameters:** Wide multi-party functions (> 6 parameters) use
`#[allow(clippy::too_many_arguments)]` explicitly rather than collecting
parameters into a struct — kept explicit so callers can see exactly which party
views are being combined.

**Return values:**
- Protocol-step functions return tuples `(Vec<...>, Vec<...>)` mirroring
  the paper's gen/eval output split.
- Check functions return `bool` (`check_zero`), or `Vec<AuthBitShare>`
  (assembly helpers).
- `Result<>` is not used; failures are panics.

**Operator overloading:**
- `Add` on `Key`, `Mac`, `AuthBitShare` means XOR (field addition in GF(2)).
- `BitXor` / `BitXorAssign` on `Block`, `Delta`, `Key` for direct 128-bit XOR.
- All combinations of `(T, &T, owned, borrowed)` are implemented for the core
  numeric types to match the Rust ergonomics convention.

## Module Design

**Exports:** All modules are declared `pub mod` in `src/lib.rs`. No barrel
re-exports (`pub use`) — callers use fully-qualified module paths
(`use crate::block::Block`).

**Barrel files:** None. Each module is a single flat `.rs` file.

**Struct fields are `pub` by default** in protocol structs (`TensorFpreGen`,
`TensorFpreEval`, `AuthTensorGen`, `AuthTensorEval`) to allow test and bench
direct field reads without accessor boilerplate.

## Logging

**No logging framework.** `print!` / `println!` appear sparingly in tests
(e.g., `print!("{} ", expected_val)` inside `test_auth_tensor_product` in
`src/lib.rs:737`) — these are leftover debug prints from early development,
not a logging convention. There is no `log`, `tracing`, or `env_logger`
dependency.

---

*Convention analysis: 2026-04-28*
