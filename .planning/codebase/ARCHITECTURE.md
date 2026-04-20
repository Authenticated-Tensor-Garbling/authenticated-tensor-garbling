# Architecture

**Analysis Date:** 2026-04-19

## Pattern Overview

**Overall:** Two-party cryptographic protocol implementation — a garbler/generator (Gen) and an evaluator (Eval) executing a tensor product gate in the garbled circuit model.

**Key Characteristics:**
- Implements the "Authenticated Tensor Garbling" protocol from the referenced paper (see `references/Authenticated_Garbling_with_Tensor_Gates-7.pdf`)
- Two protocol tiers: semi-honest (no authentication) and maliciously-secure (with authenticated bits / MACs)
- All values are represented as 128-bit `Block`s; the free-XOR trick is used (garbler holds a global `Delta`, evaluator never sees it)
- Input masking with one-time pads (`alpha`, `beta`) splits the tensor product `x ⊗ y` into three independent outer products processed in sequence
- A chunking factor controls the trade-off between memory and number of PRF evaluations per outer product

## Protocol Tiers

### Semi-Honest Tensor Product
Entry points: `src/tensor_gen.rs`, `src/tensor_eval.rs`
Preprocessing: `src/tensor_pre.rs`

Steps:
1. `SemiHonestTensorPre` samples a global `Delta`, creates label sharings for inputs `x`, `y` and random masks `alpha`, `beta`, then XORs them together to produce masked wires `x⊕alpha`, `y⊕beta`.
2. `into_gen_eval()` splits the combined state into `SemiHonestTensorPreGen` (garbler) and `SemiHonestTensorPreEval` (evaluator).
3. `TensorProductGen` / `TensorProductEval` each receive their half and execute three rounds of garbling/evaluation:
   - `garble_first_half_outer_product` / `evaluate_first_half_outer_product` — computes `(x⊕alpha) ⊗ y`
   - `garble_second_half_outer_product` / `evaluate_second_half_outer_product` — computes `(y⊕beta) ⊗ alpha`
   - `garble_final_outer_product` / `evaluate_final_outer_product` — combines the two halves and adds the `alpha⊗beta` correction term to recover `x⊗y`

### Authenticated (Maliciously Secure) Tensor Product
Entry points: `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`
Preprocessing (ideal Fpre): `src/auth_tensor_fpre.rs`

Steps:
1. `TensorFpre` (trusted dealer / ideal functionality) generates authenticated bit shares (`AuthBit`) for masks `alpha`, `beta`, their product `alpha·beta` (correlated bits), and randomness `gamma`. Each `AuthBit` contains cross-authenticated shares: the generator holds `(key_b, mac_a, bit_a)` and the evaluator holds `(key_a, mac_b, bit_b)` where `mac = key ⊕ bit·delta`.
2. `into_gen_eval()` splits into `TensorFpreGen` and `TensorFpreEval`.
3. `AuthTensorGen` / `AuthTensorEval` mirror the semi-honest protocol but use `AuthBitShare` values to derive wire labels for `alpha` and `beta`, replacing plain `Block` labels. The correlated share `alpha·beta` is used directly in the final combination step, replacing the garbler's `color_cross_product` computation.

## Core Data Flow

```
[Input bits x (n-bit), y (m-bit)]
         |
   [Fpre / TensorPre]
   Sample Delta, alpha, beta
   Build label sharings for x⊕alpha, y⊕beta, alpha, beta
   (+ AuthBits for authenticated variant)
         |
   into_gen_eval()
        / \
  Gen         Eval
   |             |
garble_first_half   evaluate_first_half
(x⊕alpha ⊗ y)   ←─── (levels, cts) ───→   fills first_half_out
         |
garble_second_half  evaluate_second_half
(y⊕beta ⊗ alpha)  ←─── (levels, cts) ───→  fills second_half_out
         |
garble_final / evaluate_final
  XOR: first_half_out ⊕ second_half_out^T ⊕ (alpha⊗beta correction)
         |
   [Output: BlockMatrix (n×m) representing x⊗y in garbled form]
```

## Key Abstractions

### `Block` (`src/block.rs`)
- A 128-bit value `[u8; 16]`, the atomic unit for all wire labels, keys, and MACs.
- Supports `BitXor`, `BitAnd`, `sigma` (swap-halves), `lsb()`, `set_lsb()` for point-and-permute.
- Used as the return type and operand throughout all crypto operations.
- Trait `BlockSerialize` provides a serialization abstraction for compound types.

### `Delta` (`src/delta.rs`)
- Newtype wrapper around `Block` representing the global garbling offset.
- Always has LSB = 1 (set in `Delta::new()`), enforcing the free-XOR invariant.
- Each garbling session (and each party in the authenticated variant) has one `Delta`.

### `Key` / `Mac` (`src/keys.rs`, `src/macs.rs`)
- Both are newtypes of `Block`.
- `Key` is held by the garbler; `Mac` is held by the evaluator.
- The invariant is `mac = key ⊕ bit·delta`.
- `Key::auth(bit, delta)` computes the corresponding `Mac` for a given bit value.
- Addition (`+`) on both types is defined as XOR (GF(2) arithmetic).

### `AuthBitShare` / `AuthBit` (`src/sharing.rs`)
- `AuthBitShare { key: Key, mac: Mac, value: bool }` — one party's view of an authenticated bit.
- `AuthBit { gen_share: AuthBitShare, eval_share: AuthBitShare }` — the full two-party sharing, used only inside `TensorFpre` before splitting.
- `Add` on `AuthBitShare` performs component-wise XOR (homomorphic addition in GF(2)).

### `InputSharing` (`src/sharing.rs`)
- `{ gen_share: Block, eval_share: Block }` — a wire label pair for the semi-honest protocol.
- `bit()` recovers the encoded value by checking equality.

### `TypedMatrix<T>` / `BlockMatrix` / `KeyMatrix` (`src/matrix.rs`)
- Column-major `n×m` matrix generic over `T: MatrixElement` (sealed to `Block` and `Key`).
- `BlockMatrix = TypedMatrix<Block>`, `KeyMatrix = TypedMatrix<Key>`.
- `MatrixViewRef` / `MatrixViewMut` — zero-copy windowed views supporting `shift`, `resize`, `transpose`, and sub-row slicing via `with_subrows`.
- All output wire labels for the tensor product are stored in `BlockMatrix` fields on `TensorProductGen`/`AuthTensorGen` (`first_half_out`, `second_half_out`).

### `FixedKeyAes` / `AesEncryptor` (`src/aes.rs`)
- `FixedKeyAes` wraps AES-128 with a globally fixed key (stored in `FIXED_KEY_AES: Lazy<FixedKeyAes>`).
- Provides three hash modes used throughout garbling:
  - `tccr` — tweakable circular correlation-robust hash: `π(π(x) ⊕ i) ⊕ π(x)`
  - `cr` — correlation-robust hash: `π(x) ⊕ x`
  - `ccr` — circular correlation-robust hash: `π(σ(x)) ⊕ σ(x)`
- `tccr` is the primary PRF used in seed-tree expansion and outer-product garbling.
- `AesEncryptor` provides the raw AES-128 encryption interface used to instantiate `FixedKeyAes`.

## Garbling Primitive: Seed-Tree Outer Product (`src/tensor_ops.rs`)

The core cryptographic primitive for each half outer product is a **GGM-style binary tree** over the `n`-bit vector `x`:

**Generator side (`gen_populate_seeds_mem_optimized`):**
1. Root seeds `S_0`, `S_1` are derived from the wire label `x[n-1]` and `x[n-1] ⊕ delta`.
2. At each level `i`, every seed is expanded into two children using `tccr(0, s)` and `tccr(1, s)`.
3. The generator publishes the XOR-sum of even-indexed seeds and odd-indexed seeds at each level (`odd_evens`). This allows the evaluator to reconstruct any missing leaf.
4. The final leaves (length-`2^n` vector) seed the unary outer product.

**Evaluator side (`eval_populate_seeds_mem_optimized` in `src/tensor_eval.rs`, `src/auth_tensor_eval.rs`):**
1. Evaluator knows only one root seed (determined by the pointer bit of `x[n-1]`).
2. At each level, the evaluator reconstructs the sibling of the "missing path" node using the published sums.
3. The missing leaf index is tracked and used to skip exactly one seed in the outer product accumulation.

**Outer product assembly (`gen_unary_outer_product`, `eval_unary_outer_product`):**
- For each column `j` of the output matrix (indexed by `y`), seeds are hashed with a column-and-row tweak and XOR'd into the appropriate output cells.
- The generator publishes correction ciphertexts (`gen_cts`) for each `y` column; the evaluator applies them.

**Chunking:**
- The `x` vector is processed in chunks of size `chunking_factor`.
- Each chunk runs a separate seed-tree expansion, reducing peak memory from `O(2^n)` to `O(2^chunking_factor)`.

## Entry Points

**`src/lib.rs`:**
- Re-exports all public modules.
- Defines the global constants `MAC_ZERO`, `MAC_ONE` (fixed public MAC values for bits 0 and 1).
- Defines `CSP = 128` (computational security parameter) and `SSP = 40` (statistical security parameter).
- Contains the integration tests `test_semihonest_tensor_product` and `test_auth_tensor_product`.

**`benches/benchmarks.rs`:**
- Criterion benchmarks over `BENCHMARK_PARAMS` (matrix sizes 4×4 to 128×128).
- Benchmarks both full protocol garbling and network-simulated runtime.
- Uses `benches/network_simulator.rs` (Tokio-based) to model latency and bandwidth.

## Error Handling

**Strategy:** Panic-on-contract-violation (research/prototype code).

**Patterns:**
- `assert!` / `assert_eq!` guard all preconditions (dimension checks, MAC verification).
- `AuthBitShare::verify` panics if `mac ≠ key.auth(bit, delta)`.
- No `Result` types or custom error enums are used anywhere in the core protocol code.

## Cross-Cutting Concerns

**Randomness:** `ChaCha12Rng` (seeded) used throughout preprocessing; `rand::rng()` (OS entropy) used for live `Delta` generation.
**Serialization:** `Block` derives `serde::Serialize/Deserialize` and `bytemuck::Pod/Zeroable`; no serialization framework is wired up for the full protocol state.
**Endianness:** All bit-vectors are little-endian (index 0 = LSB). Every file that depends on this convention carries an "Endianness note" comment.
**No networking:** The real protocol communication is entirely absent; data is passed directly between `Gen` and `Eval` structs in memory. `benches/network_simulator.rs` adds synthetic delay to model it.

---

*Architecture analysis: 2026-04-19*
