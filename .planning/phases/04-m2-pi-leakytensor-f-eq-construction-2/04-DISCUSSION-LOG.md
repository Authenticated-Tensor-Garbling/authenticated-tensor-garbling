# Phase 4: M2 Pi_LeakyTensor + F_eq (Construction 2) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 04-m2-pi-leakytensor-f-eq-construction-2
**Areas discussed:** generate() API, F_eq design, LeakyTriple cleanup, C_A/C_B helpers

---

## generate() API

| Option | Description | Selected |
|--------|-------------|----------|
| generate() → LeakyTriple | No arguments. x/y sampled internally. Fully input-independent. | ✓ |
| generate(n, m) → LeakyTriple | Pass dimensions at call time, allows varying sizes per instance. | |
| Keep dimensions on struct, rename method | Same behavior, just rename to generate_triple(). | |

**User's choice:** `generate() → LeakyTriple` — no args, fully random x and y using struct RNG.
**Notes:** Old `generate(x_clear, y_clear)` violated input-independence; removed entirely.

---

## F_eq Module Placement

| Option | Description | Selected |
|--------|-------------|----------|
| src/feq.rs module | New module matching IdealBCot pattern. Easy to swap for real impl. | ✓ |
| Inline in leaky_tensor_pre.rs | Private function, less indirection, harder to test in isolation. | |
| Inline, re-exported from lib.rs | Same as above but exposed for integration tests. | |

**User's choice:** `src/feq.rs` — separate module, matches IdealBCot pattern.

## F_eq Abort Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| panic!("F_eq abort: ...") | Unconditional abort, matches ideal functionality. Tests use #[should_panic]. | ✓ |
| Result<(), FeqError> | Propagate error, more ergonomic for tests but pretends abort is recoverable. | |
| assert_eq! (debug only) | Wrong semantics for a security check. | |

**User's choice:** `panic!` on L_1 ≠ L_2. Tests verify abort with `#[should_panic]`.

---

## LeakyTriple Field Naming

| Option | Description | Selected |
|--------|-------------|----------|
| Rename alpha→x, beta→y, correlated→z | Aligns with paper notation, auditable against Construction 2. | ✓ |
| Keep alpha/beta/correlated names | Less churn, but alpha/beta don't appear in paper Pi_LeakyTensor. | |
| Use paper subscripts: x_pa, x_pb, etc. | Most literal, verbose, maximally traceable. | |

**User's choice:** Rename throughout — `gen_x_shares`, `eval_x_shares`, `gen_y_shares`, `eval_y_shares`, `gen_z_shares`, `eval_z_shares`.

## Z Storage in LeakyTriple

| Option | Description | Selected |
|--------|-------------|----------|
| Vec<AuthBitShare> column-major | Matches x/y pattern, Phase 5 combining works directly. | ✓ |
| BlockMatrix shape, Vec<AuthBitShare> body | Cleaner API with row/col accessors, slight overhead. | |
| Two vecs: gen_z, eval_z as BlockMatrix | Separate BlockMatrix per party, requires conversion for combining. | |

**User's choice:** `Vec<AuthBitShare>` column-major (index = j*n+i).

---

## C_A/C_B Computation Style

| Option | Description | Selected |
|--------|-------------|----------|
| Inline in generate() | 3 lines of Block XOR per entry, too small to abstract. | ✓ |
| fn compute_c_a/compute_c_b helpers | Easier to unit-test in isolation, cleaner generate() body. | |
| Method on LeakyTensorPre struct | OOP style, consistent if other helpers become methods. | |

**User's choice:** Inline in `generate()`.

## R Generation Approach

| Option | Description | Selected |
|--------|-------------|----------|
| n×m bCOT calls each way (Claude's discretion) | Same pattern as x/y, no new interfaces. | ✓ |
| Batch helper on IdealBCot | Cleaner call site, expands IdealBCot API. | |

**User's choice:** n×m bCOT calls each way — Claude decides exact loop structure.

---

## Claude's Discretion

- Exact loop structure for bCOT call sequencing in generate()
- Method for wrapping C_A/C_B vecs into BlockMatrix for tensor macro input
- Nonce/ordering of bCOT calls (x, then y, then R)

## Deferred Ideas

None — discussion stayed within Phase 4 scope.
