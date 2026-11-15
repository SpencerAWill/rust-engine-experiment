# M3.13: One `engine` library crate, and the rule for splitting from it

- **Status:** Implemented (PR #54, 2026-11-14)
- **Milestone:** M3 (scheduled during M3, not part of its exit criteria; it must land before M4, which decides where the ECS lives) · **Depends on:** none · **Decides:** [ADR 0004](../../adr/0004-crate-splitting-rule.md), rewritten in place, and [ADR 0003](../../adr/0003-same-build-determinism.md), edited in place for `no_std` builds, as a one-time exception to the rule that accepted ADRs are superseded rather than rewritten (§4.8)
- **Issues:** #40. Related: M3.12 (space metrics, #41, drafting on its own branch), which will depend on this chunk and put `HeapSize` in `engine` instead of a new `engine_heap` crate (§4.11).
- **Estimated size:** about 700 changed lines of code and configuration, excluding tests, lockfiles and generated files: manifests and `xtask` about 200, the `scalar` `no_std` backend about 60, bench labels and `bench-doc` about 40, crate and module docs about 120, ADR 0004 about 120, the live documents about 150. On top of that come:
  - a move of about 12,800 lines (9,600 in the five crates' `src/`, 3,200 in their `tests/` and `benches/`) into `libs/engine`, kept in its own commit so `git diff -M` shows renames (§4.10);
  - a mechanical rewrite of about 270 crate paths in code;
  - a rewrite of the historical records: about 450 mentions across about 30 documents (§4.8);
  - a full Criterion run under the benchmark run protocol, whose results regenerate `docs/benchmarks.md` (§4.9).

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk makes the repository read as if the engine had always been one library crate. It merges the five library crates (`engine_genid`, `engine_collections`, `engine_random`, `engine_time`, `engine_math`) into `engine`, as the modules the facade already exposes (`engine::genid` and so on). It rewrites ADR 0004 in place with a new splitting rule: all library code lives in `engine`, and a separate crate is created only when Rust or Cargo requires one, when a permission boundary needs one, when two configurations cannot be additive features, or when measured compile time justifies one. It also rewrites the documents, records, benchmark labels and benchmark results that name the five crates, so that none of them describes the split layout.

It comes now because M4 decides where the ECS lives, M5 needs reflection traits implemented for `glam`, ID and collection types without orphan-rule workarounds, and M3.12 is about to add a crate whose only reason is the same cross-crate coherence problem. While nothing is published, most of the five crates' justifications protect nothing.

At the end, `libs/` holds one crate. `use engine::prelude::*;` and every `engine::` path work as before. `cargo xtask no-std` builds the whole of `engine`, math included, for a bare-metal target. Outside this spec, its plan and the git history, nothing in the repository names the five crates.

## 2. Goals and non-goals

**Goals**

- One library crate, `engine`, `#![no_std]` with `alloc` and a default `std` feature, containing everything the five crates contain today.
- Unchanged public paths under `engine::`. Only the `engine_<name>` crate paths disappear.
- A splitting rule that can be checked (ADR 0004 as rewritten, §4.6): each split criterion needs evidence, each rejected reason names what to use instead, and the process says where a proposal is made and when a crate merges back.
- A stronger `no_std` proof: math becomes `no_std`-capable, so `cargo xtask no-std` builds every library module.
- No behaviour change: the same PRNG streams, IDs, iteration orders, transform layouts and `fixed_step_sim` checksum.
- The repository reads as if there had always been one crate: code, tooling, configuration, live documents, historical documents, records, benchmark labels and benchmark results (§4.8, §4.9).

**Non-goals**

- **No rewrite of git history or GitHub text.** Commit messages on `main`, pull request titles and bodies, and the text of closed issues keep the names they were written with. Commit hashes cited in documents (for example in `mutation-triage.md`) stay valid. Only files in the repository change.
- **No rewrite of other renames.** The exception is about the crate split only. A document written before M3.11 still says `engine::handle` and `HandlePool`, as it would have in a one-crate history; M3.11 still renames `engine::handle` to `engine::genid`.
- **No change to the policy for future ADRs and records.** This chunk is a one-time exception (§4.8). After it, accepted ADRs are superseded, not rewritten, and records keep the names of their time, as before.
- No new code or API. Moving `HeapSize`, or anything else, into `engine` is the job of the chunk that introduces it (§4.11).
- No consolidation of integration tests into fewer binaries. Each test file moves as it is, with a module prefix (§4.4).
- No layering checks between modules. ADR 0004 rejects layering as a reason to split.
- No cross-platform determinism. The `std` build keeps using `std`'s transcendental functions (§4.7).
- No publishing decision. Publishing gets its own ADR when it is planned.

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

A contributor adding code puts it in a module of `engine`. New code goes in a new module when it is a new topic (the ECS in M4, reflection in M5), not in a new crate. A new crate is proposed in the spec of the chunk that needs it, by naming one of ADR 0004's criteria and giving its evidence:

- **S1, required by Rust or Cargo.** "M5's derive macros need a `proc-macro` crate." The evidence is the crate type.
- **S2, permission boundary.** "M3.7's type-erased columns need `unsafe`, and the chunk's own ADR allowlists `engine_columns`." The crate holds only the code that needs the exception. It sits below `engine`, so it cannot use `engine`'s types (K3).
- **S3, configurations that cannot be features.** For example, two backends that cannot be enabled together. The evidence is why an additive feature or a runtime option does not work.
- **S4, measured compile time.** The evidence is a `cargo build --timings` report showing `engine` on the critical path, plus before-and-after build times over repeated runs with the noise floor. The maintainer decides; there is no fixed threshold.

Building for a bare-metal target:

```sh
cargo build -p engine --no-default-features --target thumbv7em-none-eabihf
```

Without `std`, `SystemClock` is unavailable and transcendental functions (in `engine::math::scalar` and inside glam) come from `libm` instead of `std`. Within one build the backend is fixed, so same-build determinism holds (§4.7).

## 4. Reference-level design

### 4.1 Crate shape and layout

- `libs/` MUST contain exactly one crate, `engine`. `libs/engine_genid`, `libs/engine_collections`, `libs/engine_random`, `libs/engine_time` and `libs/engine_math` MUST NOT exist, and the root `Cargo.toml` MUST NOT list them in `[workspace.dependencies]`. `libs/` stays as the home of any future crate that ADR 0004 allows.
- `engine`'s crate root MUST carry `#![no_std]` and `#![forbid(unsafe_code)]`, and MUST declare `extern crate alloc;` and, under `#[cfg(feature = "std")]`, `extern crate std;`.
- The modules MUST be `genid`, `collections`, `random`, `time` and `math`, all `pub`, at the paths the facade re-exports today (`libs/engine/src/lib.rs:62-66`).
- Each former crate's `src/lib.rs` MUST become `libs/engine/src/<module>.rs`, and its other source files MUST move under `libs/engine/src/<module>/`, keeping their relative paths. This follows the existing `foo.rs` plus `foo/` style (as in `index_map.rs` and `index_map/`), not `mod.rs`.
- Former `crate::` paths inside a module MUST become `crate::<module>::` or `super::` paths. One module refers to another by `crate::<other>::`; nothing goes through the prelude.
- Visibility MUST stay as it is: what was `pub` in a crate's root is `pub` in its module, and what was `pub(crate)` stays `pub(crate)`. A `pub(crate)` item becomes visible to the other modules too, which is accepted (§6, Public API changes).
- The `engine` prelude MUST be unchanged.

### 4.2 Features, `std` and `no_std`

`engine`'s `[features]` MUST be:

| Feature | Default | Enables |
|---|---|---|
| `std` | yes | `extern crate std`; `time::SystemClock` (as today, except on `wasm32-unknown-unknown`); `std` as the transcendental backend of `math::scalar`; `glam/std` |
| `test-invariants` | no | `check_invariants` on `GenIdMap` and `GenIdAllocator`, for integration tests and fuzz targets |

- `engine` MUST have a self dev-dependency that turns on `test-invariants` for its integration tests (`engine = { path = ".", features = ["test-invariants"] }`), as the ID code's tests need it even without `--all-features`.
- `glam` MUST be a dependency with `default-features = false` and the features `f64`, `i32`, `u32` and `nostd-libm`. `nostd-libm` makes glam use `libm` only when glam's `std` feature is off (glam 0.34.3, `src/f32/math.rs:35`). glam's `scalar-math` feature MUST NOT be enabled; the existing layout assertions guard it.
- `libm` MUST be a direct, non-optional dependency, used only by `math::scalar` when `std` is off. glam's `nostd-libm` already brings it into the graph. Its licence (MIT or Apache-2.0) is in ADR 0002's allow list.
- Every function in `math::scalar` MUST be available with and without `std`. With `std` it MUST delegate to the inherent method, as today. Without `std` it MUST delegate to the `libm` function of the same name (`sinf`/`sin` for `f32`/`f64`, and so on). `ln` maps to `libm::log`, `ln_1p` to `log1p`, `exp_m1` to `expm1`, and `log(x, base)` MUST be `ln(x) / ln(base)`. `powi`, which `libm` lacks, MUST be `powf` with the exponent converted to the float type. Each mapping MUST be documented on the function.
- `clippy.toml`'s `disallowed-methods` MUST additionally list the `libm` functions that `scalar` wraps, with the reason "use `engine::math::scalar`", so that `scalar` stays the only caller. (`libm` functions are free functions, so the ban on inherent methods does not cover them.)
- `cargo xtask no-std` MUST build `engine` with `--no-default-features` for `thumbv7em-none-eabihf`. A throwaway probe (glam 0.34.3 with `nostd-libm` on that target) kept `Quat` at size and alignment 16, so `math`'s layout assertions are expected to hold there. The build proves it.
- `cargo xtask wasm` MUST build `engine` for `wasm32-unknown-unknown`, and run `engine`'s library and integration tests on `wasm32-wasip1` under wasmtime.

### 4.3 Crate and module documentation

- `engine`'s crate docs MUST describe one crate: a module map (no "Crate" column), and a first paragraph saying that `engine` is the engine's library crate and that ADR 0004 says when a separate crate is made. The words "facade" and "leaf crates" MUST NOT appear.
- Each former crate's crate docs MUST become its module's docs. The first paragraph's justification for being a separate crate MUST be removed. Everything else (guarantees, choosing a container, the `GenId` glossary, examples) MUST stay.
- Doctests MUST use `engine::<module>::` paths, and MUST NOT go through the prelude where the original did not.
- Statements that a crate is `no_std` MUST become statements about what the module needs without `std`, if anything (for example, `time`: "`SystemClock` needs the `std` feature").
- Every other doc comment or manifest text that says "this crate", "the crate", "the facade" or "leaf crates" about a former crate MUST say "this module" or name `engine` instead. There are 19 such places at the time of writing, for example `check_invariants`'s "a bug in this crate" (`allocator.rs:388`, `map.rs:710`), the `GenId` glossary's "The terms this crate uses", and the `engine` manifest's `description` and `std` feature comment.

### 4.4 Tests, benches, fuzz and examples

- Each former crate's `tests/<name>.rs` MUST move to `libs/engine/tests/<module>_<name>.rs`, for example `tests/genid_model.rs` and `tests/collections_allocations.rs`. The current `libs/engine/tests/auto_traits.rs` MUST become `tests/prelude_auto_traits.rs`. `transform_props.proptest-regressions` MUST be renamed with its test file (`math_transform_props.proptest-regressions`), since proptest finds it by the source file's name.
- Test bodies MUST change only in their `use` paths. No test is weakened or deleted.
- Files that install a global allocator (the `dhat` tests) MUST stay separate test binaries.
- `benches/fixed_timestep.rs` MUST move to `libs/engine/benches/time_fixed_timestep.rs` with its `[[bench]]` entry.
- `bench/compare`, `bench/regress`, `fuzz`, `xtask`, `spikes/*` and every `examples/*` package MUST depend on `engine` alone among the library crates, and use `engine::<module>::` paths. `fuzz` MUST depend on `engine` with `test-invariants`. `fuzz/` is outside the workspace and has its own `Cargo.lock`, which MUST be regenerated and committed. The fuzz build then compiles `engine`'s dependencies (glam, indexmap, foldhash, libm), which it did not before; the pull request MUST say so.

### 4.5 Tooling

- **`xtask no-std` and `xtask wasm`:** as in §4.2.
- **`xtask mutants`:** the unit of selection is a module of `engine`. `TRIAGED` MUST list the modules `collections`, `genid` and `random`. The command MUST select their mutants by file (`libs/engine/src/<module>.rs` and `libs/engine/src/<module>/**`). Its option `-p PACKAGE` MUST be replaced by `--module NAME` (repeatable). `--in-diff` and `-j` stay.
- **The mutants summary** MUST group rows by module for `engine`, and by package for `examples/*`.
- **`.cargo/mutants.toml`:** every exclusion regex MUST name the new path (for example `^libs/engine/src/genid/id\.rs:`) and MUST still match its mutant.
- **`.github/workflows/mutants.yml`:** the path filters MUST name the root file and the directory of each triaged module (`libs/engine/src/genid.rs`, `libs/engine/src/genid/**`, and so on). The comments in `.github/workflows/ci.yml` (line 176) and `nightly.yml` (line 61) that name the ID crate MUST name the module.
- **`std` lints:** before this chunk, a `std::` path in the ID, collection, PRNG or time code failed to compile in every build, because those crates were `#![no_std]` without `std`. In one crate with a default `std` feature, it would fail only in `cargo xtask no-std`, which `ci --quick` does not run. To keep the early failure, `engine`'s crate root MUST carry `#![warn(clippy::std_instead_of_core, clippy::std_instead_of_alloc)]` (clippy runs with warnings denied) and `#![cfg_attr(test, allow(clippy::std_instead_of_core, clippy::std_instead_of_alloc))]` for unit tests. The lints flag a `std::` path only when `core` or `alloc` provides the same item, so `SystemClock`'s `std::time::Instant` is not flagged. They are set on `engine` rather than in `[workspace.lints]`, because the non-library packages use `std` freely and Cargo does not let one package add lints to `workspace = true`.
- **`xtask coverage` summary:** rows MUST be per module for `engine` (`engine::genid` and so on). `examples/*` rows stay per package.
- **`xtask check-statics`:** the PRNG paths MUST become `libs/engine/src/random`.
- **`xtask prng-stats`:** MUST use `engine::random::Prng`.
- **`xtask bench-regress`:** its relevance rule (`libs/**`) already covers `libs/engine/`. Test fixtures that name the old crate paths MUST use the new ones.
- **`xtask hack`** needs no change: it builds `engine` with each feature alone and with none.
- **`clippy.toml`:** every comment and reason text that names a former crate MUST name the module path (`engine::collections::IndexMap`, `engine::math::scalar::sin`, the `engine::genid` glossary), plus the `libm` entries in §4.2.
- **Workspace `Cargo.toml` comments** that name a former crate (for example "the backend of `engine_collections` only") MUST name the module.

### 4.6 The splitting rule (ADR 0004, rewritten in place)

ADR 0004 MUST be rewritten to state the rule below, with a Context written from first principles (crates are Rust's unit of compilation, coherence, visibility and permission; the guidance studied in §11). It MUST NOT describe a five-crate layout or a previous version of the rule. This section is its specification; the ADR is the authoritative text once accepted.

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
2. That chunk's pull request adds the crate's row to ADR 0004's crate table. The crate's docs state the criterion in their first paragraph.
3. When a crate's criterion stops holding (for example, its `unsafe` exception is removed), it merges back into `engine` in the chunk that removed the reason.
4. Non-library packages (`xtask`, `bench/*`, `examples/*`, `spikes/*`, `fuzz`) are outside the rule.
5. New crates are flat under `libs/`, named `engine_<name>`, with the directory named after the crate.

**Crate table:** one row, `engine`, "the library (the default under this ADR)".

ADR 0004 MUST also record what the rule gives up: the compiler does not stop one module from using another, so review does; and in incremental (dev) builds rustc does not infer cross-crate inlining of small functions, so a split of hot code needs `#[inline]` on them.

### 4.7 Determinism without `std` (ADR 0003, edited in place)

ADR 0003's transcendental-funnel paragraph MUST be edited in place so that it reads as if `math` had always been `no_std`-capable:

- `engine::math::scalar` is the funnel. A build with `std` uses `std`'s transcendental functions, in `scalar` and inside glam.
- A build without `std` uses `libm`, in `scalar` and inside glam (through glam's `nostd-libm`), because no other backend exists there.
- Within one build the backend is fixed, so same-build determinism holds. A `std` build and a `no_std` build may differ in the last bits.
- The cross-platform switch (`libm` everywhere, plus glam's `scalar-math`) is a future, deliberate, engine-wide ADR, and is never a Cargo feature. The `std` feature is not that switch: it selects the only backend available on the target.

The "Sanctioned exceptions" sentence MUST name `engine::math::scalar` and `SystemClock`, and the opt-out sentence the non-library packages by package name.

### 4.8 The one-time exception for records

The repository's rules say that an accepted ADR is not rewritten but superseded (`docs/README.md`), and that historical records keep the names they were written with. This chunk is a single, deliberate exception to both, decided by the maintainer: while nothing is published and no one outside the repository depends on the crate layout, the repository should read as if it had always had one library crate. The rules themselves MUST NOT be changed, and the exception MUST NOT be cited as a precedent for anything else.

In scope, each rewritten so that it describes modules of one crate:

- **Live documents:** README (the crate table becomes the module table), CONTRIBUTING, SECURITY, AGENTS.md (if it names a crate), the roadmap, `docs/README.md` (if it names a crate), and the issue and pull request templates (if they name a crate).
- **ADRs:** ADR 0004 (§4.6), ADR 0003 (§4.7), and every other ADR that names a former crate (0005, 0006, 0010, 0011, 0012). Their decisions are otherwise unchanged. ADR 0004's and ADR 0003's status lines MUST read `Accepted (<original date>). Rewritten in place by M3.13 (<date>); see its spec §4.8.` No other ADR's status line changes.
- **Specs and plans** of M0+M1, M2.0 to M2.4, M3.0, M3.1, M3.2 and M3.11. Where a document argues for creating a crate (for example M0+M1's crate list, M3.1's "a new crate because ADR 0004"), the argument MUST be rewritten as the module it became, or removed if it was only about the crate boundary. Commands MUST use the tooling as it is after this chunk (for example `cargo xtask mutants --module genid`). Requirement IDs and section numbers MUST stay as they are.
- **`docs/mutation-triage.md`:** paths become `libs/engine/src/<module>/...` and crate names become module names, in rows and prose. Line and column numbers and commit hashes stay as recorded.
- **`docs/benchmarks.md`:** regenerated (§4.9).
- **`docs/research/data-structures.md`:** its lines about this repository.
- **CHANGELOG:** every existing entry that names a former crate MUST name the module instead (for example M3.1's "new crate `engine_collections`" becomes "new module `engine::collections`"; M3.11's "`engine_handle` is now `engine_genid`" becomes "`engine::handle` is now `engine::genid`"). This chunk's own entry is short and MUST NOT list the former crates (§4.12).

Out of scope: git history, GitHub issues and pull requests (§2), and this spec and its plan, which are the record of the change and name the former crates where they must.

### 4.9 Benchmark labels and a fresh baseline

- The Criterion subjects that name a former crate MUST be renamed to the type they measure:

  | Bench | Today | New |
  |---|---|---|
  | `bench/compare/benches/handles.rs`, value-store group (`GenIdMap`) | `engine_genid_lean` (a bug, below) | `engine::GenIdMap` |
  | `bench/compare/benches/handles.rs`, allocator group (`GenIdAllocator`) | `engine_genid_lean` | `engine::GenIdAllocator` |
  | `bench/compare/benches/prng.rs` | `engine_random` | `engine::Prng` |
  | `bench/compare/benches/collections.rs` | `engine_collections` | `engine::IndexMap` |

  `bench-doc` finds our subjects by the `engine` prefix, which still matches.
- **A bug this fixes.** M3.11 gave the value-store subject the allocator's label, `engine_genid_lean` (`bench/compare/benches/handles.rs:114`; its spec says `engine_genid`). `bench-doc`'s tally looks up the subject `engine_genid` by exact name (`xtask/src/bench_doc.rs:281`), so the "Across all comparable cases" sentence finds no points and is dropped. Following the test-first rule, a test MUST first be added that fails today: every subject that `bench-doc` tallies by name MUST be one that `bench/compare` defines. The tally subject then becomes `engine::GenIdMap`.
- Test fixtures in `xtask` (`bench_doc.rs`, `bench_report/*`) MUST use the new labels.
- A full Criterion run MUST be made under the benchmark run protocol (AC power, `caffeinate`, no builds during the run, the power log checked afterwards) and saved as a named baseline. `cargo xtask bench-doc` MUST regenerate the data sections of `docs/benchmarks.md` from it. The hand-written analysis and run-conditions text MUST be reworded where the new numbers or conditions differ, and MUST NOT carry over a claim the new run does not support. The environment records in `docs/data/` MUST be replaced with the new run's.
- `bench/regress`'s Gungraun targets and functions do not name crates and MUST keep their names, so the required gate compares base and head (R12).

### 4.10 Change order

The pull request SHOULD be built as reviewable commits in this order:

1. The `bench-doc` tally test, failing (§4.9).
2. The move: `git mv` of each crate's files into `libs/engine/`, with only the path and `use` edits needed to build. No behaviour change, so `git diff -M` shows renames.
3. `no_std` math: glam's `nostd-libm`, `libm`, the `scalar` backend and its documentation, and `cargo xtask no-std` building all of `engine`.
4. Tooling: `xtask`, the mutants configuration and workflow, `clippy.toml`.
5. Bench labels, with the tally test now passing.
6. ADR 0004 and ADR 0003 in place, the live documents, and the CHANGELOG.
7. The historical records (§4.8), in one commit per document group (ADRs, M0+M1, M2, M3, records), so each can be reviewed against its original.
8. The fresh baseline and the regenerated `benchmarks.md` (§4.9).

### 4.11 Coordination with M3.12

M3.12 (space metrics) is drafting on its own branch and plans a crate, `engine_heap`, because `HeapSize` must be implemented by both the ID and the collection code. Under ADR 0004 as rewritten, that crate meets no split criterion, and K1 keeps the trait with its implementations. M3.12's spec, not this chunk, MUST be updated to depend on M3.13 and to put `HeapSize` in a module of `engine`. M3.12 keeps ADR number 0013, since this chunk adds no ADR. This chunk MUST NOT edit M3.12's documents. M3.12's branch is based on `38b952e`, before M3.11; it is rebased onto `main` after this chunk merges, and its retrofits then land in the moved files.

### 4.12 CHANGELOG entry

One entry under "Changed", marked **breaking**, without naming the former crates: the library is one crate, `engine`, and every item is reached through its modules (`engine::genid`, `engine::collections`, `engine::random`, `engine::time`, `engine::math`) as before; `math` builds without `std`; ADR 0004 gives the rule for when a separate crate is made; `xtask mutants` selects modules with `--module`; the Criterion subjects are named after the types they measure.

## 5. Requirements

- **M3.13-R1** `libs/` MUST contain only `engine`, laid out as in §4.1, and the five former crates MUST NOT exist as packages or workspace dependencies.
- **M3.13-R2** Every public path `engine::<module>::<item>` and every prelude item that existed before this chunk MUST still resolve to the same item.
- **M3.13-R3** `engine`'s crate root MUST be `#![no_std]` and `#![forbid(unsafe_code)]`, with the features of §4.2 and no others.
- **M3.13-R4** `cargo xtask no-std` MUST build all of `engine` with `--no-default-features` for `thumbv7em-none-eabihf`, and `cargo xtask wasm` MUST pass with the `engine` package (§4.2).
- **M3.13-R5** Every `math::scalar` function MUST be available without `std` and MUST use the `libm` mapping of §4.2, documented on each function. With `std` its results MUST be bit-identical to before.
- **M3.13-R6** `math`'s layout assertions MUST hold in both the `std` build and the `no_std` build.
- **M3.13-R7** Behaviour MUST be unchanged with `std`: the PRNG golden tests, the `UntypedGenId::to_bits` golden values, the `fixed_step_sim` golden checksum and every existing test MUST pass with only `use`-path changes and the file renames of §4.4.
- **M3.13-R8** `clippy.toml` MUST ban the wrapped `libm` functions outside `scalar` (§4.2).
- **M3.13-R9** The tooling changes of §4.5 MUST be made. `cargo xtask mutants --module genid --list` MUST list the same mutants as `cargo xtask mutants -p engine_genid --list` did before (allowing for the path change), and every `.cargo/mutants.toml` exclusion MUST still match its mutant.
- **M3.13-R10** The coverage and mutants summaries MUST show one row per module of `engine`.
- **M3.13-R11** `cargo xtask fuzz-build` MUST pass with `fuzz` depending on `engine`, with `fuzz/Cargo.lock` updated and committed.
- **M3.13-R12** `cargo xtask bench-regress` MUST report no benchmark above the 5% gate, with the table and the base and head commits quoted in the pull request.
- **M3.13-R13** ADR 0004 MUST contain the rule of §4.6, ADR 0003 the text of §4.7, and both MUST carry the status line of §4.8 and be accepted.
- **M3.13-R14** Every document in scope of §4.8 MUST describe modules of one crate, with requirement IDs and section numbers unchanged. The rules in `docs/README.md` and CONTRIBUTING about superseding ADRs and keeping records MUST be unchanged.
- **M3.13-R15** The Criterion subjects MUST be renamed as in §4.9, and the `bench-doc` tally test MUST have failed before the rename and pass after it.
- **M3.13-R16** `docs/benchmarks.md`'s data sections MUST be regenerated from a baseline saved in this chunk under the run protocol, with the power log checked, and its prose MUST make no claim the new run does not support.
- **M3.13-R17** The CHANGELOG MUST have the entry of §4.12, and its earlier entries MUST name modules, not the former crates.
- **M3.13-R18** Outside this spec and its plan, no file in the repository MUST name `engine_genid`, `engine_collections`, `engine_random`, `engine_time`, `engine_math` or `engine_handle`.
- **M3.13-R19** `engine`'s crate root MUST enable `clippy::std_instead_of_core` and `clippy::std_instead_of_alloc` outside `#[cfg(test)]` code (§4.5), and no code in `engine` MUST trigger them.

## 6. Cross-cutting concerns

- **Determinism:** unchanged with `std`; R5 and R7 check it. Without `std`, `libm` is the backend, which ADR 0003 now states (§4.7). No `no_std` target is used in simulation today.
- **`unsafe`:** none. One crate root carries `#![forbid(unsafe_code)]` instead of six, and `check-unsafe` keeps checking it. ADR 0004's S2 is how a future `unsafe` crate is created.
- **Concurrency:** n/a, because no shared state is added. The auto-trait tests move with their modules.
- **Heap allocation:** unchanged. The `dhat` and fixed-capacity tests move as they are.
- **`no_std` and wasm:** stronger. All of `engine`, math included, builds without `std`. The `std` lints (§4.5) keep a stray `std::` path failing in `ci --quick`, as it did when four of the modules were `#![no_std]` crates.
- **Frozen formats:** unchanged. The PRNG streams and `to_bits` golden tests move with their files.
- **Public API changes:** breaking for code that names a former crate. Those paths are gone, with no aliases: no consumers exist outside the repository, everything is `0.x` and `publish = false`. `engine::` paths are unchanged. `pub(crate)` items become visible to the other modules. `xtask mutants -p` becomes `--module`. Criterion subjects are renamed, so saved baselines from before this chunk no longer compare; the fresh baseline replaces them (§4.9).
- **FFI exposure:** n/a now. M8's `engine_ffi` will be a crate above `engine` under S1 and S2.
- **Performance:** expected to be zero. The bench packages called the former crates across a crate boundary and call `engine` across one now; the bench profile uses thin LTO. Measured with `cargo xtask bench-regress` (R12). The Criterion run (R16) is a fresh baseline, not a before-and-after comparison. Build time: the clean build of the six crates took 1.1 s, of which glam was 1.05 s and each of our crates 0.06 to 0.11 s (`cargo build -p engine --timings`, 2026-11-14); the pull request reports the clean build time before and after.
- **Documentation:** ADR 0004 and ADR 0003 rewritten in place; every document in §4.8 rewritten; `benchmarks.md` regenerated; this spec and plan new.
- **Policy changes:** ADR 0004's rule is replaced, and ADR 0003 states the `no_std` backend. The rules for superseding ADRs and keeping records are unchanged; this chunk is a one-time exception to them (§4.8). The `unsafe`, concurrency and determinism policies are otherwise unchanged.

## 7. Alternatives considered

- **Keep the current layout (#40, option 1).** Rejected. The justifications in ADR 0004's table mostly protect nothing while nothing is published, the per-crate tooling cost is real (every list in §4.5), and M5's reflection traits would need impls for `glam`, ID and collection types in one crate that depends on all of them.
- **One `no_std` core crate under the facade, with math and the ECS outside it (#40, option 2).** Rejected. It keeps the per-crate cost for math, and the ECS gets its own crate only for layering, which ADR 0004 now rejects as a reason.
- **A core crate plus a separate `engine_ecs` crate.** Rejected: the ECS has no S1 to S4 reason today. If M3.8 decides storage needs `unsafe`, S2 puts only the storage code in its own crate.
- **A new ADR 0014 that supersedes ADR 0004 and amends ADR 0003, with the historical records kept as written plus notes.** This was the first draft of this spec. Rejected by the maintainer: the repository should read as if the split had never happened, and nothing outside it depends on the old layout.
- **Change the rule on rewriting ADRs and records, rather than make an exception.** Rejected by the maintainer: the rule stays for everything after this chunk.
- **Rewrite git history and GitHub text too.** Rejected. Rewriting `main` needs a force-push to a protected branch, invalidates the commit hashes cited in documents, and breaks every clone and link. GitHub text is outward-facing and records what was said at the time.
- **Move the crate to `engine/` at the root, or make the root manifest the package.** Rejected: `libs/` stays the home for future crates under ADR 0004, and matklad advises a virtual root manifest.
- **Keep the Criterion labels**, so old baselines still compare. Rejected: the labels name the former crates, and one of them is wrong (§4.9).
- **Rename the old benchmark tables instead of re-running.** Rejected: the tables would name subjects that were not measured under those names.
- **Keep `math` `std`-only, behind `engine`'s `std` feature.** Rejected. The `no_std` build would lose `Transform` and every type built on it, and the ECS would have to gate its transform components.
- **Gate `math::scalar` (only) behind `std`.** Rejected: `Transform2d` uses `scalar`, and glam's own transcendentals use `libm` without `std` anyway.
- **A fixed threshold for S4.** Rejected: no source gives a threshold, and any number would be arbitrary.
- **Dropping S4 until build time hurts.** Rejected: it would leave no route for a compile-time split without rewriting the ADR.
- **Layering as a split criterion.** Rejected: sibling modules have no reason to import each other, and the compiler-enforced DAG would block mistakes nobody is likely to make.
- **Consolidate integration tests into one binary per module now.** Deferred (§2): the move should be reviewable as renames.
- **Deprecated re-export crates.** Rejected: no consumers, and they would be crates with no criterion.
- **Name the crate `engine_core`.** Rejected: `engine` is the name users already depend on.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `ls libs`; review of the root `Cargo.toml`; `cargo metadata --no-deps` lists one library package under `libs/` |
| R2 | The `engine` crate docs' example and `tests/prelude_auto_traits.rs` pass; a review of the prelude and of each module's `pub` items against the former crate roots |
| R3 | `cargo xtask check-unsafe`; review of `Cargo.toml`; `cargo xtask hack` |
| R4 | `cargo xtask no-std`; `cargo xtask wasm` |
| R5 | Unit tests in `math::scalar` that compare each function with the inherent method bit for bit (with `std`); the `no_std` build; review of the per-function docs |
| R6 | The layout assertions, which fail the build, in the `std` build and both R4 builds |
| R7 | `cargo xtask test`; `git diff -M --stat` shows the test files as renames; a review that the test diffs touch only `use` lines |
| R8 | `cargo xtask clippy` with a deliberate `libm::sinf` call outside `scalar`, run once locally and reported in the pull request |
| R9 | `cargo xtask mutants --module genid --list` compared with the pre-change `-p engine_genid --list` output, saved before the move; the exclusions appear as excluded in a `cargo xtask mutants --module genid` run |
| R10 | `cargo xtask coverage` and the mutants run of R9 |
| R11 | `cargo xtask fuzz-build` |
| R12 | `cargo xtask bench-regress` in the devcontainer, table quoted in the pull request |
| R13, R14 | Review, document group by document group, against the originals (§4.10, step 7) |
| R15 | The tally test's failing output from step 1, quoted in the pull request, and `cargo xtask test` passing after step 5 |
| R16 | The saved baseline's name, the power log check and the run conditions, recorded in `benchmarks.md` and the pull request; `cargo xtask bench-doc` output matches the committed data sections |
| R17 | Review |
| R18 | `git grep -nwE 'engine_(genid\|collections\|random\|time\|math\|handle)'` returns only this spec and its plan |
| R19 | `cargo xtask clippy` with a deliberate `use std::vec::Vec;` in `libs/engine/src/genid/map.rs`, run once locally and reported in the pull request; review of `libs/engine/src/lib.rs` |

## 9. Done when

- [x] `libs/` holds only `engine`, and every `engine::` path and prelude item resolves as before. Evidence: `ls libs` prints `engine`; the public-item list matches the old one apart from the removed crate roots (Step 4); the doctests and `prelude_auto_traits` pass.
- [x] `cargo xtask ci` passes, including `no-std` (all of `engine`, math included) and `wasm`. Evidence: `cargo xtask ci` passed on the last code commit (Steps 5 to 7); the close-out and later commits change only documents and data files.
- [x] `cargo xtask bench-regress` shows every benchmark within 5%, quoted in the pull request. Evidence: No regression: 19 benchmarks, all within ±0.7%, base `dbf5c7b`, head `8b856f2` (arm64 Linux container); the table is in the pull request.
- [x] `cargo xtask fuzz-build` passes, and `cargo xtask mutants --module genid --list` matches the saved pre-change list. Evidence: `fuzz-build` passes (inside `ci`). The mutant list matches except that two descriptions say `crate::genid::InvariantViolation` where they said `crate::InvariantViolation`; the four exclusions each match one mutant.
- [x] The coverage and mutants tables have one row per module. Evidence: Unit tests `engine_rows_are_per_module` and `rows_are_ordered_by_module_with_total_last` pass. `cargo xtask coverage` itself was not run locally (`llvm-tools-preview` is missing).
- [x] The `std_instead_of_core` and `std_instead_of_alloc` lints are on, and a deliberate `std::` path fails clippy. Evidence: A temporary `use std::vec::Vec as ProbeVec;` in `genid/map.rs` failed `cargo xtask clippy` on `std_instead_of_alloc`.
- [x] The `bench-doc` tally test failed before the label rename and passes after it. Evidence: `tally_subject_is_a_compare_bench_subject` failed with "defines no subject named `engine_genid`" (Step 3), then passed.
- [x] A fresh baseline is saved under the run protocol, and `benchmarks.md` is regenerated from it. Evidence: Baseline `m3.13` (476 benchmarks), 21:10 to 22:10 local on AC power, no sleep in the power log. Caveats (a busy background process for the first 16 minutes) are in `benchmarks.md`.
- [x] ADR 0004 and ADR 0003 are rewritten in place and accepted, and every document in §4.8 describes one crate. Evidence: Steps 7 and 8.
- [x] `git grep` for the former crate names returns only this spec and its plan. Evidence: Checked after Step 9.
- [x] The pull request reports the clean build time before and after. Evidence: About 1.07 s before and about 1.9 s after (three runs); see the pull request.
- [x] The roadmap row reads `Done (PR #n)`, this spec reads `Implemented`, and the plan reads `Historical`. Evidence: Done in the close-out commit.
- [x] The pull request closes #40 and lists the eight reviews. Evidence: See the pull request.

## 10. Open questions

None. Deferred, with owners:

- **Whether M3.7's `unsafe` storage becomes a crate below `engine` (S2), and what it may depend on (K3):** M3.7 and M3.8.
- **Where `HeapSize` goes:** M3.12, rebased on this chunk (§4.11).
- **Fewer integration-test binaries:** a later chore, if link time shows up in `cargo xtask test`.
- **Publishing, and whether any module should become a published crate:** the publishing ADR, when a release is planned.

## 11. Prior art and sources

Guidance studied; no code copied ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)):

- The Rust Book, "Packages and Crates" (a package holds at most one library crate) and "Cargo Workspaces". <https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html>, <https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html>
- The Cargo Book, "Features": feature unification, "features should be *additive*", the `std` feature pattern for `no_std`, and separate packages as the alternative to mutually exclusive features (S3). <https://doc.rust-lang.org/cargo/reference/features.html>
- The Rust Reference, the orphan rule and inherent impls (K1), and procedural macros needing their own crate type (S1). <https://doc.rust-lang.org/reference/items/implementations.html#orphan-rules>, <https://doc.rust-lang.org/reference/procedural-macros.html>
- The Rust API Guidelines, C-STABLE (public dependencies of a stable crate must be stable), for the publishing ADR. <https://rust-lang.github.io/api-guidelines/necessities.html>
- matklad, "Large Rust Workspaces" (flat layout, a virtual root manifest, the directory named after the crate) and "Fast Rust Builds" ("The most important property of a crate is which crates it doesn't (transitively) depend on"; wide rather than deep graphs; generic code at crate boundaries is compiled again downstream; one link per test binary). <https://matklad.github.io/2021/08/22/large-rust-workspaces.html>, <https://matklad.github.io/2021/09/04/fast-rust-builds.html>
- The Rust Performance Book, "Compile Times" (`cargo build --timings` to find large crates that serialise the build), for S4's evidence. <https://nnethercote.github.io/perf-book/compile-times.html>
- corrode, "Tips for Faster Rust Compile Times" (only changed crates are rebuilt; proc-macro crates block their dependents). <https://corrode.dev/blog/tips-for-faster-rust-compile-times/>
- rustc's automatic cross-crate inlining of small functions (rust-lang/rust PR #116505, Rust 1.75), which is skipped in incremental builds. <https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir_transform/cross_crate_inline.rs.html>
- glam 0.34.3's `nostd-libm` feature (`src/lib.rs:321-328`, `src/f32/math.rs:35`), read from the registry source.
