---
phase: 02-m1-online-ideal-fpre-benches-cleanup
plan: 04
subsystem: documentation
tags: [rust, doc-comments, ggm-tree, cleanup, clean-10]

# Dependency graph
requires:
  - plan: 02-02
    provides: auth_tensor_gen/eval structs without gamma_auth_bit_shares; import redirect to crate::preprocessing; test call-site rename to generate_for_ideal_trusted_dealer — i.e. the code shape needed for the doc edits to land cleanly
provides:
  - src/auth_tensor_gen.rs with /// doc on garble_final and no `// awful return type` trailing comment
  - src/auth_tensor_eval.rs with /// doc on evaluate_final and a three-line explanatory comment on the GGM tree tweak domain-separation inside eval_populate_seeds_mem_optimized
  - CLEAN-10 fully discharged (combined with Plan 02's _gamma_share dead-code removal)
affects:
  - Any future reader of AuthTensorGen::garble_final / AuthTensorEval::evaluate_final — the mirror-pair semantics (garbler combines correlated share, evaluator combines correlated MAC) is now self-documenting
  - Any future implementer touching the eval-side GGM tree construction — the two bare `Block::from(0 as u128)` / `Block::from(1 as u128)` tweaks are now labelled as domain-separation values, matching the style used in src/tensor_ops.rs

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Doc-comment parallelism for mirror pairs: when a garbler-side function X and evaluator-side function Y perform the same structural step with asymmetric data (shares vs MACs), their `///` docs use parallel wording that differs only in the garbler↔evaluator terms; this makes the mirroring syntactically visible."
    - "Direction-neutral GGM tweak comment: when the paper and the code disagree on the 'left/right' labelling of child seeds (paper says 0=left, code writes tweak 0 to the odd-indexed sibling), the comment documents the code's actual mapping (tweak=0 for odd-indexed sibling, tweak=1 for even-indexed) rather than propagating the potentially inverted paper convention — avoids introducing a doc-vs-code contradiction."

key-files:
  created: []
  modified:
    - src/auth_tensor_gen.rs
    - src/auth_tensor_eval.rs

key-decisions:
  - "Dropped the `// awful return type` comment rather than refactoring to a named-tuple-struct return. Plan Task 1 explicitly sanctions this (D-13's 'Claude's discretion' clause): the refactor would require touching evaluate_first_half, evaluate_second_half in auth_tensor_eval.rs AND two bench call sites that destructure the tuple — scope expansion that the plan's minimise-churn directive rules out."
  - "Used direction-neutral GGM tweak wording (copied verbatim from RESEARCH Code Example 4). Per Pitfall 5 in 02-RESEARCH.md, D-15's stated 'tweak 0 = left child, tweak 1 = right child' may be inverted vs. the code: seeds[j*2+1] (odd index) receives tweak 0 and seeds[j*2] (even) receives tweak 1. The committed comment describes the code's actual mapping (tweak=0 for odd-indexed sibling, tweak=1 for even-indexed) — correct regardless of which side the paper calls 'left'."
  - "Applied both tasks via targeted Edit calls (not file rewrites). Each edit is 1-3 lines; the `Edit` tool is the minimum-churn mechanism for doc-only changes, and each change was verified immediately with grep before commit."
  - "Did NOT touch src/tensor_ops.rs. D-15 scopes the tweak comment to auth_tensor_eval.rs only; tensor_ops.rs (lines 50-80) already has an analogous 'Two seeds per parent: left child (even) and right child (odd)' comment that served as the style precedent but needs no additional annotation under this plan."

patterns-established:
  - "Mirror-pair doc wording: `/// Combines both half-outer-product outputs with the correlated preprocessing [share|MAC] to produce the [garbled tensor gate output|evaluator's share of the garbled tensor gate output].` — enforces garble ↔ evaluate parallelism at the doc layer."
  - "Direction-neutral domain-separation comment for GGM child-seed derivation: name the cryptographic purpose (domain separation) and identify both tweak values and their index parities, without committing to a left/right labelling that may disagree with the paper."

requirements-completed: [CLEAN-10]

# Metrics
duration: ~2min
completed: 2026-04-21
---

# Phase 02 Plan 04: CLEAN-10 Documentation Audit Summary

**Two documentation edits to auth_tensor_gen.rs (drop `// awful return type`, add `///` doc on garble_final) and two to auth_tensor_eval.rs (`///` doc on evaluate_final, explanatory comment on the GGM tree tweak domain-separation); zero code changes; baseline failure set preserved.**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-04-21T23:47:07Z
- **Completed:** 2026-04-21T23:49:29Z
- **Tasks:** 2 (both `type="auto"`, both pure documentation)
- **Files modified:** 2 (src/auth_tensor_gen.rs, src/auth_tensor_eval.rs)

## Accomplishments

- **D-13 complete.** The `// awful return type` trailing comment on `AuthTensorGen::gen_chunked_half_outer_product` is gone; the return type signature is unchanged (`(Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>)`), preserving call-site ABI in `benches/benchmarks.rs` and `auth_tensor_eval.rs` consumers.
- **D-14 complete.** Both `garble_final` (auth_tensor_gen.rs) and `evaluate_final` (auth_tensor_eval.rs) now carry two-line `///` doc comments describing their role as the combiner step of the authenticated tensor gate. The wording is parallel between the two sides, with garbler/evaluator asymmetry expressed as share↔MAC and `garbled tensor gate output`↔`evaluator's share of the garbled tensor gate output`.
- **D-15 complete.** A three-line `//` comment above the two `cipher.tccr(Block::from(0|1 as u128), seeds[j])` calls in `AuthTensorEval::eval_populate_seeds_mem_optimized` names them as GGM tree tweak domain-separation derivations for the two child seeds at each tree level. Wording is direction-neutral (per Pitfall 5): `tweak=0 for odd-indexed sibling, tweak=1 for even-indexed` — describes the code's actual mapping rather than the paper's potentially inverted labelling.
- **CLEAN-10 now fully satisfied** across Phase 02 (Plan 02's `_gamma_share` dead-code removal + this plan's three documentation edits).
- **Baseline regression gate green.** `cargo test --lib --no-fail-fast` produces exactly 4 FAILED lines — same four tests as `before.txt` (the only diff is `test_run_preprocessing_mac_invariants` still reported under its Plan-02-relocated path `preprocessing::tests`, not a new failure).
- **Scope discipline held.** `src/tensor_ops.rs` is untouched (`git diff --stat f231d20 -- src/tensor_ops.rs` is empty); `benches/benchmarks.rs` is untouched (Plan 03 owns bench edits); `src/preprocessing.rs`, `src/auth_tensor_fpre.rs`, `src/auth_tensor_pre.rs`, and `src/lib.rs` are untouched.

## Task Commits

Each task was committed atomically with `--no-verify` (parallel-executor convention):

1. **Task 1: Add /// doc to garble_final and remove awful return type comment in auth_tensor_gen.rs** — `63d7dd4` (docs)
2. **Task 2: Add /// doc to evaluate_final and GGM tweak explanatory comment in auth_tensor_eval.rs** — `b540c02` (docs)

## Files Created/Modified

- `src/auth_tensor_gen.rs` — two edits: (a) line 76 deleted the trailing ` // awful return type` from `gen_chunked_half_outer_product`'s return-type line; (b) two `///` lines inserted above the existing `pub fn garble_final(&mut self) {` (function body unchanged). Net diff: +3 insertions / -1 deletion.
- `src/auth_tensor_eval.rs` — two edits: (a) two `///` lines inserted above the existing `pub fn evaluate_final(&mut self) {`; (b) three `//` lines inserted above the two `seeds[j * 2 + 1] = cipher.tccr(...)` / `seeds[j * 2] = cipher.tccr(...)` calls inside `eval_populate_seeds_mem_optimized`'s `else` branch (the generator-side tweak usage; the evaluator-side reconstruction at `(tweak, mask) = ...` further down is unchanged because it already has context comments). Net diff: +6 insertions / -1 deletion.

## Decisions Made

- **Direction-neutral wording chosen for the GGM tweak comment.** The plan offered two acceptable forms, and only permitted directional wording ('left/right') if the implementer cross-checked the KRRW paper. I did not cross-check the paper, so per the plan's "When in doubt, use direction-neutral" directive, I used RESEARCH Code Example 4 verbatim. The specific phrasing `tweak=0 for odd-indexed sibling, tweak=1 for even-indexed` is also maximally accurate relative to the code (`seeds[j*2+1]` is the odd-indexed sibling and receives tweak 0; `seeds[j*2]` is the even-indexed sibling and receives tweak 1).
- **Dropped the awful-return-type comment rather than named-struct refactor.** Task 1 offered both paths; the named-tuple-struct refactor would cascade to `benches/benchmarks.rs` (out of Plan 04's file scope) and to `AuthTensorEval::evaluate_first_half`/`evaluate_second_half` consumers (which would need to destructure `(ciphertexts, labels)` differently). The plan's minimise-churn intent rules out the cascade, so I dropped the comment as the D-13 discretionary clause explicitly sanctions.
- **Left the evaluator-side reconstruction comment untouched.** `eval_populate_seeds_mem_optimized` also has a downstream tweak use at lines ~113-117 where `(tweak, mask) = if bit { (Block::from(0), g_evens ^ e_evens) } else { (Block::from(1), g_odds ^ e_odds) }`. This site already has a context comment (`// Reconstruct the sibling of the missing node using the ciphertext`) and is not what D-15 targets; the plan's D-15 directive is specifically the two generator-derivation calls in the `else` branch of the `j == missing` check.
- **Cross-referenced but did not edit the analog site in src/tensor_ops.rs.** Lines 50-80 of `tensor_ops.rs` already have a comment (`// Two seeds per parent: left child (even) and right child (odd)`) on the mirror derivation in `gen_populate_seeds_mem_optimized`. This served as the style precedent for my direction-neutral wording, but the plan scopes D-15 to `auth_tensor_eval.rs` only, so I left `tensor_ops.rs` alone. `git diff --stat src/tensor_ops.rs` confirms zero changes.

## Deviations from Plan

None — plan executed exactly as written. Both tasks applied cleanly via `Edit` with all grep/build acceptance criteria satisfied.

One plan-level verification note (see "Issues Encountered" below) deserves mention: plan step 3 says `grep -c "awful return type" src/` should return 0, but there is a **pre-existing** `// awful return type` comment in a separate file `src/tensor_gen.rs:57` (last touched in commit `1585a1d` long before Phase 02). Per Plan 04's `files_modified: [src/auth_tensor_gen.rs, src/auth_tensor_eval.rs]` frontmatter and the narrower Task 1 acceptance criterion (`grep -c "awful return type" src/auth_tensor_gen.rs` = 0, which passes), this file is out of scope. I logged it in `deferred-items.md` rather than fixing it, following the Scope Boundary rule.

## Issues Encountered

- **Pre-existing `awful return type` comment in out-of-scope file.** `src/tensor_gen.rs:57` also carries `) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) { // awful return type` — a mirror of the one this plan cleaned up in `auth_tensor_gen.rs`. `tensor_gen.rs` is a separate module (the non-authenticated tensor ops) whose file history predates Phase 02 entirely. Per the executor's Scope Boundary rule (only fix issues caused by your own changes; defer pre-existing issues in unrelated files), I logged this at `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/deferred-items.md` for a future audit to pick up rather than fixing it here. If the phase orchestrator wants it cleaned in this phase, a one-line follow-up plan can discharge it.
- **`cargo build --benches` fails.** Expected per Plan 02 SUMMARY: benches still reference `auth_tensor_fpre::run_preprocessing` and `generate_with_input_values`, which Plan 02 moved/renamed. Plan 03 (running in parallel with this plan) owns the bench fix. `cargo build --lib` and `cargo build --lib --tests` both exit 0 at the Plan 04 tip.

## User Setup Required

None.

## Verification Evidence

Plan-level verification checklist (from `<verification>` section):

| # | Check | Result |
|---|-------|--------|
| 1 | `cargo build --lib --tests --benches` exit 0 | ⚠ lib+tests green; benches fail by design per Plan 02 SUMMARY (Plan 03 owns bench import fix) |
| 2 | `cargo test --lib --no-fail-fast` FAILED set equals `before.txt` | ✓ 4 failures, same tests; only diff is Plan-02 relocation of `test_run_preprocessing_mac_invariants` to `preprocessing::tests` — not a new regression |
| 3 | `grep -c "awful return type" src/` returns 0 | ⚠ 0 in `auth_tensor_gen.rs` (plan target); 1 pre-existing match in out-of-scope `tensor_gen.rs:57` deferred to deferred-items.md |
| 4 | `grep -cE "/// Combines both half-outer-product outputs" src/auth_tensor_gen.rs src/auth_tensor_eval.rs` returns 2 | ✓ 1 per file |
| 5 | `grep -cE "GGM tree\|GGM tweak\|GGM domain" src/auth_tensor_eval.rs` returns ≥ 1 | ✓ 1 |
| 6 | `git diff --stat src/tensor_ops.rs` shows no changes | ✓ empty (unchanged) |
| 7 | `git diff --stat benches/benchmarks.rs` shows no changes from this plan | ✓ empty — Plan 04 did not touch benches |

Task-level acceptance criteria:

**Task 1 (auth_tensor_gen.rs):**
- `grep -c "awful return type" src/auth_tensor_gen.rs` = 0 ✓
- `grep -cE "/// Combines both half-outer-product outputs" src/auth_tensor_gen.rs` = 1 ✓
- `grep -B2 "^    pub fn garble_final" src/auth_tensor_gen.rs | grep -c "^    ///"` = 2 ✓
- `cargo build --lib` exit 0 ✓
- `auth_tensor_gen::tests::test_garble_first_half` passes ✓

**Task 2 (auth_tensor_eval.rs):**
- `grep -cE "/// Combines both half-outer-product outputs" src/auth_tensor_eval.rs` = 1 ✓
- `grep -B2 "^    pub fn evaluate_final" src/auth_tensor_eval.rs | grep -c "^    ///"` = 2 ✓
- `grep -cE "GGM tree\|GGM tweak\|GGM domain" src/auth_tensor_eval.rs` = 1 ✓
- GGM comment immediately above `seeds[j * 2 + 1]` line (line 102 comment → line 103 seeds call — within 1 line) ✓
- `cargo build --lib` exit 0 ✓
- `cargo test --lib` failure set matches baseline ✓
- `git diff src/tensor_ops.rs` zero changes ✓

## Parallel Execution Note

Plan 04 ran in parallel with Plan 03 (Wave 3). Plan 04's `files_modified` (`auth_tensor_gen.rs`, `auth_tensor_eval.rs`) is disjoint from Plan 03's (`benches/benchmarks.rs`), so there is no merge contention between the two worktrees. After both worktrees are merged back by the orchestrator, `cargo build --lib --tests --benches` should exit 0 for the first time since Plan 02 shipped (Plan 02's rename + module migration left benches broken by design, and Plan 03 fixes the bench import + rename).

## Next Plan Readiness

- **Plan 03 (bench dedup + import update)** runs in parallel and is unaffected by this plan's edits. After its merge: `cargo build --benches` goes green.
- **CLEAN-10 is fully discharged.** The only residual cosmetic item (`awful return type` in `src/tensor_gen.rs`) is tracked in `deferred-items.md` and is out of Phase 02 scope by design.
- **Phase 02 close:** Once Plan 03 merges too, the phase is functionally complete pending verifier pass.

## Self-Check: PASSED

Verified at SUMMARY write time:

- Both task commits present in `git log --oneline`:
  - `63d7dd4 docs(02-04): add /// doc to garble_final; remove awful return type comment` ✓
  - `b540c02 docs(02-04): add /// doc to evaluate_final; explain GGM tweak domain separation` ✓
- `src/auth_tensor_gen.rs` exists with expected content (doc comment present, awful comment absent, grep checks pass) ✓
- `src/auth_tensor_eval.rs` exists with expected content (doc comment present, GGM comment present, placement correct) ✓
- `src/tensor_ops.rs` unchanged (`git diff --stat` empty) ✓
- `cargo build --lib --tests` exits 0 ✓
- `cargo test --lib --no-fail-fast` failure count = 4, matching `before.txt` modulo Plan-02 test relocation ✓
- `.planning/phases/02-m1-online-ideal-fpre-benches-cleanup/deferred-items.md` created with the tensor_gen.rs out-of-scope pre-existing comment logged ✓

---
*Phase: 02-m1-online-ideal-fpre-benches-cleanup*
*Completed: 2026-04-21*
