# Phase 8: Open() + Protocol 1 Garble/Eval/Check - Pattern Map

**Mapped:** 2026-04-23
**Files analyzed:** 4 (1 new, 3 modified)
**Analogs found:** 4 / 4

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/auth_tensor_gen.rs` (modify) | online-protocol struct | request-response (struct method, in-memory transform) | self (existing `garble_final` + `new_from_fpre_gen`) | exact (in-file extension) |
| `src/auth_tensor_eval.rs` (modify) | online-protocol struct | request-response (struct method takes `&[bool]`, returns `Vec<bool>`) | `src/auth_tensor_gen.rs` (mirror) + self (`evaluate_final`) | exact (symmetric mirror) |
| `src/online.rs` (NEW) | primitive utility / protocol check | request-response (pure function over slice + delta → bool) | `src/feq.rs` (panic-on-abort variant) + `src/sharing.rs::AuthBitShare::verify` (MAC invariant) | role-match (same "consistency check" role; differs in returning `bool` vs panicking) |
| `src/lib.rs` (modify) | crate root + integration tests | module-decl + integration-test harness | self (existing `pub mod preprocessing;` line + `test_auth_tensor_product`) | exact (in-file extension) |

---

## Pattern Assignments

### `src/auth_tensor_gen.rs` (modify — add `gamma_auth_bit_shares` field + `compute_lambda_gamma()`)

**Analog:** `src/auth_tensor_gen.rs` itself (existing structure — Phase 7 stub at line 64; column-major loop at lines 179-193).

**Field-declaration pattern** (`src/auth_tensor_gen.rs:26-28` — mirror line 28 with `gamma_auth_bit_shares`):
```rust
pub alpha_auth_bit_shares: Vec<AuthBitShare>,
pub beta_auth_bit_shares: Vec<AuthBitShare>,
pub correlated_auth_bit_shares: Vec<AuthBitShare>,
// ADD: pub gamma_auth_bit_shares: Vec<AuthBitShare>,
```

**`new()` empty-init pattern** (`src/auth_tensor_gen.rs:44-46` — add a `Vec::new()` line for the new field):
```rust
alpha_auth_bit_shares: Vec::new(),
beta_auth_bit_shares: Vec::new(),
correlated_auth_bit_shares: Vec::new(),
// ADD: gamma_auth_bit_shares: Vec::new(),
```

**`new_from_fpre_gen()` field-forwarding pattern** (`src/auth_tensor_gen.rs:61-64` — replace the TODO at line 64):
```rust
alpha_auth_bit_shares: fpre_gen.alpha_auth_bit_shares,
beta_auth_bit_shares: fpre_gen.beta_auth_bit_shares,
correlated_auth_bit_shares: fpre_gen.correlated_auth_bit_shares,
// TODO(Phase 8): forward fpre_gen.gamma_auth_bit_shares to a corresponding field on AuthTensorGen
// REPLACE WITH: gamma_auth_bit_shares: fpre_gen.gamma_auth_bit_shares,
```

**Column-major iteration pattern** for `compute_lambda_gamma()` (`src/auth_tensor_gen.rs:179-193` — `garble_final` loop):
```rust
pub fn garble_final(&mut self) {
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
}
```
Reuse exactly: `for i in 0..self.n { for j in 0..self.m { ... j*self.n + i indexing ... } }`. The new method uses `self.first_half_out[(i, j)].lsb()` (extbit) and `self.gamma_auth_bit_shares[j * self.n + i].bit()`, pushes the XOR into a `Vec<bool>` of length `n*m` (column-major to match all other n×m field vecs).

**extbit semantics:** `Block::lsb()` at `src/block.rs:97` returns `(self.0[0] & 1) == 1`. This is the canonical "extract pointer bit" — used elsewhere (e.g., `src/auth_tensor_eval.rs:94`).

**Bit semantics for `AuthBitShare::bit()`** (`src/sharing.rs:54-57`):
```rust
#[inline]
pub fn bit(&self) -> bool {
    self.value
}
```
Returns the per-party local share of the bit (NOT a re-derivation from MAC LSB). Independent of which delta authenticated the share — confirms that `gamma_auth_bit_shares[k].bit()` is the correct value to XOR for `extbit([l_gamma D_gb])` semantics (RESEARCH Pitfall 1 / Assumption A1).

**Test extension pattern** (`src/auth_tensor_gen.rs:196-231` — existing `mod tests`):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth_tensor_fpre::TensorFpre};

    #[test]
    fn test_garble_first_half() {
        let n = 4;
        let m = 3;
        let mut fpre = TensorFpre::new(0, n, m, 6);
        fpre.generate_for_ideal_trusted_dealer(0b1101, 0b110);
        let (fpre_gen, _) = fpre.into_gen_eval();
        // ... dimension assertions ...
        let mut gar = AuthTensorGen::new_from_fpre_gen(fpre_gen);
        let (_chunk_levels, _chunk_cts) = gar.garble_first_half();
    }
}
```
NOTE: this existing test calls `TensorFpre::generate_for_ideal_trusted_dealer` directly — but `TensorFpre` does NOT populate `gamma_auth_bit_shares` (only `IdealPreprocessingBackend::run` does, per `src/preprocessing.rs:144-159`). For `compute_lambda_gamma()` tests, USE `IdealPreprocessingBackend.run(n, m, 1, cf)` instead — see `src/preprocessing.rs:300-321` for the exact call shape.

---

### `src/auth_tensor_eval.rs` (modify — add `gamma_auth_bit_shares` field + `compute_lambda_gamma(&[bool])` + create `mod tests {}`)

**Analog:** `src/auth_tensor_gen.rs` (symmetric mirror) + `src/auth_tensor_eval.rs` itself (existing `evaluate_final` at lines 153-163).

**Field-declaration pattern** (`src/auth_tensor_eval.rs:19-21` — mirror line 21):
```rust
pub alpha_auth_bit_shares: Vec<AuthBitShare>,
pub beta_auth_bit_shares: Vec<AuthBitShare>,
pub correlated_auth_bit_shares: Vec<AuthBitShare>,
// ADD: pub gamma_auth_bit_shares: Vec<AuthBitShare>,
```

**`new_from_fpre_eval()` field-forwarding pattern** (`src/auth_tensor_eval.rs:54-57` — replace TODO at line 57):
```rust
alpha_auth_bit_shares: fpre_eval.alpha_auth_bit_shares,
beta_auth_bit_shares: fpre_eval.beta_auth_bit_shares,
correlated_auth_bit_shares: fpre_eval.correlated_auth_bit_shares,
// TODO(Phase 8): forward fpre_eval.gamma_auth_bit_shares to a corresponding field on AuthTensorEval
// REPLACE WITH: gamma_auth_bit_shares: fpre_eval.gamma_auth_bit_shares,
```

**Column-major iteration pattern** (`src/auth_tensor_eval.rs:155-163` — `evaluate_final` loop, mirror layout for `compute_lambda_gamma`):
```rust
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
The new `compute_lambda_gamma(&self, lambda_gb: &[bool]) -> Vec<bool>` mirrors this loop shape; reads `lambda_gb[j*n+i]`, `first_half_out[(i,j)].lsb()`, `gamma_auth_bit_shares[j*n+i].bit()`; pushes XOR of all three.

**Test module creation** — `src/auth_tensor_eval.rs` has NO `#[cfg(test)] mod tests {}` block (verified — TESTING.md coverage gap). Use `src/auth_tensor_gen.rs:196-231` as the template for adding one. Standard imports: `use super::*; use crate::preprocessing::{IdealPreprocessingBackend, TensorPreprocessing};`.

---

### `src/online.rs` (NEW — `check_zero(c_gamma_shares, delta_ev) -> bool` + tests)

**Analog 1:** `src/feq.rs` (closest "ideal abort-on-mismatch consistency check" pattern). Differs in return type — `feq::check` panics, `check_zero` returns `bool` per D-07.

**Module-doc + import pattern** (`src/feq.rs:1-12`):
```rust
//! Ideal F_eq functionality: element-wise BlockMatrix equality check.
//!
//! In the real protocol, parties send L_1 and L_2 to F_eq, which compares
//! them and returns 0 (abort) if they differ, 1 (continue) otherwise. This
//! in-process ideal version panics on mismatch, matching the protocol's
//! abort semantics (per CONTEXT.md D-04).

use crate::matrix::BlockMatrix;
```
Adapt to:
```rust
//! Online phase primitives that span both garbler and evaluator views.
//!
//! Currently hosts `check_zero()` only. `open()` will be added in a future
//! phase (deferred per Phase 8 CONTEXT.md D-01).

use crate::sharing::AuthBitShare;
use crate::delta::Delta;
```

**Check signature + iteration pattern** (`src/feq.rs:19-32`):
```rust
pub fn check(l1: &BlockMatrix, l2: &BlockMatrix) {
    assert_eq!(l1.rows(), l2.rows(), "F_eq: row dimension mismatch");
    assert_eq!(l1.cols(), l2.cols(), "F_eq: column dimension mismatch");
    for j in 0..l1.cols() {
        for i in 0..l1.rows() {
            if l1[(i, j)] != l2[(i, j)] {
                panic!("F_eq abort: ...", i, j);
            }
        }
    }
}
```
Adapt to: take `&[AuthBitShare]` + `&Delta`, return `bool`. Iterate once over the slice; on any failure return `false`; otherwise `true`. Keep iteration order linear (slice already encodes column-major from caller).

**Analog 2:** `src/sharing.rs::AuthBitShare::verify` for the MAC-invariant check mechanic.

**MAC verification pattern** (`src/sharing.rs:60-63`):
```rust
pub fn verify(&self, delta: &Delta) {
    let want: Mac = self.key.auth(self.bit(), delta);
    assert_eq!(self.mac, want, "MAC mismatch in share");
}
```
**For `check_zero()` — replace `assert_eq!` with conditional return:**
```rust
let want = share.key.auth(share.value, delta_ev);
if share.mac != want { return false; }
```
Use `Key::auth(bit, delta) -> Mac` from `src/keys.rs:65-68`:
```rust
#[inline]
pub fn auth(&self, bit: bool, delta: &Delta) -> Mac {
    Mac::new(self.0 ^ if bit { delta.as_block() } else { &Block::ZERO })
}
```

**Two checks per share** (per RESEARCH Open Question 1, recommendation (a)):
1. Reconstructed bit must be 0: `if share.value { return false; }` (caller pre-XORed two parties' shares so `value` is the reconstructed bit).
2. IT-MAC invariant under `delta_ev`: `if share.mac != share.key.auth(share.value, delta_ev) { return false; }`.

**Tests pattern** (`src/feq.rs:34-69` for the test module shape; `src/sharing.rs:172-208` for the share-construction test pattern):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;
    use crate::keys::Key;
    use rand_chacha::ChaCha12Rng;
    use rand::SeedableRng;

    #[test]
    fn test_check_zero_passes_on_zero_bit_with_valid_mac() {
        let mut rng = ChaCha12Rng::seed_from_u64(1);
        let delta = Delta::random(&mut rng);
        let key = Key::new(Block::random(&mut rng));
        let mac = key.auth(false, &delta);
        let share = AuthBitShare { key, mac, value: false };
        assert!(check_zero(&[share], &delta));
    }
    // similar for fail-bit (value=true), fail-MAC (wrong delta in mac generation)
}
```
Note: existing tests across the codebase ALWAYS use `ChaCha12Rng::seed_from_u64(N)` — verified at `src/sharing.rs:181,192`, `src/keys.rs:251`, `src/preprocessing.rs:144`. Never use `rand::rng()` in tests (per CONVENTIONS.md).

---

### `src/lib.rs` (modify — add `pub mod online;` + extend `test_auth_tensor_product`)

**Analog:** `src/lib.rs` itself.

**Module-declaration pattern** (`src/lib.rs:25` — add `pub mod online;` adjacent):
```rust
pub mod auth_tensor_pre;
pub mod preprocessing;
// ADD: pub mod online;
```

**End-to-end integration-test extension pattern** (`src/lib.rs:248-380` — existing `test_auth_tensor_product`):
The test currently:
1. Constructs `TensorFpre::new_with_delta(...)` and calls `generate_for_ideal_trusted_dealer`.
2. Calls `into_gen_eval()` → `AuthTensorGen::new_from_fpre_gen` + `AuthTensorEval::new_from_fpre_eval`.
3. Calls `garble_first_half / garble_second_half / garble_final` and `evaluate_*` symmetrically.
4. Verifies output via `verify_tensor_output(...)` helper at lines 105-129.

**Key reusable helper** (`src/lib.rs:105-129`):
```rust
fn verify_tensor_output(
    clear_x: usize,
    clear_y: usize,
    n: usize,
    m: usize,
    gb_out: &BlockMatrix,
    ev_out: &BlockMatrix,
    delta: &Delta,
) -> bool {
    for i in 0..n {
        for k in 0..m {
            let expected_val = (((clear_x>>i)&1) & ((clear_y>>k)&1)) != 0;
            // ... XOR check vs delta ...
        }
    }
    true
}
```

**P1-04 extension** (new test or extension of `test_auth_tensor_product`): switch from `TensorFpre` direct path to `IdealPreprocessingBackend.run(n, m, 1, chunking_factor)` — see `src/preprocessing.rs:121-163` and `src/preprocessing.rs:300-321` for the exact construction. After `gb.garble_final()` / `ev.evaluate_final()`, call:
```rust
let lambda_gb: Vec<bool> = gb.compute_lambda_gamma();
let lambda_combined: Vec<bool> = ev.compute_lambda_gamma(&lambda_gb);
// Assert lambda_combined[j*n+i] == ((input_x>>i)&1) AND ((input_y>>j)&1) XOR l_gamma_bit
// where l_gamma_bit = gb.gamma_auth_bit_shares[j*n+i].bit() XOR ev.gamma_auth_bit_shares[j*n+i].bit()
```

**P1-05 negative test pattern** (paired test, NEW): clone `lambda_gb`, flip one entry, then assemble `c_gamma_shares` from the four D_ev share vecs (pattern in RESEARCH Pattern 3 / D-09). Per RESEARCH Open Question 1 recommendation (a), pre-XOR the two parties' shares pairwise:
```rust
let combined_share = AuthBitShare {
    key:   gb_share.key + ev_share.key,    // XOR per src/sharing.rs:66-77
    mac:   gb_share.mac + ev_share.mac,
    value: gb_share.value ^ ev_share.value,
};
```
Then call `check_zero(&combined_shares, &delta_ev)` and `assert!(!result)` for the tampered case, `assert!(result)` for honest.

**`AuthBitShare` Add (XOR) pattern** (`src/sharing.rs:66-77`):
```rust
impl Add<AuthBitShare> for AuthBitShare {
    type Output = Self;
    #[inline]
    fn add(self, rhs: AuthBitShare) -> Self {
        Self {
            key: self.key + rhs.key,
            mac: self.mac + rhs.mac,
            value: self.value ^ rhs.value,
        }
    }
}
```
Use the `+` operator to combine shares — never re-implement field-wise XOR by hand.

---

## Shared Patterns

### Column-Major Indexing (`j * n + i`)
**Sources:**
- `src/auth_tensor_gen.rs:182,185` (in `garble_final`)
- `src/auth_tensor_eval.rs:160` (in `evaluate_final`)
- `src/preprocessing.rs:35-36, 38-43, 67, 69-71` (Fpre struct field docs)
- `.planning/codebase/CONVENTIONS.md` line 67

**Apply to:** all new `n*m` field vecs (`gamma_auth_bit_shares`) and all new column-major iterations in `compute_lambda_gamma` and `c_gamma_shares` assembly.

```rust
for i in 0..self.n {
    for j in 0..self.m {
        // index into n*m vec: [j * self.n + i]
        // index into BlockMatrix(n,m): [(i, j)]
        // index into BlockMatrix(m,n): [(j, i)]   (for second_half_out)
    }
}
```

### IT-MAC Invariant Reconstruction
**Source:** `src/sharing.rs:60-63` (`AuthBitShare::verify`) + `src/keys.rs:65-68` (`Key::auth`).

**Apply to:** `check_zero()` MAC check; never panic — return `false` instead per D-07.

```rust
let want = key.auth(value, delta);   // Mac::new(key.0 ^ if value { delta.as_block() } else { &Block::ZERO })
if mac != want { /* fail */ }
```

### Cross-Party Share Composition (Pre-XOR for `check_zero`)
**Source:** `src/sharing.rs:66-115` (four `Add` impls — XORs key, mac, value field-wise).
**Apply to:** caller of `check_zero` in `src/lib.rs` tests (the c_gamma assembly).

**WARNING:** Do NOT call `share.verify(delta)` directly on a raw cross-party `AuthBitShare`; it will panic on correctly-formed shares (`src/auth_tensor_pre.rs:305-336` documents this). The caller MUST pre-XOR gen and eval shares (using `+` operator) BEFORE passing to `check_zero` so that the resulting share has aligned key/mac/value semantics.

### Deterministic-Seeded Test RNG
**Source:** `src/sharing.rs:181,192`, `src/keys.rs:251`, `src/preprocessing.rs:144`.
**Apply to:** every test in `src/online.rs::tests` and any new tests in `src/auth_tensor_eval.rs::tests`, `src/auth_tensor_gen.rs::tests`, `src/lib.rs::tests`.

```rust
use rand_chacha::ChaCha12Rng;
use rand::SeedableRng;
let mut rng = ChaCha12Rng::seed_from_u64(N);   // distinct N per test
```

Never `rand::rng()` in tests (CONVENTIONS.md). Existing `test_auth_tensor_product` at `src/lib.rs:254` does use `rand::rng()` — that is a pre-existing exception that should NOT be propagated to the new tests.

### Module-Declaration Style
**Source:** `src/lib.rs:1-25` — flat list of `pub mod foo;` with grouping blank lines.
**Apply to:** the new `pub mod online;` declaration. Place adjacent to `pub mod preprocessing;` (line 25).

### `IdealPreprocessingBackend` for Gamma-Required Tests
**Source:** `src/preprocessing.rs:121-163` and `src/preprocessing.rs:300-321`.
**Apply to:** all P1-04 / P1-05 integration tests that call `compute_lambda_gamma()`, because `UncompressedPreprocessingBackend` leaves `gamma_auth_bit_shares = vec![]` (Phase 7 stub — `src/preprocessing.rs:288-295`). `TensorFpre::generate_for_ideal_trusted_dealer` directly also does NOT populate gamma shares (only `IdealPreprocessingBackend::run` does the gamma sampling at lines 144-159).

```rust
let (fpre_gen, fpre_eval) = IdealPreprocessingBackend.run(n, m, 1, chunking_factor);
let mut gb = AuthTensorGen::new_from_fpre_gen(fpre_gen);
let mut ev = AuthTensorEval::new_from_fpre_eval(fpre_eval);
// ... existing garble/evaluate sequence ...
let lambda_gb = gb.compute_lambda_gamma();
let lambda    = ev.compute_lambda_gamma(&lambda_gb);
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | All four files have either an exact in-file analog (auth_tensor_gen, auth_tensor_eval, lib.rs) or a strong role-match (online.rs ← feq.rs + sharing.rs::verify). |

---

## Metadata

**Analog search scope:** `/Users/turan/Desktop/authenticated-tensor-garbling/src/**`
**Files scanned:** auth_tensor_gen.rs, auth_tensor_eval.rs, sharing.rs, preprocessing.rs, lib.rs, feq.rs, keys.rs, macs.rs, block.rs, delta.rs, auth_tensor_pre.rs, auth_tensor_fpre.rs (+ project conventions doc)
**Pattern extraction date:** 2026-04-23
