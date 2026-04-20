# External Integrations

**Analysis Date:** 2026-04-19

## APIs & External Services

None. The project has no network clients, no HTTP/gRPC calls, and no connections to external services. All execution is local, in-process.

## Cryptographic Library Integration

**AES-128 (RustCrypto):**
- Crates: `aes` 0.9.0-rc.0, `cipher` 0.5.0-rc.0
- Usage: `src/aes.rs` wraps `Aes128Enc` behind two higher-level abstractions:
  - `FixedKeyAes` — global singleton (via `once_cell::sync::Lazy`) keyed with a fixed constant; exposes `cr`, `ccr`, `tccr`, `tccr_many` for correlation-robust hashing as described in ePrint 2019/074 §7.2–7.4
  - `AesEncryptor` — per-instance keyed wrapper; supports `encrypt_block`, `encrypt_blocks`, and `para_encrypt` for batch/parallel encryption
- Bridge: `Block` ↔ `hybrid-array::Array<u8, U16>` via `Block::as_array_mut()` / `Block::as_array_mut_slice()` — required because `cipher`'s `BlockCipherEncrypt` trait operates on `GenericArray`/`Array` types, not raw `[u8; 16]`

**BLAKE3:**
- Crate: `blake3` 1.3.3 (lockfile: 1.8.2)
- Status: declared as a dependency but not imported in any observed source file. Reserved for future use (likely MAC commitment / hash-then-commit patterns).

**Random Number Generation:**
- `rand` 0.9.2 + `rand_chacha` 0.9.0
- `rand::rng()` provides a thread-local OS-seeded CSPRNG for one-shot random generation in non-deterministic paths
- `ChaCha12Rng::seed_from_u64(seed)` is used wherever reproducible test/bench setup is required (`TensorFpre::new`, `build_share`)
- `CryptoRng` bound is enforced at the API level for security-sensitive generators (`Block::random_array`, `Block::random_vec`)

## Data Storage

**Databases:** None.
**File Storage:** None (benchmark results are written by Criterion to `target/criterion/` automatically).
**Caching:** None.

## Inter-Module Communication Patterns

The codebase models a two-party protocol (Generator / Evaluator). All communication is simulated in-process by passing Rust data structures directly. There is no socket, channel, or IPC layer in the core library.

**Data flow for authenticated tensor product:**

```
TensorFpre                        (ideal functionality / trusted setup)
    │
    ├─ into_gen_eval()
    │     ├─ TensorFpreGen  ──→  AuthTensorGen
    │     └─ TensorFpreEval ──→  AuthTensorEval
    │
AuthTensorGen                    AuthTensorEval
    │ garble_first_half()              │
    │ → (Vec<Vec<Block>>, Vec<Vec<Block>>)
    │ ─────────────────────────────→  │ evaluate_first_half(levels, cts)
    │ garble_second_half()             │
    │ → (Vec<Vec<Block>>, Vec<Vec<Block>>)
    │ ─────────────────────────────→  │ evaluate_second_half(levels, cts)
    │ garble_final()                   │ evaluate_final()
```

The "wire" between generator and evaluator is represented as plain `Vec<Vec<Block>>` tuples returned from `garble_*` and consumed by `evaluate_*`. In benchmarks, `SimpleNetworkSimulator::send_size_with_metrics` injects an async sleep to model the cost of transmitting those bytes over a real network (100 Mbps, zero latency by default).

**Chunking parameter:**
A `chunking_factor: usize` controls how the outer-product computation is decomposed into sub-problems. Larger values trade fewer circuit levels for more ciphertext material (and thus more simulated bandwidth). Benchmarks sweep `chunking_factor` ∈ {1, 2, 4, 6, 8}.

## Network Simulation

**`benches/network_simulator.rs` — `SimpleNetworkSimulator`:**
- No real networking; models bandwidth and latency with `tokio::time::sleep`
- Parameters: `bandwidth_mbps: f64`, `latency_ms: u64`
- `send_size_with_metrics(bytes)` sleeps for `latency_ms` + transmission time derived from byte count
- Used only in benchmark groups `full_protocol_with_networking` and the per-size `*_runtime_with_networking` groups
- Driven by a lazily-initialized multi-thread Tokio runtime (`once_cell::sync::Lazy<tokio::runtime::Runtime>`)

## Serialization Format

`serde` derives (`Serialize`/`Deserialize`) are present on `Block` and `Mac`. No concrete serialization format (JSON, bincode, CBOR, etc.) is instantiated in any source file — the derives are plumbing for potential future wire encoding. `bytemuck::Pod` + `Zeroable` on `Block` enables zero-copy casting to/from `&[u8]` for any future framing layer.

## Protocol Format Dependencies

| Primitive | Source | Standard |
|-----------|--------|----------|
| AES-128 fixed-key hash | `src/aes.rs` | ePrint 2019/074 §7.2–7.4 |
| Free-XOR garbling delta (LSB=1) | `src/delta.rs`, `src/keys.rs` | Standard free-XOR / half-gate garbling |
| Point-and-permute (pointer bit in MAC LSB) | `src/macs.rs` | Standard garbled circuit pointer technique |
| SPDZ-style authenticated bits (key/MAC pairs) | `src/sharing.rs` | SPDZ / BDOZ MAC-based authentication |

## CI/CD & Deployment

None detected. No CI configuration files (`.github/`, `.gitlab-ci.yml`, etc.) are present. The project is a standalone research library.

## Environment Configuration

No environment variables are read at runtime. No `.env` files detected.

---

*Integration audit: 2026-04-19*
