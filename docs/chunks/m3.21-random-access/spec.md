# M3.21: random_access workload for the ECS storage harness

- **Status:** Implemented (PR #134, 2026-11-24); amended 2026-11-24 in step 3 (§4.6: `Lookup` is `Copy` only where it owns no cache) and in planning (§4.3: hecs's view type)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10 · **Decides:** no ADR
- **Issue:** #106
- **Estimated size:** about 1,250 changed lines, excluding tests, lockfiles and generated files (harness module about 120, `vec_aos` about 90, six ECS backends about 110 each, the prototypes' `Lookup` about 60 each, bench registration about 30, documents about 40)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds an eighth workload, `random_access`, to the ECS storage spike: every entity holds the handle of a target, and each step reads the target's `Position` through that handle to set the entity's own `Velocity`. Real frames do this all the time (a seeker reads its target, a projectile its owner), and none of the seven M3.2 workloads measures the lookup from handle to storage location. That lookup costs an archetype a location table and a row index, and a sparse set one indirection, so the storage decision (M3.8, #102) needs it measured. At the end the workload passes conformance against `vec_aos` on `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs`, both spike benches list it, and an informal Criterion run is in the pull request and in M3.8's issue.

## 2. Goals and non-goals

Goals:

- Rules for `random_access` (§4.2) precise enough that any backend that follows them reproduces the reference checksum bit for bit, in the M3.2 style: components, a spawn function, update functions that fix the floating-point order, and a per-entity hash, all in `spike_ecs_harness`.
- Implementations on `vec_aos` (the reference) and the six backends above, each passing `check_conformance` against `vec_aos`.
- Stale handles: targets despawn and their index is reused, so every backend's generation check runs inside the measured step.
- Registration in `bench_compare` and `bench_cache` with `Spec::SIZES`.

Non-goals:

- No variant backends (`vec_soa`, `sparse_set_grouped`, `bevy_ecs_sparse`, the byte-column backends): the tracking issue (#105) keeps each workload to the seven listed backends.
- No split borrow. No prototype can read one entity's component by handle while a mutable query runs (`query_mut` borrows the whole `World`), and this chunk does not add a way to; the step is two passes on every backend instead (§4.3) *(maintainer, from the brainstorm)*. The only new prototype API is the read-only `Lookup` of §4.6 *(maintainer, from the grill)*.
- No formal results. The informal run is the Criterion bench on the Mac under the benchmark run protocol; the cache bench is registered and built, not run (as M3.10). M3.8 measures both.

## 3. Guide-level explanation

A backend implements `random_access` like any other workload ([`spikes/ecs_storage/README.md`](../../../spikes/ecs_storage/README.md), "Adding a backend"). The new part is the two-pass step, here in outline for a prototype `World`:

```rust,ignore
// Pass A: read-only. One lookup by handle per entity.
self.scratch.clear();
let positions = self.world.lookup::<Position>(); // the type is resolved once (§4.6)
self.world.query::<(&Position, &Target<Entity>)>().for_each(|(p, t)| {
    let (q, retarget) = match positions.get(t.entity) {
        Some(q) => (*q, None),
        None => {
            let h = self.roster[t.slot as usize];
            (*positions.get(h).unwrap(), Some(h))
        }
    };
    self.scratch.push((rules::steer(p, &q), retarget));
});
// Pass B: mutable, same entities in the same order.
let mut i = 0;
// The prototypes' queries take tuples of up to four elements, so the six are nested.
self.world.query_mut::<(Entity, (&Slot, &mut Position, &mut Velocity), (&mut Target<Entity>, &mut Lifetime))>()
    .for_each(|(e, (slot, p, v), (t, life))| {
        let (velocity, retarget) = self.scratch[i];
        i += 1;
        // write velocity and retarget, integrate, age, note expiry
    });
// Then despawn the expired and respawn into their slots in ascending slot order.
```

## 4. Reference-level design

### 4.1 Layout

- `spikes/ecs_storage/harness/src/random_access.rs`: the module of §4.2, `pub mod random_access` in `lib.rs`.
- One `random_access.rs` per backend crate (`vec`, `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy`, `hecs`), with the type `<Prefix>RandomAccess` (`VecRandomAccess`, `ArchetypeRandomAccess`, …) and `BACKEND` the crate's usual name.
- Conformance tests in each backend's existing tests file; one line per backend in each bench crate.

### 4.2 The workload

Sizes 10,000, 100,000 and 1,000,000 *(my call: an entity is about 48 bytes in a table, so 100,000 entities fit the 16 MB L2 of the maintainer's M4 Pro and only 1,000,000 forces lookups to miss it)*.

The common rules of M3.2 §4.4 apply (initial values from `Prng::from_seed(params.seed).stream(i)` in the listed order, component types defined in the harness, no transcendental functions, hashes as `Fnv1a` call sequences).

- **Slots and the roster.** The workload has exactly `count` slots, `0..count`, and exactly one live entity per slot after `setup` and after every step. The backend keeps a roster, `Vec<E>` of length `count`, where `roster[s]` is the handle of the entity in slot `s` (`E` is the backend's entity handle; for `vec_aos`, §4.4). The roster is the game's own data: reading it is not the measured lookup.
- **Components.** `SpawnId(u64)`; `Slot(u32)`, the entity's own slot; `Position(Vec2)`; `Velocity(Vec2)`; `Lifetime(u32)`, ticks left to live; and `Target<E> { slot: u32, entity: E }`, the slot of the entity's target and the handle it last resolved there. `Target` is generic over the handle type, so every backend stores the harness's type. Every entity has all six.
- **Spawn.** `spawn(prng: &Prng, i: u64, count: usize) -> (SpawnId, Position, Velocity, Lifetime, u32)` returns entity `i`'s `SpawnId(i)`; position x in `0.0_f32..320.0`, then y in `0.0_f32..180.0`; `Velocity(Vec2::ZERO)` (no draw); lifetime one draw in `60_u32..=600`; then the target slot, one draw in `0_u32..count as u32` (the target may be the entity's own slot) *(lifetimes my call: about 0.3% of entities die per tick, so lookups dominate and `entity_churn` keeps measuring churn)*. The backend builds `Slot` and `Target` from the returned slot and its roster.
- **Setup.** Spawn entities `0..count` into slots `0..count` (entity `i` in slot `i`), fill the roster, then set every `Target::entity` to `roster[target.slot]`. Setup MAY spawn with a placeholder handle and patch it; setup is not measured. The next spawn index is `count`.
- **Step, pass A (read).** For every entity, with own position `p` and target `t`: if the backend's lookup of `Position` by `t.entity` succeeds, `q` is that position; otherwise `t.entity` is stale, the entity is retargeted to `h = roster[t.slot]`, and `q` is `h`'s position (`h` is always live). The new velocity is `steer(p, q)`: `(q.0 - p.0) * GAIN` with `GAIN = 0.5`, as `Vec2` subtraction then scalar multiplication *(my call)*. Pass A MUST NOT change any component: every lookup in a step sees the positions as they were at the start of the step, so the result does not depend on iteration order.
- **Step, pass B (write).** For every entity: write the new velocity, and the new handle if it was retargeted; then `integrate`: `position.0 = position.0 + velocity.0 * DT` (scalar multiplication, then addition) and `lifetime.0 = lifetime.0 - 1`. An entity whose lifetime is now 0 expires.
- **Step, respawn.** Despawn every expired entity. Then, for each freed slot in ascending slot order, spawn the next spawn index into it with `spawn`, giving it `Target { slot: t, entity: roster[t] }` as the roster stands just before this spawn, then set `roster[slot]` to its handle. So the bundle is complete before the spawn and no handle is patched afterwards. When `t` is the entity's own slot or a slot refilled later in the same step, that handle is the dead occupant's: the next step's pass A finds it stale and retargets to `roster[t]`, the entity it would have resolved to now, so the checksum is the same *(main session's call, from the cold read)*.
- **Hash.** `u64(spawn id)`, `u32(slot)`, `vec2(position)`, `vec2(velocity)`, `u32(lifetime)`, `u32(target slot)`. The handle is not hashed: it differs between backends. A backend that reads a dead entity's leftover position, or the wrong entity, changes the next velocity and so the checksum.

The harness module exports `RandomAccess` (the `Spec` marker), the component types, `spawn`, `steer`, `integrate`, `is_expired` and `hash`, each with the floating-point order above, so backends call them and do not write their own.

### 4.3 The two passes

*(maintainer, from the brainstorm)* Every backend, the baselines included, runs the step as two passes with a scratch buffer:

- **Pass A** is a read-only query over the entities' `Position` and `Target`, plus one lookup of `Position` by handle per entity (two for a stale handle). It pushes `(Vec2, Option<E>)` (the new velocity and the retargeted handle, if any) to a scratch `Vec` the workload owns, cleared each step and reserved for `count` entries in `setup`.
- **Pass B** is a mutable query over `Slot`, `Position`, `Velocity`, `Target` and `Lifetime` (and the entity handle where the backend needs it to despawn) that visits the same entities in the same order as pass A and takes the `i`-th scratch entry for the `i`-th entity. Every entity has the same six components and both passes match the same storage, so each backend's iteration order is the same for both queries; conformance catches a backend where it is not, since misassigned velocities change the checksum. Expired entities are collected (handle and slot) into a second reused `Vec` during pass B and despawned after it.
- **The lookup** resolves the component type once per pass, not per entity, so that every row measures the handle-to-location walk and not a type lookup *(maintainer, from the grill)*: `World::lookup::<Position>()` (§4.6) on the four prototypes, created once per pass A; `World::view::<&Position>()` once per pass A on `hecs` (which stores the harness types unwrapped) (its documented random-access path), then `ViewBorrow::get`; and on `bevy_ecs` a `QueryState<&Table<Position>>` held by the workload, created in `setup`, with `get_manual` (no archetype is created after `setup`, so its cache stays current; `update_archetypes` MAY be called at the start of pass A). `World::get` resolves the type by a map lookup on every call on the prototypes and on bevy, so no backend uses it in the step.
- **`hybrid` storage:** all six components are table types *(maintainer, from the grill)*: nothing is added or removed, which is what `hybrid`'s sparse storage is for, so its `random_access` row walks the location table as `archetype`'s does and no sparse set is probed. A sparse-`Position` variant is out of scope (#105).
- **Respawn order.** Slots freed in a step MUST be refilled in ascending slot order, so that spawn indices match `vec_aos`'s. A backend whose pass B does not visit slots in order sorts the collected expiries by slot (about 0.3% of `count` entries per step).

Two passes are the same algorithm on every backend, so the only cost that differs between rows is the storage: the lookup, the two query passes and the structural changes. The scratch buffer adds one sequential write and read of 16 to 24 bytes per entity to every row.

### 4.4 The reference

`vec_aos` keeps one `Vec` of structs indexed by slot, so the slot is the storage location and the roster is the `Vec` itself. Its handle type is the spawn id (`E = u64`): a lookup of `Target { slot, entity }` succeeds when `entities[slot].spawn_id == entity`, and is stale otherwise, the same check a generational index makes. Respawn overwrites the expired slot in place, in ascending slot order. It runs the two passes of §4.3 too, so that its reads see the start-of-step positions.

### 4.5 Benchmarks and documents

- `bench_compare/benches/compare.rs`: one `bench::<…>` line per backend. `bench_cache/benches/cache.rs`: one `cache_bench!` per backend with `[10_000, 100_000, 1_000_000]` and a `random_access` group.
- `spikes/ecs_storage/README.md` and `spikes/README.md`: eight workloads, and a link to §4.2 here as the rules of `random_access` (M3.2 §4.4 stays the source for the other seven). The harness crate docs say the same.
- The informal run: `cargo xtask spike-bench ecs` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal. Its `random_access` rows go in `log.md`, the pull request, and a comment on #102 (M3.8's issue, where the inputs to the decision are collected), with any departure from expectation said as measured or unmeasured *(my call: since M3.20 M3.8 reads its inputs from its issue, not a roadmap row)*.

### 4.6 `Lookup`: a read-only component view

*(maintainer, from the grill)* Each of the four prototype crates (`archetype`, `sparse_set`, `dense`, `hybrid`) gains:

```rust,ignore
impl World {
    /// A read-only view of every entity's `T`, with `T` resolved once.
    pub fn lookup<T: 'static>(&self) -> Lookup<'_, T>; // `T: Component` on `hybrid`
}

/// Reads one component type by entity handle.
pub struct Lookup<'w, T> { /* borrows the World */ }

impl<'w, T: 'static> Lookup<'w, T> {
    /// `entity`'s `T`, or `None` if the entity is despawned (including a stale handle whose
    /// index is reused) or has no `T`.
    pub fn get(&self, entity: Entity) -> Option<&'w T>;
}
```

- `World::lookup` resolves `T` (its `ComponentId`, and the column or storage it names) once. `Lookup::get` MUST NOT look up the type again: it checks the handle's liveness, then reaches the value through the storage's own path (the entity's location and the column of its archetype for `archetype` and for `hybrid` table types; the sparse set for `sparse_set` and for `hybrid` sparse types; the column slot for `dense`). A `Lookup` of a type with no storage yet returns `None` for every entity.
- `archetype` and `hybrid` MAY cache, when the `Lookup` is created, the column index of `T` in each archetype (indexed by archetype), so that `get` does no search; no archetype is created while a `Lookup` lives, since it borrows the `World`.
- `Lookup::get` MUST agree with `World::get` for every entity: live with `T`, live without `T`, despawned, and stale with its index reused. Each prototype's tests check the four cases.
- The name is `Lookup`, not `View`, because `sparse_set` already calls its non-owning query path "views" *(main session's call)*. `Lookup` is `Clone` and `Debug`; it is `Copy` only where it owns no cache (amended 2026-11-24 in step 3: `archetype`'s per-archetype column cache is a `Vec`).

## 5. Requirements

- **M3.21-R1** `spike_ecs_harness::random_access` MUST define `RandomAccess` with `NAME = "random_access"` and `SIZES = [10_000, 100_000, 1_000_000]`, the six components, `spawn`, `steer`, `integrate`, `is_expired` and `hash` as §4.2 gives them.
- **M3.21-R2** `vec_aos` MUST implement `random_access` as §4.4 gives it.
- **M3.21-R3** `archetype`, `sparse_set`, `dense`, `hybrid`, `bevy_ecs` and `hecs` MUST each implement `random_access` with the two passes, the lookup, the `hybrid` storage and the respawn order of §4.3, and MUST pass `check_conformance` against `vec_aos` at `count` 1,000, seed 42, for 700 ticks (past every initial entity's lifetime).
- **M3.21-R4** Within the conformance run, stale handles MUST occur: a `vec_aos` test MUST show that at least one retarget happens in the 700 ticks, and one prototype's test MUST show a stale handle whose index has been reused by a live entity (a generation mismatch, not only an absent index).
- **M3.21-R5** Pass A MUST NOT change any component, and the workload's own code MUST NOT allocate in `step`: the scratch and expiry `Vec`s are reserved in `setup` and reused. The backends' own allocations (the prototypes' per-query-run allocations recorded in their logs, storage growth) are allowed and not changed *(main session's call, from the cold read)*.
- **M3.21-R6** Both spike benches MUST register `random_access` for all seven backends with `Spec::SIZES`; `cargo xtask spike-bench ecs` lists the rows.
- **M3.21-R7** The documents of §4.5 MUST be updated.
- **M3.21-R8** The pull request MUST show one informal `cargo xtask spike-bench ecs` run under the benchmark run protocol, labelled informal, with the `random_access` rows; the same rows MUST be in `log.md` and in a comment on #102.
- **M3.21-R9** `cargo xtask ci` MUST pass.
- **M3.21-R10** Each of the four prototypes MUST provide `World::lookup` and `Lookup` as §4.6 gives them, with no type lookup in `Lookup::get`, and a test that `Lookup::get` agrees with `World::get` on a live entity with `T`, a live entity without `T`, a despawned entity and a stale handle whose index is reused.

## 6. Cross-cutting concerns

- **Determinism:** the rules use no transcendental function and fix every floating-point order in harness functions; respawn order is by slot; pass A reads only start-of-step state. Conformance proves it per backend.
- **`unsafe`:** none; every crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because the step is single-threaded.
- **Heap allocation:** none per step after `setup` beyond storage growth (R5); the prototypes' per-query-run allocations recorded in earlier logs apply to both passes.
- **`no_std` and wasm:** n/a, because spike crates are outside the `no_std` and wasm builds.
- **Frozen formats:** none touched: the workload draws from `Prng::stream` and `range`, whose streams it does not change.
- **Public API changes:** new public items in the spike crates only (the harness module, the backends' workload types, and `World::lookup` and `Lookup` in the four prototypes); nothing in `engine`.
- **FFI exposure:** n/a, because spikes have no FFI.
- **Performance:** one informal Criterion run (R8); M3.8 records results under the protocol. The 1,000,000 size adds setup time and memory to `spike-bench` (about 50 MB per backend instance) and to the cache bench under Callgrind.
- **Documentation:** §4.5; every new public item documented.
- **Policy changes:** none.

## 7. Alternatives considered

- **A split-borrow API on the prototypes** (read `Position` by handle while mutably iterating `Velocity`, as bevy's two queries in one system do), single pass everywhere. Rejected by the maintainer: four `World`s gain an API, beyond the issue's size, and the decision is about storage.
- **Prototypes in two passes, baselines in their idiomatic single pass.** Rejected by the maintainer: the rows would not run the same algorithm.
- **`World::get` per lookup on every backend**, or as it is on the prototypes with the baselines on their cached paths. Rejected by the maintainer: `World::get` resolves the type by a map lookup on every call (a `BTreeMap` in the prototypes, a hash map in bevy), which is not a storage cost, and the mixed form would charge it to the prototypes only.
- **A fresh random draw for a retarget** (a slot from a stream keyed by spawn id and tick). Rejected by the maintainer: the slot's new occupant needs no draws and is as deterministic.
- **Pass B writing back through `World::get_mut` by handle** instead of the same-order query. Rejected *(my call)*: it adds a second lookup per entity, of the entity's own row, which counts the measured cost twice.
- **Sizes 1,000 to 100,000 as `entity_churn`.** Rejected *(my call)*: at 100,000 the whole working set fits in L2 on the maintainer's machine, so the random reads would never miss it.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | review against §4.2; a spawn test: `spawn` for index `i` equals direct draws from `prng.stream(i)` in the listed order |
| R2 | review against §4.4; R3's tests use it as the reference |
| R3 | `check_conformance` tests in each backend crate (CI) |
| R4 | the retarget-count test in `spike_ecs_vec` and the reused-index test in one prototype crate |
| R5 | review of each `step` |
| R6 | the bench crates build in `ci`; the cache bench's size assertion; `spike-bench` output in the pull request |
| R7 | `review-docs` |
| R8 | the pull request, `log.md` and the comment on #102 |
| R9 | `cargo xtask ci` output |
| R10 | the four agreement tests in each prototype crate (CI); review of each `Lookup::get` for a type lookup |

## 9. Done when

- [x] The harness module, the four prototypes' `Lookup` and the seven implementations exist, and every conformance and agreement test passes. *All pass in the full `ci` on 517ed8b4ffb9e120085cb3a4e3a164f7bb28313f, with the write-back tests added in review.*
- [x] Both spike benches register `random_access` for the seven backends. *21 `random_access` ids in the informal run; the cache bench builds.*
- [x] The documents of §4.5 are updated. *`check-docs` 0 problems; `review-docs`.*
- [x] One informal run is in the pull request, `log.md` and on #102. *PR #134, `log.md`, and [the #102 comment](https://github.com/SpencerAWill/rust-engine-experiment/issues/102#issuecomment-7141297711).*
- [x] `cargo xtask ci` passes. *On 517ed8b4ffb9e120085cb3a4e3a164f7bb28313f; CI on the PR pending.*

## 10. Open questions

None.

## 11. Prior art and sources

- The M3.2 spec §4.4 (workload style) and the M3.10 spec §4.4 (baseline idioms, deferred changes applied after the query).
- bevy's `Query::get` beside a second query in one system and hecs's `World::get` with dynamic borrow checks, read as designs only ([ADR 0002](../../adr/0002-originality-and-dependency-licensing.md)).
