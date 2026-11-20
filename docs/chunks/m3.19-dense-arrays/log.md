# M3.19 log: Dense per-entity component arrays

- **Status:** Living (2026-11-20)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-20 decision (maintainer) M3.19 before M3.5: both eligible (M3.5 is first in table order). M3.19 has a ready issue (#79) and its measurements give M3.5 a third storage kind to choose per component; M3.5 is a title only.
- 2026-11-20 done Branch `m3.19-dense-arrays`; roadmap row `Drafting`; #79's Project Status Ready to Scheduled (tracking issue #78).
- 2026-11-20 decision (maintainer) Column layout: `Vec<Option<T>>` for the values plus a presence `HierarchicalBitSet` per type that drives `Join`. Rejected: a `T`-sized column with filler values (needs a `Default` bound the harness types lack), and `Option<T>` alone with scans (does not measure `Join`). The wider `Option<T>` and the per-fetch check are costs for M3.7 to measure.
- 2026-11-20 decision (maintainer) Spawning: `spawn` then one `insert` per component, as M3.3; no `spawn_with` bundles. An insert moves nothing here, so a bundle would save only the per-type lookups.
- 2026-11-20 decision Main session's calls in the draft: per-index `entities: Vec<Entity>` (the lean allocator cannot map an index back to an ID) and an `alive` `HierarchicalBitSet` that drives queries with no required type (supported, as M3.4); despawn visits every column (no per-entity mask); `ValuesMut` hands queries `get` only (ecs-5); a required fetch panics naming D1 rather than skipping; `Query` resolves its sets when built so `iter(&self)` can borrow them, and there is no by-value `IntoIterator for Query` (self-referential without `unsafe`).
- 2026-11-20 done Spec drafted (`Draft`).
