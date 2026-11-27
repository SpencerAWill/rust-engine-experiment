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
