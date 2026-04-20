# Testing Patterns

**Analysis Date:** 2026-04-19

## Test Framework

**Runner:**
- Rust built-in `cargo test` with no external test runner
- No `jest.config.*` or separate test config — tests run via `cargo test`

**Benchmark Framework:**
- Criterion 0.7 (`criterion = "0.7.0"` in `[dependencies]` and `[dev-dependencies]`)
- Async benchmarks use `criterion`'s `async_tokio` feature: `criterion = { version = "0.7", features = ["async_tokio"] }`
- Tokio runtime for async benchmarks: `tokio = { version = "1.47.1", features = ["full"] }`

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `#[should_panic]` from Rust std
- No third-party assertion or property-testing crate

**Run Commands:**
```bash
cargo test                         # Run all unit tests
cargo bench                        # Run all Criterion benchmarks
cargo bench -- <benchmark_name>    # Run a specific benchmark group
```

## Test File Organization

**Location:**
- Tests are co-located with implementation using `#[cfg(test)] mod tests { ... }` blocks at the bottom of each source file
- One exception: `src/aes.rs` has a bare `#[test]` function at module level (not inside a `mod tests` block)

**Naming:**
- Test functions use `test_` prefix: `test_set_lsb`, `test_lsb`, `test_semihonest_tensor_product`
- Benchmark functions use `bench_` prefix: `bench_full_protocol_garbling`, `bench_4x4_runtime_with_networking`

**Structure:**
```
src/
  block.rs          # #[cfg(test)] mod tests { 5 test fns }
  matrix.rs         # #[cfg(test)] mod tests { 12 test fns }
  lib.rs            # #[cfg(test)] mod tests { 2 integration-style test fns }
  auth_tensor_fpre.rs  # #[cfg(test)] mod tests { 2 test fns }
  auth_tensor_gen.rs   # #[cfg(test)] mod tests { 1 test fn }
  aes.rs            # bare #[test] fn aes_test (no mod wrapper)

benches/
  benchmarks.rs     # Criterion benchmark entry point
  network_simulator.rs  # Network simulator helper (not a test file)
```

## Test Structure

**Suite Organization:**

Tests inside `#[cfg(test)]` blocks import with `use super::*` for module-local items:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let matrix = KeyMatrix::new(3, 4);
        assert_eq!(matrix.rows(), 3);
        assert_eq!(matrix.cols(), 4);
    }
}
```

The integration-style tests in `src/lib.rs` import from specific modules rather than `super::*`:
```rust
#[cfg(test)]
mod tests {
    use crate::delta::Delta;
    use crate::tensor_gen::TensorProductGen;
    // ...

    #[test]
    fn test_semihonest_tensor_product() { ... }

    #[test]
    fn test_auth_tensor_product() { ... }
}
```

**Patterns:**
- No `before_each` / setup / teardown hooks; each test constructs its own state inline
- RNG seeded deterministically in tests: `ChaCha12Rng::seed_from_u64(seed)` or `rand::rng()` (non-deterministic)
- `println!` calls left in tests for debugging output (not removed): `src/lib.rs` lines 319–322, 360–372

## Mocking

No mocking framework is used. The codebase avoids the need for mocks by:

- Using `new_with_delta()` constructors to inject a known `Delta` into any type that would otherwise generate it randomly
- Using `ChaCha12Rng::seed_from_u64(seed)` with a fixed seed for deterministic random state
- The `TensorFpre` and `SemiHonestTensorPre` types are themselves the "ideal functionality" (Fpre) standing in for what would be a network protocol in production

**Network simulation:**
The benchmarks include a `SimpleNetworkSimulator` in `benches/network_simulator.rs` that simulates bandwidth and latency using `tokio::time::sleep` — this is a timing tool, not a mock.

## Fixtures and Factories

No dedicated fixture files or factory helpers exist. Test data is constructed inline:

```rust
// Typical test setup
let mut rng = rand::rng();
let delta = Delta::random(&mut rng);
let n = 2;
let m = 3;
let clear_x = 0b01;
let clear_y = 0b101;
let mut pre = SemiHonestTensorPre::new_with_delta(3, n, m, 6, delta);
```

**Helper verification functions** in `src/lib.rs` `tests` module act as reusable assertion utilities (not exposed outside the test module):
- `verify_vector_sharing(clear_val, gb_share, ev_share, delta, n)` — checks a wire label vector is correctly WLOL-shared
- `verify_column_matrix_sharing(clear_val, gb_share, ev_share, delta, n)` — same check for a column matrix
- `verify_tensor_output(clear_x, clear_y, n, m, gb_out, ev_out, delta)` — checks the tensor product output matrix

## Coverage

**Requirements:** No coverage target is enforced; no `.cargo/config.toml` or CI coverage tool is configured.

**Covered areas:**
- `src/block.rs`: `set_lsb`, `lsb`, `reverse_bits`, `sigma`, `MONOMIAL` constant
- `src/matrix.rs`: construction, vector indexing, matrix indexing, `BitXor`, `BitXorAssign`, `Display`, bounds panics, generic `TypedMatrix<T>` functionality with both `Key` and `Block` element types
- `src/aes.rs`: `AesEncryptor::para_encrypt` with known-answer test (hardcoded expected output blocks)
- `src/auth_tensor_fpre.rs`: auth bit generation, input sharing generation, `into_gen_eval` split
- `src/auth_tensor_gen.rs`: `new_from_fpre_gen`, `garble_first_half` (smoke test only)
- `src/lib.rs`: full end-to-end semi-honest tensor product protocol (`test_semihonest_tensor_product`), full end-to-end authenticated tensor product protocol (`test_auth_tensor_product`)

**Not covered:**
- `src/tensor_ops.rs`: no dedicated tests (only exercised indirectly through protocol tests)
- `src/tensor_eval.rs`: no dedicated tests (only exercised through `test_semihonest_tensor_product` in `src/lib.rs`)
- `src/auth_tensor_eval.rs`: no dedicated tests (only exercised through `test_auth_tensor_product`)
- `src/delta.rs`: no tests
- `src/keys.rs`: no tests
- `src/macs.rs`: no tests
- `src/sharing.rs`: no tests for `build_share`, `AuthBit::verify`
- Matrix view operations (`MatrixViewRef`, `MatrixViewMut`, `shift`, `resize`, `with_subrows`, `transpose`): not directly tested

## Test Types

**Unit Tests:**
- Found in `src/block.rs`, `src/matrix.rs`, `src/aes.rs`, `src/auth_tensor_fpre.rs`, `src/auth_tensor_gen.rs`
- Scope: individual methods and data structures

**Protocol Integration Tests:**
- Found in `src/lib.rs`
- Scope: full multi-step protocol execution between a generator and evaluator, with intermediate-state assertions at each step
- `test_semihonest_tensor_product`: tests the semi-honest protocol step by step, asserting correctness of label sharings after each phase
- `test_auth_tensor_product`: tests the authenticated protocol end to end, including auth bit correctness assertions

**Benchmarks:**
- Found in `benches/benchmarks.rs`
- Framework: Criterion 0.7 with `async_tokio`
- Three benchmark groups:
  - `bench_full_protocol_garbling`: garbler-only timing across matrix sizes `(4,4)` to `(128,128)` for chunking factors 1, 2, 4, 6, 8
  - `bench_full_protocol_with_networking`: full gen+eval+network simulation timing across the same matrix/chunking matrix
  - Per-size networking benchmarks: `bench_4x4_runtime_with_networking`, `bench_8x8_runtime_with_networking`, `bench_16x16_runtime_with_networking`, `bench_32x32_runtime_with_networking`, `bench_64x64_runtime_with_networking`, `bench_128x128_runtime_with_networking`, `bench_256x256_runtime_with_networking`
- Throughput metric set via `group.throughput(Throughput::Elements(...))` or `group.throughput(Throughput::Bytes(...))`

## Benchmark Setup Pattern

Benchmarks use `iter_batched` with `BatchSize::SmallInput` to re-initialize state each iteration without counting setup time:

```rust
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
```

The Tokio runtime is a crate-level static initialized via `once_cell::Lazy`:
```rust
static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap()
});
```

`BenchmarkId::new(chunking_factor_str, size_str)` is used for labeling: first arg is the series (chunking factor), second is the input dimension string (`"4x4"`, `"128x128"`).

---

*Testing analysis: 2026-04-19*
