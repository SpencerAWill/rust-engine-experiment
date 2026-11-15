# M3.13: One `engine` library crate, and the rule for splitting from it

- **Status:** Draft (2026-11-14)
- **Milestone:** M3 (scheduled during M3, not part of its exit criteria; it must land before M4, which decides where the ECS lives) · **Depends on:** none · **Decides:** [ADR 0014](../../../adr/0014-one-library-crate.md), which supersedes [ADR 0004](../../../adr/0004-crate-splitting-rule.md) and amends [ADR 0003](../../../adr/0003-same-build-determinism.md) for `no_std` builds (§4.7)
- **Issues:** #40. Related: M3.12 (space metrics, #41, drafting on its own branch), which will depend on this chunk and put `HeapSize` in `engine` instead of a new `engine_heap` crate (§4.10).
- **Estimated size:** about 650 changed lines, excluding tests, lockfiles and generated files: manifests and `xtask` about 200, the `scalar` `no_std` backend about 60, crate and module docs about 120, ADR 0014 about 120, other documents about 150. On top of that comes a move of about 12,800 lines (9,600 in the five crates' `src/`, 3,200 in their `tests/` and `benches/`) into `libs/engine`, kept in its own commit so `git diff -M` shows it as renames (§4.9), and a mechanical rewrite of about 270 `engine_<name>::` paths in doctests, tests, benches, fuzz targets and `xtask`.

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk merges the five library crates (`engine_genid`, `engine_collections`, `engine_random`, `engine_time`, `engine_math`) into `engine`, as the modules that the facade already exposes (`engine::genid` and so on). It also replaces ADR 0004's splitting rule with ADR 0014. Under the new rule, all library code lives in `engine`, and a separate crate is created only when Rust or Cargo requires one, when a permission boundary needs one, when two configurations cannot be additive features, or when measured compile time justifies one.

It comes now because M4 decides where the ECS lives, M5 needs reflection traits implemented for `glam`, ID and collection types without orphan-rule workarounds, and M3.12 is about to add a crate whose only reason is the same cross-crate coherence problem. Most of the five crates' ADR 0004 justifications ("independent reuse", "stability", "FFI exposure", sibling layering) protect nothing while nothing is published.

At the end, `libs/` holds one crate. `use engine::prelude::*;` and every `engine::` path work as before. `cargo xtask no-std` builds the whole of `engine`, math included, for a bare-metal target, and `cargo xtask ci` and `cargo xtask bench-regress` pass.

## 2. Goals and non-goals

**Goals**

- One library crate, `engine`, `#![no_std]` with `alloc` and a default `std` feature, containing everything the five crates contain today.
- Unchanged public paths under `engine::`. Only the `engine_<name>` crate paths disappear.
- A splitting rule that can be checked (ADR 0014, §4.6): each split criterion needs evidence, each rejected reason names what to use instead, and the process says where a proposal is made and when a crate merges back.
- A stronger `no_std` proof: math becomes `no_std`-capable, so `cargo xtask no-std` builds every library module, not only four of them.
- No behaviour change: the same PRNG streams, the same IDs, the same iteration orders, the same transform layouts, and the same `fixed_step_sim` checksum.
- Tooling that names one crate: the `xtask` lists, the mutants configuration and workflow, fuzz, benches and `clippy.toml`.

**Non-goals**

- No new code or API. Moving `HeapSize`, or anything else, into `engine` is the job of the chunk that introduces it (§4.10).
- No consolidation of integration tests into fewer binaries. Each test file moves as it is, with a prefix (§4.4). Fewer test binaries would link faster (matklad, §11), but that is a separate change.
- No layering checks between modules. ADR 0014 rejects layering as a reason to split. If a module boundary ever needs enforcing, that is a separate tooling decision.
- No cross-platform determinism. The `std` build keeps using `std`'s transcendental functions. The future ADR that ADR 0003 describes (`libm` everywhere plus glam's `scalar-math`) is still future (§4.7).
- No change to the benchmark subjects' names. Criterion IDs such as `engine_genid` stay as labels, so saved baselines and `benchmarks.md` still compare (§4.8).
- No publishing decision. ADR 0004's expectation of lockstep versioning is moot with one crate. Publishing gets its own ADR when it is planned.

## 3. Guide-level explanation

A user's code does not change:

```rust
use engine::prelude::*;
use engine::collections::IndexMap;

let mut map = GenIdMap::new();
let id = map.insert(Transform::IDENTITY);
let mut speeds = IndexMap::new();
speeds.insert("walk", 1.4);
let mut prng = Prng::from_seed(42);
assert!(map.contains(id) && prng.range(0..10) < 10);
```

Code that named a leaf crate directly (benches, fuzz targets, `xtask`, doctests) uses the module path instead: `engine_genid::GenIdMap` becomes `engine::genid::GenIdMap`, and `engine_math::scalar::sin` becomes `engine::math::scalar::sin`.

A contributor adding code puts it in a module of `engine`. New code goes in a new module when it is a new topic (the ECS in M4, reflection in M5), not in a new crate. A new crate is proposed in the spec of the chunk that needs it, by naming one of ADR 0014's criteria and giving its evidence:

- **S1, required by Rust or Cargo.** "M5's derive macros need a `proc-macro` crate." The evidence is the crate type.
- **S2, permission boundary.** "M3.7's type-erased columns need `unsafe`, and the chunk's own ADR allowlists `engine_columns`." The crate holds only the code that needs the exception. It sits below `engine`, so it cannot use `engine`'s types (K3).
- **S3, configurations that cannot be features.** For example, two backends that cannot be enabled together. The evidence is why an additive feature or a runtime option does not work.
- **S4, measured compile time.** The evidence is a `cargo build --timings` report showing `engine` on the critical path, plus before-and-after build times over repeated runs with the noise floor. The maintainer decides; there is no fixed threshold.

Building for a bare-metal target:

```sh
cargo build -p engine --no-default-features --target thumbv7em-none-eabihf
```

Without `std`, `SystemClock` is unavailable and transcendental functions (in `engine::math::scalar` and inside glam) come from `libm` instead of `std`. Within one build the backend is fixed, so ADR 0003's same-build determinism holds (§4.7).

## 4. Reference-level design

### 4.1 Crate shape and layout

- `libs/` MUST contain exactly one crate, `engine`. `libs/engine_genid`, `libs/engine_collections`, `libs/engine_random`, `libs/engine_time` and `libs/engine_math` MUST NOT exist, and the root `Cargo.toml` MUST NOT list them in `[workspace.dependencies]`.
- `engine`'s crate root MUST carry `#![no_std]` and `#![forbid(unsafe_code)]`, and MUST declare `extern crate alloc;` and, under `#[cfg(feature = "std")]`, `extern crate std;`.
- The modules MUST be `genid`, `collections`, `random`, `time` and `math`, all `pub`, at the paths the facade re-exports today (`libs/engine/src/lib.rs:62-66`).
- Each former crate's `src/lib.rs` MUST become `libs/engine/src/<module>.rs`, and its other source files MUST move under `libs/engine/src/<module>/`, keeping their relative paths. This follows the existing `foo.rs` plus `foo/` style (as in `engine_collections`'s `index_map.rs` and `index_map/`), not `mod.rs`.
- Former `crate::` paths inside a module MUST become `crate::<module>::` or `super::` paths. Inside `engine`, one module refers to another by `crate::<other>::`; nothing goes through the prelude.
- Visibility MUST stay as it is: what was `pub` in a crate's root is `pub` in its module, and what was `pub(crate)` stays `pub(crate)`. A `pub(crate)` item now becomes visible to the other modules too, which is accepted (§6, Public API changes).
- The `engine` prelude MUST be unchanged.

### 4.2 Features, `std` and `no_std`

`engine`'s `[features]` MUST be:

| Feature | Default | Enables |
|---|---|---|
| `std` | yes | `extern crate std`; `time::SystemClock` (as today, except on `wasm32-unknown-unknown`); `std` as the transcendental backend of `math::scalar`; `glam/std` |
| `test-invariants` | no | `check_invariants` on `GenIdMap` and `GenIdAllocator`, for integration tests and fuzz targets (the former `engine_genid` feature) |

- The self dev-dependency that `engine_genid` used to turn on `test-invariants` for its integration tests MUST move to `engine` (`engine = { path = ".", features = ["test-invariants"] }`).
- `glam` MUST be a dependency with `default-features = false` and the features `f64`, `i32`, `u32` and `nostd-libm`. `nostd-libm` makes glam use `libm` only when glam's `std` feature is off (glam 0.34.3, `src/f32/math.rs:35`). glam's `scalar-math` feature MUST NOT be enabled; the existing layout assertions guard it.
- `libm` MUST be a direct, non-optional dependency, used only by `math::scalar` when `std` is off. glam's `nostd-libm` already brings it into the graph, so this adds no new crate to `Cargo.lock` that a `no_std` build does not already need. Its licence (MIT or Apache-2.0) is in ADR 0002's allow list.
- Every function in `math::scalar` MUST be available with and without `std`. With `std` it MUST delegate to the inherent method, as today. Without `std` it MUST delegate to the `libm` function of the same name (`sinf`/`sin` for `f32`/`f64`, and so on). `ln` maps to `libm::log`, `ln_1p` to `log1p`, `exp_m1` to `expm1`, and `log(x, base)` MUST be `ln(x) / ln(base)`. `powi`, which `libm` lacks, MUST be `powf` with the exponent converted to the float type. Each mapping MUST be documented on the function.
- `clippy.toml`'s `disallowed-methods` MUST additionally list the `libm` functions that `scalar` wraps, with the reason "use `engine::math::scalar`", so that `scalar` stays the only caller. (`libm` functions are free functions, so the existing ban on inherent methods does not cover them.)
- `cargo xtask no-std` MUST build `engine` with `--no-default-features` for `thumbv7em-none-eabihf`. A throwaway probe (glam 0.34.3 with `nostd-libm` on that target) kept `Quat` at size and alignment 16, so `math`'s layout assertions are expected to hold there. The build proves it.
- `cargo xtask wasm` MUST build `engine` for `wasm32-unknown-unknown`, and run `engine`'s library and integration tests on `wasm32-wasip1` under wasmtime.

### 4.3 Crate and module documentation

- `engine`'s crate docs MUST keep the crate map, but with module rows only (no "Crate" column), and MUST say in their first paragraph that `engine` is the engine's one library crate under ADR 0014. The sentence "Only this facade has a prelude" MUST be reworded for one crate.
- Each former crate's crate docs MUST become its module's docs. The first paragraph's ADR 0004 justification ("`engine_genid` is a separate crate because ...") MUST be removed. Everything else (guarantees, choosing a container, the `GenId` glossary, examples) MUST stay.
- Doctests MUST use `engine::<module>::` paths, and MUST NOT go through the prelude where the original did not.
- `#![no_std]` statements in module docs ("this crate is `no_std`") MUST become statements about what the module needs without `std`, if anything (for example, `time`: "`SystemClock` needs the `std` feature").

### 4.4 Tests, benches, fuzz and examples

- Each former crate's `tests/<name>.rs` MUST move to `libs/engine/tests/<module>_<name>.rs`, for example `tests/genid_model.rs`, `tests/collections_allocations.rs`. The facade's own `tests/auto_traits.rs` MUST become `tests/prelude_auto_traits.rs`. `transform_props.proptest-regressions` MUST be renamed with its test file (`math_transform_props.proptest-regressions`), since proptest finds it by the source file's name.
- Test bodies MUST change only in their `use` paths. No test is weakened or deleted.
- Files that install a global allocator (the `dhat` tests) MUST stay separate test binaries.
- `engine_time`'s `benches/fixed_timestep.rs` MUST move to `libs/engine/benches/time_fixed_timestep.rs` with its `[[bench]]` entry.
- `bench/compare`, `bench/regress`, `fuzz`, `xtask` and every `examples/*` package MUST depend on `engine` alone among the library crates, and use `engine::<module>::` paths. `fuzz` MUST depend on `engine` with `test-invariants`.

### 4.5 Tooling

- **`xtask no-std` and `xtask wasm`:** as in §4.2.
- **`xtask mutants`:** the unit of selection becomes a module of `engine`, not a package. `TRIAGED` MUST list the modules `collections`, `genid` and `random`. The command MUST select their mutants by file (`libs/engine/src/<module>.rs` and `libs/engine/src/<module>/**`). Its option `-p PACKAGE` MUST be replaced by `--module NAME` (repeatable). `--in-diff` and `-j` stay.
- **The mutants summary** MUST group rows by module (`libs/engine/src/<module>[.rs|/]`) for `engine`, and by package for `examples/*`, as today.
- **`.cargo/mutants.toml`:** every exclusion regex MUST name the new path (for example `^libs/engine/src/genid/id\.rs:`) and MUST still match its mutant.
- **`.github/workflows/mutants.yml`:** the path filters MUST name `libs/engine/src/collections*`, `libs/engine/src/genid*` and `libs/engine/src/random*` (both the root file and the directory).
- **`xtask coverage` summary:** rows MUST be per module for `engine` (`engine::genid` and so on), so the per-area table survives the merge. `examples/*` rows stay per package.
- **`xtask check-statics`:** the `engine_random` paths MUST become `libs/engine/src/random`.
- **`xtask prng-stats`:** MUST use `engine::random::Prng`.
- **`xtask bench-regress`:** its relevance rule (`libs/**`) already covers `libs/engine/`. The test fixtures that name `libs/engine_genid/` or `libs/engine_random/` MUST use the new paths.
- **`xtask hack`** needs no change: it builds `engine` with each feature alone and with none.
- **`clippy.toml`:** every reason text that names `engine_collections::` or `engine_math::scalar::` MUST name `engine::collections::` or `engine::math::scalar::`, plus the `libm` entries in §4.2.

### 4.6 The splitting rule (ADR 0014)

ADR 0014 MUST state the rule below. This section is its specification; the ADR is the authoritative text once accepted.

**Default.** All library code lives in `engine`, organised by modules. A new library crate exists only when at least one split criterion holds and no keep-together rule forbids it.

**Split criteria.** Each MUST be shown with its evidence, not argued:

| ID | Criterion | Evidence required |
|---|---|---|
| S1 | **Required by Rust or Cargo:** procedural macros (crate type `proc-macro`) and C ABI artifacts (`cdylib` or `staticlib`) | The crate type |
| S2 | **Permission boundary:** the code needs an `unsafe` exception (ADR 0001, `unsafe-allowlist.toml`) or a concurrency exception (ADR 0008, `concurrency-allowlist.toml`), both of which are granted per crate | The allowlist entry and its ADR. The crate contains only the code that needs the exception. |
| S3 | **Configurations that cannot be features:** two variants cannot coexist as additive Cargo features, which the Cargo Book names as the case for separate packages | Why an additive feature or a runtime option does not work |
| S4 | **Measured compile time** | A `cargo build --timings` report showing `engine` on the critical path, and before-and-after times for the clean build and for the edit-and-rebuild loop, over repeated runs, with the noise floor, recorded in the spec. The maintainer decides; there is no fixed threshold. |

**Not reasons to split**, with what to use instead:

| Reason | Instead |
|---|---|
| Topic, size or code organisation | Modules |
| Layering between modules | Review, and ADR 0009's public-API discipline |
| Stability, or a frozen format | Golden tests (they freeze the output; a crate boundary does not) |
| An optional dependency | A Cargo feature of `engine` |
| Reuse by other projects | Nothing until publishing is planned. Its ADR revisits this, including the API Guidelines' C-STABLE rule (a 1.0 crate needs stable public dependencies). |

**Keep-together rules.** These override any split criterion:

- **K1, coherence.** The split would make a needed trait impl impossible under the orphan rule, or would force newtype wrappers.
- **K2, shared internals.** The split would need `#[doc(hidden)] pub` back doors.
- **K3, no cycles.** A crate split off below `engine` cannot use `engine`'s types, and `engine` cannot use a crate above it. If the code needs `engine`'s types and `engine` needs the code, it does not split.

**Process.**

1. The spec of the chunk that needs the crate proposes the split: the criterion ID, whether the crate sits below or above `engine`, and the evidence.
2. That chunk's pull request adds the crate's row to ADR 0014's crate table. The crate's docs state the criterion in their first paragraph.
3. When a crate's criterion stops holding (for example, its `unsafe` exception is removed), it merges back into `engine` in the chunk that removed the reason.
4. Non-library packages (`xtask`, `bench/*`, `examples/*`, `spikes/*`, `fuzz`) are outside the rule.
5. New crates keep the existing layout: flat under `libs/`, named `engine_<name>`, with the directory named after the crate.

**Crate table** at acceptance: one row, `engine`, "the library (the default under this ADR)".

ADR 0014 MUST also record what the rule gives up: the compiler no longer stops one module from using another, including the foundation modules using a future ECS module; and in incremental (dev) builds rustc's automatic inlining of small functions across crates is off, so a later split of hot code costs inlining unless the functions are `#[inline]`.

### 4.7 Determinism without `std` (amends ADR 0003)

ADR 0003 says that switching `scalar` and glam to `libm` "is a deliberate, versioned, engine-wide decision and never a Cargo feature". With this chunk the backend does depend on a Cargo feature, `std`, in one narrow case. ADR 0014 MUST amend ADR 0003 as follows, and ADR 0003's status line MUST say it is amended by ADR 0014:

- A build with `std` uses `std`'s transcendental functions, in `scalar` and inside glam, as today. Every target the engine runs on today builds with `std`.
- A build without `std` uses `libm`, in `scalar` and inside glam, because no other backend exists there.
- Within one build the backend is fixed, so same-build determinism holds. Results may differ in the last bits between a `std` and a `no_std` build of the same code, which ADR 0003 already does not promise against.
- The cross-platform switch that ADR 0003 describes (`libm` everywhere, plus glam's `scalar-math`) is unchanged: still a future, deliberate, engine-wide ADR.

### 4.8 Records and documents

- **Live documents** MUST describe one crate: README (the crate table becomes the module table, and the sentence on the splitting rule points to ADR 0014), CONTRIBUTING (mutants commands and the triaged modules, the fuzz paths, the coverage table, the transcendental funnel's path), SECURITY (the two crate names), AGENTS.md (unchanged unless it names a crate), the roadmap, the `engine` crate docs, and the research catalog's lines about this repository.
- **The roadmap** MUST add the M3.13 row. M4's rough list MUST say "the ECS, as modules of `engine`" instead of "`engine_ecs`". M5's "Reflection core traits at the bottom of the crate graph" MUST become "Reflection core traits in `engine`, next to the types they are implemented for". M8's `engine_ffi` stays a crate (S1 and S2).
- **ADR 0004** MUST get status `Superseded by ADR 0014 (date)` and nothing else. **ADR 0003** MUST get the amended status line and nothing else (§4.7). ADR 0012 is not edited.
- **`docs/benchmarks.md`** is a record of measurements. Its tables MUST NOT be rewritten. A dated note MUST say that the subjects named `engine_*` are now modules of `engine`, and that the Criterion subject names are unchanged labels.
- **`docs/mutation-triage.md`** is a record. Its rows MUST NOT be edited. A dated note MUST say that the triaged crates are now modules of `engine`, give the path mapping (`libs/engine_genid/src/` to `libs/engine/src/genid/`, and so on), and say how to triage a module with `--module`.
- **Historical specs and plans** keep the crate names they were written with.
- **CHANGELOG** MUST have an entry under "Changed", marked **breaking**: the five crates are modules of `engine`, `engine::` paths are unchanged, `engine_<name>::` paths are gone, `math` builds without `std`, and `xtask mutants -p` is now `--module`.

### 4.9 Change order

The pull request SHOULD be built as reviewable commits in this order:

1. ADR 0014 and this spec's documents, without the crate changes.
2. The move: `git mv` of each crate's files into `libs/engine/`, with only the path and `use` edits needed to build. No behaviour change, so `git diff -M` shows renames.
3. `no_std` math: glam's `nostd-libm`, `libm`, the `scalar` backend and its documentation, and `cargo xtask no-std` building all of `engine`.
4. Tooling: `xtask`, the mutants configuration and workflow, `clippy.toml`.
5. Documents and notes (§4.8), and the CHANGELOG.

### 4.10 Coordination with M3.12

M3.12 (space metrics) is drafting on its own branch and plans a crate, `engine_heap`, because `HeapSize` must be implemented by both `engine_genid` and `engine_collections`. Under ADR 0014 that crate meets no split criterion, and K1 keeps the trait with its implementations. M3.12's spec, not this chunk, MUST be updated to depend on M3.13 and to put `HeapSize` in a module of `engine`. This chunk MUST NOT edit M3.12's documents.

## 5. Requirements

- **M3.13-R1** `libs/` MUST contain only `engine`, laid out as in §4.1, and the five former crates MUST NOT exist as packages or workspace dependencies.
- **M3.13-R2** Every public path `engine::<module>::<item>` and every prelude item that existed before this chunk MUST still resolve to the same item.
- **M3.13-R3** `engine`'s crate root MUST be `#![no_std]` and `#![forbid(unsafe_code)]`, with the features of §4.2 and no others.
- **M3.13-R4** `cargo xtask no-std` MUST build all of `engine` with `--no-default-features` for `thumbv7em-none-eabihf`, and `cargo xtask wasm` MUST pass with the `engine` package (§4.2).
- **M3.13-R5** Every `math::scalar` function MUST be available without `std` and MUST use the `libm` mapping of §4.2, documented on each function. With `std` its results MUST be bit-identical to before.
- **M3.13-R6** `math`'s layout assertions MUST hold in both the `std` build and the `no_std` build (R4 proves the latter).
- **M3.13-R7** Behaviour MUST be unchanged with `std`: the PRNG golden tests, the `UntypedGenId::to_bits` golden values, the `fixed_step_sim` golden checksum and every existing test MUST pass with only `use`-path changes and the file renames of §4.4.
- **M3.13-R8** `clippy.toml` MUST ban the wrapped `libm` functions outside `scalar` (§4.2), and its reason texts MUST use the new paths.
- **M3.13-R9** The tooling changes of §4.5 MUST be made. `cargo xtask mutants --module genid --list` MUST list the same mutants as `cargo xtask mutants -p engine_genid --list` did before (allowing for the path change), and every `.cargo/mutants.toml` exclusion MUST still match its mutant.
- **M3.13-R10** The coverage and mutants summaries MUST show one row per module of `engine`.
- **M3.13-R11** `cargo xtask fuzz-build` MUST pass with `fuzz` depending on `engine`.
- **M3.13-R12** `cargo xtask bench-regress` MUST report no benchmark above the 5% gate, with the table and the base and head commits quoted in the pull request.
- **M3.13-R13** ADR 0014 MUST contain the rule of §4.6 and the amendment of §4.7, and MUST be accepted. ADR 0004's status MUST read `Superseded by ADR 0014 (date)`, and ADR 0003's status MUST say it is amended by ADR 0014.
- **M3.13-R14** The live documents and records MUST be updated as in §4.8, and the historical records MUST NOT be edited apart from those status lines and notes.
- **M3.13-R15** The CHANGELOG MUST have the breaking entry of §4.8.
- **M3.13-R16** Outside the historical records, ADR 0004, ADR 0014, this spec and plan, the CHANGELOG entry, the notes of §4.8 and the Criterion subject labels, no file MUST name `engine_genid`, `engine_collections`, `engine_random`, `engine_time` or `engine_math` as a crate.

## 6. Cross-cutting concerns

- **Determinism:** unchanged with `std`; R5 and R7 check it. Without `std`, `libm` is the backend, which ADR 0014 records as an amendment to ADR 0003 (§4.7). No `no_std` target is used in simulation today.
- **`unsafe`:** none. One crate root carries `#![forbid(unsafe_code)]` instead of six, and `check-unsafe` keeps checking it. ADR 0014's S2 is how a future `unsafe` crate is created.
- **Concurrency:** n/a, because no shared state is added. The auto-trait tests move with their modules.
- **Heap allocation:** unchanged. The `dhat` and fixed-capacity tests move as they are.
- **`no_std` and wasm:** stronger. All of `engine`, math included, builds without `std`. The wasm test step runs the same tests through one package.
- **Frozen formats:** unchanged. The PRNG streams and `to_bits` golden tests move with their files.
- **Public API changes:** breaking for code that names a leaf crate (`engine_genid::` and so on). Those paths are gone, with no aliases (no consumers exist outside the repository, the crates are `0.x` and `publish = false`). `engine::` paths are unchanged. `pub(crate)` items become visible to the other modules. `xtask mutants -p` becomes `--module`.
- **FFI exposure:** n/a now. M8's `engine_ffi` will be a crate above `engine` under S1 and S2.
- **Performance:** expected to be zero. The bench packages called the leaf crates across a crate boundary before and call `engine` across one now; the bench profile uses thin LTO. Measured with `cargo xtask bench-regress` (R12). Build time: the clean build of the six crates took 1.1 s, of which glam was 1.05 s and each of our crates 0.06 to 0.11 s (`cargo build -p engine --timings`, 2026-11-14), so no build-time change worth measuring is expected; the pull request reports the clean build time before and after.
- **Documentation:** new: ADR 0014 and this spec and plan. Updated: the live documents of §4.8, the status lines of ADR 0003 and ADR 0004, and notes in `benchmarks.md` and `mutation-triage.md`.
- **Policy changes:** yes. ADR 0014 replaces the crate-splitting rule (ADR 0004) and amends ADR 0003 for `no_std` builds. The `unsafe`, concurrency and determinism policies are otherwise unchanged.

## 7. Alternatives considered

- **Keep the current layout (#40, option 1).** Rejected. The justifications in ADR 0004's table mostly protect nothing while nothing is published, the per-crate tooling cost is real (every list in §4.5), and M5's reflection traits would need impls for `glam`, ID and collection types in one crate that depends on all of them, which the current graph cannot provide without contortion.
- **One `no_std` core crate under the facade, with math and the ECS outside it (#40, option 2).** Rejected after discussion. It keeps the per-crate cost for math, and the ECS gets its own crate only for layering, which ADR 0014 rejects as a reason. With the ECS inside, a facade over one core crate re-exports one crate.
- **A core crate plus a separate `engine_ecs` crate.** Rejected: the ECS has no S1 to S4 reason today. If M3.8 decides storage needs `unsafe`, S2 puts only the storage code in its own crate.
- **Keep `math` `std`-only, behind `engine`'s `std` feature.** Rejected. It avoids the ADR 0003 amendment, but the `no_std` build would lose `Transform` and every type built on it, and the ECS would have to gate its transform components. glam's `nostd-libm` keeps `std` behaviour whenever `std` is on.
- **Gate `math::scalar` (only) behind `std`.** Rejected: `Transform2d` uses `scalar`, and glam's own transcendentals use `libm` without `std` anyway, so the amendment is needed either way.
- **A fixed threshold for S4** (for example, at least 10% faster). Rejected: no source gives a threshold, and any number would be arbitrary. Measurement plus the maintainer's decision matches how benchmark claims are handled.
- **Dropping S4 until build time hurts.** Rejected: it would leave no route for a compile-time split without a new ADR, and the evidence requirement already prevents splits on intuition.
- **Layering as a split criterion.** Rejected. Sibling modules have no reason to import each other, and the foundation has no reason to import the ECS. The compiler-enforced DAG would block mistakes nobody is likely to make.
- **Consolidate integration tests into one binary per module now.** Deferred (§2): it changes test files beyond a move, and the move should be reviewable as renames.
- **Deprecated re-export crates (`engine_genid` re-exporting `engine::genid`).** Rejected: no consumers, and they would be crates with no criterion.
- **Name the crate `engine_core`.** Rejected: there is nothing for it to be the core of. `engine` is the name users already depend on.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `ls libs`; review of the root `Cargo.toml`; `cargo metadata --no-deps` lists one library package under `libs/` |
| R2 | The `engine` crate docs' example and `tests/prelude_auto_traits.rs` pass unchanged apart from their file name; a review of the prelude and of each module's `pub` items against the former crate roots |
| R3 | `cargo xtask check-unsafe`; review of `Cargo.toml`; `cargo xtask hack` |
| R4 | `cargo xtask no-std`; `cargo xtask wasm` |
| R5 | Unit tests in `math::scalar` that compare each function with the inherent method (with `std`) bit for bit; the `no_std` build; review of the per-function docs |
| R6 | The layout assertions, which fail the build, in both R4 builds and the `std` build |
| R7 | `cargo xtask test`; `git diff -M --stat` shows the test files as renames; a review that the test diffs touch only `use` lines |
| R8 | `cargo xtask clippy` (part of `ci`) with a deliberate `libm::sinf` call outside `scalar`, run once locally and reported in the pull request; review of `clippy.toml` |
| R9 | `cargo xtask mutants --module genid --list` compared with the pre-change `-p engine_genid --list` output, saved before the move; the `.cargo/mutants.toml` exclusions appear as excluded in a `cargo xtask mutants --module genid` run |
| R10 | `cargo xtask coverage` and the mutants run of R9: their tables have one row per module |
| R11 | `cargo xtask fuzz-build` |
| R12 | `cargo xtask bench-regress` in the devcontainer, table quoted in the pull request |
| R13, R15 | Review |
| R14, R16 | `git grep -nwE 'engine_(genid\|collections\|random\|time\|math)'`, reviewed against the exceptions in R16; the documentation review |

## 9. Done when

- [ ] `libs/` holds only `engine`, and every `engine::` path and prelude item resolves as before.
- [ ] `cargo xtask ci` passes, including `no-std` (all of `engine`, math included) and `wasm`.
- [ ] `cargo xtask bench-regress` shows every benchmark within 5%, quoted in the pull request.
- [ ] `cargo xtask fuzz-build` passes, and `cargo xtask mutants --module genid --list` matches the saved pre-change list.
- [ ] The coverage and mutants tables have one row per module.
- [ ] ADR 0014 is accepted, ADR 0004 reads superseded, and ADR 0003 reads amended.
- [ ] The live documents, the notes in `benchmarks.md` and `mutation-triage.md`, and the CHANGELOG are updated.
- [ ] The pull request reports the clean build time before and after.
- [ ] The roadmap row reads `Done (PR #n)`, this spec reads `Implemented`, and the plan reads `Historical`.
- [ ] The pull request closes #40 and lists the eight reviews.

## 10. Open questions

None. Deferred, with owners:

- **Whether M3.7's `unsafe` storage becomes a crate below `engine` (S2), and what it may depend on (K3):** M3.7 and M3.8.
- **Where `HeapSize` goes:** M3.12, rebased on this chunk (§4.10).
- **Fewer integration-test binaries:** a later chore, if link time shows up in `cargo xtask test`.
- **Publishing, and whether any module should become a published crate:** the publishing ADR, when a release is planned.

## 11. Prior art and sources

Guidance studied; no code copied ([ADR 0002](../../../adr/0002-originality-and-dependency-licensing.md)):

- The Rust Book, "Packages and Crates" (a package holds at most one library crate) and "Cargo Workspaces". <https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html>, <https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html>
- The Cargo Book, "Features": feature unification, "features should be *additive*", the `std` feature pattern for `no_std`, and separate packages as the alternative to mutually exclusive features (S3). <https://doc.rust-lang.org/cargo/reference/features.html>
- The Rust Reference, the orphan rule and inherent impls (K1), and procedural macros needing their own crate type (S1). <https://doc.rust-lang.org/reference/items/implementations.html#orphan-rules>, <https://doc.rust-lang.org/reference/procedural-macros.html>
- The Rust API Guidelines, C-STABLE (public dependencies of a stable crate must be stable), for the publishing ADR. <https://rust-lang.github.io/api-guidelines/necessities.html>
- matklad, "Large Rust Workspaces" (flat layout, `engine_` prefixes, directory named after the crate) and "Fast Rust Builds" ("The most important property of a crate is which crates it doesn't (transitively) depend on"; wide rather than deep graphs; generic code at crate boundaries is compiled again downstream; one link per test binary). <https://matklad.github.io/2021/08/22/large-rust-workspaces.html>, <https://matklad.github.io/2021/09/04/fast-rust-builds.html>
- The Rust Performance Book, "Compile Times" (`cargo build --timings` to find large crates that serialise the build), for S4's evidence. <https://nnethercote.github.io/perf-book/compile-times.html>
- corrode, "Tips for Faster Rust Compile Times" (only changed crates are rebuilt; proc-macro crates block their dependents). <https://corrode.dev/blog/tips-for-faster-rust-compile-times/>
- rustc's automatic cross-crate inlining of small functions (rust-lang/rust PR #116505, Rust 1.75), which is skipped in incremental builds. <https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir_transform/cross_crate_inline.rs.html>
- glam 0.34.3's `nostd-libm` feature (`src/lib.rs:321-328`, `src/f32/math.rs:35`), read from the registry source.
