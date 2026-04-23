# Features Research — v1.1

**Domain:** Authenticated garbling with tensor gates (KRRW-style)
**Paper:** `references/Authenticated_Garbling_with_Tensor_Gates/CCS2026/`
**Researched:** 2026-04-23
**Confidence:** HIGH — all findings sourced directly from CCS2026 paper files

---

## Protocol 1: Unauthenticated Tensor Macros

**Paper reference:** `5_online.tex`, Construction (Tensor macros, §Tensor macros), Construction (Garbling and Evaluation Algorithms for Protocol 1), Protocol $\Pi_{2pc,1}$

### What it does

Protocol 1 is the KRRW-blueprint 2PC that optimizes **online communication**. It uses the tensor macro (Construction `tensor_garbler` / `tensor_evaluator`, already in v1.0) to propagate garbler-key shares (`[v_w Δ_gb]`) through the circuit. After evaluation, the evaluator reveals masked wire values `Λ_w` to the garbler and parties run `CheckZero` on `Δ_ev`-shares derived locally from preprocessed material.

The "unauthenticated" label in the milestone context refers to the fact that Protocol 1's tensor macro (`tensorgb`/`tensorev`) only propagates `Δ_gb`-shares — one key — not both keys simultaneously. Authentication of the computation is deferred to the post-evaluation `CheckZero` consistency check using `Δ_ev`-shares that were produced during preprocessing.

### Garble inputs / outputs

`garble(κ, C, [l_vec Δ_gb]^gb)`:
- Input: garbler's share of all preprocessed masks authenticated under `Δ_gb`
- Internally samples `[v_w Δ_gb]^gb` for input wires
- For each tensor gate `(α, β, γ, ⊗)`:
  - Computes `[Λ_α Δ_gb]^gb := [v_α Δ_gb]^gb XOR [l_α Δ_gb]^gb` (masked wire value share)
  - Invokes `tensorgb(n, m, Δ_gb, [Λ_α Δ_gb]^gb, [l_β Δ_gb]^gb)` → `(Z_{γ,0}, halfgate_{γ,0})`
  - Invokes `tensorgb(m, n, Δ_gb, [Λ_β Δ_gb]^gb, [v_α Δ_gb]^gb)` → `(Z_{γ,1}, halfgate_{γ,1})`
  - Sets `[v_γ Δ_gb]^gb := Z_{γ,0} XOR Z_{γ,1}^T XOR [l_γ* Δ_gb]^gb`
  - Sets `[Λ_γ]^gb := extbit([v_γ Δ_gb]^gb) XOR extbit([l_γ Δ_gb]^gb)`
- Output: garbled circuit `gc = {halfgate_{γ,0}, halfgate_{γ,1}, [Λ_γ]^gb}` for all tensor gates, plus `{[v_w Δ_gb]^gb}` for input wires

### Eval inputs / outputs

`eval(κ, C, gc, {(Λ_w, [v_w Δ_gb]^ev)}_inwires, [l_vec Δ_gb]^ev)`:
- Input: evaluator's share of masks, input masked values and evaluator key-shares, garbled circuit
- For each tensor gate:
  - Computes `[Λ_α Δ_gb]^ev := [v_α Δ_gb]^ev XOR [l_α Δ_gb]^ev`
  - Invokes `tensorev(n, m, Λ_α, halfgate_{γ,0}, [Λ_α Δ_gb]^ev, [l_β Δ_gb]^ev)` → `Z_{γ,0}`
  - Invokes `tensorev(m, n, Λ_β, halfgate_{γ,1}, [Λ_β Δ_gb]^ev, [v_α Δ_gb]^ev)` → `Z_{γ,1}`
  - Sets `[v_γ Δ_gb]^ev := Z_{γ,0} XOR Z_{γ,1}^T XOR [l_γ* Δ_gb]^ev`
  - Sets `Λ_γ := [Λ_γ]^gb XOR extbit([l_γ Δ_gb]^ev) XOR extbit([v_γ Δ_gb]^ev)`
- Output: `{Λ_w, [v_w Δ_gb]^ev}` for all tensor gate and output wires

### v1.0 dependency

`tensorgb`/`tensorev` (Construction 1 from v1.0 Phases 3/6) are the direct building blocks. The garble/eval algorithms above are the circuit-level wrappers that apply them gate-by-gate and compose them with preprocessing. The existing `AuthTensorGen` and `AuthTensorEval` skeletons in `auth_tensor_gen.rs` and `auth_tensor_eval.rs` implement parts of this logic but are incomplete — they handle the GGM/outer-product kernel but do not wire up the full gate-by-gate circuit traversal, input encoding, or Open() calls.

---

## Protocol 2: Authenticated Tensor Macros

**Paper reference:** `6_total.tex`, Construction (Authenticated tensor macros), Construction (Garbling and Evaluation Algorithms for Protocol 2), Protocol $\Pi_{2pc,2}$

### What it does

Protocol 2 optimizes **total communication** by using compressed preprocessing (`F_cpre`). The key difference from Protocol 1 is that the tensor macro is extended to simultaneously propagate both `Δ_gb`-shares and `Δ_ev`-shares through the circuit. This eliminates the need for the evaluator to reveal masked wire values `Λ_w` to the garbler during the consistency check; instead the garbler opens only its `Δ_ev`-shares of tensor-gate outputs, and the evaluator checks consistency via `CheckZero` on those, without ever sending masked values back to the garbler.

### Authenticated tensor macro: AuthTensor.Gb inputs / outputs

`AuthTensor.Gb(n, m, Δ_gb, [a_vec Δ_gb]^gb, [b_vec Δ_gb]^gb, [b_vec Δ_ev]^gb)`:
- Same GGM tree construction as `tensorgb`, but each leaf seed is expanded to `(κ + ρ)` bits instead of `κ` bits
- The `m` column ciphertexts are widened: `G_k := (XOR_l X_{l,k}) XOR (B_k || B'_k)` where `B_k` is the `Δ_gb`-share of `b_k` and `B'_k` is the `Δ_ev`-share of `b_k`
- Output: `(Z_gb || Z'_gb, G)` where `Z_gb ∈ {0,1}^(nm×κ)` and `Z'_gb ∈ {0,1}^(nm×ρ)` are the κ-prefix and ρ-suffix halves of the matrix product output

`AuthTensor.Ev(n, m, a_vec, G, [a_vec Δ_gb]^ev, [b_vec Δ_gb]^ev, [b_vec Δ_ev]^ev)`:
- Same GGM tree recovery as `tensorev`, but expands each recovered leaf to `(κ + ρ)` bits
- For the missing leaf `a`: `X_{a,k} := (XOR_{l≠a} X_{l,k}) XOR G_k XOR ((B_k XOR b_k Δ_gb) || (B'_k XOR b_k Δ_ev))`
- Output: `(Z_ev || Z'_ev)` split into `κ`- and `ρ`-halves

**Correctness invariant:** `Z_gb XOR Z_ev = (a ⊗ b) Δ_gb` AND `Z'_gb XOR Z'_ev = (a ⊗ b) Δ_ev`

### Garble inputs / outputs (Protocol 2)

`garble(κ, C, [l_vec Δ_gb]^gb, [l_vec Δ_ev]^gb)`:
- Takes both Δ_gb and Δ_ev shares as input (vs Protocol 1 which only takes Δ_gb shares)
- For tensor gate: invokes `AuthTensor.Gb` twice (same half-gate structure as Protocol 1)
- Additionally maintains `[v_γ Δ_ev]^gb` for all wires (propagated via XOR for free gates, via Z'_γ for tensor gates)
- Output: garbled circuit gc (same structure as Protocol 1), plus `{[v_γ Δ_ev]^gb}` for all tensor-gate output wires

### Eval inputs / outputs (Protocol 2)

`eval(κ, C, gc, {(Λ_w, [v_w Δ_gb]^ev, [v_w Δ_ev]^ev)}_inwires, [l_vec Δ_gb]^ev, [l_vec Δ_ev]^ev)`:
- Takes both Δ_gb and Δ_ev evaluator shares
- Propagates both families of shares gate-by-gate using `AuthTensor.Ev`
- Output: `{Λ_w, [v_w Δ_gb]^ev, [v_w Δ_ev]^ev}` for tensor-gate and output wires

### Communication overhead vs Protocol 1

The `2(n-1)` GGM tree ciphertexts remain κ bits. The `m` column ciphertexts widen from κ to `(κ + ρ)` bits — an overhead of `ρm` bits per `AuthTensor.Gb` call. Protocol 2's online cost is `(3(n+m)-4)(κ+ρ) + nm` bits per tensor gate (vs Protocol 1's `(3(n+m)-4)κ + 2nm`). Protocol 2 wins total communication because its preprocessing (compressed) is asymptotically cheaper.

### v1.0 dependency

Protocol 2 is an extension of Protocol 1. It requires Protocol 1's tensor macro as a foundation and adds the `Δ_ev` propagation layer. It also requires `F_cpre` (compressed preprocessing) rather than the uncompressed `F_pre` used in v1.0.

---

## Open() Function

**Paper reference:** `5_online.tex` (Protocol 1 step 3/4/8), `6_total.tex` (Protocol 2 step 3/4/7)

### What Open() does

`open([λ_w Δ])` is a two-party protocol that reveals the authenticated bit `λ_w` to one designated party. The paper invokes it in three distinct contexts:

**Protocol 1 — three uses:**
1. **Garbler's input masks:** parties run `open([l_w Δ_gb])` to reveal `l_w` to the garbler, who then sends `Λ_w = x_w XOR l_w` and `[v_w Δ_gb]^ev` to the evaluator (step 3)
2. **Evaluator's input masks:** parties run `open([l_w Δ_ev])` to reveal `l_w` to the evaluator, who sets `Λ_w = y_w XOR l_w` (step 4)
3. **Output decoding:** parties run `open([l_w Δ_ev])` to reveal `l_w` to the evaluator for each output wire, who outputs `z_w = Λ_w XOR l_w` (step 8)

**Protocol 2 — same three contexts** (steps 3, 4, 7).

### Open() mechanics

In the IT-MAC model used throughout this codebase:
- Each party holds a share: garbler holds `(key, value)` under `Δ_other`, evaluator holds `(mac, value_xor_key)`
- The MAC invariant is: `mac = key XOR bit * delta`
- To open `[λ_w Δ]` to party P: the other party sends their MAC; P reconstructs the bit and verifies the MAC equation

In this library's convention (`AuthBitShare`): `mac = key XOR bit * delta`, with `Key LSB = 0` and `Delta LSB = 1`. This means `extbit(mac) = bit XOR extbit(key) = bit` since `extbit(key) = 0` by invariant.

In-process: since there is no network layer, Open() for a single gate reduces to: XOR the two parties' shares to reconstruct the full IT-MAC, then extract the bit.

### What Open() does NOT do in v1.1

It does not handle F_bcot — the OT-based protocol for evaluator's input wire labels (`[v_w Δ_gb]^ev = [v_w Δ_gb]^gb XOR y_w Δ_gb`). That remains ideal/out-of-scope (v2.0). Open() only covers mask revelation, not input label transfer.

---

## Consistency Check

**Paper reference:** `5_online.tex` (Protocol 1 steps 5-7), `6_total.tex` (Protocol 2 steps 5-6)

### Protocol 1 consistency check

**What is checked:** For each tensor gate `(α, β, γ, ⊗)`, parties verify that the masked wire values revealed by the evaluator are consistent with the circuit's tensor-product semantics.

**Mechanics:**
1. Evaluator sends `Λ_w` for all `w ∈ inwires ∪ andwires` to the garbler (step 5)
2. For each tensor gate, both parties locally compute shares `[c_γ Δ_ev]` of:
   ```
   c_γ := (Λ_α XOR l_α) ⊗ (Λ_β XOR l_β) XOR (Λ_γ XOR l_γ)
        = v_α ⊗ v_β XOR v_γ
   ```
   This is a linear combination of `l_α, l_β, l_γ, l_γ* = l_α ⊗ l_β` with coefficients determined by the public `Λ` values, so `[c_γ Δ_ev]` is computed locally from preprocessed Δ_ev-shares.
3. Parties run `CheckZero({[c_γ Δ_ev]}_{γ ∈ andwires})` — evaluator aborts if any `c_γ ≠ 0`

**What it proves:** If the check passes, for every tensor gate the actual wire values satisfy `v_γ = v_α ⊗ v_β` — the circuit computed correctly. The `Δ_ev`-MAC prevents a malicious garbler from cheating because forging a passing MAC on a nonzero value requires knowledge of `Δ_ev`.

### Protocol 2 consistency check

**What is checked:** Same circuit-correctness property, but without the evaluator revealing `Λ_w` to the garbler.

**Mechanics:**
1. For each tensor-gate output wire γ, both parties locally compute `[Λ_γ Δ_ev] := [v_γ Δ_ev] XOR [l_γ Δ_ev]`
2. Evaluator sets its share `[c_γ]^ev := [Λ_γ Δ_ev]^ev XOR Λ_γ Δ_ev` (using its own recovered masked value)
3. Garbler sets its share `[c_γ]^gb := [Λ_γ Δ_ev]^gb`
4. Parties run `CheckZero({[c_γ]}_{γ ∈ andwires})`

**Why no revealed masked values:** Because Protocol 2 propagates `Δ_ev`-shares of wire values through the circuit (not just `Δ_gb`-shares), the evaluator can locally compute everything needed for the consistency check without sending `Λ_w` to the garbler. This is what makes Protocol 2 compatible with compressed preprocessing (whose correlations would not appear uniform to the garbler once masked values are revealed — see paper §Protocol 2 intro).

### CheckZero implementation note

In the in-process (no-network) setting: `CheckZero` on `[c Δ_ev]` collapses to: XOR the two parties' shares and verify the result is zero using the MAC equation. With `n` tensor-gate outputs, this is `n` MAC verifications.

---

## IdealPreprocessing

**Paper reference:** `5_online.tex` §Preprocess (step 1), `6_total.tex` §Preprocess (step 1); existing code in `auth_tensor_fpre.rs` (TensorFpre), `preprocessing.rs`

### Oracle semantics

`IdealPreprocessing` (also called `F_pre` in Protocol 1 or `F_cpre` in Protocol 2) is a trusted-dealer oracle that samples:
- Two global keys `Δ_gb ∈ {0,1}^κ` (with `LSB(Δ_gb) = 1`) and `Δ_ev ∈ {0,1}^ρ` (with `LSB(Δ_ev) = 1`)
- For every circuit wire `w ∈ inwires ∪ andwires ∪ outwires`: a mask `l_w ∈ {0,1}`, with the XOR constraint for free gates (`l_γ = l_α XOR l_β`)
- Dual-authenticated bits `([l_w Δ_gb], [l_w Δ_ev], [l_w])` for each wire — i.e., IT-MAC shares under both keys plus the plaintext bit, distributed to both parties
- For each tensor gate `(α, β, γ, ⊗)`: an additional dual-authenticated triple `([l_γ* Δ_gb], [l_γ* Δ_ev], [l_γ*])` with `l_γ* = l_α ⊗ l_β`

### What it produces vs real preprocessing

| Property | IdealPreprocessing | Real preprocessing (Pi_aTensor', v1.0) |
|---|---|---|
| Global keys | Sampled uniformly | Computed via F_bCOT protocol |
| Mask bits `l_w` | Sampled uniformly | Derived from Pi_LeakyTensor + combining |
| Triple `l_γ*` | Set exactly to `l_α ⊗ l_β` | Guaranteed by combining + bucket check |
| Dual-auth guarantee | By construction (oracle) | By IT-MAC invariant + F_eq abort |
| Wire correlations | Independent, random | Same; no difference in distribution for honest parties |
| Compressed-mode `l_w` | N/A | N/A for uncompressed; see compressed preprocessing below |

The existing `TensorFpre` in `auth_tensor_fpre.rs` is the current implementation of IdealPreprocessing for a single tensor gate. It produces: `TensorFpreGen` (garbler's shares) and `TensorFpreEval` (evaluator's shares).

For v1.1, IdealPreprocessing needs to be generalized to a circuit-level oracle: given a circuit description, it produces dual-authenticated shares for all wires and all tensor-gate triples simultaneously, not just one gate. This means extending `TensorFpre` or creating `CircuitFpre` that iterates over all gates and enforces the XOR-gate constraint `l_γ = l_α XOR l_β`.

### Preprocessing trait

The `Preprocessing` trait proposed for v1.1 provides a common interface so that IdealPreprocessing, Pi_aTensor, Pi_aTensor', and (if implemented) compressed preprocessing are interchangeable backends. The trait must produce the same structured output (`TensorFpreGen`, `TensorFpreEval` or their circuit-level equivalents) regardless of which protocol generated it.

---

## Compressed Preprocessing

**Paper reference:** `appendix_cpre.tex`, `6_total.tex` §Preprocess

### What it is

Compressed preprocessing (`F_cpre`) is an alternative preprocessing backend that generates AND/tensor triples from a short authenticated seed vector `b* ∈ {0,1}^L` (length `L = O(ρ log n)` rather than `n`) expanded by a public random matrix `M ∈ {0,1}^{n×L}`. The evaluator's masks `b = M · b*` are determined by the seed; only the seed is authenticated, not the full `b` vector. This reduces authentication cost from `O(n)` to `O(L) = O(ρ log n)` bits per gate.

The functionality `F_cpre` (appendix_cpre.tex, Functionality box) outputs:
- For garbler (party A): authenticated bits `([a_vec Δ_b], [a_hat_vec Δ_b])` where `a_vec` is A's mask, `a_hat_vec` is A's share of the product wire triple
- For evaluator (party B): authenticated bits `([b*_vec Δ_a], [b_hat_vec Δ_a])` where `b*` is the seed, `b_hat` is B's share of the product
- The public expansion matrix `M`

The actual `Pi_cpre` protocol from CWYY23 instantiates this using `F_cot + F_bcot + F_DVZK + F_EQ + F_Rand`. The paper notes (appendix_cpre.tex lines 3-9) that it borrows from CWYY23 with the compression parameter changed from `σ = 2ρ` to `σ = O(ρ log κ)` to achieve `2^-ρ`-selective failure resistance for tensor gates (vs standard AND gates).

### Tensor triples from compressed AND triples

The appendix explicitly states (lines 14-17): a tensor triple `(l_x, l_y, l_x ⊗ l_y)` is `nm` structured AND triples — entry `(i,j)` of the product is `(l_x)_i · (l_y)_j`. So one tensor triple uses `nm` AND triples from F_cpre. The amortized communication cost is 2 bits per AND triple (from CWYY23), so `2nm` bits per tensor triple — independent of the security parameter.

### Feasibility assessment for v1.1

**Verdict: MEDIUM complexity — feasible but substantial implementation effort.**

What is available in the paper:
- The ideal functionality `F_cpre` is fully specified (appendix_cpre.tex, Functionality box, lines 24-66)
- The protocol `Pi_cpre` from CWYY23 is referenced but only partially reproduced (the active code is the functionality; the protocol body is commented out)
- The tensor-triple reduction (nm AND triples per tensor triple) is described

What makes this hard:
- `Pi_cpre` requires `F_cot`, `F_bcot`, `F_DVZK`, `F_EQ`, `F_Rand` — only `F_bcot` (ideal) exists in v1.0
- The commented-out protocol body in `appendix_cpre.tex` (lines 81-156) is the actual `Pi_cpre` steps, suggesting the paper authors have not finalized it
- DVZK (designated-verifier zero-knowledge) is non-trivial to implement without a reference

**Recommended v1.1 scope:** Implement `F_cpre` as an ideal functionality (oracle), analogous to `IdealBCot` for `F_bcot`. Use it to produce dual-authenticated tensor triples in Protocol 2's preprocessing slot, without implementing the real `Pi_cpre` protocol. Real `Pi_cpre` is a v2.0 item alongside real OT.

### Difference from uncompressed preprocessing

| Property | Uncompressed (F_pre, v1.0) | Compressed (F_cpre, v1.1) |
|---|---|---|
| Evaluator's mask storage | Full `n`-bit vector per gate | Seed `b* ∈ {0,1}^L`, `L = O(ρ log n)` |
| Authentication cost | O(n) field elements | O(L) = O(ρ log n) field elements |
| Expansion | None | Public matrix `M ∈ {0,1}^{n×L}` sent in preprocessing |
| Compatibility | Protocol 1 consistency check (ok) | Protocol 2 only — masks not uniform after `Λ` reveal, incompatible with Protocol 1 |
| Real protocol dependencies | F_bcot only | F_cot + F_bcot + F_DVZK + F_EQ + F_Rand |
| v1.1 approach | Fully implemented (v1.0) | Ideal oracle only |

---

## Distributed Half Gates

**Paper reference:** `4_distributed_garbling.tex`, Construction (Distributed Half Gates `dhg`), Construction (Distributed Tensor Gates `dtg`)

### What they are

Distributed garbling is a different framing of the same authenticated garbling problem. In distributed garbling syntax, both the garbler and evaluator each hold a share of the garbled circuit `gc_gb` and `gc_ev` that are XORed together to form the actual garbled circuit. Neither party's share alone reveals anything.

**Distributed half gates (`dhg`):** the AND-gate special case, equivalent to the KRRW18 half-gates in distributed form. For each AND gate `(α, β, γ, ∧)`:
- Garbler computes `halfgate_{γ,0}^(gb)`, `halfgate_{γ,1}^(gb)`, `L_γ^(gb)` using IT-MAC shares and wire labels
- Evaluator computes `halfgate_{γ,0}^(ev) := [l_β Δ]^ev`, `halfgate_{γ,1}^(ev) := 0`, `L_γ^(ev) := extbit([l_γ Δ]^ev)`
- Combined ciphertext: `halfgate_{γ,b} := halfgate_{γ,b}^(gb) XOR halfgate_{γ,b}^(ev)`
- Evaluator computes label and masked value using the combined ciphertexts plus its IT-MAC shares

**Distributed tensor gates (`dtg`):** the tensor-gate generalization. For each tensor gate `(α, β, γ, ⊗)`:
- Garbler invokes `tensorgb(n, m, Δ, Label_{α,0} XOR [l_α Δ]^gb, [l_β Δ]^gb)` for the first half and `tensorgb(m, n, Δ, Label_{β,0} XOR [l_β Δ]^gb, Label_{α,0})` for the second half
- Evaluator's shares: `halfgate_{γ,0}^(ev) := [l_β Δ]^ev`, `halfgate_{γ,1}^(ev) := 0`, `L_γ^(ev) := [l_γ Δ]^ev`
- Evaluator runs `tensorev` with combined ciphertexts

### Key difference from Protocol 1/2

Distributed garbling (Section 4) and the 2PC protocols (Sections 5/6) describe the same underlying computation from different angles:
- `dtg` uses the label-based wire-label convention (`Label_{w,0}`, `Label_{w,1}`)
- Protocol 1/2 use the share-based convention (`[v_w Δ_gb]`, `Λ_w`)
- Both ultimately call the same `tensorgb`/`tensorev` primitives from v1.0

The paper treats `dtg` as the inner distributed garbling component and uses Protocol 1/2 as the outer 2PC wrapper that adds input encoding and consistency check. For v1.1 implementation purposes, `dtg` and Protocol 1's garble/eval algorithms are equivalent; the distributed framing is primarily a proof artifact.

### Benchmark comparison: naive tensor vs distributed tensor

The paper's communication complexities for one tensor gate `(n, m)`:

| Scheme | Online ciphertexts | Online bits |
|---|---|---|
| Naive (nm AND gates, half-gates each) | `2nm` ciphertexts | `2nm·κ` bits |
| dtg / Protocol 1 | `2(n-1+m)` GGM ciphertexts | `(3(n+m)-4)κ + 2nm` bits |
| Protocol 2 | same GGM structure, wider | `(3(n+m)-4)(κ+ρ) + nm` bits |

For `n = m = 4, κ = 128, ρ = 40`:
- Naive: `2·16 = 32` ciphertexts → `32·128 = 4096` bits
- dtg: `2(3+4) = 14` GGM ciphertexts + `16` column ciphertexts = 30 total → `(3·8-4)·128 + 2·16 = 20·128 + 32 = 2592` bits
- Protocol 2: `20·168 + 16 = 3376` bits total but preprocessing dominates

**Feasibility of benchmark:** HIGH. The existing `tensor_gen`/`tensor_eval` semi-honest family implements the naive approach. The v1.0 `tensor_macro.rs` implements the GGM approach. Both can be benchmarked with Criterion for wall-clock comparison. Communication can be measured by counting ciphertext bytes.

---

## Benchmarks

**Paper reference:** `5_online.tex` §Communication complexity, `6_total.tex` §Communication complexity, `appendix_cpre.tex` §Communication Complexity

### What to measure

**Wall-clock time (Criterion):**
- `garble` time: time for garbler to produce garbled circuit for a single tensor gate `(n, m)` under Protocol 1 (and Protocol 2 if implemented)
- `eval` time: time for evaluator to evaluate the garbled circuit for a single tensor gate
- `preprocessing` time: IdealPreprocessing setup time (v1.1: ideal oracle, so this should be fast; useful as baseline)
- `full_protocol` time: preprocessing + garble + eval end-to-end
- Sweep over `n = m ∈ {1, 2, 4, 8, 16}` to show exponential scaling in input size

**Communication (byte counts):**
- Garbled circuit size per tensor gate: `(2(n-1) + m)·κ` (GGM) + `nm` (column) + `nm` (masked value) bits for Protocol 1
- Compare to naive `nm` AND gates: `2nm·κ` bits
- Report both theoretical and measured (count bytes in test fixtures)

**Comparison point (dtg vs naive):**
- Instantiate a (4,4) tensor gate using (a) the v1.0 GGM tensor macro and (b) nm=16 independent half-gate calls
- Measure both garble and eval wall-clock time
- Report speedup ratio and communication reduction ratio

### How to present

Follow the existing `benchmarks.rs` pattern (Criterion groups). Add:
1. `bench_protocol1_garble` — sweeping `(n,m)` pairs
2. `bench_protocol1_eval`
3. `bench_ideal_preprocessing`
4. `bench_dtg_vs_naive_tensor` — fixed (4,4), compare approaches

Report wall-clock in µs (Criterion default), communication in kilobytes. Paper claims `(3(n+m)-4)κ + 2nm` bits online for Protocol 1 — verify experimentally that measured ciphertext sizes match this formula.

---

## Table Stakes vs Differentiators

### Table Stakes (must-have for v1.1)

These are required for a "complete demonstrable protocol" as stated in the milestone goal.

| Feature | Why Required | Complexity | v1.0 Dependency |
|---|---|---|---|
| IdealPreprocessing (circuit-level) | Protocol 1/2 require a preprocessing oracle; without it no online phase runs | Low | Extends `TensorFpre` from `auth_tensor_fpre.rs` |
| Preprocessing trait | Enables interchangeable backends; without it the oracle is hardwired and can't be swapped | Low | Interface over existing `run_preprocessing` and `TensorFpre` |
| Protocol 1 garble/eval | Core online phase; what all the v1.0 preprocessing was built to feed | Medium | `tensorgb`/`tensorev` (Construction 1), `TensorFpreGen/Eval` |
| Open() function | Required by Protocol 1 and Protocol 2 for input encoding and output decoding | Low | `AuthBitShare`, MAC invariant already correct in v1.0 |
| Consistency check (Protocol 1) | Required for malicious security; without it the protocol is semi-honest only | Low-Medium | `Δ_ev`-shares from preprocessing |
| Wall-clock benchmarks | Milestone goal explicitly; paper claims `(3(n+m)-4)κ` bits and should be verified | Low | Criterion already in codebase |

### Differentiators (valuable but not blocking)

| Feature | Value | Complexity | v1.0 Dependency |
|---|---|---|---|
| Protocol 2 garble/eval | Total-comm optimization; needed to justify compressed preprocessing | Medium | Requires Protocol 1 as base + `Δ_ev` propagation layer |
| Consistency check (Protocol 2) | Cleaner (no masked-value reveal to garbler) but requires Protocol 2 | Medium | Protocol 2 garble/eval |
| Compressed preprocessing (ideal F_cpre) | Required to run Protocol 2; enables v2 real implementation | Medium | Needs `Δ_ev`-aware triple format |
| Distributed half gates (dhg) | Benchmark baseline for comparison; theoretical completeness | Low-Medium | `AuthBitShare`, `tensorgb`/`tensorev` |
| dtg vs naive benchmark | Shows concrete speedup from tensor macro | Low | Both approaches available in v1.0 |

### Anti-Features (explicitly out of scope)

| Anti-Feature | Why Avoid | What to Do Instead |
|---|---|---|
| Real Pi_cpre protocol | Requires F_DVZK which does not exist; CWYY23 protocol body commented out in paper | Use ideal F_cpre oracle |
| Real F_bcot / F_cot | V2.0 item; networking not in scope | Keep ideal `IdealBCot` |
| Multi-gate / multi-circuit evaluation | Out of scope per PROJECT.md; focus on single tensor gate benchmarking | Parametric sweep over gate sizes |
| Security proof infrastructure | Proof-of-concept implementation only; malicious simulation not verified | Correct honest-party execution only |

### Dependency ordering for implementation

```
IdealPreprocessing (circuit-level)
    └─> Preprocessing trait
            └─> Open()
                    └─> Protocol 1 garble/eval
                                └─> Consistency check (Protocol 1)
                                        └─> Benchmarks
                                                └─> dtg vs naive comparison

Protocol 1 garble/eval
    └─> Protocol 2 garble/eval (add Δ_ev propagation)
                └─> Compressed preprocessing (ideal F_cpre)
                            └─> Consistency check (Protocol 2)
```

---

## Sources

- `5_online.tex`: Protocol 1 full specification (Construction tensor-macro, Construction garble/eval, Protocol krrw)
- `6_total.tex`: Protocol 2 full specification (Construction auth-tensor-macro, Construction garble/eval, Protocol wrk)
- `appendix_cpre.tex`: F_cpre functionality and Pi_cpre reference (CWYY23)
- `4_distributed_garbling.tex`: dhg and dtg constructions; definitions of correctness, obliviousness, selective-failure resistance
- `src/auth_tensor_gen.rs`, `src/auth_tensor_fpre.rs`, `.planning/codebase/STRUCTURE.md`: v1.0 codebase state
- `PROJECT.md`: v1.1 milestone scope and out-of-scope items
