# Track 2 Audit Scaffolding

Single entry point for the four per-surface paper audits. **Built fresh against post-§1.2 codebase** — do **not** consult the legacy `.planning/phases/*/PATTERNS.md` documents as a current paper↔code map. Those were written 2026-04-23 (pre-§1.2) and reference removed entities (`gamma_auth_bit_shares` on `AuthTensorGen`, `compute_lambda_gamma`, `extbit`, the LSB-only CheckZero glue layer). Treat them as historical context only.

## Audit format

Each audit produces `.planning/audit/AUDIT-<surface>.md` with four sections:

- **(a) Matches** — paper step ↔ code step, variable to variable, paper line numbers ↔ source line numbers.
- **(b) Deviations** — anywhere code diverges from the paper, with line numbers and rationale (intentional simulation? bug? fixed-and-superseded?).
- **(c) Latent assumptions** — invariants the code expects but does not enforce (e.g., precondition on caller, undocumented invariant between fields).
- **(d) Required code changes** — concrete fixes that fall out of (b)/(c). Listed as numbered items; queued as separate confirm-edit-test-confirm-commit cycles **after** the AUDIT doc is committed (not in the same commit).

## Interaction model

Per the master plan (`/Users/turan/.claude/plans/concerns-md-include-many-possible-velvety-music.md` Track 2):

1. Read code + paper section + cross-references.
2. Draft AUDIT doc section by section, **surfacing each (b) and (c) finding inline as it's identified** — do not batch up findings. User confirms or rejects each before it lands in the doc.
3. Audit doc finalized + committed.
4. (d) code changes queued as a follow-up sub-phase (separate atomic commits, one per fix).

## Audit order (recommended)

**2.1 → 2.2 → 2.3 → 2.4.** Construction 1 (the GGM-tree primitive) is structurally upstream of both protocols and the bucketing pipeline. Doing it first lets later audits cite its findings instead of duplicating analysis.

---

## Surface 2.1 — Construction 1 (Generalized tensor macro / GGM tree)

**Paper:**
- Primary: `appendix_krrw_pre.tex:88` `cons:gen-tensor-macro` — generalized n×m tensor macros (used by preprocessing).
- Cross-ref: `5_online.tex:39` `cons:tensor-macro` — simpler form used by Protocol 1. Same algorithm, narrower parameterization.
- Original source: `references/2018-578-3.pdf` (KRRW18) — useful historical comparison; the paper's Construction 1 is a reformulation.

**Source (current):**
- `src/tensor_macro.rs` — `tensor_garbler`, `tensor_evaluator` (the named macros used in tests/benches).
- `src/tensor_ops.rs` — narrow GGM helpers: `gen_populate_seeds_mem_optimized`, `gen_unary_outer_product`, `eval_populate_seeds_mem_optimized`, `eval_unary_outer_product`. Wide-variant helpers (`gen_unary_outer_product_wide`, `eval_unary_outer_product_wide`) live here too but are P2-specific — audit those in 2.4.
- Block-storage primitive: `src/block.rs` — Block::lsb() (GGM pointer-bit semantics).

**Key entities to verify:**
- GGM tree endianness: `src/tensor_ops.rs` comments document index 0 = LSB, n-1 = MSB.
- Leaf-product accumulation: `Z_garbler XOR Z_evaluator == a ⊗ T` (Theorem 1 in the paper).
- Block::ZERO vs `(0u128).to_be_bytes()` deviation noted as PERF-02 / WR-09 in CONCERNS.md.

**Existing test coverage:**
- `src/tensor_macro.rs` `mod tests` — 10-test battery covering n1_m1, n2_m1, n1_m4, n2_m3, n4_m4, n4_m8, n8_m1, n4_m64, n8_m16, deterministic_seed_42.
- `src/tensor_ops.rs` `mod tests` — wide-signature shape + round-trip tests (κ-half + ρ-half).

---

## Surface 2.2 — Pi_LeakyTensor + bucketing (Constructions 2/3/4)

**Paper:**
- Primary:
  - `appendix_krrw_pre.tex:246` `prot:ltensor` — Π_LeakyTensor protocol.
  - `appendix_krrw_pre.tex:387` `prot:pre` — Π_pre (bucketing → authenticated triples).
- Bucketing: §3.1 (correct combining, two-to-one) and §3.2 (permutation bucketing, asymmetric σ ∈ S_n on x-space).
- Original source: `references/2017-030-2.pdf` (WRK17, leaky AND triples + bucketing).

**Source (current):**
- `src/leaky_tensor_pre.rs` — `LeakyTensorPre` struct, `generate()` (full triple from F_BCot), `LeakyTriple` shape.
- `src/auth_tensor_pre.rs` — `two_to_one_combine` (paper §3.1 four-step combiner), `bucket_size_for(n, ell) = 1 + ceil(SSP/log2(n*ell))`, `combine_leaky_triples` (full bucketing pipeline including Fisher-Yates permutation), `apply_permutation_to_triple` (asymmetric x-axis permutation).
- `src/feq.rs` — F_eq ideal-functionality stub (panic-on-mismatch); single-process simulation of the paper's F_eq abort.
- `src/sharing.rs::verify_cross_party` — in-process substitute for paper's "publicly reveal with appropriate MACs". Production code path, called from `two_to_one_combine`.

**Key entities to verify:**
- **Construction 2 (F_eq):** `src/feq.rs::check`'s panic-on-mismatch is the paper-equivalent ideal-functionality model (acceptable for in-process simulation; document explicitly, do NOT replace with cryptographic implementation in this audit).
- **Construction 3 (two_to_one_combine):** four steps — `d := y' XOR y''`, verify d via `verify_cross_party`, `x := x' XOR x''`, `Z := Z' XOR Z'' XOR (x'' ⊗ d)`. Match algebraically to paper §3.1.
- **Construction 4 (bucketing):** `bucket_size_for(n, ell)` formula matches paper. SSP=40 hardcoded — verify it threads through all bucket-size sites consistently. Asymmetric permutation (x rows + Z i-indices permuted; y untouched) matches paper §3.2 σ ∈ S_n action on x-space only. Hardcoded `shuffle_seed=42` in `combine_leaky_triples` flagged for SEC-05 (Phase 3.3 hardening, not this audit).
- δ_a / δ_b LSB invariants (see "Cross-cutting facts" below).

**Existing test coverage:**
- `src/leaky_tensor_pre.rs` `mod tests` — `test_leaky_triple_product_invariant`, `test_leaky_triple_mac_invariants`, `test_macro_outputs_xor_invariant`, `test_c_a_c_b_xor_invariant`, `test_correlated_randomness_dimensions`, `test_feq_passes_on_honest_run`, `test_leaky_triple_shape_field_access`, `test_key_lsb_zero_all_shares`.
- `src/auth_tensor_pre.rs` `mod tests` — `test_two_to_one_combine_product_invariant`, `test_combine_full_bucket_product_invariant`, `test_bucket_size_formula`, `test_bucket_size_formula_edge_cases`, `test_full_pipeline_no_panic`, `test_run_preprocessing_product_invariant_construction_4`.

---

## Surface 2.3 — Protocol 1 (single-delta authenticated tensor product)

**Paper:**
- Primary:
  - `5_online.tex:145` `cons:krrw-algo` — Garbling and Evaluation Algorithms for Protocol 1.
  - `5_online.tex:193` `prot:krrw` — Protocol 1 itself (input encoding, online flow, consistency check).
- Tensor macro definition: `5_online.tex:39` `cons:tensor-macro` (audit 2.1 covers this; just cite from here).
- Correctness: `lem:protocol1-correctness` line 297.
- CheckZero: `5_online.tex:226–247` (consistency check on input wires under D_ev).

**Source (current):**
- Garbler: `src/auth_tensor_gen.rs` — `AuthTensorGen` struct (Block-form fields), `get_first_inputs`, `get_second_inputs`, `garble_first_half`, `garble_second_half`, `garble_final`. Post-§1.2 paper-aligned (A.3 closed): first-half = `(x⊕α) ⊗ β`, second-half = `(y⊕β) ⊗ x`.
- Evaluator: `src/auth_tensor_eval.rs` — mirror.
- Input encoding: `src/input_encoding.rs::encode_inputs` — populates `x_gen` / `y_gen` / `masked_x_gen` / `masked_y_gen` / `masked_x_bits` / `masked_y_bits` per paper §211–215. Sits between preprocessing and garbling.
- CheckZero: `src/online.rs::block_check_zero` (full-block per-index equality, NOT LSB-only) + `block_hash_check_zero` (paper-faithful `H({V_w})` digest).
- Input-wire check helper: `src/lib.rs::assemble_e_input_wire_blocks_p1` — emits per-party block vectors for the consistency check.

**Key entities to verify:**
- Half-gate decomposition matches paper Construction 3 (already paper-aligned post-A.3).
- Column-major indexing (`j*n + i`) consistent across `garble_final` (auth_tensor_gen.rs) and `evaluate_final` (auth_tensor_eval.rs).
- CheckZero structural properties: `[e_a D_ev] := [a D_ev] ⊕ [λ_a D_ev] ⊕ (a ⊕ λ_a) · D_ev` reconstructs to zero for honest parties.
- δ_a / δ_b LSB invariants (see "Cross-cutting facts" below).

**Existing test coverage:**
- `src/lib.rs::tests` — `test_auth_tensor_product`, `run_full_protocol_1` (parameterized over (x, y)), `test_full_protocol_1_nonzero_inputs_{ideal,uncompressed}` (regression for x=0b1011, y=0b101), tamper tests for δ_a-XOR (LSB=1) and δ_b-XOR (LSB=0).
- `src/online.rs::tests` — 6-test battery covering `block_check_zero` per-index equality, length mismatch, empty slice, hash digest equality + inequality.

---

## Surface 2.4 — Protocol 2 (wide-leaf with D_ev MAC propagation)

**Paper:**
- Primary:
  - `6_total.tex:25` `cons:auth-tensor-macro` — Authenticated tensor macros (extend `tensorgb`/`tensorev` with D_ev MACs).
  - `6_total.tex:119` `cons:wrk-algo` — Garbling and Evaluation Algorithms for Protocol 2.
  - `6_total.tex:182` `prot:wrk` — Protocol 2 itself.
  - `6_total.tex:89` — wide-leaf XOR-share definition (κ + ρ).
- CheckZero: `6_total.tex:215–222` (consistency check c_α / c_β under D_ev).
- Wide-domain ciphertext shape: `6_total.tex:90` (per-row `KAPPA_BYTES + RHO_BYTES`).

**Source (current):**
- Garbler: `src/auth_tensor_gen.rs::*_p2` — `garble_first_half_p2`, `garble_second_half_p2`, `garble_final_p2` (returns `(Vec<Block>, Vec<Block>)` D_gb + D_ev — privacy-enforcing static return type, no masked wire value).
- Evaluator: `src/auth_tensor_eval.rs::*_p2` — `evaluate_first_half_p2`, `evaluate_second_half_p2`, `evaluate_final_p2` (returns `Vec<Block>` D_ev).
- Wide GGM: `src/tensor_ops.rs` — `gen_unary_outer_product_wide`, `eval_unary_outer_product_wide` (κ-half + ρ-half via `WIDE_DOMAIN`-separated tweaks).
- Wide chunked helpers: `src/auth_tensor_gen.rs::gen_chunked_half_outer_product_wide`, `src/auth_tensor_eval.rs::eval_chunked_half_outer_product_wide`.
- Input-wire check helper: `src/lib.rs::assemble_c_alpha_beta_blocks_p2` (alias of P1 helper for now — verify they remain algebraically identical per `5_online.tex:242–246` ↔ `6_total.tex:215–222`).
- `get_first_inputs_p2_y_d_ev` / `get_second_inputs_p2_y_d_ev` on both Online structs — D_ev side of the wide-tensor inputs (post-A.3 paper-aligned: first-half emits β under δ_b, second-half emits x under δ_b reconstructed via `alpha_eval ⊕ masked_x_bits·δ_b` per paper §211).

**Key entities to verify:**
- WIDE_DOMAIN tweak separation gives independent κ-half / ρ-half outputs (`tensor_ops.rs:275–285` documented this).
- Garbler emits `mac.as_block()` directly under D_ev (does NOT hold δ_b); evaluator XORs δ_b on its key view to reconstruct IT-MAC pair.
- Static return-type guarantee on `garble_final_p2` enforces P2 privacy: no path leaks the masked wire value.
- D_b output of P2 reconstructs to `x ⊗ y · δ_b` post-A.3 (verified algebraically; not directly asserted by `run_full_protocol_2` — only D_a output and CheckZero are asserted there. Confirm whether to add a D_b output assertion as a (d) item).

**Existing test coverage:**
- `src/lib.rs::tests` — `run_full_protocol_2`, `test_auth_tensor_product_full_protocol_2_{ideal,uncompressed}`, `test_full_protocol_2_nonzero_inputs_{ideal,uncompressed}`.
- `src/auth_tensor_gen.rs::tests` — `test_garble_first_half_p2_returns_wide_ciphertexts`.
- `src/auth_tensor_eval.rs::tests` — wide round-trip tests.

---

## Cross-cutting facts (load-bearing across all four audits)

These are documented in detail in `~/.claude/projects/-Users-turan-Desktop-authenticated-tensor-garbling/memory/project_phase_1_2_progress.md`. Brief versions for audit cross-reference:

### Δ LSB convention
| Delta | LSB | Constructor | Role |
|---|---|---|---|
| δ_a (D_gb) | 1 | `Delta::random` | gen's global key; LSB=1 lets `LSB(a·δ_a) = a` |
| δ_b (D_ev) | 0 | `Delta::random_b` | eval's global key; LSB=0 lets `LSB(a·δ_b) = 0` |
| δ_a ⊕ δ_b | 1 | (derived) | required by Π_LeakyTensor §F masked reveal AND by bit-recovery |

### Block-form sharing fields (post-§1.2)
For each authenticated bit X (alpha=λ_a, beta=λ_b, correlated=λ_a⊗λ_b, gamma=λ_γ):
- `*_gen` Block field → sharing under δ_a; gen's component is `key XOR (value ? δ_a : 0)`, eval's component is `mac`.
- `*_eval` Block field → sharing under δ_b; gen's component is `mac`, eval's component is `key XOR (value ? δ_b : 0)`.

### Production code is Block-form-only
`AuthTensorGen` / `AuthTensorEval` (Online structs) consume only `_eval` / `_gen` Block fields — **no `*_auth_bit_shares`**. Auth-bit shares survive on `TensorFpreGen` / `TensorFpreEval` (preprocessing producers) as trace data for MAC-invariant tests; the Block-form fields are the primary online-phase input.

### Input encoding asymmetry
Per paper §211–215: gen's `masked_x_bits = vec![false; n]` (0-vec); eval's `masked_x_bits = d_x` (cleartext masked-input vector). XOR reveals d_x. Same for masked_y_bits.

### CheckZero is paper-faithful full-block
`block_check_zero` does per-index full-block equality; `block_hash_check_zero` is the paper's `H({V_w})` digest via fixed-key AES correlation-robust hash. **No LSB-only path remains** anywhere in the CheckZero pipeline (the prior LSB-only glue was retired in 1.2(i)).

---

## How to use this scaffolding

For each audit (one at a time, in order 2.1→2.2→2.3→2.4):

1. Read this file's per-surface section.
2. Read the paper sections cited (current `*.tex` files; not the `old_*.tex` files).
3. Read the current source files cited. **Do not** consult the legacy `.planning/phases/*/PATTERNS.md` for paper↔code mapping — the entities they reference (`gamma_auth_bit_shares` on Online structs, `compute_lambda_gamma`, `extbit`, LSB-only `check_zero(&[AuthBitShare], delta)`) no longer exist.
4. Draft `.planning/audit/AUDIT-<surface>.md` per the four-section format above. Surface (b)/(c) findings inline to user as you find them.
5. Commit AUDIT doc as one atomic commit.
6. Queue (d) items as a follow-up sub-phase, one commit per fix.

Phase 1.3 progress (memory: `project_phase_1_3_progress.md`) closes Track 1 — codebase is in a known-good state for audit. No outstanding correctness fixes remain inside the scope of these surfaces.
