//! Real-protocol preprocessing pipeline.
//!
//! This module holds the output structs (`TensorFpreGen`, `TensorFpreEval`) that the
//! real two-party preprocessing protocol produces, together with the `run_preprocessing`
//! entry point. The ideal trusted-dealer functionality stays in `auth_tensor_fpre`.
//!
//! Populated in Phase 2 Plan 02 — this Wave 0 skeleton only reserves the module name
//! so downstream plans can `use crate::preprocessing::...` without module-resolution errors.
