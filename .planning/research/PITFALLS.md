# Pitfalls Research — v1.1

**Domain:** Authenticated tensor garbling — online phase extension on an existing Rust library
**Researched:** 2026-04-23
**Confidence:** HIGH (based on paper reading + codebase audit; LOW for compressed preprocessing which the paper appendix marks as incomplete)

---

## Open() and MAC Verification Pitfalls

### Pitfall O1: Verifying the wrong party's delta (most common bug)

**What goes wrong:** Open() on a value `[v * delta_A]` is called with `delta_B` instead of `delta_A`, or vice versa. The MAC check passes only if `key XOR v*delta == mac`, so using the wrong delta always produces a mismatch — but the error message ("MAC mismatch") gives no hint about which delta was wrong.

**Why it happens:** The codebase has two deltas in play at all times: `delta_a` (garbler's, carried in `TensorFpreGen`) and `delta_b` (evaluator's, carried in `TensorFpreEval`). The cross-party layout of `AuthBitShare` is counterintuitive: a generator-side share is verified with `delta_b` (the *verifying party's* key), not `delta_a`. This is already documented in `src/sharing.rs` lines 30–50 and is the hardest invariant to hold mentally during new code.

**Specific danger point for v1.1:** Protocol 1 (5_online.tex §2, step 3–4) uses `open([l_w * D_gb])` to reveal masks for garbler input wires, and separately `open([l_w * D_ev])` for evaluator input wires and output decoding. Each uses a *different* delta. If the Open() helper takes a delta parameter, callers must supply it correctly at every call site.

**Consequences:** Open() silently produces garbage output if the verification is skipped (non-aborting variant). If verification is present, it panics or returns an error, but the root cause is invisible without per-site logging.

**Prevention:**
- Make the Open() function signature take a `delta: &Delta` parameter that is visibly distinct per call site — do not thread delta through global state.
- Write a test that calls Open() with the correct delta and one that supplies the wrong delta and asserts the latter fails. The codebase currently has no negative MAC verification tests (CONCERNS.md §Test Coverage Gaps: "No negative MAC verification tests").
- Reuse the `verify_cross_party` helper pattern from `src/leaky_tensor_pre.rs` tests — it already encodes the correct cross-party delta assignment and should be the model for any new Open() test.

**Detection warning signs:** Open() returns the wrong bit value (off-by-one in a toggle), tests pass for bit=0 but silently fail for bit=1 (because `key XOR 0*delta == key` regardless of delta, so the false-bit case cannot distinguish wrong-delta from right-delta).

---

### Pitfall O2: Treating `mac = key XOR bit*delta` as `mac = key XOR delta` unconditionally

**What goes wrong:** When computing what a MAC should be for a `1`-bit versus a `0`-bit, code incorrectly always XORs delta into the key. This produces correct MACs for `bit=1` and wrong MACs for `bit=0`, or vice versa, depending on the branch that was omitted.

**Why it happens:** The formula `mac = key XOR bit*delta` has two branches: `mac = key` when `bit=0` and `mac = key XOR delta` when `bit=1`. Code written as `mac = key XOR delta` (unconditionally) is off by one branch. This exact pattern already exists in `src/auth_tensor_gen.rs` lines 188–192 where `_gamma_share` was computed (now dead code) — that code used `if bit { key XOR delta } else { key }` correctly, but the pattern is easily inverted.

**Prevention:** Extract a `key.auth(bit, delta)` call (already exists in `src/keys.rs:66`) rather than inlining the XOR. Do not inline the branch; call `auth()`. Tests must cover both bit=0 and bit=1 cases explicitly.

---

### Pitfall O3: Revealing L_w to the garbler before the consistency check

**What goes wrong:** Protocol 1 (5_online.tex §2, Consistency check step 6) requires the evaluator to send `L_w` for each input/tensor-gate wire to the garbler. If this reveal happens *before* the garbler's circuit output is fixed, a malicious garbler can adaptively choose circuit ciphertexts after seeing `L_w`, breaking security.

**Why it matters for v1.1:** This is an ordering constraint: garble → evaluate → send L_w → CheckZero. In an in-process simulation (no network) the ordering is enforced by the programmer, not by message-passing. If Open() is implemented as a function callable at any time, it is easy to call it too early in a test harness.

**Prevention:** Structure the test for Protocol 1 as a sequential state machine: `garble_phase()` → `evaluate_phase()` → `consistency_check(masked_values)`. Do not expose L_w from the evaluator struct until `evaluate_phase()` returns.

---

### Pitfall O4: Forgetting that `delta_b` appears in the preprocessing output for the evaluator's masked values

**What goes wrong:** When decoding outputs (Protocol 1 step 9, `open([l_w * D_ev])` to reveal `l_w` to evaluator), the decoding opens values authenticated under `delta_ev`, not `delta_gb`. Code that reuses the same Open() wrapper from the garbler-input step (which opened `D_gb`-authenticated values) will use the wrong delta.

**Why it happens:** There are dual authenticated bits in the preprocessing: `([ l_w D_gb ], [ l_w D_ev ], [ l_w ])`. The current `TensorFpreGen/Eval` structs carry `delta_a` (garbler's = `D_gb`) and `delta_b` (evaluator's = `D_ev`). The output decoding step needs the `D_ev`-authenticated view, which is `delta_b` from `TensorFpreEval`. Conflating the two is natural because both fields look identical structurally.

**Prevention:** Name the two Open() invocations explicitly in comments: `// open D_gb-share to get l_w for garbler input encoding` vs `// open D_ev-share to get l_w for output decoding`. Check that the preprocessing material passed to each Open() call is from the correct side of the dual-authenticated pair.

---

## Consistency Check Pitfalls

### Pitfall C1: Wrong formula for c_gamma — missing the l_gamma* term

**What goes wrong:** The consistency check (Protocol 1 step 7) requires both parties to locally compute:

```
c_gamma = (L_alpha XOR l_alpha) ⊗ (L_beta XOR l_beta) XOR (L_gamma XOR l_gamma)
```

The full expansion is `v_alpha ⊗ v_beta XOR v_gamma` (the correctness residual), computed from linear combinations of the masks. The D_ev-shares of `c_gamma` are computed from preprocessing material: shares of `l_alpha`, `l_beta`, `l_gamma`, *and* `l_gamma* = l_alpha ⊗ l_beta`. Omitting the `l_gamma*` term (the tensor product of the masks) produces a structurally plausible formula that is wrong by exactly the `l_alpha ⊗ l_beta` correction.

**Why it happens:** The paper (5_online.tex Eq. after step 7) writes: "a linear combination of l_alpha, l_beta, l_gamma, l_gamma* with coefficients determined by the L's." The `l_gamma*` term is easy to miss because it is described inline rather than as a separate equation, and because `l_gamma*` is produced by the preprocessing but stored separately from `l_gamma`.

**Consequences:** CheckZero passes for the honest case (both sides agree on the same wrong formula), but a malicious garbler can forge `L_gamma` values that pass the check — the consistency check provides no security.

**Prevention:**
- Expand the formula algebraically before coding: `c_gamma = L_alpha ⊗ L_beta XOR L_alpha ⊗ l_beta XOR l_alpha ⊗ L_beta XOR l_alpha ⊗ l_beta XOR L_gamma XOR l_gamma`. Note that `l_alpha ⊗ l_beta = l_gamma*`, so this is `L_alpha ⊗ L_beta XOR ... XOR l_gamma*` — the `l_gamma*` share must come from preprocessing.
- The test for consistency check correctness must cover the case where the garbler sends a wrong `L_gamma` and verify that `c_gamma != 0` is detected.

---

### Pitfall C2: CheckZero run by evaluator only — garbler never sees it

**What goes wrong:** The paper specifies that the *evaluator* aborts if `c_gamma != 0` (Protocol 1 step 8). In an in-process simulation there is no abort — the evaluator side can silently produce a wrong result or just return a boolean. Code that forgets to propagate the abort (or converts it to a logged warning) lets a malicious garbler go undetected.

**Prevention:** CheckZero should panic or return `Err(ProtocolAbort)` — not return `false` silently. If the project uses panics for invariant violations (it does — see CONCERNS.md "Error Handling: Strategy: Panic-on-violation"), then `assert!(c_gamma_is_zero, "consistency check failed — evaluator aborts")` is the correct pattern.

---

### Pitfall C3: Checking the consistency of XOR wires (free gates) — not needed, but missing AND/tensor wires

**What goes wrong:** The consistency check is only required for tensor (AND) gate output wires (`andwires`). XOR gate outputs are handled by the free-XOR invariant and need no separate check. Code that iterates over *all* wires for the check adds wasted work; code that iterates over the wrong set (e.g., output wires instead of tensor-gate output wires) misses the gates that actually need checking.

**Why it happens:** The paper uses distinct wire sets (`inwires`, `andwires`, `outwires`). In the current codebase there is no circuit-level abstraction — the implementation handles a single tensor gate at a time. When extending to multi-gate circuits, the wire-set distinction must be maintained explicitly.

**Prevention:** Name the loop variable clearly: `for (alpha, beta, gamma) in tensor_gate_wires { ... }`. Add a comment citing the paper step number.

---

### Pitfall C4: c_gamma computation uses wrong delta for D_ev-shares

**What goes wrong:** The `c_gamma` shares are in the `D_ev` basis (the *evaluator's* delta). If code accidentally computes them in the `D_gb` basis (garbler's delta), the XOR combination of shares still produces a consistent pair of shares, but under the wrong delta — CheckZero comparing `[c_gamma * D_ev]` shares will silently agree on zero because the wrong delta is used consistently on both sides.

**Detection warning signs:** Consistency check passes for dishonest garbler inputs. The test for "garbler sends wrong L_gamma" passes the check instead of failing.

**Prevention:** The `[c_gamma * D_ev]` shares must be computed by combining the D_ev-authenticated preprocessing shares of `l_alpha`, `l_beta`, `l_gamma`, `l_gamma*` — not the D_gb-authenticated ones. These come from `TensorFpreEval.delta_b` (= D_ev). Add a compile-time comment or type alias distinguishing which preprocessing structs hold D_ev-authenticated vs D_gb-authenticated material.

---

## Preprocessing Trait Abstraction Pitfalls

### Pitfall P1: `LeakyTensorPre` uses `&'a mut IdealBCot` — the lifetime becomes a trait parameter

**What goes wrong:** `LeakyTensorPre<'a>` currently borrows `&'a mut IdealBCot` to enforce shared delta across all leaky triples. If a `PreprocessingBackend` trait is introduced to make `IdealBCot`, `IdealPreprocessing`, and future real-OT backends interchangeable, the lifetime `'a` either (a) propagates into the trait signature as `trait PreprocessingBackend<'a>`, forcing every caller to be lifetime-parameterized, or (b) is hidden by requiring `Box<dyn PreprocessingBackend>` with an implied `'static` bound that `IdealBCot` cannot satisfy because it is constructed locally.

**Why it happens:** Rust's `Box<dyn Trait>` has an implicit `'static` lifetime bound. A locally-constructed `IdealBCot` is not `'static` (it lives on the stack or in a non-'static scope). Wrapping it in `Box<dyn PreprocessingBackend>` fails to compile unless the trait is explicitly parameterized with `+ 'a` or the bound is overridden.

**The specific Rust 2024 concern:** Edition 2024 changed return-position `impl Trait` lifetime capture rules (RFC 3498). A function returning `impl PreprocessingBackend` now captures all in-scope lifetimes by default, which can cause otherwise-working code to break when the edition is upgraded from 2021 to 2024.

**Prevention:**
- Prefer `impl PreprocessingBackend` in function return position over `Box<dyn PreprocessingBackend>` where the concrete type is known at the call site (monomorphization, no heap allocation, no lifetime issues).
- If dynamic dispatch is truly needed (`Vec<Box<dyn PreprocessingBackend>>`), add an explicit lifetime bound: `Box<dyn PreprocessingBackend + '_>` to avoid the implicit `'static`.
- Define `IdealPreprocessing` (the trusted-dealer oracle) as a struct that owns all its data with no lifetime parameters — it avoids the `&'a mut` borrow entirely and is the easiest concrete type to place behind a trait.
- The `&'a mut IdealBCot` borrow in `LeakyTensorPre` should be considered an internal implementation detail of the real preprocessing path, not exposed through the trait interface.

---

### Pitfall P2: Trait requires `generate()` to return different types for different backends

**What goes wrong:** `IdealPreprocessing::generate()` should return `(TensorFpreGen, TensorFpreEval)` directly. `run_preprocessing()` (the real Pi_aTensor path) also returns `(TensorFpreGen, TensorFpreEval)`. But `LeakyTensorPre::generate()` returns `LeakyTriple`, which is an *intermediate* type — not the final preprocessing output. If the trait is defined as `fn generate(&mut self) -> (TensorFpreGen, TensorFpreEval)`, then `LeakyTensorPre` cannot implement it directly (it would need to run the full bucketing combiner internally, changing its current role).

**Prevention:** Define two separate traits:
- `PreprocessingBackend: fn produce(&mut self) -> (TensorFpreGen, TensorFpreEval)` — for top-level interchangeable backends (IdealPreprocessing, run_preprocessing-equivalent).
- Keep `LeakyTensorPre` as an internal building block, not exposed through the top-level trait.

This matches the existing code structure where `run_preprocessing` is the public entry point and `LeakyTensorPre` is a private implementation step.

---

### Pitfall P3: Object safety violations — associated types or generic methods in the trait

**What goes wrong:** If the `PreprocessingBackend` trait includes a generic method (`fn generate<R: Rng>(&mut self, rng: &mut R)`) or an associated type (`type Output`), the trait becomes non-object-safe and cannot be used as `dyn PreprocessingBackend`. This is a compile error that is easy to trigger accidentally when the trait is first sketched.

**Prevention:** Keep the trait interface simple: no generic methods, no associated types. Use `&mut dyn rand::RngCore` instead of `<R: Rng>` if RNG threading is needed. The current codebase passes explicit seeds to constructors (`seed: u64`) and uses internal `ChaCha12Rng` — this pattern is already compatible with a clean object-safe trait.

---

### Pitfall P4: Breaking existing test infrastructure when moving structs across modules

**What goes wrong:** v1.0 Phase 2 already moved `TensorFpreGen/Eval` from `auth_tensor_fpre.rs` to `preprocessing.rs`. The move updated import paths in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `auth_tensor_pre.rs`, and `benches/benchmarks.rs`. A v1.1 trait abstraction that further reorganizes these types risks creating circular imports (`preprocessing` importing from `ideal_preprocessing` importing from `preprocessing` for the types).

**Why it happens:** Rust's module system disallows circular `use` dependencies. When a new module (`ideal_preprocessing.rs`) needs to return types defined in `preprocessing.rs`, and `preprocessing.rs` imports from `ideal_preprocessing.rs`, the cycle is immediate.

**Prevention:** Keep `TensorFpreGen`/`TensorFpreEval` in a low-level module (`preprocessing.rs` or a dedicated `fpre_types.rs`) that no other protocol module needs to import from. All protocol modules import *from* this module, never into it. The trait definition lives in a separate module (`preprocessing_trait.rs` or similar) that imports from `fpre_types.rs`.

---

### Pitfall P5: Forgetting that `combine_leaky_triples` asserts all deltas match

**What goes wrong:** The existing `combine_leaky_triples` in `src/auth_tensor_pre.rs` has an `assert_eq!` that verifies all leaky triples share the same `delta_a` / `delta_b`. If a preprocessing trait abstraction allows different backends to be mixed (e.g., some triples from `IdealPreprocessing`, some from `LeakyTensorPre`), the delta assertion will fire at runtime.

**Prevention:** The trait must guarantee that all triples produced for a single preprocessing invocation share the same global correlation pair `(delta_a, delta_b)`. This should be enforced structurally: the backend is initialized with a fixed `(delta_a, delta_b)` pair (as `IdealBCot` is today), and all triples it produces use that pair. The trait's contract should document this invariant explicitly.

---

## Compressed Preprocessing Pitfalls

### Pitfall CP1: The appendix is a draft — Pi_cpre protocol steps are commented out

**What goes wrong:** `appendix_cpre.tex` contains the compressed preprocessing protocol (`Pi_cpre`) entirely in commented-out LaTeX (lines 71–156). Only the ideal functionality `F_cpre` is presented in active text. The protocol steps (all 15+ numbered steps) are commented out with `%`. There is no complete, authoritative protocol specification to implement from.

**Consequences:** Implementing based on the commented-out draft risks implementing a version the authors have superseded or know to be wrong. The authors note they need to "discuss some things with David" and plan to "rewrite to simplify functionality and notation" (lines 2–3).

**Confidence:** LOW — the appendix cannot be trusted as an implementation specification.

**Prevention:** Do not implement Pi_cpre for v1.1. Implement only `F_cpre` (the ideal functionality), which is fully specified. The ideal functionality approach is consistent with how `IdealBCot` stands in for a real OT — use `IdealCpre` as the backend and defer Pi_cpre to v2.

---

### Pitfall CP2: Compression parameter sigma changes from 2*SSP to O(SSP * log(kappa))

**What goes wrong:** The CWYY23 paper uses `sigma = 2*SSP`. This codebase's appendix changes it to `sigma = O(SSP * log(kappa))` for tensor-gate selective failure resistance (appendix line 7). The compressed mask vector length `L` grows logarithmically with `sigma`. Using `sigma = 2*SSP` (the CWYY23 value) when the paper requires `sigma = O(SSP * log(kappa))` produces a smaller `L` that is *insufficient* for tensor-gate security.

**Prevention:** When implementing `F_cpre`, parameterize it with the correct `sigma` value and compute `L` from the formula on appendix line 28: `L = ceil(SSP + sigma * log(e*n/sigma) + log(sigma)/2)`. Do not copy the constant `sigma = 2*SSP` from CWYY23 directly.

---

### Pitfall CP3: Tensor triples from AND triples — dimension mismatch in the conversion

**What goes wrong:** The appendix (lines 14–16) explains that one tensor triple `(l_x, l_y, l_x ⊗ l_y)` requires `n*m` AND triples where `n = |l_x|`, `m = |l_y|`. The product is `(l_x ⊗ l_y)_{ij} = (l_x)_i * (l_y)_j`. If the AND-triple index ordering does not match the column-major convention (`j*n + i`) used throughout this codebase, the resulting tensor triple has correct values but in wrong positions, silently corrupting the online phase.

**Prevention:** Explicitly document and test the index mapping from AND triples to tensor triple entries. The column-major invariant (`j*n + i` for the `(i,j)` entry) is the canonical layout across the entire codebase and must be enforced in the conversion.

---

## Benchmark Measurement Pitfalls

### Pitfall B1: Benchmarking stateful structs that consume preprocessing material on first call

**What goes wrong:** `AuthTensorGen` is constructed from `TensorFpreGen` (moved, not cloned). After `garble_first_half()` + `garble_second_half()` + `garble_final()`, the internal `first_half_out` and `second_half_out` matrices contain computed values from the *previous* iteration. Re-running `garble_first_half()` overwrites them. The benchmark uses `iter` (not `iter_batched`), meaning the same `generator` struct is reused across all iterations. This is correct as long as the garbling functions do not consume the preprocessing material (they do not — they borrow it). But if new Online phase functions consume `self` or take `&mut self` in a way that invalidates state, using `iter` will benchmark subsequent calls on corrupted state.

**Current benchmark pattern (from TESTING.md):**
```rust
b.iter(|| {
    let (_first_levels, _first_cts) = generator.garble_first_half();
    let (_second_levels, _second_cts) = generator.garble_second_half();
    generator.garble_final();
})
```

**Risk:** If Open() or the consistency check is added to the benchmark loop, and either function clears or mutates the preprocessing material in a non-idempotent way, later iterations benchmark a degraded state. The criterion output will show progressively increasing times or anomalous outliers, not a clean distribution.

**Prevention:** For any function that consumes or destructively modifies preprocessing material, use `iter_batched` with a setup closure that reconstructs the preprocessing state per-batch. For idempotent functions, `iter` is fine.

---

### Pitfall B2: Compiler eliminating the computation — missing `black_box` on outputs

**What goes wrong:** If `garble_first_half()` returns `(Vec<Vec<(Block,Block)>>, Vec<Vec<Block>>)` and the result is bound to a `_`-prefixed variable, the Rust compiler may optimize the entire call away (dead-store elimination). Criterion's statistics then measure near-zero time. The current benchmark assigns to `(_first_levels, _first_cts)` — the underscore prefix signals to Rust that the values are intentionally unused, but does not prevent dead-code elimination in release builds.

**Why it happens for crypto code:** Cryptographic functions compute side-effect-free outputs. Without external side effects (network sends, file writes), the compiler is free to elide them in `--release` mode even with Criterion's wrapper.

**Evidence from the research:** The `gendignoux.com/blog/2022/01/31/rust-benchmarks.html` analysis demonstrates that `black_box` only forces a write to the stack; it does not prevent the compiler from seeing that the *computation leading to* the value is unused.

**Prevention:**
- Wrap return values in `std::hint::black_box(...)` at the use site. Example:
  ```rust
  b.iter(|| {
      let out1 = std::hint::black_box(generator.garble_first_half());
      let out2 = std::hint::black_box(generator.garble_second_half());
      generator.garble_final();
      black_box(&generator.first_half_out);
  });
  ```
- Also apply `black_box` to the *input* of the garbling function if the compiler can see the inputs are constants (e.g., `X_INPUT = 0b1101` is visible at compile time).

---

### Pitfall B3: Async benchmark overhead from `to_async` and `tokio::time::sleep` dwarfs crypto cost

**What goes wrong:** The networking benchmarks use `b.to_async(&*RT).iter_batched(setup_fn, bench_fn, BatchSize::SmallInput)`. The `SimpleNetworkSimulator::send_size_with_metrics` calls `tokio::time::sleep(duration)` where `duration` is computed from `bytes / bandwidth`. For small gate sizes (4x4, 8x8) the simulated latency is in microseconds but the async task-switch overhead from `tokio` can also be in microseconds, making the latency simulation unreliable at small sizes.

**Prevention:**
- For wall-clock benchmarks of the *cryptographic* computation only (Open(), CheckZero), use `b.iter(...)` without async. Async is only justified when the simulated network latency dominates.
- Add a comment in the benchmark indicating whether the measurement includes simulated network time or is crypto-only.
- If the goal is to benchmark Open() and consistency check as protocol steps, run them in a synchronous non-async benchmark and report `Throughput::Elements(n*m)`.

---

### Pitfall B4: `iter_batched` overhead for small-input setups at `BatchSize::SmallInput`

**What goes wrong:** `BatchSize::SmallInput` in Criterion uses a batch size of 1 for very fast functions. If `setup_fn` runs in the same order of magnitude as `bench_fn` (both sub-microsecond), the per-batch setup overhead is included in Criterion's overhead accounting but may still skew results. The criterion issue #475 documents this.

**Prevention:** For cryptographic operations that take 100ns–1µs (e.g., a single MAC check), use `BatchSize::LargeInput` or `BatchSize::NumBatches(N)` to amortize setup cost. Alternatively, pre-compute all inputs in the benchmark setup and use `iter` with pre-built state.

---

### Pitfall B5: Throughput unit mismatch — `Elements(n*m)` vs `Bytes(bits)` 

**What goes wrong:** The current benchmark reports `Throughput::Elements(n*m)` for the garbling benchmark. For Open() and consistency check, the appropriate unit is different: Open() processes one authenticated bit per call (throughput = 1 element, or more usefully, the number of bytes verified). Mixing `Elements` and `Bytes` throughput units across benchmark groups makes Criterion's "throughput" columns meaningless in comparison.

**Prevention:** Decide on a throughput unit convention before adding new benchmark groups. For consistency check: `Throughput::Elements(num_tensor_gates)` is the right unit (one check per tensor gate output). For Open(): `Throughput::Elements(num_wires_opened)` or `Throughput::Bytes(num_wires_opened * 16)` (one 128-bit MAC per wire). Document the choice in the benchmark group comment.

---

### Pitfall B6: Timing the setup (preprocessing) inside the benchmark loop

**What goes wrong:** If the benchmark for Open() calls `TensorFpre::new()` and `generate_for_ideal_trusted_dealer()` inside the timed loop, the measurement includes trusted-dealer preprocessing cost, which is not the operation being benchmarked. This produces a number that is neither the preprocessing cost nor the online cost — it is an artifact.

**Why it happens:** `setup_auth_gen` and `setup_auth_eval` are currently called *outside* the `b.iter_batched` closure (they are in the setup function). But if a new benchmark for the full online protocol is written naively as:

```rust
b.iter(|| {
    let mut fpre = TensorFpre::new(0, n, m, 1);
    fpre.generate_for_ideal_trusted_dealer(x, y);
    let (gen, eval) = fpre.into_gen_eval();
    // ... garble, evaluate, open, check ...
})
```

all steps including trusted-dealer setup are timed.

**Prevention:** Always use `iter_batched` with a separate `setup_fn` closure for any preprocessing. The setup closure should produce the preprocessed material; the bench closure should perform only the online phase steps. The existing `bench_preprocessing` benchmark correctly isolates `run_preprocessing` as the *subject* of a separate group; follow the same pattern for online phase benchmarks.

---

## Phase Assignment

| Pitfall | Severity | Assign To |
|---------|----------|-----------|
| O1: Wrong delta in Open() | Critical | Open() implementation phase (v1.1 Phase 1) |
| O2: Unconditional delta XOR | High | Open() implementation phase (v1.1 Phase 1) |
| O3: Reveal L_w too early | High | Protocol 1 integration phase (v1.1 Phase 2) |
| O4: Wrong delta for output decoding | High | Open() implementation phase (v1.1 Phase 1) |
| C1: Missing l_gamma* in c_gamma | Critical | Consistency check phase (v1.1 Phase 2) |
| C2: CheckZero not aborting | High | Consistency check phase (v1.1 Phase 2) |
| C3: Wrong wire set for check | Medium | Consistency check phase (v1.1 Phase 2) |
| C4: D_gb vs D_ev confusion in c_gamma | Critical | Consistency check phase (v1.1 Phase 2) |
| P1: Lifetime propagation from IdealBCot | High | Preprocessing trait phase (v1.1 Phase 3) |
| P2: Trait return type mismatch | Medium | Preprocessing trait phase (v1.1 Phase 3) |
| P3: Object safety violation | Medium | Preprocessing trait phase (v1.1 Phase 3) |
| P4: Circular imports on struct move | Medium | Preprocessing trait phase (v1.1 Phase 3) |
| P5: Delta mismatch across backends | High | Preprocessing trait phase (v1.1 Phase 3) |
| CP1: Pi_cpre is a draft — do not implement | Critical | Compressed preprocessing feasibility check (v1.1 Phase 4, if included) |
| CP2: Wrong sigma parameter | High | Compressed preprocessing phase (v1.1 Phase 4, if included) |
| CP3: AND-to-tensor index ordering | Medium | Compressed preprocessing phase (v1.1 Phase 4, if included) |
| B1: Stateful struct reuse across iterations | High | Benchmark cleanup phase (any benchmark phase) |
| B2: Missing black_box on outputs | High | Benchmark cleanup phase (any benchmark phase) |
| B3: Async overhead dwarfs crypto | Medium | Benchmark cleanup phase (any benchmark phase) |
| B4: iter_batched overhead at SmallInput | Low | Benchmark cleanup phase (any benchmark phase) |
| B5: Throughput unit mismatch | Medium | Benchmark cleanup phase (any benchmark phase) |
| B6: Setup timed inside loop | High | Benchmark cleanup phase (any benchmark phase) |

**Phase ordering rationale:**
- Open() pitfalls (O1–O4) must be resolved before consistency check (C1–C4) because the consistency check's CheckZero depends on correctly opened shares.
- Preprocessing trait pitfalls (P1–P5) are independent of Open()/check but should come after the output types (`TensorFpreGen`/`TensorFpreEval`) are stable — do not refactor the trait boundary while simultaneously changing the struct fields.
- Compressed preprocessing (CP1–CP3) should be deferred or scoped only to `F_cpre` (ideal functionality) — do not implement Pi_cpre from the draft appendix.
- Benchmark pitfalls (B1–B6) apply to any phase that adds benchmark groups and should be reviewed at the start of any benchmarking work, not discovered at the end.

---

## Sources

- Paper: `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex` — Protocol 1 specification, consistency check formula
- Paper: `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_cpre.tex` — F_cpre functionality; Pi_cpre draft (commented out)
- Codebase: `src/sharing.rs` — IT-MAC invariant, verify_cross_party pattern
- Codebase: `.planning/codebase/CONCERNS.md` — existing known gaps, missing MAC verification, gamma dead code, delta-mismatch risk
- Codebase: `.planning/codebase/ARCHITECTURE.md` — cross-party delta layout, AuthBitShare structure
- Codebase: `src/auth_tensor_gen.rs` lines 188–192 — existing gamma dead code pattern (MEDIUM confidence that gamma will need to be re-introduced for output authentication)
- Codebase: `benches/benchmarks.rs` — current benchmark structure, async wrapper pattern
- Guillaume Endignoux (2022): https://gendignoux.com/blog/2022/01/31/rust-benchmarks.html — black_box pitfalls
- Criterion issue #475: https://github.com/bheisler/criterion.rs/issues/475 — iter_batched overhead
- Rust RFC 3498 (2024): https://rust-lang.github.io/rfcs/3498-lifetime-capture-rules.html — RPIT lifetime capture rule changes in edition 2024
- quinedot.github.io/rust-learning/dyn-trait-lifetime.html — dyn Trait implicit 'static bound pitfalls
