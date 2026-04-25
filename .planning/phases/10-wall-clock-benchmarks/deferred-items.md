# Deferred Items — Phase 10 (wall-clock-benchmarks)

Out-of-scope discoveries logged during plan execution per executor SCOPE BOUNDARY rule.

## Network bench harness panics at n=64 in setup_auth_gen

**Discovered during:** Plan 10-04 smoke-bench verification (post-fix runs of `online/p1_garble_eval_check_4x4/1` and `online/p2_garble_eval_check_4x4/1`).

**Symptom:** After the targeted `online/...` bench completes, criterion proceeds to register/print for `bench_online_with_networking_for_size`. At the function-entry block (benches/benchmarks.rs:397-403), `setup_auth_gen(n=64, m=64, chunking_factor)` calls `TensorFpre::generate_for_ideal_trusted_dealer`, which asserts `self.n <= usize::BITS as usize - 1` (src/auth_tensor_fpre.rs:92-96). On 64-bit systems this fires for `n=64`, panicking with `n=64 exceeds usize bit width minus 1; x must be representable as usize`.

**Why deferred:**
- Pre-existing behavior — not caused by Plan 10-04. Before this plan, the gamma_d_ev_shares assert panicked first (in P1/P2) and shadowed the n=64 path.
- Plan 10-04 explicitly says "Do NOT modify bench_online_with_networking_for_size".
- The targeted online benches (P1/P2 4x4/cf=1) DO complete and print Criterion's `time:` / `thrpt:` lines successfully before this downstream panic.
- The fix likely belongs in `bench_online_with_networking_for_size` (skip n>=64 setup in the unconditional pre-loop block; or use IdealPreprocessingBackend like setup_auth_pair, which has no such assert).

**Suggested follow-up plan scope:** Either (a) widen `generate_for_ideal_trusted_dealer`'s n/m assertion (replace usize-fits-in-bits with a more permissive precondition tied to actual indexing requirements), or (b) refactor `bench_online_with_networking_for_size` to defer the byte-count print/garble for n>=64 (or use setup_auth_pair).
