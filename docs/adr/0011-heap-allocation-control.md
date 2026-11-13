# ADR 0011: Controlling heap allocation

Status: Accepted (2026-11-12)

## Context

`engine_handle` is the only crate that allocates on purpose: `HandlePool` and the allocators keep
`Vec`s (slots, and for the pool and `DenseHandleAllocator` the dense handles and values). Callers
of a real-time engine sometimes need to know exactly when the heap is touched, for example a frame
budget with no allocation, an embedded target, or a benchmark that must not measure `malloc`. The
question was whether callers should be able to supply their own allocator.

## Decision

- **Explicit capacity controls.** `with_capacity`, `reserve` and `try_reserve` (all three types)
  let a caller allocate up front. `with_fixed_capacity(n)` allocates every array once and never
  grows: once `n` slots exist, `try_insert` returns `CapacityError` and `try_allocate` returns
  `None`. After construction, insert/allocate, remove/free, `clear`, `retain` and `drain` never
  touch the heap. A dhat integration test counts heap blocks across heavy churn and asserts zero.
- **`reserve` is an explicit request to allocate**, so it can grow a fixed-capacity container's
  memory; the capacity limit keeps applying.
- **`allocate_many` allocates its result `Vec` by design.** A caller who must not allocate loops
  over `allocate`.
- **The global allocator is the process-wide hook.** Everything here goes through it, so a game or
  embedded target can install a bump, arena or counting allocator with `#[global_allocator]` and
  no change to the engine. That is how dhat is used in the tests.
- **No allocator type parameter yet** (`HandlePool<T, A = Global>`). Stable Rust has no allocator
  parameter on `Vec` (`allocator_api` is unstable), so it would need the `allocator-api2` shim crate:
  a dependency in a crate that has none, and an `A` bound threaded through every generic function that
  touches a pool.

## Consequences

- Exact heap behaviour is available today without a new type parameter, and it is tested.
- Retrofitting `A = Global` later is mostly source-compatible, so deferring costs little.
- Revisit when a concrete need appears (a per-world arena, for example) or when `allocator_api`
  stabilises.
- `engine_collections` also allocates and follows the same capacity controls (`with_capacity`, `reserve`, `try_reserve`: reserved room is used without further allocation), without a fixed-capacity mode. Other crates do not allocate, so they need none of this.
