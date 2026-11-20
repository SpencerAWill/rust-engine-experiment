# M3.4 log: Archetype prototype

- **Status:** Living (2026-11-19)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-19 decision Next chunk is M3.4: M3.3's spec names no Followed by chunk, and M3.4 is the first Planned M3 row whose dependencies (M3.2) are Done. No issue (tracking issue #66 is not a Project item for this row), so no Project status change.
- 2026-11-19 decision (maintainer) Bundle spawn: `World::spawn_with(bundle)` (tuples of 1 to 4, nested beyond) places an entity straight into its final table; setup and the respawns in `entity_churn` and `agents` use it. Single-component `insert`/`remove` stay. Rejected: one insert at a time (4 to 5 table moves per respawn inside the measured step), and both as two backends.
- 2026-11-19 decision (maintainer) Archetype transitions use an edge cache: per archetype, add and remove edges as `Vec<Option<ArchetypeId>>` indexed by `ComponentId`, filled on first use. Rejected: a `BTreeMap<Vec<ComponentId>, _>` lookup per move, which allocates and compares on every insert.
- 2026-11-19 decision (maintainer) One backend, `archetype`: one contiguous table per archetype, all seven workloads. Rejected: a chunked variant (16 KiB chunks) and a cached-query variant; M3.10's bevy_ecs and hecs baselines are the outside archetype comparisons.
- 2026-11-19 surprise The bundle question offered "tuples up to 4, nested beyond", but nesting overlaps the flat impl while any `'static` type is a component (`(A, (B, C))` is also a two-element bundle). Bundles are flat tuples of 1 to 8 instead (`agents` needs 5); spec §4.5.
- 2026-11-19 decision Main session's calls in the draft: no `WorldBuilder` (nothing to declare); a query with no required type is supported (every live entity has a row); `for_each` only on `QueryMut` (an external mutable iterator is possible here but is M4's API question); columns found by binary search on the sorted component list; a bundle cache keyed by the tuple's `TypeId`; empty archetypes never removed.
- 2026-11-19 done Spec drafted (`Draft`); roadmap row `Drafting`.
