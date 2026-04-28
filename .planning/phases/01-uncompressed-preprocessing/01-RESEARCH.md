# Phase 1: Uncompressed Preprocessing Protocol — Research

**Researched:** 2026-04-19
**Domain:** Two-party cryptographic preprocessing; GGM trees; correlated OT; leaky tensor triples; bucketing amplification
**Confidence:** HIGH (codebase analysis is direct), MEDIUM (protocol mapping to existing code)

---

## Summary

This phase replaces the ideal `TensorFpre` placeholder in `src/auth_tensor_fpre.rs` with a real two-party protocol following Appendix F of the paper (KRRW-style, Constructions 1–4). The existing codebase already contains working, tested GGM tree garbling logic in `tensor_gen.rs` / `tensor_eval.rs` and their authenticated variants in `auth_tensor_gen.rs` / `auth_tensor_eval.rs`. These files implement exactly the machinery needed for the TensorGb / TensorEv halves of Construction 1. The critical gap is the correlated OT layer (F_bCOT) required by Construction 2 (Pi_LeakyTensor) to bootstrap the authenticated shares. No OT primitive exists in the project crate — it is a pure library dependency gap. The MPZ reference codebase in `references/mpz-dev/` contains a complete ideal-COT model that can be ported or simulated inline.

The recommended approach is a four-module addition:
1. `src/bcot.rs` — a self-contained, in-process ideal bCOT (boolean correlated OT) that matches the benchmark philosophy of the existing code (no real networking required for correctness testing)
2. `src/leaky_tensor_pre.rs` — Pi_LeakyTensor (Construction 2), wiring together bcot + TensorGb/TensorEv
3. `src/auth_tensor_pre.rs` — Pi_aTensor (Construction 3), the bucketing amplifier
4. `src/auth_tensor_pre_permuted.rs` — Pi_aTensor' (Construction 4, optional), permutation bucketing

The existing `auth_tensor_fpre.rs` interface (`TensorFpreGen` / `TensorFpreEval`) is the exact output type the online phase already consumes. The real protocol must produce structs that are structurally identical to `TensorFpreGen` / `TensorFpreEval` (or replace them with a compatible type). No changes to `auth_tensor_gen.rs` or `auth_tensor_eval.rs` are needed.

**Primary recommendation:** Build a self-contained ideal-bCOT struct (single-process, shared-memory, no tokio channels needed for unit tests), implement Pi_LeakyTensor on top of it, then wrap with the bucketing combiner. Add a benchmark group for the preprocessing protocol that mirrors the existing `bench_full_protocol_with_networking` pattern.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| GGM tree seed expansion (garbler side) | `tensor_ops::gen_populate_seeds_mem_optimized` | `auth_tensor_gen` | Already implemented; reused directly |
| GGM tree seed expansion (evaluator side) | `auth_tensor_eval::eval_populate_seeds_mem_optimized` | — | Already implemented; reused directly |
| Correlated OT (bCOT) | New `src/bcot.rs` | — | Protocol primitive; nothing exists in crate |
| Label MAC generation (key/mac pairs) | `sharing::build_share` + `keys::Key::auth` | `macs::Mac` | Primitives already exist |
| Pi_LeakyTensor protocol | New `src/leaky_tensor_pre.rs` | `bcot`, `tensor_ops` | New module; orchestrates both roles |
| Bucketing (leaky → full triples) | New `src/auth_tensor_pre.rs` | — | New module; pure combiner logic |
| Output consumed by online phase | `auth_tensor_fpre::{TensorFpreGen,TensorFpreEval}` | — | Interface frozen; implementations must match |
| Preprocessing benchmark | `benches/benchmarks.rs` addition | `network_simulator` | Follow existing pattern |

---

## Existing Code Analysis

### What `tensor_gen.rs` implements

`tensor_gen.rs` defines `TensorProductGen` (the semi-honest garbler). The core GGM work lives in `tensor_ops.rs`:

- `gen_populate_seeds_mem_optimized(x, cipher, delta)` — builds the full 2^n GGM tree with level correction ciphertexts `(evens, odds)` per level. Seed expansion uses `cipher.tccr(tweak, seed)` at each node. Returns `(leaf_seeds, level_corrections)` where `level_corrections` is a `Vec<(Block, Block)>` (one pair per tree level).
- `gen_unary_outer_product(seeds, y, out, cipher)` — expands leaf seeds into the outer-product matrix using per-cell tweaked hashes. Returns `gen_cts: Vec<Block>` (one correction block per column of y).

The "chunking" wraps these into chunks of size `chunking_factor` (1–8 in benchmarks), slicing the x-vector and running independent sub-trees. This is purely a performance knob — correctness is independent of chunking.

`AuthTensorGen` in `auth_tensor_gen.rs` is the authenticated variant. It is structurally identical to `TensorProductGen` but uses `delta_a` (party A's global key) and takes its `x_labels` / `y_labels` from `TensorFpreGen.alpha_labels` / `beta_labels` — the masked offset vectors, not the raw inputs.

**Key insight for TensorGb mapping:** `gen_populate_seeds_mem_optimized` IS TensorGb's GGM expansion. The `level_corrections` output corresponds to the paper's `(G_{i,0}, G_{i,1})` pairs. The final correction ciphertext vector from `gen_unary_outer_product` corresponds to `G_k = (⊕_ℓ H(seed_ℓ, k)) ⊕ T_k^gb` where `T_k` is the y-label (`y[j]` in the code). The output matrix in `first_half_out` / `second_half_out` is the Z_gb matrix from Construction 1.

### What `tensor_eval.rs` implements

`TensorProductEval` and `AuthTensorEval` implement the evaluator side. Both contain a copy of `eval_populate_seeds_mem_optimized` and `eval_unary_outer_product`. These correspond precisely to TensorEv:

- The evaluator holds labels `x[i] = label_{x_i ⊕ alpha_i}` — it knows all labels except the one for its secret input index.
- `eval_populate_seeds_mem_optimized` reconstructs all leaf seeds except the missing one (index = `clear_value`), using the level corrections from the garbler.
- `eval_unary_outer_product` computes the evaluator's share of Z_ev using the gen_cts correction.

Note that `eval_populate_seeds_mem_optimized` is duplicated verbatim in both `tensor_eval.rs` and `auth_tensor_eval.rs`. This is a minor code smell but not a problem for the plan.

### What `auth_tensor_fpre.rs` currently provides

`TensorFpre` is a trusted dealer — a single struct that holds both parties' state and generates all correlated material from a shared RNG:

- Generates `delta_a`, `delta_b` randomly
- Generates `alpha_auth_bits` (Vec<AuthBit>, length n): authenticated bits for party A's mask
- Generates `beta_auth_bits` (Vec<AuthBit>, length m): authenticated bits for party B's mask
- Generates `correlated_auth_bits` (Vec<AuthBit>, length n*m): bits authenticating alpha_i * beta_j (the AND product), stored column-major
- Generates `gamma_auth_bits` (Vec<AuthBit>, length n*m): random one-time pads for the output
- Generates label sharings `x_labels` (length n) and `y_labels` (length m) as `InputSharing` (gen/eval block pairs)

`into_gen_eval()` splits these into `TensorFpreGen` (garbler's view) and `TensorFpreEval` (evaluator's view).

**What the real protocol must produce:** Exactly the same field structure as `TensorFpreGen` and `TensorFpreEval`. The split is:
- `TensorFpreGen`: delta_a, alpha_labels (Vec<Block>), beta_labels (Vec<Block>), alpha_auth_bit_shares (Vec<AuthBitShare>), beta_auth_bit_shares, correlated_auth_bit_shares, gamma_auth_bit_shares
- `TensorFpreEval`: delta_b, same four auth_bit_share vectors, plus alpha_labels and beta_labels

`AuthBitShare` = `{key: Key, mac: Mac, value: bool}`. The invariant is `mac == key.auth(value, delta_of_the_other_party)`.

### Gaps Summary

| Component | Status | Action |
|-----------|--------|--------|
| GGM garbler (TensorGb) | COMPLETE in `tensor_ops::gen_populate_seeds_mem_optimized` + `gen_unary_outer_product` | Reuse directly |
| GGM evaluator (TensorEv) | COMPLETE in `auth_tensor_eval::eval_populate_seeds_mem_optimized` + `eval_unary_outer_product` | Reuse directly |
| AuthBitShare / Key / Mac primitives | COMPLETE | Reuse directly |
| Correlated OT (F_bCOT) | MISSING — not in crate at all | Build `src/bcot.rs` |
| Pi_LeakyTensor (Construction 2) | MISSING | Build `src/leaky_tensor_pre.rs` |
| Bucketing combiner (Construction 3) | MISSING | Build `src/auth_tensor_pre.rs` |
| Permutation bucketing (Construction 4) | MISSING | Build `src/auth_tensor_pre_permuted.rs` (optional) |
| Equality check (F_eq) | MISSING (used in Pi_LeakyTensor consistency check) | Inline as XOR comparison (semi-honest) or simple commit |
| Preprocessing benchmarks | MISSING | Add benchmark group |

---

## COT Strategy

### What F_bCOT must provide

Construction 2 (Pi_LeakyTensor) requires a **boolean correlated OT** functionality:
- Sender holds global key Delta (128 bits), chooses keys K[0], K[1] for each pair
- Receiver holds choice bit b, receives K[b] where K[1] = K[0] XOR Delta
- This is standard 1-of-2 OT with a fixed correlation Delta

The protocol uses bCOT to generate four sets of authenticated bit-pairs:
1. `MAC_x_A / D_B` — A's x-bits authenticated under B's key
2. `MAC_x_B / D_A` — B's x-bits authenticated under A's key
3. `MAC_y_A / D_B`, `MAC_y_B / D_A` — same for y
4. `MAC_R / D` — a random mask triple

### Implementation options

**Option A (RECOMMENDED): In-process ideal bCOT**

Build a `struct IdealBCot` with shared state (similar to `TensorFpre` pattern). No channels, no async, no tokio. Both "sender" and "receiver" views are computed synchronously. This matches the existing codebase style exactly.

```rust
pub struct IdealBCot {
    delta: Delta,       // correlation key
    rng: ChaCha12Rng,
}

impl IdealBCot {
    // Sender generates keys K[0], then K[1] = K[0] XOR delta
    // Receiver with choice bit b gets K[b]
    // Returns (sender_keys_k0: Vec<Key>, receiver_chosen: Vec<Block>)
    pub fn transfer(&mut self, choices: &[bool]) -> (Vec<Block>, Vec<Block>);
}
```

This is sufficient for correctness testing and benchmarking (measuring the preprocessing computation cost, not the OT execution cost). The OT is a black box — its cost is well-studied and orthogonal to the tensor preprocessing.

**Option B: Tokio channel-based simulation**

Use the existing `SimpleNetworkSimulator` + tokio pattern from benchmarks to model the OT communication. More realistic for latency measurements but significantly more complex to implement and unnecessary for a first pass.

**Decision:** Use Option A for all correctness tests and unit benchmarks. Add an optional `BandwidthModel` wrapper for the preprocessing benchmark that accounts for the known bCOT communication cost (2n blocks per bCOT invocation for Ferret/IKNP) using the existing `SimpleNetworkSimulator.send_size_with_metrics` pattern.

### bCOT usage pattern in Pi_LeakyTensor

Construction 2 runs the bCOT to generate `MAC_x_A/D_B` etc. The mapping to `AuthBitShare` is:
- Receiver's output `mac_block = K[b] = K[0] XOR b*delta` is the MAC
- Sender's held key `K[0]` (with LSB cleared, per `Key::adjust`) is the Key
- Together they satisfy `mac == key.auth(b, delta)` — exactly `AuthBitShare::verify`

The `Key::adjust` method on `keys.rs` line 29 is precisely the tool for generating the `K[1] = K[0] XOR delta` relationship. The `build_share` function in `sharing.rs` (line 105) shows the full pattern: generate a random `Key`, call `key.auth(bit, delta)` to get the MAC.

---

## Communication Model for the Two-Party Protocol

### Current model

The existing codebase has no real IPC. The benchmark simulates communication by:
1. Running garbler computation (`garble_first_half`, etc.) to produce `chunk_levels` and `chunk_cts`
2. Calling `network.send_size_with_metrics(total_bytes).await` to simulate the transmission delay
3. Passing the computed data directly to the evaluator (in-process)

The "network" is purely a latency/bandwidth model, not actual message passing.

### Protocol message flow for Pi_LeakyTensor

The real protocol has three interactive phases:

```
Party A (Garbler)                    Party B (Evaluator)
-----------------                    ------------------
[Round 1 — bCOT setup]
delta_a, keys_x_a, keys_y_a  <--bCOT--> delta_b, macs_x_a, macs_y_a
keys_x_b, keys_y_b            <--bCOT--> macs_x_b, macs_y_b
keys_R                        <--bCOT--> macs_R

[Round 2 — TensorGb / TensorEv (A as garbler)]
A runs TensorGb(x_A labels, y_A labels) -->
         (chunk_levels, chunk_cts) -->  B runs TensorEv
         
[Round 3 — TensorGb / TensorEv (B as garbler)]
         <-- (chunk_levels, chunk_cts)  B runs TensorGb(x_B labels, y_B labels)
A runs TensorEv

[Round 4 — Consistency check (F_eq)]
C_A = S_1_A XOR S_2_A  -->
         <-- C_B = S_1_B XOR S_2_B
Both reveal lsb of their C to each other (the shared D = x XOR y XOR R bit)

[Output: MAC_Z/D = MAC_R/D XOR MAC_D/D]
```

### Recommended implementation structure

For Phase 1, implement this as a **synchronous two-struct protocol** (not async), matching the existing code style:

```rust
pub struct LeakyTensorPreGen { ... }  // Party A's state
pub struct LeakyTensorPreEval { ... } // Party B's state

// Round messages as plain structs passed by value
pub struct TensorRoundMsg {
    chunk_levels: Vec<Vec<(Block, Block)>>,
    chunk_cts: Vec<Vec<Block>>,
}
```

This keeps the benchmark harness simple (same `iter_batched` pattern as existing benchmarks) and defers real networking to a future phase.

---

## Implementation Plan Sketch

### New files to create

**`src/bcot.rs`** — Ideal boolean correlated OT
```
IdealBCot { delta_a: Delta, delta_b: Delta, rng }
  fn transfer_a_to_b(choices_b: &[bool]) -> (Vec<Key>, Vec<Mac>)
      // A holds keys[i] (for 0), receiver gets key[choice[i]]
  fn transfer_b_to_a(choices_a: &[bool]) -> (Vec<Key>, Vec<Mac>)
      // symmetric
```

**`src/leaky_tensor_pre.rs`** — Pi_LeakyTensor (Construction 2)
```
LeakyTensorPre { bcot, n, m, delta_a, delta_b }
  fn generate(...) -> (LeakyTensorPreGen, LeakyTensorPreEval)
  
LeakyTensorPreGen {
    // MAC_x_A/D_B:  alpha_auth_bit_shares: Vec<AuthBitShare>
    // MAC_y_A/D_B:  beta_auth_bit_shares: Vec<AuthBitShare>  
    // MAC_R/D:      r_auth_bit_shares: Vec<AuthBitShare>   (n*m)
    // Z_gb from first TensorGb call: first_half_out: BlockMatrix
    // Z_gb from second TensorEv call: second_half_out: BlockMatrix
}
```

**`src/auth_tensor_pre.rs`** — Pi_aTensor (Construction 3, bucketing)
```
AuthTensorPre { leaky_pre, bucket_size: B }
  fn generate(...) -> (TensorFpreGen, TensorFpreEval)
  // Combine B leaky triples into 1 authenticated triple
  // XOR the Z matrices; XOR the R bits; output MAC_Z/D
```

**`src/auth_tensor_pre_permuted.rs`** — Pi_aTensor' (Construction 4, optional)

### Bucket size formula

From Appendix F, Construction 3:
- Statistical error: `2 * ell^(1-B)` where `ell = n * m` (number of output bits per triple)
- Target: error < `2^(-ssp)` where `ssp = 40` (from `lib.rs` line 26)
- Therefore: `B = floor(ssp / log2(ell)) + 1`

For n=m=16: `ell = 256`, `log2(256) = 8`, `B = floor(40/8) + 1 = 6`
For n=m=128: `ell = 16384`, `log2(16384) = 14`, `B = floor(40/14) + 1 = 3`
For n=m=4: `ell = 16`, `log2(16) = 4`, `B = floor(40/4) + 1 = 11`

Construction 4 (permutation): `B = 1 + ceil(ssp / log2(n * ell)) = 1 + ceil(ssp / log2(n^2 * m))`

### `lib.rs` additions needed
```
pub mod bcot;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;
```

---

## Key Risks and Decisions

### Risk 1: Correctness of Construction 2's XOR combination

The paper computes `C_A XOR C_B = y * (D_A XOR D_B)` where D_A, D_B are the two parties' global deltas. The existing code uses **two separate deltas** (`delta_a` for A, `delta_b` for B). Pi_LeakyTensor introduces a **combined delta D = D_A XOR D_B** for the output triples. The `TensorFpreGen`/`TensorFpreEval` interface as currently defined uses `delta_a` only for the garbler's shares. This may require either:
  - (a) Outputting triples with delta = D_A XOR D_B (new interface field), or
  - (b) Normalizing so the bucketing combiner's output uses delta_a for all shares

This is the most important design decision before coding begins. [ASSUMED: Option (b) is standard in MASCOT-style compilers but needs verification against the paper's Construction 3 output interface.]

### Risk 2: The F_eq consistency check

Pi_LeakyTensor's consistency check (step after revealing lsb(S_1) XOR lsb(S_2)) is described as using ideal `F_eq`. In a real implementation this is either:
- A commitment-then-reveal (for malicious security)
- A plain XOR reveal (for the benchmark / semi-honest model)

Since the existing ideal `TensorFpre` is explicitly "insecure" (comment on line 1 of `auth_tensor_fpre.rs`), the consistency check can be implemented as a plain equality assertion for now. Flag this in code with a TODO for future hardening.

### Risk 3: The "selective failure" leakiness

Construction 2 is explicitly leaky: a malicious garbler can learn bits of the honest party's input by malforming the GGM correction ciphertexts. This is expected and is the whole reason for bucketing in Construction 3. The leaky triple combiner must be correct even when the individual triples have the leakage property. The combiner's XOR combination is what eliminates the leakage; no special handling is needed in the TensorGb/TensorEv code.

### Risk 4: The `correlated_auth_bits` semantics

In `auth_tensor_fpre.rs`, `correlated_auth_bits[j*n + i]` holds an authenticated bit for `alpha_i AND beta_j`. This is the `x XOR y` product in the triple, produced by Pi_LeakyTensor's combination of the two TensorGb/TensorEv runs. Getting the XOR combination formula right (matching `C_A XOR C_B`) is essential for the correctness of `garble_final()` in `auth_tensor_gen.rs`.

### Risk 5: ChaCha vs deterministic test reproducibility

The ideal `TensorFpre` uses `ChaCha12Rng::seed_from_u64` for deterministic tests. The new protocol is interactive and determinism depends on both parties' randomness. Tests should use seeded RNGs for both party instances to remain reproducible.

---

## Hash / PRG Used in the GGM Tree

The GGM tree uses `FixedKeyAes::tccr(tweak, block)` from `src/aes.rs`:

`tccr(tweak, x) = π(π(x) XOR tweak) XOR π(x)` where π is AES-128 with the fixed key `[69, 42, 69, 42, ...]` [VERIFIED: aes.rs lines 39–47].

This is the tweakable circular correlation-robust hash from [Guo et al. 2019], which provides the TCCR security property. The fixed key is arbitrarily chosen (not derived from any secret). [VERIFIED: aes.rs line 10].

Level correction ciphertexts: at level i, the garbler produces `(G_{i,even}, G_{i,odd})` = `(XOR of tccr(1, seed_j) for even j, XOR of tccr(0, seed_j) for odd j)` plus a correction derived from the secret key/label for that level. [VERIFIED: tensor_ops.rs lines 62–72].

For the outer-product expansion: `H(seed_i, j) = tccr(seeds.len() * j + i, seeds[i])`, i.e., a unique tweak per (row, column) cell. [VERIFIED: tensor_ops.rs line 105].

---

## Tensor / Matrix Types

`BlockMatrix` is `TypedMatrix<Block>` with column-major storage (`flat_index(i,j) = j*rows + i`) [VERIFIED: matrix.rs line 56]. Outer product Z[i,j] = x_i AND y_j is stored at `BlockMatrix[(i,j)]`.

The `x XOR y` tensor product (outer product over GF(2)) is never computed explicitly; it emerges from the GGM construction: Z_gb XOR Z_ev = x_vector (outer product) y_vector in label space. Each element of the matrix is a `Block` (128-bit label), where `label == label_0` means 0-bit and `label == label_0 XOR delta` means 1-bit. [VERIFIED: lib.rs test `verify_tensor_output`, lines 107–120].

---

## MPZ Reference Analysis

The MPZ `ot-core` library provides:
- `COTSender<T>` / `COTReceiver<T,U>` traits (async, message-based)
- `IdealCOTSender` / `IdealCOTReceiver` with `FlushMsg` protocol (sender sends `{delta, batches: [{count, keys}]}` to receiver)
- The receiver reconstructs `msgs[i] = keys[i] XOR choice[i]*delta`

[VERIFIED: references/mpz-dev/crates/ot-core/src/ideal/cot.rs]

MPZ uses async/tokio and `serio` for I/O. **Do not import MPZ as a dependency** — its trait system (`Context`, `Flush`, async executor requirements) is heavyweight and would require adding ~15 new crate dependencies. Instead, implement the same `IdealCOT` logic directly (the core logic is trivial: `msgs[i] = keys[i] XOR choice[i]*delta`). The MPZ code confirms the correctness of this approach.

MPZ does **not** contain a tensor-specific preprocessing protocol. The garble crates (`garble`, `garble-core`) implement standard 2-input Boolean gate garbling, which is orthogonal to the tensor gate construction here.

---

## Benchmark Framework Integration

### How existing benchmarks work

1. `criterion_group!` macro groups related benchmarks
2. Each benchmark uses `b.to_async(&*RT).iter_batched(setup_fn, benchmark_fn, BatchSize::SmallInput)`
3. The `setup_fn` creates gen/eval/network objects; the `benchmark_fn` runs the actual protocol
4. Communication is modeled by computing `total_bytes` upfront and calling `network.send_size_with_metrics(total_bytes).await`

### Adding preprocessing benchmarks

Add a new `bench_group!("preprocessing")` following the same pattern. The new setup function:

```rust
fn setup_leaky_tensor_pre(n: usize, m: usize, bucket_b: usize) -> (LeakyTensorPreGen, LeakyTensorPreEval)
fn setup_auth_tensor_pre(n: usize, m: usize) -> (TensorFpreGen, TensorFpreEval)
```

Communication bytes for preprocessing:
- bCOT phase: 2*(n+m+n*m)*16 bytes (two rounds of n+m input bits + n*m product bits, each a Block)
- Two TensorGb/TensorEv exchanges: same formula as existing `levels_bytes + cts_bytes` calculation
- Bucketing: no additional communication (local computation)

Add these to `criterion_group!(benches, ..., bench_preprocessing)` in `benchmarks.rs`.

---

## Common Pitfalls

### Pitfall 1: Column-major vs row-major indexing

The `correlated_auth_bits` vector uses column-major order `j*n + i` [VERIFIED: auth_tensor_fpre.rs line 182, auth_tensor_gen.rs line 182]. This must be respected when constructing the output of Pi_LeakyTensor. Swapping to row-major would produce wrong results silently.

### Pitfall 2: Little-endian label bit convention

All vector-to-integer conversions are little-endian: index 0 = LSB [VERIFIED: tensor_eval.rs comment line 107: "Endianness note (little-endian vectors): consume bit at position n-i-1"]. The GGM tree processes bits MSB-first (x[n-1] first) but the output index mapping is little-endian. Construction 2's outer product uses the same convention.

### Pitfall 3: LSB of Key must be 0

`Key::adjust` explicitly calls `set_lsb(false)` [VERIFIED: keys.rs line 38]. Random keys generated for COT must also have their LSB zeroed before use as `Key` values. The `build_share` function in `sharing.rs` handles this correctly; any direct key generation for bCOT must do the same.

### Pitfall 4: delta's LSB is always 1

`Delta::new()` sets `lsb(true)` [VERIFIED: delta.rs line 13]. Any `Delta` generated by either party must respect this. The XOR-combined delta `D = delta_a XOR delta_b` may or may not have LSB=1 depending on the parties' values — this requires a protocol-level normalization step or a choice of which party's delta is "canonical."

### Pitfall 5: The `garble_final` XOR combination

`AuthTensorGen::garble_final` (line 194) computes:
```
first_half_out[i,j] ^= second_half_out[j,i] ^ correlated_share[j*n+i]
```
where `correlated_share` is `key XOR bit*delta` (the garbler's authenticated share). This exact formula must be what Pi_LeakyTensor produces in its output's `correlated_auth_bit_shares`. Getting the XOR structure of C_A XOR C_B wrong here will produce garbling errors that are extremely hard to debug.

### Pitfall 6: Bucketing requires B *independent* leaky triples

The bucket combiner XORs B triples element-wise. Independence requires B separate executions of Pi_LeakyTensor with fresh randomness each time. Using the same bCOT seed or the same GGM seeds across buckets would destroy security. Use separate `ChaCha12Rng::seed_from_u64(seed + bucket_index)` for each execution in testing.

---

## Environment Availability

Step 2.6: SKIPPED — no external tool dependencies beyond Cargo. All required cryptographic primitives (`aes`, `rand_chacha`, `once_cell`, `tokio`) are already in `Cargo.toml`.

---

## Validation Architecture

### Test framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `criterion` for benchmarks |
| Config file | None (standard `cargo test`) |
| Quick run | `cargo test` |
| Full suite + benchmarks | `cargo bench` |

### Phase requirements to test map

| Behavior | Test Type | Command |
|----------|-----------|---------|
| IdealBCot produces correct correlation (msg[b] = key XOR b*delta) | unit | `cargo test bcot::tests` |
| Pi_LeakyTensor produces valid AuthBitShares (verify() passes) | unit | `cargo test leaky_tensor_pre::tests` |
| Pi_LeakyTensor output correlated_bits = x AND y (with high probability) | unit | `cargo test leaky_tensor_pre::tests::test_correctness` |
| Pi_aTensor output matches TensorFpreGen/Eval interface | integration | `cargo test auth_tensor_pre::tests` |
| Full online phase still passes when fed real preprocessing output | integration | `cargo test` (existing test_auth_tensor_product re-used) |
| Bucketing formula B is computed correctly for various (n,m) | unit | `cargo test auth_tensor_pre::tests::test_bucket_size` |
| Preprocessing benchmark runs without panic | smoke | `cargo bench -- preprocessing 2>/dev/null` |

### Wave 0 gaps

- [ ] `src/bcot.rs` — covers bCOT correctness
- [ ] `src/leaky_tensor_pre.rs` + its `#[cfg(test)] mod tests` — covers Construction 2
- [ ] `src/auth_tensor_pre.rs` + its `#[cfg(test)] mod tests` — covers Construction 3
- [ ] `benches/benchmarks.rs` addition — covers preprocessing benchmark

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The combined delta D = delta_a XOR delta_b is used as the MAC key for the bucketing output; normalizing to delta_a is safe | COT Strategy / Risk 1 | Wrong MAC invariant breaks garble_final; must verify against paper's Construction 3 output spec |
| A2 | The F_eq consistency check can be a plain assert_eq! (no commitment) for the Phase 1 implementation | Key Risks / Risk 2 | Only a security issue, not correctness; acceptable for Phase 1 |
| A3 | bCOT communication cost can be approximated as 2*(n+m+n*m)*16 bytes for benchmark purposes | Benchmark Integration | Underestimates real Ferret/IKNP overhead; affects communication benchmarks only, not computation |

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)
- `src/tensor_ops.rs` — GGM tree implementation, gen/eval functions
- `src/auth_tensor_fpre.rs` — exact interface to replace
- `src/auth_tensor_gen.rs` / `src/auth_tensor_eval.rs` — online phase consumers
- `src/sharing.rs` — AuthBitShare, build_share, Key/Mac invariant
- `src/aes.rs` — TCCR hash function
- `src/delta.rs` — Delta type and LSB constraint
- `src/lib.rs` — CSP=128, SSP=40 constants

### Secondary (MEDIUM confidence — reference code)
- `references/mpz-dev/crates/ot-core/src/ideal/cot.rs` — confirms ideal COT structure and msg format
- `references/mpz-dev/crates/ot-core/src/cot.rs` — COT sender/receiver trait signatures

### Tertiary (LOW confidence — protocol spec mapped to code)
- Protocol specification in task description (Appendix F) — mapped to code by inspection [ASSUMED where paper notation is ambiguous]

---

## Metadata

**Confidence breakdown:**
- Existing code analysis: HIGH — all files read directly, all key functions traced
- COT gap identification: HIGH — grep confirms no OT/COT anywhere in `src/`
- Protocol-to-code mapping: MEDIUM — the mathematical spec mapped to code by reasoning; some details (especially the combined-delta question) need verification against the full paper
- Bucket size formula: HIGH — verified against SSP=40 in lib.rs
- Benchmark integration: HIGH — pattern is identical to existing benchmarks

**Research date:** 2026-04-19
**Valid until:** 2026-05-19 (codebase is stable; no external dependencies to expire)

---

## RESEARCH COMPLETE
