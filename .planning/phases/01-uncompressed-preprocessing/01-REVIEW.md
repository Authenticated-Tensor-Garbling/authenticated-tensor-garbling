---
phase: 01-uncompressed-preprocessing
reviewed: 2026-04-21T00:00:00Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - benches/benchmarks.rs
  - src/aes.rs
  - src/auth_tensor_fpre.rs
  - src/auth_tensor_pre.rs
  - src/bcot.rs
  - src/keys.rs
  - src/leaky_tensor_pre.rs
  - src/lib.rs
  - src/matrix.rs
  - src/sharing.rs
  - src/tensor_ops.rs
  - src/tensor_pre.rs
findings:
  critical: 2
  warning: 3
  info: 4
  total: 9
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-21
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

Phase 1 was a cleanup pass covering Key-type invariants, visibility tightening, doc comments, and renames. The refactoring is largely sound: the `Key::new` constructor correctly enforces `lsb() == 0`, `AuthBitShare::Add` preserves the MAC invariant, the single-shared-`IdealBCot` pattern ensures all leaky triples carry the same deltas, and the cross-party delta-assertion in `combine_leaky_triples` is a valuable correctness guard.

Two critical issues remain: `bucket_size_for` panics with division-by-zero for n=1,m=1 (ell=1, log2=0), and `Key::BitXorAssign`/`Key::BitXor` do not re-enforce the `lsb==0` invariant after XOR, which can produce keys whose LSB is 1 when both operands happen to have LSB=1 (possible after `Key::adjust` with `adjust=true`, or after combining via `Add`). Three warnings cover a stale mutable benchmark variable that taints bandwidth accounting, a misleading comment in `random_zeros`, and a debug `println!` inside a test body that fires on every `cargo test`. Four info items cover dead benchmark functions, an unresolved TODO comment, unused `_setup_semihonest_*` functions, and a cosmetic `.clone()` on `Copy` types.

---

## Critical Issues

### CR-01: Division-by-zero panic in `bucket_size_for` when `n * m == 1`

**File:** `src/auth_tensor_pre.rs:15-21`

**Issue:** When `n=1, m=1` (or any pair where `n*m=1`), `ell=1` and `ell.leading_zeros()` is 63 on a 64-bit target. The expression `usize::BITS - ell.leading_zeros() - 1` evaluates to `64 - 63 - 1 = 0`, so `log2_ell = 0`, and `SSP / log2_ell` panics with integer division by zero. Similarly `n=0` or `m=0` gives `ell=0` with `leading_zeros()=64`, causing unsigned underflow (`64 - 64 - 1` wraps to `usize::MAX` in release, or panics in debug).

The minimum `BENCHMARK_PARAMS` entry is `(4, 4)` so tests never trigger this today, but the function has no guard and will be called in future batch/unit scenarios.

**Fix:**
```rust
pub fn bucket_size_for(n: usize, m: usize) -> usize {
    const SSP: usize = 40;
    let ell = n.checked_mul(m).expect("n*m overflow");
    assert!(ell >= 2, "bucket_size_for requires n*m >= 2 (got n={n}, m={m})");
    let log2_ell = (usize::BITS - ell.leading_zeros() - 1) as usize;
    SSP / log2_ell + 1
}
```

---

### CR-02: `Key::BitXor` and `Key::BitXorAssign` can silently produce a key with `lsb() == 1`

**File:** `src/keys.rs:170-182, 184-218`

**Issue:** The `Key` type documents the invariant `key.lsb() == 0`. The `BitXorAssign` and `BitXor` implementations XOR the inner `Block` fields directly without re-clearing the LSB:

```rust
impl BitXorAssign for Key {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;  // LSB = lsb(self) XOR lsb(rhs)
    }
}
```

When both keys have `lsb() == 0` this is fine (0 XOR 0 = 0). However, `Key::adjust` can temporarily produce an intermediate whose XOR with another adjusted key via `Add` (which delegates to XOR) could yield `lsb() == 1`. More directly: any code path that calls `Key::from(block)` (the zero-cost cast documented to NOT clear LSB — see `keys.rs:113-118`) followed by a XOR operation can produce an invalid key without a compile error.

The invariant is partially documented but not enforced by the XOR traits. A future caller combining two keys via `^=` after a `Key::from` bypass has no safety net.

**Fix:** Either document clearly in the `BitXor`/`BitXorAssign` impls that callers must only XOR keys that have previously enforced the invariant, or add a debug-mode assertion:

```rust
impl BitXorAssign for Key {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
        debug_assert!(!self.0.lsb(), "Key XOR produced lsb==1; invariant violated");
    }
}
```

Apply the same to all four `BitXor`/`BitXorAssign` variants. This catches misuse in tests and debug builds at zero release cost.

---

## Warnings

### WR-01: Benchmark bandwidth calculation uses a stale `total_bytes` captured before the inner `iter_batched` loop

**File:** `benches/benchmarks.rs:175-213` (and repeated for `chunking_factor` 2, 4, 6, 8)

**Issue:** `total_bytes` is computed once from a single `garble_first_half()` / `garble_second_half()` call made before `iter_batched`. The captured value is then used inside the async closure to annotate network traffic:

```rust
network.send_size_with_metrics(total_bytes).await;
```

Because `total_bytes` is a `usize` captured by move, it always reflects the **pre-loop** single-run measurement rather than the actual bytes produced in the current iteration. If garble output sizes are not deterministic (e.g., depend on internal mutable state), the bandwidth annotation will silently be wrong. Even if sizes are deterministic, the pattern is misleading — future maintainers may not realize the value comes from outside the timed loop.

**Fix:** Compute `total_bytes` inside `iter_batched` from the actual output, or assert that the pre-computed value matches:

```rust
|(mut generator, mut evaluator, network)| async move {
    let (first_levels_inner, first_cts_inner) = generator.garble_first_half();
    let (second_levels_inner, second_cts_inner) = generator.garble_second_half();
    generator.garble_final();

    let actual_bytes: usize =
        first_levels_inner.iter().map(|r| r.len() * 2 * block_sz).sum::<usize>()
        + first_cts_inner.iter().map(|r| r.len() * block_sz).sum::<usize>()
        + second_levels_inner.iter().map(|r| r.len() * 2 * block_sz).sum::<usize>()
        + second_cts_inner.iter().map(|r| r.len() * block_sz).sum::<usize>();

    network.send_size_with_metrics(actual_bytes).await;
    // ... evaluate ...
```

---

### WR-02: Misleading comment in `random_zeros` — says "last byte" but clears byte 0

**File:** `src/matrix.rs:148` and `src/matrix.rs:183`

**Issue:** Both `BlockMatrix::random_zeros` and `KeyMatrix::random_zeros` contain the comment:

```rust
bytes[0] &= 0xFE; // Clear last bit of last byte
```

`bytes[0]` is the **first** byte of the 16-byte array, not the last. The LSB of a `Block` is bit 0 of byte 0 (see `Block::lsb`: `(self.0[0] & 1) == 1`). The operation is correct — it does clear the LSB — but the comment is inverted, which is dangerous in a cryptographic codebase where the distinction between byte 0 and byte 15 matters for protocol correctness.

**Fix:**
```rust
bytes[0] &= 0xFE; // Clear LSB (bit 0 of byte 0) — encodes the zero label
```

---

### WR-03: Debug `println!` inside `test_auth_tensor_product` fires on every `cargo test`

**File:** `src/lib.rs:323-326` and `src/lib.rs:364, 375`

**Issue:** The integration test `test_auth_tensor_product` contains several `println!` and `print!` calls that emit output on every test run:

```rust
println!("gen_chunk_levels: {:?}", gen_chunk_levels.len());
println!("gen_chunk_levels[0] (each hold 2 blocks): {:?}", gen_chunk_levels[0].len());
println!("gen_chunk_cts: {:?}", gen_chunk_cts.len());
println!("gen_chunk_cts[0] (ecah hold one block): {:?}", gen_chunk_cts[0].len());
// ...
print!("{} ", expected_val);
// ...
println!();
```

Note the typo "ecah" at line 326. These were useful during development but produce noisy output in CI. Rust suppresses test output by default only when the test passes and `--nocapture` is absent; with `cargo test -- --nocapture` or on test failure they pollute the log.

**Fix:** Remove the `println!`/`print!` calls, or gate them behind `cfg(feature = "verbose-tests")`. Fix the typo regardless.

---

## Info

### IN-01: Two benchmark functions are defined but never registered in `criterion_group!`

**File:** `benches/benchmarks.rs:86-161` (`bench_full_protocol_garbling`) and `benches/benchmarks.rs:164-373` (`bench_full_protocol_with_networking`)

**Issue:** Both functions are fully implemented but absent from `criterion_group!(benches, ...)`. They will never execute. The compiler may warn about dead code, or may not (criterion benchmark functions are public). This makes it unclear whether they were intentionally removed or accidentally dropped during a refactor.

**Fix:** Either add them to `criterion_group!` or delete them. If they are kept for future use, add a `#[allow(dead_code)]` with a comment explaining the intent.

---

### IN-02: Two `_setup_semihonest_*` functions are prefixed with `_` to suppress unused warnings

**File:** `benches/benchmarks.rs:46-67`

**Issue:** `_setup_semihonest_gen` and `_setup_semihonest_eval` are defined with a leading underscore to suppress dead-code warnings. This is a code smell — if they are truly unused they should be removed; if they are needed for planned work they should be documented.

**Fix:** Remove both functions if they are not needed. If retained for future semi-honest benchmarks, add a comment and consider removing the underscore prefix once they are wired up.

---

### IN-03: Unresolved `TODO` comment in production code

**File:** `src/auth_tensor_fpre.rs:1`

**Issue:**
```rust
// TODO refactor authbit from fpre to a common module, or redefine with new name.
```

This is a structural refactoring note left in a production source file. It is reasonable for Phase 1, but should be tracked as a ticket rather than an inline comment to avoid it being forgotten.

**Fix:** File as a tracked issue and replace the comment with a reference, or resolve it in Phase 2 during the planned module restructuring.

---

### IN-04: `.clone()` called on `Copy` types in `auth_tensor_fpre.rs`

**File:** `src/auth_tensor_fpre.rs:144, 171`

**Issue:**
```rust
eval_label = gen_label.clone();
```

`Block` derives `Copy`, so `.clone()` is a no-op call that adds visual noise. The compiler will eliminate it, but idiomatic Rust uses direct assignment for `Copy` types.

**Fix:**
```rust
eval_label = gen_label;
```

Apply to both occurrences (lines 144 and 171).

---

_Reviewed: 2026-04-21_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
