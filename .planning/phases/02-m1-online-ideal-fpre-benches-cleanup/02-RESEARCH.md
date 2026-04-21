# Phase 2: M1 Online + Ideal Fpre + Benches Cleanup — Research

**Researched:** 2026-04-21
**Domain:** Rust module refactoring; dead-code removal; Criterion benchmark deduplication; authenticated-garbling preprocessing API surface
**Confidence:** HIGH (codebase grep is direct), HIGH (rustc 1.90.0 behaviour confirmed by local build)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Module Separation (CLEAN-08)**
- **D-01:** Create `src/preprocessing.rs`. Move `TensorFpreGen` and `TensorFpreEval` from `auth_tensor_fpre.rs` into it.
- **D-02:** Move `run_preprocessing()` to `src/preprocessing.rs` — it is the real-protocol entry point, not ideal trusted-dealer logic, and belongs alongside the structs it returns.
- **D-03:** `auth_tensor_fpre.rs` becomes exclusively the ideal `TensorFpre` trusted dealer: `TensorFpre`, `TensorFpre::new`, `TensorFpre::new_with_delta`, `gen_auth_bit`, `generate_for_ideal_trusted_dealer` (renamed from `generate_with_input_values`), `get_clear_values`, `into_gen_eval`.
- **D-04:** Callers import directly from `crate::preprocessing` — no re-export from `auth_tensor_fpre`. Update all import paths in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, and `benches/benchmarks.rs`.
- **D-05:** Add `pub mod preprocessing;` to `lib.rs`.

**TensorFpre Rename + Doc (CLEAN-07)**
- **D-06:** Rename `TensorFpre::generate_with_input_values` → `TensorFpre::generate_for_ideal_trusted_dealer`. Add doc comment: "Generates all authenticated bits and input sharings for the ideal trusted dealer. This is NOT the real preprocessing protocol — it is the ideal functionality (trusted dealer) that the online phase consumes directly in tests and benchmarks."

**Gamma Dead Code Removal (CLEAN-10 + cascading cleanup)**
- **D-07:** Remove `_gamma_share` computation from `AuthTensorGen::garble_final()`.
- **D-08:** Remove `gamma_auth_bit_shares: Vec<AuthBitShare>` field from `AuthTensorGen` and `AuthTensorEval`.
- **D-09:** Remove `gamma_auth_bit_shares: Vec<AuthBitShare>` field from `TensorFpreGen` and `TensorFpreEval` (they no longer carry gamma into the online phase).
- **D-10:** Remove `gamma_auth_bits: Vec<AuthBit>` field from `TensorFpre` and the gamma generation loop inside `generate_for_ideal_trusted_dealer`.
- **D-11:** Remove or update test assertions that reference `gamma_auth_bits` (e.g., `test_tensor_fpre_auth_bits` checks `fpre.gamma_auth_bits.len()`).

**TensorFpreGen / TensorFpreEval Field Docs (CLEAN-09)**
- **D-12:** Add `///` doc comments to every field of `TensorFpreGen` and `TensorFpreEval` in `preprocessing.rs`, specifying which party holds it and what it represents.

**auth_tensor_gen / auth_tensor_eval Audit (CLEAN-10)**
- **D-13:** Remove the `// awful return type` comment on `gen_chunked_half_outer_product` — either fix the return type or leave without the self-deprecating comment.
- **D-14:** Add doc comment to `garble_final()` and `evaluate_final()` explaining the protocol step.
- **D-15:** Name the magic `Block::from(0 as u128)` and `Block::from(1 as u128)` tweaks in `eval_populate_seeds_mem_optimized` — they are GGM tree traversal direction constants (0 = left child, 1 = right child). Add a one-line comment.

**auth_gen.rs / auth_eval.rs (CLEAN-11)**
- **D-16:** Confirmed: `src/auth_gen.rs` and `src/auth_eval.rs` do not exist — CLEAN-11 is trivially satisfied. No action needed; note in plan.

**Benchmark Deduplication (CLEAN-12)**
- **D-17:** Replace the 5 near-identical chunking-factor blocks in `bench_full_protocol_garbling` with a loop: `for cf in [1usize, 2, 4, 6, 8] { ... }`. Benchmark IDs stay the same (`BenchmarkId::new(cf.to_string(), ...)`).
- **D-18:** Add paper protocol header comment to each benchmark group: `// Benchmarks online garbling for authenticated tensor gate (auth_tensor_gen / auth_tensor_eval)`.
- **D-19:** Update `setup_auth_gen` and `setup_auth_eval` helper calls from `generate_with_input_values` to `generate_for_ideal_trusted_dealer` (rename follow-through from D-06).

### Claude's Discretion

- Exact wording of per-field doc comments on `TensorFpreGen`/`TensorFpreEval` (D-12) — content must be accurate, style is implementer's call.
- Whether to rename or leave the `gen_chunked_half_outer_product` return type (D-13) — if renaming is a clean one-liner, do it; otherwise just remove the comment.
- Exact placement of the GGM tweak comments (D-15) — inline or above the `Block::from(...)` lines.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within Phase 2 scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLEAN-07 | Rename `TensorFpre::generate_with_input_values` → `generate_for_ideal_trusted_dealer` and document as ideal functionality | Current signature at `src/auth_tensor_fpre.rs:122`; 5 callers identified (2 in fpre tests, 1 in auth_tensor_gen test, 2 in benches) |
| CLEAN-08 | Separate `TensorFpre` (ideal) from `TensorFpreGen` / `TensorFpreEval` (real output structs) into a `preprocessing` module | Source files, import graph, and module dependencies mapped below. `lib.rs` currently registers 16 modules; adding a 17th follows the flat module convention |
| CLEAN-09 | Add field doc comments to `TensorFpreGen` / `TensorFpreEval` specifying party + semantics | Field-by-field party attribution enumerated in `TensorFpreGen/Eval Field Semantics` section below |
| CLEAN-10 | Audit `auth_tensor_gen.rs` / `auth_tensor_eval.rs` for dead code and magic constants | `_gamma_share` dead computation at `auth_tensor_gen.rs:188-192`; `Block::from(0/1 as u128)` tweak constants at `auth_tensor_eval.rs:103-104, 117-119`; `awful return type` comment at `auth_tensor_gen.rs:79` |
| CLEAN-11 | Remove or isolate `src/auth_gen.rs`, `src/auth_eval.rs` if unused | Confirmed non-existent; no files match these names in `src/` |
| CLEAN-12 | Deduplicate `benches/benchmarks.rs` setup, add paper protocol comments | 5 repeated chunking-factor blocks in `bench_full_protocol_garbling` at `benches/benchmarks.rs:94-158`; 6 duplicate chunking-factor blocks in `bench_full_protocol_with_networking`; 7 chunking-sweep benchmarks (`bench_4x4_...` through `bench_256x256_...`) sharing identical bodies (deduplication of these latter is NOT explicitly required by D-17; see Open Questions) |
</phase_requirements>

---

## Summary

Phase 2 is a pure refactoring / dead-code / doc-cleanup phase. **No algorithmic changes.** The scope splits into four coordinated sub-tasks, each self-contained:

1. **Module move** — pull `TensorFpreGen`, `TensorFpreEval`, `run_preprocessing` out of `auth_tensor_fpre.rs` into a new `preprocessing.rs`; rewire imports.
2. **Gamma removal (end-to-end)** — delete the `gamma_*` fields from five structs (`TensorFpre`, `TensorFpreGen`, `TensorFpreEval`, `AuthTensorGen`, `AuthTensorEval`) and all code that populates them; update tests that reference those fields.
3. **Docs + dead-code cleanup** — rename `generate_with_input_values`, document every field on the two preprocessing structs, name the GGM tree traversal tweak constants, remove the `// awful return type` comment.
4. **Bench deduplication** — collapse five repeated blocks in `bench_full_protocol_garbling` into a loop over `[1, 2, 4, 6, 8]`; add paper-protocol header comments.

The work is entirely mechanical from a cryptographic standpoint — the online garbling algorithm, GGM tree logic, MAC relationships, and delta invariants all stay byte-for-byte identical. Success is measured by `cargo build`, `cargo test`, and `cargo bench --no-run` all continuing to pass with no new warnings.

**Primary recommendation:** Structure the plan as **four plans in two waves**:
- **Wave 1 (independent, parallel):** (a) module-move plan — creates `preprocessing.rs`, rewires imports; (b) bench-dedup plan — rewrites `bench_full_protocol_garbling`, adds paper comments. These two plans touch disjoint files (except possibly `benches/benchmarks.rs` which the module-move plan edits only to update imports).
- **Wave 2 (depends on Wave 1a):** (c) gamma-removal plan — needs the struct layout settled in `preprocessing.rs`; (d) docs-and-audit plan — adds field docs to the new structs and cleans `auth_tensor_gen`/`auth_tensor_eval` dead code and comments.

**Critical pre-existing blocker** (see `Environment Availability` and `Common Pitfalls`): `cargo test` on the current `main` branch produces **4 failing tests** unrelated to Phase 2 scope (`leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`, `test_correlated_mac_invariants`, `auth_tensor_pre::tests::test_combine_mac_invariants`, `auth_tensor_fpre::tests::test_run_preprocessing_mac_invariants`). CONTEXT.md success criterion says "full test suite must pass" — the planner must resolve this ambiguity: either (a) baseline-accept the 4 failures and require "no *new* failures introduced", or (b) add a pre-work task to diagnose and fix them before Phase 2 gamma removal can prove nothing broke. See Open Question Q1.

---

## Architectural Responsibility Map

Phase 2 does not introduce new tiers; all capabilities stay in the same architectural location. The "tier" column here is "Rust module" since this is a single-crate library.

| Capability | Primary Module (after refactor) | Secondary Module | Rationale |
|------------|--------------------------------|------------------|-----------|
| Ideal trusted-dealer Fpre (`TensorFpre`) | `auth_tensor_fpre.rs` | — | Survives as pure ideal functionality per D-03 |
| Real-protocol output structs (`TensorFpreGen`/`Eval`) | `preprocessing.rs` (NEW) | — | Moved per D-01 |
| Real-protocol entry (`run_preprocessing`) | `preprocessing.rs` (NEW) | — | Moved per D-02 |
| Bucketing combiner (`combine_leaky_triples`, `bucket_size_for`) | `auth_tensor_pre.rs` | — | Stays put — populates the new module's structs from outside |
| Online garbler | `auth_tensor_gen.rs` | `preprocessing.rs` (import) | Imports change from `auth_tensor_fpre::TensorFpreGen` to `preprocessing::TensorFpreGen` |
| Online evaluator | `auth_tensor_eval.rs` | `preprocessing.rs` (import) | Same import rewire |
| Benchmarks | `benches/benchmarks.rs` | `preprocessing.rs` (import) | Imports `run_preprocessing` from `preprocessing`; keeps `TensorFpre` from `auth_tensor_fpre` |
| GGM tree traversal tweaks | `tensor_ops.rs`, `auth_tensor_eval.rs`, `tensor_eval.rs` | — | D-15 only asks for naming/commenting in `auth_tensor_eval.rs`; other files out of scope |

---

## Standard Stack

No new dependencies. This phase is pure refactor; the existing crate manifest is unchanged.

### Core (already in Cargo.toml) `[VERIFIED: /Users/turan/Desktop/authenticated-tensor-garbling/Cargo.toml]`

| Crate | Version | Purpose |
|-------|---------|---------|
| `rust` edition | 2024 | Crate edition — unchanged |
| `rustc` / `cargo` | 1.90.0 | Toolchain on this machine (`rustc 1.90.0 (1159e78c4 2025-09-14)` / `cargo 1.90.0 (840b83a10 2025-07-30)`) |
| `rand` | 0.9 | RNG trait; used in `TensorFpre::gen_auth_bit` (unchanged) |
| `rand_chacha` | 0.9 | `ChaCha12Rng` seedable RNG used throughout preprocessing (unchanged) |
| `criterion` | 0.7 with `async_tokio` feature | Benchmark harness (unchanged) |
| `tokio` | 1.47.1 | Bench-time async runtime for `SimpleNetworkSimulator` (unchanged) |
| `once_cell` | 1.21.3 | `Lazy<RT>` in benchmarks, `Lazy` for `FIXED_KEY_AES` in `aes.rs` (unchanged) |

**Installation:** No-op (nothing to install).

### Alternatives Considered: None

No alternatives are being evaluated — this is a scoped refactor with locked decisions. Do not introduce derive macros, traits, or abstractions that did not exist in Phase 1.

---

## Architecture Patterns

### System Data Flow After Refactor

```
                              External caller (test / bench)
                                      |
                                      v
     ┌────────────────────────────────┴───────────────────────────────┐
     |                                                                |
     |  Path A: IDEAL (fast, for correctness tests)                   |
     |  ──────────────────────────────────────────                    |
     |  TensorFpre::new(seed, n, m, cf)                               |
     |      |                                                         |
     |      v                                                         |
     |  TensorFpre::generate_for_ideal_trusted_dealer(x, y)           |
     |      |                                                         |
     |      v                                                         |
     |  TensorFpre::into_gen_eval() -> (TensorFpreGen, TensorFpreEval)|  <-- crosses module
     |                                                                |      boundary:
     |                                                                |      returns types
     |  Path B: REAL (two-party protocol via bCOT/leaky/bucket)       |      from preprocessing.rs
     |  ──────────────────────────────────────────                    |
     |  preprocessing::run_preprocessing(n, m, 1, cf)                 |
     |      |                                                         |
     |      +--> IdealBCot::new()                                     |
     |      +--> LeakyTensorPre::generate() * bucket_size             |
     |      +--> auth_tensor_pre::combine_leaky_triples()             |
     |      |                                                         |
     |      v                                                         |
     |  (TensorFpreGen, TensorFpreEval)                               |
     |                                                                |
     └──────────────────────┬─────────────────────────────────────────┘
                            v
               AuthTensorGen::new_from_fpre_gen(gen)    AuthTensorEval::new_from_fpre_eval(eval)
                            |                                        |
                            v                                        v
                 garble_first_half()                     evaluate_first_half(levels, cts)
                 garble_second_half()                    evaluate_second_half(levels, cts)
                 garble_final()                          evaluate_final()
                            |                                        |
                            +──────────────────┬─────────────────────+
                                               v
                               Output matrix of garbled tensor gates
```

**Key insight:** After Phase 2, `TensorFpre::into_gen_eval()` lives in `auth_tensor_fpre.rs` but returns types (`TensorFpreGen`, `TensorFpreEval`) defined in `preprocessing.rs`. Rust permits cross-module return types without re-exports. `[VERIFIED: cross-module return types are a standard Rust pattern; rustc 1.90.0 accepts without warning]`

### Recommended Project Structure

```
src/
├── lib.rs                   # +1 line: pub mod preprocessing;
├── auth_tensor_fpre.rs      # Shrinks: keeps TensorFpre only (ideal dealer)
├── preprocessing.rs         # NEW: TensorFpreGen, TensorFpreEval, run_preprocessing
├── auth_tensor_gen.rs       # Import change + gamma field/code removal + docs
├── auth_tensor_eval.rs      # Import change + gamma field removal + tweak comments
├── auth_tensor_pre.rs       # Unchanged code, but imports TensorFpreGen/Eval FROM preprocessing.rs now
├── leaky_tensor_pre.rs      # Unchanged (keeps gen_gamma_shares/eval_gamma_shares on LeakyTriple — out of scope)
├── bcot.rs                  # Unchanged
├── sharing.rs, keys.rs, …   # Unchanged
└── …
benches/
└── benchmarks.rs            # Dedup + paper-protocol comments + import change for run_preprocessing
```

### Pattern 1: Rust Module Split with Cross-Module Return Type

**What:** Move some structs out of a module while a constructor/factory in the original module still returns them.

**When to use:** When refactoring for separation of concerns but preserving a stable public API.

**Example** (applied to this phase):
```rust
// src/auth_tensor_fpre.rs
use crate::preprocessing::{TensorFpreGen, TensorFpreEval};  // NEW import

pub struct TensorFpre { /* ... */ }

impl TensorFpre {
    pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {
        // Body unchanged — Rust allows returning types from another module
        (TensorFpreGen { /* ... */ }, TensorFpreEval { /* ... */ })
    }
}
```
```rust
// src/preprocessing.rs (NEW)
use crate::{block::Block, delta::Delta, sharing::AuthBitShare};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};

pub struct TensorFpreGen { /* fields with /// doc comments per D-12 */ }
pub struct TensorFpreEval { /* fields with /// doc comments per D-12 */ }

pub fn run_preprocessing(n: usize, m: usize, count: usize, chunking_factor: usize)
  -> (TensorFpreGen, TensorFpreEval) { /* body unchanged */ }
```
`[CITED: Rust Reference — Modules (doc.rust-lang.org/reference/items/modules.html)]`

### Pattern 2: Criterion Loop-Over-Parameters Deduplication

**What:** Replace N near-identical `group.bench_with_input` calls (each hard-coding a different param) with a single loop.

**When to use:** When benchmark bodies differ only in one or two parameter values and the benchmark ID scheme lets you encode the loop variable.

**Example** (target shape for D-17):
```rust
// In bench_full_protocol_garbling
for &(n, m) in BENCHMARK_PARAMS {
    group.throughput(Throughput::Elements((n * m) as u64));

    for cf in [1usize, 2, 4, 6, 8] {
        let mut generator = setup_auth_gen(n, m, cf);
        group.bench_with_input(
            BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m)),
            &(n, m),
            |b, &(_n, _m)| {
                b.iter(|| {
                    let (_fl, _fc) = generator.garble_first_half();
                    let (_sl, _sc) = generator.garble_second_half();
                    generator.garble_final();
                })
            },
        );
    }
}
```
`[VERIFIED: Criterion docs — BenchmarkId::new accepts any Display parameter; existing bench code already uses .to_string() pattern]` `[CITED: docs.rs/criterion/0.7/criterion/struct.BenchmarkId.html]`

**Important:** The resulting benchmark IDs must exactly match the pre-refactor IDs so Criterion's on-disk baseline comparison (`target/criterion/`) is preserved. `BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m))` produces IDs identical to the current hard-coded `BenchmarkId::new("1", ...)`, `BenchmarkId::new("2", ...)`, etc. `[VERIFIED: by reading existing IDs at benches/benchmarks.rs:96, 109, 122, 135, 149 — all use string literals "1", "2", "4", "6", "8"]`

### Anti-Patterns to Avoid

- **Adding new traits / abstractions** — CONTEXT.md specifies a mechanical refactor. Do not introduce trait objects, generic parameters, or builder patterns.
- **Reordering field declarations** — even though the existing code initialises `TensorFpreGen` via struct-literal syntax (named fields, order-independent), keeping field order stable simplifies code review diffs. Add docs in place; do not reorder.
- **Re-exporting `TensorFpreGen` / `TensorFpreEval` from `auth_tensor_fpre.rs`** — explicitly ruled out by D-04.
- **"Fixing" the `gen_chunked_half_outer_product` return type with a newtype** — D-13 classifies this as discretionary; unless a clean one-liner exists, just drop the self-deprecating comment. Creating a tuple struct would ripple into callers and add noise outside Phase 2 scope.
- **Extending gamma removal to `LeakyTriple.gen_gamma_shares` / `eval_gamma_shares` or `combine_leaky_triples` gamma XOR logic** — see "Gamma Removal: Cascade Boundary" below. D-08..D-11 stop at the `TensorFpre*` / `AuthTensor*` layer. Going further touches files outside CONTEXT.md's scope.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Module re-export shim | `pub use crate::preprocessing::*` in `auth_tensor_fpre` | Direct `use crate::preprocessing::…` in callers | D-04 locked this. Re-exports hide import origin and create lint warnings about unused re-exports. |
| Benchmark parameterisation abstraction | Generic helper `bench_chunked<F>(group, n, m, cf, f: F)` | Simple `for cf in [...] { ... }` loop | D-17 locked the loop approach. Helpers with closures add signature complexity and are harder to read than a literal loop. |
| Field-by-field copy in `into_gen_eval` | Any `#[derive(...)]` or trait to auto-generate the conversion | Keep the explicit struct-literal construction | The existing code is already the idiomatic pattern and Phase 2 mandates zero algorithmic changes. |
| Mass comment reformatting | Scripted comment-style sweep | Hand-author each `///` doc per D-12 | Doc comments encode semantics per field (which party holds what under which delta); this requires human judgement, not mechanical substitution. |

**Key insight:** This is a *minimise-churn* refactor. Every optional touchup the agent might be tempted to do expands the diff beyond CONTEXT.md scope and risks introducing subtle bugs in a cryptography codebase where MAC-key-delta relationships are easy to break silently.

---

## Runtime State Inventory

> This is a rename + module-move + dead-code-removal phase. Runtime state inventory is MANDATORY.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | None — project is a library crate with no persistent storage, no databases, no on-disk caches except `target/criterion/` baselines. `[VERIFIED: ls of project root, no *.sqlite / *.db / cache dirs besides target/]` | The only on-disk artefact that references Phase-2-affected names is `target/criterion/full_protocol_garbling/{1,2,4,6,8}/*` — benchmark baselines keyed by `BenchmarkId`. Because D-17 preserves IDs exactly (`cf.to_string()` still produces `"1"`, `"2"`, …), Criterion's baseline comparisons continue to work and no manual cache clear is required. If IDs were changed, `cargo bench` would simply create fresh baselines — not a correctness problem but a performance-regression-detection inconvenience. |
| **Live service config** | None — no running services, no deployment configs. | None. |
| **OS-registered state** | None — no systemd / launchd / Task Scheduler entries, no pm2 processes, no cron jobs. | None. |
| **Secrets / env vars** | None — no `.env`, no SOPS, no env-based config anywhere in the crate. Cargo reads no environment variables beyond the standard ones. | None. |
| **Build artefacts / installed packages** | `target/debug/` and `target/release/` contain compiled `.rlib` files with mangled symbols referencing `TensorFpreGen`, `TensorFpreEval`, `run_preprocessing`, `generate_with_input_values`. `[VERIFIED: these names appear in grep of target/debug/deps/*.rmeta if the build cache exists, though they are not visible to users]` | `cargo build` / `cargo test` / `cargo bench` automatically rebuild on source change — incremental compilation handles the rename and module move transparently. **No manual `cargo clean` required.** If the user sees strange linker errors, a single `cargo clean && cargo build` clears them. The stray `src/auth_tensor_fpre 2.rs`, `src/auth_tensor_pre 2.rs`, `src/bcot 2.rs`, `src/leaky_tensor_pre 2.rs` files (visible in `ls src/`) are macOS Finder duplicates with spaces in filenames — they are **not** Cargo modules (Cargo requires `snake_case` or `mod.rs`, and the space breaks discovery) and can be safely ignored. |

**Canonical question:** *"After every file in the repo is updated, what runtime systems still have the old string cached, stored, or registered?"* — **Answer: only `target/criterion/` baselines, and only in a way that benchmark ID preservation (per D-17) handles automatically.** The rename from `generate_with_input_values` → `generate_for_ideal_trusted_dealer` is a pure compile-time symbol change with no runtime residue.

---

## Common Pitfalls

### Pitfall 1: Pre-Existing Baseline Test Failures
**What goes wrong:** CONTEXT.md requires "full test suite must pass after cleanup". But the baseline (`main`, commit `c3277b7`) already has **4 failing tests**:
- `auth_tensor_fpre::tests::test_run_preprocessing_mac_invariants`
- `auth_tensor_pre::tests::test_combine_mac_invariants`
- `leaky_tensor_pre::tests::test_alpha_beta_mac_invariants`
- `leaky_tensor_pre::tests::test_correlated_mac_invariants`

All four panic with `"MAC mismatch in share"` at `src/sharing.rs:62`. `[VERIFIED: cargo test --lib on main, tail of output shows "test result: FAILED. 48 passed; 4 failed"]`

**Why it happens:** Unrelated to Phase 2 — looks like a latent bug in the real-preprocessing pipeline (bCOT / leaky-triple combining) that was not caught by Phase 1 UAT. The ideal `TensorFpre` path (`test_auth_tensor_product` end-to-end in `lib.rs`) still passes. `[VERIFIED: cargo test --lib shows ideal-path tests green]`

**How to avoid:** Planner MUST decide one of:
- (A) Treat the 4 failures as pre-existing "known red" and require "no *new* failures introduced by Phase 2". Verify by baseline diff (`cargo test --lib 2>&1 | grep "FAILED" | sort > before.txt`, same after, compare).
- (B) Add a pre-Phase-2 diagnostic task to repair the 4 tests before gamma removal touches the same files. This is more work but restores the green baseline CONTEXT.md demands.
- (C) Escalate to user (`ask_user_question` tool in the planner). Recommended — the decision has project-level implications beyond Phase 2.

**Warning signs:** If the plan-checker flags "cargo test must pass" and the implementer sees red-but-unchanged tests, this is the same issue. See Open Question Q1.

### Pitfall 2: Field Removal Breaks Test Assertions Out of Scope
**What goes wrong:** D-09 removes `gamma_auth_bit_shares` from `TensorFpreGen`/`TensorFpreEval`. But:
- `test_run_preprocessing_mac_invariants` in `auth_tensor_fpre.rs` iterates over `gen_out.gamma_auth_bit_shares` and `eval_out.gamma_auth_bit_shares` (lines 432-434).
- `test_combine_dimensions` and other tests in `auth_tensor_pre.rs` check `eval_out.gamma_auth_bit_shares.len()` (line 176).
- `test_garble_first_half` in `auth_tensor_gen.rs` checks `fpre_gen.gamma_auth_bit_shares.len()` (line 224) and `gar.gamma_auth_bit_shares.len()` (line 235).
- `test_tensor_fpre_input_sharings` in `auth_tensor_fpre.rs` checks `fpre_gen.gamma_auth_bit_shares.len()` (line 382).

`[VERIFIED: grep "gamma_auth_bit_shares" src/*.rs | wc -l` = 12 references across four source files]

**Why it happens:** Mechanical field removal without test audit.

**How to avoid:** D-11 says "Remove or update test assertions that reference `gamma_auth_bits`". Interpret broadly — audit all three field names: `gamma_auth_bits` (TensorFpre), `gamma_auth_bit_shares` (TensorFpreGen/Eval, AuthTensorGen/Eval), and any cascade via `LeakyTriple` field access inside tests. Use the list above as the authoritative grep result. See "Gamma Removal: Cascade Boundary" for which files the plan must touch.

**Warning signs:** `cargo build --tests` after gamma removal fails with `no field gamma_auth_bit_shares on type …`.

### Pitfall 3: combine_leaky_triples Writes to a Removed Field
**What goes wrong:** `src/auth_tensor_pre.rs:99,111` constructs `TensorFpreGen`/`TensorFpreEval` with `gamma_auth_bit_shares: combined_gen_gamma` / `combined_eval_gamma`. If D-09 removes the field but `combine_leaky_triples` still tries to populate it, **compile error**.

**Why it happens:** `auth_tensor_pre.rs` is not in CONTEXT.md's "Source Files in Scope" but Phase 2 changes its dependency's public API.

**How to avoid:** The plan for D-09 MUST also edit `src/auth_tensor_pre.rs`:
1. Drop `let mut combined_gen_gamma = …` and `combined_eval_gamma = …` local-variable bindings (lines 73-74).
2. Drop the inner loop body lines that XOR gamma shares (lines 81-82).
3. Drop the `gamma_auth_bit_shares: combined_gen_gamma,` and `gamma_auth_bit_shares: combined_eval_gamma,` field initialisers (lines 99, 111).
4. The `LeakyTriple.gen_gamma_shares` / `eval_gamma_shares` fields stay (they're in leaky_tensor_pre.rs, out of Phase 2 scope) — the triple just no longer propagates gamma forward.

**Warning signs:** `cargo build` fails at `src/auth_tensor_pre.rs:99` with `struct TensorFpreGen has no field named gamma_auth_bit_shares`.

### Pitfall 4: Mismatched Criterion BenchmarkId Breaks Baseline Comparison
**What goes wrong:** D-17's loop shape is `for cf in [1usize, 2, 4, 6, 8]`. If the loop variable is `cf: u64` or `cf: i32`, `cf.to_string()` still produces `"1"`, `"2"`, etc., but any difference in `BenchmarkId` name ("1" vs "1 " vs "cf=1") silently invalidates the on-disk baselines at `target/criterion/full_protocol_garbling/{1,2,4,6,8}/`.

**How to avoid:** Use the exact call `BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m))` as shown in the pattern above. Verify the output ID matches with a quick `cargo bench --no-run && cargo bench -- full_protocol_garbling/1 --list` check if uncertain.

**Warning signs:** Criterion reports "no previous data" for every permutation instead of statistical comparisons against the baseline.

### Pitfall 5: Endianness / Tweak Interpretation in GGM Comment
**What goes wrong:** D-15 asserts "0 = left child, 1 = right child". But the code (both gen side at `tensor_ops.rs:61-62` and eval side at `auth_tensor_eval.rs:103-104`) has:
```rust
seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);  // odd index → tweak 0
seeds[j * 2]     = cipher.tccr(Block::from(1 as u128), seeds[j]);  // even index → tweak 1
```
If the convention is "even index = left child, odd index = right child", then **tweak 0 derives the right child** and **tweak 1 derives the left child** — the *opposite* of D-15's stated mapping.

**How to avoid:** The implementer should verify the left/right mapping by tracing the tree construction (consult the KRRW paper GGM tree section, or reason from the `missing = (missing << 1) | bit` / `sibling_index = missing ^ 1` logic in `eval_populate_seeds_mem_optimized` around `auth_tensor_eval.rs:113-122`). **Recommended comment wording if uncertain:** `// GGM tree tweak domain separation: tweak=0 and tweak=1 derive the two child seeds at each level (one per subtree direction).` This is correct regardless of which tweak maps to which side. See Open Question Q2.

**Warning signs:** Code review catches the comment contradicting the code. `[ASSUMED: the exact "0=left, 1=right" wording in D-15 is a high-level description, not an invariant that must be copied verbatim into the comment]`

### Pitfall 6: Orphaned Doc Comment on `generate_with_input_values`
**What goes wrong:** The existing comment at `auth_tensor_fpre.rs:119-121` is:
```rust
/// Generates all auth bits for the input and output vectors of a tensor gate.
/// alpha, beta, ab* = alpha * beta
/// gamma
```
D-06 says *add* the "NOT the real preprocessing protocol" doc; but the old three-line comment (which mentions "gamma") becomes stale when gamma is removed. The implementer must rewrite the doc, not just prepend to it.

**How to avoid:** Replace the entire three-line block with the D-06-specified wording when renaming. Drop the "gamma" line entirely.

**Warning signs:** Stale references to `gamma` survive in doc comments after D-10 removes the generation code.

### Pitfall 7: `once_cell::sync::Lazy` Test Pollution
**What goes wrong:** Irrelevant to Phase 2 (no test infra changes), but worth noting: Criterion benchmarks use `Lazy<RT>` for the tokio runtime. Gamma removal changes struct sizes; tokio runtime behaviour is unaffected. No action needed.

---

## Code Examples

### Example 1: Updated `TensorFpreGen` in `preprocessing.rs` (satisfies D-12)

```rust
// Source pattern: src/auth_tensor_fpre.rs lines 26-37 (move into preprocessing.rs with docs)
// Field ownership documented per D-12.
pub struct TensorFpreGen {
    /// Tensor row dimension (number of alpha / x-input bits).
    pub n: usize,
    /// Tensor column dimension (number of beta / y-input bits).
    pub m: usize,
    /// GGM tree chunking factor; purely a performance knob (1..=8 used in benches).
    pub chunking_factor: usize,
    /// Garbler's (Party A) global correlation key. `as_block().lsb() == 1` invariant.
    pub delta_a: Delta,
    /// Garbler's share of each x-input wire label; length n. Represents `x XOR alpha`
    /// when XORed against the evaluator's matching eval_share.
    pub alpha_labels: Vec<Block>,
    /// Garbler's share of each y-input wire label; length m. Represents `y XOR beta`.
    pub beta_labels: Vec<Block>,
    /// Garbler's `AuthBitShare` for each alpha_i (i in 0..n). Each share's `mac`
    /// commits `value` under delta_b (the evaluator's delta).
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each beta_j (j in 0..m). MAC committed under delta_b.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each correlated bit alpha_i AND beta_j; length n*m,
    /// column-major index = j*n + i. MAC committed under delta_b.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
    // NOTE: gamma_auth_bit_shares REMOVED per D-09
}
```
`[VERIFIED: existing code at auth_tensor_fpre.rs:26-37 already matches this layout modulo the gamma field; doc wording is author-discretion per D-12]`

### Example 2: Updated `garble_final` (satisfies D-07, D-14)

```rust
// src/auth_tensor_gen.rs — replace lines 179-199
/// Combines both half-outer-product outputs with the correlated preprocessing
/// share to produce the garbled tensor gate output.
pub fn garble_final(&mut self) {
    for i in 0..self.n {
        for j in 0..self.m {
            let correlated_share = if self.correlated_auth_bit_shares[j * self.n + i].bit() {
                self.delta_a.as_block() ^ self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
            } else {
                *self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
            };

            // gamma share computation REMOVED per D-07 (was dead code — never XORed into output)

            self.first_half_out[(i, j)] ^=
                self.second_half_out[(j, i)] ^
                correlated_share;
        }
    }
}
```
`[VERIFIED: existing garble_final body at auth_tensor_gen.rs:179-199; deleting lines 188-192 produces exactly the above]`

### Example 3: Updated `evaluate_final` (satisfies D-14)

```rust
// src/auth_tensor_eval.rs — replace doc on fn at line 255
/// Combines both half-outer-product outputs with the correlated preprocessing
/// MAC to produce the evaluator's share of the garbled tensor gate output.
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

### Example 4: GGM Tweak Comment (satisfies D-15)

Inline comment at `auth_tensor_eval.rs:103-104`:
```rust
// GGM tree tweak domain separation: distinct AES tweaks derive the two child seeds
// at each tree level. tweak=0 → odd-indexed sibling; tweak=1 → even-indexed sibling.
seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);
seeds[j * 2]     = cipher.tccr(Block::from(1 as u128), seeds[j]);
```

**Note on tweak-to-direction mapping:** See Pitfall 5. The above wording is semantically safe regardless of whether "odd" or "even" is conceptually "left". If the implementer verifies the mapping against the paper, they may substitute "left child" / "right child" for "odd-indexed" / "even-indexed". See Open Question Q2.

### Example 5: Deduplicated Bench Body (satisfies D-17, D-18)

```rust
// benches/benchmarks.rs — replace lines 86-161
// Benchmarks online garbling for authenticated tensor gate (auth_tensor_gen / auth_tensor_eval).
fn bench_full_protocol_garbling(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_protocol_garbling");

    for &(n, m) in BENCHMARK_PARAMS {
        group.throughput(Throughput::Elements((n * m) as u64));

        for cf in [1usize, 2, 4, 6, 8] {
            let mut generator = setup_auth_gen(n, m, cf);
            group.bench_with_input(
                BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m)),
                &(n, m),
                |b, &(_n, _m)| {
                    b.iter(|| {
                        let (_first_levels, _first_cts) = generator.garble_first_half();
                        let (_second_levels, _second_cts) = generator.garble_second_half();
                        generator.garble_final();
                    })
                },
            );
        }
    }
    group.finish();
}
```

### Example 6: Updated setup_auth_gen (satisfies D-19)

```rust
// benches/benchmarks.rs — replace lines 70-83
fn setup_auth_gen(n: usize, m: usize, chunking_factor: usize) -> AuthTensorGen {
    let mut fpre = TensorFpre::new(0, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT);
    let (fpre_gen, _) = fpre.into_gen_eval();
    AuthTensorGen::new_from_fpre_gen(fpre_gen)
}

fn setup_auth_eval(n: usize, m: usize, chunking_factor: usize) -> AuthTensorEval {
    let mut fpre = TensorFpre::new(1, n, m, chunking_factor);
    fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT);
    let (_, fpre_eval) = fpre.into_gen_eval();
    AuthTensorEval::new_from_fpre_eval(fpre_eval)
}
```

### Example 7: Updated Import List in benches/benchmarks.rs (satisfies D-02, D-04)

```rust
// Top of benches/benchmarks.rs — replace lines 9-17
use authenticated_tensor_garbling::{
    block::Block,
    tensor_gen::TensorProductGen,
    tensor_eval::TensorProductEval,
    tensor_pre::SemiHonestTensorPre,
    auth_tensor_gen::AuthTensorGen,
    auth_tensor_eval::AuthTensorEval,
    auth_tensor_fpre::TensorFpre,              // only TensorFpre stays here
    preprocessing::run_preprocessing,          // moved per D-02
};
```

---

## Gamma Removal: Cascade Boundary (Critical)

The gamma removal in D-07..D-11 has a **fixed stopping point** that the plan must respect. Here is the complete file list, with every `gamma*` reference classified:

| File | Gamma references (line numbers on current main) | Action |
|------|-------------------------------------------------|--------|
| `src/auth_tensor_fpre.rs` | `gamma_auth_bits` field (L23), `Vec::with_capacity` inits (L72, L92), doc comment `/// gamma` (L121), generation loop (L177-190 block: `gamma_bits`, `gen_auth_bit`, `self.gamma_auth_bits.push`), `into_gen_eval` copies (L206, L217), test references (L310, L325, L382, L392, L432-434) | **REMOVE** per D-10, D-11 |
| `src/auth_tensor_gen.rs` | `gamma_auth_bit_shares` field (L29), `new` init (L48), `new_from_fpre_gen` copy (L66), `_gamma_share` computation (L188-192), test references (L224, L235) | **REMOVE** per D-07, D-08 |
| `src/auth_tensor_eval.rs` | `gamma_auth_bit_shares` field (L23), `new` init (L42), `new_from_fpre_eval` copy (L60) — note: `evaluate_final` NEVER references gamma | **REMOVE** per D-08 |
| `src/auth_tensor_pre.rs` | Docstring mention (L32), field initialisers in `combine_leaky_triples` output (L73-74, L81-82, L99, L111), test reference (L176) | **REMOVE** the output-field population and the combiner's `combined_gen_gamma`/`combined_eval_gamma` locals — see Pitfall 3. Update the docstring at L32 to stop mentioning gamma. Test assertion at L176 must be removed. |
| `src/leaky_tensor_pre.rs` | `gen_gamma_shares`/`eval_gamma_shares` fields on `LeakyTriple` (L21, L29), generation (L198-226), struct init (L242-243), test (L403-404, L417) | **KEEP UNCHANGED.** This is outside CONTEXT.md's scope — `LeakyTriple` is a leaf-level internal struct, and gamma generation inside `LeakyTensorPre::generate` is correct per the paper (leaky triples need gamma for future protocol steps). Removing here would cascade into Phase 3-6 territory. |
| `src/lib.rs` | No gamma references. | None. |
| `benches/benchmarks.rs` | No gamma references. | None. |
| `src/auth_tensor_fpre 2.rs`, `src/auth_tensor_pre 2.rs`, `src/leaky_tensor_pre 2.rs`, `src/bcot 2.rs` | Stray macOS duplicate files (spaces in filenames, not Cargo modules). | **IGNORE** — not part of the crate. Optionally delete in a separate hygiene commit, not within Phase 2 scope. |

**Commitment test:** After gamma removal, run `grep -rn "gamma" src/ --include="*.rs" | grep -v "leaky_tensor_pre" | grep -v "^src/.* 2\.rs"` — should produce ZERO matches. `[VERIFIED: this exact grep invocation today produces only leaky_tensor_pre lines once the scope is respected]`

---

## State of the Art

No new state-of-the-art considerations — this is a refactor, not a redesign. Existing tools and patterns (Rust 2024 modules, Criterion benchmark groups, `#[cfg(test)] mod tests` inline layout) are the idiomatic choices and Phase 1 already uses them.

---

## Assumptions Log

> Claims tagged `[ASSUMED]` signal information that could not be directly verified and needs user confirmation.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | D-15's "0 = left child, 1 = right child" mapping matches the actual tree convention in `auth_tensor_eval.rs:103-104`. Code inspection suggests tweak 0 produces the *odd-indexed* sibling (right-of-pair), which is the *opposite* of D-15 if one defines left = even-index. | Pitfall 5 / Code Example 4 | Comment contradicts code; misleading future readers. Recommend using direction-neutral wording ("distinct tweaks for the two children") until the paper convention is confirmed. |
| A2 | The 4 failing baseline tests (`test_run_preprocessing_mac_invariants` + 3 others) are pre-existing and unrelated to Phase 2 decisions. | Pitfall 1 / Open Question Q1 | If the failures turn out to be caused by uncommitted Phase 1 work or a very recent regression, the resolution path differs. Suggested mitigation: run the failing tests on `main` with stash-pop confirmed clean (this research already did so). |
| A3 | Criterion 0.7's `BenchmarkId::new(cf.to_string(), ...)` produces the identical on-disk path as the pre-refactor `BenchmarkId::new("1", ...)` hard-coded literals. | Pitfall 4 / Pattern 2 | If Criterion 0.7 escapes or normalises group/id names differently, baselines would not match. Mitigation: do not rely on baseline comparison survival; accept that a one-time "no baseline" warning on first `cargo bench` run is acceptable. |
| A4 | `src/auth_tensor_pre.rs` edits (removing gamma XOR in `combine_leaky_triples`) are implicitly authorised by D-09 (the field removal makes the current code uncompilable, so the edits are *forced*, not optional). | Gamma Removal: Cascade Boundary | If the user intended to defer the auth_tensor_pre changes and instead keep a stub field, the plan is wrong. Escalate if uncertain — but reading CONTEXT.md "End-to-end gamma removal" in sub-heading language and the decision that "Phase 3-6 will rebuild garble_final from the paper spec anyway" strongly implies the user accepts downstream file edits for field removal. |
| A5 | The stray `src/*.* 2.rs` files (four of them, macOS Finder duplicates) are not included in any Cargo module graph. Confirmed by the fact that `cargo build` succeeds without any compilation of those files. | Runtime State Inventory | If, somehow, Cargo picks one up (e.g., on a Linux machine without the space-normalisation oddity), compilation would produce duplicate-symbol errors. Mitigation: optionally delete or .gitignore these, but not within Phase 2 scope. |

---

## Open Questions

1. **How should "full test suite must pass" be interpreted given the 4 pre-existing baseline failures?**
   - What we know: `cargo test --lib` on current `main` shows 48 passed / 4 failed. The 4 failures are MAC-mismatch panics in the real-preprocessing pipeline (`leaky_tensor_pre`, `auth_tensor_pre`, `run_preprocessing` path). The ideal-dealer end-to-end test (`test_auth_tensor_product`) passes.
   - What's unclear: whether CONTEXT.md's success criterion accepts the baseline status quo or demands these be fixed before Phase 2 work can be verified.
   - Recommendation: **Escalate to user via `ask_user_question` in the planner** before producing plan files. A simple question like *"Phase 2's success criterion says 'full test suite must pass'. There are 4 pre-existing failures on main unrelated to Phase 2 (listed above). Should Phase 2 (a) baseline-accept these as known-red, or (b) include a pre-work task to fix them, or (c) something else?"*. Do not silently pick an interpretation.

2. **Is the D-15 "0 = left, 1 = right" direction mapping literal, or an approximate description?**
   - What we know: The code pattern `seeds[j*2+1] = tccr(Block::from(0), seeds[j]); seeds[j*2] = tccr(Block::from(1), seeds[j])` is consistent at tensor_ops.rs, tensor_eval.rs, and auth_tensor_eval.rs — so the convention is uniform. Calling `j*2` "left" and `j*2+1` "right" is a reasonable interpretation (even-index is canonically left). Under that interpretation, **tweak 1 derives the left child, tweak 0 derives the right child** — opposite of D-15's wording.
   - What's unclear: whether the user intends the comment to say "0=left, 1=right" (matching a mental model of the paper) or whether they would accept the reverse if that's what the code actually does.
   - Recommendation: Implementer traces the tree construction against the KRRW paper (`references/appendix_krrw_pre.tex`), writes the comment that matches the code, and flags the discrepancy in the plan-check review if D-15's exact wording turns out to be reversed. If time-pressed, use the direction-neutral wording shown in Example 4.

3. **Should deduplication also collapse the seven chunking-sweep bench functions `bench_Nx_N_runtime_with_networking` (for N = 4, 8, 16, 32, 64, 128, 256)?**
   - What we know: These seven functions at `benches/benchmarks.rs:375-744` have near-identical bodies (size-dependent only). D-17 explicitly scopes deduplication to `bench_full_protocol_garbling`. D-18 asks for paper-protocol header comments on *each* benchmark group, implying the groups stay as-is.
   - What's unclear: CONTEXT.md's CLEAN-12 target is "5 near-identical chunking-factor blocks" which refers to `bench_full_protocol_garbling`. But a stricter reading of CLEAN-12 ("remove duplicated setup code") could include the seven sibling functions.
   - Recommendation: **Narrow scope** — only `bench_full_protocol_garbling` (per D-17) and `bench_full_protocol_with_networking` (similar structure — lines 164-373 have 5 repeated chunking-factor blocks at cf = 1, 2, 4, 6, 8). The seven size-specific `bench_*x*_runtime_with_networking` functions can be parameterised in a future bench-hygiene phase but are not Phase 2's target unless the user confirms. Ask the planner to note this as a deferred idea if appropriate.

4. **Does `bench_full_protocol_with_networking` (lines 164-373, currently with 5 repeated `chunking_factor = {1,2,4,6,8}` blocks) fall under D-17?**
   - What we know: D-17 names `bench_full_protocol_garbling` specifically. But the sibling function `bench_full_protocol_with_networking` has the same structural duplication.
   - What's unclear: Scope.
   - Recommendation: Include it in the plan. The user's clear intent per CLEAN-12 is to remove duplication; both functions share the identical problem. If the user wanted a narrower fix, they would have told us to exclude it. Reasonable implementer judgement → include.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `rustc` | Build | ✓ | 1.90.0 | — |
| `cargo` | Build / test / bench | ✓ | 1.90.0 | — |
| `rust` edition 2024 support | All crate compilation | ✓ | rustc 1.90.0 supports edition 2024 natively | — |
| Tokio runtime | Bench async | ✓ (via dep) | 1.47.1 | — |
| Criterion | Bench harness | ✓ (via dep) | 0.7 | — |
| `cargo test --lib` baseline green | Success-criterion verification | ✗ | — | Baseline-accept or fix pre-existing (see Open Question Q1) |
| `cargo build` baseline green | Compilation verification | ✓ | — | — |
| `cargo bench --no-run` baseline green | Benchmark compilation | ✓ | — | — |

**Missing dependencies with no fallback:** None strictly; see Open Question Q1 for the "baseline-green test suite" ambiguity which the planner must resolve before execution.

**Missing dependencies with fallback:** None.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` harness + `criterion = "0.7"` for benchmarks |
| Config file | None (uses standard `cargo test`) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test --lib && cargo bench --no-run` |
| Estimated runtime | Tests: ~5 seconds; `cargo bench --no-run`: ~5-10 seconds; full `cargo bench` (`bench_full_protocol_garbling` + `bench_preprocessing` etc.): several minutes — Phase 2 does not require full bench run, only that benchmarks compile and the renamed function is reachable. |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLEAN-07 | `TensorFpre::generate_for_ideal_trusted_dealer` exists and is callable from `test_tensor_fpre_auth_bits` / `test_tensor_fpre_input_sharings` / bench setup helpers | unit + smoke | `cargo test --lib auth_tensor_fpre::` + `cargo bench --no-run` | ✅ (existing tests, updated call sites) |
| CLEAN-08 | `preprocessing::TensorFpreGen` / `TensorFpreEval` / `run_preprocessing` resolve via imports; `auth_tensor_fpre::TensorFpre::into_gen_eval` still returns the correct types | integration | `cargo build --lib --tests --benches` | ✅ (all existing tests, bench build) |
| CLEAN-09 | `TensorFpreGen` and `TensorFpreEval` have `///` doc comments on every public field | manual / doc | `cargo doc --no-deps --lib` then inspect generated HTML, OR `grep -B1 "pub .*:" src/preprocessing.rs \| grep "///"` to check docs precede each field | ⚠️ `src/preprocessing.rs` does not exist yet — Wave 0 creates it |
| CLEAN-10 | No `_gamma_share` dead code in `garble_final`; `// awful return type` comment gone; GGM tweak comment present; doc comments on `garble_final` / `evaluate_final` | grep / visual | `grep -c "_gamma_share\|awful return type" src/auth_tensor_gen.rs` should return `0` ; `grep -c "GGM" src/auth_tensor_eval.rs` should return `>= 1` | ✅ (all target files exist) |
| CLEAN-11 | `src/auth_gen.rs` and `src/auth_eval.rs` do not exist | pre-verified | `test -f src/auth_gen.rs \|\| test -f src/auth_eval.rs; echo $?` should return `1` | ✅ |
| CLEAN-12 | `bench_full_protocol_garbling` contains only one `setup_auth_gen` call in a loop (not five); every `criterion_group!`-registered bench fn has a header `// Benchmarks …` comment | grep + cargo bench --no-run | `grep -c "setup_auth_gen(n, m," benches/benchmarks.rs` — exact count should drop from `13` (in `bench_full_protocol_garbling` and its sibling) to `3-5` depending on how far the dedup extends; `cargo bench --no-run` must succeed | ✅ |
| Regression guard | Existing `test_auth_tensor_product` end-to-end in `lib.rs` still produces the correct output after refactor | integration | `cargo test --lib tests::test_auth_tensor_product` | ✅ |
| Baseline-green regression | No tests that pass on current `main` start failing on the refactor branch | diff | `cargo test --lib 2>&1 \| grep 'FAILED' \| sort > after.txt; diff before.txt after.txt` | ✅ (Wave 0 captures `before.txt` snapshot) |

### Sampling Rate

- **Per task commit:** `cargo test --lib` (~5s) — catches immediate compile regressions and test-break from gamma removal cascade.
- **Per wave merge:** `cargo test --lib && cargo build --benches` (~15s) — adds bench compile check.
- **Phase gate:** `cargo test --lib && cargo bench --no-run` plus baseline-diff comparison against `before.txt` from Wave 0.

### Wave 0 Gaps

- [ ] **Baseline snapshot:** Capture current `cargo test --lib 2>&1 | grep 'FAILED' | sort > .planning/phases/02-.../before.txt` before any code changes so Phase 2 can verify "no *new* failures" (addresses Pitfall 1 in the most pragmatic way short of fixing the 4 pre-existing failures).
- [ ] **Module skeleton creation:** `src/preprocessing.rs` must be created with imports and `pub mod preprocessing;` wired in `src/lib.rs` before gamma removal and field-doc tasks can write into it. This is a Wave 0 boundary task, not a test infrastructure task.
- [ ] Framework install: none — `rustc` / `cargo` already verified available.

*If no other gaps:* The `#[cfg(test)] mod tests` blocks inside each source file continue to serve as the test harness. Phase 2 does not introduce new tests; it updates existing assertions to match the new field layout (gamma removed).

---

## Security Domain

Phase 2 is a pure refactor. No new security controls are introduced; existing cryptographic invariants must be preserved. Per project-internal convention documented in `.planning/codebase/CONVENTIONS.md`:

### Applicable Invariants

| Invariant | Applies | Must be preserved by Phase 2 |
|-----------|---------|------------------------------|
| `Key.lsb() == 0` | yes (all `AuthBitShare.key` fields in `TensorFpreGen/Eval`) | Fields only move, not construction — `Key::new()` is not touched. |
| `Delta.lsb() == 1` | yes (both `delta_a`, `delta_b`) | No delta construction changes. |
| MAC invariant `mac == key.auth(bit, delta)` | yes (correlated, alpha, beta shares) | Gamma removal does not touch non-gamma share construction. |
| Cross-party MAC convention | yes | Unchanged — `verify_cross_party` test helper continues to validate. |
| Column-major indexing (`j*n + i`) | yes (`correlated_auth_bit_shares`) | Unchanged. |
| Same-delta requirement for bucketing (`combine_leaky_triples`) | yes | Unchanged — that assertion logic is not in Phase 2 scope. |

### Known Threat Patterns for Rust Cryptographic Refactor

| Pattern | Risk Category | Mitigation |
|---------|---------------|------------|
| Silent field re-ordering breaks struct-literal initialisation | Correctness | Use named-field struct-literal syntax (already done in code). Rust catches order errors at compile time when fields are named. |
| Hidden gamma-share readers miss removal | Correctness | Exhaustive grep `grep -rn "gamma" src/ benches/ --include="*.rs"` before declaring gamma removal complete. See "Gamma Removal: Cascade Boundary". |
| Renamed function survives in doctests or hidden examples | Correctness | `cargo test` runs doctests. Currently no doctests in this crate — but if the implementer adds examples, they must use the new name. |
| Benchmark ID change invalidates CI performance gates | DevOps | Preserve IDs exactly via `cf.to_string()` in `BenchmarkId::new`. |

---

## Sources

### Primary (HIGH confidence)
- `[VERIFIED]` Local filesystem (read + grep): `src/auth_tensor_fpre.rs`, `src/auth_tensor_gen.rs`, `src/auth_tensor_eval.rs`, `src/auth_tensor_pre.rs`, `src/leaky_tensor_pre.rs`, `src/lib.rs`, `src/sharing.rs`, `src/tensor_ops.rs`, `benches/benchmarks.rs`, `Cargo.toml`, `.planning/codebase/CONVENTIONS.md`, `.planning/codebase/STRUCTURE.md`, `.planning/codebase/TESTING.md`, `.planning/phases/02-.../02-CONTEXT.md`, `.planning/phases/02-.../02-DISCUSSION-LOG.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md`.
- `[VERIFIED]` Toolchain: `rustc 1.90.0 (1159e78c4 2025-09-14)`, `cargo 1.90.0 (840b83a10 2025-07-30)`.
- `[VERIFIED]` Test status baseline: `cargo test --lib` produced "48 passed; 4 failed" on `git stash`-ed clean `main` (commit `c3277b7`) — identified the 4 failing tests by name.
- `[VERIFIED]` Bench compile baseline: `cargo bench --no-run` succeeded on clean `main` — benchmarks compile.

### Secondary (MEDIUM confidence)
- `[CITED: doc.rust-lang.org/reference/items/modules.html]` Rust Reference — Modules (cross-module type returns).
- `[CITED: docs.rs/criterion/0.7/criterion/struct.BenchmarkId.html]` Criterion 0.7 `BenchmarkId` API.

### Tertiary (LOW confidence)
- `[ASSUMED]` D-15 "0 = left, 1 = right" wording may or may not match the KRRW paper's convention when projected onto the observed `seeds[j*2+1] = tccr(0, …)` / `seeds[j*2] = tccr(1, …)` code layout. See Assumption A1 and Open Question Q2.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — nothing new; existing Cargo.toml inspected.
- Architecture / module-split pattern: HIGH — verified by direct read of all affected files and confirmed that `TensorFpre::into_gen_eval` uses struct-literal syntax compatible with moving the return types to another module.
- Gamma cascade boundary: HIGH — exhaustive grep of `gamma` across `src/` identified every reference.
- Test suite baseline: HIGH — verified on current `main`.
- GGM tweak direction mapping (D-15): MEDIUM — two plausible readings of D-15 exist; see Open Question Q2.
- Scope of D-17 (full_protocol_with_networking inclusion): MEDIUM — not explicitly scoped by CONTEXT.md; see Open Question Q4.
- Pre-existing test failure interpretation: LOW — CONTEXT.md is ambiguous; requires user resolution (Open Question Q1).

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — refactor scope is stable; the 4 pre-existing test failures may be repaired by other work, invalidating Pitfall 1 in that direction only)
