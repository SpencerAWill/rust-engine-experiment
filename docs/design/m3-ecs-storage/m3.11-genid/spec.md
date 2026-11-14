# M3.11: `engine_genid`: one lean allocator, `GenId<T>`, and an untyped core

- **Status:** Draft (2026-11-14)
- **Milestone:** M3 (scheduled during M3; not part of its exit criteria, which are about the ECS storage decision) · **Depends on:** none · **Decides:** [ADR 0012](../../../adr/0012-generational-ids.md), which supersedes [ADR 0010](../../../adr/0010-two-handle-allocators.md)
- **Issues:** #34 (remove `DenseHandleAllocator`) and #39 (rename `Handle` to `GenId`, with an `UntypedGenId` core). #40 (crate granularity) is related but out of scope.
- **Estimated size:** about 450 changed lines of new or rewritten code, excluding tests, lockfiles and generated files: the untyped core and the typed wrapper about 250, the fuzz and bench changes about 80, configuration and documents about 120. On top of that come about 1,100 deleted lines (`dense_allocator.rs`, its fuzz target, its tests and benches) and a mechanical rename that touches about 1,500 lines. The rename is kept in its own commits so it can be reviewed as a rename (§4.8).

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk does three things to `engine_handle`, in order:

1. It deletes `DenseHandleAllocator<T>` (#34), leaving one ID-only allocator, with `GenIdPool<()>` as the way to iterate live IDs.
2. It renames the crate to `engine_genid` and its types from `Handle*` to `GenId*` (#39).
3. It splits the identifier into an untyped core, `UntypedGenId`, and a typed `#[repr(transparent)]` wrapper, `GenId<T>`.

It comes now because nothing depends on these names yet. M4 builds `Entity` on this crate and the M3.6 prototype passes type-erased IDs, so the names and the untyped type should be settled before either exists.

At the end, `use engine::prelude::*;` gives `GenId`, `GenIdPool` and `GenIdAllocator`. The bit layout `(index << 32) | generation` is unchanged and still proven by the same golden values, and the instruction-count gate shows no change in the container hot paths.

## 2. Goals and non-goals

**Goals**

- One ID-only allocator. Iterating live IDs uses `GenIdPool<()>`, which already does exactly that.
- A name that says what the type is: a generational ID, an index plus a generation that is checked on use. It must not suggest ownership. "Handle" stays free for a future reference-counted asset handle.
- A real type for code without a compile-time `T`: the M3.6 runtime-registered components, the M4 registry, M5 serialisation and the M8 C ABI. Today that code passes a bare `u64`. With `UntypedGenId` it gets `index()`, `generation()`, a readable `Debug` and the `Option` niche.
- No change at runtime: the same sizes, the same bit layout, and the same IDs for the same operations.
- Live documents use the new names. Historical ones keep the names they were written with.

**Non-goals**

- No change to the bit layout, and no other widths or pluggable layouts (32-bit IDs, configurable splits). See #39's "Future" section.
- No untyped operations on the containers (`pool.get_untyped(..)` and the like). They are added when M3.6 or the M4 registry needs them (§10).
- No change to the crate layout beyond the rename. Merging crates is #40.
- No removal of `GenIdAllocator`. Whether M4's world needs an ID-only allocator is decided after M3 (#34, "Alternatives").
- No deprecated aliases for the old names (§7).
- No new wall-clock measurements. The tables in `benchmarks.md` stay as they were measured (§4.7).

## 3. Guide-level explanation

A `GenId<T>` names a value of type `T` in the pool that issued it. When the value is removed, the slot's generation goes up, so the old ID stops resolving:

```rust
use engine_genid::{GenId, GenIdPool};

struct Enemy { hp: u32 }

let mut enemies = GenIdPool::new();
let id: GenId<Enemy> = enemies.insert(Enemy { hp: 10 });
assert_eq!(enemies[id].hp, 10);

enemies.remove(id);
assert!(enemies.get(id).is_none()); // stale, not a different enemy
```

If the data lives elsewhere, use the allocator, which only hands out IDs. To iterate the live IDs too, use a pool of `()`, which stores no values:

```rust
use engine_genid::{GenIdAllocator, GenIdPool};

struct Body;

let mut ids = GenIdAllocator::<Body>::new();
let a = ids.allocate();
assert!(ids.is_alive(a));

let mut live = GenIdPool::<()>::new();
let b = live.insert(());
assert_eq!(live.iter().map(|(id, _)| id).collect::<Vec<_>>(), [b]);
```

Code that does not know `T` at compile time (a registry of runtime-defined components, a serialiser, the C ABI) works with `UntypedGenId`. Converting is explicit in both directions, and nothing converts one typed ID into another:

```rust
use engine_genid::{GenId, UntypedGenId};

struct Texture;

let typed = GenId::<Texture>::from_bits((3 << 32) | 2).unwrap();
let erased: UntypedGenId = typed.untyped();
assert_eq!((erased.index(), erased.generation().get()), (3, 2));
assert_eq!(format!("{erased:?}"), "UntypedGenId(3v2)");

// Back to a typed ID: the caller states the type.
let again = GenId::<Texture>::from_untyped(erased);
assert_eq!(again, typed);
assert_eq!(again.to_bits(), erased.to_bits()); // one frozen layout
```

## 4. Reference-level design

### 4.1 Removal of `DenseHandleAllocator`

- `DenseHandleAllocator<T>` and `DenseAllocIter<'a, T>` MUST be deleted, together with `libs/engine_handle/src/dense_allocator.rs`, their re-exports in the crate root and in `engine` (module and prelude), the `dense_allocator_ops` fuzz target (its file, its `fuzz/Cargo.toml` entry, and every list in `xtask` and the workflows that names it), their cases in the integration tests and `libs/engine/tests/auto_traits.rs`, and their benchmarks in `bench/regress` and `bench/compare`.
- Tests that use the dense allocator only to cover shared code (the slot table, the slot limit) MUST be moved onto `GenIdPool<()>` or `GenIdAllocator`, so the shared code keeps the same coverage. The mutation-triage row that a dense-allocator test fixed (`HandlePool<T>::slot_limit`, `docs/mutation-triage.md`) MUST still be caught by a test after the move.
- The model test requiring the lean allocator and the pool to issue identical IDs for the same operations MUST be kept.
- Iterating live IDs without values is `GenIdPool<()>`: `iter`, `retain`, `len`, `drain`. The crate documentation MUST say so in its "Choosing a container" table.

### 4.2 Renames

| Before | After |
|---|---|
| crate `engine_handle`, at `libs/engine_handle` | crate `engine_genid`, at `libs/engine_genid` |
| facade module `engine::handle` | `engine::genid` |
| `Handle<T>` | `GenId<T>` |
| (new) | `UntypedGenId` (§4.3) |
| `HandlePool<T>` | `GenIdPool<T>` |
| `HandleAllocator<T>` | `GenIdAllocator<T>` |
| `InvalidHandleBits` | `InvalidGenIdBits` |
| `CapacityError<T>`, `InvariantViolation` | unchanged |
| `Iter`, `IterMut`, `Drain`, `IntoIter` (pool iterators) | unchanged, under `engine_genid` |
| fuzz targets `handle_from_bits`, `handle_pool_ops`, `handle_allocator_ops` | `genid_from_bits`, `genid_pool_ops`, `genid_allocator_ops` |
| regress bench target `bench/regress/benches/handle.rs` (`[[bench]] name = "handle"`) | unchanged in this chunk (§4.7) |
| Criterion subjects `engine_handle`, `engine_handle_lean` (`bench/compare`) | `engine_genid` |

- Method names MUST NOT change. None of them contains "handle".
- Parameters, locals, private fields (the pool's `handles` array becomes `ids`), messages and prose in code and docs MUST say "ID" or "id" for the identifier, not "handle". Panic and `Display` messages MUST name the new types, for example `GenIdPool is full: ...`.
- The Gungraun bench target name (`handle`), its group names (`pool`, `allocator`) and its function names (`pool_insert_1k`, `allocator_allocate_1k`, ...) MUST NOT change. Gungraun keys results by target, group and function, so any rename would make the instruction-count gate treat the benchmark as new and compare nothing (§4.7). The code inside the target uses the new names.
- The old names MUST NOT remain as aliases or deprecated items (§7).
- Workspace configuration that names the crate or its paths MUST be updated: `[workspace.dependencies]`, the self dev-dependency that enables `test-invariants`, `fuzz/Cargo.toml`, the `no-std`, `wasm` and `mutants` crate lists in `xtask`, the `mutants.yml` path filter, the comments in `ci.yml` and `nightly.yml`, and the `.cargo/mutants.toml` exclusion patterns. Those patterns match file paths and function names. A pattern that matches nothing after the rename does not fail; it silently stops excluding the mutant. So every pattern MUST be rewritten to the new path and name, and checked to still match (§8).

### 4.3 `UntypedGenId`

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UntypedGenId {
    index: u32,
    generation: NonZeroU32,
}
```

| Item | Semantics |
|---|---|
| `const fn index(self) -> u32` | The slot index |
| `const fn generation(self) -> NonZeroU32` | The slot's generation when the ID was issued |
| `const fn to_bits(self) -> u64` | `(index as u64) << 32 \| generation as u64`. Never 0. **Frozen** (§4.5) |
| `const fn from_bits(bits: u64) -> Option<Self>` | The inverse of `to_bits`. `None` if and only if the low 32 bits are 0 |
| `impl From<UntypedGenId> for u64` | Same as `to_bits` |
| `impl TryFrom<u64> for UntypedGenId`, `Error = InvalidGenIdBits` | Same as `from_bits`, carrying the rejected bits |
| `Ord`, `PartialOrd` | Orders by `(index, generation)`, which equals the numeric order of `to_bits` |
| `Hash` | Hashes `to_bits()` |
| `Debug` | `UntypedGenId(3v2)` (index 3, generation 2) |

- The fields MUST stay private. The field order and struct layout are not part of any contract: only `to_bits` is. M8 passes IDs across the C ABI as `to_bits` values, not as structs.
- `UntypedGenId` MUST NOT implement `Default`. No ID is a sensible default, and 0 is not a valid encoding.
- It MUST be `Send`, `Sync`, `Unpin`, `UnwindSafe` and `RefUnwindSafe` (checked by the auto-trait tests).
- Public construction is through `from_bits`, `TryFrom<u64>` and `GenId::untyped`. A crate-private constructor from an `(index, generation)` pair MAY exist for the containers.

### 4.4 `GenId<T>`

```rust
#[repr(transparent)]
pub struct GenId<T> {
    untyped: UntypedGenId,
    // `fn() -> T`: covariant in `T`, `Send + Sync` for any `T`, and no drop-check implications.
    _marker: PhantomData<fn() -> T>,
}
```

| Item | Semantics |
|---|---|
| `const fn untyped(self) -> UntypedGenId` | The same index and generation, without the type |
| `const fn from_untyped(id: UntypedGenId) -> Self` | The same index and generation, typed as `T`. Meaningful only for an ID that a pool of `T` issued, the same caveat as `from_bits` |
| `const fn index`, `generation`, `to_bits`, `from_bits` | Forward to `UntypedGenId`. `to_bits` MUST equal `self.untyped().to_bits()` for every ID |
| `From<GenId<T>> for u64`, `TryFrom<u64> for GenId<T>` (`InvalidGenIdBits`) | As before, through the untyped versions |
| `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` | Implemented by hand for every `T`, with no bounds on `T`, and identical to the same trait on `untyped()` |
| `Debug` | `GenId<Particle>(3v2)`, with the short type name (every `path::` prefix removed, as today) |

- `GenId<T>` MUST NOT have `cast::<U>()`, `Deref<Target = UntypedGenId>`, `From<UntypedGenId>`, `AsRef`, or any other conversion that changes or adds a type without naming `GenId::<T>::from_untyped`. The first two are covered by `compile_fail` doctests.
- It MUST NOT implement `Default`.
- The containers MUST keep accepting and returning only `GenId<T>`.
- `#[repr(transparent)]` is a layout guarantee, not an `unsafe` licence: the crate stays `#![forbid(unsafe_code)]`. The attribute documents the intent, and the size checks in §4.6 prove the result.

### 4.5 The frozen format

The encoding is unchanged: `(index << 32) | generation`, index in the high half, never 0.

- Its normative home MUST move to `UntypedGenId::to_bits`, and `GenId::to_bits` MUST be documented as forwarding to it.
- The existing golden value, `to_bits` of index 3 and generation 2 equals `0x0000_0003_0000_0002`, MUST be checked for both types.
- The `fixed_step_sim` golden checksum feeds `to_bits` into `Prng::stream`, and MUST NOT change.
- Live documents that name `Handle::to_bits` as frozen MUST name `UntypedGenId::to_bits` (and `GenId::to_bits`) instead: AGENTS.md, CONTRIBUTING.md, README.md, the CHANGELOG preamble, `bug_report.md`, `feature_request.md`, `determinism_bug.yml` and the pull request template. The rule itself does not change.

### 4.6 Size and niche

Compile-time assertions (`const _: () = assert!(...)`) in the crate MUST check:

- `size_of::<UntypedGenId>() == 8` and `size_of::<Option<UntypedGenId>>() == 8`;
- `size_of::<GenId<T>>() == 8` and `size_of::<Option<GenId<T>>>() == 8`, for a zero-sized `T` and for a large `T`;
- `align_of::<GenId<T>>() == align_of::<UntypedGenId>()`.

### 4.7 Benchmarks and measurement records

- `bench/regress`: the `dense` group is removed. The rest keep their target, group and function names (§4.2), and a comment in the target says why it is still called `handle`. `cargo xtask bench-regress` MUST be run in the devcontainer before the pull request is ready. Every remaining benchmark MUST be within the gate's 5%, and the pull request MUST quote the table with base and head commits. Its relevance rule MUST cover `libs/engine_genid/`.
- `bench/compare`: the dense subjects are removed, and the remaining ones are renamed per §4.2. `bench-doc` finds our subjects by the `engine` prefix, which still matches. Its hard-coded tally subject (`"engine_handle"` in `xtask/src/bench_doc.rs`) and its test fixtures MUST be updated to `"engine_genid"`.
- `docs/benchmarks.md` is a record of measurements. Its tables and analysis MUST NOT be rewritten to the new names without a new run. A dated note at the top of the handle section MUST say that the results were measured before ADR 0012, map the old subject names to the new ones, and say that the `engine_handle_dense` rows describe a removed type. The next `bench-doc` run regenerates the tables under the new names.
- `docs/mutation-triage.md` is also a record. Its existing rows MUST NOT be edited. A dated note MUST say that `engine_handle` is now `engine_genid`, that the `dense_allocator.rs` rows refer to a deleted file, and where the `slot_limit` row's test now lives.

### 4.8 Change order

The pull request SHOULD be built as reviewable commits in this order:

1. Remove the dense allocator, with tests moved first so the shared code's coverage does not drop.
2. Rename the crate and types mechanically (`git mv` and identifier renames only, no behaviour change), so `git diff -M` shows renames.
3. Introduce `UntypedGenId` and rewrite `GenId<T>` over it.
4. Write ADR 0012 and update the documents and configuration.

### 4.9 Crate documentation

The first paragraph of the crate docs MUST give its [ADR 0004](../../../adr/0004-crate-splitting-rule.md) justification under the new name: platform (`no_std` with `alloc`), dependency-free, FFI exposure (IDs cross the C ABI in M8), and layering (the foundation for the ECS, assets and FFI). The `engine_handle` row of ADR 0004's crate table MUST be updated to `engine_genid`. That table is the current list of crates, as M3.1 established when it added `engine_collections`; the ADR's decision text is not touched.

The "Choosing a container" table becomes:

| Need | Use |
| --- | --- |
| Store values, iterate them contiguously | `GenIdPool<T>` |
| Only IDs (data lives elsewhere), no iteration | `GenIdAllocator<T>` |
| Only IDs, and iterate the live ones | `GenIdPool<()>` |
| Work with IDs whose `T` is not known at compile time | `UntypedGenId` |

The "Known limitation" section (an ID used on a pool other than the one that issued it) stays, and is worded for `GenId`. SECURITY.md's equivalent sentence is updated the same way.

## 5. Requirements

- **M3.11-R1** `DenseHandleAllocator` and `DenseAllocIter` MUST NOT exist. No file names them except the historical records (§6, Documentation), ADR 0012, this spec and plan, and this chunk's CHANGELOG entry.
- **M3.11-R2** The crate MUST be `engine_genid` at `libs/engine_genid`, re-exported as `engine::genid`, with `GenId`, `GenIdPool` and `GenIdAllocator` in `engine::prelude` and `UntypedGenId` not in the prelude. `engine::handle` and `engine_handle` MUST NOT exist.
- **M3.11-R3** Every rename in §4.2 MUST be applied, with no alias left for an old name.
- **M3.11-R4** `UntypedGenId` MUST provide exactly the API in §4.3, with no `Default`.
- **M3.11-R5** `GenId<T>` MUST be `#[repr(transparent)]` over `UntypedGenId` and provide the API in §4.4. `untyped()` and `from_untyped()` MUST round-trip every ID.
- **M3.11-R6** `GenId<T>` MUST NOT have any conversion that changes or adds a type other than `from_untyped`. Mixing two `GenId` types and calling `cast` MUST fail to compile.
- **M3.11-R7** `to_bits` MUST produce `(index << 32) | generation` for both types, and `GenId::to_bits` MUST equal `untyped().to_bits()`. The golden value `0x0000_0003_0000_0002` and the `fixed_step_sim` golden checksum MUST be unchanged.
- **M3.11-R8** `Eq`, `Ord` and `Hash` of `GenId<T>` MUST agree with the same traits on `untyped()`, and `Ord` MUST agree with the numeric order of `to_bits`.
- **M3.11-R9** `Debug` MUST print `UntypedGenId(3v2)` and `GenId<Particle>(3v2)`, with short type names inside generics as today.
- **M3.11-R10** The sizes and alignment in §4.6 MUST be asserted at compile time.
- **M3.11-R11** Container behaviour MUST be unchanged: the same IDs and iteration order for the same operations. The existing model, contract, panic and fixed-capacity tests MUST pass with only renames, apart from removing the dense cases and moving shared-code tests (§4.1).
- **M3.11-R12** The fuzz targets MUST be renamed as in §4.2, and `genid_from_bits` MUST also check `UntypedGenId::from_bits` and the typed round trip. `cargo xtask fuzz-build` MUST pass, and the nightly job MUST list exactly the three targets.
- **M3.11-R13** Every `.cargo/mutants.toml` exclusion MUST name the new paths and functions and still match its mutant.
- **M3.11-R14** `cargo xtask bench-regress` MUST report no benchmark above the 5% gate, with the result quoted in the pull request.
- **M3.11-R15** Live documents (README, CONTRIBUTING, AGENTS, SECURITY, the issue and PR templates, the CHANGELOG preamble, the roadmap, ADR 0004's crate table, the crate docs and the `engine` crate map) MUST use the new names. `benchmarks.md` and `mutation-triage.md` MUST get the notes in §4.7. The historical records (§6) MUST NOT be edited, apart from ADR 0010's status line and those notes.
- **M3.11-R16** ADR 0012 MUST be accepted and ADR 0010's status MUST read `Superseded by ADR 0012 (date)`.
- **M3.11-R17** The CHANGELOG MUST have an entry under "Changed" marked **breaking** that lists the renames, the removal and the new type.
- **M3.11-R18** The crate MUST stay `no_std` with `alloc`, dependency-free and `#![forbid(unsafe_code)]`, and `cargo xtask no-std` and `cargo xtask wasm` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** no change. The containers issue the same IDs in the same order. R7 and R11 check it, and so does the unchanged `fixed_step_sim` checksum.
- **`unsafe`:** none. `#[repr(transparent)]` needs no `unsafe`, and nothing transmutes between the two types.
- **Concurrency:** n/a, because no shared state or atomics are added. The auto-trait tests keep checking `Send` and `Sync`.
- **Heap allocation:** unchanged. The fixed-capacity tests keep their guarantees, with the dense cases moved onto `GenIdPool<()>`.
- **`no_std` and wasm:** unchanged. The `xtask` lists are renamed (R18).
- **Frozen formats:** the layout does not change. Its normative home moves to `UntypedGenId::to_bits`, the golden values are unchanged, and the documents that name it are updated (§4.5).
- **Public API changes:** breaking. One crate, one module, four types and three fuzz targets are renamed, two types are removed and one is added. There are no consumers outside the repository, and the crates are `0.x` with `publish = false`.
- **FFI exposure:** `UntypedGenId` is the type that M8's FFI layer is expected to convert to and from. It crosses the boundary as a `to_bits` value, so this chunk fixes no C layout.
- **Performance:** expected to be zero, because the wrapper is `#[repr(transparent)]` and the methods are the same `const fn`s. This is measured with `cargo xtask bench-regress` (instruction counts, base against head, R14). The Criterion comparison is not re-run (§4.7).
- **Documentation:** new: ADR 0012 and this spec and plan. Updated: the live documents in R15. The **historical records** keep the old names by decision: the specs and plans of M0+M1, M2.x, M3.0 to M3.2 and M3.9; ADRs 0003, 0010 (apart from its status line) and 0011; the research catalog (`docs/research/data-structures.md`); the CHANGELOG entries written before this chunk; and the tables, analysis and rows of `benchmarks.md` and `mutation-triage.md` (which get the notes in §4.7). ADR 0012 has the old-to-new name table for readers of those documents.
- **Policy changes:** none. The frozen-format rule in AGENTS.md and CONTRIBUTING.md names the new method but is the same rule.

## 7. Alternatives considered

- **Keep the name `Handle`.** Rejected. It is the traditional game-development term (handle maps), but it suggests a pointer or ownership, and readers who know Bevy expect a reference-counted asset handle. The engine will likely want that kind of handle later under this name.
- **`Id<T>`.** Rejected: too generic, and it collides with `egui::Id` and others in user code.
- **`Gid<T>`.** Rejected: easily read as a Unix group ID, and the abbreviation needs explaining.
- **`Key<T>` (slotmap) or `Index` (generational-arena).** Reasonable, but `GenId` says more about what the type is.
- **`RawGenId` for the untyped type.** Rejected: "raw" in Rust usually means fewer guarantees, without saying which. "Untyped" says exactly what is missing.
- **No untyped type: keep `u64` bits for type-erased code.** Simpler, but that code would work with bare integers that have no `Debug`, no accessors and no niche. #39 decided on the split.
- **Monomorphisation as a reason for the split.** #39 lists less monomorphisation as a benefit. The slot table (`slots.rs`) is already non-generic and works on `(u32, NonZeroU32)`, so the split adds little there. Its value is the type-erased API. ADR 0012 records it that way.
- **Keep the dense allocator, renamed `IterableGenIdAllocator`.** Rejected. It has no consumer, `GenIdPool<()>` does the same, and it costs a fuzz target, a model test, benchmarks and documentation (#34).
- **Remove `GenIdAllocator` too.** Deferred until after M3, because it depends on whether M4's world wants an ID-only allocator.
- **Deprecated aliases (`pub type Handle<T> = GenId<T>` and so on).** Rejected. There are no consumers, the crates are `0.x`, and aliases would leave two names in the docs and search results.
- **`From<GenId<T>> for UntypedGenId`.** Not added. Typed-to-untyped is lossless, but #39 asks for explicit names at both ends, and `untyped()` keeps every erasure searchable. It can be added later without breaking anything.
- **Crate name `engine_id`.** Rejected: broader than the contents. A future persistent or asset ID gets its own home when it exists.
- **Put the type into the `engine` facade.** Rejected. The facade sits at the top of the dependency graph, so everything built on IDs would have to move into it too. Crate granularity is #40.
- **Rename the Gungraun bench target (`handle`) or its functions in this chunk.** Rejected. The gate would treat every benchmark as new and compare nothing, which is the measurement that shows the rename costs nothing at runtime. The function names already avoid "handle". The target is renamed later (§10).
- **Rewrite the historical documents to the new names.** Rejected. They record what was decided when they were written. ADR 0012's name table maps them.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `git grep -n DenseHandleAllocator` and `git grep -n DenseAllocIter` return only the files R1 allows |
| R2 | `cargo xtask ci`; the `engine` doctests and `libs/engine/tests/auto_traits.rs` use `engine::genid` and the prelude; `git grep -nw engine_handle` returns only the historical records, the notes in §4.7, ADR 0012, this spec and plan, the CHANGELOG entry, and the comment in the `handle` bench target |
| R3 | Review of the §4.2 table against the crate root's `pub use` list; `git grep -nwE 'Handle\|HandlePool\|HandleAllocator\|InvalidHandleBits'` outside the historical records, ADR 0012, this spec and plan, and the CHANGELOG entry |
| R4, R5 | Unit tests and doctests in `src/id.rs` (the renamed `handle.rs`, holding both types); `untyped`/`from_untyped` round trip as a proptest over all `u64` with a non-zero low half |
| R6 | `compile_fail` doctests: mixing `GenId<A>` and `GenId<B>`, calling `cast`, and passing an `UntypedGenId` where a `GenId<T>` is expected |
| R7 | The golden unit tests for both types; a proptest that `GenId::to_bits == untyped().to_bits()`; `fixed_step_sim`'s `golden_checksum` test, unchanged |
| R8 | Proptests over pairs of IDs: `cmp`, `eq` and a fixed-hasher `hash` agree between typed and untyped, and with `to_bits` |
| R9 | Unit tests for both `Debug` formats, including generic type names |
| R10 | Compile-time assertions (the build fails if one does not hold) |
| R11 | `cargo xtask test`; the diff of `tests/` shows renames only, apart from §4.1's moves |
| R12 | `cargo xtask fuzz-build`; a 60-second local `cargo xtask fuzz` run of each target, reported in the pull request; review of `nightly.yml` and `xtask` |
| R13 | `cargo xtask mutants` on `engine_genid`, whose summary shows the excluded mutants as excluded and nothing missed that was not missed before |
| R14 | `cargo xtask bench-regress` in the devcontainer, table quoted in the pull request |
| R15 | `git grep` for the old names, reviewed against the list in §6 (Documentation); the documentation review |
| R16, R17 | Review |
| R18 | `cargo xtask ci` (includes `no-std`, `wasm` and `check-unsafe`) |

## 9. Done when

- [ ] `DenseHandleAllocator` and its fuzz target, tests and benchmarks are gone, and the shared code's tests run on `GenIdPool<()>` or `GenIdAllocator`.
- [ ] The crate, module, types, fuzz targets and Criterion subjects carry the new names. Nothing outside the historical records (§6) uses the old ones, apart from the `handle` bench target (§4.7).
- [ ] `UntypedGenId` and `GenId<T>` match §4.3 and §4.4, with size assertions, golden tests, proptests and `compile_fail` doctests.
- [ ] `cargo xtask ci` passes.
- [ ] `cargo xtask bench-regress` shows every benchmark within 5%, quoted in the pull request.
- [ ] Each fuzz target ran locally for 60 seconds without a finding.
- [ ] `cargo xtask mutants` on `engine_genid` shows the exclusions still applying.
- [ ] ADR 0012 is accepted and ADR 0010 is marked superseded. ADR 0004's table, the live documents and the notes in `benchmarks.md` and `mutation-triage.md` are updated, and the CHANGELOG has the breaking entry.
- [ ] The roadmap row reads `Done (PR #n)`, this spec reads `Implemented`, and the plan reads `Historical`.
- [ ] The pull request closes #34 and #39 and lists the eight reviews.

## 10. Open questions

None. Deferred, with owners:

- **Untyped operations on containers** (`contains`, `get` by `UntypedGenId`): added by M3.6 or the M4 registry chunk, whichever first needs them.
- **Crate granularity**, including whether `engine_genid` later merges into a core crate: #40, decided before M4.
- **32-bit and configurable layouts:** #39's "Future" section, when M8, GPU-driven rendering or networking needs one.
- **Whether `GenIdAllocator` stays:** after M3, with M4's world design.
- **Renaming the `handle` bench target to `genid`:** in a later pull request that changes nothing under `libs/`, so the gate has nothing to lose by treating the benchmarks as new. It can go with any small chore pull request.

## 11. Prior art and sources

Designs studied, not code copied ([ADR 0002](../../../adr/0002-originality-and-dependency-licensing.md)):

- slotmap's `KeyData`, wrapped by typed keys from `new_key_type!`: an untyped core with typed newtypes over it. <https://docs.rs/slotmap>
- Bevy's `UntypedHandle` and `Handle<A>` for assets: the same pairing, for an owning handle, and the source of the name clash this chunk avoids. <https://docs.rs/bevy_asset>
- generational-arena's `Index`, and thunderdome's `Index`, as other names for the same concept. <https://docs.rs/generational-arena>, <https://docs.rs/thunderdome>
- The Rust API Guidelines on casing (C-CASE): acronyms and contractions are one word in UpperCamelCase, as in `Uuid`, hence `GenId`. <https://rust-lang.github.io/api-guidelines/naming.html>
- The niche optimisation for `Option<NonZeroU32>` and for `#[repr(transparent)]` wrappers: the Rust Reference, "Type layout". <https://doc.rust-lang.org/reference/type-layout.html>
