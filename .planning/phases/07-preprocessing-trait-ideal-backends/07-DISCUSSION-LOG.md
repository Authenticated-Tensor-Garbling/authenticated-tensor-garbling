# Phase 7: Preprocessing Trait + Ideal Backends - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-23
**Phase:** 07-preprocessing-trait-ideal-backends
**Areas discussed:** Trait location + wrappers, PRE-04 field design, IdealPreprocessingBackend internals, Compressed backend shape (deferred)

---

## Trait location + wrappers

*(Session 1 — recovered from checkpoint)*

| Option | Description | Selected |
|--------|-------------|----------|
| In `preprocessing.rs` | Alongside TensorFpreGen/Eval and run_preprocessing | ✓ |
| New `src/tensor_preprocessing.rs` | Separate file | |
| In `auth_tensor_fpre.rs` | Alongside TensorFpre | |

**User's choice:** `preprocessing.rs`
**Notes:** Module already owns the preprocessing boundary; no new file needed.

---

| Option | Description | Selected |
|--------|-------------|----------|
| `UncompressedPreprocessingBackend` | Explicitly signals "uncompressed" as a property | ✓ |
| `PiATensorBackend` | Protocol-name wrapper | |
| `StandardPreprocessingBackend` | Generic label | |

**User's choice:** `UncompressedPreprocessingBackend`

---

## PRE-04 field design

*(Session 1 — recovered from checkpoint)*

| Question | Answer |
|----------|--------|
| Symmetric layout (same field on both structs)? | Yes — symmetric, strict MAC correctness |
| What is [l_w Delta_ev]? | (l^ev_w * Delta_ev) + (K[l^gb_w] + l^gb_w * Delta_ev). Open on D_ev shares = Ev uses key to verify and learn Gb's share |
| How many fields for l_gamma? | ONE field — gamma_auth_bit_shares and output_mask_auth_bit_shares collapse to one |
| Field name? | `gamma_auth_bit_shares` |
| Field length? | n*m per triple (same as correlated_auth_bit_shares) |
| REQUIREMENTS.md note | PRE-04 says "l_gamma*" but should say "l_gamma". correlated_auth_bit_shares already encodes l_gamma*. The two-field split in REQUIREMENTS.md collapses to one. |

---

## IdealPreprocessingBackend internals

*(Session 2)*

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed seed=0 (zero-field struct) | Matches IdealBCot pattern | ✓ |
| Caller-supplied seed | Adds a field | |

**User's choice:** Fixed seed=0 — zero-field unit struct.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Recompute alpha AND beta locally | Paper trusted-dealer semantics | ✓ |
| Set gamma = 0 (stub) | Simple but breaks consistency check | |

**User's choice:** Recompute alpha AND beta locally — l_gamma is the authenticated gate output mask.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Delegate to TensorFpre | Reuse proven logic | ✓ |
| Duplicate inline | Second copy to maintain | |

**User's choice:** Delegate — create TensorFpre internally, call generate_for_ideal_trusted_dealer() + into_gen_eval(), then append gamma_auth_bit_shares.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Separate random auth bit (l_gamma) | Independent wire mask, distinct from correlated | ✓ |
| Same as correlated (alpha AND beta) | Conflates two distinct fields | |

**User's choice:** Separate random authenticated bit per (i,j). l_gamma ≠ l_gamma*.

---

## Compressed backend shape

*(Session 2 — area deferred)*

User decided to defer `IdealCompressedPreprocessingBackend` (PRE-05) to v3.
No implementation decisions made. See Deferred section of CONTEXT.md.

---

## Claude's Discretion

- Rust trait form (associated fn vs `&self` method) for zero-field structs
- `count > 1` handling in `UncompressedPreprocessingBackend` (retain existing panic)

## Deferred Ideas

- PRE-05 / IdealCompressedPreprocessingBackend — deferred to v3. The M·b* compressed mask derivation and sigma parameter design were discussed but not decided; user chose to defer entirely.
