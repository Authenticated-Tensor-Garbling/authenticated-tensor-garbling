# Phase 1: M1 Primitives & Sharing Cleanup - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Refactor the stable primitive layer — `block.rs`, `delta.rs`, `keys.rs`, `macs.rs`, `aes.rs`, `sharing.rs`, `matrix.rs`, `tensor_ops.rs` — to enforce type-level invariants, fix naming ambiguities, and add doc comments on non-obvious behavior. **Zero algorithmic changes.** `cargo build` and the full test suite must pass unchanged after cleanup.

Requirements in scope: CLEAN-01, CLEAN-02, CLEAN-03, CLEAN-04, CLEAN-05, CLEAN-06.

Out of scope: any changes to protocol logic, online garbling, ideal TensorFpre, or benchmarks (those are Phase 2).

</domain>

<decisions>
## Implementation Decisions

### Key LSB=0 Invariant (CLEAN-01 + CLEAN-04)

- **D-01:** Add `Key::new(mut block: Block) -> Self` that calls `block.set_lsb(false)` before constructing. This is the canonical safe constructor.
- **D-02:** `From<Block> for Key` stays as a zero-cost escape hatch (no hidden operation). Callers that have already cleared the LSB may use it; callers constructing from uncleared material must use `Key::new()`.
- **D-03:** Update `Key::random()` to call `Key::new(Block::random(rng))` — ensures all random keys satisfy the invariant.
- **D-04:** Update `build_share` in `sharing.rs` to use `Key::new(Block::random(rng))` instead of `Key::from(rng.random::<[u8;16]>())`.
- **D-05:** Update all existing callers that do `block.set_lsb(false); Key::from(block)` (e.g., in `bcot.rs`, `leaky_tensor_pre.rs`, `auth_tensor_fpre.rs`) to use `Key::new(block)` instead — eliminate the two-step manual pattern.

### AuthBitShare vs AuthBit Disambiguation (CLEAN-02)

- **D-06:** Add `///` doc comments to `AuthBitShare` explaining it holds **one party's** view: sender's OT key (LSB=0), receiver's IT-MAC (authenticated under the other party's delta), and the committed bit. No field renames.
- **D-07:** Add `///` doc comments to `AuthBit` explaining it holds **both parties' views** — the full two-party representation needed for testing and trusted-dealer construction.
- **D-08:** Add a doc comment to `build_share(rng, bit, delta)` explaining that `delta` is the **verifying party's** global correlation key. No signature or name change.

### InputSharing.bit() Rename (CLEAN-03)

- **D-09:** Rename `InputSharing.bit()` to `InputSharing.shares_differ()`. The method returns `gen_share != eval_share` — the XOR of the two label blocks — not the underlying input bit. Update all 4 call sites in `auth_tensor_fpre.rs` (lines 235, 239) and `tensor_pre.rs` (lines 109, 119).

### tensor_ops / matrix Visibility (CLEAN-05)

- **D-10:** Change `gen_populate_seeds_mem_optimized` and `gen_unary_outer_product` in `tensor_ops.rs` from `pub` to `pub(crate)`. They are only imported within the crate (`tensor_gen.rs`, `auth_tensor_gen.rs`).
- **D-11:** Change `MatrixViewRef`, `MatrixViewMut`, and their `flat_index` helper to `pub(crate)` in `matrix.rs`. Add a doc comment on `TypedMatrix` (or its `flat_index` method) documenting the column-major indexing: `index = j * rows + i` where `j` is the column index and `i` is the row index.

### aes.rs Singleton (CLEAN-06)

- **D-12 (Claude's Discretion):** Add a doc comment on `FIXED_KEY_AES: Lazy<FixedKeyAes>` explaining why `once_cell::sync::Lazy` is used (lazy initialization on first use, `Send + Sync` guarantees thread safety across all callers) and that the fixed key is a protocol constant, not a secret.

### Claude's Discretion

- aes.rs singleton documentation (CLEAN-06): content and placement of the thread-safety note is left to the implementer.
- Column-major indexing doc placement in matrix.rs: add to struct-level doc, `flat_index` comment, or both — implementer's call.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Requirements
- `.planning/ROADMAP.md` — Phase 1 goal, success criteria, requirements list (CLEAN-01 to CLEAN-06)
- `.planning/REQUIREMENTS.md` — Full v1 requirements; M1 section defines exactly what each CLEAN-XX requires

### Source Files in Scope
- `src/keys.rs` — Key type, From impls, Key::random(), Key::adjust()
- `src/sharing.rs` — AuthBitShare, AuthBit, InputSharing, build_share
- `src/tensor_ops.rs` — gen_populate_seeds_mem_optimized, gen_unary_outer_product
- `src/matrix.rs` — TypedMatrix, MatrixViewRef, MatrixViewMut, flat_index
- `src/aes.rs` — FIXED_KEY_AES Lazy singleton

### Call Sites to Update
- `src/bcot.rs` — manual set_lsb(false) + Key::from() pattern → Key::new()
- `src/leaky_tensor_pre.rs` — same pattern
- `src/auth_tensor_fpre.rs` — same pattern; also uses InputSharing.bit() → shares_differ()
- `src/tensor_pre.rs` — uses InputSharing.bit() → shares_differ()

### Note on Stale Research File
- `.planning/phases/01-uncompressed-preprocessing/01-RESEARCH.md` — Stale. Describes the old (April 19) Phase 1 which was a protocol implementation task. **Do not use as spec for this phase.** ROADMAP.md is authoritative.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Key::adjust()` in `src/keys.rs` already calls `set_lsb(false)` — the correct pattern exists, just needs a `Key::new()` wrapper to generalize it.
- `Delta::new()` already sets LSB=1 at construction time — the exact pattern we want to replicate for `Key`.
- `AuthBitShare::verify(delta)` is the canonical MAC check — its doc comment already explains the invariant; extend this pattern to the struct-level doc.

### Established Patterns
- Column-major indexing: `flat_index` at `src/matrix.rs:56` computes `j*rows + i` — undocumented at the type level; needs one line added.
- `once_cell::sync::Lazy` pattern: used in `src/aes.rs:15` — thread-safe singleton, same pattern as common Rust idiom.
- `pub(crate)` visibility: `Mac::new` and `MAC_ZERO`/`MAC_ONE` in `src/macs.rs` are already `pub(crate)` — follow the same convention for tensor_ops and matrix view types.

### Integration Points
- Changing `InputSharing.bit()` → `shares_differ()` touches `auth_tensor_fpre.rs` and `tensor_pre.rs`; these are Phase 2 cleanup targets but this rename is Phase 1's responsibility.
- `Key::new()` addition does not change the `From<Block>` impl — no downstream breakage in bcot/leaky/fpre call sites that already cleared LSB.

</code_context>

<specifics>
## Specific Ideas

- User explicitly chose `Key::new()` over auto-clearing `From<Block>` to preserve the zero-cost cast semantics of `From`. This distinction matters: `From<Block>` remains valid for callers that have already cleared LSB (like the `Key::adjust()` path).
- User chose `shares_differ()` rename over a doc-only fix — permanent removal of the naming ambiguity.
- User chose `pub(crate)` for internal tensor_ops / matrix items — signals these are implementation details, not the library's public API.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 1 scope.

</deferred>

---

*Phase: 01-uncompressed-preprocessing*
*Context gathered: 2026-04-21*
