<!-- refreshed: 2026-04-28 -->
# Architecture

**Analysis Date:** 2026-04-28

## System Overview

```text
┌───────────────────────────────────────────────────────────────────────┐
│                         lib.rs (crate root)                           │
│  CSP=128, SSP=40 constants; assemble_gate_semantics_shares;           │
│  assemble_e_input_wire_shares_p1; assemble_c_alpha_beta_shares_p2     │
└────────────────────────────┬──────────────────────────────────────────┘
                             │
         ┌───────────────────┼──────────────────────┐
         ▼                   ▼                      ▼
┌─────────────────┐ ┌─────────────────┐  ┌──────────────────────────┐
│  Preprocessing  │ │  Online Phase   │  │ Primitive / Crypto Layer │
│                 │ │                 │  │                          │
│ auth_tensor_    │ │ auth_tensor_    │  │ block.rs  delta.rs       │
│   fpre.rs       │ │   gen.rs  (P1)  │  │ keys.rs   macs.rs        │
│ preprocessing   │ │ auth_tensor_    │  │ aes.rs    sharing.rs     │
│   .rs           │ │   eval.rs  (P2) │  │ matrix.rs                │
│ leaky_tensor_   │ │ online.rs       │  │                          │
│   pre.rs        │ │                 │  │                          │
│ auth_tensor_    │ │                 │  │                          │
│   pre.rs        │ └─────────────────┘  └──────────────────────────┘
│ bcot.rs feq.rs  │
└─────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Tensor Gate Substrate                                               │
│  tensor_pre.rs   tensor_gen.rs   tensor_eval.rs                     │
│  tensor_ops.rs   tensor_macro.rs tensor_ops.rs                      │
└─────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Harnesses                                                           │
│  benches/benchmarks.rs   benches/network_simulator.rs               │
│  tools/parse_results.py  tools/comparison_table.py                  │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Block | 128-bit GF(2^128) word; all crypto operations | `src/block.rs` |
| Delta | Global correlation key wrapper (LSB invariant) | `src/delta.rs` |
| Key / Mac | IT-MAC sender key and receiver MAC newtypes | `src/keys.rs`, `src/macs.rs` |
| AuthBitShare | One party's view of a BDOZ-style authenticated bit | `src/sharing.rs` |
| AuthBit | Both parties' views of an authenticated bit (ideal/test use) | `src/sharing.rs` |
| InputSharing | Wire-label pair `(gen_share, eval_share)` for XOR-shared inputs | `src/sharing.rs` |
| FixedKeyAes | Global singleton fixed-key AES; TCCR/CCR/CR hash constructions | `src/aes.rs` |
| BlockMatrix / KeyMatrix | Column-major dense 2-D matrix over Block or Key | `src/matrix.rs` |
| TensorFpre | Ideal trusted-dealer functionality (in-process, insecure) | `src/auth_tensor_fpre.rs` |
| TensorFpreGen / TensorFpreEval | Preprocessing output structs consumed by online structs | `src/preprocessing.rs` |
| TensorPreprocessing (trait) | Object-safe interface unifying ideal and real backends | `src/preprocessing.rs` |
| IdealPreprocessingBackend | Wraps TensorFpre; used in tests and benchmarks | `src/preprocessing.rs` |
| UncompressedPreprocessingBackend | Wraps real Pi_aTensor' (Construction 4) | `src/preprocessing.rs` |
| IdealBCot | Ideal boolean correlated-OT (in-process; shares delta across triples) | `src/bcot.rs` |
| LeakyTensorPre | Pi_LeakyTensor (Construction 2, Appendix F) — produces LeakyTriple | `src/leaky_tensor_pre.rs` |
| LeakyTriple | Output of one Pi_LeakyTensor run; both parties' x/y/Z shares | `src/leaky_tensor_pre.rs` |
| two_to_one_combine | Construction 3 two-to-one bucketing combiner step | `src/auth_tensor_pre.rs` |
| combine_leaky_triples | Full bucket combining from many leaky to one authenticated triple | `src/auth_tensor_pre.rs` |
| Feq | Ideal F_eq matrix-equality check; panics on mismatch | `src/feq.rs` |
| tensor_garbler / tensor_evaluator | Paper Construction 1 GGM-tree macro primitives | `src/tensor_macro.rs` |
| tensor_ops functions | Low-level GGM seed expansion and unary outer-product routines | `src/tensor_ops.rs` |
| SemiHonestTensorPre | Semi-honest (single-delta) preprocessing (legacy / reference path) | `src/tensor_pre.rs` |
| TensorProductGen / TensorProductEval | Semi-honest online garbler/evaluator | `src/tensor_gen.rs`, `src/tensor_eval.rs` |
| AuthTensorGen | Maliciously-secure online garbler (P1 and P2 variants) | `src/auth_tensor_gen.rs` |
| AuthTensorEval | Maliciously-secure online evaluator (P1 and P2 variants) | `src/auth_tensor_eval.rs` |
| check_zero / hash_check_zero | Online consistency-check primitives under a delta | `src/online.rs` |
| assemble_*_shares | Crate-root helpers assembling AuthBitShare vecs for CheckZero | `src/lib.rs` |

## Pattern Overview

**Overall:** Paper-faithful two-party MPC simulation (garbler P1 / evaluator P2) with a clear separation between a preprocessing phase (offline) and an online phase.

**Key Characteristics:**
- All cross-party state lives in plain structs (no network sockets); the codebase simulates a real protocol in-process for testing and benchmarking.
- Two global correlation keys: `delta_a` (garbler, LSB=1) and `delta_b` (evaluator, LSB=0), satisfying `lsb(delta_a XOR delta_b) == 1` (required by Pi_LeakyTensor §F).
- Column-major indexing throughout: `n*m` vectors use `index = j*n + i` (j = column, i = row).
- Protocol variant split: `garble_first_half` / `garble_final` = Protocol 1 (narrow ciphertexts); `garble_first_half_p2` / `garble_final_p2` = Protocol 2 (wide/D_ev ciphertexts).

## Layers

**Primitive / Crypto Layer:**
- Purpose: 128-bit word arithmetic, AES, IT-MAC primitives, sharing types.
- Location: `src/block.rs`, `src/delta.rs`, `src/keys.rs`, `src/macs.rs`, `src/aes.rs`, `src/sharing.rs`, `src/matrix.rs`
- Contains: `Block`, `Delta`, `Key`, `Mac`, `AuthBitShare`, `AuthBit`, `InputSharing`, `BlockMatrix`, `FixedKeyAes`
- Depends on: external crates only (`aes`, `bytemuck`, `rand`, etc.)
- Used by: all higher layers

**Tensor Gate Substrate:**
- Purpose: GGM-tree-based unary outer-product garbling (semi-honest + authenticated variants); Construction 1 macro.
- Location: `src/tensor_macro.rs`, `src/tensor_ops.rs`, `src/tensor_gen.rs`, `src/tensor_eval.rs`, `src/tensor_pre.rs`
- Contains: `tensor_garbler`, `tensor_evaluator`, `TensorProductGen`, `TensorProductEval`, `SemiHonestTensorPre`
- Depends on: primitive layer
- Used by: preprocessing layer (`leaky_tensor_pre.rs`), online layer

**Preprocessing Layer:**
- Purpose: Generate authenticated tensor triples (leaky → bucketed → authenticated). Implements Pi_LeakyTensor (C2), Pi_aTensor' (C4), and ideal F_pre.
- Location: `src/auth_tensor_fpre.rs`, `src/leaky_tensor_pre.rs`, `src/auth_tensor_pre.rs`, `src/preprocessing.rs`, `src/bcot.rs`, `src/feq.rs`
- Contains: `TensorFpre`, `LeakyTensorPre`, `LeakyTriple`, `two_to_one_combine`, `combine_leaky_triples`, `TensorFpreGen`, `TensorFpreEval`, `TensorPreprocessing` trait, `IdealPreprocessingBackend`, `UncompressedPreprocessingBackend`, `IdealBCot`, `feq::check`
- Depends on: primitive layer, tensor gate substrate
- Used by: online layer, benches, lib.rs tests

**Online Layer:**
- Purpose: Paper-faithful garble/evaluate/check execution consuming preprocessing artifacts.
- Location: `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`, `src/online.rs`, `src/lib.rs` (root functions)
- Contains: `AuthTensorGen`, `AuthTensorEval`, `check_zero`, `hash_check_zero`, `assemble_gate_semantics_shares`, `assemble_e_input_wire_shares_p1`, `assemble_c_alpha_beta_shares_p2`
- Depends on: preprocessing layer, tensor gate substrate, primitive layer
- Used by: benches, lib.rs tests

**Harness Layer:**
- Purpose: Criterion benchmarks (compute + simulated network), result parsing, paper figures.
- Location: `benches/benchmarks.rs`, `benches/network_simulator.rs`, `tools/parse_results.py`, `tools/comparison_table.py`, `tools/aes_microbench.rs`
- Depends on: online layer (via crate pub API)

## Data Flow

### Preprocessing → Online (Ideal Backend)

1. Caller invokes `IdealPreprocessingBackend::run(n, m, 1, cf)` (`src/preprocessing.rs:139`)
2. `TensorFpre::new(0, n, m, cf)` creates ideal F_pre with fresh deltas (`src/auth_tensor_fpre.rs:25`)
3. `fpre.generate_for_ideal_trusted_dealer(0, 0)` samples masks α, β, correlated bits α·β, and all IT-MAC shares (`src/auth_tensor_fpre.rs:99`)
4. `fpre.gen_auth_bit(l_gamma)` for n*m output masks, consuming the shared RNG before `into_gen_eval()` (`src/preprocessing.rs:168-173`)
5. `fpre.into_gen_eval()` splits into `(TensorFpreGen, TensorFpreEval)`, deriving D_ev label blocks for α/β/corr fields from the auth-bit MAC/key values (`src/auth_tensor_fpre.rs:171`)
6. `AuthTensorGen::new_from_fpre_gen(fpre_gen)` / `AuthTensorEval::new_from_fpre_eval(fpre_eval)` copy fields into the online structs (`src/auth_tensor_gen.rs:85`, `src/auth_tensor_eval.rs:76`)

### Preprocessing → Online (Real Backend)

1. Caller invokes `UncompressedPreprocessingBackend::run(n, m, 1, cf)` → `run_preprocessing(n, m, 1, cf)` (`src/preprocessing.rs:114-125`)
2. Single `IdealBCot::new(0, 1)` created; all leaky triples share the same `delta_a` and `delta_b` (`src/preprocessing.rs:235`)
3. `bucket_size_for(n, 1)` determines B; B leaky triples are generated via `LeakyTensorPre::new(t+2, n, m, &mut bcot).generate()` (`src/preprocessing.rs:238-243`)
4. `combine_leaky_triples(triples, B, n, m, cf, 42)` applies Construction 3 bucketing: random permutation, repeated `two_to_one_combine` calls (`src/auth_tensor_pre.rs`)
5. Post-bucketing: input labels (α, β), D_ev block shares, and γ IT-MAC shares are synthesized and written into `TensorFpreGen` / `TensorFpreEval` (`src/preprocessing.rs:253-337`)
6. Same `AuthTensorGen::new_from_fpre_gen` / `AuthTensorEval::new_from_fpre_eval` constructors are used

### Online Protocol 1 (Narrow Ciphertexts — Garble → Evaluate → Check)

1. `gb.garble_first_half()` → `gen_chunked_half_outer_product` → GGM tree over `x_labels ⊗ beta` (`src/auth_tensor_gen.rs:276`); returns `(chunk_levels, chunk_cts)` sent to evaluator.
2. `ev.evaluate_first_half(chunk_levels, chunk_cts)` reconstructs the first outer-product half (`src/auth_tensor_eval.rs`).
3. `gb.garble_second_half()` / `ev.evaluate_second_half(...)` — symmetric, covers `y_labels ⊗ alpha`.
4. `gb.garble_final()` / `ev.evaluate_final()` — XOR-combine half-results with correlated α·β IT-MAC shares; write combined output to `first_half_out` / `second_half_out`.
5. Caller assembles CheckZero shares via `assemble_e_input_wire_shares_p1(...)` (`src/lib.rs:248`).
6. `check_zero(&e_shares, &ev.delta_b)` verifies consistency under D_ev (`src/online.rs:55`).

### Online Protocol 2 (Wide Ciphertexts — Garble → Evaluate → Check)

1. `gb.garble_first_half_p2()` → `gen_chunked_half_outer_product_wide` writes both D_gb and D_ev accumulators (`src/auth_tensor_gen.rs:344`).
2. `ev.evaluate_first_half_p2(...)` / `gb.garble_second_half_p2()` / `ev.evaluate_second_half_p2(...)` — wide variants.
3. `gb.garble_final_p2()` returns `(gb_d_gb_out, gb_d_ev_out)` — two separate `Vec<Block>` (no `Vec<bool>` sent, by type design). `ev.evaluate_final_p2()` returns `ev_d_ev_out`.
4. Caller assembles P2 CheckZero via `assemble_c_alpha_beta_shares_p2(...)` (alias for P1 helper) (`src/lib.rs:386`).
5. `check_zero(&c_shares, &ev.delta_b)` verifies.

**State Management:**
- All state is in plain structs on the stack/heap; no global mutable state except `FIXED_KEY_AES: Lazy<FixedKeyAes>` (`src/aes.rs:36`).
- `AuthTensorGen.final_computed` flag prevents out-of-order calls to `compute_lambda_gamma` (`src/auth_tensor_gen.rs:56`).

## Key Abstractions

**Block (`src/block.rs:17`):**
- Purpose: The fundamental 128-bit word; every label, key, MAC, delta, and seed is a `Block`.
- Pattern: `#[repr(transparent)] struct Block([u8; 16])`. Implements XOR via `BitXor/BitXorAssign`. All arithmetic is GF(2^128) XOR.

**Delta (`src/delta.rs:7`):**
- Purpose: Global correlation key; newtype over `Block` with an LSB invariant.
- Pattern: `Delta::random` forces LSB=1 (garbler, D_gb); `Delta::random_b` forces LSB=0 (evaluator, D_ev). Required: `lsb(delta_a XOR delta_b) == 1`.

**AuthBitShare (`src/sharing.rs:43`):**
- Purpose: One party's view of a BDOZ-style IT-MAC share. Carries `key` (sender), `mac` (receiver), `value` (committed bit).
- Invariant: `mac == key.auth(value, verifier_delta)`.
- Pattern: `Add` (via `impl Add<AuthBitShare> for AuthBitShare`) performs XOR-field combination used in `two_to_one_combine` and Construction 3.
- Cross-party shares MUST NOT be verified with `share.verify(delta)` directly — use `verify_cross_party` from `src/auth_tensor_pre.rs`.

**TensorFpreGen / TensorFpreEval (`src/preprocessing.rs:15,54`):**
- Purpose: Preprocessing output structs carrying all wire labels and IT-MAC shares needed by the online phase.
- Fields: `alpha_labels` (n), `beta_labels` (m), `*_auth_bit_shares` (n, m, n*m), `*_d_ev_shares` (n, m, n*m), `gamma_d_ev_shares` (n*m). All n*m vectors are column-major with `index = j*n + i`.

**TensorPreprocessing (trait, `src/preprocessing.rs:97`):**
- Purpose: Object-safe interface for preprocessing backends. `run(n, m, count, cf) -> (TensorFpreGen, TensorFpreEval)`.
- Implementations: `IdealPreprocessingBackend` (tests/benches), `UncompressedPreprocessingBackend` (real protocol).

**AuthTensorGen / AuthTensorEval (`src/auth_tensor_gen.rs:18`, `src/auth_tensor_eval.rs:8`):**
- Purpose: Online garbler (P1) and evaluator (P2) structs. Hold all preprocessing artifacts plus intermediate computation buffers (`first_half_out`, `second_half_out`, `*_ev` variants for P2).
- Key methods (garbler): `garble_first_half`, `garble_second_half`, `garble_final` (P1); `garble_first_half_p2`, `garble_second_half_p2`, `garble_final_p2` (P2); `compute_lambda_gamma`.
- Key methods (evaluator): symmetric `evaluate_*` variants.

**TensorMacroCiphertexts (`src/tensor_macro.rs:54`):**
- Purpose: Ciphertext bundle returned by `tensor_garbler` and consumed by `tensor_evaluator`; maps to paper Construction 1 field names.
- Fields: `level_cts: Vec<(Block, Block)>` (n-1 entries), `leaf_cts: Vec<Block>` (m entries).

**LeakyTriple (`src/leaky_tensor_pre.rs:37`):**
- Purpose: Output of one Pi_LeakyTensor run; both parties' x/y/Z IT-MAC shares in a single struct.
- Column-major layout for Z: `index = j*n + i`.

## Entry Points

**Library API (`src/lib.rs`):**
- Constants: `CSP = 128`, `SSP = 40` — single source of truth for security parameters; benchmarks and network simulation derive byte widths from these.
- Public functions: `assemble_gate_semantics_shares`, `assemble_e_input_wire_shares_p1`, `assemble_c_alpha_beta_shares_p2`.
- All public submodules re-exported via `pub mod` declarations.

**Criterion Benchmark Harness (`benches/benchmarks.rs`):**
- Entry point: `criterion_main!` at bottom of file.
- Groups: `online` (100 Mbps networked P1/P2), plus legacy ideal/uncompressed preprocessing groups.
- Parameters: `BENCHMARK_PARAMS` defines `(n, m)` sweep matching paper Table 1 (64×64, 128×128, 256×256 enabled; others commented out).
- Network model: 100 Mbps, zero jitter; transit time computed as `bytes * 8 / NETWORK_BANDWIDTH_BPS` ns and added to measured compute time.
- Helper: `SimpleNetworkSimulator` in `benches/network_simulator.rs`.

**Result Processing (`tools/parse_results.py`):**
- Reads: bench log (KB accounting lines) + Criterion JSON (`target/criterion/...`).
- Writes: `results.csv`, PDF figures per paper layout.

**Examples / Extras:**
- `tools/aes_microbench.rs` — standalone AES microbenchmark (not a Criterion bench, standalone binary).
- `tools/comparison_table.py` — generates paper comparison table.

## Architectural Constraints

- **Threading:** Single-threaded protocol simulation; `FIXED_KEY_AES` is `Lazy<T>` (`Send + Sync`) so bench threads can read it safely. No protocol parallelism — benchmark routines run sequentially.
- **Global state:** `FIXED_KEY_AES: Lazy<FixedKeyAes>` (`src/aes.rs:36`) is the only global. All other state is in local structs.
- **Circular imports:** None detected. Dependency order: primitives ← substrate ← preprocessing ← online ← lib.rs.
- **Delta LSB invariant:** `delta_a.lsb() == 1`, `delta_b.lsb() == 0`, `lsb(delta_a XOR delta_b) == 1`. Enforced at construction; breaking this silently corrupts MAC verification.
- **Shared IdealBCot:** `run_preprocessing` creates ONE `IdealBCot` and all `LeakyTensorPre` instances borrow `&mut bcot`. Creating separate `IdealBCot` instances per triple breaks the MAC invariant in `combine_leaky_triples`.
- **Ordering constraint in IdealPreprocessingBackend:** ALL `gen_auth_bit()` calls must happen BEFORE `fpre.into_gen_eval()` (which consumes fpre by value). Violating this order causes a compile error.
- **Column-major n*m indexing:** Every n*m vector uses `index = j*n + i`. Deviation silently produces wrong protocol output. Enforced by convention, not the type system.

## Anti-Patterns

### Using `AuthBitShare::verify(delta)` on cross-party shares

**What happens:** Calling `share.verify(&delta_a)` on a cross-party `AuthBitShare` (where the MAC was computed under the other party's delta) panics with "MAC mismatch in share" even for correctly formed shares.
**Why it's wrong:** Each party's MAC is authenticated under the *other* party's delta; the local `verify` method does not know which delta applies to which party's contribution.
**Do this instead:** Use `verify_cross_party(&gen_share, &eval_share, &delta_a, &delta_b)` from `src/auth_tensor_pre.rs`, or assemble a combined share and call `check_zero` per the pattern in `src/lib.rs:119-165`.

### Naively XOR-ing MACs across parties in check_zero input

**What happens:** Passing `AuthBitShare { key: gen.key ^ ev.key, mac: gen.mac ^ ev.mac, value: gen.value ^ ev.value }` into `check_zero`.
**Why it's wrong:** The two MACs are authenticated under opposite deltas; their XOR is not a valid IT-MAC under either delta, so `check_zero` will spuriously fail or pass depending on the specific values.
**Do this instead:** Accumulate the reconstructed bit and combined key, then recompute `mac = combined_key.auth(reconstructed_bit, delta_mac)` — exactly as done in `assemble_e_input_wire_shares_p1` (`src/lib.rs:305-315`).

### Creating separate IdealBCot per LeakyTensorPre

**What happens:** Each `IdealBCot::new(seed_a, seed_b)` generates fresh `delta_a` and `delta_b`. If each `LeakyTensorPre` gets its own `IdealBCot`, the triples have different deltas.
**Why it's wrong:** `combine_leaky_triples` and `two_to_one_combine` XOR-combine share vectors; the XOR combination preserves the IT-MAC invariant only when all triples share the same `(delta_a, delta_b)` pair.
**Do this instead:** Create one shared `IdealBCot` before the generation loop, as done in `run_preprocessing` (`src/preprocessing.rs:235`), and pass `&mut bcot` to each `LeakyTensorPre::new`.

## Error Handling

**Strategy:** `assert!` / `assert_eq!` for protocol invariants (panics on violation); no `Result`/`Error` types in the core protocol path. This is appropriate for a research simulation codebase where invariant violations indicate bugs, not recoverable runtime errors.

**Patterns:**
- Dimension assertions guard all vector accesses: `assert_eq!(gb.alpha_auth_bit_shares.len(), n)` etc.
- `feq::check` panics with `"F_eq abort: ..."` to model protocol abort semantics.
- `two_to_one_combine` panics on MAC mismatch in d-share verification (in-process substitute for network abort).

## Cross-Cutting Concerns

**Logging:** None. All verification is done via `assert!` panics or return-value checks.
**Validation:** Protocol correctness properties are checked via `#[test]` unit tests (see TESTING.md). No runtime validation hooks outside tests.
**Authentication:** IT-MAC (BDOZ-style) throughout. `Key::auth(bit, delta)` computes `key XOR (bit * delta)` (`src/keys.rs`). The LSB of `Key` is always 0; the LSB of `Mac` encodes the authenticated bit's pointer value (free squeezing).

---

*Architecture analysis: 2026-04-28*
