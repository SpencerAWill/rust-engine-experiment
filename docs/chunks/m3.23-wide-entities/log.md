# M3.23 log: wide_entities workload for the ECS storage harness

- **Status:** Living (2026-11-26).

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-26 done Picked by `/chunk next`: M3.23 (#108), order 13, the lowest unblocked Scheduled chunk in M3; no scheduled chores; nothing in flight. Branch `m3.23-wide-entities` pushed.
- 2026-11-26 surprise The `archetype` and `hybrid` prototypes implement `Bundle` for 1 to 8 types and their (and `sparse_set`'s, `dense`'s) queries for 1 to 4 parameters. A 24-type entity spawned with `spawn_with` of 8 plus 16 `insert`s leaves 16 empty intermediate archetypes that every query run then matches and walks.
- 2026-11-26 decision (maintainer) The structural change is a 25th type added to every entity, then removed in a separate pass, every tick (24 -> 25 -> 24 types), the shape of `add_remove`, so the two rows compare directly.
- 2026-11-26 decision (maintainer) `hybrid` stores `Extra` (the 25th type) sparse, as `add_remove`'s `B`; the 24 wide types are table types.
- 2026-11-26 decision (maintainer) The 24 types are `Wk(pub [f32; N])` with N cycling 1, 2, 3, 4, 8, 16: four of each size, 544 bytes per entity.
- 2026-11-26 decision (maintainer) Sizes 1,000, 10,000 and 100,000, as `add_remove`.
- 2026-11-26 decision (maintainer) `archetype` and `hybrid` `Bundle` impls extended to 1 to 24 types; both spawn each entity with one 24-tuple; a test pins the archetype count after setup and after a step.
- 2026-11-26 decision Main session's calls in the draft: update pass `(&mut W0, &mut W7, &mut W14, &mut W21)` with the amended `frag_iter` rotation per element; `Extra(w0.0[0])`; hash over the 136 values then `Extra` as `add_remove`'s `B`; `bevy_ecs` spawns from a nested tuple, `hecs` from an `EntityBuilder`; M3.26's per-step insert test and `vec_aos` hook on every backend.
- 2026-11-26 done Spec drafted (`spec.md`, Draft).
- 2026-11-26 done Grill stub (scratchpad crate, own `[workspace]`, toolchain 1.100.0, path deps on `engine`, `spike_ecs_harness`, `spike_ecs_bevy`; `bevy_ecs` 0.20.1, `hecs` 0.11.2): the 24 types from one macro, `size_of::<Wide>()` 544; a nested `(12-tuple, 12-tuple)` bevy bundle of `Table<Wk>` spawns into one new archetype; a 24-value `hecs::EntityBuilder` gives 2 archetypes (root and the 24-type one); a trait impl for a 24-tuple compiles.
- 2026-11-26 done ecs-13 simulation (same stub, the §4.2 spawn, update and hash, `count` 1,000, seed 42): over 1,000 ticks no checksum equals an earlier tick's (tick 0 included) and every entity changes at every tick (0 unchanged entity-ticks).
- 2026-11-26 done Grill cold read (`spec-reader`, registered agent): 2 questions. Q1 (the ecs-13 simulation) answered by the log entry above, figures now in §4.2. Q2 (the update pass is in `wide_entities` and not in `add_remove`, so the row difference is not the move cost alone, by a different amount per backend) goes to the maintainer.
- 2026-11-26 decision Main session's calls from the cold read: `checksum` reads by any read path (nested tuple queries or `get`); a `bevy_ecs` test that `Sparse<Extra>` is never registered and the 25-type archetype exists (ecs-11); `hecs`'s `EntityBuilder` fresh or reused, the builder's choice.
- 2026-11-26 decision (maintainer) The update pass's share of the `wide_entities` rows is a caveat in the #102 comment only (expected scale, unmeasured); no moves-only row and no separate timing.
- 2026-11-26 decision (maintainer) Spec approved; status Accepted.
- 2026-11-26 done Lessons (`lessons-researcher`, registered agent): ecs-1, 2, 4, 5, 6, 7, 8, 9, 11, 13; specs-1, 2, 3, 4, 6; docs-1, 3, 4. ecs-4, 5, 8 not applicable; the rest followed (plan §4). Plan written and accepted: 13 steps; agents dispatched by registered name.
- 2026-11-26 done Step 1: harness wide_entities module (macro for W0..W23/Wide/spawn/hash), 5 tests green; fails-first was E0432 (module items missing). ci --quick rc 0.
- 2026-11-26 done Step 2: impl_bundle! extended to 24 on archetype and hybrid (macro unchanged; docs say 1 to 24, plus archetype World::spawn_with doc); spawn_with_takes_24_types in each world.rs tests, fails-first E0277. ci --quick rc 0.
