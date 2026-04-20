# Codebase Concerns

**Analysis Date:** 2026-04-19

---

## TODOs / FIXMEs

**Unresolved module refactor:**
- Issue: `AuthBit` / `AuthBitShare` types are defined inside `auth_tensor_fpre.rs` but are general-purpose authenticated-bit primitives that belong in a shared module.
- Files: `src/auth_tensor_fpre.rs:1` — `// TODO refactor authbit from fpre to a common module, or redefine with new name.`
- Impact: `AuthBitShare` and related helpers are imported directly from `fpre`, coupling the Fpre type to consumers that only need the share type. Future protocol variants will either re-duplicate or rely on this incidental location.
- Fix approach: Move `AuthBit`, `AuthBitShare`, `build_share`, and related `Add` impls out of `auth_tensor_fpre.rs` into `src/sharing.rs` (which already holds `InputSharing`). Update all `use` paths.

---

## Unsafe Code

**Severity: Medium** — The safety invariants are straightforward and currently correct, but they are fragile: any future refactor that changes the repr of `Mac`, `Key`, or `Block` will silently break memory safety.

**Pointer-cast slice transmutes in `Mac` and `Key`:**
- Files: `src/macs.rs:59`, `src/macs.rs:67`, `src/keys.rs:67`, `src/keys.rs:75`, `src/keys.rs:83`
- Pattern:
  ```rust
  // macs.rs:59
  unsafe { &*(slice as *const [Self] as *const [Block]) }
  // macs.rs:67 / keys.rs:75 / keys.rs:83
  unsafe { std::mem::transmute(blocks) }
  ```
- Both `Mac(Block)` and `Key(Block)` are `#[repr(transparent)]` newtypes (inferred — there is no explicit `#[repr(transparent)]` attribute on either struct). Without the attribute the layout guarantee is only by convention; if `derive(Debug, Clone, Copy, PartialEq)` is later expanded or a field is added, these transmutes become UB.
- Fix approach: Add `#[repr(transparent)]` explicitly to `Mac` and `Key`. Consider replacing `transmute` with `bytemuck::cast_vec` / `bytemuck::cast_slice` after implementing `Pod`+`Zeroable` (already done for `Block`).

**Raw-pointer slice construction in `Block`:**
- Files: `src/block.rs:125–127`, `src/block.rs:136–138`
- Uses `unchecked_mul` (nightly-only intrinsic) and `from_raw_parts`. The SAFETY comments are accurate but `unchecked_mul` requires a nightly feature and will panic in debug if the invariant is ever violated differently.
- Fix approach: Replace with `slice.len() * Self::LEN` (safe multiplication is fine here — the slice already lives in the address space) and use `bytemuck::cast_slice` or `<[[u8; 16]]>::as_flattened` (stabilised in Rust 1.80).

**`std::mem::transmute` for `Array<u8, U16>` slices:**
- Files: `src/block.rs:158`, `src/block.rs:164`
- `Block` is `#[repr(transparent)]` over `[u8; 16]` and `Array<u8, U16>` is `#[repr(transparent)]` over `[u8; 16]` via hybrid-array, making this sound — but it is not documented and is not verified by the compiler.
- Fix approach: Use `bytemuck` conversions or add a `// SAFETY:` comment citing the repr guarantees of both types.

---

## Potential Panics

**Severity: Low in production, Medium for correctness** — Panics occur only at compile-time-selected constant inputs or in test/bench code, not in the protocol hot-path. However, some cases can be triggered by bad runtime parameters.

**`unwrap()` on AES key construction:**
- Files: `src/aes.rs:16` (static initialiser), `src/aes.rs:149` (`AesEncryptor::new`)
- Both call `Aes128Enc::new_from_slice(&key).unwrap()`. The key is always exactly 16 bytes, so this cannot fail in practice, but a future key-length change will cause a panic at program startup (static) or silently at runtime.
- Fix approach: Use `Aes128Enc::new(&key.into())` (infallible `KeyInit` via `From<[u8; 16]>`) as already done in `FixedKeyAes::new` at line 28.

**`assert!` in `para_encrypt` hot path:**
- Files: `src/aes.rs:194`
- `assert!(blks.len() >= NM * NK)` fires a panic with no error context in production builds. This is the correct bound check, but the caller has no way to recover.
- Fix approach: Return a `Result` or use a type-level guarantee (const generics already used for `NK`/`NM`).

**`assert!` for input size validation:**
- Files: `src/tensor_pre.rs:52–53`
- `assert!(x < 1<<self.n)` / `assert!(y < 1<<self.m)` in `SemiHonestTensorPre::gen_inputs` panic with no descriptive message.
- Fix approach: Use `assert!(x < 1<<self.n, "input x={x} exceeds n-bit width {}", self.n)` or return a `Result`.

**`assert!` in `MatrixViewRef::len` / `MatrixViewMut::len`:**
- Files: `src/matrix.rs:338`, `src/matrix.rs:415`
- `assert!(self.view_cols == 1)` — hard panic if a non-column-vector is accidentally passed to a scalar-index operation. `debug_assert!` is used elsewhere for dimension mismatches (lines 190, 197, 224, 249) but these two use regular `assert!`, meaning they fire in release builds.
- These are consistent and arguably correct, but worth documenting.

**Out-of-bounds index risk in chunked algorithms:**
- Files: `src/tensor_eval.rs:206`, `src/auth_tensor_eval.rs:210`
- `chunk_levels[s]` and `chunk_cts[s]` are indexed with `s` derived from `x.rows()` / `chunking_factor`. If the caller passes mismatched `chunk_levels` / `chunk_cts` vectors (different length from what the garbler produced), this panics.
- There is no length validation before indexing.
- Fix approach: Add a `debug_assert_eq!(chunk_levels.len(), expected_chunks)` at the top of `eval_chunked_half_outer_product`.

**Exponential allocation in seed tree:**
- Files: `src/tensor_ops.rs:20`, `src/tensor_eval.rs:70`, `src/auth_tensor_eval.rs:75`
- `vec![Block::default(); 1 << n]` allocates 2^n blocks. For `n = chunking_factor = 8` this is 256 × 16 = 4 KB — safe. For `chunking_factor > 20` (not currently benchmarked but reachable by API) this overflows or OOMs.
- Fix approach: Document the practical upper bound on `chunking_factor` or add a check.

---

## Security Considerations

**Severity: High** — This is a cryptographic protocol implementation. The concerns below do not indicate a known break, but they represent deviations from defensive cryptographic engineering practice.

**`TensorFpre` is explicitly insecure / ideal:**
- Files: `src/auth_tensor_fpre.rs:8`
- The struct comment reads "Insecure ideal Fpre that pre-generates auth bits...". This component simulates the Fpre functionality in the clear (both parties' shares are held in the same process). It is not a real OT-based or MPC-based Fpre.
- Impact: The entire authenticated garbling protocol is only tested against this ideal-world simulator, not against a real two-party Fpre. The security of the full protocol depends on correctly replacing `TensorFpre` with a real Fpre; that component does not exist in this codebase.
- Fix approach: Document clearly in README and module docs that `TensorFpre` is a placeholder. Add a real Fpre or link to where one would be plugged in.

**MAC constants hardcoded in two places:**
- Files: `src/lib.rs:29–35`, `src/macs.rs:7–12`
- `MAC_ZERO` and `MAC_ONE` are defined twice with identical values but independently. A future update to one copy that misses the other will silently produce an inconsistency.
- Fix approach: Remove the duplicate in `src/lib.rs` and re-export from `src/macs.rs`.

**Fixed AES key is an easily-guessable pattern:**
- Files: `src/aes.rs:10–12`
- `FIXED_KEY = [69, 42, 69, 42, ...]` ("69 42" repeated). The key is used as a correlation-robust hash (TCCR). The choice of key is publicly known, which is intentional for the construction, but the repeating pattern is aesthetically poor and may invite unnecessary scrutiny. More importantly, it is never checked against the constant from the paper reference.
- Fix approach: Use a nothing-up-my-sleeve constant (e.g., the first 16 bytes of SHA-256("authenticated-tensor-garbling")) and add a comment linking to the paper section justifying the use of a fixed key.

**`Delta` LSB invariant not enforced at the type level:**
- Files: `src/delta.rs`
- `Delta::new` sets LSB=1. However, `Delta::set_lsb(self, false)` exists as a public method and can produce a `Delta` with LSB=0, violating the free-XOR invariant that the entire protocol depends on.
- Impact: Calling `delta.set_lsb(false)` would silently break authentication without any compile-time or runtime error.
- Fix approach: Make `set_lsb` private or remove it; expose only `Delta::random` and `Delta::new` as constructors.

**No constant-time operations:**
- Files: All of `src/` — no `subtle` crate or explicit constant-time comparisons.
- MAC verification in `src/sharing.rs:47–49` (`AuthBitShare::verify`) uses `assert_eq!` which compares `Mac` values via `PartialEq`, which is not constant-time. This is a concern if the verify path is ever on a side-channel-sensitive code path.
- The benchmarking and test contexts make this low-priority now, but it is a gap for any production deployment.
- Fix approach: Use `subtle::ConstantTimeEq` for MAC comparison.

**ChaCha12 used instead of ChaCha20:**
- Files: `src/sharing.rs:7`, `src/auth_tensor_fpre.rs:5`
- `rand_chacha::ChaCha12Rng` uses 12 rounds. ChaCha20 (20 rounds) is the standard recommendation for cryptographic RNG. ChaCha12 provides a lower security margin and is not the default recommendation from the `rand` project for cryptographic use.
- Fix approach: Replace `ChaCha12Rng` with `ChaCha20Rng` from the same crate.

---

## Performance Concerns

**Cloning entire `BlockMatrix` in return paths:**
- Files: `src/tensor_gen.rs:162`, `src/tensor_eval.rs:271`
- `garble_final_outer_product` and `evaluate_final_outer_product` return `self.first_half_out.clone()`. For a 128×128 matrix this is 128×128×16 = 262 KB copied at the end of every benchmark iteration.
- Fix approach: Return a reference, or restructure so ownership is consumed / the caller extracts the result.

**`eval_cts` return value is discarded:**
- Files: `src/tensor_eval.rs:207`, `src/auth_tensor_eval.rs:211`
- `_eval_cts` is assigned but never used. `eval_unary_outer_product` builds and returns a full `Vec<Block>` on every call, performing heap allocations and computing values that are thrown away.
- Fix approach: Either remove the return value and accumulation logic if it is genuinely unused, or document why it must be computed but not consumed.

**Intermediate `tree` accumulation in seed algorithms:**
- Files: `src/tensor_ops.rs:14`, `src/tensor_eval.rs:67`, `src/auth_tensor_eval.rs:72`
- Each level of the GGM tree is pushed onto a `tree: Vec<Block>` (growing exponentially), and only the last `1 << n` elements are used. This doubles the peak memory usage compared to computing only the leaves.
- For `n = chunking_factor = 8`, peak `tree` size is `(2 + 4 + ... + 256) × 16 bytes = 8 KB` — acceptable now but wasteful.
- Fix approach: Compute leaves directly without storing intermediate levels, or use a ring-buffer of two levels.

**Repeated allocation of `slice` BlockMatrix inside chunked loop:**
- Files: `src/tensor_gen.rs:65–67`, `src/tensor_eval.rs:187–190`, `src/auth_tensor_gen.rs:86–89`, `src/auth_tensor_eval.rs:192–195`
- A new `BlockMatrix::new(slice_size, 1)` is allocated on every chunk iteration inside the hot loop. For large `n` with small `chunking_factor` this creates many short-lived heap allocations.
- Fix approach: Pre-allocate a scratch `BlockMatrix` outside the loop and reuse it.

**Benchmark code duplication:**
- Files: `benches/benchmarks.rs` (the file is ~756 lines)
- Each of the 8 `bench_NxN_runtime_with_networking` functions is a near-identical copy with only `n`, `m` values changed. The repeated setup / communication-size computation blocks are copy-pasted ~8 times per chunking factor.
- Fix approach: Extract a single parameterised `bench_protocol_with_networking(c, n, m)` function.

---

## Architectural Concerns

**No real network / two-party split:**
- Files: All of `src/`
- The generator (`AuthTensorGen`) and evaluator (`AuthTensorEval`) run in the same process, sharing memory. The "communication" is just passing `Vec` values between function calls. The `SimpleNetworkSimulator` in `benches/network_simulator.rs` only sleeps for a computed duration; it does not actually serialize or transmit data.
- Impact: Serialization bugs, network protocol bugs, and concurrent-execution issues (e.g., ordering of messages) cannot be caught by this test suite.
- Fix approach: Use real channels (e.g., `tokio::sync::mpsc` or actual TCP) to separate the two roles, even in tests.

**`gamma_auth_bit_shares` computed but never consumed in final output:**
- Files: `src/auth_tensor_gen.rs:188–192`, `src/auth_tensor_eval.rs` (no gamma consumption)
- In `garble_final`, `_gamma_share` is computed but immediately discarded (prefixed `_`). The evaluator's `evaluate_final` never uses `gamma_auth_bit_shares`. If gamma is required by the protocol, this is a correctness gap; if it is not needed, the ~n×m `AuthBitShare` structures are generated, transmitted in Fpre, and then wasted.
- Fix approach: Clarify in a comment whether gamma is needed for the MAC-check phase (which is not implemented — see below) or remove it.

**MAC verification / abort not implemented:**
- Files: `src/auth_tensor_eval.rs`, `src/auth_tensor_gen.rs`
- The authenticated garbling protocol requires the evaluator to verify MACs on received values and abort if any check fails. No such verification exists in the current implementation. `AuthBitShare::verify` exists in `src/sharing.rs` but is only called in test code, never in the protocol execution paths.
- Impact: The "authenticated" in "authenticated tensor garbling" is not enforced at runtime; a malicious garbler could send corrupted ciphertexts undetected.
- Fix approach: Add MAC verification calls in `evaluate_first_half`, `evaluate_second_half`, and `evaluate_final`, returning `Result` on failure.

**`usize` used as the plaintext type:**
- Files: `src/tensor_pre.rs`, `src/auth_tensor_fpre.rs`, `src/lib.rs`
- Input values are `usize`, which is 64 bits on 64-bit platforms. `n` and `m` can exceed 64, in which case bit operations like `(x >> i) & 1` silently drop high bits of the input. Benchmarks use `n = m = 128` and `256`, but tests only use `n = m ≤ 16`.
- Impact: For `n > 64`, `gen_inputs` will silently treat all high bits as 0.
- Fix approach: Replace `usize` inputs with `Vec<bool>` or a bignum type, and add an assertion `assert!(n <= usize::BITS as usize)` until then.

**`Debug` / `Display` for `Block` only shows one byte:**
- Files: `src/block.rs:169–180`
- Both `Display` and `Debug` print only `self.0[15]` (the last byte). This makes debugging very difficult — distinct blocks that share the last byte are indistinguishable in output. Existing test output (`println!("gen_chunk_levels: {:?}", ...)`) is affected.
- Fix approach: Print all 16 bytes in hex, or at minimum the first and last.

---

## Missing Error Handling

**All public protocol methods return `()`:**
- Files: `src/auth_tensor_gen.rs` (`garble_final`, `garble_first_half`, `garble_second_half`), `src/auth_tensor_eval.rs` (`evaluate_first_half`, `evaluate_second_half`, `evaluate_final`)
- There is no way for callers to detect protocol failures. The `thiserror` crate is declared in `Cargo.toml` but no error types are defined anywhere in `src/`.
- Fix approach: Define an `Error` enum using `thiserror`, and have garble/evaluate methods return `Result<_, Error>`.

---

## Dependencies at Risk

**Pre-release / RC versions pinned:**
- `aes = "0.9.0-pre.3"` — resolved to `0.9.0-rc.0` in `Cargo.lock`. Pre-release crates are not subject to semver stability guarantees and may have breaking changes before final release.
- `cipher = "0.5.0-pre.8"` — same concern.
- Impact: A future `cargo update` may pull in a breaking RC that prevents compilation.
- Fix approach: Either pin to exact versions (`= "0.9.0-rc.0"`) or wait for stable releases.

**`once_cell` superseded by `std`:**
- `once_cell = "1.21.3"` is used only for `Lazy<FixedKeyAes>` in `src/aes.rs` and `src/auth_tensor_gen.rs` / `src/auth_tensor_eval.rs`.
- `std::sync::LazyLock` (stable since Rust 1.80) and `std::sync::OnceLock` are direct replacements.
- Fix approach: Replace `once_cell::sync::Lazy` with `std::sync::LazyLock` and remove the `once_cell` dependency.

**`criterion` listed in both `[dependencies]` and `[dev-dependencies]`:**
- Files: `Cargo.toml:29`, `Cargo.toml:35`
- `criterion = "0.7.0"` appears in `[dependencies]` (compiled into the library) and again in `[dev-dependencies]`. The library should not depend on a benchmarking framework.
- Fix approach: Remove the `criterion` entry from `[dependencies]`; keep only the `[dev-dependencies]` entry with `async_tokio` feature.

**`tokio` in `[dependencies]` with `features = ["full"]`:**
- `tokio` with `full` features (including `net`, `fs`, `process`, etc.) is listed as a regular dependency but is only used for the benchmark network simulator. This significantly inflates the dependency tree and compile times for users of the library.
- Fix approach: Move `tokio` to `[dev-dependencies]` or use `features = ["time", "rt-multi-thread"]` only for what the benchmark needs.

---

## Test Coverage Gaps

**No tests for `chunking_factor` edge cases:**
- What is not tested: `chunking_factor = 0` (would cause division-by-zero or infinite loop in `(x.rows() + 0 - 1) / 0`), and `chunking_factor > n` (the last chunk slice-size would be 0, which may produce an empty seed tree).
- Files: `src/tensor_gen.rs:62`, `src/tensor_eval.rs:185`, `src/auth_tensor_gen.rs:84`, `src/auth_tensor_eval.rs:190`
- Risk: Caller passes `chunking_factor = 0` at runtime and causes a panic or infinite loop.
- Priority: Medium

**No tests for `n > 64` inputs:**
- What is not tested: The documented `usize` truncation concern above is not caught by any test.
- Files: `src/tensor_pre.rs:52–53`, `src/auth_tensor_fpre.rs:119`
- Priority: High (correctness gap for large matrices)

**No tests for mismatched garbler/evaluator parameter disagreement:**
- What is not tested: Calling `evaluate_first_half` with ciphertexts from a generator configured with different `n`, `m`, or `chunking_factor`.
- Files: `src/tensor_eval.rs`, `src/auth_tensor_eval.rs`
- Risk: Silently produces wrong output or panics on index.
- Priority: Medium

**`_eval_cts` return values never verified:**
- What is not tested: The ciphertexts returned by `eval_unary_outer_product` are discarded. If the eval-side ciphertexts are meant to be used in a subsequent protocol step (e.g., for output reconstruction or consistency check), that step is absent and untested.
- Files: `src/tensor_eval.rs:207`, `src/auth_tensor_eval.rs:211`
- Priority: Medium

---

*Concerns audit: 2026-04-19*
