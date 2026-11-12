# ADR 0008: No global mutable state

Status: Accepted (2026-11-12)

## Context

Many engine instances must coexist in one process: parallel tests, multi-match servers, editor and play-mode worlds, rollback worlds. Global mutable state makes instances observe each other, breaks determinism (ADR 0003), and makes a context-free C API tempting.

## Decision

There is no global mutable state anywhere in the engine. All state lives in instances the caller creates and passes explicitly.

- `static mut` is already rejected (it requires `unsafe`).
- `cargo xtask check-statics` parses every library crate with `syn` and fails on a `static` whose type contains interior mutability (`Cell`, `RefCell`, `UnsafeCell`, `Mutex`, `RwLock`, `Atomic*`, `OnceCell`, `OnceLock`, `LazyLock`, `LazyCell`) and on any `thread_local!`. `disallowed-macros` in `clippy.toml` also covers `thread_local!`.
- An exception needs a preceding `// engine:allow-global-state: <reason>` comment, and `check-statics` reports each one. M1 has none.
- The check is best-effort, not a proof. It matches type names syntactically, so `type` aliases, user structs wrapping a `Mutex`, and third-party lazy-static macros can evade it. Code review is the backstop.
- Dependencies can carry global state. Policy:
  - Diagnostics facades (logging, tracing, profiling) are the one sanctioned category of process-global state. Engine code only emits events, and only the host installs subscribers.
  - Thread pools are always explicit instances. When rayon or similar is added, its global-pool entry points go on the `disallowed-methods` list.
  - The global allocator belongs to the host application, never to an engine library.
- Immutable statics and consts (lookup tables, constants) are fine.

## Consequences

- M2's type registry and component IDs are per-world, never process-global.
- M3's thread pool is an instance the host creates.
- M4's C API passes an explicit context handle to every function.
