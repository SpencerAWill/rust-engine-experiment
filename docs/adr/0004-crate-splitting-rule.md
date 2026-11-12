# ADR 0004: Crate-splitting rule

Status: Accepted (2026-11-12)

## Context

A crate is Rust's unit of compilation, coherence (trait impls), visibility, and versioning, so crate boundaries are expensive to move later. The project prefers many fine-grained crates, but "one crate per topic" produces empty crates and orphan-rule trouble. The rule below follows the practice described in matklad's "Large Rust Workspaces" and the Materialize writeup on compile times and code graphs.

## Decision

**Split into a separate crate when at least one of these holds:**

1. **Dependencies.** It needs a dependency others shouldn't carry, or must stay dependency-free.
2. **Permission boundary.** It needs an `unsafe` or concurrency exception, or must be provably free of them.
3. **FFI exposure.** It is exposed (or not) across the C ABI.
4. **Optionality.** Users should be able to omit it, and it has heavy dependencies or a distinct API. Small additive extensions are Cargo features instead.
5. **Platform.** `no_std` versus `std`, or platform-specific code.
6. **Stability.** A small, rarely-changing interface that others build against is separated from fast-moving implementation, so dependents don't churn.
7. **Enforced layering.** A lower layer must be provably unable to depend on a higher one.
8. **Measured compile time.** `cargo build --timings` shows the crate on the critical path and a split improves parallelism. This requires measurement, not intuition.
9. **Forced by the language.** Procedural macros live in their own crate.

**Keep code in one crate when any of these holds** (these override the split reasons unless an ADR says otherwise):

1. **Coherence.** Splitting would make needed trait impls impossible under the orphan rule, or force newtype workarounds. Foundational traits that many crates implement live at the bottom of the dependency graph.
2. **Shared internals.** The pieces need each other's private details, and a split would force `#[doc(hidden)] pub` back doors (which also contradict ADR 0009).
3. **Lockstep change.** They always change together.
4. **Too small to stand alone.** No boundary from the list above applies, and fixed per-crate costs would outweigh any benefit.

Topic alone is not a reason to split. No empty crates are created up front.

**Every crate states its justification** in the first paragraph of its crate docs and in this table:

| Crate | Justification |
|---|---|
| `engine_handle` | Platform (`no_std` + `alloc`), dependency-free; FFI exposure (handles cross the C ABI in M4); layering (foundation for the ECS, assets, FFI) |
| `engine_random` | Stability (output streams frozen for all of 0.x); platform (`no_std`, no `alloc`); independent reuse |
| `engine_time` | Platform (`no_std` core with a `std` feature for `SystemClock`); independent reuse; layering (no dependency on math or handles) |
| `engine_math` | Dependencies (`glam`); platform (`std`) |
| `engine` | Facade: the single dependency for users, re-exporting the above |

**Layout.** The workspace stays flat (`libs/*`, `engine_` prefix). When publishing is decided, the expected default is lockstep versioning of all `engine_*` crates, with its own ADR at that point.

## Consequences

- Splits are argued from the checklist, so reviewers can challenge a crate by naming the criterion it fails.
- Expect many crates over time, but never a crate that exists only because of its topic.
