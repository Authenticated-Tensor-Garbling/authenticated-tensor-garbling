# Technology Stack

**Analysis Date:** 2026-04-19

## Languages

**Primary:**
- Rust (edition 2024) - all source code, benchmarks, and tests

## Runtime

**Environment:**
- Rust toolchain 1.90.0 (stable)

**Package Manager:**
- Cargo 1.90.0
- Lockfile: present (`Cargo.lock`)

## Frameworks

**Benchmarking:**
- `criterion` 0.7.0 - statistical microbenchmarking; also used as a `[dependencies]` entry (not only `[dev-dependencies]`) so it is available at bench compile time; async support via `criterion = { version = "0.7", features = ["async_tokio"] }` in `[dev-dependencies]`

**Async Runtime:**
- `tokio` 1.47.1 (features = ["full"]) - used exclusively in `benches/network_simulator.rs` to simulate transmission latency with `tokio::time::sleep`; the benchmark binary drives it through a lazily-initialized `tokio::runtime::Runtime` (`once_cell::sync::Lazy<tokio::runtime::Runtime>`)

## Key Dependencies

**Cryptographic Primitives:**
- `aes` 0.9.0-rc.0 (pre-release) - AES-128 block cipher (`Aes128Enc`); used in `src/aes.rs` to implement fixed-key AES and correlation-robust hash functions (CR, CCR, TCCR) per ePrint 2019/074
- `cipher` 0.5.0-rc.0 (pre-release) - `RustCrypto` trait abstractions (`BlockCipherEncrypt`, `KeyInit`) consumed by the `aes` crate
- `blake3` 1.3.3 (resolves to 1.8.2 in lockfile) - present as a dependency but not observed in active source paths; likely reserved for future commitment/hashing

**Randomness:**
- `rand` 0.9.2 (lockfile) - thread-local `rand::rng()`, `Rng` trait, `CryptoRng` bound used throughout for random block and delta generation
- `rand_chacha` 0.9.0 - `ChaCha12Rng` used as the deterministic CSPRNG in `src/sharing.rs` and `src/auth_tensor_fpre.rs` (seeded via `SeedableRng::seed_from_u64`)

**Serialization:**
- `serde` 1.0 (features = ["derive"]) - `Serialize`/`Deserialize` derives on `Block`, `Mac`, and related types
- `bytemuck` 1.23.2 (lockfile) (features = ["derive"]) - `Pod`/`Zeroable` derives on `Block`; used for zero-copy casting between `[u8; 16]`, `[u64; 2]`, and `Block`
- `serde_arrays` 0.1 - serde support for fixed-size arrays; available but not prominently used in observed source

**Type / Array Utilities:**
- `hybrid-array` 0.3 - typed fixed-length arrays (`Array<u8, U16>`) used as the bridge between `Block` and `aes`/`cipher` API surfaces (`block.rs` line 5, 144–165)
- `itybity` 0.3 - bit-level iterator traits (`BitIterable`, `BitLength`, `GetBit`, `FromBitIterator`, `Lsb0`, `Msb0`) implemented on `Block` in `src/block.rs`

**Error Handling:**
- `thiserror` 2 - derive macro for custom `Error` types; declared but no custom error types are yet defined in the observed source (reserved for future use)

**Lazy Initialization:**
- `once_cell` 1.21.3 - `once_cell::sync::Lazy` used in `src/aes.rs` to create a global `FIXED_KEY_AES` singleton and in `benches/benchmarks.rs` for the global Tokio runtime

## Build / Dev Tooling

**Bench harness:**
- Custom `[[bench]]` target `benchmarks` with `harness = false` — wired to `benches/benchmarks.rs` and `benches/network_simulator.rs`
- Benchmark groups cover tensor sizes 4×4 through 256×256 with chunking factors 1–8

**No build scripts detected** (`build.rs` absent).

## Security Parameters (constants in `src/lib.rs`)

| Constant | Value | Meaning |
|----------|-------|---------|
| `CSP` | 128 | Computational security parameter (bits) |
| `SSP` | 40 | Statistical security parameter (bits) |

## Module Map

| File | Role |
|------|------|
| `src/block.rs` | 128-bit `Block` newtype — the universal primitive |
| `src/aes.rs` | `FixedKeyAes` (CR/CCR/TCCR) and `AesEncryptor` wrappers |
| `src/delta.rs` | `Delta` newtype for the garbling global offset |
| `src/keys.rs` | `Key` newtype (MAC key) |
| `src/macs.rs` | `Mac` newtype |
| `src/sharing.rs` | `InputSharing`, `AuthBitShare`, `AuthBit`, `build_share` |
| `src/matrix.rs` | `TypedMatrix<T>`, `BlockMatrix`, `KeyMatrix`, view types |
| `src/tensor_pre.rs` | Semi-honest pre-processing (`SemiHonestTensorPre`) |
| `src/tensor_gen.rs` | Semi-honest garbler (`TensorProductGen`) |
| `src/tensor_eval.rs` | Semi-honest evaluator (`TensorProductEval`) |
| `src/tensor_ops.rs` | Core garbling primitives (`gen_populate_seeds_mem_optimized`, `gen_unary_outer_product`) |
| `src/auth_tensor_fpre.rs` | Malicious-secure ideal Fpre (`TensorFpre`, `TensorFpreGen`, `TensorFpreEval`) |
| `src/auth_tensor_gen.rs` | Malicious-secure garbler (`AuthTensorGen`) |
| `src/auth_tensor_eval.rs` | Malicious-secure evaluator (`AuthTensorEval`) |
| `benches/benchmarks.rs` | Criterion benchmark suite |
| `benches/network_simulator.rs` | `SimpleNetworkSimulator` — tokio-based bandwidth/latency model |

---

*Stack analysis: 2026-04-19*
