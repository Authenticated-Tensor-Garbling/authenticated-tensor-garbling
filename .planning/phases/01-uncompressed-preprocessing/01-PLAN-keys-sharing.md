---
phase: 01-uncompressed-preprocessing
plan: 01-PLAN-keys-sharing
type: execute
wave: 1
depends_on: []
files_modified:
  - src/keys.rs
  - src/sharing.rs
  - src/auth_tensor_fpre.rs
  - src/tensor_pre.rs
autonomous: true
requirements:
  - CLEAN-01
  - CLEAN-02
  - CLEAN-03
  - CLEAN-04
tags:
  - refactor
  - primitives
  - invariants
user_setup: []

must_haves:
  truths:
    - "Every Key produced by Key::new(block) has lsb() == 0"
    - "Key::random(&mut rng) returns a Key whose block has lsb() == 0"
    - "build_share(rng, bit, delta) returns an AuthBitShare whose key.as_block().lsb() == false and whose mac satisfies mac == key.auth(bit, delta)"
    - "InputSharing.shares_differ() exists and returns the same value InputSharing.bit() used to return (gen_share != eval_share)"
    - "InputSharing.bit() no longer exists"
    - "AuthBitShare type has a doc comment stating it is one party's view and the invariant mac == key.auth(value, verifier_delta)"
    - "AuthBit type has a doc comment stating it holds both parties' views"
    - "build_share has a doc comment explaining delta is the verifying party's global correlation key"
    - "cargo build succeeds after the change"
    - "cargo test passes all tests that passed before the change"
  artifacts:
    - path: "src/keys.rs"
      provides: "Key type with Key::new() safe constructor and Key::random() invariant-preserving constructor"
      contains: "pub fn new"
    - path: "src/sharing.rs"
      provides: "AuthBitShare/AuthBit doc comments, InputSharing.shares_differ(), build_share using Key::new"
      contains: "pub fn shares_differ"
    - path: "src/auth_tensor_fpre.rs"
      provides: "get_clear_values updated to use shares_differ()"
      contains: "shares_differ()"
    - path: "src/tensor_pre.rs"
      provides: "mask_inputs updated to use shares_differ()"
      contains: "shares_differ()"
  key_links:
    - from: "src/sharing.rs (build_share)"
      to: "src/keys.rs (Key::new)"
      via: "direct call: Key::new(Block::random(rng))"
      pattern: "Key::new\\(Block::random"
    - from: "src/auth_tensor_fpre.rs (get_clear_values)"
      to: "src/sharing.rs (InputSharing::shares_differ)"
      via: "method call on InputSharing values"
      pattern: "\\.shares_differ\\(\\)"
    - from: "src/tensor_pre.rs (mask_inputs)"
      to: "src/sharing.rs (InputSharing::shares_differ)"
      via: "method call on InputSharing values"
      pattern: "\\.shares_differ\\(\\)"
---

<objective>
Refactor the Key type and sharing module so the `Key.lsb() == 0` invariant is enforced at construction (CLEAN-01, CLEAN-04), the AuthBitShare/AuthBit distinction is documented (CLEAN-02), and the confusingly-named `InputSharing.bit()` is renamed to `shares_differ()` (CLEAN-03). Zero algorithmic changes — every existing test must pass unchanged.

Purpose: Move from runtime-maintained invariants (manual `set_lsb(false)` + `Key::from(...)` two-step) to a construction-time type guarantee via `Key::new()`, eliminating a class of LSB-bug risks in future protocol code. Clarify type semantics so future implementers (Phase 2–6) do not confuse "one party's view" with "both parties' views" or "XOR of share blocks" with "the underlying input bit".
Output: Updated `src/keys.rs`, `src/sharing.rs`, and the two files that call `InputSharing.bit()` (`src/auth_tensor_fpre.rs`, `src/tensor_pre.rs`). `cargo build` and `cargo test` both green.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/REQUIREMENTS.md
@.planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md

<interfaces>
<!-- Existing types the executor must preserve. Do not change signatures beyond what this plan specifies. -->

From src/keys.rs:
```rust
pub struct Key(Block);
impl Key {
    pub fn pointer(&self) -> bool;
    pub fn set_pointer(&mut self, bit: bool);
    pub fn adjust(&mut self, adjust: bool, delta: &Delta);  // already calls set_lsb(false)
    pub fn auth(&self, bit: bool, delta: &Delta) -> Mac;
    pub fn as_block(&self) -> &Block;
    pub fn as_blocks(slice: &[Self]) -> &[Block];
    pub fn from_blocks(blocks: Vec<Block>) -> Vec<Self>;
    pub fn into_blocks(keys: Vec<Self>) -> Vec<Block>;
    pub fn random<R: Rng>(rng: &mut R) -> Self;   // CURRENT: does NOT clear LSB
}
impl From<Block> for Key { fn from(block: Block) -> Key { Key(block) } }   // zero-cost cast, keep as-is
impl From<[u8; 16]> for Key { ... }   // used by build_share today; keep the impl, change the caller
```

From src/sharing.rs:
```rust
pub struct InputSharing {
    pub gen_share: Block,
    pub eval_share: Block,
}
impl InputSharing {
    pub fn bit(&self) -> bool { /* returns gen_share != eval_share */ }
}

pub struct AuthBitShare {
    pub key: Key,
    pub mac: Mac,
    pub value: bool,
}
impl AuthBitShare {
    pub fn bit(&self) -> bool;
    pub fn verify(&self, delta: &Delta);
}
pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
    let key: Key = Key::from(rng.random::<[u8; 16]>());   // <-- bug: does NOT clear LSB
    let mac: Mac = key.auth(bit, delta);
    AuthBitShare { key, mac, value: bit }
}

pub struct AuthBit {
    pub gen_share: AuthBitShare,
    pub eval_share: AuthBitShare,
}
impl AuthBit {
    pub fn full_bit(&self) -> bool;
    pub fn verify(&self, delta_a: &Delta, delta_b: &Delta);
}
```

From src/delta.rs (precedent for the construction-time invariant pattern — replicate for Key):
```rust
impl Delta {
    pub fn new(mut value: Block) -> Self { value.set_lsb(true); Self(value) }   // sets LSB=1 at construction
    pub fn random<R: Rng>(rng: &mut R) -> Self { Self::new(Block::from(rng.random::<[u8; 16]>())) }
}
```

From src/block.rs:
```rust
impl Block {
    pub fn random<R: Rng>(rng: &mut R) -> Self;   // exists and returns a Block
    pub fn lsb(&self) -> bool;
    pub fn set_lsb(&mut self, bit: bool);
}
```

Call sites of InputSharing.bit() that must be updated (verified by grep):
- src/auth_tensor_fpre.rs:235   `x |= (self.x_labels[i].bit() as usize) << i;`
- src/auth_tensor_fpre.rs:239   `y |= (self.y_labels[j].bit() as usize) << j;`
- src/tensor_pre.rs:109          `masked_x |= (self.x_labels[i].bit() as usize ^ self.alpha_labels[i].bit() as usize) << i;`
- src/tensor_pre.rs:119          `masked_y |= (self.y_labels[j].bit() as usize ^ self.beta_labels[j].bit() as usize) << j;`

Do NOT touch `.bit()` calls on `AuthBitShare` (e.g. `correlated_auth_bit_shares[...].bit()` in auth_tensor_gen.rs, auth_tensor_eval.rs, lib.rs) — those are a different method on a different type.
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
  <name>Task 1: Add Key::new() and update Key::random() to enforce the lsb==0 invariant (CLEAN-01)</name>
  <files>src/keys.rs</files>
  <read_first>
    - src/keys.rs (full file — to see existing `adjust`, `random`, `From<Block>`, `From<[u8;16]>` impls)
    - src/delta.rs (precedent: `Delta::new(mut value: Block) -> Self { value.set_lsb(true); Self(value) }`)
    - src/block.rs (lines around `Block::random` and `Block::set_lsb` to confirm signatures)
  </read_first>
  <behavior>
    - `Key::new(Block::new([0xFF; 16])).as_block().lsb()` returns `false` (LSB cleared)
    - `Key::new(Block::new([0xFE; 16])).as_block().lsb()` returns `false` (LSB already 0; idempotent)
    - For any rng, `Key::random(&mut rng).as_block().lsb()` returns `false`
    - `Key::from(block)` is unchanged — it still constructs without clearing LSB (zero-cost cast semantics preserved per D-02)
    - `Key::from([u8;16])` is unchanged — it still constructs without clearing LSB
    - All existing methods (`pointer`, `set_pointer`, `adjust`, `auth`, `as_block`, etc.) compile unchanged
  </behavior>
  <action>
    Make exactly these edits to `src/keys.rs`:

    1. Add a new method `Key::new` inside `impl Key { ... }` (place it immediately before the existing `fn pointer(&self) -> bool` method, right after the `impl Key {` line). Exact code:

    ```rust
    /// Safe constructor that enforces the `Key.lsb() == 0` invariant at construction time.
    ///
    /// The LSB of a `Key` must be zero so the prover can store the authenticated bit in
    /// `LSB(MAC)` (see `auth`). This constructor clears the LSB before wrapping the block.
    ///
    /// Prefer `Key::new` over `Key::from(block)` whenever the caller has not already
    /// cleared the LSB. `From<Block>` is retained as a zero-cost cast for callers that
    /// have already enforced the invariant themselves (for example via `Key::adjust`).
    #[inline]
    pub fn new(mut block: Block) -> Self {
        block.set_lsb(false);
        Self(block)
    }
    ```

    2. Replace the existing `Key::random` implementation (currently `Self(Block::random(rng))`) with a call to `Key::new` so random keys satisfy the invariant (per D-03). Exact replacement:

    ```rust
    #[inline]
    pub fn random<R: Rng>(rng: &mut R) -> Self {
        Self::new(Block::random(rng))
    }
    ```

    3. Do NOT modify `impl From<Block> for Key`, `impl From<[u8; 16]> for Key`, `adjust`, `auth`, `pointer`, `set_pointer`, `as_block`, `as_blocks`, `from_blocks`, `into_blocks`, or any of the `Add`/`BitXor`/`Display` impls (per D-02).

    4. Do NOT add or remove any `use` imports — `Block`, `Delta`, `Mac`, and `Rng` are already imported.

    5. Add or update the existing test module at the bottom of `src/keys.rs`. If no `#[cfg(test)] mod tests` exists, append this block at the end of the file:

    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::block::Block;
        use rand_chacha::ChaCha12Rng;
        use rand::SeedableRng;

        #[test]
        fn test_key_new_clears_lsb_when_set() {
            let mut b = Block::new([0xFF; 16]);
            assert!(b.lsb());
            let k = Key::new(b);
            assert!(!k.as_block().lsb(), "Key::new must clear LSB");
        }

        #[test]
        fn test_key_new_idempotent_when_already_cleared() {
            let mut b = Block::new([0xFF; 16]);
            b.set_lsb(false);
            let k = Key::new(b);
            assert!(!k.as_block().lsb());
        }

        #[test]
        fn test_key_random_lsb_is_zero() {
            let mut rng = ChaCha12Rng::seed_from_u64(0xC0FFEE);
            for _ in 0..64 {
                let k = Key::random(&mut rng);
                assert!(!k.as_block().lsb(), "Key::random must produce lsb==0");
            }
        }

        #[test]
        fn test_key_from_block_preserves_lsb_for_backward_compat() {
            // From<Block> is documented as zero-cost cast; it must NOT clear LSB.
            let mut b = Block::new([0xFF; 16]);
            b.set_lsb(true);
            let k = Key::from(b);
            assert!(k.as_block().lsb(), "Key::from<Block> must not clear LSB (zero-cost cast)");
        }
    }
    ```

    If a `#[cfg(test)] mod tests` already exists in `src/keys.rs`, insert these four `#[test]` functions into it without removing or changing any existing test.
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -20 &amp;&amp; cargo test --lib keys::tests 2>&amp;1 | tail -20</automated>
  </verify>
  <acceptance_criteria>
    - `grep -n "pub fn new(mut block: Block) -> Self" src/keys.rs` matches exactly one line
    - `grep -n "block.set_lsb(false);" src/keys.rs` matches at least 2 lines (one in `new`, one retained inside `adjust`)
    - `grep -n "Self::new(Block::random(rng))" src/keys.rs` matches exactly one line (inside `Key::random`)
    - `grep -n "impl From<Block> for Key" src/keys.rs` still matches (unchanged)
    - `grep -n "impl From<\[u8; 16\]> for Key" src/keys.rs` still matches (unchanged)
    - `cargo build --lib` exits 0
    - `cargo test --lib keys::tests::test_key_new_clears_lsb_when_set` exits 0
    - `cargo test --lib keys::tests::test_key_random_lsb_is_zero` exits 0
    - `cargo test --lib keys::tests::test_key_from_block_preserves_lsb_for_backward_compat` exits 0
  </acceptance_criteria>
  <done>Key::new exists, Key::random uses Key::new, From<Block> unchanged, four new tests pass, cargo build green.</done>
</task>

<task type="auto" tdd="false">
  <name>Task 2: Fix build_share to preserve Key LSB=0, add AuthBitShare/AuthBit/build_share docs, rename InputSharing.bit() to shares_differ() (CLEAN-02, CLEAN-03, CLEAN-04)</name>
  <files>src/sharing.rs</files>
  <read_first>
    - src/sharing.rs (full file — to see InputSharing, AuthBitShare, AuthBit, build_share)
    - src/keys.rs (to confirm Key::new from Task 1 is available; Task 1 is in the SAME plan and must be completed first in-plan)
    - .planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md (D-06, D-07, D-08, D-09 verbatim)
  </read_first>
  <behavior>
    - `InputSharing::shares_differ(&self) -> bool` returns the same value that `InputSharing::bit()` returned (i.e., `gen_share != eval_share`)
    - `InputSharing::bit()` no longer exists (hard rename per D-09, not an additive alias)
    - `build_share(&mut rng, bit, delta)` produces an `AuthBitShare` where `share.key.as_block().lsb() == false` for all inputs
    - `build_share(&mut rng, bit, delta).verify(delta)` does not panic (MAC invariant still holds)
    - `AuthBitShare` carries a `///` doc comment (on the struct) stating it holds one party's view and naming the invariant `mac == key.auth(value, verifier_delta)`
    - `AuthBit` carries a `///` doc comment (on the struct) stating it holds both parties' views
    - `build_share` carries a `///` doc comment stating `delta` is the verifying party's global correlation key
  </behavior>
  <action>
    Depends on Task 1 (Key::new must exist). Make these edits to `src/sharing.rs`:

    1. **Rename `InputSharing.bit()` to `shares_differ()` (D-09):** Replace the existing method definition

    ```rust
    impl InputSharing {
        pub fn bit(&self) -> bool {
            if self.gen_share == self.eval_share {
                false
            } else {
                true
            }
        }
    }
    ```

    with

    ```rust
    impl InputSharing {
        /// Returns whether the two parties' share blocks differ.
        ///
        /// Under the BDOZ-style XOR sharing used here, a bit `b` is encoded as
        /// `gen_share XOR eval_share`. This method returns `gen_share != eval_share`
        /// — it does **not** recover the underlying input bit of a masked wire
        /// (which would require knowing both parties' deltas). Historical name
        /// `bit()` was ambiguous; use this instead.
        #[inline]
        pub fn shares_differ(&self) -> bool {
            self.gen_share != self.eval_share
        }
    }
    ```

    2. **Fix `build_share` to clear the Key LSB (D-04):** Replace the existing body

    ```rust
    pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
        let key: Key = Key::from(rng.random::<[u8; 16]>());
        let mac: Mac = key.auth(bit, delta);
        AuthBitShare { key, mac, value: bit }
    }
    ```

    with

    ```rust
    /// Builds one `AuthBitShare` for the given `bit` under the verifying party's `delta`.
    ///
    /// `delta` is the **verifying party's** global correlation key (for example,
    /// when A holds the key and B holds the MAC, `delta` is B's delta). The returned
    /// share satisfies the IT-MAC invariant `mac == key.auth(bit, delta)` and its
    /// `key` has `lsb() == 0` (enforced by `Key::new`).
    pub fn build_share(rng: &mut ChaCha12Rng, bit: bool, delta: &Delta) -> AuthBitShare {
        let key: Key = Key::new(Block::random(rng));
        let mac: Mac = key.auth(bit, delta);
        AuthBitShare { key, mac, value: bit }
    }
    ```

    Keep the existing `use crate::block::Block;` at the top of the file (already imported — verify before editing).

    3. **Add struct-level doc comment to `AuthBitShare` (D-06):** Replace the single-line `/// AuthBitShare consisting of a bool and a (key, mac) pair` with

    ```rust
    /// One party's view of an authenticated bit.
    ///
    /// In the two-party BDOZ-style IT-MAC sharing, each bit `b` is held by two
    /// parties simultaneously. This struct holds **one party's** view:
    /// - `key`: the bCOT sender's key for this position (`lsb() == 0` invariant)
    /// - `mac`: the bCOT receiver's chosen MAC, authenticating `value` under the
    ///          **verifying party's** delta
    /// - `value`: the committed bit the holder claims
    ///
    /// Invariant: `mac == key.auth(value, verifier_delta)` where `verifier_delta`
    /// is the other party's global correlation key. `AuthBitShare::verify(&delta)`
    /// checks this equation.
    #[derive(Debug, Clone, Default, Copy)]
    pub struct AuthBitShare { ... }
    ```

    Leave the struct body identical to what is there today.

    4. **Add struct-level doc comment to `AuthBit` (D-07):** Replace the existing `/// Represents an auth bit [x] = [r]+[s] where [r] is known to gen, auth by eval and [s] is known to eval, auth by gen.` with

    ```rust
    /// Both parties' views of an authenticated bit, paired together.
    ///
    /// `AuthBit` holds an `AuthBitShare` for each party (gen and eval) and is
    /// used in the ideal trusted-dealer `TensorFpre` and in tests that need to
    /// reconstruct the full two-party state. Compare with `AuthBitShare`, which
    /// holds only one party's view.
    ///
    /// The additive-sharing relation is `[x] = gen_share.value XOR eval_share.value`
    /// (see `full_bit()`); MAC invariants are verified under each party's delta by
    /// `verify(&delta_a, &delta_b)`.
    #[derive(Debug, Clone)]
    pub struct AuthBit { ... }
    ```

    Leave the struct body identical to what is there today.

    5. Do NOT change the signatures of `AuthBitShare::bit`, `AuthBitShare::verify`, `AuthBit::full_bit`, `AuthBit::verify`, or any of the `Add` impls (per D-06 / D-07 "no field renames").

    6. Append these tests to the `#[cfg(test)] mod tests` block at the end of `src/sharing.rs`. If no test module exists, create one:

    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::block::Block;
        use rand_chacha::ChaCha12Rng;
        use rand::SeedableRng;

        #[test]
        fn test_build_share_key_lsb_is_zero() {
            let mut rng = ChaCha12Rng::seed_from_u64(7);
            let delta = Delta::random(&mut rng);
            for bit in [false, true] {
                let share = build_share(&mut rng, bit, &delta);
                assert!(!share.key.as_block().lsb(),
                    "build_share must clear Key LSB (bit={})", bit);
            }
        }

        #[test]
        fn test_build_share_mac_invariant_holds() {
            let mut rng = ChaCha12Rng::seed_from_u64(11);
            let delta = Delta::random(&mut rng);
            for bit in [false, true] {
                let share = build_share(&mut rng, bit, &delta);
                share.verify(&delta);   // panics on mismatch
            }
        }

        #[test]
        fn test_input_sharing_shares_differ() {
            let a = Block::new([1u8; 16]);
            let b = Block::new([1u8; 16]);
            let c = Block::new([2u8; 16]);
            assert_eq!(InputSharing { gen_share: a, eval_share: b }.shares_differ(), false);
            assert_eq!(InputSharing { gen_share: a, eval_share: c }.shares_differ(), true);
        }
    }
    ```
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -20 &amp;&amp; cargo test --lib sharing::tests 2>&amp;1 | tail -20</automated>
  </verify>
  <acceptance_criteria>
    - `grep -n "pub fn shares_differ" src/sharing.rs` matches exactly one line
    - `grep -n "pub fn bit(&self) -> bool" src/sharing.rs` matches exactly one line (the one on `AuthBitShare`, NOT on `InputSharing`)
    - `grep -n "fn bit(&self)" src/sharing.rs | wc -l` outputs exactly `1` (only `AuthBitShare::bit`, `InputSharing::bit` is gone)
    - `grep -n "Key::new(Block::random(rng))" src/sharing.rs` matches exactly one line (inside `build_share`)
    - `grep -c "Key::from(rng.random::<\[u8; 16\]>())" src/sharing.rs` outputs `0` (old pattern removed)
    - `grep -n "One party's view of an authenticated bit" src/sharing.rs` matches one line (AuthBitShare doc)
    - `grep -n "Both parties' views of an authenticated bit" src/sharing.rs` matches one line (AuthBit doc)
    - `grep -n "verifying party's" src/sharing.rs` matches at least one line (build_share doc)
    - `cargo build --lib` exits 0
    - `cargo test --lib sharing::tests::test_build_share_key_lsb_is_zero` exits 0
    - `cargo test --lib sharing::tests::test_build_share_mac_invariant_holds` exits 0
    - `cargo test --lib sharing::tests::test_input_sharing_shares_differ` exits 0
  </acceptance_criteria>
  <done>shares_differ replaces InputSharing::bit, build_share uses Key::new, docs added to AuthBitShare/AuthBit/build_share, three new tests pass.</done>
</task>

<task type="auto" tdd="false">
  <name>Task 3: Update the 4 InputSharing.bit() call sites in auth_tensor_fpre.rs and tensor_pre.rs to use shares_differ() (CLEAN-03 follow-through)</name>
  <files>src/auth_tensor_fpre.rs, src/tensor_pre.rs</files>
  <read_first>
    - src/auth_tensor_fpre.rs (lines 220–245 — the `get_clear_values` method)
    - src/tensor_pre.rs (lines 100–130 — the `mask_inputs` method)
    - src/sharing.rs (to confirm `shares_differ` from Task 2 is available)
  </read_first>
  <behavior>
    - Compilation no longer references `InputSharing::bit` anywhere in the crate
    - `cargo build --lib` and `cargo build --tests --benches` succeed
    - Full pre-existing test suite passes (`cargo test` exit 0)
    - AuthBitShare::bit() calls (e.g. in auth_tensor_gen.rs, auth_tensor_eval.rs, lib.rs) are NOT touched — only `InputSharing` method calls are renamed
  </behavior>
  <action>
    Depends on Task 2 (InputSharing::shares_differ must exist). Make exactly these edits:

    1. **`src/auth_tensor_fpre.rs`**: in the `get_clear_values` method (the loop around lines 234–240), replace

    ```rust
    x |= (self.x_labels[i].bit() as usize) << i;
    ```
    with
    ```rust
    x |= (self.x_labels[i].shares_differ() as usize) << i;
    ```

    and replace

    ```rust
    y |= (self.y_labels[j].bit() as usize) << j;
    ```
    with
    ```rust
    y |= (self.y_labels[j].shares_differ() as usize) << j;
    ```

    Do NOT change any other `.bit()` call in this file — specifically the `.full_bit()` calls on `AuthBit` and any `.bit()` calls on `AuthBitShare` must remain untouched.

    2. **`src/tensor_pre.rs`**: in the `mask_inputs` method (lines 107–119), replace

    ```rust
    masked_x |= (self.x_labels[i].bit() as usize ^ self.alpha_labels[i].bit() as usize) << i;
    ```
    with
    ```rust
    masked_x |= (self.x_labels[i].shares_differ() as usize ^ self.alpha_labels[i].shares_differ() as usize) << i;
    ```

    and replace

    ```rust
    masked_y |= (self.y_labels[j].bit() as usize ^ self.beta_labels[j].bit() as usize) << j;
    ```
    with
    ```rust
    masked_y |= (self.y_labels[j].shares_differ() as usize ^ self.beta_labels[j].shares_differ() as usize) << j;
    ```

    Note: `x_labels`, `y_labels`, `alpha_labels`, `beta_labels` in `tensor_pre.rs` are `InputSharing` instances (see the `push(InputSharing { ... })` calls above). All four `.bit()` calls in this method (on both lines) must be updated.

    3. Do NOT modify any other file. Do NOT edit `.bit()` calls elsewhere in the crate — other `.bit()` calls are on `AuthBitShare`, not `InputSharing`, and are unaffected by this rename.
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -10 &amp;&amp; cargo test --lib 2>&amp;1 | tail -20</automated>
  </verify>
  <acceptance_criteria>
    - `grep -rn "\.bit() as usize" src/` returns zero lines (all InputSharing.bit() → shares_differ() renames done; the pattern "(.bit() as usize)" was unique to InputSharing call sites)
    - `grep -n "self.x_labels\[i\].shares_differ()" src/auth_tensor_fpre.rs` matches exactly one line
    - `grep -n "self.y_labels\[j\].shares_differ()" src/auth_tensor_fpre.rs` matches exactly one line
    - `grep -c "shares_differ()" src/tensor_pre.rs` outputs `4` (two on each of the two replaced lines)
    - `grep -c "InputSharing" src/sharing.rs` still matches at least 2 (struct def + impl block)
    - `cargo build --lib` exits 0
    - `cargo build --tests --benches` exits 0
    - `cargo test --lib` exits 0 (all pre-existing tests still pass)
  </acceptance_criteria>
  <done>Four call-site lines rewritten, `.bit() as usize` no longer appears anywhere, full test suite green.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Key producer → Key consumer | Any site that constructs a Key must uphold lsb==0; callers assume this when storing auth bits in mac LSB |
| Test harness → production build | Tests added here run under `#[cfg(test)]` only; no exposure to the production crypto path |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-01 | Tampering | `Key::from(block)` zero-cost cast can still produce Keys with LSB=1 | accept | By design (D-02): `From<Block>` is documented as a zero-cost escape hatch. Callers that construct from uncleared material must use `Key::new`. The only remaining in-crate caller of the `set_lsb + Key::from` pattern (`src/bcot.rs`) is migrated in Wave 2 by `01-PLAN-bcot-migration.md`. |
| T-01-02 | Information disclosure | `build_share` previously used `Key::from(rng.random::<[u8;16]>())` which did not clear LSB; a key with LSB=1 could corrupt the mac-LSB convention used by downstream verifiers | mitigate | Task 2 replaces the body with `Key::new(Block::random(rng))`; acceptance criterion greps for removal of the old pattern and presence of the new one. Test `test_build_share_key_lsb_is_zero` enforces the invariant over 2 bit values with a seeded RNG. |
| T-01-03 | Repudiation | Renaming `InputSharing::bit` → `shares_differ` is a breaking rename; a stale caller could silently compile against a type-coercing name elsewhere | mitigate | Hard rename (not an alias). Compile-time breakage of every caller is the enforcement mechanism; Task 3 migrates the four known call sites; `cargo build` is the acceptance gate. |
| T-01-04 | Denial of service | New unit tests could flake under a non-deterministic RNG | accept | All new tests use `ChaCha12Rng::seed_from_u64(...)` with fixed seeds; no wall-clock or thread-local randomness. |
| T-01-05 | Elevation of privilege | Doc comments on `AuthBitShare`/`AuthBit` make no new security claims they cannot back with code; misreading them could lead a Phase 2+ implementer to conflate one-party and two-party state | mitigate | Doc text is prescriptive and names the invariant formula `mac == key.auth(value, verifier_delta)`; the existing `verify(&delta)` method is the runtime enforcement and is unchanged. |

No high-severity threats introduced. ASVS L1 requirement: the Key LSB invariant is strengthened, not weakened.
</threat_model>

<verification>
After Task 3:

```bash
cargo build --lib
cargo build --tests --benches
cargo test --lib
```

All three must exit 0. No new warnings other than pre-existing ones.

Spot checks:

```bash
# No caller still uses the old InputSharing.bit() pattern
grep -rn "InputSharing" src/ | grep -v "shares_differ\|impl\|struct\|pub fn\|gen_share\|eval_share"   # should return nothing of the form `.bit()`

# The Key invariant pattern is now visible in the safe places
grep -n "Key::new" src/keys.rs src/sharing.rs   # expect hits in both

# The ideal Key::from escape hatch still exists (for zero-cost casts)
grep -n "impl From<Block> for Key" src/keys.rs   # expect 1 hit
```
</verification>

<success_criteria>
- `Key::new(block)` exists and clears `block.set_lsb(false)` before constructing
- `Key::random(&mut rng)` produces a Key with `lsb()==0` (verified by seeded unit test)
- `impl From<Block> for Key` retained unchanged (verified by unit test that a block with LSB=1 survives the From cast)
- `build_share` uses `Key::new(Block::random(rng))`; the old `Key::from(rng.random::<[u8;16]>())` pattern no longer appears in `src/sharing.rs`
- `InputSharing.shares_differ()` replaces `InputSharing.bit()`; the old method is deleted (not aliased)
- All 4 call sites in `src/auth_tensor_fpre.rs` (2 lines) and `src/tensor_pre.rs` (2 lines) updated
- `AuthBitShare`, `AuthBit`, and `build_share` have doc comments that state the one-party-vs-both-parties distinction and name the IT-MAC invariant
- `cargo build --lib` and `cargo test --lib` exit 0
</success_criteria>

<output>
After completion, create `.planning/phases/01-uncompressed-preprocessing/01-keys-sharing-SUMMARY.md`.
</output>
