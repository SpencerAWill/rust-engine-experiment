# ADR 0003: Same-build determinism

Status: Accepted (2026-11-12)

## Context

Replays, save/load, rollback netcode, and "record a bug, replay it" all depend on the simulation being reproducible. Full cross-platform bit-exactness is much more expensive: it constrains every transcendental function and every SIMD reduction. Rapier's networked desyncs were traced to differing `sin_cos` results across platforms (see the glam discussion on libm, cited in the spec).

## Decision

**Same-build determinism:** the same binary on the same platform with the same inputs produces identical results. Cross-platform bit-exactness is not promised. Server-authoritative and same-platform rollback models are unaffected.

**Principle.** Simulation code receives time, randomness, and input only as explicit parameters (a `TimeSource`, a `Prng`, input data). It never reads ambient or global state.

**Enforcement** is mechanical, through the workspace `clippy.toml`:
- `disallowed-types`: `std::collections::HashMap`, `HashSet`, and `std::hash::RandomState` (their default hasher is seeded from OS randomness, so iteration order differs between runs).
- `disallowed-methods`: `Instant::now`, `SystemTime::now`, and the inherent transcendental float methods on `f32` and `f64`.
- `disallowed-macros`: `thread_local!`.
- Sanctioned exceptions use `#[expect(clippy::disallowed_methods, reason = "...")]` on the smallest possible item. M1 has exactly two: `SystemClock` and `engine_math::scalar`. Non-simulation crates (`xtask`, `engine_benches`) opt out at the crate root.

**Transcendental math funnel.** All transcendental math goes through `engine_math::scalar`, which delegates to `std` in M1. The upgrade path to cross-platform determinism is a future ADR with three changes together: back `scalar` with the pure-Rust `libm` crate, enable glam's `libm` feature, and enable glam's `scalar-math` feature. That switch changes output values and the size and alignment of `Vec3A`, `Vec4`, `Quat`, `Mat3A`, `Mat4`, so it is a deliberate, versioned, engine-wide decision and never a Cargo feature. Static layout assertions in `engine_math` make an accidental change loud.

**Float notes.** Rust does not contract `a * b + c` into fused multiply-add, and we never enable fast-math. Basic IEEE operations are bit-reproducible. glam's SIMD backends sum horizontally in different orders (SSE2 and NEON differ), so `dot`, `length`, `normalize`, and quaternion/matrix products can differ in the last bit between x86_64 and aarch64. That is consistent with same-build determinism, but it limits what can share a cross-architecture golden value.

**Independence from thread count and timing.** Rules for M2/M3 and beyond:
- parallel work uses fixed partitioning (chunk boundaries depend on data size, never on thread count);
- reductions combine partial results in a fixed tree or order;
- deferred commands from parallel systems merge in a deterministic order, never in completion order;
- no floating-point accumulation through atomics.

**Handle allocation order is simulation input.** PRNG streams are commonly keyed by `handle.to_bits()`, so which handle an entity receives affects output. Handles are allocated only at deterministic points (single-threaded code, or deferred commands applied in schedule order). The `to_bits` layout is part of the deterministic output, and changing it is a breaking change requiring an ADR.

**Snapshot-able state.** All simulation state is plain data, fully capturable and restorable (`Clone` at minimum, raw-state accessors for value-like state). This is what enables replays, save/load, and rollback.

## Consequences

- Nondeterminism hazards are caught at lint time rather than found as desyncs later.
- The cost of a later cross-platform switch is contained to one module plus a feature decision.
- Downstream crates can enable glam's `fast-math` or `scalar-math` through Cargo feature unification. `scalar-math` is caught by the layout assertions; `fast-math` cannot be detected, so the README documents it as unsupported.
