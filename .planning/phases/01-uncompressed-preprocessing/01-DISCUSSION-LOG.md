# Phase 1: M1 Primitives & Sharing Cleanup - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 01-uncompressed-preprocessing
**Areas discussed:** Key LSB=0 enforcement, AuthBitShare vs AuthBit, InputSharing.bit() rename, tensor_ops / matrix API scope

---

## Key LSB=0 enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Key::new() clears LSB | Add Key::new(block) that calls set_lsb(false). From<Block> stays zero-cost. Key::random() and build_share updated to use Key::new(). | ✓ |
| From impls auto-clear LSB | Change From<Block> and From<[u8;16]> to always clear LSB. Simpler for callers, hidden cost on already-cleared blocks. | |
| Panic on LSB=1 | Add debug_assert in From impls. Catches violations in tests, not at compile time. | |

**User's choice:** Key::new() clears LSB

---

| Option | Description | Selected |
|--------|-------------|----------|
| Update to Key::new() | Replace all set_lsb(false) + Key::from() patterns at existing call sites (bcot.rs, leaky_tensor_pre.rs, auth_tensor_fpre.rs). | ✓ |
| Leave existing callers alone | Only fix build_share and Key::random(). Existing two-step pattern stays. | |

**User's choice:** Update to Key::new() at all existing call sites.

---

## AuthBitShare vs AuthBit

| Option | Description | Selected |
|--------|-------------|----------|
| Doc comments only | Add /// to AuthBitShare (one party's view) and AuthBit (both parties' views). No renames. | ✓ |
| Rename AuthBit fields | Rename gen_share/eval_share in AuthBit to party_a/party_b or similar. | |
| Both: docs + field renames | Add docs AND rename fields. | |

**User's choice:** Doc comments only — crypto notation (key/mac/value) stays short.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Keep signature, add doc comment | Add doc comment to build_share explaining delta is the verifying party's global key. | ✓ |
| Rename to build_share_for_verifier | Rename to make delta role explicit. Requires updating 3–4 call sites. | |

**User's choice:** Keep signature, add doc comment.

---

## InputSharing.bit() rename

| Option | Description | Selected |
|--------|-------------|----------|
| Rename to shares_differ() | Precisely describes the method: returns gen_share != eval_share. Update 4 call sites. | ✓ |
| Rename to secret_bit() | Less ambiguous than bit(), still slightly misleading. | |
| Keep bit(), add doc comment | No rename; confusion stays in the name. | |

**User's choice:** Rename to shares_differ() — permanent fix over documentation.

---

## tensor_ops / matrix API scope

| Option | Description | Selected |
|--------|-------------|----------|
| pub(crate) for internal-only items | Change gen_populate_seeds_mem_optimized, gen_unary_outer_product, MatrixViewRef, MatrixViewMut, flat_index to pub(crate). | ✓ |
| Keep all pub, just add docs | Leave visibility unchanged; only add documentation. | |

**User's choice:** pub(crate) for all internal-only tensor_ops and matrix items.

---

## Claude's Discretion

- aes.rs FIXED_KEY_AES doc comment content and placement
- Column-major indexing doc placement in matrix.rs (struct-level vs. flat_index comment)

## Deferred Ideas

None.
