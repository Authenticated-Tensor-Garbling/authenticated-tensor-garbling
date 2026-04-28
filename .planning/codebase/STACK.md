# Technology Stack

**Analysis Date:** 2026-04-28

## Languages

**Primary:**
- Rust (edition 2024) — all protocol logic, benchmarks, and examples

**Secondary:**
- Python 3 — post-processing tools: `tools/parse_results.py`, `tools/comparison_table.py`

## Runtime

**Environment:**
- Native binary (no WASM, no embedded target)
- Tested on Apple Silicon (ARMv8 AES hardware acceleration confirmed in `examples/aes_microbench.rs`)

**Package Manager:**
- Cargo
- Lockfile: `Cargo.lock` present and committed

**Rust Version:**
- Edition 2024 (requires rustc ≥ 1.85)
- No `rust-version` (MSRV) field in `Cargo.toml`; no `rust-toolchain.toml` pinning
- Observed runtime: rustc 1.90.0

## Frameworks

**Benchmark Harness:**
- `criterion` 0.7.0 — wall-clock and throughput benchmarking; async variant enabled via `features = ["async_tokio"]` in dev-dependencies
- Two benchmark groups: `online_benches` (wired into `criterion_main`), plus `preprocessing_benches` and `network_benches` (defined but inactive — one-line re-enable)
- Bench entry: `benches/benchmarks.rs` (harness = false)

**Async Runtime:**
- `tokio` 1.47.1 with `features = ["full"]` — used exclusively in `benches/benchmarks.rs` for the `network_benches` group; `SimpleNetworkSimulator` uses `tokio::time::sleep` to model 100 Mbps wire latency
- Not used in `src/` (no async protocol code)

## Key Dependencies

**Cryptographic Primitives:**

| Crate | Locked Version | Purpose |
|-------|---------------|---------|
| `aes` | 0.9.0-rc.0 | `Aes128Enc` block cipher — fixed-key AES for TCCR / CCR / CR hash constructions (`src/aes.rs`) |
| `cipher` | 0.5.0-rc.0 | `BlockCipherEncrypt`, `KeyInit` traits that `aes` implements |
| `blake3` | 1.8.2 | Declared in `[dependencies]` but **not imported anywhere in `src/` or `benches/`** — a stale dependency |
| `rand` | 0.9.2 | `Rng`, `CryptoRng`, `SeedableRng` traits; `rand::rng()` for thread-local CSPRNG |
| `rand_chacha` | 0.9.0 | `ChaCha12Rng` — deterministic seeded RNG used throughout preprocessing and ideal functionalities |

**Hash Construction:**
- Fixed-key AES-128 (TCCR: `π(π(x) ⊕ i) ⊕ π(x)`, CR: `π(x) ⊕ x`) — implements the correlation-robust hash assumed in the paper. See `src/aes.rs`. No SHA-2 / SHA-3 / BLAKE3 used in this crate.
- `CheckZero` digest: CR-fold over `share.mac` blocks via `FIXED_KEY_AES.cr(...)` in `src/online.rs:92-98`

**Serialization:**

| Crate | Locked Version | Purpose |
|-------|---------------|---------|
| `serde` | 1.0.228 | `Serialize`/`Deserialize` derive on `Block` and related types |
| `bytemuck` | 1.23.2 | `Pod`/`Zeroable` derives; zero-cost reinterpretation of `Block` as `[u8; 16]` |
| `serde_arrays` | 0.1.0 | Fixed-size array serde support |

**Utility:**

| Crate | Locked Version | Purpose |
|-------|---------------|---------|
| `hybrid-array` | 0.3.1 | `Array<u8, U16>` — typenum-indexed fixed-size array for `Block` internals |
| `itybity` | 0.3.1 | `BitIterable`, `GetBit`, `Lsb0`/`Msb0` bit-level iteration on `Block` |
| `once_cell` | 1.21.3 | `Lazy<FixedKeyAes>` global singleton (process-level fixed-key AES cache) and `OnceCell` in benches |
| `thiserror` | 2.0.16 | `#[derive(Error)]` for protocol error types |

## Configuration

**Security Parameters (source of truth: `src/lib.rs`):**
```rust
pub const CSP: usize = 128;  // κ — computational security parameter (bits)
pub const SSP: usize = 40;   // ρ — statistical security parameter (bits)
```
Bench communication accounting (`KAPPA_BYTES`, `RHO_BYTES`) and the network simulator all derive from these constants.

**Build:**
- No `build.rs`
- No `.cargo/config.toml`
- No feature flags defined

**Benchmark Parameters (`benches/benchmarks.rs`):**
- Active sweep: `(n, m)` in `[(64,64), (128,128), (256,256)]`
- Chunking factor sweep: `1..=8`
- Network model: 100 Mbps, 0 ms latency (`NETWORK_BANDWIDTH_BPS = 100_000_000`)

## Platform Requirements

**Development:**
- Rust ≥ 1.85 (edition 2024)
- ARMv8 AES extension recommended for performance (`aarch64-apple-darwin` target enables `+aes` by default)
- Python 3 with `matplotlib` for `tools/parse_results.py` and `tools/comparison_table.py`

**Production:**
- Library crate only; no binary targets
- No deployment infrastructure — research prototype

---

*Stack analysis: 2026-04-28*
