# Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) - Pattern Map

**Mapped:** 2026-04-21
**Files analyzed:** 6 (1 new, 5 modified)
**Analogs found:** 6 / 6 (all exact or strong role-match)

---

## File Classification

| New/Modified File | Status | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|--------|------|-----------|----------------|---------------|
| `src/feq.rs` | NEW | module (ideal functionality; equality-check oracle) | request-response (transform, panic-on-abort) | `src/bcot.rs` (ideal F_bCOT module) | role-match (ideal-functionality module pattern; different primitive) |
| `src/leaky_tensor_pre.rs` | REWRITTEN | module (`LeakyTriple` struct + `LeakyTensorPre::generate` orchestrator) | event-driven (5-step protocol transcript) | self (existing file) + `src/tensor_macro.rs` tests (for the macro-call idiom) | exact (same file; body replacement) |
| `src/delta.rs` | MODIFIED | type (newtype wrapper over `Block`) | config / constructor | self | exact (add `new_with_lsb` constructor + `random_b` for lsb=0) |
| `src/bcot.rs` | MODIFIED | module (ideal F_bCOT constructor) | config | self | exact (one-line change in `IdealBCot::new` to use `Delta::random_b` for B) |
| `src/lib.rs` | MODIFIED | module-graph root | config | self (existing `pub mod` declarations) | exact (one added line: `pub mod feq;`) |
| `src/preprocessing.rs` | MODIFIED | caller (`run_preprocessing`) | request-response | self | exact (call-site update `ltp.generate(0, 0)` → `ltp.generate()`) |
| `src/auth_tensor_pre.rs` | MODIFIED | consumer (`combine_leaky_triples`) | transform | self | exact (field-reference rename only — logic rewrite is Phase 5) |

---

## Shared Patterns

### Cross-party `AuthBitShare` layout (canonical codebase convention)
**Source:** `src/leaky_tensor_pre.rs:60-67` (doc-comment), `src/leaky_tensor_pre.rs:87-101` (concrete assembly)
**Apply to:** Every bCOT batch pair in `LeakyTensorPre::generate` (x_A, x_B pooled into one pair; y_A, y_B pooled into one pair; R_A, R_B pooled into one pair — 5 pairs total per D-11)

```rust
// Layout invariant:
//   gen_share.key  = cot_a_to_b.sender_keys[i]   (A's sender key from A->B COT, LSB=0)
//   gen_share.mac  = cot_b_to_a.receiver_macs[i] (A's MAC on A's bit under delta_b)
//   eval_share.key = cot_b_to_a.sender_keys[i]   (B's sender key from B->A COT, LSB=0)
//   eval_share.mac = cot_a_to_b.receiver_macs[i] (B's MAC on B's bit under delta_a)
let gen_shares: Vec<AuthBitShare> = (0..len)
    .map(|i| AuthBitShare {
        key: cot_a_to_b.sender_keys[i],
        mac: Mac::new(*cot_b_to_a.receiver_macs[i].as_block()),
        value: gen_portions[i],
    })
    .collect();
let eval_shares: Vec<AuthBitShare> = (0..len)
    .map(|i| AuthBitShare {
        key: cot_b_to_a.sender_keys[i],
        mac: Mac::new(*cot_a_to_b.receiver_macs[i].as_block()),
        value: eval_portions[i],
    })
    .collect();
```

### Cross-party MAC verification (test-only helper)
**Source:** `src/leaky_tensor_pre.rs:275-287` (identical at `src/auth_tensor_pre.rs:134-152`, `src/preprocessing.rs:133-136`)
**Apply to:** All new `test_*_mac_invariants` tests in the new test module (TEST-02)

```rust
fn verify_cross_party(
    pa_share: &AuthBitShare, pb_share: &AuthBitShare,
    delta_a: &Delta, delta_b: &Delta,
) {
    AuthBitShare { key: pb_share.key, mac: pa_share.mac, value: pa_share.value }
        .verify(delta_b);
    AuthBitShare { key: pa_share.key, mac: pb_share.mac, value: pb_share.value }
        .verify(delta_a);
}
```

Never call `share.verify(&delta)` directly on a cross-party share — it panics with `"MAC mismatch in share"` (see `src/sharing.rs:60-63`).

### Inline test module skeleton
**Source:** `src/bcot.rs:129-256`, `src/leaky_tensor_pre.rs:252-427`
**Apply to:** Both `src/feq.rs` and the rewritten `src/leaky_tensor_pre.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ...

    #[test]
    fn test_xxx() { ... }

    #[test]
    #[should_panic(expected = "F_eq abort")]
    fn test_xxx_panics() { ... }
}
```

### Protocol violation = `panic!` (NO Result/Error)
**Source:** `src/sharing.rs:62` (`assert_eq!(self.mac, want, "MAC mismatch in share")`), `src/bcot.rs:48` (dimensionality assertions), `src/auth_tensor_pre.rs:46,56-68` (delta-equality assertions)
**Apply to:** `feq::check` (panic on L_1 != L_2, per D-04); any dimension assertions inside `generate()`.

Project-wide convention (`.planning/codebase/CONVENTIONS.md` lines 105-111, summarized in RESEARCH.md): use `assert!` / `assert_eq!` / `panic!` for protocol violations. No `Result<()>` return types.

### Deterministic seeded `ChaCha12Rng`
**Source:** `src/bcot.rs:48-53`, `src/leaky_tensor_pre.rs:51-57`, `src/sharing.rs:181-182`
**Apply to:** RNG for x_A, x_B, y_A, y_B, R_A, R_B bit sampling inside `generate()` — use `self.rng` (the existing `ChaCha12Rng` field on `LeakyTensorPre`).

```rust
// In LeakyTensorPre::generate
let x_a_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
```

Never `rand::rng()` / `thread_rng()` in protocol code or tests.

---

## Pattern Assignments

### `src/feq.rs` — NEW (ideal F_eq module)

**Primary analog:** `src/bcot.rs` (ideal F_bCOT module — same "module exposes one ideal functionality" shape)
**Secondary analog (for module doc-comment):** `src/bcot.rs:11-20`
**Secondary analog (for inline tests):** `src/leaky_tensor_pre.rs:252-427`

#### Imports pattern — copy from `src/bcot.rs:1-9`, `src/leaky_tensor_pre.rs:1-9`

Adapt to the F_eq surface (only `BlockMatrix` needed):

```rust
use crate::matrix::BlockMatrix;
```

#### Module-level doc-comment — copy pattern from `src/bcot.rs:11-20`

```rust
//! Ideal F_eq functionality: element-wise BlockMatrix equality check.
//!
//! In the real protocol, parties send L_1 and L_2 to F_eq, which compares them
//! and returns 0 (abort) if they differ, 1 (continue) otherwise. This in-process
//! ideal version panics on mismatch, matching the protocol's abort semantics.
//!
//! TODO: Replace with a real equality-check protocol (e.g., commit-and-open hash)
//!       for production.
```

(Mirrors the `bcot.rs` top-of-file doc, including the `TODO: Replace with a real ...` line at `src/bcot.rs:20`.)

#### Core pattern — element-wise comparison with panic on mismatch (D-04/D-05)

Model the assertion pattern on `src/sharing.rs:60-63` (`assert_eq!(self.mac, want, "MAC mismatch in share")`) and the dimension-assertion pattern on `src/tensor_macro.rs:89-92` (`assert!(n > 0, ...); assert_eq!(a_keys.len(), n, ...); assert_eq!(t_gen.rows(), m, ...)`):

```rust
pub fn check(l1: &BlockMatrix, l2: &BlockMatrix) {
    assert_eq!(l1.rows(), l2.rows(), "F_eq: row dimension mismatch");
    assert_eq!(l1.cols(), l2.cols(), "F_eq: column dimension mismatch");
    for j in 0..l1.cols() {
        for i in 0..l1.rows() {
            if l1[(i, j)] != l2[(i, j)] {
                panic!("F_eq abort: consistency check failed — L_1 != L_2 at ({}, {})", i, j);
            }
        }
    }
}
```

**Why this shape:** `BlockMatrix::Index<(usize, usize)>` is already defined at `src/matrix.rs:222-228` with column-major storage. The nested `for j in 0..cols { for i in 0..rows }` order matches every consumer in the codebase (`src/matrix.rs:252-258`, `src/matrix.rs:277-283`, `src/leaky_tensor_pre.rs:165-168`).

#### Test pattern — copy structure from `src/bcot.rs:129-255` and `src/leaky_tensor_pre.rs:252-427`

Three tests per D-04/TEST-04:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    #[test]
    fn test_check_equal_matrices_passes() {
        let mut a = BlockMatrix::new(3, 4);
        let mut b = BlockMatrix::new(3, 4);
        for j in 0..4 { for i in 0..3 {
            let v = Block::new([(i as u8) ^ (j as u8); 16]);
            a[(i, j)] = v;
            b[(i, j)] = v;
        }}
        check(&a, &b);  // must not panic
    }

    #[test]
    #[should_panic(expected = "F_eq abort")]
    fn test_check_differing_matrices_panics() {
        let a = BlockMatrix::new(2, 2);
        let mut b = BlockMatrix::new(2, 2);
        b[(0, 0)] = Block::new([1; 16]);
        check(&a, &b);
    }

    #[test]
    #[should_panic(expected = "F_eq: row dimension mismatch")]
    fn test_check_dimension_mismatch_panics() {
        let a = BlockMatrix::new(2, 2);
        let b = BlockMatrix::new(3, 2);
        check(&a, &b);
    }
}
```

The `#[should_panic(expected = "...")]` pattern with a substring match is exactly what `src/tensor_macro.rs` tests use (referenced in phase 3 PATTERNS.md) and is the codebase's standard for abort-path verification.

---

### `src/leaky_tensor_pre.rs` — REWRITTEN (LeakyTriple struct + generate orchestrator)

**Primary analog:** self (existing file — preserve module shape, rewrite body)
**Secondary analog for macro call orchestration:** `src/tensor_macro.rs:201-254` (the `run_one_case` test fixture demonstrates the full garbler→evaluator call pattern)
**Secondary analog for BlockMatrix column-vector construction:** `src/tensor_macro.rs:219-225`
**Secondary analog for BlockMatrix XOR assembly of S_1/S_2:** `src/matrix.rs:263-286` (`&TypedMatrix ^ &TypedMatrix`)

#### Imports pattern — copy from existing `src/leaky_tensor_pre.rs:1-9` and extend

```rust
use crate::{
    bcot::IdealBCot,
    block::Block,
    delta::Delta,
    keys::Key,            // NEW: needed for Vec<Key> passing to tensor_garbler
    macs::Mac,
    matrix::BlockMatrix,  // NEW: needed for wrapping C_A/C_B and S_1/S_2
    sharing::AuthBitShare,
    tensor_macro::{tensor_garbler, tensor_evaluator},  // NEW: Phase 3 primitive
    feq,                  // NEW: ideal F_eq module (D-03)
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
```

Note: `tensor_garbler` and `tensor_evaluator` are `pub(crate)` (`src/tensor_macro.rs:82,132`), which is fine since `leaky_tensor_pre` is in the same crate.

#### `LeakyTriple` struct pattern — adapt existing shape

The existing struct is at `src/leaky_tensor_pre.rs:13-35`. After D-06 / D-07 / D-08 / D-09:

```rust
/// One leaky tensor triple (output of a single Pi_LeakyTensor execution).
/// Both gen and eval views are stored together for in-process use.
pub struct LeakyTriple {
    pub n: usize,
    pub m: usize,
    // Garbler A's view
    pub gen_x_shares: Vec<AuthBitShare>,    // was gen_alpha_shares; length n
    pub gen_y_shares: Vec<AuthBitShare>,    // was gen_beta_shares; length m
    /// length n*m, column-major: index = j*n+i (j = y index, i = x index)
    pub gen_z_shares: Vec<AuthBitShare>,    // was gen_correlated_shares; length n*m
    // Evaluator B's view
    pub eval_x_shares: Vec<AuthBitShare>,
    pub eval_y_shares: Vec<AuthBitShare>,
    pub eval_z_shares: Vec<AuthBitShare>,
    // The deltas (shared across all triples produced by one run_preprocessing call)
    pub delta_a: Delta,
    pub delta_b: Delta,
}
```

Removed (per D-07): `gen_gamma_shares`, `eval_gamma_shares`, `gen_alpha_labels`, `eval_alpha_labels`, `gen_beta_labels`, `eval_beta_labels`.

#### `LeakyTensorPre<'a>` struct — UNCHANGED per D-02

Preserve verbatim from `src/leaky_tensor_pre.rs:43-58`:

```rust
pub struct LeakyTensorPre<'a> {
    pub n: usize,
    pub m: usize,
    bcot: &'a mut IdealBCot,
    rng: ChaCha12Rng,
}

impl<'a> LeakyTensorPre<'a> {
    pub fn new(seed: u64, n: usize, m: usize, bcot: &'a mut IdealBCot) -> Self {
        Self { n, m, bcot, rng: ChaCha12Rng::seed_from_u64(seed) }
    }
    // ... generate rewritten below
}
```

#### Step 1 — bCOT batch pair assembly (5×) — copy exactly from `src/leaky_tensor_pre.rs:73-101`, 103-129, 162-196

The existing code has this pattern 4× (alpha, beta, correlated, gamma). Phase 4 needs it 3× (x, y, R) — per D-01, sample BOTH party shares independently instead of deriving one from the other (anti-pattern per RESEARCH.md).

Example for x (lengths n):

```rust
// Step 1a: x shares (length n, two independent bit vectors per D-01)
let x_a_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();
let x_b_bits: Vec<bool> = (0..self.n).map(|_| self.rng.random_bool(0.5)).collect();

let cot_x_a_to_b = self.bcot.transfer_a_to_b(&x_b_bits);  // A sender, B receiver
let cot_x_b_to_a = self.bcot.transfer_b_to_a(&x_a_bits);  // B sender, A receiver

let gen_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_a_to_b.sender_keys[i],
    mac: Mac::new(*cot_x_b_to_a.receiver_macs[i].as_block()),
    value: x_a_bits[i],
}).collect();
let eval_x_shares: Vec<AuthBitShare> = (0..self.n).map(|i| AuthBitShare {
    key: cot_x_b_to_a.sender_keys[i],
    mac: Mac::new(*cot_x_a_to_b.receiver_macs[i].as_block()),
    value: x_b_bits[i],
}).collect();
```

Repeat for y (length m) and R (length n*m). R is **local to `generate()`** per D-12 — `gen_r_shares`, `eval_r_shares` never land on `LeakyTriple`.

**Anti-pattern to avoid (existing `src/leaky_tensor_pre.rs:73-80`):** do NOT sample a single `alpha_bits` vector and derive `eval_alpha_portions = gen_portion ^ alpha`. Per D-01 and RESEARCH.md "Anti-Patterns to Avoid", sample both independent uniform bit vectors directly.

#### Step 2 — C_A / C_B / C_A^(R) / C_B^(R) inline XOR (D-10) — new code, but Block XOR pattern copied from `src/tensor_macro.rs:240-247`, `src/leaky_tensor_pre.rs:137-144`

```rust
let delta_a_block = *self.bcot.delta_a.as_block();
let delta_b_block = *self.bcot.delta_b.as_block();

// C_A, C_B: length m (one Block per macro T-column)
let mut c_a: Vec<Block> = Vec::with_capacity(self.m);
let mut c_b: Vec<Block> = Vec::with_capacity(self.m);
for j in 0..self.m {
    let y_a_term = if gen_y_shares[j].value  { delta_a_block } else { Block::ZERO };
    let y_b_term = if eval_y_shares[j].value { delta_b_block } else { Block::ZERO };
    c_a.push(y_a_term
        ^ *gen_y_shares[j].key.as_block()   // key(y_B @ A)
        ^ *gen_y_shares[j].mac.as_block()); // mac(y_A @ B)
    c_b.push(y_b_term
        ^ *eval_y_shares[j].mac.as_block()  // mac(y_B @ A)
        ^ *eval_y_shares[j].key.as_block()); // key(y_A @ B)
}

// C_A^(R), C_B^(R): length n*m (column-major, k = j*n + i)
let mut c_a_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
let mut c_b_r: Vec<Block> = Vec::with_capacity(self.n * self.m);
for k in 0..(self.n * self.m) {
    let r_a_term = if gen_r_shares[k].value  { delta_a_block } else { Block::ZERO };
    let r_b_term = if eval_r_shares[k].value { delta_b_block } else { Block::ZERO };
    c_a_r.push(r_a_term
        ^ *gen_r_shares[k].key.as_block()
        ^ *gen_r_shares[k].mac.as_block());
    c_b_r.push(r_b_term
        ^ *eval_r_shares[k].mac.as_block()
        ^ *eval_r_shares[k].key.as_block());
}
```

Field selection is per RESEARCH.md Pitfall 3 (the `gen.key` = `key(y_B @ A)` / `gen.mac` = `mac(y_A @ B)` mapping derived from the cross-party layout doc-comment).

#### Step 3 — BlockMatrix wrapping + tensor_macro calls — copy from `src/tensor_macro.rs:219-229`

Tensor-macro preconditions: `t_gen`/`t_eval` MUST be `BlockMatrix(m, 1)` (column vector) per `src/tensor_macro.rs:91-92`:

```rust
let mut t_a = BlockMatrix::new(self.m, 1);
let mut t_b = BlockMatrix::new(self.m, 1);
for j in 0..self.m {
    t_a[j] = c_a[j];  // column-vector indexing [k] — see src/matrix.rs:206-220
    t_b[j] = c_b[j];
}

// Macro Call 1: A is garbler, B is evaluator
let (z_gb1, g_1) = tensor_garbler(
    self.n, self.m, self.bcot.delta_a,
    &cot_x_a_to_b.sender_keys,    // Vec<Key> — A's keys for x_B @ A
    &t_a,
);
let e_1 = tensor_evaluator(
    self.n, self.m, &g_1,
    &cot_x_a_to_b.receiver_macs,  // Vec<Mac> — B's MACs for x_B, under Δ_A
    &t_b,
);

// Macro Call 2: B is garbler, A is evaluator
let (z_gb2, g_2) = tensor_garbler(
    self.n, self.m, self.bcot.delta_b,
    &cot_x_b_to_a.sender_keys,    // Vec<Key> — B's keys for x_A @ B
    &t_b,
);
let e_2 = tensor_evaluator(
    self.n, self.m, &g_2,
    &cot_x_b_to_a.receiver_macs,  // Vec<Mac> — A's MACs for x_A, under Δ_B
    &t_a,
);
```

**Note:** the macro takes `&[Key]` / `&[Mac]` (newtype slices), not `&[Block]`. The bCOT output `sender_keys: Vec<Key>` and `receiver_macs: Vec<Mac>` are already the correct types — pass them directly, no collection/conversion. (See `src/bcot.rs:33,38` for the output field types, and `src/tensor_macro.rs:215-217` for a verbatim example.)

#### Step 4 — Masked reveal (S_1, S_2, D) — `BlockMatrix` XOR via `^` operator (`src/matrix.rs:263-286`)

S_1 / S_2 are n×m `BlockMatrix`. We need C_A^(R) / C_B^(R) as n×m `BlockMatrix` too so the `^` operator works element-wise:

```rust
// Wrap C_A^(R), C_B^(R) as BlockMatrix(n, m). k = j*n + i in column-major.
let mut c_a_r_mat = BlockMatrix::new(self.n, self.m);
let mut c_b_r_mat = BlockMatrix::new(self.n, self.m);
for j in 0..self.m {
    for i in 0..self.n {
        let k = j * self.n + i;
        c_a_r_mat[(i, j)] = c_a_r[k];
        c_b_r_mat[(i, j)] = c_b_r[k];
    }
}

// S_1 := Z_gb1 ⊕ E_2 ⊕ C_A^(R)   (D-16)
// S_2 := Z_gb2 ⊕ E_1 ⊕ C_B^(R)
let s_1 = &(&z_gb1 ^ &e_2) ^ &c_a_r_mat;
let s_2 = &(&z_gb2 ^ &e_1) ^ &c_b_r_mat;

// D = lsb(S_1) ⊕ lsb(S_2), column-major Vec<bool> of length n*m
// (pattern mirrors existing column-major accumulation in src/leaky_tensor_pre.rs:164-168)
let mut d_bits: Vec<bool> = Vec::with_capacity(self.n * self.m);
for j in 0..self.m {
    for i in 0..self.n {
        d_bits.push(s_1[(i, j)].lsb() ^ s_2[(i, j)].lsb());
    }
}
```

`Block::lsb()` is at `src/block.rs:96-99`. `BlockMatrix ^ BlockMatrix` borrowed form is at `src/matrix.rs:263-286`.

**Correctness precondition (RESEARCH.md Pitfall 1):** This step produces `D = x ⊗ y ⊕ R` **only if** `lsb(Δ_A ⊕ Δ_B) == 1`. The current codebase has `lsb(Δ_A) == 1` AND `lsb(Δ_B) == 1` ⇒ XOR lsb = 0 ⇒ D is garbage. See the `src/delta.rs` / `src/bcot.rs` patterns below (must ship in Wave 0, before this step runs).

#### Step 5 — L_1, L_2 construction + F_eq + Z assembly — inline

L_1, L_2 follow the same element-wise `BlockMatrix` pattern:

```rust
let mut l_1 = BlockMatrix::new(self.n, self.m);
let mut l_2 = BlockMatrix::new(self.n, self.m);
for j in 0..self.m {
    for i in 0..self.n {
        let k = j * self.n + i;
        let d_term_a = if d_bits[k] { delta_a_block } else { Block::ZERO };
        let d_term_b = if d_bits[k] { delta_b_block } else { Block::ZERO };
        l_1[(i, j)] = s_1[(i, j)] ^ d_term_a;
        l_2[(i, j)] = s_2[(i, j)] ^ d_term_b;
    }
}
feq::check(&l_1, &l_2);   // panics on L_1 != L_2 per D-04
```

For `itmac{Z}{Δ} = itmac{R}{Δ} ⊕ itmac{D}{Δ}` (D-17): D is public, so the local share of D is constructed inline and combined with R via the existing `AuthBitShare + AuthBitShare` impl at `src/sharing.rs:66-77`:

```rust
let gen_z_shares: Vec<AuthBitShare> = (0..self.n * self.m).map(|k| {
    let d = d_bits[k];
    // Construction convention for public-bit share (RESEARCH.md Q1 — confirm during planning):
    //   gen_d_share = { value: d, key: ZERO, mac: ZERO }
    //   eval_d_share = { value: false, key: ZERO, mac: if d { Δ_B-block } else { ZERO } }
    // XOR-combine via Add impl at src/sharing.rs:66-77.
    let gen_d = AuthBitShare { key: Key::default(), mac: Mac::default(), value: d };
    gen_r_shares[k] + gen_d
}).collect();
let eval_z_shares: Vec<AuthBitShare> = (0..self.n * self.m).map(|k| {
    let d = d_bits[k];
    let mac_block = if d { delta_b_block } else { Block::ZERO };
    let eval_d = AuthBitShare {
        key: Key::default(),
        mac: Mac::new(mac_block),
        value: false,
    };
    eval_r_shares[k] + eval_d
}).collect();
```

**HIGH-risk convention (RESEARCH.md Q1 / A1):** the exact `gen_d_share` / `eval_d_share` assignment must be validated by paper derivation before implementation. The planner's first task should be a worked 2×2 example verifying `verify_cross_party` passes on the resulting `gen_z_shares[k], eval_z_shares[k]`.

`Key::default()` and `Mac::default()` exist (`Default` derive at `src/keys.rs` and `src/macs.rs:17` — `Mac` has `#[derive(... Default ...)]`; `Key` analogous via newtype). Same for `AuthBitShare::default()` at `src/sharing.rs:42`.

#### Final assembly — match existing `src/leaky_tensor_pre.rs:230-248` shape

```rust
LeakyTriple {
    n: self.n,
    m: self.m,
    delta_a: self.bcot.delta_a,
    delta_b: self.bcot.delta_b,
    gen_x_shares,
    eval_x_shares,
    gen_y_shares,
    eval_y_shares,
    gen_z_shares,
    eval_z_shares,
}
```

#### Test module — delete 4 broken tests, add 9 new tests

Existing tests at `src/leaky_tensor_pre.rs:252-427`:
- **DELETE** (use removed fields or old signature): `test_alpha_beta_mac_invariants` (lines 301-321), `test_correlated_mac_invariants` (389-409), `test_alpha_label_sharing` (323-341), `test_alpha_beta_dimensions` (291-299), `test_correlated_bit_correctness` (358-387), `test_generate_dimensions_full` (411-419), `test_key_lsb_zero` (343-354), `test_large_n_m` (421-426).
- **PRESERVE** (helpers): `verify_cross_party` (275-287) — use verbatim.
- **ADD** (per RESEARCH.md test map):
  - `test_correlated_randomness_dimensions` — PROTO-04: lengths of x/y/z shares
  - `test_c_a_c_b_xor_invariant` — PROTO-05: `C_A[j] ⊕ C_B[j] == y_full[j]·(Δ_A ⊕ Δ_B)`
  - `test_macro_outputs_xor_invariant` — PROTO-06
  - `test_d_extraction_and_z_assembly` — PROTO-07
  - `test_feq_passes_on_honest_run` — PROTO-08 (no panic)
  - `test_leaky_triple_shape` — PROTO-09: struct field presence (compile-time)
  - `test_leaky_triple_mac_invariants` — TEST-02 (uses `verify_cross_party` on x, y, Z)
  - `test_leaky_triple_product_invariant` — TEST-03: `z_ij == x_i AND y_j`
  - `test_key_lsb_zero_all_shares` — preserves the existing LSB invariant check (adapted to new field names)

`make_bcot` helper (line 257-259) is preserved as-is.

---

### `src/delta.rs` — MODIFIED (add `new_with_lsb` + `random_b` constructors)

**Primary analog:** self (`src/delta.rs:9-34`)

#### Existing constructor — `src/delta.rs:9-34`

```rust
impl Delta {
    /// Creates a new Delta, setting the pointer bit to 1.
    #[inline]
    pub fn new(mut value: Block) -> Self {
        value.set_lsb(true);
        Self(value)
    }

    #[inline]
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        Self::new(Block::from(rng.random::<[u8; 16]>()))
    }
    // ...
}
```

#### Pattern to apply — add `new_with_lsb` and `random_b` matching the existing style

```rust
impl Delta {
    /// Creates a new Delta with an explicit pointer-bit value.
    ///
    /// Used when the two parties' deltas must satisfy `lsb(Δ_A ⊕ Δ_B) == 1`
    /// (paper §F requires this for Construction 2's masked reveal).
    #[inline]
    pub fn new_with_lsb(mut value: Block, lsb_value: bool) -> Self {
        value.set_lsb(lsb_value);
        Self(value)
    }

    /// Generate a random Party-B Delta (LSB cleared to 0).
    ///
    /// Pairs with `Delta::random` (LSB=1) so that `lsb(delta_a ⊕ delta_b) == 1`,
    /// which is required by Pi_LeakyTensor Construction 2's masked reveal.
    #[inline]
    pub fn random_b<R: Rng>(rng: &mut R) -> Self {
        Self::new_with_lsb(Block::from(rng.random::<[u8; 16]>()), false)
    }
}
```

Both fns mirror the existing `Delta::new` / `Delta::random` pair's `#[inline]` + doc-comment style.

---

### `src/bcot.rs` — MODIFIED (use `Delta::random_b` for `delta_b`)

**Primary analog:** self (`src/bcot.rs:48-55`)

#### Existing `IdealBCot::new` — `src/bcot.rs:48-55`

```rust
pub fn new(seed_a: u64, seed_b: u64) -> Self {
    let mut rng_a = ChaCha12Rng::seed_from_u64(seed_a);
    let mut rng_b = ChaCha12Rng::seed_from_u64(seed_b);
    let delta_a = Delta::random(&mut rng_a);
    let delta_b = Delta::random(&mut rng_b);   // ← BOTH have lsb=1 (bug source per Pitfall 1)
    let rng = ChaCha12Rng::seed_from_u64(seed_a ^ seed_b);
    Self { delta_a, delta_b, rng }
}
```

#### Pattern to apply — one-line swap

```rust
let delta_a = Delta::random(&mut rng_a);        // lsb = 1
let delta_b = Delta::random_b(&mut rng_b);      // lsb = 0
```

**Cross-codebase safety (RESEARCH.md A3):** Verified no test asserts `delta_b.lsb()` ⇒ this change does not break any existing test. The only LSB assertions are on `delta_a` (e.g., `src/preprocessing.rs:124` `assert!(gen_out.delta_a.as_block().lsb(), "delta_a LSB must be 1")`) — these still pass.

#### Add regression test — `src/bcot.rs` test module

```rust
#[test]
fn test_delta_xor_lsb_is_one() {
    let bcot = IdealBCot::new(42, 99);
    let xor_lsb = bcot.delta_a.as_block().lsb() ^ bcot.delta_b.as_block().lsb();
    assert!(xor_lsb, "Paper §F requires lsb(Δ_A ⊕ Δ_B) == 1");
    assert!(bcot.delta_a.as_block().lsb(), "Δ_A lsb must remain 1");
    assert!(!bcot.delta_b.as_block().lsb(), "Δ_B lsb must be 0");
}
```

Matches the test style at `src/bcot.rs:213-229` (`test_key_lsb_is_zero`).

---

### `src/lib.rs` — MODIFIED (add `pub mod feq;`)

**Primary analog:** self (`src/lib.rs:1-24`)

#### Existing pattern — `src/lib.rs:1-24`

```rust
pub mod block;
pub mod delta;
pub mod keys;
pub mod macs;
pub mod sharing;
// ...
pub mod bcot;
pub mod leaky_tensor_pre;
pub mod auth_tensor_pre;
pub mod preprocessing;
```

#### Pattern to apply — insert `pub mod feq;` alphabetically (or adjacent to `bcot` since both are ideal functionalities):

```rust
pub mod bcot;
pub mod feq;                    // NEW
pub mod leaky_tensor_pre;
```

(Both orderings are used in the existing file — grouping by role is preferred; `bcot` and `feq` are both in-process ideal-functionality modules.)

---

### `src/preprocessing.rs` — MODIFIED (update call site for no-arg `generate()`)

**Primary analog:** self (`src/preprocessing.rs:96-101`)

#### Existing call — `src/preprocessing.rs:99-100`

```rust
let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, &mut bcot);
triples.push(ltp.generate(0, 0));
```

#### Pattern to apply — drop both arguments per D-01

```rust
let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, &mut bcot);
triples.push(ltp.generate());
```

The surrounding comment at `src/preprocessing.rs:77-78` ("x_clear and y_clear are zero — preprocessing generates masks without specific input binding") becomes obsolete — delete or rephrase to "preprocessing is fully input-independent per Construction 2".

#### Tests in this file — `src/preprocessing.rs:113-154`

All 4 existing tests already use `run_preprocessing(4, 4, 1, 1)` (no leaky-specific args) — **no test-body changes needed** beyond the field-rename cascade flowing from `combine_leaky_triples`. `test_run_preprocessing_mac_invariants` at lines 129-146 may start passing (or failing differently) depending on Q1/Q2 resolution; leave it in place and re-verify after combine_leaky_triples is updated.

---

### `src/auth_tensor_pre.rs` — MODIFIED (field-rename cascade, Option A per RESEARCH.md Pitfall 8)

**Primary analog:** self (`src/auth_tensor_pre.rs:71-105`)

#### Existing field reads — `src/auth_tensor_pre.rs:71,72,77,78,90,91,92,93,94,101,102,103,104,105`

```rust
let mut combined_gen_corr = triples[0].gen_correlated_shares.clone();
let mut combined_eval_corr = triples[0].eval_correlated_shares.clone();
// ...
combined_gen_corr[k] = combined_gen_corr[k] + t.gen_correlated_shares[k];
// ...
alpha_labels: t0.gen_alpha_labels.clone(),
beta_labels: t0.gen_beta_labels.clone(),
alpha_auth_bit_shares: t0.gen_alpha_shares.clone(),
beta_auth_bit_shares: t0.gen_beta_shares.clone(),
correlated_auth_bit_shares: combined_gen_corr,
```

#### Pattern to apply — rename fields (Option A: keep build green; Phase 5 rewrites the semantics)

```rust
let mut combined_gen_z = triples[0].gen_z_shares.clone();
let mut combined_eval_z = triples[0].eval_z_shares.clone();
// ...
combined_gen_z[k] = combined_gen_z[k] + t.gen_z_shares[k];
// ...
alpha_labels: Vec::new(),  // STUB: removed in Phase 4; rewritten in Phase 5
beta_labels: Vec::new(),   // STUB
alpha_auth_bit_shares: t0.gen_x_shares.clone(),
beta_auth_bit_shares: t0.gen_y_shares.clone(),
correlated_auth_bit_shares: combined_gen_z,
```

(Mirror for the `TensorFpreEval` arm at lines 96-106.)

**Why Option A not Option B:** keeping the build green at every commit follows the codebase's established wave-0-through-wave-N gate pattern (see `.planning/phases/03-*/03-VERIFICATION.md`). Option B (`unimplemented!()`) would break every intermediate test run until Phase 5 ships.

#### Tests — `src/auth_tensor_pre.rs:110-213`

`make_triples` at lines 120-129 calls `ltp.generate(0b1010, 0b1100)` — update to `ltp.generate()` (drop the two args per D-01). All other test bodies reference `combined_*` through the already-renamed struct fields (`alpha_auth_bit_shares`, `correlated_auth_bit_shares`), which keep their names on `TensorFpreGen` / `TensorFpreEval` (those structs are NOT in scope for Phase 4 rename — only `LeakyTriple` is). So no further changes in this test module.

`test_combine_mac_invariants` at lines 174-200 may pass or fail depending on Q1 / Q2; treat as acceptance criterion, not a Wave 0 blocker.

---

## No Analog Found

None — every file being created/modified has a strong codebase analog. The only genuinely novel pattern is the F_eq `check` function, but its shape (element-wise matrix comparison with panic on mismatch) is a direct composition of two existing patterns: the dimension-assertion style from `src/tensor_macro.rs:89-92` and the equality-panic style from `src/sharing.rs:60-63`.

---

## Metadata

**Analog search scope:**
- `src/` (all 20+ source files)
- `.planning/phases/03-*/03-PATTERNS.md` (prior-phase pattern maps for style consistency)
- `.planning/codebase/CONVENTIONS.md` (indirectly via RESEARCH.md summary)

**Files scanned (full reads):**
- `src/leaky_tensor_pre.rs` (427 lines — the file under rewrite)
- `src/bcot.rs` (256 lines — F_bCOT ideal-functionality analog for F_eq)
- `src/sharing.rs` (208 lines — AuthBitShare type + Add impls + verify)
- `src/delta.rs` (138 lines — Delta constructors, LSB invariant)
- `src/preprocessing.rs` (155 lines — the call-site consumer)
- `src/auth_tensor_pre.rs` (213 lines — the struct-field consumer)
- `src/lib.rs` (42 lines — module graph)
- `src/macs.rs` (100 lines — Mac type + as_blocks)

**Files scanned (targeted reads):**
- `src/tensor_macro.rs:1-260` (macro signatures + test fixture)
- `src/matrix.rs:1-180, 200-320` (BlockMatrix storage, Index, BitXor)
- `src/block.rs:1-120` (Block::ZERO, Block::lsb, Block::set_lsb)
- `src/keys.rs` grep (Key::new, Key::as_blocks, Key::auth, From<Block>)

**Pattern extraction date:** 2026-04-21
