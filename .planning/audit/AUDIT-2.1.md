# AUDIT 2.1 — Construction 1 (Generalized tensor macro / GGM tree)

## Scope

**Paper:**
- Primary: `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_krrw_pre.tex:78-148` `cons:gen-tensor-macro` (generalized form, used by `Π_LeakyTensor`).
- Cross-ref: `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/5_online.tex:39-99` `cons:tensor-macro` (specialized to `[B] = [b·δ_gb]`, used by Protocol 1). Identical algorithm, narrower second input.
- Correctness lemma: `5_online.tex:101-103` `lem:tensor-macro-correctness`.

**Code:**
- `src/tensor_macro.rs` — named macros `tensor_garbler` / `tensor_evaluator` (full-`(n,m)`, no chunking).
- `src/tensor_ops.rs` narrow GGM helpers: `gen_populate_seeds_mem_optimized`, `gen_unary_outer_product`, `eval_populate_seeds_mem_optimized`, `eval_unary_outer_product`.

**Out of scope (deferred to other audits):**
- Wide variants `gen_unary_outer_product_wide` / `eval_unary_outer_product_wide` (`tensor_ops.rs:289-396`) — audit 2.4 (Protocol 2 wide-leaf with D_ev MAC propagation).
- P1 chunking wrappers `gen_chunked_half_outer_product` / `eval_chunked_half_outer_product` in `auth_tensor_gen.rs` / `auth_tensor_eval.rs` — audit 2.3 (P1 online tile iteration over Construction 1 primitives).

## (a) Matches

Paper-step ↔ code-step mapping. Endianness convention: paper indexes `A_0..A_{n-1}` from level 0 down; code uses an MSB-first traversal where `a_keys[n-1]` (paper's `A_0` analog under bit-position-equal-to-tree-level) feeds level 0. The leaf-distribution kernel (step 7 below) compensates so the externally-observable `Z = a ⊗ T` semantics match paper.

### Garbler side — `tensor_garbler` (`src/tensor_macro.rs:82-116`)

| Paper step (`5_online.tex` / `appendix_krrw_pre.tex`) | Code | Match |
|---|---|---|
| 1. Interpret `[a δ_gb]^gb = A_0 A_1 … A_{n-1}` as n κ-bit labels (`5_online.tex:43`) | `tensor_garbler` accepts `a_keys: &[Key]`; `a_blocks = Key::as_blocks(a_keys)` (`tensor_macro.rs:100`); precondition `a_keys.len() == n` (`tensor_macro.rs:90`) | ✓ matches (modulo MSB-first traversal — see B1, plus C1 latent assumption) |
| 2. Initialize level-0 width-2 encoding: `S_{0,0} := A_0 ⊕ δ_gb`, `S_{0,1} := A_0` (`5_online.tex:44`) | `gen_populate_seeds_mem_optimized:29-35` writes `seeds[0]=TCCR(0, x[n-1] ⊕ δ)`, `seeds[1]=TCCR(0, x[n-1])` under `Key.lsb()==0` (i.e., always, in production) | ✗ deviates — see B2 (extra init TCCR) and B1 (HK21 vs improved one-hot fold both into the same rewrite) |
| 3a. `R_{i,j} := H(S_{i-1,j}, ν_{i,j})` for `j ∈ [2^i]` (`5_online.tex:47`) | Loop body `gen_populate_seeds_mem_optimized:38-66`: TWO independent TCCRs per parent — `seeds[2j+1] := TCCR(tweak_odd, seeds[j])`, `seeds[2j] := TCCR(tweak_even, seeds[j])` | ✗ deviates — see B1 (HK21-style 2-hash-per-parent vs paper's single R) |
| 3b. `S_{i,j} := R_{i,j} ⊕ S_{i-1,j}`, `S_{i,2^i+j} := R_{i,j}` (FreeXOR sibling) (`5_online.tex:48-53`) | Code does NOT XOR parent into child — siblings are two independent TCCR outputs | ✗ deviates — see B1 |
| 3c. `G_i := (⊕_j R_{i,j}) ⊕ A_i` (single ct per level) (`5_online.tex:54`) | `(evens, odds) = (⊕_j seeds[2j] ⊕ TCCR(tweak_even, A_i⊕δ),  ⊕_j seeds[2j+1] ⊕ TCCR(tweak_odd, A_i))` — TWO cts per level | ✗ deviates — see B1 |
| 4. `G := {G_i}_{i=1}^{n-1}` (`5_online.tex:56`) | `level_cts: Vec<(Block, Block)>` of length `n-1` (`tensor_macro.rs:55-56`) | ✗ deviates — see B1 (pair vs scalar) |
| 5. Leaf expansion: `Label_ℓ := S_{n-1,ℓ}`; `X_{ℓ,k} := H(Label_ℓ, ν_{ℓ,k})` (`5_online.tex:57`) | `gen_unary_outer_product:96-99` — `s = TCCR((m·leaf_i + j), seeds[leaf_i])` with `j ∈ [m]` is the column index playing role of `k`. Note nonce `m·j + leaf_i` ↔ paper's `ν_{ℓ,k}`. | ✓ matches (subject to C-finding on tweak domain — see below) |
| 6. `G_k := (⊕_ℓ X_{ℓ,k}) ⊕ B_k` for `k ∈ [m]` (`5_online.tex:58-62`) | `row ^= s` accumulates `⊕_i TCCR(...)`; then `row ^= y[j]` and `gen_cts.push(row)` (`tensor_ops.rs:99-111`) | ✓ matches (`y[j]` plays role of `B_k`) |
| 7. `Z_gb := truthtable(I)^T · X_gb` — row `i` of `Z` accumulates leaves where bit `i` of leaf-index is 1 (`5_online.tex:63-66`) | `if ((i >> k) & 1) == 1 { out[(k, j)] ^= s }` distributes leaf `i`'s contribution to row `k` when bit-`k` of `i` is set (`tensor_ops.rs:104-108`) | ✓ matches (paper's row-index `i` ↔ code's row-index `k`; both = `a_k` once leaf-index encodes the active path) |

### Evaluator side — `tensor_evaluator` (`src/tensor_macro.rs:138-187`)

| Paper step (`5_online.tex:71-97`) | Code | Match |
|---|---|---|
| 1. Parse `G = {G_i} ‖ {G_k}` and `[a δ_gb]^ev = A_0 ⊕ a_0 δ, …` (`5_online.tex:72`) | `tensor_evaluator(g, a_macs, a_bits, t_eval)` (`tensor_macro.rs:138-156`); shape asserts on `g.level_cts.len() == n-1` and `g.leaf_cts.len() == m` | ✓ matches (modulo B1's pair-shape on `level_cts`); `a_bits` is a separate parameter — see C-finding on `a_bits` decoupling |
| 2. Init: `S_{0,0}^ev := S_{0,1}^ev := A_0 ⊕ a_0·δ_gb`; `α_0 := a_0` (`5_online.tex:73`) | `seeds[!a0] = TCCR(0, x[n-1])` (i.e., `TCCR(0, MAC)`); `seeds[a0] = Block::default` sentinel; `missing = a0` (`tensor_ops.rs:155-159`) | ✗ deviates — see B1 + B2 (HK21 init applies TCCR; only inactive position holds a value, active position is sentinel — paper sets BOTH equal to `MAC` without hashing) |
| 3a-c. Recover `R_{i, α_{i-1}}^ev := G_i ⊕ (⊕_{j ≠ α_{i-1}} R_{i,j}^ev) ⊕ (A_i ⊕ a_i δ)` and update `α_i := α_{i-1} + a_i · 2^i` (`5_online.tex:75-87`) | `seeds[sibling_index] = TCCR(tweak, x[n-i-1]) ^ mask` where `mask = G_evens ^ e_evens` if bit else `G_odds ^ e_odds`; `missing = (missing<<1) \| bit` (`tensor_ops.rs:172-202`) | ✗ deviates — see B1 (sibling-recovery applies TCCR to MAC because code's tree hashes MAC; paper recovers `R` directly without hashing MAC) |
| 4. `Label_ℓ^ev := S_{n-1, ℓ}^ev` for `ℓ ≠ α_{n-1}` (`5_online.tex:89`) | `seeds[i]` for `i ≠ missing` after seed reconstruction loop completes | ✓ matches (modulo upstream B1 structural differences) |
| 5. Leaf expansion `X_{ℓ,k}^ev := H(Label_ℓ^ev, ν_{ℓ,k})` for `ℓ ≠ α_{n-1}` (`5_online.tex:89`) | `s = TCCR((m·j + i), seeds[i])` for `i ≠ missing` (`tensor_ops.rs:240-244`) | ✓ matches |
| 6. `X_{α_{n-1}, k}^ev := (⊕_{ℓ ≠ α} X_{ℓ,k}^ev) ⊕ G_k ⊕ (B_k ⊕ b_k δ)` (`5_online.tex:91-93`) | `eval_ct = ⊕_{i ≠ missing} TCCR(…, seeds[i]) ^ gen_cts[j] ^ y[j]` (`tensor_ops.rs:239-254`) — `y[j]` plays role of `[B]^ev` | ✓ matches |
| 7. `Z_ev := truthtable(I)^T · X_ev` (`5_online.tex:94-96`) | Same row distribution as gen-side step 7 for non-missing leaves; missing-leaf contribution `eval_ct` distributed to `out[(k, j)]` where `((missing >> k) & 1) == 1` (`tensor_ops.rs:247-261`) | ✓ matches |

### Correctness invariant verification

Paper Theorem 1 / `lem:tensor-macro-correctness` (`5_online.tex:101-103`): `Z_gb ⊕ Z_eval = a ⊗ T`.

Code analytically reduces to: at each `(k, j)`, `out_gen[(k,j)] ⊕ out_eval[(k,j)] = [bit_k(missing) = 1] · T[j] = a_k · T[j]`. The leaf-index encoding `bit_k(missing) = a_k` (verified by tracing `missing := (missing<<1) | bit` against paper's `α_i := α_{i-1} + a_i·2^i` recurrence) makes the two sides agree on the eval's missing-leaf compensation. Existing tests (`tensor_macro.rs::tests`, n=1,2,4,8 × m=1,3,4,8,16,64) pass.

## (b) Deviations

### B1 — Code implements HK21's two-ciphertext-per-level construction, not the paper's improved one-hot one-ciphertext-per-level

**Paper claim:** `5_online.tex:20-21,24` — "We modify [HK21]'s privacy-free tensor gate construction to use the **improved one-hot construction** of [Heath24], which requires sending only **1 ciphertext per level** of the GGM tree." Communication cost: "`G` comprising **(n-1) + m** ciphertexts of length κ."

**Paper recurrence (`5_online.tex:46-54`, mirrored in `appendix_krrw_pre.tex:95-104`):**
- `R_{i,j} := H(S_{i-1,j}, ν_{i,j})` for `j ∈ [2^i]` — **one** hash per parent.
- `S_{i,j} := R_{i,j} ⊕ S_{i-1,j}`, `S_{i,2^i+j} := R_{i,j}` — FreeXOR links the two siblings.
- `G_i := (⊕_j R_{i,j}) ⊕ A_i` — **one** ciphertext per level.

**Code recurrence (`src/tensor_ops.rs:60-72`):**
- `seeds[2j] := TCCR(tweak_even, seeds[j])` and `seeds[2j+1] := TCCR(tweak_odd, seeds[j])` — **two independent** TCCRs per parent; no FreeXOR linking siblings.
- Per level emits `(G_{i,0}, G_{i,1}) := (⊕_j seeds[2j] ⊕ TCCR(tweak_even, A_i ⊕ δ),  ⊕_j seeds[2j+1] ⊕ TCCR(tweak_odd, A_i))` — **two** ciphertexts per level (`tensor_macro.rs:49-53` documents this explicitly).

**Cost mismatch:**
- `TensorMacroCiphertexts.level_cts: Vec<(Block, Block)>` (`tensor_macro.rs:55-56`) has length `n-1` → **`2(n-1) + m`** ciphertexts emitted, vs paper's `(n-1) + m`.
- Per-tensor-macro communication doubled at the level-tree portion. Affects every Construction-1 invocation: `AuthTensorGen::garble_first_half/_second_half` (P1 online), `LeakyTensorPre::generate` (preprocessing).

**Correctness:** `Z_gb ⊕ Z_eval = a ⊗ T` holds — both sides apply the same tree recurrence, so XOR cancellation works regardless of which construction (HK21 vs improved one-hot) is in use. Existing tests (`tensor_macro.rs::tests`, n=1,2,4,8 × m=1,3,4,8,16,64) pass.

**Hypothesis:** holdover from an earlier HK21-baseline implementation that was never upgraded to the paper's improved construction. Headline communication-cost claim of the paper (`5_online.tex:4` "≈ 2(n+m)κ bits per tensor gate") is therefore not realized by current code.

**Required fix:** queued as (d) item. Scope: rewrite `gen_populate_seeds_mem_optimized` + `eval_populate_seeds_mem_optimized` to use paper's `R_{i,j} = H(S_{i-1,j}, ν_{i,j})` recurrence with FreeXOR sibling derivation; collapse `level_cts` to `Vec<Block>` of length `n-1`; update `eval_populate_seeds_mem_optimized` reconstruction logic; update `tensor_macro.rs` doc comments. Tests should add a ciphertext-count assertion (`g.level_cts.len() == n-1` and entry type `Block`, not `(Block, Block)`).

### B2 — Init applies an extra TCCR not present in paper or HK21

**Paper (`5_online.tex:44` / `appendix_krrw_pre.tex:93`):** Step 2 — `S_{0,0} := A_0 ⊕ δ_gb`, `S_{0,1} := A_0`. **No hash applied at level 0.**

**Code (`src/tensor_ops.rs:29-35`):**
```rust
if x[n-1].lsb() {
    seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
    seeds[1] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1] ^ delta);
} else {
    seeds[1] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1]);
    seeds[0] = cipher.tccr(Block::new((0 as u128).to_be_bytes()), x[n-1] ^ delta);
}
```
Both branches apply `TCCR(0, ·)` to `A_0 ⊕ δ` and `A_0` before storing into `seeds[0..2]`. Adds 1 extra hash per leaf path beyond paper/HK21. Eval mirrors at `src/tensor_ops.rs:155-156`, so the protocol invariant holds — this is redundant work, not a correctness bug.

**Required fix:** queued as a (d) item. The B1 rewrite (paper's improved one-hot) naturally subsumes this — the corrected init writes `S_{0,*}` directly without hashing.

## (c) Latent assumptions

### C1 — `Key.lsb() == 0` invariant required for level-0 dispatch but not re-checked at the macro boundary

**Doc claim (`src/tensor_macro.rs:79`):**
> "Every `a_keys[i].lsb() == 0` (enforced by `Key::new()` — re-asserted here as defence in depth)"

**Reality:** `tensor_garbler` body (`src/tensor_macro.rs:82-116`) contains no `assert!(key.lsb() == false)` — only `assert_eq!` on lengths and shapes. The "re-asserted here" claim is false.

**Why it matters:** `gen_populate_seeds_mem_optimized:29-35` branches on `x[n-1].lsb()`. Under the production guarantee `Key::new()` clears LSB, so the `if x[n-1].lsb()` arm is dead code. If a future caller passes raw `Block`s instead of `Key`s (e.g., a refactor that re-uses the function for non-key inputs), the dead arm becomes live and the level-0 layout (`seeds[0]` ↔ paper's `S_{0,0}` even-side) silently flips. No existing test catches this because all current callers (`tensor_garbler` + `gen_chunked_half_outer_product`) pass `Key::as_blocks(a_keys)`.

**Required fix candidates (queued as (d)):** (a) tighten the doc comment to remove the false "re-asserted here" claim; or (b) add `debug_assert!(a_blocks.iter().all(|b| !b.lsb()))` at `src/tensor_macro.rs:100`; or (c) remove the dead branch entirely from `gen_populate_seeds_mem_optimized` (preferred — simplifies the function and forces the invariant onto the call site, where it is already guaranteed by `Key::new()`).

### C2 — Tweak collisions across stages rely on input-distinctness rather than explicit tweak-domain separation

**Narrow tweak ranges:**
- Init (`src/tensor_ops.rs:30-34, 156`): tweak = `0`.
- Loop (`src/tensor_ops.rs:55-56, 169-170`): tweaks `{2i, 2i+1}` for `i ∈ [1, n-1]` → `{2, 3, …, 2n-1}`.
- Leaf expansion (`src/tensor_ops.rs:97, 242`): tweak `m·j + leaf_i` for `(j, leaf_i) ∈ [m] × [2^n]` → `[0, m·2^n)`.

Tweak values overlap across stages. Examples:
- Init's tweak `0` = leaf-expansion's `m·0 + 0`.
- Loop's tweak `2` (level 1, even) = leaf-expansion's `m·0 + 2` (when `m ≥ 1, n ≥ 2`).

**Why it does not break correctness in practice:** TCCR inputs at colliding tweaks are different objects — init operates on `A_0` / `A_0 ⊕ δ` (raw keys), leaf-expansion operates on derived seeds (TCCR outputs of the tree). With overwhelming probability over δ-randomness these don't collide, so `(tweak, input)` pairs are distinct.

**Why it's a latent assumption:** the correlation-robustness security argument relies on **input distinctness** rather than the more conservative property of **`(tweak, input)` distinctness via tweak alone**. The wide variants (`tensor_ops.rs:289-396`) explicitly avoid this fragility — they reserve bit 64 of the tweak as `WIDE_DOMAIN` (`tensor_ops.rs:267-273`), guaranteeing wide tweaks never collide with narrow tweaks regardless of input. The narrow path has no analogous guard.

**Required fix candidates (queued as (d)):** (a) document the input-distinctness argument explicitly in `tensor_ops.rs` comments so future refactors don't reorder/reuse tweaks; or (b) add explicit domain separation (e.g., reserve bit 63 of tweak: `INIT_DOMAIN`, `LOOP_DOMAIN`, `LEAF_DOMAIN`) to bring the narrow path in line with the wide path's hardening.

### C3 — `tensor_evaluator`'s `a_bits` must equal the cleartext bits underlying `a_macs`, but consistency between the two is not enforced

**Code (`src/tensor_macro.rs:138-187`):** `tensor_evaluator` takes `a_macs: &[Mac]` and `a_bits: &[bool]` as separate parameters. The function trusts the invariant `a_macs[i] = A_i ⊕ a_bits[i]·δ_gb` for some `A_i` matching the garbler's view; mismatch silently produces wrong `Z_eval` (no panic, no consistency check).

The doc comment (`src/tensor_macro.rs:126-129`) explains the parameter is decoupled "to allow the tree traversal to work even when the garbler's Δ has `lsb == 0`" — generalizing paper's `a_i = mac.lsb()` convention to support `δ.lsb=0` (used by `Δ_b` on the eval side per the project's split-delta convention). This is a deliberate spec extension of the paper, not a bug. `a_bits` length is asserted (`src/tensor_macro.rs:148`) but per-bit consistency with `a_macs` is not.

**Why it's a latent assumption:** if a caller passes mismatched `a_bits` (off-by-one indexing, accidentally swapped vectors from another auth-bit slot), eval traverses the wrong subtree and produces silently-wrong `Z_eval`. The fragility surfaces only at downstream CheckZero or output-equality assertions, not at the macro boundary.

**Existing callers** (`src/auth_tensor_eval.rs`, `src/leaky_tensor_pre.rs`) pair `a_macs` with the matching cleartext source consistently — invariant holds in production today. The fragility is in the macro contract, not in current call sites.

**Required fix candidates (queued as (d)):** (a) tighten the doc comment to make caller responsibility for `a_macs[i] ⊕ a_bits[i]·δ_gb = A_i` explicit; or (b) accept the deliberate-flexibility design and leave as-is (the function intentionally trades safety for δ-flexibility — adding a consistency check would require knowing δ, which the function avoids by design).

## (d) Required code changes

Queued as a follow-up sub-phase. Each item is a separate atomic commit per the Track 2 interaction model. Numbering reflects suggested execution order — earlier items are structural rewrites, later items are localized doc/assertion tweaks.

| # | Source finding | Scope | Notes |
|---|---|---|---|
| D1 | B1 + B2 | Rewrite `gen_populate_seeds_mem_optimized` + `eval_populate_seeds_mem_optimized` to implement paper's improved one-hot construction: paper recurrence `R_{i,j} := H(S_{i-1,j}, ν_{i,j})` with FreeXOR sibling derivation; init writes `S_{0,*}` directly without TCCR; emit `Vec<Block>` of length `n-1` for level cts (collapse the pair). Update `TensorMacroCiphertexts.level_cts` type. Update `tensor_macro.rs` doc comments. | Subsumes B2 (extra init TCCR vanishes naturally). Mirror updates needed in `gen_chunked_half_outer_product` / `eval_chunked_half_outer_product` since they reuse the same primitives — but those are audit 2.3's responsibility; coordinate. |
| D2 | B1 (downstream) | Update tests: add ciphertext-count assertion `g.level_cts.len() == n-1` with element type `Block` (not `(Block, Block)`). Existing protocol-invariant tests in `tensor_macro.rs::tests` continue to apply. | Should land in the same commit as D1 to keep tree green. |
| D3 | B1 (downstream) | Update bench fixtures and `assemble_e_input_wire_blocks_p1` callers if any encode the `(Block, Block)` shape. Audit `auth_tensor_gen.rs` / `auth_tensor_eval.rs` chunked-half wrappers for assumptions about pair-shape level cts. | Coordinate with audit 2.3 — if D1 lands first, audit 2.3 should consume the corrected shape. |
| D4 | C1 | Either (a) tighten the doc comment at `src/tensor_macro.rs:79` to drop the false "re-asserted here" claim; or (b) add `debug_assert!(a_blocks.iter().all(\|b\| !b.lsb()))` at `src/tensor_macro.rs:100`; or (c) remove the dead `if x[n-1].lsb()` branch in `gen_populate_seeds_mem_optimized:29-35` (preferred — simplifies; invariant is guaranteed by `Key::new`). | Independent of D1; can land separately. |
| D5 | C2 | Either (a) document the input-distinctness security argument in `src/tensor_ops.rs` near the tweak constants so future refactors don't reorder/reuse tweaks; or (b) add explicit domain separation (reserve high bit of tweak: `INIT_DOMAIN`, `LOOP_DOMAIN`, `LEAF_DOMAIN`) bringing the narrow path in line with the wide path's `WIDE_DOMAIN`. | Option (b) ideally lands together with D1 since D1 already rewrites the tweak structure — combining avoids two churns of the same code. |
| D6 | C3 | Tighten doc comment on `tensor_evaluator` to make the `a_macs[i] ⊕ a_bits[i]·δ_gb = A_i` invariant explicit as a caller responsibility. Decision required on whether to accept deliberate-flexibility design or add a consistency check. | Smallest-blast-radius change; can land standalone. |

### Coordination notes

- **D1 is the load-bearing item.** Most other Ds are documentation/assertion changes that can land independently; D1 is the structural rewrite that closes B1 + B2.
- **Defer to audit 2.3 before landing D1's chunked-wrapper updates.** Audit 2.3 covers `gen_chunked_half_outer_product` / `eval_chunked_half_outer_product` (the P1 chunking layer), which call into the same primitives. D1's primitive-level rewrite must be consistent with how the chunking wrappers consume `level_cts`. The clean ordering is: AUDIT-2.1 commit → AUDIT-2.3 commit → D1+D3 fix sub-phase.
- **D5 option (b) ideally lands with D1.** Combining the construction rewrite with explicit tweak-domain separation avoids a second churn of `tensor_ops.rs` for tweak reorganization.
