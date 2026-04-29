# AUDIT 2.2 — Π_LeakyTensor + bucketing (Constructions 2/3/4)

## Scope

**Paper:**
- Construction 2 (`Π_LeakyTensor`): `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/appendix_krrw_pre.tex:246-280` `prot:ltensor`, with correctness theorem at `:278-311` and security theorem at `:313-359`.
- Construction 3 (two-to-one combining): `appendix_krrw_pre.tex:361-379` `sec:bucketing-tensor`.
- Construction 4 (`Π_pre` randomized bucketing): `appendix_krrw_pre.tex:381-400` `prot:pre`, with bucketing lemma at `:402-470` `lem:bucketing`.
- F_eq ideal-functionality dependence: cited from `prot:ltensor` `consistency check` step (`:273-275`).

**Code:**
- `src/leaky_tensor_pre.rs` — `LeakyTensorPre` orchestrator + `LeakyTriple` output struct + `generate()` body.
- `src/auth_tensor_pre.rs` — `two_to_one_combine` (Construction 3), `bucket_size_for` (Construction 4 formula), `combine_leaky_triples` (full bucketing pipeline + Fisher-Yates permutation), `apply_permutation_to_triple` (asymmetric x-axis permutation).
- `src/feq.rs` — F_eq ideal-functionality stub (panic-on-mismatch single-process simulation).
- `src/sharing.rs::verify_cross_party` (`:144-162`) — in-process substitute for paper's "publicly reveal with appropriate MACs."
- `src/preprocessing.rs::run_preprocessing` (`:233-254`) — top-level orchestrator that strings Construction 2 → Construction 4 together.

**Out of scope (deferred to other audits):**
- Construction 1 / generalized tensor macro called inside `Π_LeakyTensor` (`tensor_garbler` / `tensor_evaluator`) — already covered in `AUDIT-2.1.md`.
- `AuthTensorGen` / `AuthTensorEval` consumption of `TensorFpreGen` / `TensorFpreEval` — audit 2.3 (Protocol 1).
- `derive_sharing_blocks` Block-form lowering post-bucketing — not paper-side; project-internal Block-form fields. Touch only insofar as `run_preprocessing` calls it (audit 2.3 covers downstream consumers).

## (a) Matches

Three constructions audited. Conventions: paper uses `Δ_gb` / `Δ_ev` for the two parties' global keys; project-internal naming is `δ_a` / `δ_b` (= `bcot.delta_a` / `bcot.delta_b`). LSB convention: `δ_a.lsb()=1`, `δ_b.lsb()=0`, `(δ_a ⊕ δ_b).lsb()=1` — satisfying paper's `lsb(Δ_gb ⊕ Δ_ev) = 1` precondition (per SCAFFOLDING cross-cutting facts).

### Construction 2 — `Π_LeakyTensor` (`src/leaky_tensor_pre.rs::generate`)

| Paper step (`appendix_krrw_pre.tex`) | Code | Match |
|---|---|---|
| Correlated randomness: sample `Δ_gb, Δ_ev` with `lsb(Δ_gb ⊕ Δ_ev) = 1`; obtain `[x_gb·Δ_ev], [x_ev·Δ_gb], [y_gb·Δ_ev], [y_ev·Δ_gb], ⟨R⟩` from `F_bcot` (`:249`) | `IdealBCot::new` enforces `δ_a.lsb()=1` / `δ_b.lsb()=0` (`bcot.rs`); 6 bCOT calls (`leaky_tensor_pre.rs:99-104`) — three `transfer_a_to_b` (A is sender on δ_a) and three `transfer_b_to_a` (B is sender on δ_b) for x, y, R; cross-party `AuthBitShare` assembly at lines `:107-136` | ✓ matches |
| `[C]^gb := y_gb·Δ_gb ⊕ [y_ev·Δ_gb]^gb ⊕ [y_gb·Δ_ev]^gb` (`:251`) | `c_a[j] = y_a_term ⊕ gen_y_shares[j].key ⊕ gen_y_shares[j].mac` (`:158-164`) where `y_a_term = if y_a then δ_a else 0`, `gen_y.key = cot_y_a_to_b.sender_keys[j] = [y_ev·δ_a]^gb`, `gen_y.mac = cot_y_b_to_a.receiver_macs[j] = [y_gb·δ_b]^gb` | ✓ matches |
| `[C]^ev` (analogous, with A↔B swapped) (`:254`) | `c_b[j] = y_b_term ⊕ eval_y.mac ⊕ eval_y.key` (`:165-169`); `eval_y.mac = [y_ev·δ_a]^ev`, `eval_y.key = [y_gb·δ_b]^ev` | ✓ matches |
| `[C^(R)]` analogously with R replacing y (`:252`) | `c_a_r[k] = r_a_term ⊕ gen_r.key ⊕ gen_r.mac` and `c_b_r[k] = r_b_term ⊕ eval_r.mac ⊕ eval_r.key` (`:172-187`); length `n·m` indexed `k = j·n + i` (column-major) | ✓ matches |
| Tensor macro #1: `(Z_gb,1, G_1) := tensorgb(n, m, Δ_gb, [x_ev·Δ_gb]^gb, [C]^gb)`; ev computes `E_1 := tensorev(n, m, x_ev, G_1, [x_ev·Δ_gb]^ev, [C]^ev)` (`:258-262`) | `tensor_garbler(n, m, δ_a, &cot_x_a_to_b.sender_keys, &t_a)` returns `(z_gb1, g_1)`; `tensor_evaluator(n, m, &g_1, &cot_x_a_to_b.receiver_macs, &x_b_bits, &t_b)` returns `e_1` (`:205-215`) | ✓ matches (subject to AUDIT-2.1 findings on the macro internals) |
| Tensor macro #2: `(Z_gb,2, G_2) := tensorgb(n, m, Δ_ev, [x_gb·Δ_ev]^ev, [C]^ev)`; gb computes `E_2 := tensorev(n, m, x_gb, G_2, [x_gb·Δ_ev]^gb, [C]^gb)` (`:263-267`) | `tensor_garbler(n, m, δ_b, &cot_x_b_to_a.sender_keys, &t_b)` returns `(z_gb2, g_2)`; `tensor_evaluator(n, m, &g_2, &cot_x_b_to_a.receiver_macs, &x_a_bits, &t_a)` returns `e_2` (`:221-231`) | ✓ matches; eval-side decoding under `δ_b.lsb()=0` requires explicit `a_bits` parameter — see AUDIT-2.1 C3 |
| Reveal: `S_1 := Z_gb,1 ⊕ E_2 ⊕ [C^(R)]^gb`; `S_2 := Z_gb,2 ⊕ E_1 ⊕ [C^(R)]^ev`; both send `lsb(S_·)`; `D := lsb(S_1) ⊕ lsb(S_2)` (`:270-271`) | `s_1 = (z_gb1 ⊕ e_2) ⊕ c_a_r_mat`; `s_2 = (z_gb2 ⊕ e_1) ⊕ c_b_r_mat`; `d_bits[k] = s_1[(i,j)].lsb() ^ s_2[(i,j)].lsb()` (`:250-262`) | ✓ matches |
| Consistency check: `L_1 := S_1 ⊕ D·Δ_gb`; `L_2 := S_2 ⊕ D·Δ_ev`; `F_eq(L_1, L_2)`; abort on mismatch (`:273-275`) | `l_1[(i,j)] = s_1[(i,j)] ^ (d ? δ_a : 0)`; `l_2[(i,j)] = s_2[(i,j)] ^ (d ? δ_b : 0)`; `feq::check(&l_1, &l_2)` panics on mismatch (`:269-282`) | ✓ matches — `feq::check` is the in-process F_eq stub (see C1) |
| Output `⟨Z⟩ := ⟨R⟩ ⊕ ⟨D⟩` (`:275`) | `gen_z[k] = gen_r[k] + gen_d` where `gen_d = {key=0, mac=if d {δ_b} else 0, value=d}`; `eval_z = eval_r` (`:298-309`) | ✓ matches — asymmetric ⟨D⟩ assignment is a valid additive sharing of public bit D (see C4) |

### Construction 3 — two-to-one combining (`src/auth_tensor_pre.rs::two_to_one_combine`)

| Paper step (`appendix_krrw_pre.tex:361-379`) | Code | Match |
|---|---|---|
| Public reveal `d := y' ⊕ y'' ∈ {0,1}^m` with MACs (`:368`) | `gen_d[j] = prime.gen_y[j] + dprime.gen_y[j]`, `eval_d[j] = prime.eval_y[j] + dprime.eval_y[j]`; `verify_cross_party(gen_d[j], eval_d[j], &δ_a, &δ_b)` per `j ∈ [m]`; `d_bits[j] = gen_d[j].value ^ eval_d[j].value` (`:53-66`) | ✓ matches — `verify_cross_party` is the in-process MAC reveal substitute (see C2) |
| `⟨x⟩ := ⟨x'⟩ ⊕ ⟨x''⟩` (`:368`) | `gen_x[i] = prime.gen_x[i] + dprime.gen_x[i]`; `eval_x[i] = prime.eval_x[i] + dprime.eval_x[i]` (`:69-74`) | ✓ matches |
| `⟨y⟩ := ⟨y'⟩` (`:368`) | `gen_y = prime.gen_y_shares` (moved); `eval_y = prime.eval_y_shares` (`:103-104`) | ✓ matches |
| `⟨Z⟩ := ⟨Z'⟩ ⊕ ⟨Z''⟩ ⊕ ⟨x''⟩ ⊗ d` — public `d` makes the rightmost term local (`:374-377`) | column-major `(i, j) ↦ k = j·n + i`: `gen_z[k] = prime.gen_z[k] + dprime.gen_z[k] + (if d_bits[j] then dprime.gen_x[i] else 0)`; eval analogous (`:79-99`) | ✓ matches — when `d_j = 0`, the rightmost term is `AuthBitShare::default()` (zero share) |

### Construction 4 — randomized bucketing `Π_pre` (`src/auth_tensor_pre.rs::bucket_size_for`, `combine_leaky_triples`, `apply_permutation_to_triple`)

| Paper step (`appendix_krrw_pre.tex:387-400`) | Code | Match |
|---|---|---|
| Bucket size `B := 1 + ⌈ssp/log(n*·ℓ)⌉`, `ssp = 40` (`:390`) | `bucket_size_for(n, ell)`: `1 + (SSP + log2_floor(n·ell) - 1) / log2_floor(n·ell)` for `n·ell ≥ 2`; fallback `SSP = 40` for `n·ell ≤ 1` (`:137-145`) | ✓ matches; fallback is reasonable defense for log-of-≤1 — see C5 |
| Sample `Δ_gb, Δ_ev` (`:393`) | Done by `IdealBCot::new` upstream of `LeakyTensorPre`; not re-sampled inside `combine_leaky_triples` | ✓ matches (split across modules) |
| Produce `⟨r⟩` for `r ← {0,1}^N` via auth-bit gen (`:394`) | Not inside `combine_leaky_triples` — input-wire authenticated bits are produced by `src/input_encoding.rs::encode_inputs` per project memory (see C3) | ✓ matches functionally (architectural split) |
| Invoke `F_LeakyTensor(n*, m*)` `ℓB` times (`:395-396`) | `run_preprocessing` calls `LeakyTensorPre::generate` `bucket_size` times (i.e., `ℓB` for `ℓ=1`) sharing one `IdealBCot` instance for matching `δ_a, δ_b` | ✓ matches structurally; **`ℓ` hardcoded to 1 — see B1** |
| Sample uniform partition of `[ℓB]` into `ℓ` buckets `B_k` of size `B` via shared randomness (`:397`) | No partition logic; single bucket trivially | ✗ deviates — see B1 (`ℓ=1` collapses partition step) |
| Sample independent uniform `π_j ∈ S_{n*}` per `j` via shared randomness (`:397`) | Per-triple `ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j))`; `Vec<usize>::(0..n).collect()` shuffled via `SliceRandom::shuffle` (Fisher-Yates) (`:207-212`) | ✗ deviates — `shuffle_seed` is hardcoded to 42 in the only call site, see B3 |
| Apply `π_j` to `⟨x^(j)⟩` and rows of `⟨Z^(j)⟩`; `⟨y^(j)⟩` untouched (`:397`) | `apply_permutation_to_triple`: `new_x[i] = old_x[perm[i]]`; for each column `j ∈ [m]`, `new_z[j·n + i] = old_z[j·n + perm[i]]`; y unchanged (`:286-317`) | ✓ matches — passive-interpretation `new[i] = old[perm[i]]` works regardless of whether `perm` represents `π` or `π^{-1}` since both are uniform |
| Per `B_k`, iterate Construction 3 across `B` permuted triples → `(⟨x_k⟩, ⟨y_k⟩, ⟨Z_k⟩)` (`:398`) | `acc = triples[0].clone()`; `for next in triples[1..] { acc = two_to_one_combine(acc, next); }` (`:217-220`) | ✓ matches |
| Truncate t-th combined triple to `(n_t, m_t)` (`:398`) | No truncation logic | ✗ deviates — see B1 (single output triple = no truncation needed at `ℓ=1`) |

### Correctness invariant verification

Paper Theorem (`appendix_krrw_pre.tex:278-311`): `Π_LeakyTensor` correctly realizes `F_LeakyTensor` when both parties are honest. Reduces to `S_1 ⊕ S_2 = (x ⊗ y ⊕ R)(Δ_gb ⊕ Δ_ev)` and (under `lsb(Δ_gb ⊕ Δ_ev) = 1`) `D = x ⊗ y ⊕ R`, so `⟨Z⟩ = ⟨R⟩ ⊕ ⟨D⟩` satisfies `Z = x ⊗ y`.

Lemma `lem:bucketing` (`:402-470`): bucketed output is `2^{-ssp}`-close to ℓ independent uniform authenticated tensor triples, given `B = 1 + ⌈ssp/log(n·ℓ)⌉`.

Existing tests verify the protocol-level invariants: `test_leaky_triple_product_invariant` (Construction 2), `test_two_to_one_combine_product_invariant` (Construction 3), `test_combine_full_bucket_product_invariant` and `test_run_preprocessing_product_invariant_construction_4` (full pipeline), plus MAC invariants via `verify_cross_party` on every output share. Negative tests cover F_eq abort on tampered transcript and MAC-mismatch panic on tampered y'' value.

## (b) Deviations

### B1 — `Π_pre` runs in single-bucket / `ℓ=1` mode only; paper specifies batched `ℓ ≥ 1` with partition + truncation steps

**Paper (`appendix_krrw_pre.tex:387-400`):**
- Inputs: authenticated-bit count `N ∈ ℕ` and tensor-triple dimensions `{(n_t, m_t)}_{t ∈ [T]}`. Set `n* := max_t n_t`, `m* := max_t m_t`, `ℓ := T`, `B := 1 + ⌈ssp/log(n*·ℓ)⌉`.
- Step 3: parties invoke `F_LeakyTensor(n*, m*)` a total of `ℓB` times → `{(⟨x^(j)⟩, ⟨y^(j)⟩, ⟨Z^(j)⟩)}_{j=1}^{ℓB}`.
- Step 4: via shared randomness, sample a uniform partition of `[ℓB]` into `ℓ` buckets `B_1, …, B_ℓ` of size `B`, plus independent uniform `π_j ∈ S_{n*}` for each `j`. Apply `π_j` locally to `⟨x^(j)⟩` and rows of `⟨Z^(j)⟩`.
- Step 5: per bucket `B_k`, iterate combining (Construction 3) across the `B` permuted triples to produce `(⟨x_k⟩, ⟨y_k⟩, ⟨Z_k⟩)`. **Truncate the t-th combined triple to `(n_t, m_t)` by discarding unused coordinates.**

**Code (`src/preprocessing.rs:233-254`):**
```rust
pub fn run_preprocessing(n: usize, m: usize, chunking_factor: usize)
    -> (TensorFpreGen, TensorFpreEval)
{
    let bucket_size = bucket_size_for(n, 1);  // ℓ hardcoded to 1
    let mut bcot = IdealBCot::new(0, 1);
    let mut triples = Vec::with_capacity(bucket_size);
    for t in 0..bucket_size {
        let mut ltp = LeakyTensorPre::new((t + 2) as u64, n, m, &mut bcot);
        triples.push(ltp.generate());
    }
    let (mut gen_out, mut eval_out) =
        combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42);
    /* … Block-form lowering … */
    (gen_out, eval_out)
}
```
- Produces exactly **one** authenticated tensor triple per call.
- `ℓ = 1` hardcoded into `bucket_size_for(n, 1)`. No `ell` parameter exposed.
- No partition logic (one bucket = trivial partition).
- No truncation step (input `(n, m)` = output `(n, m)`; `n* = max_t n_t = n`, `m* = max_t m_t = m`).
- `TensorPreprocessing::run` trait signature accepts `count` but `run_preprocessing` ignores it; `count != 1` panics per CONCERNS CORR-05 (`src/preprocessing.rs:220-227`).

**Implications:**
- For the project's current goal (single-tensor-gate protocol demonstration), `ℓ = 1` is sufficient — every test/bench currently produces one triple.
- For benchmark accuracy or eventual realistic deployment, missing — paper's amortization claim (`B = 1 + ⌈40/log(n·ℓ)⌉` shrinks with larger `ℓ`) cannot be exercised. At `ℓ = 1`: `B = 1 + ⌈40/log₂(n)⌉` (e.g., `B = 21` at `n = 4`, `B = 11` at `n = 16`); paper's lemma would give `B = 1 + ⌈40/log₂(n·ℓ)⌉` shrinking to `B = 5` at `ℓ = 2^{20}, n = 4`, etc.
- Project decision (per SCAFFOLDING.md and the Phase 7 PRE-04 / D-12 notes) is to defer batch-mode preprocessing until after the single-tensor-gate protocol is stable.

**Required fix (queued as (d)):** expose `ell` parameter on `run_preprocessing`; implement Step 4 partition + Step 5 per-bucket combining; thread `n* = max_t n_t` / `m* = max_t m_t` through `LeakyTensorPre::generate` and add per-output truncation. Coordinate with B2 (preprocessing chunking) — the same API surface needs to land both.

### B2 — Preprocessing has no chunking; `bench_preprocessing` is dormant

SCAFFOLDING-flagged.

**Code (`src/leaky_tensor_pre.rs:205-215`):**
```rust
let (z_gb1, g_1) = tensor_garbler(
    self.n, self.m, self.bcot.delta_a,
    &cot_x_a_to_b.sender_keys,
    &t_a,
);
```
`LeakyTensorPre::generate` invokes `tensor_garbler` with the full `n` directly, allocating a `2^n` GGM-leaf buffer in `gen_populate_seeds_mem_optimized` (`src/tensor_ops.rs:22`).

**P1 contrast (`src/auth_tensor_gen.rs::gen_chunked_half_outer_product`):** tiles via `chunking_factor ∈ 1..=8`, splitting the `2^n` work into `2^{n/cf}` tiles of `2^cf` leaves each.

**Project invariant (cross-cutting fact in SCAFFOLDING — "Chunking-size matching invariant"):** when chunked preprocessing lands, the `chunking_factor` used by preprocessing **MUST match** the `chunking_factor` used by P1 for the same matrix; mismatched chunking would yield triples whose tile boundaries don't align with what P1 consumes.

**Bench impact:** `benches/benchmarks.rs:581` declares `criterion_main!(online_benches)` only — `preprocessing_benches` is defined but never registered. Re-enabling at any current `BENCHMARK_PARAMS` (n=64, 128, 256) would OOM under the no-chunking implementation.

**Required fix (queued as (d)):** design + implement chunked `LeakyTensorPre` with `cf`-parameterized API matching P1's `chunking_factor`. Track as a future phase, **not** during this audit.

### B3 — Hardcoded `shuffle_seed = 42` in `combine_leaky_triples`

Already flagged as **SEC-05** (Phase 3.3 hardening) per SCAFFOLDING.

**Code (`src/preprocessing.rs:254`):**
```rust
let (mut gen_out, mut eval_out) =
    combine_leaky_triples(triples, bucket_size, n, m, chunking_factor, 42);
```
Literal `42` passed as `shuffle_seed`. Inside `combine_leaky_triples` (`src/auth_tensor_pre.rs:208`):
```rust
let mut rng = ChaCha12Rng::seed_from_u64(shuffle_seed.wrapping_add(j as u64));
```

**Paper (`appendix_krrw_pre.tex:397`):** "via shared randomness, parties sample … independent uniform `π_j ∈ S_{n*}`."

**In-process simulation:** deterministic seed is acceptable — both parties trivially "agree" on the seed since they execute in the same process.

**Real two-party deployment:** parties must agree via coin-tossing or an `F_Rand` ideal functionality.

**Required fix (queued as (d)):** replace literal `42` with a coin-tossed / `F_Rand`-derived seed when the protocol moves to a multi-process implementation. Documentation-only note in code comment until then.

## (c) Latent assumptions

### C1 — `feq::check` is the paper-equivalent ideal F_eq for in-process simulation

`src/feq.rs:19-32` panics with `"F_eq abort: …"` on element-wise BlockMatrix mismatch. Paper's `F_eq` aborts on mismatch — this stub matches the abort semantics. The TODO comment (`feq.rs:8-9`) flags it for replacement with a real equality-check protocol (e.g., commit-and-open hash) in production.

SCAFFOLDING explicitly says: "acceptable for in-process simulation; document explicitly, do NOT replace with cryptographic implementation in this audit." Listing only for completeness — no fix queued.

### C2 — `verify_cross_party` short-circuits the paper's two-message "publicly reveal with MACs" cycle

`src/sharing.rs:144-162` reconstructs both parties' views and verifies both MAC equations locally:
```rust
AuthBitShare { key: eval_share.key, mac: gen_share.mac, value: gen_share.value }.verify(delta_b);
AuthBitShare { key: gen_share.key, mac: eval_share.mac, value: eval_share.value }.verify(delta_a);
```

In a real two-party setting, this would be: A sends `value` → B verifies MAC under `δ_a`; B sends `value` → A verifies MAC under `δ_b`. Functionally equivalent for honest-execution; semantically short-circuited (both views accessible to the verifier in-process). Documented in `sharing.rs:140-143`. Same simulation envelope as C1.

Listing only for completeness — no fix queued.

### C3 — Paper's `Π_pre` Step 2 (auth-bit gen for `r ← {0,1}^N`) is split off into `src/input_encoding.rs::encode_inputs`

Paper packs auth-bit generation and tensor-triple generation in a single `Π_pre` invocation (`appendix_krrw_pre.tex:394-396`). Project's three-phase split (preprocessing → input encoding → garbling, per project memory `project_input_encoding_phase.md`) decomposes them. The auth-bit gen for input wires (`r ← {0,1}^N`) is realized in `src/input_encoding.rs::encode_inputs` (commit `74e627a`), not in `run_preprocessing`.

Functionally equivalent; architectural choice. Listing only for completeness — no fix queued.

### C4 — Asymmetric `⟨D⟩` assignment in `LeakyTensorPre::generate`

**Code (`src/leaky_tensor_pre.rs:298-309`):**
```rust
let gen_z_shares: Vec<AuthBitShare> = (0..(self.n * self.m)).map(|k| {
    let mac_block = if d_bits[k] { delta_b_block } else { Block::ZERO };
    let gen_d = AuthBitShare { key: Key::default(), mac: Mac::new(mac_block), value: d_bits[k] };
    gen_r_shares[k] + gen_d
}).collect();
let eval_z_shares: Vec<AuthBitShare> = eval_r_shares;
```

gen holds `{key=0, mac=if d {δ_b} else 0, value=d}`; eval holds `{key=0, mac=0, value=0}`.

**Cross-party MAC invariant (verified):**
- `mac(gen_D) = key(eval_D) ⊕ value(gen_D)·δ_b` ⇒ `d·δ_b = 0 ⊕ d·δ_b` ✓
- `mac(eval_D) = key(gen_D) ⊕ value(eval_D)·δ_a` ⇒ `0 = 0 ⊕ 0·δ_a` ✓

**Total value across parties:** `d ⊕ 0 = d` ✓ — valid additive sharing of public bit `D`.

Code comment at `:286-297` documents this as the "TEST-02-corrected A1 convention" with reasoning for why a symmetric split would break MAC verification of the combined `⟨Z⟩ = ⟨R⟩ ⊕ ⟨D⟩`. Listing only for completeness — no fix queued.

### C5 — `bucket_size_for(n·ℓ ≤ 1)` falls back to `SSP = 40`

**Code (`src/auth_tensor_pre.rs:137-145`):**
```rust
pub fn bucket_size_for(n: usize, ell: usize) -> usize {
    const SSP: usize = 40;
    let product = n.saturating_mul(ell);
    if product <= 1 { return SSP; }
    let log2_p = (usize::BITS - product.leading_zeros() - 1) as usize;
    1 + (SSP + log2_p - 1) / log2_p
}
```

Paper's formula `B := 1 + ⌈ssp/log(n*·ℓ)⌉` (`:390`) is undefined at `log₂(0)` and gives `B → ∞` at `log₂(1) = 0`. `SSP = 40` is the §3.1 preamble's "iterating over ssp triples drives guessing to `2^{-ssp}`" baseline (no-bucketing-amplification case). Existing test `test_bucket_size_formula_edge_cases` covers `n·ell ∈ {0, 1}`.

Reasonable defense at the formula's domain boundary; not a paper deviation. Listing only for completeness — no fix queued.

## (d) Required code changes

Queued as a follow-up sub-phase. Each item is a separate atomic commit per the Track 2 interaction model. Numbering reflects suggested execution order.

| # | Source finding | Scope | Notes |
|---|---|---|---|
| D1 | B1 + B2 | Implement batched `Π_pre`: expose `ell` parameter on `run_preprocessing` (and `TensorPreprocessing` trait); add Step 4 partition logic over `[ℓB]`; thread `n* = max_t n_t` / `m* = max_t m_t` through `LeakyTensorPre::generate`; add per-output truncation in `combine_leaky_triples`; couple with chunked `LeakyTensorPre` (`cf`-parameterized API). | Coordinated with B2 — chunked preprocessing and batched preprocessing share the same API surface; should land together as one phase. Subsumes the CONCERNS CORR-05 (`count != 1` panic) item. |
| D2 | B2 (downstream) | Re-register `preprocessing_benches` in `criterion_main!` (`benches/benchmarks.rs:581`) once D1 lands; verify chunking-size matching invariant via a bench that pairs preprocessing-cf with P1-cf. | Depends on D1. |
| D3 | B3 | When the protocol moves to a multi-process implementation: replace literal `shuffle_seed = 42` (`src/preprocessing.rs:254`) with a coin-tossed / `F_Rand`-derived seed. Until then, add a `// SEC-05: deterministic seed; replace with shared randomness in production` comment at the call site. | Documentation-only fix in the current codebase; full fix gated on multi-process work (per SCAFFOLDING SEC-05 / Phase 3.3 hardening). |
| D4 | C1 + C2 | When the protocol moves to a multi-process implementation: replace `feq::check` with a real equality-check protocol (commit-and-open hash, per `feq.rs:8-9` TODO); replace `verify_cross_party` with explicit reveal-and-verify message exchange. | Documentation-only until multi-process work begins. |

### Coordination notes

- **D1 is the load-bearing item.** It opens up batched preprocessing — paper's amortization claim (`B = 1 + ⌈40/log(n·ℓ)⌉` shrinks with larger `ℓ`) becomes exercisable. Without it, the project cannot reproduce the paper's headline preprocessing throughput numbers.
- **D2 + D1 must land together for benchmark coherence.** Re-registering `preprocessing_benches` without chunking would OOM at any production matrix size; without batching, the bench measures only single-triple latency, not amortized throughput.
- **D3 + D4 are paired with the multi-process transition** (separate phase, deferred to v2 per `STATE.md` Deferred Items).
- **AUDIT-2.1 D1 (Construction 1 paper-faithful rewrite) is upstream of B1's bucketing pipeline.** Reordering: AUDIT-2.1 D1 → AUDIT-2.3 → AUDIT-2.2 D1+D2 keeps the chunking-size matching invariant intact.
