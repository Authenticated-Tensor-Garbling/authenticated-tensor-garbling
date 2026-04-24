# Phase 8: Open() + Protocol 1 Garble/Eval/Check - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-23
**Phase:** 08-open-protocol-1-garble-eval-check
**Areas discussed:** Module layout, open() scope, CheckZero return type, L_gamma computation

---

## Module layout

| Option | Description | Selected |
|--------|-------------|----------|
| online.rs: thin layer only | open() + check_zero() in online.rs; Protocol 1 garble/eval stays in gen/eval structs | ✓ |
| online.rs: full orchestration | protocol1_garble() / protocol1_eval() free functions in online.rs | |
| Structs only, no online.rs yet | All new logic in auth_tensor_gen/eval; online.rs deferred | |

**User's choice:** online.rs created now for check_zero() only. Protocol 1 garble/eval stays as struct methods.

**Follow-up:** With open() deferred, online.rs hosts check_zero() only for now. User confirmed this is the right split.

---

## open() scope

| Option | Description | Selected |
|--------|-------------|----------|
| Implement open() in this phase | fn open(auth_bit, delta) -> bool; ONL-01/02 active | |
| Defer open() entirely | ONL-01 and ONL-02 out of scope for Phase 8 | ✓ |

**User's choice:** open() deferred. User noted it is an interactive protocol and the message-passing design was not settled. Will be addressed in a later phase.

**Notes:** Discussion explored simulation shortcut (AuthBit + delta) vs explicit send/receive; user concluded open() should be deferred rather than designed under time pressure.

---

## CheckZero return type

| Option | Description | Selected |
|--------|-------------|----------|
| -> bool | true = pass, false = abort. Consistent with existing verify() style | ✓ |
| -> Result<(), ()> | Idiomatic Rust fallible; no codebase precedent in protocol layer | |
| panic on failure | Simple; makes negative test use #[should_panic] | |

**User's choice:** `-> bool`

---

## L_gamma computation

| Option | Description | Selected |
|--------|-------------|----------|
| New methods on gen/eval structs | compute_lambda_gamma() on AuthTensorGen and AuthTensorEval; no signature changes to existing methods | ✓ |
| Extend garble_final / evaluate_final in-place | Return Vec<bool> from garble_final; changes existing signatures and callers | |
| Free functions in online.rs | compute_lambda_gamma_gb/ev in online.rs; gen/eval files untouched | |

**User's choice:** New methods on structs. Evaluator's method takes `[L_gamma]^gb: &[bool]` as a parameter.

---

## CheckZero signature

| Option | Description | Selected |
|--------|-------------|----------|
| Callers pre-compute c_gamma, pass &[AuthBitShare] | check_zero(c_gamma_shares, delta_ev) -> bool. Thin primitive. | ✓ |
| Takes gen + eval structs directly | Computes c_gamma internally. Tighter coupling to struct types. | |

**User's choice:** Callers pre-compute `c_gamma` and pass the D_ev-share vec. `check_zero()` stays a thin primitive.

---

## Deferred Ideas

- `open()` (ONL-01, ONL-02) — message-passing design deferred to a future phase; will live in `src/online.rs`
