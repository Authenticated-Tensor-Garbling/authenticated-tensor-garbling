---
phase: 01-uncompressed-preprocessing
plan: 01-PLAN-bcot-migration
type: execute
wave: 2
depends_on:
  - 01-PLAN-keys-sharing
files_modified:
  - src/bcot.rs
autonomous: true
requirements:
  - CLEAN-01
tags:
  - refactor
  - primitives
  - invariants
user_setup: []

must_haves:
  truths:
    - "Every caller in src/bcot.rs that constructs a Key from a random Block does so via Key::new, not via the two-step `block.set_lsb(false); Key::from(block)` pattern"
    - "transfer_a_to_b and transfer_b_to_a still produce sender_keys whose blocks have lsb()==0 and receiver_macs that satisfy mac == key.auth(b, delta)"
    - "All existing tests in bcot::tests pass unchanged"
    - "cargo build and the full crate test suite pass"
  artifacts:
    - path: "src/bcot.rs"
      provides: "IdealBCot::transfer_a_to_b and IdealBCot::transfer_b_to_a using Key::new"
      contains: "Key::new("
  key_links:
    - from: "src/bcot.rs (transfer_a_to_b, transfer_b_to_a)"
      to: "src/keys.rs (Key::new)"
      via: "direct call replacing the two-step set_lsb + Key::from pattern"
      pattern: "Key::new\\(Block::random"
---

<objective>
Migrate the two call sites in `src/bcot.rs` that use the manual `block.set_lsb(false); let k0 = Key::from(k0_block);` two-step pattern to the new `Key::new(Block::random(rng))` constructor added in plan `01-PLAN-keys-sharing` (D-05).

Purpose: Close the last remaining manual-invariant pattern in the codebase. After this plan, the `Key.lsb()==0` invariant is enforced in one place (the `Key::new` constructor) and no caller-side discipline is required to preserve it. Zero algorithmic change — the produced keys are functionally identical, only the construction idiom changes.
Output: `src/bcot.rs` with both `transfer_a_to_b` and `transfer_b_to_a` using `Key::new`, the existing `bcot::tests` module passing unchanged.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/REQUIREMENTS.md
@.planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md
@.planning/phases/01-uncompressed-preprocessing/01-keys-sharing-SUMMARY.md

<interfaces>
<!-- Interfaces from the upstream plan (01-PLAN-keys-sharing) that this plan consumes. -->

From src/keys.rs (added by 01-PLAN-keys-sharing, Task 1):
```rust
impl Key {
    /// Safe constructor that enforces `Key.lsb() == 0` at construction time.
    #[inline]
    pub fn new(mut block: Block) -> Self {
        block.set_lsb(false);
        Self(block)
    }
}
```

Current state of `src/bcot.rs` at the two call sites (verified by grep):

- transfer_a_to_b (lines ~64–80):
```rust
pub fn transfer_a_to_b(&mut self, choices: &[bool]) -> BcotOutput {
    let mut sender_keys = Vec::with_capacity(choices.len());
    let mut receiver_macs = Vec::with_capacity(choices.len());

    for &b in choices {
        let mut k0_block = Block::random(&mut self.rng);
        k0_block.set_lsb(false);                 // <-- two-step pattern
        let k0 = Key::from(k0_block);            // <-- two-step pattern
        let mac = k0.auth(b, &self.delta_a);
        sender_keys.push(k0);
        receiver_macs.push(mac);
    }

    BcotOutput { sender_keys, receiver_macs, choices: choices.to_vec() }
}
```

- transfer_b_to_a (lines ~89–107):
```rust
pub fn transfer_b_to_a(&mut self, choices: &[bool]) -> BcotOutput {
    // ... identical structure, uses self.delta_b instead of self.delta_a ...
    let mut k0_block = Block::random(&mut self.rng);
    k0_block.set_lsb(false);
    let k0 = Key::from(k0_block);
    let mac = k0.auth(b, &self.delta_b);
    // ...
}
```

All other code in `src/bcot.rs` (BcotOutput struct, IdealBCot::new, output_to_auth_bit_shares_a_holds_key, the `#[cfg(test)] mod tests` block) is outside the scope of this plan and must remain unchanged.
</interfaces>
</context>

<tasks>

<task type="auto" tdd="false">
  <name>Task 1: Collapse the set_lsb(false) + Key::from(block) two-step to Key::new(...) in both transfer_* methods</name>
  <files>src/bcot.rs</files>
  <read_first>
    - src/bcot.rs (full file — to confirm the two call sites and the surrounding context at lines ~64–80 and ~89–107)
    - src/keys.rs (to confirm Key::new signature `pub fn new(mut block: Block) -> Self` is present; dependency on 01-PLAN-keys-sharing must be satisfied before starting this task)
    - .planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md (D-05 verbatim)
  </read_first>
  <behavior>
    - `transfer_a_to_b(&[b])` still returns a `BcotOutput` where for every index i: `receiver_macs[i] == sender_keys[i].auth(choices[i], &delta_a)`
    - `transfer_b_to_a(&[b])` still returns a `BcotOutput` where for every index i: `receiver_macs[i] == sender_keys[i].auth(choices[i], &delta_b)`
    - `sender_keys[i].as_block().lsb() == false` for every i in both methods
    - The existing tests (`test_transfer_a_to_b_all_false`, `test_transfer_a_to_b_all_true`, etc.) pass without modification
    - No other methods, struct definitions, or test code in `src/bcot.rs` are changed
  </behavior>
  <action>
    Per D-05. Make exactly these edits to `src/bcot.rs`:

    1. In `transfer_a_to_b`, replace the three-line block
    ```rust
            let mut k0_block = Block::random(&mut self.rng);
            k0_block.set_lsb(false);
            let k0 = Key::from(k0_block);
    ```
    with the single line
    ```rust
            let k0 = Key::new(Block::random(&mut self.rng));
    ```
    Preserve the surrounding `for &b in choices { ... }` loop and the subsequent `let mac = k0.auth(b, &self.delta_a); sender_keys.push(k0); receiver_macs.push(mac);` lines exactly as they are.

    2. In `transfer_b_to_a`, replace the three-line block
    ```rust
            let mut k0_block = Block::random(&mut self.rng);
            k0_block.set_lsb(false);
            let k0 = Key::from(k0_block);
    ```
    with the single line
    ```rust
            let k0 = Key::new(Block::random(&mut self.rng));
    ```
    Preserve the surrounding loop and the subsequent `let mac = k0.auth(b, &self.delta_b); ...` lines.

    3. Do NOT modify: the `use` imports at the top of the file, `BcotOutput`, `IdealBCot::new`, `output_to_auth_bit_shares_a_holds_key`, any doc comment in the file, or any code inside `#[cfg(test)] mod tests`. The test module must continue to exercise the exact same behavior.

    4. If the `use` block at the top does not already import `Block` (needed for `Block::random`), it already should (verify with a quick `grep -n "use crate::block::Block" src/bcot.rs`). If for any reason it is not imported, add `use crate::block::Block;` to the existing `use` block. Do NOT remove any existing imports even if they appear unused after the edit — the test module below uses them.

    5. After the edits, `grep -n "k0_block" src/bcot.rs` must return no matches in the non-test portion of the file. It may still appear inside `#[cfg(test)] mod tests` if a test explicitly constructs a block with that variable name; do not touch such test code.
  </action>
  <verify>
    <automated>cargo build --lib 2>&amp;1 | tail -10 &amp;&amp; cargo test --lib bcot::tests 2>&amp;1 | tail -20 &amp;&amp; cargo test --lib 2>&amp;1 | tail -5</automated>
  </verify>
  <acceptance_criteria>
    - `grep -n "Key::new(Block::random(&mut self.rng))" src/bcot.rs` matches exactly two lines (one per transfer_* method)
    - `grep -c "k0_block.set_lsb(false);" src/bcot.rs` outputs `0`
    - `grep -c "Key::from(k0_block)" src/bcot.rs` outputs `0`
    - `grep -n "fn transfer_a_to_b" src/bcot.rs` still matches one line (signature preserved)
    - `grep -n "fn transfer_b_to_a" src/bcot.rs` still matches one line (signature preserved)
    - `grep -n "pub struct BcotOutput" src/bcot.rs` still matches one line (BcotOutput unchanged)
    - `cargo build --lib` exits 0
    - `cargo test --lib bcot::tests` exits 0 (all pre-existing bcot tests pass)
    - `cargo test --lib` exits 0 (full crate test suite green — no regressions)
  </acceptance_criteria>
  <done>Two three-line blocks replaced by one-liner `Key::new(Block::random(&mut self.rng))` each; all bcot tests and the full suite pass.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| IdealBCot sender ↔ IdealBCot receiver | The bCOT functionality produces (key, mac) pairs that must satisfy `mac = key.auth(b, delta)`; any LSB corruption here would silently break downstream IT-MAC verification |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-10 | Tampering | Mechanical rewrite could accidentally change the key/mac pair semantics if `Key::new` behaves differently from the manual `set_lsb(false) + Key::from` | mitigate | `Key::new` is defined as `mut block; block.set_lsb(false); Self(block)` — byte-identical to the old two-step sequence. Acceptance criteria include re-running the existing bcot test module which already asserts `mac == key.auth(b, delta)` for both `all_false` and `all_true` choice vectors. |
| T-01-11 | Repudiation | A partial migration (only one of two sites touched) would leave an invariant-preserving pattern the next phase might miss | mitigate | Acceptance criterion `grep -c "k0_block.set_lsb(false);"` outputs `0` enforces a complete sweep. Both transfer_* methods must be migrated. |
| T-01-12 | Denial of service | Full test suite regression | accept | `cargo test --lib` is part of the acceptance criteria. Any regression fails the task. |

No new high-severity threats. This plan strictly consolidates an existing invariant pattern — it cannot weaken security.
</threat_model>

<verification>
After Task 1:

```bash
cargo build --lib
cargo test --lib bcot::tests
cargo test --lib
```

All must exit 0. Additionally, confirm the migration is complete by grep:

```bash
# Old pattern must be gone
grep -n "k0_block" src/bcot.rs                     # should only match inside #[cfg(test)] if at all
grep -n "Key::from(k0_block)" src/bcot.rs          # expect 0 matches

# New pattern must appear exactly twice
grep -c "Key::new(Block::random(&mut self.rng))" src/bcot.rs   # expect 2
```
</verification>

<success_criteria>
- Both `transfer_a_to_b` and `transfer_b_to_a` use `Key::new(Block::random(&mut self.rng))` exactly once each
- The old `block.set_lsb(false); let k = Key::from(block)` pattern no longer appears anywhere in `src/bcot.rs` non-test code
- Every pre-existing bcot test (and the full crate test suite) passes unchanged
- `cargo build --lib` exits 0
</success_criteria>

<output>
After completion, create `.planning/phases/01-uncompressed-preprocessing/01-bcot-migration-SUMMARY.md`.
</output>
