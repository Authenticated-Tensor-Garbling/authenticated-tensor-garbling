# Testing Patterns

**Analysis Date:** 2026-04-28

## Test Framework

**Runner:**
- Rust built-in `#[test]` + `cargo test` (no external test runner).
- No `jest.config.*`, `vitest.config.*`, or equivalent.

**Assertion Library:**
- `std` macros: `assert!`, `assert_eq!`, `assert_ne!` with custom messages
  (`assert_eq!(a, b, "descriptive message")` is the dominant pattern).
- `#[should_panic(expected = "...")]` for protocol-abort tests.
- `share.verify(&delta)` panics on MAC mismatch — used as a test-time assertion
  in several helpers.

**Benchmark runner:**
- Criterion 0.7 (criterion_group / criterion_main) in `benches/benchmarks.rs`.
- Config: `[[bench]] name = "benchmarks" harness = false` in `Cargo.toml`.
- Dev-dependency: `criterion = { version = "0.7", features = ["async_tokio"] }`.

**Run Commands:**
```bash
cargo test                      # Run all unit tests
cargo test -- --nocapture       # Run with stdout (useful for debug prints)
cargo bench                     # Run Criterion benchmarks
cargo bench --bench benchmarks  # Run the specific bench file
cargo run --release --example aes_microbench  # AES throughput verification
```

## Test File Organization

**Location:** Co-located in-file. Every source module that has tests defines
a `#[cfg(test)] mod tests { ... }` block at the bottom of the same `.rs` file.

**No `tests/` directory.** There are no separate integration-test files.

**Naming:** Test functions are named `test_<what_is_being_tested>` with
descriptive suffixes:
- `test_check_zero_passes_on_zero_bit_with_valid_mac`
- `test_auth_tensor_product_full_protocol_1_ideal`
- `test_protocol_1_e_input_wire_check_aborts_on_garbler_d_ev_tamper`
- `test_uncompressed_backend_d_ev_shares_bit_correlation`
- `test_two_to_one_combine_tampered_d_panics`

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specific_dep::Dep;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    // Optional: shared helper function (private to mod tests)
    fn make_triples(n: usize, m: usize, count: usize) -> Vec<LeakyTriple> { ... }

    #[test]
    fn test_something_specific() {
        // Arrange
        let mut rng = ChaCha12Rng::seed_from_u64(N);
        let delta = Delta::random(&mut rng);
        // Act
        let result = function_under_test(...);
        // Assert
        assert!(result, "descriptive failure message");
    }

    #[test]
    #[should_panic(expected = "exact panic message prefix")]
    fn test_something_panics_on_invalid_input() { ... }
}
```

**Patterns:**
- `use super::*` at the top of every `mod tests` block.
- Protocol-body parameterization: shared `fn run_full_protocol_1(backend: &dyn
  TensorPreprocessing)` helper in `src/lib.rs` called by separate `#[test]`
  functions for each backend variant.
- Negative tests ("must abort") use `#[should_panic(expected = "...")]` with
  the expected substring of the panic message.

## RNG Seeding and Determinism

**This is the most important testing convention in a cryptographic codebase.**

**All cryptographic tests use seeded `ChaCha12Rng`:**
```rust
use rand_chacha::ChaCha12Rng;
use rand::SeedableRng;

let mut rng = ChaCha12Rng::seed_from_u64(42);
```

**Seed assignment strategy:**
- Each test function gets a unique fixed integer seed: `seed_from_u64(1)`,
  `seed_from_u64(2)`, ..., `seed_from_u64(5)` in `src/online.rs` tests;
  `seed_from_u64(7)`, `seed_from_u64(11)` in `src/sharing.rs` tests;
  `seed_from_u64(0xC0FFEE)` in `src/keys.rs` tests.
- Seeds are chosen once and never change — tests are fully deterministic.
- `ChaCha12Rng::from_seed([0; 32])` is used in `src/block.rs:462` (the
  `sigma_test`) as an alternative seeding form.

**Production code uses seeded RNG too:**
- `IdealBCot::new(seed_a, seed_b)` seeds the ideal bCOT with explicit integers.
- `TensorFpre::new(seed, ...)` seeds the ideal preprocessor.
- `IdealPreprocessingBackend::run` uses `TensorFpre::new(0, ...)` (fixed seed 0)
  internally — documented in `src/preprocessing.rs:131`.
- `ChaCha12Rng::seed_from_u64(42)` for gamma sampling; `seed_from_u64(43)` for
  uncompressed-path gamma; `seed_from_u64(44)` for input label generation.
- Seed constants are chosen to be distinct so that independently sampled fields
  are genuinely independent (not the same ChaCha stream).

**Non-deterministic RNG in integration-flavored tests:**
- `src/lib.rs` tests `test_semihonest_tensor_product` and
  `test_auth_tensor_product` call `rand::rng()` (the global unseeded RNG).
  These tests are NOT deterministic — they pass given any random delta. This is
  intentional: correctness is an algebraic identity that holds for all deltas,
  so determinism is not required.
- **Convention:** Use a seeded `ChaCha12Rng` when the test verifies a specific
  numeric result (e.g., MAC value). Use `rand::rng()` when the test verifies
  an invariant that holds for all inputs.

## Mocking

**Framework:** None. This codebase uses in-process ideal functionalities
instead of mocks.

**"Mock" pattern — Ideal trusted-dealer backends:**
- `IdealBCot` (`src/bcot.rs`) simulates the bCOT correlation in-process.
- `TensorFpre` (`src/auth_tensor_fpre.rs`) simulates the ideal preprocessing
  oracle in-process (both parties' state combined).
- `IdealPreprocessingBackend` / `UncompressedPreprocessingBackend`
  (`src/preprocessing.rs`) implement the `TensorPreprocessing` trait, enabling
  trait-dispatch tests via `let backend: &dyn TensorPreprocessing = &Ideal...`.

**What to "mock" (use ideal backend):**
- Preprocessing output that would require network round-trips in production.
  Always pass `IdealPreprocessingBackend` or `UncompressedPreprocessingBackend`
  to test online-phase functions.

**What NOT to mock:**
- Cryptographic primitives (`FixedKeyAes`, `Delta`, `Key::auth`). These are
  tested directly.
- MAC invariants. Always use `verify_cross_party(gen, eval, delta_a, delta_b)`
  from `src/auth_tensor_pre.rs:324` to check cross-party IT-MAC invariants.
  Never call `share.verify(delta)` on a cross-party share directly — it panics
  on correctly-formed cross-party shares (documented in `src/preprocessing.rs:540`).

## Fixtures and Factories

**Test data helpers (private to `mod tests`):**

```rust
// In src/auth_tensor_pre.rs mod tests
fn make_triples(n: usize, m: usize, count: usize) -> Vec<LeakyTriple> {
    let mut bcot = IdealBCot::new(42, 99);
    let mut triples = Vec::new();
    for seed in 0..count {
        let mut ltp = LeakyTensorPre::new(seed as u64, n, m, &mut bcot);
        triples.push(ltp.generate());
    }
    triples
}

// Field equality helper (AuthBitShare does NOT derive PartialEq)
fn shares_eq(a: &AuthBitShare, b: &AuthBitShare) -> bool {
    a.key == b.key && a.mac == b.mac && a.value == b.value
}
```

**Protocol pair factory (in `src/lib.rs mod tests`):**
```rust
fn verify_vector_sharing(clear_val: usize, gb_share: &Vec<Block>, ev_share: &Vec<Block>,
    delta: &Delta, n: usize) -> bool { ... }
fn verify_column_matrix_sharing(...) -> bool { ... }
fn verify_tensor_output(...) -> bool { ... }
```

**Parameterized protocol runner:**
```rust
fn run_full_protocol_1(backend: &dyn TensorPreprocessing) { ... }

#[test]
fn test_auth_tensor_product_full_protocol_1_ideal() {
    run_full_protocol_1(&IdealPreprocessingBackend);
}
#[test]
fn test_auth_tensor_product_full_protocol_1_uncompressed() {
    run_full_protocol_1(&UncompressedPreprocessingBackend);
}
```

**Location:**
- All fixtures are private functions inside `mod tests` in the same file.
- No external fixture files or separate `testdata/` directory.

## Benchmark Organization (`benches/benchmarks.rs`)

**Setup helpers** (outside `criterion_main!`):
```rust
fn setup_auth_gen(n, m, cf) -> AuthTensorGen { ... }
fn setup_auth_eval(n, m, cf) -> AuthTensorEval { ... }
fn setup_auth_pair(n, m, cf) -> (AuthTensorGen, AuthTensorEval) { ... }
fn gc_bytes_p1(n, m, cf) -> usize { ... }
fn gc_bytes_p2(n, m, cf) -> usize { ... }
```

**Parameter sweep:**
```rust
const BENCHMARK_PARAMS: &[(usize, usize)] = &[(64, 64), (128, 128), (256, 256)];
```
Commented-out entries `(4, 4)` through `(96, 96)` remain in the file for easy
re-enablement.

**Criterion group wiring:**
```rust
criterion_group!(preprocessing_benches, bench_preprocessing);
criterion_group!(online_benches, bench_online_p1, bench_online_p2);
criterion_main!(online_benches);  // only the online group is active
```
`preprocessing_benches` is declared but NOT in `criterion_main!` — it is
commented out of the active run. Add it back when preprocessing benchmarks
are needed.

**Network simulation in benches:**
- `SimpleNetworkSimulator` (from `benches/network_simulator.rs`) simulates
  100 Mbps transit deterministically: `(bytes * 8 * 1e9) / BANDWIDTH_BPS` ns.
- Each bench cell reports both compute time and simulated transit time.

## `should_panic` Tests (Negative / Abort Tests)

```rust
#[test]
#[should_panic(expected = "F_eq abort")]
fn test_check_differing_matrices_panics() {
    let a = BlockMatrix::new(2, 2);
    let mut b = BlockMatrix::new(2, 2);
    b[(0, 0)] = Block::new([1; 16]);
    check(&a, &b);
}
```

The `expected` string must match a substring of the panic message. Common abort
messages to match:
- `"F_eq abort"` (`src/feq.rs`)
- `"MAC mismatch in share"` (`src/sharing.rs`, `src/auth_tensor_pre.rs`)
- `"apply_permutation_to_triple: perm.len() must equal n"` (`src/auth_tensor_pre.rs`)
- `"compute_lambda_gamma: lambda_gb length must equal n*m"` (`src/auth_tensor_eval.rs`)

## Coverage

**Requirements:** None enforced. No tarpaulin or cargo-llvm-cov configured.

**Manual coverage strategy:** The code-review-fix cycle (see
`.planning/REVIEW.md`, commit `a141d6f`) checks that:
1. Each paper Construction step has at least one test in the module that
   implements it.
2. Each `#[should_panic]` negative test covers a distinct tamper path.
3. Both `IdealPreprocessingBackend` and `UncompressedPreprocessingBackend`
   exercise every property test (parameterized via `dyn TensorPreprocessing`).

**Running tests:**
```bash
cargo test 2>&1 | grep -E "test .* (ok|FAILED)"   # terse summary
cargo test -- --nocapture 2>&1 | head -100          # with print! output
```

## Test Count by Module (as of 2026-04-28)

| File | Test count |
|------|-----------|
| `src/preprocessing.rs` | 16 |
| `src/matrix.rs` | 12 |
| `src/auth_tensor_pre.rs` | 11 |
| `src/tensor_macro.rs` | 10 |
| `src/leaky_tensor_pre.rs` | 10 |
| `src/lib.rs` | 8 |
| `src/bcot.rs` | 7 |
| `src/auth_tensor_gen.rs` | 6 |
| `src/online.rs` | 5 |
| `src/block.rs` | 5 |
| `src/tensor_ops.rs` | 4 |
| `src/keys.rs` | 4 |
| `src/auth_tensor_eval.rs` | 4 |
| `src/sharing.rs` | 3 |
| `src/feq.rs` | 3 |
| `src/auth_tensor_fpre.rs` | 2 |
| `src/aes.rs` | 1 |
| `src/tensor_pre.rs` | 0 |
| `src/tensor_gen.rs` | 0 |
| `src/tensor_eval.rs` | 0 |
| `src/macs.rs` | 0 |
| `src/delta.rs` | 0 |

Modules with 0 tests (`tensor_gen.rs`, `tensor_eval.rs`, `tensor_pre.rs`,
`macs.rs`, `delta.rs`) are either simple newtype wrappers (tested via their
consumers) or earlier semi-honest implementations superseded by the
authenticated variants.

---

*Testing analysis: 2026-04-28*
