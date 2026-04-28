# Codebase Structure

**Analysis Date:** 2026-04-28

## Directory Layout

```
authenticated-tensor-garbling/
├── Cargo.toml              # Crate manifest; [[bench]] harness = false for Criterion
├── Cargo.lock              # Pinned dependency versions
├── README.md               # Project overview
├── src/                    # Library crate root
│   ├── lib.rs              # Crate entry: pub mod declarations, CSP/SSP constants, root API fns
│   │
│   │   ── Primitive / Crypto Layer ──────────────────────────────────────────
│   ├── block.rs            # Block([u8; 16]) — 128-bit word, XOR arithmetic, sigma
│   ├── delta.rs            # Delta(Block) — global correlation key with LSB invariant
│   ├── keys.rs             # Key(Block) — IT-MAC sender key (LSB=0 enforced)
│   ├── macs.rs             # Mac(Block) — IT-MAC receiver MAC
│   ├── sharing.rs          # AuthBitShare, AuthBit, InputSharing, build_share
│   ├── aes.rs              # FixedKeyAes singleton, TCCR/CCR/CR hash constructions
│   ├── matrix.rs           # TypedMatrix<T>, BlockMatrix, KeyMatrix (column-major)
│   │
│   │   ── Tensor Gate Substrate ─────────────────────────────────────────────
│   ├── tensor_macro.rs     # Construction 1: tensor_garbler / tensor_evaluator (GGM macro)
│   ├── tensor_ops.rs       # Low-level GGM seed expansion, unary outer-product routines
│   ├── tensor_pre.rs       # SemiHonestTensorPre — semi-honest (single-delta) preprocessing
│   ├── tensor_gen.rs       # TensorProductGen — semi-honest online garbler
│   ├── tensor_eval.rs      # TensorProductEval — semi-honest online evaluator
│   │
│   │   ── Preprocessing Layer ──────────────────────────────────────────────
│   ├── bcot.rs             # IdealBCot — ideal bCOT (in-process); shared delta across triples
│   ├── feq.rs              # feq::check — ideal F_eq matrix-equality check (abort on mismatch)
│   ├── auth_tensor_fpre.rs # TensorFpre — ideal F_pre (trusted dealer, insecure)
│   ├── leaky_tensor_pre.rs # LeakyTensorPre, LeakyTriple — Pi_LeakyTensor (Construction 2)
│   ├── auth_tensor_pre.rs  # two_to_one_combine, combine_leaky_triples, bucket_size_for
│   ├── preprocessing.rs    # TensorFpreGen/Eval, TensorPreprocessing trait, both backends
│   │
│   │   ── Online Layer ─────────────────────────────────────────────────────
│   ├── auth_tensor_gen.rs  # AuthTensorGen — maliciously-secure online garbler (P1 + P2)
│   ├── auth_tensor_eval.rs # AuthTensorEval — maliciously-secure online evaluator (P1 + P2)
│   └── online.rs           # check_zero, hash_check_zero — CheckZero primitives
│
├── benches/
│   ├── benchmarks.rs       # Criterion bench entry; online/preprocessing groups; network model
│   └── network_simulator.rs # SimpleNetworkSimulator — 100 Mbps async transit simulation
│
├── tools/
│   ├── parse_results.py    # Bench log + Criterion JSON → results.csv + paper PDF figures
│   ├── comparison_table.py # Generates paper comparison table
│   └── aes_microbench.rs   # Standalone AES microbenchmark (not a Criterion bench)
│
├── examples/               # (empty or exploratory; not part of the primary protocol)
├── references/             # Paper TeX sources (CCS2026 draft)
├── figures/                # Generated PDF figures from parse_results.py
├── .planning/              # GSD planning artefacts (roadmap, phase plans, codebase maps)
└── target/                 # Cargo build output (not committed)
```

## Directory Purposes

**`src/`:**
- Purpose: Entire library crate. No `main.rs` — this is a `lib` crate consumed by benches.
- Key files: `lib.rs` (crate root), `preprocessing.rs` (backend trait), `auth_tensor_gen.rs` / `auth_tensor_eval.rs` (online phase).

**`benches/`:**
- Purpose: Criterion benchmark harnesses. `benchmarks.rs` is the only Criterion entry point (declared in `Cargo.toml` as `[[bench]] name = "benchmarks" harness = false`).
- Run with: `cargo bench --bench benchmarks`

**`tools/`:**
- Purpose: Post-processing scripts and a standalone AES microbench. Not part of the Cargo build graph.
- `parse_results.py` requires Python 3 + matplotlib.

**`references/`:**
- Purpose: Paper TeX source files referenced from doc-comments (`5_online.tex`, `6_total.tex`, `appendix_krrw_pre.tex`, `appendix_experiments.tex`). Line numbers in code comments refer to these files.

**`.planning/`:**
- Purpose: GSD phase plans, roadmaps, and codebase maps (ARCHITECTURE.md, STRUCTURE.md, etc.). Not compiled.

## Key File Locations

**Entry Points:**
- `src/lib.rs`: Crate root; `pub mod` declarations, `CSP`/`SSP` constants, `assemble_*` API functions.
- `benches/benchmarks.rs`: Criterion `criterion_main!` entry; all benchmark groups.

**Core Protocol:**
- `src/preprocessing.rs`: `TensorPreprocessing` trait, `IdealPreprocessingBackend`, `UncompressedPreprocessingBackend`, `run_preprocessing`.
- `src/auth_tensor_gen.rs`: `AuthTensorGen` — online garbler, all garble methods.
- `src/auth_tensor_eval.rs`: `AuthTensorEval` — online evaluator, all evaluate methods.
- `src/online.rs`: `check_zero`, `hash_check_zero`.

**Preprocessing Internals:**
- `src/auth_tensor_fpre.rs`: `TensorFpre` ideal trusted dealer; `into_gen_eval()` that derives D_ev block shares.
- `src/leaky_tensor_pre.rs`: `LeakyTensorPre::generate()` — Pi_LeakyTensor; `LeakyTriple` output struct.
- `src/auth_tensor_pre.rs`: `two_to_one_combine`, `combine_leaky_triples`, `bucket_size_for`.

**Primitives:**
- `src/block.rs`: `Block` — everything builds on this.
- `src/sharing.rs`: `AuthBitShare`, `AuthBit`, `build_share`.
- `src/aes.rs`: `FIXED_KEY_AES` global singleton; `tccr`/`ccr`/`cr` hash functions.

**Testing:**
- `src/lib.rs` `#[cfg(test)] mod tests`: Integration-level tests for Protocol 1 and 2 honest-run and tamper scenarios.
- `src/preprocessing.rs` `#[cfg(test)] mod tests`: Backend trait dispatch and D_ev invariant tests.
- `src/online.rs` `#[cfg(test)] mod tests`: `check_zero` unit tests.
- `src/sharing.rs`, `src/block.rs`, `src/auth_tensor_fpre.rs`, `src/auth_tensor_pre.rs`, `src/leaky_tensor_pre.rs`: Each has its own `#[cfg(test)]` block.
- No `tests/` integration test directory exists; all tests are inline.

## Naming Conventions

**Files:**
- `snake_case` throughout: `auth_tensor_gen.rs`, `leaky_tensor_pre.rs`, `tensor_ops.rs`.
- Naming pattern reflects protocol role: `auth_tensor_*` = maliciously-secure online; `tensor_*` = semi-honest or substrate; `leaky_tensor_*` = Construction 2 leaky preprocessing.

**Types / Structs:**
- `PascalCase`: `AuthTensorGen`, `TensorFpreGen`, `LeakyTriple`, `BlockMatrix`, `AuthBitShare`.
- Suffixes: `Gen` = garbler (P1) side; `Eval` = evaluator (P2) side; `Pre` = preprocessing; `Fpre` = ideal F_pre functionality.

**Functions:**
- `snake_case`: `garble_first_half`, `evaluate_final_p2`, `check_zero`, `bucket_size_for`.
- Protocol variant suffix `_p2` marks wide-ciphertext (Protocol 2) variants: `garble_first_half_p2`, `garble_final_p2`, `evaluate_final_p2`.

**Constants:**
- `SCREAMING_SNAKE_CASE`: `CSP`, `SSP`, `MAC_ZERO`, `MAC_ONE`, `FIXED_KEY_AES`, `FIXED_KEY`, `BENCHMARK_PARAMS`, `NETWORK_BANDWIDTH_BPS`.

**Fields:**
- `snake_case` with semantic prefix: `alpha_auth_bit_shares`, `beta_d_ev_shares`, `correlated_d_ev_shares`, `gamma_d_ev_shares`.
- D_ev field naming: `*_d_ev_shares` for Block-valued D_ev label pairs (n, m, n*m); `gamma_d_ev_shares` for `Vec<AuthBitShare>` (the only D_ev field with IT-MAC structure).

**Index conventions:**
- All n*m vectors: column-major `index = j * n + i` (j = column/beta index, i = row/alpha index).
- Tree endianness: index 0 = LSB, index n-1 = MSB; GGM tree starts from MSB (`x[n-1]`).

## Where to Add New Code

**New preprocessing backend (e.g., compressed preprocessing):**
- Implement `TensorPreprocessing` trait defined in `src/preprocessing.rs:97`.
- Add a new zero-field unit struct in `src/preprocessing.rs`.
- Must populate all fields of `TensorFpreGen` / `TensorFpreEval` including all four `*_d_ev_shares` pairs.
- Tests: add `test_trait_dispatch_*` and D_ev invariant tests in `src/preprocessing.rs` `#[cfg(test)]`.

**New online-phase garble/evaluate method:**
- Add `pub fn garble_*(...)` to `src/auth_tensor_gen.rs` and `pub fn evaluate_*(...)` to `src/auth_tensor_eval.rs`.
- If the method is a new protocol variant (e.g., Protocol 3), suffix methods with `_p3`.
- Integration test: add a `run_full_protocol_*` body + `#[test]` functions in `src/lib.rs` `#[cfg(test)]`.

**New online primitive (e.g., `open`):**
- Add to `src/online.rs` following the existing `check_zero` / `hash_check_zero` pattern.
- Unit tests in the same file's `#[cfg(test)] mod tests` block.

**New crypto primitive (helper over Block/Delta/Key):**
- Add to the appropriate primitive file (`src/block.rs`, `src/keys.rs`, `src/aes.rs`) or create a new `src/<name>.rs` and add `pub mod <name>;` to `src/lib.rs`.

**New benchmark group:**
- Add a `fn bench_*(c: &mut Criterion)` function in `benches/benchmarks.rs` and include it in the `criterion_group!` / `criterion_main!` macro at the bottom.
- Follow the `setup_correlated_pair(n, m)` helper pattern for constructing test state.

**New paper-figure metric:**
- Emit a `KB,...` or similar tagged line from `benches/benchmarks.rs`, then update `tools/parse_results.py` to parse and plot it.

## Special Directories

**`target/`:**
- Purpose: Cargo build output (compiled libs, bench binaries, criterion JSON).
- Generated: Yes. Committed: No.

**`.planning/`:**
- Purpose: GSD planning documents, phase plans, codebase maps.
- Generated: By GSD agents. Committed: Yes (in repo).

**`references/`:**
- Purpose: Paper TeX sources. Committed: Yes. Referenced in doc-comments by filename and line number.

**`figures/`:**
- Purpose: PDF figures generated by `tools/parse_results.py`. Committed: Conditionally (not auto-generated by Cargo).

---

*Structure analysis: 2026-04-28*
