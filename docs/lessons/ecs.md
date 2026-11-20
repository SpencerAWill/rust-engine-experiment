---
applies_to:
  - "spikes/ecs_storage/**"
---
# Lessons: ECS prototypes and queries

- Status: Living (2026-11-20)

Add `libs/engine/src/ecs/**` to `applies_to` when M4 creates that module.

- **ecs-1.** A test that checks which path a query takes builds the query through the same function the measured `step` uses, never a copy of its type. *Why:* M3.3's path tests wrote out their own read-only copies of each step query, so changing a step's `&mut Lifetime` to `Option<&mut Lifetime>` sent the grouped backend down the view path while conformance and the path test both still passed (code review). *From:* M3.3 · *Confirmed:* 2026-11-19
- **ecs-2.** A spec or plan that defers per-query state ("it is only X per query") counts the allocations and lookups of the code as built before close-out, and records that cost as a caveat for whoever reads the numbers. Count allocations from outside the crate (a scratchpad binary that depends on it, or `dhat`): a counting `#[global_allocator]` needs `unsafe`, which spike crates forbid. *Why:* M3.3's spec deferred cached query state for "a few `BTreeMap` lookups", but each query run made about five heap allocations inside every measured `step` (performance review); in M3.4 a builder lifted `forbid(unsafe_code)` in the working tree to count them. *From:* M3.3, M3.4 · *Confirmed:* 2026-11-20
- **ecs-3.** In a sealed query trait, the item GAT (`type Item<'w>`) goes on the private seal trait, not the public one; the public trait names it through its supertrait. *Why:* on rustc 1.100, seal methods that named `<Self as QueryParam>::Item<'a>` under `where Self: QueryParam` made the other associated types fail to normalize in impls (E0053, E0599), in M3.3 step 3. *Retire when:* the compiler normalizes that projection. *From:* M3.3 · *Confirmed:* 2026-11-19
- **ecs-4.** A `# Panics` or error doc that says the world (or a count) is unchanged after a failure is tested on the cold path, the first call of its kind, and the test asserts the panic's own message, not just that something panicked. *Why:* M3.4's `spawn_with` builds and caches its bundle's archetype before the allocator check, so a cold failure left a new archetype behind although the doc said "unchanged"; the only test warmed the cache first, and the cold test as first written passed on any panic (three reviewers). *From:* M3.4 · *Confirmed:* 2026-11-20
- **ecs-5.** A claim that a borrow handed out of a storage "cannot change a length" is checked against every method of what it hands out: `&mut dyn Trait` exposes all of the trait's methods, and `&mut Box<dyn Trait>` also lets the caller replace the box. Hand out a newtype that offers only slices instead. *Why:* M3.4's `Table::columns_mut` returned `&mut [Box<dyn ErasedColumn>]`, whose `swap_remove_*` methods let any crate code break T1 while the doc and spec said they could not (code review). *From:* M3.4 · *Confirmed:* 2026-11-20
