# Deferred Items — Phase 02

Out-of-scope discoveries logged during plan execution, per Scope Boundary rule
(executor fixes only issues caused by its own changes; pre-existing issues in
unrelated files are deferred).

## Plan 04 discoveries

### 1. `// awful return type` comment in `src/tensor_gen.rs:57`

- **File:** `src/tensor_gen.rs` (separate from `src/auth_tensor_gen.rs`)
- **Line:** 57 — `) -> (Vec<Vec<(Block, Block)>>, Vec<Vec<Block>>) { // awful return type`
- **Status:** Pre-existing (last touched in commit `1585a1d "shared tensor ops"`, long before Phase 02)
- **Out of scope because:** Plan 04's `files_modified` frontmatter restricts edits to
  `src/auth_tensor_gen.rs` and `src/auth_tensor_eval.rs`. D-13 in `02-CONTEXT.md`
  explicitly targets only `auth_tensor_gen.rs::gen_chunked_half_outer_product`. The
  `tensor_gen.rs` file is in a different module (the non-authenticated tensor ops
  used by other code paths) and its return-type wart mirrors the one this plan
  cleaned up in `auth_tensor_gen.rs` but is architecturally separate.
- **Suggested follow-up:** If a future plan broadens the CLEAN-10 audit beyond the
  authenticated path, strip this trailing comment too (or rename the return type via
  a named-tuple struct for both sites together).

## Plan-level verification note

Plan 04's `<verification>` step 3 reads
`grep -c "awful return type" src/` expected 0. The stricter phrasing matches
the entire `src/` tree, but the plan's files_modified scope and Task 1
acceptance criterion (`grep -c "awful return type" src/auth_tensor_gen.rs` = 0)
make clear the authoritative target was `auth_tensor_gen.rs`. That narrower
check passes. The broader sweep finds one match in `tensor_gen.rs` — deferred
as noted above.
