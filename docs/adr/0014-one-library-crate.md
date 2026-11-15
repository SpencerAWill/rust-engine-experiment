# ADR 0014: One library crate, and the rule for splitting from it

Status: Draft (2026-11-14). Supersedes [ADR 0004](0004-crate-splitting-rule.md). Amends [ADR 0003](0003-same-build-determinism.md) for builds without `std`. Specified in [M3.13](../design/m3-ecs-storage/m3.13-one-engine-crate/spec.md).

## Context

ADR 0004 let a crate split off for any of nine reasons, and the workspace grew five leaf crates (`engine_genid`, `engine_collections`, `engine_random`, `engine_time`, `engine_math`) under an `engine` facade. While nothing is published, most of their recorded justifications protect nothing:

- **"Independent reuse"** (`engine_random`, `engine_time`) only matters once crates are published.
- **"Stability"** (`engine_random`): the golden tests freeze the PRNG streams; the crate boundary does not.
- **"FFI exposure"** (`engine_genid`): an M8 concern, and about the API rather than the crate.
- **"Layering"** between the leaves: siblings with no reason to import each other, so the boundary blocks a mistake nobody is likely to make.

The cost of each crate is real, and mostly this repository's own: the `xtask` lists for `no-std`, `wasm` and mutants, `.cargo/mutants.toml`, the mutants workflow's path filters, fuzz and bench dependencies, `clippy.toml` texts, and per-crate documentation. A clean build of all six crates takes 1.1 s, of which glam is 1.05 s (`cargo build --timings`, 2026-11-14), so compile time argues for no split.

The coming milestones make the split layout an obstacle. M5's reflection traits must be implemented for `glam` types, which the orphan rule allows only in the crate that defines the trait. They must also be implemented for IDs and collections, so the trait crate has to depend on all of them, and they cannot implement it themselves. M3.12 already needed a crate (`engine_heap`) for the same reason. Rust's own guidance points the same way: crates are the unit of compilation, coherence and permission, and community practice (matklad, the Rust Performance Book) is to split when there is a concrete, preferably measured, reason.

## Decision

**Default.** All library code lives in one crate, `engine`, organised by modules. `engine` is `#![no_std]` with `alloc` and a default `std` feature. A new library crate exists only when at least one split criterion holds and no keep-together rule forbids it.

**Split criteria.** Each must be shown with its evidence:

| ID | Criterion | Evidence required |
|---|---|---|
| S1 | **Required by Rust or Cargo:** procedural macros (crate type `proc-macro`) and C ABI artifacts (`cdylib` or `staticlib`) | The crate type |
| S2 | **Permission boundary:** the code needs an `unsafe` exception ([ADR 0001](0001-unsafe-policy.md)) or a concurrency exception ([ADR 0008](0008-no-global-state.md)); both allowlists grant exceptions per crate | The allowlist entry and its ADR. The crate contains only the code that needs the exception. |
| S3 | **Configurations that cannot be features:** two variants cannot coexist as additive Cargo features | Why an additive feature or a runtime option does not work |
| S4 | **Measured compile time** | A `cargo build --timings` report showing `engine` on the critical path, and before-and-after times for the clean build and for the edit-and-rebuild loop, over repeated runs, with the noise floor. The maintainer decides; there is no fixed threshold. |

**Not reasons to split:**

| Reason | Instead |
|---|---|
| Topic, size or code organisation | Modules |
| Layering between modules | Review, and [ADR 0009](0009-tooling-uses-public-apis.md)'s public-API discipline |
| Stability, or a frozen format | Golden tests |
| An optional dependency | A Cargo feature of `engine` |
| Reuse by other projects | Nothing until publishing is planned; its ADR revisits this, including the API Guidelines' C-STABLE rule |

**Keep-together rules.** These override any split criterion:

- **K1, coherence.** The split would make a needed trait impl impossible under the orphan rule, or would force newtype wrappers.
- **K2, shared internals.** The split would need `#[doc(hidden)] pub` back doors.
- **K3, no cycles.** A crate below `engine` cannot use `engine`'s types, and `engine` cannot use a crate above it. If the code needs `engine`'s types and `engine` needs the code, it does not split.

**Process.**

1. The spec of the chunk that needs a crate proposes it: the criterion ID, whether it sits below or above `engine`, and the evidence.
2. That chunk's pull request adds the crate's row to the table below. The crate's docs state the criterion in their first paragraph.
3. When a crate's criterion stops holding, it merges back into `engine` in the chunk that removed the reason.
4. Non-library packages (`xtask`, `bench/*`, `examples/*`, `spikes/*`, `fuzz`) are outside this rule.
5. New crates keep the flat layout: `libs/engine_<name>`, with the directory named after the crate.

| Crate | Criterion |
|---|---|
| `engine` | The library (the default under this ADR) |

**Amendment to ADR 0003.** ADR 0003 says the transcendental backend is never chosen by a Cargo feature. With `math` in a `no_std` crate, it is chosen by `std` in one narrow case:

- A build with `std` uses `std`'s transcendental functions, in `engine::math::scalar` and inside glam, as before.
- A build without `std` uses `libm` (through glam's `nostd-libm` feature and `scalar`'s `no_std` backend), because no other backend exists there.
- Within one build the backend is fixed, so same-build determinism holds. A `std` build and a `no_std` build may differ in the last bits, which ADR 0003 does not promise against.
- The cross-platform switch ADR 0003 describes (`libm` everywhere, plus glam's `scalar-math`) is still a future, deliberate, engine-wide decision.

## Consequences

- `engine::` paths stay as they were; `engine_<name>::` paths are gone. The repository's tooling names one crate.
- The `no_std` proof covers all library code, math included.
- M4's ECS, M5's reflection and M7's scheduler are modules of `engine` by default. Reflection traits sit next to the types they are implemented for, with no orphan-rule workarounds.
- If M3.8 decides that component storage needs `unsafe`, S2 moves only that code into a crate below `engine`. K3 means it cannot use `engine`'s types, so its interface is plain data (for example, byte columns indexed by `u32`).
- **What this gives up.** The compiler no longer stops one module from using another, and review takes its place. In incremental (dev) builds rustc does not infer cross-crate inlining, so a later split of hot code needs `#[inline]` on its small functions.
- Splits are argued from the criteria, so a reviewer can reject a crate by naming the criterion it fails, and a crate whose reason has gone is merged back.
