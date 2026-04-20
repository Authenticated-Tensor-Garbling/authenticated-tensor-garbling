# Codebase Structure

**Analysis Date:** 2026-04-19

## Directory Layout

```
authenticated-tensor-garbling/
├── src/                        # Core library implementation
│   ├── lib.rs                  # Crate root: module declarations, constants, integration tests
│   ├── block.rs                # Block (128-bit) primitive type
│   ├── delta.rs                # Delta (global garbling offset) newtype
│   ├── keys.rs                 # Key newtype (garbler-held MAC key)
│   ├── macs.rs                 # Mac newtype (evaluator-held MAC)
│   ├── sharing.rs              # InputSharing, AuthBitShare, AuthBit structs
│   ├── matrix.rs               # TypedMatrix<T>, BlockMatrix, KeyMatrix, view types
│   ├── aes.rs                  # FixedKeyAes (TCCR/CR/CCR hash), AesEncryptor
│   ├── tensor_pre.rs           # Semi-honest preprocessing: SemiHonestTensorPre, PreGen, PreEval
│   ├── tensor_gen.rs           # Semi-honest garbler: TensorProductGen
│   ├── tensor_eval.rs          # Semi-honest evaluator: TensorProductEval
│   ├── tensor_ops.rs           # Shared garbling primitives (seed tree, outer product)
│   ├── auth_tensor_fpre.rs     # Authenticated preprocessing (ideal Fpre): TensorFpre, FpreGen, FpreEval
│   ├── auth_tensor_gen.rs      # Authenticated garbler: AuthTensorGen
│   └── auth_tensor_eval.rs     # Authenticated evaluator: AuthTensorEval
├── benches/
│   ├── benchmarks.rs           # Criterion benchmark suite (Cargo [[bench]] entry point)
│   └── network_simulator.rs    # Tokio-based synthetic network delay helper
├── references/                 # Academic papers and reference implementation (not compiled)
│   ├── Authenticated_Garbling_with_Tensor_Gates-7.pdf
│   ├── 2022-798.pdf
│   ├── appendix_krrw_pre.tex
│   └── mpz-dev/                # Reference library (separate Cargo workspace, not a dependency)
├── Cargo.toml                  # Package manifest and dependencies
├── Cargo.lock                  # Locked dependency versions
├── README.md                   # Usage and project overview
├── communication.csv           # Benchmark output data (communication costs)
├── walltime.csv                # Benchmark output data (wall-clock times)
└── .vscode/                    # Editor configuration (launch.json, settings.json)
```

## Module Hierarchy

`src/lib.rs` declares all modules as `pub mod`:

```
lib
├── block          (Block, BlockSerialize)
├── delta          (Delta)
├── keys           (Key)
├── macs           (Mac)
├── sharing        (InputSharing, AuthBitShare, AuthBit, build_share)
├── matrix         (TypedMatrix<T>, BlockMatrix, KeyMatrix, MatrixViewRef, MatrixViewMut)
├── aes            (FixedKeyAes, FIXED_KEY_AES, AesEncryptor)
├── tensor_pre     (SemiHonestTensorPre, SemiHonestTensorPreGen, SemiHonestTensorPreEval)
├── tensor_gen     (TensorProductGen)
├── tensor_eval    (TensorProductEval)
├── tensor_ops     (gen_populate_seeds_mem_optimized, gen_unary_outer_product)
├── auth_tensor_fpre  (TensorFpre, TensorFpreGen, TensorFpreEval)
├── auth_tensor_gen   (AuthTensorGen)
└── auth_tensor_eval  (AuthTensorEval)
```

**Dependency relationships between modules (inner → outer):**

```
block ← delta ← keys ← sharing
block             ← macs ← sharing
block ← matrix
block ← aes
block, delta, sharing ← tensor_pre
block, delta, matrix, aes, tensor_pre, tensor_ops ← tensor_gen
block, matrix, aes, tensor_pre ← tensor_eval
block, delta, matrix, aes ← tensor_ops
block, delta, sharing ← auth_tensor_fpre
block, delta, sharing, auth_tensor_fpre, matrix, aes, tensor_ops ← auth_tensor_gen
block, delta, sharing, auth_tensor_fpre, matrix, aes ← auth_tensor_eval
```

## Directory Purposes

**`src/` — Core library (compiled as a Rust library crate):**
- No `main.rs`; the crate is a pure library.
- All modules are `pub`; there is no internal/private module split.
- `tensor_ops.rs` is the only module shared between the semi-honest and authenticated garbler paths.

**`benches/` — Benchmark harness:**
- `benchmarks.rs` is the Criterion entry point (`[[bench]] name = "benchmarks"` in `Cargo.toml`).
- `network_simulator.rs` is included as a submodule of `benchmarks.rs` via `mod network_simulator;`, not as a separate Cargo module.
- Uses `tokio` for async network simulation; the benchmark group `bench_full_protocol_garbling` sweeps matrix sizes from 4×4 to 128×128.

**`references/` — Research material (not compiled, not a Cargo workspace member):**
- Contains the paper being implemented and a reference Rust library (`mpz-dev`).
- `mpz-dev/` has its own `Cargo.toml` and workspace but is not referenced by this project's `Cargo.toml`.

## Key File Locations

**Entry Points:**
- `src/lib.rs`: crate root; module declarations, global constants (`CSP`, `SSP`, `MAC_ZERO`, `MAC_ONE`), integration tests
- `benches/benchmarks.rs`: `criterion_main!` benchmark entry point

**Cryptographic Primitives:**
- `src/block.rs`: `Block` — 128-bit array with XOR, AND, sigma, LSB manipulation
- `src/aes.rs`: `FixedKeyAes` with `tccr`, `cr`, `ccr` hash functions; global `FIXED_KEY_AES` singleton via `once_cell`
- `src/delta.rs`: `Delta` — garbling offset, always has LSB = 1

**Authentication Layer:**
- `src/keys.rs`: `Key` — garbler-side MAC key; `Key::auth(bit, delta)` derives the corresponding `Mac`
- `src/macs.rs`: `Mac` — evaluator-side MAC; public constants `MAC_ZERO`, `MAC_ONE`
- `src/sharing.rs`: `AuthBitShare`, `AuthBit`, `build_share`; defines the cross-authenticated sharing structure

**Matrix / Wire Label Storage:**
- `src/matrix.rs`: `TypedMatrix<T>`, type aliases `BlockMatrix` / `KeyMatrix`; view types `MatrixViewRef`, `MatrixViewMut`

**Protocol — Semi-Honest:**
- `src/tensor_pre.rs`: Preprocessing — samples Delta, generates label sharings, splits into Gen/Eval halves
- `src/tensor_gen.rs`: `TensorProductGen` — garbles first half, second half, final combination
- `src/tensor_eval.rs`: `TensorProductEval` — evaluates all three rounds; contains duplicated `eval_populate_seeds_mem_optimized`
- `src/tensor_ops.rs`: Shared primitives — `gen_populate_seeds_mem_optimized`, `gen_unary_outer_product`

**Protocol — Authenticated:**
- `src/auth_tensor_fpre.rs`: Ideal Fpre — generates `AuthBit`s for alpha/beta/correlated/gamma; splits into FpreGen/FpreEval
- `src/auth_tensor_gen.rs`: `AuthTensorGen` — authenticated garbling of first/second/final half
- `src/auth_tensor_eval.rs`: `AuthTensorEval` — authenticated evaluation; also contains duplicated `eval_populate_seeds_mem_optimized`

## Naming Conventions

**Files:**
- Primitive/utility modules: `<noun>.rs` (e.g., `block.rs`, `delta.rs`, `matrix.rs`)
- Protocol roles follow the pattern `<protocol_variant>_<role>.rs`:
  - `tensor_gen.rs`, `tensor_eval.rs` — semi-honest Gen/Eval
  - `auth_tensor_gen.rs`, `auth_tensor_eval.rs` — authenticated Gen/Eval
  - `tensor_pre.rs`, `auth_tensor_fpre.rs` — preprocessing
- Shared operations: `tensor_ops.rs`

**Structs:**
- Protocol state: `TensorProductGen`, `TensorProductEval`, `AuthTensorGen`, `AuthTensorEval`
- Preprocessing: `SemiHonestTensorPre`, `SemiHonestTensorPreGen`, `SemiHonestTensorPreEval`
- Authenticated preprocessing: `TensorFpre`, `TensorFpreGen`, `TensorFpreEval`
- Primitive newtypes: `Block`, `Delta`, `Key`, `Mac`
- Sharing types: `InputSharing`, `AuthBitShare`, `AuthBit`
- Matrix types: `TypedMatrix<T>`, `BlockMatrix` (alias), `KeyMatrix` (alias)

**Functions:**
- `snake_case` throughout.
- Garbler methods: `garble_*` (e.g., `garble_first_half`, `garble_final`).
- Evaluator methods: `evaluate_*` (e.g., `evaluate_first_half`, `evaluate_final`).
- Input preparation: `get_first_inputs`, `get_second_inputs`.
- Preprocessing split: `into_gen_eval()` on all `*Pre` / `*Fpre` structs.

## Where to Add New Code

**New cryptographic hash / PRF:**
- Add to `src/aes.rs` as a method on `FixedKeyAes`.

**New wire label type (not `Block` or `Key`):**
- Implement `MatrixElement` (sealed trait in `src/matrix.rs`) for the new type and add to `matrix.rs`.

**New protocol variant (e.g., a different security model):**
- Follow the naming pattern `<variant>_fpre.rs`, `<variant>_gen.rs`, `<variant>_eval.rs`.
- Declare all three as `pub mod` in `src/lib.rs`.
- Reuse `tensor_ops.rs` for the seed-tree and outer-product primitives.

**New sharing / authentication primitive:**
- Add to `src/sharing.rs`.

**New benchmark:**
- Add a new benchmark function to `benches/benchmarks.rs` and register it in the `criterion_group!` macro at the bottom of that file.

## Special Directories

**`references/`:**
- Purpose: academic papers (PDF, LaTeX) and a reference Rust codebase.
- Generated: No
- Committed: Yes (tracked in git)
- Not compiled: `references/mpz-dev/` is a completely separate Cargo workspace not referenced by this project.

**`.vscode/`:**
- Purpose: VSCode launch and settings configuration.
- Generated: No (manually maintained)
- Committed: Yes

---

*Structure analysis: 2026-04-19*
