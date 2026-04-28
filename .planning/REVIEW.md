---
phase: codebase-review
reviewed: 2026-04-25T00:00:00Z
depth: deep
files_reviewed: 23
files_reviewed_list:
  - src/aes.rs
  - src/block.rs
  - src/lib.rs
  - src/delta.rs
  - src/keys.rs
  - src/macs.rs
  - src/sharing.rs
  - src/matrix.rs
  - src/tensor_ops.rs
  - src/tensor_pre.rs
  - src/tensor_gen.rs
  - src/tensor_eval.rs
  - src/tensor_macro.rs
  - src/auth_tensor_fpre.rs
  - src/auth_tensor_gen.rs
  - src/auth_tensor_eval.rs
  - src/auth_tensor_pre.rs
  - src/preprocessing.rs
  - src/leaky_tensor_pre.rs
  - src/bcot.rs
  - src/feq.rs
  - src/online.rs
  - benches/benchmarks.rs
  - benches/network_simulator.rs
findings:
  critical: 3
  warning: 17
  info: 7
  total: 27
status: issues_found
---

# Codebase Code Review Report

**Reviewed:** 2026-04-25
**Depth:** deep
**Files Reviewed:** 23
**Status:** issues_found

## Summary

This is a Rust implementation of an authenticated tensor garbling protocol for two-party computation. The codebase is research-quality, structurally coherent, and generally follows good Rust idioms. However, deep cross-file analysis surfaces three critical issues: a duplicated constant definition that will diverge silently, an `unsafe transmute` that bypasses a security-relevant struct invariant, and a dead-code constructor whose subtle initialization differences create a latent correctness trap. Seventeen warnings cover code duplication (four files share a near-identical inner loop), a transposed `Display` implementation, hardcoded RNG seeds that defeat benchmark freshness, fragile partial initialization via empty-`Vec` sentinel fields, and excessive `pub` exposure of internal accumulator fields. Seven informational items round out the quality picture.

---

## Critical Issues

### CR-01: `MAC_ZERO` / `MAC_ONE` duplicated across two modules — silent divergence risk

**File:** `src/macs.rs:7-13` and `src/lib.rs:41-48`

**Issue:** Both files define `MAC_ZERO` and `MAC_ONE` with identical byte literals today. Because these are separate `const` definitions rather than re-exports, any future change to one copy will not propagate to the other. Code in `lib.rs` that uses `MAC_ZERO`/`MAC_ONE` and code in the rest of the crate that uses `macs::MAC_ZERO` will silently diverge.

```rust
// src/macs.rs
pub const MAC_ZERO: Mac = Mac(Block::new([0u8; 16]));
pub const MAC_ONE:  Mac = Mac(Block::new([255u8; 16]));

// src/lib.rs  (duplicate — must be removed)
const MAC_ZERO: Mac = Mac(Block::new([0u8; 16]));
const MAC_ONE:  Mac = Mac(Block::new([255u8; 16]));
```

**Fix:** Remove the definitions from `src/lib.rs` and import from `macs`:

```rust
// src/lib.rs
use crate::macs::{MAC_ZERO, MAC_ONE};
```

---

### CR-02: `Key::from_blocks` bypasses the LSB-zero invariant via `transmute`

**File:** `src/keys.rs:86-90`

**Issue:** `Key::new` enforces `key.0.clear_lsb()` to maintain the IT-MAC invariant (key LSB must be 0). `Key::from_blocks` uses `unsafe { std::mem::transmute(blocks) }` to reinterpret a `Vec<Block>` as `Vec<Key>` without clearing any LSBs. Any `Block` whose LSB is 1 will produce a structurally invalid `Key`. Callers (e.g. GGM tree output, OT key material) typically produce random blocks with random LSBs, so on average half the keys produced by this path violate the invariant. MAC verification will then silently produce wrong results.

```rust
// current — invariant-breaking
pub fn from_blocks(blocks: Vec<Block>) -> Vec<Self> {
    unsafe { std::mem::transmute(blocks) }  // no LSB clearing
}
```

**Fix:** Either enforce the invariant in place, or document and enforce a precondition that all input blocks already have LSB 0 with a debug assertion:

```rust
pub fn from_blocks(mut blocks: Vec<Block>) -> Vec<Self> {
    for b in &mut blocks {
        b.clear_lsb();  // enforce Key LSB invariant
    }
    // SAFETY: Key is repr(transparent) over Block; LSBs are now cleared.
    unsafe { std::mem::transmute(blocks) }
}
```

If callers guarantee LSB=0 already (e.g., because the GGM tree clears it), add a `debug_assert` and add a comment explaining the contract:

```rust
pub fn from_blocks(blocks: Vec<Block>) -> Vec<Self> {
    debug_assert!(blocks.iter().all(|b| b.lsb() == 0),
        "Key::from_blocks: all blocks must have LSB=0");
    unsafe { std::mem::transmute(blocks) }
}
```

---

### CR-03: `TensorProductEval` has two constructors for the same input type with diverging initialisation

**File:** `src/tensor_eval.rs:30-58`

**Issue:** `TensorProductEval::new` (line 30) and `TensorProductEval::new_from_fpre_eval` (line 45) both accept `SemiHonestTensorPreEval` and are nearly identical. However, `new()` is never called anywhere in the codebase (confirmed by grep), making it dead code. More dangerously, if `new()` ever is called, it initialises `first_half_out` and `second_half_out` differently than `new_from_fpre_eval` — a subtle divergence. Having two nearly-identical constructors for the same type with one being silently dead is a correctness trap.

**Fix:** Remove `new()` entirely. Rename `new_from_fpre_eval` to `new` for consistency with the rest of the codebase:

```rust
// tensor_eval.rs — remove new() at lines 30-44, rename new_from_fpre_eval to new
pub fn new(tensor_eval: SemiHonestTensorPreEval) -> Self {
    // ... existing new_from_fpre_eval body ...
}
```

---

## Warnings

### WR-01: `verify_cross_party` duplicated verbatim across two files

**File:** `src/leaky_tensor_pre.rs:339-357` and `src/auth_tensor_pre.rs:324-342`

**Issue:** Both files contain an identical `verify_cross_party` function inside their respective `#[cfg(test)]` modules. The function body is byte-for-byte the same. Any bug fix or extension to one will not be applied to the other.

**Fix:** Extract to a shared test utility module, e.g. `src/test_utils.rs` (gated with `#[cfg(test)]`):

```rust
// src/test_utils.rs
#[cfg(test)]
pub mod verify {
    use crate::{delta::Delta, sharing::AuthBit};
    pub fn verify_cross_party(bit: &AuthBit, delta_a: &Delta, delta_b: &Delta) {
        // ... shared body ...
    }
}
```

---

### WR-02: `Block::clone()` called on a `Copy` type

**File:** `src/auth_tensor_fpre.rs:123` and `src/auth_tensor_fpre.rs:150`

**Issue:** `Block` derives `Copy`. Calling `.clone()` on a `Copy` type compiles fine but is misleading — it implies ownership transfer semantics that don't apply, and it obscures intent.

```rust
// current
eval_label = gen_label.clone();

// fix
eval_label = gen_label;
```

**Fix:** Replace both `.clone()` calls with direct assignment.

---

### WR-03: `AuthTensorGen::new` and `AuthTensorEval::new` are dead code using the global OS RNG

**File:** `src/auth_tensor_gen.rs:61-83` and `src/auth_tensor_eval.rs:52-74`

**Issue:** Both `new()` constructors call `Delta::random(&mut rand::rng())`, using the global OS-backed RNG. Neither constructor is called anywhere in `src/` or `benches/` (confirmed by grep). Dead constructors with different RNG semantics than the rest of the codebase will confuse future readers and may be called accidentally.

**Fix:** Remove both dead constructors. All callers already use `new_from_preprocessing` or equivalent injection-based constructors.

---

### WR-04: `build_share` is hardcoded to `&mut ChaCha12Rng` instead of a generic RNG bound

**File:** `src/sharing.rs:124`

**Issue:** The function signature is:

```rust
pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare
```

This unnecessarily couples the utility to a specific RNG type. Any caller using a different `Rng + CryptoRng` implementor (e.g. in tests with `StdRng`) cannot call it without a type dance.

**Fix:**

```rust
pub fn build_share<R: Rng + CryptoRng>(rng: &mut R, bit: bool, delta: &Delta) -> AuthBitShare
```

---

### WR-05: Chunked half outer product loop duplicated across four files

**File:** `src/tensor_gen.rs:52-91`, `src/tensor_eval.rs:55-95`, `src/auth_tensor_gen.rs:111-149`, `src/auth_tensor_eval.rs:100-150`

**Issue:** The inner loop logic for `gen_chunked_half_outer_product` / `eval_chunked_half_outer_product` is nearly identical across all four files. Differences are only in whether `tccr` or `cr` is called, and in which Delta is used. Any algorithmic change or bug fix must be applied four times.

**Fix:** Extract the shared skeleton into a function in `tensor_ops.rs` parameterized by the hash call and delta:

```rust
pub fn chunked_half_outer_product<F>(
    n: usize, m: usize, chunk_size: usize,
    x_labels: &[Block], y_labels: &[Block],
    hash_fn: F,
    out: &mut TypedMatrix<Block>,
) where F: Fn(Block, Block) -> Block { ... }
```

---

### WR-06: `Display` for `TypedMatrix` prints columns as text rows (transposed output)

**File:** `src/matrix.rs:328-335`

**Issue:** The outer loop iterates `j in 0..self.cols` and the inner loop iterates `i in 0..self.rows`, writing a newline after each `j` iteration. For column-major storage this means each line of output corresponds to one column of the matrix, not one row. For a 2×3 matrix (2 rows, 3 cols) this prints 3 lines of 2 elements — the transpose.

```rust
// current (prints columns as rows)
for j in 0..self.cols {
    for i in 0..self.rows {
        write!(f, "{} ", self.elements[j * self.rows + i])?;
    }
    writeln!(f)?;
}

// fix (prints rows as rows)
for i in 0..self.rows {
    for j in 0..self.cols {
        write!(f, "{} ", self.elements[j * self.rows + i])?;
    }
    writeln!(f)?;
}
```

---

### WR-07: Variable name typo `self_u12` / `rhs_u12` in `Block` arithmetic

**File:** `src/block.rs:394-395`

**Issue:** The local variables are named `self_u12` and `rhs_u12` — missing the trailing `8`. They hold `u128` values.  While functionally correct (they're just local names), the typo is a maintenance hazard: anyone auditing big-integer arithmetic will be confused about the type.

```rust
// current
let self_u12 = u128::from_le_bytes(self.0);
let rhs_u12  = u128::from_le_bytes(rhs.0);

// fix
let self_u128 = u128::from_le_bytes(self.0);
let rhs_u128  = u128::from_le_bytes(rhs.0);
```

---

### WR-08: `IdealPreprocessingBackend` uses hardcoded seeds — every run produces identical preprocessing

**File:** `src/preprocessing.rs:163-185`

**Issue:** Every call to `IdealPreprocessingBackend::run` constructs `TensorFpre::new(42, ...)`, `TensorFpre::new(43, ...)`, etc. The same seed produces the same pseudorandom sequence every time, so all benchmark invocations share the same preprocessing data. This defeats the purpose of benchmarking multiple independent samples.

**Fix:** Accept a seed parameter, or generate a fresh seed from OS entropy each invocation:

```rust
// option A: pass in seed
pub fn run(seed: u64, count: usize, n: usize, m: usize, cf: usize) -> ... { ... }

// option B: per-call fresh seed
let seed: u64 = rand::rng().random();
```

---

### WR-09: Redundant `(0 as u128)` casts — should use `0u128` or `Block::ZERO`

**File:** `src/tensor_ops.rs:30`, `31`, `33`, `34`, `156`

**Issue:** `(0 as u128)` is a verbose cast form for a literal. At call sites the intent is to pass a zero block as the tweak, which already has a named constant `Block::ZERO` (or equivalent) in `block.rs`.

```rust
// current
seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);

// fix
seeds[0] = cipher.tccr(Block::ZERO, x[n-1]);
```

---

### WR-10: `assemble_c_gamma_shares` and `assemble_c_gamma_shares_p2` are simulation-only helpers exposed in the crate root public API

**File:** `src/lib.rs:94-171` and `src/lib.rs:206-275`

**Issue:** Both functions are used only in tests and the online benchmark setup. Exposing them as `pub fn` in the crate root makes them part of the external API surface. They carry complex preconditions (correct column-major indexing, matching auth bit vectors) with no enforcement.

**Fix:** Move them into the test module or a `#[cfg(any(test, bench))]` gated module:

```rust
#[cfg(any(test, feature = "bench-internals"))]
pub mod simulation {
    pub fn assemble_c_gamma_shares(...) { ... }
    pub fn assemble_c_gamma_shares_p2(...) { ... }
}
```

---

### WR-11: `AuthTensorGen` and `AuthTensorEval` expose internal accumulator fields as `pub`

**File:** `src/auth_tensor_gen.rs` (struct definition ~lines 20-60) and `src/auth_tensor_eval.rs` (~lines 20-55)

**Issue:** Fields `first_half_out`, `second_half_out`, `first_half_out_ev`, `second_half_out_ev` are `pub`. These are internal intermediate computation buffers. External access allows callers to corrupt mid-computation state. Meanwhile the dimension fields `n` and `m` are private, so external code can read internal buffers but not the dimensions needed to interpret them.

**Fix:** Make accumulator fields private. Provide a read accessor only if needed after computation completes:

```rust
pub fn first_half_output(&self) -> &TypedMatrix<Block> { &self.first_half_out }
```

---

### WR-12: `TensorFpreGen` / `TensorFpreEval` use empty `Vec` sentinels for uninitialized D_ev fields

**File:** `src/preprocessing.rs` (`TensorFpreGen` and `TensorFpreEval` struct definitions), populated at `src/auth_tensor_fpre.rs:180-197`

**Issue:** Both structs have four fields (`alpha_d_ev_shares`, `beta_d_ev_shares`, `correlated_d_ev_shares`, `gamma_d_ev_shares`) that are always initialized to `vec![]` by `into_gen_eval` and populated later by `IdealPreprocessingBackend`. Any code path that uses these structs before the backend populates them will silently operate on empty data. There is no type-level or runtime guard against premature use.

**Fix:** Use `Option<Vec<...>>` so premature access panics with a meaningful message, or split the struct into two types (pre- and post-D_ev-population):

```rust
pub struct TensorFpreGen {
    // always present
    pub alpha_labels: Vec<Block>,
    // ...
    // present only after D_ev population
    pub d_ev: Option<TensorFpreGenDev>,
}
```

---

### WR-13: Misleading `_key` variable name in `AesEncryptor::new`

**File:** `src/aes.rs:169`

**Issue:** The variable is named `_key` — the leading underscore conventionally signals "intentionally unused" to the Rust compiler and to readers. But `_key` is immediately used on the next line to construct the `AesEncryptor`. The compiler does not warn, but every human reader will be confused.

```rust
// current
let _key: [u8; 16] = key.into();
AesEncryptor(Aes128Enc::new_from_slice(&_key).unwrap())

// fix
let key_bytes: [u8; 16] = key.into();
AesEncryptor(Aes128Enc::new_from_slice(&key_bytes).unwrap())
```

---

### WR-14: Dead statement `let _ = count` after an assert that `count == 1`

**File:** `src/preprocessing.rs:151`

**Issue:** After `assert_eq!(count, 1, ...)`, the very next statement is `let _ = count;`. The binding is already consumed by the assert. This dead statement adds noise without purpose.

**Fix:** Remove the `let _ = count;` line.

---

### WR-15: `garble_final_outer_product` and `evaluate_final_outer_product` return unnecessary clones

**File:** `src/tensor_gen.rs:162` and `src/tensor_eval.rs:172`

**Issue:** Both methods mutate `self.first_half_out` in-place and then return `self.first_half_out.clone()`. Since these methods consume `self` (or should), the clone is unnecessary — the caller could take ownership of the field directly.

```rust
// current (in tensor_gen.rs)
pub fn garble_final_outer_product(...) -> TypedMatrix<Block> {
    // ... mutate self.first_half_out in-place ...
    self.first_half_out.clone()  // unnecessary
}

// fix — consume self and return the field
pub fn garble_final_outer_product(mut self, ...) -> TypedMatrix<Block> {
    // ... mutate self.first_half_out in-place ...
    self.first_half_out
}
```

---

### WR-16: Both online benchmark groups use the same Criterion group name `"online"`

**File:** `benches/benchmarks.rs:179` and `benches/benchmarks.rs:293`

**Issue:** `bench_online_p1` calls `criterion.benchmark_group("online")` and `bench_online_p2` also calls `criterion.benchmark_group("online")`. Criterion identifies groups by name, and merging two distinct groups under the same name will cause benchmark ID collisions and confuse comparison plots.

```rust
// bench_online_p1 — line 179
let mut group = c.benchmark_group("online");  // should be "online_p1"

// bench_online_p2 — line 293
let mut group = c.benchmark_group("online");  // should be "online_p2"
```

**Fix:** Use distinct group names `"online_p1"` and `"online_p2"`.

---

### WR-17: `_eval_cts` discarded return value with no explanation

**File:** `src/tensor_eval.rs:100`, `src/auth_tensor_eval.rs:140` and `src/auth_tensor_eval.rs:209`

**Issue:** Multiple call sites discard a return value into `_eval_cts` (or `let _ = ...`). A leading underscore on a non-trivially-typed return suppresses the unused-binding lint, but gives no explanation to readers of why the ciphertexts are intentionally discarded. In an eval path that is supposed to verify garbled ciphertexts, silently discarding output is a red flag.

**Fix:** Add a comment explaining the discard:

```rust
// The evaluator's ciphertexts are used only for the cross-check in the online phase;
// in this bench/test path we discard them as the check is not exercised here.
let _eval_cts = self.eval_chunked_half_outer_product(...);
```

---

## Info

### IN-01: `AuthBitShare::bit()` is a trivial alias for `self.value` with inconsistent call-site usage

**File:** `src/sharing.rs:55`

**Issue:** `bit()` returns `self.value` unchanged. Call sites mix `share.bit()` and `share.value` access directly. The thin wrapper adds a method that doesn't enforce any invariant and whose name (`bit`) duplicates the field semantic (`value: bool`).

**Fix:** Remove `bit()` and use `share.value` everywhere, or rename the field to `bit` and remove the method.

---

### IN-02: Seven near-identical networking benchmark wrapper functions

**File:** `benches/benchmarks.rs:457-477`

**Issue:** `bench_4x4_runtime_with_networking`, `bench_8x8_runtime_with_networking`, ..., `bench_256x256_runtime_with_networking` are seven functions with identical bodies differing only in the `(n, m)` parameter passed to the inner function.

**Fix:** Replace with a parameter loop:

```rust
for (n, m) in &[(4,4),(8,8),(16,16),(32,32),(64,64),(128,128),(256,256)] {
    bench_runtime_with_networking(c, *n, *m);
}
```

---

### IN-03: TODO comments mark three placeholder ideal functionalities

**File:** `src/bcot.rs:20`, `src/auth_tensor_fpre.rs:1`, `src/feq.rs:8`

**Issue:** All three mark locations where real cryptographic protocols must replace the ideal/insecure stubs before production use. They are expected during research but should be tracked.

**Fix:** Convert to `// FIXME(production):` comments and create tracking issues, or gate the stubs behind a `#[cfg(feature = "ideal-functionalities")]` feature flag so they cannot accidentally ship in production builds.

---

### IN-04: `CSP` and `SSP` constants defined but suppressed as dead code

**File:** `src/lib.rs:34,36`

**Issue:**
```rust
#[allow(dead_code)]
const CSP: usize = 128;
#[allow(dead_code)]
const SSP: usize = 40;
```

These constants are never used anywhere. Suppressing the lint rather than removing them signals they are aspirationally relevant but not yet wired in.

**Fix:** Either use them to parameterize the GGM tree depth and statistical security checks, or remove them until they are needed.

---

### IN-05: `n` and `m` are private on `AuthTensorGen`/`AuthTensorEval` with no public accessors

**File:** `src/auth_tensor_gen.rs` and `src/auth_tensor_eval.rs`

**Issue:** The dimension fields `n` and `m` are `pub(crate)` or private, while all data buffers are `pub`. External code that receives a completed `AuthTensorGen` cannot query its dimensions without inspecting the buffer lengths directly.

**Fix:** Add `pub fn n(&self) -> usize { self.n }` and `pub fn m(&self) -> usize { self.m }` accessors, and make the data buffer fields private (see WR-11).

---

### IN-06: `SemiHonestTensorPre::new()` silently uses global OS RNG for delta

**File:** `src/tensor_pre.rs`

**Issue:** `new()` draws `delta` from `rand::rng()` (global OS RNG, non-deterministic), while `new_with_delta` allows injection. The inconsistency between these two constructors is invisible at call sites.

**Fix:** Remove `new()` and require callers to provide delta explicitly via `new_with_delta`, matching the pattern used by `TensorFpre`.

---

### IN-07: `BcotOutput::choices` field duplicates data already held by the caller

**File:** `src/bcot.rs`

**Issue:** `IdealBCot::extend` takes `choices: Vec<bool>` from the caller and stores them inside `BcotOutput::choices`. The caller already owns the choices and can retain a reference; storing a copy inside the output struct doubles memory use and invites desynchronisation.

**Fix:** Remove `choices` from `BcotOutput` and have callers retain their own copy, or store a reference/slice if lifetime permits.

---

_Reviewed: 2026-04-25_
_Reviewer: Claude (adversarial deep review)_
_Depth: deep_
