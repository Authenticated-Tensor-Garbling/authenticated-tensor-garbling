# Phase 9: Protocol 2 Garble/Eval/Check - Pattern Map

**Mapped:** 2026-04-24
**Files analyzed:** 5 modified files (no new files)
**Analogs found:** 5 / 5

---

## File Classification

| Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/preprocessing.rs` | model / data struct | CRUD (field add + rename) | `src/preprocessing.rs` (Phase 7 gamma field addition) | exact — same struct, same operation type |
| `src/tensor_ops.rs` | utility / transform | transform (dual PRG expansion) | `src/tensor_ops.rs:80–114` (`gen_unary_outer_product`) | exact — parallel accumulation variant of the same function |
| `src/auth_tensor_gen.rs` | service / garbler | request-response (garble pipeline) | `src/auth_tensor_gen.rs:171–250` (P1 garble methods) | exact — `_p2` methods mirror P1 method naming and return-type conventions |
| `src/auth_tensor_eval.rs` | service / evaluator | request-response (eval pipeline) | `src/auth_tensor_eval.rs:151–229` (P1 evaluate methods) | exact — `_p2` methods mirror P1 method naming and struct layout |
| `src/lib.rs` | test | request-response (in-process E2E) | `src/lib.rs:507–574` (`test_auth_tensor_product_full_protocol_1`) | exact — same test structure, same helpers, P2 delta swap |

---

## Pattern Assignments

### `src/preprocessing.rs` — field rename + three new D_ev fields

**Analog:** `src/preprocessing.rs:140–162` (`IdealPreprocessingBackend::run`) — Phase 7 pattern for adding `gamma_auth_bit_shares`.

**Existing field declaration pattern** (`preprocessing.rs:41–44` for `TensorFpreGen`, `69–72` for `TensorFpreEval`):
```rust
// Current field being renamed (TensorFpreGen):
pub gamma_auth_bit_shares: Vec<AuthBitShare>,

// Current field being renamed (TensorFpreEval):
pub gamma_auth_bit_shares: Vec<AuthBitShare>,
```

**Target state after rename + three new fields** — copy this pattern for each of `TensorFpreGen` and `TensorFpreEval`:
```rust
// Rename gamma_auth_bit_shares -> gamma_d_ev_shares everywhere (D-05).
// Add the three new D_ev fields below (D-04).
pub alpha_d_ev_shares: Vec<AuthBitShare>,      // length n — l_alpha under D_ev
pub beta_d_ev_shares: Vec<AuthBitShare>,       // length m — l_beta under D_ev
pub correlated_d_ev_shares: Vec<AuthBitShare>, // length n*m col-major — l_gamma* under D_ev
pub gamma_d_ev_shares: Vec<AuthBitShare>,      // length n*m col-major — l_gamma under D_ev (renamed)
```

**Constructor initialization pattern** (`preprocessing.rs:59–76`, `new_from_fpre_gen`):
```rust
// Every constructor must set all new fields; leaving any unset causes compile error.
// Follow this existing pattern for the struct literal in new_from_fpre_gen / new_from_fpre_eval:
gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
// After rename, all four D_ev fields become:
alpha_d_ev_shares: fpre_gen.alpha_d_ev_shares,
beta_d_ev_shares: fpre_gen.beta_d_ev_shares,
correlated_d_ev_shares: fpre_gen.correlated_d_ev_shares,
gamma_d_ev_shares: fpre_gen.gamma_d_ev_shares,
```

**`IdealPreprocessingBackend::run` generation pattern** (`preprocessing.rs:144–160`) — copy this block three more times for the three new fields:
```rust
// Existing gamma generation (lines 145–149, 158–159) — template for the three new fields:
let mut rng = ChaCha12Rng::seed_from_u64(42);
let mut gamma_auth_bits: Vec<crate::sharing::AuthBit> = Vec::with_capacity(n * m);
for _ in 0..(n * m) {
    let l_gamma: bool = rng.random_bool(0.5);
    gamma_auth_bits.push(fpre.gen_auth_bit(l_gamma));
}
// After into_gen_eval():
gen_out.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.gen_share).collect();
eval_out.gamma_auth_bit_shares = gamma_auth_bits.iter().map(|b| b.eval_share).collect();
```
For the three new fields use distinct RNG seeds (e.g., 43, 44, 45) and lengths `n`, `m`, `n*m` respectively. The ordering constraint (all `gen_auth_bit` calls BEFORE `into_gen_eval`) is documented at `preprocessing.rs:141–154` and MUST be preserved.

**Rename scope** — grep target before committing:
```
grep -rn "gamma_auth_bit_shares" src/
```
Must return zero matches after the rename. Known locations: `preprocessing.rs`, `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `src/lib.rs` (the `assemble_c_gamma_shares` function and tests).

---

### `src/tensor_ops.rs` — `gen_unary_outer_product_wide` + `eval_unary_outer_product_wide`

**Analog:** `src/tensor_ops.rs:80–114` (`gen_unary_outer_product`) — direct template.

**Imports pattern** (`tensor_ops.rs:1–6`) — unchanged, no new imports needed:
```rust
use crate::{
    aes::FixedKeyAes,
    block::Block,
    delta::Delta,
    matrix::{MatrixViewMut, MatrixViewRef},
};
```

**gen_unary_outer_product core pattern** (`tensor_ops.rs:80–114`) — the wide variant doubles the inner loop body:
```rust
// Narrow version (template, lines 93–113):
for j in 0..m {
    let mut row: Block = Block::default();
    for i in 0..seeds.len() {
        let tweak = (seeds.len() * j + i) as u128;
        let s = cipher.tccr(Block::from(tweak), seeds[i]);
        row ^= s;
        for k in 0..out.rows() {
            if ((i >> k) & 1) == 1 {
                out[(k, j)] ^= s;
            }
        }
    }
    row ^= y[j];
    gen_cts.push(row);
}
```

**Wide leaf expansion** (D-03) — replace the single TCCR call with even/odd pair matching the existing `gen_populate_seeds_mem_optimized` tweak convention (`tensor_ops.rs:55–56`):
```rust
// Even/odd tweak convention (lines 55–56 — established pattern):
let tweak_even = Block::from(((i as u128) << 1) as u128);
let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);

// Applied to gen_unary_outer_product_wide inner loop (D-03):
let base = (seeds.len() * j + i) as u128;
let s_gb = cipher.tccr(Block::from(base << 1),     seeds[i]);   // κ-half
let s_ev = cipher.tccr(Block::from(base << 1 | 1), seeds[i]);   // ρ-half
// Accumulate BOTH halves in the SAME loop pass (never separate passes):
row_gb ^= s_gb;
row_ev ^= s_ev;
for k in 0..out_gb.rows() {
    if ((i >> k) & 1) == 1 {
        out_gb[(k, j)] ^= s_gb;
        out_ev[(k, j)] ^= s_ev;
    }
}
```

**gen_unary_outer_product_wide full signature** (D-01, CONTEXT specifics):
```rust
pub(crate) fn gen_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,   // B_k values (D_gb half, length m)
    y_d_ev: &MatrixViewRef<Block>,   // B'_k values (D_ev half, length m)
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
) -> Vec<(Block, Block)>             // wide ciphertexts: (kappa_half, rho_half)
```

**eval_unary_outer_product core pattern** (`tensor_ops.rs:220–265`) — wide variant mirrors gen pattern; both `y_d_gb` and `y_d_ev` are required parameters:
```rust
// Narrow eval signature (lines 220–228):
pub(crate) fn eval_unary_outer_product(
    seeds: &[Block],
    y: &MatrixViewRef<Block>,
    out: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[Block],
) -> Vec<Block>

// Wide eval signature — both y halves + wide ciphertexts:
pub(crate) fn eval_unary_outer_product_wide(
    seeds: &[Block],
    y_d_gb: &MatrixViewRef<Block>,
    y_d_ev: &MatrixViewRef<Block>,
    out_gb: &mut MatrixViewMut<Block>,
    out_ev: &mut MatrixViewMut<Block>,
    cipher: &FixedKeyAes,
    missing: usize,
    gen_cts: &[(Block, Block)],       // wide ciphertexts from garbler
) -> Vec<(Block, Block)>
```

**eval_unary_outer_product missing-leaf recovery** (`tensor_ops.rs:237–264`) — in the wide variant, the `eval_ct` line and the post-loop `out[(k,j)]` update both split across `_gb` and `_ev`:
```rust
// Narrow (line 254):
eval_ct ^= gen_cts[j] ^ y[j];
// Wide equivalent (per Construction 4 step 4 of 6_total.tex):
eval_ct_gb ^= gen_cts[j].0 ^ y_d_gb[j];
eval_ct_ev ^= gen_cts[j].1 ^ y_d_ev[j];
```

---

### `src/auth_tensor_gen.rs` — `_p2` garble methods

**Analog:** `src/auth_tensor_gen.rs:171–251` (existing P1 garble methods).

**Imports pattern** (`auth_tensor_gen.rs:1–12`) — add the wide variant import:
```rust
use crate::{
    aes::{FixedKeyAes, FIXED_KEY_AES},
    delta::Delta,
    sharing::AuthBitShare,
    preprocessing::TensorFpreGen,
    block::Block,
    matrix::{BlockMatrix, MatrixViewRef},
    // Add wide ops import:
    tensor_ops::{gen_populate_seeds_mem_optimized, gen_unary_outer_product, gen_unary_outer_product_wide},
};
```

**New D_ev fields on `AuthTensorGen` struct** — follow the existing field block pattern (`auth_tensor_gen.rs:26–36`):
```rust
// Existing fields (lines 26–29) — D_ev counterparts follow same naming:
pub alpha_auth_bit_shares: Vec<AuthBitShare>,
pub beta_auth_bit_shares: Vec<AuthBitShare>,
pub correlated_auth_bit_shares: Vec<AuthBitShare>,
pub gamma_auth_bit_shares: Vec<AuthBitShare>,   // rename to gamma_d_ev_shares

// Three new D_ev fields to add (D-04):
pub alpha_d_ev_shares: Vec<AuthBitShare>,
pub beta_d_ev_shares: Vec<AuthBitShare>,
pub correlated_d_ev_shares: Vec<AuthBitShare>,
// gamma_d_ev_shares already exists after rename
```

**`new_from_fpre_gen` constructor pattern** (`auth_tensor_gen.rs:59–76`) — add new field assignments in the struct literal, one per new field:
```rust
// Existing pattern (line 71):
gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
// After rename + new fields:
alpha_d_ev_shares: fpre_gen.alpha_d_ev_shares,
beta_d_ev_shares: fpre_gen.beta_d_ev_shares,
correlated_d_ev_shares: fpre_gen.correlated_d_ev_shares,
gamma_d_ev_shares: fpre_gen.gamma_d_ev_shares,
```

**`garble_first_half` / `garble_second_half` method pattern** (`auth_tensor_gen.rs:171–183`) — `_p2` variants follow the same get_inputs → gen_chunked call structure, using a `_wide` chunked helper:
```rust
// P1 pattern (lines 171–176):
pub fn garble_first_half(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) {
    let (x, y) = self.get_first_inputs();
    let (chunk_levels, chunk_cts) = self.gen_chunked_half_outer_product(&x.as_view(), &y.as_view(), true);
    (chunk_levels, chunk_cts)
}

// P2 variant signature — returns wide ciphertexts Vec<Vec<(Block,Block)>> instead of Vec<Vec<Block>>:
pub fn garble_first_half_p2(&mut self) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<(Block, Block)>>)
//                                          chunk_levels (unchanged)    chunk_cts (wide)
```

**`garble_final_p2` return type** (D-10) — differs from `garble_final` which returns nothing:
```rust
// P1 garble_final (lines 187–202): mutates first_half_out in place, returns ()
pub fn garble_final(&mut self) { ... }

// P2 variant: combines halves for BOTH D_gb and D_ev paths; returns both output share vecs:
pub fn garble_final_p2(&mut self) -> (Vec<Block>, Vec<Block>)
//                                    D_gb shares   D_ev shares
// First Vec<Block>:  [v_gamma D_gb]^gb — one Block per (i,j) gate output, length n*m
// Second Vec<Block>: [v_gamma D_ev]^gb — one Block per (i,j) gate output, length n*m
// D_gb assembly mirrors garble_final (lines 190–198); D_ev assembly uses correlated_d_ev_shares.
```

**`garble_final_p2` core assembly pattern** (D_gb path mirrors `garble_final`, `auth_tensor_gen.rs:188–201`):
```rust
// D_gb path (copy from garble_final lines 190–201):
for i in 0..self.n {
    for j in 0..self.m {
        let correlated_share = if self.correlated_auth_bit_shares[j * self.n + i].bit() {
            self.delta_a.as_block() ^ self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
        } else {
            *self.correlated_auth_bit_shares[j * self.n + i].key.as_block()
        };
        self.first_half_out[(i, j)] ^=
            self.second_half_out[(j, i)] ^
            correlated_share;
    }
}
// D_ev path — same loop, using correlated_d_ev_shares (key-based) for garbler side:
// (The garbler's D_ev correlated share is the key-based encoding, same bit() pattern)
```

---

### `src/auth_tensor_eval.rs` — `_p2` evaluate methods

**Analog:** `src/auth_tensor_eval.rs:33–229` (existing P1 evaluate methods). Symmetric to gen.

**New D_ev fields on `AuthTensorEval` struct** — follow `auth_tensor_eval.rs:19–22`:
```rust
// Existing D_ev-committed fields (lines 19–22):
pub alpha_auth_bit_shares: Vec<AuthBitShare>,
pub beta_auth_bit_shares: Vec<AuthBitShare>,
pub correlated_auth_bit_shares: Vec<AuthBitShare>,
pub gamma_auth_bit_shares: Vec<AuthBitShare>,   // rename to gamma_d_ev_shares

// Three new D_ev fields (D-04):
pub alpha_d_ev_shares: Vec<AuthBitShare>,
pub beta_d_ev_shares: Vec<AuthBitShare>,
pub correlated_d_ev_shares: Vec<AuthBitShare>,
```

**`new_from_fpre_eval` constructor pattern** (`auth_tensor_eval.rs:52–69`) — identical to gen constructor, eval side:
```rust
// Existing line 64:
gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,
// After rename + new fields:
alpha_d_ev_shares: fpre_eval.alpha_d_ev_shares,
beta_d_ev_shares: fpre_eval.beta_d_ev_shares,
correlated_d_ev_shares: fpre_eval.correlated_d_ev_shares,
gamma_d_ev_shares: fpre_eval.gamma_d_ev_shares,
```

**`evaluate_first_half` / `evaluate_second_half` method pattern** (`auth_tensor_eval.rs:151–158`) — `_p2` variants pass wide ciphertexts:
```rust
// P1 pattern (lines 151–153):
pub fn evaluate_first_half(&mut self, chunk_levels: Vec<Vec<(Block, Block)>>, chunk_cts: Vec<Vec<Block>>) {
    let (x, y) = self.get_first_inputs();
    self.eval_chunked_half_outer_product(&x.as_view(), &y.as_view(), chunk_levels, chunk_cts, true);
}

// P2 variant — wide chunk_cts, calls _wide chunked helper:
pub fn evaluate_first_half_p2(&mut self,
    chunk_levels: Vec<Vec<(Block, Block)>>,
    chunk_cts: Vec<Vec<(Block, Block)>>,    // wide ciphertexts
)
```

**`evaluate_final_p2` return type** (D-11, symmetric to D-10):
```rust
// P1 evaluate_final (lines 163–172): mutates first_half_out in place, returns ()
pub fn evaluate_final(&mut self) { ... }

// P2 variant: also computes D_ev output shares; returns them (D-11):
pub fn evaluate_final_p2(&mut self) -> Vec<Block>
//                                     [v_gamma D_ev]^ev, length n*m
// D_ev assembly uses correlated_d_ev_shares.mac on evaluator side (parallel to evaluate_final using
// correlated_auth_bit_shares.mac — lines 166–169).
```

**`evaluate_final` core assembly** (`auth_tensor_eval.rs:163–172`) — D_ev path copy:
```rust
// P1 evaluate_final (lines 163–172):
pub fn evaluate_final(&mut self) {
    for i in 0..self.n {
        for j in 0..self.m {
            self.first_half_out[(i, j)] ^=
                self.second_half_out[(j, i)] ^
                self.correlated_auth_bit_shares[j * self.n + i].mac.as_block();
        }
    }
    self.final_computed = true;
}
// D_ev path in evaluate_final_p2: same loop, substitute correlated_d_ev_shares.mac
```

**`compute_lambda_gamma` on evaluator** (`auth_tensor_eval.rs:199–228`) — P2 equivalent uses D_ev shares instead of D_gb:
```rust
// P1 evaluator lambda assembly (lines 218–226):
for j in 0..self.m {
    for i in 0..self.n {
        let idx = j * self.n + i;
        let v_extbit  = self.first_half_out[(i, j)].lsb();
        let lg_extbit = self.gamma_auth_bit_shares[idx].bit();
        out.push(lambda_gb[idx] ^ v_extbit ^ lg_extbit);
    }
}
```
For P2, the evaluator reconstructs `L_gamma` from the D_ev output wire shares returned by `evaluate_final_p2` directly (no garbler-emitted lambda needed — garbler keeps D_ev shares private).

---

### `src/lib.rs` — `test_auth_tensor_product_full_protocol_2` (P2-05)

**Analog:** `src/lib.rs:507–574` (`test_auth_tensor_product_full_protocol_1`) — copy structure exactly, substitute P2 path and delta.

**Test scaffolding pattern** (`lib.rs:523–536`) — identical setup:
```rust
let n = 4;
let m = 3;
let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, 1);
let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
```

**P1 garble/eval sequence** (`lib.rs:531–536`) — P2 variant calls `_p2` methods:
```rust
// P1 sequence (lines 531–536):
let (cl1, ct1) = gb.garble_first_half();
ev.evaluate_first_half(cl1, ct1);
let (cl2, ct2) = gb.garble_second_half();
ev.evaluate_second_half(cl2, ct2);
gb.garble_final();
ev.evaluate_final();

// P2 sequence — wide ciphertexts, p2-suffixed methods:
let (cl1, ct1) = gb.garble_first_half_p2();
ev.evaluate_first_half_p2(cl1, ct1);
let (cl2, ct2) = gb.garble_second_half_p2();
ev.evaluate_second_half_p2(cl2, ct2);
let (gb_d_gb_out, gb_d_ev_out) = gb.garble_final_p2();
let ev_d_ev_out = ev.evaluate_final_p2();
```

**Input wire D_ev initialization** (CONTEXT specifics, `6_total.tex` step 5) — done before the garble/eval sequence:
```rust
// Per CONTEXT.md "Claude's Discretion" + 6_total.tex step 5:
// Garbler side: [v_w D_ev]^gb := [l_w D_ev]^gb (no change — copy from preprocessing)
// Evaluator side: [v_w D_ev]^ev := [l_w D_ev]^ev XOR L_w * D_ev
// For IdealPreprocessingBackend with input=0: L_w = 0 ^ l_w, so [v_w D_ev]^ev XOR l_w * D_ev
// This is set directly on the input wire D_ev share vecs before calling the _p2 garble methods.
```

**P2 c_gamma assembly** (D-13) — NEW helper `assemble_c_gamma_shares_p2`, distinct from P1's `assemble_c_gamma_shares`:
```rust
// P2 formula (D-13, 2 terms only — do NOT copy P1's 6-term formula):
// [L_gamma D_ev] := [v_gamma D_ev] XOR [l_gamma D_ev]
// [c_gamma]^gb  := [L_gamma D_ev]^gb  (garbler side — gb_d_ev_out XOR gamma_d_ev_shares.key-encoding)
// [c_gamma]^ev  := [L_gamma D_ev]^ev XOR L_gamma * D_ev  (evaluator corrects with known L_gamma * delta_b)
// Combined key: XOR of gen-side keys from gamma_d_ev_shares (same key-accumulation as P1 but single field)
// MAC: combined_key.auth(c_gamma_bit, &ev.delta_b)   ← delta_b NOT delta_a
```

**check_zero call with D_ev delta** (`lib.rs:570–573` shows P1 pattern with `gb.delta_a`):
```rust
// P1 (line 570–572):
assert!(
    check_zero(&c_gamma_shares, &gb.delta_a),
    "P1-04: honest Protocol 1 run must pass check_zero"
);

// P2 — substitute delta_b (D_ev is authenticated under delta_b, the evaluator's delta):
assert!(
    check_zero(&c_gamma_shares_p2, &ev.delta_b),
    "P2-04: honest Protocol 2 run must pass check_zero under D_ev"
);
```

**verify_cross_party / output correctness pattern** (`auth_tensor_eval.rs tests:244–250` — `run_full_garble_eval` helper) — P2 test can use a similar local helper to run the full _p2 sequence:
```rust
fn run_full_garble_eval_p2(gb: &mut AuthTensorGen, ev: &mut AuthTensorEval)
    -> ((Vec<Block>, Vec<Block>), Vec<Block>) {
    let (cl1, ct1) = gb.garble_first_half_p2();
    ev.evaluate_first_half_p2(cl1, ct1);
    let (cl2, ct2) = gb.garble_second_half_p2();
    ev.evaluate_second_half_p2(cl2, ct2);
    let (gb_d_gb_out, gb_d_ev_out) = gb.garble_final_p2();
    let ev_d_ev_out = ev.evaluate_final_p2();
    ((gb_d_gb_out, gb_d_ev_out), ev_d_ev_out)
}
```

---

## Shared Patterns

### IT-MAC Share Generation (`gen_auth_bit`)
**Source:** `src/auth_tensor_fpre.rs:66–86`
**Apply to:** `IdealPreprocessingBackend::run` for all three new D_ev fields
```rust
// gen_auth_bit (lines 66–86) — used identically for alpha/beta/correlated D_ev fields:
pub fn gen_auth_bit(&mut self, x: bool) -> AuthBit {
    let a = self.rng.random_bool(0.5);
    let b = x ^ a;
    let a_share = build_share(&mut self.rng, a, &self.delta_b);
    let b_share = build_share(&mut self.rng, b, &self.delta_a);
    AuthBit {
        gen_share: AuthBitShare { key: b_share.key, mac: a_share.mac, value: a },
        eval_share: AuthBitShare { key: a_share.key, mac: b_share.mac, value: b },
    }
}
```

### check_zero MAC assembly
**Source:** `src/lib.rs:266–378` (`assemble_c_gamma_shares`)
**Apply to:** `assemble_c_gamma_shares_p2` in P2-05 test
```rust
// Key assembly pattern (lines 365–374):
let combined_mac = combined_key.auth(c_gamma_bit, &gb.delta_a);
let share = AuthBitShare { key: combined_key, mac: combined_mac, value: c_gamma_bit };
// P2 variant: substitute &ev.delta_b for &gb.delta_a
```

### Column-major indexing
**Source:** All field access across `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `lib.rs`
**Apply to:** All new D_ev field accesses
```rust
// Column-major index (j * n + i) used everywhere (e.g., auth_tensor_gen.rs:191):
let idx = j * self.n + i;
// All new D_ev fields use the same column-major indexing for the n*m entries.
```

### Even/odd GGM tweak convention
**Source:** `src/tensor_ops.rs:55–56`
**Apply to:** `gen_unary_outer_product_wide` and `eval_unary_outer_product_wide`
```rust
let tweak_even = Block::from(((i as u128) << 1) as u128);
let tweak_odd  = Block::from(((i as u128) << 1 | 1) as u128);
// In gen_unary_outer_product_wide: base = (seeds.len() * j + i) as u128
// kappa_half: tccr(Block::from(base << 1), seeds[i])
// rho_half:   tccr(Block::from(base << 1 | 1), seeds[i])
```

### `final_computed` guard
**Source:** `src/auth_tensor_gen.rs:34–36, 228–235` and `src/auth_tensor_eval.rs:28–30, 200–205`
**Apply to:** `garble_final_p2` and `evaluate_final_p2`
```rust
// The existing `final_computed: bool` flag guards compute_lambda_gamma.
// P2 also needs this guard — set self.final_computed = true; at the end of
// garble_final_p2 / evaluate_final_p2, same as the P1 finals do.
self.final_computed = true;
```

---

## No Analog Found

None. All files are modifications of existing files with close P1 analogs in the same file.

---

## Metadata

**Analog search scope:** `src/` directory — `tensor_ops.rs`, `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `preprocessing.rs`, `online.rs`, `lib.rs`, `auth_tensor_fpre.rs`, `sharing.rs`
**Files scanned:** 8
**Pattern extraction date:** 2026-04-24
