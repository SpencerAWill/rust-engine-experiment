# ADR 0004: Crate-splitting rule

Status: Accepted (2026-11-12). Rewritten in place by M3.13 (2026-11-14); see its [spec §4.8](../design/m3-ecs-storage/m3.13-one-engine-crate/spec.md).

## Context

A crate is Rust's unit of compilation, coherence (which trait impls are allowed where), visibility, and permission (`#![forbid(unsafe_code)]` and this repository's allowlists apply per crate). Crate boundaries are therefore expensive to move later, and each one has a fixed cost: a manifest, its place in the tooling (`no-std`, `wasm`, mutants, fuzz, benches, coverage), its documentation, and the orphan rule between it and every other crate.

The official documentation gives no rule for when to split. It gives hard constraints: procedural macros need their own crate type, a package holds at most one library crate, a trait impl is allowed only in the crate that defines the trait or one of the types, and Cargo features must be additive because features are unified across the dependency graph (the Cargo Book names separate packages as the alternative to mutually exclusive features). Community guidance on large workspaces (matklad's "Large Rust Workspaces" and "Fast Rust Builds", the Rust Performance Book) splits for compile time only where measurement shows a crate serialising the build, and notes that generic code at a crate boundary is compiled again in each crate that uses it.

The engine's coming layers make coherence the deciding constraint. Reflection (M5) must be implemented for `glam` types, which the orphan rule allows only in the crate that defines the reflection traits, and for IDs, collections and transforms. Traits and the types they are implemented for belong together. Nothing is published, so reuse and version stability are not yet reasons for a boundary.

## Decision

**Default.** All library code lives in one crate, `engine`, organised by modules (`engine::genid`, `engine::collections`, `engine::random`, `engine::time`, `engine::math`, and later the ECS, reflection and the scheduler). `engine` is `#![no_std]` with `alloc` and a default `std` feature. A new library crate exists only when at least one split criterion holds and no keep-together rule forbids it.

**Split criteria.** Each must be shown with its evidence:

| ID | Criterion | Evidence required |
|---|---|---|
| S1 | **Required by Rust or Cargo:** procedural macros (crate type `proc-macro`) and C ABI artifacts (`cdylib` or `staticlib`) | The crate type |
| S2 | **Permission boundary:** the code needs an `unsafe` exception ([ADR 0001](0001-unsafe-policy.md)) or a concurrency exception ([ADR 0008](0008-no-global-state.md)); both allowlists grant exceptions per crate | The allowlist entry and its ADR. The crate contains only the code that needs the exception. |
| S3 | **Configurations that cannot be features:** two variants cannot coexist as additive Cargo features | Why an additive feature or a runtime option does not work |
| S4 | **Measured compile time** | A `cargo build --timings` report showing `engine` on the critical path, and before-and-after times for the clean build and for the edit-and-rebuild loop, over repeated runs, with the noise floor, recorded in the spec. The maintainer decides; there is no fixed threshold. |

**Not reasons to split:**

| Reason | Instead |
|---|---|
| Topic, size or code organisation | Modules |
| Layering between modules | Review, and [ADR 0009](0009-tooling-uses-public-apis.md)'s public-API discipline |
| Stability, or a frozen format | Golden tests (they freeze the output; a crate boundary does not) |
| An optional dependency | A Cargo feature of `engine` |
| Reuse by other projects | Nothing until publishing is planned; its ADR revisits this, including the API Guidelines' C-STABLE rule (a 1.0 crate needs stable public dependencies) |

**Keep-together rules.** These override any split criterion:

- **K1, coherence.** The split would make a needed trait impl impossible under the orphan rule, or would force newtype wrappers.
- **K2, shared internals.** The split would need `#[doc(hidden)] pub` back doors.
- **K3, no cycles.** A crate below `engine` cannot use `engine`'s types, and `engine` cannot use a crate above it. If the code needs `engine`'s types and `engine` needs the code, it does not split.

**Process.**

1. The spec of the chunk that needs a crate proposes it: the criterion ID, whether it sits below or above `engine`, and the evidence.
2. That chunk's pull request adds the crate's row to the table below. The crate's docs state the criterion in their first paragraph.
3. When a crate's criterion stops holding, it merges back into `engine` in the chunk that removed the reason.
4. Non-library packages (`xtask`, `bench/*`, `examples/*`, `spikes/*`, `fuzz`) are outside this rule.
5. New crates are flat under `libs/`, named `engine_<name>`, with the directory named after the crate.

| Crate | Criterion |
|---|---|
| `engine` | The library (the default under this ADR) |

## Consequences

- Users depend on one crate and reach everything through `engine::` paths and `engine::prelude`.
- The `no_std` proof covers all library code: `cargo xtask no-std` builds `engine` without default features.
- The ECS (M4), reflection (M5) and the scheduler (M7) are modules of `engine` by default. Reflection traits sit next to the types they are implemented for, with no orphan-rule workarounds.
- If component storage needs `unsafe`, S2 moves only that code into a crate below `engine`. K3 means it cannot use `engine`'s types, so its interface is plain data (for example, byte columns indexed by `u32`). M8's `engine_ffi` sits above `engine` under S1 and S2.
- **What this gives up.** The compiler does not stop one module from using another, so review does. In incremental (dev) builds rustc does not infer cross-crate inlining of small functions, so a split of hot code needs `#[inline]` on them.
- Splits are argued from the criteria, so a reviewer can reject a crate by naming the criterion it fails, and a crate whose reason has gone is merged back.
