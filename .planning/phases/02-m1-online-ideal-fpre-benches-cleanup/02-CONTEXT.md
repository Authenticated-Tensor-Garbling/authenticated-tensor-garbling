# Phase 2: M1 Online + Ideal Fpre + Benches Cleanup - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Refactor `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, `auth_tensor_fpre.rs`, and `benches/benchmarks.rs` — separate ideal `TensorFpre` (trusted dealer) from real-protocol output structs (`TensorFpreGen`/`TensorFpreEval`), remove gamma dead code end-to-end, document all fields, and deduplicate benchmark setup. **Zero algorithmic changes.** `cargo build`, full test suite, and `cargo bench` must pass after cleanup.

Requirements in scope: CLEAN-07, CLEAN-08, CLEAN-09, CLEAN-10, CLEAN-11, CLEAN-12.

Out of scope: any online garbling algorithm changes, protocol logic, Phase 3-6 preprocessing rewrite.

</domain>

<decisions>
## Implementation Decisions

### Module Separation (CLEAN-08)

- **D-01:** Create `src/preprocessing.rs`. Move `TensorFpreGen` and `TensorFpreEval` from `auth_tensor_fpre.rs` into it.
- **D-02:** Move `run_preprocessing()` to `src/preprocessing.rs` — it is the real-protocol entry point, not ideal trusted-dealer logic, and belongs alongside the structs it returns.
- **D-03:** `auth_tensor_fpre.rs` becomes exclusively the ideal `TensorFpre` trusted dealer: `TensorFpre`, `TensorFpre::new`, `TensorFpre::new_with_delta`, `gen_auth_bit`, `generate_for_ideal_trusted_dealer` (renamed from `generate_with_input_values`), `get_clear_values`, `into_gen_eval`.
- **D-04:** Callers import directly from `crate::preprocessing` — no re-export from `auth_tensor_fpre`. Update all import paths in `auth_tensor_gen.rs`, `auth_tensor_eval.rs`, and `benches/benchmarks.rs`.
- **D-05:** Add `pub mod preprocessing;` to `lib.rs`.

### TensorFpre Rename + Doc (CLEAN-07)

- **D-06:** Rename `TensorFpre::generate_with_input_values` → `TensorFpre::generate_for_ideal_trusted_dealer`. Add a doc comment: "Generates all authenticated bits and input sharings for the ideal trusted dealer. This is NOT the real preprocessing protocol — it is the ideal functionality (trusted dealer) that the online phase consumes directly in tests and benchmarks."

### Gamma Dead Code Removal (CLEAN-10 + cascading cleanup)

Gamma bits are computed throughout the stack but never used in the online phase: `garble_final()` computes `_gamma_share` and discards it; `evaluate_final()` ignores `gamma_auth_bit_shares` entirely. Phase 3-6 rewrites `garble_final` from the paper. Remove gamma end-to-end:

- **D-07:** Remove `_gamma_share` computation from `AuthTensorGen::garble_final()`.
- **D-08:** Remove `gamma_auth_bit_shares: Vec<AuthBitShare>` field from `AuthTensorGen` and `AuthTensorEval`.
- **D-09:** Remove `gamma_auth_bit_shares: Vec<AuthBitShare>` field from `TensorFpreGen` and `TensorFpreEval` (they no longer carry gamma into the online phase).
- **D-10:** Remove `gamma_auth_bits: Vec<AuthBit>` field from `TensorFpre` and the gamma generation loop inside `generate_for_ideal_trusted_dealer`.
- **D-11:** Remove or update test assertions that reference `gamma_auth_bits` (e.g., `test_tensor_fpre_auth_bits` checks `fpre.gamma_auth_bits.len()`).

### TensorFpreGen / TensorFpreEval Field Docs (CLEAN-09)

- **D-12:** Add `///` doc comments to every field of `TensorFpreGen` and `TensorFpreEval` in `preprocessing.rs`, specifying which party holds it and what it represents. Example: `/// Garbler's (Party A) global correlation key. LSB is always 1.` and `/// Garbler's share of the authenticated bit for input wire i. Committed under delta_b.`

### auth_tensor_gen / auth_tensor_eval Audit (CLEAN-10)

- **D-13:** Remove the `// awful return type` comment on `gen_chunked_half_outer_product` — either fix the return type or leave without the self-deprecating comment.
- **D-14:** Add a doc comment to `garble_final()` and `evaluate_final()` explaining the protocol step: "Combines both half-outer-product outputs with the correlated preprocessing share to produce the garbled tensor gate output."
- **D-15:** Name the magic `Block::from(0 as u128)` and `Block::from(1 as u128)` tweaks in `eval_populate_seeds_mem_optimized` — they are GGM tree traversal direction constants (0 = left child, 1 = right child). Add a one-line comment.

### auth_gen.rs / auth_eval.rs (CLEAN-11)

- **D-16:** Confirmed: `src/auth_gen.rs` and `src/auth_eval.rs` do not exist — CLEAN-11 is trivially satisfied. No action needed; note in plan.

### Benchmark Deduplication (CLEAN-12)

- **D-17:** Replace the 5 near-identical chunking-factor blocks in `bench_full_protocol_garbling` with a loop: `for cf in [1usize, 2, 4, 6, 8] { ... }`. Benchmark IDs stay the same (`BenchmarkId::new(cf.to_string(), ...)`).
- **D-18:** Add paper protocol header comment to each benchmark group: `// Benchmarks online garbling for authenticated tensor gate (auth_tensor_gen / auth_tensor_eval)`.
- **D-19:** Update `setup_auth_gen` and `setup_auth_eval` helper calls from `generate_with_input_values` to `generate_for_ideal_trusted_dealer` (rename follow-through from D-06).

### Claude's Discretion

- Exact wording of per-field doc comments on `TensorFpreGen`/`TensorFpreEval` (D-12) — content must be accurate, style is implementer's call.
- Whether to rename or leave the `gen_chunked_half_outer_product` return type (D-13) — if renaming is a clean one-liner, do it; otherwise just remove the comment.
- Exact placement of the GGM tweak comments (D-15) — inline or above the `Block::from(...)` lines.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Requirements
- `.planning/ROADMAP.md` — Phase 2 goal, success criteria, requirements (CLEAN-07 to CLEAN-12)
- `.planning/REQUIREMENTS.md` — Full v1 requirements; M1 section defines each CLEAN-XX precisely

### Source Files in Scope
- `src/auth_tensor_fpre.rs` — TensorFpre, TensorFpreGen, TensorFpreEval, generate_with_input_values, run_preprocessing (all to be reorganized)
- `src/auth_tensor_gen.rs` — AuthTensorGen, garble_final (gamma removal, doc cleanup)
- `src/auth_tensor_eval.rs` — AuthTensorEval, evaluate_final (gamma removal)
- `benches/benchmarks.rs` — setup helpers and repeated chunking-factor blocks (deduplication target)

### Upstream Context (Phase 1 decisions carry through)
- `.planning/phases/01-uncompressed-preprocessing/01-CONTEXT.md` — Key::new pattern, pub(crate) convention, shares_differ rename — all already executed

### Protocol Background
- `references/appendix_krrw_pre.tex` — paper spec; confirms TensorFpre is the ideal functionality, online garbling is auth_tensor_gen/eval

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `TensorFpre::into_gen_eval()` stays in `auth_tensor_fpre.rs` but returns types from `preprocessing.rs` — cross-module return type, no issue in Rust.
- `run_preprocessing()` already imports `combine_leaky_triples` and `bucket_size_for` from `auth_tensor_pre` — those imports move with it to `preprocessing.rs`.

### Established Patterns
- `pub(crate)` for internal items (from Phase 1) — apply to anything in `preprocessing.rs` not used outside the crate if applicable.
- `once_cell::sync::Lazy` pattern already documented in `aes.rs` — no change needed.

### Integration Points
- `auth_tensor_gen.rs` imports `TensorFpreGen` from `auth_tensor_fpre` → changes to `preprocessing`.
- `auth_tensor_eval.rs` imports `TensorFpreEval` from `auth_tensor_fpre` → changes to `preprocessing`.
- `benches/benchmarks.rs` imports `TensorFpre` and `run_preprocessing` from `auth_tensor_fpre` → `TensorFpre` stays, `run_preprocessing` moves to `preprocessing`.
- `lib.rs` needs `pub mod preprocessing;` added.

</code_context>

<specifics>
## Specific Ideas

- User explicitly chose `src/preprocessing.rs` (new file) over inline `mod preprocessing` — clean physical separation.
- User chose direct imports from `crate::preprocessing` with no re-exports — no hidden indirection in `auth_tensor_fpre`.
- User explicitly chose to remove gamma end-to-end now, not defer to Phase 4 — gamma in the ideal `TensorFpre` path is dead code (never consumed in online phase), and Phase 3-6 will rebuild `garble_final` from the paper spec anyway.
- Benchmark deduplication: loop over `[1, 2, 4, 6, 8]` chunking factors — user confirmed data-driven approach.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 2 scope.

</deferred>

---

*Phase: 02-m1-online-ideal-fpre-benches-cleanup*
*Context gathered: 2026-04-21*
