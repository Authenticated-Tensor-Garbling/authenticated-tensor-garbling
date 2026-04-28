# Phase 2: M1 Online + Ideal Fpre + Benches Cleanup — Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 6 files (1 create, 5 modify)
**Analogs found:** 6 / 6

---

## File Classification

| File | Change | Role | Data Flow | Closest Analog | Match Quality |
|------|--------|------|-----------|----------------|---------------|
| `src/preprocessing.rs` | CREATE | Rust module (protocol output structs + real-protocol entry) | transform (leaky → authenticated triples) | `src/auth_tensor_pre.rs` (sibling protocol module with struct + free function + tests) and `src/auth_tensor_fpre.rs` (source of the extracted structs + `run_preprocessing`) | exact (structs are being physically moved; `run_preprocessing` is being physically relocated verbatim) |
| `src/auth_tensor_fpre.rs` | MODIFY (shrink) | Rust module (ideal trusted dealer) | CRUD over internal state | Current file (lines 11-245) minus the gamma path and minus `run_preprocessing` | exact (self-analog; this is a trim + rename) |
| `src/auth_tensor_gen.rs` | MODIFY (trim + doc) | Rust module (online garbler) | streaming (chunked outer product) | Current file — apply D-07/D-08/D-13/D-14 in-place | exact (self-analog) |
| `src/auth_tensor_eval.rs` | MODIFY (trim + doc) | Rust module (online evaluator) | streaming (chunked outer product) | Current file — apply D-08/D-14/D-15 in-place | exact (self-analog) |
| `src/auth_tensor_pre.rs` | MODIFY (forced) | Rust module (bucketing combiner) | transform (B leaky triples → one authenticated triple) | Current file — remove `combined_*_gamma` locals + field initialisers; update import of `TensorFpreGen/Eval` to `preprocessing` | exact (self-analog; forced by downstream field removal) |
| `src/lib.rs` | MODIFY (1-line) | Crate root | module declaration | Current file lines 16-22 (existing `pub mod` stanza) | exact (grammatical copy) |
| `benches/benchmarks.rs` | MODIFY (dedup + rename follow-through + import) | Criterion benchmark harness | batch (parameter-swept benchmarks) | `bench_4x4_runtime_with_networking` at lines 375-427 — already demonstrates the exact `for chunking_factor in 1..=8` loop shape the dedup plan needs to adopt for `bench_full_protocol_garbling` | role-match (already-parameterised sibling inside the same file) |

---

## Pattern Assignments

### `src/preprocessing.rs` (NEW module: protocol output structs + real-protocol entry)

**Primary analog:** `src/auth_tensor_fpre.rs` (source of the extraction)
**Secondary analog:** `src/auth_tensor_pre.rs` (sibling module that already holds a protocol-combiner free function plus tests — same structural shape the new module will take)

**Imports pattern** (copy exactly from `auth_tensor_fpre.rs` lines 1-8, minus `AuthBit`/`build_share`/`InputSharing` which stay behind with `TensorFpre`):

```rust
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
use crate::{block::Block, delta::Delta, sharing::AuthBitShare};
```

Import ordering matches the `auth_tensor_pre.rs` precedent — `use crate::{...}` style for the primitive/sharing bundle, single-path `use crate::X` for protocol-layer siblings. Do NOT import `rand` or `rand_chacha` — `run_preprocessing` does not use them directly.

**Struct definitions** (move verbatim from `auth_tensor_fpre.rs:26-50`, drop the `gamma_auth_bit_shares` field per D-09, add `///` docs per D-12):

```rust
pub struct TensorFpreGen {
    /// Tensor row dimension (number of alpha / x-input bits).
    pub n: usize,
    /// Tensor column dimension (number of beta / y-input bits).
    pub m: usize,
    /// GGM tree chunking factor; purely a performance knob (1..=8 in benches).
    pub chunking_factor: usize,
    /// Garbler's (Party A) global correlation key. LSB is always 1.
    pub delta_a: Delta,
    /// Garbler's share of each x-input wire label (length n). Together with the
    /// evaluator's matching `alpha_labels`, reveals `x XOR alpha` via `shares_differ`.
    pub alpha_labels: Vec<Block>,
    /// Garbler's share of each y-input wire label (length m). Reveals `y XOR beta`.
    pub beta_labels: Vec<Block>,
    /// Garbler's `AuthBitShare` for each alpha_i (i in 0..n). MAC committed under delta_b.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each beta_j (j in 0..m). MAC committed under delta_b.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Garbler's `AuthBitShare` for each correlated bit alpha_i AND beta_j (length n*m);
    /// column-major index j*n + i. MAC committed under delta_b.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
}

pub struct TensorFpreEval {
    /// Tensor row dimension (matches TensorFpreGen.n).
    pub n: usize,
    /// Tensor column dimension (matches TensorFpreGen.m).
    pub m: usize,
    /// GGM tree chunking factor.
    pub chunking_factor: usize,
    /// Evaluator's (Party B) global correlation key. LSB is always 1.
    pub delta_b: Delta,
    /// Evaluator's share of each x-input wire label; equals alpha_labels_gen when
    /// `(x XOR alpha)_i = 0`, and equals `alpha_labels_gen XOR delta_a` when =1.
    pub alpha_labels: Vec<Block>,
    /// Evaluator's share of each y-input wire label (symmetric to alpha_labels).
    pub beta_labels: Vec<Block>,
    /// Evaluator's `AuthBitShare` for each alpha_i. MAC committed under delta_a.
    pub alpha_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each beta_j. MAC committed under delta_a.
    pub beta_auth_bit_shares: Vec<AuthBitShare>,
    /// Evaluator's `AuthBitShare` for each correlated bit (column-major, length n*m).
    /// MAC committed under delta_a.
    pub correlated_auth_bit_shares: Vec<AuthBitShare>,
}
```

Field doc style — follow the `LeakyTriple` precedent in `src/leaky_tensor_pre.rs:13-35`:

```rust
/// length n*m, column-major: index = j*n+i (j = beta/y index, i = alpha/x index)
pub gen_correlated_shares: Vec<AuthBitShare>,
```

That short, per-field `///` with column-major annotation is the established shape.

**`run_preprocessing` body** (move verbatim from `auth_tensor_fpre.rs:247-292`, no changes to internals):

```rust
/// Run the real two-party uncompressed preprocessing protocol (Pi_aTensor, Construction 3).
/// [... full existing docblock at auth_tensor_fpre.rs:247-266 copied as-is ...]
pub fn run_preprocessing(
    n: usize,
    m: usize,
    count: usize,
    chunking_factor: usize,
) -> (TensorFpreGen, TensorFpreEval) {
    assert_eq!(count, 1, "Phase 1: only count=1 is supported; batch output requires Vec return");
    let bucket_size = bucket_size_for(n, m);
    let total_leaky = bucket_size * count;
    let mut bcot = IdealBCot::new(0, 1);
    let mut triples = Vec::with_capacity(total_leaky);
    for t in 0..total_leaky {
        let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, &mut bcot);
        triples.push(ltp.generate(0, 0));
    }
    combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 0)
}
```

**Tests sub-module pattern** (preserve the `test_run_preprocessing_*` tests; relocate them to `preprocessing.rs` alongside the moved function — analog `auth_tensor_pre.rs:116-219` shows the `#[cfg(test)] mod tests` layout):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    use crate::sharing::AuthBitShare;

    // The four tests currently at auth_tensor_fpre.rs:398-443 move here,
    // with the gamma loop (lines 432-434) removed per D-11.
    // Keep test names identical for stable test IDs.
}
```

---

### `src/auth_tensor_fpre.rs` (MODIFY — trim to ideal-only)

**Self-analog** — the remaining code is the current file minus (a) the two output structs, (b) `run_preprocessing`, (c) all gamma generation/storage, (d) the renamed method.

**Imports pattern after trim** (drop the three imports used only by `run_preprocessing`):

```rust
// Current line 1: the TODO comment stays — it predates Phase 2.
// TODO refactor authbit from fpre to a common module, or redefine with new name.
use crate::{block::Block, delta::Delta, sharing::{AuthBit, build_share, InputSharing}};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
// DROPPED: use crate::bcot::IdealBCot;
// DROPPED: use crate::leaky_tensor_pre::LeakyTensorPre;
// DROPPED: use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
// DROPPED: AuthBitShare (no longer referenced here after struct move)
```

**Cross-module return type in `into_gen_eval`** — add a new import (this is the critical cross-module linkage):

```rust
use crate::preprocessing::{TensorFpreGen, TensorFpreEval};
```

Body of `into_gen_eval` (current lines 194-219) stays byte-for-byte identical EXCEPT remove the two `gamma_auth_bit_shares: self.gamma_auth_bits.iter().map(...).collect(),` lines. The struct-literal syntax is field-name-based so this is a clean deletion.

**`TensorFpre` struct** (current lines 11-24) — delete line 23 `gamma_auth_bits: Vec<AuthBit>,`.

**`new` / `new_with_delta`** (current lines 52-94) — delete the `gamma_auth_bits: Vec::with_capacity(n * m),` initialiser from both.

**`generate_for_ideal_trusted_dealer` (renamed from `generate_with_input_values`)** — current lines 119-192:
- Replace the three-line doc at 119-121 with the D-06 wording.
- Rename `generate_with_input_values` → `generate_for_ideal_trusted_dealer` (signature line 122).
- Inside the nested loop at lines 179-190 delete the three lines that compute and push `gamma_auth_bit`:

```rust
// DELETE these three lines inside the column-major loop:
let g = self.rng.random_bool(0.5);
let gamma_auth_bit = self.gen_auth_bit(g);
self.gamma_auth_bits.push(gamma_auth_bit);
```

The `alpha`/`beta`/`alpha_beta` triple still executes; only the gamma initialiser is removed.

**Tests at lines 294-443**:
- `test_tensor_fpre_auth_bits`: call site at 304 renames; delete `assert_eq!(fpre.gamma_auth_bits.len(), n * m);` at line 310; delete the `for bit in &fpre.gamma_auth_bits { ... }` loop at lines 325-327.
- `test_tensor_fpre_input_sharings`: call site at 347 renames; delete `assert_eq!(fpre_gen.gamma_auth_bit_shares.len(), n * m);` at line 382 and `assert_eq!(fpre_eval.gamma_auth_bit_shares.len(), n * m);` at line 392.
- `test_run_preprocessing_*` tests at 399-443 **move to `preprocessing.rs`** (they test `run_preprocessing`). Delete them from this file.
- Inside the moved `test_run_preprocessing_mac_invariants`, drop the gamma verification loop at lines 432-434 per D-11.

**Doc comment pattern for D-06** (use the CONTEXT.md wording verbatim):

```rust
/// Generates all authenticated bits and input sharings for the ideal trusted dealer.
/// This is NOT the real preprocessing protocol — it is the ideal functionality
/// (trusted dealer) that the online phase consumes directly in tests and benchmarks.
pub fn generate_for_ideal_trusted_dealer(&mut self, x: usize, y: usize) -> (usize, usize) {
```

---

### `src/auth_tensor_gen.rs` (MODIFY — gamma removal + docs + comment cleanup)

**Self-analog** for all changes. Five distinct edits:

**1. Import path change** (line 8):

```rust
// BEFORE:
use crate::auth_tensor_fpre::TensorFpreGen,
// AFTER:
use crate::preprocessing::TensorFpreGen,
```

**2. Struct field removal** (line 29) — delete:
```rust
pub gamma_auth_bit_shares: Vec<AuthBitShare>,
```

**3. Constructor field removal** (lines 48 and 66) — delete from both `new` and `new_from_fpre_gen`:
```rust
gamma_auth_bit_shares: Vec::new(),                            // line 48
gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,        // line 66
```

**4. `gen_chunked_half_outer_product` return comment** (line 79) — per D-13, remove the `// awful return type` trailing comment. Keep the existing return type unchanged (the CONTEXT discretion note in D-13 authorises "just drop the comment" if renaming is not a clean one-liner; creating a named tuple struct for `(Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>)` would ripple into callers in `auth_tensor_eval.rs` and `benchmarks.rs`, so drop the comment only):

```rust
// BEFORE (line 79):
) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) { // awful return type
// AFTER:
) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
```

**5. `garble_final` doc + dead code removal** (lines 179-199) — per D-07 + D-14, add a `///` doc header and delete the `_gamma_share` block:

```rust
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
            // DELETE: lines 188-192 — the _gamma_share let-binding was never XORed
            // into first_half_out, confirming it was dead code.
            self.first_half_out[(i, j)] ^=
                self.second_half_out[(j, i)] ^
                correlated_share;
        }
    }
}
```

Doc-comment style precedent — copy the shape of the sibling docs on `get_first_inputs` (lines 115-117) which already use `///` line-doc on a public protocol step method.

**6. Tests at lines 202-239**:
- Call-site rename at line 213: `fpre.generate_with_input_values(0b1101, 0b110);` → `fpre.generate_for_ideal_trusted_dealer(0b1101, 0b110);`.
- Delete line 224 (`assert_eq!(fpre_gen.gamma_auth_bit_shares.len(), n * m);`).
- Delete line 235 (`assert_eq!(gar.gamma_auth_bit_shares.len(), n * m);`).

---

### `src/auth_tensor_eval.rs` (MODIFY — gamma removal + docs + GGM comment)

**Self-analog.** Four distinct edits:

**1. Import path change** (line 4):

```rust
// BEFORE:
use crate::auth_tensor_fpre::TensorFpreEval;
// AFTER:
use crate::preprocessing::TensorFpreEval;
```

**2. Struct field removal** (line 23) — delete:
```rust
pub gamma_auth_bit_shares: Vec<AuthBitShare>,
```

**3. Constructor field removal** (lines 42 and 60) — delete from both `new` and `new_from_fpre_eval`:
```rust
gamma_auth_bit_shares: Vec::new(),                            // line 42
gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,       // line 60
```

**4. GGM tweak comment** (lines 103-104) — per D-15 with Pitfall-5 mitigation (direction-neutral wording is safest; the code does `seeds[j*2+1] = tccr(0, ...)` and `seeds[j*2] = tccr(1, ...)`, so naive "0=left, 1=right" wording contradicts the `j*2` = even = left reading). Use the neutral form from RESEARCH Example 4:

```rust
// GGM tree tweak domain separation: distinct AES tweaks derive the two child
// seeds at each level (tweak=0 for odd-indexed sibling, tweak=1 for even-indexed).
seeds[j * 2 + 1] = cipher.tccr(Block::from(0 as u128), seeds[j]);
seeds[j * 2]     = cipher.tccr(Block::from(1 as u128), seeds[j]);
```

**Analog precedent for tweak naming** — `src/tensor_ops.rs:58-66` already has the pattern "Two seeds per parent: left child (even) and right child (odd)" as an inline comment. Reuse that same comment body style here (`auth_tensor_eval.rs` currently has no such comment). Do NOT modify `tensor_ops.rs` — D-15 scope is `auth_tensor_eval.rs` only.

**5. `evaluate_final` doc** (line 255) — per D-14:

```rust
/// Combines both half-outer-product outputs with the correlated preprocessing
/// MAC to produce the evaluator's share of the garbled tensor gate output.
pub fn evaluate_final(&mut self) {
    // body unchanged — evaluate_final never referenced gamma, so no code deletion.
}
```

No code body changes to `evaluate_final` — the file already did not consume gamma (confirmed by grepping: no `gamma` tokens appear inside `evaluate_final`).

---

### `src/auth_tensor_pre.rs` (MODIFY — forced by field removal; NOT originally in Phase 2 scope but required)

**Self-analog + Pitfall 3.** This file is not in CONTEXT.md's "Source Files in Scope" but it must change, otherwise `cargo build` fails after D-09 removes the `gamma_auth_bit_shares` field from `TensorFpreGen`/`TensorFpreEval`.

**1. Import path change** (lines 1-4):

```rust
// BEFORE:
use crate::{
    auth_tensor_fpre::{TensorFpreGen, TensorFpreEval},
    leaky_tensor_pre::LeakyTriple,
};
// AFTER:
use crate::{
    preprocessing::{TensorFpreGen, TensorFpreEval},
    leaky_tensor_pre::LeakyTriple,
};
```

**2. Docstring edit at line 32** — drop "and gamma shares" from the `combine_leaky_triples` algorithm description:

```rust
// BEFORE (line 30-33):
/// Algorithm (XOR combination):
///   Keep first triple's alpha/beta/labels.
///   XOR-combine all B triples' correlated and gamma shares.
// AFTER:
/// Algorithm (XOR combination):
///   Keep first triple's alpha/beta/labels.
///   XOR-combine all B triples' correlated shares.
```

**3. Local bindings + loop body** (lines 70-84) — drop the two gamma locals and their inner-loop update:

```rust
// BEFORE:
let mut combined_gen_corr = triples[0].gen_correlated_shares.clone();
let mut combined_eval_corr = triples[0].eval_correlated_shares.clone();
let mut combined_gen_gamma = triples[0].gen_gamma_shares.clone();
let mut combined_eval_gamma = triples[0].eval_gamma_shares.clone();

for t in triples[1..].iter() {
    for k in 0..(n * m) {
        combined_gen_corr[k] = combined_gen_corr[k] + t.gen_correlated_shares[k];
        combined_eval_corr[k] = combined_eval_corr[k] + t.eval_correlated_shares[k];
        combined_gen_gamma[k] = combined_gen_gamma[k] + t.gen_gamma_shares[k];
        combined_eval_gamma[k] = combined_eval_gamma[k] + t.eval_gamma_shares[k];
    }
}
// AFTER: drop the two `combined_*_gamma` let-bindings AND the two inner loop lines
```

**4. Struct-literal field removal** (lines 99 and 111) — delete:

```rust
gamma_auth_bit_shares: combined_gen_gamma,    // line 99
gamma_auth_bit_shares: combined_eval_gamma,   // line 111
```

**5. Test at line 176** — delete:

```rust
assert_eq!(eval_out.gamma_auth_bit_shares.len(), n * m);
```

**Critical non-edit:** `LeakyTriple.gen_gamma_shares` / `eval_gamma_shares` (defined at `src/leaky_tensor_pre.rs:21, 29`) MUST remain. D-10 / D-11 do not extend into `leaky_tensor_pre.rs` — it is out of Phase 2 scope per RESEARCH "Gamma Removal: Cascade Boundary". `LeakyTriple` still generates gamma shares; `auth_tensor_pre.rs` simply stops propagating them.

---

### `src/lib.rs` (MODIFY — one-line module declaration)

**Self-analog** — the existing lines 16-22 already declare sibling protocol modules. Add one line:

```rust
// Current lines 16-22:
pub mod auth_tensor_fpre;
pub mod auth_tensor_gen;
pub mod auth_tensor_eval;

pub mod bcot;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;

// ADD (recommended placement — grouped with the preprocessing-pipeline modules
// near `auth_tensor_pre`, since the new module holds the real-protocol entry):
pub mod preprocessing;
```

No other edits to `lib.rs`. The integration test `test_auth_tensor_product` at lines 249-377 already uses the method name `generate_with_input_values` at line 262 — **this rename follow-through is required**:

```rust
// lib.rs line 262, BEFORE:
fpre.generate_with_input_values(input_x, input_y);
// AFTER:
fpre.generate_for_ideal_trusted_dealer(input_x, input_y);
```

(This is a Phase-2 rename follow-through that must not be missed — `lib.rs` is not listed in CONTEXT.md's "Source Files in Scope" but the rename D-06 forces this single-site update.)

---

### `benches/benchmarks.rs` (MODIFY — dedup + rename follow-through + import path)

**Primary analog:** `bench_4x4_runtime_with_networking` at lines 375-427 of the same file. It already uses the target loop shape:

```rust
for chunking_factor in 1..=8 {
    let n = 4;
    let m = 4;
    let mut generator = setup_auth_gen(n, m, chunking_factor);
    // ...
    group.bench_with_input(
        BenchmarkId::new("Chunking factor", format!("{}", chunking_factor)),
        &(chunking_factor),
        |b, &chunking_factor| { /* ... */ },
    );
}
```

The deduplicated `bench_full_protocol_garbling` must preserve the existing benchmark IDs (`"1"`, `"2"`, `"4"`, `"6"`, `"8"`) so Criterion baselines at `target/criterion/full_protocol_garbling/{1,2,4,6,8}/` stay valid — so use `[1usize, 2, 4, 6, 8]` explicit iteration and `cf.to_string()`, NOT `1..=8`.

**Four distinct edits:**

**1. Import path change** (line 16):

```rust
// BEFORE:
auth_tensor_fpre::{TensorFpre, run_preprocessing},
// AFTER:
auth_tensor_fpre::TensorFpre,
preprocessing::run_preprocessing,
```

**2. Setup helper rename follow-through** (lines 73 and 80) — per D-19:

```rust
// BEFORE (line 73):
fpre.generate_with_input_values(X_INPUT, Y_INPUT);
// AFTER:
fpre.generate_for_ideal_trusted_dealer(X_INPUT, Y_INPUT);
// (same at line 80)
```

**3. Deduplicate `bench_full_protocol_garbling`** (lines 86-161) — per D-17, collapse five near-identical blocks into a loop. Target shape:

```rust
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

**4. Paper-protocol header comments** (D-18) — add to each `fn bench_*` definition. Precedent: there is currently NO such comment on any bench function. Use:

```rust
// Benchmarks online garbling for authenticated tensor gate (auth_tensor_gen / auth_tensor_eval).
fn bench_full_protocol_garbling(c: &mut Criterion) { ... }

// Benchmarks online garbling + simulated network I/O for the authenticated tensor gate.
fn bench_full_protocol_with_networking(c: &mut Criterion) { ... }

// Benchmarks the real two-party preprocessing pipeline (Pi_aTensor, Construction 3).
fn bench_preprocessing(c: &mut Criterion) { ... }
```

And similar headers for each of `bench_Nx_N_runtime_with_networking`.

**Scope note on `bench_full_protocol_with_networking`:** Per RESEARCH Open Question Q4, the same 5-block duplication exists in this function at lines 164-373. RESEARCH recommends including it under D-17's umbrella. The plan should either (a) dedupe it the same way, or (b) explicitly skip per planner judgement. Preferred: dedupe.

---

## Shared Patterns

### Pattern S1: Cross-Module Return Type (module split)
**Source:** Rust Reference — standard pattern; concrete use: `TensorFpre::into_gen_eval` returns `preprocessing` types from `auth_tensor_fpre`.
**Apply to:** Structure of `src/preprocessing.rs` + `src/auth_tensor_fpre.rs` after split.

```rust
// In auth_tensor_fpre.rs:
use crate::preprocessing::{TensorFpreGen, TensorFpreEval};
impl TensorFpre {
    pub fn into_gen_eval(self) -> (TensorFpreGen, TensorFpreEval) {
        (TensorFpreGen { /* named fields */ }, TensorFpreEval { /* named fields */ })
    }
}
```

Rust allows returning types defined in another module without re-exports. Named-field struct-literal construction is tolerant of field reordering, so the removed gamma field simply vanishes from the constructor without touching other fields.

### Pattern S2: Per-field `///` Doc Style with Column-Major Annotation
**Source:** `src/leaky_tensor_pre.rs:13-35` (`LeakyTriple` struct — existing precedent).
**Apply to:** `TensorFpreGen` and `TensorFpreEval` fields in `preprocessing.rs` per D-12.

```rust
/// length n*m, column-major: index = j*n+i (j = beta/y index, i = alpha/x index)
pub gen_correlated_shares: Vec<AuthBitShare>,
```

Short, one-line-when-possible, ends with layout annotation for any n*m-length field. Specify MAC-verifier-delta (e.g., "MAC committed under delta_b") for every `AuthBitShare` field — matches the cross-party MAC convention documented in `.planning/codebase/CONVENTIONS.md:51-64`.

### Pattern S3: Import Ordering
**Source:** `src/leaky_tensor_pre.rs:1-9` (precedent cited in `.planning/codebase/CONVENTIONS.md:92-102`).
**Apply to:** `src/preprocessing.rs` top-of-file.

```rust
// Layer 1: crate:: imports grouped by conceptual layer (primitives first, protocol second)
use crate::{block::Block, delta::Delta, sharing::AuthBitShare};
use crate::bcot::IdealBCot;
use crate::leaky_tensor_pre::LeakyTensorPre;
use crate::auth_tensor_pre::{combine_leaky_triples, bucket_size_for};
// Layer 2: external crates (none needed in preprocessing.rs)
// Layer 3: std:: (none needed)
```

### Pattern S4: `#[cfg(test)] mod tests` Bottom-of-File Layout
**Source:** `src/auth_tensor_pre.rs:116-219`.
**Apply to:** `src/preprocessing.rs` — the four `test_run_preprocessing_*` tests from `auth_tensor_fpre.rs:398-443` move here, preserving names and structure.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_tensor_gen::AuthTensorGen;
    use crate::auth_tensor_eval::AuthTensorEval;
    // tests identical except: gamma-verifying loop at lines 432-434 deleted per D-11
}
```

### Pattern S5: Named-Field Struct Literal (no field reordering)
**Source:** `src/auth_tensor_fpre.rs:194-219` and `src/auth_tensor_pre.rs:89-113`.
**Apply to:** All struct constructions after gamma field removal — simply drop the removed field's `name: value,` line; no reordering.

Rust's named-field struct literal is order-independent and will emit E0063 ("missing field") at compile time if any required field is omitted — this is the cryptographic safety net against silent breakage.

### Pattern S6: `assert!` / `assert_eq!` Precondition Errors
**Source:** `src/auth_tensor_pre.rs:46-47` and `:56-67`; convention cited in `.planning/codebase/CONVENTIONS.md:107-112`.
**Apply to:** `preprocessing::run_preprocessing` (preserves existing `assert_eq!(count, 1, ...)` at current `auth_tensor_fpre.rs:273`). Do NOT introduce `Result`/`Error` types — the project uses panics for protocol preconditions.

### Pattern S7: Criterion BenchmarkId Preservation
**Source:** `benches/benchmarks.rs:96, 109, 122, 135, 149` (existing baseline IDs `"1"`, `"2"`, `"4"`, `"6"`, `"8"`).
**Apply to:** Deduplicated `bench_full_protocol_garbling`. Use `[1usize, 2, 4, 6, 8]` iteration and `BenchmarkId::new(cf.to_string(), format!("{}x{}", n, m))` to produce byte-identical IDs. Preserves Criterion baseline comparison at `target/criterion/full_protocol_garbling/{1,2,4,6,8}/`.

---

## No Analog Found

All six files have exact or role-match analogs. No files require fallback to RESEARCH.md patterns.

---

## Metadata

**Analog search scope:** `src/`, `benches/`, `.planning/codebase/` (for conventions).
**Files scanned (full):** `src/auth_tensor_fpre.rs` (443 lines, full), `src/auth_tensor_gen.rs` (240 lines, full), `src/auth_tensor_eval.rs` (264 lines, full), `src/auth_tensor_pre.rs` (220 lines, full), `src/lib.rs` (378 lines, full), `benches/benchmarks.rs` (803 lines, full).
**Files scanned (targeted sections):** `src/leaky_tensor_pre.rs` (lines 1-80 for doc-comment precedent), `src/tensor_ops.rs` (lines 50-122 for GGM tweak comment precedent).
**Convention sources:** `.planning/codebase/CONVENTIONS.md` (full), `.planning/codebase/STRUCTURE.md` (lines 1-80).

**Pattern extraction date:** 2026-04-21
