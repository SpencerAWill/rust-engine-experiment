# M3.24 log: hierarchy workload for the ECS storage harness

- **Status:** Living (2026-11-27).

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-27 done Picked by `/chunk next`: M3.24 (#110), order 14, the lowest unblocked Scheduled chunk in M3 (M3.25 order 15, M3.7 order 20 also eligible; M3.8 blocked by #101, #109, #110); no scheduled chores; nothing in flight. Branch `m3.24-hierarchy-workload` pushed.
- 2026-11-27 surprise Every prototype already has what a level-by-level propagation needs: `With`/`Without` filters, the read-only `Lookup` of M3.21 and `query`/`query_mut`. None has a mutable by-handle path other than `World::get_mut`, which resolves the type on every call (rejected for the step by M3.21 §4.3).
- 2026-11-27 dead-end Forest generator "each entity joins a random ancestor on the current pre-order path or starts a root" (scratchpad simulation, Python): 37% of entities are roots, trees average 2.7 entities and depth 7 or 8 is under 0.1% at every size. Too shallow for "depths 1 to 8".
- 2026-11-27 done Forest generator "each root draws its tree depth D in 1..=8; each node above level D has 1 or 2 children; pre-order spawn" (same simulation): every level holds 5% to 21% of entities at 1,000 and 6% to 17% at 100,000, trees average 12 to 16 entities (max 73 at 1,000, 138 at 100,000). Child counts 1..=3 put 50% of entities at levels 7 and 8; 0..=3 is close to 1..=2 with larger trees.
- 2026-11-27 decision (maintainer) Propagation by level passes: each entity holds `Parent(handle)` (roots none) and one depth-marker type of eight; per depth from 2 to 8, a read query looks up the parent's world transform through `Lookup` into a scratch buffer and a write query stores the results in the same order (the `random_access` two-pass shape). Rejected: a children walk from the roots (needs a mutable by-handle path the prototypes lack), an upward walk per entity (not parent first, O(n x depth)).
- 2026-11-27 decision (maintainer) The engine's `Transform2d` (local) and `WorldTransform2d` (world), composed with `WorldTransform2d::mul_transform`, which calls `scalar::sin_cos` (same-build determinism, ADR 0003). Rejected: trig-free harness types.
- 2026-11-27 decision (maintainer) Forest: each root draws its tree depth D in 1..=8, each node above level D has 1 or 2 children, spawned in pre-order. Rejected: 1 to 3 children, a shuffled spawn order.
- 2026-11-27 decision (maintainer) Sizes 1,000, 10,000 and 100,000, as `add_remove` and `wide_entities`.
- 2026-11-27 decision Main session's calls in the draft: the local update rotates every local by `SPIN` with a hand wrap (no trig); roots' world is `local.to_world(DVec2::ZERO)`; harness newtypes `LocalTransform`, `GlobalTransform` (not `World*`, which clashes with the backends' `World`), `Parent<E>`, `SpawnId`, `Depth1` to `Depth8`; the hash takes the parent's spawn id and the f64 translation by `to_bits`; `vec_aos` propagates in one pass in spawn order (pre-order puts each parent first); every type a table type on `hybrid`; no structural change in the step.
- 2026-11-27 done Spec drafted (`spec.md`, Draft).
