# Phase 1: Uncompressed Preprocessing Protocol - Pattern Map

**Mapped:** 2026-04-19
**Files analyzed:** 3 target files (src/auth_tensor_fpre.rs modified; src/cot.rs or src/ot.rs new; benches/benchmarks.rs modified)
**Analogs found:** 3 / 3

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/auth_tensor_fpre.rs` | service (preprocessing) | request-response (two-party) | `src/auth_tensor_fpre.rs` (current ideal) + `src/tensor_pre.rs` | exact (self-replacement) |
| `src/cot.rs` or `src/ot.rs` | utility (crypto primitive) | request-response (interactive) | `src/tensor_ops.rs` (GGM tree) + `src/aes.rs` | partial-match (same crypto layer) |
| `src/leaky_tensor.rs` | service (triple generation) | request-response (two-party) | `src/auth_tensor_fpre.rs` + `src/sharing.rs` | role-match |
| `benches/benchmarks.rs` | test/benchmark | batch | `benches/benchmarks.rs` (current) | exact |

---

## Pattern Assignments

### `src/auth_tensor_fpre.rs` (service, request-response — replace ideal with real two-party)

**Primary analog:** `src/auth_tensor_fpre.rs` (current file — understand what the ideal does, then replace it)
**Secondary analog:** `src/tensor_pre.rs` (shows how a preprocessing struct splits into Gen/Eval halves)

**Struct definition pattern** (`src/auth_tensor_fpre.rs` lines 8–47):
```rust
pub struct TensorFpre {
    rng: ChaCha12Rng,
    n: usize,
    m: usize,
    chunking_factor: usize,
    delta_a: Delta,
    delta_b: Delta,
    x_labels: Vec<InputSharing>,
    y_labels: Vec<InputSharing>,
    alpha_auth_bits: Vec<AuthBit>,
    beta_auth_bits: Vec<AuthBit>,
    correlated_auth_bits: Vec<AuthBit>,
    gamma_auth_bits: Vec<AuthBit>,
}

pub struct TensorFpreGen {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub delta_a: Delta,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}

pub struct TensorFpreEval {
    pub n: usize,
    pub m: usize,
    pub chunking_factor: usize,
    pub delta_b: Delta,
    pub alpha_labels: Vec<Block>,
    pub beta_labels: Vec<Block>,
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    pub gamma_auth_bit_shares: Vec<AuthBitShare>,
}
```

**Constructor pattern** (`src/auth_tensor_fpre.rs` lines 51–91):
```rust
pub fn new(seed: u64, n: usize, m: usize, chunking_factor: usize) -> Self {
    let mut rng = ChaCha12Rng::seed_from_u64(seed);
    let delta_a = Delta::random(&mut rng);
    let delta_b = Delta::random(&mut rng);
    Self {
        rng, n, m, chunking_factor, delta_a, delta_b,
        x_labels:              Vec::with_capacity(n),
        y_labels:              Vec::with_capacity(m),
        alpha_auth_bits:       Vec::with_capacity(n),
        beta_auth_bits:        Vec::with_capacity(m),
        correlated_auth_bits:  Vec::with_capacity(n * m),
        gamma_auth_bits:       Vec::with_capacity(n * m),
    }
}
```

**Auth-bit generation pattern** (`src/auth_tensor_fpre.rs` lines 94–113):
```rust
pub fn gen_auth_bit(&mut self, x: bool) -> AuthBit {
    let a = self.rng.random_bool(0.5);
    let b = x ^ a;
    let a_share = build_share(&mut self.rng, a, &self.delta_b);
    let b_share = build_share(&mut self.rng, b, &self.delta_a);
    AuthBit {
        gen_share:  AuthBitShare { key: b_share.key, mac: a_share.mac, value: a },
        eval_share: AuthBitShare { key: a_share.key, mac: b_share.mac, value: b },
    }
}
```
The real implementation must produce the same `AuthBit` layout — only the mechanism for choosing `a` and generating the (key, mac) pairs changes: instead of local `rng`, party A's key comes from OT/COT and party B's MAC is derived from it.

**`into_gen_eval` split pattern** (`src/auth_tensor_fpre.rs` lines 191–216):
```rust
pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {
    (TensorFpreGen {
        n: self.n, m: self.m, chunking_factor: self.chunking_factor,
        delta_a: self.delta_a,
        alpha_labels: self.x_labels.iter().map(|s| s.gen_share).collect(),
        beta_labels:  self.y_labels.iter().map(|s| s.gen_share).collect(),
        alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|b| b.gen_share).collect(),
        beta_auth_bit_shares:  self.beta_auth_bits.iter().map(|b| b.gen_share).collect(),
        correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|b| b.gen_share).collect(),
        gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|b| b.gen_share).collect(),
    }, TensorFpreEval {
        n: self.n, m: self.m, chunking_factor: self.chunking_factor,
        delta_b: self.delta_b,
        alpha_labels: self.x_labels.iter().map(|s| s.eval_share).collect(),
        beta_labels:  self.y_labels.iter().map(|s| s.eval_share).collect(),
        alpha_auth_bit_shares: self.alpha_auth_bits.iter().map(|b| b.eval_share).collect(),
        beta_auth_bit_shares:  self.beta_auth_bits.iter().map(|b| b.eval_share).collect(),
        correlated_auth_bit_shares: self.correlated_auth_bits.iter().map(|b| b.eval_share).collect(),
        gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(|b| b.eval_share).collect(),
    })
}
```
The public API (`TensorFpreGen` / `TensorFpreEval` field names and types) must remain identical so that `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` need no changes.

**AuthBitShare structure** (`src/sharing.rs` lines 28–50):
```rust
#[derive(Debug, Clone, Default, Copy)]
pub struct AuthBitShare {
    pub key: Key,
    pub mac: Mac,
    pub value: bool,
}

impl AuthBitShare {
    pub fn bit(&self) -> bool { self.value }
    pub fn verify(&self, delta: &Delta) {
        let want: Mac = self.key.auth(self.bit(), delta);
        assert_eq!(self.mac, want, "MAC mismatch in share");
    }
}
```

**Key.auth relationship** (`src/keys.rs` lines 52–54):
```rust
pub fn auth(&self, bit: bool, delta: &Delta) -> Mac {
    Mac::new(self.0 ^ if bit { delta.as_block() } else { &Block::ZERO })
}
```
MAC = Key XOR (bit ? Delta : 0). This is the BDOZ/SPDZ-style linear MAC. Any COT-based generation must produce (key_A, mac_B) satisfying mac_B = key_A XOR (b * delta_A).

**build_share helper** (`src/sharing.rs` lines 105–109):
```rust
pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
    let key: Key = Key::from(rng.random::<[u8; 16]>());
    let mac: Mac = key.auth(bit, delta);
    AuthBitShare { key, mac, value: bit }
}
```
In the real protocol `build_share` is replaced by COT outputs: party holding delta generates random keys; the other party receives mac = key XOR (b * delta) via a single COT call.

**Correlated auth bit generation (column-major indexing)** (`src/auth_tensor_fpre.rs` lines 175–188):
```rust
// column-major: gamma[j * n + i] = alpha[i] AND beta[j]
for j in 0..self.m {
    for i in 0..self.n {
        let g = self.rng.random_bool(0.5);
        let gamma_auth_bit = self.gen_auth_bit(g);
        self.gamma_auth_bits.push(gamma_auth_bit);
        let alpha = &self.alpha_auth_bits[i];
        let beta  = &self.beta_auth_bits[j];
        let alpha_beta = self.gen_auth_bit(alpha.full_bit() && beta.full_bit());
        self.correlated_auth_bits.push(alpha_beta);
    }
}
```
Column-major indexing `j * n + i` is consumed identically in `auth_tensor_gen.rs` (line 182) and `auth_tensor_eval.rs` (line 259). Do not change the indexing convention.

---

### `src/cot.rs` / `src/ot.rs` (utility, request-response — new file)

**No direct analog exists.** The closest crypto-layer analog is:

**GGM tree construction (garbler side)** (`src/tensor_ops.rs` lines 9–85):
```rust
pub fn gen_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    cipher: &FixedKeyAes,
    delta: Delta,
) -> (Vec<Block>, Vec<(Block, Block)>) {
    // Base case: derive two seeds from the top label using TCCR
    if x[n-1].lsb() {
        seeds[0] = cipher.tccr(Block::new((0u128).to_be_bytes()), x[n-1]);
        seeds[1] = cipher.tccr(Block::new((0u128).to_be_bytes()), x[n-1] ^ delta);
    } else {
        seeds[1] = cipher.tccr(Block::new((0u128).to_be_bytes()), x[n-1]);
        seeds[0] = cipher.tccr(Block::new((0u128).to_be_bytes()), x[n-1] ^ delta);
    }
    // Expand level-by-level; accumulate XOR-sums of even/odd children
    for i in 1..n {
        for j in (0..(1 << i)).rev() {
            seeds[j * 2 + 1] = cipher.tccr(Block::from(0u128), seeds[j]);
            seeds[j * 2]     = cipher.tccr(Block::from(1u128), seeds[j]);
            evens ^= seeds[j * 2];
            odds  ^= seeds[j * 2 + 1];
        }
        // Add key contributions, push (evens, odds) as the correction ciphertext for this level
        evens ^= cipher.tccr(Block::from(0u128), key0);
        odds  ^= cipher.tccr(Block::from(1u128), key1);
        odd_evens.push((evens, odds));
    }
}
```

**GGM tree evaluation (evaluator side)** (`src/tensor_eval.rs` lines 61–130 — identical copy also in `src/auth_tensor_eval.rs` lines 66–135):
```rust
fn eval_populate_seeds_mem_optimized(
    x: &MatrixViewRef<Block>,
    levels: Vec<(Block, Block)>,
    _clear_value: &usize,
    cipher: &FixedKeyAes,
) -> Vec<Block> {
    // Evaluator knows one leaf; missing path reconstructed from correction values
    seeds[!x[n-1].lsb() as usize] = cipher.tccr(Block::new((0u128).to_be_bytes()), x[n-1]);
    let mut missing = x[n-1].lsb() as usize;
    for i in 1..n {
        // ... expand known nodes, XOR partial sums, recover sibling of missing node
        let computed_seed = cipher.tccr(tweak, x[n-i-1]) ^ mask;
        seeds[sibling_index] = computed_seed;
    }
}
```

**TCCR hash primitive** (`src/aes.rs` lines 38–47):
```rust
pub fn tccr(&self, tweak: Block, block: Block) -> Block {
    let mut h1 = block;
    self.aes.encrypt_block(h1.as_array_mut());
    let mut h2 = h1 ^ tweak;
    self.aes.encrypt_block(h2.as_array_mut());
    h1 ^ h2
}
```
COT for a single bit b with sender's key K: sender sends (K, K XOR delta). The GGM tree above is the n-bit COT sender's procedure. For standard 1-of-2 OT on a bit, the same TCCR can serve as the random oracle.

**Delta type** (`src/delta.rs` lines 6–56):
```rust
pub struct Delta(Block);
impl Delta {
    pub fn new(mut value: Block) -> Self { value.set_lsb(true); Self(value) }
    pub fn random<R: Rng>(rng: &mut R) -> Self { Self::new(Block::from(rng.random::<[u8; 16]>())) }
    pub fn mul_bool(self, value: bool) -> Block { if value { self.0 } else { Block::ZERO } }
    pub fn as_block(&self) -> &Block { &self.0 }
}
```
Delta always has LSB = 1 (pointer bit). COT correlation is: mac = key XOR (b * delta).

**Imports pattern for crypto utils** (`src/tensor_ops.rs` lines 1–6):
```rust
use crate::{
    aes::FixedKeyAes,
    block::Block,
    delta::Delta,
    matrix::{MatrixViewMut, MatrixViewRef},
};
```

---

### `src/leaky_tensor.rs` (service, request-response — new file)

**Primary analog:** `src/auth_tensor_fpre.rs` (role: preprocessing service that produces `TensorFpreGen`/`TensorFpreEval`)
**Secondary analog:** `src/sharing.rs` (AuthBit / AuthBitShare types reused as-is)

**Key insight:** `leaky_tensor.rs` is a real two-party replacement for `TensorFpre::generate_with_input_values`. It must produce the same output types (`Vec<AuthBitShare>` for each party, `Vec<Block>` for labels) that downstream `AuthTensorGen::new_from_fpre_gen` consumes.

**Downstream consumer of fpre output** (`src/auth_tensor_gen.rs` lines 54–70):
```rust
pub fn new_from_fpre_gen(fpre_gen: TensorFpreGen) -> Self {
    Self {
        cipher: &(*FIXED_KEY_AES),
        n: fpre_gen.n, m: fpre_gen.m, chunking_factor: fpre_gen.chunking_factor,
        delta_a: fpre_gen.delta_a,
        x_labels: fpre_gen.alpha_labels,
        y_labels: fpre_gen.beta_labels,
        alpha_auth_bit_shares: fpre_gen.alpha_auth_bit_shares,
        beta_auth_bit_shares:  fpre_gen.beta_auth_bit_shares,
        correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
        gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
        first_half_out:  BlockMatrix::new(fpre_gen.n, fpre_gen.m),
        second_half_out: BlockMatrix::new(fpre_gen.m, fpre_gen.n),
    }
}
```

**Downstream consumer (evaluator side)** (`src/auth_tensor_eval.rs` lines 48–64):
```rust
pub fn new_from_fpre_eval(fpre_eval: TensorFpreEval) -> Self {
    Self {
        cipher: &(*FIXED_KEY_AES),
        chunking_factor: fpre_eval.chunking_factor,
        n: fpre_eval.n, m: fpre_eval.m,
        delta_b: fpre_eval.delta_b,
        x_labels: fpre_eval.alpha_labels,
        y_labels: fpre_eval.beta_labels,
        alpha_auth_bit_shares: fpre_eval.alpha_auth_bit_shares,
        beta_auth_bit_shares:  fpre_eval.beta_auth_bit_shares,
        correlated_auth_bit_shares: fpre_eval.correlated_auth_bit_shares,
        gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,
        first_half_out:  BlockMatrix::new(fpre_eval.n, fpre_eval.m),
        second_half_out: BlockMatrix::new(fpre_eval.m, fpre_eval.n),
    }
}
```

**How gen_share/eval_share are used in garble_final** (`src/auth_tensor_gen.rs` lines 179–199):
```rust
pub fn garble_final(&mut self) {
    for i in 0..self.n {
        for j in 0..self.m {
            let correlated_share = if self.correlated_auth_bit_shares[j * self.n + i].bit() {
                self.delta_a.as_block() ^ self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
            } else {
                *self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
            };
            self.first_half_out[(i, j)] ^= self.second_half_out[(j, i)] ^ correlated_share;
        }
    }
}
```
Garbler uses `key` field of `AuthBitShare` (not `mac`). Evaluator uses `mac` field (see `auth_tensor_eval.rs` line 258–260). The COT-based generation must correctly populate both fields according to the BDOZ relation.

**How mac is used in evaluate_final** (`src/auth_tensor_eval.rs` lines 255–263):
```rust
pub fn evaluate_final(&mut self) {
    for i in 0..self.n {
        for j in 0..self.m {
            self.first_half_out[(i, j)] ^=
                self.second_half_out[(j, i)] ^
                self.correlated_auth_bit_shares[j * self.n + i].mac.as_block();
        }
    }
}
```

---

### `benches/benchmarks.rs` (benchmark, batch — add preprocessing benchmarks)

**Analog:** `benches/benchmarks.rs` (current file — copy its structure exactly)

**Imports pattern** (`benches/benchmarks.rs` lines 1–27):
```rust
use std::time::Duration;
use std::mem::size_of;

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, BatchSize};

mod network_simulator;
use network_simulator::SimpleNetworkSimulator;

use authenticated_tensor_garbling::{
    block::Block,
    auth_tensor_gen::AuthTensorGen,
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,
    // add new imports here, e.g.:
    // cot::CotSender, cot::CotReceiver,
};

use once_cell::sync::Lazy;
static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap()
});
```

**Benchmark parameters array** (`benches/benchmarks.rs` lines 29–39):
```rust
const BENCHMARK_PARAMS: &[(usize, usize)] = &[
    (4, 4), (8, 8), (16, 16), (24, 24), (32, 32), (48, 48),
    (64, 64), (96, 96), (128, 128),
];
```

**Setup function pattern (fpre)** (`benches/benchmarks.rs` lines 69–82):
```rust
fn setup_auth_gen(n: usize, m: usize, chunking_factor: usize) -> AuthTensorGen {
    let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
    fpre.generate_with_input_values(X_INPUT, Y_INPUT);
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}
```
New preprocessing benchmarks follow the same pattern — replace `TensorFpre::new` with the real two-party constructor, and measure both parties' wall time separately or combined.

**Async benchmark with network simulation** (`benches/benchmarks.rs` lines 189–213):
```rust
group.bench_with_input(
    BenchmarkId::new("1", format!("{}x{}", n, m)),
    &(n, m),
    |b, &(n, m)| {
        b.to_async(&*RT)
        .iter_batched(
            || (
                setup_auth_gen(n, m, chunking_factor),
                setup_auth_eval(n, m, chunking_factor),
                SimpleNetworkSimulator::new(100.0, 0)
            ),
            |(mut generator, mut evaluator, network)| async move {
                let (first_levels, first_cts) = generator.garble_first_half();
                let (second_levels, second_cts) = generator.garble_second_half();
                generator.garble_final();
                network.send_size_with_metrics(total_bytes).await;
                evaluator.evaluate_first_half(first_levels, first_cts);
                evaluator.evaluate_second_half(second_levels, second_cts);
                evaluator.evaluate_final();
            },
            BatchSize::SmallInput
        )
    },
);
```
New preprocessing benchmarks should use the same `iter_batched` + `to_async` structure. The `setup_*` closure in `iter_batched`'s first argument is the right place to call the real preprocessing — it runs outside the timed region when `BatchSize::SmallInput` is used (note: `SmallInput` does NOT exclude setup from timing in all Criterion versions; verify and use `iter_custom` if precise isolation is needed).

**Throughput reporting pattern** (`benches/benchmarks.rs` lines 180–187):
```rust
let levels_bytes_1: usize = first_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
let cts_bytes_1: usize    = first_cts.iter().map(|row| row.len() * block_sz).sum();
let levels_bytes_2: usize = second_levels.iter().map(|row| row.len() * 2 * block_sz).sum();
let cts_bytes_2: usize    = second_cts.iter().map(|row| row.len() * block_sz).sum();
let total_bytes = levels_bytes_1 + cts_bytes_1 + levels_bytes_2 + cts_bytes_2;
group.throughput(Throughput::Bytes(total_bytes as u64));
```
For preprocessing benchmarks, report `Throughput::Elements(n_auth_bits as u64)` where `n_auth_bits = n + m + n*m + n*m` (alpha + beta + correlated + gamma auth bits).

**criterion_group! registration** (`benches/benchmarks.rs` lines 745–754):
```rust
criterion_group!(
    benches,
    bench_4x4_runtime_with_networking,
    // ... add new preprocessing benchmark fns here
);
criterion_main!(benches);
```

---

## Shared Patterns

### Block — the fundamental 128-bit type
**Source:** `src/block.rs` lines 15–17
**Apply to:** All new files
```rust
#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, Serialize, Deserialize, Pod, Zeroable)]
pub struct Block([u8; 16]);
```
XOR is the core operation: `block_a ^ block_b`. Use `Block::random(&mut rng)` for random generation. Use `.lsb()` / `.set_lsb()` to read/write the pointer bit.

### Delta — the global OT correlation offset
**Source:** `src/delta.rs` lines 6–56
**Apply to:** `src/auth_tensor_fpre.rs`, `src/cot.rs`, `src/leaky_tensor.rs`
```rust
pub struct Delta(Block);
impl Delta {
    pub fn new(mut value: Block) -> Self { value.set_lsb(true); Self(value) }
    pub fn random<R: Rng>(rng: &mut R) -> Self { ... }
    pub fn as_block(&self) -> &Block { &self.0 }
    pub fn mul_bool(self, value: bool) -> Block { if value { self.0 } else { Block::ZERO } }
}
```
Delta always has LSB = 1. `delta_a` is the garbler's global key; `delta_b` is the evaluator's global key. The real protocol: garbler holds `delta_a` secret; evaluator learns `delta_a * b` = mac for each bit `b` via COT.

### AuthBitShare — the wire label for authenticated bits
**Source:** `src/sharing.rs` lines 28–109
**Apply to:** `src/auth_tensor_fpre.rs`, `src/leaky_tensor.rs`

The invariant that all generated shares must satisfy:
```
eval_share.mac == gen_share.key.auth(eval_share.bit(), &delta_a)
// i.e., mac_B = key_A XOR (b * delta_A)
```
Verified with `AuthBitShare::verify(&delta)`. Any new COT-based generation code must uphold this invariant.

`Add` is implemented as XOR on all fields (`key`, `mac`, `value`), enabling additive secret sharing.

### FixedKeyAes / TCCR
**Source:** `src/aes.rs` lines 14–47
**Apply to:** `src/cot.rs`, `src/leaky_tensor.rs`
```rust
pub static FIXED_KEY_AES: Lazy<FixedKeyAes> = Lazy::new(|| FixedKeyAes {
    aes: Aes128Enc::new_from_slice(&FIXED_KEY).unwrap(),
});
// Usage:
cipher.tccr(tweak, block)  // -> Block
```
All symmetric-key operations use the global `FIXED_KEY_AES` singleton. Import with `use crate::aes::{FixedKeyAes, FIXED_KEY_AES};`.

### RNG convention
**Source:** `src/auth_tensor_fpre.rs` lines 4–5, 52
**Apply to:** All new files
```rust
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
// seeded:
let mut rng = ChaCha12Rng::seed_from_u64(seed);
// or thread-local:
let mut rng = rand::rng();
```

### Column-major indexing for n×m auth bit arrays
**Source:** `src/auth_tensor_fpre.rs` lines 175–188, `src/auth_tensor_gen.rs` line 182, `src/auth_tensor_eval.rs` line 259
**Apply to:** `src/auth_tensor_fpre.rs`, `src/leaky_tensor.rs`

Index = `j * n + i` where `j` is the column (beta/y index) and `i` is the row (alpha/x index). This convention is used consistently in fpre generation AND in garble_final/evaluate_final. Do not change it.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `src/cot.rs` / `src/ot.rs` | utility | request-response (interactive) | No interactive two-party OT protocol exists in the codebase. The GGM tree in `tensor_ops.rs` is the sender-side of a multi-bit COT but is not a standalone OT module. New file must be written from scratch using the TCCR primitive from `src/aes.rs`. |

---

## Key Observations for the Planner

1. **Public API must not change.** `TensorFpreGen` and `TensorFpreEval` struct field names and types are consumed verbatim by `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval`. Any real protocol must populate these exact fields.

2. **The ideal `TensorFpre` struct can be kept or deleted.** The simplest migration is to replace `generate_with_input_values` with a function that runs the real two-party protocol and fills the same `alpha_auth_bits`, `beta_auth_bits`, etc. vectors.

3. **`gen_auth_bit` is the choke point.** It currently uses local RNG. The real protocol replaces it with a COT call: sender (garbler) picks a random key K; receiver (evaluator) gets mac = K XOR (b * delta) via the OT. `build_share` in `sharing.rs` shows the algebraic relationship.

4. **GGM tree code is duplicated** between `src/tensor_eval.rs` (lines 61–130) and `src/auth_tensor_eval.rs` (lines 66–135). These are byte-for-byte identical. A `cot.rs` module could deduplicate this.

5. **Benchmark setup functions** (`setup_auth_gen`, `setup_auth_eval`) are the correct insertion point for calling the real preprocessing instead of `TensorFpre`. New preprocessing benchmarks should add analogous `setup_real_preprocessing_gen/eval` functions.

---

## Metadata

**Analog search scope:** `src/`, `benches/`
**Files scanned:** 13 (all source files + both bench files)
**Pattern extraction date:** 2026-04-19
