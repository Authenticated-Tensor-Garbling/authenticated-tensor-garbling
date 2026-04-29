# AUDIT 2.3 — Protocol 1 (single-delta authenticated tensor product)

## Scope

**Paper:**
- Garbling/Evaluation algorithms (`cons:krrw-algo`): `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex:145-187`.
- Protocol `Π_2pc^t` (`prot:krrw`): `5_online.tex:193-251`, with stages Preprocess / Garble / Encode-inputs / Evaluate / Consistency-check / Decode-outputs.
- Correctness lemma (`lem:protocol1-correctness`): `5_online.tex:253-299`.
- CheckZero specification: `5_online.tex:226-247`.
- Tensor macro definition (`cons:tensor-macro`) cited from this audit but covered by AUDIT-2.1.

**Code:**
- `src/auth_tensor_gen.rs` — `AuthTensorGen` struct + `garble_first_half` / `garble_second_half` / `garble_final` orchestration on top of `gen_chunked_half_outer_product` (P1 chunking wrapper over Construction 1 primitives).
- `src/auth_tensor_eval.rs` — `AuthTensorEval` mirror with `evaluate_first_half` / `evaluate_second_half` / `evaluate_final` and `eval_chunked_half_outer_product`.
- `src/input_encoding.rs::encode_inputs` — input-encoding stage (post-preprocessing, pre-garble).
- `src/online.rs` — `block_check_zero` (per-index simulation form) + `block_hash_check_zero` (paper-faithful `H({V_w})` digest).
- `src/lib.rs::assemble_e_input_wire_blocks_p1` (`:89-140`) — emits `[e_a δ_ev]` / `[e_b δ_ev]` per-party block vectors for CheckZero per paper §240.

**Out of scope (deferred to other audits):**
- `_p2` variants on the same structs (`garble_*_p2`, `evaluate_*_p2`, `garble_final_p2`, `evaluate_final_p2`, `gen_chunked_half_outer_product_wide`, `eval_chunked_half_outer_product_wide`) — audit 2.4 (Protocol 2).
- GGM-tree primitives (`gen_populate_seeds_mem_optimized` / `eval_populate_seeds_mem_optimized` / `gen_unary_outer_product` / `eval_unary_outer_product`) — audit 2.1 covers Construction 1 directly.
- Preprocessing producers (`TensorFpreGen` / `TensorFpreEval` field semantics, `IdealPreprocessingBackend`, `LeakyTensorPre`, `combine_leaky_triples`) — audit 2.2.

## (a) Matches

Three sub-sections matching the paper protocol's natural decomposition: per-gate garbling/evaluation (`cons:krrw-algo`, `5_online.tex:145-187`), input encoding (`prot:krrw` Encode-inputs, `5_online.tex:209-217`), consistency check (`prot:krrw` Consistency-check, `5_online.tex:224-245`).

Conventions: paper's `Δ_gb` / `Δ_ev` ↔ code's `δ_a` / `δ_b`. Paper's `λ_a, λ_b, λ_a⊗λ_b` ↔ code's `α, β, correlated`. Block-form fields: `*_gen` = sharing under δ_a, `*_eval` = sharing under δ_b (per AUDIT-2.2 cross-cutting facts).

### Per-gate garbling — `tensorgb`-side (`AuthTensorGen`)

Paper-aligned via post-§1.2 `A.3` rewrite (per project memory `project_phase_1_2_progress.md`): first-half = `(x⊕α) ⊗ β`, second-half = `(y⊕β) ⊗ x`.

| Paper step (`5_online.tex:148-168`) | Code | Match |
|---|---|---|
| For tensor gate `(a, b, c, ⊗)`, locally compute `[(a ⊕ λ_a) δ_gb]^gb`, `[(b ⊕ λ_b) δ_gb]^gb`; reveal `[a ⊕ λ_a]^gb, [b ⊕ λ_b]^gb` (`:154`) | `encode_inputs` populates `gar.masked_x_gen` (`= [(x⊕α) δ_a]^gb`) and `gar.masked_y_gen` (`= [(y⊕β) δ_a]^gb`); the `[a ⊕ λ_a]^gb` clear-text component is gen's `masked_x_bits = vec![false; n]` per the asymmetric cleartext-masked-bit sharing (`input_encoding.rs:42, 189-192`) | ✓ matches |
| First half: `(Z_{c,0}, halfgate_{c,0}) := tensorgb(n, m, Δ_gb, [(a ⊕ λ_a) Δ_gb], [λ_b Δ_gb])` (`:158`) | `garble_first_half`: `x = masked_x_gen` (paper's `[(a⊕λ_a) Δ_gb]`), `y = beta_gen` (paper's `[λ_b Δ_gb]`); chunked over `chunking_factor` via `gen_chunked_half_outer_product` calling `gen_populate_seeds_mem_optimized` + `gen_unary_outer_product` per chunk (`auth_tensor_gen.rs:103-142, 274-279`) | ✓ matches; chunking wrapper inherits AUDIT-2.1's 2-ct level-cts shape — see B2 |
| Second half: `(Z_{c,1}, halfgate_{c,1}) := tensorgb(m, n, Δ_gb, [(b ⊕ λ_b) Δ_gb], [a Δ_gb])` (`:159`) | `garble_second_half`: `x = masked_y_gen` (paper's `[(b⊕λ_b) Δ_gb]`), `y = x_gen` (paper's `[a Δ_gb]`) (`auth_tensor_gen.rs:281-286`) | ✓ matches |
| Combine: `[c Δ_gb] := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) Δ_gb]` (`:160`) | `garble_final`: `first_half_out[(i,j)] ^= second_half_out[(j,i)] ^ correlated_gen[j*n + i]` for `i ∈ [n], j ∈ [m]`; `final_computed` flag prevents double-application (`auth_tensor_gen.rs:292-308`) | ✓ matches; `correlated_gen` naming is misleading — see C2 |
| Append `halfgate_{c,0}, halfgate_{c,1}` to `gc` (`:163`) | Returned to caller as `(chunk_levels: Vec<Vec<(Block, Block)>>, chunk_cts: Vec<Vec<Block>>)` from `garble_first_half` / `garble_second_half`; caller threads them into `evaluate_*_half` (`auth_tensor_gen.rs:274-286`) | ✓ matches functionally; pair shape on `chunk_levels` is downstream of AUDIT-2.1 B1 |

### Per-gate evaluation — `tensorev`-side (`AuthTensorEval`)

| Paper step (`5_online.tex:170-186`) | Code | Match |
|---|---|---|
| Reconstruct masked values `a ⊕ λ_a, b ⊕ λ_b` by XORing gb's components in `gc` with ev's `[(a ⊕ λ_a) Δ_gb]^ev` (`:175`) | `encode_inputs` directly writes `ev.masked_x_gen[i] = K_x ⊕ ev.alpha_gen[i] ⊕ lsb_shift` and `ev.masked_x_bits = d_x` — eval's view holds both the sharing block AND the cleartext masked bit explicitly (`input_encoding.rs:135-159, 191`); GGM-tree traversal uses `masked_x_bits` directly | ✓ matches; collapses paper's "compute share + XOR with `gc` component" into a single in-process write — see B3 |
| First half: `Z_{c,0} := tensorev(n, m, a ⊕ λ_a, halfgate_{c,0}, [(a ⊕ λ_a) Δ_gb]^ev, [λ_b Δ_gb]^ev)` (`:178`) | `evaluate_first_half`: `x = masked_x_gen`, `y = beta_gen`, `choice_bits = masked_x_bits` (paper's `a ⊕ λ_a`), driven via `eval_chunked_half_outer_product` calling `eval_populate_seeds_mem_optimized` + `eval_unary_outer_product` per chunk (`auth_tensor_eval.rs:111-172, 347-352`) | ✓ matches; explicit `choice_bits` parameter required because `δ_b.lsb()=0` makes MAC LSBs unreliable — see AUDIT-2.1 C3 |
| Second half: `Z_{c,1} := tensorev(m, n, b ⊕ λ_b, halfgate_{c,1}, [(b ⊕ λ_b) Δ_gb]^ev, [a Δ_gb]^ev)` (`:179`) | `evaluate_second_half`: `x = masked_y_gen`, `y = x_gen`, `choice_bits = masked_y_bits` (`auth_tensor_eval.rs:354-358`) | ✓ matches |
| Combine: `[c Δ_gb]^ev := Z_{c,0} ⊕ Z_{c,1}^T ⊕ [(λ_a ⊗ λ_b) Δ_gb]^ev` (`:180`) | `evaluate_final`: identical structural form to `garble_final` — `first_half_out[(i,j)] ^= second_half_out[(j,i)] ^ correlated_gen[j*n + i]` (`auth_tensor_eval.rs:364-380`) | ✓ matches |

### Input encoding — paper Encode-inputs stage (`5_online.tex:209-217`)

| Paper step | Code (`src/input_encoding.rs::encode_inputs`) | Match |
|---|---|---|
| For gb's `x`: gb sends `[x Δ_gb]^ev := [x Δ_gb]^gb ⊕ x·Δ_gb` (`:211`) | `let input_mac = if x_i { input_key ^ delta_a_block } else { input_key }`; `gar.x_gen.push(input_mac)`, `ev.x_gen.push(input_key)` (`:145-155`) — both halves of `[x δ_a]` populated locally; pair encodes `x_i · δ_a` (XOR reveals) | ✓ matches algebraically; paper's two-message exchange collapsed to in-process write — see B3 |
| Parties run `open([λ_x Δ_gb])` to reveal `λ_x` to gb (`:212`) | Bit-recovery from local Block-form: `let a_i = (gar.alpha_eval[i] ^ gar.alpha_gen[i]).lsb()`; `let b_i = (ev.alpha_eval[i] ^ ev.alpha_gen[i]).lsb()` (`:140-141`) — each party reads its own `λ_a` bit from its own state | ✓ matches; in-process recovery substitutes for the paper's `open()` protocol — see B3 |
| gb sends masked `x ⊕ λ_x` to ev (`:212`) | `d_i = x_i ^ a_i ^ b_i`; `ev.masked_x_bits.push(d_i)` (`:142, 191`) — eval receives the cleartext masked bit; gen's component is the 0-vec (`:189`) | ✓ matches; in-process write substitutes for the paper's send |
| gb sets `[x Δ_ev]^gb := [λ_x Δ_ev]^gb`; ev sets `[x Δ_ev]^ev := [λ_x Δ_ev]^ev ⊕ (x ⊕ λ_x)·Δ_ev` (`:212`) | NOT populated during encode. Derived at CheckZero time via `assemble_e_input_wire_blocks_p1` from `gb.alpha_eval` (= `[λ_x δ_b]^gb`) and `ev.alpha_eval ⊕ L_a·δ_b` (= `[λ_x δ_b]^ev ⊕ (x⊕λ_x)·δ_b`) (`lib.rs:115-126`) | ✓ matches algebraically; δ_ev tracking deferred to CheckZero — see B3 |
| For ev's `y`: parties run `open([λ_y Δ_ev])`; ev sends masked `y ⊕ λ_y`; gb sends correction `W = [y Δ_gb]^gb ⊕ [λ_y Δ_gb]^gb ⊕ (y⊕λ_y)·Δ_gb`; ev sets `[y Δ_gb]^ev := [λ_y Δ_gb]^ev ⊕ W` (`:213-215`) | Same path as x — y is treated symmetrically (no separate gb-input vs ev-input encoding); same bit-recovery + in-process write pattern via β/m loop (`:167-185`) | ✗ deviates structurally — see B3 (paper distinguishes gb's-input vs ev's-input encoding paths; code unifies them) |
| Per-input wire `i`: `lsb_shift = (x_i ⊕ a_i)·δ_a` applied to both `gar.masked_x_gen[i]` and `ev.masked_x_gen[i]` (cancels in XOR sum) | `let lsb_shift = if x_i ^ a_i { delta_a_block } else { Block::ZERO }`; `gar.masked_x_gen.push(input_mac ^ gar.alpha_gen[i] ^ lsb_shift)`; `ev.masked_x_gen.push(input_key ^ ev.alpha_gen[i] ^ lsb_shift)` (`:152-157`) | Code-internal LSB-landing trick (gen's seed LSB=0, eval's seed LSB=`d_i`) for GGM-tree convention; not in paper but algebraically transparent (cancels in combined XOR) — see C2 |

### Consistency check — paper §226-247

| Paper step | Code | Match |
|---|---|---|
| ev sends all revealed masked values `a ⊕ λ_a, b ⊕ λ_b` to gb (`:226`) | In-process: cleartext masked bits already shared via `gar.masked_*_bits = vec![false; n]` and `ev.masked_*_bits = d_*`; combined XOR yields the cleartext vector. Tests pass `l_alpha_pub = combined-XOR` directly to `assemble_e_input_wire_blocks_p1` | ✓ matches in-process |
| Compute `[c Δ_ev]` shares walking the circuit (`:227-235`) | NOT walked — single-gate audit; circuit walk is part of B1 | (out of scope here) |
| Per tensor gate: `[e_a Δ_ev] := [a Δ_ev] ⊕ [λ_a Δ_ev] ⊕ (a ⊕ λ_a)·Δ_ev` (length n); `[e_b Δ_ev]` symmetric (length m) (`:240`) | `assemble_e_input_wire_blocks_p1` (`lib.rs:89-140`): for `i ∈ [n]`, `gen_blocks.push(gb_v_alpha_eval[i] ^ gb.alpha_eval[i])`; `eval_blocks.push(ev_v_alpha_eval[i] ^ ev.alpha_eval[i] ^ l_a_correction)` where `l_a_correction = if l_alpha_pub[i] { δ_b } else { 0 }`; same for β (`lib.rs:115-137`) | ✓ matches paper §240 — `[v_a δ_ev]` ↔ `gb_v_alpha_eval`/`ev_v_alpha_eval`, `[λ_a δ_ev]` ↔ `gb.alpha_eval`/`ev.alpha_eval`, `(a⊕λ_a)·δ_ev` ↔ `l_a_correction` |
| `CheckZero({[e_a Δ_ev], [e_b Δ_ev]})`; ev aborts if any `e_a, e_b ≠ 0` (`:244`) | `block_check_zero(gen_blocks, eval_blocks)` returns `false` on any per-index mismatch (`online.rs:33-43`); also `block_hash_check_zero` for the paper-faithful `H({V_w})` digest (`online.rs:57-63`) | ✓ matches; per-index simulation form vs paper-faithful digest — see C1 |
| Decode outputs: `open([z Δ_ev])` reveals `z` to ev (`:249`) | Not implemented — see B1 | ✗ deferred |

### Correctness invariant verification

Paper Lemma `lem:protocol1-correctness` (`5_online.tex:253-299`): consistency check passes and ev outputs `z = C(x, y)` for honest parties. Reduces to:
- δ_gb-shares: `Z_{c,0}^gb ⊕ Z_{c,0}^ev = (a⊕λ_a)⊗λ_b · Δ_gb` and `Z_{c,1}^gb ⊕ Z_{c,1}^ev = (b⊕λ_b)⊗a · Δ_gb` (Lemma `lem:tensor-macro-correctness` from AUDIT-2.1); summing with `[(λ_a⊗λ_b) Δ_gb]` gives `(a⊗b) Δ_gb`.
- δ_ev-shares: induction on input wires yields `[a Δ_ev] = [λ_a Δ_ev] ⊕ (a⊕λ_a)·Δ_ev`; the consistency-check formula `[e_a Δ_ev] := [a Δ_ev] ⊕ [λ_a Δ_ev] ⊕ (a⊕λ_a)·Δ_ev` therefore sums to `0` and CheckZero accepts.

Existing tests verify the protocol-level invariants for a single tensor gate: `test_full_protocol_1_nonzero_inputs_{ideal,uncompressed}` (regression for x=0b1011, y=0b101); tampered-δ_a-XOR (LSB=1) and tampered-δ_b-XOR (LSB=0) negative tests confirm CheckZero aborts on either tamper. Online-module tests cover `block_check_zero` per-index equality, length mismatch, empty slice, hash digest equality + inequality (`online.rs::tests`).

## (b) Deviations

### B1 — Code is per-gate, paper is per-circuit; output-decode (`open([z δ_ev])`) is deferred

**Paper (`5_online.tex:145-187, :193-251`):** `Π_2pc^t` operates at the circuit level. `garble(1^κ, C, [λ δ_gb])` walks circuit `C` in topological order:
- For each XOR gate: locally compute `[c δ_gb] := [a δ_gb] ⊕ [b δ_gb]`.
- For each tensor gate: emit `[a ⊕ λ_a]^gb`, `[b ⊕ λ_b]^gb` to `gc`, run two `tensorgb` calls, append half-gate ciphertexts, combine with `[(λ_a⊗λ_b) δ_gb]^gb`.
- Output: `(gc, [(x ‖ y) δ_gb]^gb)`.

`eval(1^κ, C, gc, [(x ‖ y) δ_gb]^ev, [λ δ_gb]^ev)` mirrors. CheckZero step iterates over **every** tensor gate `(a, b, c, ⊗) ∈ C` (paper §237).

Output decode (paper §247-250):
> "Parties run `open([z δ_ev])` revealing the circuit output `z` to ev."

**Code:** `AuthTensorGen` / `AuthTensorEval` represent a **single tensor gate**. The struct's `first_half_out` / `second_half_out` / `correlated_gen` etc. are sized to one gate's `(n, m)`. `garble_final` is gated by a `final_computed: bool` flag that asserts the method runs at most once per instance:
```rust
assert!(!self.final_computed,
    "garble_final called twice on the same instance — \
     first_half_out would be double-XOR'd; create a new instance per gate");
```
There is no XOR-gate handler, no topological-order walker, and no `open()` primitive in `online.rs` (`:5-6` documents `open()` as deferred per Phase 8 CONTEXT D-01).

**Existing test orchestration:** tests in `src/lib.rs::tests` build a paired `(AuthTensorGen, AuthTensorEval)`, drive a single tensor gate end-to-end, and verify outputs by reading the Block-form `first_half_out` matrices directly — bypassing the paper's `open([z δ_ev])` step entirely.

**Implication:** full circuit support — sequencing multiple tensor gates, handling intermediate XOR gates, and producing decoded output to the evaluator — requires:
1. A circuit data structure (gate list with topological order).
2. A wire-share state machine that threads `[w δ_gb]` / `[w δ_ev]` shares across gates.
3. An XOR-gate handler that locally XORs share-blocks (free, no preprocessing consumption).
4. An `open([z δ_ev])` primitive in `online.rs` that performs the masked reveal under δ_ev with MAC verification.
5. A consistency-check assembly that iterates over every tensor gate's input wires (today `assemble_e_input_wire_blocks_p1` is single-gate).

For the project's current goal (single-tensor-gate protocol demonstration + benchmarks), this scope is acceptable. For full Protocol 1 realization, missing.

**Required fix (queued as (d)):** introduce circuit orchestrator + `open()` primitive. Track as a future phase, not in this audit.

### B2 — `gen_chunked_half_outer_product` / `eval_chunked_half_outer_product` carry the AUDIT-2.1 B1 2-ct level-cts shape downstream

**Code (`src/auth_tensor_gen.rs:103-142`, `src/auth_tensor_eval.rs:111-172`):** both wrappers thread `Vec<Vec<(Block, Block)>>` for `chunk_levels` end-to-end. Each `(Block, Block)` is a per-level `(G_{i,0}, G_{i,1})` pair from the underlying `gen_populate_seeds_mem_optimized` / `eval_populate_seeds_mem_optimized`.

When AUDIT-2.1 D1 lands (paper's improved one-hot rewrite — single ct per level), the primitive returns `Vec<Block>` for level cts. The wrappers must follow:
- `gen_chunked_half_outer_product` return type: `(Vec<Vec<Block>>, Vec<Vec<Block>>)`.
- `eval_chunked_half_outer_product` parameter type: `chunk_levels: Vec<Vec<Block>>`.
- Bench fixtures and any caller capturing the pair shape must update.

Already coordinated as AUDIT-2.1 D3 in `AUDIT-2.1.md`. AUDIT-2.3 surfaces this finding here so the chunking wrapper isn't accidentally treated as paper-faithful in isolation.

**Required fix (queued as (d)):** consume AUDIT-2.1 D1's primitive-level rewrite by updating both chunked wrappers' signatures + tests + benches.

### B3 — Input encoding decomposed differently from paper §211-215; collapses two distinct gb-input vs ev-input encoding paths into a single in-process function

**Paper (`5_online.tex:211-215`):** Encode-inputs is a multi-message exchange with **two distinct sub-protocols**:
- For gb's input `x`: gb sends `[x δ_gb]^ev`; parties run `open([λ_x δ_gb])` to reveal `λ_x` to gb; gb sends `x ⊕ λ_x`; gb sets `[x δ_ev]^gb = [λ_x δ_ev]^gb`; ev sets `[x δ_ev]^ev = [λ_x δ_ev]^ev ⊕ (x⊕λ_x)·δ_ev`.
- For ev's input `y`: parties run `open([λ_y δ_ev])` to reveal `λ_y` to ev; ev sends `y ⊕ λ_y`; gb sends correction `W = [y δ_gb]^gb ⊕ [λ_y δ_gb]^gb ⊕ (y⊕λ_y)·δ_gb`; ev sets `[y δ_gb]^ev = [λ_y δ_gb]^ev ⊕ W`.

The two paths differ in **which party reveals the masked input** and **which party sends the correction term**, mirroring asymmetric input ownership.

**Code (`src/input_encoding.rs::encode_inputs(gar, ev, x, y, rng)`):**
- Treats `x` and `y` symmetrically — both indexed via gen's `alpha_gen` / `beta_gen` Block-form fields, both produce `(input_key, input_mac)` via the same `if x_i { input_key ^ delta_a } else { input_key }` recipe (`:144-159, 167-185`).
- No `open()` calls — λ-bit recovery is a local read: `let a_i = (gar.alpha_eval[i] ^ gar.alpha_gen[i]).lsb()` (`:140`).
- No W correction term — `[y δ_gb]^ev` is recoverable from local Block-form fields without a message.
- δ_ev tracking deferred to CheckZero time via `assemble_e_input_wire_blocks_p1` (`lib.rs:115-126`), which derives `[v_a δ_ev]^ev = ev.alpha_eval[i] ⊕ L_a·δ_b` matching paper's `[a δ_ev]^ev := [λ_a δ_ev]^ev ⊕ (a⊕λ_a)·δ_ev`.
- Adds an `lsb_shift = (x_i ⊕ a_i)·δ_a` trick (`:152`) to land LSBs on `(0, d_i)` for the GGM-tree convention — code-internal, not in paper, applied symmetrically so it cancels in the combined XOR.

**Algebraic equivalence:** for honest execution, the in-process function produces Block-form state algebraically identical to what the paper's two-message exchange would yield. The δ_ev shares emerge correctly at CheckZero time.

**Implication:** the code's encoding cannot be transcribed to a real two-party protocol without:
1. Splitting the function into separate gb-input and ev-input encode paths.
2. Introducing an `open([λ δ])` primitive (also flagged in B1).
3. Reintroducing the W correction term for ev's input under δ_gb.
4. Tracking δ_ev shares explicitly during encode (or keeping the lazy CheckZero-time derivation but ensuring it survives the multi-message transport).

**Required fix (queued as (d)):** when the protocol moves to a multi-process implementation, refactor `encode_inputs` into two paper-faithful sub-protocols; add the `open()` primitive (paired with B1's circuit-orchestrator open); preserve or refactor the `lsb_shift` trick depending on real-protocol message budget. Documentation-only fix in current single-process simulation: tighten the doc comment to flag the structural divergence.

## (c) Latent assumptions

### C1 — CheckZero exists in two flavors; current call sites use the simulation form, not the paper-faithful digest

`src/online.rs:33-43` `block_check_zero` performs per-index full-block equality — the in-process simulation form, taking both parties' block vectors directly.
`src/online.rs:57-63` `block_hash_check_zero` is the paper-faithful `H({V_w})` digest (one-pass `h_{i+1} = cr(h_i ⊕ block_i)` via fixed-key AES correlation-robust hash).

Caller picks. Tests use `block_check_zero` (per-index). The doc comment at `online.rs:24-27` documents this:
> "SIMULATION ONLY in this in-process testbed: takes both parties' block vectors directly. In a real two-party run, each party hashes its own blocks via `block_hash_check_zero` and parties exchange digests; matching digests imply per-index equality by collision-resistance of the hash."

When the protocol moves to multi-process, callers must switch to `block_hash_check_zero` + digest exchange. Same simulation envelope as AUDIT-2.2 C1/C2 (in-process F_eq stub + `verify_cross_party` MAC reveal).

### C2 — `correlated_gen` field naming is misleading; `lsb_shift` trick is a code-internal GGM-convention adjustment not in paper

**Naming:** `correlated_gen` reads as "gen's view of the correlated mask," but actually means "share of the correlated mask under δ_gb (= δ_a)." Both parties have their own `correlated_gen` field — gb's is `[(λ_a⊗λ_b) δ_a]^gb`, ev's is `[(λ_a⊗λ_b) δ_a]^ev`. The δ_b counterpart is `correlated_eval`. Same pattern for `alpha_gen`/`alpha_eval` and `beta_gen`/`beta_eval`. Algebra is correct; the naming reads as owner-tagged when it's actually delta-tagged.

**`lsb_shift` trick (`src/input_encoding.rs:152`):** `lsb_shift = (x_i ⊕ a_i)·δ_a` applied symmetrically to both `gar.masked_x_gen[i]` and `ev.masked_x_gen[i]`. Cancels in the combined XOR sum (XOR-equivalent to no shift), while flipping LSBs to land on `(0, d_i)` per the GGM-tree convention (gen's seed LSB=0, eval's seed LSB=d_i). Documented in the encoding-loop comment but adds an implementation-detail layer not present in the paper.

Both findings are documentation-only — naming refactor or comment tightening would clarify.

### C3 — `chunking_factor` cross-cutting invariant is not enforced at the AuthTensorGen/Eval boundary

Per AUDIT-2.2 B2's "Chunking-size matching invariant" cross-cutting fact: P1's `chunking_factor` MUST match preprocessing's `chunking_factor` for the same matrix once chunked preprocessing lands. Today, preprocessing has no chunking, so the invariant is vacuous; once it lands, mismatch silently breaks tile alignment.

`chunking_factor` flows through unchecked:
- `IdealPreprocessingBackend::run(n, m, cf)` sets `TensorFpreGen.chunking_factor = cf`.
- `AuthTensorGen::new_from_fpre_gen` copies `fpre_gen.chunking_factor` (`auth_tensor_gen.rs:79`).
- No assertion that gb's `chunking_factor` equals ev's at struct construction.
- No cross-check with the (future) chunked preprocessing factor.

Caller responsibility today; will become a real invariant when AUDIT-2.2 B2's fix lands.

**Required fix candidates (queued as (d)):** when chunked preprocessing lands, add an assertion in `AuthTensorGen::new_from_fpre_gen` and `AuthTensorEval::new_from_fpre_eval` that `chunking_factor` matches the value baked into the preprocessing output, plus a cross-party assertion that gb's and ev's factors agree.

## (d) Required code changes

Queued as a follow-up sub-phase. Each item is a separate atomic commit per the Track 2 interaction model. Numbering reflects suggested execution order.

| # | Source finding | Scope | Notes |
|---|---|---|---|
| D1 | B1 | Introduce a circuit orchestrator: gate list with topological order; XOR-gate handler that locally XORs share-blocks; wire-share state machine threading `[w δ_gb]` / `[w δ_ev]` across gates; circuit-level garble/eval/check loops that iterate over every tensor gate (including the per-tensor-gate `[e_a δ_ev]` / `[e_b δ_ev]` assembly currently single-gate in `assemble_e_input_wire_blocks_p1`). | Major scope — multi-phase work. Project's current single-tensor-gate goal does not require it. |
| D2 | B1 | Implement `open([z δ_ev])` primitive in `src/online.rs` per Phase 8 CONTEXT D-01 (currently deferred). Includes wrong-delta negative test. | Paired with D1; needed for paper-faithful output decode. |
| D3 | B2 | Update `gen_chunked_half_outer_product` (`auth_tensor_gen.rs`) and `eval_chunked_half_outer_product` (`auth_tensor_eval.rs`) to consume AUDIT-2.1 D1's `Vec<Vec<Block>>` level-cts shape (replacing `Vec<Vec<(Block, Block)>>`). Update bench fixtures and any caller of the pair shape. | Subsumed by AUDIT-2.1 D3; surfaced here for cross-audit visibility. Must land in the same fix sub-phase as AUDIT-2.1 D1. |
| D4 | B3 | When multi-process implementation begins: refactor `encode_inputs` into two paper-faithful sub-protocols (gb-input encode + ev-input encode); reintroduce `open([λ δ])` calls and the W correction term; preserve or refactor `lsb_shift` trick. Documentation-only until then: tighten doc comment to flag structural divergence from paper §211-215. | Documentation fix can land standalone; full refactor paired with multi-process transition. |
| D5 | C1 | When multi-process implementation begins: switch consistency-check call sites to `block_hash_check_zero` + digest exchange; remove the `block_check_zero` simulation path or guard it behind `#[cfg(test)]`. | Documentation-only until multi-process work begins. |
| D6 | C2 | Rename `correlated_gen` / `correlated_eval` (and analogous `alpha_gen`/`alpha_eval`, `beta_gen`/`beta_eval`) to delta-tagged names that don't read as owner-tagged (e.g., `correlated_dgb` / `correlated_dev`). Optionally tighten the `lsb_shift` comment in `encode_inputs` to flag it as a code-internal GGM-convention adjustment. | Pure naming refactor; large blast radius (every consumer of the Block-form fields). Defer until project priorities allow a churn window. |
| D7 | C3 | When AUDIT-2.2 B2 (chunked preprocessing) lands: add `assert_eq!(gb.chunking_factor, ev.chunking_factor)` cross-party check at AuthTensor{Gen,Eval} construction, plus a cross-check that the value matches what preprocessing baked in. | Trivial fix; gated on AUDIT-2.2 B2's fix landing first. |

### Coordination notes

- **D3 must land with AUDIT-2.1 D1** — the chunking wrappers and the GGM-tree primitive are tightly coupled by the level-cts type signature.
- **D1 + D2 are paired** — the circuit orchestrator is the natural caller of `open([z δ_ev])`. Don't land one without the other.
- **D4 + D5 are paired with the multi-process transition** — same fix sub-phase as AUDIT-2.2 D3+D4 (replace simulation primitives with real two-party message exchange).
- **D6 (naming refactor) is independent** but high-blast-radius across the codebase; better to bundle with another touch of the Block-form fields rather than land standalone.
- **D7 is gated on AUDIT-2.2 B2's fix** (chunked preprocessing) — without it, there's nothing to assert.
