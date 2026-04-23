# Stack Research — v1.1

**Researched:** 2026-04-23
**Confidence:** HIGH (all findings grounded in existing code + paper text; no speculative dependencies)

---

## New Capabilities Needed

Four distinct capability additions. Each is analyzed below for stack implications.

1. **Preprocessing trait abstraction** — a common Rust interface that `TensorFpre` (ideal), `TensorFpreGen/Eval` (real uncompressed), and any future compressed backend all satisfy, so `AuthTensorGen`/`AuthTensorEval` can be written once against the trait.
2. **Open() and consistency check** — the `open([\l_w D_gb])` operation and `CheckZero({[c_gamma D_ev]})` from Protocol 1, §5 of the paper. Purely local XOR arithmetic on existing `AuthBitShare` / `Block` values; no new cryptographic primitives.
3. **Compressed preprocessing** — `F_cpre` (appendix_cpre.tex). At the level of paper detail present (the main body is a commented-out functionality with a `TODO: rewrite` note from the authors), only the ideal oracle (`IdealCompressedPre`) is in scope for v1.1. The full `Pi_cpre` protocol requires DVZK, EQ-check, and a COT-to-dual-auth conversion that are not implemented and should be deferred.
4. **Wall-clock throughput benchmarks** — replacing the current async `to_async`/`iter_batched` pattern with synchronous `iter_custom` using `std::time::Instant` for garbling throughput (gates/sec, bits/sec).

---

## Recommended Patterns

### 1. Preprocessing Trait — Use Static Dispatch (Generics), Not `dyn`

**Pattern:** Define a `PreprocessingOutput` struct pair (gen-side and eval-side) and a `trait Preprocessing` that produces them. Wire `AuthTensorGen<P: Preprocessing>` generically.

**Why generics over `dyn Trait`:** The preprocessing backend is selected at construction time and never changes at runtime. Generics give zero-cost dispatch and let the compiler inline the backend's `generate()` call into the garbling loop. `dyn Trait` adds a vtable indirection that is measurable at benchmark scale (these functions are called in tight loops). The mpz-dev reference (`garble/src/protocol/semihonest.rs`) follows this same pattern: `Garbler<IdealCOTSender>` and `Evaluator<IdealCOTReceiver>` are generic over their OT backend, never `dyn`.

**Concrete design:**

```rust
// In preprocessing.rs or a new trait module

/// Output types are already defined: TensorFpreGen, TensorFpreEval.
/// Add a trait that any backend must satisfy.
pub trait PreprocessingBackend {
    /// Generate one authenticated tensor triple for an (n, m) gate.
    fn generate(n: usize, m: usize, chunking_factor: usize)
        -> (TensorFpreGen, TensorFpreEval);
}

// Ideal (trusted-dealer) backend — wraps existing TensorFpre
pub struct IdealPreprocessing;
impl PreprocessingBackend for IdealPreprocessing { ... }

// Real uncompressed backend — wraps existing run_preprocessing
pub struct UncompressedPreprocessing;
impl PreprocessingBackend for UncompressedPreprocessing { ... }

// Ideal compressed backend (v1.1 scope only)
pub struct IdealCompressedPreprocessing;
impl PreprocessingBackend for IdealCompressedPreprocessing { ... }
```

**IMPORTANT:** `TensorFpreGen` and `TensorFpreEval` are the right output types — they are already the common representation consumed by `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval`. The trait does not need a new output type; it just needs to produce the existing structs. Do not introduce a new `PreprocessingOutput` wrapper type.

**Interchangeability:** Callers (benchmarks, tests, the 2PC protocol driver) parameterize over `P: PreprocessingBackend`. Swapping backends requires changing one type parameter, not refactoring call sites.

---

### 2. Open() and Consistency Check — Pure Local Computation, No New Crates

**From the paper (Protocol 1, §5):**

- `open([l_w D_gb])`: Each party holds an `AuthBitShare`. Open reveals the bit by having both parties exchange their share's `value` field, then XOR them. In the in-process simulator, this is a function `fn open(gen_share: AuthBitShare, eval_share: AuthBitShare) -> bool` — one XOR.
- `CheckZero({[c_gamma D_ev]})`: For each tensor gate output wire, compute `c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)` as a linear combination of D_ev-shares (local arithmetic on `AuthBitShare` fields). Verify the reconstructed `Block` is zero.

**Pattern:** Free functions in a new `online.rs` module (or in `auth_tensor_gen.rs`/`auth_tensor_eval.rs`). No trait, no struct. Signatures:

```rust
// Reveals the bit value; both parties' shares required (in-process model)
pub fn open(gen: &AuthBitShare, eval: &AuthBitShare) -> bool {
    gen.value ^ eval.value
}

// Returns Ok(()) if all c_gamma reconstruct to zero, Err(wire_index) on failure
pub fn check_zero(
    c_shares_gen: &[AuthBitShare],  // garbler's D_ev-shares of c_gamma
    c_shares_eval: &[AuthBitShare], // evaluator's D_ev-shares
) -> Result<(), usize> { ... }
```

The `c_gamma` computation itself is XOR-linear in `l_alpha, l_beta, l_gamma, l_gamma_star` (all held as `AuthBitShare` with D_ev MACs). It needs no hash, no PRG — just XOR of `Block` fields. The existing `AuthBitShare`, `Block`, and `Delta` types cover this completely.

**No new crates needed for this feature.**

---

### 3. Compressed Preprocessing — Ideal Oracle Only for v1.1

**What the paper says:** `appendix_cpre.tex` is explicitly marked `\nakul{rewrite to simplify functionality and notation, need to discuss with David}` and the full `Pi_cpre` protocol is entirely commented out in the source. The active text defines only the ideal functionality `F_cpre`.

**In-scope for v1.1:** `IdealCompressedPre` — a trusted dealer that samples compressed wire masks (a matrix `M` of size `n x L` where `L = ceil(ssp + sigma * log(en/sigma) + log(sigma)/2)`) and distributes dual-authenticated AND triples. For tensor triples, one tensor triple requires `n*m` AND triples composed element-wise: `(l_x ⊗ l_y)_{ij} = (l_x)_i * (l_y)_j`.

**Out-of-scope for v1.1:** The full `Pi_cpre` protocol requires DVZK (designated-verifier zero-knowledge), an EQ-check functionality, and a B2F (bit-to-field) conversion — none of which exist in the codebase. Do not implement these.

**Implementation approach:** `IdealCompressedPre` is structurally similar to `TensorFpre` (the existing ideal). It samples `b_star <- F_2^L` uniformly, computes `b = M * b_star` (matrix-vector product over GF(2)), then generates dual-authenticated bits for each AND triple via `build_share`. The output format is the same `TensorFpreGen`/`TensorFpreEval` pair. Use `rand` for the matrix sampling (already a dependency).

**GF(2) matrix multiply:** `b = M * b_star` is a sequence of dot products over GF(2) — popcount parity. Implement as a free function using `u64` chunks for efficiency; no new crate needed. For `n` up to 256 and `L` up to ~300 bits, this is trivially fast.

---

### 4. Wall-Clock Benchmarks — Use `iter_custom` in Criterion

**Current situation:** The existing benchmarks use `b.to_async(&*RT).iter_batched(...)`. This correctly measures wall time for the online garbling phase (which is synchronous CPU work wrapped in async only because of `SimpleNetworkSimulator`). The `bench_preprocessing` benchmark already uses `Throughput::Elements` and `measurement_time`. These are good.

**What needs to change:** The online garbling benchmarks currently mix real garbling work with `SimpleNetworkSimulator` overhead in the same timed iteration. For wall-clock garbling throughput (gates/sec without networking), use `iter_custom` with `std::time::Instant` to time only the garbling + evaluation steps:

```rust
use std::time::Instant;

group.throughput(Throughput::Elements(n_gates as u64));
group.bench_function(BenchmarkId::new("garble_throughput", format!("{}x{}", n, m)), |b| {
    b.iter_custom(|iters| {
        let mut total = std::time::Duration::ZERO;
        for _ in 0..iters {
            let (gen, eval) = setup(n, m, chunking_factor);
            let start = Instant::now();
            // Only the garbling + evaluation; no network
            let (lvls, cts) = gen.garble_first_half();
            eval.evaluate_first_half(lvls, cts);
            // ... second half, final
            total += start.elapsed();
        }
        total
    });
});
```

**Why `iter_custom` over `iter_batched`:** `iter_batched` measures setup + work together unless the batch is large enough that setup is amortized. `iter_custom` gives explicit control: time exactly what you want. This is the criterion-idiomatic way to benchmark operations where setup must be excluded. The tokio runtime and `to_async` wrappers are not needed for purely synchronous garbling work; dropping them removes ~microseconds of overhead per iteration that distort throughput at small (n, m).

**Keep the async preprocessing benchmark as-is.** `run_preprocessing` is correctly measured with `iter_batched` since setup is the operation.

**Criterion is already at 0.7.0 in Cargo.toml.** `iter_custom` has been stable since 0.3.x. No version change needed.

---

## Crate Additions

**None required.**

All features are implementable with the existing dependency set:

| Feature | Uses |
|---------|------|
| Preprocessing trait | Rust generics, existing `TensorFpreGen`/`TensorFpreEval` |
| Open() / CheckZero | `AuthBitShare`, `Block`, `Delta` — all existing |
| `IdealCompressedPre` | `rand` (already dep), `rand_chacha` (already dep) |
| Wall-clock benchmarks | `criterion 0.7.0` (already dep), `std::time::Instant` |

The `blake3` crate is already declared but unused. It remains dormant; do not add it to the open/consistency check path (FIXED_KEY_AES is the correct primitive there per existing convention).

---

## Integration Points

### AuthTensorGen / AuthTensorEval

Both structs are already parameterized by their input: `new_from_fpre_gen(TensorFpreGen)` and `new_from_fpre_eval(TensorFpreEval)`. Adding the `PreprocessingBackend` trait does not require changing these constructors. The trait's `generate()` output feeds directly into the existing constructors. No struct fields change.

### Open() placement

`open()` is called during input encoding (Protocol 1, step 3) and output decoding (step 8) — operations that happen around garbling, not inside `AuthTensorGen`/`AuthTensorEval`. Place `open()` as a free function in a new `online.rs` module (exported from `lib.rs`). Do not put it inside `AuthTensorGen`/`AuthTensorEval` — those structs own only the garbling phase.

### CheckZero placement

`CheckZero` is called after evaluation (Protocol 1, steps 6-7). It takes the evaluator's `D_ev`-authenticated shares of `c_gamma` values. These shares are computed locally from the preprocessing output already held by `AuthTensorEval`. The check function belongs in `online.rs` alongside `open()`.

### Benchmark wiring

New wall-clock benchmarks extend `benches/benchmarks.rs`. The preprocessing benchmark group already exists (`bench_preprocessing`). Add a new group `bench_garble_throughput` using `iter_custom`. Both groups coexist in `criterion_group!`.

### Compressed ideal backend wiring

`IdealCompressedPre::generate(n, m, chunking_factor)` returns `(TensorFpreGen, TensorFpreEval)` — same as `TensorFpre::generate_for_ideal_trusted_dealer` + `into_gen_eval()`. Place it in a new file `compressed_pre.rs` alongside `auth_tensor_fpre.rs`. Export from `lib.rs` gated with a comment marking it as the ideal oracle only.

---

## What NOT to Add

**`dyn Trait` for preprocessing dispatch.** There is no runtime polymorphism requirement. Benchmarks and tests select a backend at compile time. Using `dyn PreprocessingBackend` would require boxing and heap allocation for each `generate()` call — measurable overhead in the preprocessing benchmark.

**Tokio for wall-clock garbling benchmarks.** The online garbling path (`garble_first_half`, `evaluate_first_half`, etc.) is entirely synchronous. The current use of `to_async(&*RT)` in the online benchmarks exists only because `SimpleNetworkSimulator` is `async`. For pure throughput benchmarks without networking, drop the async wrapper entirely and use `iter_custom`.

**Full `Pi_cpre` protocol.** The paper's compressed preprocessing appendix is explicitly incomplete (commented-out protocol, `TODO: rewrite` note). Implementing the full protocol requires DVZK, EQ-check, and B2F — not present in this codebase. Scope v1.1 to the ideal oracle only; defer the real protocol to v2.

**A new `PreprocessingOutput` wrapper type.** `TensorFpreGen` and `TensorFpreEval` are already the right types. Adding a wrapper would require touching every existing call site that uses these structs.

**Rayon or parallel iterators.** The tensor garbling and preprocessing loops are sequential as written in the paper. Introducing parallelism now would complicate correctness verification without protocol-level justification.

**Serde on new types.** None of the new types (`IdealCompressedPre`, `IdealPreprocessing` wrapper) need serialization. The existing `Block: Serialize` is sufficient for any wire-format needs.

**`thiserror` error enums for Open/CheckZero.** A `Result<(), usize>` (returning the failing wire index) is sufficient for the in-process consistency check. `thiserror` is reserved for structured error types exposed at library boundaries; this is internal protocol logic.
