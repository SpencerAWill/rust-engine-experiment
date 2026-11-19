# M3.3 log: Sparse-set prototype

- **Status:** Living (2026-11-19)

- 2026-11-19 surprise The branch `m3.3-sparse-set` held a spec and plan from 2026-11-14, ten merges behind `main` (M3.11 to M3.18), in the old `docs/design/m3-ecs-storage/` folder. Stale: `Handle`/`engine_handle` names, the old review list, and a planned "M3.11 memory metric" row whose ID M3.11 (genid) has since taken.
- 2026-11-19 decision Merged `main` into the branch instead of rebasing (a rebase needs a force push, which the push guard blocks; the squash merge hides the merge commit) and moved the documents to `docs/chunks/m3.3-sparse-set/`.
- 2026-11-19 decision Keep the spec, with targeted updates (names, links, review wording); it goes through the grill and approval again because it was never `Accepted`. The old plan is rewritten from scratch at stage 3: its stops, review list and M3.11 step belong to the old process. It is kept in the folder until then (history: 8e093c9).
- 2026-11-19 decision (maintainer) Memory metric: point at issue #41 instead of adding a roadmap row. No new row and no new M3.8 dependency; paging and heap numbers wait for #41; M3.8's row notes that the decision rests on time alone unless #41 lands first.
- 2026-11-19 done Grill cold read (`spec-reader`, registered agent): two open points, both from `GenIdAllocator` not counting or listing live IDs: a query with no required type, and how `entity_churn`/`agents` know the live count.
- 2026-11-19 decision Main session's calls from the grill: `World::len`/`is_empty` from a live counter (used by the respawn loops, checked by the model test); `spike_ecs_sparse_set` added to coverage's `SPIKE_PACKAGES`.
