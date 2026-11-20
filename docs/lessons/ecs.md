---
applies_to:
  - "spikes/ecs_storage/**"
---
# Lessons: ECS prototypes and queries

- Status: Living (2026-11-19)

Add `libs/engine/src/ecs/**` to `applies_to` when M4 creates that module.

- **ecs-1.** A test that checks which path a query takes builds the query through the same function the measured `step` uses, never a copy of its type. *Why:* M3.3's path tests wrote out their own read-only copies of each step query, so changing a step's `&mut Lifetime` to `Option<&mut Lifetime>` sent the grouped backend down the view path while conformance and the path test both still passed (code review). *From:* M3.3 · *Confirmed:* 2026-11-19
- **ecs-2.** A spec or plan that defers per-query state ("it is only X per query") counts the allocations and lookups of the code as built before close-out, and records that cost as a caveat for whoever reads the numbers. *Why:* M3.3's spec deferred cached query state for "a few `BTreeMap` lookups", but each query run made about five heap allocations inside every measured `step` (performance review). *From:* M3.3 · *Confirmed:* 2026-11-19
- **ecs-3.** In a sealed query trait, the item GAT (`type Item<'w>`) goes on the private seal trait, not the public one; the public trait names it through its supertrait. *Why:* on rustc 1.100, seal methods that named `<Self as QueryParam>::Item<'a>` under `where Self: QueryParam` made the other associated types fail to normalize in impls (E0053, E0599), in M3.3 step 3. *Retire when:* the compiler normalizes that projection. *From:* M3.3 · *Confirmed:* 2026-11-19
